//! The settings window shown in the root viewport.

use crate::capture::SessionStatus;
use crate::config::{
    Config, Edge, Falloff, GlowMode, MonitorId, OutputConfig, SegmentMapping, FPS_RANGE,
    GLOW_DEPTH_RANGE, SEGMENTS_RANGE, STRIDE_RANGE, ZONE_DEPTH_RANGE,
};
use crate::display::layout::{source_monitors, ResolvedOutput};
use crate::display::{MonitorInfo, PxRect};
use crate::glow::bus::ColorBus;
use crate::glow::color::{adjust, to_color32};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, StrokeKind};

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
                    .on_hover_text("Create a glow for every monitor touching a captured one");
                ui.checkbox(&mut view.config.show_zone_preview, "Show capture zones");
            });

        egui::CollapsingHeader::new("Look")
            .default_open(true)
            .show(ui, |ui| look_section(ui, view.config));

        egui::CollapsingHeader::new("Capture").show(ui, |ui| {
            let capture = &mut view.config.capture;
            ui.add(egui::Slider::new(&mut capture.target_fps, FPS_RANGE).text("Capture FPS"));
            ui.add(
                egui::Slider::new(&mut capture.sample_stride, STRIDE_RANGE)
                    .text("Sample every Nth pixel"),
            );
        });

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
            if ui.button(pause_label).clicked() {
                action = Some(SettingsAction::TogglePause);
            }
            if ui.button("Reset to defaults").clicked() {
                action = Some(SettingsAction::ResetDefaults);
            }
            if ui.button("Quit").clicked() {
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

fn look_section(ui: &mut egui::Ui, config: &mut Config) {
    let look = &mut config.look;
    ui.add(egui::Slider::new(&mut look.brightness, 0.0..=2.0).text("Brightness"));
    ui.add(egui::Slider::new(&mut look.saturation, 0.0..=2.0).text("Saturation"));
    ui.add(
        egui::Slider::new(&mut look.smoothing_ms, 0.0..=2000.0)
            .text("Smoothing")
            .suffix(" ms"),
    );
    ui.add(egui::Slider::new(&mut look.glow_depth, GLOW_DEPTH_RANGE).text("Glow depth"));
    combo(
        ui,
        "falloff",
        "Falloff",
        &mut look.falloff,
        &Falloff::ALL,
        |f| f.label(),
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
        let count = colors.len() as f32;
        for (i, &c) in colors.iter().enumerate() {
            let (t0, t1) = (i as f32 / count, (i + 1) as f32 / count);
            let (from, to) = output.segments.target;
            let (a, b) = (from + (to - from) * t0, from + (to - from) * t1);
            let strip = edge_strip(target, edge, a, b, 5.0);
            let color = adjust(c, config.look.brightness, config.look.saturation);
            painter.rect_filled(strip, 0.0, to_color32(color, 1.0));
        }
    }

    if config.auto_layout {
        let response = response.on_hover_text("Click a monitor to toggle capturing it");
        if response.clicked() {
            let clicked = response
                .interact_pointer_pos()
                .and_then(|pos| monitors.iter().find(|m| to_screen(&m.rect).contains(pos)));
            if let Some(monitor) = clicked {
                toggle_source(config, &sources, &monitor.id);
            }
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
    let manual = !config.auto_layout;
    let mut action = None;
    let mut remove = None;

    if config.outputs.is_empty() {
        ui.label("No outputs. Connect a monitor next to a captured one, or add one manually.");
    }

    for (index, output) in config.outputs.iter_mut().enumerate() {
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
            ui.checkbox(&mut output.enabled, "");
            egui::CollapsingHeader::new(title)
                .id_salt(("output", index))
                .show(ui, |ui| {
                    if manual {
                        link_editor(ui, index, output, monitors);
                        if ui.button("Remove").clicked() {
                            remove = Some(index);
                        }
                        ui.separator();
                    }
                    output_editor(ui, index, output);
                });
        });
    }

    if let Some(index) = remove {
        config.outputs.remove(index);
    }

    if manual {
        ui.horizontal(|ui| {
            if ui.button("Add output").clicked() {
                config.outputs.push(new_manual_output(monitors));
            }
            if ui
                .button("Detect from layout")
                .on_hover_text("Replace the outputs with the automatic layout once")
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
        &mut output.source,
        monitors,
    );
    combo(
        ui,
        ("edge", index),
        "Edge",
        &mut output.source_edge,
        &Edge::ALL,
        |e| e.label(),
    );
    monitor_combo(
        ui,
        ("target", index),
        "Glow on",
        &mut output.target,
        monitors,
    );
}

fn output_editor(ui: &mut egui::Ui, index: usize, output: &mut OutputConfig) {
    combo(
        ui,
        ("mode", index),
        "Mode",
        &mut output.mode,
        &GlowMode::ALL,
        |m| m.label(),
    );
    if output.mode == GlowMode::Overlay {
        ui.add(egui::Slider::new(&mut output.opacity, 0.0..=1.0).text("Opacity"));
    }
    ui.add(egui::Slider::new(&mut output.segments, SEGMENTS_RANGE).text("Segments"));
    ui.add(
        egui::Slider::new(&mut output.zone_depth_px, ZONE_DEPTH_RANGE)
            .text("Capture zone depth")
            .suffix(" px"),
    );
    combo(
        ui,
        ("mapping", index),
        "Mapping",
        &mut output.mapping,
        &SegmentMapping::ALL,
        |m| m.label(),
    );
    optional_slider(
        ui,
        "Own glow depth",
        &mut output.glow_depth,
        0.45,
        GLOW_DEPTH_RANGE,
    );
    optional_slider(ui, "Own brightness", &mut output.brightness, 1.0, 0.0..=2.0);
}

fn optional_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f32>,
    default: f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *value = enabled.then_some(default);
        }
        if let Some(v) = value {
            ui.add(egui::Slider::new(v, range));
        }
    });
}

fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
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
            });
        ui.label(label);
    });
}

fn monitor_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
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
            });
        ui.label(label);
    });
}
