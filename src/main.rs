#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{fs, thread};
use std::time::{Duration};
use serde::{Serialize, Deserialize};

use crossbeam_channel::TryRecvError;

use eframe::egui::{
    self, Align, Button, Color32, ColorImage, Image, Layout, RichText, TextureHandle, TextureOptions, TopBottomPanel, Visuals
};
use egui_twemoji::EmojiLabel;

use rand::prelude::*;
use chrono::prelude::*;

mod types;
mod grabber;
mod processor;
mod imagework;
mod parser;

use crate::types::{Quality, Screenshot};
use crate::grabber::Watcher;
use crate::processor::new_processor_pipeline;

const USE_TRY_RECV: bool = false;

const COFFEE_BREAK_FOR_NOISE: u64 = 10;
const COFFEE_BREAK_BETWEEN_REPAINTS: u64 = 100;

// use windows_capture::frame::Frame;

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
const WINDOW_WIDTH: f32 = GAP + PREVIEWER_WIDTH + GAP; // tied to previewer
// const WINDOW_HEIGHT: f32 = GAP + HEADER_HEIGHT + GAP + PREVIEWER_HEIGHT + GAP + CONSOLE_HEIGHT + GAP + CONTROLS_HEIGHT + GAP;
const WINDOW_HEIGHT: f32 = GAP + HEADER_HEIGHT + GAP + PREVIEWER_HEIGHT + GAP;

const ICON_SLEEPING: &'static str = "💤";
const ICON_WATCHING: &'static str = "👀";
const ICON_CLOSE: &'static str = "❌";

const COLOR_SLEEPING: Color32 = Color32::from_rgb(128,128,255);
const COLOR_WATCHING: Color32 = Color32::from_rgb(255,255,128);

const SCREENSHOT_PATH: &'static str = "./screenshots/";


//////////////////////////////////////////////////////////////////////////////
/// main
//////////////////////////////////////////////////////////////////////////////

fn main() -> eframe::Result {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(false)
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_max_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_resizable(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/targeted_rhombus.png")[..])
                    .expect("Failed to load icon"),
            )
            ,
        ..Default::default()
    };

    eframe::run_native(
        "LOGazer",
        options,
        Box::new(|cc| Ok(Box::new(LOGazer::new(cc)))),
    )
}

//////////////////////////////////////////////////////////////////////////////
/// LOGazerConfig
//////////////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize)]
pub struct LOGazerConfig {
    pub quality_threshold: Quality,
}


impl Default for LOGazerConfig {
    fn default() -> Self {
        Self {
            quality_threshold: Quality::Rare,
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
/// LOGazer
//////////////////////////////////////////////////////////////////////////////

struct LOGazer {
    #[allow(unused)]
    config: LOGazerConfig,
    quality_threshold: Arc<Mutex<Quality>>,
    update_need: Arc<AtomicBool>,
    watcher: Watcher,
    shot: Arc<Mutex<ColorImage>>,
    tex: TextureHandle,
    // notes: String,
    #[allow(unused)]
    ctx: egui::Context,
}

// fn init_fonts(_ctx: &egui::Context) {
    // let mut font_def = FontDefinitions::default();
    // font_def.font_data.insert(
    //     "8370".to_owned(),
    //     Cow::Borrowed(include_bytes!("../assets/3270-Regular.ttf")),
    // );
    // font_def
    //     .families
    //     .entry(FontFamily::Monospace)
    //     .or_default()
    //     .push("3270".to_owned());
    // font_def
    //     .families
    //     .entry(FontFamily::Monospace)
    //     .or_default()
    //     .push("3270".to_owned());
    // ctx.set_fonts(font_def);
// }

/////////////////////////////////////////
/// LOGazer
/////////////////////////////////////////

pub fn ts_now() -> String {
    Local::now().format("%Y%m%d%H%M%S").to_string()
}
#[allow(unused)]
fn fill_noise(canvas: &mut ColorImage) {
    let w = canvas.width();
    let h = canvas.height();
    let mut rng = rand::rng();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let rgb: [u8;3] = rng.random();
            canvas.pixels[i] = Color32::from_rgb(rgb[0],rgb[1],rgb[2]);
        }
    }
}

fn paint(canvas: &mut ColorImage, ss: &Screenshot) {
    let resized = ss.image.resize_exact(canvas.width() as u32, canvas.height() as u32, image::imageops::FilterType::Triangle);
    let data = resized.to_rgba8();
    eprintln!("canvas: {}x{}, shot {}x{}, resized: {}x{}",
              canvas.width(), canvas.height(),
              ss.image.width(), ss.image.height(),
              resized.width(), resized.height());
    canvas.as_raw_mut().copy_from_slice(&data);
}

fn save(ss: &Screenshot) {
    let time = ts_now();
    let suffix = format!("{}_{}", ss.meta.quality.to_str(), ss.meta.entity.to_str());
    let path = format!("{}/{}-{}.png", SCREENSHOT_PATH, time, suffix);
    if let Err(err) = fs::create_dir_all(SCREENSHOT_PATH) {
        println!("create_dir_all error {}", err);
        return;
    }
    if let Err(err) = ss.image.save(&path) {
        println!("save error {}", err);
    }
    println!("saved to {}", path);
}

impl LOGazer {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // init_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let config: LOGazerConfig =  if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            LOGazerConfig::default()
        };
        let quality_threshold = Arc::new(Mutex::new(config.quality_threshold));
        let update_need = Arc::new(AtomicBool::new(false));
        let raw_shot = ColorImage::filled([SHOT_WIDTH, SHOT_HEIGHT], Color32::BLACK);
        let shot = Arc::new(Mutex::new(raw_shot));
        let tex = cc.egui_ctx.load_texture("shot", shot.lock().unwrap().clone(), TextureOptions::NEAREST);
        let (tx, rx) = new_processor_pipeline();
        let mut watcher = Watcher::new(tx);
        watcher.start();

