use super::{Effect, EffectParameter};

pub struct WhiteNoise {
    level: f32,
    rng: u32,
    /// Envelope follower: tracks input amplitude so noise gates with the voice
    env_level: f32,
    attack_coeff: f32,  // ~1ms — snaps up quickly to catch note onsets
    release_coeff: f32, // ~150ms — fades out after note ends
}

impl WhiteNoise {
    pub fn new(sample_rate: f32) -> Self {
        WhiteNoise {
            level: 0.3,
            rng: 0xdeadbeef,
            env_level: 0.0,
            attack_coeff:  (-1.0 / (0.001 * sample_rate)).exp(),
            release_coeff: (-1.0 / (0.15  * sample_rate)).exp(),
        }
    }

    fn next_sample(&mut self) -> f32 {
        // xorshift32 — allocation-free, lock-free, good enough for audio noise
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Effect for WhiteNoise {
    fn process(&mut self, input: f32) -> f32 {
        // Envelope follower: fast attack so noise appears immediately on note-on,
        // slower release so it fades naturally after note-off rather than cutting abruptly.
        let abs_in = input.abs();
        let coeff = if abs_in > self.env_level { self.attack_coeff } else { self.release_coeff };
        self.env_level = self.env_level * coeff + abs_in * (1.0 - coeff);

        input + self.next_sample() * self.level * self.env_level
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        if param_name == "level" {
            self.level = value.clamp(0.0, 1.0);
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![EffectParameter::new("level", self.level, 0.0, 1.0)]
    }

    fn name(&self) -> &str {
        "WhiteNoise"
    }
}
