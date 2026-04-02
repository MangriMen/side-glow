// --- Core (Domain) ---
use parking_lot::RwLock;
use std::sync::Arc;

pub struct AppSettings {
    pub smoothing: f32,
    pub glow_depth: f32,
    pub brightness: f32,
    pub dither: f32,
    pub left_color: [f32; 3],
    pub right_color: [f32; 3],
    pub show_settings: bool,
    pub show_zone_preview: bool,
    pub zone_width: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            smoothing: 0.12,
            glow_depth: 0.45,
            brightness: 1.0,
            dither: 0.4,
            left_color: [0.0, 0.0, 0.0],
            right_color: [0.0, 0.0, 0.0],
            show_settings: false,
            show_zone_preview: false,
            zone_width: 120.0,
        }
    }
}

impl AppSettings {
    pub fn reset(&mut self) {
        let show_settings = self.show_settings;
        let show_zone_preview = self.show_zone_preview;
        *self = Self::default();
        self.show_settings = show_settings;
        self.show_zone_preview = show_zone_preview;
    }
}

pub type SharedSettings = Arc<RwLock<AppSettings>>;
