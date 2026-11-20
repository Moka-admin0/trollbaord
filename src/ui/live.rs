use super::widgets::meter_level;
use crate::engine::{DeviceConfig, Engine, Soundboard};
use crate::hotkeys::{Binding, HotkeyAction, HotkeyThread, KeyConfig, SLOT_COUNT, format_hotkey};
use std::sync::Arc;

const METER_FALL_PER_SECOND: f32 = 1.2;

#[derive(Default)]
pub struct LiveAudio {
    engine: Option<Engine>,
    started_with: Option<DeviceConfig>,
    error: Option<String>,
}

impl LiveAudio {
    pub fn is_running(&self) -> bool {
        self.engine.is_some()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn retry(&mut self) {
        self.started_with = None;
    }

    pub fn update(&mut self, live: bool, config: &DeviceConfig, soundboard: &Arc<Soundboard>) {
        if !live {
            *self = Self::default();
            return;
        }
        if self.started_with.as_ref() == Some(config) {
            return;
        }

        self.engine = None;
        self.started_with = Some(config.clone());
        match Engine::start(config, soundboard) {
            Ok(engine) => {
                self.engine = Some(engine);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }
}

#[derive(Default)]
pub struct ActiveHotkeys {
    thread: Option<HotkeyThread>,
    registered: Option<KeyConfig>,
    problem: Option<String>,
}

impl ActiveHotkeys {
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    pub fn update(&mut self, keys: &KeyConfig, soundboard: &Arc<Soundboard>) {
        if self.registered.as_ref() == Some(keys) {
            return;
        }

        self.thread = None;
        self.registered = Some(keys.clone());
        self.problem = None;

        let bindings = match keys.bindings() {
            Ok(bindings) => bindings,
            Err(error) => {
                self.problem = Some(format!("Hotkeys are off: {error:#}"));
                return;
            }
        };

        let soundboard = soundboard.clone();
        let (thread, taken) = HotkeyThread::spawn(bindings, move |action| soundboard.handle_hotkey(action));
        self.thread = Some(thread);
        if !taken.is_empty() {
            let keys = describe_taken_keys(&taken, &keys.slot_mods);
            self.problem = Some(format!("Another app already uses {keys}. Pick different keys in Setup."));
        }
    }
}

fn describe_taken_keys(taken: &[Binding], slot_mods: &str) -> String {
    let is_slot = |binding: &&Binding| matches!(binding.action, HotkeyAction::PlaySlot(_));
    let mut labels: Vec<String> = taken.iter().filter(|b| !is_slot(b)).map(|b| b.label.clone()).collect();
    let slots: Vec<&Binding> = taken.iter().filter(is_slot).collect();
    if slots.len() == SLOT_COUNT {
        labels.push(format!("{}+1–9", format_hotkey(slot_mods)));
    } else {
        labels.extend(slots.iter().map(|b| b.label.clone()));
    }
    labels.join(", ")
}

#[derive(Default)]
pub struct LevelMeters {
    pub hearing: f32,
    pub sending: f32,
}

impl LevelMeters {
    pub fn update(&mut self, soundboard: &Soundboard, seconds_since_last_frame: f32) {
        let (hearing_peak, sending_peak) =
            soundboard.session().map(|session| (session.hearing.take(), session.sending.take())).unwrap_or_default();
        let fall = seconds_since_last_frame.min(0.1) * METER_FALL_PER_SECOND;
        self.hearing = meter_level(hearing_peak).max(self.hearing - fall);
        self.sending = meter_level(sending_peak).max(self.sending - fall);
    }
}
