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
        }
    }
}

impl AppSettings {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub type SharedSettings = Arc<RwLock<AppSettings>>;
