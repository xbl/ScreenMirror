use crate::network;
use crate::permissions;
use crate::signaling::devices::{ConnectedDevicesService, Device};
use crate::signaling::handlers::ViewerSinkMap;
use crate::signaling::room_id::RoomIDService;
use crate::storage::Storage;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

pub struct CommandState {
    pub room_ids: Arc<Mutex<RoomIDService>>,
    pub devices: Arc<Mutex<ConnectedDevicesService>>,
    pub port: Arc<Mutex<u16>>,
    pub waiting_session_id: Arc<Mutex<Option<String>>>,
    pub waiting_source_id: Arc<Mutex<Option<String>>>,
    pub capture_target: Arc<Mutex<Option<crate::webrtc::CaptureTarget>>>,
    pub viewer_sinks:
        Arc<Mutex<std::collections::HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>>,
}

#[tauri::command]
pub fn get_lan_ip(cli_ip: State<'_, Option<String>>) -> Option<String> {
    network::get_lan_ip(cli_ip.as_deref())
}

#[tauri::command]
pub fn check_wifi_connection() -> bool {
    network::is_wifi_connected()
}

#[tauri::command]
pub fn check_screen_recording_permission() -> bool {
    permissions::check_screen_recording_permission()
}

#[tauri::command]
pub fn request_screen_recording_permission() -> bool {
    permissions::request_screen_recording_permission()
}

#[tauri::command]
pub fn open_screen_recording_settings() -> Result<(), String> {
    permissions::open_screen_recording_settings()
}

#[tauri::command]
pub fn get_port(state: State<'_, CommandState>) -> u16 {
    *state.port.lock()
}

#[tauri::command]
pub fn get_app_language() -> String {
    Storage::open()
        .ok()
        .and_then(|s| s.get_string("language"))
        .unwrap_or_else(|| "en".into())
}

#[tauri::command]
pub fn set_app_language(lang: String) -> Result<(), String> {
    let mut s = Storage::open().map_err(|e| e.to_string())?;
    s.set_string("language", &lang);
    Ok(())
}

#[tauri::command]
pub fn get_is_first_time_start() -> bool {
    Storage::open()
        .ok()
        .and_then(|s| s.get_string("appStartedOnce"))
        .is_none()
}

#[tauri::command]
pub fn set_app_started_once() -> Result<(), String> {
    let mut s = Storage::open().map_err(|e| e.to_string())?;
    s.set_string("appStartedOnce", "true");
    Ok(())
}

#[tauri::command]
pub fn get_current_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn open_external_link(app: AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_text_to_clipboard(_text: String) -> Result<(), String> {
    // No clipboard plugin dependency for v0.1; this is a stub.
    // Frontend uses navigator.clipboard as fallback.
    Ok(())
}

#[tauri::command]
pub fn relaunch_app(app: AppHandle) -> Result<(), String> {
    app.restart();
    Ok(())
}

#[tauri::command]
pub fn get_connected_devices(state: State<'_, CommandState>) -> Vec<Device> {
    state.devices.lock().get_devices()
}

#[tauri::command]
pub fn disconnect_device(state: State<'_, CommandState>, id: String) -> bool {
    state.devices.lock().release_device(&id)
}

#[tauri::command]
pub fn disconnect_all_devices(state: State<'_, CommandState>) {
    state.devices.lock().release_all();
}

#[tauri::command]
pub fn is_viewer_slot_available(state: State<'_, CommandState>) -> bool {
    state.devices.lock().is_slot_available()
}

#[tauri::command]
pub fn create_waiting_session(
    state: State<'_, CommandState>,
    room_id: Option<String>,
) -> Result<String, String> {
    let id = room_id.unwrap_or_else(|| state.room_ids.lock().get_simple_available_room_id());
    state.room_ids.lock().mark_taken(&id);
    *state.waiting_session_id.lock() = Some(id.clone());
    Ok(id)
}

#[tauri::command]
pub fn reset_waiting_session(state: State<'_, CommandState>) -> Result<(), String> {
    let mut ids = state.room_ids.lock();
    if let Some(prev) = state.waiting_session_id.lock().take() {
        ids.unmark_taken(&prev);
    }
    *state.waiting_source_id.lock() = None;
    Ok(())
}

#[tauri::command]
pub fn set_desktop_capturer_source_id(
    state: State<'_, CommandState>,
    id: String,
) -> Result<(), String> {
    *state.waiting_source_id.lock() = Some(id);
    Ok(())
}

#[tauri::command]
pub fn get_waiting_source_id(state: State<'_, CommandState>) -> Option<String> {
    state.waiting_source_id.lock().clone()
}

#[tauri::command]
pub fn start_sharing(state: State<'_, CommandState>) -> Result<(), String> {
    let pending = state.devices.lock().get_pending();
    if let Some(p) = pending {
        state
            .devices
            .lock()
            .add_device(p)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_pending_device(state: State<'_, CommandState>) -> Option<Device> {
    state.devices.lock().get_pending()
}

#[tauri::command]
pub fn set_device_connected_status(state: State<'_, CommandState>) -> Result<(), String> {
    let pending = state.devices.lock().get_pending();
    if let Some(p) = pending {
        let room_id = p.room_id.clone();
        state
            .devices
            .lock()
            .add_device(p)
            .map_err(|e| e.to_string())?;
        // Notify the waiting viewer that it's been approved so the WS
        // handshake can advance from PENDING to ALLOWED_TO_CONNECT.
        if let Some(sink) = state.viewer_sinks.lock().get(&room_id).cloned() {
            let _ = sink.send(serde_json::json!({"type": "ALLOWED_TO_CONNECT"}).to_string());
            tracing::info!("approved viewer in room {}", room_id);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn enumerate_capture_sources() -> Result<Vec<crate::webrtc::CaptureSourceInfo>, String> {
    crate::webrtc::enumerate_sources()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTargetArgs {
    pub kind: String,
    pub id: u32,
    pub source_id: Option<String>,
    pub quality: Option<f32>,
}

#[tauri::command]
pub fn set_capture_target(
    state: State<'_, CommandState>,
    args: CaptureTargetArgs,
) -> Result<(), String> {
    let kind = match args.kind.as_str() {
        "screen" => crate::webrtc::CaptureKind::Screen,
        "window" => crate::webrtc::CaptureKind::Window,
        "test" | "testpattern" => crate::webrtc::CaptureKind::TestPattern,
        _ => return Err(format!("unknown kind {}", args.kind)),
    };
    *state.capture_target.lock() = Some(crate::webrtc::CaptureTarget {
        kind,
        id: args.id,
        source_id: args.source_id,
        quality: args.quality.unwrap_or(0.75),
    });
    Ok(())
}
