#![deny(clippy::unwrap_used)]
#![allow(clippy::enum_variant_names)]

use std::collections::HashMap;
use std::os::raw::{c_char, c_int};
use std::ffi::{CString, CStr};
use std::time::Duration;

use api::activate_license_request;
use file_io::{check_key_file_async, get_or_init_hwinfo_file, get_or_init_license_file, save_hw_info_file};
use generated::software_licensor_client::{LicenseActivationResponse, LicenseKeyFile, Stats};
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

impl LicenseData {
    pub(crate) fn new(
        int_result: c_int, 
        first_name: &str, 
        last_name: &str, 
        email: &str, 
        license_type: &str, 
        version: &str, 
        error_message: &str, 
        license_code: &str
    ) -> Self {
        Self {
            result_code: int_result,
            customer_first_name: CString::new(first_name).expect("CString::new failed").into_raw(),
            customer_last_name: CString::new(last_name).expect("CString::new failed").into_raw(),
            customer_email: CString::new(email).expect("CString::new failed").into_raw(),
            license_type: CString::new(license_type).expect("CString::new failed").into_raw(),
            version: CString::new(version).expect("CString::new failed").into_raw(),
            error_message: CString::new(error_message).expect("CString::new failed").into_raw(),
            license_code: CString::new(license_code).expect("CString::new failed").into_raw()
        }
    }
    pub(crate) fn error(message: &str) -> Self {
        Self::new(
            -1, 
            "Error", 
            "Error", 
            "Error", 
            "Error", 
            "Error", 
            message, 
            "Error"
        )
    }
    pub(crate) fn from_key_file_and_license_response(key_file: &LicenseKeyFile, license_response: &LicenseActivationResponse, status_code: c_int) -> Self {
        Self::new(
            status_code, 
            &license_response.customer_first_name, 
            &license_response.customer_last_name, 
            &license_response.customer_email, 
            &key_file.license_type, 
            &key_file.product_version, 
            "",
            &key_file.license_code
        )
    }
    pub(crate) fn licensing_error(code: c_int, license_code: &str) -> Self {
        Self::new(code, "", "", "", "", "", "", license_code)
    }
}

