//! The settings window shown in the root viewport.

use crate::capture::SessionStatus;
use crate::config::{
    CaptureConfig, Config, Edge, GlowMode, LookConfig, MonitorId, OutputConfig, SegmentMapping,
    FPS_RANGE, GLOW_SPREAD_RANGE, SEGMENTS_RANGE, STRIDE_RANGE, ZONE_DEPTH_RANGE,
};
use crate::display::layout::{source_monitors, ResolvedOutput};
use crate::display::{MonitorInfo, PxRect};
use crate::glow::bus::ColorBus;
use crate::glow::color::{adjust, to_color32};
use eframe::egui::{self, emath::Numeric, Align2, Color32, FontId, Rect, Sense, StrokeKind};
use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsAction {
    TogglePause,
    Quit,
    ResetDefaults,
    /// Rebuild the outputs from the monitor layout once (manual layout mode).
    DetectLayout,
}

pub struct SettingsView<'a> {
    pub config: &'a mut Config,
    pub monitors: &'a [MonitorInfo],
    pub outputs: &'a [ResolvedOutput],
    pub status: &'a [(String, SessionStatus)],
    pub bus: &'a ColorBus,
    pub paused: bool,
}

pub fn show(ui: &mut egui::Ui, view: SettingsView) -> Option<SettingsAction> {
    let mut action = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        status_line(ui, view.status, view.paused);
        ui.add_space(4.0);

        egui::CollapsingHeader::new("Monitors")
            .default_open(true)
            .show(ui, |ui| {
                monitor_map(ui, view.config, view.monitors, view.outputs, view.bus);
                ui.checkbox(&mut view.config.auto_layout, "Automatic layout")
                    .on_hover_text(help::AUTO_LAYOUT);
                ui.checkbox(&mut view.config.show_zone_preview, "Show capture zones")
                    .on_hover_text(help::ZONE_PREVIEW);
            });

        egui::CollapsingHeader::new("Look")
            .default_open(true)
            .show(ui, |ui| look_section(ui, &mut view.config.look));

        egui::CollapsingHeader::new("Capture")
            .default_open(true)
            .show(ui, |ui| capture_section(ui, &mut view.config.capture));

        egui::CollapsingHeader::new(format!("Outputs ({})", view.config.outputs.len()))
            .default_open(true)
            .show(ui, |ui| {
                if let Some(a) = outputs_section(ui, view.config, view.monitors) {
                    action = Some(a);
                }
            });

        ui.separator();
        ui.horizontal(|ui| {
            let pause_label = if view.paused { "Resume" } else { "Pause" };
            if ui.button(pause_label).on_hover_text(help::PAUSE).clicked() {
                action = Some(SettingsAction::TogglePause);
            }
            if ui
                .button("Reset to defaults")
                .on_hover_text(help::RESET)
                .clicked()
            {
                action = Some(SettingsAction::ResetDefaults);
            }
            if ui.button("Quit").on_hover_text(help::QUIT).clicked() {
                action = Some(SettingsAction::Quit);
            }
        });
    });
    action
}

fn status_line(ui: &mut egui::Ui, status: &[(String, SessionStatus)], paused: bool) {
    if paused {
        ui.colored_label(ui.visuals().warn_fg_color, "Paused");
        return;
    }
    if status.is_empty() {
        ui.colored_label(ui.visuals().warn_fg_color, "Nothing to capture");
    }
    for (name, state) in status {
        let (color, text) = match state {
            SessionStatus::Running => (Color32::from_rgb(80, 200, 120), "capturing".to_owned()),
            SessionStatus::Starting => (ui.visuals().warn_fg_color, "starting".to_owned()),
            SessionStatus::Failed(err) => (ui.visuals().error_fg_color, format!("error: {err}")),
        };
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(format!("{name}: {text}"));
        });
    }
}

