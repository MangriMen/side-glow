#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture;
mod config;
mod display;
mod glow;
mod logging;
mod ui;

use crate::app::SideGlowApp;
use eframe::egui;

const ICON_SIZE: u32 = 64;
/// Rasterized from `assets/icon.svg` by `build.rs`.
const ICON_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba"));

fn main() -> eframe::Result<()> {
    logging::init();
    log::info!("SideGlow {} starting", env!("CARGO_PKG_VERSION"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SideGlow Settings")
            .with_inner_size([440.0, 700.0])
            .with_min_inner_size([360.0, 300.0])
            .with_icon(egui::IconData {
                rgba: ICON_RGBA.to_vec(),
                width: ICON_SIZE,
                height: ICON_SIZE,
            })
            .with_visible(false)
            .with_active(false),
        ..Default::default()
    };

    eframe::run_native(
        "SideGlow",
        options,
        Box::new(|cc| {
            let icon = tray_icon::Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE)?;
            Ok(Box::new(SideGlowApp::new(cc, icon)?))
        }),
    )
}
