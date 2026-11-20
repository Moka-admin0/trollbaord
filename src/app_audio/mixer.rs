use crate::audio::{AtomicF32, Feed, FeedReader};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct AppFeeds {
    feeds: Mutex<Vec<(u32, Arc<Feed>)>>,
    version: AtomicU64,
}

impl AppFeeds {
    pub fn publish(&self, feeds: Vec<(u32, Arc<Feed>)>) {
        *self.feeds.lock().unwrap() = feeds;
        self.version.fetch_add(1, Relaxed);
    }
}

pub struct AppMixer {
    feeds: Arc<AppFeeds>,
    seen_version: Option<u64>,
    readers: Vec<(u32, FeedReader)>,
    output_rate: u32,
    gain: Arc<AtomicF32>,
}

impl AppMixer {
    pub fn new(feeds: Arc<AppFeeds>, output_rate: u32, gain: Arc<AtomicF32>) -> Self {
        Self { feeds, seen_version: None, readers: Vec::new(), output_rate, gain }
    }

    pub fn mix_into(&mut self, output: &mut [f32]) {
        self.pick_up_new_feeds();
        for (_, reader) in &mut self.readers {
            reader.mix_into(output);
        }
    }

    fn pick_up_new_feeds(&mut self) {
        let version = self.feeds.version.load(Relaxed);
        if self.seen_version == Some(version) {
            return;
        }
        let Ok(feeds) = self.feeds.feeds.try_lock() else {
            return;
        };

        self.seen_version = Some(version);
        self.readers.retain(|(pid, _)| feeds.iter().any(|(feed_pid, _)| feed_pid == pid));
        for (pid, feed) in feeds.iter() {
            if !self.readers.iter().any(|(reader_pid, _)| reader_pid == pid) {
                self.readers.push((*pid, FeedReader::new(feed.clone(), self.output_rate, self.gain.clone())));
            }
        }
    }
}
