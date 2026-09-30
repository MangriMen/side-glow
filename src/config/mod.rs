//! Persistent user configuration. Everything here is plain data: the rest of the app
//! derives capture plans and window specs from it.

pub mod store;

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

pub const CONFIG_VERSION: u32 = 1;

pub const FPS_RANGE: std::ops::RangeInclusive<u32> = 1..=144;
pub const STRIDE_RANGE: std::ops::RangeInclusive<u32> = 1..=64;
pub const SEGMENTS_RANGE: std::ops::RangeInclusive<u32> = 1..=64;
pub const ZONE_DEPTH_RANGE: std::ops::RangeInclusive<u32> = 8..=1000;
pub const GLOW_DEPTH_RANGE: std::ops::RangeInclusive<f32> = 0.01..=1.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub capture: CaptureConfig,
    pub look: LookConfig,
    /// Rebuild `outputs` from the physical monitor layout whenever it changes.
    pub auto_layout: bool,
    /// Monitors that are captured by the auto layout. Empty means the primary monitor.
    pub sources: Vec<MonitorId>,
    pub show_zone_preview: bool,
    pub outputs: Vec<OutputConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            capture: CaptureConfig::default(),
            look: LookConfig::default(),
            auto_layout: true,
            sources: Vec::new(),
            show_zone_preview: false,
            outputs: Vec::new(),
        }
    }
}

impl Config {
    /// Clamps every value into its valid range, so hand-edited files can't break the app.
    pub fn sanitize(&mut self) {
        self.version = CONFIG_VERSION;
        self.capture.target_fps = clamp_range(self.capture.target_fps, &FPS_RANGE);
        self.capture.sample_stride = clamp_range(self.capture.sample_stride, &STRIDE_RANGE);
        self.look.sanitize();
        for output in &mut self.outputs {
            output.sanitize();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureConfig {
    pub target_fps: u32,
    /// Distance in pixels between sampled pixels inside a capture zone.
    pub sample_stride: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            target_fps: 30,
            sample_stride: 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LookConfig {
    #[serde(serialize_with = "pretty_f32::serialize")]
    pub brightness: f32,
    #[serde(serialize_with = "pretty_f32::serialize")]
    pub saturation: f32,
    /// Time constant of the exponential color smoothing, in milliseconds.
    #[serde(serialize_with = "pretty_f32::serialize")]
    pub smoothing_ms: f32,
    /// Glow reach as a fraction of the target monitor size across the edge.
    #[serde(serialize_with = "pretty_f32::serialize")]
    pub glow_depth: f32,
    pub falloff: Falloff,
}

impl Default for LookConfig {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            saturation: 1.0,
            smoothing_ms: 150.0,
            glow_depth: 0.45,
            falloff: Falloff::Smooth,
        }
    }
}

impl LookConfig {
    fn sanitize(&mut self) {
        self.brightness = clamp_f32(self.brightness, 0.0, 2.0, 1.0);
        self.saturation = clamp_f32(self.saturation, 0.0, 2.0, 1.0);
        self.smoothing_ms = clamp_f32(self.smoothing_ms, 0.0, 2000.0, 150.0);
        self.glow_depth = clamp_f32(
            self.glow_depth,
            *GLOW_DEPTH_RANGE.start(),
            *GLOW_DEPTH_RANGE.end(),
            0.45,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Falloff {
    Linear,
    Smooth,
    Exponential,
}

impl Falloff {
    pub const ALL: [Self; 3] = [Self::Linear, Self::Smooth, Self::Exponential];

    /// Glow intensity at relative distance `t` (0 = edge, 1 = end of the glow).
    pub fn weight(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => 1.0 - t,
            Self::Smooth => (1.0 - t) * (1.0 - t) * (1.0 + 2.0 * t),
            Self::Exponential => {
                const K: f32 = 4.0;
                let end = (-K).exp();
                ((-K * t).exp() - end) / (1.0 - end)
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Smooth => "Smooth",
            Self::Exponential => "Exponential",
        }
    }
}

/// Side of a monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    pub fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }

    /// Left and right edges run vertically, so their segments are stacked along y.
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlowMode {
    /// Transparent click-through overlay on top of whatever the monitor shows.
    Overlay,
    /// The monitor is used only as a light: the glow fades into black.
    Dedicated,
}

impl GlowMode {
    pub const ALL: [Self; 2] = [Self::Overlay, Self::Dedicated];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overlay => "Overlay",
            Self::Dedicated => "Dedicated",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentMapping {
    /// The whole source edge is stretched over the whole target edge.
    Stretch,
    /// Only the part of the edges that physically face each other is used.
    Aligned,
}

impl SegmentMapping {
    pub const ALL: [Self; 2] = [Self::Stretch, Self::Aligned];

    pub fn label(self) -> &'static str {
        match self {
            Self::Stretch => "Stretch",
            Self::Aligned => "Aligned",
        }
    }
}

/// Identifies a physical monitor across reboots and reconnects.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorId {
    /// Device interface path, stable for a given monitor on a given port.
    pub device_path: String,
    /// GDI name such as `\\.\DISPLAY1`; used as a fallback, it may change between boots.
    pub device_name: String,
    /// Human readable name, only for display.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
}

impl MonitorId {
    pub fn matches(&self, other: &MonitorId) -> bool {
        if !self.device_path.is_empty() && !other.device_path.is_empty() {
            return self.device_path == other.device_path;
        }
        !self.device_name.is_empty() && self.device_name == other.device_name
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.device_name
        } else {
            &self.name
        }
    }
}

/// One glow: a capture zone on the `source` monitor lighting up the `target` monitor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub enabled: bool,
    pub source: MonitorId,
    pub target: MonitorId,
    /// Edge of the source monitor that is sampled.
    pub source_edge: Edge,
    /// Depth of the capture zone into the source monitor, in pixels.
    pub zone_depth_px: u32,
    /// Number of independent colors along the edge.
    pub segments: u32,
    pub mapping: SegmentMapping,
    pub mode: GlowMode,
    /// Maximum opacity of the overlay glow.
    #[serde(serialize_with = "pretty_f32::serialize")]
    pub opacity: f32,
    /// Per-output overrides of the global look.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "pretty_f32::serialize_option"
    )]
    pub glow_depth: Option<f32>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "pretty_f32::serialize_option"
    )]
    pub brightness: Option<f32>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source: MonitorId::default(),
            target: MonitorId::default(),
            source_edge: Edge::Left,
            zone_depth_px: 120,
            segments: 8,
            mapping: SegmentMapping::Stretch,
            mode: GlowMode::Overlay,
            opacity: 1.0,
            glow_depth: None,
            brightness: None,
        }
    }
}

