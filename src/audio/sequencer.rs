use crate::audio::voice::Voice;

#[derive(Clone, Copy)]
pub struct SequencerStep {
    pub enabled: bool,
    pub midi_note: u8,
    pub velocity: f32,
}

impl Default for SequencerStep {
    fn default() -> Self {
        SequencerStep { enabled: false, midi_note: 60, velocity: 0.75 }
    }
}

pub struct SequencerPattern {
    pub steps: [[SequencerStep; 16]; 16],
    pub step_count: usize,
    pub bpm: f32,
    pub swing: f32,
    pub playing: bool,
}

impl Default for SequencerPattern {
    fn default() -> Self {
        SequencerPattern {
            steps: [[SequencerStep::default(); 16]; 16],
            step_count: 16,
            bpm: 120.0,
            swing: 0.0,
            playing: false,
        }
    }
}

pub struct AudioSequencer {
    pub pattern: SequencerPattern,
    sample_rate: f32,
    samples_per_tick: f64,
    tick_accumulator: f64,
    pub current_step: usize,
    pub tick_in_step: u64,
    ticks_per_step: u64,
}

impl AudioSequencer {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = AudioSequencer {
            pattern: SequencerPattern::default(),
            sample_rate,
            samples_per_tick: 0.0,
            tick_accumulator: 0.0,
            current_step: 0,
            tick_in_step: 0,
            ticks_per_step: 6, // 1/16 note at 24 PPQN
        };
        s.recalc_timing();
        s
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.pattern.bpm = bpm.clamp(20.0, 300.0);
        self.recalc_timing();
    }

    fn recalc_timing(&mut self) {
        // 24 PPQN: samples_per_beat / 24
        let samples_per_beat = self.sample_rate as f64 * 60.0 / self.pattern.bpm as f64;
        self.samples_per_tick = samples_per_beat / 24.0;
        // 1/16 note = 6 ticks at 24 PPQN
        self.ticks_per_step = 6;
    }

    fn step_threshold(&self, step: usize) -> u64 {
        // Swing: delay odd steps
        let swing_ticks = (self.pattern.swing * self.ticks_per_step as f32) as u64;
        if step % 2 == 1 {
            self.ticks_per_step + swing_ticks
        } else {
            self.ticks_per_step
        }
    }

    fn samples_per_step(&self) -> f64 {
        self.samples_per_tick * self.ticks_per_step as f64
    }

    pub fn stop(&mut self) {
        self.pattern.playing = false;
        self.current_step = 0;
        self.tick_in_step = 0;
        self.tick_accumulator = 0.0;
    }

    fn fire_step(&self, voices: &mut [Voice; 16], step: usize) {
        let gate_samples = (self.samples_per_step() * 0.8) as u64;
        for voice_idx in 0..16 {
            let s = &self.pattern.steps[voice_idx][step];
            if s.enabled {
                voices[voice_idx].note_on_raw(s.midi_note, s.velocity, gate_samples);
            }
        }
    }

    /// Called once per output sample from the audio thread.
    pub fn tick(&mut self, voices: &mut [Voice; 16]) {
        if !self.pattern.playing {
            return;
        }
        self.tick_accumulator += 1.0;
        while self.tick_accumulator >= self.samples_per_tick {
            self.tick_accumulator -= self.samples_per_tick;
            self.tick_in_step += 1;
            if self.tick_in_step >= self.step_threshold(self.current_step) {
                self.tick_in_step = 0;
                self.current_step = (self.current_step + 1) % self.pattern.step_count;
                self.fire_step(voices, self.current_step);
            }
        }
    }
}
