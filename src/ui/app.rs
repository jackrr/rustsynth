use std::sync::Arc;
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

use crate::state::messages::{ConfigCommand, EffectType};
use crate::state::synth_state::SynthState;
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
            .constraints([Constraint::Length(20), Constraint::Min(0), Constraint::Length(15)])
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
            UIMode::Voices   => self.voice_panel.help_text(),
            UIMode::FxGroups => "↑↓:Select effect  ←→:Adjust param  a:Add  d:Delete  e:Toggle  Tab:Mode  q:Quit",
            UIMode::Routing  => "↑↓:Voice  Tab:Group  ←→:Adjust level  c:Copy  p:Paste  z:Zero  q:Quit",
        };
        let p = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, area);
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        // q always quits (unless in a text entry field, which we don't have)
        if key.code == KeyCode::Char('q') {
            self.running = false;
            return;
        }

        // Tab switches mode only when the voice panel is in Grid mode
        // (in other sections Tab moves between sub-sections)
        let voices_in_grid = self.mode == UIMode::Voices
            && self.voice_panel.edit_section == VoiceEditSection::Grid;

        if key.code == KeyCode::Tab && (self.mode != UIMode::Voices || voices_in_grid) {
            self.mode = match self.mode {
                UIMode::Voices   => UIMode::FxGroups,
                UIMode::FxGroups => UIMode::Routing,
                UIMode::Routing  => UIMode::Voices,
            };
            return;
        }

        match key.code {
            KeyCode::Char('1') if voices_in_grid => { self.mode = UIMode::Voices; return; }
            KeyCode::Char('2') if voices_in_grid => { self.mode = UIMode::FxGroups; return; }
            KeyCode::Char('3') if voices_in_grid => { self.mode = UIMode::Routing; return; }
            _ => {}
        }

        match self.mode {
            UIMode::Voices   => self.handle_voices_key(key, state),
            UIMode::FxGroups => self.handle_fx_key(key, state),
            UIMode::Routing  => self.handle_routing_key(key, state),
        }
    }

    fn handle_voices_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        if let Some(cmd) = self.voice_panel.handle_key(key, state) {
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
            KeyCode::Left  => { if panel.selected_param > 0 { panel.selected_param -= 1; } }
            KeyCode::Right => { panel.selected_param += 1; }
            KeyCode::Char('e') => {
                let enabled = !state.groups[panel.selected_group].enabled;
                let _ = self.config_tx.try_send(ConfigCommand::EnableGroup {
                    group: panel.selected_group, enabled,
                });
            }
            KeyCode::Char('a') => {
                // Add reverb as default; a real picker could be added later
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
                    panel.selected_effect = panel.selected_effect.saturating_sub(1);
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
            // ↑↓ navigate voices
            KeyCode::Up   => { panel.selected_voice = panel.selected_voice.saturating_sub(1); }
            KeyCode::Down => { panel.selected_voice = (panel.selected_voice + 1).min(15); }

            // Tab / Shift-Tab cycle groups
            KeyCode::Tab      => { panel.selected_group = (panel.selected_group + 1) % 4; }
            KeyCode::BackTab  => { panel.selected_group = panel.selected_group.checked_sub(1).unwrap_or(3); }

            // ← → directly adjust the selected cell's send level
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

            // Enter toggles 0 / 100%
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
