mod capture;
mod mixer;
mod sessions;

pub use mixer::{AppFeeds, AppMixer};

use crate::engine::Soundboard;
use capture::Capture;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const PLAYING_THRESHOLD: f32 = 0.01;

#[derive(Clone, Debug)]
pub struct AudioApp {
    pub exe: String,
    pub name: String,
    pub pids: Vec<u32>,
    pub peak: f32,
    pub blocked_reason: Option<&'static str>,
    pub sending: bool,
    pub error: Option<String>,
}

impl AudioApp {
    pub fn is_playing(&self) -> bool {
        self.peak > PLAYING_THRESHOLD
    }
}

#[derive(Default)]
struct SharedState {
    apps: Mutex<Vec<AudioApp>>,
    enabled: Mutex<BTreeSet<String>>,
    stop: AtomicBool,
}

pub struct AppWatcher {
    state: Arc<SharedState>,
    wake: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl AppWatcher {
    pub fn spawn(soundboard: Arc<Soundboard>) -> Self {
        let state = Arc::new(SharedState::default());
        let (wake, woken) = mpsc::channel();
        let watcher = Watcher {
            soundboard,
            state: state.clone(),
            display_names: HashMap::new(),
            captures: HashMap::new(),
            failures: HashMap::new(),
            published: Vec::new(),
        };
        let thread = thread::spawn(move || watcher.run(woken));
        Self { state, wake, thread: Some(thread) }
    }

    pub fn apps(&self) -> Vec<AudioApp> {
        self.state.apps.lock().unwrap().clone()
    }

    pub fn set_enabled(&self, exes: &BTreeSet<String>) {
        let mut enabled = self.state.enabled.lock().unwrap();
        if *enabled != *exes {
            enabled.clone_from(exes);
            let _ = self.wake.send(());
        }
    }
}

impl Drop for AppWatcher {
    fn drop(&mut self) {
        self.state.stop.store(true, Relaxed);
        let _ = self.wake.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Watcher {
    soundboard: Arc<Soundboard>,
    state: Arc<SharedState>,
    display_names: HashMap<String, String>,
    captures: HashMap<u32, Capture>,
    failures: HashMap<u32, String>,
    published: Vec<u32>,
}

impl Watcher {
    fn run(mut self, woken: Receiver<()>) {
        let _ = wasapi::initialize_mta();
        while !self.state.stop.load(Relaxed) {
            self.update();
            if let Err(RecvTimeoutError::Disconnected) = woken.recv_timeout(POLL_INTERVAL) {
                break;
            }
        }
        self.soundboard.app_feeds.publish(Vec::new());
    }

    fn update(&mut self) {
        let mut apps = sessions::list_audio_apps(&mut self.display_names);
        let enabled = self.state.enabled.lock().unwrap().clone();

        self.forget_failures_of_disabled_apps(&apps, &enabled);
        let wanted = self.pids_to_capture(&apps, &enabled);
        self.sync_captures(&wanted);
        self.publish_feeds();

        for app in &mut apps {
            app.sending = app.pids.iter().any(|pid| self.captures.contains_key(pid));
            app.error = app.pids.iter().find_map(|pid| self.failures.get(pid).cloned());
        }
        *self.state.apps.lock().unwrap() = apps;
        self.soundboard.request_repaint();
    }

    fn forget_failures_of_disabled_apps(&mut self, apps: &[AudioApp], enabled: &BTreeSet<String>) {
        self.failures.retain(|pid, _| apps.iter().any(|app| app.pids.contains(pid) && enabled.contains(&app.exe)));
    }

    fn pids_to_capture(&self, apps: &[AudioApp], enabled: &BTreeSet<String>) -> HashSet<u32> {
        if !self.soundboard.is_live() {
            return HashSet::new();
        }
        apps.iter()
            .filter(|app| app.blocked_reason.is_none() && enabled.contains(&app.exe))
            .flat_map(|app| app.pids.iter().copied())
            .filter(|pid| !self.failures.contains_key(pid))
            .collect()
    }

    fn sync_captures(&mut self, wanted: &HashSet<u32>) {
        let failures = &mut self.failures;
        self.captures.retain(|pid, capture| match capture.error() {
            Some(error) => {
                failures.insert(*pid, error);
                false
            }
            None => wanted.contains(pid),
        });

        for &pid in wanted {
            if !self.failures.contains_key(&pid) {
                self.captures.entry(pid).or_insert_with(|| Capture::start(pid));
            }
        }
    }

    fn publish_feeds(&mut self) {
        let mut pids: Vec<u32> = self.captures.keys().copied().collect();
        pids.sort_unstable();
        if pids == self.published {
            return;
        }
        let feeds = self.captures.iter().map(|(&pid, capture)| (pid, capture.feed())).collect();
        self.soundboard.app_feeds.publish(feeds);
        self.published = pids;
    }
}
