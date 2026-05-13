use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::sample::memory::MemorySample;
use crate::tui::format::*;
use crate::tui::style;

// Width of the label slot inside each cell. Values start at this offset so
// every cell's value aligns vertically with the cells in the rows above
// and below — assuming all rows use the same horizontal split.
const CELL_LABEL_WIDTH: usize = 9; // longest label: "gpu-wired"

/// Three-row × five-cell grid. Each row is laid out with
/// `Layout::horizontal([Ratio(1,5); 5])`, so the cells stretch to fill the
/// available width — extra space appears as widening gaps on the right of
/// each cell, not as dead space at the end of the line.
pub fn render(f: &mut Frame, area: Rect, mem: &MemorySample) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style::BORDER)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    render_row(f, rows[0], state_cells(mem));
    render_row(f, rows[1], rate_cells(mem));
    render_row(f, rows[2], status_cells(mem));
}

pub fn pane_height() -> u16 {
    // 3 content rows + 2 borders
    5
}

// ── cell content ──────────────────────────────────────────────────────────

struct Cell {
    label: &'static str,
    value: String,
    value_style: Style,
}

fn plain_cell(label: &'static str, value: String) -> Cell {
    Cell {
        label,
        value,
        value_style: style::VALUE,
    }
}

fn state_cells(mem: &MemorySample) -> Vec<Cell> {
    vec![
        plain_cell("Wired", fmt_bytes(mem.wired_bytes())),
        plain_cell("Active", fmt_bytes(mem.active_bytes())),
        plain_cell("Inactive", fmt_bytes(mem.inactive_bytes())),
        plain_cell("Free", fmt_bytes(mem.free_bytes())),
        plain_cell("Comp", fmt_bytes(mem.compressor_bytes())),
    ]
}

fn rate_cells(mem: &MemorySample) -> Vec<Cell> {
    let ps = mem.page_size;
    vec![
        plain_cell("Compress", fmt_rate_bytes(mem.compress_rate, ps)),
        plain_cell("Decomp", fmt_rate_bytes(mem.decompress_rate, ps)),
        plain_cell("Pageout", fmt_rate_bytes(mem.pageout_rate, ps)),
        plain_cell("Out/s", fmt_rate_bytes(mem.swapout_rate, ps)),
        plain_cell("In/s", fmt_rate_bytes(mem.swapin_rate, ps)),
    ]
}

fn status_cells(mem: &MemorySample) -> Vec<Cell> {
    let pressure_str = mem.pressure.to_string();
    let pressure_color = style::pressure_color(&pressure_str);
    vec![
        Cell {
            label: "Pressure",
            value: pressure_str,
            value_style: Style::new().fg(pressure_color),
        },
        plain_cell("Σ out", fmt_count(mem.raw.swapouts)),
        plain_cell("Σ in", fmt_count(mem.raw.swapins)),
        plain_cell("Purge", fmt_bytes(mem.purgeable_bytes())),
        plain_cell(
            "gpu-wired",
            mem.iogpu_wired_limit.clone().unwrap_or_else(|| "—".into()),
        ),
    ]
}

// ── row + cell rendering ─────────────────────────────────────────────────

fn render_row(f: &mut Frame, area: Rect, cells: Vec<Cell>) {
    if cells.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let n = cells.len();
    let constraints: Vec<Constraint> = (0..n)
        .map(|_| Constraint::Ratio(1, n as u32))
        .collect();
    let slots = Layout::horizontal(constraints).split(area);

    for (cell, slot) in cells.into_iter().zip(slots.iter()) {
        render_cell(f, *slot, cell);
    }
}

fn render_cell(f: &mut Frame, area: Rect, cell: Cell) {
    if area.width == 0 {
        return;
    }
    let line = Line::from(vec![
        Span::styled(
            format!("{:<CELL_LABEL_WIDTH$}", cell.label),
            style::LABEL,
        ),
        Span::raw(" "),
        Span::styled(cell.value, cell.value_style),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
