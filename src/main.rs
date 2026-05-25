use anyhow::Result;
use clap::Parser;
use std::time::Instant;

use llmtop::alert::{AlertConfig, Alerter};
use llmtop::cli::Args;
use llmtop::log::{CsvLogger, JsonlLogger};
use llmtop::sample::{DefaultCollector, SampleCollector};
use llmtop::tui::App;

fn main() -> Result<()> {
    let args = Args::parse();

    let interval = if args.once { 0.1 } else { args.interval };
    let mut collector = DefaultCollector::new(
        interval,
        args.r#match.clone(),
        args.proc_scan_interval,
        args.ollama_port,
        args.omlx_port,
        args.omlx_api_key.clone(),
        args.lmstudio_port,
    )?;

    if args.once {
        let sample = collector.collect(None)?;
        println!("{}", serde_json::to_string_pretty(&sample)?);
        return Ok(());
    }

    let alerter = Alerter::new(AlertConfig {
        swap_mb: args.alert_swap_mb,
        swap_rate: args.alert_swap_rate,
        pressure: args.alert_pressure,
    });
    let csv_logger = args.log.as_ref().map(|p| CsvLogger::new(p)).transpose()?;
    let jsonl_logger = args.jsonl.as_ref().map(|p| JsonlLogger::new(p)).transpose()?;

    if args.no_tui {
        return run_headless(collector, alerter, csv_logger, jsonl_logger, &args);
    }

    let app = App::new(collector, args.pane, alerter, csv_logger, jsonl_logger);
    app.run()
}

fn run_headless(
    mut collector: DefaultCollector,
    mut alerter: Alerter,
    mut csv_logger: Option<CsvLogger>,
    mut jsonl_logger: Option<JsonlLogger>,
    args: &Args,
) -> Result<()> {
    let mut prev = None;
    let interval = std::time::Duration::from_secs_f64(args.interval);

    eprintln!("llmtop: headless mode, interval={:.1}s", args.interval);

    loop {
        let next_tick = Instant::now() + interval;
        let sample = collector.collect(prev.as_ref())?;

        if let Some(ref mut logger) = csv_logger {
            logger.append(&sample)?;
        }
        if let Some(ref mut logger) = jsonl_logger {
            logger.append(&sample)?;
        }
        alerter.check(&sample);

        prev = Some(sample);

        let remaining = next_tick.saturating_duration_since(Instant::now());
        if remaining > std::time::Duration::ZERO {
            std::thread::sleep(remaining);
        }
    }
}
