use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{Write, Read};
use std::time::{SystemTime, UNIX_EPOCH};
use base64::prelude::{Engine as _, BASE64_STANDARD};
#[cfg(target_os = "macos")]
use directories::ProjectDirs;
use p384::ecdsa::{Signature, VerifyingKey, signature::DigestVerifier};
use prost::Message;
use sha2::Digest;

use crate::error::{Error, LicensingError};
use crate::generated::software_licensor_client::{ClientSideDataStorage, ClientSideHwInfoStorage, LicenseActivationResponse, LicenseKeyFile};
use crate::api::{activate_license_request, get_pubkeys, EcdsaDigest};
use crate::{LicenseData, log_error, log_info};
use crate::generated::software_licensor_client::LicenseData as LicenseDataProto;

/// Gets the path to where the license file will be created.
fn get_license_file_path(company_name_str: &str) -> Result<PathBuf, Error> {
    #[cfg(target_os = "windows")]
    let dir_path = format!("C:\\ProgramData\\{}\\license.bin", company_name_str);
    #[cfg(target_os = "macos")]
    let dir_path = {
        if let Some(proj_dirs) = ProjectDirs::from("com", company_name_str, "Software Licensor") {
            proj_dirs.data_dir().join("license.bin")
        } else {
            "".into()
        }
    };
    #[cfg(target_os = "linux")]
    let dir_path = format!("{}/.local/share/{}/license.bin", std::env::var("HOME")?, company_name_str);
    #[cfg(target_os = "android")]
    let dir_path = format!("/data/data/{}/files/license.bin", company_name_str);
    
    // instead of panicking in this function, this will return a path that will
    // probably cause an error
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
    let dir_path = format!("/{}/license.bin", company_name_str);

    Ok(Path::new(&dir_path).to_owned())
}

/// Gets the path to where the machine info will be created.
fn get_machine_stats_path() -> Result<PathBuf, Error> {
    #[cfg(target_os = "windows")]
    let dir_path = format!("C:\\ProgramData\\HyperformanceSolutions\\hwinfo.bin");
    #[cfg(target_os = "macos")]
    let dir_path = {
        if let Some(proj_dirs) = ProjectDirs::from("com", "Hyperformance Solutions", "Software Licensor") {
            proj_dirs.data_dir().join("hwinfo.bin")
        } else {
            "".into()
        }
    };
    #[cfg(target_os = "linux")]
    let dir_path = format!("{}/.local/share/HyperformanceSolutions/hwinfo.bin", std::env::var("HOME")?);
    #[cfg(target_os = "android")]
    let dir_path = format!("/data/data/HyperformanceSolutions/files/hwinfo.bin");
    
    // instead of panicking in this function, this will return a path that will
    // probably cause an error
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
    let dir_path = format!("/HyperformanceSolutions/hwinfo.bin");

    Ok(Path::new(&dir_path).to_owned())
}

/// Gets the path to where the log file will be created.
pub(crate) fn get_log_file_path() -> Result<PathBuf, Error> {
    #[cfg(target_os = "windows")]
    return Ok(Path::new(&format!("C:\\ProgramData\\HyperformanceSolutions")).to_owned());
    #[cfg(target_os = "macos")]
    {
        if let Some(proj_dirs) = ProjectDirs::from("com", "Hyperformance Solutions", "Software Licensor") {
            return Ok(proj_dirs.data_dir().to_path_buf())
        } else {
            return Ok(Path::new("").into())
        }
    };
    #[cfg(target_os = "linux")]
    return Ok(Path::new(&format!("{}/.local/share/HyperformanceSolutions", std::env::var("HOME")?)).into());
    #[cfg(target_os = "android")]
    return Ok(Path::new(&format!("/data/data/HyperformanceSolutions/files")));
    
    // instead of panicking in this function, this will return a path that will
    // probably cause an error
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
    return Ok(Path::new(&format!("/HyperformanceSolutions")));
}

