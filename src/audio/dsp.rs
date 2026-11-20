const LOUDNESS_WINDOW_MS: u32 = 10;
const SILENCE_RMS: f32 = 0.01;
const PADDING_MS: u32 = 150;
const TARGET_PEAK: f32 = 0.9;
const MAX_GAIN: f32 = 8.0;

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |loudest, sample| loudest.max(sample.abs()))
}

pub fn trim_silence(samples: &[f32], sample_rate: u32) -> &[f32] {
    let window = (sample_rate * LOUDNESS_WINDOW_MS / 1000).max(1) as usize;
    let padding = (PADDING_MS / LOUDNESS_WINDOW_MS) as usize;
    let windows: Vec<&[f32]> = samples.chunks(window).collect();

    let first_loud = windows.iter().position(|chunk| is_loud(chunk));
    let last_loud = windows.iter().rposition(|chunk| is_loud(chunk));
    let (Some(first), Some(last)) = (first_loud, last_loud) else {
        return &[];
    };

    let start = first.saturating_sub(padding) * window;
    let end = ((last + 1 + padding) * window).min(samples.len());
    &samples[start..end]
}

pub fn normalize(samples: &mut [f32]) {
    let gain = (TARGET_PEAK / peak(samples)).min(MAX_GAIN);
    for sample in samples {
        *sample *= gain;
    }
}

pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }
    let step = f64::from(from_rate) / f64::from(to_rate);
    let output_len = (samples.len() as f64 / step) as usize;

    (0..output_len)
        .map(|index| {
            let position = index as f64 * step;
            let before = position as usize;
            let after = (before + 1).min(samples.len() - 1);
            lerp(samples[before], samples[after], (position - before as f64) as f32)
        })
        .collect()
}

pub fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn is_loud(samples: &[f32]) -> bool {
    rms(samples) > SILENCE_RMS
}

fn rms(samples: &[f32]) -> f32 {
    let sum_of_squares: f32 = samples.iter().map(|sample| sample * sample).sum();
    (sum_of_squares / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: u32 = 48_000;

    fn tone(seconds: f32, amplitude: f32) -> impl Iterator<Item = f32> {
        let len = (RATE as f32 * seconds) as usize;
        (0..len).map(move |i| amplitude * (i as f32 * 440.0 * TAU / RATE as f32).sin())
    }

    fn silence(seconds: f32) -> impl Iterator<Item = f32> {
        std::iter::repeat_n(0.0, (RATE as f32 * seconds) as usize)
    }

    #[test]
    fn trims_silence_but_keeps_padding() {
        let recording: Vec<f32> = silence(1.0).chain(tone(0.5, 0.2)).chain(silence(1.0)).collect();
        let seconds = trim_silence(&recording, RATE).len() as f32 / RATE as f32;
        assert!((0.75..0.85).contains(&seconds), "expected 0.5 s plus padding, got {seconds}");
    }

    #[test]
    fn background_noise_is_not_a_clip() {
        assert!(trim_silence(&vec![0.001; RATE as usize], RATE).is_empty());
    }

    #[test]
    fn normalize_brings_quiet_audio_up_to_target() {
        let mut quiet: Vec<f32> = tone(0.1, 0.2).collect();
        normalize(&mut quiet);
        assert!((peak(&quiet) - TARGET_PEAK).abs() < 0.01);
    }

    #[test]
    fn resample_changes_length_by_rate_ratio() {
        let samples: Vec<f32> = tone(0.1, 0.5).collect();
        assert_eq!(resample(&samples, 48_000, 44_100).len(), 4410);
        assert_eq!(resample(&samples, 48_000, 48_000), samples);
    }
}
