use super::theme::{self, semibold};
use super::{TrollApp, widgets};
use crate::clips::Clip;
use crate::engine::MAX_CLIP_SECONDS;
use crate::hotkeys::{SLOT_COUNT, format_hotkey};
use anyhow::Result;
use eframe::egui::{
    self, Button, Color32, Frame, Key, Label, Margin, Rect, RichText, Slider, TextEdit, Ui, pos2, vec2,
};
use std::path::Path;

pub enum ClipEdit {
    Renaming { file_name: String, text: String, focused: bool },
    ConfirmingDelete { file_name: String },
}

enum ClipAction {
    Play(usize),
    Stop,
    StartRename(Clip),
    SaveRename,
    CancelEdit,
    Move { from: usize, to: usize },
    AskToDelete(Clip),
    Delete,
}

impl TrollApp {
    pub(super) fn clips_tab(&mut self, ui: &mut Ui) {
        self.clip_button(ui);
        self.clip_length_slider(ui);
        ui.add_space(10.0);
        self.clip_list(ui);
    }

    fn clip_button(&mut self, ui: &mut Ui) {
        let seconds = self.settings.clip_seconds.round();
        let label = semibold(format!("✂   Clip the last {seconds} seconds")).size(17.0).color(Color32::WHITE);
        let hotkey =
            RichText::new(format_hotkey(&self.settings.keys.clip)).size(13.0).color(Color32::from_white_alpha(170));
        let button = Button::new(label)
            .right_text(hotkey)
            .fill(theme::ACCENT)
            .corner_radius(10)
            .min_size(vec2(ui.available_width(), 52.0));

        let tooltip = "Saves what you just heard as a new clip.\nThe hotkey works from any app, even in-game.";
        if ui.add_enabled(self.live_audio.is_running(), button).on_hover_text(tooltip).clicked() {
            self.soundboard.clip_now();
        }
    }

