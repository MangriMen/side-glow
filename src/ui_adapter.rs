use crate::components::glow_panel::GlowPanel;
use crate::components::settings_panel::SettingsPanel;
use crate::components::tray_manager::TrayManager;
use crate::core::SharedSettings;
use eframe::egui;

const MONITOR_W: f32 = 2560.0;
const MONITOR_H: f32 = 1440.0;

pub struct UiAdapter {
    settings: SharedSettings,
    tray_manager: TrayManager,
    left_panel: GlowPanel,
    right_panel: GlowPanel,
}

impl UiAdapter {
    pub fn new(settings: SharedSettings, icon: tray_icon::Icon) -> Self {
        Self {
            settings: settings.clone(),
            tray_manager: TrayManager::new(icon),
            left_panel: GlowPanel::new("left_panel", "Left", true, [-MONITOR_W, 0.0]),
            right_panel: GlowPanel::new("right_panel", "Right", false, [MONITOR_W, 0.0]),
        }
    }

    pub fn handle_tray_events(&mut self, _ctx: &egui::Context) {
        self.tray_manager.handle_events(self.settings.clone());
    }

    pub fn render_panels(&self, ctx: &egui::Context) {
        self.left_panel
            .show(ctx, self.settings.clone(), MONITOR_W, MONITOR_H);
        self.right_panel
            .show(ctx, self.settings.clone(), MONITOR_W, MONITOR_H);
    }

    pub fn render_settings(&mut self, ui: &mut egui::Ui) {
        let show = self.settings.read().show_settings;
        if show {
            ui.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            egui::CentralPanel::default().show_inside(ui, |ui| {
                SettingsPanel::show(ui, &mut self.settings.write());
            });
        } else {
            ui.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        if ui.input(|i| i.viewport().close_requested()) {
            self.settings.write().show_settings = false;
            ui.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
}
