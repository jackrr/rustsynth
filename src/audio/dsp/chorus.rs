use std::f32::consts::PI;
use super::{Effect, EffectParameter};

pub struct Chorus {
    buffer: Vec<f32>,
    write_pos: usize,
    rate: f32,        // LFO rate in Hz
    depth: f32,       // LFO depth in samples
    delay: f32,       // Base delay in seconds
    mix: f32,
    lfo_phase: f32,
    sample_rate: f32,
}

impl Chorus {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay = (sample_rate * 0.05) as usize; // 50ms max
        Chorus {
            buffer: vec![0.0; max_delay],
            write_pos: 0,
            rate: 0.5,
            depth: 20.0,  // samples
            delay: 0.02,  // 20ms base delay
            mix: 0.5,
            lfo_phase: 0.0,
            sample_rate,
        }
    }
}

impl Effect for Chorus {
    fn process(&mut self, input: f32) -> f32 {
        let buf_len = self.buffer.len();
        self.buffer[self.write_pos] = input;

        // LFO modulates delay time
        let lfo = self.lfo_phase.sin();
        self.lfo_phase += 2.0 * PI * self.rate / self.sample_rate;
        if self.lfo_phase >= 2.0 * PI {
            self.lfo_phase -= 2.0 * PI;
        }

        let delay_samples = (self.delay * self.sample_rate + lfo * self.depth)
            .max(1.0) as usize;
        let delay_samples = delay_samples.min(buf_len - 1);

        let read_pos = (self.write_pos + buf_len - delay_samples) % buf_len;
        let delayed = self.buffer[read_pos];

        self.write_pos = (self.write_pos + 1) % buf_len;

        input * (1.0 - self.mix) + delayed * self.mix
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "rate" => self.rate = value.clamp(0.1, 10.0),
            "depth" => self.depth = value.clamp(1.0, 50.0),
            "delay" => self.delay = value.clamp(0.005, 0.04),
            "mix" => self.mix = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("rate", self.rate, 0.1, 10.0),
            EffectParameter::new("depth", self.depth, 1.0, 50.0),
            EffectParameter::new("delay", self.delay, 0.005, 0.04),
            EffectParameter::new("mix", self.mix, 0.0, 1.0),
        ]
    }

    fn name(&self) -> &str {
        "Chorus"
    }
}

/// Vibrato: pitch LFO (like chorus but no dry signal)
pub struct Vibrato {
    buffer: Vec<f32>,
    write_pos: usize,
    rate: f32,
    depth: f32,
    delay: f32,
    lfo_phase: f32,
    sample_rate: f32,
}

impl Vibrato {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay = (sample_rate * 0.05) as usize;
        Vibrato {
            buffer: vec![0.0; max_delay],
            write_pos: 0,
            rate: 5.0,
            depth: 10.0,
            delay: 0.01,
            lfo_phase: 0.0,
            sample_rate,
        }
    }
}

impl Effect for Vibrato {
    fn process(&mut self, input: f32) -> f32 {
        let buf_len = self.buffer.len();
        self.buffer[self.write_pos] = input;

        let lfo = self.lfo_phase.sin();
        self.lfo_phase += 2.0 * PI * self.rate / self.sample_rate;
        if self.lfo_phase >= 2.0 * PI {
            self.lfo_phase -= 2.0 * PI;
        }

        let delay_samples = (self.delay * self.sample_rate + lfo * self.depth)
            .max(1.0) as usize;
        let delay_samples = delay_samples.min(buf_len - 1);

        let read_pos = (self.write_pos + buf_len - delay_samples) % buf_len;
        let output = self.buffer[read_pos];

        self.write_pos = (self.write_pos + 1) % buf_len;
        output
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "rate" => self.rate = value.clamp(0.1, 20.0),
            "depth" => self.depth = value.clamp(1.0, 50.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("rate", self.rate, 0.1, 20.0),
            EffectParameter::new("depth", self.depth, 1.0, 50.0),
        ]
    }

    fn name(&self) -> &str {
        "Vibrato"
    }
}
