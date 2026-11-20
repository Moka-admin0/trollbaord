use anyhow::Result;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

pub struct Sound {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub fn is_wav(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
}

pub fn duration_seconds(path: &Path) -> Option<f32> {
    let reader = WavReader::open(path).ok()?;
    Some(reader.duration() as f32 / reader.spec().sample_rate as f32)
}

pub fn read_mono(path: &Path) -> Result<Sound> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        SampleFormat::Int => {
            let scale = 1.0 / (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|sample| sample.map(|value| value as f32 * scale)).collect::<Result<_, _>>()?
        }
    };

    let channels = usize::from(spec.channels.max(1));
    let samples = interleaved.chunks(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect();
    Ok(Sound { samples, sample_rate: spec.sample_rate })
}

pub fn write_mono(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = WavSpec { channels: 1, sample_rate, bits_per_sample: 16, sample_format: SampleFormat::Int };
    let mut writer = WavWriter::create(path, spec)?;
    for &sample in samples {
        writer.write_sample((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}
