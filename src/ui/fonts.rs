//! Fonts for the settings window: the system's own instead of egui's bundled set,
//! which would add ~1.4 MB to the exe.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use std::sync::Arc;

/// Tried in order; the first one that exists is used. Segoe UI ships with every Windows.
const PROPORTIONAL: &[&str] = &["segoeui.ttf", "tahoma.ttf", "arial.ttf"];
const MONOSPACE: &[&str] = &["consola.ttf", "cour.ttf"];

fn fonts_dir() -> PathBuf {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
    PathBuf::from(windir).join("Fonts")
}

fn load(dir: &std::path::Path, candidates: &[&str]) -> Option<(String, FontData)> {
    candidates.iter().find_map(|file| {
        let bytes = std::fs::read(dir.join(file)).ok()?;
        Some((file.to_string(), FontData::from_owned(bytes)))
    })
}

pub fn install(ctx: &egui::Context) {
    let dir = fonts_dir();
    let mut fonts = FontDefinitions::empty();
    for (family, candidates) in [
        (FontFamily::Proportional, PROPORTIONAL),
        (FontFamily::Monospace, MONOSPACE),
    ] {
        match load(&dir, candidates) {
            Some((name, data)) => {
                fonts.font_data.insert(name.clone(), Arc::new(data));
                fonts.families.entry(family).or_default().push(name);
            }
            // Not fatal: the text just won't render.
            None => log::warn!("no system font found for {family:?} in {}", dir.display()),
        }
    }
    ctx.set_fonts(fonts);
}
