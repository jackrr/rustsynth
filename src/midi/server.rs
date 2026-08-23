use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection};

use crate::midi::parser::pad_to_voice;
use crate::state::messages::NoteCommand;
use crate::state::synth_state::SynthState;

const PORT_NAME_MATCH: &str = "Launchpad X";

/// Novation SysEx header, device ID 0C = Launchpad X (Pro MK3 is 0E, Mini MK3 is 0D).
const SYSEX_HEADER: [u8; 6] = [0xF0, 0x00, 0x20, 0x29, 0x02, 0x0C];

/// Current state of the MIDI controller connection, shared with the TUI for display.
#[derive(Debug, Clone)]
pub enum MidiStatus {
    Starting,
    Connected { port_name: String },
    NotFound,
    Failed { reason: String },
}

/// Enter Programmer mode, which hands raw control over pad notes/LEDs to us
/// instead of the device's built-in "Live" behavior.
fn enter_programmer_mode(out: &mut MidiOutputConnection) -> Result<(), Box<dyn std::error::Error>> {
    let mut msg = SYSEX_HEADER.to_vec();
    msg.extend_from_slice(&[0x0E, 0x01, 0xF7]);
    out.send(&msg)?;
    Ok(())
}

pub fn run_midi_server(
    note_tx: Sender<NoteCommand>,
    synth_state: Arc<ArcSwap<SynthState>>,
    sample_rate: f32,
    status: Arc<Mutex<MidiStatus>>,
) {
    if let Err(e) = try_run(note_tx, synth_state, sample_rate, &status) {
        *status.lock().unwrap() = MidiStatus::Failed { reason: e.to_string() };
    }
}

fn try_run(
    note_tx: Sender<NoteCommand>,
    synth_state: Arc<ArcSwap<SynthState>>,
    sample_rate: f32,
    status: &Arc<Mutex<MidiStatus>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let midi_out = MidiOutput::new("rustsynth-out")?;
    let out_port = midi_out
        .ports()
        .into_iter()
        .find(|p| midi_out.port_name(p).map(|n| n.contains(PORT_NAME_MATCH)).unwrap_or(false));

    let mut midi_in = MidiInput::new("rustsynth-in")?;
    let in_port = midi_in
        .ports()
        .into_iter()
        .find(|p| midi_in.port_name(p).map(|n| n.contains(PORT_NAME_MATCH)).unwrap_or(false));

    let (Some(out_port), Some(in_port)) = (out_port, in_port) else {
        *status.lock().unwrap() = MidiStatus::NotFound;
        return Ok(());
    };

    let mut out_conn = midi_out.connect(&out_port, "rustsynth-launchpad-out")?;
    enter_programmer_mode(&mut out_conn)?;
    let out_conn = Arc::new(Mutex::new(out_conn));

    midi_in.ignore(Ignore::TimeAndActiveSense);

    let port_name = midi_in.port_name(&in_port).unwrap_or_else(|_| PORT_NAME_MATCH.to_string());

    let cb_out_conn = out_conn.clone();
    let _in_conn = midi_in.connect(
        &in_port,
        "rustsynth-launchpad-in",
        move |_stamp, message, _| {
            handle_message(message, &note_tx, &synth_state, sample_rate, &cb_out_conn);
        },
        (),
    )?;

    *status.lock().unwrap() = MidiStatus::Connected { port_name };

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Handle one raw MIDI message from the Launchpad X: Note On lights the pad and
/// triggers that voice's configured default note/octave at the pad's velocity;
/// Note Off (or Note On velocity 0) turns the pad's LED back off.
fn handle_message(
    message: &[u8],
    note_tx: &Sender<NoteCommand>,
    synth_state: &Arc<ArcSwap<SynthState>>,
    sample_rate: f32,
    out_conn: &Arc<Mutex<MidiOutputConnection>>,
) {
    if message.len() < 3 {
        return;
    }
    let status = message[0] & 0xF0;
    let note = message[1];
    let velocity = message[2];

    match status {
        0x90 if velocity > 0 => {
            let Some(voice) = pad_to_voice(note) else { return };
            let state = synth_state.load();
            let v = &state.voices[voice];
            let _ = note_tx.try_send(NoteCommand {
                channel: voice,
                midi_note: v.default_midi_note,
                velocity: velocity as f32 / 127.0,
                length_samples: (0.5 * sample_rate) as u64,
                length_units: 35,
                detune_cents: 0.0,
            });
            if let Ok(mut out) = out_conn.lock() {
                let _ = out.send(&[0x90, note, velocity]);
            }
        }
        0x80 | 0x90 => {
            if let Ok(mut out) = out_conn.lock() {
                let _ = out.send(&[0x80, note, 0]);
            }
        }
        _ => {}
    }
}
