use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{Write, Read};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use base64::prelude::{Engine as _, BASE64_STANDARD};
#[cfg(target_os = "macos")]
use directories::ProjectDirs;
use p384::ecdsa::{Signature, VerifyingKey, signature::DigestVerifier};
use prost::Message;
use sha2::Digest;

use crate::error::{Error, LicensingError};
use crate::generated::software_licensor_client::{ClientSideDataStorage, ClientSideHwInfoStorage, LicenseActivationResponse, LicenseErrorsAndVersions, LicenseKeyFile};
use crate::api::{activate_license_request, get_pubkeys, EcdsaDigest};
use crate::{LicenseData, log_error, log_info};
use crate::generated::software_licensor_client::LicenseData as LicenseDataProto;

use std::{
    io::{Seek, SeekFrom},
    sync::OnceLock,
};

use fs2::FileExt;
use tokio::sync::RwLock;

const LICENSE_FS_CHECK_INTERVAL: Duration = Duration::from_millis(500);
static LICENSE_CACHE: OnceLock<RwLock<HashMap<PathBuf, CachedLicenseFile>>> = OnceLock::new();

fn license_cache() -> &'static RwLock<HashMap<PathBuf, CachedLicenseFile>> {
    LICENSE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Clone)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

#[derive(Clone)]
struct CachedLicenseFile {
    stamp: FileStamp,
    data: ClientSideDataStorage,
    last_fs_check: Instant,
}

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

fn get_file_stamp(path: &Path) -> Result<Option<FileStamp>, Error> {
    match fs::metadata(path) {
        Ok(meta) => Ok(Some(FileStamp {
            modified: meta.modified().ok(),
            len: meta.len(),
        })),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

impl PartialEq for FileStamp {
    fn eq(&self, other: &Self) -> bool {
        self.len.eq(&other.len) && self.modified.eq(&other.modified)
    }
}

fn should_check_fs(cached: &CachedLicenseFile) -> bool {
    cached.last_fs_check.elapsed() >= LICENSE_FS_CHECK_INTERVAL
}

fn decode_license_bytes(buffer: &[u8]) -> Result<ClientSideDataStorage, Error> {
    ClientSideDataStorage::decode_length_delimited(buffer).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to decode license file: {e}"),
        )
        .into()
    })
}

fn encode_license_bytes(data_storage: &ClientSideDataStorage) -> Vec<u8> {
    data_storage.encode_length_delimited_to_vec()
}

