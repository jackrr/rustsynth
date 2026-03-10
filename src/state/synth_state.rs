use crate::state::messages::OscillatorType;

/// Read-only snapshot of audio engine state, published to TUI
#[derive(Debug, Clone)]
pub struct VoiceState {
    pub active: bool,
    pub midi_note: u8,
    pub velocity: f32,
    pub amplitude: f32,        // Current envelope level
    pub osc_type: OscillatorType,
}

#[derive(Debug, Clone)]
pub struct EffectParamState {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
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
    pub routing: [[f32; 4]; 16],  // [voice][group] send levels
    pub sample_rate: f32,
}

impl Default for VoiceState {
    fn default() -> Self {
        VoiceState {
            active: false,
            midi_note: 60,
            velocity: 0.0,
            amplitude: 0.0,
            osc_type: OscillatorType::Sine,
        }
    }
}

impl Default for GroupState {
    fn default() -> Self {
        GroupState {
            enabled: true,
            effects: Vec::new(),
        }
    }
}

impl Default for SynthState {
    fn default() -> Self {
        SynthState {
            voices: std::array::from_fn(|_| VoiceState::default()),
            groups: std::array::from_fn(|_| GroupState::default()),
            routing: [[0.0; 4]; 16],
            sample_rate: 48000.0,
        }
    }
}
