use crate::{file_io::check_key_file_async, generated::software_licensor_client::Stats, stats::Language};
use std::env::consts::{OS, ARCH};
use crate::LicenseData;
use std::collections::HashMap;
use crate::file_io::{get_or_init_license_file, get_or_init_hw_info_file, save_hw_info_file};
use tokio::time::sleep;
use std::time::Duration;

const ENGLISH_STATUS_MESSAGES: &[&str] = &[
    "License is valid and unlocked.",
    "License not found.",
    "Machine limit reached. You can regenerate your license code to remove old machines from your license.",
    "Your trial has ended. Please purchase a license to continue using all the features.",
    "Your license is inactive. Please renew your license to continue using all the features.",
    "The offline code was incorrect.",
    "Offline codes are disabled for this software.",
    "The license code is invalid.",
    "This machine has been deactivated. Please enter another license code.",
    "Unknown error. Please contact support with the error code to resolve this issue.",
];

const FRENCH_STATUS_MESSAGES: &[&str] = &[
    "La licence est valide et déverrouillée.",
    "Licence introuvable.",
    "Limite de machines atteinte. Vous pouvez régénérer votre code de licence pour supprimer les anciennes machines de votre licence.",
    "Votre période d’essai est terminée. Veuillez acheter une licence pour continuer à utiliser toutes les fonctionnalités.",
    "Votre licence est inactive. Veuillez renouveler votre licence pour continuer à utiliser toutes les fonctionnalités.",
    "Le code hors ligne est incorrect.",
    "Les codes hors ligne sont désactivés pour ce logiciel.",
    "Le code de licence est invalide.",
    "Cette machine a été désactivée. Veuillez saisir un autre code de licence.",
    "Erreur inconnue. Veuillez contacter le support avec le code d’erreur afin de résoudre ce problème.",
];

const SPANISH_STATUS_MESSAGES: &[&str] = &[
    "La licencia es válida y está desbloqueada.",
    "Licencia no encontrada.",
    "Se alcanzó el límite de dispositivos. Puede regenerar su código de licencia para eliminar los dispositivos antiguos de su licencia.",
    "Su período de prueba ha finalizado. Por favor, compre una licencia para seguir usando todas las funciones.",
    "Su licencia está inactiva. Por favor, renueve su licencia para seguir usando todas las funciones.",
    "El código sin conexión es incorrecto.",
    "Los códigos sin conexión están deshabilitados para este software.",
    "El código de licencia no es válido.",
    "Este dispositivo ha sido desactivado. Por favor, introduzca otro código de licencia.",
    "Error desconocido. Por favor, contacte con soporte con el código de error para resolver este problema.",
];

const SUPPORTED_LANGUAGES: &[&str] = &[
    "en",
    "fr",
    "es",
];

#[cfg_attr(test, derive(Default))]
pub struct LicenseStatus {
    pub store_id: String,
    pub company_name: String,
    pub product_ids_and_pubkeys: HashMap<String, String>,
}

/// Normalizes a locale string.
fn normalize_locale(s: &str) -> String {
    let mut s = s.trim().to_lowercase();

    // Remove encoding (e.g. .UTF-8)
    if let Some(idx) = s.find('.') {
        s.truncate(idx);
    }

    // Remove modifiers (e.g. @euro)
    if let Some(idx) = s.find('@') {
        s.truncate(idx);
    }

    // Normalize separator
    s = s.replace('_', "-");

    s
}
/// Extracts the base language from a locale string (e.g. "en" from "en-US").
fn base_language(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}
/// Gets the status message corresponding to a status code and language.
pub(crate) fn get_status_message_from_code(code: i32) -> String {
    let language = super::stats::language();
    let display_language_normalized = normalize_locale(&language.display_language);
    let users_language_normalized = normalize_locale(&language.users_language);
    let base_language_display_lang = base_language(&display_language_normalized);
    let base_language_users_lang = base_language(&users_language_normalized);
    let language = if language.display_language.len() != 0 && SUPPORTED_LANGUAGES.contains(&base_language_display_lang) {
        &base_language_display_lang
    } else if language.users_language.len() != 0 && SUPPORTED_LANGUAGES.contains(&base_language_users_lang) {
        &base_language_users_lang
    } else {
        "en"
    };
    let messages = match language {
        "en" => ENGLISH_STATUS_MESSAGES,
        "fr" => FRENCH_STATUS_MESSAGES,
        "es" => SPANISH_STATUS_MESSAGES,
        _ => ENGLISH_STATUS_MESSAGES,
    };

    for m in 0..8 {
        if code & (1 << m) != 0 {
            if let Some(msg) = messages.get(m) {
                return msg.to_string();
            }
        }
    }
    messages.last().unwrap_or(&"Unknown status code. Please contact support.").to_string()
}

/// A semantic version for some software. Can be constructed via `From` or `Into` 
/// from `&str` in the format:
/// * major (e.g. `1`)
/// * major.minor (e.g. `1.2`)
/// * major.minor.patch (e.g. `1.2.3`)
/// * major.minor.patch.build (e.g. `1.2.3.4`)
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
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

