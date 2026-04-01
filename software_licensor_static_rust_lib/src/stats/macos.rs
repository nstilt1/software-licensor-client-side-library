//! macOS-specific implementation of system stats collection.

use super::*;
use libc::{c_char, c_int, c_void, gethostname, size_t};
use std::mem::size_of;

extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *mut c_void,
        newlen: size_t,
    ) -> c_int;
}

pub fn get_language() -> Language {
    let users_language = super::env_locale_fallback();
    let display_language = users_language.clone();
    Language { display_language, users_language }
}

#[inline(always)]
pub fn get_device_id() -> String {
    let cpu_vendor = sysctl_string("machdep.cpu.vendor").unwrap_or_default();
    let cpu_model = sysctl_string("machdep.cpu.brand_string")
        .or_else(|| sysctl_string("hw.model"))
        .unwrap_or_default();

    let hw_model = sysctl_string("hw.model").unwrap_or_default();
    let board_id = sysctl_string("hw.target").unwrap_or_default();

    super::sha256_hex(&[
        "macos",
        &cpu_vendor,
        &cpu_model,
        &hw_model,
        &board_id,
    ])
}
pub fn collect() -> Stats {
    let cpu_vendor = sysctl_string("machdep.cpu.vendor").unwrap_or_default();
    let cpu_model = sysctl_string("machdep.cpu.brand_string")
        .or_else(|| sysctl_string("hw.model"))
        .unwrap_or_default();

    let ram_mb = sysctl_u64("hw.memsize")
        .map(super::mib_to_u32)
        .unwrap_or(0);

    let num_physical_cores = sysctl_u32("hw.physicalcpu").unwrap_or(0);

    let cpu_freq_mhz = sysctl_u64("hw.cpufrequency")
        .map(|hz| (hz / 1_000_000).min(u32::MAX as u64) as u32)
        .unwrap_or(0);

    let users_language = super::env_locale_fallback();
    let display_language = users_language.clone();
    let computer_name = computer_name().unwrap_or_default();

    Stats {
        cpu_vendor,
        cpu_model,
        ram_mb,
        num_physical_cores,
        cpu_freq_mhz,
        users_language,
        display_language,
        computer_name,
    }
}

fn sysctl_string(name: &str) -> Option<String> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut len: size_t = 0;
    let rc = unsafe {
        sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len == 0 {
        return None;
    }

    let mut buf = vec![0u8; len];
    let rc = unsafe {
        sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len == 0 {
        return None;
    }

    let end = buf.iter().position(|&b| b == 0).unwrap_or(len);
    let s = String::from_utf8_lossy(&buf[..end]).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn sysctl_u32(name: &str) -> Option<u32> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut value: u32 = 0;
    let mut len: size_t = size_of::<u32>();
    let rc = unsafe {
        sysctlbyname(
            cname.as_ptr(),
            &mut value as *mut _ as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc == 0 && len == size_of::<u32>() {
        Some(value)
    } else {
        None
    }
}

fn sysctl_u64(name: &str) -> Option<u64> {
    let cname = std::ffi::CString::new(name).ok()?;
    let mut value: u64 = 0;
    let mut len: size_t = size_of::<u64>();
    let rc = unsafe {
        sysctlbyname(
            cname.as_ptr(),
            &mut value as *mut _ as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc == 0 && len == size_of::<u64>() {
        Some(value)
    } else {
        None
    }
}

fn computer_name() -> Option<String> {
    let mut buf = [0u8; 256];
    let rc = unsafe { gethostname(buf.as_mut_ptr() as *mut i8, buf.len()) };
    if rc != 0 {
        return None;
    }

    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let s = String::from_utf8_lossy(&buf[..end]).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