        let quality_threshold_clone = quality_threshold.clone();
        let shot_clone = shot.clone();
        let update_need_clone = update_need.clone();
        let ctx_clone = cc.egui_ctx.clone();
        // start thread
        thread::spawn(move || {
            eprintln!("screenshot receiver spawned");
            if USE_TRY_RECV {
                    loop {
                    match rx.try_recv() {
                        Ok(ss) => {
                            let threshold = *quality_threshold_clone.lock().unwrap();
                            if ss.meta.quality >= threshold {
                                save(&ss);
                                paint(&mut shot_clone.lock().unwrap(), &ss);
                                update_need_clone.store(true, Ordering::Relaxed);
                                ctx_clone.request_repaint();
                            }
                        } // Ok
                        Err(TryRecvError::Empty) => {
                            //     let t = Instant::now();
                            //     fill_noise(&mut shot_clone.lock().unwrap());
                            //     eprintln!("no data. noise spent {:?}...", t.elapsed());
                            //     update_need_clone.store(true, Ordering::Relaxed);
                            //     ctx_clone.request_repaint();
                            thread::sleep(Duration::from_millis(COFFEE_BREAK_FOR_NOISE));
                        } // Empty
                        Err(TryRecvError::Disconnected) => {
                            eprintln!("screenshot receiver disconnected");
                            break;
                        }
                    }
                }
            } else {
                for ss in rx {
                    let threshold = *quality_threshold_clone.lock().unwrap();
                    if ss.meta.quality >= threshold {
                        save(&ss);
                        paint(&mut shot_clone.lock().unwrap(), &ss);
                        update_need_clone.store(true, Ordering::Relaxed);
                        ctx_clone.request_repaint();
                    }
                }
            }
            eprintln!("screenshot receiver finished");
        });

        Self {
            ctx: cc.egui_ctx.clone(),
            config: config,
            quality_threshold: quality_threshold,
            watcher: watcher,
            // notes: String::new(),
            update_need: update_need,
            shot: shot,
            tex: tex,
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
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    eprintln!("main.watcher: running: {:?}, grabbing:{:?}, capturing:{:?}",
                        self.watcher.is_running(), self.watcher.is_grabbing(), self.watcher.is_capturing());
                    let active = self.watcher.is_grabbing() || self.watcher.is_capturing();
                    EmojiLabel::new(if active {ICON_WATCHING} else {ICON_SLEEPING}).show(ui);
                    ui.spacing();
                    ui.label(RichText::new(" [Last Oasis]: Gazer").color(if active {COLOR_WATCHING} else {COLOR_SLEEPING}));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {                        
                    // ui.colored_label(qvalue.to_color32(), qvalue.to_str());

                    let close_btn = ui.add(Button::new(ICON_CLOSE));
                    if close_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
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
        egui::CentralPanel::default().show(ctx, |ui| {
            //** render all the GUI **//
            ui.vertical(|ui| {

                self.render_shot(ui, ctx);

                // ui.separator();

                // let (console_rect, _console_resp) = ui.allocate_exact_size(
                //     egui::vec2(PREVIEWER_WIDTH, PREVIEWER_HEIGHT),
                //     egui::Sense::hover(),
                // );

                // ui.separator();

                // ScrollArea::vertical().show(ui, |ui| {
                //     ui.add(egui::TextEdit::multiline(&mut self.notes));
                // });
            }); // ui.vertical
        });
        ctx.request_repaint_after(Duration::from_millis(COFFEE_BREAK_BETWEEN_REPAINTS));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.config);
    }    
}
