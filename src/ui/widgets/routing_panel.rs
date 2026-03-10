use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Table, Row, Cell},
};

use crate::state::synth_state::SynthState;

pub struct RoutingPanel {
    pub selected_voice: usize,
    pub selected_group: usize,
    pub clipboard: Option<[f32; 4]>, // copy-paste buffer
}

impl RoutingPanel {
    pub fn new() -> Self {
        RoutingPanel {
            selected_voice: 0,
            selected_group: 0,
            clipboard: None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(20),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_matrix(frame, chunks[0], state);
        self.render_status(frame, chunks[1], state);
    }

    fn render_matrix(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let group_names = ["Group A", "Group B", "Group C", "Group D"];

        let header = Row::new(vec![
            Cell::from("Voice"),
            Cell::from(group_names[0]),
            Cell::from(group_names[1]),
            Cell::from(group_names[2]),
            Cell::from(group_names[3]),
        ]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = (0..16usize).map(|v| {
            let voice_label = format!("Voice {:X}", v);
            let cells: Vec<Cell> = (0..4usize).map(|g| {
                let level = state.routing[v][g];
                let bar = send_bar(level, 8);
                let text = format!("{} {:.0}%", bar, level * 100.0);
                let style = if v == self.selected_voice && g == self.selected_group {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else if level > 0.0 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Cell::from(text).style(style)
            }).collect();

            let mut all_cells = vec![Cell::from(voice_label).style(
                if v == self.selected_voice {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }
            )];
            all_cells.extend(cells);
            Row::new(all_cells)
        }).collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Percentage(23),
                Constraint::Percentage(23),
                Constraint::Percentage(23),
                Constraint::Percentage(23),
            ],
        )
        .header(header)
        .block(Block::default().title("Send Matrix").borders(Borders::ALL));

        frame.render_widget(table, area);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let level = state.routing[self.selected_voice][self.selected_group];
        let group_names = ["A", "B", "C", "D"];
        let bar = send_bar(level, 20);
        let text = format!(
            "Voice {:X} → Group {}: {} {:.0}%  ←→: Adjust  c: Copy  p: Paste  z: Zero All",
            self.selected_voice,
            group_names[self.selected_group],
            bar,
            level * 100.0,
        );
        let p = Paragraph::new(Line::from(text))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(p, area);
    }
}

fn send_bar(level: f32, width: usize) -> String {
    let filled = (level * width as f32) as usize;
    let filled = filled.min(width);
    format!("[{}{}]",
        "█".repeat(filled),
        "░".repeat(width - filled),
    )
}
