use super::SourcePlan;
use super::sampler::ZoneSampler;
use crate::display::layout::capture_zone;
use crate::glow::bus::ColorBus;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;

pub struct CaptureFlags {
    /// Updated live by the UI; read once per frame.
    pub plan: Arc<Mutex<Arc<SourcePlan>>>,
    pub bus: Arc<ColorBus>,
}

/// Samples the capture zones of one source monitor on every (throttled) frame.
pub struct ZoneCapture {
    plan: Arc<Mutex<Arc<SourcePlan>>>,
    bus: Arc<ColorBus>,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    samplers: Vec<ZoneSampler>,
    last_frame: Option<Instant>,
}

impl GraphicsCaptureApiHandler for ZoneCapture {
    type Flags = CaptureFlags;
    type Error = anyhow::Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            plan: ctx.flags.plan,
            bus: ctx.flags.bus,
            device: ctx.device,
            context: ctx.device_context,
            samplers: Vec::new(),
            last_frame: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let plan = self.plan.lock().clone();

        // MinUpdateInterval is not available on every Windows build and is fixed for the
        // session lifetime, so the live FPS setting is enforced here as well.
        let interval = Duration::from_secs(1) / plan.target_fps.max(1);
        let now = Instant::now();
        if self.last_frame.is_some_and(|last| now - last < interval) {
            return Ok(());
        }
        self.last_frame = Some(now);

        self.samplers
            .resize_with(plan.zones.len(), ZoneSampler::default);
        let texture = frame.as_raw_texture();
        let (width, height) = (frame.width(), frame.height());

        for (zone, sampler) in plan.zones.iter().zip(&mut self.samplers) {
            let rect = capture_zone(width, height, zone.edge, zone.depth_px);
            let colors = sampler.sample(
                &self.device,
                &self.context,
                texture,
                rect,
                zone.edge.is_vertical(),
                &zone.spans,
                plan.sample_stride,
            )?;
            self.bus.publish(zone.key, colors);
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        log::info!("capture item closed");
        Ok(())
    }
}
