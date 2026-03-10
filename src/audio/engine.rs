use std::sync::Arc;
use arc_swap::ArcSwap;
use crossbeam_channel::Receiver;

use crate::audio::effect_group::EffectGroup;
use crate::audio::routing::RoutingMatrix;
use crate::audio::voice::Voice;
use crate::state::messages::{ConfigCommand, NoteCommand};
use crate::state::synth_state::{
    EffectParamState, EffectState, EnvelopeParams, GroupState, SynthState, VoiceState,
};

pub struct AudioEngine {
    voices: [Voice; 16],
    effect_groups: [EffectGroup; 4],
    routing: RoutingMatrix,
    note_rx: Receiver<NoteCommand>,
    config_rx: Receiver<ConfigCommand>,
    state_publisher: Arc<ArcSwap<SynthState>>,
    sample_rate: f32,
    frame_count: u64,
}

impl AudioEngine {
    pub fn new(
        sample_rate: f32,
        note_rx: Receiver<NoteCommand>,
        config_rx: Receiver<ConfigCommand>,
        state_publisher: Arc<ArcSwap<SynthState>>,
    ) -> Self {
        let voices = std::array::from_fn(|_| Voice::new(sample_rate));
        let effect_groups = [
            EffectGroup::new("A", sample_rate),
            EffectGroup::new("B", sample_rate),
            EffectGroup::new("C", sample_rate),
            EffectGroup::new("D", sample_rate),
        ];
        AudioEngine {
            voices,
            effect_groups,
            routing: RoutingMatrix::new(),
            note_rx,
            config_rx,
            state_publisher,
            sample_rate,
            frame_count: 0,
        }
    }

    fn handle_note_command(&mut self, cmd: NoteCommand) {
        let channel = cmd.channel.min(15);
        self.voices[channel].note_on(&cmd);
    }

    fn apply_config_change(&mut self, config: ConfigCommand) {
        match config {
            ConfigCommand::SetOscillator { voice, osc_type } => {
                if voice < 16 {
                    self.voices[voice].set_oscillator_type(osc_type);
                }
            }
            ConfigCommand::SetEnvelope { voice, attack, decay, sustain, release } => {
                if voice < 16 {
                    let env = &mut self.voices[voice].envelope;
                    env.attack_time = attack;
                    env.decay_time = decay;
                    env.sustain_level = sustain;
                    env.release_time = release;
                }
            }
            ConfigCommand::SetSendLevel { voice, group, level } => {
                self.routing.set(voice, group, level);
                if voice < 16 && group < 4 {
                    self.voices[voice].sends[group] = level;
                }
            }
            ConfigCommand::AddEffect { group, effect_type, position } => {
                if group < 4 {
                    self.effect_groups[group].add_effect(effect_type, position);
                }
            }
            ConfigCommand::RemoveEffect { group, position } => {
                if group < 4 {
                    self.effect_groups[group].remove_effect(position);
                }
            }
            ConfigCommand::SetEffectParam { group, effect_idx, param, value } => {
                if group < 4 {
                    self.effect_groups[group].set_effect_param(effect_idx, &param, value);
                }
            }
            ConfigCommand::EnableGroup { group, enabled } => {
                if group < 4 {
                    self.effect_groups[group].enabled = enabled;
                }
            }
        }
    }

    fn publish_state_snapshot(&self) {
        let voices: [VoiceState; 16] = std::array::from_fn(|i| {
            let env = &self.voices[i].envelope;
            VoiceState {
                active: self.voices[i].active,
                midi_note: self.voices[i].midi_note,
                velocity: self.voices[i].velocity,
                amplitude: self.voices[i].amplitude(),
                osc_type: self.voices[i].oscillator.osc_type,
                envelope: EnvelopeParams {
                    attack: env.attack_time,
                    decay: env.decay_time,
                    sustain: env.sustain_level,
                    release: env.release_time,
                },
            }
        });

        let groups: [GroupState; 4] = std::array::from_fn(|i| {
            let group = &self.effect_groups[i];
            let effects: Vec<EffectState> = group.effects.iter().map(|e| {
                EffectState {
                    name: e.name().to_string(),
                    params: e.get_parameters().into_iter().map(|p| EffectParamState {
                        name: p.name,
                        value: p.value,
                        min: p.min,
                        max: p.max,
                    }).collect(),
                }
            }).collect();
            GroupState {
                enabled: group.enabled,
                effects,
            }
        });

        let routing: [[f32; 4]; 16] = std::array::from_fn(|v| {
            std::array::from_fn(|g| self.routing.get(v, g))
        });

        let snapshot = Arc::new(SynthState {
            voices,
            groups,
            routing,
            sample_rate: self.sample_rate,
        });

        self.state_publisher.store(snapshot);
    }

    /// Called from the CPAL audio callback. Fills the output buffer.
    pub fn process_block(&mut self, output: &mut [f32]) {
        // Drain command queues (non-blocking)
        while let Ok(cmd) = self.note_rx.try_recv() {
            self.handle_note_command(cmd);
        }
        while let Ok(config) = self.config_rx.try_recv() {
            self.apply_config_change(config);
        }

        for sample in output.iter_mut() {
            // Initialize group inputs
            let mut group_inputs = [0.0_f32; 4];

            // Process each voice and route to groups
            for (voice_idx, voice) in self.voices.iter_mut().enumerate() {
                let voice_sample = voice.process();
                for (group_idx, &send_level) in voice.sends.iter().enumerate() {
                    group_inputs[group_idx] += voice_sample * send_level;
                }
                let _ = voice_idx; // suppress unused warning
            }

            // Process effect groups and sum outputs
            let mut final_mix = 0.0_f32;
            for (group, &input) in self.effect_groups.iter_mut().zip(&group_inputs) {
                final_mix += group.process(input);
            }

            *sample = (final_mix * 0.25).clamp(-1.0, 1.0);
            self.frame_count += 1;
        }

        // Publish state snapshot at ~20 FPS (every 2400 samples at 48kHz)
        if self.frame_count % 2400 < output.len() as u64 {
            self.publish_state_snapshot();
        }
    }
}
