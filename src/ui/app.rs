use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::preset;
use crate::state::messages::{ConfigCommand, NoteCommand};
use crate::state::synth_state::SynthState;
use crate::udp::server::UdpStatus;
use crate::ui::mode::UIMode;
use crate::ui::widgets::{
    fx_group_panel::FxGroupPanel,
    routing_panel::RoutingPanel,
    voice_panel::VoicePanel,
};

pub struct App {
    mode: UIMode,
    voice_panel: VoicePanel,
    fx_panel: FxGroupPanel,
    routing_panel: RoutingPanel,
    state: Arc<ArcSwap<SynthState>>,
    config_tx: Sender<ConfigCommand>,
    note_tx: Sender<NoteCommand>,
    udp_status: Arc<Mutex<UdpStatus>>,
    running: bool,
    status_msg: Option<(String, Instant)>,
}

impl App {
    pub fn new(
        state: Arc<ArcSwap<SynthState>>,
        config_tx: Sender<ConfigCommand>,
        note_tx: Sender<NoteCommand>,
        udp_status: Arc<Mutex<UdpStatus>>,
    ) -> Self {
        App {
            mode: UIMode::Voices,
            voice_panel: VoicePanel::new(),
            fx_panel: FxGroupPanel::new(),
            routing_panel: RoutingPanel::new(),
            state,
            config_tx,
            note_tx,
            udp_status,
            running: true,
            status_msg: None,
        }
    }

    pub fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        let frame_duration = Duration::from_millis(16);

