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
        Span::styled(format!("{:>6}", "% RAM"), style::LABEL),
        Span::raw("  "),
        Span::styled(format!("{:>6}", "% Res"), style::LABEL),
    ]));

    for m in models {
        let residency_pct = match (m.size_bytes, m.resident_bytes) {
            (Some(sz), Some(rs)) if sz > 0 => Some(rs as f64 / sz as f64),
            _ => None,
        };
        let ram_pct = match m.resident_bytes {
            Some(rs) if total_ram > 0 => Some(rs as f64 / total_ram as f64),
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
        if let Some(rp) = ram_pct {
            let c = style::bar_color(rp.clamp(0.0, 1.0) as f32);
            spans.push(Span::styled(format!("{:>5.1}%", rp * 100.0), Style::new().fg(c)));
        } else {
            spans.push(Span::styled("     —", style::LABEL));
        }
        spans.push(Span::raw("  "));
        if let Some(pct) = residency_pct {
            let c = style::residency_color(pct);
            spans.push(Span::styled(format!("{:>5.0}%", pct * 100.0), Style::new().fg(c)));
        } else {
            spans.push(Span::styled("     —", style::LABEL));
        }
        lines.push(Line::from(spans));
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
