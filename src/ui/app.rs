use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs},
};

use crate::midi::server::MidiStatus;
use crate::preset;
use crate::state::messages::{ConfigCommand, NoteCommand};
use crate::state::synth_state::SynthState;
use crate::udp::server::UdpStatus;
use crate::ui::mode::UIMode;
use crate::ui::widgets::{
    fx_group_panel::FxGroupPanel,
    sequencer_panel::SequencerPanel,
    voice_panel::{self, VoicePanel},
};

enum PathPromptMode {
    Save,
    Load,
    LoadSample { voice: usize },
}

struct PathPrompt {
    mode: PathPromptMode,
    input: String,
    /// Index into the current directory's file listing, when navigating by arrow key.
    selected: Option<usize>,
}

pub struct App {
    mode: UIMode,
    voice_panel: VoicePanel,
    fx_panel: FxGroupPanel,
    sequencer_panel: SequencerPanel,
    state: Arc<ArcSwap<SynthState>>,
    config_tx: Sender<ConfigCommand>,
    note_tx: Sender<NoteCommand>,
    udp_status: Arc<Mutex<UdpStatus>>,
    midi_status: Arc<Mutex<MidiStatus>>,
    running: bool,
    status_msg: Option<(String, Instant)>,
    path_prompt: Option<PathPrompt>,
    last_filename: String,
    last_sample_filename: String,
    preset_dir: std::path::PathBuf,
    sample_dir: std::path::PathBuf,
}

impl App {
    pub fn new(
        state: Arc<ArcSwap<SynthState>>,
        config_tx: Sender<ConfigCommand>,
        note_tx: Sender<NoteCommand>,
        udp_status: Arc<Mutex<UdpStatus>>,
        midi_status: Arc<Mutex<MidiStatus>>,
        preset_dir: std::path::PathBuf,
        sample_dir: std::path::PathBuf,
    ) -> Self {
        App {
            mode: UIMode::Voices,
            voice_panel: VoicePanel::new(),
            fx_panel: FxGroupPanel::new(),
            sequencer_panel: SequencerPanel::new(),
            state,
            config_tx,
            note_tx,
            udp_status,
            midi_status,
            running: true,
            status_msg: None,
            path_prompt: None,
            last_filename: "preset.json".to_string(),
            last_sample_filename: String::new(),
            preset_dir,
            sample_dir,
        }
    }

    /// Directory the currently open path prompt lists/loads/saves files in.
    fn prompt_dir(&self) -> &std::path::Path {
        match self.path_prompt.as_ref().map(|p| &p.mode) {
            Some(PathPromptMode::LoadSample { .. }) => &self.sample_dir,
            _ => &self.preset_dir,
        }
    }

    pub fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        let frame_duration = Duration::from_millis(16);

        while self.running {
            let frame_start = Instant::now();
            let state = self.state.load_full();

            // Expire status message after 2 seconds
            if let Some((_, ts)) = self.status_msg
                && ts.elapsed() > Duration::from_secs(2)
            {
                self.status_msg = None;
            }

            terminal.draw(|f| self.render(f, &state))?;

            let elapsed = frame_start.elapsed();
            let remaining = frame_duration.saturating_sub(elapsed);
            if event::poll(remaining)?
                && let Event::Key(key) = event::read()?
            {
                self.handle_key(key, &state);
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
                Constraint::Length(10),
                Constraint::Length(2),
            ])
            .split(frame.area());

        self.render_header(frame, chunks[0]);

        match self.mode {
            UIMode::Voices    => self.voice_panel.render(frame, chunks[1], state),
            UIMode::FxGroups  => self.fx_panel.render(frame, chunks[1], state),
            UIMode::Sequencer => self.sequencer_panel.render(frame, chunks[1], state),
        }

        voice_panel::render_oscilloscope(frame, chunks[2], &state.scope);
        self.render_status_bar(frame, chunks[3]);

