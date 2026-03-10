use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::state::messages::{ConfigCommand, OscillatorType};
use crate::state::synth_state::SynthState;

/// Which sub-section of the voice detail is focused for editing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiceEditSection {
    Grid,        // navigating the voice grid
    Oscillator,  // editing osc type
    Envelope,    // editing ADSR (selected_env_param selects A/D/S/R)
    Sends,       // editing send levels (selected_send selects group A-D)
}

pub struct VoicePanel {
    pub selected_voice: usize,
    pub edit_section: VoiceEditSection,
    pub selected_env_param: usize,  // 0=Attack 1=Decay 2=Sustain 3=Release
    pub selected_send: usize,       // 0-3 = group A-D
}

impl VoicePanel {
    pub fn new() -> Self {
        VoicePanel {
            selected_voice: 0,
            edit_section: VoiceEditSection::Grid,
            selected_env_param: 0,
            selected_send: 0,
        }
    }

    /// Convert voice index to visual (row, col) in the 2×8 grid
    fn voice_to_grid(v: usize) -> (usize, usize) {
        if v < 4       { (0, v) }
        else if v < 8  { (1, v - 4) }
        else if v < 12 { (0, v - 4) }   // col 4-7, row 0
        else           { (1, v - 8) }   // col 4-7, row 1 — note: v-8 gives col 4-7
    }

    fn grid_to_voice(row: usize, col: usize) -> usize {
        if col < 4 { row * 4 + col }
        else       { 8 + row * 4 + (col - 4) }
    }

