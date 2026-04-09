/// A note-on command from UDP
#[derive(Debug, Clone)]
pub struct NoteCommand {
    pub channel: usize,    // 0-15
    pub midi_note: u8,     // MIDI note number
    pub velocity: f32,     // 0.0-1.0
    pub length_samples: u64, // Duration in samples before auto-release
    pub detune_cents: f32, // -100.0 to +100.0 cents (0 = no detune)
}

/// Configuration changes sent from TUI to audio thread
#[derive(Debug, Clone)]
pub enum ConfigCommand {
    ClearGroup { group: usize },
    SetOscillator { voice: usize, osc_type: OscillatorType },
    SetDefaultNote { voice: usize, midi_note: u8 },
    SetDefaultVelocity { voice: usize, velocity: f32 },
    SetSubOsc { voice: usize, enabled: bool, octave: i32, level: f32 },
    MuteVoice { voice: usize, muted: bool },
    SoloVoice { voice: usize, soloed: bool },
    SetEnvelope { voice: usize, attack: f32, decay: f32, sustain: f32, release: f32 },
    SetSendLevel { voice: usize, group: usize, level: f32 },
    AddEffect { group: usize, effect_type: EffectType, position: usize },
    RemoveEffect { group: usize, position: usize },
    SetEffectParam { group: usize, effect_idx: usize, param: String, value: f32 },
    EnableGroup { group: usize, enabled: bool },
    SeqPlay,
    SeqStop,
    SeqTogglePlay,
    SeqSetBpm { bpm: f32 },
    SeqSetStepCount { count: usize },
    SeqSetSwing { swing: f32 },
    SeqSetStep { voice: usize, step: usize, enabled: bool, midi_note: u8, velocity: f32 },
    SeqClearRow { voice: usize },
    SeqCopyRow { src_voice: usize, dst_voice: usize },
    SeqClearAll,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OscillatorType {
    Sine,
    Triangle,
    Square,
    Sawtooth,
    Sine2,
    Sine3,
    Sine4,
    Triangle2,
    Triangle3,
    Triangle4,
    Square2,
    Square3,
    Square4,
    Sawtooth2,
    Sawtooth3,
    Sawtooth4,
}

impl OscillatorType {
    pub fn name(&self) -> &str {
        match self {
            OscillatorType::Sine => "Sine",
            OscillatorType::Triangle => "Triangle",
            OscillatorType::Square => "Square",
            OscillatorType::Sawtooth => "Sawtooth",
            OscillatorType::Sine2 => "Sine2",
            OscillatorType::Sine3 => "Sine3",
            OscillatorType::Sine4 => "Sine4",
            OscillatorType::Triangle2 => "Triangle2",
            OscillatorType::Triangle3 => "Triangle3",
            OscillatorType::Triangle4 => "Triangle4",
            OscillatorType::Square2 => "Square2",
            OscillatorType::Square3 => "Square3",
            OscillatorType::Square4 => "Square4",
            OscillatorType::Sawtooth2 => "Sawtooth2",
            OscillatorType::Sawtooth3 => "Sawtooth3",
            OscillatorType::Sawtooth4 => "Sawtooth4",
        }
    }

    pub fn all() -> &'static [OscillatorType] {
        &[
            OscillatorType::Sine,
            OscillatorType::Triangle,
            OscillatorType::Square,
            OscillatorType::Sawtooth,
            OscillatorType::Sine2,
            OscillatorType::Sine3,
            OscillatorType::Sine4,
            OscillatorType::Triangle2,
            OscillatorType::Triangle3,
            OscillatorType::Triangle4,
            OscillatorType::Square2,
            OscillatorType::Square3,
            OscillatorType::Square4,
            OscillatorType::Sawtooth2,
            OscillatorType::Sawtooth3,
            OscillatorType::Sawtooth4,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EffectType {
    Gain,
    Bitcrusher,
    Distortion,
    Limiter,
    Delay,
    Reverb,
    Tremolo,
    Chorus,
    Phaser,
    Vibrato,
    Lowpass,
    Highpass,
    Bandpass,
    Eq3,
    Compressor,
    WhiteNoise,
}

impl EffectType {
    pub fn name(&self) -> &str {
        match self {
            EffectType::Gain => "Gain",
            EffectType::Bitcrusher => "Bitcrusher",
            EffectType::Distortion => "Distortion",
            EffectType::Limiter => "Limiter",
            EffectType::Delay => "Delay",
            EffectType::Reverb => "Reverb",
            EffectType::Tremolo => "Tremolo",
            EffectType::Chorus => "Chorus",
            EffectType::Phaser => "Phaser",
            EffectType::Vibrato => "Vibrato",
            EffectType::Lowpass => "Lowpass",
            EffectType::Highpass => "Highpass",
            EffectType::Bandpass => "Bandpass",
            EffectType::Eq3 => "EQ3",
            EffectType::Compressor => "Compressor",
            EffectType::WhiteNoise => "WhiteNoise",
        }
    }

}
