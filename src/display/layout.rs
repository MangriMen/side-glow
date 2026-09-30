//! Pure geometry: which monitors touch, which outputs the auto layout creates, and how
//! capture segments on a source edge map onto the target monitor.

use super::{MonitorInfo, PxRect};
use crate::config::{Config, Edge, OutputConfig, OutputKey, SegmentMapping};

/// Gap in pixels still treated as "touching" (some drivers leave off-by-one seams).
const ADJACENCY_TOLERANCE: i32 = 2;

/// Returns the edge of `src` that `dst` is attached to, if the monitors share a border.
pub fn adjacent_edge(src: &PxRect, dst: &PxRect) -> Option<Edge> {
    let overlap_x = src.right.min(dst.right) - src.left.max(dst.left);
    let overlap_y = src.bottom.min(dst.bottom) - src.top.max(dst.top);
    let touches = |a: i32, b: i32| (a - b).abs() <= ADJACENCY_TOLERANCE;

    if overlap_y > 0 && touches(dst.right, src.left) {
        Some(Edge::Left)
    } else if overlap_y > 0 && touches(dst.left, src.right) {
        Some(Edge::Right)
    } else if overlap_x > 0 && touches(dst.bottom, src.top) {
        Some(Edge::Top)
    } else if overlap_x > 0 && touches(dst.top, src.bottom) {
        Some(Edge::Bottom)
    } else {
        None
    }
}

/// Monitors captured by the auto layout: the configured ones, or the primary monitor.
pub fn source_monitors<'a>(config: &Config, monitors: &'a [MonitorInfo]) -> Vec<&'a MonitorInfo> {
    let configured: Vec<_> = monitors
        .iter()
        .filter(|m| config.sources.iter().any(|id| id.matches(&m.id)))
        .collect();
    if !configured.is_empty() {
        return configured;
    }
    monitors
        .iter()
        .find(|m| m.is_primary)
        .or(monitors.first())
        .into_iter()
        .collect()
}

/// Builds one output per monitor that touches a source monitor.
///
/// Settings of outputs that already exist are kept. Outputs whose monitors are currently
/// disconnected are kept too, so their settings survive until the monitor comes back.
pub fn auto_outputs(
    monitors: &[MonitorInfo],
    sources: &[&MonitorInfo],
    existing: &[OutputConfig],
) -> Vec<OutputConfig> {
    let is_source = |m: &MonitorInfo| sources.iter().any(|s| s.id.matches(&m.id));
    let is_connected = |id| monitors.iter().any(|m| m.id.matches(id));

    let mut outputs = Vec::new();
    for source in sources {
        for target in monitors.iter().filter(|m| !is_source(m)) {
            let Some(edge) = adjacent_edge(&source.rect, &target.rect) else {
                continue;
            };
            let fresh = OutputConfig {
                source: source.id.clone(),
                target: target.id.clone(),
                source_edge: edge,
                ..Default::default()
            };
            let output = match existing.iter().find(|o| o.same_link(&fresh)) {
                // Keep the user's tuning but refresh names and paths.
                Some(old) => OutputConfig {
                    source: fresh.source,
                    target: fresh.target,
                    ..old.clone()
                },
                None => fresh,
            };
            outputs.push(output);
        }
    }

    let disconnected: Vec<OutputConfig> = existing
        .iter()
        .filter(|o| !is_connected(&o.source) || !is_connected(&o.target))
        .filter(|o| !outputs.iter().any(|n| n.same_link(o)))
        .cloned()
        .collect();
    outputs.extend(disconnected);
    outputs
}

/// An enabled output whose monitors are both connected.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOutput {
    pub key: OutputKey,
    pub config: OutputConfig,
    pub source: MonitorInfo,
    pub target: MonitorInfo,
    pub segments: SegmentLayout,
}

pub fn resolve(config: &Config, monitors: &[MonitorInfo]) -> Vec<ResolvedOutput> {
    let find = |id| monitors.iter().find(|m| m.id.matches(id));
    config
        .outputs
        .iter()
        .filter(|o| o.enabled)
        .filter_map(|o| {
            let source = find(&o.source)?;
            let target = find(&o.target)?;
            if source.handle == target.handle {
                return None;
            }
            Some(ResolvedOutput {
                key: o.key(),
                config: o.clone(),
                source: source.clone(),
                target: target.clone(),
                segments: SegmentLayout::new(o.mapping, o.source_edge, &source.rect, &target.rect),
            })
        })
        .collect()
}

/// Where the segments live along the edge axis (y for left/right edges, x for top/bottom),
/// as fractions of the source and target monitor sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentLayout {
    pub source: (f32, f32),
    pub target: (f32, f32),
}

impl SegmentLayout {
    pub const FULL: Self = Self {
        source: (0.0, 1.0),
        target: (0.0, 1.0),
    };

