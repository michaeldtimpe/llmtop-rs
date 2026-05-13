use ratatui::style::{Color, Style};

pub const BORDER: Style = Style::new().fg(Color::DarkGray);
pub const TITLE: Style = Style::new().fg(Color::Cyan);
pub const LABEL: Style = Style::new().fg(Color::DarkGray);
pub const VALUE: Style = Style::new().fg(Color::White);
pub const HIGHLIGHT: Style = Style::new().fg(Color::Yellow);

pub fn residency_color(pct: f64) -> Color {
    if pct >= 0.9 {
        Color::Green
    } else if pct >= 0.5 {
        Color::Yellow
    } else {
        Color::Red
    }
}

pub fn pressure_color(level: &str) -> Color {
    match level {
        "normal" => Color::Green,
        "warn" => Color::Yellow,
        _ => Color::Red,
    }
}

/// Color used by the default theme for a bar based on how full it is.
pub fn bar_color(pct: f32) -> Color {
    if pct >= 0.90 {
        Color::Red
    } else if pct >= 0.71 {
        Color::Yellow
    } else {
        Color::Green
    }
}

// ── Usage-bar themes ──────────────────────────────────────────────────────
//
// Cycled by the `c` keystroke. The default theme is `UsageStatus`, which
// matches the original behavior (red/yellow/green based on fill). Solid
// and PerBar themes are static; Rainbow varies per column and shifts each
// frame so the gradient appears to roll downward through the four bars.

#[derive(Debug, Clone, Copy)]
pub enum BarTheme {
    UsageStatus,
    Solid(Color),
    PerBar([Color; 4]),
    Rainbow,
}

pub struct ThemeEntry {
    pub name: &'static str,
    pub theme: BarTheme,
}

pub const THEMES: &[ThemeEntry] = &[
    ThemeEntry { name: "status",  theme: BarTheme::UsageStatus },
    ThemeEntry { name: "green",   theme: BarTheme::Solid(Color::Green) },
    ThemeEntry { name: "cyan",    theme: BarTheme::Solid(Color::Cyan) },
    ThemeEntry { name: "magenta", theme: BarTheme::Solid(Color::Magenta) },
    ThemeEntry { name: "blue",    theme: BarTheme::Solid(Color::Blue) },
    // llmtop's own palette: the cyan that titles use, its light variant, the
    // yellow used for highlights, and the green that means "healthy" in the
    // status theme. Each bar gets one signature color.
    ThemeEntry {
        name: "llmtop",
        theme: BarTheme::PerBar([
            Color::Cyan,      // CPU — matches the title accent
            Color::LightCyan, // GPU — lighter shade of the same
            Color::Yellow,    // Memory — matches HIGHLIGHT style
            Color::Green,     // Power — matches "normal" pressure / "ok" status
        ]),
    },
    // Gradient inside the cool half of the spectrum; each bar is a different
    // shade so adjacent bars stay visually distinct.
    ThemeEntry {
        name: "cool",
        theme: BarTheme::PerBar([
            Color::LightCyan,
            Color::Cyan,
            Color::LightBlue,
            Color::Blue,
        ]),
    },
    // Gradient inside the warm half. LightYellow/Yellow/LightRed/Red walk
    // from pale yellow through orange to deep red.
    ThemeEntry {
        name: "warm",
        theme: BarTheme::PerBar([
            Color::LightYellow,
            Color::Yellow,
            Color::LightRed,
            Color::Red,
        ]),
    },
    ThemeEntry { name: "rainbow", theme: BarTheme::Rainbow },
];

/// 6-color rainbow used by `BarTheme::Rainbow`. Order matters: it's the
/// visible-spectrum sequence so the gradient reads naturally as a rainbow.
const RAINBOW: &[Color] = &[
    Color::Red,
    Color::Yellow,
    Color::Green,
    Color::Cyan,
    Color::Blue,
    Color::Magenta,
];

/// Color for a single filled cell in a usage bar.
///
/// `bar_idx` is the bar's position in the pane (0 = top). `col` is the
/// 0-indexed character position within the filled portion of the bar.
/// `fill_pct` is the bar's current fill (0.0–1.0). `frame` is a monotonic
/// frame counter that drives the rainbow's downward roll.
pub fn theme_bar_color(
    theme: &BarTheme,
    bar_idx: usize,
    col: usize,
    fill_pct: f32,
    frame: u64,
) -> Color {
    match theme {
        BarTheme::UsageStatus => bar_color(fill_pct),
        BarTheme::Solid(c) => *c,
        BarTheme::PerBar(palette) => palette[bar_idx.min(palette.len() - 1)],
        BarTheme::Rainbow => {
            // phase = (col + bar_idx - frame) mod N. Negative frame contribution
            // makes the color at (col, bar) at frame F equal the color at
            // (col, bar-1) at frame F-1 — i.e. the pattern shifts to higher
            // bar indices (downward) as the frame advances.
            let n = RAINBOW.len() as i64;
            let raw = col as i64 + bar_idx as i64 - (frame as i64);
            let idx = ((raw % n) + n) % n;
            RAINBOW[idx as usize]
        }
    }
}