/// Updates machine information in the license file. It should be optional for
/// the end user to have the stats saved, but there isn't a super convenient 
/// way to save them all, and there isn't a way for Rust code to grab all of 
/// the machine stats for all machines. These stats are readily available with 
/// the JUCE library.
#[no_mangle]
#[inline(always)]
pub extern "C" fn update_machine_info(
    save_system_stats: bool, 
    os_name: *const c_char, 
    computer_name: *const c_char, 
    is_64_bit: bool, 
    users_language: *const c_char, 
    display_language: *const c_char, 
    num_logical_cores: c_int, 
    num_physical_cores: c_int, 
    cpu_freq_mhz: c_int, 
    ram_mb: c_int, 
    page_size: c_int, 
    cpu_vendor: *const c_char, 
    cpu_model: *const c_char, 
    has_mmx: bool, 
    has_3d_now: bool, 
    has_fma3: bool, 
    has_fma4: bool, 
    has_sse: bool, 
    has_sse2: bool, 
    has_sse3: bool, 
    has_ssse3: bool, 
    has_sse41: bool, 
    has_sse42: bool,
    has_avx: bool,
    has_avx2: bool,
    has_avx512f: bool,
    has_avx512bw: bool,
    has_avx512cd: bool,
    has_avx512dq: bool,
    has_avx512er: bool,
    has_avx512ifma: bool,
    has_avx512pf: bool,
    has_avx512vbmi: bool,
    has_avx512vl: bool,
    has_avx512vpopcntdq: bool,
    has_neon: bool,
) {
    let os_name_str = parse_c_char!(os_name);
    let computer_name_str = parse_c_char!(computer_name);
    let users_language_str = parse_c_char!(users_language);
    let display_language_str = parse_c_char!(display_language);
    let cpu_vendor_str = parse_c_char!(cpu_vendor);
    let cpu_model_str = parse_c_char!(cpu_model);
    let rt = match Runtime::new() {
        Ok(v) => v,
        Err(_) => return
    };

    rt.block_on(async {
        let mut hw_info_file = match get_or_init_hwinfo_file() {
            Ok(v) => v,
            Err(_) => return
        };

        if !save_system_stats {
            hw_info_file.machine_stats = None;
            let _result = save_hw_info_file(&hw_info_file).unwrap_or_else(|_| ());
            sleep(Duration::from_secs(1)).await;
            return
        }

        let current_stats = Some(Stats {
            os_name: os_name_str.to_string(),
            computer_name: computer_name_str.to_string(),
            is_64_bit,
            users_language: users_language_str.to_string(),
            display_language: display_language_str.to_string(),
            num_logical_cores: num_logical_cores as u32,
            num_physical_cores: num_physical_cores as u32,
            cpu_freq_mhz: cpu_freq_mhz as u32,
            ram_mb: ram_mb as u32,
            page_size: page_size as u32,
            cpu_vendor: cpu_vendor_str.to_string(),
            cpu_model: cpu_model_str.to_string(),
            has_mmx,
            has_3d_now,
            has_fma3,
            has_fma4,
            has_sse,
            has_sse2,
            has_sse3,
            has_ssse3,
            has_sse41,
            has_sse42,
            has_avx,
            has_avx2,
            has_avx512f,
            has_avx512bw,
            has_avx512cd,
            has_avx512dq,
            has_avx512er,
            has_avx512ifma,
            has_avx512pf,
            has_avx512vbmi,
            has_avx512vl,
            has_avx512vpopcntdq,
            has_neon,
        });

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
pub extern "C" fn read_reply_from_webserver(company_name: *const c_char, store_id: *const c_char, machine_id: *const c_char, license_code: *const c_char, product_ids_and_pubkeys: *const *const c_char, len: c_int) -> *mut LicenseData {
    let store_id_str = parse_c_char!(store_id, "Failed to parse store id", true);
    let company_name_str = parse_c_char!(company_name, "Failed to parse company name", true);
    let machine_id_str = parse_c_char!(machine_id, "Failed to parse machine id", true);
    let license_code_str = parse_c_char!(license_code, "Failed to parse license code", true);

    let array_size = unsafe { std::slice::from_raw_parts(product_ids_and_pubkeys, len as usize) };
    
    let mut product_ids_and_pubkeys_vec: Vec<&str> = Vec::with_capacity(len as usize);
    for s in array_size.iter() {
        match unsafe { CStr::from_ptr(*s).to_str() } {
            Ok(v) => product_ids_and_pubkeys_vec.push(v),
            Err(_) => return box_out!(LicenseData::error("UTF-8 error when decoding product IDs and pubkeys"))
        }
    }

    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys_vec.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return box_out!(LicenseData::error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1"));
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = runtime!(true);

    rt.block_on(async {
        let mut license_file = match get_or_init_license_file(company_name_str).await {
            Ok(v) => v,
            Err(e) => return box_out!(LicenseData::error(&e.to_string()))
        };
        sleep(Duration::from_secs(5)).await;
        match activate_license_request(store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap.keys().collect::<Vec<&String>>(), machine_id_str, license_code_str, &mut license_file).await {
            Ok(()) => (),
            Err(v) => {
                match v {
                    Error::LicensingError(e) => return box_out!(LicenseData::licensing_error(e.get_error_code() as i32, license_code_str)),
                    _ => return box_out!(LicenseData::error(&v.to_string()))
                }
            }
        };
        match check_key_file_async(store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap, machine_id_str, false).await {
            Ok(v) => return box_out!(v),
            Err(e) => {
                match e {
                    Error::LicensingError(error) => return box_out!(LicenseData::licensing_error(error.get_error_code() as i32, license_code_str)),
                    _ => return box_out!(LicenseData::error(&e.to_string()))
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
pub extern "C" fn check_license(company_name: *const c_char, store_id: *const c_char, machine_id: *const c_char, product_ids_and_pubkeys: *const *const c_char, len: c_int) -> *mut LicenseData {
    let store_id_str = parse_c_char!(store_id, "Failed to parse store id", true);
    let company_name_str = parse_c_char!(company_name, "Failed to parse company name", true);
    let machine_id_str = parse_c_char!(machine_id, "Failed to parse machine id", true);

    let array_size = unsafe { std::slice::from_raw_parts(product_ids_and_pubkeys, len as usize) };
    
    let mut product_ids_and_pubkeys_vec: Vec<&str> = Vec::with_capacity(len as usize);
    for s in array_size.iter() {
        match unsafe { CStr::from_ptr(*s).to_str() } {
            Ok(v) => product_ids_and_pubkeys_vec.push(v),
            Err(_) => {
                return box_out!(LicenseData::error("UTF-8 error when decoding product IDs and pubkeys"))
            }
        }
    }

    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys_vec.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return box_out!(LicenseData::error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1"))
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = runtime!(true);

    rt.block_on(async {
        match check_key_file_async(store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap, machine_id_str, true).await {
            Ok(v) => {
                box_out!(v)
            },
            Err(e) => {
                match e {
                    Error::LicensingError(v) => {
                        let r = LicenseData::licensing_error(v.get_error_code() as i32, &v.get_license_code());
                        box_out!(r)
                    },
                    _ => {
                        let r = LicenseData::error(e.to_string().as_str());
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
pub extern "C" fn check_license_no_api_request(company_name: *const c_char, store_id: *const c_char, machine_id: *const c_char, product_ids_and_pubkeys: *const *const c_char, len: c_int) -> *mut LicenseData {
    let store_id_str = parse_c_char!(store_id, "Failed to parse store id", true);
    let company_name_str = parse_c_char!(company_name, "Failed to parse company name", true);
    let machine_id_str = parse_c_char!(machine_id, "Failed to parse machine id", true);

    let array_size = unsafe { std::slice::from_raw_parts(product_ids_and_pubkeys, len as usize) };
    
    let mut product_ids_and_pubkeys_vec: Vec<&str> = Vec::with_capacity(len as usize);
    for s in array_size.iter() {
        match unsafe { CStr::from_ptr(*s).to_str() } {
            Ok(v) => product_ids_and_pubkeys_vec.push(v),
            Err(_) => return box_out!(LicenseData::error("UTF-8 error when decoding product IDs and pubkeys"))
        }
    }

    let mut product_ids_and_pubkeys_hashmap: HashMap<String, String> = HashMap::new();
    for product_id_and_key in product_ids_and_pubkeys_vec.iter() {
        let split = product_id_and_key.split(';').collect::<Vec<&str>>();
        if split.len() != 2 {
            return box_out!(LicenseData::error("product_ids_and_pubkeys contained a string with an amount of semicolons not equal to 1"))
        }
        product_ids_and_pubkeys_hashmap.insert(split[0].to_string(), split[1].to_string());
    }

    let rt = runtime!(true);

    rt.block_on(async {
        match check_key_file_async(store_id_str, company_name_str, &product_ids_and_pubkeys_hashmap, machine_id_str, false).await {
            Ok(v) => {
                return box_out!(v)
            },
            Err(e) => {
                match e {
                    Error::LicensingError(v) => {
                        let r = LicenseData::licensing_error(v.get_error_code() as i32, &v.get_license_code());
                        return box_out!(r)
                    },
                    _ => {
                        let r = LicenseData::error(e.to_string().as_str());
                        return box_out!(r)
                    }
                }
            }
        }
    })
}