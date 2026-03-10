# Pilot Rust TUI Synthesizer - Implementation Plan

## Context

Rewriting [Pilot](https://github.com/hundredrabbits/Pilot) as a Rust terminal UI application to modernize the synthesizer while preserving compatibility with [Orca](https://github.com/hundredrabbits/Orca) livecoding environment.

**Why this rewrite:**
- Replace Electron desktop app with lightweight Rust TUI
- Maintain full UDP communication protocol for Orca integration
- Achieve better performance and lower resource usage
- Use native audio instead of Web Audio API

**Design improvements over original Pilot:**
- 16 polyphonic voices (channels 0-F) ✓ preserved
- UDP server on port 49161 - **simplified to note triggering only**
- Command format: `04C` (play C4 on channel 0) - minimal UDP protocol
- 16 oscillator types (sine, triangle, square, sawtooth + harmonic variants) ✓ preserved
- ADSR envelope per voice ✓ preserved
- **4 effect groups** with variable/unlimited effects per group (vs 8 global effects)
- **Flexible routing:** Each voice has send levels (0.0-1.0) to each of 4 groups
- **TUI-based configuration:** All settings (oscillators, envelopes, effects, routing) via terminal UI
- FM synthesis capabilities ✓ preserved

## Technology Stack

**Core Dependencies:**
- **TUI:** `ratatui` (0.28) + `crossterm` (0.28) - Best Rust TUI library for real-time updates (60+ FPS)
- **Audio:** `cpal` (0.15) - Cross-platform audio I/O
- **Networking:** `tokio` (1.40) - Async UDP server
- **Concurrency:** `crossbeam-channel` (0.5) for lock-free command queue, `arc-swap` (1.7) for state snapshots
- **DSP:** Consider `fundsp` (0.18) for complex effects, or implement from scratch using `biquad` (0.4) for filters
- **Parsing:** `nom` (7.1) - Parser combinators for UDP commands

## Architecture

### Threading Model
```
Main Thread (TUI)          Tokio Thread (UDP)       Audio Thread (Real-time)
    |                            |                          |
    | Arc<ArcSwap<State>>        |                          |
    | (bidirectional)            | crossbeam channel        |
    |<---------------------------+--------------------------+
    | 60 FPS UI updates          | Parse note commands      | Generate audio samples
    | Ratatui rendering          | (04C format only)        | Process note commands
    | Keyboard input             | Send to audio            | Update 16 voices
    | Send config changes        |                          | Route voices to 4 FX groups
    | (oscillators, envelopes)   |                          | Apply FX chains per group
    |                            |                          | Mix group outputs
    | crossbeam channel          |                          | Publish state (20 FPS)
    +-------------------------------------------------------->
                                                   Read config changes
```

**Key Design Principles:**
- Audio thread NEVER blocks (no mutexes in audio callback)
- Lock-free communication via `crossbeam-channel`:
  - UDP → Audio: Note triggering commands only (play/stop)
  - TUI → Audio: Configuration changes (oscillator type, envelope params, routing, effects)
- `arc-swap` for state snapshots (Audio → TUI)
- Audio thread is single writer, TUI reads state and sends config updates
- Bounded channels prevent memory growth

### Project Structure
```
rustsynth/
├── src/
│   ├── main.rs                    # Entry point, TUI event loop
│   ├── udp/
│   │   ├── server.rs              # Tokio UDP server (port 49161)
│   │   └── parser.rs              # Simple note command parser
│   ├── audio/
│   │   ├── engine.rs              # Audio callback, routing, mixing
│   │   ├── voice.rs               # Voice = Oscillator + Envelope + Sends
│   │   ├── oscillator.rs          # 16 waveform types
│   │   ├── envelope.rs            # ADSR generator
│   │   ├── effect_group.rs        # Effect group (chain of effects)
│   │   ├── routing.rs             # Send levels (voice → groups)
│   │   └── dsp/                   # Individual effect implementations
│   │       ├── mod.rs             # Effect trait + registry
│   │       ├── bitcrusher.rs
│   │       ├── distortion.rs
│   │       ├── delay.rs
│   │       ├── reverb.rs
│   │       ├── chorus.rs
│   │       ├── phaser.rs
│   │       ├── tremolo.rs
│   │       ├── compressor.rs
│   │       ├── eq.rs
│   │       └── ... (more effects)
│   ├── state/
│   │   ├── synth_state.rs         # Global state snapshot (read-only)
│   │   ├── synth_config.rs        # Mutable config (TUI → Audio)
│   │   └── messages.rs            # Command enums
│   └── ui/
│       ├── app.rs                 # TUI main loop + input handling
│       ├── mode.rs                # UI modes (Voice, FX, Routing)
│       └── widgets/
│           ├── voice_panel.rs     # Voice grid + oscillator/envelope editor
│           ├── fx_group_panel.rs  # Effect group editor (add/remove/reorder)
│           ├── routing_panel.rs   # Send level matrix (16 voices × 4 groups)
│           ├── fx_param_panel.rs  # Effect parameter controls
│           └── spectrum.rs        # Spectrum analyzer (optional)
```

## Core Components

### 1. UDP Command Parser (src/udp/parser.rs)

**Simplified protocol - note triggering only:**

Parse ASCII/hex commands for note playback:
- **Play commands:** `[channel][octave][note][velocity?][length?][note_offset?]`
  - `04C` = Channel 0, C4, default velocity (64), default length (1/16 bar)
  - `04Cff` = Channel 0, C4, max velocity (127), full bar length
  - `04C8` = Channel 0, C4, velocity 128 (hex 8 = 8/15 * 127 ≈ 67)
  - `04C81` = Channel 0, C4, velocity 128 (hex 8 = 8/15 * 127 ≈ 67), actually C#4 (+1 half step)
  - `0498` = Channel 0, G3, velocity 128 (hex 8 = 8/15 * 127 ≈ 67)  - Channel: 0-Fa (0-15)
  - Octave: 0-9
  - Note: C, C#, D, D#, E, F, F#, G, G#, A, A#, B (or lowercase) -- Jack's note: take full advantage of alphanum values (a-g are in current octave, 0-9 are lower, h-z are higher)
  - Velocity: optional hex digit (0-Z → 0-127), default 64
  - Length: optional hex digit (0-Z → note duration), default 1/16 bar
  - Note offset: specifies how many chromatic steps to shift by from the specified note
  - messages are case-insensitive

**All other configuration via TUI:**
- Oscillator types, envelope parameters, effect chains, routing matrix, etc.
- This keeps UDP protocol minimal and Orca integration simple

### 2. Voice Management (src/audio/voice.rs)

```rust
pub struct Voice {
    oscillator: Oscillator,      // Waveform generator
    envelope: EnvelopeGenerator, // ADSR
    active: bool,
    velocity: f32,
}
```

Each of 16 voices operates independently:
- Frequency from MIDI note formula: `440 * 2^((note - 69) / 12)`
- Envelope shapes amplitude over time (Attack → Decay → Sustain → Release)
- Output: `oscillator_sample * envelope_level * velocity`

### 3. Oscillator Types (src/audio/oscillator.rs)

16 types to implement:
- **Basic:** sine, triangle, square, sawtooth
- **Harmonic variants:** sine2/3/4, triangle2/3/4, square2/3/4, sawtooth2/3/4
  - Use additive synthesis (sum multiple harmonics) for variants

Phase accumulator approach:
```rust
phase += 2π * frequency / sample_rate
sample = waveform_function(phase)
```

### 4. ADSR Envelope (src/audio/envelope.rs)

State machine: Idle → Attack → Decay → Sustain → Release → Idle

Each stage uses linear interpolation:
- **Attack:** Ramp 0.0 → 1.0 over attack_time
- **Decay:** Ramp 1.0 → sustain_level over decay_time
- **Sustain:** Hold at sustain_level
- **Release:** Ramp current_level → 0.0 over release_time

### 5. Effect Groups & Routing (src/audio/effect_group.rs)

**Architecture:**
- **4 effect groups** (A, B, C, D) - each is an independent effects chain
- **Variable/unlimited effects per group** - add/remove/reorder via TUI
- **Send-based routing** - each voice has send level (0.0-1.0) to each group
- **Final mix** - sum all group outputs

**Effect Group:**
```rust
struct EffectGroup {
    name: String,              // "Reverb", "Distortion", etc.
    effects: Vec<Box<dyn Effect>>, // Variable-length chain
    enabled: bool,
}

impl EffectGroup {
    fn process(&mut self, input: f32) -> f32 {
        if !self.enabled { return 0.0; }
        let mut sample = input;
        for effect in &mut self.effects {
            sample = effect.process(sample);
        }
        sample
    }
}
```

**Routing (src/audio/routing.rs):**
```rust
struct VoiceSends {
    levels: [f32; 4],  // Send to groups A, B, C, D
}

// Example: Voice 0 sends 50% to group A, 30% to group B
// voice_0.sends.levels = [0.5, 0.3, 0.0, 0.0]
```

**Available Effects:**

*Simple effects (implement first):*
1. **Gain** - Volume control
2. **Bitcrusher** - Reduce bit depth + sample rate
3. **Distortion** - Waveshaping/clipping
4. **Limiter** - Peak limiting

*Time-based effects:*
5. **Delay** - Ring buffer + feedback + time control
6. **Reverb** - Schroeder reverb (4 comb + 2 allpass)

*Modulation effects:*
7. **Tremolo** - Amplitude LFO
8. **Chorus** - Delayed + pitch LFO
9. **Phaser** - Allpass LFO
10. **Vibrato** - Pitch LFO

*Filters & Dynamics:*
11. **Lowpass Filter** - Biquad lowpass
12. **Highpass Filter** - Biquad highpass
13. **Bandpass Filter** - Biquad bandpass
14. **EQ3** - 3-band parametric EQ
15. **Compressor** - Dynamic range

Effect trait:
```rust
trait Effect: Send {
    fn process(&mut self, input: f32) -> f32;
    fn set_parameter(&mut self, param_name: &str, value: f32);
    fn get_parameters(&self) -> Vec<EffectParameter>;
    fn name(&self) -> &str;
}

struct EffectParameter {
    name: String,
    value: f32,
    min: f32,
    max: f32,
}
```

### 6. Audio Engine (src/audio/engine.rs)

Main audio callback with routing and effect groups (runs at ~48kHz):
```rust
fn process_frame(&mut self, output: &mut [f32]) {
    // 1. Process commands (non-blocking)
    while let Ok(cmd) = self.note_rx.try_recv() {
        self.handle_note_command(cmd);  // Play/stop notes
    }
    while let Ok(config) = self.config_rx.try_recv() {
        self.apply_config_change(config);  // Osc/env/routing changes
    }

    // 2. Generate samples
    for sample in output {
        // Initialize group inputs
        let mut group_inputs = [0.0_f32; 4];

        // Process voices and route to groups
        for (voice_idx, voice) in self.voices.iter_mut().enumerate() {
            let voice_sample = voice.process();  // oscillator * envelope
            let sends = &self.routing[voice_idx];

            // Send to each group based on send levels
            for (group_idx, send_level) in sends.levels.iter().enumerate() {
                group_inputs[group_idx] += voice_sample * send_level;
            }
        }

        // Process effect groups and sum outputs
        let mut final_mix = 0.0;
        for (group, input) in self.effect_groups.iter_mut().zip(&group_inputs) {
            final_mix += group.process(*input);
        }

        *sample = final_mix * 0.25;  // Master volume with headroom
    }

    // 3. Publish state snapshot (every ~2400 samples = 20 FPS at 48kHz)
    if self.frame_count % 2400 == 0 {
        self.publish_state_snapshot();
    }
}
```

**Configuration Changes from TUI:**
```rust
enum ConfigCommand {
    SetOscillator { voice: usize, osc_type: OscillatorType },
    SetEnvelope { voice: usize, attack: f32, decay: f32, sustain: f32, release: f32 },
    SetSendLevel { voice: usize, group: usize, level: f32 },
    AddEffect { group: usize, effect_type: EffectType, position: usize },
    RemoveEffect { group: usize, position: usize },
    SetEffectParam { group: usize, effect_idx: usize, param: String, value: f32 },
    EnableGroup { group: usize, enabled: bool },
}
```

### 7. TUI Layout & Modes (src/ui/app.rs)

**Multiple UI modes accessible via tabs/keys:**

**Mode 1: VOICES (default, key: 1)**
```
┌─────────────────────────────────────────────────────────────────────┐
│ PILOT Rust Synth    [1:VOICES] 2:FX GROUPS  3:ROUTING   UDP:49161  │
├─────────────────────────────────────────────────────────────────────┤
│ Voices (4x4 grid):                                                  │
│ ┌────┬────┬────┬────┐ ┌────┬────┬────┬────┐                        │
│ │ 0  │ 1  │ 2  │ 3  │ │ 8  │ 9  │ A  │ B  │                        │
│ │ C4 │ -- │ E4 │ -- │ │ -- │ G5 │ -- │ -- │                        │
│ │Sine│    │ Sq │    │ │    │Tri │    │    │                        │
│ │███ │    │████│    │ │    │██  │    │    │ (amplitude bars)        │
│ ├────┼────┼────┼────┤ ├────┼────┼────┼────┤                        │
│ │ 4  │ 5  │ 6  │ 7  │ │ C  │ D  │ E  │ F  │                        │
│ └────┴────┴────┴────┘ └────┴────┴────┴────┘                        │
├─────────────────────────────────────────────────────────────────────┤
│ Selected: Voice 0                                                   │
│ ┌─ Oscillator ────────┐ ┌─ Envelope (ADSR) ──────────────────────┐ │
│ │ Type: [Sine    ▼]   │ │    1.0┤     ╱╲___                       │ │
│ │ Detune: 0.00 cents  │ │       │    ╱  │  ╲                      │ │
│ │                     │ │    0.5┤   ╱   │   ╲___                  │ │
│ └─────────────────────┘ │       │  ╱    │       ╲                 │ │
│ ┌─ Sends ──────────────┐ │    0.0└──┴────┴────────┴───            │ │
│ │ A: [████░░] 40%     │ │         A    D    S     R               │ │
│ │ B: [██░░░░] 20%     │ │ A:0.01s D:0.20s S:0.70 R:0.30s          │ │
│ │ C: [░░░░░░]  0%     │ └─────────────────────────────────────────┘ │
│ │ D: [░░░░░░]  0%     │                                             │
│ └─────────────────────┘                                             │
├─────────────────────────────────────────────────────────────────────┤
│ ↑↓:Select Voice  ←→:Edit Param  Tab:Next Mode  q:Quit              │
└─────────────────────────────────────────────────────────────────────┘
```

**Mode 2: FX GROUPS (key: 2)**
```
┌─────────────────────────────────────────────────────────────────────┐
│ PILOT Rust Synth     1:VOICES [2:FX GROUPS] 3:ROUTING  UDP:49161   │
├─────────────────────────────────────────────────────────────────────┤
│ Effect Groups:                                                      │
│                                                                     │
│ ┌─ Group A: Reverb Room ──────────────────────────┐ [Enabled ✓]   │
│ │ 1. Lowpass Filter    (Cutoff: 8000Hz, Res: 0.3) │                │
│ │ 2. Reverb           (Size: 0.7, Damping: 0.5)   │                │
│ │ 3. Gain             (Level: -6dB)                │                │
│ └──────────────────────────────────────────────────┘                │
│                                                                     │
│ ┌─ Group B: Distortion ───────────────────────────┐ [Enabled ✓]   │
│ │ 1. Distortion       (Drive: 0.8, Type: Hard)    │ ◄ Selected     │
│ │ 2. Highpass Filter  (Cutoff: 100Hz)             │                │
│ └──────────────────────────────────────────────────┘                │
│                                                                     │
│ ┌─ Group C: Unused ───────────────────────────────┐ [Disabled]     │
│ │ (empty)                                          │                │
│ └──────────────────────────────────────────────────┘                │
│                                                                     │
│ ┌─ Group D: Delay ────────────────────────────────┐ [Enabled ✓]   │
│ │ 1. Delay            (Time: 250ms, Feedback: 0.4)│                │
│ └──────────────────────────────────────────────────┘                │
├─────────────────────────────────────────────────────────────────────┤
│ Selected: Group B → Effect 1 (Distortion)                          │
│ Drive:  [████████░░░░░░] 0.80  ←→: Adjust                          │
│ Type:   [Hard Clip    ▼]       ↑↓: Param/Effect   a: Add Effect   │
│                                 d: Delete Effect   e: Enable Group  │
├─────────────────────────────────────────────────────────────────────┤
│ ↑↓:Select  ←→:Adjust  a:Add Effect  d:Delete  e:Toggle  Tab:Mode   │
└─────────────────────────────────────────────────────────────────────┘
```

**Mode 3: ROUTING (key: 3)**
```
┌─────────────────────────────────────────────────────────────────────┐
│ PILOT Rust Synth     1:VOICES  2:FX GROUPS [3:ROUTING] UDP:49161   │
├─────────────────────────────────────────────────────────────────────┤
│ Send Matrix (Voice → FX Group):                                    │
│                                                                     │
│        │  Group A  │  Group B  │  Group C  │  Group D  │           │
│        │  Reverb   │  Distort  │  Unused   │  Delay    │           │
│ ───────┼───────────┼───────────┼───────────┼───────────┤           │
│ Voice 0│ [███░] 40%│ [██░░] 20%│ [░░░░]  0%│ [░░░░]  0%│           │
│ Voice 1│ [░░░░]  0%│ [░░░░]  0%│ [░░░░]  0%│ [░░░░]  0%│           │
│ Voice 2│ [████] 50%│ [░░░░]  0%│ [░░░░]  0%│ [██░░] 25%│           │
│ Voice 3│ [░░░░]  0%│ [████]100%│ [░░░░]  0%│ [░░░░]  0%│           │
│ Voice 4│ [░░░░]  0%│ [░░░░]  0%│ [░░░░]  0%│ [░░░░]  0%│           │
│ Voice 5│ [██░░] 30%│ [░░░░]  0%│ [░░░░]  0%│ [███░] 40%│           │
│  ...   │    ...    │    ...    │    ...    │    ...    │           │
│ Voice F│ [░░░░]  0%│ [░░░░]  0%│ [░░░░]  0%│ [░░░░]  0%│           │
│                                                                     │
│ Selected: Voice 0 → Group A (40%)                                  │
│ [████░░░░░░] 40%  ←→: Adjust  c: Copy Row  p: Paste  z: Zero All   │
├─────────────────────────────────────────────────────────────────────┤
│ ↑↓←→:Navigate  ←→:Adjust  c:Copy Row  p:Paste  z:Zero  Tab:Mode    │
└─────────────────────────────────────────────────────────────────────┘
```

**UI Features:**
- Ratatui provides 60+ FPS rendering with smooth updates
- Constraint-based responsive layouts adapt to terminal size
- Keyboard-driven navigation (no mouse required)
- Real-time visualization of active voices and effect processing
- Interactive parameter editing with immediate audio feedback

## Implementation Phases

### Phase 1: UDP + Basic Audio (Days 1-2)
- Set up project structure and dependencies
- Implement UDP server (tokio) on port 49161
- Implement command parser for play commands (`04C`)
- Create basic oscillator (sine wave only)
- Set up CPAL audio output with single voice
- **Verify:** `echo "04C" | nc -u localhost 49161` plays C4 tone

### Phase 2: Polyphony + Envelopes (Days 3-4)
- Implement ADSR envelope generator
- Create Voice struct (oscillator + envelope)
- Implement 16-voice management
- Add note-on/note-off handling
- **Verify:** Play polyphonic chords via UDP

### Phase 3: Oscillator Variety (Day 5)
- Implement basic waveforms (sine, triangle, square, sawtooth)
- Implement harmonic variants using additive synthesis
- Add OscillatorType enum with 16 variants
- **Verify:** Manually test each oscillator type sounds different

### Phase 4: Effect Groups & Routing (Days 6-7)
- Implement Effect trait with parameter system
- Create EffectGroup struct (Vec of effects)
- Implement routing system (16 voices × 4 groups send matrix)
- Implement audio engine with routing and group mixing
- Add ConfigCommand enum for TUI → Audio communication
- **Verify:** Manually set routing, verify groups receive correct signals

### Phase 5: Basic Effects (Days 8-9)
- Implement simple effects: Gain, Bitcrusher, Distortion, Limiter
- Add effects to groups, verify audio processing
- **Verify:** Each effect produces expected sound modification

### Phase 6: Advanced Effects (Days 10-12)
- Implement time-based: Delay, Reverb
- Implement modulation: Tremolo, Chorus, Phaser
- Implement filters: Lowpass, Highpass, Bandpass, EQ3
- Implement dynamics: Compressor
- **Verify:** Build complex effect chains, verify sound quality

### Phase 7: State Management (Day 13)
- Design SynthState (read-only snapshot) and SynthConfig (mutable config)
- Implement ArcSwap for lock-free state sharing (Audio → TUI)
- Implement crossbeam channels for config updates (TUI → Audio)
- Implement periodic state publishing (20 FPS)
- **Verify:** State updates don't cause audio glitches

### Phase 8: Basic TUI - Voices Mode (Days 14-15)
- Set up Ratatui + Crossterm with event loop
- Implement Mode 1: VOICES (grid, oscillator selector, envelope params, sends)
- Implement ADSR visualization (line chart)
- Implement keyboard navigation
- Connect TUI to state: read from ArcSwap, send ConfigCommand
- **Verify:** TUI shows real-time voice activity, can edit parameters

### Phase 9: FX Groups TUI Mode (Days 16-17)
- Implement Mode 2: FX GROUPS (list groups, show effect chains)
- Implement effect parameter editor
- Implement add/delete/reorder effects in group
- Implement effect browser/selector popup
- Connect to ConfigCommand for FX modifications
- **Verify:** Can build complex effect chains via TUI, hear results

### Phase 10: Routing Matrix TUI Mode (Day 18)
- Implement Mode 3: ROUTING (16×4 send matrix visualization)
- Implement interactive send level adjustment
- Implement copy/paste routing configurations
- **Verify:** Can route voices to groups, verify mixing

### Phase 11: Polish & UX (Days 19-20)
- Add color theming and visual polish
- Add help screen (key bindings)
- Add preset save/load system (JSON files)
- Add keyboard shortcuts for common operations
- Add status bar with CPU usage, active voices, peak meter
- **Verify:** Smooth, intuitive workflow

### Phase 12: Testing & Optimization (Days 21-22)
- Write unit tests (oscillators, envelopes, effects, parser)
- Write integration tests (UDP → Audio pipeline)
- Profile audio thread (cargo flamegraph)
- Optimize hot paths (reduce allocations)
- Test with Orca livecoding sessions
- Stress test (all 16 voices + complex FX chains)
- **Verify:** <25% CPU usage, no audio dropouts, works with Orca

### Phase 13: Documentation & Release (Day 23)
- Write comprehensive README with examples
- Document UDP protocol
- Document TUI key bindings
- Add example presets
- Tag v0.1.0 release

## Critical Files to Create (in order)

1. **src/state/messages.rs** - Define `NoteCommand` and `ConfigCommand` enums (contracts between components)
2. **src/udp/parser.rs** - Parse UDP note commands (simplified, notes only)
3. **src/audio/oscillator.rs** - Waveform generation (16 types)
4. **src/audio/envelope.rs** - ADSR envelope generator
5. **src/audio/voice.rs** - Combine oscillator + envelope + send levels
6. **src/audio/dsp/mod.rs** - Effect trait definition + parameter system
7. **src/audio/effect_group.rs** - EffectGroup (variable-length effect chain)
8. **src/audio/routing.rs** - VoiceSends (16 voices × 4 groups matrix)
9. **src/audio/engine.rs** - Audio callback with routing and group mixing
10. **src/audio/dsp/*.rs** - Individual effects (15+ files: gain, bitcrusher, distortion, delay, reverb, filters, etc.)
11. **src/udp/server.rs** - Tokio UDP server (note triggering only)
12. **src/state/synth_state.rs** - Read-only state snapshot for TUI
13. **src/state/synth_config.rs** - Mutable config structure
14. **src/ui/mode.rs** - UI mode enum (Voices, FxGroups, Routing)
15. **src/ui/app.rs** - TUI main loop, mode switching, keyboard input
16. **src/ui/widgets/voice_panel.rs** - Voice grid + oscillator/envelope editor
17. **src/ui/widgets/fx_group_panel.rs** - Effect group editor
18. **src/ui/widgets/routing_panel.rs** - Send matrix editor
19. **src/main.rs** - Tie everything together (spawn threads, run TUI)

## Technical Challenges & Solutions

**Challenge 1: Real-time audio constraints**
- Audio callback must complete in <10ms (512 samples @ 48kHz)
- Solution: Pre-allocate all buffers, use lock-free channels, no allocations in hot path
- Profile with `cargo flamegraph` to identify bottlenecks

**Challenge 2: Variable-length effect chains in audio thread**
- Problem: Vec<Box<dyn Effect>> involves heap allocations and dynamic dispatch
- Solution: Pre-allocate Vec capacity, accept dynamic dispatch overhead (minimal), consider `smallvec` if needed

**Challenge 3: Complex effects (reverb, delay)**
- Problem: High-quality reverb requires significant processing
- Solution: Start with Schroeder reverb (simple, sounds good), optimize later, consider `fundsp` for advanced effects

**Challenge 4: Thread-safe bidirectional communication**
- Problem: Audio thread needs to both receive config updates and publish state
- Solution:
  - TUI → Audio: `crossbeam-channel` for config commands (non-blocking `try_recv`)
  - Audio → TUI: `arc-swap` for state snapshots (lock-free reads)
  - Keep audio thread as priority, never blocks

**Challenge 5: TUI complexity with 3 modes**
- Problem: Managing UI state, navigation, and input across multiple modes
- Solution: Use mode enum, separate rendering functions per mode, state machine for navigation

## Verification & Testing

### Unit Tests
- Oscillator frequency accuracy (count zero-crossings)
- Envelope timing (attack reaches peak after specified time)
- Command parser (all syntax variants)

### Integration Tests
- UDP → Audio pipeline (send command, verify voice activated)
- State publishing (verify TUI receives updates)

### Orca Integration Test
1. Start rustsynth
2. Configure effects in TUI (Mode 2): Add reverb to Group A, distortion to Group B
3. Configure routing in TUI (Mode 3): Route voices 0-3 to Group A, voices 4-7 to Group B
4. Send Orca pattern: `04C;14E;24G;44C` (voices 0-3 with reverb, voice 4 with distortion)
5. Verify four simultaneous tones with correct effects
6. Test rapid command sequences (livecoding scenario)
7. Change routing live, verify immediate effect

### Performance Benchmarks
- Audio callback latency: Target <1ms for 512 samples (48kHz = 10.6ms available)
- CPU usage: <25% with 16 voices + 4 complex effect groups
- TUI rendering: 60 FPS without frame drops
- UDP latency: <10ms from packet receive to audio output
- Config update latency: <20ms from TUI input to audio parameter change

### Manual Test Checklist
- [ ] All 16 voices work independently
- [ ] All 16 oscillator types sound distinct
- [ ] ADSR envelopes shape sound correctly (visualized and audible)
- [ ] All effects produce audible changes
- [ ] Can add/remove/reorder effects in groups
- [ ] Routing matrix correctly routes voices to groups
- [ ] Multiple sends work (voice to multiple groups)
- [ ] Polyphony works (play chords via UDP)
- [ ] TUI updates in real-time (voice activity, parameter changes)
- [ ] No audio glitches when changing config
- [ ] All 3 UI modes work smoothly
- [ ] Works with Orca livecoding sessions
- [ ] Preset save/load works

## Success Criteria

✅ **Orca compatibility:** UDP note triggering works seamlessly with Orca livecoding
✅ **Performance:** <25% CPU with 16 voices + 4 complex effect groups active
✅ **Latency:** <20ms from UDP note command to audio output
✅ **Stability:** No crashes or audio glitches in 1-hour Orca session
✅ **TUI responsiveness:** 60 FPS updates with smooth animations
✅ **Audio quality:** Clean output, no aliasing, professional-grade synthesis
✅ **Flexibility:** More powerful than original Pilot:
  - Variable effect chains per group (vs fixed 8 global)
  - Flexible routing (vs all voices to same effects)
  - Real-time visual feedback in TUI
✅ **Usability:** Intuitive TUI interface for all configuration (oscillators, envelopes, effects, routing)
✅ **Preset system:** Can save/load complete synth configurations
