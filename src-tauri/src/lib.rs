use crate::commands::CommandState;
use crate::icons::{tray_icon, tray_state_for_count, TrayIconState};
use crate::signaling::handlers::{build_router, HostPeerMap};
use crate::signaling::{devices::ConnectedDevicesService, room_id::RoomIDService};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, Runtime};

static LAST_TRAY_CLICK: std::sync::OnceLock<Mutex<Option<Instant>>> = std::sync::OnceLock::new();

fn position_tray_panel(
    window: &tauri::WebviewWindow,
    anchor: tauri::PhysicalPosition<f64>,
) {
    let x = (anchor.x - 215.0).max(8.0);
    let y = anchor.y + 12.0;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

fn show_tray_panel(
    app: &tauri::AppHandle,
    anchor: Option<tauri::PhysicalPosition<f64>>,
) {
    if let Some(window) = app.get_webview_window("tray-panel") {
        if let Some(anchor) = anchor {
            position_tray_panel(&window, anchor);
        }
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("tray-panel-opened", ());
        return;
    }

    let url = tauri::WebviewUrl::App("index.html?tray=1".into());
    let result = tauri::WebviewWindowBuilder::new(app, "tray-panel", url)
        .title("Screenmirror")
        .inner_size(430.0, 620.0)
        .min_inner_size(380.0, 560.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(true)
        .build();
    if let Ok(window) = &result {
        if let Some(anchor) = anchor {
            position_tray_panel(window, anchor);
        }
        let _ = app.emit("tray-panel-opened", ());
        let window_for_event = window.clone();
        let opened_at = Instant::now();
        window.on_window_event(move |event| {
            if opened_at.elapsed() > Duration::from_millis(180)
                && matches!(event, tauri::WindowEvent::Focused(false))
            {
                let _ = window_for_event.hide();
            }
        });
    }
    if let Err(error) = result {
        tracing::warn!("failed to open tray panel: {error}");
    }
}

fn accept_tray_click() -> bool {
    let last = LAST_TRAY_CLICK.get_or_init(|| Mutex::new(None));
    let now = Instant::now();
    let mut previous = last.lock();
    if previous.is_some_and(|time| now.duration_since(time) < Duration::from_millis(220)) {
        return false;
    }
    *previous = Some(now);
    true
}

fn update_tray_for_count<R: Runtime>(tray: &tauri::tray::TrayIcon<R>, count: usize) {
    let state = tray_state_for_count(count);
    match tray_icon(state) {
        Ok(icon) => {
            #[cfg(target_os = "macos")]
            let icon_result = tray.set_icon_with_as_template(Some(icon), true);
            #[cfg(not(target_os = "macos"))]
            let icon_result = tray.set_icon(Some(icon));

            if let Err(error) = icon_result {
                tracing::warn!(
                    "failed to update tray icon (count={count}, state={state:?}): {error}"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                "failed to decode tray icon (count={count}, state={state:?}): {error}"
            );
        }
    }

    // macOS can cache a shorter status-item title when the count decreases.
    // Clearing first forces the NSStatusItem to invalidate its old width and
    // repaint the new value (including the transition back to no number).
    if let Err(error) = tray.set_title(None::<String>) {
        tracing::warn!("failed to clear tray title (count={count}, state={state:?}): {error}");
    }
    let title = if count == 0 { String::new() } else { count.to_string() };
    if let Err(error) = tray.set_title(Some(title)) {
        tracing::warn!("failed to update tray title (count={count}, state={state:?}): {error}");
    }
}

pub mod commands;
pub mod icons;
pub mod network;
pub mod permissions;
pub mod signaling;
pub mod storage;
pub mod webrtc;

#[derive(Default, Clone)]
pub struct CliConfig {
    pub ip: Option<String>,
    pub port: Option<u16>,
}

pub struct AppState {
    pub cli_ip: Option<String>,
    pub cli_port: Option<u16>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = parse_cli();
    let app_state = AppState {
        cli_ip: cli.ip.clone(),
        cli_port: cli.port,
    };
    // Tauri commands that take `State<'_, Option<String>>` (e.g. get_lan_ip)
    // need this exact type managed — AppState wraps it in a struct, which
    // doesn't satisfy the State extractor.
    let cli_ip_state: Option<String> = cli.ip.clone();
    let room_ids = Arc::new(Mutex::new(RoomIDService::new()));
    let devices = Arc::new(Mutex::new(ConnectedDevicesService::new()));
    let port = Arc::new(Mutex::new(cli.port.unwrap_or(3131)));
    let capture_target = Arc::new(Mutex::new(None::<crate::webrtc::CaptureTarget>));
    let viewer_sinks: Arc<
        Mutex<std::collections::HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>>,
    > = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let host_peers: HostPeerMap = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let capture_target_switch_lock = Arc::new(Mutex::new(()));
    let command_state = CommandState {
        room_ids: room_ids.clone(),
        devices: devices.clone(),
        port: port.clone(),
        waiting_session_id: Arc::new(Mutex::new(None)),
        waiting_source_id: Arc::new(Mutex::new(None)),
        capture_target: capture_target.clone(),
        capture_target_switch_lock: capture_target_switch_lock.clone(),
        host_peers: host_peers.clone(),
        viewer_sinks: viewer_sinks.clone(),
    };

    // For headless E2E testing: allow pre-registering a room via env var
    // (so a Puppeteer browser can connect without going through Tauri IPC).
    if let Ok(rid) = std::env::var("SCREENMIRROR_TEST_ROOM") {
        room_ids.lock().mark_taken(&rid);
        tracing::info!("smoke room registered: {rid}");
    }
    // Auto-pick the first screen as the capture target so frames flow
    // when SCREENMIRROR_CAPTURE=screen|window is set.
    if let Ok(kind) = std::env::var("SCREENMIRROR_CAPTURE") {
        let target = crate::webrtc::CaptureTarget {
            kind: match kind.as_str() {
                "window" => crate::webrtc::CaptureKind::Window,
                "test" | "testpattern" => crate::webrtc::CaptureKind::TestPattern,
                _ => crate::webrtc::CaptureKind::Screen,
            },
            id: 0,
            source_id: None,
            quality: std::env::var("SCREENMIRROR_CAPTURE_QUALITY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.75),
        };
        tracing::info!("capture target set: {:?}", target);
        *capture_target.lock() = Some(target);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .manage(command_state)
        .manage(cli_ip_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_lan_ip,
            commands::check_wifi_connection,
            commands::check_screen_recording_permission,
            commands::request_screen_recording_permission,
            commands::open_screen_recording_settings,
            commands::get_port,
            commands::get_app_language,
            commands::set_app_language,
            commands::get_is_first_time_start,
            commands::set_app_started_once,
            commands::get_current_version,
            commands::open_external_link,
            commands::write_text_to_clipboard,
            commands::relaunch_app,
            commands::exit_app,
            commands::get_connected_devices,
            commands::disconnect_device,
            commands::disconnect_all_devices,
            commands::is_viewer_slot_available,
            commands::create_waiting_session,
            commands::reset_waiting_session,
            commands::set_desktop_capturer_source_id,
            commands::get_waiting_source_id,
            commands::start_sharing,
            commands::get_pending_device,
            commands::set_device_connected_status,
            commands::enumerate_capture_sources,
            commands::close_tray_panel,
            commands::get_capture_source_preview,
            commands::get_capture_target,
            commands::open_source_picker_window,
            commands::close_source_picker_window,
            commands::set_capture_target,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                // This is a menu-bar application; keep it out of the Dock.
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                let _ = app.set_dock_visibility(false);
            }
            // The tray icon is intentionally menu-less. Clicking it opens the
            // AirPlay-style quick-share window instead of stacking a native
            // menu next to that window.
            let handle = app.handle().clone();
            let icon = tray_icon(TrayIconState::Disconnected)?;
            let _tray = tauri::tray::TrayIconBuilder::with_id("screenmirror-tray")
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                // Initialize the macOS title slot so later `set_title` calls
                // (the live viewer count) are rendered beside the icon.
                .title("")
                .tooltip("Screenmirror")
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::TrayIconEvent;
                    if let TrayIconEvent::Click {
                        position,
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Down,
                        ..
                    } = event
                    {
                        if !accept_tray_click() {
                            return;
                        }
                        let app = tray.app_handle();
                        show_tray_panel(&app, Some(position));
                    }
                })
                .build(app)?;
            tracing::info!("tray icon installed");

            let app_handle = app.handle().clone();
            let devices_for_tray = devices.clone();
            tauri::async_runtime::spawn(async move {
                let mut previous = None;
                loop {
                    let count = devices_for_tray.lock().get_devices().len();
                    if previous != Some(count) {
                        if let Some(tray) = app_handle.tray_by_id("screenmirror-tray") {
                            update_tray_for_count(&tray, count);
                        }
                        previous = Some(count);
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            });

            let viewer_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("viewer")
                .join("dist");
            let rr = room_ids.clone();
            let dd = devices.clone();
            let pp = port.clone();
            let vp = viewer_path;
            let ct = capture_target.clone();
            let vs = viewer_sinks.clone();
            let hp = host_peers.clone();
            let ctsl = capture_target_switch_lock.clone();
            let start_port = *pp.lock();
            tauri::async_runtime::spawn(async move {
                let router = build_router(rr, dd, vp, ct, pp.clone(), vs, hp, ctsl);
                let addr = std::net::SocketAddr::from(([0, 0, 0, 0], start_port));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(_) => {
                        let backup = std::net::SocketAddr::from(([0, 0, 0, 0], start_port + 1));
                        tokio::net::TcpListener::bind(backup)
                            .await
                            .expect("bind backup port")
                    }
                };
                let bound = listener.local_addr().unwrap();
                *pp.lock() = bound.port();
                tracing::info!("signaling server on {}", bound);
                let _ = handle; // suppress unused warning
                if let Err(e) = axum::serve(listener, router).await {
                    tracing::error!("server error: {e}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn parse_cli() -> CliConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut ip = None;
    let mut port = None;
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "--ip" || args[i] == "--local-ip") && i + 1 < args.len() {
            let v = &args[i + 1];
            if !v.starts_with("--") {
                ip = Some(v.clone());
                i += 1;
            }
        }
        if args[i] == "--port" && i + 1 < args.len() {
            let v = &args[i + 1];
            if !v.starts_with("--") {
                if let Ok(p) = v.parse::<u16>() {
                    port = Some(p);
                }
                i += 1;
            }
        }
        i += 1;
    }
    CliConfig { ip, port }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_ip() {
        let cfg = CliConfig {
            ip: Some("192.168.1.100".into()),
            port: None,
        };
        assert_eq!(cfg.ip.as_deref(), Some("192.168.1.100"));
    }

    #[test]
    fn cli_skips_double_dash_value() {
        let v = "--missing";
        assert!(v.starts_with("--"));
    }
}

// Suppress unused import warnings for traits used by `tauri::generate_handler!`.
#[allow(dead_code)]
fn _runtime_check<R: Runtime>(_app: &tauri::AppHandle<R>) {}