        while self.running {
            let frame_start = Instant::now();
            let state = self.state.load_full();

            // Expire status message after 2 seconds
            if let Some((_, ts)) = self.status_msg {
                if ts.elapsed() > Duration::from_secs(2) {
                    self.status_msg = None;
                }
            }

            terminal.draw(|f| self.render(f, &state))?;

            let elapsed = frame_start.elapsed();
            let remaining = frame_duration.saturating_sub(elapsed);
            if event::poll(remaining)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key, &state);
                }
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame, state: &SynthState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(frame.area());

        self.render_header(frame, chunks[0]);

        match self.mode {
            UIMode::Voices   => self.voice_panel.render(frame, chunks[1], state),
            UIMode::FxGroups => self.fx_panel.render(frame, chunks[1], state),
            UIMode::Routing  => self.routing_panel.render(frame, chunks[1], state),
        }

        self.render_status_bar(frame, chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(0), Constraint::Length(22)])
            .split(area);

        let title = Paragraph::new("PILOT Rust Synth")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        let modes = [UIMode::Voices, UIMode::FxGroups, UIMode::Routing];
        let tab_titles: Vec<Line> = modes.iter().map(|m| {
            let style = if *m == self.mode {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(m.tab_label(), style))
        }).collect();
        let selected_tab = modes.iter().position(|m| *m == self.mode).unwrap_or(0);
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL))
            .select(selected_tab)
            .highlight_style(Style::default())  // span styles already handle highlighting
            .divider("|");
        frame.render_widget(tabs, chunks[1]);

        // UDP status widget
        let (udp_text, udp_style) = match &*self.udp_status.lock().unwrap() {
            UdpStatus::Starting => (
                "UDP: starting…".to_string(),
                Style::default().fg(Color::Yellow),
            ),
            UdpStatus::Bound { addr } => (
                format!("UDP: {}", addr),
                Style::default().fg(Color::Green),
            ),
            UdpStatus::Failed { reason } => (
                format!("UDP ERR: {}", reason),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        let udp = Paragraph::new(udp_text)
            .style(udp_style)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(udp, chunks[2]);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // Show timed status message if active
        if let Some((ref msg, _)) = self.status_msg {
            let p = Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Yellow));
            frame.render_widget(p, area);
            return;
        }

        let help = if self.mode == UIMode::FxGroups && self.fx_panel.show_picker {
            "↑↓:Select effect  Enter:Add  Esc:Cancel"
        } else {
            match self.mode {
                UIMode::Voices   => self.voice_panel.help_text(),
                UIMode::FxGroups => "↑↓:Navigate effects  Enter:Edit params  a:Add effect  d:Delete  e:Toggle group  1/2/3:Page  q:Quit",
                UIMode::Routing  => "↑↓:Voice  Tab/[]:Group  ←→:Adjust  Enter:Toggle 0/100%  c:Copy  p:Paste  z:Zero  1/2/3:Page  q:Quit",
            }
        };
        let p = Paragraph::new(help).style(Style::default().fg(Color::White));
        frame.render_widget(p, area);
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        // Picker intercepts all keys when open
        if self.mode == UIMode::FxGroups && self.fx_panel.show_picker {
            self.handle_picker_key(key, state);
            return;
        }

        if key.code == KeyCode::Char('q') {
            self.running = false;
            return;
        }

        let preset_path = std::path::Path::new("preset.json");

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            let msg = match preset::save(&state, preset_path) {
                Ok(()) => format!("Saved preset to {}", preset_path.display()),
                Err(e) => format!("Save failed: {e}"),
            };
            self.status_msg = Some((msg, Instant::now()));
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            let msg = match preset::load(preset_path) {
                Ok(cmds) => {
                    for cmd in cmds {
                        let _ = self.config_tx.try_send(cmd);
                    }
                    format!("Loaded preset from {}", preset_path.display())
                }
                Err(e) => format!("Load failed: {e}"),
            };
            self.status_msg = Some((msg, Instant::now()));
            return;
        }

        // 1/2/3 always switch pages
        match key.code {
            KeyCode::Char('1') => { self.mode = UIMode::Voices; return; }
            KeyCode::Char('2') => { self.mode = UIMode::FxGroups; return; }
            KeyCode::Char('3') => { self.mode = UIMode::Routing; return; }
            _ => {}
        }


        match self.mode {
            UIMode::Voices   => self.handle_voices_key(key, state),
            UIMode::FxGroups => self.handle_fx_key(key, state),
            UIMode::Routing  => self.handle_routing_key(key, state),
        }
    }

    fn handle_picker_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let panel = &mut self.fx_panel;
        match key.code {
            KeyCode::Up => {
                panel.picker_selection = panel.picker_selection.saturating_sub(1);
            }
            KeyCode::Down => {
                panel.picker_selection = (panel.picker_selection + 1).min(15); // 16 effects
            }
            KeyCode::Enter => {
                let effect_type = panel.picker_selected_effect();
                let position = state.groups[panel.selected_group].effects.len();
                let _ = self.config_tx.try_send(ConfigCommand::AddEffect {
                    group: panel.selected_group,
                    effect_type,
                    position,
                });
                // Move selection to the newly added effect
                panel.selected_effect = position;
                panel.selected_param = 0;
                panel.show_picker = false;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                panel.show_picker = false;
            }
            _ => {}
        }
    }

    fn handle_voices_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        if key.code == KeyCode::Char(' ') {
            let v = &state.voices[self.voice_panel.selected_voice];
            let _ = self.note_tx.try_send(NoteCommand {
                channel: self.voice_panel.selected_voice,
                midi_note: v.default_midi_note,
                velocity: v.default_velocity,
                length_samples: 24000, // ~0.5s at 48kHz
            });
            return;
        }
        for cmd in self.voice_panel.handle_key(key, state) {
            let _ = self.config_tx.try_send(cmd);
        }
    }

    fn handle_fx_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let panel = &mut self.fx_panel;

        if panel.editing {
            match key.code {
                KeyCode::Up => {
                    if panel.selected_param > 0 { panel.selected_param -= 1; }
                }
                KeyCode::Down => {
                    let param_count = state.groups[panel.selected_group]
                        .effects.get(panel.selected_effect)
                        .map(|e| e.params.len()).unwrap_or(0);
                    if panel.selected_param + 1 < param_count {
                        panel.selected_param += 1;
                    }
                }
                KeyCode::Left  => self.adjust_fx_param(state, -1, key.modifiers.contains(KeyModifiers::SHIFT)),
                KeyCode::Right => self.adjust_fx_param(state,  1, key.modifiers.contains(KeyModifiers::SHIFT)),
                KeyCode::Enter | KeyCode::Esc => { self.fx_panel.editing = false; }
                _ => {}
            }
            return;
        }

        // Navigate mode
        match key.code {
            KeyCode::Up => {
                if panel.selected_effect > 0 {
                    panel.selected_effect -= 1;
                } else if panel.selected_group > 0 {
                    panel.selected_group -= 1;
                    panel.selected_effect = state.groups[panel.selected_group].effects.len().saturating_sub(1);
                }
            }
            KeyCode::Down => {
                let effect_count = state.groups[panel.selected_group].effects.len();
                if panel.selected_effect + 1 < effect_count {
                    panel.selected_effect += 1;
                } else if panel.selected_group < 3 {
                    panel.selected_group += 1;
                    panel.selected_effect = 0;
                }
            }
            KeyCode::Enter => {
                let has_params = state.groups[panel.selected_group]
                    .effects.get(panel.selected_effect)
                    .map(|e| !e.params.is_empty()).unwrap_or(false);
                if has_params {
                    panel.editing = true;
                    panel.selected_param = 0;
                }
            }
            KeyCode::Char('e') => {
                let enabled = !state.groups[panel.selected_group].enabled;
                let _ = self.config_tx.try_send(ConfigCommand::EnableGroup {
                    group: panel.selected_group, enabled,
                });
            }
            KeyCode::Char('a') => {
                panel.show_picker = true;
                panel.picker_selection = 0;
            }
            KeyCode::Char('d') => {
                if !state.groups[panel.selected_group].effects.is_empty() {
                    let _ = self.config_tx.try_send(ConfigCommand::RemoveEffect {
                        group: panel.selected_group,
                        position: panel.selected_effect,
                    });
                    panel.selected_effect = panel.selected_effect.saturating_sub(1);
                    panel.selected_param = 0;
                }
            }
            _ => {}
        }
    }

    fn adjust_fx_param(&self, state: &SynthState, dir: i32, fine: bool) {
        let panel = &self.fx_panel;
        let group = &state.groups[panel.selected_group];
        if let Some(effect) = group.effects.get(panel.selected_effect) {
            if let Some(param) = effect.params.get(panel.selected_param) {
                let new_value = if param.labels.is_some() {
                    // Enum param: always step by 1, wrap at boundaries.
                    (param.value + dir as f32).clamp(param.min, param.max)
                } else if param.logarithmic {
                    // Logarithmic param (frequency): multiply by a semitone-based factor.
                    // Coarse = 2 semitones, fine = 1 semitone per press.
                    let semitones: f32 = if fine { 1.0 } else { 2.0 };
                    let factor = (2.0_f32).powf(semitones / 12.0);
                    let factor = if dir > 0 { factor } else { 1.0 / factor };
                    (param.value * factor).clamp(param.min, param.max)
                } else {
                    let range = param.max - param.min;
                    let step = if fine { 0.01 } else { 0.05 };
                    (param.value + dir as f32 * step * range).clamp(param.min, param.max)
                };
                let _ = self.config_tx.try_send(ConfigCommand::SetEffectParam {
                    group: panel.selected_group,
                    effect_idx: panel.selected_effect,
                    param: param.name.clone(),
                    value: new_value,
                });
            }
        }
    }

    fn handle_routing_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let panel = &mut self.routing_panel;
        match key.code {
            KeyCode::Up   => { panel.selected_voice = panel.selected_voice.saturating_sub(1); }
            KeyCode::Down => { panel.selected_voice = (panel.selected_voice + 1).min(15); }
            KeyCode::Tab  => { panel.selected_group = (panel.selected_group + 1) % 4; }

            KeyCode::Char(']') => { panel.selected_group = (panel.selected_group + 1) % 4; }
            KeyCode::Char('[') => { panel.selected_group = panel.selected_group.checked_sub(1).unwrap_or(3); }

            KeyCode::Left => {
                let step = if key.modifiers.contains(KeyModifiers::SHIFT) { 0.01 } else { 0.1 };
                let cur = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: (cur - step).clamp(0.0, 1.0),
                });
            }
            KeyCode::Right => {
                let step = if key.modifiers.contains(KeyModifiers::SHIFT) { 0.01 } else { 0.1 };
                let cur = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: (cur + step).clamp(0.0, 1.0),
                });
            }
            KeyCode::Enter => {
                let cur = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: if cur > 0.01 { 0.0 } else { 1.0 },
                });
            }
            KeyCode::Char('c') => { panel.clipboard = Some(state.routing[panel.selected_voice]); }
            KeyCode::Char('p') => {
                if let Some(cb) = panel.clipboard {
                    for g in 0..4 {
                        let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                            voice: panel.selected_voice, group: g, level: cb[g],
                        });
                    }
                }
            }
            KeyCode::Char('z') => {
                for g in 0..4 {
                    let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                        voice: panel.selected_voice, group: g, level: 0.0,
                    });
                }
            }
            _ => {}
        }
    }
}
