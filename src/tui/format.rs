pub fn fmt_bytes(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1}GB", b / GB)
    } else {
        format!("{:.0}MB", b / MB)
    }
}

/// Always renders bytes as `XX.XX GB` (2 decimal places, no abbreviation
/// switch). Used in the Usage pane label where the unit needs to stay GB
/// even for sub-gigabyte values, so `0.54 GB` of swap reads naturally.
pub fn fmt_bytes_gb(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.2} GB", bytes as f64 / GB)
}

/// Frequency in GHz with 1 decimal place. Used in compact CPU labels where
/// "3.3 GHz" reads better than "3340 MHz".
pub fn fmt_freq_ghz(mhz: u32) -> String {
    format!("{:.1}", mhz as f64 / 1000.0)
}

pub fn fmt_rate_bytes(pages_per_sec: f64, page_size: u64) -> String {
    let bps = pages_per_sec * page_size as f64;
    const MB: f64 = 1024.0 * 1024.0;
    if bps.abs() < 0.05 * MB {
        "0.0".into()
    } else {
        format!("{:.1} MB/s", bps / MB)
    }
}

pub fn fmt_rate_pages(pages_per_sec: f64) -> String {
    if pages_per_sec.abs() < 0.5 {
        "0".into()
    } else {
        format!("{:.0} pg/s", pages_per_sec)
    }
}

pub fn fmt_watts(w: Option<f32>) -> String {
    match w {
        Some(v) => format!("{v:.1}W"),
        None => "—".into(),
    }
}

pub fn fmt_freq(mhz: u32) -> String {
    format!("{mhz}MHz")
}

pub fn fmt_pct(pct: f32) -> String {
    format!("{:.0}%", pct * 100.0)
}

pub fn fmt_temp(c: Option<f32>) -> String {
    match c {
        Some(v) => format!("{v:.0}°C"),
        None => "—".into(),
    }
}

pub fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}
