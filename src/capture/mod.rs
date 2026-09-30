//! Screen capture sessions: one Windows Graphics Capture session per source monitor.

mod handler;
pub mod sampler;

use crate::config::{Edge, OutputKey};
use crate::display::layout::ResolvedOutput;
use crate::display::MonitorInfo;
use crate::glow::bus::ColorBus;
use handler::{CaptureFlags, ZoneCapture};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows_capture::capture::{CaptureControl, GraphicsCaptureApiHandler};
use windows_capture::graphics_capture_api::GraphicsCaptureApi;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

/// Wait this long after the last FPS change before restarting a session with it.
const FPS_RESTART_DELAY: Duration = Duration::from_millis(400);
const ERROR_RETRY_DELAY: Duration = Duration::from_secs(3);

/// Everything a capture session needs to know about one source monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct SourcePlan {
    pub monitor: MonitorInfo,
    pub target_fps: u32,
    pub sample_stride: u32,
    pub zones: Vec<ZonePlan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZonePlan {
    pub key: OutputKey,
    pub edge: Edge,
    pub depth_px: u32,
    /// Segment spans along the edge, as fractions of the frame size.
    pub spans: Vec<(f32, f32)>,
}

/// Groups outputs by their source monitor.
pub fn plan_sources(outputs: &[ResolvedOutput], target_fps: u32, stride: u32) -> Vec<SourcePlan> {
    let mut plans: Vec<SourcePlan> = Vec::new();
    for output in outputs {
        let zone = ZonePlan {
            key: output.key,
            edge: output.config.source_edge,
            depth_px: output.config.zone_depth_px,
            spans: output.segments.source_spans(output.config.segments),
        };
        match plans
            .iter_mut()
            .find(|p| p.monitor.handle == output.source.handle)
        {
            Some(plan) => plan.zones.push(zone),
            None => plans.push(SourcePlan {
                monitor: output.source.clone(),
                target_fps,
                sample_stride: stride,
                zones: vec![zone],
            }),
        }
    }
    plans
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Running,
    Starting,
    Failed(String),
}

struct Session {
    name: String,
    plan: Arc<Mutex<Arc<SourcePlan>>>,
    control: Option<CaptureControl<ZoneCapture, anyhow::Error>>,
    /// FPS the running session was started with (its MinUpdateInterval).
    session_fps: u32,
    restart_at: Option<Instant>,
    error: Option<String>,
}

pub struct CaptureService {
    bus: Arc<ColorBus>,
    sessions: HashMap<isize, Session>,
}

impl CaptureService {
    pub fn new(bus: Arc<ColorBus>) -> Self {
        Self {
            bus,
            sessions: HashMap::new(),
        }
    }

    /// Brings the running sessions in line with `plans`. Zone changes are applied live;
    /// FPS changes and failures restart the session after a delay.
    ///
    /// Returns when `sync` should run again to perform a pending restart.
    pub fn sync(&mut self, plans: &[SourcePlan]) -> Option<Duration> {
        let now = Instant::now();

        self.sessions.retain(|handle, session| {
            let keep = plans.iter().any(|p| p.monitor.handle == *handle);
            if !keep {
                session.stop();
            }
            keep
        });

        for plan in plans {
            let session = self
                .sessions
                .entry(plan.monitor.handle)
                .or_insert_with(|| Session {
                    name: plan.monitor.id.display_name().to_owned(),
                    plan: Arc::new(Mutex::new(Arc::new(plan.clone()))),
                    control: None,
                    session_fps: plan.target_fps,
                    restart_at: Some(now),
                    error: None,
                });

            if **session.plan.lock() != *plan {
                *session.plan.lock() = Arc::new(plan.clone());
            }

            if session.control.as_ref().is_some_and(|c| c.is_finished()) {
                let control = session.control.take().expect("checked above");
                let error = match control.wait() {
                    Ok(()) => "capture stopped".to_owned(),
                    Err(err) => err.to_string(),
                };
                log::warn!("capture of {} ended: {error}", session.name);
                session.error = Some(error);
                session.restart_at = Some(now + ERROR_RETRY_DELAY);
            }

            if session.control.is_some()
                && session.session_fps != plan.target_fps
                && session.restart_at.is_none()
            {
                session.restart_at = Some(now + FPS_RESTART_DELAY);
            }

            if session.restart_at.is_some_and(|at| at <= now) {
                session.restart(&self.bus);
                if session.control.is_none() {
                    session.restart_at = Some(now + ERROR_RETRY_DELAY);
                }
            }
        }

        let next = self.sessions.values().filter_map(|s| s.restart_at).min()?;
        Some(
            next.saturating_duration_since(now)
                .max(Duration::from_millis(16)),
        )
    }

    pub fn status(&self) -> Vec<(String, SessionStatus)> {
        let mut status: Vec<_> = self
            .sessions
            .values()
            .map(|s| {
                let state = match (&s.control, &s.error) {
                    (Some(_), _) => SessionStatus::Running,
                    (None, Some(err)) => SessionStatus::Failed(err.clone()),
                    (None, None) => SessionStatus::Starting,
                };
                (s.name.clone(), state)
            })
            .collect();
        status.sort_by(|a, b| a.0.cmp(&b.0));
        status
    }

    pub fn stop_all(&mut self) {
        for (_, mut session) in self.sessions.drain() {
            session.stop();
        }
    }
}

impl Drop for CaptureService {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl Session {
    fn stop(&mut self) {
        if let Some(control) = self.control.take() {
            if let Err(err) = control.stop() {
                log::warn!("failed to stop capture of {}: {err}", self.name);
            }
        }
    }

    fn restart(&mut self, bus: &Arc<ColorBus>) {
        self.stop();
        self.restart_at = None;

        let plan = self.plan.lock().clone();
        let monitor = Monitor::from_raw_hmonitor(plan.monitor.handle as *mut std::ffi::c_void);
        let interval = match GraphicsCaptureApi::is_minimum_update_interval_supported() {
            Ok(true) => MinimumUpdateIntervalSettings::Custom(
                Duration::from_secs(1) / plan.target_fps.max(1),
            ),
            _ => MinimumUpdateIntervalSettings::Default,
        };
        let settings = Settings::new(
            monitor,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Exclude,
            interval,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            CaptureFlags {
                plan: self.plan.clone(),
                bus: bus.clone(),
            },
        );

        match ZoneCapture::start_free_threaded(settings) {
            Ok(control) => {
                log::info!(
                    "capturing {} at {} fps ({} zones)",
                    self.name,
                    plan.target_fps,
                    plan.zones.len()
                );
                self.control = Some(control);
                self.session_fps = plan.target_fps;
                self.error = None;
            }
            Err(err) => {
                log::error!("failed to start capture of {}: {err}", self.name);
                self.error = Some(err.to_string());
            }
        }
    }
}
