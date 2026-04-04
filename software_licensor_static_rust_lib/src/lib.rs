#![deny(clippy::unwrap_used)]
#![allow(clippy::enum_variant_names)]

use std::collections::HashMap;
#[cfg(not(feature = "rlib"))]
use std::{
    ffi::{CString, CStr}, 
    os::raw::{c_char, c_int}
};
#[cfg(all(feature = "rlib", feature = "serde"))]
use serde::{Serialize, Deserialize};
use std::time::Duration;

mod logging;
pub(crate) use logging::*;

use api::activate_license_request;
use file_io::{check_key_file_async, get_or_init_hw_info_file, get_or_init_license_file, save_hw_info_file};
use generated::software_licensor_client::{LicenseActivationResponse, LicenseKeyFile, Stats};
use tokio::runtime::Runtime;

mod api;
mod generated;
mod error;
mod file_io;
mod macros;
mod status_messages;

#[inline(always)]
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("Time went backwards").as_secs()
}

#[cfg(feature = "rlib")]
pub mod lib_api;
#[cfg(feature = "rlib")]
pub use lib_api::*;
pub(crate) mod stats;

use error::{Error, LicensingError};
use tokio::time::sleep;

/// The URL to the Software Licensor Public Key repository. Change this if you 
/// have built the code for yourself.
const PUBLIC_KEY_REPO_URL: &str = "https://software-licensor-public-keys.s3.amazonaws.com/public_keys";
const LICENSE_ACTIVATION_URL: &str = "https://01lzc0nx9e.execute-api.us-east-1.amazonaws.com/v2/license_activation_refactor";

#[cfg(not(feature = "rlib"))]
#[repr(C)]
pub struct LicenseData {
    result_code: c_int,
    customer_first_name: *mut c_char,
    customer_last_name: *mut c_char,
    customer_email: *mut c_char,
    license_type: *mut c_char,
    version: *mut c_char,
    error_message: *mut c_char,
    license_code: *mut c_char
}

#[cfg(feature = "rlib")]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct LicenseData {
    pub result_code: i32,
    pub customer_first_name: String,
    pub customer_last_name: String,
    pub customer_email: String,
    pub license_type: String,
    pub version: String,
    pub error_message: String,
    pub license_code: String,
    pub machine_count: Option<u32>,
    pub machine_limit: Option<u32>,
}

impl LicenseData {
    #[cfg(not(feature = "rlib"))]
    fn new(
        int_result: i32, 
        first_name: &str, 
        last_name: &str, 
        email: &str, 
        license_type: &str, 
        version: &str, 
        error_message: &str, 
        license_code: &str,
        _machine_count: Option<u32>,
        _machine_limit: Option<u32>
    ) -> Self {
        Self {
            result_code: int_result as c_int,
            customer_first_name: CString::new(first_name).expect("CString::new failed").into_raw(),
            customer_last_name: CString::new(last_name).expect("CString::new failed").into_raw(),
            customer_email: CString::new(email).expect("CString::new failed").into_raw(),
            license_type: CString::new(license_type).expect("CString::new failed").into_raw(),
            version: CString::new(version).expect("CString::new failed").into_raw(),
            error_message: CString::new(error_message).expect("CString::new failed").into_raw(),
            license_code: CString::new(license_code).expect("CString::new failed").into_raw()
        }
    }
    #[cfg(feature = "rlib")]
    fn new(int_result: i32, first_name: &str, last_name: &str, email: &str, license_type: &str, version: &str, error_message: &str, license_code: &str, machine_count: Option<u32>, machine_limit: Option<u32>) -> Self {
        Self {
            result_code: int_result,
            customer_first_name: first_name.to_string(),
            customer_last_name: last_name.to_string(),
            customer_email: email.to_string(),
            license_type: license_type.to_string(),
            version: version.to_string(),
            error_message: error_message.to_string(),
            license_code: license_code.to_string(),
            machine_count,
            machine_limit,
        }
    }
    #[deprecated(note = "This function is deprecated because it doesn't parse the error")]
    fn error_old(message: &str, license_code: &str) -> Self {
        Self::new(
            -1, 
            "Error", 
            "Error", 
            "Error", 
            "Error", 
            "Error", 
            message, 
            license_code,
            None,
            None
        )
    }
    fn error(error: &Error) -> Self {
        let (license_code, error_code, version) = error.get_license_code_and_error_code_and_version();
        let error_message = status_messages::get_status_message_from_code(error_code as i32);
        Self::new(
            error_code as i32,
            "Error",
            "Error",
            "Error",
            "Error",
            version,
            &error_message,
            license_code,
            None,
            None
        )
    }
    fn general_error(error_message: &str) -> Self {
        Self::new(
            -1,
            "Error",
            "Error",
            "Error",
            "Error",
            "Error",
            error_message,
            "Error",
            None,
            None,
        )
    }
    fn from_key_file_and_license_response(key_file: &LicenseKeyFile, license_response: &LicenseActivationResponse, status_code: i32) -> Self {
        Self::new(
            status_code, 
            &license_response.customer_first_name, 
            &license_response.customer_last_name, 
            &license_response.customer_email, 
            &key_file.license_type, 
            &key_file.product_version,
            #[cfg(feature = "rlib")]
            &status_messages::get_status_message_from_code(status_code),
            #[cfg(not(feature = "rlib"))] 
            "",
            &key_file.license_code,
            key_file.current_machine_count,
            key_file.current_machine_limit,
        )
    }
    fn licensing_error(licensing_error: &LicensingError) -> Self {
        let (error_code, license_code, version) = licensing_error.get_error_and_license_codes_and_version();
        Self::new(
            error_code as i32, 
            "", 
            "", 
            "", 
            "", 
            version, 
            licensing_error.to_string().as_str(), 
            &license_code, 
            None, 
            None
        )
    }
}