pub(crate) async fn get_or_init_license_file(company_name_str: &str, mut api_key: String) -> Result<ClientSideDataStorage, Error> {
    log_info!("Getting or initializing license file");
    let path = get_license_file_path(company_name_str)?;
    api_key.truncate(20);
    
    if path.exists() {
        log_info!("License file exists, trying to read it");
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        match ClientSideDataStorage::decode_length_delimited(buffer.as_slice()) {
            Ok(mut data_storage) => {
                // ensure that the next key exists before returning
                if data_storage.next_server_ecdh_key.is_none() {
                    get_pubkeys(&mut data_storage, true).await?;
                }
                if !data_storage.license_data.contains_key(&api_key) {
                    data_storage.license_data.insert(api_key.to_string(), LicenseDataProto {
                        license_activation_response: None,
                        license_code: "".to_string(),
                    });
                }
                log_info!("Successfully decoded license file");
                save_license_file(&data_storage, company_name_str)?;
                Ok(data_storage)
            },
            Err(_) => {
                log_error!("Failed to decode license file, initializing a new one");
                // need to initialize the file
                let mut license_data = HashMap::new();
                license_data.insert(api_key, LicenseDataProto {
                    license_activation_response: None,
                    license_code: "".to_string(),
                });
                let mut data_storage = ClientSideDataStorage {
                    license_data,
                    next_server_ecdh_key: None,
                    server_ecdsa_key: None,
                };
                get_pubkeys(&mut data_storage, true).await?;
                save_license_file(&data_storage, company_name_str)?;
                Ok(data_storage)
            }
        }
    } else {
        log_info!("License file does not exist, initializing a new one");
        // path does not exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut data_storage = ClientSideDataStorage {
            license_data: HashMap::new(),
            next_server_ecdh_key: None,
            server_ecdsa_key: None,
        };
        get_pubkeys(&mut data_storage, true).await?;
        save_license_file(&data_storage, company_name_str)?;
        log_info!("Successfully initialized license file");
        Ok(data_storage)
    }
}

pub(crate) async fn get_or_init_hw_info_file() -> Result<ClientSideHwInfoStorage, Error> {
    log_info!("Getting or initializing hw info file");
    let path = get_machine_stats_path()?;

    if path.exists() {
        log_info!("hw info file exists, trying to read it");
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        match ClientSideHwInfoStorage::decode_length_delimited(buffer.as_slice()) {
            Ok(stats) => {
                log_info!("Successfully decoded hw info file");

                Ok(stats)
            },
            Err(_) => {
                log_info!("Failed to decode hw info file, initializing a new one");
                let hw_info_storage = ClientSideHwInfoStorage {
                    machine_stats: None,
                };
                Ok(hw_info_storage)
            }
        }
    } else {
        log_info!("hw info file does not exist, initializing a new one");
        let hw_info_storage = ClientSideHwInfoStorage {
            machine_stats: None,
        };
        Ok(hw_info_storage)
    }
}

/// Saves the license file to the path (if the permissions are correct).
pub(crate) fn save_license_file(data_storage: &ClientSideDataStorage, company_name_str: &str) -> Result<(), Error> {
    log_info!("Saving license file");
    let path = get_license_file_path(company_name_str)?;
    log_info!("License path: {}", path.to_str().unwrap_or("Path is not valid unicode"));
    
    if !path.exists() {
        log_info!("License file does not exist, creating a new one");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // write the file
        let mut file = File::create_new(path)?;
        file.write_all(data_storage.encode_length_delimited_to_vec().as_slice())?;
    } else {
        log_info!("License file exists, overwriting it");
        let mut file = OpenOptions::new()
            .write(true)
            .append(false)
            .truncate(true)
            .open(path)?;
        file.write_all(data_storage.encode_length_delimited_to_vec().as_slice())?;
    }
    log_info!("Successfully saved license file");
    Ok(())
}

pub(crate) fn save_hw_info_file(data: &ClientSideHwInfoStorage) -> Result<(), Error> {
    log_info!("Saving hw info file");
    let path = get_machine_stats_path()?;
    log_info!("Saving hw info file to: {}", path.to_str().unwrap_or("Path is not valid unicode"));

    let contents = data.encode_length_delimited_to_vec();

    if !path.exists() {
        log_info!("hw info file does not exist, creating a new one");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create_new(path)?;
        file.write_all(&contents)?;
    } else {
        log_info!("hw info file exists, overwriting it");
        let mut file = OpenOptions::new()
            .write(true)
            .append(false)
            .truncate(true)
            .open(path)?;
        file.write_all(&contents)?;
    }
    log_info!("Successfully saved hw info file");
    Ok(())
}

