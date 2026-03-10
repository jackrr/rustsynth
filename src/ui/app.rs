use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::state::messages::ConfigCommand;
use crate::state::synth_state::SynthState;
use crate::udp::server::UdpStatus;
use crate::ui::mode::UIMode;
use crate::ui::widgets::{
    fx_group_panel::FxGroupPanel,
    routing_panel::RoutingPanel,
    voice_panel::{VoiceEditSection, VoicePanel},
};

pub struct App {
    mode: UIMode,
    voice_panel: VoicePanel,
    fx_panel: FxGroupPanel,
    routing_panel: RoutingPanel,
    state: Arc<ArcSwap<SynthState>>,
    config_tx: Sender<ConfigCommand>,
    udp_status: Arc<Mutex<UdpStatus>>,
    running: bool,
}

impl App {
    pub fn new(
        state: Arc<ArcSwap<SynthState>>,
        config_tx: Sender<ConfigCommand>,
        udp_status: Arc<Mutex<UdpStatus>>,
    ) -> Self {
        App {
            mode: UIMode::Voices,
            voice_panel: VoicePanel::new(),
            fx_panel: FxGroupPanel::new(),
            routing_panel: RoutingPanel::new(),
            state,
            config_tx,
            udp_status,
            running: true,
        }
    }

    pub fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        let frame_duration = Duration::from_millis(16);

        while self.running {
            let frame_start = Instant::now();
            let state = self.state.load_full();

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
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL))
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
        let help = if self.mode == UIMode::FxGroups && self.fx_panel.show_picker {
            "↑↓:Select effect  Enter:Add  Esc:Cancel"
        } else {
            match self.mode {
                UIMode::Voices   => self.voice_panel.help_text(),
                UIMode::FxGroups => "↑↓:Select effect  ←→:Adjust param  a:Add  d:Delete  e:Toggle  Tab:Mode  q:Quit",
                UIMode::Routing  => "↑↓:Voice  []:Group  ←→:Adjust  Enter:Toggle 0/100%  c:Copy  p:Paste  z:Zero  q:Quit",
            }
        };
        let p = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
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

        let voices_in_grid = self.mode == UIMode::Voices
            && self.voice_panel.edit_section == VoiceEditSection::Grid;
        let tab_switches_mode = self.mode != UIMode::Voices || voices_in_grid;

        if key.code == KeyCode::Tab && tab_switches_mode {
            self.mode = match self.mode {
                UIMode::Voices   => UIMode::FxGroups,
                UIMode::FxGroups => UIMode::Routing,
                UIMode::Routing  => UIMode::Voices,
            };
            return;
        }

        match key.code {
            KeyCode::Char('1') if tab_switches_mode => { self.mode = UIMode::Voices; return; }
            KeyCode::Char('2') if tab_switches_mode => { self.mode = UIMode::FxGroups; return; }
            KeyCode::Char('3') if tab_switches_mode => { self.mode = UIMode::Routing; return; }
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
                panel.picker_selection = (panel.picker_selection + 1).min(14); // 15 effects
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
        for cmd in self.voice_panel.handle_key(key, state) {
            let _ = self.config_tx.try_send(cmd);
        }
    }

    fn handle_fx_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let panel = &mut self.fx_panel;
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
            KeyCode::Left => {
                if panel.selected_param > 0 { panel.selected_param -= 1; }
            }
            KeyCode::Right => {
                let param_count = state.groups[panel.selected_group]
                    .effects.get(panel.selected_effect)
                    .map(|e| e.params.len()).unwrap_or(0);
                if panel.selected_param + 1 < param_count {
                    panel.selected_param += 1;
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
            KeyCode::Char('+') | KeyCode::Char('=') => self.adjust_fx_param(state,  0.05),
            KeyCode::Char('-')                       => self.adjust_fx_param(state, -0.05),
            _ => {}
        }
    }

    fn adjust_fx_param(&self, state: &SynthState, delta: f32) {
        let panel = &self.fx_panel;
        let group = &state.groups[panel.selected_group];
        if let Some(effect) = group.effects.get(panel.selected_effect) {
            if let Some(param) = effect.params.get(panel.selected_param) {
                let new_value = (param.value + delta * (param.max - param.min))
                    .clamp(param.min, param.max);
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

            KeyCode::Char(']') => { panel.selected_group = (panel.selected_group + 1) % 4; }
            KeyCode::Char('[') => { panel.selected_group = panel.selected_group.checked_sub(1).unwrap_or(3); }

            KeyCode::Left => {
                let cur = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: (cur - 0.05).clamp(0.0, 1.0),
                });
            }
            KeyCode::Right => {
                let cur = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: (cur + 0.05).clamp(0.0, 1.0),
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
