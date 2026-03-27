use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::state::messages::{ConfigCommand, OscillatorType};
use crate::state::synth_state::{EnvelopeParams, SynthState};

/// Flat parameter indices used when editing a voice.
/// ↑↓ cycles through these; ←→ adjusts the selected one.
/// 0-2: OSC (wave, note, velocity)
/// 3-5: SUB (on, octave, level)
/// 6-9: ENV (attack, decay, sustain, release)
/// 10-13: SENDS (A, B, C, D)
const PARAM_COUNT: usize = 14;

/// Snapshot of a voice's configuration, used for copy/paste
#[derive(Debug, Clone)]
pub struct VoiceClipboard {
    pub osc_type: OscillatorType,
    pub envelope: EnvelopeParams,
    pub sends: [f32; 4],
    pub sub_osc_enabled: bool,
    pub sub_osc_octave: i32,
    pub sub_osc_level: f32,
}

pub struct VoicePanel {
    pub selected_voice: usize,
    /// Whether we're in edit mode (Enter toggles)
    pub editing: bool,
    /// Which param is highlighted when editing (0–13)
    pub selected_param: usize,
    pub clipboard: Option<VoiceClipboard>,
}

impl VoicePanel {
    pub fn new() -> Self {
        VoicePanel {
            selected_voice: 0,
            editing: false,
            selected_param: 0,
            clipboard: None,
        }
    }

    /// Convert voice index to visual (row, col) in the 2×8 grid
    fn voice_to_grid(v: usize) -> (usize, usize) {
        if v < 4       { (0, v) }
        else if v < 8  { (1, v - 4) }
        else if v < 12 { (0, v - 4) }
        else           { (1, v - 8) }
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
        let mode_hint = if self.editing { "  [EDITING — ↑↓:param  ←→:adjust  Shift:fine  Enter/Esc:done]" } else { "  [↑↓←→:navigate  Enter:edit  c:copy  p:paste]" };
        let title = format!("Voices{}{}", mode_hint, clip_hint);
        let block = Block::default().title(title).borders(Borders::ALL).border_style(
            if self.editing { Style::default().fg(Color::Yellow) } else { Style::default() }
        );
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

                    let border_style = if is_selected && self.editing {
                        Style::default().fg(Color::Yellow)
                    } else if is_selected {
                        Style::default().fg(Color::Cyan)
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
                Constraint::Percentage(20),  // osc + env params (stacked)
                Constraint::Percentage(57),  // adsr shape
                Constraint::Percentage(23),  // sends
            ])
            .split(rows[0]);

        // --- Left: stacked Osc + Sub + Env params ---
        render_params_panel(
            frame, chunks[0], state, self.selected_voice,
            self.editing, self.selected_param,
        );

        // --- Middle: ADSR shape ---
        render_adsr_shape(frame, chunks[1], &voice.envelope);

        // --- Right: Sends ---
        let send_labels = ["A", "B", "C", "D"];
        let sends_focused = self.editing && self.selected_param >= 10;

        let sends: Vec<ListItem> = (0..4).map(|g| {
            let level = state.routing[self.selected_voice][g];
            let bar = send_bar(level, 10);
            let is_selected = sends_focused && g == self.selected_param - 10;
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

        let send_title = if sends_focused { "Sends [↑↓  ←→]" } else { "Sends" };
        let sends_list = List::new(sends)
            .block(Block::default().title(send_title).borders(Borders::ALL).border_style(
                if sends_focused { Style::default().fg(Color::Yellow) } else { Style::default() }
            ));
        frame.render_widget(sends_list, chunks[2]);

        render_oscilloscope(frame, rows[1], &state.scope);
    }

    pub fn help_text(&self) -> &str {
        if self.editing {
            "↑↓:Select param  ←→:Adjust  Shift:Fine  Space:Trigger  Enter/Esc:Done editing  q:Quit"
        } else {
            "↑↓←→:Navigate voice  Tab:Next  Space:Trigger  Enter:Edit  o:Cycle osc  c:Copy  p:Paste  q:Quit"
        }
    }

    /// Handle a key event; returns commands to send to the audio engine.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) -> Vec<ConfigCommand> {
        use crossterm::event::{KeyCode, KeyModifiers};
        let fine = key.modifiers.contains(KeyModifiers::SHIFT);

        if self.editing {
            match key.code {
                KeyCode::Up    => { self.selected_param = self.selected_param.saturating_sub(1); vec![] }
                KeyCode::Down  => { self.selected_param = (self.selected_param + 1).min(PARAM_COUNT - 1); vec![] }
                KeyCode::Left  => self.adjust_param(state, -1, fine),
                KeyCode::Right => self.adjust_param(state,  1, fine),
                KeyCode::Enter | KeyCode::Esc => { self.editing = false; vec![] }
                _ => vec![],
            }
        } else {
            match key.code {
                KeyCode::Up    => { self.move_grid(-1,  0); vec![] }
                KeyCode::Down  => { self.move_grid( 1,  0); vec![] }
                KeyCode::Left  => { self.move_grid( 0, -1); vec![] }
                KeyCode::Right => { self.move_grid( 0,  1); vec![] }
                KeyCode::Tab      => { self.selected_voice = (self.selected_voice + 1) % 16; vec![] }
                KeyCode::BackTab  => { self.selected_voice = self.selected_voice.checked_sub(1).unwrap_or(15); vec![] }
                KeyCode::Enter    => { self.editing = true; vec![] }
                KeyCode::Char('o') => self.cycle_osc(state, 1).into_iter().collect(),
                KeyCode::Char('c') => { self.copy_voice(state); vec![] }
                KeyCode::Char('p') => self.paste_voice(state),
                _ => vec![],
            }
        }
    }