fn look_section(ui: &mut egui::Ui, look: &mut LookConfig) {
    slider(
        ui,
        &mut look.brightness,
        0.0..=2.0,
        "Brightness",
        help::BRIGHTNESS,
    );
    slider(
        ui,
        &mut look.saturation,
        0.0..=2.0,
        "Saturation",
        help::SATURATION,
    );
    slider(ui, &mut look.opacity, 0.0..=1.0, "Opacity", help::OPACITY);
    slider(
        ui,
        &mut look.glow_spread,
        GLOW_SPREAD_RANGE,
        "Glow spread",
        help::GLOW_SPREAD,
    );
    ui.add(
        egui::Slider::new(&mut look.smoothing_ms, 0.0..=2000.0)
            .text("Smoothing")
            .suffix(" ms"),
    )
    .on_hover_text(help::SMOOTHING);
}

fn capture_section(ui: &mut egui::Ui, capture: &mut CaptureConfig) {
    slider(
        ui,
        &mut capture.segments,
        SEGMENTS_RANGE,
        "Zones per edge",
        help::ZONES,
    );
    ui.add(
        egui::Slider::new(&mut capture.zone_depth_px, ZONE_DEPTH_RANGE)
            .text("Capture zone depth")
            .suffix(" px"),
    )
    .on_hover_text(help::ZONE_DEPTH);
    slider(
        ui,
        &mut capture.target_fps,
        FPS_RANGE,
        "Capture FPS",
        help::FPS,
    );
    slider(
        ui,
        &mut capture.sample_stride,
        STRIDE_RANGE,
        "Sample every Nth pixel",
        help::STRIDE,
    );
}

/// Scaled-down picture of the virtual desktop. Captured monitors are highlighted, and
/// every glow is drawn as a strip of its live colors.
fn monitor_map(
    ui: &mut egui::Ui,
    config: &mut Config,
    monitors: &[MonitorInfo],
    outputs: &[ResolvedOutput],
    bus: &ColorBus,
) {
    let Some(bounds) = monitors.iter().map(|m| m.rect).reduce(|a, b| PxRect {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }) else {
        ui.label("No monitors found");
        return;
    };

    let size = egui::vec2(ui.available_width(), 150.0);
    let (response, painter) = ui.allocate_painter(size, Sense::click());
    let area = response.rect.shrink(8.0);
    let scale = (area.width() / bounds.width() as f32).min(area.height() / bounds.height() as f32);
    let offset =
        area.center() - egui::vec2(bounds.width() as f32, bounds.height() as f32) * scale / 2.0;
    let to_screen = |r: &PxRect| {
        Rect::from_min_max(
            offset + egui::vec2((r.left - bounds.left) as f32, (r.top - bounds.top) as f32) * scale,
            offset
                + egui::vec2(
                    (r.right - bounds.left) as f32,
                    (r.bottom - bounds.top) as f32,
                ) * scale,
        )
        .shrink(1.5)
    };

    let visuals = ui.visuals().clone();
    let sources: Vec<MonitorId> = source_monitors(config, monitors)
        .into_iter()
        .map(|m| m.id.clone())
        .collect();
    let is_source = |m: &MonitorInfo| sources.iter().any(|id| id.matches(&m.id));

    for monitor in monitors {
        let rect = to_screen(&monitor.rect);
        let (fill, stroke) = if is_source(monitor) {
            (visuals.selection.bg_fill, visuals.selection.stroke)
        } else {
            (
                visuals.faint_bg_color,
                visuals.widgets.noninteractive.bg_stroke,
            )
        };
        painter.rect(rect, 3.0, fill, stroke, StrokeKind::Inside);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!(
                "{}\n{}×{}",
                monitor.id.display_name(),
                monitor.rect.width(),
                monitor.rect.height()
            ),
            FontId::proportional(10.0),
            visuals.text_color(),
        );
    }

    for output in outputs {
        let Some(colors) = bus.latest(output.key) else {
            continue;
        };
        let target = to_screen(&output.target.rect);
        let edge = output.config.source_edge.opposite();
        let brightness = output.config.brightness.unwrap_or(config.look.brightness);
        let count = colors.len() as f32;
        for (i, &c) in colors.iter().enumerate() {
            let (t0, t1) = (i as f32 / count, (i + 1) as f32 / count);
            let (from, to) = output.segments.target;
            let (a, b) = (from + (to - from) * t0, from + (to - from) * t1);
            let strip = edge_strip(target, edge, a, b, 5.0);
            let color = adjust(c, brightness, config.look.saturation);
            painter.rect_filled(strip, 0.0, to_color32(color, 1.0));
        }
    }

    let hint = if config.auto_layout {
        help::MAP_AUTO
    } else {
        help::MAP_MANUAL
    };
    let response = response.on_hover_text(hint);
    if config.auto_layout && response.clicked() {
        let clicked = response
            .interact_pointer_pos()
            .and_then(|pos| monitors.iter().find(|m| to_screen(&m.rect).contains(pos)));
        if let Some(monitor) = clicked {
            toggle_source(config, &sources, &monitor.id);
        }
    }
}

