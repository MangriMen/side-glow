//! Color data flowing from the capture threads to the glow windows.

pub mod bus;
pub mod color;
pub mod smoothing;

use crate::config::OutputKey;
use eframe::egui::ViewportId;

/// Linear-light RGB in `0.0..=1.0`.
pub type Rgb = [f32; 3];

pub fn glow_viewport_id(key: OutputKey) -> ViewportId {
    ViewportId::from_hash_of(("sideglow-glow", key.0))
}
