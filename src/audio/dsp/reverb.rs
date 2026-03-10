/// Schroeder reverb: 4 comb filters + 2 allpass filters
use super::{Effect, EffectParameter};

struct CombFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp: f32,
    damp_state: f32,
}

impl CombFilter {
    fn new(delay_samples: usize, feedback: f32, damp: f32) -> Self {
        CombFilter {
            buffer: vec![0.0; delay_samples],
            pos: 0,
            feedback,
            damp,
            damp_state: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.pos];
        self.damp_state = output * (1.0 - self.damp) + self.damp_state * self.damp;
        self.buffer[self.pos] = input + self.damp_state * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }

    fn set_feedback(&mut self, f: f32) {
        self.feedback = f;
    }

    fn set_damp(&mut self, d: f32) {
        self.damp = d;
    }
}

struct AllpassFilter {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllpassFilter {
    fn new(delay_samples: usize) -> Self {
        AllpassFilter {
            buffer: vec![0.0; delay_samples],
            pos: 0,
            feedback: 0.5,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let output = -input + delayed;
        self.buffer[self.pos] = input + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }
}

pub struct Reverb {
    combs: [CombFilter; 4],
    allpasses: [AllpassFilter; 2],
    room_size: f32,
    damping: f32,
    mix: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        // Schroeder reverb delay times (in samples at 44.1kHz, scaled)
        let scale = sample_rate / 44100.0;
        let comb_delays = [
            (1557.0 * scale) as usize,
            (1617.0 * scale) as usize,
            (1491.0 * scale) as usize,
            (1422.0 * scale) as usize,
        ];
        let allpass_delays = [
            (225.0 * scale) as usize,
            (556.0 * scale) as usize,
        ];

        let room_size = 0.7;
        let damping = 0.5;
        let feedback = 0.84 + room_size * 0.1;

        Reverb {
            combs: [
                CombFilter::new(comb_delays[0], feedback, damping),
                CombFilter::new(comb_delays[1], feedback, damping),
                CombFilter::new(comb_delays[2], feedback, damping),
                CombFilter::new(comb_delays[3], feedback, damping),
            ],
            allpasses: [
                AllpassFilter::new(allpass_delays[0]),
                AllpassFilter::new(allpass_delays[1]),
            ],
            room_size,
            damping,
            mix: 0.3,
        }
    }

    fn update_params(&mut self) {
        let feedback = 0.84 + self.room_size * 0.1;
        for comb in &mut self.combs {
            comb.set_feedback(feedback);
            comb.set_damp(self.damping);
        }
    }
}

impl Effect for Reverb {
    fn process(&mut self, input: f32) -> f32 {
        // Sum all comb filter outputs
        let wet = self.combs[0].process(input)
            + self.combs[1].process(input)
            + self.combs[2].process(input)
            + self.combs[3].process(input);

        // Pass through allpass filters
        let wet = self.allpasses[0].process(wet * 0.25);
        let wet = self.allpasses[1].process(wet);

        input * (1.0 - self.mix) + wet * self.mix
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "room_size" => {
                self.room_size = value.clamp(0.0, 1.0);
                self.update_params();
            }
            "damping" => {
                self.damping = value.clamp(0.0, 1.0);
                self.update_params();
            }
            "mix" => self.mix = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("room_size", self.room_size, 0.0, 1.0),
            EffectParameter::new("damping", self.damping, 0.0, 1.0),
            EffectParameter::new("mix", self.mix, 0.0, 1.0),
        ]
    }

    fn name(&self) -> &str {
        "Reverb"
    }
}
