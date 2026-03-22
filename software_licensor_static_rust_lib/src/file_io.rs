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
use crate::LicenseData;
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

pub(crate) async fn get_or_init_license_file(company_name_str: &str, mut api_key: String) -> Result<ClientSideDataStorage, Error> {
    let path = get_license_file_path(company_name_str)?;
    api_key.truncate(20);
    
    if path.exists() {
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
                save_license_file(&data_storage, company_name_str)?;
                Ok(data_storage)
            },
            Err(_) => {
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
        Ok(data_storage)
    }
}

pub(crate) async fn get_or_init_hw_info_file() -> Result<ClientSideHwInfoStorage, Error> {
    let path = get_machine_stats_path()?;

    if path.exists() {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        match ClientSideHwInfoStorage::decode_length_delimited(buffer.as_slice()) {
            Ok(stats) => {
                Ok(stats)
            },
            Err(_) => {
                let hw_info_storage = ClientSideHwInfoStorage {
                    machine_stats: None,
                };
                Ok(hw_info_storage)
            }
        }
    } else {
        let hw_info_storage = ClientSideHwInfoStorage {
            machine_stats: None,
        };
        Ok(hw_info_storage)
    }
}

/// Saves the license file to the path (if the permissions are correct).
pub(crate) fn save_license_file(data_storage: &ClientSideDataStorage, company_name_str: &str) -> Result<(), Error> {
    let path = get_license_file_path(company_name_str)?;
    
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // write the file
        let mut file = File::create_new(path)?;
        file.write_all(data_storage.encode_length_delimited_to_vec().as_slice())?;
    } else {
        let mut file = OpenOptions::new()
            .write(true)
            .append(false)
            .truncate(true)
            .open(path)?;
        file.write_all(data_storage.encode_length_delimited_to_vec().as_slice())?;
    }
    Ok(())
}

pub(crate) fn save_hw_info_file(data: &ClientSideHwInfoStorage) -> Result<(), Error> {
    let path = get_machine_stats_path()?;

    let contents = data.encode_length_delimited_to_vec();

    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create_new(path)?;
        file.write_all(&contents)?;
    } else {
        let mut file = OpenOptions::new()
            .write(true)
            .append(false)
            .truncate(true)
            .open(path)?;
        file.write_all(&contents)?;
    }
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
            None => return Err(LicensingError::NoLicenseFound(key_file.license_code.clone()))
        };
        let signature: Signature = match Signature::from_bytes(sig_bytes.as_slice().into()) {
            Ok(v) => v,
            Err(_) => return Err(LicensingError::NoLicenseFound(key_file.license_code.clone()))
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
    api_key: String
) -> Result<LicenseData, Error> {
    let mut license_file = match license_file {
        Some(file) => file,
        None => &mut get_or_init_license_file(company_name_str, api_key.clone()).await?
    };
    let mut trimmed_api_key = api_key.clone();
    trimmed_api_key.truncate(20);
    let license_data = match license_file.license_data.get_mut(&trimmed_api_key) {
        Some(v) => v,
        None => return Err(Error::LicensingError((2, "".to_string()).into()))
    };
    let license_code: String = match license_data.license_code.len() < 16 {
        true => return Err(Error::LicensingError((2, license_data.license_code.clone()).into())),
        false => license_data.license_code.clone()
    }.to_owned();
    let product_ids: Vec<&String> = product_ids_and_pubkeys.keys().collect();
    let (mut key_file, mut signature, mut license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone()) {
        Ok(v) => v,
        Err(licensing_error) => return Err(licensing_error.into())
    };
    if key_file.message_code != 1 {
        return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.message_code as i32));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if key_file.expiration_timestamp < now {
        if !should_send_request {
            return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32));
        }
        // send request to check for an update
        match activate_license_request(store_id, company_name_str, &product_ids, machine_id, &license_code, &mut license_file).await {
            Ok(_) => (),
            Err(_) => {
                return Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.post_expiration_error_code as i32))
            }
        }
        (key_file, signature, license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone()) {
            Ok(v) => v,
            Err(licensing_error) => return Err(handle_licensing_error(&mut license_file, &product_ids, company_name_str, licensing_error, api_key))
        };
        if key_file.message_code != 1 && key_file.message_code < 512 {
            return Err(handle_licensing_error(&mut license_file, &product_ids, company_name_str, LicensingError::from((key_file.message_code as u32, license_code.clone())), api_key))
        }
        if key_file.message_code >= 512 {
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
            let err = if key_file.post_expiration_error_code == 16 {
                LicensingError::LicenseNoLongerActive(license_code)
            } else {
                LicensingError::TrialEnded(license_code)
            };
            return Err(handle_licensing_error(&mut license_file, &product_ids, company_name_str, err, store_id.to_string()))
        }
    }
    if key_file.check_back_timestamp < now && should_send_request {
        // send request
        if let Ok(_) = activate_license_request(store_id, company_name_str, &product_ids, machine_id, &license_code, &mut license_file).await {
            (key_file, signature, license_activation_response) = match get_latest_key_file(&license_file, &product_ids, api_key.clone()) {
                Ok(v) => v,
                Err(licensing_error) => return Err(handle_licensing_error(&mut license_file, &product_ids, company_name_str, licensing_error, api_key))
            }
        }
    }

    if machine_id.ne(&key_file.machine_id) {
        remove_key_files(&mut license_file, &product_ids, company_name_str, api_key);
        return Err(LicensingError::NoLicenseFound(license_code).into())
    }
    
    // verify signature on the key file
    let pubkey_b64 = match product_ids_and_pubkeys.get(&key_file.product_id) {
        Some(v) => v,
        None => return Err(LicensingError::NoLicenseFound(license_code).into())
    };
    let decoded_pubkey = match BASE64_STANDARD.decode(pubkey_b64) {
        Ok(v) => v,
        Err(_) => return Err(LicensingError::NoLicenseFound(license_code).into())
    };

    let bytes = key_file.encode_length_delimited_to_vec();
    let verifying_key = match VerifyingKey::from_sec1_bytes(&decoded_pubkey) {
        Ok(v) => v,
        Err(_) => {
            remove_key_files(&mut license_file, &product_ids, company_name_str, api_key);
            return Err(LicensingError::NoLicenseFound(license_code).into())
        }
    };
    match verifying_key.verify_digest(EcdsaDigest::new_with_prefix(bytes), &signature) {
        Ok(_) => Ok(LicenseData::from_key_file_and_license_response(&key_file, &license_activation_response, key_file.message_code as i32)),
        Err(_) => {
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