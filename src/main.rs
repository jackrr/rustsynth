mod audio;
mod state;
mod udp;
mod ui;

use std::sync::Arc;
use std::io;

use arc_swap::ArcSwap;
use crossbeam_channel::bounded;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use audio::engine::AudioEngine;
use state::{messages::{ConfigCommand, NoteCommand}, synth_state::SynthState};
use udp::server::run_udp_server;
use ui::app::App;

fn main() -> anyhow::Result<()> {
    // Create channels
    let (note_tx, note_rx) = bounded::<NoteCommand>(256);
    let (config_tx, config_rx) = bounded::<ConfigCommand>(256);

    // Shared state (Audio → TUI)
    let synth_state = Arc::new(ArcSwap::new(Arc::new(SynthState::default())));
    let state_for_tui = synth_state.clone();
    let state_for_audio = synth_state.clone();

    // Start UDP server (blocking, in its own thread — no tokio needed)
    let note_tx_for_udp = note_tx.clone();
    std::thread::Builder::new()
        .name("udp-server".into())
        .spawn(move || run_udp_server(note_tx_for_udp, 48000.0))
        .expect("Failed to spawn UDP server thread");

    // Set up CPAL audio
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host.default_output_device()
        .expect("No output audio device found");

    let supported_config = device.default_output_config()
        .expect("Could not get default output config");

    let sample_rate = supported_config.sample_rate().0 as f32;

    // Force F32 output regardless of device default — avoids conversion bugs
    // and the panic on unsupported formats (I32, U32, etc.)
    let stream_config = cpal::StreamConfig {
        channels: supported_config.channels(),
        sample_rate: supported_config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut engine = AudioEngine::new(sample_rate, note_rx, config_rx, state_for_audio);

    let stream = device.build_output_stream(
        &stream_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            engine.process_block(data);
        },
        |err| eprintln!("Audio stream error: {}", err),
        None,
    )?;

    stream.play()?;

    // Set up TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(state_for_tui, config_tx);
    let result = app.run(&mut terminal);

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
