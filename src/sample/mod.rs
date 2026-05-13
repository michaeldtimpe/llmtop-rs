pub mod api;
pub mod ffi;
pub mod memory;
pub mod models;
pub mod process;
pub mod soc;

use anyhow::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::time::Instant;

use memory::MemorySample;
use models::{ModelDetector, ModelEntry};
use process::{MatchedProcess, ProcessEnumerator, SysinfoScanner};
use soc::{MacmonSampler, SocSample, SocSampler};

#[derive(Debug, Serialize)]
pub struct SampleSet {
    #[serde(skip)]
    pub ts: Instant,
    pub wall_ts: DateTime<Local>,
    pub memory: MemorySample,
    pub soc: SocSample,
    pub models: Vec<ModelEntry>,
}

pub trait SampleCollector {
    fn collect(&mut self, prev: Option<&SampleSet>) -> Result<SampleSet>;
}

pub struct DefaultCollector {
    page_size: u64,
    soc_sampler: MacmonSampler,
    interval_ms: u32,
    scanner: SysinfoScanner,
    detector: ModelDetector,
    patterns: Vec<String>,
    slow_scan_interval: f64,
    last_full_scan: Instant,
    cached_procs: Vec<MatchedProcess>,
}

impl DefaultCollector {
    pub fn new(
        interval_secs: f64,
        patterns: Vec<String>,
        slow_scan_interval: f64,
        ollama_port: u16,
        omlx_port: Option<u16>,
        lmstudio_port: u16,
    ) -> Result<Self> {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
        let soc_sampler = MacmonSampler::new()?;
        let interval_ms = (interval_secs * 1000.0) as u32;
        Ok(Self {
            page_size,
            soc_sampler,
            interval_ms,
            scanner: SysinfoScanner::new(),
            detector: ModelDetector::new(ollama_port, omlx_port, lmstudio_port),
            patterns,
            slow_scan_interval,
            last_full_scan: Instant::now() - std::time::Duration::from_secs(999),
            cached_procs: Vec::new(),
        })
    }
}

impl SampleCollector for DefaultCollector {
    fn collect(&mut self, prev: Option<&SampleSet>) -> Result<SampleSet> {
        let soc = self.soc_sampler.sample(self.interval_ms)?;

        let ts = Instant::now();
        let wall_ts = Local::now();

        let raw = memory::read_vm_stats()?;
        let swap = memory::read_swap_info()?;
        let total_ram = memory::read_total_ram()?;
        let pressure = memory::read_pressure_level();
        let iogpu = memory::read_iogpu_wired_limit();

        let elapsed = prev.map(|p| ts.duration_since(p.ts).as_secs_f64());
        let prev_raw = prev.map(|p| &p.memory.raw);

        let memory = MemorySample::from_raw(
            raw, swap, total_ram, pressure, iogpu, prev_raw, elapsed, self.page_size,
        );

        let slow_scan_due =
            ts.duration_since(self.last_full_scan).as_secs_f64() >= self.slow_scan_interval;

        let models = if slow_scan_due {
            self.last_full_scan = ts;
            self.cached_procs = self.scanner.full_rescan(&self.patterns);
            self.detector.detect(&self.cached_procs)
        } else {
            self.cached_procs = self.scanner.refresh_rss();
            self.detector.detect(&self.cached_procs)
        };

        Ok(SampleSet {
            ts,
            wall_ts,
            memory,
            soc,
            models,
        })
    }
}
