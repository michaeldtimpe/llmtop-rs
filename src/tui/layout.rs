use ratatui::layout::{Constraint, Layout, Rect};

pub struct PaneAreas {
    pub usage: Option<Rect>,
    pub memory: Option<Rect>,
    pub models: Option<Rect>,
    pub footer: Rect,
}

pub fn compute(area: Rect, visible: &[&str], usage_height: u16, memory_height: u16) -> PaneAreas {
    let show_all = visible.is_empty();
    let show = |name: &str| show_all || visible.contains(&name);

    let mut constraints = Vec::new();
    let mut slots: Vec<&str> = Vec::new();

    if show("usage") {
        constraints.push(Constraint::Length(usage_height));
        slots.push("usage");
    }
    if show("memory") {
        constraints.push(Constraint::Length(memory_height));
        slots.push("memory");
    }
    if show("models") {
        constraints.push(Constraint::Min(5));
        slots.push("models");
    }
    constraints.push(Constraint::Length(1)); // footer

    let chunks = Layout::vertical(constraints).split(area);

    let mut usage = None;
    let mut memory = None;
    let mut models = None;

    for (i, slot) in slots.iter().enumerate() {
        match *slot {
            "usage" => usage = Some(chunks[i]),
            "memory" => memory = Some(chunks[i]),
            "models" => models = Some(chunks[i]),
            _ => {}
        }
    }

    let footer = *chunks.last().unwrap();

    PaneAreas {
        usage,
        memory,
        models,
        footer,
    }
}