        self.render_path_prompt(frame, frame.area());
    }

    fn render_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(0), Constraint::Length(34)])
            .split(area);

        let title = Paragraph::new("PILOT Rust Synth")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        let modes = [UIMode::Voices, UIMode::FxGroups, UIMode::Sequencer];
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

        // UDP + MIDI status widget
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
        let (midi_text, midi_style) = match &*self.midi_status.lock().unwrap() {
            MidiStatus::Starting => (
                "MIDI: starting…".to_string(),
                Style::default().fg(Color::Yellow),
            ),
            MidiStatus::Connected { port_name } => (
                format!("MIDI: {}", port_name),
                Style::default().fg(Color::Green),
            ),
            MidiStatus::NotFound => (
                "MIDI: Launchpad X not found".to_string(),
                Style::default().fg(Color::Gray),
            ),
            MidiStatus::Failed { reason } => (
                format!("MIDI ERR: {}", reason),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        let status_lines = vec![
            Line::from(Span::styled(udp_text, udp_style)),
            Line::from(Span::styled(midi_text, midi_style)),
        ];
        let status = Paragraph::new(status_lines).block(Block::default().borders(Borders::ALL));
        frame.render_widget(status, chunks[2]);
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
                UIMode::Voices    => self.voice_panel.help_text(),
                UIMode::FxGroups  => "↑↓:Navigate effects  Enter:Edit params  a:Add effect  d:Delete  </>:Reorder  e:Toggle group  1/2/3:Page  q:Quit",
                UIMode::Sequencer => self.sequencer_panel.help_text(),
            }
        };
        let p = Paragraph::new(help).style(Style::default().fg(Color::White));
        frame.render_widget(p, area);
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        // Path prompt intercepts all keys when open
        if self.path_prompt.is_some() {
            self.handle_path_prompt_key(key);
            return;
        }

        // Picker intercepts all keys when open
        if self.mode == UIMode::FxGroups && self.fx_panel.show_picker {
            self.handle_picker_key(key, state);
            return;
        }

        if key.code == KeyCode::Char('q') {
            self.running = false;
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.path_prompt = Some(PathPrompt { mode: PathPromptMode::Save, input: self.last_filename.clone(), selected: None });
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            self.path_prompt = Some(PathPrompt { mode: PathPromptMode::Load, input: self.last_filename.clone(), selected: None });
            return;
        }

        // 1/2/3 always switch pages
        match key.code {
            KeyCode::Char('1') => { self.mode = UIMode::Voices; return; }
            KeyCode::Char('2') => { self.mode = UIMode::FxGroups; return; }
            KeyCode::Char('3') => { self.mode = UIMode::Sequencer; return; }
            _ => {}
        }

        match self.mode {
            UIMode::Voices    => self.handle_voices_key(key, state),
            UIMode::FxGroups  => self.handle_fx_key(key, state),
            UIMode::Sequencer => self.handle_sequencer_key(key, state),
        }
    }

    fn handle_path_prompt_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.path_prompt.is_none() { return; }
        match key.code {
            KeyCode::Char(c) => {
                let prompt = self.path_prompt.as_mut().unwrap();
                prompt.input.push(c);
                prompt.selected = None;
            }
            KeyCode::Backspace => {
                let prompt = self.path_prompt.as_mut().unwrap();
                prompt.input.pop();
                prompt.selected = None;
            }
            KeyCode::Up | KeyCode::Down => {
                let files = list_files(self.prompt_dir());
                let prompt = self.path_prompt.as_mut().unwrap();
                if !files.is_empty() {
                    let next = match prompt.selected {
                        Some(i) if key.code == KeyCode::Up => i.saturating_sub(1),
                        Some(i) => (i + 1).min(files.len() - 1),
                        None => 0,
                    };
                    prompt.input = files[next].clone();
                    prompt.selected = Some(next);
                }
            }
            KeyCode::Esc => {
                self.path_prompt = None;
            }
            KeyCode::Enter => {
                let prompt = self.path_prompt.take().unwrap();
                let msg = match prompt.mode {
                    PathPromptMode::Save => {
                        let path = self.preset_dir.join(&prompt.input);
                        let result = match preset::save(&self.state.load_full(), &path) {
                            Ok(()) => format!("Saved preset to {}", path.display()),
                            Err(e) => format!("Save failed: {e}"),
                        };
                        self.last_filename = prompt.input;
                        result
                    }
                    PathPromptMode::Load => {
                        let path = self.preset_dir.join(&prompt.input);
                        let result = match preset::load(&path, &self.sample_dir) {
                            Ok(cmds) => {
                                for cmd in cmds {
                                    let _ = self.config_tx.try_send(cmd);
                                }
                                format!("Loaded preset from {}", path.display())
                            }
                            Err(e) => format!("Load failed: {e}"),
                        };
                        self.last_filename = prompt.input;
                        result
                    }
                    PathPromptMode::LoadSample { voice } => {
                        let path = self.sample_dir.join(&prompt.input);
                        let result = match crate::audio::sample::load_wav(&path) {
                            Ok(sample) => {
                                let name = prompt.input.clone();
                                let _ = self.config_tx.try_send(ConfigCommand::LoadSample {
                                    voice,
                                    sample: Arc::new(sample),
                                    name: name.clone(),
                                    root_note: 60,
                                });
                                format!("Loaded sample {} into voice {:X}", name, voice)
                            }
                            Err(e) => format!("Sample load failed: {e}"),
                        };
                        self.last_sample_filename = prompt.input;
                        result
                    }
                };
                self.status_msg = Some((msg, Instant::now()));
            }
            _ => {}
        }
    }

    fn render_path_prompt(&self, frame: &mut Frame, area: Rect) {
        let Some(ref prompt) = self.path_prompt else { return };
        let popup = centered_rect(50, 40, area);
        frame.render_widget(Clear, popup);
        let title = match prompt.mode {
            PathPromptMode::Save => "Save preset as  (↑↓:pick  Enter:confirm  Esc:cancel)",
            PathPromptMode::Load => "Load preset  (↑↓:pick  Enter:confirm  Esc:cancel)",
            PathPromptMode::LoadSample { .. } => "Load sample (.wav)  (↑↓:pick  Enter:confirm  Esc:cancel)",
        };
        let dir = self.prompt_dir();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(popup);

        let input = Paragraph::new(format!("{}█", prompt.input))
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        frame.render_widget(input, chunks[0]);

        let files = list_files(dir);
        let list_block = Block::default()
            .title(format!("Files in {}", dir.display()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        if files.is_empty() {
            let empty = Paragraph::new(format!("(no files in {})", dir.display()))
                .style(Style::default().fg(Color::Gray))
                .block(list_block);
            frame.render_widget(empty, chunks[1]);
        } else {
            let items: Vec<ListItem> = files.iter().enumerate().map(|(i, name)| {
                let is_sel = prompt.selected == Some(i);
                let style = if is_sel {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                let indicator = if is_sel { "► " } else { "  " };
                ListItem::new(Line::styled(format!("{}{}", indicator, name), style))
            }).collect();
            frame.render_widget(List::new(items).block(list_block), chunks[1]);
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
                let position = group_ref(state, panel.selected_group).effects.len();
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
                length_units: 35, // full sample length if the voice is playing a sample
                detune_cents: 0.0,
            });
            return;
        }

        if key.code == KeyCode::Char('L') {
            let voice = self.voice_panel.selected_voice;
            self.path_prompt = Some(PathPrompt {
                mode: PathPromptMode::LoadSample { voice },
                input: self.last_sample_filename.clone(),
                selected: None,
            });
            return;
        }

        if key.code == KeyCode::Char('X') {
            let voice = self.voice_panel.selected_voice;
            let _ = self.config_tx.try_send(ConfigCommand::ClearSample { voice });
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
                KeyCode::Up if panel.selected_param > 0 => { panel.selected_param -= 1; }
                KeyCode::Down => {
                    let param_count = group_ref(state, panel.selected_group)
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
                    panel.selected_effect = group_ref(state, panel.selected_group).effects.len().saturating_sub(1);
                }
            }
            KeyCode::Down => {
                let effect_count = group_ref(state, panel.selected_group).effects.len();
                if panel.selected_effect + 1 < effect_count {
                    panel.selected_effect += 1;
                } else if panel.selected_group < 4 {
                    panel.selected_group += 1;
                    panel.selected_effect = 0;
                }
            }
            KeyCode::Enter => {
                let has_params = group_ref(state, panel.selected_group)
                    .effects.get(panel.selected_effect)
                    .map(|e| !e.params.is_empty()).unwrap_or(false);
                if has_params {
                    panel.editing = true;
                    panel.selected_param = 0;
                }
            }
            KeyCode::Char('e') => {
                let enabled = !group_ref(state, panel.selected_group).enabled;
                let _ = self.config_tx.try_send(ConfigCommand::EnableGroup {
                    group: panel.selected_group, enabled,
                });
            }
            KeyCode::Char('a') => {
                panel.show_picker = true;
                panel.picker_selection = 0;
            }
            KeyCode::Char('d') if !group_ref(state, panel.selected_group).effects.is_empty() => {
                let _ = self.config_tx.try_send(ConfigCommand::RemoveEffect {
                    group: panel.selected_group,
                    position: panel.selected_effect,
                });
                panel.selected_effect = panel.selected_effect.saturating_sub(1);
                panel.selected_param = 0;
            }
            KeyCode::Char('<') => {
                let effect_count = group_ref(state, panel.selected_group).effects.len();
                if panel.selected_effect > 0 && effect_count > 1 {
                    let _ = self.config_tx.try_send(ConfigCommand::ReorderEffect {
                        group: panel.selected_group,
                        from: panel.selected_effect,
                        to: panel.selected_effect - 1,
                    });
                    panel.selected_effect -= 1;
                }
            }
            KeyCode::Char('>') => {
                let effect_count = group_ref(state, panel.selected_group).effects.len();
                if panel.selected_effect + 1 < effect_count {
                    let _ = self.config_tx.try_send(ConfigCommand::ReorderEffect {
                        group: panel.selected_group,
                        from: panel.selected_effect,
                        to: panel.selected_effect + 1,
                    });
                    panel.selected_effect += 1;
                }
            }
            _ => {}
        }
    }

    fn adjust_fx_param(&self, state: &SynthState, dir: i32, fine: bool) {
        let panel = &self.fx_panel;
        let group = group_ref(state, panel.selected_group);
        if let Some(effect) = group.effects.get(panel.selected_effect)
            && let Some(param) = effect.params.get(panel.selected_param)
        {
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

    fn handle_sequencer_key(&mut self, key: crossterm::event::KeyEvent, state: &SynthState) {
        let cmds = self.sequencer_panel.handle_key(key, state);
        // Single-step edits (not bulk copy/paste) get an audible preview.
        if let [ConfigCommand::SeqSetStep { voice, enabled: true, midi_note, velocity, .. }] = cmds.as_slice() {
            let _ = self.note_tx.try_send(NoteCommand {
                channel: *voice,
                midi_note: *midi_note,
                velocity: *velocity,
                length_samples: 12000, // ~0.25s at 48kHz
                length_units: 35, // full sample length if the voice is playing a sample
                detune_cents: 0.0,
            });
        }
        for cmd in cmds {
            let _ = self.config_tx.try_send(cmd);
        }
    }
}

fn list_files(dir: &std::path::Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Groups 0-3 are the send groups A-D; group 4 is the global bus.
fn group_ref(state: &SynthState, group: usize) -> &crate::state::synth_state::GroupState {
    if group < 4 { &state.groups[group] } else { &state.global }
}

/// Returns a centered rect of `percent_x` × `percent_y` within `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(layout[1])[1]
}
