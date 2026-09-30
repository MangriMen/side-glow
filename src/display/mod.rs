//! Physical monitor geometry and the mapping of capture zones onto neighbouring monitors.

mod enumerate;
pub mod layout;

pub use enumerate::enumerate_monitors;

use crate::config::MonitorId;

/// Rectangle in physical pixels of the virtual desktop. `right`/`bottom` are exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PxRect {
    #[cfg(test)]
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        Self {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MonitorInfo {
    pub id: MonitorId,
    /// Raw `HMONITOR`. Only valid until the display configuration changes.
    pub handle: isize,
    pub rect: PxRect,
    /// DPI scale factor (1.0 = 96 DPI).
    pub scale: f32,
    pub is_primary: bool,
}
