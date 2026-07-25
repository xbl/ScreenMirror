use screenmirror_lib::permissions::{
    check_screen_recording_permission, open_screen_recording_settings,
    request_screen_recording_permission,
};

#[test]
fn check_returns_bool() {
    let _: bool = check_screen_recording_permission();
}

#[test]
fn request_returns_bool() {
    let _: bool = request_screen_recording_permission();
}

#[test]
fn open_settings_returns_result_unit() {
    // On non-macOS this is Ok(()) without launching anything.
    // On macOS this launches System Settings — skip in environments where
    // we don't want a UI side-effect.
    #[cfg(not(target_os = "macos"))]
    {
        let r = open_screen_recording_settings();
        assert!(r.is_ok(), "open_screen_recording_settings must Ok on non-macOS");
    }
}