/// Strip of `thickness` points along `edge` of `rect`, from fraction `a` to `b`.
fn edge_strip(rect: Rect, edge: Edge, a: f32, b: f32, thickness: f32) -> Rect {
    let along_x = |t: f32| egui::lerp(rect.left()..=rect.right(), t);
    let along_y = |t: f32| egui::lerp(rect.top()..=rect.bottom(), t);
    match edge {
        Edge::Left => Rect::from_x_y_ranges(
            rect.left()..=rect.left() + thickness,
            along_y(a)..=along_y(b),
        ),
        Edge::Right => Rect::from_x_y_ranges(
            rect.right() - thickness..=rect.right(),
            along_y(a)..=along_y(b),
        ),
        Edge::Top => {
            Rect::from_x_y_ranges(along_x(a)..=along_x(b), rect.top()..=rect.top() + thickness)
        }
        Edge::Bottom => Rect::from_x_y_ranges(
            along_x(a)..=along_x(b),
            rect.bottom() - thickness..=rect.bottom(),
        ),
    }
}

fn toggle_source(config: &mut Config, current: &[MonitorId], id: &MonitorId) {
    let mut sources = current.to_vec();
    if let Some(index) = sources.iter().position(|s| s.matches(id)) {
        if sources.len() == 1 {
            return; // Keep at least one captured monitor.
        }
        sources.remove(index);
    } else {
        sources.push(id.clone());
    }
    config.sources = sources;
}

fn outputs_section(
    ui: &mut egui::Ui,
    config: &mut Config,
    monitors: &[MonitorInfo],
) -> Option<SettingsAction> {
    let Config {
        outputs,
        capture,
        look,
        auto_layout,
        ..
    } = config;
    let manual = !*auto_layout;
    let mut action = None;
    let mut remove = None;

    if outputs.is_empty() {
        ui.label("No outputs. Connect a monitor next to a captured one, or add one manually.");
    }

    for (index, output) in outputs.iter_mut().enumerate() {
        let connected = [&output.source, &output.target]
            .iter()
            .all(|id| monitors.iter().any(|m| m.id.matches(id)));
        // Plain ASCII: egui's default fonts have no arrows.
        let mut title = format!(
            "{}: {} edge of {}",
            output.target.display_name(),
            output.source_edge.label(),
            output.source.display_name(),
        );
        if !connected {
            title.push_str(" (disconnected)");
        }

        ui.horizontal(|ui| {
            ui.checkbox(&mut output.enabled, "")
                .on_hover_text(help::OUTPUT_ENABLED);
            egui::CollapsingHeader::new(title)
                .id_salt(("output", index))
                .show(ui, |ui| {
                    if manual {
                        link_editor(ui, index, output, monitors);
                        if ui.button("Remove").on_hover_text(help::REMOVE).clicked() {
                            remove = Some(index);
                        }
                        ui.separator();
                    }
                    output_editor(ui, index, output, capture, look);
                });
        });
    }

    if let Some(index) = remove {
        outputs.remove(index);
    }

    if manual {
        ui.horizontal(|ui| {
            if ui.button("Add output").on_hover_text(help::ADD).clicked() {
                outputs.push(new_manual_output(monitors));
            }
            if ui
                .button("Detect from layout")
                .on_hover_text(help::DETECT)
                .clicked()
            {
                action = Some(SettingsAction::DetectLayout);
            }
        });
    }
    action
}

