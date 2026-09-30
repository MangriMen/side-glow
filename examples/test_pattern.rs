//! Full-screen test pattern on the primary monitor, for repeatable benchmarks.
//!
//! `cargo run --release --example test_pattern -- <static|slow|fast>`
//! - static: fixed colors, nothing changes (SideGlow should be idle)
//! - slow:   hues drifting across the screen (a calm scene)
//! - fast:   a grid of random colors changing 30 times per second (busy video)

use eframe::egui::{self, ecolor::Hsva, Color32, Rect};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Static,
    Slow,
    Fast,
}

const GRID: (u32, u32) = (16, 9);
const FAST_INTERVAL: Duration = Duration::from_millis(33);

fn main() -> eframe::Result<()> {
    let mode = match std::env::args().nth(1).as_deref() {
        None | Some("static") => Mode::Static,
        Some("slow") => Mode::Slow,
        Some("fast") => Mode::Fast,
        Some(other) => {
            eprintln!("unknown mode '{other}', expected static, slow or fast");
            std::process::exit(2);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SideGlow test pattern")
            // (0, 0) is the top-left corner of the primary monitor. The size is fitted to
            // the monitor once the window knows it (see `fit_to_monitor`); `with_fullscreen`
            // is not reliable for a window that eframe creates hidden.
            .with_position([0.0, 0.0])
            .with_inner_size([800.0, 600.0])
            .with_decorations(false)
            .with_taskbar(false)
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "SideGlow test pattern",
        options,
        Box::new(move |_| Ok(Box::new(Pattern { mode, tick: 0 }))),
    )
}

struct Pattern {
    mode: Mode,
    tick: u64,
}

impl eframe::App for Pattern {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        fit_to_monitor(ui.ctx());
        let rect = ui.max_rect();
        let time = ui.input(|i| i.time) as f32;
        let (cols, rows) = GRID;
        for row in 0..rows {
            for col in 0..cols {
                let cell = Rect::from_min_max(
                    rect.lerp_inside(egui::vec2(
                        col as f32 / cols as f32,
                        row as f32 / rows as f32,
                    )),
                    rect.lerp_inside(egui::vec2(
                        (col + 1) as f32 / cols as f32,
                        (row + 1) as f32 / rows as f32,
                    )),
                );
                let color = match self.mode {
                    Mode::Static => static_color(col, cols),
                    Mode::Slow => {
                        let hue = (col as f32 / cols as f32
                            + row as f32 / rows as f32 * 0.5
                            + time / 8.0)
                            .fract();
                        Hsva::new(hue, 0.8, 0.8, 1.0).into()
                    }
                    Mode::Fast => random_color(self.tick, row * cols + col),
                };
                ui.painter().rect_filled(cell, 0.0, color);
            }
        }

        match self.mode {
            Mode::Static => {}
            Mode::Slow => ui.ctx().request_repaint(),
            Mode::Fast => {
                self.tick += 1;
                ui.ctx().request_repaint_after(FAST_INTERVAL);
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Covers the primary monitor exactly, correcting the window until it does.
fn fit_to_monitor(ctx: &egui::Context) {
    let (monitor, outer) = ctx.input(|i| (i.viewport().monitor_size, i.viewport().outer_rect));
    let (Some(monitor), Some(outer)) = (monitor, outer) else {
        ctx.request_repaint();
        return;
    };
    let target = Rect::from_min_size(egui::Pos2::ZERO, monitor);
    let off = (outer.min - target.min).length() + (outer.size() - target.size()).length();
    if off > 0.5 {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(target.min));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target.size()));
        ctx.request_repaint();
    }
}

fn static_color(col: u32, cols: u32) -> Color32 {
    match col * 3 / cols {
        0 => Color32::from_rgb(200, 40, 40),
        1 => Color32::from_rgb(90, 90, 90),
        _ => Color32::from_rgb(40, 60, 200),
    }
}

/// Deterministic pseudo-random color per frame and cell (splitmix64).
fn random_color(tick: u64, cell: u32) -> Color32 {
    let mut x = tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ u64::from(cell);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let [r, g, b, ..] = x.to_le_bytes();
    Color32::from_rgb(r, g, b)
}
