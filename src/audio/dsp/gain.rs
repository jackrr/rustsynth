use super::{Effect, EffectParameter};

pub struct Gain {
    level: f32,
}

impl Gain {
    pub fn new() -> Self {
        Gain { level: 1.0 }
    }
}

impl Effect for Gain {
    fn process(&mut self, input: f32) -> f32 {
        input * self.level
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        if param_name == "level" {
            self.level = value.clamp(0.0, 2.0);
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![EffectParameter::new("level", self.level, 0.0, 2.0)]
    }

    fn name(&self) -> &str {
        "Gain"
    }
}
