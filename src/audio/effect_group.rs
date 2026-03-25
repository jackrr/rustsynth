use crate::audio::dsp::{Effect, create_effect};
use crate::state::messages::EffectType;

pub struct EffectGroup {
    pub effects: Vec<(EffectType, Box<dyn Effect>)>,
    pub enabled: bool,
    sample_rate: f32,
}

impl EffectGroup {
    pub fn new(_name: &str, sample_rate: f32) -> Self {
        EffectGroup {
            effects: Vec::new(),
            enabled: true,
            sample_rate,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let mut sample = input;
        for (_, effect) in &mut self.effects {
            sample = effect.process(sample);
        }
        sample
    }

    pub fn add_effect(&mut self, effect_type: EffectType, position: usize) {
        let effect = create_effect(effect_type, self.sample_rate);
        let pos = position.min(self.effects.len());
        self.effects.insert(pos, (effect_type, effect));
    }

    pub fn remove_effect(&mut self, position: usize) {
        if position < self.effects.len() {
            self.effects.remove(position);
        }
    }

    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn set_effect_param(&mut self, effect_idx: usize, param: &str, value: f32) {
        if let Some((_, effect)) = self.effects.get_mut(effect_idx) {
            effect.set_parameter(param, value);
        }
    }

}
