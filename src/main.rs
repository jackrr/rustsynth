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

    // Set up CPAL audio
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host.default_output_device()
        .expect("No output audio device found");

    let config = device.default_output_config()
        .expect("Could not get default output config");

    let sample_rate = config.sample_rate().0 as f32;
    eprintln!("Audio: {} Hz, {:?}", sample_rate, config.sample_format());

    let mut engine = AudioEngine::new(sample_rate, note_rx, config_rx, state_for_audio);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    engine.process_block(data);
                },
                |err| eprintln!("Audio error: {}", err),
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            device.build_output_stream(
                &config.into(),
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    let mut buf: Vec<f32> = vec![0.0; data.len()];
                    engine.process_block(&mut buf);
                    for (d, s) in data.iter_mut().zip(buf.iter()) {
                        *d = cpal::Sample::from_sample(*s);
                    }
                },
                |err| eprintln!("Audio error: {}", err),
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            device.build_output_stream(
                &config.into(),
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    let mut buf: Vec<f32> = vec![0.0; data.len()];
                    engine.process_block(&mut buf);
                    for (d, s) in data.iter_mut().zip(buf.iter()) {
                        *d = cpal::Sample::from_sample(*s);
                    }
                },
                |err| eprintln!("Audio error: {}", err),
                None,
            )?
        }
        fmt => panic!("Unsupported sample format: {:?}", fmt),
    };

    stream.play()?;

    // Start UDP server in a tokio runtime (background thread)
    let note_tx_for_udp = note_tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(run_udp_server(note_tx_for_udp, sample_rate));
    });

    // Set up TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(state_for_tui, config_tx);
    let result = app.run(&mut terminal);

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
