use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "llmtop", about = "Realtime memory, SoC, and model-residency monitor for Apple Silicon")]
pub struct Args {
    /// Sample interval in seconds
    #[arg(short, long, default_value_t = 1.0)]
    pub interval: f64,

    /// Process filter substring (repeatable)
    #[arg(short, long)]
    pub r#match: Vec<String>,

    /// Show only this pane (repeatable)
    #[arg(long, value_parser = ["usage", "memory", "models"])]
    pub pane: Vec<String>,

    /// Append CSV per sample
    #[arg(long)]
    pub log: Option<String>,

    /// Append JSONL per sample
    #[arg(long)]
    pub jsonl: Option<String>,

    /// Headless mode (no TUI)
    #[arg(long)]
    pub no_tui: bool,

    /// Print one JSON sample to stdout and exit
    #[arg(long)]
    pub once: bool,

    /// Full process rescan interval in seconds
    #[arg(long, default_value_t = 5.0)]
    pub proc_scan_interval: f64,

    /// Notify on swap threshold (MB)
    #[arg(long)]
    pub alert_swap_mb: Option<u64>,

    /// Notify on sustained swapout rate (pages/s)
    #[arg(long)]
    pub alert_swap_rate: Option<f64>,

    /// Notify on pressure transitions
    #[arg(long)]
    pub alert_pressure: bool,

    /// Ollama API port
    #[arg(long, default_value_t = 11434)]
    pub ollama_port: u16,

    /// LM Studio API port
    #[arg(long, default_value_t = 1234, env = "LMSTUDIO_PORT")]
    pub lmstudio_port: u16,

    /// Override omlx port
    #[arg(long)]
    pub omlx_port: Option<u16>,
}
