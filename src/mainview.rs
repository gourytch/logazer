use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{fs, thread};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use log::{debug, info, trace, warn};
use chrono::Local;

use crossbeam_channel::Select;

use eframe::egui::{
    self, Align, Button, Color32,
    ColorImage, Image, Layout,
    RichText, Ui,
    TextureHandle, TextureOptions,
    TopBottomPanel, Visuals, Sense,
    ViewportCommand, WindowLevel
};
use egui_twemoji::EmojiLabel;

use rand::prelude::*;

use crate::types::{Quality, Screenshot};
use crate::grabber::Watcher;
use crate::processor::new_processor_pipeline;
use crate::historybar::HistoryBar;

const COFFEE_BREAK_FOR_NOISE: u64 = 33;
// const COFFEE_BREAK_BETWEEN_REPAINTS: u64 = 100;

// layout constants
const SHOT_WIDTH: usize = 1280; // 2560/2
const SHOT_HEIGHT: usize = 740; // 1440/2
// const SHOT_SIZE: usize = SHOT_WIDTH * SHOT_HEIGHT;

const PREVIEWER_WIDTH: f32 = SHOT_WIDTH as f32;
const PREVIEWER_HEIGHT: f32 = SHOT_HEIGHT as f32;
// const CONSOLE_HEIGHT: f32 = 100.;
// const CONTROLS_HEIGHT: f32 = 15.;
const HEADER_HEIGHT: f32 = 20.;
// const HEADER_ICON_HEIGHT: f32 = 24.;
// overall height = previewer + console + controls

const GAP: f32 = 9.;
pub const WINDOW_WIDTH: f32 = GAP + PREVIEWER_WIDTH + GAP; // tied to previewer
// pub const WINDOW_HEIGHT: f32 = GAP + HEADER_HEIGHT + GAP + PREVIEWER_HEIGHT + GAP + CONSOLE_HEIGHT + GAP + CONTROLS_HEIGHT + GAP;
pub const WINDOW_HEIGHT: f32 = GAP + HEADER_HEIGHT + GAP + PREVIEWER_HEIGHT + GAP;

const ICON_SLEEPING: &str = "💤";
const ICON_WATCHING: &str = "👀";
const ICON_PINNED: &str = "📍";
const ICON_UNPINNED: &str = "📌";
const ICON_CLOSE: &str = "❌";

const COLOR_SLEEPING: Color32 = Color32::from_rgb(128, 128, 255);
const COLOR_WATCHING: Color32 = Color32::from_rgb(255, 255, 128);

const SCREENSHOT_PATH: &str = "./screenshots/";
const HISTORY_SIZE: usize = 100;

//////////////////////////////////////////////////////////////////////////////

fn emoji_button(ui: &mut Ui, emoji: &str, clicked: impl FnOnce()) {
    if false {
        let resp = ui.add_sized(
            ui.spacing().interact_size,
            |ui: &mut egui::Ui| {
                EmojiLabel::new(emoji).show(ui)
            },
        );
        if resp.interact(Sense::click()).clicked() {
            clicked();
        }
    } else {
        let button = ui.add(Button::new(
            egui::RichText::new(emoji).size(16.0),
        ));
        if button.clicked() {
            clicked();
        }
    }
}

#[allow(unused)]
fn fill_noise(canvas: &mut ColorImage) {
    let w = canvas.width();
    let h = canvas.height();
    let mut rng = rand::rng();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let c: u8 = (rng.random::<u8>() >> 2) + 128; // [0..64) -> [128..192)
            canvas.pixels[i] = Color32::from_rgb(c, c, c);
        }
    }
}

fn paint(canvas: &mut ColorImage, ss: &Screenshot) {
    let resized = ss.image.resize_exact(canvas.width() as u32, canvas.height() as u32, image::imageops::FilterType::Triangle);
    let data = resized.to_rgba8();
    trace!("canvas: {}x{}, shot {}x{}, resized: {}x{}",
              canvas.width(), canvas.height(),
              ss.image.width(), ss.image.height(),
              resized.width(), resized.height());
    canvas.as_raw_mut().copy_from_slice(&data);
}

