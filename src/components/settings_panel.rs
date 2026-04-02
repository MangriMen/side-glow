use crate::core::AppSettings;
use eframe::egui;

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn show(ui: &mut egui::Ui, settings: &mut AppSettings) {
        ui.add(egui::Slider::new(&mut settings.brightness, 0.0..=2.0).text("Brightness"));
        ui.add(egui::Slider::new(&mut settings.smoothing, 0.01..=0.5).text("Smoothing"));
        ui.add(egui::Slider::new(&mut settings.glow_depth, 0.01..=1.0).text("Glow Depth"));
        ui.add(egui::Slider::new(&mut settings.dither, 0.0..=2.0).text("Dither"));
        if ui.button("Reset to Defaults").clicked() {
            settings.reset();
        }
    }
}
