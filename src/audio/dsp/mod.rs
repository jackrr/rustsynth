pub mod gain;
pub mod bitcrusher;
pub mod distortion;
pub mod limiter;
pub mod delay;
pub mod reverb;
pub mod tremolo;
pub mod chorus;
pub mod phaser;
pub mod filters;
pub mod compressor;
pub mod noise;

use crate::state::messages::EffectType;

#[derive(Debug, Clone)]
pub struct EffectParameter {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    /// Named options for enum params. When set, ←→ steps by 1 and the UI shows
    /// the label instead of the raw number.
    pub labels: Option<&'static [&'static str]>,
    /// When true, ←→ uses multiplicative (semitone-interval) stepping instead of
    /// linear percentage-of-range steps. Use for frequency params.
    pub logarithmic: bool,
}

impl EffectParameter {
    pub fn new(name: &str, value: f32, min: f32, max: f32) -> Self {
        EffectParameter { name: name.to_string(), value, min, max, labels: None, logarithmic: false }
    }

    pub fn new_log(name: &str, value: f32, min: f32, max: f32) -> Self {
        EffectParameter { name: name.to_string(), value, min, max, labels: None, logarithmic: true }
    }

    pub fn new_enum(name: &str, value: f32, labels: &'static [&'static str]) -> Self {
        EffectParameter {
            name: name.to_string(),
            value,
            min: 0.0,
            max: (labels.len() - 1) as f32,
            labels: Some(labels),
            logarithmic: false,
        }
    }
}

pub trait Effect: Send {
    fn process(&mut self, input: f32) -> f32;
    fn set_parameter(&mut self, param_name: &str, value: f32);
    fn get_parameters(&self) -> Vec<EffectParameter>;
    fn name(&self) -> &str;
}

/// Create a boxed Effect from an EffectType
pub fn create_effect(effect_type: EffectType, sample_rate: f32) -> Box<dyn Effect> {
    match effect_type {
        EffectType::Gain => Box::new(gain::Gain::new()),
        EffectType::Bitcrusher => Box::new(bitcrusher::Bitcrusher::new()),
        EffectType::Distortion => Box::new(distortion::Distortion::new()),
        EffectType::Limiter => Box::new(limiter::Limiter::new()),
        EffectType::Delay => Box::new(delay::Delay::new(sample_rate)),
        EffectType::Reverb => Box::new(reverb::Reverb::new(sample_rate)),
        EffectType::Tremolo => Box::new(tremolo::Tremolo::new(sample_rate)),
        EffectType::Chorus => Box::new(chorus::Chorus::new(sample_rate)),
        EffectType::Phaser => Box::new(phaser::Phaser::new(sample_rate)),
        EffectType::Vibrato => Box::new(chorus::Vibrato::new(sample_rate)),
        EffectType::Lowpass => Box::new(filters::BiquadFilter::lowpass(sample_rate)),
        EffectType::Highpass => Box::new(filters::BiquadFilter::highpass(sample_rate)),
        EffectType::Bandpass => Box::new(filters::BiquadFilter::bandpass(sample_rate)),
        EffectType::Eq3 => Box::new(filters::Eq3::new(sample_rate)),
        EffectType::Compressor => Box::new(compressor::Compressor::new(sample_rate)),
        EffectType::WhiteNoise => Box::new(noise::WhiteNoise::new(sample_rate)),
    }
}
