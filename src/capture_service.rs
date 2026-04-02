use crate::core::SharedSettings;
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext,
};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::GraphicsCaptureApi,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, TryIntoCaptureItemWithType,
    },
};

const CAPTURE_WIDTH: u32 = 120;

pub struct CaptureProcessor {
    settings: SharedSettings,
}

impl GraphicsCaptureApiHandler for CaptureProcessor {
    type Flags = SharedSettings;
    type Error = anyhow::Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            settings: ctx.flags,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: windows_capture::graphics_capture_api::InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let width = frame.width();
        let height = frame.height();
        let mut buffer = frame.buffer()?;
        let raw_data = buffer.as_raw_buffer();

        let l_avg = self.get_edge_avg(raw_data, width, height, true);
        let r_avg = self.get_edge_avg(raw_data, width, height, false);

        let mut s = self.settings.write();
        let (sl, br) = (s.smoothing, s.brightness);

        for i in 0..3 {
            s.left_color[i] += (l_avg[i] * br - s.left_color[i]) * sl;
            s.right_color[i] += (r_avg[i] * br - s.right_color[i]) * sl;
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl CaptureProcessor {
    fn get_edge_avg(&self, buffer: &[u8], w: u32, h: u32, is_left: bool) -> [f32; 3] {
        let mut r = 0u64;
        let mut g = 0u64;
        let mut b = 0u64;
        let mut count = 0u64;

        let start_x = if is_left {
            0
        } else {
            w.saturating_sub(CAPTURE_WIDTH)
        };
        let end_x = if is_left { CAPTURE_WIDTH.min(w) } else { w };

        for y in (0..h).step_by(20) {
            for x in (start_x..end_x).step_by(20) {
                let offset = ((y * w + x) * 4) as usize;
                if offset + 2 < buffer.len() {
                    r += buffer[offset] as u64;
                    g += buffer[offset + 1] as u64;
                    b += buffer[offset + 2] as u64;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return [0.0, 0.0, 0.0];
        }
        [
            (r / count) as f32 / 255.0,
            (g / count) as f32 / 255.0,
            (b / count) as f32 / 255.0,
        ]
    }
}

pub fn start_capture_thread(settings: SharedSettings) {
    std::thread::spawn(move || {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED).ok().unwrap() };
        let monitor = Monitor::primary().expect("Monitor not found");
        let mut d3d_device: Option<ID3D11Device> = None;
        let mut d3d_device_context: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE(std::ptr::null_mut()),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                Some(&mut d3d_device_context),
            )
            .unwrap();
        }
        let d3d_device = d3d_device.unwrap();
        let d3d_device_context = d3d_device_context.unwrap();
        let (capture_item, item_type) = monitor.try_into_capture_item().unwrap();

        let callback = Arc::new(Mutex::new(
            CaptureProcessor::new(Context {
                flags: settings,
                device: d3d_device.clone(),
                device_context: d3d_device_context.clone(),
            })
            .unwrap(),
        ));

        let mut api = GraphicsCaptureApi::new(
            d3d_device,
            d3d_device_context,
            capture_item,
            item_type,
            callback,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Exclude,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
            Arc::new(Mutex::new(None)),
        )
        .unwrap();

        api.start_capture().unwrap();
        unsafe {
            let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
            while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0)
                .as_bool()
            {
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
            }
        }
    });
}