    pub fn new(mapping: SegmentMapping, edge: Edge, src: &PxRect, dst: &PxRect) -> Self {
        if mapping == SegmentMapping::Stretch {
            return Self::FULL;
        }
        let (src_range, dst_range) = if edge.is_vertical() {
            ((src.top, src.bottom), (dst.top, dst.bottom))
        } else {
            ((src.left, src.right), (dst.left, dst.right))
        };
        let lo = src_range.0.max(dst_range.0);
        let hi = src_range.1.min(dst_range.1);
        if hi <= lo {
            return Self::FULL;
        }
        let fraction = |(start, end): (i32, i32)| {
            let len = (end - start) as f32;
            ((lo - start) as f32 / len, (hi - start) as f32 / len)
        };
        Self {
            source: fraction(src_range),
            target: fraction(dst_range),
        }
    }

    /// Splits the source range into `count` equal spans.
    pub fn source_spans(&self, count: u32) -> Vec<(f32, f32)> {
        let (start, end) = self.source;
        let step = (end - start) / count.max(1) as f32;
        (0..count.max(1))
            .map(|i| (start + step * i as f32, start + step * (i + 1) as f32))
            .collect()
    }

    /// Position of segment `index`'s center on the target edge, as a fraction.
    pub fn target_center(&self, index: usize, count: usize) -> f32 {
        let (start, end) = self.target;
        start + (end - start) * (index as f32 + 0.5) / count.max(1) as f32
    }
}

/// Capture zone `(x0, y0, x1, y1)` inside a `width`×`height` frame for the given edge.
pub fn capture_zone(width: u32, height: u32, edge: Edge, depth: u32) -> (u32, u32, u32, u32) {
    let depth_x = depth.clamp(1, width.max(1));
    let depth_y = depth.clamp(1, height.max(1));
    match edge {
        Edge::Left => (0, 0, depth_x, height),
        Edge::Right => (width - depth_x, 0, width, height),
        Edge::Top => (0, 0, width, depth_y),
        Edge::Bottom => (0, height - depth_y, width, height),
    }
}

