/// ADSR Envelope Generator state machine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct EnvelopeGenerator {
    pub attack_time: f32,   // seconds
    pub decay_time: f32,    // seconds
    pub sustain_level: f32, // 0.0-1.0
    pub release_time: f32,  // seconds

    stage: EnvelopeStage,
    level: f32,             // Current amplitude 0.0-1.0
    sample_rate: f32,
}

impl EnvelopeGenerator {
    pub fn new(sample_rate: f32) -> Self {
        EnvelopeGenerator {
            attack_time: 0.01,
            decay_time: 0.2,
            sustain_level: 0.7,
            release_time: 0.3,
            stage: EnvelopeStage::Idle,
            level: 0.0,
            sample_rate,
        }
    }

    pub fn note_on(&mut self) {
        self.stage = EnvelopeStage::Attack;
        // Don't reset level to 0 — allows legato playing
    }

    pub fn note_off(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage = EnvelopeStage::Release;
        }
    }

    pub fn is_active(&self) -> bool {
        self.stage != EnvelopeStage::Idle
    }

    pub fn current_level(&self) -> f32 {
        self.level
    }

/// Advance envelope by one sample and return current amplitude
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            EnvelopeStage::Idle => {
                self.level = 0.0;
            }
            EnvelopeStage::Attack => {
                let increment = 1.0 / (self.attack_time * self.sample_rate);
                self.level += increment;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = EnvelopeStage::Decay;
                }
            }
            EnvelopeStage::Decay => {
                let decrement = (1.0 - self.sustain_level) / (self.decay_time * self.sample_rate);
                self.level -= decrement;
                if self.level <= self.sustain_level {
                    self.level = self.sustain_level;
                    self.stage = EnvelopeStage::Sustain;
                }
            }
            EnvelopeStage::Sustain => {
                self.level = self.sustain_level;
            }
            EnvelopeStage::Release => {
                let decrement = self.level / (self.release_time * self.sample_rate).max(1.0);
                self.level -= decrement;
                if self.level <= 0.001 {
                    self.level = 0.0;
                    self.stage = EnvelopeStage::Idle;
                }
            }
        }
        self.level
    }
}
