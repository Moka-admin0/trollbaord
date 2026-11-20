use super::AtomicF32;
use super::dsp::lerp;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const MAX_BACKLOG_MS: usize = 150;
const BACKLOG_AFTER_CATCH_UP_MS: usize = 40;
const PRIMING_MS: usize = 20;

pub struct Feed {
    queue: Mutex<VecDeque<f32>>,
    sample_rate: u32,
}

impl Feed {
    pub fn new(sample_rate: u32) -> Self {
        Self { queue: Mutex::new(VecDeque::new()), sample_rate }
    }

    pub fn push(&self, samples: &[f32]) {
        let mut queue = self.queue.lock().unwrap();
        queue.extend(samples);
        if queue.len() > self.samples_in(MAX_BACKLOG_MS) {
            let excess = queue.len() - self.samples_in(BACKLOG_AFTER_CATCH_UP_MS);
            queue.drain(..excess);
        }
    }

    fn samples_in(&self, milliseconds: usize) -> usize {
        self.sample_rate as usize * milliseconds / 1000
    }
}

pub struct FeedReader {
    feed: Arc<Feed>,
    gain: Arc<AtomicF32>,
    step: f64,
    position: f64,
    previous: f32,
    next: f32,
    primed: bool,
}

impl FeedReader {
    pub fn new(feed: Arc<Feed>, output_rate: u32, gain: Arc<AtomicF32>) -> Self {
        let step = f64::from(feed.sample_rate) / f64::from(output_rate);
        Self { feed, gain, step, position: 0.0, previous: 0.0, next: 0.0, primed: false }
    }

    pub fn mix_into(&mut self, output: &mut [f32]) {
        let mut queue = self.feed.queue.lock().unwrap();
        if !self.primed {
            if queue.len() < self.feed.samples_in(PRIMING_MS) {
                return;
            }
            self.primed = true;
        }

        let gain = self.gain.get();
        for out in output {
            *out += lerp(self.previous, self.next, self.position as f32) * gain;
            self.position += self.step;
            while self.position >= 1.0 {
                self.position -= 1.0;
                self.previous = self.next;
                self.next = queue.pop_front().unwrap_or_else(|| {
                    self.primed = false;
                    0.0
                });
            }
        }
    }
}
