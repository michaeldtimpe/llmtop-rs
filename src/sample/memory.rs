use anyhow::{Result, bail};
use serde::Serialize;
use std::fmt;
use std::mem;

// ── host_statistics64 FFI ───────────────────────────────────────────────────

const HOST_VM_INFO64: i32 = 4;
const HOST_VM_INFO64_COUNT: u32 =
    (mem::size_of::<VmStatistics64>() / mem::size_of::<i32>()) as u32;

unsafe extern "C" {
    fn mach_host_self() -> u32;
    fn host_statistics64(
        host: u32,
        flavor: i32,
        info: *mut VmStatistics64,
        count: *mut u32,
    ) -> i32;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct VmCountersRaw {
    pub pages_wired_down: u64,
    pub pages_active: u64,
    pub pages_inactive: u64,
    pub pages_free: u64,
    pub pages_speculative: u64,
    pub pages_occupied_by_compressor: u64,
    pub pages_purgeable: u64,
    pub swapins: u64,
    pub swapouts: u64,
    pub pages_compressed: u64,
    pub pages_decompressed: u64,
    pub pageouts: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SwapInfo {
    pub used_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PressureLevel {
    Normal,
    Warn,
    Critical,
}

impl fmt::Display for PressureLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PressureLevel::Normal => "normal",
            PressureLevel::Warn => "warn",
            PressureLevel::Critical => "critical",
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MemorySample {
    pub raw: VmCountersRaw,
    pub total_ram_bytes: u64,
    pub page_size: u64,
    pub swap: SwapInfo,
    pub pressure: PressureLevel,
    pub iogpu_wired_limit: Option<String>,
    // Derived rates (pages/s)
    pub compress_rate: f64,
    pub decompress_rate: f64,
    pub swapout_rate: f64,
    pub swapin_rate: f64,
    pub pageout_rate: f64,
}

impl MemorySample {
    pub fn from_raw(
        raw: VmCountersRaw,
        swap: SwapInfo,
        total_ram_bytes: u64,
        pressure: PressureLevel,
        iogpu_wired_limit: Option<String>,
        prev: Option<&VmCountersRaw>,
        elapsed: Option<f64>,
        page_size: u64,
    ) -> Self {
        let (compress_rate, decompress_rate, swapout_rate, swapin_rate, pageout_rate) =
            match (prev, elapsed) {
                (Some(p), Some(dt)) if dt > 0.0 => {
                    let rate = |cur: u64, prev: u64| cur.wrapping_sub(prev) as f64 / dt;
                    (
                        rate(raw.pages_compressed, p.pages_compressed),
                        rate(raw.pages_decompressed, p.pages_decompressed),
                        rate(raw.swapouts, p.swapouts),
                        rate(raw.swapins, p.swapins),
                        rate(raw.pageouts, p.pageouts),
                    )
                }
                _ => (0.0, 0.0, 0.0, 0.0, 0.0),
            };

        Self {
            raw,
            total_ram_bytes,
            page_size,
            swap,
            pressure,
            iogpu_wired_limit,
            compress_rate,
            decompress_rate,
            swapout_rate,
            swapin_rate,
            pageout_rate,
        }
    }

    pub fn wired_bytes(&self) -> u64 {
        self.raw.pages_wired_down * self.page_size
    }

    pub fn active_bytes(&self) -> u64 {
        self.raw.pages_active * self.page_size
    }

    pub fn inactive_bytes(&self) -> u64 {
        self.raw.pages_inactive * self.page_size
    }

    pub fn free_bytes(&self) -> u64 {
        self.raw.pages_free * self.page_size
    }

    pub fn compressor_bytes(&self) -> u64 {
        self.raw.pages_occupied_by_compressor * self.page_size
    }

    pub fn purgeable_bytes(&self) -> u64 {
        self.raw.pages_purgeable * self.page_size
    }

    /// Approximation of Activity Monitor's "Memory Used":
    /// wired + active + compressor pages. Does not include speculative or
    /// purgeable pages, which the kernel can reclaim cheaply.
    pub fn used_bytes(&self) -> u64 {
        self.wired_bytes() + self.active_bytes() + self.compressor_bytes()
    }
}

// ── Collection functions ────────────────────────────────────────────────────

pub fn read_vm_stats() -> Result<VmCountersRaw> {
    let mut info = VmStatistics64::default();
    let mut count = HOST_VM_INFO64_COUNT;

    let kr = unsafe { host_statistics64(mach_host_self(), HOST_VM_INFO64, &mut info, &mut count) };
    if kr != 0 {
        bail!("host_statistics64 failed: kern_return {kr}");
    }

    Ok(VmCountersRaw {
        pages_wired_down: info.wire_count as u64,
        pages_active: info.active_count as u64,
        pages_inactive: info.inactive_count as u64,
        pages_free: info.free_count as u64,
        pages_speculative: info.speculative_count as u64,
        pages_occupied_by_compressor: info.compressor_page_count as u64,
        pages_purgeable: info.purgeable_count as u64,
        swapins: info.swapins,
        swapouts: info.swapouts,
        pages_compressed: info.compressions,
        pages_decompressed: info.decompressions,
        pageouts: info.pageouts,
    })
}

pub fn read_swap_info() -> Result<SwapInfo> {
    // sysctl crate returns raw struct bytes for vm.swapusage; parse from CLI output instead
    let output = std::process::Command::new("sysctl")
        .args(["-n", "vm.swapusage"])
        .output()?;
    let val = String::from_utf8_lossy(&output.stdout);

    // Format: "total = 1024.00M  used = 46.19M  free = 977.81M  (encrypted)"
    let parse_mb = |key: &str| -> u64 {
        val.find(key).and_then(|i| {
            let after = &val[i + key.len()..];
            let after = after.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
            let num_end = after.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(after.len());
            after[..num_end].parse::<f64>().ok()
        }).map(|mb| (mb * 1024.0 * 1024.0) as u64).unwrap_or(0)
    };

    Ok(SwapInfo {
        total_bytes: parse_mb("total"),
        used_bytes: parse_mb("used"),
    })
}

pub fn read_total_ram() -> Result<u64> {
    use sysctl::Sysctl;
    let ctl = sysctl::Ctl::new("hw.memsize")?;
    match ctl.value()? {
        sysctl::CtlValue::U64(v) => Ok(v),
        sysctl::CtlValue::S64(v) => Ok(v as u64),
        other => bail!("unexpected hw.memsize type: {other:?}"),
    }
}

pub fn read_pressure_level() -> PressureLevel {
    use sysctl::Sysctl;
    let level = sysctl::Ctl::new("kern.memorystatus_vm_pressure_level")
        .and_then(|c| c.value())
        .ok();

    // `kern.memorystatus_vm_pressure_level` returns the dispatch_source_memorypressure
    // flag value, not an ordinal: 1=NORMAL, 2=WARN, 4=CRITICAL. Anything else means
    // the sysctl is unavailable or unfamiliar — treat as Normal.
    match level {
        Some(sysctl::CtlValue::Int(1)) => PressureLevel::Normal,
        Some(sysctl::CtlValue::Int(2)) => PressureLevel::Warn,
        Some(sysctl::CtlValue::Int(4)) => PressureLevel::Critical,
        _ => PressureLevel::Normal,
    }
}

pub fn read_iogpu_wired_limit() -> Option<String> {
    use sysctl::Sysctl;
    sysctl::Ctl::new("iogpu.wired_limit_mb")
        .and_then(|c| c.value_string())
        .ok()
}
