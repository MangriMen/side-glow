use crate::core::AppSettings;
use crate::ui_adapter::UiAdapter;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct SideGlowApp {
    ui_adapter: UiAdapter,
}

impl SideGlowApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        settings: Arc<RwLock<AppSettings>>,
        icon: tray_icon::Icon,
    ) -> Self {
        Self {
            ui_adapter: UiAdapter::new(settings, icon),
        }
    }
}

impl eframe::App for SideGlowApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_adapter.render_settings(ui);
    }

    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.ui_adapter.handle_tray_events(ctx);
        self.ui_adapter.render_panels(ctx);
        ctx.request_repaint();
    }
}
