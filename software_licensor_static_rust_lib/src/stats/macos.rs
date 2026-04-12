//! macOS-specific implementation of system stats collection.

use super::*;
use libc::{c_char, c_int, c_void, gethostname, size_t};
use std::mem::size_of;

/// Gets the computer name using gethostname.
pub fn computer_name() -> Option<String> {
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

pub fn get_language() -> Language {
    let users_language = super::env_locale_fallback();
    let display_language = users_language.clone();
    Language { display_language, users_language }
}

pub async fn collect() -> Stats {
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
        gpu_info: super::detect_primary_gpu_info().await
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

use sha2::{Digest, Sha256};
use std::ptr;

type KernReturn = i32;
type MachPort = u32;
type IoObjectT = MachPort;
type IoRegistryEntryT = IoObjectT;
type IoServiceT = IoObjectT;
type IoIteratorT = IoObjectT;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFTypeID = usize;
type Boolean = u8;
type SizeT = usize;

const KERN_SUCCESS: KernReturn = 0;
const K_IO_OBJECT_NULL: IoObjectT = 0;
const KCF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const KCF_ALLOCATOR_DEFAULT: CFAllocatorRef = ptr::null();

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingServices(
        master_port: MachPort,
        matching: *mut c_void,
        existing: *mut IoIteratorT,
    ) -> KernReturn;
    fn IOIteratorNext(iterator: IoIteratorT) -> IoObjectT;
    fn IOObjectRelease(object: IoObjectT) -> KernReturn;
    fn IORegistryEntryCreateCFProperty(
        entry: IoRegistryEntryT,
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> CFTypeRef;
    static kIOMainPortDefault: MachPort;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCStringPtr(the_string: CFStringRef, encoding: u32) -> *const c_char;
    fn CFStringGetCString(
        the_string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> Boolean;
    fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    fn CFStringGetTypeID() -> CFTypeID;
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "c")]
unsafe extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut SizeT,
        newp: *mut c_void,
        newlen: SizeT,
    ) -> c_int;
}

#[inline(always)]
pub fn get_device_id() -> String {
    let mut fingerprint = Vec::with_capacity(256);
    fingerprint.extend_from_slice(b"MACOS-FP-V1");

    if let Some(v) = io_platform_string("IOPlatformUUID") {
        push_tagged_str(&mut fingerprint, 1, 1, &v);
    } else {
        debug_assert!(false);
    }
    if let Some(v) = io_platform_string("IOPlatformSerialNumber") {
        push_tagged_str(&mut fingerprint, 1, 2, &v);
    } else {
        debug_assert!(false)
    }
    if let Some(v) = sysctl_string("hw.model") {
        push_tagged_str(&mut fingerprint, 1, 3, &v);
    } else {
        debug_assert!(false);
    }

    if fingerprint.len() == b"MACOS-FP-V1".len() {
        debug_assert!(false);
        return String::new();
    }

    let digest = sha256_bytes(&[b"macos", &fingerprint]);
    format!("M{}", hex::encode_upper(digest))
}

