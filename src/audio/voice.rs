use std::sync::Arc;

use crate::audio::envelope::EnvelopeGenerator;
use crate::audio::oscillator::{Oscillator, midi_to_freq};
use crate::state::messages::{NoteCommand, OscillatorType, SampleData};

pub struct Voice {
    pub oscillator: Oscillator,
    pub envelope: EnvelopeGenerator,
    pub active: bool,
    pub midi_note: u8,
    pub velocity: f32,
    /// Default note played by spacebar preview / when no UDP note specifies one
    pub default_midi_note: u8,
    /// Default velocity for spacebar preview
    pub default_velocity: f32,
    /// Remaining samples before auto-release
    pub length_remaining: u64,
    /// Send levels to effect groups [0..4]
    pub sends: [f32; 4],
    /// Sub-oscillator (always sine, tracks main pitch at an octave offset)
    pub sub_osc: Oscillator,
    pub sub_osc_enabled: bool,
    pub sub_osc_octave: i32, // -2 to +2 octaves relative to main
    pub sub_osc_level: f32,  // 0.0–1.0 mix into main signal
    pub muted: bool,
    pub soloed: bool,
    /// Loaded sample, if any; play it instead of the oscillator when `use_sample` is set.
    pub sample: Option<Arc<SampleData>>,
    pub sample_name: Option<String>,
    pub use_sample: bool,
    /// MIDI note at which the sample plays back at its original recorded pitch
    pub sample_root_note: u8,
    /// Gain applied to sample playback, to compensate for quieter source material (0.0-2.0)
    pub sample_level: f32,
    sample_pos: f64,
    engine_sample_rate: f32,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        let mut sends = [0.0_f32; 4];
        sends[0] = 1.0; // Default: send 100% to group A
        Voice {
            oscillator: Oscillator::new(sample_rate),
            envelope: EnvelopeGenerator::new(sample_rate),
            active: false,
            midi_note: 60,
            velocity: 0.0,
            default_midi_note: 60,
            default_velocity: 0.75,
            length_remaining: 0,
            sends,
            sub_osc: Oscillator::new(sample_rate),
            sub_osc_enabled: false,
            sub_osc_octave: -1,
            sub_osc_level: 0.5,
            muted: false,
            soloed: false,
            sample: None,
            sample_name: None,
            use_sample: false,
            sample_root_note: 60,
            sample_level: 1.0,
            sample_pos: 0.0,
            engine_sample_rate: sample_rate,
        }
    }

    pub fn note_on_raw(&mut self, midi_note: u8, velocity: f32, length_samples: u64) {
        self.midi_note = midi_note;
        self.velocity = velocity;
        self.length_remaining = length_samples;
        let freq = midi_to_freq(midi_note);
        self.oscillator.set_frequency(freq);
        self.oscillator.reset();
        self.sub_osc
            .set_frequency(freq * 2f32.powi(self.sub_osc_octave));
        self.sub_osc.reset();
        self.sample_pos = 0.0;
        self.envelope.note_on();
        self.active = true;
    }

    pub fn note_on(&mut self, cmd: &NoteCommand) {
        self.midi_note = cmd.midi_note;
        self.velocity = cmd.velocity;
        self.length_remaining = if self.use_sample {
            self.sample_length_remaining(cmd.length_units)
                .unwrap_or(cmd.length_samples)
        } else {
            cmd.length_samples
        };
        let freq = midi_to_freq(cmd.midi_note) * 2f32.powf(cmd.detune_cents / 1200.0);
        self.oscillator.set_frequency(freq);
        self.oscillator.reset();
        self.sub_osc
            .set_frequency(freq * 2f32.powi(self.sub_osc_octave));
        self.sub_osc.reset();
        self.sample_pos = 0.0;
        self.envelope.note_on();
        self.active = true;
    }

    pub fn set_sub_osc(&mut self, enabled: bool, octave: i32, level: f32) {
        self.sub_osc_enabled = enabled;
        self.sub_osc_octave = octave.clamp(-2, 2);
        self.sub_osc_level = level.clamp(0.0, 1.0);
        // Sync sub-osc frequency to current note
        self.sub_osc
            .set_frequency(midi_to_freq(self.midi_note) * 2f32.powi(self.sub_osc_octave));
    }

    pub fn note_off(&mut self) {
        self.envelope.note_off();
    }

    pub fn set_oscillator_type(&mut self, osc_type: OscillatorType) {
        self.oscillator.osc_type = osc_type;
    }

    pub fn load_sample(&mut self, sample: Arc<SampleData>, name: String, root_note: u8) {
        self.sample = Some(sample);
        self.sample_name = Some(name);
        self.sample_root_note = root_note;
        self.use_sample = true;
    }

    pub fn clear_sample(&mut self) {
        self.sample = None;
        self.sample_name = None;
        self.use_sample = false;
    }

    pub fn set_sample_mode(&mut self, use_sample: bool) {
        self.use_sample = use_sample && self.sample.is_some();
    }

    pub fn set_sample_root_note(&mut self, root_note: u8) {
        self.sample_root_note = root_note;
    }

    pub fn set_sample_level(&mut self, level: f32) {
        self.sample_level = level.clamp(0.0, 10.0);
    }

    /// How far `sample_pos` advances per engine output sample, given the current note's pitch
    /// relative to `sample_root_note` and the sample's native rate vs. the engine's rate.
    fn sample_advance_rate(&self, native_rate: f32) -> f64 {
        let pitch_ratio = 2f64.powf((self.midi_note as f64 - self.sample_root_note as f64) / 12.0);
        let rate_ratio = native_rate as f64 / self.engine_sample_rate as f64;
        pitch_ratio * rate_ratio
    }

    /// Number of engine samples needed to play `length_units + 1` 36ths of the loaded sample
    /// at the current pitch. Returns `None` when no sample is loaded.
    fn sample_length_remaining(&self, length_units: u8) -> Option<u64> {
        let sample = self.sample.as_ref()?;
        let len = sample.samples.len();
        let advance = self.sample_advance_rate(sample.sample_rate);
        if len == 0 || advance <= 0.0 {
            return Some(0);
        }
        let total_engine_samples = (len - 1) as f64 / advance;
        let fraction = (length_units as f64 + 1.0) / 36.0;
        Some((total_engine_samples * fraction).round() as u64)
    }

    /// Advance sample playback by one output frame using linear interpolation.
    /// Returns 0.0 once no sample is loaded or playback has reached the end.
    fn sample_next(&mut self) -> f32 {
        let (native_rate, len) = match &self.sample {
            Some(s) => (s.sample_rate, s.samples.len()),
            None => return 0.0,
        };
        if len == 0 || self.sample_pos >= (len - 1) as f64 {
            return 0.0;
        }

        let i0 = self.sample_pos as usize;
        let i1 = (i0 + 1).min(len - 1);
        let frac = (self.sample_pos - i0 as f64) as f32;
        let data = &self.sample.as_ref().unwrap().samples;
        let out = (data[i0] * (1.0 - frac) + data[i1] * frac) * self.sample_level;

        self.sample_pos += self.sample_advance_rate(native_rate);

        out
    }

    /// Process one sample. Returns the raw voice sample (before routing).
    pub fn process(&mut self) -> f32 {
        if !self.active && !self.envelope.is_active() {
            return 0.0;
        }

        // Auto-release after length_remaining samples
        if self.active && self.length_remaining > 0 {
            self.length_remaining -= 1;
            if self.length_remaining == 0 {
                self.note_off();
                self.active = false;
            }
        }

        let osc_sample = if self.use_sample {
            self.sample_next()
        } else {
            self.oscillator.next_sample()
        };
        let sub_sample = if self.sub_osc_enabled {
            self.sub_osc.next_sample() * self.sub_osc_level
        } else {
            0.0
        };
        let env_level = self.envelope.next_sample();

        // Mark inactive once envelope finishes
        if !self.envelope.is_active() {
            self.active = false;
        }

        (osc_sample + sub_sample) * env_level * self.velocity
    }

    /// Advance voice state without producing audio (used when muted/not soloed).
    pub fn tick_silent(&mut self) -> f32 {
        self.process();
        0.0
    }

    pub fn amplitude(&self) -> f32 {
        self.envelope.current_level() * self.velocity
    }
}
