use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, TextStyle, vec2};
use std::path::PathBuf;
use std::sync::Arc;

pub const ACCENT: Color32 = Color32::from_rgb(0x8B, 0x5C, 0xF6);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0x2B, 0x22, 0x47);
pub const LINK: Color32 = Color32::from_rgb(0xB4, 0x9C, 0xFF);
pub const GREEN: Color32 = Color32::from_rgb(0x4A, 0xDE, 0x80);
pub const GREEN_SOFT: Color32 = Color32::from_rgb(0x15, 0x33, 0x22);
pub const YELLOW: Color32 = Color32::from_rgb(0xFA, 0xCC, 0x15);
pub const RED: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);
pub const RED_STRONG: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
pub const RED_PRESSED: Color32 = Color32::from_rgb(0xB9, 0x1C, 0x1C);

pub const BACKGROUND: Color32 = Color32::from_rgb(0x13, 0x13, 0x18);
pub const INPUT_BACKGROUND: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x12);
pub const SURFACE: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x24);
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(0x28, 0x28, 0x33);
pub const SURFACE_HOVERED: Color32 = Color32::from_rgb(0x33, 0x33, 0x41);
pub const SURFACE_PRESSED: Color32 = Color32::from_rgb(0x3D, 0x3D, 0x4D);
pub const TEXT: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xEE);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x8E, 0x8E, 0x9C);

const SEMIBOLD: &str = "semibold";

pub fn semibold(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Name(SEMIBOLD.into()))
}

pub fn semibold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn apply(ctx: &egui::Context) {
    load_fonts(ctx);
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, semibold_font(20.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Small, FontId::proportional(12.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.interact_size.y = 28.0;

        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = SURFACE;
        visuals.extreme_bg_color = INPUT_BACKGROUND;
        visuals.faint_bg_color = SURFACE;
        visuals.window_corner_radius = 12.into();
        visuals.window_stroke = Stroke::new(1.0, SURFACE_RAISED);
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.hyperlink_color = LINK;
        visuals.weak_text_color = Some(TEXT_MUTED);

        visuals.widgets.noninteractive.fg_stroke.color = TEXT;
        visuals.widgets.noninteractive.bg_stroke.color = SURFACE_RAISED;
        let widget_states = [
            (&mut visuals.widgets.inactive, SURFACE_RAISED, TEXT),
            (&mut visuals.widgets.hovered, SURFACE_HOVERED, Color32::WHITE),
            (&mut visuals.widgets.active, SURFACE_PRESSED, Color32::WHITE),
            (&mut visuals.widgets.open, SURFACE_RAISED, TEXT),
        ];
        for (widget, fill, text) in widget_states {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = Stroke::NONE;
            widget.fg_stroke.color = text;
            widget.corner_radius = 8.into();
        }
    });
}

fn load_fonts(ctx: &egui::Context) {
    let fonts_folder = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let mut fonts = FontDefinitions::default();
    let mut regular = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut semibold = regular.clone();

    let mut load = |name: &str, file: &str| {
        let bytes = std::fs::read(fonts_folder.join(file)).ok()?;
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
        Some(name.to_owned())
    };

    if let Some(segoe) = load("segoe", "segoeui.ttf") {
        regular.insert(0, segoe.clone());
        semibold.insert(0, segoe);
    }
    if let Some(segoe_semibold) = load("segoe-semibold", "seguisb.ttf") {
        semibold.insert(0, segoe_semibold);
    }
    if let Some(symbols) = load("segoe-symbol", "seguisym.ttf") {
        for family in [&mut regular, &mut semibold] {
            let after_text_fonts = family.iter().rposition(|font| font.starts_with("segoe")).map_or(0, |i| i + 1);
            family.insert(after_text_fonts, symbols.clone());
        }
    }

    fonts.families.insert(FontFamily::Proportional, regular);
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    ctx.set_fonts(fonts);
}
