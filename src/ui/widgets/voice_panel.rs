use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::state::messages::{ConfigCommand, OscillatorType};
use crate::state::synth_state::{EnvelopeParams, SynthState};

/// Which sub-section of the voice detail is focused for editing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiceEditSection {
    Grid,        // navigating the voice grid
    Oscillator,  // editing osc type
    Envelope,    // editing ADSR (selected_env_param selects A/D/S/R)
    Sends,       // editing send levels (selected_send selects group A-D)
}

/// Snapshot of a voice's configuration, used for copy/paste
#[derive(Debug, Clone)]
pub struct VoiceClipboard {
    pub osc_type: OscillatorType,
    pub envelope: EnvelopeParams,
    pub sends: [f32; 4],
}

pub struct VoicePanel {
    pub selected_voice: usize,
    pub edit_section: VoiceEditSection,
    pub selected_env_param: usize,  // 0=Attack 1=Decay 2=Sustain 3=Release
    pub selected_send: usize,       // 0-3 = group A-D
    pub clipboard: Option<VoiceClipboard>,
}

impl VoicePanel {
    pub fn new() -> Self {
        VoicePanel {
            selected_voice: 0,
            edit_section: VoiceEditSection::Grid,
            selected_env_param: 0,
            selected_send: 0,
            clipboard: None,
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
        let clip_hint = if self.clipboard.is_some() { "  [clipboard ready — p:paste]" } else { "" };
        let title = format!("Voices  (↑↓←→:navigate  Enter:edit  c:copy  p:paste{})", clip_hint);
        let block = Block::default().title(title).borders(Borders::ALL);
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

                    let is_copied = self.clipboard.is_some() && voice_idx == self.selected_voice
                        && self.edit_section == VoiceEditSection::Grid;

                    let border_style = if is_selected {
                        Style::default().fg(Color::Yellow)
                    } else if is_copied {
                        Style::default().fg(Color::Magenta)
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

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(10)])
            .split(area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(rows[0]);

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
        render_envelope_diagram(frame, chunks[1], env, env_focused, self.selected_env_param);

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

        render_oscilloscope(frame, rows[1], &state.scope);
    }

    /// Returns the help text for the current edit section
    pub fn help_text(&self) -> &str {
        match self.edit_section {
            VoiceEditSection::Grid =>
                "↑↓←→:Navigate  Space:Trigger C4  Enter:Edit  o:Cycle osc  c:Copy voice  p:Paste voice  Tab:Mode  q:Quit",
            VoiceEditSection::Oscillator =>
                "←→:Cycle osc type  Tab:Next section  Esc:Back to grid  q:Quit",
            VoiceEditSection::Envelope =>
                "↑↓:Select param  ←→:Adjust value  Tab:Next section  Esc:Back to grid  q:Quit",
            VoiceEditSection::Sends =>
                "↑↓:Select group  ←→:Adjust level  Tab:Next section  Esc:Back to grid  q:Quit",
        }
    }

    /// Handle a key event; returns commands to send to the audio engine.
    /// Returns Vec because paste emits multiple commands at once.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) -> Vec<ConfigCommand> {
        use crossterm::event::{KeyCode, KeyModifiers};

        match self.edit_section {
            VoiceEditSection::Grid => match key.code {
                KeyCode::Up    => { self.move_grid(-1,  0); vec![] }
                KeyCode::Down  => { self.move_grid( 1,  0); vec![] }
                KeyCode::Left  => { self.move_grid( 0, -1); vec![] }
                KeyCode::Right => { self.move_grid( 0,  1); vec![] }
                KeyCode::Enter => { self.edit_section = VoiceEditSection::Oscillator; vec![] }
                KeyCode::Char('o') => self.cycle_osc(state, 1).into_iter().collect(),
                KeyCode::Char('c') => { self.copy_voice(state); vec![] }
                KeyCode::Char('p') => self.paste_voice(state),
                _ => vec![],
            },

            VoiceEditSection::Oscillator => match key.code {
                KeyCode::Left  => self.cycle_osc(state, -1).into_iter().collect(),
                KeyCode::Right => self.cycle_osc(state,  1).into_iter().collect(),
                KeyCode::Tab   => { self.edit_section = VoiceEditSection::Envelope; vec![] }
                KeyCode::Esc   => { self.edit_section = VoiceEditSection::Grid; vec![] }
                _ => vec![],
            },

            VoiceEditSection::Envelope => match key.code {
                KeyCode::Up    => { self.selected_env_param = self.selected_env_param.saturating_sub(1); vec![] }
                KeyCode::Down  => { self.selected_env_param = (self.selected_env_param + 1).min(3); vec![] }
                KeyCode::Left  => self.adjust_envelope(state, -1, key.modifiers.contains(KeyModifiers::SHIFT)).into_iter().collect(),
                KeyCode::Right => self.adjust_envelope(state,  1, key.modifiers.contains(KeyModifiers::SHIFT)).into_iter().collect(),
                KeyCode::Tab   => { self.edit_section = VoiceEditSection::Sends; vec![] }
                KeyCode::Esc   => { self.edit_section = VoiceEditSection::Grid; vec![] }
                _ => vec![],
            },

            VoiceEditSection::Sends => match key.code {
                KeyCode::Up    => { self.selected_send = self.selected_send.saturating_sub(1); vec![] }
                KeyCode::Down  => { self.selected_send = (self.selected_send + 1).min(3); vec![] }
                KeyCode::Left  => self.adjust_send(state, -1, key.modifiers.contains(KeyModifiers::SHIFT)).into_iter().collect(),
                KeyCode::Right => self.adjust_send(state,  1, key.modifiers.contains(KeyModifiers::SHIFT)).into_iter().collect(),
                KeyCode::Tab   => { self.edit_section = VoiceEditSection::Grid; vec![] }
                KeyCode::Esc   => { self.edit_section = VoiceEditSection::Grid; vec![] }
                _ => vec![],
            },
        }
    }

