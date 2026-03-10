use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::state::synth_state::SynthState;

pub struct VoicePanel {
    pub selected_voice: usize,
    pub selected_param: usize, // 0=osc, 1=attack, 2=decay, 3=sustain, 4=release, 5-8=sends
}

impl VoicePanel {
    pub fn new() -> Self {
        VoicePanel {
            selected_voice: 0,
            selected_param: 0,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Voice grid
                Constraint::Min(10),    // Selected voice details
            ])
            .split(area);

        self.render_voice_grid(frame, chunks[0], state);
        self.render_voice_detail(frame, chunks[1], state);
    }

    fn render_voice_grid(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let block = Block::default()
            .title("Voices")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // 4 columns, 4 rows for 16 voices
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(inner);

        for half in 0..2 {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(cols[half]);

            for row in 0..2 {
                let cells = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ])
                    .split(rows[row]);

                for col in 0..4 {
                    let voice_idx = half * 8 + row * 4 + col;
                    let voice = &state.voices[voice_idx];
                    let is_selected = voice_idx == self.selected_voice;

                    let note_name = midi_note_name(voice.midi_note);
                    let amp_bar = amplitude_bar(voice.amplitude, 8);

                    let style = if is_selected {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else if voice.active {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let content = vec![
                        Line::from(Span::styled(
                            format!(" {:X}", voice_idx),
                            style.add_modifier(Modifier::BOLD),
                        )),
                        Line::from(Span::styled(
                            format!(" {}", if voice.active { note_name } else { "--".to_string() }),
                            style,
                        )),
                        Line::from(Span::styled(
                            format!(" {}", voice.osc_type.name()),
                            Style::default().fg(Color::Cyan),
                        )),
                        Line::from(Span::styled(amp_bar, Style::default().fg(Color::Green))),
                    ];

                    let p = Paragraph::new(content)
                        .block(Block::default().borders(Borders::ALL).border_style(
                            if is_selected {
                                Style::default().fg(Color::Yellow)
                            } else {
                                Style::default()
                            },
                        ));
                    frame.render_widget(p, cells[col]);
                }
            }
        }
    }

    fn render_voice_detail(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let voice = &state.voices[self.selected_voice];

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(area);

        // Oscillator block
        let osc_lines = vec![
            Line::from(format!("Type: {}", voice.osc_type.name())),
            Line::from(format!("Note: {}", midi_note_name(voice.midi_note))),
            Line::from(format!("Vel:  {:.0}%", voice.velocity * 100.0)),
        ];
        let osc_block = Paragraph::new(osc_lines)
            .block(Block::default().title("Oscillator").borders(Borders::ALL));
        frame.render_widget(osc_block, chunks[0]);

        // Envelope block (simple text display)
        let env = {
            // We don't have envelope params in state, display placeholder
            vec![
                Line::from("ADSR Envelope"),
                Line::from("  A: 0.01s"),
                Line::from("  D: 0.20s"),
                Line::from("  S: 0.70"),
                Line::from("  R: 0.30s"),
            ]
        };
        let env_block = Paragraph::new(env)
            .block(Block::default().title("Envelope").borders(Borders::ALL));
        frame.render_widget(env_block, chunks[1]);

        // Sends block
        let send_labels = ["A", "B", "C", "D"];
        let sends: Vec<ListItem> = (0..4).map(|g| {
            let level = state.routing[self.selected_voice][g];
            let bar = send_bar(level, 10);
            ListItem::new(format!("{}: {} {:.0}%", send_labels[g], bar, level * 100.0))
        }).collect();
        let sends_list = List::new(sends)
            .block(Block::default().title("Sends").borders(Borders::ALL));
        frame.render_widget(sends_list, chunks[2]);
    }

    pub fn handle_key(&mut self, key: char, state: &SynthState) -> Option<crate::state::messages::ConfigCommand> {
        match key {
            'j' | '\x1b' => {
                // Down - handled by caller via KeyCode
                None
            }
            'k' => None, // Up - handled by caller
            'h' => {
                // Decrease current param
                self.adjust_param(state, -0.05)
            }
            'l' => {
                // Increase current param
                self.adjust_param(state, 0.05)
            }
            _ => None,
        }
    }

    pub fn adjust_param(&self, state: &SynthState, delta: f32) -> Option<crate::state::messages::ConfigCommand> {
        use crate::state::messages::ConfigCommand;
        match self.selected_param {
            5..=8 => {
                let group = self.selected_param - 5;
                let current = state.routing[self.selected_voice][group];
                Some(ConfigCommand::SetSendLevel {
                    voice: self.selected_voice,
                    group,
                    level: (current + delta).clamp(0.0, 1.0),
                })
            }
            _ => None,
        }
    }
}

fn midi_note_name(midi: u8) -> String {
    const NAMES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let name = NAMES[(midi % 12) as usize];
    let octave = (midi / 12) as i32 - 1;
    format!("{}{}", name, octave)
}

fn amplitude_bar(amp: f32, width: usize) -> String {
    let filled = (amp * width as f32) as usize;
    let filled = filled.min(width);
    format!("{}{}",
        "█".repeat(filled),
        "░".repeat(width - filled),
    )
}

fn send_bar(level: f32, width: usize) -> String {
    let filled = (level * width as f32) as usize;
    let filled = filled.min(width);
    format!("[{}{}]",
        "█".repeat(filled),
        "░".repeat(width - filled),
    )
}
