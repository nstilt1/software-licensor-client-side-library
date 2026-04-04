//! Linux-specific implementation of system stats collection.

use super::*;
use libc::{gethostname, sysinfo as libc_sysinfo, sysinfo as sysinfo_struct};
use std::fs;

pub fn get_language() -> Language {
    let users_language = super::env_locale_fallback();
    let display_language = users_language.clone();
    Language { display_language, users_language }
}

#[inline(always)]
pub fn get_device_id() -> String {
    let cpu_vendor = parse_cpuinfo_value("vendor_id")
        .or_else(|| parse_cpuinfo_value("CPU implementer"))
        .unwrap_or_default();

    let cpu_model = parse_cpuinfo_value("model name")
        .or_else(|| parse_cpuinfo_value("Hardware"))
        .or_else(|| parse_cpuinfo_value("Processor"))
        .unwrap_or_default();

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

pub async fn collect() -> Stats {
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
        gpu_info: super::detect_primary_gpu_info().await
    }
}

#[inline(always)]
fn read_trimmed(path: &str) -> Option<String> {
    let s = fs::read_to_string(path).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[inline(always)]
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

// GPU STUFF
pub fn linux_gpu_probe_vulkan(
    vendor_id: u32,
    device_id: u32,
    adapter_name: &str,
) -> Option<PlatformGpuProbe> {
    use ash::vk;
    use std::ffi::{CStr, CString};

    unsafe {
        let entry = ash::Entry::load().ok()?;

        let app_name = CString::new("gpu_probe").ok()?;
        let engine_name = CString::new("gpu_probe").ok()?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(&engine_name)
            .api_version(vk::make_api_version(0, 1, 0, 0));

        let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);

        let instance = entry.create_instance(&create_info, None).ok()?;
        let result = (|| {
            let physical_devices = instance.enumerate_physical_devices().ok()?;

            for physical_device in physical_devices {
                let props = instance.get_physical_device_properties(physical_device);
                let mem = instance.get_physical_device_memory_properties(physical_device);

                let vk_name = CStr::from_ptr(props.device_name.as_ptr())
                    .to_str()
                    .ok()
                    .unwrap_or_default();

                let id_match = props.vendor_id == vendor_id && props.device_id == device_id;
                let name_match =
                    !adapter_name.is_empty() && vk_name.eq_ignore_ascii_case(adapter_name);

                if id_match || name_match {
                    let mut device_local_bytes: u64 = 0;

                    for heap in mem.memory_heaps[..mem.memory_heap_count as usize].iter() {
                        if heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
                            device_local_bytes = device_local_bytes.saturating_add(heap.size);
                        }
                    }

                    return Some(PlatformGpuProbe {
                        vram_bytes: Some(device_local_bytes),
                        unified_memory: Some(device_local_bytes == 0),
                    });
                }
            }

            None
        })();

        instance.destroy_instance(None);
        result
    }
}