/// Updates machine information in the license file. It should be optional for
/// the end user to have the stats saved, but there isn't a super convenient 
/// way to save them all, and there isn't a way for Rust code to grab all of 
/// the machine stats for all machines. These stats are readily available with 
/// the JUCE library.
#[no_mangle]
#[inline(always)]
#[cfg(not(feature = "rlib"))]
pub extern "C" fn update_machine_info(
    save_system_stats: bool,
) {
    #[cfg(feature = "logging")]
    {
        use crate::inner::init_logger;

        if let Ok(log_path) = init_logger() {
            log_info!("file logging initialized at update_machine_info: {}", log_path.display());
        } else {
            log_error!("failed to initialize file logging at update_machine_info");
        }
    }
    let rt = match Runtime::new() {
        Ok(v) => v,
        Err(_) => return
    };

    rt.block_on(async {
        let mut hw_info_file = match get_or_init_hw_info_file().await {
            Ok(v) => v,
            Err(_) => return
        };

        if !save_system_stats {
            hw_info_file.machine_stats = None;
            let _result = save_hw_info_file(&hw_info_file).unwrap_or_else(|_| ());
            sleep(Duration::from_secs(1)).await;
            return
        }

        // Safety: Stats are only `Some` if save_system_stats is true.
        let current_stats = unsafe {
            crate::stats::get_machine_stats(save_system_stats)
        };

        if hw_info_file.machine_stats.ne(&current_stats) {
            hw_info_file.machine_stats = current_stats;
            let _result = save_hw_info_file(&hw_info_file).unwrap_or_else(|_| ());
            sleep(Duration::from_secs(1)).await;
        }
    });
}

/// Deallocate license data after C++ code has evaluated/copied the data
#[no_mangle]
#[inline(always)]
#[cfg(not(feature = "rlib"))]
pub extern "C" fn free_license_data(ptr: *mut LicenseData) {
    if !ptr.is_null() {
        // Reconstitute the Box to take ownership back from C++
        let data = unsafe { Box::from_raw(ptr) };

        // Properly deallocate CString for each string field if not null
        unsafe {
            if !data.customer_first_name.is_null() {
                let _ = CString::from_raw(data.customer_first_name);
            }
            if !data.customer_last_name.is_null() {
                let _ = CString::from_raw(data.customer_last_name);
            }
            if !data.customer_email.is_null() {
                let _ = CString::from_raw(data.customer_email);
            }
            if !data.license_type.is_null() {
                let _ = CString::from_raw(data.license_type);
            }
            if !data.error_message.is_null() {
                let _ = CString::from_raw(data.error_message);
            }
            if !data.license_code.is_null() {
                let _ = CString::from_raw(data.license_code);
            }
        }
    }
}

