use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::state::synth_state::SynthState;

pub struct FxGroupPanel {
    pub selected_group: usize,
    pub selected_effect: usize,
    pub selected_param: usize,
}

impl FxGroupPanel {
    pub fn new() -> Self {
        FxGroupPanel {
            selected_group: 0,
            selected_effect: 0,
            selected_param: 0,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ])
            .split(area);

        self.render_groups(frame, chunks[0], state);
        self.render_params(frame, chunks[1], state);
    }

    fn render_groups(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let group_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        let group_names = ["A", "B", "C", "D"];
        for (i, group) in state.groups.iter().enumerate() {
            let is_selected = i == self.selected_group;
            let title = format!(
                "Group {}: {} {}",
                group_names[i],
                if group.effects.is_empty() { "Unused" } else { "" },
                if group.enabled { "[Enabled]" } else { "[Disabled]" }
            );

            let items: Vec<ListItem> = group.effects.iter().enumerate().map(|(j, e)| {
                let style = if is_selected && j == self.selected_effect {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let params_str: Vec<String> = e.params.iter()
                    .map(|p| format!("{}: {:.2}", p.name, p.value))
                    .collect();
                ListItem::new(Line::styled(
                    format!("  {}. {} ({})", j + 1, e.name, params_str.join(", ")),
                    style,
                ))
            }).collect();

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(if is_selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });

            if items.is_empty() {
                let p = Paragraph::new("  (empty)")
                    .block(block)
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(p, group_areas[i]);
            } else {
                let list = List::new(items).block(block);
                frame.render_widget(list, group_areas[i]);
            }
        }
    }

    fn render_params(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let group = &state.groups[self.selected_group];
        if let Some(effect) = group.effects.get(self.selected_effect) {
            let lines: Vec<Line> = effect.params.iter().enumerate().map(|(i, p)| {
                let bar = param_bar(p.value, p.min, p.max, 20);
                let style = if i == self.selected_param {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                Line::styled(
                    format!("{:12} {} {:.3}", p.name, bar, p.value),
                    style,
                )
            }).collect();

            let block = Paragraph::new(lines)
                .block(Block::default()
                    .title(format!("Parameters: {}", effect.name))
                    .borders(Borders::ALL));
            frame.render_widget(block, area);
        } else {
            let p = Paragraph::new("No effect selected")
                .block(Block::default().title("Parameters").borders(Borders::ALL));
            frame.render_widget(p, area);
        }
    }
}

fn param_bar(value: f32, min: f32, max: f32, width: usize) -> String {
    let range = (max - min).max(0.001);
    let normalized = ((value - min) / range).clamp(0.0, 1.0);
    let filled = (normalized * width as f32) as usize;
    format!("[{}{}]",
        "█".repeat(filled),
        "░".repeat(width - filled),
    )
}
