use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::sample::memory::MemorySample;
use crate::sample::soc::SocSample;
use crate::tui::format::*;
use crate::tui::style;
use crate::tui::style::BarTheme;

/// Heuristic peak package power used to scale the Power bar. M-series chips
/// vary widely (M4 ~30 W, M5 Max ~80 W). 100 W gives every chip headroom
/// without the bar pegging at 100% under normal load.
const PEAK_PACKAGE_W: f32 = 100.0;

/// Width of the bar-name prefix column. The longest prefix is "Memory" (6);
/// padding to 9 leaves a 3-space gap before the stats so values line up
/// across the four bars.
const PREFIX_WIDTH: usize = 9;

/// Four stacked bars: CPU, GPU, Memory, Power. Each occupies one label row
/// plus one bar row. The pane height is fixed at 10 (4 × 2 + 2 borders).
///
/// `theme` controls per-bar coloring (cycled by the `c` key). `frame` is a
/// monotonic counter that drives the rainbow theme's downward roll.
pub fn render(
    f: &mut Frame,
    area: Rect,
    soc: &SocSample,
    mem: &MemorySample,
    theme: &BarTheme,
    frame: u64,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style::BORDER)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into 4 equal vertical slots so extra terminal height pads
    // between bars instead of crowding at the top.
    let slots = Layout::vertical([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .split(inner);

    render_bar(f, slots[0], cpu_label(soc), soc.cpu.combined_util_pct, 0, theme, frame);
    render_bar(f, slots[1], gpu_label(soc), soc.gpu.util_pct, 1, theme, frame);
    render_bar(f, slots[2], mem_label(mem), mem_fill(mem), 2, theme, frame);
    render_bar(f, slots[3], power_label(soc), power_fill(soc), 3, theme, frame);
}

pub fn pane_height() -> u16 {
    // 4 bars × 2 lines (label + bar) + 2 border lines
    10
}

// ── label builders ────────────────────────────────────────────────────────

fn cpu_label(soc: &SocSample) -> String {
    let chip = &soc.chip;
    let total = chip.ecpu_cores + chip.pcpu_cores;
    let pct = soc.cpu.combined_util_pct * 100.0;
    let stats = format!(
        "{total} Cores ({p}P/{s}S) {pct:.2}% @ P{pf}/S{sf} GHz ({t})",
        p = chip.pcpu_cores,
        s = chip.ecpu_cores,
        pf = fmt_freq_ghz(soc.cpu.pcpu_freq_mhz),
        sf = fmt_freq_ghz(soc.cpu.ecpu_freq_mhz),
        t = fmt_temp(soc.thermal.cpu_temp_c),
    );
    format!("{:<PREFIX_WIDTH$}{stats}", "CPU")
}

fn gpu_label(soc: &SocSample) -> String {
    let stats = format!(
        "{:.0}% @ {} MHz ({})",
        soc.gpu.util_pct * 100.0,
        soc.gpu.freq_mhz,
        fmt_temp(soc.thermal.gpu_temp_c),
    );
    format!("{:<PREFIX_WIDTH$}{stats}", "GPU")
}

fn power_label(soc: &SocSample) -> String {
    let watts = soc.power.total_package_w.unwrap_or(0.0);
    let pct = (watts / PEAK_PACKAGE_W * 100.0).clamp(0.0, 100.0);
    let stats = format!("{pct:.2}% {watts:.2} W");
    format!("{:<PREFIX_WIDTH$}{stats}", "Power")
}

fn mem_label(mem: &MemorySample) -> String {
    let stats = format!(
        "{} / {} (Swap: {}/{})",
        fmt_bytes_gb(mem.used_bytes()),
        fmt_bytes_gb(mem.total_ram_bytes),
        fmt_bytes_gb(mem.swap.used_bytes),
        fmt_bytes_gb(mem.swap.total_bytes),
    );
    format!("{:<PREFIX_WIDTH$}{stats}", "Memory")
}

// ── bar fill calculators ──────────────────────────────────────────────────

fn power_fill(soc: &SocSample) -> f32 {
    let watts = soc.power.total_package_w.unwrap_or(0.0);
    (watts / PEAK_PACKAGE_W).clamp(0.0, 1.0)
}

fn mem_fill(mem: &MemorySample) -> f32 {
    if mem.total_ram_bytes == 0 {
        return 0.0;
    }
    (mem.used_bytes() as f64 / mem.total_ram_bytes as f64).clamp(0.0, 1.0) as f32
}

// ── bar rendering ─────────────────────────────────────────────────────────

fn render_bar(
    f: &mut Frame,
    area: Rect,
    label: String,
    pct: f32,
    bar_idx: usize,
    theme: &BarTheme,
    frame: u64,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // First row: label (dim). Last row: bar (colored). Any rows between
    // are blank padding — that's where extra vertical space goes.
    let label_line = Line::from(Span::styled(label, style::LABEL));
    let bar_line = Line::from(render_bar_spans(
        pct,
        area.width as usize,
        bar_idx,
        theme,
        frame,
    ));

    let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);
    lines.push(label_line);
    for _ in 1..area.height.saturating_sub(1) {
        lines.push(Line::raw(""));
    }
    if area.height >= 2 {
        lines.push(bar_line);
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Build the per-column spans for one bar's filled region. The empty
/// region is always rendered in dark gray so fill level stays visible.
///
/// For solid themes we coalesce the filled chars into a single span;
/// rainbow needs one span per column so each cell can carry its own color.
fn render_bar_spans(
    pct: f32,
    width: usize,
    bar_idx: usize,
    theme: &BarTheme,
    frame: u64,
) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let pct = pct.clamp(0.0, 1.0);
    let filled = ((pct * width as f32).round() as usize).min(width);
    let empty = width - filled;

    let mut spans = Vec::new();
    if matches!(theme, BarTheme::Rainbow) {
        for col in 0..filled {
            let color = style::theme_bar_color(theme, bar_idx, col, pct, frame);
            spans.push(Span::styled("█", Style::new().fg(color)));
        }
    } else {
        let color = style::theme_bar_color(theme, bar_idx, 0, pct, frame);
        spans.push(Span::styled("█".repeat(filled), Style::new().fg(color)));
    }
    spans.push(Span::styled(
        "░".repeat(empty),
        Style::new().fg(Color::DarkGray),
    ));
    spans
}
