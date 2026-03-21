use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::CStr;

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub cpu_vendor: String,
    pub cpu_model: String,
    pub ram_mb: u32,
    pub num_physical_cores: u32,
    pub cpu_freq_mhz: u32,
    pub users_language: String,
    pub display_language: String,
    pub computer_name: String,
}

impl Stats {
    /// Collects the stats for the current device. This may be a slow operation, 
    /// so it's recommended to call this only once and cache the result. 
    /// 
    /// # Safety
    /// This function may call platform-specific APIs that are unsafe. However, 
    /// it should not cause any harm to the system.
    pub unsafe fn collect() -> Self {
        platform::collect()
    }
}

pub fn device_id() -> String {
    platform::get_device_id()
}

pub struct Language {
    pub display_language: String,
    pub users_language: String,
}
pub fn language() -> Language {
    platform::get_language()
}

fn sha256_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn env_locale_fallback() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

fn mib_to_u32(bytes: u64) -> u32 {
    (bytes / (1024 * 1024)).min(u32::MAX as u64) as u32
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::Globalization::{
        GetUserDefaultLocaleName, GetUserDefaultUILanguage, LCIDToLocaleName,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };
    use windows_sys::Win32::System::SystemInformation::{
        ComputerNamePhysicalDnsHostname, GetComputerNameExW, GetLogicalProcessorInformationEx,
        GetSystemFirmwareTable, GlobalMemoryStatusEx, RelationProcessorCore,
        LOGICAL_PROCESSOR_RELATIONSHIP, MEMORYSTATUSEX, PROCESSOR_RELATIONSHIP,
        SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    };

    pub fn get_device_id() -> String {
        let cpu_vendor = cpu_vendor().unwrap_or_default();
        let cpu_model = cpu_model().unwrap_or_default();
        
        let smbios_uuid = smbios_system_uuid().unwrap_or_default();
        let smbios_serial = smbios_system_serial().unwrap_or_default();
        let board_serial = smbios_baseboard_serial().unwrap_or_default();

        super::sha256_hex(&[
            "windows",
            &cpu_vendor,
            &cpu_model,
            &smbios_uuid,
            &smbios_serial,
            &board_serial,
        ])
    }

    pub fn get_language() -> Language {
        let users_language = user_locale_name().unwrap_or_default();
        let display_language = display_language_name().unwrap_or_default();
        Language {
            users_language,
            display_language,
        }
    }

    pub fn collect() -> Stats {
        let cpu_vendor = cpu_vendor().unwrap_or_default();
        let cpu_model = cpu_model().unwrap_or_default();
        let ram_mb = ram_mb().unwrap_or(0);
        let num_physical_cores = num_physical_cores().unwrap_or(0);
        let cpu_freq_mhz = cpu_freq_mhz().unwrap_or(0);
        let users_language = user_locale_name().unwrap_or_default();
        let display_language = display_language_name().unwrap_or_default();
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

    fn utf16_buf_to_string(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn user_locale_name() -> Option<String> {
        let mut buf = [0u16; 85];
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if len > 0 {
            Some(utf16_buf_to_string(&buf))
        } else {
            None
        }
    }

    fn display_language_name() -> Option<String> {
        let lang_id = unsafe { GetUserDefaultUILanguage() };
        if lang_id == 0 {
            return None;
        }

        let mut buf = [0u16; 85];
        let lcid = lang_id as u32;
        let len = unsafe { LCIDToLocaleName(lcid, buf.as_mut_ptr(), buf.len() as i32, 0) };
        if len > 0 {
            Some(utf16_buf_to_string(&buf))
        } else {
            None
        }
    }

    fn computer_name() -> Option<String> {
        let mut size = 0u32;
        unsafe {
            GetComputerNameExW(ComputerNamePhysicalDnsHostname, null_mut(), &mut size);
        }
        if size == 0 {
            return None;
        }

        let mut buf = vec![0u16; size as usize + 1];
        let ok = unsafe {
            GetComputerNameExW(
                ComputerNamePhysicalDnsHostname,
                buf.as_mut_ptr(),
                &mut size,
            )
        };
        if ok == 0 {
            None
        } else {
            Some(String::from_utf16_lossy(&buf[..size as usize]))
        }
    }

    fn ram_mb() -> Option<u32> {
        let mut mem: MEMORYSTATUSEX = unsafe { zeroed() };
        mem.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        let ok = unsafe { GlobalMemoryStatusEx(&mut mem) };
        if ok == 0 {
            None
        } else {
            Some(super::mib_to_u32(mem.ullTotalPhys))
        }
    }

    fn num_physical_cores() -> Option<u32> {
        let mut len = 0u32;
        unsafe {
            GetLogicalProcessorInformationEx(RelationProcessorCore, null_mut(), &mut len);
        }
        if len == 0 {
            return None;
        }

        let mut buf = vec![0u8; len as usize];
        let ok = unsafe {
            GetLogicalProcessorInformationEx(
                RelationProcessorCore,
                buf.as_mut_ptr() as *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
                &mut len,
            )
        };
        if ok == 0 {
            return None;
        }

        let mut count = 0u32;
        let mut offset = 0usize;
        while offset + size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>() <= buf.len() {
            let info = unsafe {
                &*(buf.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX)
            };
            if info.Relationship == RelationProcessorCore {
                count = count.saturating_add(1);
            }
            let size = info.Size as usize;
            if size == 0 {
                break;
            }
            offset += size;
        }

        Some(count)
    }

    fn cpu_freq_mhz() -> Option<u32> {
        let subkey = wide(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0");
        let value_name = wide("~MHz");

        let mut hkey: HKEY = 0;
        let open = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                0,
                KEY_READ,
                &mut hkey,
            )
        };
        if open != 0 {
            return None;
        }

        let mut data_type = 0u32;
        let mut data = 0u32;
        let mut data_len = size_of::<u32>() as u32;

        let query = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                null(),
                &mut data_type,
                &mut data as *mut _ as *mut u8,
                &mut data_len,
            )
        };

        unsafe {
            RegCloseKey(hkey);
        }

        if query == 0 {
            Some(data)
        } else {
            None
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn cpu_vendor() -> Option<String> {
        let mut bytes = [0u8; 12];

        #[cfg(target_arch = "x86")]
        use std::arch::x86::__cpuid;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::__cpuid;

        let leaf0 = unsafe { __cpuid(0) };
        bytes[0..4].copy_from_slice(&leaf0.ebx.to_le_bytes());
        bytes[4..8].copy_from_slice(&leaf0.edx.to_le_bytes());
        bytes[8..12].copy_from_slice(&leaf0.ecx.to_le_bytes());

        Some(String::from_utf8_lossy(&bytes).trim().to_string())
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    fn cpu_vendor() -> Option<String> {
        None
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn cpu_model() -> Option<String> {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::__cpuid;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::__cpuid;

        let max_ext = unsafe { __cpuid(0x80000000) }.eax;
        if max_ext < 0x80000004 {
            return None;
        }

        let mut brand = Vec::with_capacity(48);
        for leaf in [0x80000002, 0x80000003, 0x80000004] {
            let r = unsafe { __cpuid(leaf) };
            brand.extend_from_slice(&r.eax.to_le_bytes());
            brand.extend_from_slice(&r.ebx.to_le_bytes());
            brand.extend_from_slice(&r.ecx.to_le_bytes());
            brand.extend_from_slice(&r.edx.to_le_bytes());
        }

        let s = String::from_utf8_lossy(&brand)
            .trim_matches(char::from(0))
            .trim()
            .to_string();

        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    fn cpu_model() -> Option<String> {
        None
    }

    fn get_raw_smbios() -> Option<Vec<u8>> {
        let sig = u32::from_le_bytes(*b"RSMB");
        let needed = unsafe { GetSystemFirmwareTable(sig, 0, null_mut(), 0) };
        if needed == 0 {
            return None;
        }

        let mut buf = vec![0u8; needed as usize];
        let got = unsafe {
            GetSystemFirmwareTable(sig, 0, buf.as_mut_ptr() as *mut _, needed)
        };
        if got == 0 || got as usize != buf.len() {
            None
        } else {
            Some(buf)
        }
    }

    fn smbios_table_bytes(raw: &[u8]) -> Option<&[u8]> {
        if raw.len() < 8 {
            None
        } else {
            Some(&raw[8..])
        }
    }

    #[derive(Debug)]
    struct SmbiosStruct<'a> {
        ty: u8,
        formatted: &'a [u8],
        strings: Vec<&'a str>,
    }

    fn parse_smbios_structs(table: &[u8]) -> Vec<SmbiosStruct<'_>> {
        let mut out = Vec::new();
        let mut i = 0usize;

        while i + 4 <= table.len() {
            let ty = table[i];
            let len = table[i + 1] as usize;
            if len < 4 || i + len > table.len() {
                break;
            }

            let formatted = &table[i..i + len];
            let mut strings = Vec::new();
            let mut p = i + len;

            while p < table.len() {
                if p + 1 < table.len() && table[p] == 0 && table[p + 1] == 0 {
                    p += 2;
                    break;
                }

                let start = p;
                while p < table.len() && table[p] != 0 {
                    p += 1;
                }
                if p <= table.len() {
                    if let Ok(s) = std::str::from_utf8(&table[start..p]) {
                        strings.push(s);
                    }
                }
                if p < table.len() {
                    p += 1;
                }
            }

            out.push(SmbiosStruct {
                ty,
                formatted,
                strings,
            });

            i = p;
        }

        out
    }

    fn smbios_string<'a>(s: &'a SmbiosStruct<'a>, idx: usize) -> Option<String> {
        if idx == 0 {
            return None;
        }
        s.strings.get(idx - 1).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    }

    fn smbios_system_uuid() -> Option<String> {
        let raw = get_raw_smbios()?;
        let table = smbios_table_bytes(&raw)?;
        for s in parse_smbios_structs(table) {
            if s.ty == 1 && s.formatted.len() >= 0x19 {
                let uuid = &s.formatted[8..24];
                let all_zero = uuid.iter().all(|&b| b == 0);
                let all_ff = uuid.iter().all(|&b| b == 0xFF);
                if all_zero || all_ff {
                    continue;
                }

                let d1 = u32::from_le_bytes([uuid[0], uuid[1], uuid[2], uuid[3]]);
                let d2 = u16::from_le_bytes([uuid[4], uuid[5]]);
                let d3 = u16::from_le_bytes([uuid[6], uuid[7]]);
                let d4 = &uuid[8..10];
                let d5 = &uuid[10..16];

                return Some(format!(
                    "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    d4[0], d4[1], d5[0], d5[1], d5[2], d5[3], d5[4], d5[5]
                ));
            }
        }
        None
    }

    fn smbios_system_serial() -> Option<String> {
        let raw = get_raw_smbios()?;
        let table = smbios_table_bytes(&raw)?;
        for s in parse_smbios_structs(table) {
            if s.ty == 1 && s.formatted.len() >= 8 {
                let serial_idx = s.formatted[7] as usize;
                return smbios_string(&s, serial_idx);
            }
        }
        None
    }

    fn smbios_baseboard_serial() -> Option<String> {
        let raw = get_raw_smbios()?;
        let table = smbios_table_bytes(&raw)?;
        for s in parse_smbios_structs(table) {
            if s.ty == 2 && s.formatted.len() >= 8 {
                let serial_idx = s.formatted[7] as usize;
                return smbios_string(&s, serial_idx);
            }
        }
        None
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use libc::{gethostname, sysinfo as libc_sysinfo, sysinfo as sysinfo_struct};
    use std::fs;

    pub fn get_language() -> Language {
        let users_language = super::env_locale_fallback();
        let display_language = users_language.clone();
        Language { display_language, users_language }
    }

    pub fn get_device_id() -> String {
        let cpu_vendor = parse_cpuinfo_value("vendor_id")
            .or_else(|| parse_cpuinfo_value("CPU implementer"))
            .unwrap_or_default();

        let cpu_model = parse_cpuinfo_value("model name")
            .or_else(|| parse_cpuinfo_value("Hardware"))
            .or_else(|| parse_cpuinfo_value("Processor"))
            .unwrap_or_default();

        let cpu_freq_mhz = cpu_freq_mhz().unwrap_or(0);
        let ram_mb = ram_mb().unwrap_or(0);
        let num_physical_cores = num_physical_cores().unwrap_or(0);

        let product_uuid = read_trimmed("/sys/class/dmi/id/product_uuid").unwrap_or_default();
        let board_serial = read_trimmed("/sys/class/dmi/id/board_serial").unwrap_or_default();
        let board_name = read_trimmed("/sys/class/dmi/id/board_name").unwrap_or_default();

        super::sha256_hex(&[
            "linux",
            &cpu_vendor,
            &cpu_model,
            &product_uuid,
            &board_serial,
            &board_name,
        ])
    }

    pub fn collect() -> Stats {
        let cpu_vendor = parse_cpuinfo_value("vendor_id")
            .or_else(|| parse_cpuinfo_value("CPU implementer"))
            .unwrap_or_default();

        let cpu_model = parse_cpuinfo_value("model name")
            .or_else(|| parse_cpuinfo_value("Hardware"))
            .or_else(|| parse_cpuinfo_value("Processor"))
            .unwrap_or_default();

        let cpu_freq_mhz = cpu_freq_mhz().unwrap_or(0);
        let ram_mb = ram_mb().unwrap_or(0);
        let num_physical_cores = num_physical_cores().unwrap_or(0);

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

    fn read_trimmed(path: &str) -> Option<String> {
        let s = fs::read_to_string(path).ok()?;
        let s = s.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    fn parse_cpuinfo_value(key: &str) -> Option<String> {
        let content = fs::read_to_string("/proc/cpuinfo").ok()?;
        for line in content.lines() {
            let (k, v) = line.split_once(':')?;
            if k.trim() == key {
                let val = v.trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    fn cpu_freq_mhz() -> Option<u32> {
        if let Some(v) = read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq") {
            if let Ok(khz) = v.parse::<u64>() {
                return Some((khz / 1000).min(u32::MAX as u64) as u32);
            }
        }

        if let Some(v) = parse_cpuinfo_value("cpu MHz") {
            if let Ok(mhz) = v.parse::<f64>() {
                return Some(mhz.round().clamp(0.0, u32::MAX as f64) as u32);
            }
        }

        None
    }

    fn ram_mb() -> Option<u32> {
        let mut info: sysinfo_struct = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc_sysinfo(&mut info) };
        if rc != 0 {
            return None;
        }

        let total_bytes = (info.totalram as u128) * (info.mem_unit as u128);
        Some(super::mib_to_u32(total_bytes.min(u64::MAX as u128) as u64))
    }

    fn num_physical_cores() -> Option<u32> {
        let mut pairs = BTreeSet::<(String, String)>::new();

        let cpu_root = std::path::Path::new("/sys/devices/system/cpu");
        if let Ok(entries) = fs::read_dir(cpu_root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("cpu") || name.len() <= 3 || !name[3..].chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                let topo = entry.path().join("topology");
                let core_id = fs::read_to_string(topo.join("core_id")).ok().map(|s| s.trim().to_string());
                let pkg_id = fs::read_to_string(topo.join("physical_package_id")).ok().map(|s| s.trim().to_string());

                if let (Some(core_id), Some(pkg_id)) = (core_id, pkg_id) {
                    pairs.insert((pkg_id, core_id));
                }
            }
        }

        if !pairs.is_empty() {
            return Some(pairs.len().min(u32::MAX as usize) as u32);
        }

        let content = fs::read_to_string("/proc/cpuinfo").ok()?;
        let mut current_physical = String::new();
        let mut current_core = String::new();
        let mut found_any = false;

        for line in content.lines().chain(std::iter::once("")) {
            if line.trim().is_empty() {
                if !current_physical.is_empty() && !current_core.is_empty() {
                    pairs.insert((current_physical.clone(), current_core.clone()));
                    found_any = true;
                }
                current_physical.clear();
                current_core.clear();
                continue;
            }

            if let Some((k, v)) = line.split_once(':') {
                match k.trim() {
                    "physical id" => current_physical = v.trim().to_string(),
                    "core id" => current_core = v.trim().to_string(),
                    _ => {}
                }
            }
        }

        if found_any && !pairs.is_empty() {
            Some(pairs.len().min(u32::MAX as usize) as u32)
        } else {
            let logical = std::thread::available_parallelism().ok()?.get();
            Some(logical.min(u32::MAX as usize) as u32)
        }
    }

    fn computer_name() -> Option<String> {
        let mut buf = [0u8; 256];
        let rc = unsafe { gethostname(buf.as_mut_ptr() as *mut u8, buf.len()) };
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
}

#[cfg(target_os = "macos")]
mod platform {
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
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod platform {
    use super::*;

    pub fn collect() -> Stats {
        Stats::default()
    }
}