#[inline(always)]
fn sha256_bytes(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[inline(always)]
fn push_tagged_str(out: &mut Vec<u8>, structure_type: u8, field_tag: u8, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }

    out.push(structure_type);
    out.push(field_tag);

    let bytes = trimmed.as_bytes();
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

#[inline(always)]
fn io_platform_string(key: &str) -> Option<String> {
    let service = get_platform_expert_device()?;
    let result = unsafe { read_io_registry_string(service, key) };
    unsafe {
        let _ = IOObjectRelease(service);
    }
    result
}

#[inline(always)]
fn get_platform_expert_device() -> Option<IoServiceT> {
    let mut class_name = b"IOPlatformExpertDevice\0".to_vec();
    let matching = unsafe { IOServiceMatching(class_name.as_mut_ptr() as *const c_char) };
    if matching.is_null() {
        return None;
    }

    let mut iter: IoIteratorT = K_IO_OBJECT_NULL;
    let kr = unsafe { IOServiceGetMatchingServices(kIOMainPortDefault, matching, &mut iter) };
    if kr != KERN_SUCCESS || iter == K_IO_OBJECT_NULL {
        return None;
    }

    let service = unsafe { IOIteratorNext(iter) };
    unsafe {
        let _ = IOObjectRelease(iter);
    }

    if service == K_IO_OBJECT_NULL {
        None
    } else {
        Some(service)
    }
}

#[inline(always)]
unsafe fn read_io_registry_string(entry: IoRegistryEntryT, key: &str) -> Option<String> {
    unsafe {
        let cf_key = cfstring_from_str(key)?;
        let value = IORegistryEntryCreateCFProperty(entry, cf_key, KCF_ALLOCATOR_DEFAULT, 0);
        CFRelease(cf_key as CFTypeRef);

        if value.is_null() {
            return None;
        }

        let out = cfstring_to_rust_string(value as CFStringRef);
        CFRelease(value);
        out
    }
}

#[inline(always)]
unsafe fn cfstring_from_str(s: &str) -> Option<CFStringRef> {
    if s.as_bytes().contains(&0) {
        return None;
    }

    let mut nul = Vec::with_capacity(s.len() + 1);
    nul.extend_from_slice(s.as_bytes());
    nul.push(0);

    let cf = unsafe {
        CFStringCreateWithCString(
            KCF_ALLOCATOR_DEFAULT,
            nul.as_ptr() as *const c_char,
            KCF_STRING_ENCODING_UTF8,
        )
    };

    if cf.is_null() {
        None
    } else {
        Some(cf)
    }
}

#[inline(always)]
unsafe fn cfstring_to_rust_string(s: CFStringRef) -> Option<String> {
    unsafe {
        if s.is_null() || CFGetTypeID(s as CFTypeRef) != CFStringGetTypeID() {
            return None;
        }

        let direct = CFStringGetCStringPtr(s, KCF_STRING_ENCODING_UTF8);
        if !direct.is_null() {
            let len = libc::strlen(direct);
            let bytes = std::slice::from_raw_parts(direct as *const u8, len);
            let value = String::from_utf8_lossy(bytes).trim().to_string();
            return if value.is_empty() { None } else { Some(value) };
        }

        let mut buf = vec![0u8; 512];
        let ok = CFStringGetCString(
            s,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as isize,
            KCF_STRING_ENCODING_UTF8,
        );

        if ok == 0 {
            return None;
        }

        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let value = String::from_utf8_lossy(&buf[..end]).trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    }
}

#[inline(always)]
fn sysctl_string(name: &str) -> Option<String> {
    if name.as_bytes().contains(&0) {
        return None;
    }

    let mut cname = Vec::with_capacity(name.len() + 1);
    cname.extend_from_slice(name.as_bytes());
    cname.push(0);

    let mut len: SizeT = 0;
    let rc = unsafe {
        sysctlbyname(
            cname.as_ptr() as *const c_char,
            ptr::null_mut(),
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len == 0 {
        return None;
    }

    let mut buf = vec![0u8; len];
    let rc = unsafe {
        sysctlbyname(
            cname.as_ptr() as *const c_char,
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len == 0 {
        return None;
    }

    let used = len.min(buf.len());
    let end = buf[..used].iter().position(|&b| b == 0).unwrap_or(used);
    let value = String::from_utf8_lossy(&buf[..end]).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}


// GPU Stuff
pub fn macos_gpu_probe(adapter_name: &str) -> Option<PlatformGpuProbe> {
    let devices = metal::Device::all();

    let device = devices
        .into_iter()
        .find(|d| d.name() == adapter_name)
        .or_else(|| metal::Device::all().into_iter().next())?;

    Some(PlatformGpuProbe {
        vram_bytes: Some(device.recommended_max_working_set_size()),
        unified_memory: Some(device.has_unified_memory()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let retrieved = get_device_id();
        let retrieved = &retrieved[0..11];
        assert_eq!(retrieved, "M7AA99C679A");
    }
}