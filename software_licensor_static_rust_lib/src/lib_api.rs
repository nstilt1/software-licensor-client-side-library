use crate::stats::get_machine_stats;
use crate::{file_io::check_key_file_async, log_info, status_messages::get_status_message_from_code};
use crate::LicenseData;
use std::collections::HashMap;
use crate::file_io::{get_or_init_license_file, get_or_init_hw_info_file, save_hw_info_file};
use tokio::time::sleep;
use std::time::Duration;
pub use crate::stats::{StatsDisplay, get_machine_stats_for_display};

/// A semantic version for some software. Can be constructed via `From` or `Into` 
/// from `&str` in the format:
/// * major (e.g. `1`)
/// * major.minor (e.g. `1.2`)
/// * major.minor.patch (e.g. `1.2.3`)
/// * major.minor.patch.build (e.g. `1.2.3.4`)
#[derive(Debug, Clone)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: u32,
}

impl From<&str> for SemanticVersion {
    fn from(s: &str) -> Self {
        let parts: Vec<&str> = s.split('.').collect();
        let major = parts.get(0).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let build = parts.get(3).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        Self { major, minor, patch, build}
    }
}

impl PartialEq for SemanticVersion {
    fn eq(&self, other: &Self) -> bool {
        if self.major == 0 && self.major == self.minor && self.minor == self.patch {
            return true;
        }
        if other.major == 0 && other.major == other.minor && other.minor == other.patch {
            return true;
        }
        self.major == other.major && self.minor == other.minor && self.patch == other.patch && self.build == other.build
    }
}

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.major == 0 && self.major == self.minor && self.minor == self.patch {
            return self.major.partial_cmp(&self.minor)
        }
        if other.major == 0 && other.major == other.minor && other.minor == other.patch {
            return other.major.partial_cmp(&other.minor)
        }
        if self.major != other.major {
            return self.major.partial_cmp(&other.major);
        }
        if self.minor != other.minor {
            return self.minor.partial_cmp(&other.minor);
        }
        if self.patch != other.patch {
            return self.patch.partial_cmp(&other.patch);
        }
        self.build.partial_cmp(&other.build)
    }
}

/// A license status data structure that contains information about the store.
/// 
/// This information is used to check where to look for the license file, and 
/// which products and public keys to use when checking the license. Several 
/// methods are implemented on this struct to check the license.
pub struct LicenseStatus {
    pub store_id: String,
    pub company_name: String,
    pub product_ids_and_pubkeys: HashMap<String, String>,
    pub send_computer_name: bool,
}

