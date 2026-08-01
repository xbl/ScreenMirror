use tauri::image::Image;

const TRAY_DISCONNECTED_PNG: &[u8] = include_bytes!("../icons/tray-disconnected.png");
const TRAY_CONNECTED_PNG: &[u8] = include_bytes!("../icons/tray-connected.png");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayIconState {
    Disconnected,
    Connected,
}

pub fn tray_icon(state: TrayIconState) -> tauri::Result<Image<'static>> {
    let bytes = match state {
        TrayIconState::Disconnected => TRAY_DISCONNECTED_PNG,
        TrayIconState::Connected => TRAY_CONNECTED_PNG,
    };
    Image::from_bytes(bytes)
}

pub fn tray_state_for_count(count: usize) -> TrayIconState {
    if count == 0 {
        TrayIconState::Disconnected
    } else {
        TrayIconState::Connected
    }
}
