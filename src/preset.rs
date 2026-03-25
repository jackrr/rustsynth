use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::state::messages::{ConfigCommand, EffectType, OscillatorType};
use crate::state::synth_state::SynthState;

#[derive(Serialize, Deserialize)]
pub struct VoicePreset {
    osc_type: OscillatorType,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    default_midi_note: u8,
    default_velocity: f32,
}

#[derive(Serialize, Deserialize)]
pub struct EffectPreset {
    effect_type: EffectType,
    params: Vec<(String, f32)>,
}

#[derive(Serialize, Deserialize)]
pub struct GroupPreset {
    enabled: bool,
    effects: Vec<EffectPreset>,
}

#[derive(Serialize, Deserialize)]
pub struct Preset {
    voices: Vec<VoicePreset>,
    groups: Vec<GroupPreset>,
    routing: Vec<[f32; 4]>,
}

impl Preset {
    pub fn from_state(state: &SynthState) -> Self {
        let voices = state.voices.iter().map(|v| VoicePreset {
            osc_type: v.osc_type,
            attack: v.envelope.attack,
            decay: v.envelope.decay,
            sustain: v.envelope.sustain,
            release: v.envelope.release,
            default_midi_note: v.default_midi_note,
            default_velocity: v.default_velocity,
        }).collect();

        let groups = state.groups.iter().map(|g| GroupPreset {
            enabled: g.enabled,
            effects: g.effects.iter().map(|e| EffectPreset {
                effect_type: e.effect_type,
                params: e.params.iter().map(|p| (p.name.clone(), p.value)).collect(),
            }).collect(),
        }).collect();

        let routing = state.routing.to_vec();

        Preset { voices, groups, routing }
    }

    pub fn to_commands(&self) -> Vec<ConfigCommand> {
        let mut cmds = Vec::new();

        for (i, v) in self.voices.iter().enumerate() {
            cmds.push(ConfigCommand::SetOscillator { voice: i, osc_type: v.osc_type });
            cmds.push(ConfigCommand::SetDefaultNote { voice: i, midi_note: v.default_midi_note });
            cmds.push(ConfigCommand::SetDefaultVelocity { voice: i, velocity: v.default_velocity });
            cmds.push(ConfigCommand::SetEnvelope {
                voice: i,
                attack: v.attack,
                decay: v.decay,
                sustain: v.sustain,
                release: v.release,
            });
        }

        for (g, group) in self.groups.iter().enumerate() {
            cmds.push(ConfigCommand::ClearGroup { group: g });
            cmds.push(ConfigCommand::EnableGroup { group: g, enabled: group.enabled });
            for (pos, effect) in group.effects.iter().enumerate() {
                cmds.push(ConfigCommand::AddEffect {
                    group: g,
                    effect_type: effect.effect_type,
                    position: pos,
                });
                for (param_name, value) in &effect.params {
                    cmds.push(ConfigCommand::SetEffectParam {
                        group: g,
                        effect_idx: pos,
                        param: param_name.clone(),
                        value: *value,
                    });
                }
            }
        }

        for (v, row) in self.routing.iter().enumerate() {
            for (g, &level) in row.iter().enumerate() {
                cmds.push(ConfigCommand::SetSendLevel { voice: v, group: g, level });
            }
        }

        cmds
    }
}

pub fn save(state: &SynthState, path: &Path) -> anyhow::Result<()> {
    let preset = Preset::from_state(state);
    let json = serde_json::to_string_pretty(&preset)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load(path: &Path) -> anyhow::Result<Vec<ConfigCommand>> {
    let json = std::fs::read_to_string(path)?;
    let preset: Preset = serde_json::from_str(&json)?;
    Ok(preset.to_commands())
}
