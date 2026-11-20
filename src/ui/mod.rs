mod apps_tab;
mod clips_tab;
mod footer;
mod header;
mod live;
mod setup;
mod theme;
mod widgets;
mod window_frame;

use crate::app_audio::AppWatcher;
use crate::audio::DeviceList;
use crate::clips::{self, Library};
use crate::engine::{DeviceConfig, Soundboard};
use crate::settings::{Settings, Tab};
use clips_tab::ClipEdit;
use eframe::egui::{self, Align2, Color32, Frame, Id, LayerId, Margin, Order, Rect};
use live::{ActiveHotkeys, LevelMeters, LiveAudio};
use std::sync::Arc;
use std::time::{Duration, Instant};

const LIBRARY_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const LIVE_REPAINT_INTERVAL: Duration = Duration::from_millis(40);
const IDLE_REPAINT_INTERVAL: Duration = Duration::from_millis(500);

pub struct TrollApp {
    soundboard: Arc<Soundboard>,
    settings: Settings,
    devices: DeviceList,
    live_audio: LiveAudio,
    hotkeys: ActiveHotkeys,
    app_watcher: AppWatcher,
    meters: LevelMeters,
    setup_open: bool,
    clip_edit: Option<ClipEdit>,
    last_library_refresh: Instant,
    title_bar: Rect,
    window_buttons_slot: Rect,
}

impl TrollApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open_setup: bool) -> Self {
        theme::apply(&cc.egui_ctx);
        let devices = DeviceList::scan();
        let mut settings = Settings::load(cc.storage);
        if !settings.setup_done
            && let Some(detected) = DeviceConfig::detect(&devices)
        {
            settings.devices = detected;
            settings.setup_done = true;
        }