impl OutputConfig {
    /// Stable identity of the output; used for window ids and color routing.
    pub fn key(&self) -> OutputKey {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.source.device_path.hash(&mut hasher);
        self.source.device_name.hash(&mut hasher);
        self.target.device_path.hash(&mut hasher);
        self.target.device_name.hash(&mut hasher);
        self.source_edge.hash(&mut hasher);
        OutputKey(hasher.finish())
    }

    /// Whether `other` describes the same source/target/edge link.
    pub fn same_link(&self, other: &OutputConfig) -> bool {
        self.source_edge == other.source_edge
            && self.source.matches(&other.source)
            && self.target.matches(&other.target)
    }

    fn sanitize(&mut self) {
        self.zone_depth_px = clamp_range(self.zone_depth_px, &ZONE_DEPTH_RANGE);
        self.segments = clamp_range(self.segments, &SEGMENTS_RANGE);
        self.opacity = clamp_f32(self.opacity, 0.0, 1.0, 1.0);
        self.glow_depth = self
            .glow_depth
            .map(|d| clamp_f32(d, *GLOW_DEPTH_RANGE.start(), *GLOW_DEPTH_RANGE.end(), 0.45));
        self.brightness = self.brightness.map(|b| clamp_f32(b, 0.0, 2.0, 1.0));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutputKey(pub u64);

/// TOML floats are f64, so `0.45f32` would be written as `0.44999998807907104`.
/// Going through the shortest decimal representation keeps the file readable.
mod pretty_f32 {
    use serde::Serializer;

    fn widen(value: f32) -> f64 {
        value.to_string().parse().unwrap_or(f64::from(value))
    }

    pub fn serialize<S: Serializer>(value: &f32, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(widen(*value))
    }

    pub fn serialize_option<S: Serializer>(
        value: &Option<f32>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&widen(*value)),
            None => serializer.serialize_none(),
        }
    }
}

fn clamp_range(value: u32, range: &std::ops::RangeInclusive<u32>) -> u32 {
    value.clamp(*range.start(), *range.end())
}

fn clamp_f32(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloff_hits_both_ends() {
        for falloff in Falloff::ALL {
            assert!((falloff.weight(0.0) - 1.0).abs() < 1e-5, "{falloff:?}");
            assert!(falloff.weight(1.0).abs() < 1e-5, "{falloff:?}");
            assert!(falloff.weight(0.3) > falloff.weight(0.6), "{falloff:?}");
        }
    }

    #[test]
    fn sanitize_clamps_hand_edited_values() {
        let mut config = Config::default();
        config.capture.target_fps = 0;
        config.look.brightness = f32::NAN;
        config.outputs.push(OutputConfig {
            segments: 0,
            zone_depth_px: 1_000_000,
            ..Default::default()
        });
        config.sanitize();
        assert_eq!(config.capture.target_fps, 1);
        assert_eq!(config.look.brightness, 1.0);
        assert_eq!(config.outputs[0].segments, 1);
        assert_eq!(config.outputs[0].zone_depth_px, *ZONE_DEPTH_RANGE.end());
    }

    #[test]
    fn monitor_id_prefers_device_path() {
        let a = MonitorId {
            device_path: "path-a".into(),
            device_name: r"\\.\DISPLAY1".into(),
            name: String::new(),
        };
        let renumbered = MonitorId {
            device_name: r"\\.\DISPLAY2".into(),
            ..a.clone()
        };
        let other = MonitorId {
            device_path: "path-b".into(),
            ..a.clone()
        };
        assert!(a.matches(&renumbered));
        assert!(!a.matches(&other));
    }
}
