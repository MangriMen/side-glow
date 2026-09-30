//! Frames drawn over the capture zones so they can be tuned visually.

use super::overlay::{overlay_builder, OverlayState};
use crate::config::OutputKey;
use crate::display::layout::{zone_rect, ResolvedOutput};
use crate::display::PxRect;
use eframe::egui::{self, Color32, Stroke, StrokeKind, ViewportId};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewSpec {
    pub key: OutputKey,
    pub title: String,
    pub rect: PxRect,
    pub scale: f32,
    pub segments: u32,
    pub along_y: bool,
}

impl PreviewSpec {
    pub fn new(output: &ResolvedOutput) -> Self {
        Self {
            key: output.key,
            title: format!("SideGlow zone {:016x}", output.key.0),
            rect: zone_rect(output),
            scale: output.source.scale,
            segments: output.config.segments,
            along_y: output.config.source_edge.is_vertical(),
        }
    }

    fn viewport_id(&self) -> ViewportId {
        ViewportId::from_hash_of(("sideglow-zone", self.key.0))
    }
}

pub struct ZonePreview {
    spec: Arc<PreviewSpec>,
    state: Arc<Mutex<OverlayState>>,
}

impl ZonePreview {
    pub fn new(spec: PreviewSpec) -> Self {
        Self {
            spec: Arc::new(spec),
            state: Arc::default(),
        }
    }

    pub fn set_spec(&mut self, spec: PreviewSpec) -> bool {
        if *self.spec == spec {
            return false;
        }
        self.spec = Arc::new(spec);
        true
    }

    pub fn show(&self, ctx: &egui::Context) {
        let spec = self.spec.clone();
        let state = self.state.clone();
        let builder = overlay_builder(&spec.title, spec.rect, spec.scale, true);
        ctx.show_viewport_deferred(spec.viewport_id(), builder, move |ui, _| {
            state.lock().update(ui.ctx(), &spec.title, spec.rect);
            paint(ui, &spec);
        });
    }

    pub fn request_repaint(&self, ctx: &egui::Context) {
        ctx.request_repaint_of(self.spec.viewport_id());
    }
}

fn paint(ui: &egui::Ui, spec: &PreviewSpec) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let stroke = Stroke::new(2.0, Color32::RED);
    painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);

    let divider = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 0, 0, 160));
    for i in 1..spec.segments {
        let t = i as f32 / spec.segments as f32;
        if spec.along_y {
            let y = egui::lerp(rect.top()..=rect.bottom(), t);
            painter.hline(rect.x_range(), y, divider);
        } else {
            let x = egui::lerp(rect.left()..=rect.right(), t);
            painter.vline(x, rect.y_range(), divider);
        }
    }
}
