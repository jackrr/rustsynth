use std::f32::consts::PI;
use super::{Effect, EffectParameter};

/// Single allpass stage for phaser
struct AllpassStage {
    a1: f32,
    z1: f32,
}

impl AllpassStage {
    fn new() -> Self {
        AllpassStage { a1: 0.0, z1: 0.0 }
    }

    fn set_cutoff(&mut self, freq: f32, sample_rate: f32) {
        let w = 2.0 * PI * freq / sample_rate;
        let tan_w = (w / 2.0).tan();
        self.a1 = (tan_w - 1.0) / (tan_w + 1.0);
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.a1 * input + self.z1;
        self.z1 = input - self.a1 * output;
        output
    }
}

pub struct Phaser {
    stages: [AllpassStage; 6],
    lfo_phase: f32,
    rate: f32,        // LFO rate Hz
    depth: f32,       // LFO depth (semitones range)
    base_freq: f32,   // Center frequency Hz
    feedback: f32,
    feedback_val: f32,
    mix: f32,
    sample_rate: f32,
}

impl Phaser {
    pub fn new(sample_rate: f32) -> Self {
        Phaser {
            stages: std::array::from_fn(|_| AllpassStage::new()),
            lfo_phase: 0.0,
            rate: 0.5,
            depth: 0.7,
            base_freq: 1000.0,
            feedback: 0.7,
            feedback_val: 0.0,
            mix: 0.5,
            sample_rate,
        }
    }
}

impl Effect for Phaser {
    fn process(&mut self, input: f32) -> f32 {
        let lfo = self.lfo_phase.sin();
        self.lfo_phase += 2.0 * PI * self.rate / self.sample_rate;
        if self.lfo_phase >= 2.0 * PI {
            self.lfo_phase -= 2.0 * PI;
        }

        // Modulate cutoff frequency
        let freq = self.base_freq * (2.0_f32).powf(lfo * self.depth);

        // Update all allpass stages
        for stage in &mut self.stages {
            stage.set_cutoff(freq, self.sample_rate);
        }

        // Process through allpass stages
        let mut wet = input + self.feedback_val * self.feedback;
        for stage in &mut self.stages {
            wet = stage.process(wet);
        }
        self.feedback_val = wet;

        input * (1.0 - self.mix) + wet * self.mix
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "rate" => self.rate = value.clamp(0.01, 10.0),
            "depth" => self.depth = value.clamp(0.0, 2.0),
            "base_freq" => self.base_freq = value.clamp(100.0, 8000.0),
            "feedback" => self.feedback = value.clamp(0.0, 0.95),
            "mix" => self.mix = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("rate", self.rate, 0.01, 10.0),
            EffectParameter::new("depth", self.depth, 0.0, 2.0),
            EffectParameter::new("base_freq", self.base_freq, 100.0, 8000.0),
            EffectParameter::new("feedback", self.feedback, 0.0, 0.95),
            EffectParameter::new("mix", self.mix, 0.0, 1.0),
        ]
    }

    fn name(&self) -> &str {
        "Phaser"
    }
}