#[no_mangle]
#[inline(always)]
#[cfg(not(feature = "rlib"))]
pub extern "C" fn read_reply_from_webserver(
    company_name: *const c_char, 
    store_id: *const c_char, 
    machine_id: *const c_char, 
    license_code: *const c_char, 
    product_ids_and_pubkeys: *const *const c_char, 
    len: c_int,
    preferred_product_id_for_version_check: *const c_char
) -> *mut LicenseData {
    #[cfg(feature = "logging")]
    {
        use crate::inner::init_logger;

        if let Ok(log_path) = init_logger() {
            log_info!("file logging initialized at read_reply_from_webserver: {}", log_path.display());
        } else {
            log_error!("failed to initialize file logging at read_reply_from_webserver");
        }
    }
    log_info!("read_reply_from_webserver");
    let store_id_str = parse_c_char!(store_id, "Failed to parse store id", true);
    let company_name_str = parse_c_char!(company_name, "Failed to parse company name", true);
    let machine_id_str = parse_c_char!(machine_id, "Failed to parse machine id", true);
    let license_code_str = parse_c_char!(license_code, "Failed to parse license code", true);
    let preferred_product_id_for_version_check_str = parse_c_char!(preferred_product_id_for_version_check, "Failed to parse preferred product ID for version check", true);

    let array_size = unsafe { std::slice::from_raw_parts(product_ids_and_pubkeys, len as usize) };
    
    let mut product_ids_and_pubkeys_vec: Vec<&str> = Vec::with_capacity(len as usize);
    for s in array_size.iter() {
        match unsafe { CStr::from_ptr(*s).to_str() } {
            Ok(v) => product_ids_and_pubkeys_vec.push(v),
            Err(_) => {
                log_error!("UTF-8 error when decoding product IDs and pubkeys");
                return box_out!(LicenseData::general_error("UTF-8 error when decoding product IDs and pubkeys"))
            }
        }
    }

    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys_vec.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            log_error!("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1: {}", product_id_and_key);
            return box_out!(LicenseData::general_error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1"));
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = runtime!(true);

    rt.block_on(async {
        let mut license_file = match get_or_init_license_file(company_name_str, store_id_str.to_string()).await {
            Ok(v) => v,
            Err(e) => {
                log_error!("Failed to get or initialize license file: {}", e);
                return box_out!(LicenseData::general_error(&e.to_string()))
            }
        };
        sleep(Duration::from_secs(5)).await;
        match activate_license_request(store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap.keys().collect::<Vec<&String>>(), machine_id_str, license_code_str, &mut license_file, false).await {
            Ok(()) => (),
            Err(v) => {
                log_error!("There was an error when activating the license: {}", v);
                match v {
                    Error::LicensingError(e) => return box_out!(LicenseData::licensing_error(&e)),
                    _ => return box_out!(LicenseData::general_error(&v.to_string()))
                }
            }
        };
        match check_key_file_async(Some(&mut license_file), store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap, machine_id_str, false, store_id_str.to_string(), false, preferred_product_id_for_version_check_str).await {
            Ok(v) => return box_out!(v),
            Err(e) => {
                log_error!("There was an error when checking the license after activation: {}", e);
                match e {
                    Error::LicensingError(error) => return box_out!(LicenseData::licensing_error(&error)),
                    _ => return box_out!(LicenseData::general_error(&e.to_string()))
                }
            }
        }
    })
}

/// Checks the license and returns the result.
/// 
/// This function may make an API request, so it shouldn't be called while processing audio.
/// 
/// # Arguments
/// 
/// * `app_name` - the application name, used for file paths
/// * `store_id` - the store ID string found in the `Software Licensor` page of 
/// the WordPress admin dashboard
/// * `machine_id` - the user's machine ID
/// * `product_ids_and_pubkeys` - any product ID and its associated public key 
/// that might be associated with this software. This takes an array in case 
/// this software can come both as a bundle or individually. There should be a 
/// colon (:) separating each product ID from the public key.
/// * `len` - the length of the `product_ids_and_pubkeys` array 
#[no_mangle]
#[inline(always)]
#[cfg(not(feature = "rlib"))]
pub extern "C" fn check_license(
    company_name: *const c_char, 
    store_id: *const c_char, 
    machine_id: *const c_char, 
    product_ids_and_pubkeys: *const *const c_char, 
    len: c_int,
    preferred_product_id_for_version_check: *const c_char,
) -> *mut LicenseData {
    #[cfg(feature = "logging")]
    {
        use crate::inner::init_logger;

        if let Ok(log_path) = init_logger(){
            log_info!("file logging initialized at check_license: {}", log_path.display());
        } else {
            log_error!("failed to initialize file logging at check_license");
        }
    }
    let store_id_str = parse_c_char!(store_id, "Failed to parse store id", true);
    let company_name_str = parse_c_char!(company_name, "Failed to parse company name", true);
    let machine_id_str = parse_c_char!(machine_id, "Failed to parse machine id", true);
    let preferred_product_id_for_version_check_str = parse_c_char!(preferred_product_id_for_version_check, "Failed to parse preferred product ID for version check", true);

    let array_size = unsafe { std::slice::from_raw_parts(product_ids_and_pubkeys, len as usize) };
    
    let mut product_ids_and_pubkeys_vec: Vec<&str> = Vec::with_capacity(len as usize);
    for s in array_size.iter() {
        match unsafe { CStr::from_ptr(*s).to_str() } {
            Ok(v) => product_ids_and_pubkeys_vec.push(v),
            Err(_) => {
                log_error!("UTF-8 error when decoding product IDs and pubkeys");
                return box_out!(LicenseData::general_error("UTF-8 error when decoding product IDs and pubkeys"))
            }
        }
    }

    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys_vec.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            log_error!("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1: {}", product_id_and_key);
            return box_out!(LicenseData::general_error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1"))
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = runtime!(true);

    rt.block_on(async {
        match check_key_file_async(None, store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap, machine_id_str, true, store_id_str.to_string(), false, preferred_product_id_for_version_check_str).await {
            Ok(v) => {
                box_out!(v)
            },
            Err(e) => {
                log_error!("There was an error when checking the license: {}", e);
                log_error!("Error message for code: {}", status_messages::get_status_message_from_code(e.get_license_code_and_error_code_and_version().1 as i32));
                match e {
                    Error::LicensingError(v) => {
                        let r = LicenseData::licensing_error(&v);
                        box_out!(r)
                    },
                    _ => {
                        let r = LicenseData::general_error(e.to_string().as_str());
                        box_out!(r)
                    }
                }
            }
        }
    })
}

/// Checks the license file with a guarantee that it will not ping the server 
/// for an update.
/// 
/// This might be useful because it directly returns the LicenseData struct 
/// through an inline function call. Refer to the documentation in 
/// `check_license`.
#[no_mangle]
#[inline(always)]
#[cfg(not(feature = "rlib"))]
pub extern "C" fn check_license_no_api_request(
    company_name: *const c_char, 
    store_id: *const c_char, 
    machine_id: *const c_char, 
    product_ids_and_pubkeys: *const *const c_char, 
    len: c_int,
    preferred_product_id_for_version_check: *const c_char,
) -> *mut LicenseData {
    #[cfg(feature = "logging")]
    {
        use crate::inner::init_logger;

        if let Ok(log_path) = init_logger() {
            log_info!("file logging initialized at check_license_no_api_request: {}", log_path.display());
        } else {
            log_error!("Failed to initialize file logging in check_license_no_api_request");
        }
    }
    let store_id_str = parse_c_char!(store_id, "Failed to parse store id", true);
    let company_name_str = parse_c_char!(company_name, "Failed to parse company name", true);
    let machine_id_str = parse_c_char!(machine_id, "Failed to parse machine id", true);
    let preferred_product_id_for_version_check_str = parse_c_char!(preferred_product_id_for_version_check, "Failed to parse preferred product ID for version check", true);
    
    let array_size = unsafe { std::slice::from_raw_parts(product_ids_and_pubkeys, len as usize) };
    
    let mut product_ids_and_pubkeys_vec: Vec<&str> = Vec::with_capacity(len as usize);
    for s in array_size.iter() {
        match unsafe { CStr::from_ptr(*s).to_str() } {
            Ok(v) => product_ids_and_pubkeys_vec.push(v),
            Err(_) => return box_out!(LicenseData::general_error("UTF-8 error when decoding product IDs and pubkeys"))
        }
    }

    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys_vec.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return box_out!(LicenseData::general_error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1"))
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = runtime!(true);

    rt.block_on(async {
        match check_key_file_async(None, store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap, machine_id_str, false, store_id_str.to_string(), false, preferred_product_id_for_version_check_str).await {
            Ok(v) => {
                return box_out!(v)
            },
            Err(e) => box_out!(LicenseData::error(&e))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use crate::generated::software_licensor_client::ClientSideDataStorage;
    use prost::Message;

    #[test]
    fn failing_test() {
        let path = "src/tmp/license.bin";
        let mut file = File::open(path).unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer);
        match ClientSideDataStorage::decode_length_delimited(buffer.as_slice()) {
            Ok(mut data_storage) => {
                let april_7_2026 = 1775576703;
                assert_eq!(data_storage.server_ecdsa_key.unwrap().expiration, april_7_2026);
                let license_data = data_storage.license_data;
                //let first_key = license_data.keys().next().unwrap();
                //assert_eq!(first_key, "MOFO");
                assert_eq!(license_data, HashMap::new());
                //assert_eq!(data_storage.license_data.get(data_storage.license_data.keys().first().unwrap()).license_activation_response.licensing_errors, Default::default())
            },
            Err(_) => {
                panic!("Couldn't read license file");
            }
        }
    }
}