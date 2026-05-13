use std::process::Command;
use std::time::Instant;

use crate::sample::SampleSet;
use crate::sample::memory::PressureLevel;

pub struct AlertConfig {
    pub swap_mb: Option<u64>,
    pub swap_rate: Option<f64>,
    pub pressure: bool,
}

pub struct Alerter {
    config: AlertConfig,
    last_alert: Instant,
    debounce_secs: f64,
    last_pressure: PressureLevel,
}

impl Alerter {
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config,
            last_alert: Instant::now() - std::time::Duration::from_secs(999),
            debounce_secs: 30.0,
            last_pressure: PressureLevel::Normal,
        }
    }

    pub fn check(&mut self, sample: &SampleSet) {
        let now = Instant::now();
        if now.duration_since(self.last_alert).as_secs_f64() < self.debounce_secs {
            return;
        }

        let m = &sample.memory;

        if let Some(threshold_mb) = self.config.swap_mb {
            let swap_mb = m.swap.used_bytes / (1024 * 1024);
            if swap_mb >= threshold_mb {
                self.fire(
                    &format!("Swap usage: {swap_mb}MB (threshold: {threshold_mb}MB)"),
                    now,
                );
                return;
            }
        }

        if let Some(threshold) = self.config.swap_rate {
            if m.swapout_rate > threshold {
                self.fire(
                    &format!("Swapout rate: {:.1} pg/s (threshold: {threshold})", m.swapout_rate),
                    now,
                );
                return;
            }
        }

        if self.config.pressure && m.pressure != self.last_pressure {
            let old = self.last_pressure;
            let new = m.pressure;
            self.last_pressure = m.pressure;
            if m.pressure != PressureLevel::Normal {
                self.fire(
                    &format!("Memory pressure: {old} → {new}"),
                    now,
                );
            }
        }
    }

    fn fire(&mut self, msg: &str, now: Instant) {
        self.last_alert = now;
        let _ = Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "display notification \"{}\" with title \"llmtop\"",
                    msg.replace('"', "\\\"")
                ),
            ])
            .spawn();
    }
}