impl LicenseStatus {
    /// Initializes a new LicenseStatus with the given store ID.
    /// 
    /// Also runs `check_license()` during initialization to populate the 
    /// license data, and returns an error message in the license data if there was an error during initialization.
    pub async fn new(store_id: &str, company_name: &str, product_ids_and_pubkeys: HashMap<String, String>) -> (Self, LicenseData) {
        let result = Self {
            store_id: store_id.to_string(),
            company_name: company_name.to_string(),
            product_ids_and_pubkeys,
        };
        let license_data = match result.check_license(true).await {
            Ok((is_unlocked, license_data)) => license_data,
            Err(e) => LicenseData::error(&e),
        };
        (result, license_data)
    }
    /// Checks if the license is unlocked without making an API request. This 
    /// is a quick check that can be used to determine if the license is unlocked.
    #[inline(always)]
    pub async fn is_unlocked(&self) -> bool {
        check_key_file_async(None, &self.store_id, &self.company_name, &self.product_ids_and_pubkeys, &super::stats::device_id(), false, self.store_id.clone()).await.is_ok()
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
    /// Returns Ok(true) if the license is valid and unlocked, or Ok(false) or 
    /// Err(String) with an error message if the license is not valid.
    #[inline(always)]
    pub async fn check_license(&self, should_check_cloud: bool) -> Result<(bool, LicenseData), String> {
        let (license_data, success) = match check_key_file_async(None, &self.store_id, &self.company_name, &self.product_ids_and_pubkeys, &super::stats::device_id(), should_check_cloud, self.store_id.clone()).await {
            Ok(v) => (v, true),
            Err(e) => (LicenseData::error(&e.to_string()), false),
        };
        if success {
            Ok((success, license_data))
        } else {
            Err(license_data.error_message)
        }
    }

    /// Reads the reply from the webserver after attempting to activate the license.
    pub async fn read_reply_from_webserver(&self, license_code: &str, save_system_stats: bool) -> Result<(bool, LicenseData), String> {
        let result = match read_reply_from_webserver(&self.company_name, &self.store_id, license_code, &self.product_ids_and_pubkeys, save_system_stats).await {
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
    pub fn is_update_available(&self, current_version: &str, license_data: &LicenseData) -> bool {
        let current_version: SemanticVersion = current_version.into();
        let cloud_version: SemanticVersion = license_data.version.as_str().into();
        cloud_version > current_version
    }
}

/// Detects if the current machine supports a given x86 feature.
macro_rules! detect_x86_feature {
    ($feature:literal) => {{
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            std::is_x86_feature_detected!($feature)
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            false
        }
    }};
}

/// Detects if the current machine supports a given ARM/aarch64 feature.
macro_rules! detect_arm_feature {
    ($feature:literal) => {{
        #[cfg(target_arch = "arm")]
        {
            std::arch::is_arm_feature_detected!($feature)
        }
        #[cfg(target_arch = "aarch64")]
        {
            true
        }
        #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
        {
            false
        }
    }};
}

/// Gets the page size of the current machine.
fn get_page_size() -> Option<u32> {
    #[cfg(unix)]
    {
        let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if v <= 0 {
            None
        } else {
            u32::try_from(v).ok()
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

        unsafe {
            let mut info = std::mem::zeroed::<SYSTEM_INFO>();
            GetSystemInfo(&mut info);
            Some(info.dwPageSize)
        }
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        None
    }
}

/// Returns the current machine's stats. This includes various hardware 
/// information about the machine, such as the CPU vendor, model, number of 
/// cores, amount of RAM, and various CPU features.
/// 
/// # Safety
/// 
/// This function collects hardware information that could potentially be used to
/// uniquely identify a machine, and therefore could be a privacy concern. It is 
/// the caller's responsibility to ensure that this function is only called when 
/// the user has explicitly opted in to the collection of this information.
#[inline(always)]
pub(crate) unsafe fn get_machine_stats(save_system_stats: bool) -> Option<Stats> {
    if !save_system_stats {
        return None;
    }
    unsafe {
        let s = super::stats::Stats::collect();

        return Some(Stats { 
            os_name: OS.to_string(), 
            computer_name: s.computer_name, 
            is_64_bit: if size_of::<usize>() == 8 { true } else { false }, 
            users_language: s.users_language, 
            display_language: s.display_language, 
            num_logical_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32, 
            num_physical_cores: s.num_physical_cores, 
            cpu_freq_mhz: s.cpu_freq_mhz, 
            cpu_architecture: ARCH.to_string(), 
            ram_mb: s.ram_mb, 
            page_size: get_page_size().unwrap_or(0), 
            cpu_vendor: s.cpu_vendor, 
            cpu_model: s.cpu_model, 
            has_mmx: detect_x86_feature!("mmx"),
            has_3d_now: false,
            has_fma3: detect_x86_feature!("fma"),
            has_fma4: false,
            has_sse: detect_x86_feature!("sse"),
            has_sse2: detect_x86_feature!("sse2"),
            has_sse3: detect_x86_feature!("sse3"),
            has_ssse3: detect_x86_feature!("ssse3"),
            has_sse41: detect_x86_feature!("sse4.1"),
            has_sse42: detect_x86_feature!("sse4.2"),
            has_avx: detect_x86_feature!("avx"),
            has_avx2: detect_x86_feature!("avx2"),
            has_avx512f: detect_x86_feature!("avx512f"),
            has_avx512bw: detect_x86_feature!("avx512bw"),
            has_avx512cd: detect_x86_feature!("avx512cd"),
            has_avx512dq: detect_x86_feature!("avx512dq"),
            has_avx512er: detect_x86_feature!("avx512er"),
            has_avx512ifma: detect_x86_feature!("avx512ifma"),
            has_avx512pf: detect_x86_feature!("avx512pf"),
            has_avx512vbmi: detect_x86_feature!("avx512vbmi"),
            has_avx512vl: detect_x86_feature!("avx512vl"),
            has_avx512vpopcntdq: detect_x86_feature!("avx512vpopcntdq"),
            has_neon: detect_arm_feature!("neon"),
        })
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
async fn read_reply_from_webserver(company_name: &str, store_id: &str, license_code: &str, product_ids_and_pubkeys: &HashMap<String, String>, save_system_stats: bool) -> Result<(bool, LicenseData), String> {
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
    }
}