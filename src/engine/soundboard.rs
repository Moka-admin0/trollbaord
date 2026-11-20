use super::{MAX_CLIP_SECONDS, Session};
use crate::app_audio::AppFeeds;
use crate::audio::{self, AtomicF32, NowPlaying};
use crate::clips::{self, Library};
use crate::hotkeys::HotkeyAction;
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct Status {
    pub text: String,
    pub is_error: bool,
    pub shown_at: Instant,
}

pub struct Soundboard {
    ui: egui::Context,
    pub library: Library,
    pub clip_volume: Arc<AtomicF32>,
    pub mic_volume: Arc<AtomicF32>,
    pub app_volume: Arc<AtomicF32>,
    pub clip_seconds: AtomicF32,
    pub app_feeds: Arc<AppFeeds>,
    session: Mutex<Option<Arc<Session>>>,
    status: Mutex<Option<Status>>,
}

impl Soundboard {
    pub fn new(ui: egui::Context, library: Library) -> Self {
        Self {
            ui,
            library,
            clip_volume: Arc::new(AtomicF32::new(1.0)),
            mic_volume: Arc::new(AtomicF32::new(1.0)),
            app_volume: Arc::new(AtomicF32::new(0.5)),
            clip_seconds: AtomicF32::new(8.0),
            app_feeds: Arc::new(AppFeeds::default()),
            session: Mutex::new(None),
            status: Mutex::new(None),
        }
    }

    pub fn request_repaint(&self) {
        self.ui.request_repaint();
    }

    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.lock().unwrap().clone()
    }

    pub(super) fn set_session(&self, session: Option<Arc<Session>>) {
        *self.session.lock().unwrap() = session;
    }

    pub fn is_live(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    pub fn status(&self) -> Option<Status> {
        self.status.lock().unwrap().clone()
    }

    pub fn show_info(&self, text: impl Into<String>) {
        self.show_status(text.into(), false);
    }

    pub fn show_error(&self, text: impl Into<String>) {
        self.show_status(text.into(), true);
    }

    fn show_status(&self, text: String, is_error: bool) {
        *self.status.lock().unwrap() = Some(Status { text, is_error, shown_at: Instant::now() });
        self.request_repaint();
    }

    pub fn handle_hotkey(&self, action: HotkeyAction) {
        match action {
            HotkeyAction::Clip => self.clip_now(),
            HotkeyAction::PlayRandom => self.play_random(),
            HotkeyAction::Stop => self.stop(),
            HotkeyAction::PlaySlot(slot) => self.play(slot),
        }
        self.request_repaint();
    }

    pub fn clip_now(&self) {
        let Some(session) = self.live_session() else {
            return;
        };
        let seconds = self.clip_seconds.get().min(MAX_CLIP_SECONDS);
        let sample_rate = session.history.sample_rate();
        let recording = session.history.last(seconds);

        let mut clip = audio::trim_silence(&recording, sample_rate).to_vec();
        if clip.is_empty() {
            self.show_error(format!("Nothing loud enough in the last {seconds:.0} s. Was anyone talking?"));
            return;
        }
        audio::normalize(&mut clip);

        match self.library.save_recording(&clip, sample_rate) {
            Ok(saved) => self.show_info(format!("Saved \"{}\" ({:.1} s)", saved.name, saved.seconds)),
            Err(error) => self.show_error(format!("Couldn't save the clip: {error:#}")),
        }
    }

    pub fn play(&self, index: usize) {
        let Some(session) = self.live_session() else {
            return;
        };
        let Some(clip) = self.library.get(index) else {
            self.show_error(format!("Slot {} is empty.", index + 1));
            return;
        };
        match clips::read_mono(&self.library.path_of(&clip)) {
            Ok(sound) => {
                for player in &session.players {
                    let samples = audio::resample(&sound.samples, sound.sample_rate, player.sample_rate());
                    player.play(clip.name.clone(), samples);
                }
            }
            Err(error) => self.show_error(format!("Couldn't play \"{}\": {error:#}", clip.name)),
        }
    }

    pub fn play_random(&self) {
        match self.library.len() {
            0 => self.show_error("No clips yet."),
            count => self.play(fastrand::usize(..count)),
        }
    }

    pub fn stop(&self) {
        if let Some(session) = self.session() {
            for player in &session.players {
                player.stop();
            }
        }
    }

    pub fn now_playing(&self) -> Option<NowPlaying> {
        self.session()?.players.first()?.now_playing()
    }

    fn live_session(&self) -> Option<Arc<Session>> {
        let session = self.session();
        if session.is_none() {
            self.show_error("Paused. Switch Live on first.");
        }
        session
    }
}
