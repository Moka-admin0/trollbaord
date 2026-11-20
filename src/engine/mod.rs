mod config;
mod soundboard;

pub use config::{DeviceConfig, Route};
pub use soundboard::Soundboard;

use crate::app_audio::AppMixer;
use crate::audio::{self, ErrorHandler, Feed, FeedReader, History, PeakMeter, Player};
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, Stream, SupportedStreamConfig};
use std::sync::Arc;

pub const MAX_CLIP_SECONDS: f32 = 30.0;

pub struct Session {
    history: Arc<History>,
    players: Vec<Arc<Player>>,
    pub hearing: Arc<PeakMeter>,
    pub sending: Arc<PeakMeter>,
}

pub struct Engine {
    soundboard: Arc<Soundboard>,
    _streams: Vec<Stream>,
}

impl Engine {
    pub fn start(config: &DeviceConfig, soundboard: &Arc<Soundboard>) -> Result<Self> {
        config.validate()?;
        let on_error = report_errors_to(soundboard);
        let record = OutputDevice::open(&config.record_from)?;
        let send = OutputDevice::open(&config.send_to)?;

        let history = Arc::new(History::new(record.sample_rate(), MAX_CLIP_SECONDS));
        let hearing = Arc::new(PeakMeter::default());
        let sending = Arc::new(PeakMeter::default());
        let clip_player = Arc::new(Player::new(send.sample_rate(), soundboard.clip_volume.clone()));
        let mut players = vec![clip_player.clone()];
        let mut streams = Vec::new();

        let mut mic = None;
        if config.forwards_mic() {
            let (stream, reader) = forward_mic(&config.mic, send.sample_rate(), soundboard, on_error.clone())?;
            streams.push(stream);
            mic = Some(reader);
        }

        let mut virtual_mic = VirtualMicMix {
            mic,
            apps: AppMixer::new(soundboard.app_feeds.clone(), send.sample_rate(), soundboard.app_volume.clone()),
            clips: clip_player,
        };
        let meter = sending.clone();
        streams.push(send.play(on_error.clone(), move |buffer| {
            virtual_mic.mix_into(buffer);
            meter.record(buffer);
        })?);

        let (recorder, meter) = (history.clone(), hearing.clone());
        streams.push(record.record(on_error.clone(), move |samples| {
            recorder.push(samples);
            meter.record(samples);
        })?);

        if config.hear_clips {
            let monitor = Arc::new(Player::new(record.sample_rate(), soundboard.clip_volume.clone()));
            players.push(monitor.clone());
            streams.push(record.play(on_error, move |buffer| monitor.mix_into(buffer))?);
        }

        for stream in &streams {
            stream.play().context("Couldn't start audio")?;
        }
        soundboard.set_session(Some(Arc::new(Session { history, players, hearing, sending })));
        Ok(Self { soundboard: soundboard.clone(), _streams: streams })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.soundboard.set_session(None);
    }
}

struct VirtualMicMix {
    mic: Option<FeedReader>,
    apps: AppMixer,
    clips: Arc<Player>,
}

impl VirtualMicMix {
    fn mix_into(&mut self, buffer: &mut [f32]) {
        if let Some(mic) = &mut self.mic {
            mic.mix_into(buffer);
        }
        self.apps.mix_into(buffer);
        self.clips.mix_into(buffer);
    }
}

struct OutputDevice {
    name: String,
    device: Device,
    config: SupportedStreamConfig,
}

impl OutputDevice {
    fn open(name: &str) -> Result<Self> {
        let device = audio::find_output(name).with_context(|| not_connected(name))?;
        let config = device.default_output_config().with_context(|| format!("Couldn't use \"{name}\""))?;
        Ok(Self { name: name.to_owned(), device, config })
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate().0
    }

    fn play(&self, on_error: ErrorHandler, fill: impl FnMut(&mut [f32]) + Send + 'static) -> Result<Stream> {
        audio::open_output(&self.device, &self.config, on_error, fill)
            .with_context(|| format!("Couldn't open \"{}\"", self.name))
    }

    fn record(&self, on_error: ErrorHandler, on_audio: impl FnMut(&[f32]) + Send + 'static) -> Result<Stream> {
        audio::open_input(&self.device, &self.config, on_error, on_audio)
            .with_context(|| format!("Couldn't record from \"{}\"", self.name))
    }
}

fn forward_mic(
    name: &str,
    output_rate: u32,
    soundboard: &Soundboard,
    on_error: ErrorHandler,
) -> Result<(Stream, FeedReader)> {
    let device = audio::find_input(name).with_context(|| not_connected(name))?;
    let config = device.default_input_config().with_context(|| format!("Couldn't use \"{name}\""))?;
    let feed = Arc::new(Feed::new(config.sample_rate().0));
    let reader = FeedReader::new(feed.clone(), output_rate, soundboard.mic_volume.clone());
    let stream = audio::open_input(&device, &config, on_error, move |samples| feed.push(samples))
        .with_context(|| format!("Couldn't open \"{name}\""))?;
    Ok((stream, reader))
}

fn report_errors_to(soundboard: &Arc<Soundboard>) -> ErrorHandler {
    let soundboard = soundboard.clone();
    Arc::new(move |error| soundboard.show_error(format!("Audio device problem: {error}")))
}

fn not_connected(name: &str) -> String {
    format!("\"{name}\" isn't connected. Plug it in or pick another device in Setup.")
}
