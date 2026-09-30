use super::{glow_viewport_id, Rgb};
use crate::config::OutputKey;
use eframe::egui::{self, ViewportId};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Changes smaller than this are not worth a repaint.
const CHANGE_EPSILON: f32 = 1.0 / 2048.0;

/// Latest captured colors per output. Capture threads publish, glow windows read.
///
/// Glow and zone-preview windows are immediate viewports (see `ui::overlay`), which egui
/// only redraws as part of the root viewport's own pass — so every publish wakes root, not
/// just the specific window, even when the settings UI isn't shown.
pub struct ColorBus {
    ctx: egui::Context,
    slots: Mutex<HashMap<OutputKey, Arc<[Rgb]>>>,
}

impl ColorBus {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            slots: Mutex::default(),
        }
    }

    pub fn publish(&self, key: OutputKey, colors: Vec<Rgb>) {
        {
            let mut slots = self.slots.lock();
            if slots
                .get(&key)
                .is_some_and(|old| nearly_equal(old, &colors))
            {
                return;
            }
            slots.insert(key, colors.into());
        }
        self.ctx.request_repaint_of(glow_viewport_id(key));
        self.ctx.request_repaint_of(ViewportId::ROOT);
    }

    pub fn latest(&self, key: OutputKey) -> Option<Arc<[Rgb]>> {
        self.slots.lock().get(&key).cloned()
    }

    /// Drops colors of outputs that no longer exist.
    pub fn retain(&self, mut keep: impl FnMut(OutputKey) -> bool) {
        self.slots.lock().retain(|key, _| keep(*key));
    }
}

fn nearly_equal(a: &[Rgb], b: &[Rgb]) -> bool {
    a.len() == b.len()
        && a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(x, y)| (x - y).abs() < CHANGE_EPSILON)
}
