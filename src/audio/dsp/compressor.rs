use super::{Effect, EffectParameter};

pub struct Compressor {
    threshold: f32,   // dB
    ratio: f32,       // e.g., 4.0 = 4:1
    attack: f32,      // seconds
    release: f32,     // seconds
    makeup_gain: f32, // linear
    envelope: f32,
    sample_rate: f32,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        Compressor {
            threshold: -12.0,
            ratio: 4.0,
            attack: 0.003,
            release: 0.1,
            makeup_gain: 1.0,
            envelope: 0.0,
            sample_rate,
        }
    }

    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    fn linear_to_db(linear: f32) -> f32 {
        if linear <= 0.0 {
            -100.0
        } else {
            20.0 * linear.log10()
        }
    }
}

impl Effect for Compressor {
    fn process(&mut self, input: f32) -> f32 {
        let abs_input = input.abs();
        let attack_coeff = (-1.0 / (self.attack * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release * self.sample_rate)).exp();

        // Envelope follower
        if abs_input > self.envelope {
            self.envelope = abs_input + attack_coeff * (self.envelope - abs_input);
        } else {
            self.envelope = abs_input + release_coeff * (self.envelope - abs_input);
        }

        // Compute gain reduction
        let db_level = Self::linear_to_db(self.envelope);
        let gain = if db_level > self.threshold {
            let excess = db_level - self.threshold;
            let db_gain = self.threshold + excess / self.ratio - db_level;
            Self::db_to_linear(db_gain)
        } else {
            1.0
        };

        input * gain * self.makeup_gain
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "threshold" => self.threshold = value.clamp(-60.0, 0.0),
            "ratio" => self.ratio = value.clamp(1.0, 20.0),
            "attack" => self.attack = value.clamp(0.001, 0.5),
            "release" => self.release = value.clamp(0.01, 2.0),
            "makeup_gain" => self.makeup_gain = value.clamp(0.0, 4.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("threshold", self.threshold, -60.0, 0.0),
            EffectParameter::new("ratio", self.ratio, 1.0, 20.0),
            EffectParameter::new("attack", self.attack, 0.001, 0.5),
            EffectParameter::new("release", self.release, 0.01, 2.0),
            EffectParameter::new("makeup_gain", self.makeup_gain, 0.0, 4.0),
        ]
    }

    fn name(&self) -> &str {
        "Compressor"
    }
}
