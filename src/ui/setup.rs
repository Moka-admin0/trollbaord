use super::theme::{self, semibold};
use super::{TrollApp, widgets};
use crate::audio::DeviceList;
use crate::engine::{DeviceConfig, Route};
use eframe::egui::{
    self, Align, Button, Color32, Frame, Grid, Id, Layout, Margin, RichText, Slider, TextEdit, Ui, vec2,
};

const VB_CABLE_URL: &str = "https://vb-audio.com/Cable/";

impl TrollApp {
    pub(super) fn setup_dialog(&mut self, ctx: &egui::Context) {
        let window = ctx.content_rect();
        let width = (window.width() - 80.0).min(470.0);
        let scroll_height = (window.height() - 190.0).max(160.0);
        let frame = Frame::new().fill(theme::SURFACE).corner_radius(14).inner_margin(Margin::same(22));

        let mut done = false;
        let modal = egui::Modal::new(Id::unique("setup")).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            ui.label(semibold("Setup").size(21.0).color(theme::TEXT));
            ui.add_space(2.0);
            egui::ScrollArea::vertical().max_height(scroll_height).auto_shrink([false, true]).show(ui, |ui| {
                self.routing_help(ui);
                self.audio_section(ui, (width - 150.0).clamp(150.0, 290.0));
                self.hotkey_section(ui);
            });
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let button =
                    Button::new(semibold("Done").color(Color32::WHITE)).fill(theme::ACCENT).min_size(vec2(90.0, 32.0));
                done = ui.add(button).clicked();
            });
        });

        if done || modal.should_close() {
            self.setup_open = false;
            self.settings.setup_done = true;
        }
    }

    fn routing_help(&self, ui: &mut Ui) {
        let (text, link) = routing_instructions(&self.settings.devices, &self.devices);
        Frame::new().fill(theme::ACCENT_SOFT).corner_radius(8).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).color(theme::TEXT));
            if let Some((label, url)) = link {
                ui.hyperlink_to(label, url);
            }
        });
    }

    fn audio_section(&mut self, ui: &mut Ui, picker_width: f32) {
        widgets::section_title(ui, "Audio");
        let devices = &mut self.settings.devices;
        let (outputs, inputs) = (&self.devices.outputs, &self.devices.inputs);

        Grid::new("devices").num_columns(2).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
            ui.label("Record from");
            widgets::device_picker(ui, "record_from", picker_width, &mut devices.record_from, outputs);
            ui.end_row();
            hint_row(ui, "Where you hear your friends in Discord.");

            ui.label("Send clips to");
            widgets::device_picker(ui, "send_to", picker_width, &mut devices.send_to, outputs);
            ui.end_row();
            hint_row(ui, "The virtual mic Discord listens to.");

            ui.checkbox(&mut devices.forward_mic, "Forward my mic");
            ui.add_enabled_ui(devices.forward_mic, |ui| {
                widgets::device_picker(ui, "mic", picker_width, &mut devices.mic, inputs);
            });
            ui.end_row();
            hint_row(ui, "Needed with VB-CABLE so friends still hear you.");

            if devices.forward_mic {
                ui.label("Mic volume");
                ui.add(Slider::new(&mut self.settings.mic_volume, 0.0..=2.0).show_value(false));
                ui.end_row();
            }

            ui.checkbox(&mut devices.hear_clips, "Hear clips myself");
            widgets::hint(ui, "Plays clips on “Record from” too.");
            ui.end_row();
        });

        ui.horizontal(|ui| {
            if ui.button("✨  Auto-detect").on_hover_text("Look for a GoXLR or VB-CABLE").clicked() {
                match DeviceConfig::detect(&self.devices) {
                    Some(detected) => self.settings.devices = detected,
                    None => self.soundboard.show_error("No GoXLR or VB-CABLE found."),
                }
            }
            if ui.button("↻  Refresh devices").clicked() {
                self.devices = DeviceList::scan();
                self.live_audio.retry();
            }
        });
    }

    fn hotkey_section(&mut self, ui: &mut Ui) {
        widgets::section_title(ui, "Hotkeys");
        let keys = &mut self.settings.keys;

        Grid::new("hotkeys").num_columns(2).spacing(vec2(16.0, 6.0)).show(ui, |ui| {
            for (label, value) in
                [("Clip", &mut keys.clip), ("Random clip", &mut keys.random), ("Stop", &mut keys.stop)]
            {
                ui.label(label);
                ui.add(TextEdit::singleline(value).desired_width(150.0));
                ui.end_row();
            }
            ui.label("Play clip 1–9");
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut keys.slot_mods).desired_width(150.0));
                ui.label(RichText::new("+ 1 … 9").color(theme::TEXT_MUTED));
            });
            ui.end_row();
        });

        match keys.bindings() {
            Ok(_) => widgets::hint(ui, "e.g. F9, ctrl+shift+c, numpad5. Applied when you close Setup."),
            Err(error) => {
                ui.label(RichText::new(format!("{error:#}")).small().color(theme::RED));
            }
        }
    }
}

fn hint_row(ui: &mut Ui, text: &str) {
    ui.label("");
    widgets::hint(ui, text);
    ui.end_row();
}

fn routing_instructions(
    config: &DeviceConfig,
    devices: &DeviceList,
) -> (&'static str, Option<(&'static str, &'static str)>) {
    let has_output = |word: &str| devices.outputs.iter().any(|name| name.to_lowercase().contains(word));
    match config.route() {
        Route::GoXlr => (
            "GoXLR: in the GoXLR app → Routing, tick Samples → Chat Mic. \
             In Discord set Output to “Chat” and Input to “Chat Mic”.",
            None,
        ),
        Route::VbCable => {
            ("In Discord → Voice & Video, set Input Device to “CABLE Output”. Keep Output on your headphones.", None)
        }
        Route::Other if !has_output("goxlr") && !has_output("cable input") => (
            "No virtual mic found. Install VB-CABLE (free), restart your PC, then press Refresh devices.",
            Some(("Get VB-CABLE", VB_CABLE_URL)),
        ),
        Route::Other => ("Pick a virtual mic under “Send clips to”, then set Discord's input to its other end.", None),
    }
}
