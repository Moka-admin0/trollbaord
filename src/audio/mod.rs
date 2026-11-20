mod devices;
mod dsp;
mod feed;
mod history;
mod player;
mod streams;

pub use devices::{DeviceList, find_input, find_output};
pub use dsp::{normalize, peak, resample, trim_silence};
pub use feed::{Feed, FeedReader};
pub use history::History;
pub use player::{NowPlaying, Player};
pub use streams::{ErrorHandler, open_input, open_output};

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Relaxed))
    }

    pub fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Relaxed);
    }
}

#[derive(Default)]
pub struct PeakMeter(AtomicU32);

impl PeakMeter {
    pub fn record(&self, samples: &[f32]) {
        self.0.fetch_max(peak(samples).to_bits(), Relaxed);
    }

    pub fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0, Relaxed))
    }
}
