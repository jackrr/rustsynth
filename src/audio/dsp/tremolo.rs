use std::f32::consts::PI;
use super::{Effect, EffectParameter};

pub struct Tremolo {
    rate: f32,    // Hz
    depth: f32,   // 0.0-1.0
    phase: f32,
    sample_rate: f32,
}

impl Tremolo {
    pub fn new(sample_rate: f32) -> Self {
        Tremolo {
            rate: 5.0,
            depth: 0.5,
            phase: 0.0,
            sample_rate,
        }
    }
}

impl Effect for Tremolo {
    fn process(&mut self, input: f32) -> f32 {
        let lfo = 1.0 - self.depth * (1.0 + self.phase.sin()) * 0.5;
        self.phase += 2.0 * PI * self.rate / self.sample_rate;
        if self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        input * lfo
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "rate" => self.rate = value.clamp(0.1, 20.0),
            "depth" => self.depth = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("rate", self.rate, 0.1, 20.0),
            EffectParameter::new("depth", self.depth, 0.0, 1.0),
        ]
    }

    fn name(&self) -> &str {
        "Tremolo"
    }
}
