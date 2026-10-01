use super::{MonitorInfo, PxRect};
use crate::config::MonitorId;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EDD_GET_DEVICE_INTERFACE_NAME, MONITORINFOF_PRIMARY,
};
use windows::core::{BOOL, PCWSTR};

/// Lists the connected monitors with their physical rectangles.
///
/// Coordinates are physical pixels only because winit makes the process per-monitor DPI
/// aware, so this must run after the event loop has been created.
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let handles = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
        handles.push(monitor);
        true.into()
    }
    let ok = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
        )
    };
    if !ok.as_bool() {
        log::error!("EnumDisplayMonitors failed");
    }

    let mut monitors: Vec<MonitorInfo> = handles.into_iter().filter_map(monitor_info).collect();
    monitors.sort_by_key(|m| (m.rect.left, m.rect.top));
    disambiguate_names(&mut monitors);
    monitors
}

/// Identical models get the same friendly name; add the display number to tell them apart.
fn disambiguate_names(monitors: &mut [MonitorInfo]) {
    let names: Vec<String> = monitors.iter().map(|m| m.id.name.clone()).collect();
    for monitor in monitors.iter_mut() {
        if names.iter().filter(|n| **n == monitor.id.name).count() > 1 {
            let number = monitor.id.device_name.trim_start_matches(r"\\.\");
            monitor.id.name = format!("{} ({number})", monitor.id.name);
        }
    }
}

fn monitor_info(handle: HMONITOR) -> Option<MonitorInfo> {
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(handle, &mut info.monitorInfo) }.as_bool() {
        log::warn!("GetMonitorInfoW failed for {handle:?}");
        return None;
    }
    let rc = info.monitorInfo.rcMonitor;
    let device_name = wide_to_string(&info.szDevice);

    let (mut dpi_x, mut dpi_y) = (96, 96);
    if let Err(err) = unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
    {
        log::warn!("GetDpiForMonitor failed for {device_name}: {err}");
    }

    let (device_path, device_string) = device_interface(&info.szDevice);
    let name = windows_capture::monitor::Monitor::from_raw_hmonitor(handle.0)
        .name()
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or(device_string);

    Some(MonitorInfo {
        id: MonitorId {
            device_path,
            device_name,
            name,
        },
        handle: handle.0 as isize,
        rect: PxRect {
            left: rc.left,
            top: rc.top,
            right: rc.right,
            bottom: rc.bottom,
        },
        scale: dpi_x as f32 / 96.0,
        is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

/// Returns the monitor's device interface path and its description.
fn device_interface(adapter_device: &[u16]) -> (String, String) {
    let mut device = DISPLAY_DEVICEW {
        cb: size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    let ok = unsafe {
        EnumDisplayDevicesW(
            PCWSTR(adapter_device.as_ptr()),
            0,
            &mut device,
            EDD_GET_DEVICE_INTERFACE_NAME,
        )
    };
    if ok.as_bool() {
        (
            wide_to_string(&device.DeviceID),
            wide_to_string(&device.DeviceString),
        )
    } else {
        (String::new(), String::new())
    }
}

fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}
