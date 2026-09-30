use crate::capture::{plan_sources, CaptureService};
use crate::config::store::ConfigStore;
use crate::config::{Config, MonitorId, OutputKey};
use crate::display::layout::{self, ResolvedOutput};
use crate::display::{self, MonitorInfo};
use crate::glow::bus::ColorBus;
use crate::glow::glow_viewport_id;
use crate::ui::glow_window::{GlowSpec, GlowWindow};
use crate::ui::settings::{self, SettingsAction, SettingsView};
use crate::ui::tray::{Tray, TrayCommand};
use crate::ui::zone_preview::{PreviewSpec, ZonePreview};
use eframe::egui::{self, ViewportCommand};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MONITOR_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// eframe shows the root window after its first frame, so it is hidden again for a few.
const STARTUP_HIDE_FRAMES: u64 = 3;

/// What the auto layout was last computed from.
type LayoutInputs = (Vec<MonitorInfo>, Vec<MonitorId>);

/// The root viewport is the (usually hidden) settings window. Its `logic` pass drives
/// everything else: monitors, capture sessions and the glow windows, which are deferred
/// viewports that repaint on their own when new colors arrive.
pub struct SideGlowApp {
    config: Config,
    store: ConfigStore,
    monitors: Vec<MonitorInfo>,
    monitors_polled: Instant,
    layout_inputs: Option<LayoutInputs>,
    outputs: Vec<ResolvedOutput>,
    bus: Arc<ColorBus>,
    capture: CaptureService,
    glows: HashMap<OutputKey, GlowWindow>,
    previews: HashMap<OutputKey, ZonePreview>,
    tray: Tray,
    show_settings: bool,
    shown_settings: Option<bool>,
    paused: bool,
    quitting: bool,
}

impl SideGlowApp {
    pub fn new(cc: &eframe::CreationContext<'_>, icon: tray_icon::Icon) -> anyhow::Result<Self> {
        let ctx = &cc.egui_ctx;
        let (store, config) = ConfigStore::load();
        let bus = Arc::new(ColorBus::new(ctx.clone()));
        let monitors = display::enumerate_monitors();
        log::info!("monitors: {}", describe(&monitors));

        Ok(Self {
            config,
            store,
            monitors,
            monitors_polled: Instant::now(),
            layout_inputs: None,
            outputs: Vec::new(),
            capture: CaptureService::new(bus.clone()),
            bus,
            glows: HashMap::new(),
            previews: HashMap::new(),
            tray: Tray::new(icon, ctx)?,
            show_settings: false,
            shown_settings: None,
            paused: false,
            quitting: false,
        })
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        let commands: Vec<_> = self.tray.poll().collect();
        for command in commands {
            match command {
                TrayCommand::ShowSettings => self.show_settings = true,
                TrayCommand::TogglePause => self.set_paused(!self.paused),
                TrayCommand::Quit => self.quit(ctx),
            }
        }
    }