fn new_manual_output(monitors: &[MonitorInfo]) -> OutputConfig {
    let source = monitors.iter().find(|m| m.is_primary).or(monitors.first());
    let target = monitors
        .iter()
        .find(|m| Some(m.handle) != source.map(|s| s.handle));
    OutputConfig {
        source: source.map(|m| m.id.clone()).unwrap_or_default(),
        target: target.map(|m| m.id.clone()).unwrap_or_default(),
        ..Default::default()
    }
}

fn link_editor(
    ui: &mut egui::Ui,
    index: usize,
    output: &mut OutputConfig,
    monitors: &[MonitorInfo],
) {
    monitor_combo(
        ui,
        ("source", index),
        "Capture from",
        help::SOURCE,
        &mut output.source,
        monitors,
    );
    combo(
        ui,
        ("edge", index),
        "Edge",
        help::EDGE,
        &mut output.source_edge,
        &Edge::ALL,
        |e| e.label(),
    );
    monitor_combo(
        ui,
        ("target", index),
        "Glow on",
        help::TARGET,
        &mut output.target,
        monitors,
    );
}

fn output_editor(
    ui: &mut egui::Ui,
    index: usize,
    output: &mut OutputConfig,
    capture: &CaptureConfig,
    look: &LookConfig,
) {
    combo(
        ui,
        ("mode", index),
        "Mode",
        help::MODE,
        &mut output.mode,
        &GlowMode::ALL,
        |m| m.label(),
    );
    combo(
        ui,
        ("mapping", index),
        "Mapping",
        help::MAPPING,
        &mut output.mapping,
        &SegmentMapping::ALL,
        |m| m.label(),
    );

    ui.label("Overrides").on_hover_text(help::OVERRIDES);
    override_slider(
        ui,
        "Zones per edge",
        &mut output.segments,
        capture.segments,
        SEGMENTS_RANGE,
    );
    override_slider(
        ui,
        "Capture zone depth",
        &mut output.zone_depth_px,
        capture.zone_depth_px,
        ZONE_DEPTH_RANGE,
    );
    if output.mode == GlowMode::Overlay {
        override_slider(ui, "Opacity", &mut output.opacity, look.opacity, 0.0..=1.0);
    }
    override_slider(
        ui,
        "Glow spread",
        &mut output.glow_spread,
        look.glow_spread,
        GLOW_SPREAD_RANGE,
    );
    override_slider(
        ui,
        "Brightness",
        &mut output.brightness,
        look.brightness,
        0.0..=2.0,
    );
}

fn slider<T: Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    label: &str,
    help: &str,
) {
    ui.add(egui::Slider::new(value, range).text(label))
        .on_hover_text(help);
}

/// A per-output value that falls back to the global setting while unchecked.
fn override_slider<T: Numeric + std::fmt::Display>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<T>,
    global: T,
    range: RangeInclusive<T>,
) {
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui
            .checkbox(&mut enabled, label)
            .on_hover_text(help::OVERRIDES)
            .changed()
        {
            // Start from the current global value, so ticking the box changes nothing yet.
            *value = enabled.then_some(global);
        }
        match value {
            Some(v) => {
                ui.add(egui::Slider::new(v, range));
            }
            None => {
                ui.weak(format!("global: {global}"));
            }
        }
    });
}

fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    help: &str,
    value: &mut T,
    options: &[T],
    name: impl Fn(T) -> &'static str,
) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(id)
            .selected_text(name(*value))
            .show_ui(ui, |ui| {
                for &option in options {
                    ui.selectable_value(value, option, name(option));
                }
            })
            .response
            .on_hover_text(help);
        ui.label(label).on_hover_text(help);
    });
}

