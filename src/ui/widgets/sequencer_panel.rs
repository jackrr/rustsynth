use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::state::messages::ConfigCommand;
use crate::state::synth_state::{SequencerStepSnapshot, SynthState};

pub struct SequencerPanel {
    pub selected_voice: usize,
    pub selected_step: usize,
    pub editing_global: bool,
    pub global_param: usize, // 0=BPM, 1=steps, 2=swing
    pub clipboard: Option<[SequencerStepSnapshot; 16]>,
}

impl SequencerPanel {
    pub fn new() -> Self {
        SequencerPanel {
            selected_voice: 0,
            selected_step: 0,
            editing_global: false,
            global_param: 0,
            clipboard: None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(18),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_transport(frame, chunks[0], state);
        self.render_grid(frame, chunks[1], state);
        self.render_detail(frame, chunks[2], state);
    }

    fn render_transport(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let seq = &state.seq;
        let play_icon = if seq.playing { "► PLAYING" } else { "■ STOPPED" };
        let play_style = if seq.playing {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let bpm_style = if self.editing_global && self.global_param == 0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let steps_style = if self.editing_global && self.global_param == 1 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let swing_style = if self.editing_global && self.global_param == 2 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let line = Line::from(vec![
            Span::styled(format!("  {play_icon}   "), play_style),
            Span::raw("BPM: "),
            Span::styled(format!("{:.1}", seq.bpm), bpm_style),
            Span::raw("   Steps: "),
            Span::styled(format!("{}", seq.step_count), steps_style),
            Span::raw("   Swing: "),
            Span::styled(format!("{:.0}%", seq.swing * 100.0), swing_style),
            Span::raw("   "),
            Span::styled(
                if self.editing_global { "[Tab/Esc: exit global  ←→: adjust  ↑↓: param]" }
                else { "[Tab: global params  p: play/stop  r: reset]" },
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let transport = Paragraph::new(line)
            .block(Block::default().title("Transport").borders(Borders::ALL));
        frame.render_widget(transport, area);
    }

    fn render_grid(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let seq = &state.seq;
        let step_count = seq.step_count;

        // Build header row: voice header + step numbers + info
        let mut header_cells = vec![Cell::from("    ")];
        for s in 0..step_count {
            let label = format!("{:02}", s + 1);
            let style = if s == seq.current_step && seq.playing {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if s == self.selected_step {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            header_cells.push(Cell::from(label).style(style));
        }
        header_cells.push(Cell::from("Note/Vel"));
        let header = Row::new(header_cells)
            .style(Style::default().add_modifier(Modifier::BOLD))
            .height(1);

        let rows: Vec<Row> = (0..16usize).map(|v| {
            let voice_label = format!("{:X}   ", v);
            let voice_style = if v == self.selected_voice {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let mut cells = vec![Cell::from(voice_label).style(voice_style)];

            for s in 0..step_count {
                let step = &seq.steps[v][s];
                let is_cursor = v == self.selected_voice && s == self.selected_step;
                let is_playhead = s == seq.current_step && seq.playing;

                let (text, style) = match (step.enabled, is_cursor, is_playhead) {
                    (true, true, _) => (
                        "██",
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                    (false, true, _) => (
                        "··",
                        Style::default().fg(Color::Cyan).bg(Color::Black),
                    ),
                    (true, false, true) => (
                        "██",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                    (false, false, true) => (
                        "··",
                        Style::default().fg(Color::Yellow),
                    ),
                    (true, false, false) => (
                        "██",
                        Style::default().fg(Color::Green),
                    ),
                    (false, false, false) => (
                        "··",
                        Style::default().fg(Color::DarkGray),
                    ),
                };
                cells.push(Cell::from(text).style(style));
            }

            // Info column: default note + velocity of the cursor step for selected voice,
            // or just voice default note for others
            let info = if v == self.selected_voice {
                let step = &seq.steps[v][self.selected_step];
                format!("{} {:.0}%", midi_note_name(step.midi_note), step.velocity * 100.0)
            } else {
                // Show the voice default note from SynthState
                let note = state.voices[v].default_midi_note;
                format!("{}", midi_note_name(note))
            };
            cells.push(Cell::from(info).style(Style::default().fg(Color::Gray)));

            let row_style = if v == self.selected_voice {
                Style::default()
            } else {
                Style::default()
            };
            Row::new(cells).style(row_style)
        }).collect();

        // Build column constraints: voice header (4), step_count * 2-char cols, info col
        let mut widths = vec![Constraint::Length(4)];
        for _ in 0..step_count {
            widths.push(Constraint::Length(3));
        }
        widths.push(Constraint::Min(8));

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default()
                .title("Step Grid  (↑↓:voice  ←→:step  Space:toggle  +/-:note  Shift:octave  c:copy  v:paste  z:clear)")
                .borders(Borders::ALL));

        frame.render_widget(table, area);
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, state: &SynthState) {
        let seq = &state.seq;
        let step = &seq.steps[self.selected_voice][self.selected_step];
        let status = if step.enabled { "ON " } else { "off" };
        let note_name = midi_note_name(step.midi_note);

        let line = Line::from(vec![
            Span::raw("  Voice "),
            Span::styled(format!("{:X}", self.selected_voice), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" · Step "),
            Span::styled(format!("{:02}", self.selected_step + 1), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" · "),
            Span::styled(note_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" · vel: {:.0}%", step.velocity * 100.0)),
            Span::raw(format!("  [{status}]")),
            Span::styled(
                "   [Space:toggle  +/-:note  Shift+±:octave  c:Copy row  v:Paste  z:Clear row  Z:Clear all]",
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let detail = Paragraph::new(line)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(detail, area);
    }

    pub fn help_text(&self) -> &str {
        if self.editing_global {
            "↑↓:Param  ←→:Adjust  Tab/Esc:Back to grid"
        } else {
            "↑↓←→:Navigate  Space:Toggle  p:Play/Stop  r:Reset  Tab:Global  c:Copy  v:Paste  z:Clear row  Z:Clear all  q:Quit"
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) -> Vec<ConfigCommand> {
        let mut cmds = Vec::new();
        let seq = &state.seq;

        if self.editing_global {
            match key.code {
                KeyCode::Tab | KeyCode::Esc => {
                    self.editing_global = false;
                }
                KeyCode::Up => {
                    self.global_param = self.global_param.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.global_param = (self.global_param + 1).min(2);
                }
                KeyCode::Left | KeyCode::Right => {
                    let dir: f32 = if key.code == KeyCode::Right { 1.0 } else { -1.0 };
                    let fine = key.modifiers.contains(KeyModifiers::SHIFT);
                    match self.global_param {
                        0 => {
                            let step = if fine { 0.1 } else { 1.0 };
                            let new_bpm = (seq.bpm + dir * step).clamp(20.0, 300.0);
                            cmds.push(ConfigCommand::SeqSetBpm { bpm: new_bpm });
                        }
                        1 => {
                            let cur = seq.step_count as i32;
                            let new_count = (cur + dir as i32).clamp(1, 16) as usize;
                            cmds.push(ConfigCommand::SeqSetStepCount { count: new_count });
                        }
                        2 => {
                            let step = if fine { 0.01 } else { 0.05 };
                            let new_swing = (seq.swing + dir * step).clamp(0.0, 0.5);
                            cmds.push(ConfigCommand::SeqSetSwing { swing: new_swing });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return cmds;
        }

        // Navigate mode
        match key.code {
            KeyCode::Up => {
                self.selected_voice = self.selected_voice.saturating_sub(1);
            }
            KeyCode::Down => {
                self.selected_voice = (self.selected_voice + 1).min(15);
            }
            KeyCode::Left => {
                self.selected_step = self.selected_step.saturating_sub(1);
            }
            KeyCode::Right => {
                let max_step = seq.step_count.saturating_sub(1);
                self.selected_step = (self.selected_step + 1).min(max_step);
            }
            KeyCode::Char(' ') => {
                let step = &seq.steps[self.selected_voice][self.selected_step];
                cmds.push(ConfigCommand::SeqSetStep {
                    voice: self.selected_voice,
                    step: self.selected_step,
                    enabled: !step.enabled,
                    midi_note: step.midi_note,
                    velocity: step.velocity,
                });
            }
            KeyCode::Char('p') => {
                cmds.push(ConfigCommand::SeqTogglePlay);
            }
            KeyCode::Char('r') => {
                cmds.push(ConfigCommand::SeqStop);
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let step = &seq.steps[self.selected_voice][self.selected_step];
                let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                let delta: i16 = if shift { 12 } else { 1 };
                let new_note = (step.midi_note as i16 + delta).clamp(0, 127) as u8;
                cmds.push(ConfigCommand::SeqSetStep {
                    voice: self.selected_voice,
                    step: self.selected_step,
                    enabled: step.enabled,
                    midi_note: new_note,
                    velocity: step.velocity,
                });
            }
            KeyCode::Char('-') => {
                let step = &seq.steps[self.selected_voice][self.selected_step];
                let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                let delta: i16 = if shift { 12 } else { 1 };
                let new_note = (step.midi_note as i16 - delta).clamp(0, 127) as u8;
                cmds.push(ConfigCommand::SeqSetStep {
                    voice: self.selected_voice,
                    step: self.selected_step,
                    enabled: step.enabled,
                    midi_note: new_note,
                    velocity: step.velocity,
                });
            }
            KeyCode::Char('c') => {
                // Copy entire row for selected voice
                let row: [SequencerStepSnapshot; 16] = std::array::from_fn(|s| {
                    seq.steps[self.selected_voice][s]
                });
                self.clipboard = Some(row);
            }
            KeyCode::Char('v') => {
                if let Some(cb) = self.clipboard {
                    for s in 0..16 {
                        cmds.push(ConfigCommand::SeqSetStep {
                            voice: self.selected_voice,
                            step: s,
                            enabled: cb[s].enabled,
                            midi_note: cb[s].midi_note,
                            velocity: cb[s].velocity,
                        });
                    }
                }
            }
            KeyCode::Char('z') => {
                cmds.push(ConfigCommand::SeqClearRow { voice: self.selected_voice });
            }
            KeyCode::Char('Z') => {
                cmds.push(ConfigCommand::SeqClearAll);
            }
            KeyCode::Tab => {
                self.editing_global = true;
                self.global_param = 0;
            }
            _ => {}
        }

        cmds
    }
}

/// Convert a MIDI note number to a human-readable name like "C4", "F#3", etc.
fn midi_note_name(midi_note: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (midi_note as i32 / 12) - 1;
    let name = NAMES[(midi_note % 12) as usize];
    format!("{}{}", name, octave)
}