/// Returns a `LicenseKeyFile` where `message_code = 1` is prioritized, but
/// are otherwise sorted by the `check_back_timestamp`, prioritizing higher 
/// timestamps. This allows for multiple product IDs to be set for when there 
/// might be bundled software as well as individual software.
/// 
/// # Errors
/// 
/// This function can only result in an `Error::LicensingError`, so the error number can be returned to the external code.
#[inline(always)]
pub(crate) fn get_latest_key_file(data_storage: &ClientSideDataStorage, product_ids: &Vec<&String>, mut api_key: String) -> Result<(LicenseKeyFile, Signature, LicenseActivationResponse), LicensingError> {
    log_info!("Getting latest key file for product ids: {:?}", product_ids);
    api_key.truncate(20);
    let license_data = match data_storage.license_data.get(&api_key) {
        Some(v) => v,
        None => return Err(LicensingError::NoLicenseFound("".into()))
    };
    
    let license_activation_response = match &license_data.license_activation_response {
        Some(v) => v,
        None => return Err(LicensingError::NoLicenseFound("".into()))
    };
    // get all license key files for the valid product ids. These product ids 
    // could include bundled products as opposed to just the individual product
    let mut found_key_files = Vec::new();
    for product_id in product_ids {
        let key_file = match &license_activation_response.key_files.get(*product_id) {
            Some(v) => *v,
            None => continue
        };
        found_key_files.push(key_file);
    }
    if found_key_files.len() == 1 {
        let key_file = found_key_files[0];
        let product_id = &key_file.product_id;
        let sig_bytes = match license_activation_response.key_file_signatures.get(product_id) {
            Some(v) => v,
            None => {
                log_error!("Failed to find signature for product id: {}", product_id);
                return Err(LicensingError::NoLicenseFound(key_file.license_code.clone()))
            }
        };
        let signature: Signature = match Signature::from_bytes(sig_bytes.as_slice().into()) {
            Ok(v) => v,
            Err(_) => {
                log_error!("Failed to create signature from bytes");
                return Err(LicensingError::NoLicenseFound(key_file.license_code.clone()))
            }
        };
        return Ok((key_file.clone(), signature, license_activation_response.clone()))
    }
    if found_key_files.is_empty() {
        let errors = &license_activation_response.licensing_errors;
        let mut error_codes = Vec::with_capacity(errors.len());
        errors.iter().for_each(|(k,v)| {
            if product_ids.contains(&k) {
                error_codes.push(*v);
            }
        });
        log_error!("No key files found for the product ids. Licensing error codes for the product ids: {:?}", error_codes);
        if error_codes.is_empty() {

            return Err(LicensingError::NoLicenseFound(license_data.license_code.clone()))
        }
        // prioritizing specific licensing errors over others
        if error_codes.contains(&4) { // machine limit reached
            return Err(LicensingError::MachineLimitReached(license_data.license_code.clone()))
        }
        if error_codes.contains(&16) { // license no longer active
            return Err(LicensingError::LicenseNoLongerActive(license_data.license_code.clone()))
        }
        if error_codes.contains(&8) { // trial ended
            return Err(LicensingError::TrialEnded(license_data.license_code.clone()))
        }
        return Err(LicensingError::from((error_codes[0], license_data.license_code.clone())))
    }
    found_key_files.sort_unstable_by(|a, b| {
        let a_success = a.message_code == 1;
        let b_success = b.message_code == 1;

        match (a_success, b_success) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.check_back_timestamp.cmp(&a.check_back_timestamp)
        }
    });
    let key_file = found_key_files[0];
    let product_id = &key_file.product_id;
    let sig_bytes = match license_activation_response.key_file_signatures.get(product_id) {
        Some(v) => v,
        None => return Err(LicensingError::NoLicenseFound(key_file.license_code.clone()))
    };
    let signature: Signature = match Signature::from_bytes(sig_bytes.as_slice().into()) {
        Ok(v) => v,
        Err(_) => return Err(LicensingError::NoLicenseFound(key_file.license_code.clone()))
    };
    Ok((key_file.clone(), signature, license_activation_response.clone()))
}

