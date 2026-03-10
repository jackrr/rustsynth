use super::{Effect, EffectParameter};

pub struct Limiter {
    threshold: f32,  // 0.0-1.0
    envelope: f32,
    attack: f32,     // attack coefficient
    release: f32,    // release coefficient
}

impl Limiter {
    pub fn new() -> Self {
        Limiter {
            threshold: 0.9,
            envelope: 0.0,
            attack: 0.001,
            release: 0.1,
        }
    }
}

impl Effect for Limiter {
    fn process(&mut self, input: f32) -> f32 {
        let abs_input = input.abs();
        // Follow the envelope
        if abs_input > self.envelope {
            self.envelope = self.envelope + self.attack * (abs_input - self.envelope);
        } else {
            self.envelope = self.envelope + self.release * (abs_input - self.envelope);
        }

        // Apply gain reduction if over threshold
        if self.envelope > self.threshold {
            input * (self.threshold / self.envelope)
        } else {
            input
        }
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "threshold" => self.threshold = value.clamp(0.1, 1.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![EffectParameter::new("threshold", self.threshold, 0.1, 1.0)]
    }

    fn name(&self) -> &str {
        "Limiter"
    }
}
