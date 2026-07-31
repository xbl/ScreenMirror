use crate::commands::CommandState;
use crate::signaling::handlers::build_router;
use crate::signaling::{devices::ConnectedDevicesService, room_id::RoomIDService};
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{Manager, Runtime};

pub mod commands;
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
    let command_state = CommandState {
        room_ids: room_ids.clone(),
        devices: devices.clone(),
        port: port.clone(),
        waiting_session_id: Arc::new(Mutex::new(None)),
        waiting_source_id: Arc::new(Mutex::new(None)),
        capture_target: capture_target.clone(),
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
            quality: std::env::var("SCREENMIRROR_CAPTURE_QUALITY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0),
        };
        *capture_target.lock() = Some(target);
        tracing::info!("capture target set: {:?}", target);
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
            commands::set_capture_target,
        ])
        .setup(move |app| {
            // System tray: "Show window" and "Quit" entries. Without this the
            // app would not appear in the macOS menu bar.
            let handle = app.handle().clone();
            if let Some(icon) = app.default_window_icon().cloned() {
                let _tray = tauri::tray::TrayIconBuilder::with_id("screenmirror-tray")
                    .icon(icon)
                    .tooltip("Screenmirror")
                    .menu(
                        &tauri::menu::Menu::with_items(
                            app,
                            &[
                                &tauri::menu::MenuItem::with_id(
                                    app,
                                    "show",
                                    "Show",
                                    true,
                                    None::<&str>,
                                )
                                .unwrap(),
                                &tauri::menu::MenuItem::with_id(
                                    app,
                                    "quit",
                                    "Quit",
                                    true,
                                    None::<&str>,
                                )
                                .unwrap(),
                            ],
                        )
                        .unwrap(),
                    )
                    .on_menu_event(move |app, event| match event.id().as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        use tauri::tray::TrayIconEvent;
                        if let TrayIconEvent::Click { .. } = event {
                            let app = tray.app_handle();
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    })
                    .build(app)?;
                tracing::info!("tray icon installed");
            } else {
                tracing::warn!("no default window icon; tray disabled");
            }

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
            let start_port = *pp.lock();
            tauri::async_runtime::spawn(async move {
                let router = build_router(rr, dd, vp, ct, pp.clone(), vs);
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