/// Removes key files so that we don't keep automatically checking up
/// on them.
#[inline(always)]
pub(crate) fn remove_key_files(license_file: &mut ClientSideDataStorage, product_ids: &Vec<&String>, company_name_str: &str, mut api_key: String) {
    log_info!("Removing key files for product ids: {:?}", product_ids);
    api_key.truncate(20);
    let license_data = match license_file.license_data.get_mut(&api_key) {
        Some(v) => v,
        None => return
    };
    let mut license_response = match &license_data.license_activation_response {
        Some(v) => v.clone(),
        None => return
    };
    for product_id in product_ids {
        license_response.key_files.remove(*product_id);
        license_response.key_file_signatures.remove(*product_id);
        license_response.licensing_errors.remove(*product_id);
    }
    license_data.license_activation_response = Some(license_response);
    save_license_file(license_file, company_name_str).unwrap_or_else(|_| ());
}

/// Handles licensing errors by removing key files before returning the error
#[inline(always)]
pub(crate) fn handle_licensing_error(license_file: &mut ClientSideDataStorage, product_ids: &Vec<&String>, company_name_str: &str, licensing_error: LicensingError, api_key: String) -> Error {
    remove_key_files(license_file, product_ids, company_name_str, api_key);
    licensing_error.into()
}

