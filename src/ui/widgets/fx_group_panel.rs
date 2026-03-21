use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::state::messages::EffectType;
use crate::state::synth_state::SynthState;

const ALL_EFFECTS: &[EffectType] = &[
    EffectType::Gain,
    EffectType::Bitcrusher,
    EffectType::Distortion,
    EffectType::Limiter,
    EffectType::Delay,
    EffectType::Reverb,
    EffectType::Tremolo,
    EffectType::Chorus,
    EffectType::Phaser,
    EffectType::Vibrato,
    EffectType::Lowpass,
    EffectType::Highpass,
    EffectType::Bandpass,
    EffectType::Eq3,
    EffectType::Compressor,
];

pub struct FxGroupPanel {
    pub selected_group: usize,
    pub selected_effect: usize,
    pub selected_param: usize,
    pub show_picker: bool,
    pub picker_selection: usize,
}

impl FxGroupPanel {
    pub fn new() -> Self {
        FxGroupPanel {
            selected_group: 0,
            selected_effect: 0,
            selected_param: 0,
            show_picker: false,
            picker_selection: 0,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        self.render_groups(frame, chunks[0], state);
        self.render_params(frame, chunks[1], state);

        if self.show_picker {
            self.render_picker(frame, area);
        }
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
                "Group {}  {}",
                group_names[i],
                if group.enabled { "[On]" } else { "[Off]" }
            );

            let items: Vec<ListItem> = group.effects.iter().enumerate().map(|(j, e)| {
                let is_effect_selected = is_selected && j == self.selected_effect;
                let style = if is_effect_selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let indicator = if is_effect_selected { "►" } else { " " };
                let params_str: Vec<String> = e.params.iter()
                    .map(|p| format!("{}: {:.2}", p.name, p.value))
                    .collect();
                ListItem::new(Line::styled(
                    format!("{} {}. {}  ({})", indicator, j + 1, e.name, params_str.join(", ")),
                    style,
                ))
            }).collect();

            let hint = if is_selected {
                "  a:add  d:del  e:toggle  ↑↓:select"
            } else {
                ""
            };

            let block = Block::default()
                .title(format!("{}{}", title, hint))
                .borders(Borders::ALL)
                .border_style(if is_selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });

            if items.is_empty() {
                let p = Paragraph::new("  (empty — press 'a' to add an effect)")
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
                let is_selected = i == self.selected_param;
                let style = if is_selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let indicator = if is_selected { "►" } else { " " };
                Line::styled(
                    format!("{} {:12} {} {:.3}", indicator, p.name, bar, p.value),
                    style,
                )
            }).collect();

            let block = Paragraph::new(lines)
                .block(Block::default()
                    .title(format!("Parameters: {}  ([]:select  ←→:adjust)", effect.name))
                    .borders(Borders::ALL));
            frame.render_widget(block, area);
        } else {
            let p = Paragraph::new("No effect selected — navigate up to a group and press 'a' to add one")
                .block(Block::default().title("Parameters").borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(p, area);
        }
    }

    fn render_picker(&self, frame: &mut Frame, area: Rect) {
        // Center a popup over the content area
        let popup = centered_rect(40, 80, area);

        // Clear the background so the popup is legible
        frame.render_widget(Clear, popup);

        let items: Vec<ListItem> = ALL_EFFECTS.iter().enumerate().map(|(i, effect)| {
            let selected = i == self.picker_selection;
            let style = if selected {
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::styled(
                format!("  {}", effect.name()),
                style,
            ))
        }).collect();

        let list = List::new(items)
            .block(Block::default()
                .title("Add Effect  (↑↓:select  Enter:add  Esc:cancel)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)));

        frame.render_widget(list, popup);
    }

    /// Returns the effect type the picker currently has selected (for use when Enter is pressed)
    pub fn picker_selected_effect(&self) -> EffectType {
        ALL_EFFECTS[self.picker_selection.min(ALL_EFFECTS.len() - 1)]
    }
}

/// Returns a centered rect of `percent_x` × `percent_y` within `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(layout[1])[1]
}

fn param_bar(value: f32, min: f32, max: f32, width: usize) -> String {
    let range = (max - min).max(0.001);
    let normalized = ((value - min) / range).clamp(0.0, 1.0);
    let filled = (normalized * width as f32) as usize;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}
