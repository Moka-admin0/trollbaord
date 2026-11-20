use crate::audio::Feed;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat};

pub const CAPTURE_RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const BYTES_PER_FRAME: usize = CHANNELS * size_of::<f32>();
const STOP_CHECK_INTERVAL_MS: u32 = 100;

pub struct Capture {
    feed: Arc<Feed>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn start(pid: u32) -> Self {
        let feed = Arc::new(Feed::new(CAPTURE_RATE));
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));

        let thread = thread::spawn({
            let (feed, stop, error) = (feed.clone(), stop.clone(), error.clone());
            move || {
                if let Err(failure) = capture_process(pid, &feed, &stop) {
                    *error.lock().unwrap() = Some(format!("{failure:#}"));
                }
            }
        });
        Self { feed, stop, error, thread: Some(thread) }
    }

    pub fn feed(&self) -> Arc<Feed> {
        self.feed.clone()
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn capture_process(pid: u32, feed: &Feed, stop: &AtomicBool) -> Result<()> {
    let _ = wasapi::initialize_mta();
    let format = WaveFormat::new(32, 32, SampleType::Float, CAPTURE_RATE as usize, CHANNELS, None);
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
    let mut client = AudioClient::new_application_loopback_client(pid, true)?;
    client.initialize_client(&format, Direction::Capture, mode, None)?;
    let packets_ready = client.set_get_eventhandle()?;
    let capture_client = client.get_audiocaptureclient()?;
    client.start_stream()?;

    let mut bytes = Vec::new();
    let mut mono = Vec::new();
    while !stop.load(Relaxed) {
        if packets_ready.wait_for_event(STOP_CHECK_INTERVAL_MS).is_err() {
            continue;
        }
        while let Some(frames) = capture_client.get_next_packet_size()?.filter(|&frames| frames > 0) {
            bytes.resize(frames as usize * BYTES_PER_FRAME, 0);
            let (frames_read, info) = capture_client.read_from_device(&mut bytes)?;
            let packet = &bytes[..frames_read as usize * BYTES_PER_FRAME];

            mono.clear();
            if info.flags.silent {
                mono.resize(frames_read as usize, 0.0);
            } else {
                mono.extend(packet.chunks_exact(BYTES_PER_FRAME).map(stereo_frame_to_mono));
            }
            feed.push(&mono);
        }
    }
    client.stop_stream()?;
    Ok(())
}

fn stereo_frame_to_mono(frame: &[u8]) -> f32 {
    let left = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
    let right = f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
    (left + right) / 2.0
}