impl LicenseStatus {
    /// Initializes a new LicenseStatus with the given store ID.
    /// 
    /// Also runs `check_license()` during initialization to populate the 
    /// license data, and returns an error message in the license data if there 
    /// was an error during initialization.
    /// 
    /// `send_computer_name` is here to determine whether the store wants to 
    /// force the clients to send their computer name to the cloud. This will be 
    /// used for the user to view which computers are activated on their license.
    pub async fn new(store_id: &str, company_name: &str, product_ids_and_pubkeys: HashMap<String, String>, send_computer_name: bool) -> (Self, LicenseData) {
        #[cfg(feature = "logging")]
        {
            use crate::inner::init_logger;

            if let Ok(log_path) = init_logger() {
                log_info!("file logging initialized at LicenseStatus::new(): {}", log_path.display());
            } else {
                use crate::log_error;

                log_error!("failed to initialize file logging at LicenseStatus::new()");
            }
        }
        let result = Self {
            store_id: store_id.to_string(),
            company_name: company_name.to_string(),
            product_ids_and_pubkeys,
            send_computer_name,
        };
        let license_data = match result.check_license(true).await {
            Ok((_is_unlocked, license_data, _license_code)) => license_data,
            Err(e) => e.1,
        };
        (result, license_data)
    }
    /// Checks if the license is unlocked without making an API request. This 
    /// is a quick check that can be used to determine if the license is unlocked.
    #[inline(always)]
    pub async fn is_unlocked(&self) -> bool {
        log_info!("Running is_unlocked check");
        check_key_file_async(None, &self.store_id, &self.company_name, &self.product_ids_and_pubkeys, &super::stats::device_id(), false, self.store_id.clone(), self.send_computer_name).await.is_ok()
    }
    /// Gets the error message corresponding to the license status code, using 
    /// the appropriate language based on the machine's stats. This is a user-friendly
    /// error message that can be displayed to the user if the license is not valid.
    pub fn get_error_message_from_error_code(&self, codes: i32) -> String {
        get_status_message_from_code(codes)
    }
    /// Checks the license, potentially making an API request to the webserver 
    /// if the bool is true and if necessary.
    /// 
    /// Returns Ok(true, licenseData, licenseCode) if the license is valid and unlocked, or Ok(false) or 
    /// Err((error_message, license_code)) with an error message if the license is not valid.
    #[inline(always)]
    pub async fn check_license(&self, should_check_cloud: bool) -> Result<(bool, LicenseData, String), (bool, LicenseData)> {
        log_info!("Running check_license(should_check_cloud = {})", should_check_cloud);
        let (license_data, success) = match check_key_file_async(None, &self.store_id, &self.company_name, &self.product_ids_and_pubkeys, &super::stats::device_id(), should_check_cloud, self.store_id.clone(), self.send_computer_name).await {
            Ok(v) => (v, true),
            Err(e) => {
                let license_data = match e {
                    crate::Error::LicensingError(e) => LicenseData::licensing_error(&e),
                    _ => LicenseData::error(e, "")
                };
                (license_data, false)
            },
        };
        if success {
            Ok((success, license_data.clone(), license_data.license_code.clone()))
        } else {
            Err((success, license_data))
        }
    }

    /// Reads the reply from the webserver after attempting to activate the license.
    #[inline(always)]
    pub async fn read_reply_from_webserver(&self, license_code: &str, save_system_stats: bool) -> Result<(bool, LicenseData), String> {
        let result = match read_reply_from_webserver(&self.company_name, &self.store_id, license_code, &self.product_ids_and_pubkeys, save_system_stats, self.send_computer_name).await {
            Ok(v) => v,
            Err(e) => return Err(e.to_string()),
        };
        if !result.0 {
            return Err("License activation failed. Please check your internet connection and try again.".to_string());
        }
        Ok(result)
    }

    /// Checks if an update is available by comparing the current version with 
    /// the version in the license data.
    /// 
    /// If an empty string is supplied as the current version or if the version 
    /// in the license data is an empty string, this function will return false 
    /// so that no update is thought to be available, since an empty version string 
    /// is uninitialized and should not be treated as a valid version.
    pub fn is_update_available(&self, current_version: &str, license_data: &LicenseData) -> bool {
        let current_version: SemanticVersion = current_version.into();
        let cloud_version: SemanticVersion = license_data.version.as_str().into();
        cloud_version > current_version
    }

    /// Gets the current system information that is already stored in the cloud.
    pub async fn get_current_system_information_that_is_stored_in_cloud(&self) -> StatsDisplay {
        match get_or_init_hw_info_file().await {
            Ok(v) => {
                v.machine_stats.unwrap_or_default().into()
            },
            Err(_) => StatsDisplay::default(),
        }
    }

