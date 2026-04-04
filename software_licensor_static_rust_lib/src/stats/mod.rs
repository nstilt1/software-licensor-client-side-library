use sha2::{Digest, Sha256};
use std::env::consts::{OS, ARCH};

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;

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
    pub gpu_info: Option<GpuInfo>,
}

/// Stats that can be displayed to the user.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct StatsDisplay {
    pub os_name: String,
    pub computer_name: String,
    pub is_64_bit: bool,
    pub users_language: String,
    pub display_language: String,
    pub num_logical_cores: u32,
    pub num_physical_cores: u32,
    pub cpu_freq_mhz: u32,
    pub cpu_architecture: String,
    pub ram_mb: u32,
    pub page_size: u32,
    pub cpu_vendor: String,
    pub cpu_model: String,
    pub has_mmx: bool,
    pub has_3d_now: bool,
    pub has_fma3: bool,
    pub has_fma4: bool,
    pub has_sse: bool,
    pub has_sse2: bool,
    pub has_sse3: bool,
    pub has_ssse3: bool,
    pub has_sse41: bool,
    pub has_sse42: bool,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_avx512f: bool,
    pub has_avx512bw: bool,
    pub has_avx512cd: bool,
    pub has_avx512dq: bool,
    pub has_avx512er: bool,
    pub has_avx512ifma: bool,
    pub has_avx512pf: bool,
    pub has_avx512vbmi: bool,
    pub has_avx512vl: bool,
    pub has_avx512vpopcntdq: bool,
    pub has_neon: bool,
    pub gpu_name: Option<String>,
    pub gpu_brand: Option<String>,
    pub gpu_backend: Option<String>,
    pub gpu_type: Option<String>,
    pub gpu_vram_bytes: Option<u64>,
    pub gpu_unified_memory: Option<bool>,
    pub gpu_core_count: Option<u32>,
    pub npu_available: Option<bool>,
    pub tpu_available: Option<bool>,
}

impl From<crate::generated::software_licensor_client::Stats> for StatsDisplay {
    fn from(value: crate::generated::software_licensor_client::Stats) -> Self {
        let gpu_info = value.gpu_info.unwrap_or_default();
        Self {
            os_name: value.os_name,
            computer_name: value.computer_name,
            is_64_bit: value.is_64_bit,
            users_language: value.users_language,
            display_language: value.display_language,
            num_logical_cores: value.num_logical_cores,
            num_physical_cores: value.num_physical_cores,
            cpu_freq_mhz: value.cpu_freq_mhz,
            cpu_architecture: value.cpu_architecture,
            ram_mb: value.ram_mb,
            page_size: value.page_size,
            cpu_vendor: value.cpu_vendor,
            cpu_model: value.cpu_model,
            has_mmx: value.has_mmx,
            has_3d_now: value.has_3d_now,
            has_fma3: value.has_fma3,
            has_fma4: value.has_fma4,
            has_sse: value.has_sse,
            has_sse2: value.has_sse2,
            has_sse3: value.has_sse3,
            has_ssse3: value.has_ssse3,
            has_sse41: value.has_sse41,
            has_sse42: value.has_sse42,
            has_avx: value.has_avx,
            has_avx2: value.has_avx2,
            has_avx512f: value.has_avx512f,
            has_avx512bw: value.has_avx512bw,
            has_avx512cd: value.has_avx512cd,
            has_avx512dq: value.has_avx512dq,
            has_avx512er: value.has_avx512er,
            has_avx512ifma: value.has_avx512ifma,
            has_avx512pf: value.has_avx512pf,
            has_avx512vbmi: value.has_avx512vbmi,
            has_avx512vl: value.has_avx512vl,
            has_avx512vpopcntdq: value.has_avx512vpopcntdq,
            has_neon: value.has_neon,
            gpu_name: gpu_info.gpu_name,
            gpu_brand: gpu_info.gpu_brand,
            gpu_backend: gpu_info.gpu_backend,
            gpu_type: gpu_info.gpu_type,
            gpu_vram_bytes: gpu_info.gpu_vram_bytes,
            gpu_unified_memory: gpu_info.gpu_unified_memory,
            gpu_core_count: gpu_info.gpu_core_count,
            npu_available: gpu_info.npu_available,
            tpu_available: gpu_info.tpu_available,
        }
    }
}

