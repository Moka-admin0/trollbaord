use super::theme::{self, semibold};
use eframe::egui::{self, Align2, Color32, Frame, Margin, Response, RichText, Sense, Stroke, Ui, Widget, pos2, vec2};

const METER_FLOOR_DB: f32 = -48.0;

pub fn meter_level(peak: f32) -> f32 {
    if peak <= 1e-4 {
        return 0.0;
    }
    let decibels = 20.0 * peak.log10();
    ((decibels - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

pub fn level_meter(ui: &mut Ui, label: &str, level: f32) {
    ui.label(RichText::new(label).small().color(theme::TEXT_MUTED));
    let (rect, _) = ui.allocate_exact_size(vec2(120.0, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 3.0, theme::SURFACE_RAISED);
    if level <= 0.0 {
        return;
    }
    let color = match level {
        level if level > 0.92 => theme::RED,
        level if level > 0.75 => theme::YELLOW,
        _ => theme::GREEN,
    };
    let mut filled = rect;
    filled.set_width(rect.width() * level);
    ui.painter().rect_filled(filled, 3.0, color);
}

pub fn vertical_divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, theme::SURFACE_RAISED));
}

pub fn slot_badge(ui: &mut Ui, index: usize, has_hotkey: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
    let (text, color) =
        if has_hotkey { ((index + 1).to_string(), theme::TEXT) } else { ("·".to_owned(), theme::TEXT_MUTED) };
    ui.painter().rect_filled(rect, 7.0, theme::SURFACE_RAISED);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, theme::semibold_font(13.0), color);
}

pub fn banner(ui: &mut Ui, color: Color32, text: &str, buttons: &[&str]) -> Option<usize> {
    let mut clicked = None;
    Frame::new()
        .fill(color.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(text).color(theme::TEXT));
                for (index, label) in buttons.iter().enumerate() {
                    if ui.small_button(*label).clicked() {
                        clicked = Some(index);
                    }
                }
            });
        });
    clicked
}

pub fn empty_state(ui: &mut Ui, icon: &str, title: &str, lines: &[&str]) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon).size(38.0).color(theme::TEXT_MUTED));
        ui.label(semibold(title).size(16.0).color(theme::TEXT));
        for line in lines {
            ui.label(RichText::new(*line).color(theme::TEXT_MUTED));
        }
    });
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(semibold(text).size(17.0).color(theme::TEXT));
}

pub fn section_title(ui: &mut Ui, title: &str) {
    ui.add_space(12.0);
    ui.label(semibold(title).size(15.0).color(theme::TEXT));
    ui.add_space(2.0);
}

pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(theme::TEXT_MUTED));
}

pub fn volume_slider(ui: &mut Ui, volume: &mut f32, max: f32, tooltip: &str) {
    ui.spacing_mut().slider_width = 90.0;
    let percent = *volume * 100.0;
    ui.add(egui::Slider::new(volume, 0.0..=max).show_value(false)).on_hover_text(format!("{tooltip}: {percent:.0}%"));
    ui.label(RichText::new("🔊").color(theme::TEXT_MUTED));
}

pub fn device_picker(ui: &mut Ui, id: &str, width: f32, selected: &mut String, devices: &[String]) {
    let shown = if selected.is_empty() {
        "Choose…".to_owned()
    } else if devices.contains(selected) {
        selected.clone()
    } else {
        format!("{selected} (not connected)")
    };
    egui::ComboBox::from_id_salt(id).width(width).truncate().selected_text(shown).show_ui(ui, |ui| {
        for device in devices {
            ui.selectable_value(selected, device.clone(), device.as_str());
        }
    });
}

pub fn switch(on: &mut bool) -> impl Widget + '_ {
    move |ui: &mut Ui| -> Response {
        let (rect, mut response) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        if ui.is_rect_visible(rect) {
            let position = ui.ctx().animate_bool_responsive(response.id, *on);
            let (mut track, mut knob) = (theme::SURFACE_RAISED.lerp_to_gamma(theme::ACCENT, position), Color32::WHITE);
            if !ui.is_enabled() {
                track = track.gamma_multiply(0.5);
                knob = theme::TEXT_MUTED;
            }
            let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), position);
            ui.painter().rect_filled(rect, 10.0, track);
            ui.painter().circle_filled(pos2(knob_x, rect.center().y), 7.0, knob);
        }
        response
    }
}