    /// Erases the current system information that is stored in the cloud.
    /// 
    /// The way this works is, the data in the hardware info file is sent to the 
    /// cloud during license activation, which is done repeatedly over time. The 
    /// client will send a copy of whatever is currently in the hardware info 
    /// file, so by erasing the machine stats, the next time the client sends a 
    /// request, the cloud will erase the machine stats that it has stored for 
    /// this machine since the client is now sending an empty machine stats. 
    /// This is a way to erase the machine stats from the cloud without having 
    /// to make a specific API request to erase the machine stats, since there 
    /// isn't currently an API endpoint to specifically erase the machine stats.
    pub async fn erase_cloud_hardware_info(&self) -> Result<(), String> {
        let mut hw_info_file = match get_or_init_hw_info_file().await {
            Ok(v) => v,
            Err(e) => return Err(e.to_string())
        };
        hw_info_file.machine_stats = None;
        match save_hw_info_file(&hw_info_file) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Updates the machine info file with the latest machine stats, if the bool is 
/// set to true. If the bool is set to false, the machine stats are removed from 
/// the machine info file.
/// 
/// # Safety
/// 
/// This is the only function that uses the `unsafe fn get_machine_stats()`, 
/// which collects various hardware information about the machine. This function 
/// is marked as unsafe because it collects hardware information that could potentially
/// be used to uniquely identify a machine, and therefore could be a privacy concern.
#[inline(always)]
async unsafe fn update_machine_info(save_system_stats: bool) {
    let mut hw_info_file = match get_or_init_hw_info_file().await {
        Ok(v) => v,
        Err(_) => return
    };

    // Safety: `current_stats` is none when save_system_stats is false.
    unsafe {
        let current_stats = get_machine_stats(save_system_stats);
        assert!(
            (current_stats.is_none() && !save_system_stats) || 
            (current_stats.is_some() && save_system_stats), 
            "get_machine_stats should return None if save_system_stats is false, and Some if save_system_stats is true");
        if hw_info_file.machine_stats.ne(&current_stats) {
            hw_info_file.machine_stats = current_stats;
            // Silent error handling since we don't want to cause any issues 
            // for the user if we fail to save the machine stats.
            let _result = save_hw_info_file(&hw_info_file).unwrap_or_else(|_| ());
        }
    }
}

/// Reads the reply from the webserver after attempting to activate the license.
#[inline(always)]
async fn read_reply_from_webserver(company_name: &str, store_id: &str, license_code: &str, product_ids_and_pubkeys: &HashMap<String, String>, save_system_stats: bool, send_computer_name: bool) -> Result<(bool, LicenseData), String> {
    // Safety: This function is called while using save_system_stats.
    unsafe {
        update_machine_info(save_system_stats).await;
    }
    let mut license_file = match get_or_init_license_file(company_name, store_id.to_string()).await {
        Ok(v) => v,
        Err(e) => return Err(e.to_string())
    };

    let machine_id = super::stats::device_id();

    match crate::api::activate_license_request(
        store_id, 
        company_name, 
        &product_ids_and_pubkeys.keys().collect::<Vec<&String>>(),
        &machine_id, 
        license_code,
        &mut license_file,
        send_computer_name,
    ).await {
        Ok(_) => (),
        Err(e) => {
            sleep(Duration::from_secs(5)).await;
            return Err(e.to_string())
        }
    }
    match check_key_file_async(
        Some(&mut license_file),
        store_id, 
        company_name, 
        &product_ids_and_pubkeys, 
        &super::stats::device_id(),
        false,
        store_id.to_string(),
        send_computer_name,
    ).await {
        Ok(v) => return Ok((true, v)),
        Err(e) => {
            sleep(Duration::from_secs(5)).await;
            return Err(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_versioning() {
        let v0: SemanticVersion = "0.1.9".into();
        let v1: SemanticVersion = "1.2.3".into();
        let v2: SemanticVersion = "1.2.4".into();
        let v3: SemanticVersion = "1.3.0".into();
        let v4: SemanticVersion = "2.0.0".into();
        let v5: SemanticVersion = "1.2.3".into();

        let v6: SemanticVersion = "2".into();
        let v7: SemanticVersion = "2.1".into();
        let v8: SemanticVersion = "2.1.0.1".into();

        assert!(v0 < v1);
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v1 == v5);
        
        assert!(v6 < v7);
        assert!(v6 == v4);
        assert!(v6 < v8);
        assert!(v7 < v8);

        let v0: SemanticVersion = "".into();
        let all_0s: SemanticVersion = "0.0.0.0".into();

        assert!(v0 == all_0s);
        assert!(v0 == v1);
        assert!(v0 == v5);
        assert!(v5 == v0);
    }
}