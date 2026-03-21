use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
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
        self.render_groups(frame, area, state);

        if self.show_picker {
            self.render_picker(frame, area);
        }
    }

    fn render_groups(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        // Give the selected group a bit more vertical space for its inline param row
        let constraints: Vec<Constraint> = (0..4).map(|i| {
            if i == self.selected_group { Constraint::Ratio(2, 5) } else { Constraint::Ratio(1, 5) }
        }).collect();
        let group_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let group_names = ["A", "B", "C", "D"];
        for (i, group) in state.groups.iter().enumerate() {
            let is_selected = i == self.selected_group;
            let status = if group.enabled { "On" } else { "Off" };
            let hint = if is_selected { "  ↑↓:effect  []:param  ←→:adjust  a:add  d:del  e:toggle" } else { "" };
            let title = format!("Group {} [{}]{}", group_names[i], status, hint);

            let items: Vec<ListItem> = group.effects.iter().enumerate().map(|(j, e)| {
                let is_effect_selected = is_selected && j == self.selected_effect;
                let effect_style = if is_effect_selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let indicator = if is_effect_selected { "►" } else { " " };
                let main_line = Line::styled(
                    format!("{} {}. {}", indicator, j + 1, e.name),
                    effect_style,
                );

                if is_effect_selected && !e.params.is_empty() {
                    // Inline param row: [selected:val] others dim
                    let mut spans = vec![Span::raw("   ")];
                    for (k, p) in e.params.iter().enumerate() {
                        if k > 0 { spans.push(Span::raw("  ")); }
                        let is_param_sel = k == self.selected_param;
                        let val_str = if let Some(labels) = p.labels {
                            let idx = (p.value.round() as usize).min(labels.len().saturating_sub(1));
                            labels[idx].to_string()
                        } else {
                            format!("{:.2}", p.value)
                        };
                        if is_param_sel {
                            spans.push(Span::styled(
                                format!("[{}:{}]", p.name, val_str),
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::styled(
                                format!(" {}:{} ", p.name, val_str),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                    use ratatui::text::Text;
                    let mut text = Text::default();
                    text.push_line(main_line);
                    text.push_line(Line::from(spans));
                    ListItem::new(text)
                } else {
                    ListItem::new(main_line)
                }
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