        let soundboard = Arc::new(Soundboard::new(cc.egui_ctx.clone(), Library::open(clips::default_folder())));
        Self {
            app_watcher: AppWatcher::spawn(soundboard.clone()),
            soundboard,
            setup_open: open_setup || !settings.setup_done,
            settings,
            devices,
            live_audio: LiveAudio::default(),
            hotkeys: ActiveHotkeys::default(),
            meters: LevelMeters::default(),
            clip_edit: None,
            last_library_refresh: Instant::now(),
            title_bar: Rect::NOTHING,
            window_buttons_slot: Rect::NOTHING,
        }
    }

    fn update_services(&mut self, ctx: &egui::Context) {
        self.soundboard.clip_volume.set(self.settings.clip_volume);
        self.soundboard.mic_volume.set(self.settings.mic_volume);
        self.soundboard.app_volume.set(self.settings.app_volume);
        self.soundboard.clip_seconds.set(self.settings.clip_seconds);
        self.app_watcher.set_enabled(&self.settings.shared_apps);
        self.live_audio.update(self.settings.live, &self.settings.devices, &self.soundboard);
        if !self.setup_open {
            self.hotkeys.update(&self.settings.keys, &self.soundboard);
        }
        if self.last_library_refresh.elapsed() > LIBRARY_REFRESH_INTERVAL {
            self.soundboard.library.refresh();
            self.last_library_refresh = Instant::now();
        }
        self.import_dropped_files(ctx);
        self.meters.update(&self.soundboard, ctx.input(|input| input.stable_dt));
    }

    fn import_dropped_files(&self, ctx: &egui::Context) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        for file in &dropped {
            let path = file.path();
            match self.soundboard.library.import(path) {
                Ok(clip) => self.soundboard.show_info(format!("Added \"{}\"", clip.name)),
                Err(error) => {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    self.soundboard.show_error(format!("Couldn't add {file_name}: {error:#}"));
                }
            }
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        self.banners(ui);
        ui.add_space(2.0);
        self.tab_bar(ui);
        ui.add_space(4.0);
        match self.settings.tab {
            Tab::Clips => self.clips_tab(ui),
            Tab::Apps => self.apps_tab(ui),
        }
    }

    fn banners(&mut self, ui: &mut egui::Ui) {
        if !self.settings.live {
            let paused = "Paused. Nothing is recorded and clips can't play.";
            if widgets::banner(ui, theme::TEXT_MUTED, paused, &["Go live"]).is_some() {
                self.settings.live = true;
            }
        } else if let Some(error) = self.live_audio.error().map(str::to_owned) {
            match widgets::banner(ui, theme::RED, &format!("⚠  {error}"), &["Setup", "Retry"]) {
                Some(0) => self.setup_open = true,
                Some(_) => self.live_audio.retry(),
                None => {}
            }
        }

        if let Some(problem) = self.hotkeys.problem().map(str::to_owned)
            && widgets::banner(ui, theme::YELLOW, &format!("⌨  {problem}"), &["Setup"]).is_some()
        {
            self.setup_open = true;
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let sending = self.app_watcher.apps().iter().filter(|app| app.sending).count();
        let apps_label = match sending {
            0 => "🎵  Apps".to_owned(),
            count => format!("🎵  Apps  ·  {count} sending"),
        };
        let tabs = [(Tab::Clips, "✂  Clips".to_owned()), (Tab::Apps, apps_label)];

        Frame::new().fill(theme::SURFACE).corner_radius(10).inner_margin(Margin::same(4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.columns(tabs.len(), |columns| {
                for (column, (tab, label)) in columns.iter_mut().zip(tabs) {
                    let selected = self.settings.tab == tab;
                    let text = egui::RichText::new(label).color(if selected { theme::TEXT } else { theme::TEXT_MUTED });
                    let fill = if selected { theme::SURFACE_RAISED } else { Color32::TRANSPARENT };
                    let button =
                        egui::Button::new(text).fill(fill).min_size(egui::vec2(column.available_width(), 30.0));
                    if column.add(button).clicked() {
                        self.settings.tab = tab;
                    }
                }
            });
        });
    }

    fn drop_overlay(&self, ctx: &egui::Context) {
        if ctx.input(|input| input.raw.hovered_files.is_empty()) {
            return;
        }
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("drop_overlay")));
        let window = ctx.content_rect();
        painter.rect_filled(window, 0.0, Color32::from_black_alpha(200));
        painter.text(
            window.center(),
            Align2::CENTER_CENTER,
            "Drop .wav files to add them",
            theme::semibold_font(20.0),
            Color32::WHITE,
        );
    }
}

impl eframe::App for TrollApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_services(&ctx);

        window_frame::title_bar(ui, self.title_bar);
        self.title_bar = egui::Panel::top("header")
            .frame(panel_frame(theme::BACKGROUND, Margin { left: 20, right: 12, top: 12, bottom: 8 }))
            .show_separator_line(false)
            .show(ui, |ui| self.header(ui))
            .response
            .rect;
        egui::Panel::bottom("footer")
            .frame(panel_frame(theme::SURFACE, Margin { left: 20, right: 20, top: 12, bottom: 10 }))
            .show_separator_line(false)
            .show(ui, |ui| self.footer(ui));
        egui::CentralPanel::no_frame()
            .frame(panel_frame(theme::BACKGROUND, Margin { left: 20, right: 20, top: 8, bottom: 12 }))
            .show(ui, |ui| self.body(ui));

        self.drop_overlay(&ctx);
        if self.setup_open {
            self.setup_dialog(&ctx);
        }
        window_frame::window_buttons(&ctx, self.window_buttons_slot);
        window_frame::border_and_resize_handles(&ctx);

        let repaint_interval = if self.live_audio.is_running() { LIVE_REPAINT_INTERVAL } else { IDLE_REPAINT_INTERVAL };
        ctx.request_repaint_after(repaint_interval);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.save(storage);
    }
}

fn panel_frame(fill: Color32, margin: Margin) -> Frame {
    Frame::new().fill(fill).inner_margin(margin)
}