    fn copy_voice(&mut self, state: &SynthState) {
        let voice = &state.voices[self.selected_voice];
        self.clipboard = Some(VoiceClipboard {
            osc_type: voice.osc_type,
            envelope: voice.envelope.clone(),
            sends: state.routing[self.selected_voice],
            sub_osc_enabled: voice.sub_osc_enabled,
            sub_osc_octave: voice.sub_osc_octave,
            sub_osc_level: voice.sub_osc_level,
        });
    }

    fn paste_voice(&self, state: &SynthState) -> Vec<ConfigCommand> {
        let Some(ref clip) = self.clipboard else { return vec![]; };
        let dst = self.selected_voice;
        let _ = state;
        let mut cmds = vec![
            ConfigCommand::SetOscillator { voice: dst, osc_type: clip.osc_type },
            ConfigCommand::SetEnvelope {
                voice: dst,
                attack:  clip.envelope.attack,
                decay:   clip.envelope.decay,
                sustain: clip.envelope.sustain,
                release: clip.envelope.release,
            },
            ConfigCommand::SetSubOsc {
                voice: dst,
                enabled: clip.sub_osc_enabled,
                octave: clip.sub_osc_octave,
                level: clip.sub_osc_level,
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

    /// Adjust the currently selected param by `dir` (+1 or -1).
    fn adjust_param(&self, state: &SynthState, dir: i32, fine: bool) -> Vec<ConfigCommand> {
        let v = &state.voices[self.selected_voice];
        let voice = self.selected_voice;
        match self.selected_param {
            // --- OSC ---
            0 => self.cycle_osc(state, dir).into_iter().collect(),
            1 => {
                let step: i32 = if fine { 1 } else { 12 };
                let new_note = (v.default_midi_note as i32 + dir * step).clamp(0, 127) as u8;
                vec![ConfigCommand::SetDefaultNote { voice, midi_note: new_note }]
            }
            2 => {
                let step = if fine { 0.01 } else { 0.05 };
                let new_vel = (v.default_velocity + dir as f32 * step).clamp(0.0, 1.0);
                vec![ConfigCommand::SetDefaultVelocity { voice, velocity: new_vel }]
            }
            // --- SUB ---
            3 => vec![ConfigCommand::SetSubOsc {
                voice,
                enabled: !v.sub_osc_enabled,
                octave: v.sub_osc_octave,
                level: v.sub_osc_level,
            }],
            4 => {
                let new_oct = (v.sub_osc_octave + dir).clamp(-2, 2);
                vec![ConfigCommand::SetSubOsc { voice, enabled: v.sub_osc_enabled, octave: new_oct, level: v.sub_osc_level }]
            }
            5 => {
                let step = if fine { 0.01 } else { 0.05 };
                let new_level = (v.sub_osc_level + dir as f32 * step).clamp(0.0, 1.0);
                vec![ConfigCommand::SetSubOsc { voice, enabled: v.sub_osc_enabled, octave: v.sub_osc_octave, level: new_level }]
            }
            // --- ENV ---
            6..=9 => {
                let env = &v.envelope;
                let (a, d, s, r) = (env.attack, env.decay, env.sustain, env.release);
                let sign = dir as f32;
                let (na, nd, ns, nr) = match self.selected_param {
                    6 => { let st = if fine { 0.01 } else { 0.1 }; ((a + sign * st).clamp(0.001, 10.0), d, s, r) }
                    7 => { let st = if fine { 0.01 } else { 0.1 }; (a, (d + sign * st).clamp(0.001, 10.0), s, r) }
                    8 => { let st = if fine { 0.01 } else { 0.05 }; (a, d, (s + sign * st).clamp(0.0, 1.0), r) }
                    9 => { let st = if fine { 0.01 } else { 0.1 }; (a, d, s, (r + sign * st).clamp(0.001, 10.0)) }
                    _ => unreachable!(),
                };
                vec![ConfigCommand::SetEnvelope { voice, attack: na, decay: nd, sustain: ns, release: nr }]
            }
            // --- SENDS ---
            10..=13 => {
                let g = self.selected_param - 10;
                let step = if fine { 0.01 } else { 0.1 };
                let current = state.routing[voice][g];
                vec![ConfigCommand::SetSendLevel { voice, group: g, level: (current + dir as f32 * step).clamp(0.0, 1.0) }]
            }
            _ => vec![],
        }
    }
}

fn render_oscilloscope(frame: &mut Frame, area: Rect, scope: &[f32]) {
    let block = Block::default().title("Oscilloscope").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 || scope.is_empty() {
        return;
    }

    let dot_w = inner.width as usize * 2;
    let dot_h = inner.height as usize * 4;

    let sync_pos = (1..scope.len().saturating_sub(dot_w + 2))
        .find(|&i| scope[i - 1] < 0.0 && scope[i] >= 0.0)
        .unwrap_or(0);

    let next_crossing = ((sync_pos + 4).min(scope.len())..scope.len().saturating_sub(1))
        .find(|&i| scope[i - 1] < 0.0 && scope[i] >= 0.0);

    let display_len = if let Some(next) = next_crossing {
        let period = next - sync_pos;
        ((period as f32 * 2.5) as usize)
            .clamp(32, scope.len().saturating_sub(sync_pos))
    } else {
        (dot_w * 3).min(scope.len().saturating_sub(sync_pos))
    };

    let samples = &scope[sync_pos..(sync_pos + display_len).min(scope.len())];
    if samples.len() < 2 { return; }

    let mut dots = vec![false; dot_w * dot_h];

    let to_dot_row = |v: f32| -> usize {
        let zoom = 0.55_f32;
        let norm = (1.0 - (v / zoom).clamp(-1.0, 1.0)) / 2.0;
        (norm * (dot_h - 1) as f32).round() as usize
    };

    let interp = |dx: usize| -> f32 {
        let t = dx as f32 / (dot_w - 1).max(1) as f32;
        let si_f = t * (samples.len() - 1) as f32;
        let si0 = si_f as usize;
        let si1 = (si0 + 1).min(samples.len() - 1);
        let frac = si_f - si0 as f32;
        samples[si0] * (1.0 - frac) + samples[si1] * frac
    };

    for dx in 0..dot_w {
        let row = to_dot_row(interp(dx));
        let prev_row = if dx > 0 { to_dot_row(interp(dx - 1)) } else { row };
        let (r_min, r_max) = if row <= prev_row { (row, prev_row) } else { (prev_row, row) };
        for r in r_min..=r_max {
            if r < dot_h { dots[r * dot_w + dx] = true; }
        }
    }

    const DOT_MAP: [(usize, usize, u32); 8] = [
        (0, 0, 0x01), (1, 0, 0x02), (2, 0, 0x04), (3, 0, 0x40),
        (0, 1, 0x08), (1, 1, 0x10), (2, 1, 0x20), (3, 1, 0x80),
    ];

    let center_char_row = dot_h / 2 / 4;

    let lines: Vec<Line> = (0..inner.height as usize).map(|cy| {
        Line::from((0..inner.width as usize).map(|cx| {
            let mut bits: u32 = 0;
            for (dr, dc, bit) in &DOT_MAP {
                let r = cy * 4 + dr;
                let c = cx * 2 + dc;
                if r < dot_h && c < dot_w && dots[r * dot_w + c] { bits |= bit; }
            }

            let (ch, color) = if bits != 0 {
                let ch = char::from_u32(0x2800 + bits).unwrap_or('?');
                let cx_dot = cx * 2 + 1;
                let t = cx_dot as f32 / (dot_w - 1).max(1) as f32;
                let si = (t * (samples.len() - 1) as f32) as usize;
                let amp = samples[si.min(samples.len() - 1)].abs();
                let color = if amp > 0.8 { Color::Red }
                            else if amp > 0.4 { Color::Yellow }
                            else { Color::Green };
                (ch, color)
            } else if cy == center_char_row {
                ('·', Color::DarkGray)
            } else {
                (' ', Color::Reset)
            };

            Span::styled(ch.to_string(), Style::default().fg(color))
        }).collect::<Vec<_>>())
    }).collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Combined oscillator + sub + envelope params panel (left column of voice detail).
fn render_params_panel(
    frame: &mut Frame,
    area: Rect,
    state: &SynthState,
    voice_idx: usize,
    editing: bool,
    selected_param: usize,
) {
    let v = &state.voices[voice_idx];

    let title = if editing { "Params [↑↓  ←→]" } else { "Params" };
    let border_style = if editing { Style::default().fg(Color::Yellow) } else { Style::default() };
    let block = Block::default().title(title).borders(Borders::ALL).border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 { return; }

    let osc_types = OscillatorType::all();
    let osc_idx = osc_types.iter().position(|&t| t == v.osc_type).unwrap_or(0);

    let osc_fields: [(&str, String); 3] = [
        ("Wave", format!("◄{}►  {}/{}", v.osc_type.name(), osc_idx + 1, osc_types.len())),
        ("Note", midi_note_name(v.default_midi_note)),
        ("Vel",  format!("{:.0}%", v.default_velocity * 100.0)),
    ];

    const PHASE_COLORS: [Color; 4] = [Color::Green, Color::Yellow, Color::Cyan, Color::Magenta];
    let env = &v.envelope;
    let env_fields: [(&str, f32, &str, f32, f32); 4] = [
        ("A", env.attack,  "s", 0.001, 10.0),
        ("D", env.decay,   "s", 0.001, 10.0),
        ("S", env.sustain, "",  0.0,   1.0),
        ("R", env.release, "s", 0.001, 10.0),
    ];

    let mut lines: Vec<Line> = Vec::new();

    // OSC section
    lines.push(Line::styled(
        " OSC",
        Style::default().fg(if editing && selected_param <= 2 { Color::Yellow } else { Color::DarkGray })
            .add_modifier(if editing && selected_param <= 2 { Modifier::BOLD } else { Modifier::empty() }),
    ));
    for (i, (label, value)) in osc_fields.iter().enumerate() {
        let is_sel = editing && i == selected_param;
        let ind = if is_sel { "►" } else { " " };
        if is_sel {
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}: ", ind, label), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(value.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}: ", ind, label), Style::default().fg(Color::DarkGray)),
                Span::raw(value.clone()),
            ]));
        }
    }

    // SUB section
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        " SUB",
        Style::default().fg(if editing && selected_param >= 3 && selected_param <= 5 { Color::Yellow } else { Color::DarkGray })
            .add_modifier(if editing && selected_param >= 3 && selected_param <= 5 { Modifier::BOLD } else { Modifier::empty() }),
    ));
    let sub_fields: [(&str, String); 3] = [
        ("On",  if v.sub_osc_enabled { "On".to_string() } else { "Off".to_string() }),
        ("Oct", format!("{:+}", v.sub_osc_octave)),
        ("Lvl", format!("{:.0}%", v.sub_osc_level * 100.0)),
    ];
    for (i, (label, value)) in sub_fields.iter().enumerate() {
        let param_idx = 3 + i;
        let is_sel = editing && param_idx == selected_param;
        let ind = if is_sel { "►" } else { " " };
        let dim = !v.sub_osc_enabled && i > 0;
        let val_color = if dim { Color::DarkGray } else { Color::Cyan };
        if is_sel {
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}: ", ind, label), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(value.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}: ", ind, label), Style::default().fg(Color::DarkGray)),
                Span::styled(value.clone(), Style::default().fg(val_color)),
            ]));
        }
    }

    // ENV section
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        " ENV",
        Style::default().fg(if editing && selected_param >= 6 && selected_param <= 9 { Color::Yellow } else { Color::DarkGray })
            .add_modifier(if editing && selected_param >= 6 && selected_param <= 9 { Modifier::BOLD } else { Modifier::empty() }),
    ));
    for (i, (name, val, unit, min, max)) in env_fields.iter().enumerate() {
        let param_idx = 6 + i;
        let is_sel = editing && param_idx == selected_param;
        let color = PHASE_COLORS[i];
        let bar = mini_bar(*val, *min, *max, 4);
        let ind = if is_sel { "►" } else { " " };
        let text = format!("{}{} {:.2}{} {}", ind, name, val, unit, bar);
        let style = if is_sel {
            Style::default().fg(color).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(color)
        };
        lines.push(Line::styled(text, style));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// ADSR shape rendered with braille dots for sub-character resolution.
fn render_adsr_shape(frame: &mut Frame, area: Rect, env: &EnvelopeParams) {
    let block = Block::default().title("Envelope").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 { return; }

    let dot_w = inner.width as usize * 2;
    let dot_h = inner.height as usize * 4;

    const PHASE_COLORS: [Color; 4] = [Color::Green, Color::Yellow, Color::Cyan, Color::Magenta];

    let sustain_t = 0.3f32;
    let total_t = env.attack + env.decay + sustain_t + env.release;
    let a_cols = ((env.attack  / total_t) * dot_w as f32).round() as usize;
    let d_cols = ((env.decay   / total_t) * dot_w as f32).round() as usize;
    let r_cols = ((env.release / total_t) * dot_w as f32).round() as usize;
    let s_cols = dot_w.saturating_sub(a_cols + d_cols + r_cols);

    let phase_of = |x: usize| -> usize {
        if x < a_cols { 0 }
        else if x < a_cols + d_cols { 1 }
        else if x < a_cols + d_cols + s_cols { 2 }
        else { 3 }
    };

    let envelope_at = |x: usize| -> f32 {
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
    };

    let to_dot_row = |v: f32| -> usize {
        ((1.0 - v.clamp(0.0, 1.0)) * (dot_h - 1) as f32).round() as usize
    };

    let mut dots: Vec<(bool, Color)> = vec![(false, Color::Reset); dot_w * dot_h];

    for dx in 0..dot_w {
        let outline_row = to_dot_row(envelope_at(dx));
        let color = PHASE_COLORS[phase_of(dx)];

        for r in outline_row..dot_h {
            dots[r * dot_w + dx] = (true, color);
        }

        if dx > 0 {
            let prev_outline = to_dot_row(envelope_at(dx - 1));
            let prev_color = PHASE_COLORS[phase_of(dx - 1)];
            if outline_row < prev_outline {
                for r in outline_row..prev_outline {
                    dots[r * dot_w + dx] = (true, color);
                }
            } else if prev_outline < outline_row {
                for r in prev_outline..outline_row {
                    if !dots[r * dot_w + (dx - 1)].0 {
                        dots[r * dot_w + (dx - 1)] = (true, prev_color);
                    }
                }
            }
        }
    }

    const DOT_MAP: [(usize, usize, u32); 8] = [
        (0, 0, 0x01), (1, 0, 0x02), (2, 0, 0x04), (3, 0, 0x40),
        (0, 1, 0x08), (1, 1, 0x10), (2, 1, 0x20), (3, 1, 0x80),
    ];

    let h = inner.height as usize;
    let w = inner.width as usize;

    let lines: Vec<Line> = (0..h).map(|cy| {
        Line::from((0..w).map(|cx| {
            let mut bits: u32 = 0;
            let mut cell_color = Color::DarkGray;
            let mut top_set_row = dot_h;
            for (dr, dc, bit) in &DOT_MAP {
                let r = cy * 4 + dr;
                let c = cx * 2 + dc;
                if r < dot_h && c < dot_w {
                    let (set, col) = dots[r * dot_w + c];
                    if set {
                        bits |= bit;
                        if r < top_set_row {
                            top_set_row = r;
                            cell_color = col;
                        }
                    }
                }
            }
            let ch = if bits != 0 {
                char::from_u32(0x2800 + bits).unwrap_or('?')
            } else {
                ' '
            };
            Span::styled(ch.to_string(), Style::default().fg(cell_color))
        }).collect::<Vec<_>>())
    }).collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn mini_bar(value: f32, min: f32, max: f32, width: usize) -> String {
    let range = (max - min).max(0.001);
    let filled = (((value - min) / range).clamp(0.0, 1.0) * width as f32) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
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
