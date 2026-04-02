#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture_service;
mod components;
mod core;
mod ui_adapter;
mod utils;

use crate::app::AmbientApp;
use crate::core::AppSettings;
use parking_lot::RwLock;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let settings = Arc::new(RwLock::new(AppSettings::default()));

    capture_service::start_capture_thread(Arc::clone(&settings));

    let icon_data = include_bytes!("../assets/icon.svg");
    let icon_rgba = crate::utils::render_svg_to_rgba(icon_data, 64, 64);
    let icon =
        tray_icon::Icon::from_rgba(icon_rgba.clone(), 64, 64).expect("Failed to create tray icon");

    let eframe_icon = eframe::egui::IconData {
        rgba: icon_rgba,
        width: 64,
        height: 64,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("AmbientSide Settings")
            .with_inner_size([350.0, 450.0])
            .with_icon(eframe_icon)
            .with_visible(false)
            .with_active(false),
        ..Default::default()
    };

    eframe::run_native(
        "AmbientSide",
        options,
        Box::new(|cc| Ok(Box::new(AmbientApp::new(cc, settings, icon)))),
    )
}