    fn handle_action(&mut self, ctx: &egui::Context, action: SettingsAction) {
        match action {
            SettingsAction::TogglePause => self.set_paused(!self.paused),
            SettingsAction::Quit => self.quit(ctx),
            SettingsAction::ResetDefaults => {
                self.config = Config::default();
                self.layout_inputs = None;
            }
            SettingsAction::DetectLayout => self.apply_layout(true),
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.tray.set_paused(paused);
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        self.shutdown();
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn shutdown(&mut self) {
        self.store.flush(&self.config);
        self.capture.stop_all();
    }

    fn update_settings_visibility(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            // Closing the settings window only hides it; the app lives in the tray.
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.show_settings = false;
        }

        let starting = ctx.cumulative_frame_nr() < STARTUP_HIDE_FRAMES;
        if self.shown_settings != Some(self.show_settings) || starting {
            ctx.send_viewport_cmd(ViewportCommand::Visible(self.show_settings));
            if self.show_settings && self.shown_settings != Some(true) {
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            self.shown_settings = Some(self.show_settings);
            if starting {
                ctx.request_repaint();
            }
        }
    }

    fn refresh_monitors(&mut self) {
        if self.monitors_polled.elapsed() < MONITOR_POLL_INTERVAL {
            return;
        }
        self.monitors_polled = Instant::now();
        let monitors = display::enumerate_monitors();
        if monitors != self.monitors {
            log::info!("monitor configuration changed: {}", describe(&monitors));
            self.monitors = monitors;
        }
    }

    /// Rebuilds the outputs from the monitor layout when it (or the chosen sources) changed.
    fn apply_layout(&mut self, force: bool) {
        if !self.config.auto_layout && !force {
            return;
        }
        let inputs = (self.monitors.clone(), self.config.sources.clone());
        if !force && self.layout_inputs.as_ref() == Some(&inputs) {
            return;
        }
        let sources = layout::source_monitors(&self.config, &self.monitors);
        self.config.outputs = layout::auto_outputs(&self.monitors, &sources, &self.config.outputs);
        self.layout_inputs = Some(inputs);
    }

    /// Resolves the outputs and brings capture sessions and windows in line with them.
    /// Returns when this needs to run again for a pending capture restart.
    fn sync_outputs(&mut self, ctx: &egui::Context) -> Option<Duration> {
        self.outputs = layout::resolve(&self.config, &self.monitors);
        let active: &[ResolvedOutput] = if self.paused { &[] } else { &self.outputs };

        let capture = &self.config.capture;
        let plans = plan_sources(active, capture.target_fps, capture.sample_stride);
        let wake = self.capture.sync(&plans);
        self.bus.retain(|key| active.iter().any(|o| o.key == key));

        // Windows that are not shown in this pass are closed by egui.
        let mut glows = HashMap::with_capacity(active.len());
        for output in active {
            let spec = GlowSpec::new(output, &self.config.look);
            let window = match self.glows.remove(&output.key) {
                Some(mut window) => {
                    if window.set_spec(spec) {
                        ctx.request_repaint_of(glow_viewport_id(output.key));
                    }
                    window
                }
                None => GlowWindow::new(spec),
            };
            window.show(ctx, &self.bus);
            glows.insert(output.key, window);
        }
        self.glows = glows;

        let mut previews = HashMap::new();
        if self.config.show_zone_preview {
            for output in active {
                let spec = PreviewSpec::new(output);
                let preview = match self.previews.remove(&output.key) {
                    Some(mut preview) => {
                        if preview.set_spec(spec) {
                            preview.request_repaint(ctx);
                        }
                        preview
                    }
                    None => ZonePreview::new(spec),
                };
                preview.show(ctx);
                previews.insert(output.key, preview);
            }
        }
        self.previews = previews;

        wake
    }
}

impl eframe::App for SideGlowApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_tray(ctx);
        if self.quitting {
            return;
        }
        self.update_settings_visibility(ctx);
        self.refresh_monitors();
        self.apply_layout(false);

        let capture_wake = self.sync_outputs(ctx);
        let save_wake = self.store.tick(&self.config);
        let poll_wake = MONITOR_POLL_INTERVAL.saturating_sub(self.monitors_polled.elapsed());
        let wake = [capture_wake, save_wake]
            .into_iter()
            .flatten()
            .fold(poll_wake, Duration::min);
        ctx.request_repaint_after(wake);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if !self.show_settings || self.quitting {
            return;
        }
        let before = self.config.clone();
        let status = self.capture.status();
        let action = egui::CentralPanel::default()
            .show(ui, |ui| {
                settings::show(
                    ui,
                    SettingsView {
                        config: &mut self.config,
                        monitors: &self.monitors,
                        outputs: &self.outputs,
                        status: &status,
                        bus: &self.bus,
                        paused: self.paused,
                    },
                )
            })
            .inner;

        if let Some(action) = action {
            self.handle_action(ui.ctx(), action);
        }
        if self.config != before {
            // Apply the change in the next logic pass right away.
            ui.ctx().request_repaint();
        }
    }

    fn on_exit(&mut self) {
        self.shutdown();
    }
}

fn describe(monitors: &[MonitorInfo]) -> String {
    monitors
        .iter()
        .map(|m| {
            format!(
                "{} {}x{} at ({}, {}) x{:.2}{}",
                m.id.display_name(),
                m.rect.width(),
                m.rect.height(),
                m.rect.left,
                m.rect.top,
                m.scale,
                if m.is_primary { " primary" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}
