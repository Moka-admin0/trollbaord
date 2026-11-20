use super::AtomicF32;
use std::sync::{Arc, Mutex};

pub struct NowPlaying {
    pub name: String,
    pub progress: f32,
}

pub struct Player {
    sample_rate: u32,
    volume: Arc<AtomicF32>,
    current: Mutex<Option<Playback>>,
}

struct Playback {
    name: String,
    samples: Vec<f32>,
    position: usize,
}

impl Player {
    pub fn new(sample_rate: u32, volume: Arc<AtomicF32>) -> Self {
        Self { sample_rate, volume, current: Mutex::new(None) }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn play(&self, name: String, samples: Vec<f32>) {
        *self.current.lock().unwrap() = Some(Playback { name, samples, position: 0 });
    }

    pub fn stop(&self) {
        *self.current.lock().unwrap() = None;
    }

    pub fn now_playing(&self) -> Option<NowPlaying> {
        let current = self.current.lock().unwrap();
        let playback = current.as_ref()?;
        let progress = playback.position as f32 / playback.samples.len().max(1) as f32;
        Some(NowPlaying { name: playback.name.clone(), progress })
    }

    pub fn mix_into(&self, output: &mut [f32]) {
        let mut current = self.current.lock().unwrap();
        let Some(playback) = current.as_mut() else {
            return;
        };

        let volume = self.volume.get();
        let remaining = &playback.samples[playback.position..];
        let count = output.len().min(remaining.len());
        for (out, sample) in output.iter_mut().zip(&remaining[..count]) {
            *out += sample * volume;
        }

        playback.position += count;
        if playback.position == playback.samples.len() {
            *current = None;
        }
    }
}