    fn clip_length_slider(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Length").color(theme::TEXT_MUTED));
            ui.spacing_mut().slider_width = (ui.available_width() - 80.0).max(60.0);
            let slider = Slider::new(&mut self.settings.clip_seconds, 2.0..=MAX_CLIP_SECONDS)
                .step_by(1.0)
                .fixed_decimals(0)
                .suffix(" s");
            ui.add(slider);
        });
    }

    fn clip_list(&mut self, ui: &mut Ui) {
        let clips = self.soundboard.library.clips();
        let folder = self.soundboard.library.folder().to_path_buf();
        egui::Sides::new().show(
            ui,
            |ui| {
                widgets::heading(ui, "Clips");
                ui.label(RichText::new(clips.len().to_string()).color(theme::TEXT_MUTED));
            },
            |ui| {
                if ui.button("📂  Open folder").on_hover_text(folder.display().to_string()).clicked() {
                    open_in_explorer(&folder);
                }
            },
        );

        let now_playing = self.soundboard.now_playing();
        let scroll = egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if clips.is_empty() {
                ui.add_space(36.0);
                let press_hotkey =
                    format!("Press {} right after a friend says something,", format_hotkey(&self.settings.keys.clip));
                widgets::empty_state(ui, "🎙", "No clips yet", &[&press_hotkey, "or drag .wav files into this window."]);
            }

            let mut action = None;
            for (index, clip) in clips.iter().enumerate() {
                let progress =
                    now_playing.as_ref().filter(|playing| playing.name == clip.name).map(|playing| playing.progress);
                if let Some(row_action) = self.clip_row(ui, index, clip, clips.len(), progress) {
                    action = Some(row_action);
                }
            }
            action
        });

        if let Some(action) = scroll.inner {
            self.apply(action);
        }
    }

    fn clip_row(
        &mut self,
        ui: &mut Ui,
        index: usize,
        clip: &Clip,
        clip_count: usize,
        progress: Option<f32>,
    ) -> Option<ClipAction> {
        let fill = if progress.is_some() { theme::ACCENT_SOFT } else { theme::SURFACE };
        let row_frame = Frame::new().fill(fill).corner_radius(10).inner_margin(Margin::symmetric(8, 6));

        let row = row_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            match &mut self.clip_edit {
                Some(ClipEdit::Renaming { file_name, text, focused }) if *file_name == clip.file_name => {
                    rename_row(ui, index, text, focused)
                }
                Some(ClipEdit::ConfirmingDelete { file_name }) if *file_name == clip.file_name => {
                    delete_confirmation_row(ui, index, clip)
                }
                _ => {
                    let play_hotkey = (index < SLOT_COUNT).then(|| self.settings.keys.slot_label(index));
                    let live = self.live_audio.is_running();
                    clip_row(ui, index, clip, clip_count, progress, play_hotkey, live)
                }
            }
        });

        if let Some(progress) = progress {
            paint_progress_bar(ui, row.response.rect, progress);
        }
        row.inner
    }

    fn apply(&mut self, action: ClipAction) {
        if let Err(error) = self.try_apply(action) {
            self.soundboard.show_error(format!("{error:#}"));
        }
    }

    fn try_apply(&mut self, action: ClipAction) -> Result<()> {
        let library = &self.soundboard.library;
        let index_of = |file_name: &str| library.clips().iter().position(|clip| clip.file_name == file_name);

        match action {
            ClipAction::Play(index) => self.soundboard.play(index),
            ClipAction::Stop => self.soundboard.stop(),
            ClipAction::StartRename(clip) => {
                self.clip_edit =
                    Some(ClipEdit::Renaming { file_name: clip.file_name, text: clip.name, focused: false });
            }
            ClipAction::SaveRename => {
                if let Some(ClipEdit::Renaming { file_name, text, .. }) = self.clip_edit.take()
                    && let Some(index) = index_of(&file_name)
                    && let Err(error) = library.rename(index, &text)
                {
                    self.clip_edit = Some(ClipEdit::Renaming { file_name, text, focused: false });
                    return Err(error);
                }
            }
            ClipAction::CancelEdit => self.clip_edit = None,
            ClipAction::Move { from, to } => library.move_clip(from, to)?,
            ClipAction::AskToDelete(clip) => {
                self.clip_edit = Some(ClipEdit::ConfirmingDelete { file_name: clip.file_name });
            }
            ClipAction::Delete => {
                if let Some(ClipEdit::ConfirmingDelete { file_name }) = self.clip_edit.take()
                    && let Some(index) = index_of(&file_name)
                {
                    library.delete(index)?;
                    let name = Path::new(&file_name).file_stem().unwrap_or_default().to_string_lossy();
                    self.soundboard.show_info(format!("Moved \"{name}\" to the Recycle Bin"));
                }
            }
        }
        Ok(())
    }
}

fn clip_row(
    ui: &mut Ui,
    index: usize,
    clip: &Clip,
    clip_count: usize,
    progress: Option<f32>,
    play_hotkey: Option<String>,
    live: bool,
) -> Option<ClipAction> {
    let (_, action) = egui::Sides::new().shrink_left().show(
        ui,
        |ui| {
            widgets::slot_badge(ui, index, index < SLOT_COUNT);
            ui.add(Label::new(RichText::new(&clip.name).color(theme::TEXT)).truncate());
        },
        |ui| {
            let menu_action = clip_menu(ui, index, clip, clip_count);
            let play_action = play_button(ui, index, progress, play_hotkey, live);
            ui.label(RichText::new(format!("{:.1} s", clip.seconds)).small().color(theme::TEXT_MUTED));
            menu_action.or(play_action)
        },
    );
    action
}

