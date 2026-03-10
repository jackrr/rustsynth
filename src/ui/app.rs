use std::sync::Arc;
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

use crate::state::messages::{ConfigCommand, EffectType};
use crate::state::synth_state::SynthState;
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
    running: bool,
}

impl App {
    pub fn new(state: Arc<ArcSwap<SynthState>>, config_tx: Sender<ConfigCommand>) -> Self {
        App {
            mode: UIMode::Voices,
            voice_panel: VoicePanel::new(),
            fx_panel: FxGroupPanel::new(),
            routing_panel: RoutingPanel::new(),
            state,
            config_tx,
            running: true,
        }
    }

    pub fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        let frame_duration = Duration::from_millis(16); // ~60 FPS

        while self.running {
            let frame_start = Instant::now();

            // Read current state snapshot
            let state = self.state.load_full();

            // Render
            terminal.draw(|f| self.render(f, &state))?;

            // Handle input with timeout
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
        let size = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header/tabs
                Constraint::Min(0),    // Content
                Constraint::Length(2), // Status bar
            ])
            .split(size);

        // Header
        self.render_header(frame, chunks[0]);

        // Content area
        match self.mode {
            UIMode::Voices => self.voice_panel.render(frame, chunks[1], state),
            UIMode::FxGroups => self.fx_panel.render(frame, chunks[1], state),
            UIMode::Routing => self.routing_panel.render(frame, chunks[1], state),
        }

        // Status bar
        self.render_status_bar(frame, chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20),
                Constraint::Min(0),
                Constraint::Length(15),
            ])
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

        let udp = Paragraph::new("UDP:49161")
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(udp, chunks[2]);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let help = match self.mode {
            UIMode::Voices => "↑↓:Select Voice  ←→:Adjust  Tab:Mode  q:Quit",
            UIMode::FxGroups => "↑↓:Select  ←→:Adjust  a:Add Effect  d:Delete  e:Toggle  Tab:Mode  q:Quit",
            UIMode::Routing => "↑↓←→:Navigate  ←→:Adjust  c:Copy  p:Paste  z:Zero  Tab:Mode  q:Quit",
        };
        let p = Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, area);
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        // Global keys
        match key.code {
            KeyCode::Char('q') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) || true {
                    self.running = false;
                }
            }
            KeyCode::Tab => {
                self.mode = match self.mode {
                    UIMode::Voices => UIMode::FxGroups,
                    UIMode::FxGroups => UIMode::Routing,
                    UIMode::Routing => UIMode::Voices,
                };
                return;
            }
            KeyCode::Char('1') => { self.mode = UIMode::Voices; return; }
            KeyCode::Char('2') => { self.mode = UIMode::FxGroups; return; }
            KeyCode::Char('3') => { self.mode = UIMode::Routing; return; }
            _ => {}
        }

        match self.mode {
            UIMode::Voices => self.handle_voices_key(key, state),
            UIMode::FxGroups => self.handle_fx_key(key, state),
            UIMode::Routing => self.handle_routing_key(key, state),
        }
    }

    fn handle_voices_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let panel = &mut self.voice_panel;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if panel.selected_voice > 0 {
                    panel.selected_voice -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if panel.selected_voice < 15 {
                    panel.selected_voice += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if panel.selected_param > 0 {
                    panel.selected_param -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if panel.selected_param < 8 {
                    panel.selected_param += 1;
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(cmd) = panel.adjust_param(state, 0.05) {
                    let _ = self.config_tx.try_send(cmd);
                }
            }
            KeyCode::Char('-') => {
                if let Some(cmd) = panel.adjust_param(state, -0.05) {
                    let _ = self.config_tx.try_send(cmd);
                }
            }
            _ => {}
        }
    }

    fn handle_fx_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let panel = &mut self.fx_panel;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if panel.selected_effect > 0 {
                    panel.selected_effect -= 1;
                } else if panel.selected_group > 0 {
                    panel.selected_group -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let effect_count = state.groups[panel.selected_group].effects.len();
                if panel.selected_effect + 1 < effect_count {
                    panel.selected_effect += 1;
                } else if panel.selected_group < 3 {
                    panel.selected_group += 1;
                    panel.selected_effect = 0;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if panel.selected_param > 0 {
                    panel.selected_param -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                panel.selected_param += 1;
            }
            KeyCode::Char('e') => {
                let enabled = !state.groups[panel.selected_group].enabled;
                let _ = self.config_tx.try_send(ConfigCommand::EnableGroup {
                    group: panel.selected_group,
                    enabled,
                });
            }
            KeyCode::Char('a') => {
                // Add reverb to selected group (demo - could show a picker)
                let _ = self.config_tx.try_send(ConfigCommand::AddEffect {
                    group: panel.selected_group,
                    effect_type: EffectType::Reverb,
                    position: state.groups[panel.selected_group].effects.len(),
                });
            }
            KeyCode::Char('d') => {
                if !state.groups[panel.selected_group].effects.is_empty() {
                    let _ = self.config_tx.try_send(ConfigCommand::RemoveEffect {
                        group: panel.selected_group,
                        position: panel.selected_effect,
                    });
                    if panel.selected_effect > 0 {
                        panel.selected_effect -= 1;
                    }
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.adjust_fx_param(state, 0.05);
            }
            KeyCode::Char('-') => {
                self.adjust_fx_param(state, -0.05);
            }
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
            KeyCode::Up | KeyCode::Char('k') => {
                if panel.selected_voice > 0 {
                    panel.selected_voice -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if panel.selected_voice < 15 {
                    panel.selected_voice += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if panel.selected_group > 0 {
                    panel.selected_group -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if panel.selected_group < 3 {
                    panel.selected_group += 1;
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let current = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: (current + 0.05).clamp(0.0, 1.0),
                });
            }
            KeyCode::Char('-') => {
                let current = state.routing[panel.selected_voice][panel.selected_group];
                let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                    voice: panel.selected_voice,
                    group: panel.selected_group,
                    level: (current - 0.05).clamp(0.0, 1.0),
                });
            }
            KeyCode::Char('c') => {
                // Copy routing row
                panel.clipboard = Some(state.routing[panel.selected_voice]);
            }
            KeyCode::Char('p') => {
                // Paste routing row
                if let Some(clipboard) = panel.clipboard {
                    for g in 0..4 {
                        let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                            voice: panel.selected_voice,
                            group: g,
                            level: clipboard[g],
                        });
                    }
                }
            }
            KeyCode::Char('z') => {
                // Zero all sends for this voice
                for g in 0..4 {
                    let _ = self.config_tx.try_send(ConfigCommand::SetSendLevel {
                        voice: panel.selected_voice,
                        group: g,
                        level: 0.0,
                    });
                }
            }
            _ => {}
        }
    }
}