fn monitor_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    help: &str,
    value: &mut MonitorId,
    monitors: &[MonitorInfo],
) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(id)
            .selected_text(value.display_name().to_owned())
            .show_ui(ui, |ui| {
                for monitor in monitors {
                    let selected = value.matches(&monitor.id);
                    let text = format!(
                        "{} ({}×{})",
                        monitor.id.display_name(),
                        monitor.rect.width(),
                        monitor.rect.height()
                    );
                    if ui.selectable_label(selected, text).clicked() {
                        *value = monitor.id.clone();
                    }
                }
            })
            .response
            .on_hover_text(help);
        ui.label(label).on_hover_text(help);
    });
}

/// Hover texts for the settings.
mod help {
    pub const AUTO_LAYOUT: &str = "Create a glow on every monitor that touches a captured \
        monitor, and keep it up to date when monitors are connected or moved.\n\
        Turn off to add and edit outputs by hand.";
    pub const ZONE_PREVIEW: &str =
        "Draw red frames around the areas of the screen that are sampled.";
    pub const MAP_AUTO: &str = "Captured monitors are highlighted; click a monitor to toggle \
        capturing it. The strips show the current glow colors.";
    pub const MAP_MANUAL: &str = "The strips show the current glow colors. Choose captured \
        monitors per output below, or turn on the automatic layout.";

    pub const BRIGHTNESS: &str =
        "Multiplies the glow color. Above 1 lets dark scenes glow brighter.";
    pub const SATURATION: &str = "Color intensity: 0 is grayscale, 1 is as captured, 2 is boosted.";
    pub const OPACITY: &str = "Maximum opacity of Overlay glows at the edge next to the main \
        screen. Lower values keep the windows underneath visible.\n\
        Not used in Dedicated mode.";
    pub const GLOW_SPREAD: &str = "How quickly the glow fades away from the edge, as a \
        fraction of the neighbouring monitor's width (or height, for monitors above and \
        below). Smaller values give a tighter, more contained glow; larger values spread \
        further before fading.";
    pub const SMOOTHING: &str = "How long color changes take. Higher values are calmer and \
        hide flicker; 0 follows the screen instantly.";

    pub const ZONES: &str = "How many separate colors are taken along each edge.\n\
        1: one average color per side.\n\
        More: the glow follows the picture along the edge, like Ambilight.";
    pub const ZONE_DEPTH: &str = "How far into the captured screen, in pixels, colors are \
        averaged for each edge. Small values react to what is right at the border.";
    pub const FPS: &str = "How often the screen is sampled. Higher is more responsive and \
        uses more CPU and GPU. Changing it briefly restarts capture.";
    pub const STRIDE: &str = "Only every Nth pixel of a capture zone is averaged, in both \
        directions. Higher is cheaper and usually looks the same.";

    pub const OUTPUT_ENABLED: &str = "Turn this glow on or off.";
    pub const MODE: &str = "Overlay: a transparent layer on top of whatever that monitor \
        shows; clicks pass through it.\n\
        Dedicated: the monitor is used only as a light; the glow fades into black.";
    pub const MAPPING: &str = "Stretch: the whole edge of the captured screen is spread over \
        the whole side of this monitor.\n\
        Aligned: only the parts of the edges that physically face each other are used \
        (for monitors of different sizes or with an offset).";
    pub const OVERRIDES: &str = "Tick to use a different value for this output only. \
        Unticked, the global setting above is used.";
    pub const SOURCE: &str = "The monitor whose picture is sampled.";
    pub const EDGE: &str = "Which edge of the captured monitor is sampled.";
    pub const TARGET: &str = "The monitor that shows the glow.";
    pub const ADD: &str = "Add an output to configure by hand.";
    pub const REMOVE: &str = "Delete this output.";
    pub const DETECT: &str = "Replace the outputs with what the automatic layout would \
        create, once.";

    pub const PAUSE: &str = "Stop capturing and hide the glow until resumed.";
    pub const RESET: &str = "Restore all settings, including the outputs, to their defaults.";
    pub const QUIT: &str = "Exit SideGlow. Closing this window only hides it to the tray.";
}
