use super::theme::{self, semibold};
use super::{TrollApp, widgets, window_frame};
use eframe::egui::{self, Button, RichText, Sense};

impl TrollApp {
    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().interaction.selectable_labels = false;

        egui::Sides::new().show(
            ui,
            |ui| {
                ui.label(semibold("trollboard").size(24.0).color(theme::TEXT));
            },
            |ui| {
                self.window_buttons_slot = ui.allocate_exact_size(window_frame::CONTROLS_SIZE, Sense::hover()).0;
                widgets::vertical_divider(ui);
                if ui.button("⚙  Setup").clicked() {
                    self.setup_open = true;
                }
                self.live_toggle(ui);
            },
        );

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            widgets::level_meter(ui, "Hearing", self.meters.hearing);
            ui.add_space(18.0);
            widgets::level_meter(ui, "Sending", self.meters.sending);
        });
    }

    fn live_toggle(&mut self, ui: &mut egui::Ui) {
        let (text, color, fill, tooltip) = match (self.settings.live, self.live_audio.is_running()) {
            (true, true) => {
                ("●  Live", theme::GREEN, theme::GREEN_SOFT, "Listening. Click to pause and release the audio devices.")
            }
            (true, false) => (
                "●  Not working",
                theme::RED,
                theme::SURFACE_RAISED,
                "Something's wrong with the audio setup. See the message below.",
            ),
            (false, _) => ("○  Paused", theme::TEXT_MUTED, theme::SURFACE_RAISED, "Click to start listening."),
        };

        let button = Button::new(RichText::new(text).color(color)).fill(fill).corner_radius(14);
        if ui.add(button).on_hover_text(tooltip).clicked() {
            self.settings.live = !self.settings.live;
        }
    }
}