/// The sampled part of the source monitor, in virtual desktop pixels.
pub fn zone_rect(output: &ResolvedOutput) -> PxRect {
    let src = &output.source.rect;
    let (x0, y0, x1, y1) = capture_zone(
        src.width().max(0) as u32,
        src.height().max(0) as u32,
        output.config.source_edge,
        output.config.zone_depth_px,
    );
    let mut rect = PxRect {
        left: src.left + x0 as i32,
        top: src.top + y0 as i32,
        right: src.left + x1 as i32,
        bottom: src.top + y1 as i32,
    };
    let (from, to) = output.segments.source;
    if output.config.source_edge.is_vertical() {
        let h = src.height() as f32;
        rect.top = src.top + (h * from).round() as i32;
        rect.bottom = src.top + (h * to).round() as i32;
    } else {
        let w = src.width() as f32;
        rect.left = src.left + (w * from).round() as i32;
        rect.right = src.left + (w * to).round() as i32;
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MonitorId;

    fn monitor(name: &str, rect: PxRect, primary: bool) -> MonitorInfo {
        MonitorInfo {
            id: MonitorId {
                device_path: format!("path-{name}"),
                device_name: format!(r"\\.\{name}"),
                name: name.into(),
            },
            handle: rect.left as isize * 7 + rect.top as isize + 1,
            rect,
            scale: 1.0,
            is_primary: primary,
        }
    }

    fn links(outputs: &[OutputConfig]) -> Vec<(String, Edge)> {
        outputs
            .iter()
            .map(|o| (o.target.name.clone(), o.source_edge))
            .collect()
    }

    #[test]
    fn three_in_a_row() {
        let monitors = [
            monitor("L", PxRect::new(-2560, 0, 2560, 1440), false),
            monitor("C", PxRect::new(0, 0, 2560, 1440), true),
            monitor("R", PxRect::new(2560, 0, 2560, 1440), false),
        ];
        let sources = source_monitors(&Config::default(), &monitors);
        let outputs = auto_outputs(&monitors, &sources, &[]);
        assert_eq!(
            links(&outputs),
            [("L".into(), Edge::Left), ("R".into(), Edge::Right)]
        );
    }

    #[test]
    fn mixed_sizes_offsets_and_portrait() {
        let monitors = [
            // 1080p monitor sitting lower than the main one.
            monitor("L", PxRect::new(-1920, 600, 1920, 1080), false),
            monitor("C", PxRect::new(0, 0, 3840, 2160), true),
            // Portrait monitor, taller than the main one.
            monitor("R", PxRect::new(3840, -400, 1440, 2560), false),
            // Laptop screen below, horizontally centered.
            monitor("B", PxRect::new(960, 2160, 1920, 1200), false),
            // Not touching anything.
            monitor("X", PxRect::new(10000, 0, 1920, 1080), false),
        ];
        let sources = source_monitors(&Config::default(), &monitors);
        let outputs = auto_outputs(&monitors, &sources, &[]);
        assert_eq!(
            links(&outputs),
            [
                ("L".into(), Edge::Left),
                ("R".into(), Edge::Right),
                ("B".into(), Edge::Bottom),
            ]
        );
    }

    #[test]
    fn corner_contact_is_not_adjacent() {
        let src = PxRect::new(0, 0, 100, 100);
        assert_eq!(adjacent_edge(&src, &PxRect::new(100, 100, 100, 100)), None);
        assert_eq!(
            adjacent_edge(&src, &PxRect::new(0, -100, 100, 100)),
            Some(Edge::Top)
        );
        assert_eq!(
            adjacent_edge(&src, &PxRect::new(101, 50, 100, 100)),
            Some(Edge::Right)
        );
    }

    #[test]
    fn two_sources_do_not_glow_on_each_other() {
        let monitors = [
            monitor("A", PxRect::new(0, 0, 1920, 1080), true),
            monitor("B", PxRect::new(1920, 0, 1920, 1080), false),
            monitor("C", PxRect::new(3840, 0, 1920, 1080), false),
        ];
        let config = Config {
            sources: vec![monitors[0].id.clone(), monitors[1].id.clone()],
            ..Default::default()
        };
        let sources = source_monitors(&config, &monitors);
        let outputs = auto_outputs(&monitors, &sources, &[]);
        assert_eq!(links(&outputs), [("C".into(), Edge::Right)]);
        assert_eq!(outputs[0].source.name, "B");
    }

    #[test]
    fn auto_layout_keeps_user_settings_and_disconnected_outputs() {
        let monitors = [
            monitor("C", PxRect::new(0, 0, 1920, 1080), true),
            monitor("R", PxRect::new(1920, 0, 1920, 1080), false),
        ];
        let sources = source_monitors(&Config::default(), &monitors);
        let mut tuned = auto_outputs(&monitors, &sources, &[]);
        tuned[0].segments = 3;
        let unplugged = OutputConfig {
            source: monitors[0].id.clone(),
            target: MonitorId {
                device_path: "path-gone".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        tuned.push(unplugged.clone());

        let outputs = auto_outputs(&monitors, &sources, &tuned);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].segments, 3);
        assert_eq!(outputs[1], unplugged);
    }

    #[test]
    fn aligned_segments_use_the_shared_border() {
        let src = PxRect::new(0, 0, 2560, 1440);
        // Shorter monitor on the right, starting 360 px lower.
        let dst = PxRect::new(2560, 360, 1920, 720);
        let layout = SegmentLayout::new(SegmentMapping::Aligned, Edge::Right, &src, &dst);
        assert_eq!(layout.source, (0.25, 0.75));
        assert_eq!(layout.target, (0.0, 1.0));
        assert_eq!(layout.source_spans(2), [(0.25, 0.5), (0.5, 0.75)]);

        let stretched = SegmentLayout::new(SegmentMapping::Stretch, Edge::Right, &src, &dst);
        assert_eq!(stretched, SegmentLayout::FULL);
        assert_eq!(stretched.target_center(0, 4), 0.125);
    }

    #[test]
    fn capture_zone_is_clamped_to_the_frame() {
        assert_eq!(capture_zone(1920, 1080, Edge::Left, 120), (0, 0, 120, 1080));
        assert_eq!(
            capture_zone(1920, 1080, Edge::Right, 120),
            (1800, 0, 1920, 1080)
        );
        assert_eq!(
            capture_zone(1920, 1080, Edge::Top, 5000),
            (0, 0, 1920, 1080)
        );
        assert_eq!(
            capture_zone(1920, 1080, Edge::Bottom, 100),
            (0, 980, 1920, 1080)
        );
    }

    #[test]
    fn resolve_skips_disabled_and_missing() {
        let monitors = [
            monitor("C", PxRect::new(0, 0, 1920, 1080), true),
            monitor("R", PxRect::new(1920, 0, 1920, 1080), false),
        ];
        let sources = source_monitors(&Config::default(), &monitors);
        let mut outputs = auto_outputs(&monitors, &sources, &[]);
        outputs.push(OutputConfig {
            source: monitors[0].id.clone(),
            target: MonitorId {
                device_path: "path-gone".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut config = Config {
            outputs,
            ..Default::default()
        };
        assert_eq!(resolve(&config, &monitors).len(), 1);
        config.outputs[0].enabled = false;
        assert!(resolve(&config, &monitors).is_empty());
    }
}
