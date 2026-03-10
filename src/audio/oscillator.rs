use std::f32::consts::PI;
use crate::state::messages::OscillatorType;

pub struct Oscillator {
    pub osc_type: OscillatorType,
    pub frequency: f32,
    pub sample_rate: f32,
    phase: f32,
}

impl Oscillator {
    pub fn new(sample_rate: f32) -> Self {
        Oscillator {
            osc_type: OscillatorType::Sine,
            frequency: 440.0,
            sample_rate,
            phase: 0.0,
        }
    }

    pub fn set_frequency(&mut self, freq: f32) {
        self.frequency = freq;
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    pub fn next_sample(&mut self) -> f32 {
        let sample = self.generate(self.phase);
        self.phase += 2.0 * PI * self.frequency / self.sample_rate;
        // Keep phase in [0, 2π] to avoid float precision issues
        if self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        sample
    }

    fn generate(&self, phase: f32) -> f32 {
        match self.osc_type {
            OscillatorType::Sine => phase.sin(),
            OscillatorType::Triangle => triangle(phase),
            OscillatorType::Square => square(phase),
            OscillatorType::Sawtooth => sawtooth(phase),
            // Harmonic variants use additive synthesis
            OscillatorType::Sine2 => additive_sine(phase, 2),
            OscillatorType::Sine3 => additive_sine(phase, 3),
            OscillatorType::Sine4 => additive_sine(phase, 4),
            OscillatorType::Triangle2 => additive_triangle(phase, 2),
            OscillatorType::Triangle3 => additive_triangle(phase, 3),
            OscillatorType::Triangle4 => additive_triangle(phase, 4),
            OscillatorType::Square2 => additive_square(phase, 2),
            OscillatorType::Square3 => additive_square(phase, 3),
            OscillatorType::Square4 => additive_square(phase, 4),
            OscillatorType::Sawtooth2 => additive_sawtooth(phase, 2),
            OscillatorType::Sawtooth3 => additive_sawtooth(phase, 3),
            OscillatorType::Sawtooth4 => additive_sawtooth(phase, 4),
        }
    }
}

fn triangle(phase: f32) -> f32 {
    let t = phase / (2.0 * PI);
    if t < 0.5 {
        4.0 * t - 1.0
    } else {
        3.0 - 4.0 * t
    }
}

fn square(phase: f32) -> f32 {
    if phase < PI { 1.0 } else { -1.0 }
}

fn sawtooth(phase: f32) -> f32 {
    phase / PI - 1.0
}

/// Additive synthesis: sum first `harmonics` partials of sine
fn additive_sine(phase: f32, harmonics: usize) -> f32 {
    let mut sum = phase.sin();
    let mut amplitude = 1.0_f32;
    let mut total = amplitude;
    for h in 2..=harmonics {
        amplitude /= h as f32;
        sum += amplitude * (h as f32 * phase).sin();
        total += amplitude;
    }
    sum / total
}

/// Additive synthesis for triangle wave: odd harmonics with alternating signs
fn additive_triangle(phase: f32, harmonics: usize) -> f32 {
    let mut sum = 0.0_f32;
    let mut total = 0.0_f32;
    let mut sign = 1.0_f32;
    for h in (1..=(2 * harmonics - 1)).step_by(2) {
        let amp = 1.0 / (h as f32 * h as f32);
        sum += sign * amp * (h as f32 * phase).sin();
        total += amp;
        sign = -sign;
    }
    sum / total
}

/// Additive synthesis for square wave: odd harmonics
fn additive_square(phase: f32, harmonics: usize) -> f32 {
    let mut sum = 0.0_f32;
    let mut total = 0.0_f32;
    for h in (1..=(2 * harmonics - 1)).step_by(2) {
        let amp = 1.0 / h as f32;
        sum += amp * (h as f32 * phase).sin();
        total += amp;
    }
    sum / total
}

/// Additive synthesis for sawtooth: all harmonics
fn additive_sawtooth(phase: f32, harmonics: usize) -> f32 {
    let mut sum = 0.0_f32;
    let mut total = 0.0_f32;
    for h in 1..=harmonics {
        let amp = 1.0 / h as f32;
        let sign = if h % 2 == 0 { -1.0 } else { 1.0 };
        sum += sign * amp * (h as f32 * phase).sin();
        total += amp;
    }
    sum / total
}

/// Convert MIDI note to frequency in Hz
pub fn midi_to_freq(midi_note: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi_note as f32 - 69.0) / 12.0)
}
