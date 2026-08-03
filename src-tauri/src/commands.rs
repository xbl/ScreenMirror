use crate::network;
use crate::permissions;
use crate::signaling::devices::{ConnectedDevicesService, Device};
use crate::signaling::handlers::HostPeerMap;
use crate::signaling::room_id::RoomIDService;
use crate::storage::Storage;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

pub struct CommandState {
    pub room_ids: Arc<Mutex<RoomIDService>>,
    pub devices: Arc<Mutex<ConnectedDevicesService>>,
    pub port: Arc<Mutex<u16>>,
    pub waiting_session_id: Arc<Mutex<Option<String>>>,
    pub waiting_source_id: Arc<Mutex<Option<String>>>,
    pub capture_target: Arc<Mutex<Option<crate::webrtc::CaptureTarget>>>,
    /// Serializes the prepare/commit transaction across every active peer.
    pub capture_target_switch_lock: Arc<Mutex<()>>,
    pub host_peers: HostPeerMap,
    pub shared_capture: Arc<crate::webrtc::SharedCapture>,
    pub mobile_capture: Arc<crate::webrtc::SharedCapture>,
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
#[allow(unreachable_code)]
pub fn relaunch_app(app: AppHandle) -> Result<(), String> {
    app.restart();
    Ok(())
}

#[tauri::command]
pub fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_connected_devices(state: State<'_, CommandState>) -> Vec<Device> {
    state.devices.lock().get_devices()
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

#[tauri::command]
pub async fn get_capture_source_preview(
    source_id: String,
    force_refresh: bool,
    source_kind: Option<String>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::webrtc::get_capture_source_preview(
            &source_id,
            force_refresh,
            source_kind.as_deref().unwrap_or("screen"),
        )
    })
    .await
    .map_err(|error| format!("capture source preview task failed: {error}"))?
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTargetArgs {
    pub kind: String,
    pub id: u32,
    pub source_id: Option<String>,
    pub quality: Option<f32>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTargetState {
    pub kind: String,
    pub id: u32,
    pub source_id: Option<String>,
    pub quality: f32,
}

#[tauri::command]
pub fn get_capture_target(state: State<'_, CommandState>) -> Option<CaptureTargetState> {
    state
        .capture_target
        .lock()
        .as_ref()
        .map(|target| CaptureTargetState {
            kind: match target.kind {
                crate::webrtc::CaptureKind::Screen => "screen",
                crate::webrtc::CaptureKind::Window => "window",
                crate::webrtc::CaptureKind::TestPattern => "test",
            }
            .into(),
            id: target.id,
            source_id: target.source_id.clone(),
            quality: target.quality,
        })
}

#[tauri::command]
pub fn open_source_picker_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("source-picker") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        app.emit("source-picker-opened", ())
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        "source-picker",
        tauri::WebviewUrl::App("index.html?source-picker=1".into()),
    )
    .title("Screenmirror")
    .inner_size(720.0, 430.0)
    .min_inner_size(560.0, 400.0)
    .resizable(false)
    // The picker owns its chrome so it can match the AirPlay-style layout.
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn close_source_picker_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("source-picker") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn start_source_picker_drag(app: AppHandle) -> Result<(), String> {
    app.get_webview_window("source-picker")
        .ok_or_else(|| "source picker window not found".to_string())?
        .start_dragging()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn close_tray_panel(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("tray-panel") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn validate_target_for_empty_peer_set<F>(
    target: &crate::webrtc::CaptureTarget,
    validate: F,
) -> Result<(), String>
where
    F: FnOnce(&crate::webrtc::CaptureTarget) -> Result<(), String>,
{
    validate(target)
}

#[tauri::command]
pub fn set_capture_target(
    state: State<'_, CommandState>,
    app: AppHandle,
    args: CaptureTargetArgs,
) -> Result<(), String> {
    let kind = match args.kind.as_str() {
        "screen" => crate::webrtc::CaptureKind::Screen,
        "window" => crate::webrtc::CaptureKind::Window,
        "test" | "testpattern" => crate::webrtc::CaptureKind::TestPattern,
        _ => return Err(format!("unknown kind {}", args.kind)),
    };
    let target = crate::webrtc::CaptureTarget {
        kind,
        id: args.id,
        source_id: args.source_id,
        quality: args.quality.unwrap_or(0.75),
    };
    let target_event = CaptureTargetState {
        kind: args.kind,
        id: args.id,
        source_id: target.source_id.clone(),
        quality: target.quality,
    };
    let fps = crate::webrtc::profile_fps(target.quality);
    let _transaction = state.capture_target_switch_lock.lock();
    let has_peers = !state.host_peers.lock().is_empty();
    if !has_peers {
        validate_target_for_empty_peer_set(&target, |candidate| {
            crate::webrtc::capture_one_at(candidate, 0).map(|_| ())
        })?;
    }
    if has_peers {
        state.shared_capture.switch_target(target.clone(), fps)?;
        let mut mobile_target = target.clone();
        mobile_target.quality = mobile_target.quality.min(0.5);
        state.mobile_capture.switch_target(
            mobile_target.clone(),
            crate::webrtc::profile_fps(mobile_target.quality),
        )?;
    }
    *state.capture_target.lock() = Some(target);
    app.emit("capture-target-changed", target_event)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_target_for_empty_peer_set;
    use crate::webrtc::{CaptureKind, CaptureTarget};

    #[test]
    fn invalid_target_is_rejected_before_empty_peer_state_can_publish_it() {
        let target = CaptureTarget {
            kind: CaptureKind::Screen,
            id: 99,
            source_id: Some("missing".into()),
            quality: 0.75,
        };

        let error = validate_target_for_empty_peer_set(&target, |_| {
            Err("screen source missing is unavailable".into())
        })
        .expect_err("invalid targets must not be accepted without peers");

        assert_eq!(error, "screen source missing is unavailable");
    }
}
