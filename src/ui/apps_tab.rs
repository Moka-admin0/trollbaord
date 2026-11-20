use super::theme;
use super::{TrollApp, widgets};
use crate::app_audio::AudioApp;
use crate::engine::Route;
use eframe::egui::{self, Frame, Label, Margin, RichText, Sense, Ui, vec2};

impl TrollApp {
    pub(super) fn apps_tab(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        egui::Sides::new().show(
            ui,
            |ui| widgets::heading(ui, "Send app sound to your mic"),
            |ui| widgets::volume_slider(ui, &mut self.settings.app_volume, 1.5, "App volume"),
        );
        widgets::hint(ui, "Friends hear the app through your mic. You still hear it normally.");
        if self.settings.devices.route() == Route::GoXlr {
            widgets::hint(
                ui,
                "GoXLR also plays Samples in your headphones, so you'll hear shared apps twice. \
                 To avoid that, untick Samples → Headphones in the GoXLR app and turn on \
                 “Hear clips myself” in Setup.",
            );
        }
        ui.add_space(6.0);
        self.app_list(ui);
    }

    fn app_list(&mut self, ui: &mut Ui) {
        let apps = self.app_watcher.apps();
        let live = self.live_audio.is_running();
        let mut toggled = None;

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if apps.is_empty() {
                ui.add_space(30.0);
                let start_something = "Start some music or a video and it shows up here.";
                widgets::empty_state(ui, "🎵", "No apps are playing sound", &[start_something]);
                return;
            }
            for app in &apps {
                let shared = self.settings.shared_apps.contains(&app.exe);
                if let Some(now_shared) = app_row(ui, app, shared, live) {
                    toggled = Some((app.exe.clone(), now_shared));
                }
            }
            ui.add_space(4.0);
            widgets::hint(ui, "Music cutting out for friends? Set Discord's Noise Suppression to None.");
        });

        if let Some((exe, shared)) = toggled {
            self.settings.set_app_shared(&exe, shared);
        }
    }
}

fn app_row(ui: &mut Ui, app: &AudioApp, shared: bool, live: bool) -> Option<bool> {
    let mut switched_on = shared;
    let fill = if app.sending && app.is_playing() { theme::ACCENT_SOFT } else { theme::SURFACE };

    Frame::new().fill(fill).corner_radius(10).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Sides::new().shrink_left().show(
            ui,
            |ui| {
                let tooltip = if shared { "Stop sending this app" } else { "Send this app through your mic" };
                ui.add_enabled(app.blocked_reason.is_none(), widgets::switch(&mut switched_on))
                    .on_hover_text(tooltip)
                    .on_disabled_hover_text(app.blocked_reason.unwrap_or_default());

                let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                let dot_color = if app.is_playing() { theme::GREEN } else { theme::SURFACE_RAISED };
                ui.painter().circle_filled(dot.center(), 4.0, dot_color);

                let name_color = if app.blocked_reason.is_some() { theme::TEXT_MUTED } else { theme::TEXT };
                ui.add(Label::new(RichText::new(&app.name).color(name_color)).truncate()).on_hover_text(&app.exe);
            },
            |ui| {
                let (text, color, tooltip) = app_state(app, shared, live);
                let label = ui.label(RichText::new(text).small().color(color));
                if !tooltip.is_empty() {
                    label.on_hover_text(tooltip);
                }
            },
        );
    });

    (switched_on != shared).then_some(switched_on)
}

fn app_state(app: &AudioApp, shared: bool, live: bool) -> (&str, egui::Color32, &str) {
    if let Some(error) = &app.error {
        return ("Couldn't capture", theme::RED, error);
    }
    if let Some(reason) = app.blocked_reason {
        return ("Can't send", theme::TEXT_MUTED, reason);
    }
    match (shared, live, app.sending, app.is_playing()) {
        (true, false, ..) => ("Paused", theme::TEXT_MUTED, "Switch Live on to send it."),
        (true, true, true, true) => ("Sending", theme::GREEN, "Friends can hear this app."),
        (true, true, true, false) => ("On · silent", theme::TEXT_MUTED, "Sends as soon as it plays something."),
        (true, true, false, _) => ("Starting…", theme::TEXT_MUTED, ""),
        (false, _, _, true) => ("Playing", theme::TEXT_MUTED, "Only you hear it."),
        (false, _, _, false) => ("Idle", theme::TEXT_MUTED, "Only you hear it."),
    }
}
