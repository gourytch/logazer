#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::env;
use std::fs::OpenOptions;
use log::info;
use fern::Dispatch;
use log::LevelFilter;
use chrono::Local;

mod types;
mod grabber;
mod processor;
mod classifier;
mod imagework;
mod parser;
mod historybar;
mod mainview;

use crate::mainview::{LOGazer, WINDOW_WIDTH, WINDOW_HEIGHT};

//////////////////////////////////////////////////////////////////////////////

fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    let time = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let fname = format!("logazer.{}.log", time);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(fname)?;

    // RUST_LOG=trace|debug|info|warn|error
    let level = env::var("RUST_LOG")
        .ok()
        .and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);

    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(level)
        .chain(std::io::stdout())
        .chain(file)
        .apply()?;

    Ok(())
}

//////////////////////////////////////////////////////////////////////////////
/// main
//////////////////////////////////////////////////////////////////////////////

fn main() -> eframe::Result {
    init_logging().unwrap();
    info!("started");

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(false)
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_max_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_resizable(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/targeted_rhombus.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "LOGazer",
        options,
        Box::new(|cc| Ok(Box::new(LOGazer::new(cc)))),
    )
}
