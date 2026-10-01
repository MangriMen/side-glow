//! Shared plumbing for the borderless always-on-top windows (glow and zone preview).

use crate::display::PxRect;
use eframe::egui::{self, ViewportBuilder, ViewportCommand};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
};
use windows::core::HSTRING;

/// Tells DWM not to draw an accent border, matching `DWMWA_COLOR_NONE` in the Win32 headers
/// (not exposed as a constant by the `windows` crate).
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

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
    tuned: bool,
}

impl OverlayState {
    /// Called from inside the window's own viewport pass.
    ///
    /// egui positions are in points of the window's current monitor, which is ambiguous
    /// with mixed DPI. So the actual physical rectangle is compared with the wanted one
    /// and corrected until they match.
    pub fn update(&mut self, ctx: &egui::Context, title: &str, rect: PxRect) {
        if !self.tuned {
            // Also mark it done on failure: retrying FindWindow every frame is not worth it.
            self.tuned = true;
            tune_window(title);
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

/// One-time Win32 touch-up for a freshly created overlay window:
/// - excludes it from screen capture, so the glow and the zone frames never feed back into
///   the colors that are sampled;
/// - turns off Windows 11's automatic corner rounding and accent border, which otherwise eat
///   a sliver of every edge on an undecorated window and leave whatever is behind it showing
///   through.
fn tune_window(title: &str) {
    let hwnd = match unsafe { FindWindowW(None, &HSTRING::from(title)) } {
        Ok(hwnd) => hwnd,
        Err(err) => {
            log::warn!("cannot find window '{title}' to tune: {err}");
            return;
        }
    };
    if let Err(err) = unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) } {
        log::warn!("cannot exclude '{title}' from capture: {err}");
    }
    let no_round = DWMWCP_DONOTROUND;
    if let Err(err) = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::addr_of!(no_round).cast(),
            size_of_val(&no_round) as u32,
        )
    } {
        log::warn!("cannot disable rounded corners for '{title}': {err}");
    }
    let no_border = DWMWA_COLOR_NONE;
    if let Err(err) = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            std::ptr::addr_of!(no_border).cast(),
            size_of_val(&no_border) as u32,
        )
    } {
        log::warn!("cannot disable the accent border for '{title}': {err}");
    }
}