pub(crate) async fn get_or_init_license_file(
    company_name_str: &str,
    mut api_key: String,
) -> Result<ClientSideDataStorage, Error> {
    log_info!("Getting or initializing license file");
    let path = get_license_file_path(company_name_str)?;
    api_key.truncate(20);

    // Fast path: if we have a cached entry and it is still within the freshness
    // window, return it without even touching filesystem metadata.
    {
        let cache = license_cache().read().await;
        if let Some(cached) = cache.get(&path) {
            if !should_check_fs(cached) {
                log_info!("Returning cached license file without fs metadata check");
                let mut data_storage = cached.data.clone();

                let mut should_save_license_file = false;

                if data_storage.next_server_ecdh_key.is_none() {
                    get_pubkeys(&mut data_storage, true).await?;
                    should_save_license_file = true;
                }

                if !data_storage.license_data.contains_key(&api_key) {
                    data_storage.license_data.insert(
                        api_key.clone(),
                        LicenseDataProto {
                            license_activation_response: None,
                            license_code: "".to_string(),
                        },
                    );
                    should_save_license_file = true;
                }

                if should_save_license_file {
                    save_license_file(&data_storage, company_name_str).await?;
                }

                return Ok(data_storage);
            }
        }
    }

    // Only now do we pay for the filesystem metadata check.
    let current_stamp = get_file_stamp(&path)?;

    {
        let cache = license_cache().read().await;
        if let (Some(stamp), Some(cached)) = (&current_stamp, cache.get(&path)) {
            if &cached.stamp == stamp {
                log_info!("Returning cached license file after fs metadata check");
                let mut data_storage = cached.data.clone();

                let mut should_save_license_file = false;

                if data_storage.next_server_ecdh_key.is_none() {
                    get_pubkeys(&mut data_storage, true).await?;
                    should_save_license_file = true;
                }

                if !data_storage.license_data.contains_key(&api_key) {
                    data_storage.license_data.insert(
                        api_key.clone(),
                        LicenseDataProto {
                            license_activation_response: None,
                            license_code: "".to_string(),
                        },
                    );
                    should_save_license_file = true;
                }

                if should_save_license_file {
                    save_license_file(&data_storage, company_name_str).await?;
                } else {
                    // Refresh only the last_fs_check timestamp since the file is still current.
                    drop(cache);
                    let mut cache = license_cache().write().await;
                    if let Some(entry) = cache.get_mut(&path) {
                        entry.last_fs_check = Instant::now();
                    }
                }

                return Ok(data_storage);
            }
        }
    }

    // Slow path: reload from disk under write lock so only one task per process does it.
    let mut cache = license_cache().write().await;

    // Re-check after acquiring the write lock in case another task already refreshed it.
    let current_stamp = get_file_stamp(&path)?;
    if let (Some(stamp), Some(cached)) = (&current_stamp, cache.get(&path)) {
        if &cached.stamp == stamp {
            log_info!("Returning cached license file after re-check");
            let mut data_storage = cached.data.clone();

            let mut should_save_license_file = false;

            if data_storage.next_server_ecdh_key.is_none() {
                get_pubkeys(&mut data_storage, true).await?;
                should_save_license_file = true;
            }

            if !data_storage.license_data.contains_key(&api_key) {
                data_storage.license_data.insert(
                    api_key.clone(),
                    LicenseDataProto {
                        license_activation_response: None,
                        license_code: "".to_string(),
                    },
                );
                should_save_license_file = true;
            }

            if should_save_license_file {
                save_license_file(&data_storage, company_name_str).await?;
            }

            return Ok(data_storage);
        }
    }

    let data_storage = if path.exists() {
        log_info!("License file exists, trying to read it");

        let mut file = OpenOptions::new().read(true).open(&path)?;
        file.lock_shared()?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        file.unlock()?;

        match decode_license_bytes(&buffer) {
            Ok(mut data_storage) => {
                let mut should_save_license_file = false;

                if data_storage.next_server_ecdh_key.is_none() {
                    get_pubkeys(&mut data_storage, true).await?;
                    should_save_license_file = true;
                }

                if !data_storage.license_data.contains_key(&api_key) {
                    data_storage.license_data.insert(
                        api_key.clone(),
                        LicenseDataProto {
                            license_activation_response: None,
                            license_code: "".to_string(),
                        },
                    );
                    should_save_license_file = true;
                }

                log_info!("Successfully decoded license file");

                if should_save_license_file {
                    save_license_file(&data_storage, company_name_str).await?;
                    return Ok(data_storage);
                }

                data_storage
            }
            Err(_) => {
                log_error!("Failed to decode license file, initializing a new one");

                let mut license_data = HashMap::new();
                license_data.insert(
                    api_key.clone(),
                    LicenseDataProto {
                        license_activation_response: None,
                        license_code: "".to_string(),
                    },
                );

                let mut data_storage = ClientSideDataStorage {
                    license_data,
                    next_server_ecdh_key: None,
                    server_ecdsa_key: None,
                };

                get_pubkeys(&mut data_storage, true).await?;
                save_license_file(&data_storage, company_name_str).await?;
                return Ok(data_storage);
            }
        }
    } else {
        log_info!("License file does not exist, initializing a new one");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut data_storage = ClientSideDataStorage {
            license_data: HashMap::new(),
            next_server_ecdh_key: None,
            server_ecdsa_key: None,
        };

        if !data_storage.license_data.contains_key(&api_key) {
            data_storage.license_data.insert(
                api_key.clone(),
                LicenseDataProto {
                    license_activation_response: None,
                    license_code: "".to_string(),
                },
            );
        }

        get_pubkeys(&mut data_storage, true).await?;
        save_license_file(&data_storage, company_name_str).await?;
        return Ok(data_storage);
    };

    let new_stamp = get_file_stamp(&path)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "license file missing immediately after load",
        )
    })?;

    cache.insert(
        path.clone(),
        CachedLicenseFile {
            stamp: new_stamp,
            data: data_storage.clone(),
            last_fs_check: Instant::now(),
        },
    );

    Ok(data_storage)
}