    fn move_grid(&mut self, drow: i32, dcol: i32) {
        let (row, col) = Self::voice_to_grid(self.selected_voice);
        let new_row = (row as i32 + drow).clamp(0, 1) as usize;
        let new_col = (col as i32 + dcol).clamp(0, 7) as usize;
        self.selected_voice = Self::grid_to_voice(new_row, new_col);
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
        let block = Block::default().title("Voices  (↑↓←→: navigate, Enter: edit)").borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        for half in 0..2 {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
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

                    let border_style = if is_selected {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let label_style = if is_selected {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else if voice.active {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let content = vec![
                        Line::from(Span::styled(format!(" {:X}", voice_idx), label_style.add_modifier(Modifier::BOLD))),
                        Line::from(Span::styled(
                            format!(" {}", if voice.active { note_name } else { "--".to_string() }),
                            label_style,
                        )),
                        Line::from(Span::styled(format!(" {}", voice.osc_type.name()), Style::default().fg(Color::Cyan))),
                        Line::from(Span::styled(amp_bar, Style::default().fg(Color::Green))),
                    ];

                    let p = Paragraph::new(content)
                        .block(Block::default().borders(Borders::ALL).border_style(border_style));
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

        // --- Oscillator ---
        let osc_types = OscillatorType::all();
        let osc_idx = osc_types.iter().position(|&t| t == voice.osc_type).unwrap_or(0);
        let osc_focused = self.edit_section == VoiceEditSection::Oscillator;

        let osc_lines = vec![
            Line::from(vec![
                Span::raw("Type: "),
                Span::styled(
                    format!("◄ {} ►", voice.osc_type.name()),
                    if osc_focused { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) }
                    else { Style::default().fg(Color::Cyan) },
                ),
            ]),
            Line::from(format!("  ({}/{})", osc_idx + 1, osc_types.len())),
            Line::from(format!("Note: {}", midi_note_name(voice.midi_note))),
            Line::from(format!("Vel:  {:.0}%", voice.velocity * 100.0)),
            Line::from(""),
            Line::from(if osc_focused {
                Span::styled("←→ cycle types", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("Enter to edit", Style::default().fg(Color::DarkGray))
            }),
        ];
        let osc_title = if osc_focused { "Oscillator [editing]" } else { "Oscillator" };
        let osc_block = Paragraph::new(osc_lines)
            .block(Block::default().title(osc_title).borders(Borders::ALL).border_style(
                if osc_focused { Style::default().fg(Color::Yellow) } else { Style::default() }
            ));
        frame.render_widget(osc_block, chunks[0]);

        // --- Envelope ---
        let env = &voice.envelope;
        let env_focused = self.edit_section == VoiceEditSection::Envelope;
        let env_param_names = ["Attack", "Decay", "Sustain", "Release"];
        let env_values = [env.attack, env.decay, env.sustain, env.release];
        let env_units = ["s", "s", "", "s"];

        let env_lines: Vec<Line> = env_param_names.iter().enumerate().map(|(i, name)| {
            let val = env_values[i];
            let is_selected = env_focused && i == self.selected_env_param;
            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if env_focused {
                Style::default().fg(Color::Gray)
            } else {
                Style::default()
            };
            let indicator = if is_selected { "►" } else { " " };
            Line::from(Span::styled(
                format!("{} {}: {:.3}{}", indicator, name, val, env_units[i]),
                style,
            ))
        }).collect();

        let env_title = if env_focused { "Envelope [↑↓:param  ←→:adjust]" } else { "Envelope" };
        let env_block = Paragraph::new(env_lines)
            .block(Block::default().title(env_title).borders(Borders::ALL).border_style(
                if env_focused { Style::default().fg(Color::Yellow) } else { Style::default() }
            ));
        frame.render_widget(env_block, chunks[1]);

        // --- Sends ---
        let send_labels = ["A", "B", "C", "D"];
        let sends_focused = self.edit_section == VoiceEditSection::Sends;

        let sends: Vec<ListItem> = (0..4).map(|g| {
            let level = state.routing[self.selected_voice][g];
            let bar = send_bar(level, 12);
            let is_selected = sends_focused && g == self.selected_send;
            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if sends_focused {
                Style::default().fg(Color::Gray)
            } else {
                Style::default()
            };
            let indicator = if is_selected { "►" } else { " " };
            ListItem::new(Line::styled(
                format!("{} {}: {} {:.0}%", indicator, send_labels[g], bar, level * 100.0),
                style,
            ))
        }).collect();

        let send_title = if sends_focused { "Sends [↑↓:group  ←→:adjust]" } else { "Sends" };
        let sends_list = List::new(sends)
            .block(Block::default().title(send_title).borders(Borders::ALL).border_style(
                if sends_focused { Style::default().fg(Color::Yellow) } else { Style::default() }
            ));
        frame.render_widget(sends_list, chunks[2]);
    }

    /// Returns the help text for the current edit section
    pub fn help_text(&self) -> &str {
        match self.edit_section {
            VoiceEditSection::Grid =>
                "↑↓←→:Navigate  Enter:Edit section  o:Cycle osc  Tab:Mode  q:Quit",
            VoiceEditSection::Oscillator =>
                "←→:Cycle osc type  Tab:Next section  Esc:Back to grid  q:Quit",
            VoiceEditSection::Envelope =>
                "↑↓:Select param  ←→:Adjust value  Tab:Next section  Esc:Back to grid  q:Quit",
            VoiceEditSection::Sends =>
                "↑↓:Select group  ←→:Adjust level  Tab:Next section  Esc:Back to grid  q:Quit",
        }
    }

    /// Handle a key event; returns a ConfigCommand if the audio engine should be notified
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) -> Option<ConfigCommand> {
        use crossterm::event::KeyCode;

        match self.edit_section {
            VoiceEditSection::Grid => match key.code {
                KeyCode::Up    => { self.move_grid(-1,  0); None }
                KeyCode::Down  => { self.move_grid( 1,  0); None }
                KeyCode::Left  => { self.move_grid( 0, -1); None }
                KeyCode::Right => { self.move_grid( 0,  1); None }
                KeyCode::Enter => {
                    self.edit_section = VoiceEditSection::Oscillator;
                    None
                }
                KeyCode::Char('o') => {
                    // Quick oscillator cycle without entering edit mode
                    self.cycle_osc(state, 1)
                }
                _ => None,
            },

            VoiceEditSection::Oscillator => match key.code {
                KeyCode::Left  => self.cycle_osc(state, -1),
                KeyCode::Right => self.cycle_osc(state,  1),
                KeyCode::Tab   => { self.edit_section = VoiceEditSection::Envelope; None }
                KeyCode::Esc   => { self.edit_section = VoiceEditSection::Grid; None }
                _ => None,
            },

            VoiceEditSection::Envelope => match key.code {
                KeyCode::Up    => { self.selected_env_param = self.selected_env_param.saturating_sub(1); None }
                KeyCode::Down  => { self.selected_env_param = (self.selected_env_param + 1).min(3); None }
                KeyCode::Left  => self.adjust_envelope(state, -0.05),
                KeyCode::Right => self.adjust_envelope(state,  0.05),
                KeyCode::Tab   => { self.edit_section = VoiceEditSection::Sends; None }
                KeyCode::Esc   => { self.edit_section = VoiceEditSection::Grid; None }
                _ => None,
            },

            VoiceEditSection::Sends => match key.code {
                KeyCode::Up    => { self.selected_send = self.selected_send.saturating_sub(1); None }
                KeyCode::Down  => { self.selected_send = (self.selected_send + 1).min(3); None }
                KeyCode::Left  => self.adjust_send(state, -0.05),
                KeyCode::Right => self.adjust_send(state,  0.05),
                KeyCode::Tab   => { self.edit_section = VoiceEditSection::Grid; None }
                KeyCode::Esc   => { self.edit_section = VoiceEditSection::Grid; None }
                _ => None,
            },
        }
    }

    fn cycle_osc(&self, state: &SynthState, dir: i32) -> Option<ConfigCommand> {
        let types = OscillatorType::all();
        let current = state.voices[self.selected_voice].osc_type;
        let idx = types.iter().position(|&t| t == current).unwrap_or(0) as i32;
        let new_idx = (idx + dir).rem_euclid(types.len() as i32) as usize;
        Some(ConfigCommand::SetOscillator {
            voice: self.selected_voice,
            osc_type: types[new_idx],
        })
    }

    fn adjust_envelope(&self, state: &SynthState, delta: f32) -> Option<ConfigCommand> {
        let env = &state.voices[self.selected_voice].envelope;
        let (attack, decay, sustain, release) = (env.attack, env.decay, env.sustain, env.release);

        // Clamp ranges: A/D/R in seconds, S is 0-1
        let (new_a, new_d, new_s, new_r) = match self.selected_env_param {
            0 => ((attack  + delta * 2.0).clamp(0.001, 10.0), decay, sustain, release),
            1 => (attack, (decay   + delta * 2.0).clamp(0.001, 10.0), sustain, release),
            2 => (attack, decay,   (sustain + delta).clamp(0.0, 1.0), release),
            3 => (attack, decay,   sustain, (release + delta * 2.0).clamp(0.001, 10.0)),
            _ => return None,
        };
        Some(ConfigCommand::SetEnvelope {
            voice: self.selected_voice,
            attack: new_a, decay: new_d, sustain: new_s, release: new_r,
        })
    }

    fn adjust_send(&self, state: &SynthState, delta: f32) -> Option<ConfigCommand> {
        let current = state.routing[self.selected_voice][self.selected_send];
        Some(ConfigCommand::SetSendLevel {
            voice: self.selected_voice,
            group: self.selected_send,
            level: (current + delta).clamp(0.0, 1.0),
        })
    }
}

fn midi_note_name(midi: u8) -> String {
    const NAMES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (midi / 12) as i32 - 1;
    format!("{}{}", NAMES[(midi % 12) as usize], octave)
}

fn amplitude_bar(amp: f32, width: usize) -> String {
    let filled = ((amp * width as f32) as usize).min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn send_bar(level: f32, width: usize) -> String {
    let filled = ((level * width as f32) as usize).min(width);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}