/// Checks the key file and verifies the signature, while also checking 
/// for updates from the cloud, and if the key file is expired.
/// 
/// # Arguments
/// 
/// - `license_file`: The license file to use, if you have access to it already.
/// If you don't already have access to the license file, you can pass in `None`.
/// - `store_id`: The API Key/Store ID.
/// - `company_name_str`: The company name, which is used to name the directory 
/// for the license file.
/// - `product_ids_and_pubkeys`: A hashmap of product IDs to their corresponding 
/// publick keys, which is used to verify the signature on the key file.
/// - `machine_id`: The machine ID, which is used to check if the license file 
/// is being used on the correct machine.
/// - `should_send_request`: Whether to send a request to the cloud to check for 
/// an updated license.
/// - `api_key`: The API key, aka the store ID.
/// 
/// # Returns
/// 
/// Returns Ok(LicenseData) only if the license is active.
/// 
/// Returns Err(LicensingError) if the license is inactive, including the 
/// user's license code for reference or saving into the license file.
#[inline(always)]
pub(crate) async fn check_key_file_async(
    license_file: Option<&mut ClientSideDataStorage>, 
    store_id: &str, 
    company_name_str: &str, 
    product_ids_and_pubkeys: &HashMap<String, String>, 
    machine_id: &str, 
    should_send_request: bool, 
    api_key: String,
    send_computer_name: bool,
) -> Result<LicenseData, Error> {
    let mut license_file = match license_file {
        Some(file) => file,
        None => {
            &mut match get_or_init_license_file(company_name_str, api_key.clone()).await {
                Ok(v) => v,
                Err(e) => {
                    log_error!("Failed to get or initialize license file: {}", e);
                    return Err(Error::IoError)
                }
            }
        }
    };
    let mut trimmed_api_key = api_key.clone();
    trimmed_api_key.truncate(20);
    let license_data = match license_file.license_data.get_mut(&trimmed_api_key) {
        Some(v) => v,
        None => {
            log_error!("API key not found in license file. API Key: {}", trimmed_api_key);
            log_error!("License data keys: {:?}", license_file.license_data.keys());
            return Err(Error::LicensingError((2, "".to_string()).into()))
        }
    };
    let license_code: String = match license_data.license_code.len() < 16 {
        true => {
            log_error!("License code in license file is less than 16 chars: {}", license_data.license_code);
            return Err(Error::LicensingError((2, license_data.license_code.clone()).into()))
        },
        false => license_data.license_code.clone()
    }.to_owned();
    let product_ids: Vec<&String> = product_ids_and_pubkeys.keys().collect();
    let (mut key_file, mut signature, mut license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone()) {
        Ok(v) => v,
        Err(licensing_error) => {
            log_error!("Failed to get latest key file: {:?}", licensing_error.get_error_and_license_codes());
            return Err(Error::LicensingError(licensing_error))
        }
    };
    if key_file.message_code != 1 {
        log_error!("key_file.message code != 1: {}", key_file.message_code);
        return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.message_code as i32));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if key_file.expiration_timestamp < now {
        log_error!("key file expired at {}, now is {}", key_file.expiration_timestamp, now);
        if !should_send_request {
            return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32));
        }
        // send request to check for an update
        log_info!("Sending request to check for an update since the key file is expired");
        match activate_license_request(store_id, company_name_str, &product_ids, machine_id, &license_code, &mut license_file, send_computer_name).await {
            Ok(_) => (),
            Err(e) => {
                log_error!("Failed to activate license: {}", e);
                return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32))
            }
        }
        (key_file, signature, license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone()) {
            Ok(v) => v,
            Err(licensing_error) =>  {
                log_error!("Failed to get latest key file after sending request");
                log_error!("Licensing error: {:?}", licensing_error.get_error_and_license_codes().0);
                return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32))
            }
        };
        if key_file.message_code != 1 && key_file.message_code < 512 {
            log_error!("After sending request, key_file.message code is still not 1: {}", key_file.message_code);
            return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32));
        }
        if key_file.message_code >= 512 {
            log_error!("Error code is >= 512: {}", key_file.message_code);
            return Err(
                handle_licensing_error(
                    &mut license_file, 
                    &product_ids, 
                    company_name_str, 
                    LicensingError::UnknownError((
                        key_file.message_code, 
                        format!("Unknown error: {}", key_file.message)
                    )), 
                    api_key
                ))
        }
        if key_file.expiration_timestamp < now {
            log_error!("Key file expired at {}, now is {}", key_file.expiration_timestamp, now);
            let err = if key_file.post_expiration_error_code == 16 {
                LicensingError::LicenseNoLongerActive(license_code)
            } else {
                LicensingError::TrialEnded(license_code)
            };
            return Ok(LicenseData::error(&Error::LicensingError(err)))
        }
    }
    if key_file.check_back_timestamp < now && should_send_request {
        // send request
        log_info!("Key file check back timestamp is {}, now is {}, sending request to check for an update", key_file.check_back_timestamp, now);
        if let Ok(_) = activate_license_request(store_id, company_name_str, &product_ids, machine_id, &license_code, &mut license_file, send_computer_name).await {
            (key_file, signature, license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone()) {
                Ok(v) => v,
                Err(licensing_error) => {
                    log_error!("activate_license_request failed when check back timestmap was less than now");
                    return Ok(LicenseData::error(&Error::LicensingError(licensing_error)));
                }
            }
        }
    }

    if machine_id.ne(&key_file.machine_id) {
        log_error!("Machine ID does not match key file machine ID");

        remove_key_files(&mut license_file, &product_ids, company_name_str, api_key);
        return Err(LicensingError::NoLicenseFound(license_code).into())
    }
    
    // verify signature on the key file
    let pubkey_b64 = match product_ids_and_pubkeys.get(&key_file.product_id) {
        Some(v) => v,
        None => {
            log_error!("Product ID not found in public keys");
            return Err(LicensingError::NoLicenseFound(license_code).into())
        }
    };
    let decoded_pubkey = match BASE64_STANDARD.decode(pubkey_b64) {
        Ok(v) => v,
        Err(e) => {
            log_error!("Failed to decode public key: {}", e);
            return Err(LicensingError::NoLicenseFound(license_code).into())
        }
    };

    let bytes = key_file.encode_length_delimited_to_vec();
    let verifying_key = match VerifyingKey::from_sec1_bytes(&decoded_pubkey) {
        Ok(v) => v,
        Err(e) => {
            log_error!("Failed to parse verifying key from developer supplied public key: {}", e);
            remove_key_files(&mut license_file, &product_ids, company_name_str, api_key);
            return Err(LicensingError::NoLicenseFound(license_code).into())
        }
    };
    match verifying_key.verify_digest(EcdsaDigest::new_with_prefix(bytes), &signature) {
        Ok(_) => Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.message_code as i32)),
        Err(_) => {
            log_error!("Failed to verify signature on key file");
            remove_key_files(&mut license_file, &product_ids, company_name_str, api_key);
            Err(LicensingError::NoLicenseFound(license_code).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::generated::software_licensor_client::LicenseActivationResponse;

    use super::*;

    #[tokio::test]
    async fn key_file_ordering() {
        let mut data_storage = get_or_init_license_file("software_licensor_test_company", "ABCDEFGHIJKL".to_string()).await.expect("This should succeed unless the file is lacking permissions");

        let license_data = data_storage.license_data.get_mut("ABCDEFGHIJKL").unwrap();

        let mut license_response = LicenseActivationResponse { 
            key_files: HashMap::new(), 
            customer_first_name: "".to_string(), 
            customer_last_name: "".to_string(), 
            customer_email: "".to_string(), 
            licensing_errors: HashMap::new(), 
            key_file_signatures: HashMap::new()
        };

        let expired_product_id = "expired_product_id".to_string();
        let recent_but_inactive_product_id = "recent_product_id_but_inactive".to_string();
        let newest_product_id = "newest_product_id".to_string();
        let product_ids: Vec<&String> = vec![
            &expired_product_id,
            &recent_but_inactive_product_id,
            &newest_product_id,
        ];
        license_response.key_files.insert(
            product_ids[0].to_string(),
            LicenseKeyFile { 
                product_id: product_ids[0].to_string(), 
                product_version: "1.0".to_string(), 
                license_code: "A".to_string(), 
                license_type: "trial".to_string(), 
                machine_id: "A".to_string(), 
                timestamp: 0, 
                expiration_timestamp: 5000, 
                check_back_timestamp: 3000, 
                message: "".to_string(), 
                message_code: 8, 
                post_expiration_error_code: 0, 
                current_machine_count: None,
                current_machine_limit: None,
            }
        );
        license_response.key_files.insert(
            product_ids[1].to_string(),
            LicenseKeyFile { 
                product_id: product_ids[1].to_string(), 
                product_version: "1.0".to_string(), 
                license_code: "A".to_string(), 
                license_type: "trial".to_string(), 
                machine_id: "A".to_string(), 
                timestamp: 0, 
                expiration_timestamp: 8000, 
                check_back_timestamp: 6000, 
                message: "".to_string(), 
                message_code: 8, 
                post_expiration_error_code: 0,
                current_machine_count: None,
                current_machine_limit: None,
            }
        );
        license_response.key_files.insert(
            product_ids[2].to_string(),
            LicenseKeyFile { 
                product_id: product_ids[2].to_string(), 
                product_version: "1.0".to_string(), 
                license_code: "A".to_string(), 
                license_type: "trial".to_string(), 
                machine_id: "A".to_string(), 
                timestamp: 0, 
                expiration_timestamp: 6000, 
                check_back_timestamp: 5000, 
                message: "".to_string(), 
                message_code: 1, 
                post_expiration_error_code: 0,
                current_machine_count: None,
                current_machine_limit: None,
            }
        );

        license_response.key_file_signatures.insert(product_ids[0].to_string(), vec![5u8;96]);
        license_response.key_file_signatures.insert(product_ids[1].to_string(), vec![5u8;96]);
        license_response.key_file_signatures.insert(product_ids[2].to_string(), vec![5u8;96]);

        license_data.license_activation_response = Some(license_response);

        let newest_key_file = get_latest_key_file(&data_storage, &product_ids.clone(), "ABCDEFGHIJKL".to_string()).expect("Possibly lacking file read permissions").0;

        assert_eq!("newest_product_id", newest_key_file.product_id);

        let newest_key_file = get_latest_key_file(&data_storage, &product_ids.clone(), "ABCDEFGHIJKL".to_string()).expect("Possibly lacking file read permissions").0;

        assert_eq!("newest_product_id", newest_key_file.product_id);
    }
}