fn save(ss: &Screenshot) {
    let time = Local::now().format("%Y%m%d_%H%M%S").to_string();
    // let suffix = format!("{}_{}", ss.meta.quality.to_str(), ss.meta.entity.to_str());
    let suffix = format!("{}", ss.meta.quality.to_str());
    let path = format!("{}/{}-{}.png", SCREENSHOT_PATH, time, suffix);
    if let Err(err) = fs::create_dir_all(SCREENSHOT_PATH) {
        warn!("create_dir_all error {}", err);
        return;
    }
    if let Err(err) = ss.image.save(&path) {
        warn!("save error {}", err);
    }
    info!("saved to {}", path);
}

//////////////////////////////////////////////////////////////////////////////
/// LOGazerConfig
//////////////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize)]
pub struct LOGazerConfig {
    pub quality_threshold: Quality,
    pub pinned: bool,
}

impl Default for LOGazerConfig {
    fn default() -> Self {
        Self {
            quality_threshold: Quality::Rare,
            pinned: false,
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
/// LOGazer
//////////////////////////////////////////////////////////////////////////////

pub struct LOGazer {
    #[allow(unused)]
    config: LOGazerConfig,
    quality_threshold: Arc<Mutex<Quality>>,
    update_need: Arc<AtomicBool>,
    watcher: Watcher,
    shot: Arc<Mutex<ColorImage>>,
    tex: TextureHandle,
    history_bar: Arc<Mutex<HistoryBar>>,
    #[allow(unused)]
    ctx: egui::Context,
}

/////////////////////////////////////////
/// LOGazer
/////////////////////////////////////////

impl LOGazer {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        info!("create GUI instance");
        // init_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let config: LOGazerConfig = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            LOGazerConfig::default()
        };
        let quality_threshold = Arc::new(Mutex::new(config.quality_threshold));
        let update_need = Arc::new(AtomicBool::new(false));
        let raw_shot = ColorImage::filled([SHOT_WIDTH, SHOT_HEIGHT], Color32::BLACK);
        let shot = Arc::new(Mutex::new(raw_shot));
        let tex = cc.egui_ctx.load_texture("shot", shot.lock().unwrap().clone(), TextureOptions::NEAREST);
        let history_bar = Arc::new(Mutex::new(HistoryBar::new(HISTORY_SIZE)));
        let (tx, rx) = new_processor_pipeline();

        let mut window_watcher = Watcher::new(tx);
        window_watcher.start();

        let quality_threshold_clone = quality_threshold.clone();
        let shot_clone = shot.clone();
        let update_need_clone = update_need.clone();
        let ctx_clone = cc.egui_ctx.clone();
        let history_bar_clone = Arc::clone(&history_bar);
        // start thread
        thread::spawn(move || {
            info!("screenshot receiver spawned");

            let generate_noise = || {
                fill_noise(&mut shot_clone.lock().unwrap());
                update_need_clone.store(true, Ordering::Relaxed);
                ctx_clone.request_repaint();
            };

            let process_frame = |ss: Screenshot| {
                let quality = ss.meta.quality;
                if quality != Quality::Unknown {
                    if let Ok(mut bar) = history_bar_clone.lock() {
                        bar.push(quality.to_color32());
                    }
                }
                let threshold = *quality_threshold_clone.lock().unwrap();
                if quality >= threshold {
                    save(&ss);
                    paint(&mut shot_clone.lock().unwrap(), &ss);
                }
                update_need_clone.store(true, Ordering::Relaxed);
                ctx_clone.request_repaint();
            };

            let warmup = || -> Option<Screenshot> {
                let mut sel = Select::new();
                let op_rx = sel.recv(&rx);
                let noise_interval = Duration::from_millis(COFFEE_BREAK_FOR_NOISE);
                loop {
                    let oper = sel.select_timeout(noise_interval);
                    match oper {
                        Ok(o) if o.index() == op_rx => {
                            match o.recv(&rx) {
                                Ok(ss) => return Some(ss),
                                Err(_) => return None,
                            }
                        }
                        _ => {
                            generate_noise();
                        }
                    }
                }
            };
            if let Some(first) = warmup() {
                process_frame(first);
                for ss in rx { process_frame(ss); }
            }
            info!("screenshot receiver finished");
        });

        Self {
            ctx: cc.egui_ctx.clone(),
            config,
            quality_threshold,
            watcher: window_watcher,
            update_need,
            shot,
            tex,
            history_bar,
        }
    }

    fn get_header_color(&mut self) -> egui::Color32 {
        egui::Color32::from_rgb(10, 35, 70)
    }

    fn render_header(&mut self, ctx: &egui::Context) {
        let header_frame = egui::Frame {
            fill: self.get_header_color(),
            inner_margin: egui::Margin::same(4),
            outer_margin: egui::Margin::same(0),
            corner_radius: egui::CornerRadius::same(0), // u8
            shadow: egui::epaint::Shadow::NONE,
            stroke: egui::Stroke::NONE,
        };
        // draw top bar
        TopBottomPanel::top("top_panel").frame(header_frame).show(ctx, |ui| {
            if ui.interact(ui.max_rect(), egui::Id::new("drag_header"), egui::Sense::drag()).drag_started() {
                ctx.send_viewport_cmd(egui::viewport::ViewportCommand::StartDrag);
            }

            egui::MenuBar::new().ui(ui, |ui| {
                ui.style_mut().interaction.selectable_labels = false;
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let active = self.watcher.active();
                    EmojiLabel::new(if active { ICON_WATCHING } else { ICON_SLEEPING }).show(ui);
                    ui.spacing();
                    ui.label(RichText::new(" [Last Oasis]: Gazer").color(if active { COLOR_WATCHING } else { COLOR_SLEEPING }));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        emoji_button(ui, ICON_CLOSE, || {
                            debug!("BUTTON CLICKED: CLOSE");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        });
                        emoji_button(ui, if self.config.pinned { ICON_PINNED } else { ICON_UNPINNED }, || {
                            debug!("BUTTON CLICKED: PIN");
                            self.config.pinned = !self.config.pinned;
                            let level = if self.config.pinned {
                                WindowLevel::AlwaysOnTop
                            } else {
                                WindowLevel::Normal
                            };
                            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
                        });
                    });
                    // ui.colored_label(qvalue.to_color32(), qvalue.to_str());
                    let mut quality = *self.quality_threshold.lock().unwrap();
                    egui::ComboBox::from_id_salt("quality_threshold")
                        .selected_text(RichText::new(quality.to_str()).color(quality.to_color32()))
                        .show_ui(ui, |ui| {
                            let qualities = [
                                Quality::Common,
                                Quality::Uncommon,
                                Quality::Rare,
                                Quality::Epic,
                                Quality::Legendary,
                            ];
                            for &q in &qualities {
                                let text = RichText::new(q.to_str()).color(q.to_color32());
                                if ui.selectable_label(quality == q, text).clicked() {
                                    quality = q;
                                }
                            }
                        });
                    *self.quality_threshold.lock().unwrap() = quality;
                    self.config.quality_threshold = quality;
                    if let Ok(history_bar) = self.history_bar.lock() {
                        ui.add(history_bar.clone());
                    }
                });
            });
        }); // TopBottomPanel::top
    }

    fn render_shot(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if ui.interact(ui.max_rect(), egui::Id::new("drag_previewer"), egui::Sense::drag()).drag_started() {
            ctx.send_viewport_cmd(egui::viewport::ViewportCommand::StartDrag);
        }
        // update
        if self.update_need.swap(false, Ordering::Acquire) {
            self.tex.set(self.shot.lock().unwrap().clone(), TextureOptions::NEAREST);
        }
        let tex_size = self.tex.size_vec2();
        let sized_tex = egui::load::SizedTexture::new(&mut self.tex, tex_size);
        ui.add(Image::new(sized_tex).fit_to_exact_size(tex_size));
    }
}

/////////////////////////////////////////
/// LOGazer @ App
/////////////////////////////////////////

impl eframe::App for LOGazer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(Visuals::dark());
        self.render_header(ctx);
        egui::CentralPanel::default()
            // .frame(egui::Frame::default().inner_margin(0.))
            .frame(egui::Frame::default().inner_margin(GAP))
            .show(ctx, |ui| {
                //** render all the GUI **//
                ui.vertical(|ui| {
                    self.render_shot(ui, ctx);
                }); // ui.vertical
            });
        // ctx.request_repaint_after(Duration::from_millis(COFFEE_BREAK_BETWEEN_REPAINTS));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.config);
        info!("GUI configuration saved")
    }
}
