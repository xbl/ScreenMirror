use screenmirror_lib::icons::{tray_icon, tray_state_for_count, TrayIconState};

#[test]
fn tray_icons_decode_to_retina_status_bar_size() {
    for state in [TrayIconState::Disconnected, TrayIconState::Connected] {
        let image = tray_icon(state).expect("embedded tray PNG must decode");
        assert_eq!(image.width(), 44);
        assert_eq!(image.height(), 44);
        assert_eq!(image.rgba().len(), 44 * 44 * 4);
    }
}

#[test]
fn tray_icons_have_transparent_corners_and_visible_content() {
    for state in [TrayIconState::Disconnected, TrayIconState::Connected] {
        let image = tray_icon(state).expect("embedded tray PNG must decode");
        let rgba = image.rgba();
        let corner_offsets = [0, 43 * 4, 43 * 44 * 4, (44 * 44 - 1) * 4];
        for offset in corner_offsets {
            assert_eq!(rgba[offset + 3], 0, "{state:?} corner must be transparent");
        }
        assert!(
            rgba.chunks_exact(4).any(|pixel| pixel[3] > 0),
            "{state:?} must contain visible pixels"
        );
    }
}

#[test]
fn tray_icons_are_monochrome_template_sources() {
    for state in [TrayIconState::Disconnected, TrayIconState::Connected] {
        let image = tray_icon(state).expect("embedded tray PNG must decode");
        for pixel in image.rgba().chunks_exact(4).filter(|pixel| pixel[3] > 0) {
            assert_eq!(pixel[0], pixel[1], "{state:?} red and green differ");
            assert_eq!(pixel[1], pixel[2], "{state:?} green and blue differ");
        }
    }
}

#[test]
fn connected_and_disconnected_icons_are_distinct() {
    let disconnected = tray_icon(TrayIconState::Disconnected).unwrap();
    let connected = tray_icon(TrayIconState::Connected).unwrap();
    assert_ne!(disconnected.rgba(), connected.rgba());
}

#[test]
fn device_count_maps_to_expected_tray_state() {
    assert_eq!(tray_state_for_count(0), TrayIconState::Disconnected);
    assert_eq!(tray_state_for_count(1), TrayIconState::Connected);
    assert_eq!(tray_state_for_count(12), TrayIconState::Connected);
}
