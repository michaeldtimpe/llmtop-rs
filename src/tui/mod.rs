pub mod format;
pub mod layout;
pub mod panes;
pub mod style;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use std::io::stdout;
use std::time::{Duration, Instant};

use crate::alert::Alerter;
use crate::log::{CsvLogger, JsonlLogger};
use crate::sample::{DefaultCollector, SampleCollector, SampleSet};

pub struct App {
    collector: DefaultCollector,
    visible_panes: Vec<String>,
    prev_sample: Option<SampleSet>,
    alerter: Alerter,
    csv_logger: Option<CsvLogger>,
    jsonl_logger: Option<JsonlLogger>,
    theme_idx: usize,
    frame: u64,
}

impl App {
    pub fn new(
        collector: DefaultCollector,
        visible_panes: Vec<String>,
        alerter: Alerter,
        csv_logger: Option<CsvLogger>,
        jsonl_logger: Option<JsonlLogger>,
    ) -> Self {
        Self {
            collector,
            visible_panes,
            prev_sample: None,
            alerter,
            csv_logger,
            jsonl_logger,
            theme_idx: 0,
            frame: 0,
        }
    }

    pub fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = self.event_loop(&mut terminal);

        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        loop {
            let sample = self.collector.collect(self.prev_sample.as_ref())?;
            self.alerter.check(&sample);

            if let Some(ref mut logger) = self.csv_logger {
                let _ = logger.append(&sample);
            }
            if let Some(ref mut logger) = self.jsonl_logger {
                let _ = logger.append(&sample);
            }

            let visible: Vec<&str> = self.visible_panes.iter().map(|s| s.as_str()).collect();
            let usage_height = panes::usage::pane_height();
            let memory_height = panes::memory::pane_height();
            let theme_entry = &style::THEMES[self.theme_idx % style::THEMES.len()];
            let theme = theme_entry.theme;
            let theme_name = theme_entry.name;
            let frame = self.frame;

            terminal.draw(|f| {
                let areas = layout::compute(f.area(), &visible, usage_height, memory_height);

                if let Some(area) = areas.usage {
                    panes::usage::render(f, area, &sample.soc, &sample.memory, &theme, frame);
                }
                if let Some(area) = areas.memory {
                    panes::memory::render(f, area, &sample.memory);
                }
                if let Some(area) = areas.models {
                    panes::active_models::render(
                        f,
                        area,
                        &sample.models,
                        sample.memory.total_ram_bytes,
                    );
                }

                let footer_text = format!("[c] {theme_name}  [q] quit");
                let footer_width = footer_text.chars().count() as u16;
                let footer = Paragraph::new(Line::from(vec![Span::styled(
                    footer_text,
                    style::LABEL,
                )]));
                f.render_widget(
                    footer,
                    ratatui::layout::Rect {
                        x: areas.footer.x + areas.footer.width.saturating_sub(footer_width),
                        y: areas.footer.y,
                        width: footer_width,
                        height: 1,
                    },
                );
            })?;

            self.prev_sample = Some(sample);
            self.frame = self.frame.wrapping_add(1);

            let deadline = Instant::now() + Duration::from_millis(50);
            while Instant::now() < deadline {
                if event::poll(Duration::from_millis(10))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Char('c')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                return Ok(());
                            }
                            KeyCode::Char('c') => {
                                self.theme_idx =
                                    (self.theme_idx + 1) % style::THEMES.len();
                            }
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

}
