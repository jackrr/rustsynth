use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::state::synth_state::SynthState;

pub struct RoutingPanel {
    pub selected_voice: usize,
    pub selected_group: usize,
    pub clipboard: Option<[f32; 4]>,
}

impl RoutingPanel {
    pub fn new() -> Self {
        RoutingPanel { selected_voice: 0, selected_group: 0, clipboard: None }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(20), Constraint::Length(4)])
            .split(area);

        self.render_matrix(frame, chunks[0], state);
        self.render_status(frame, chunks[1], state);
    }

    fn render_matrix(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let group_names = ["Group A", "Group B", "Group C", "Group D"];

        let header = Row::new(vec![
            Cell::from("Voice"),
            Cell::from(if self.selected_group == 0 { "► Group A" } else { "  Group A" }),
            Cell::from(if self.selected_group == 1 { "► Group B" } else { "  Group B" }),
            Cell::from(if self.selected_group == 2 { "► Group C" } else { "  Group C" }),
            Cell::from(if self.selected_group == 3 { "► Group D" } else { "  Group D" }),
        ]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = (0..16usize).map(|v| {
            let voice_cell = Cell::from(format!("  {:X}", v)).style(
                if v == self.selected_voice {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                }
            );

            let group_cells: Vec<Cell> = (0..4).map(|g| {
                let level = state.routing[v][g];
                let bar = send_bar(level, 6);
                let text = format!("{} {:3.0}%", bar, level * 100.0);
                let style = if v == self.selected_voice && g == self.selected_group {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else if level > 0.0 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Cell::from(text).style(style)
            }).collect();

            let mut cells = vec![voice_cell];
            cells.extend(group_cells);
            Row::new(cells)
        }).collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(7),
                Constraint::Percentage(23),
                Constraint::Percentage(23),
                Constraint::Percentage(23),
                Constraint::Percentage(23),
            ],
        )
        .header(header)
        .block(Block::default()
            .title("Send Matrix  (↑↓:voice  []:group  ←→:adjust  Enter:toggle 0/100%)")
            .borders(Borders::ALL));

        frame.render_widget(table, area);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let level = state.routing[self.selected_voice][self.selected_group];
        let group_names = ["A", "B", "C", "D"];
        let big_bar = send_bar(level, 30);

        let lines = vec![
            Line::from(format!(
                "Voice {:X} → Group {}:  {} {:.0}%",
                self.selected_voice,
                group_names[self.selected_group],
                big_bar,
                level * 100.0,
            )),
            Line::from(format!(
                "  c:Copy row  p:Paste  z:Zero row  {}",
                if self.clipboard.is_some() { "[clipboard: ready]" } else { "" },
            )),
        ];

        let p = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(p, area);
    }
}

fn send_bar(level: f32, width: usize) -> String {
    let filled = ((level * width as f32) as usize).min(width);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}
