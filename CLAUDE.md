# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build
cargo build --release

# Run
cargo run

# Test
cargo test
cargo test udp::parser   # run a specific module's tests

# Lint
cargo clippy
```

## Architecture

RustSynth is a Rust TUI synthesizer compatible with [Orca](https://github.com/hundredrabbits/Orca) via UDP. It runs three concurrent threads:

1. **Main thread** — TUI (ratatui 60fps), keyboard input → sends `ConfigCommand` to audio
2. **Tokio thread** — UDP server on port 49161, parses note commands → sends `NoteCommand` to audio
3. **CPAL audio thread** — `AudioEngine::process_block()`, real-time hot path, publishes `SynthState` via ArcSwap at 20fps

**Critical constraint:** The audio thread must never block. No mutexes, no allocations in the hot path. All cross-thread communication is lock-free:
- UDP/TUI → Audio: `crossbeam-channel` (bounded, `try_recv`)
- Audio → TUI: `arc-swap` state snapshots

## Key Data Flow

```
UDP packet "04C" → parse_command() → NoteCommand { channel:0, midi_note:60, ... }
    → AudioEngine → Voice[0].trigger() → oscillator * ADSR * velocity
    → RoutingMatrix → EffectGroup[A..D] → mix → f32 output samples

TUI keypress → ConfigCommand::SetOscillator { voice, osc_type }
    → AudioEngine::apply_config_change() → Voice[voice].oscillator.set_type()

AudioEngine (20fps) → SynthState snapshot → ArcSwap → TUI reads for display
```

## UDP Protocol

Format: `[channel][octave][note][velocity?][length?][offset?]`
All fields are base-36 alphanumeric, case-insensitive.

- `channel`: hex 0-F (voice 0-15)
- `octave`: digit 0-9
- `note`: letter a-g (chromatic in octave), h-z (above G), 0-9 (below C)
- `velocity`: optional base-36 digit (0-Z → 0.0–1.0), default 0.5
- `length`: optional base-36 digit (maps to note duration), default 1/16 bar at 120bpm
- `offset`: optional chromatic semitone shift

Example: `echo "04C" | nc -u localhost 49161` plays C4 on channel 0.

## Effect System

Effects implement the `Effect` trait (`src/audio/dsp/mod.rs`):
```rust
trait Effect: Send {
    fn process(&mut self, input: f32) -> f32;
    fn set_parameter(&mut self, param_name: &str, value: f32);
    fn get_parameters(&self) -> Vec<EffectParameter>;
    fn name(&self) -> &str;
}
```

New effects: implement the trait, add a variant to `EffectType` in `messages.rs`, and register it in `create_effect()` in `dsp/mod.rs`.

## State Snapshot

`SynthState` (read-only, published by audio thread) is what the TUI reads. `ConfigCommand` (sent by TUI) is how the TUI mutates audio state. These two types in `src/state/messages.rs` are the contract between threads — changes here ripple everywhere.

## TUI Modes

Three modes switched via number keys `1`/`2`/`3`:
- **Voices** (default): 4×4 voice grid, oscillator selector, ADSR params, per-voice send levels
- **FX Groups**: effect chain editor for groups A–D (add/remove/reorder effects, edit params)
- **Routing**: 16×4 send matrix (voice → group send levels)
