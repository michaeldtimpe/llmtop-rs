use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::sample::SampleSet;

pub struct CsvLogger {
    file: File,
    wrote_header: bool,
}

impl CsvLogger {
    pub fn new(path: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            file,
            wrote_header: false,
        })
    }

    pub fn append(&mut self, s: &SampleSet) -> Result<()> {
        if !self.wrote_header {
            writeln!(
                self.file,
                "timestamp,ram_total,wired,active,inactive,free,compressor,purgeable,\
                 swap_used,swap_total,pressure,compress_rate,decompress_rate,\
                 swapout_rate,swapin_rate,pageout_rate,\
                 ecpu_freq,ecpu_util,pcpu_freq,pcpu_util,gpu_freq,gpu_util,\
                 cpu_w,gpu_w,ane_w,ram_w,package_w,system_w,\
                 cpu_temp,gpu_temp,\
                 model_count,total_model_size,total_model_resident"
            )?;
            self.wrote_header = true;
        }

        let m = &s.memory;
        let soc = &s.soc;
        let total_model_size: u64 = s.models.iter().filter_map(|m| m.size_bytes).sum();
        let total_model_resident: u64 = s.models.iter().filter_map(|m| m.resident_bytes).sum();

        writeln!(
            self.file,
            "{},{},{},{},{},{},{},{},{},{},{},{:.1},{:.1},{:.1},{:.1},{:.1},\
             {},{:.4},{},{:.4},{},{:.4},\
             {},{},{},{},{},{},\
             {},{},\
             {},{},{}",
            s.wall_ts.to_rfc3339(),
            m.total_ram_bytes,
            m.wired_bytes(),
            m.active_bytes(),
            m.inactive_bytes(),
            m.free_bytes(),
            m.compressor_bytes(),
            m.purgeable_bytes(),
            m.swap.used_bytes,
            m.swap.total_bytes,
            m.pressure,
            m.compress_rate,
            m.decompress_rate,
            m.swapout_rate,
            m.swapin_rate,
            m.pageout_rate,
            soc.cpu.ecpu_freq_mhz,
            soc.cpu.ecpu_util_pct,
            soc.cpu.pcpu_freq_mhz,
            soc.cpu.pcpu_util_pct,
            soc.gpu.freq_mhz,
            soc.gpu.util_pct,
            opt_f32(soc.power.cpu_w),
            opt_f32(soc.power.gpu_w),
            opt_f32(soc.power.ane_w),
            opt_f32(soc.power.ram_w),
            opt_f32(soc.power.total_package_w),
            opt_f32(soc.power.total_system_w),
            opt_f32(soc.thermal.cpu_temp_c),
            opt_f32(soc.thermal.gpu_temp_c),
            s.models.len(),
            total_model_size,
            total_model_resident,
        )?;
        Ok(())
    }
}

pub struct JsonlLogger {
    file: File,
}

impl JsonlLogger {
    pub fn new(path: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn append(&mut self, s: &SampleSet) -> Result<()> {
        let json = serde_json::to_string(s)?;
        writeln!(self.file, "{json}")?;
        Ok(())
    }
}

fn opt_f32(v: Option<f32>) -> String {
    match v {
        Some(f) => format!("{f:.2}"),
        None => String::new(),
    }
}
