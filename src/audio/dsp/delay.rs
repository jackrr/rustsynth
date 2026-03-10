use super::{Effect, EffectParameter};

pub struct Delay {
    buffer: Vec<f32>,
    write_pos: usize,
    delay_samples: usize,
    feedback: f32,
    mix: f32,         // wet/dry mix
    sample_rate: f32,
    delay_time: f32,  // in seconds
}

impl Delay {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_secs = 2.0;
        let buf_size = (sample_rate * max_delay_secs) as usize;
        let delay_time = 0.25; // 250ms default
        Delay {
            buffer: vec![0.0; buf_size],
            write_pos: 0,
            delay_samples: (delay_time * sample_rate) as usize,
            feedback: 0.4,
            mix: 0.5,
            sample_rate,
            delay_time,
        }
    }
}

impl Effect for Delay {
    fn process(&mut self, input: f32) -> f32 {
        let buf_len = self.buffer.len();
        let read_pos = (self.write_pos + buf_len - self.delay_samples) % buf_len;
        let delayed = self.buffer[read_pos];

        self.buffer[self.write_pos] = input + delayed * self.feedback;
        self.write_pos = (self.write_pos + 1) % buf_len;

        input * (1.0 - self.mix) + delayed * self.mix
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "time" => {
                self.delay_time = value.clamp(0.01, 2.0);
                self.delay_samples = (self.delay_time * self.sample_rate) as usize;
            }
            "feedback" => self.feedback = value.clamp(0.0, 0.95),
            "mix" => self.mix = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("time", self.delay_time, 0.01, 2.0),
            EffectParameter::new("feedback", self.feedback, 0.0, 0.95),
            EffectParameter::new("mix", self.mix, 0.0, 1.0),
        ]
    }

    fn name(&self) -> &str {
        "Delay"
    }
}