pub(crate) async fn get_or_init_hw_info_file() -> Result<ClientSideHwInfoStorage, Error> {
    log_info!("Getting or initializing hw info file");
    let path = get_machine_stats_path()?;

    let mut result = if path.exists() {
        log_info!("hw info file exists, trying to read it");
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        match ClientSideHwInfoStorage::decode_length_delimited(buffer.as_slice()) {
            Ok(stats) => {
                log_info!("Successfully decoded hw info file");

                stats
            },
            Err(_) => {
                log_info!("Failed to decode hw info file, initializing a new one");
                let hw_info_storage = ClientSideHwInfoStorage {
                    machine_stats: None,
                };
                hw_info_storage
            }
        }
    } else {
        log_info!("hw info file does not exist, initializing a new one");
        let hw_info_storage = ClientSideHwInfoStorage {
            machine_stats: None,
        };
        hw_info_storage
    };
    let save_system_stats = result.machine_stats.is_some();
    
    // Safety: This is safe because we only read the stats if 
    // `save_system_stats` is true, which means that the stats 
    // were successfully read from the file and are valid. If 
    // `save_system_stats` is false, then we don't read the 
    // stats and just initialize them, so there is no risk of 
    // reading invalid stats.
    let current_stats = unsafe {
        crate::stats::get_machine_stats(save_system_stats).await
    };
    if result.machine_stats.ne(&current_stats) {
        log_info!("Machine stats have changed since last save, updating hw info file");
        result.machine_stats = current_stats;
        save_hw_info_file(&result)?;
    } else {
        log_info!("Machine stats have not changed since last save");
    }
    
    Ok(result)
}

/// Saves the license file to the path (if the permissions are correct).
pub(crate) async fn save_license_file(
    data_storage: &ClientSideDataStorage,
    company_name_str: &str,
) -> Result<(), Error> {
    log_info!("Saving license file");

    let path = get_license_file_path(company_name_str)?;
    log_info!("License path: {}", path.to_str().unwrap_or("Path is not valid unicode"));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bytes = encode_license_bytes(data_storage);

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)?;

    file.lock_exclusive()?;

    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    file.unlock()?;

    let stamp = get_file_stamp(&path)?.ok_or_else(|| {
        log_error!("license file missing immediately after save");
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "license file missing immediately after save",
        )
    })?;

    let mut cache = license_cache().write().await;
    cache.insert(
        path,
        CachedLicenseFile {
            stamp,
            data: data_storage.clone(),
            last_fs_check: Instant::now(),
        },
    );

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

struct LicensingErrors {
    error_code: i32,
    product_version: String,
    error_code_to_version: HashMap<i32, String>,
    product_id_to_version: HashMap<String, String>,
    error_codes: Vec<(i32, String)>,
}

