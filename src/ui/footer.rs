use super::theme;
use super::{TrollApp, widgets};
use crate::hotkeys::format_hotkey;
use eframe::egui::{Align, Button, Label, Layout, RichText, Ui};
use std::time::Duration;

const STATUS_DURATION: Duration = Duration::from_secs(8);

impl TrollApp {
    pub(super) fn footer(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            self.playback_buttons(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::volume_slider(ui, &mut self.settings.clip_volume, 2.0, "Clip volume");
            });
        });
        self.status_line(ui);
    }

    fn playback_buttons(&mut self, ui: &mut Ui) {
        let live = self.live_audio.is_running();
        let playing = self.soundboard.now_playing().is_some();
        let has_clips = self.soundboard.library.len() > 0;
        let keys = &self.settings.keys;
        let shortcut = |spec: &str| RichText::new(format_hotkey(spec)).small().color(theme::TEXT_MUTED);

        let stop = Button::new("⏹  Stop").right_text(shortcut(&keys.stop));
        if ui.add_enabled(live && playing, stop).clicked() {
            self.soundboard.stop();
        }
        let random = Button::new("🎲  Random").right_text(shortcut(&keys.random));
        if ui.add_enabled(live && has_clips, random).clicked() {
            self.soundboard.play_random();
        }
    }

    fn status_line(&self, ui: &mut Ui) {
        let status = self.soundboard.status().filter(|status| status.shown_at.elapsed() < STATUS_DURATION);
        let (text, color) = match status {
            Some(status) => (status.text, if status.is_error { theme::RED } else { theme::GREEN }),
            None => (self.hotkey_summary(), theme::TEXT_MUTED),
        };
        ui.add(Label::new(RichText::new(text).small().color(color)).truncate());
    }

    fn hotkey_summary(&self) -> String {
        let keys = &self.settings.keys;
        format!(
            "{} clip  ·  {}+1–9 play  ·  {} random  ·  {} stop",
            format_hotkey(&keys.clip),
            format_hotkey(&keys.slot_mods),
            format_hotkey(&keys.random),
            format_hotkey(&keys.stop),
        )
    }
}
