//! Windows-specific implementation of system stats collection.

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
    ComputerNamePhysicalDnsHostname, FIRMWARE_TABLE_PROVIDER, GetComputerNameExW, GetLogicalProcessorInformationEx, GetSystemFirmwareTable, GlobalMemoryStatusEx, LOGICAL_PROCESSOR_RELATIONSHIP, MEMORYSTATUSEX, PROCESSOR_RELATIONSHIP, RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX
};

fn machine_guid() -> Option<String> {
    let subkey = wide(r"SOFTWARE\Microsoft\Cryptography");
    let value_name = wide("MachineGuid");

    let mut hkey: HKEY = null_mut();
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
    let mut data_len = 0u32;

    // First call: get the required buffer size
    unsafe {
        RegQueryValueExW(
            hkey,
            value_name.as_ptr(),
            null(),
            &mut data_type,
            null_mut(),
            &mut data_len,
        )
    };

    if data_len == 0 {
        unsafe { RegCloseKey(hkey) };
        return None;
    }

    let mut buf = vec![0u16; (data_len / 2 + 1) as usize];
    let query = unsafe {
        RegQueryValueExW(
            hkey,
            value_name.as_ptr(),
            null(),
            &mut data_type,
            buf.as_mut_ptr() as *mut u8,
            &mut data_len,
        )
    };

    unsafe { RegCloseKey(hkey) };

    if query == 0 {
        Some(utf16_buf_to_string(&buf))
    } else {
        None
    }
}

pub fn get_language() -> Language {
    let users_language = user_locale_name().unwrap_or_default();
    let display_language = display_language_name().unwrap_or_default();
    Language {
        users_language,
        display_language,
    }
}

pub async fn collect() -> Stats {
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
        gpu_info: super::detect_primary_gpu_info().await
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

pub fn computer_name() -> Option<String> {
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

    let mut hkey: HKEY = null_mut();
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
#[inline(always)]
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
#[inline(always)]
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

fn generate_sig(input: &[u8; 4]) -> u32 {
    let mut result = 0;
    input.iter().rev().zip(&[0, 8, 16, 24]).for_each(|n| {
        result |= (*n.0 as u32) << n.1;
    });
    result
}

fn get_raw_smbios() -> Option<Vec<u8>> {
    let sig = generate_sig(b"RSMB");
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

#[inline(always)]
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

#[inline(always)]
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

#[inline(always)]
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

// GPU Stuff
pub fn windows_gpu_probe(
    vendor_id: u32,
    device_id: u32,
    adapter_name: &str,
) -> Option<PlatformGpuProbe> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1};

    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;

        let mut index = 0;
        loop {
            let adapter: IDXGIAdapter1 = match factory.EnumAdapters1(index) {
                Ok(a) => a,
                Err(_) => break,
            };
            index += 1;

            let desc = adapter.GetDesc1().ok()?;
            let desc_name = utf16_array_to_string(&desc.Description);

            let name_match = !adapter_name.is_empty() && desc_name.eq_ignore_ascii_case(adapter_name);
            let id_match = desc.VendorId == vendor_id && desc.DeviceId == device_id;

            if name_match || id_match {
                return Some(PlatformGpuProbe {
                    vram_bytes: Some(desc.DedicatedVideoMemory as u64),
                    unified_memory: Some(desc.DedicatedVideoMemory == 0),
                });
            }
        }
    }

    None
}

fn utf16_array_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

use sha2::{Digest, Sha256};

#[inline(always)]
pub fn get_device_id() -> String {
    let windows = get_platform_prefix_char();
    let fingerprint = Hardware::build_selected_fingerprint_bytes();

    if fingerprint.is_empty() {
        debug_assert!(false);
        return String::new();
    }

    let digest = sha256_bytes(&[
        b"windows",
        &fingerprint,
    ]);

    format!("{windows}{}", hex::encode_upper(digest))
}

