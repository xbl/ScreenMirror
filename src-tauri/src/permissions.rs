#[cfg(target_os = "macos")]
pub fn check_screen_recording_permission() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
pub fn request_screen_recording_permission() -> bool {
    // Best-effort nudge. After explicit denial macOS may NOT show the prompt
    // again; this call returns the new state without guaranteeing a UI event.
    // Callers must follow up with `check_screen_recording_permission()`.
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
pub fn open_screen_recording_settings() -> Result<(), String> {
    // `x-apple.systempreferences:` is a private scheme; tauri-plugin-opener
    // may refuse it. Use `open` directly to be reliable across macOS versions.
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .status()
        .map(|_| ())
        .map_err(|e| format!("failed to launch System Settings: {e}"))
}

#[cfg(not(target_os = "macos"))]
pub fn check_screen_recording_permission() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording_permission() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn open_screen_recording_settings() -> Result<(), String> {
    Ok(())
}
