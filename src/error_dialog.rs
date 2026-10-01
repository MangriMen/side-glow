//! Release builds have no console, so a failed start would otherwise be invisible.

#[cfg(windows)]
pub fn show(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::{HSTRING, w};

    // SAFETY: both strings outlive the call, and no window handle is passed.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            w!("SideGlow"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
pub fn show(message: &str) {
    eprintln!("{message}");
}
