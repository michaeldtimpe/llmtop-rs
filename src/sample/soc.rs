use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ChipInfo {
    pub chip_name: String,
    pub memory_gb: u8,
    pub ecpu_cores: u8,
    pub pcpu_cores: u8,
    pub gpu_cores: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuMetrics {
    pub ecpu_freq_mhz: u32,
    pub ecpu_util_pct: f32,
    pub pcpu_freq_mhz: u32,
    pub pcpu_util_pct: f32,
    /// Combined ecpu+pcpu utilization, weighted by core count. 0.0–1.0.
    pub combined_util_pct: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuMetrics {
    pub freq_mhz: u32,
    pub util_pct: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerCoreUtilization {
    pub ecpu: Vec<f32>,
    pub pcpu: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PowerMetrics {
    pub cpu_w: Option<f32>,
    pub gpu_w: Option<f32>,
    pub ane_w: Option<f32>,
    pub ram_w: Option<f32>,
    pub gpu_ram_w: Option<f32>,
    pub total_package_w: Option<f32>,
    pub total_system_w: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThermalMetrics {
    pub cpu_temp_c: Option<f32>,
    pub gpu_temp_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SocSample {
    pub chip: ChipInfo,
    pub cpu: CpuMetrics,
    pub gpu: GpuMetrics,
    pub power: PowerMetrics,
    pub thermal: ThermalMetrics,
    pub per_core: PerCoreUtilization,
}

pub trait SocSampler {
    fn sample(&mut self, duration_ms: u32) -> Result<SocSample>;
    fn chip_info(&self) -> &ChipInfo;
}

// macmon::Sampler contains raw CF pointers that aren't Send, but we only
// access it from the sampler thread so this is safe.
unsafe impl Send for MacmonSampler {}

pub struct MacmonSampler {
    inner: macmon::Sampler,
    chip: ChipInfo,
    sys: sysinfo::System,
}

fn opt(v: f32) -> Option<f32> {
    if v.is_nan() || v < 0.0 { None } else { Some(v) }
}

impl MacmonSampler {
    pub fn new() -> Result<Self> {
        let inner = macmon::Sampler::new().map_err(|e| anyhow::anyhow!("{e}"))?;
        let soc = inner.get_soc_info();
        let chip = ChipInfo {
            chip_name: soc.chip_name.clone(),
            memory_gb: soc.memory_gb,
            ecpu_cores: soc.ecpu_cores,
            pcpu_cores: soc.pcpu_cores,
            gpu_cores: soc.gpu_cores,
        };
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_usage();
        Ok(Self { inner, chip, sys })
    }
}

impl SocSampler for MacmonSampler {
    fn sample(&mut self, duration_ms: u32) -> Result<SocSample> {
        let m = self
            .inner
            .get_metrics(duration_ms)
            .map_err(|e| anyhow::anyhow!("macmon sampling failed: {e}"))?;

        self.sys.refresh_cpu_usage();
        let cpus = self.sys.cpus();
        let ecpu_count = self.chip.ecpu_cores as usize;
        let pcpu_count = self.chip.pcpu_cores as usize;
        let total_cpu = ecpu_count + pcpu_count;

        let mut ecpu_pcts = Vec::with_capacity(ecpu_count);
        let mut pcpu_pcts = Vec::with_capacity(pcpu_count);
        for (i, cpu) in cpus.iter().enumerate() {
            let pct = cpu.cpu_usage() / 100.0;
            if i < ecpu_count {
                ecpu_pcts.push(pct);
            } else if i < total_cpu {
                pcpu_pcts.push(pct);
            }
        }

        // macOS exposes only an aggregate GPU utilization; per-core GPU usage is
        // not available through any public API. The UI renders this as a single
        // bar — we don't fabricate per-core values.

        Ok(SocSample {
            chip: self.chip.clone(),
            cpu: CpuMetrics {
                ecpu_freq_mhz: m.ecpu_usage.0,
                ecpu_util_pct: m.ecpu_usage.1,
                pcpu_freq_mhz: m.pcpu_usage.0,
                pcpu_util_pct: m.pcpu_usage.1,
                combined_util_pct: m.cpu_usage_pct,
            },
            gpu: GpuMetrics {
                freq_mhz: m.gpu_usage.0,
                util_pct: m.gpu_usage.1,
            },
            power: PowerMetrics {
                cpu_w: opt(m.cpu_power),
                gpu_w: opt(m.gpu_power),
                ane_w: opt(m.ane_power),
                ram_w: opt(m.ram_power),
                gpu_ram_w: opt(m.gpu_ram_power),
                total_package_w: opt(m.all_power),
                total_system_w: opt(m.sys_power),
            },
            thermal: ThermalMetrics {
                cpu_temp_c: opt(m.temp.cpu_temp_avg),
                gpu_temp_c: opt(m.temp.gpu_temp_avg),
            },
            per_core: PerCoreUtilization {
                ecpu: ecpu_pcts,
                pcpu: pcpu_pcts,
            },
        })
    }

    fn chip_info(&self) -> &ChipInfo {
        &self.chip
    }
}
