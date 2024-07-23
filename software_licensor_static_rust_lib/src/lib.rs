#![deny(clippy::unwrap_used)]
#![allow(clippy::enum_variant_names)]

use std::collections::HashMap;
use std::os::raw::{c_char, c_int};
use std::ffi::{CString, CStr};
use std::time::Duration;

use api::activate_license_request;
use ffi::LicenseData;
use file_io::{check_key_file_async, get_or_init_license_file};
use generated::software_licensor_client::{LicenseActivationResponse, LicenseKeyFile};
use tokio::runtime::Runtime;

mod api;
mod generated;
mod error;
mod file_io;
mod macros;

use error::Error;
use tokio::time::sleep;

/// The URL to the Software Licensor Public Key repository. Change this if you 
/// have built the code for yourself.
const PUBLIC_KEY_REPO_URL: &str = "https://software-licensor-public-keys.s3.amazonaws.com/public_keys";
const LICENSE_ACTIVATION_URL: &str = "https://01lzc0nx9e.execute-api.us-east-1.amazonaws.com/v2/license_activation_refactor";

#[cxx::bridge]
mod ffi {
    struct LicenseData {
        result_code: i32,
        customer_first_name: String,
        customer_last_name: String,
        customer_email: String,
        license_type: String,
        version: String,
        error_message: String
    }

    extern "Rust" {
        fn read_reply_from_webserver(company_name_str: String, store_id_str: String, machine_id_str: String, license_code_str: String, product_ids_and_pubkeys: Vec<String>) -> LicenseData;
        fn check_license_with_potential_api_request(company_name_str: String, store_id_str: String, machine_id_str: String, product_ids_and_pubkeys: Vec<String>) -> LicenseData;
        fn check_license_no_api_request(company_name_str: String, store_id_str: String, machine_id_str: String, product_ids_and_pubkeys: Vec<String>) -> LicenseData;
    }
}

impl LicenseData {
    pub(crate) fn new(int_result: c_int, first_name: &str, last_name: &str, email: &str, license_type: &str, version: &str, error_message: &str) -> Self {
        Self {
            result_code: int_result,
            customer_first_name: first_name.to_string(),
            customer_last_name: last_name.to_string(),
            customer_email: email.to_string(),
            license_type: license_type.to_string(),
            version: version.to_string(),
            error_message: error_message.to_string()
        }
    }
    pub(crate) fn error(message: &str) -> Self {
        Self::new(-1, "Error", "Error", "Error", "Error", "Error", message)
    }
    pub(crate) fn from_key_file_and_license_response(key_file: &LicenseKeyFile, license_response: &LicenseActivationResponse, status_code: c_int) -> Self {
        Self::new(status_code, &license_response.customer_first_name, &license_response.customer_last_name, &license_response.customer_email, &key_file.license_type, &key_file.product_version, "")
    }
    pub(crate) fn licensing_error(code: c_int) -> Self {
        Self::new(code, "", "", "", "", "", "")
    }
}

#[no_mangle]
#[inline(always)]
pub extern "C" fn read_reply_from_webserver(company_name_str: String, store_id_str: String, machine_id_str: String, license_code_str: String, product_ids_and_pubkeys: Vec<String>) -> LicenseData {
    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return LicenseData::error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1")
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = match Runtime::new() {
        Ok(v) => v,
        Err(_) => return LicenseData::error("Error creating runtime")
    };

    rt.block_on(async {
        let mut license_file = match get_or_init_license_file(&company_name_str).await {
            Ok(v) => v,
            Err(e) => return LicenseData::error(&e.to_string())
        };
        sleep(Duration::from_secs(5)).await;
        match activate_license_request(&store_id_str, &company_name_str, &product_ids_and_pubkeys_hashmap.keys().collect::<Vec<&String>>(), &machine_id_str, &license_code_str, &mut license_file).await {
            Ok(()) => (),
            Err(v) => {
                match v {
                    Error::LicensingError(e) => return LicenseData::licensing_error(e as i32),
                    _ => return LicenseData::error(&v.to_string())
                }
            }
        };
        match check_key_file_async(&store_id_str, &company_name_str, &product_ids_and_pubkeys_hashmap, &machine_id_str, true).await {
            Ok(v) => return v,
            Err(e) => {
                match e {
                    Error::LicensingError(c) => return LicenseData::licensing_error(c as i32),
                    _ => return LicenseData::error(&e.to_string())
                }
            }
        }
    })
}

/// Checks the license and returns the result, potentially making an API 
/// request.
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
pub extern "C" fn check_license_with_potential_api_request(company_name_str: String, store_id_str: String, machine_id_str: String, product_ids_and_pubkeys: Vec<String>) -> LicenseData {
    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return LicenseData::error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1")
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = match Runtime::new() {
        Ok(v) => v,
        Err(_) => return LicenseData::error("Error creating runtime")
    };

    rt.block_on(async {
        match check_key_file_async(&store_id_str, &company_name_str, &product_ids_and_pubkeys_hashmap, &machine_id_str, false).await {
            Ok(v) => {
                return v;
            },
            Err(e) => {
                match e {
                    Error::LicensingError(v) => {
                        let r = LicenseData::licensing_error(v as i32);
                        return r
                    },
                    _ => {
                        let r = LicenseData::error(e.to_string().as_str());
                        return r
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
pub extern "C" fn check_license_no_api_request(company_name_str: String, store_id_str: String, machine_id_str: String, product_ids_and_pubkeys: Vec<String>) -> LicenseData {
    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return LicenseData::error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1")
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = match Runtime::new() {
        Ok(v) => v,
        Err(_) => return LicenseData::error("Error creating runtime")
    };

    rt.block_on(async {
        match check_key_file_async(&store_id_str, &company_name_str, &product_ids_and_pubkeys_hashmap, &machine_id_str, true).await {
            Ok(v) => {
                return v
            },
            Err(e) => {
                match e {
                    Error::LicensingError(v) => {
                        let r = LicenseData::licensing_error(v as i32);
                        return r
                    },
                    _ => {
                        let r = LicenseData::error(e.to_string().as_str());
                        return r
                    }
                }
            }
        }
    })
}