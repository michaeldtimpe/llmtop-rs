use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

use crate::sample::models::ModelEntry;
use crate::tui::format::*;
use crate::tui::style;

pub fn render(f: &mut Frame, area: Rect, models: &[ModelEntry], total_ram: u64) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style::BORDER)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(format!("{:<8}", "Source"), style::LABEL),
        Span::styled(format!("{:<24}", "Model"), style::LABEL),
        Span::styled(format!("{:>8}", "Size"), style::LABEL),
        Span::styled(format!("{:>10}", "Resident"), style::LABEL),
        Span::raw("  "),
        Span::styled("Residency", style::LABEL),
    ]));

    let mut total_size: u64 = 0;
    let mut total_resident: u64 = 0;

    for m in models {
        if let Some(sz) = m.size_bytes {
            total_size += sz;
        }
        if let Some(rs) = m.resident_bytes {
            total_resident += rs;
        }

        let pct_opt = match (m.size_bytes, m.resident_bytes) {
            (Some(sz), Some(rs)) if sz > 0 => Some(rs as f64 / sz as f64),
            _ => None,
        };

        let source_str = format!("{:?}", m.source).to_lowercase();
        let model_display = if m.model_id.len() > 22 {
            format!("{}…", &m.model_id[..21])
        } else {
            m.model_id.clone()
        };

        let size_str = m.size_bytes.map(fmt_bytes).unwrap_or_else(|| "—".into());
        let resident_str = m.resident_bytes.map(fmt_bytes).unwrap_or_else(|| "—".into());

        let mut spans = vec![
            Span::styled(format!("{source_str:<8}"), style::VALUE),
            Span::styled(format!("{model_display:<24}"), style::VALUE),
            Span::styled(format!("{size_str:>8}"), style::VALUE),
            Span::styled(format!("{resident_str:>10}"), style::VALUE),
            Span::raw("  "),
        ];
        if let Some(pct) = pct_opt {
            let bar = residency_bar(pct, 10);
            let bar_color = style::residency_color(pct);
            spans.push(Span::styled(bar, Style::new().fg(bar_color)));
            spans.push(Span::styled(format!(" {:>3.0}%", pct * 100.0), Style::new().fg(bar_color)));
        } else {
            spans.push(Span::styled("          —", style::LABEL));
        }
        lines.push(Line::from(spans));
    }

    // Total row
    if !models.is_empty() {
        let ram_pct = if total_ram > 0 {
            total_resident as f64 / total_ram as f64 * 100.0
        } else {
            0.0
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:─<8}", ""), style::LABEL),
            Span::styled(format!("{:<24}", "TOTAL"), style::HIGHLIGHT),
            Span::styled(format!("{:>8}", fmt_bytes(total_size)), style::HIGHLIGHT),
            Span::styled(format!("{:>10}", fmt_bytes(total_resident)), style::HIGHLIGHT),
            Span::raw("  "),
            Span::styled(format!("{ram_pct:>5.1}% RAM"), style::HIGHLIGHT),
        ]));
    }

    if models.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no models detected)",
            style::LABEL,
        )));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

fn residency_bar(pct: f64, width: usize) -> String {
    let filled = (pct * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
