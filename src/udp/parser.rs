use crate::state::messages::NoteCommand;

/// Parse a hex/alphanumeric digit (0-9, a-z case-insensitive) to value 0-35
fn parse_base36(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        '0'..='9' => Some(c as u8 - b'0'),
        'a'..='z' => Some(c as u8 - b'a' + 10),
        _ => None,
    }
}

/// Parse a note character to chromatic offset within octave.
/// Letters a-g map to notes in the current octave (a=A, b=B, c=C, d=D, e=E, f=F, g=G).
/// Digits 0-9 are lower (shifts down by semitones from C).
/// h-z are higher notes above G.
fn parse_note_char(c: char) -> Option<i32> {
    // Map note letters to chromatic offsets from C
    // C=0, C#=1, D=2, D#=3, E=4, F=5, F#=6, G=7, G#=8, A=9, A#=10, B=11
    match c.to_ascii_lowercase() {
        'c' => Some(0),
        'd' => Some(2),
        'e' => Some(4),
        'f' => Some(5),
        'g' => Some(7),
        'a' => Some(9),
        'b' => Some(11),
        // h-z extend above B: h=12, i=13, j=14, k=15, l=16, m=17, n=18, o=19, p=20, q=21, r=22, s=23, t=24, u=25, v=26, w=27, x=28, y=29, z=30
        'h' => Some(12),
        'i' => Some(13),
        'j' => Some(14),
        'k' => Some(15),
        'l' => Some(16),
        'm' => Some(17),
        'n' => Some(18),
        'o' => Some(19),
        'p' => Some(20),
        'q' => Some(21),
        'r' => Some(22),
        's' => Some(23),
        't' => Some(24),
        'u' => Some(25),
        'v' => Some(26),
        'w' => Some(27),
        'x' => Some(28),
        'y' => Some(29),
        'z' => Some(30),
        // Digits 0-9 map to notes below C (down by 1-10 semitones relative to C)
        '0' => Some(-10),
        '1' => Some(-9),
        '2' => Some(-8),
        '3' => Some(-7),
        '4' => Some(-6),
        '5' => Some(-5),
        '6' => Some(-4),
        '7' => Some(-3),
        '8' => Some(-2),
        '9' => Some(-1),
        _ => None,
    }
}

/// Convert note letter to MIDI note number.
/// octave: 0-9 (Pilot octave numbering)
/// note_char: c, d, e, f, g, a, b (and extended alphanum per protocol)
/// note_offset: additional chromatic steps to add
fn note_to_midi(octave: u8, note_char: char, note_offset: i32) -> Option<u8> {
    let semitone = parse_note_char(note_char)?;
    // MIDI note for C4 = 60; Pilot octave 4, C = MIDI 60
    // MIDI = (octave + 1) * 12 + semitone_from_c + offset
    let midi = (octave as i32 + 1) * 12 + semitone + note_offset;
    if midi >= 0 && midi <= 127 {
        Some(midi as u8)
    } else {
        None
    }
}

/// Parse a base-36 velocity (0-Z → 0.0-1.0)
fn parse_velocity(c: char) -> f32 {
    let v = parse_base36(c).unwrap_or(8) as f32;
    v / 35.0
}

/// Parse note length digit: 0-Z → duration multiplier
/// 0 = very short, Z = very long. Default (no digit) = 1/16 bar.
fn parse_length(c: char, sample_rate: f32) -> u64 {
    let v = parse_base36(c).unwrap_or(4) as f32;
    // Map 0-35 to a length in samples. 1/16 bar at 120bpm = 0.125s
    // Let's say 4 = 1/16 bar = 0.125s. Each unit is 1/16 of a bar.
    let bars_per_unit = 1.0 / 16.0;
    let bpm = 120.0;
    let seconds_per_bar = 60.0 / bpm * 4.0;
    let length_secs = (v + 1.0) * bars_per_unit * seconds_per_bar;
    (length_secs * sample_rate) as u64
}

/// Parse a UDP command string into a NoteCommand.
/// Format: [channel][octave][note][velocity?][length?][note_offset?]
/// - channel: hex digit 0-F (0-15)
/// - octave: digit 0-9
/// - note: letter (a-z) or digit (0-9) — see parse_note_char
/// - velocity: optional base-36 digit (0-Z → 0-1)
/// - length: optional base-36 digit
/// - note_offset: optional base-36 digit (chromatic shift)
pub fn parse_command(input: &str, sample_rate: f32) -> Option<NoteCommand> {
    let s = input.trim();
    let chars: Vec<char> = s.chars().collect();

    if chars.len() < 3 {
        return None;
    }

    // Channel: first char, hex 0-F
    let channel = parse_base36(chars[0])? as usize;
    if channel > 15 {
        return None;
    }

    // Octave: second char, digit 0-9
    let octave = chars[1].to_digit(10)? as u8;

    // Note: third char
    let note_char = chars[2];

    // Optional velocity (4th char)
    let velocity = if chars.len() > 3 {
        parse_velocity(chars[3])
    } else {
        0.5  // default: half velocity
    };

    // Optional length (5th char)
    let length_samples = if chars.len() > 4 {
        parse_length(chars[4], sample_rate)
    } else {
        parse_length('4', sample_rate)  // default: 1/16 bar
    };

    // Optional note offset (6th char)
    let note_offset = if chars.len() > 5 {
        parse_base36(chars[5])? as i32
    } else {
        0
    };

    let midi_note = note_to_midi(octave, note_char, note_offset)?;

    Some(NoteCommand {
        channel,
        midi_note,
        velocity,
        length_samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_c4() {
        let cmd = parse_command("04C", 48000.0).unwrap();
        assert_eq!(cmd.channel, 0);
        assert_eq!(cmd.midi_note, 60); // C4 = MIDI 60
        assert!((cmd.velocity - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_with_velocity() {
        let cmd = parse_command("04Cf", 48000.0).unwrap();
        assert_eq!(cmd.channel, 0);
        assert_eq!(cmd.midi_note, 60);
        // f = 15 in hex, but we're base-36, so f = 15/35 ≈ 0.43
        assert!((cmd.velocity - 15.0 / 35.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_channel_f() {
        let cmd = parse_command("f4G", 48000.0).unwrap();
        assert_eq!(cmd.channel, 15);
        assert_eq!(cmd.midi_note, 67); // G4
    }

    #[test]
    fn test_parse_invalid_too_short() {
        assert!(parse_command("04", 48000.0).is_none());
    }
}
