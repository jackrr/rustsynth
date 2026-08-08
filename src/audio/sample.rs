use anyhow::Context;

use crate::state::messages::SampleData;

/// Decode a WAV file to mono f32 samples. Runs on the UI thread (file I/O),
/// never on the audio thread.
pub fn load_wav(path: &std::path::Path) -> anyhow::Result<SampleData> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("failed to read float samples")?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<_>, _>>()
                .context("failed to read int samples")?
        }
    };

    let samples: Vec<f32> = if channels > 1 {
        interleaved
            .chunks(channels)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        interleaved
    };

    Ok(SampleData { samples, sample_rate: spec.sample_rate as f32 })
}
