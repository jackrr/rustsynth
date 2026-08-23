/// Decode a Launchpad X grid pad's note number (11-88, row*10+col, 1-indexed,
/// row 1 = bottom) into (row, col), or None if it's not a grid pad.
fn decode_pad(note: u8) -> Option<(u8, u8)> {
    let row = note / 10;
    let col = note % 10;
    if (1..=8).contains(&row) && (1..=8).contains(&col) {
        Some((row, col))
    } else {
        None
    }
}

/// Map a Launchpad X grid pad to one of the synth's 16 voices, mirroring the
/// TUI's voice grid layout (two 4x4 halves side by side, `voice = half*8 +
/// row*4 + col`). Uses the bottom 4 rows of the 8x8 grid; the top 4 rows are
/// unused for now.
pub fn pad_to_voice(note: u8) -> Option<usize> {
    let (row, col) = decode_pad(note)?;
    if row > 4 {
        return None;
    }
    let overall_row = (row - 1) as usize;
    let overall_col = (col - 1) as usize;
    let half = overall_col / 4;
    let local_col = overall_col % 4;
    Some(half * 8 + overall_row * 4 + local_col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottom_left_pad_is_voice_zero() {
        assert_eq!(pad_to_voice(11), Some(0));
    }

    #[test]
    fn test_second_half_starts_at_voice_eight() {
        // row 1, col 5 -> half 1, local_col 0 -> voice 8
        assert_eq!(pad_to_voice(15), Some(8));
    }

    #[test]
    fn test_top_row_of_voice_grid() {
        // row 4, col 4 -> overall_row 3, local_col 3 -> voice 15
        assert_eq!(pad_to_voice(44), Some(15));
    }

    #[test]
    fn test_rows_above_voice_grid_ignored() {
        assert_eq!(pad_to_voice(58), None);
        assert_eq!(pad_to_voice(81), None);
    }

    #[test]
    fn test_non_grid_note_ignored() {
        assert_eq!(pad_to_voice(0), None);
        assert_eq!(pad_to_voice(99), None);
    }
}
