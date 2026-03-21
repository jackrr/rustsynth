use crate::state::messages::OscillatorType;

/// ADSR envelope parameters (carried in state snapshot so TUI can display/edit them)
#[derive(Debug, Clone)]
pub struct EnvelopeParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvelopeParams {
    fn default() -> Self {
        EnvelopeParams { attack: 0.01, decay: 0.2, sustain: 0.7, release: 0.3 }
    }
}

/// Read-only snapshot of audio engine state, published to TUI
#[derive(Debug, Clone)]
pub struct VoiceState {
    pub active: bool,
    pub midi_note: u8,
    pub amplitude: f32,
    pub osc_type: OscillatorType,
    pub envelope: EnvelopeParams,
    pub default_midi_note: u8,
    pub default_velocity: f32,
}

#[derive(Debug, Clone)]
pub struct EffectParamState {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub labels: Option<&'static [&'static str]>,
}

#[derive(Debug, Clone)]
pub struct EffectState {
    pub name: String,
    pub params: Vec<EffectParamState>,
}

#[derive(Debug, Clone)]
pub struct GroupState {
    pub enabled: bool,
    pub effects: Vec<EffectState>,
}

#[derive(Debug, Clone)]
pub struct SynthState {
    pub voices: [VoiceState; 16],
    pub groups: [GroupState; 4],
    pub routing: [[f32; 4]; 16],
    /// Recent output samples for oscilloscope display (oldest → newest)
    pub scope: Vec<f32>,
}

impl Default for VoiceState {
    fn default() -> Self {
        VoiceState {
            active: false,
            midi_note: 60,
            amplitude: 0.0,
            osc_type: OscillatorType::Sine,
            envelope: EnvelopeParams::default(),
            default_midi_note: 60,
            default_velocity: 0.75,
        }
    }
}

impl Default for GroupState {
    fn default() -> Self {
        GroupState { enabled: true, effects: Vec::new() }
    }
}

impl Default for SynthState {
    fn default() -> Self {
        SynthState {
            voices: std::array::from_fn(|_| VoiceState::default()),
            groups: std::array::from_fn(|_| GroupState::default()),
            routing: [[0.0; 4]; 16],
            scope: vec![0.0; 4096],
        }
    }
}
