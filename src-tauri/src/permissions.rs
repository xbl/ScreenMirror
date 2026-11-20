#[cfg(target_os = "macos")]
pub fn check_screen_recording_permission() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    if unsafe { CGPreflightScreenCaptureAccess() } {
        return true;
    }

    // On macOS 15+ (Sequoia) and for ad-hoc / locally signed builds,
    // CGPreflightScreenCaptureAccess() has a known bug returning false even after
    // permission has been enabled in System Settings.
    // As an authoritative fallback, check if we can inspect shareable windows
    // from other processes (macOS strips titles & app names when denied).
    can_read_windows_fallback()
}

#[cfg(target_os = "macos")]
fn can_read_windows_fallback() -> bool {
    if let Ok(windows) = xcap::Window::all() {
        for w in windows {
            if let Ok(title) = w.title() {
                if !title.is_empty() {
                    return true;
                }
            }
            if let Ok(app_name) = w.app_name() {
                if !app_name.is_empty() && app_name != "Window Server" {
                    return true;
                }
            }
        }
    }
    false
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

#[cfg(target_os = "macos")]
pub fn request_and_open_screen_recording_settings() -> Result<(), String> {
    // Trigger system dialog and ensure the app is registered in macOS TCC database
    let _ = request_screen_recording_permission();
    open_screen_recording_settings()
}

#[cfg(target_os = "macos")]
pub fn check_accessibility_permission() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
pub fn request_accessibility_permission() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();
    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const std::ffi::c_void) }
}

#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map(|_| ())
        .map_err(|e| format!("failed to launch System Settings: {e}"))
}

#[cfg(target_os = "macos")]
pub fn request_and_open_accessibility_settings() -> Result<(), String> {
    let _ = request_accessibility_permission();
    open_accessibility_settings()
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

#[cfg(not(target_os = "macos"))]
pub fn request_and_open_screen_recording_settings() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permission() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility_permission() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn request_and_open_accessibility_settings() -> Result<(), String> {
    Ok(())
}