    fn copy_voice(&mut self, state: &SynthState) {
        let voice = &state.voices[self.selected_voice];
        self.clipboard = Some(VoiceClipboard {
            osc_type: voice.osc_type,
            envelope: voice.envelope.clone(),
            sends: state.routing[self.selected_voice],
        });
    }

    fn paste_voice(&self, state: &SynthState) -> Vec<ConfigCommand> {
        let Some(ref clip) = self.clipboard else { return vec![]; };
        let dst = self.selected_voice;

        // Don't paste onto the same voice the clipboard was copied from
        // (detect by checking if config already matches — just paste anyway, it's harmless)
        let _ = state; // state available if we need it for guards

        let mut cmds = vec![
            ConfigCommand::SetOscillator { voice: dst, osc_type: clip.osc_type },
            ConfigCommand::SetEnvelope {
                voice: dst,
                attack:  clip.envelope.attack,
                decay:   clip.envelope.decay,
                sustain: clip.envelope.sustain,
                release: clip.envelope.release,
            },
        ];
        for g in 0..4 {
            cmds.push(ConfigCommand::SetSendLevel { voice: dst, group: g, level: clip.sends[g] });
        }
        cmds
    }

    fn cycle_osc(&self, state: &SynthState, dir: i32) -> Option<ConfigCommand> {
        let types = OscillatorType::all();
        let current = state.voices[self.selected_voice].osc_type;
        let idx = types.iter().position(|&t| t == current).unwrap_or(0) as i32;
        let new_idx = (idx + dir).rem_euclid(types.len() as i32) as usize;
        Some(ConfigCommand::SetOscillator { voice: self.selected_voice, osc_type: types[new_idx] })
    }

    fn adjust_envelope(&self, state: &SynthState, dir: i32, fine: bool) -> Option<ConfigCommand> {
        let env = &state.voices[self.selected_voice].envelope;
        let (a, d, s, r) = (env.attack, env.decay, env.sustain, env.release);
        let sign = dir as f32;
        // Time params: coarse=0.1s, fine=0.01s  |  Sustain (0-1): coarse=0.05, fine=0.01
        let (new_a, new_d, new_s, new_r) = match self.selected_env_param {
            0 => { let step = if fine { 0.01 } else { 0.1 }; ((a + sign * step).clamp(0.001, 10.0), d, s, r) }
            1 => { let step = if fine { 0.01 } else { 0.1 }; (a, (d + sign * step).clamp(0.001, 10.0), s, r) }
            2 => { let step = if fine { 0.01 } else { 0.05 }; (a, d, (s + sign * step).clamp(0.0, 1.0), r) }
            3 => { let step = if fine { 0.01 } else { 0.1 }; (a, d, s, (r + sign * step).clamp(0.001, 10.0)) }
            _ => return None,
        };
        Some(ConfigCommand::SetEnvelope { voice: self.selected_voice, attack: new_a, decay: new_d, sustain: new_s, release: new_r })
    }