#[inline(always)]
fn get_platform_prefix_char() -> char {
    'W'
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
fn push_tagged_bytes(out: &mut Vec<u8>, structure_type: u8, field_tag: u8, bytes: &[u8]) {
    out.push(structure_type);
    out.push(field_tag);

    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

#[inline(always)]
fn push_tagged_str(out: &mut Vec<u8>, structure_type: u8, field_tag: u8, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    push_tagged_bytes(out, structure_type, field_tag, trimmed.as_bytes());
}

#[inline(always)]
fn get_smbios_string<'a>(s: &'a SmbiosStruct<'a>, one_based_index: usize) -> Option<&'a str> {
    if one_based_index == 0 {
        return None;
    }

    s.strings
        .get(one_based_index - 1)
        .copied()
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

#[inline(always)]
fn get_formatted_string_field<'a>(
    s: &'a SmbiosStruct<'a>,
    offset: usize,
) -> Option<&'a str> {
    let index = *s.formatted.get(offset)? as usize;
    get_smbios_string(s, index)
}

#[inline(always)]
fn push_uuid_field_if_valid(
    out: &mut Vec<u8>,
    structure_type: u8,
    field_tag: u8,
    uuid: &[u8],
) {
    if uuid.len() != 16 {
        return;
    }

    let all_zero = uuid.iter().all(|&b| b == 0);
    let all_ff = uuid.iter().all(|&b| b == 0xFF);

    if all_zero || all_ff {
        return;
    }

    push_tagged_bytes(out, structure_type, field_tag, uuid);
}

pub struct Hardware;

impl Hardware {
    #[inline(always)]
    pub fn build_selected_fingerprint_bytes() -> Vec<u8> {
        let raw = match get_raw_smbios() {
            Some(v) => v,
            None => return Vec::new(),
        };

        let table = match smbios_table_bytes(&raw) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let structs = parse_smbios_structs(table);
        let mut out = Vec::with_capacity(512);

        // Version marker so you can evolve the format later.
        out.extend_from_slice(b"SMBIOS-FP-V2");

        for s in structs {
            match s.ty {
                1 => {
                    // SMBIOS Type 1 (System Information)
                    // See DMTF SMBIOS spec (DSP0134), section "System Information (Type 1)".
                    // 04h manufacturer string index
                    // 05h product name string index
                    // 06h version string index
                    // 07h serial string index
                    // 08h..17h UUID, 16 bytes
                    // 19h SKU string index
                    // 1Ah family string index

                    if let Some(v) = get_formatted_string_field(&s, 0x04) {
                        push_tagged_str(&mut out, 1, 1, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x05) {
                        push_tagged_str(&mut out, 1, 2, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x07) {
                        push_tagged_str(&mut out, 1, 4, v);
                    }
                    if s.formatted.len() >= 24 {
                        push_uuid_field_if_valid(&mut out, 1, 5, &s.formatted[8..24]);
                    } else {
                        debug_assert!(false)
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x19) {
                        push_tagged_str(&mut out, 1, 6, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x1A) {
                        push_tagged_str(&mut out, 1, 7, v);
                    }
                }

                2 => {
                    // SMBIOS Type 2 (Baseboard Information)
                    // See DMTF SMBIOS spec (DSP0134), section "Baseboard Information (Type 2)".
                    // 04h manufacturer
                    // 05h product
                    // 06h version
                    // 07h serial
                    // 08h asset tag

                    if let Some(v) = get_formatted_string_field(&s, 0x04) {
                        push_tagged_str(&mut out, 2, 1, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x05) {
                        push_tagged_str(&mut out, 2, 2, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x07) {
                        push_tagged_str(&mut out, 2, 4, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x08) {
                        push_tagged_str(&mut out, 2, 5, v);
                    }
                }

                4 => {
                    // SMBIOS Type 4 (Processor Information)
                    // See DMTF SMBIOS spec (DSP0134), section "Processor Information (Type 4)".
                    // 07h manufacturer
                    // 10h version
                    // 21h asset tag
                    // 22h part number

                    if let Some(v) = get_formatted_string_field(&s, 0x07) {
                        push_tagged_str(&mut out, 4, 1, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x21) {
                        push_tagged_str(&mut out, 4, 3, v);
                    }
                    if let Some(v) = get_formatted_string_field(&s, 0x22) {
                        push_tagged_str(&mut out, 4, 4, v);
                    }
                }

                _ => {}
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_id_compatibility() {
        let expected = "W7DC966697DD63E544BC7C4FD289DFBA8BB6642E86034140C9260E0091A580E26";
        let retrieved = get_device_id();
        //let retrieved = SystemStats::get_unique_device_id();
        assert_eq!(retrieved, expected);
    }
}