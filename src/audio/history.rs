use std::collections::VecDeque;
use std::sync::Mutex;

pub struct History {
    samples: Mutex<VecDeque<f32>>,
    capacity: usize,
    sample_rate: u32,
}

impl History {
    pub fn new(sample_rate: u32, seconds: f32) -> Self {
        let capacity = (sample_rate as f32 * seconds) as usize;
        Self { samples: Mutex::new(VecDeque::with_capacity(capacity)), capacity, sample_rate }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn push(&self, samples: &[f32]) {
        let mut buffer = self.samples.lock().unwrap();
        buffer.extend(samples);
        let overflow = buffer.len().saturating_sub(self.capacity);
        buffer.drain(..overflow);
    }

    pub fn last(&self, seconds: f32) -> Vec<f32> {
        let buffer = self.samples.lock().unwrap();
        let wanted = ((self.sample_rate as f32 * seconds) as usize).min(buffer.len());
        buffer.range(buffer.len() - wanted..).copied().collect()
    }
}