    fn adjust_send(&self, state: &SynthState, dir: i32, fine: bool) -> Option<ConfigCommand> {
        let step = if fine { 0.01 } else { 0.1 };
        let current = state.routing[self.selected_voice][self.selected_send];
        Some(ConfigCommand::SetSendLevel {
            voice: self.selected_voice,
            group: self.selected_send,
            level: (current + dir as f32 * step).clamp(0.0, 1.0),
        })
    }
}

fn render_oscilloscope(frame: &mut Frame, area: Rect, scope: &[f32]) {
    let block = Block::default().title("Oscilloscope").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 || inner.width < 4 || scope.is_empty() {
        return;
    }

    let chart_h = inner.height as usize;
    let chart_w = inner.width as usize;

    // For each display column, find min/max of the samples mapped to it
    let samples_per_col = (scope.len() as f32 / chart_w as f32).max(1.0);
    let center_row = (chart_h - 1) / 2;

    // Map amplitude (-1.0..1.0) to row (0=top=+1.0, bottom=-1.0)
    let to_row = |v: f32| -> usize {
        ((1.0 - v.clamp(-1.0, 1.0)) / 2.0 * (chart_h - 1) as f32).round() as usize
    };

    // Build grid: (char, Color)
    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', Color::Reset); chart_w]; chart_h];

    // Draw center line
    for x in 0..chart_w {
        grid[center_row][x] = ('·', Color::DarkGray);
    }

    // Draw waveform as min-max bars per column
    for x in 0..chart_w {
        let start = ((x as f32 * samples_per_col) as usize).min(scope.len().saturating_sub(1));
        let end   = (((x + 1) as f32 * samples_per_col) as usize).min(scope.len());
        let slice = &scope[start..end.max(start + 1).min(scope.len())];

        let min_v = slice.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_v = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let top_row = to_row(max_v);
        let bot_row = to_row(min_v);

        // Color based on absolute amplitude (green → yellow → red)
        let peak = max_v.abs().max(min_v.abs());
        let color = if peak > 0.8 { Color::Red }
                    else if peak > 0.5 { Color::Yellow }
                    else { Color::Green };

        for yr in top_row..=bot_row {
            if yr < chart_h {
                let ch = if yr == top_row && yr == bot_row { '▪' }
                         else if yr == top_row { '▀' }
                         else if yr == bot_row { '▄' }
                         else { '█' };
                grid[yr][x] = (ch, color);
            }
        }
    }

    // Render
    let lines: Vec<Line> = grid.iter().map(|row| {
        Line::from(row.iter().map(|(ch, color)| {
            let style = if *ch == '·' {
                Style::default().fg(*color)
            } else {
                Style::default().fg(*color).add_modifier(Modifier::BOLD)
            };
            Span::styled(ch.to_string(), style)
        }).collect::<Vec<_>>())
    }).collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_envelope_diagram(
    frame: &mut Frame,
    area: Rect,
    env: &EnvelopeParams,
    focused: bool,
    selected_param: usize,
) {
    let title = if focused { "Envelope [↑↓:param  ←→:adjust]" } else { "Envelope" };
    let border_style = if focused { Style::default().fg(Color::Yellow) } else { Style::default() };
    let block = Block::default().title(title).borders(Borders::ALL).border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 5 || inner.width < 10 {
        return;
    }

    let chart_h = inner.height.saturating_sub(2) as usize;
    let chart_w = inner.width as usize;

    const PHASE_COLORS: [Color; 4] = [Color::Green, Color::Yellow, Color::Cyan, Color::Magenta];
    const PHASE_NAMES: [&str; 4] = ["A", "D", "S", "R"];

    // Proportional column widths; sustain gets a fixed display time
    let sustain_t = 0.3f32;
    let total_t = env.attack + env.decay + sustain_t + env.release;
    let a_cols = ((env.attack / total_t) * chart_w as f32).round() as usize;
    let d_cols = ((env.decay  / total_t) * chart_w as f32).round() as usize;
    let r_cols = ((env.release / total_t) * chart_w as f32).round() as usize;
    let s_cols = chart_w.saturating_sub(a_cols + d_cols + r_cols);

    // Where each phase starts (column index)
    let phase_starts = [a_cols, a_cols + d_cols, a_cols + d_cols + s_cols];
    let phase_ranges = [
        (0,                        a_cols),
        (a_cols,                   a_cols + d_cols),
        (a_cols + d_cols,          a_cols + d_cols + s_cols),
        (a_cols + d_cols + s_cols, chart_w),
    ];

    // Envelope amplitude value (0.0–1.0) for each column
    let values: Vec<f32> = (0..chart_w).map(|x| {
        if x < a_cols {
            if a_cols > 0 { x as f32 / a_cols as f32 } else { 1.0 }
        } else if x < a_cols + d_cols {
            let t = (x - a_cols) as f32 / d_cols.max(1) as f32;
            1.0 - t * (1.0 - env.sustain)
        } else if x < a_cols + d_cols + s_cols {
            env.sustain
        } else {
            let t = (x - a_cols - d_cols - s_cols) as f32 / r_cols.max(1) as f32;
            (env.sustain * (1.0 - t)).max(0.0)
        }
    }).collect();

    // Amplitude -> row (row 0 = top = 1.0, row chart_h-1 = bottom = 0.0)
    let to_row = |v: f32| -> usize {
        ((1.0 - v.clamp(0.0, 1.0)) * (chart_h - 1) as f32).round() as usize
    };

    let phase_of = |x: usize| -> usize {
        if x < a_cols { 0 } else if x < a_cols + d_cols { 1 } else if x < a_cols + d_cols + s_cols { 2 } else { 3 }
    };

    // Build character grid: (char, Color, bold)
    let mut grid: Vec<Vec<(char, Color, bool)>> =
        vec![vec![(' ', Color::Reset, false); chart_w]; chart_h];

    for x in 0..chart_w {
        let v     = values[x];
        let prev_v = if x > 0 { values[x - 1] } else { 0.0 };
        let y      = to_row(v);
        let prev_y = to_row(prev_v);
        let color  = PHASE_COLORS[phase_of(x)];

        // Fill area below the line
        for yr in (y + 1)..chart_h {
            if grid[yr][x].0 == ' ' {
                grid[yr][x] = ('░', color, false);
            }
        }

        // Draw the outline character(s)
        if x == 0 || y == prev_y {
            if y < chart_h { grid[y][x] = ('─', color, true); }
        } else {
            let rising = v > prev_v;
            let (y_min, y_max) = if y < prev_y { (y, prev_y) } else { (prev_y, y) };
            for yr in y_min..=y_max {
                if yr < chart_h {
                    grid[yr][x] = (if rising { '╱' } else { '╲' }, color, true);
                }
            }
        }

        // Phase boundary: dim vertical dashes in empty space only
        if phase_starts.contains(&x) {
            for yr in 0..chart_h {
                if grid[yr][x].0 == ' ' {
                    grid[yr][x] = ('┊', Color::DarkGray, false);
                }
            }
        }
    }

    // Render chart rows
    let mut all_lines: Vec<Line> = grid.iter().map(|row| {
        Line::from(row.iter().map(|(ch, color, bold)| {
            let style = if *bold {
                Style::default().fg(*color).add_modifier(Modifier::BOLD)
            } else if *ch == '░' {
                Style::default().fg(*color).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(*color)
            };
            Span::styled(ch.to_string(), style)
        }).collect::<Vec<_>>())
    }).collect();

    // Phase label row — phase letter centered in its column range
    let mut label_spans: Vec<Span> = Vec::new();
    let mut cursor = 0usize;
    for (p, (start, end)) in phase_ranges.iter().enumerate() {
        let width = end - start;
        if width == 0 { continue; }
        let mid = start + width / 2;
        if mid > cursor {
            label_spans.push(Span::raw(" ".repeat(mid - cursor)));
            cursor = mid;
        }
        let is_sel = p == selected_param && focused;
        let style = if is_sel {
            Style::default().fg(PHASE_COLORS[p]).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().fg(PHASE_COLORS[p])
        };
        label_spans.push(Span::styled(PHASE_NAMES[p], style));
        cursor += 1;
    }
    all_lines.push(Line::from(label_spans));

    // Values row — highlighted when selected
    let param_strs = [
        (format!("A:{:.2}s", env.attack),  0usize),
        (format!("D:{:.2}s", env.decay),   1),
        (format!("S:{:.2} ", env.sustain), 2),
        (format!("R:{:.2}s", env.release), 3),
    ];
    let mut value_spans: Vec<Span> = Vec::new();
    for (i, (label, p)) in param_strs.iter().enumerate() {
        if i > 0 { value_spans.push(Span::raw(" ")); }
        let is_sel = *p == selected_param && focused;
        let style = if is_sel {
            Style::default().fg(PHASE_COLORS[*p]).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(PHASE_COLORS[*p])
        };
        value_spans.push(Span::styled(label.clone(), style));
    }
    all_lines.push(Line::from(value_spans));

    frame.render_widget(Paragraph::new(all_lines), inner);
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