impl LicensingErrors {
    fn init(errors_and_versions: &HashMap<String, LicenseErrorsAndVersions>) -> Self {
        let mut error_code_to_version = HashMap::with_capacity(errors_and_versions.len());
        let mut product_id_to_version = HashMap::with_capacity(errors_and_versions.len());
        let mut error_codes = Vec::with_capacity(errors_and_versions.len());
        for (product_id, errors_and_versions) in errors_and_versions {
            let error_code = errors_and_versions.licensing_error;
            let version = &errors_and_versions.version;
            error_code_to_version.insert(error_code as i32, version.clone());
            product_id_to_version.insert(product_id.clone(), version.clone());
            error_codes.push((error_code as i32, version.clone()));
        }
        error_codes.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        Self {
            error_code: 0,
            product_version: "0".into(),
            error_code_to_version,
            product_id_to_version,
            error_codes,
         }
    }
    fn get_strings_error_code(&self, error_code: i32) -> Option<&String> {
        self.error_code_to_version.get(&error_code)
    }
    fn get_version_of_product(&self, product_id: &str) -> Option<&String> {
        self.product_id_to_version.get(product_id)
    }
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
pub(crate) fn get_latest_key_file(data_storage: &ClientSideDataStorage, product_ids: &Vec<&String>, mut api_key: String, preferred_product_id_for_version_check: &str) -> Result<(LicenseKeyFile, Signature, LicenseActivationResponse), LicensingError> {
    log_info!("Getting latest key file for product ids: {:?}", product_ids);
    api_key.truncate(20);
    let license_data = match data_storage.license_data.get(&api_key) {
        Some(v) => v,
        None => return Err(LicensingError::NoLicenseFound(("".into(), "".into()).into()))
    };
    
    let license_activation_response = match &license_data.license_activation_response {
        Some(v) => v,
        None => return Err(LicensingError::NoLicenseFound(("".into(), "".into())))
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
                return Err(LicensingError::NoLicenseFound((key_file.license_code.clone(), key_file.product_version.to_string()).into()))
            }
        };
        let signature: Signature = match Signature::from_bytes(sig_bytes.as_slice().into()) {
            Ok(v) => v,
            Err(_) => {
                log_error!("Failed to create signature from bytes");
                return Err(LicensingError::NoLicenseFound((key_file.license_code.clone(), key_file.product_version.to_string()).into()))
            }
        };
        return Ok((key_file.clone(), signature, license_activation_response.clone()))
    }
    if found_key_files.is_empty() {
        let errors = &license_activation_response.license_errors_and_versions;
        let mut error_codes = LicensingErrors::init(errors);
        log_error!("No key files found for the product ids. Licensing error codes for the product ids: {:?}", error_codes.error_code_to_version);
        if error_codes.error_code_to_version.is_empty() {
            log_info!("No license errors found for this license, but also no key files found.");
            return Err(LicensingError::NoLicenseFound((license_data.license_code.clone(), "0".into()).into()))
        }
        // prioritizing specific licensing errors over others
        if let Some(v) = error_codes.get_strings_error_code(4) {
            log_info!("Machine limit reached for this license.");
            // machine limit reached
            return Err(LicensingError::MachineLimitReached((license_data.license_code.clone(), error_codes.get_version_of_product(preferred_product_id_for_version_check).unwrap_or(&"0".to_string()).to_string())).into())
        }
        if let Some(v) = error_codes.get_strings_error_code(16) {
            log_info!("License is no longer active.");
            return Err(LicensingError::LicenseNoLongerActive((license_data.license_code.clone(), error_codes.get_version_of_product(preferred_product_id_for_version_check).unwrap_or(&"0".to_string()).to_string()).into()))
        }
        if let Some(v) = error_codes.get_strings_error_code(8) {
            log_info!("Trial has ended for this license.");
            return Err(LicensingError::TrialEnded((license_data.license_code.clone(), error_codes.get_version_of_product(preferred_product_id_for_version_check).unwrap_or(&"0".to_string()).to_string()).into()))
        }
        log_info!("Returning most relevant licensing error code for this license: {}", error_codes.error_codes[0].0);
        return Err(LicensingError::from(
            (
                error_codes.error_codes[0].0 as u32, 
                license_data.license_code.as_str(), 
                error_codes
                    .get_version_of_product(preferred_product_id_for_version_check)
                    .unwrap_or(&"0".to_string())
                    .as_str()
            )
        ))
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
        None => return Err(LicensingError::NoLicenseFound((key_file.license_code.clone(), "0".into())))
    };
    let signature: Signature = match Signature::from_bytes(sig_bytes.as_slice().into()) {
        Ok(v) => v,
        Err(_) => return Err(LicensingError::NoLicenseFound((key_file.license_code.clone(), "0".into())))
    };
    Ok((key_file.clone(), signature, license_activation_response.clone()))
}

/// Removes key files so that we don't keep automatically checking up
/// on them.
#[inline(always)]
pub(crate) async fn remove_key_files(license_file: &mut ClientSideDataStorage, product_ids: &Vec<&String>, company_name_str: &str, mut api_key: String) {
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
    save_license_file(license_file, company_name_str).await.unwrap_or_else(|_| ());
}

