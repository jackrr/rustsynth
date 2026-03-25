use std::f32::consts::PI;
use super::{Effect, EffectParameter};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterMode {
    Lowpass,
    Highpass,
    Bandpass,
}

/// Biquad filter (lowpass, highpass, bandpass)
pub struct BiquadFilter {
    mode: FilterMode,
    cutoff: f32,
    resonance: f32,
    sample_rate: f32,
    // Biquad coefficients
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32,
    // State
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl BiquadFilter {
    pub fn lowpass(sample_rate: f32) -> Self {
        let mut f = BiquadFilter {
            mode: FilterMode::Lowpass,
            cutoff: 8000.0,
            resonance: 0.707,
            sample_rate,
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        };
        f.update_coefficients();
        f
    }

    pub fn highpass(sample_rate: f32) -> Self {
        let mut f = BiquadFilter {
            mode: FilterMode::Highpass,
            cutoff: 100.0,
            resonance: 0.707,
            sample_rate,
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        };
        f.update_coefficients();
        f
    }

    pub fn bandpass(sample_rate: f32) -> Self {
        let mut f = BiquadFilter {
            mode: FilterMode::Bandpass,
            cutoff: 1000.0,
            resonance: 1.0,
            sample_rate,
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        };
        f.update_coefficients();
        f
    }

    fn update_coefficients(&mut self) {
        let w0 = 2.0 * PI * self.cutoff / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.resonance);

        match self.mode {
            FilterMode::Lowpass => {
                let a0 = 1.0 + alpha;
                self.b0 = (1.0 - cos_w0) / 2.0 / a0;
                self.b1 = (1.0 - cos_w0) / a0;
                self.b2 = (1.0 - cos_w0) / 2.0 / a0;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
            }
            FilterMode::Highpass => {
                let a0 = 1.0 + alpha;
                self.b0 = (1.0 + cos_w0) / 2.0 / a0;
                self.b1 = -(1.0 + cos_w0) / a0;
                self.b2 = (1.0 + cos_w0) / 2.0 / a0;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
            }
            FilterMode::Bandpass => {
                let a0 = 1.0 + alpha;
                self.b0 = sin_w0 / 2.0 / a0;
                self.b1 = 0.0;
                self.b2 = -sin_w0 / 2.0 / a0;
                self.a1 = -2.0 * cos_w0 / a0;
                self.a2 = (1.0 - alpha) / a0;
            }
        }
    }
}

impl Effect for BiquadFilter {
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "cutoff" => {
                self.cutoff = value.clamp(20.0, self.sample_rate / 2.0 - 1.0);
                self.update_coefficients();
            }
            "resonance" => {
                self.resonance = value.clamp(0.1, 20.0);
                self.update_coefficients();
            }
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new_log("cutoff", self.cutoff, 20.0, 20000.0),
            EffectParameter::new("resonance", self.resonance, 0.1, 20.0),
        ]
    }

    fn name(&self) -> &str {
        match self.mode {
            FilterMode::Lowpass => "Lowpass",
            FilterMode::Highpass => "Highpass",
            FilterMode::Bandpass => "Bandpass",
        }
    }
}

/// 3-band parametric EQ
pub struct Eq3 {
    low: BiquadFilter,
    mid: BiquadFilter,
    high: BiquadFilter,
    low_gain: f32,
    mid_gain: f32,
    high_gain: f32,
}

impl Eq3 {
    pub fn new(sample_rate: f32) -> Self {
        let mut low = BiquadFilter::lowpass(sample_rate);
        low.cutoff = 300.0;
        low.update_coefficients();

        let mut mid = BiquadFilter::bandpass(sample_rate);
        mid.cutoff = 1000.0;
        mid.update_coefficients();

        let mut high = BiquadFilter::highpass(sample_rate);
        high.cutoff = 5000.0;
        high.update_coefficients();

        Eq3 {
            low,
            mid,
            high,
            low_gain: 1.0,
            mid_gain: 1.0,
            high_gain: 1.0,
        }
    }
}

impl Effect for Eq3 {
    fn process(&mut self, input: f32) -> f32 {
        let low = self.low.process(input) * self.low_gain;
        let mid = self.mid.process(input) * self.mid_gain;
        let high = self.high.process(input) * self.high_gain;
        (low + mid + high) / 3.0
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "low_gain" => self.low_gain = value.clamp(0.0, 4.0),
            "mid_gain" => self.mid_gain = value.clamp(0.0, 4.0),
            "high_gain" => self.high_gain = value.clamp(0.0, 4.0),
            "low_freq" => self.low.set_parameter("cutoff", value),
            "mid_freq" => self.mid.set_parameter("cutoff", value),
            "high_freq" => self.high.set_parameter("cutoff", value),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("low_gain", self.low_gain, 0.0, 4.0),
            EffectParameter::new("mid_gain", self.mid_gain, 0.0, 4.0),
            EffectParameter::new("high_gain", self.high_gain, 0.0, 4.0),
            EffectParameter::new_log("low_freq", self.low.cutoff, 20.0, 1000.0),
            EffectParameter::new_log("mid_freq", self.mid.cutoff, 200.0, 8000.0),
            EffectParameter::new_log("high_freq", self.high.cutoff, 1000.0, 20000.0),
        ]
    }

    fn name(&self) -> &str {
        "EQ3"
    }
}
