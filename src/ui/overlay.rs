//! Shared plumbing for the borderless always-on-top windows (glow and zone preview).

use crate::display::PxRect;
use eframe::egui::{self, ViewportBuilder, ViewportCommand};
use windows::core::HSTRING;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
};

/// Placement corrections to try before giving up (e.g. if Windows refuses the size).
const MAX_PLACEMENT_ATTEMPTS: u32 = 8;

pub fn overlay_builder(
    title: &str,
    rect: PxRect,
    scale: f32,
    passthrough: bool,
) -> ViewportBuilder {
    // Creation positions are logical; `OverlayState::update` fixes them up in physical pixels.
    let scale = scale.max(0.1);
    ViewportBuilder::default()
        .with_title(title)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_resizable(false)
        .with_active(false)
        .with_mouse_passthrough(passthrough)
        .with_position([rect.left as f32 / scale, rect.top as f32 / scale])
        .with_inner_size([rect.width() as f32 / scale, rect.height() as f32 / scale])
}

/// Per-window state that keeps an overlay exactly on its physical rectangle.
#[derive(Default)]
pub struct OverlayState {
    placed_for: Option<PxRect>,
    attempts: u32,
    capture_excluded: bool,
}

impl OverlayState {
    /// Called from inside the window's own viewport pass.
    ///
    /// egui positions are in points of the window's current monitor, which is ambiguous
    /// with mixed DPI. So the actual physical rectangle is compared with the wanted one
    /// and corrected until they match.
    pub fn update(&mut self, ctx: &egui::Context, title: &str, rect: PxRect) {
        if !self.capture_excluded {
            // Also mark it done on failure: retrying FindWindow every frame is not worth it.
            self.capture_excluded = true;
            exclude_from_capture(title);
        }

        if self.placed_for != Some(rect) {
            self.placed_for = Some(rect);
            self.attempts = 0;
        }
        if self.attempts >= MAX_PLACEMENT_ATTEMPTS {
            return;
        }
        let Some(outer) = ctx.input(|i| i.viewport().outer_rect) else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        let actual = PxRect {
            left: (outer.min.x * ppp).round() as i32,
            top: (outer.min.y * ppp).round() as i32,
            right: (outer.max.x * ppp).round() as i32,
            bottom: (outer.max.y * ppp).round() as i32,
        };
        let close = |a: i32, b: i32| (a - b).abs() <= 1;
        if close(actual.left, rect.left)
            && close(actual.top, rect.top)
            && close(actual.width(), rect.width())
            && close(actual.height(), rect.height())
        {
            log::debug!(
                "'{title}' placed at {rect:?} after {} corrections",
                self.attempts
            );
            self.attempts = MAX_PLACEMENT_ATTEMPTS;
            return;
        }

        self.attempts += 1;
        if self.attempts == MAX_PLACEMENT_ATTEMPTS {
            log::warn!("could not place '{title}' at {rect:?}, it is at {actual:?}");
        }
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(
            rect.left as f32 / ppp,
            rect.top as f32 / ppp,
        )));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(
            rect.width() as f32 / ppp,
            rect.height() as f32 / ppp,
        )));
        ctx.request_repaint();
    }
}

/// Keeps our own windows out of screen capture, so the glow and the zone frames never
/// feed back into the colors that are sampled.
fn exclude_from_capture(title: &str) {
    let result = unsafe {
        FindWindowW(None, &HSTRING::from(title))
            .and_then(|hwnd| SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE))
    };
    if let Err(err) = result {
        log::warn!("cannot exclude '{title}' from capture: {err}");
    }
}
