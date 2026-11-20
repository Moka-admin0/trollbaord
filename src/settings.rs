use crate::engine::DeviceConfig;
use crate::hotkeys::KeyConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub devices: DeviceConfig,
    pub keys: KeyConfig,
    pub clip_seconds: f32,
    pub clip_volume: f32,
    pub mic_volume: f32,
    pub app_volume: f32,
    pub shared_apps: BTreeSet<String>,
    pub tab: Tab,
    pub live: bool,
    #[serde(alias = "set_up")]
    pub setup_done: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Clips,
    Apps,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            devices: DeviceConfig::default(),
            keys: KeyConfig::default(),
            clip_seconds: 8.0,
            clip_volume: 1.0,
            mic_volume: 1.0,
            app_volume: 0.5,
            shared_apps: BTreeSet::new(),
            tab: Tab::Clips,
            live: true,
            setup_done: false,
        }
    }
}

impl Settings {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage.and_then(|storage| eframe::get_value(storage, eframe::APP_KEY)).unwrap_or_default()
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    pub fn set_app_shared(&mut self, exe: &str, shared: bool) {
        if shared {
            self.shared_apps.insert(exe.to_owned());
        } else {
            self.shared_apps.remove(exe);
        }
    }
}
