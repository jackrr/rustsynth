use super::{Effect, EffectParameter};

pub struct Bitcrusher {
    bit_depth: f32,    // 1-16
    rate_reduce: f32,  // 1-32 (sample rate reduction factor)
    sample_hold: f32,
    hold_counter: f32,
}

impl Bitcrusher {
    pub fn new() -> Self {
        Bitcrusher {
            bit_depth: 8.0,
            rate_reduce: 1.0,
            sample_hold: 0.0,
            hold_counter: 0.0,
        }
    }
}

impl Effect for Bitcrusher {
    fn process(&mut self, input: f32) -> f32 {
        // Sample rate reduction
        self.hold_counter += 1.0;
        if self.hold_counter >= self.rate_reduce {
            self.hold_counter = 0.0;
            // Bit depth reduction
            let levels = 2.0_f32.powf(self.bit_depth);
            self.sample_hold = (input * levels).round() / levels;
        }
        self.sample_hold
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "bit_depth" => self.bit_depth = value.clamp(1.0, 16.0),
            "rate_reduce" => self.rate_reduce = value.clamp(1.0, 32.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("bit_depth", self.bit_depth, 1.0, 16.0),
            EffectParameter::new("rate_reduce", self.rate_reduce, 1.0, 32.0),
        ]
    }

    fn name(&self) -> &str {
        "Bitcrusher"
    }
}