/// Gets the current machine's stats in a format that can be displayed to the user.
pub async unsafe fn get_machine_stats_for_display() -> Result<StatsDisplay, String> {
    if let Some(stats) = get_machine_stats(true).await {
        Ok(StatsDisplay::from(stats))
    } else {
        Err("Failed to get machine stats".into())
    }
}


/// Detects if the current machine supports a given x86 feature.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
fn detect_x86_feature(feature: &str) -> bool {
    match feature {
        "mmx" => std::is_x86_feature_detected!("mmx"),
        "fma" => std::is_x86_feature_detected!("fma"),
        "fma3" => std::is_x86_feature_detected!("fma"),
        "fma4" => std::is_x86_feature_detected!("fma"),
        "sse" => std::is_x86_feature_detected!("sse"),
        "sse2" => std::is_x86_feature_detected!("sse2"),
        "sse3" => std::is_x86_feature_detected!("sse3"),
        "ssse3" => std::is_x86_feature_detected!("ssse3"),
        "sse4.1" => std::is_x86_feature_detected!("sse4.1"),
        "sse4.2" => std::is_x86_feature_detected!("sse4.2"),
        "avx" => std::is_x86_feature_detected!("avx"),
        "avx2" => std::is_x86_feature_detected!("avx2"),
        "avx512f" => std::is_x86_feature_detected!("avx512f"),
        "avx512bw" => std::is_x86_feature_detected!("avx512bw"),
        "avx512cd" => std::is_x86_feature_detected!("avx512cd"),
        "avx512dq" => std::is_x86_feature_detected!("avx512dq"),
        "avx512er" => std::is_x86_feature_detected!("avx512er"),
        "avx512ifma" => std::is_x86_feature_detected!("avx512ifma"),
        "avx512pf" => std::is_x86_feature_detected!("avx512pf"),
        "avx512vbmi" => std::is_x86_feature_detected!("avx512vbmi"),
        "avx512vl" => std::is_x86_feature_detected!("avx512vl"),
        "avx512vpopcntdq" => std::is_x86_feature_detected!("avx512vpopcntdq"),
        _ => false,
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
fn detect_x86_feature(_feature: &str) -> bool {
    false
}

#[cfg(target_arch = "arm")]
#[inline]
fn detect_arm_feature(feature: &str) -> bool {
    match feature {
        "neon" => std::arch::is_arm_feature_detected!("neon"),
        _ => false,
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn detect_arm_feature(feature: &str) -> bool {
    match feature {
        "neon" => std::arch::is_aarch64_feature_detected!("neon"),
        _ => false,
    }
}

#[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
#[inline]
fn detect_arm_feature(_feature: &str) -> bool {
    false
}

/// Gets the page size of the current machine.
fn get_page_size() -> Option<u32> {
    #[cfg(unix)]
    {
        let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if v <= 0 {
            None
        } else {
            u32::try_from(v).ok()
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

        unsafe {
            let mut info = std::mem::zeroed::<SYSTEM_INFO>();
            GetSystemInfo(&mut info);
            Some(info.dwPageSize)
        }
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        None
    }
}

/// Returns the current machine's stats. This includes various hardware 
/// information about the machine, such as the CPU vendor, model, number of 
/// cores, amount of RAM, and various CPU features.
/// 
/// # Safety
/// 
/// This function collects hardware information that could potentially be used to
/// uniquely identify a machine, and therefore could be a privacy concern. It is 
/// the caller's responsibility to ensure that this function is only called when 
/// the user has explicitly opted in to the collection of this information.
#[inline(always)]
pub(crate) async unsafe fn get_machine_stats(save_system_stats: bool) -> Option<crate::generated::software_licensor_client::Stats> {
    if !save_system_stats {
        return None;
    }
    unsafe {
        let s = super::stats::Stats::collect().await;
        let gpu = if let Some(gpu_info) = s.gpu_info {
            Some(crate::generated::software_licensor_client::GpuInfo {
                gpu_name: gpu_info.name,
                gpu_brand: gpu_info.brand,
                gpu_backend: gpu_info.backend,
                gpu_type: gpu_info.gpu_type,
                gpu_vram_bytes: gpu_info.vram_bytes,
                gpu_unified_memory: gpu_info.unified_memory,
                gpu_core_count: gpu_info.core_count,
                npu_available: gpu_info.npu_available,
                tpu_available: gpu_info.tpu_available,
                ..Default::default()
            })
        } else {
            None
        };

        return Some(crate::generated::software_licensor_client::Stats { 
            os_name: OS.to_string(), 
            computer_name: s.computer_name, 
            is_64_bit: if size_of::<usize>() == 8 { true } else { false }, 
            users_language: s.users_language, 
            display_language: s.display_language, 
            num_logical_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32, 
            num_physical_cores: s.num_physical_cores, 
            cpu_freq_mhz: s.cpu_freq_mhz, 
            cpu_architecture: ARCH.to_string(), 
            ram_mb: s.ram_mb, 
            page_size: get_page_size().unwrap_or(0), 
            cpu_vendor: s.cpu_vendor, 
            cpu_model: s.cpu_model, 
            has_mmx: detect_x86_feature("mmx"),
            has_3d_now: false,
            has_fma3: detect_x86_feature("fma"),
            has_fma4: false,
            has_sse: detect_x86_feature("sse"),
            has_sse2: detect_x86_feature("sse2"),
            has_sse3: detect_x86_feature("sse3"),
            has_ssse3: detect_x86_feature("ssse3"),
            has_sse41: detect_x86_feature("sse4.1"),
            has_sse42: detect_x86_feature("sse4.2"),
            has_avx: detect_x86_feature("avx"),
            has_avx2: detect_x86_feature("avx2"),
            has_avx512f: detect_x86_feature("avx512f"),
            has_avx512bw: detect_x86_feature("avx512bw"),
            has_avx512cd: detect_x86_feature("avx512cd"),
            has_avx512dq: detect_x86_feature("avx512dq"),
            has_avx512er: detect_x86_feature("avx512er"),
            has_avx512ifma: detect_x86_feature("avx512ifma"),
            has_avx512pf: detect_x86_feature("avx512pf"),
            has_avx512vbmi: detect_x86_feature("avx512vbmi"),
            has_avx512vl: detect_x86_feature("avx512vl"),
            has_avx512vpopcntdq: detect_x86_feature("avx512vpopcntdq"),
            has_neon: detect_arm_feature("neon"),
            gpu_info: gpu,
        })
    }
}

impl Stats {
    /// Collects the stats for the current device. This may be a slow operation, 
    /// so it's recommended to call this only once and cache the result. 
    /// 
    /// # Safety
    /// This function may call platform-specific APIs that are unsafe. However, 
    /// it should not cause any harm to the system.
    pub async unsafe fn collect() -> Self {
        platform::collect().await
    }
}

#[inline(always)]
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

#[inline(always)]
pub fn computer_name() -> Option<String> {
    platform::computer_name()
}

#[inline(always)]
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

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod platform {
    use super::*;

    pub fn collect() -> Stats {
        Stats::default()
    }
}

use std::cmp::Reverse;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct GpuInfo {
    /// Human-readable adapter name, e.g. "NVIDIA GeForce RTX 4070"
    pub name: Option<String>,

    /// Best-effort vendor/brand name, e.g. "NVIDIA", "AMD", "Intel", "Apple"
    pub brand: Option<String>,

    /// wgpu backend name, e.g. "Vulkan", "Metal", "Dx12"
    pub backend: Option<String>,

    /// "Integrated" | "Discrete" | "Virtual" | "CPU" | "Other" | "Arm"
    pub gpu_type: Option<String>,

    /// Best-effort memory figure.
    ///
    /// Semantics differ by platform:
    /// - Windows DXGI: dedicated VRAM bytes
    /// - macOS Metal: recommended max working set size
    /// - Linux/Vulkan: sum of DEVICE_LOCAL heap sizes
    pub vram_bytes: Option<u64>,

    /// True on unified-memory GPUs when detectable.
    pub unified_memory: Option<bool>,

    /// Not reliably exposed cross-platform from one stable API.
    pub core_count: Option<u32>,

    /// Placeholder for future accelerator detection.
    pub npu_available: Option<bool>,

    /// Placeholder for future accelerator detection.
    pub tpu_available: Option<bool>,
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            name: None,
            brand: None,
            backend: None,
            gpu_type: None,
            vram_bytes: None,
            unified_memory: None,
            core_count: None,
            npu_available: None,
            tpu_available: None,
        }
    }
}

/// Async version for crates that already have an async runtime.
pub async fn detect_primary_gpu_info() -> Option<GpuInfo> {
    let instance = make_wgpu_instance();

    let adapters = instance.enumerate_adapters(wgpu::Backends::all()).await;
    if adapters.is_empty() {
        return None;
    }

    let mut candidates = adapters
        .into_iter()
        .map(|adapter| {
            let info = adapter.get_info();
            let score = adapter_priority(&info.device_type);
            (score, adapter, info)
        })
        .collect::<Vec<_>>();

    // Prefer discrete > integrated > other > virtual > cpu
    candidates.sort_by_key(|(score, _, _)| Reverse(*score));

    for (_, _adapter, info) in candidates {
        // Treat pure software adapters as "no GPU".
        if info.device_type == wgpu::DeviceType::Cpu {
            continue;
        }

        let platform_probe = platform_gpu_probe(info.vendor, info.device, &info.name);

        let unified_memory = platform_probe.unified_memory;
        let gpu_type = Some(classify_gpu_type(&info, unified_memory));

        return Some(GpuInfo {
            name: non_empty(&info.name),
            brand: infer_brand(info.vendor, &info.name),
            backend: Some(format!("{:?}", info.backend)),
            gpu_type,
            vram_bytes: platform_probe.vram_bytes,
            unified_memory,
            core_count: None,
            npu_available: None,
            tpu_available: None,
        });
    }

    None
}

/// Blocking convenience wrapper for static libraries or sync call sites.
pub fn detect_primary_gpu_info_blocking() -> Option<GpuInfo> {
    pollster::block_on(detect_primary_gpu_info())
}

fn make_wgpu_instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

fn adapter_priority(device_type: &wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => 5,
        wgpu::DeviceType::IntegratedGpu => 4,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Cpu => 1,
    }
}

fn classify_gpu_type(info: &wgpu::AdapterInfo, unified_memory: Option<bool>) -> String {
    let lower_name = info.name.to_ascii_lowercase();

    // Best-effort "Arm" bucket for Apple/ARM-family GPUs.
    // This is heuristic because wgpu's portable enum does not have an Arm variant.
    if info.device_type != wgpu::DeviceType::Cpu
        && (lower_name.contains("apple")
            || lower_name.contains("mali")
            || lower_name.contains("adreno")
            || info.vendor == 0x13B5
            || info.vendor == 0x106B)
    {
        return "Arm".to_string();
    }

    match info.device_type {
        wgpu::DeviceType::IntegratedGpu => {
            if unified_memory == Some(true) && lower_name.contains("apple") {
                "Arm".to_string()
            } else {
                "Integrated".to_string()
            }
        }
        wgpu::DeviceType::DiscreteGpu => "Discrete".to_string(),
        wgpu::DeviceType::VirtualGpu => "Virtual".to_string(),
        wgpu::DeviceType::Cpu => "CPU".to_string(),
        wgpu::DeviceType::Other => "Other".to_string(),
    }
}

fn infer_brand(vendor_id: u32, name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();

    let from_vendor = match vendor_id {
        0x10DE => Some("NVIDIA"),
        0x1002 | 0x1022 => Some("AMD"),
        0x8086 => Some("Intel"),
        0x106B => Some("Apple"),
        0x13B5 => Some("ARM"),
        _ => None,
    };

    from_vendor
        .map(str::to_string)
        .or_else(|| {
            if lower.contains("nvidia") {
                Some("NVIDIA".to_string())
            } else if lower.contains("amd") || lower.contains("radeon") {
                Some("AMD".to_string())
            } else if lower.contains("intel") {
                Some("Intel".to_string())
            } else if lower.contains("apple") {
                Some("Apple".to_string())
            } else if lower.contains("mali") || lower.contains("arm") {
                Some("ARM".to_string())
            } else if lower.contains("adreno") || lower.contains("qualcomm") {
                Some("Qualcomm".to_string())
            } else {
                None
            }
        })
}

fn some_if_nonzero(v: u32) -> Option<u32> {
    if v == 0 { None } else { Some(v) }
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct PlatformGpuProbe {
    vram_bytes: Option<u64>,
    unified_memory: Option<bool>,
}

fn platform_gpu_probe(vendor_id: u32, device_id: u32, adapter_name: &str) -> PlatformGpuProbe {
    #[cfg(target_os = "windows")]
    {
        return platform::windows_gpu_probe(vendor_id, device_id, adapter_name).unwrap_or_default();
    }

    #[cfg(target_os = "macos")]
    {
        return platform::macos_gpu_probe(adapter_name).unwrap_or_default();
    }

    #[cfg(target_os = "linux")]
    {
        return platform::linux_gpu_probe_vulkan(vendor_id, device_id, adapter_name).unwrap_or_default();
    }

    #[allow(unreachable_code)]
    PlatformGpuProbe::default()
}