//! The glow itself: one borderless window covering the target monitor per output.

use super::overlay::{overlay_builder, OverlayState};
use crate::config::{Edge, GlowMode, LookConfig, OutputKey};
use crate::display::layout::{ResolvedOutput, SegmentLayout};
use crate::display::PxRect;
use crate::glow::bus::ColorBus;
use crate::glow::color::{adjust, to_color32};
use crate::glow::smoothing::Smoother;
use crate::glow::{glow_viewport_id, Rgb};
use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Shape};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

/// Vertex columns across the glow depth; enough for a smooth falloff curve. Columns are
/// spaced by `falloff_point`, not evenly, so this covers the whole monitor depth without
/// visible banding near the edge, where the curve bends fastest.
const FALLOFF_COLUMNS: usize = 24;
/// Frame interval while smoothing towards new colors. A soft glow gains nothing from
/// the monitor's full refresh rate, and each repaint presents a full-screen window.
const ANIMATION_FRAME: Duration = Duration::from_micros(1_000_000 / 60);

/// Everything needed to draw one glow window; rebuilt whenever the config changes.
#[derive(Clone, Debug, PartialEq)]
pub struct GlowSpec {
    pub key: OutputKey,
    pub title: String,
    pub rect: PxRect,
    pub scale: f32,
    /// Side of the target monitor that faces the source; the glow starts there.
    pub anchor: Edge,
    pub segments: SegmentLayout,
    pub mode: GlowMode,
    pub opacity: f32,
    pub spread: f32,
    pub brightness: f32,
    pub saturation: f32,
    pub smoothing_ms: f32,
}

impl GlowSpec {
    pub fn new(output: &ResolvedOutput, look: &LookConfig) -> Self {
        let config = &output.config;
        Self {
            key: output.key,
            title: format!("SideGlow glow {:016x}", output.key.0),
            rect: output.target.rect,
            scale: output.target.scale,
            anchor: config.source_edge.opposite(),
            segments: output.segments,
            mode: config.mode,
            opacity: config.opacity.unwrap_or(look.opacity),
            spread: config.glow_spread.unwrap_or(look.glow_spread),
            brightness: config.brightness.unwrap_or(look.brightness),
            saturation: look.saturation,
            smoothing_ms: look.smoothing_ms,
        }
    }
}

#[derive(Default)]
struct GlowState {
    overlay: OverlayState,
    smoother: Smoother,
}

pub struct GlowWindow {
    spec: Arc<GlowSpec>,
    state: Arc<Mutex<GlowState>>,
}

impl GlowWindow {
    pub fn new(spec: GlowSpec) -> Self {
        Self {
            spec: Arc::new(spec),
            state: Arc::default(),
        }
    }

    /// Replaces the spec; returns whether anything changed.
    pub fn set_spec(&mut self, spec: GlowSpec) -> bool {
        if *self.spec == spec {
            return false;
        }
        self.spec = Arc::new(spec);
        true
    }

    /// Registers the window for this root pass. It repaints on its own afterwards, whenever
    /// the color bus publishes new colors for it.
    pub fn show(&self, ctx: &egui::Context, bus: &Arc<ColorBus>) {
        let spec = self.spec.clone();
        let state = self.state.clone();
        let bus = bus.clone();
        let builder = overlay_builder(
            &spec.title,
            spec.rect,
            spec.scale,
            spec.mode == GlowMode::Overlay,
        );
        ctx.show_viewport_deferred(glow_viewport_id(spec.key), builder, move |ui, _| {
            render(ui, &spec, &mut state.lock(), &bus);
        });
    }
}

fn render(ui: &mut egui::Ui, spec: &GlowSpec, state: &mut GlowState, bus: &ColorBus) {
    let ctx = ui.ctx().clone();
    state.overlay.update(&ctx, &spec.title, spec.rect);

    let animating = match bus.latest(spec.key) {
        Some(colors) => {
            let target: Vec<Rgb> = colors
                .iter()
                .map(|&c| adjust(c, spec.brightness, spec.saturation))
                .collect();
            state
                .smoother
                .step(&target, ctx.input(|i| i.time), spec.smoothing_ms)
        }
        None => false,
    };

    let rect = ui.max_rect();
    if spec.mode == GlowMode::Dedicated {
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    }
    let colors = state.smoother.colors();
    if !colors.is_empty() {
        ui.painter().add(Shape::mesh(glow_mesh(spec, rect, colors)));
    }

    if animating {
        ctx.request_repaint_after(ANIMATION_FRAME);
    }
}

/// Point along the glow's depth axis and its intensity there, following the illumination
/// a flat surface gets from a line source standing `spread` away from it (fraction of the
/// monitor's size): a Lorentzian `1 / (1 + (x/spread)^2)`, with no cutoff — it just keeps
/// decaying. `t` in `0..=1` is remapped through `x = spread * tan(θ)`, so `weight = cos²θ`
/// exactly, and the substitution itself concentrates points near the edge, where the curve
/// bends fastest, without a separate density table.
fn falloff_point(t: f32, spread: f32) -> (f32, f32) {
    let theta_max = (1.0 / spread).atan();
    let theta = t * theta_max;
    let x = spread * theta.tan();
    let weight = theta.cos().powi(2);
    (x, weight)
}

/// A grid of vertices: rows along the edge at the segment centers, columns across the
/// glow depth following the falloff curve.
fn glow_mesh(spec: &GlowSpec, rect: Rect, colors: &[Rgb]) -> Mesh {
    let count = colors.len();
    let mut rows: Vec<(f32, Rgb)> = Vec::with_capacity(count + 2);
    rows.push((0.0, colors[0]));
    rows.extend(
        colors
            .iter()
            .enumerate()
            .map(|(i, &c)| (spec.segments.target_center(i, count), c)),
    );
    rows.push((1.0, colors[count - 1]));

    let along_y = spec.anchor.is_vertical();
    let reach = if along_y { rect.width() } else { rect.height() };
    let point = |along: f32, dist: f32| -> Pos2 {
        let a = if along_y {
            egui::lerp(rect.top()..=rect.bottom(), along)
        } else {
            egui::lerp(rect.left()..=rect.right(), along)
        };
        match spec.anchor {
            Edge::Left => egui::pos2(rect.left() + dist, a),
            Edge::Right => egui::pos2(rect.right() - dist, a),
            Edge::Top => egui::pos2(a, rect.top() + dist),
            Edge::Bottom => egui::pos2(a, rect.bottom() - dist),
        }
    };

    let mut mesh = Mesh::default();
    for &(along, color) in &rows {
        for col in 0..FALLOFF_COLUMNS {
            let t = col as f32 / (FALLOFF_COLUMNS - 1) as f32;
            let (x_frac, weight) = falloff_point(t, spec.spread);
            let vertex_color = match spec.mode {
                GlowMode::Overlay => to_color32(color, weight * spec.opacity),
                GlowMode::Dedicated => to_color32(color.map(|c| c * weight), 1.0),
            };
            mesh.colored_vertex(point(along, x_frac * reach), vertex_color);
        }
    }
    for row in 0..rows.len() as u32 - 1 {
        for col in 0..FALLOFF_COLUMNS as u32 - 1 {
            let i = row * FALLOFF_COLUMNS as u32 + col;
            let below = i + FALLOFF_COLUMNS as u32;
            mesh.add_triangle(i, i + 1, below);
            mesh.add_triangle(i + 1, below + 1, below);
        }
    }
    mesh
}
