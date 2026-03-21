use crate::audio::envelope::EnvelopeGenerator;
use crate::audio::oscillator::{Oscillator, midi_to_freq};
use crate::state::messages::{NoteCommand, OscillatorType};

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
        }
    }

    pub fn note_on(&mut self, cmd: &NoteCommand) {
        self.midi_note = cmd.midi_note;
        self.velocity = cmd.velocity;
        self.length_remaining = cmd.length_samples;
        self.oscillator.set_frequency(midi_to_freq(cmd.midi_note));
        self.oscillator.reset();
        self.envelope.note_on();
        self.active = true;
    }

    pub fn note_off(&mut self) {
        self.envelope.note_off();
    }

    pub fn set_oscillator_type(&mut self, osc_type: OscillatorType) {
        self.oscillator.osc_type = osc_type;
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

        let osc_sample = self.oscillator.next_sample();
        let env_level = self.envelope.next_sample();

        // Mark inactive once envelope finishes
        if !self.envelope.is_active() {
            self.active = false;
        }

        osc_sample * env_level * self.velocity
    }

    pub fn amplitude(&self) -> f32 {
        self.envelope.current_level() * self.velocity
    }
}