fn clip_menu(ui: &mut Ui, index: usize, clip: &Clip, clip_count: usize) -> Option<ClipAction> {
    let menu = ui.menu_button(RichText::new("•••").size(11.0).color(theme::TEXT_MUTED), |ui| {
        if ui.button("✏  Rename").clicked() {
            return Some(ClipAction::StartRename(clip.clone()));
        }
        if index > 0 && ui.button("⬆  Move up").clicked() {
            return Some(ClipAction::Move { from: index, to: index - 1 });
        }
        if index + 1 < clip_count && ui.button("⬇  Move down").clicked() {
            return Some(ClipAction::Move { from: index, to: index + 1 });
        }
        ui.separator();
        if ui.button(RichText::new("🗑  Delete").color(theme::RED)).clicked() {
            return Some(ClipAction::AskToDelete(clip.clone()));
        }
        None
    });
    menu.inner.flatten()
}

fn play_button(
    ui: &mut Ui,
    index: usize,
    progress: Option<f32>,
    play_hotkey: Option<String>,
    live: bool,
) -> Option<ClipAction> {
    let playing = progress.is_some();
    let (icon, fill) = if playing { ("⏹", theme::ACCENT) } else { ("▶", theme::SURFACE_RAISED) };
    let tooltip = match (playing, play_hotkey) {
        (true, _) => "Stop".to_owned(),
        (false, Some(hotkey)) => format!("Play  ·  {hotkey}"),
        (false, None) => "Play".to_owned(),
    };

    let button = Button::new(RichText::new(icon).size(14.0)).fill(fill).min_size(vec2(36.0, 28.0));
    let clicked = ui.add_enabled(live, button).on_hover_text(tooltip).clicked();
    let action = if playing { ClipAction::Stop } else { ClipAction::Play(index) };
    clicked.then_some(action)
}

fn rename_row(ui: &mut Ui, index: usize, text: &mut String, focused: &mut bool) -> Option<ClipAction> {
    let (typed_action, button_action) = egui::Sides::new().shrink_left().show(
        ui,
        |ui| {
            widgets::slot_badge(ui, index, index < SLOT_COUNT);
            let response =
                ui.add(TextEdit::singleline(text).hint_text("Clip name").desired_width(ui.available_width()));
            if !*focused {
                response.request_focus();
                *focused = true;
            }
            if !response.lost_focus() {
                return None;
            }
            if ui.input(|input| input.key_pressed(Key::Enter)) {
                Some(ClipAction::SaveRename)
            } else if ui.input(|input| input.key_pressed(Key::Escape)) {
                Some(ClipAction::CancelEdit)
            } else {
                None
            }
        },
        |ui| {
            if ui.button("✖").on_hover_text("Cancel (Esc)").clicked() {
                return Some(ClipAction::CancelEdit);
            }
            let save = Button::new("✔").fill(theme::ACCENT);
            ui.add(save).on_hover_text("Save (Enter)").clicked().then_some(ClipAction::SaveRename)
        },
    );
    typed_action.or(button_action)
}

fn delete_confirmation_row(ui: &mut Ui, index: usize, clip: &Clip) -> Option<ClipAction> {
    let (_, action) = egui::Sides::new().shrink_left().show(
        ui,
        |ui| {
            widgets::slot_badge(ui, index, index < SLOT_COUNT);
            let question = RichText::new(format!("Delete \"{}\"?", clip.name)).color(theme::TEXT);
            ui.add(Label::new(question).truncate());
        },
        |ui| {
            let delete = Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(theme::RED_STRONG);
            if ui.add(delete).on_hover_text("Moves the file to the Recycle Bin").clicked() {
                return Some(ClipAction::Delete);
            }
            ui.button("Cancel").clicked().then_some(ClipAction::CancelEdit)
        },
    );
    action
}

fn paint_progress_bar(ui: &Ui, row: Rect, progress: f32) {
    let width = (row.width() - 20.0) * progress.clamp(0.0, 1.0);
    let bar = Rect::from_min_size(pos2(row.left() + 10.0, row.bottom() - 3.0), vec2(width, 2.0));
    ui.painter().rect_filled(bar, 1.0, theme::ACCENT);
}

fn open_in_explorer(folder: &Path) {
    let _ = std::fs::create_dir_all(folder);
    let _ = std::process::Command::new("explorer").arg(folder).spawn();
}