/// Handles licensing errors by removing key files before returning the error.
/// 
/// Not entirely sure if this should ever be used... removing key files is best 
/// used when cryptographic issues arise or the machine ID doesn't match. Which 
/// is done using remove_key_files directly.
#[inline(always)]
pub(crate) async fn handle_licensing_error(license_file: &mut ClientSideDataStorage, product_ids: &Vec<&String>, company_name_str: &str, licensing_error: LicensingError, api_key: String) -> Error {
    remove_key_files(license_file, product_ids, company_name_str, api_key).await;
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
    should_send_request: bool, 
    api_key: String,
    send_computer_name: bool,
    preferred_product_id_for_version_check: &str,
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
            return Err(Error::LicensingError((2u32, "", "0").into()))
        }
    };
    let license_code: String = match license_data.license_code.len() < 16 {
        true => {
            log_error!("License code in license file is less than 16 chars: {}", license_data.license_code);
            return Err(Error::LicensingError((2u32, license_data.license_code.as_str(), "0").into()))
        },
        false => license_data.license_code.clone()
    }.to_owned();
    let product_ids: Vec<&String> = product_ids_and_pubkeys.keys().collect();
    let (mut key_file, mut signature, mut license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone(), preferred_product_id_for_version_check) {
        Ok(v) => v,
        Err(licensing_error) => {
            log_error!("Failed to get latest key file: {:?}", licensing_error.get_error_and_license_codes_and_version());
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
        match activate_license_request(store_id, company_name_str, &product_ids, &license_code, &mut license_file, send_computer_name).await {
            Ok(_) => (),
            Err(e) => {
                log_error!("Failed to activate license: {}", e);
                return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32))
            }
        }
        (key_file, signature, license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone(), preferred_product_id_for_version_check) {
            Ok(v) => v,
            Err(licensing_error) =>  {
                log_error!("Failed to get latest key file after sending request");
                log_error!("Licensing error: {:?}", licensing_error.get_error_and_license_codes_and_version().0);
                return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32))
            }
        };
        if key_file.message_code != 1 && key_file.message_code < 512 {
            log_error!("After sending request, key_file.message code is still not 1: {}", key_file.message_code);
            return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32));
        }
        if key_file.message_code >= 512 {
            log_error!("Error code is >= 512: {}", key_file.message_code);
            return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.message_code as i32))
        }
        if key_file.expiration_timestamp < now {
            log_error!("Key file expired at {}, now is {}", key_file.expiration_timestamp, now);
            let err = if key_file.post_expiration_error_code == 16 {
                LicensingError::LicenseNoLongerActive((license_code.to_string(), key_file.product_version))
            } else {
                LicensingError::TrialEnded((license_code, key_file.product_version))
            };
            return Ok(LicenseData::error(&Error::LicensingError(err)))
        }
    }
    if key_file.check_back_timestamp < now && should_send_request {
        // send request
        log_info!("Key file check back timestamp is {}, now is {}, sending request to check for an update", key_file.check_back_timestamp, now);
        if let Ok(_) = activate_license_request(store_id, company_name_str, &product_ids, &license_code, &mut license_file, send_computer_name).await {
            (key_file, signature, license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone(), preferred_product_id_for_version_check) {
                Ok(v) => v,
                Err(licensing_error) => {
                    log_error!("activate_license_request failed when check back timestmap was less than now");
                    return Ok(LicenseData::error(&Error::LicensingError(licensing_error)));
                }
            }
        }
    }

    let device_id = crate::stats::device_id();
    if device_id.ne(&key_file.machine_id) {
        log_error!("Machine ID does not match key file machine ID. device_id: {}\nkey_file.machine_id: {}", device_id, key_file.machine_id);

        remove_key_files(&mut license_file, &product_ids, company_name_str, api_key).await;
        return Err(LicensingError::NoLicenseFound((license_code, key_file.product_version)).into())
    }
    
    // verify signature on the key file
    let pubkey_b64 = match product_ids_and_pubkeys.get(&key_file.product_id) {
        Some(v) => v,
        None => {
            log_error!("Product ID not found in public keys");
            return Err(LicensingError::NoLicenseFound((license_code, key_file.product_version)).into())
        }
    };
    let decoded_pubkey = match BASE64_STANDARD.decode(pubkey_b64) {
        Ok(v) => v,
        Err(e) => {
            log_error!("Failed to decode public key: {}", e);
            return Err(LicensingError::NoLicenseFound((license_code, key_file.product_version)).into())
        }
    };

    let bytes = key_file.encode_length_delimited_to_vec();
    let verifying_key = match VerifyingKey::from_sec1_bytes(&decoded_pubkey) {
        Ok(v) => v,
        Err(e) => {
            log_error!("Failed to parse verifying key from developer supplied public key: {}", e);
            remove_key_files(&mut license_file, &product_ids, company_name_str, api_key).await;
            return Err(LicensingError::NoLicenseFound((license_code, key_file.product_version)).into())
        }
    };
    match verifying_key.verify_digest(EcdsaDigest::new_with_prefix(bytes), &signature) {
        Ok(_) => Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.message_code as i32)),
        Err(_) => {
            log_error!("Failed to verify signature on key file");
            remove_key_files(&mut license_file, &product_ids, company_name_str, api_key).await;
            Err(LicensingError::NoLicenseFound((license_code, key_file.product_version)).into())
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
            key_file_signatures: HashMap::new(),
            license_errors_and_versions: HashMap::new(),
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

        let newest_key_file = get_latest_key_file(&data_storage, &product_ids.clone(), "ABCDEFGHIJKL".to_string(), &expired_product_id).expect("Possibly lacking file read permissions").0;

        assert_eq!("newest_product_id", newest_key_file.product_id);

        let newest_key_file = get_latest_key_file(&data_storage, &product_ids.clone(), "ABCDEFGHIJKL".to_string(), &expired_product_id).expect("Possibly lacking file read permissions").0;

        assert_eq!("newest_product_id", newest_key_file.product_id);
    }
}