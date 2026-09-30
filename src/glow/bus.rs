use super::{glow_viewport_id, Rgb};
use crate::config::OutputKey;
use eframe::egui::{self, ViewportId};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Changes smaller than this are not worth a repaint.
const CHANGE_EPSILON: f32 = 1.0 / 2048.0;

/// Latest captured colors per output. Capture threads publish, glow windows read.
///
/// Publishing wakes only the window that shows the output, so an unchanged screen
/// costs no repaints at all.
pub struct ColorBus {
    ctx: egui::Context,
    slots: Mutex<HashMap<OutputKey, Arc<[Rgb]>>>,
    notify_root: AtomicBool,
}

impl ColorBus {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            slots: Mutex::default(),
            notify_root: AtomicBool::new(false),
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
        if self.notify_root.load(Ordering::Relaxed) {
            self.ctx.request_repaint_of(ViewportId::ROOT);
        }
    }

    pub fn latest(&self, key: OutputKey) -> Option<Arc<[Rgb]>> {
        self.slots.lock().get(&key).cloned()
    }

    /// Drops colors of outputs that no longer exist.
    pub fn retain(&self, mut keep: impl FnMut(OutputKey) -> bool) {
        self.slots.lock().retain(|key, _| keep(*key));
    }

    /// Also repaint the settings window on new colors (for its live preview).
    pub fn set_notify_root(&self, notify: bool) {
        self.notify_root.store(notify, Ordering::Relaxed);
    }
}

fn nearly_equal(a: &[Rgb], b: &[Rgb]) -> bool {
    a.len() == b.len()
        && a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(x, y)| (x - y).abs() < CHANGE_EPSILON)
}
