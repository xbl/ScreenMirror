//! Standalone signaling server binary (no GUI). Useful for smoke testing
//! without launching the Tauri runtime.
//!
//! Run with: cargo run --bin screenmirror-server
//! Then visit: http://127.0.0.1:3131/api/health

use parking_lot::Mutex;
use screenmirror_lib::signaling::devices::ConnectedDevicesService;
use screenmirror_lib::signaling::handlers::build_router;
use screenmirror_lib::signaling::room_id::RoomIDService;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let viewer_path = std::env::var("VIEWER_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("viewer")
                .join("dist")
        });

    let room_ids = Arc::new(Mutex::new(RoomIDService::new()));
    let devices = Arc::new(Mutex::new(ConnectedDevicesService::new()));
    let capture_target = Arc::new(Mutex::new(None::<screenmirror_lib::webrtc::CaptureTarget>));
    let port = Arc::new(Mutex::new(3131_u16));

    // For smoke testing: register a room on startup so WS clients can connect
    // without going through the Tauri IPC layer.
    if let Ok(rid) = std::env::var("SCREENMIRROR_TEST_ROOM") {
        room_ids.lock().mark_taken(&rid);
        tracing::info!("smoke room registered: {rid}");
    }

    // For E2E: auto-pick the first screen as the capture target so frames flow.
    if let Ok(kind) = std::env::var("SCREENMIRROR_CAPTURE") {
        let target = screenmirror_lib::webrtc::CaptureTarget {
            kind: if kind == "window" {
                screenmirror_lib::webrtc::CaptureKind::Window
            } else {
                screenmirror_lib::webrtc::CaptureKind::Screen
            },
            id: 0,
            source_id: None,
            quality: 0.75,
        };
        tracing::info!("capture target set: {:?}", target);
        *capture_target.lock() = Some(target);
    }

    // For E2E: bind to LAN IP (not loopback) so both ends can agree.
    if std::env::var("SCREENMIRROR_USE_LAN_IP").is_ok() {
        if let Some(lan) = screenmirror_lib::network::get_lan_ip(None) {
            tracing::info!("using LAN IP for ICE: {}", lan);
            // Note: actual binding happens in HostPeer::init when first peer connects.
            // We pass via env to inform the first peer.
            std::env::set_var("SCREENMIRROR_HOST_IP", &lan);
        }
    }
    let capture_target = Arc::new(Mutex::new(None::<screenmirror_lib::webrtc::CaptureTarget>));
    let viewer_sinks =
        std::sync::Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new()));
    let app = build_router(
        room_ids,
        devices,
        viewer_path,
        capture_target.clone(),
        port.clone(),
        viewer_sinks,
    );

    let addr: std::net::SocketAddr = std::env::var("SCREENMIRROR_PORT")
        .map(|p| format!("0.0.0.0:{p}"))
        .unwrap_or_else(|_| "0.0.0.0:3131".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    *port.lock() = bound.port();
    tracing::info!(
        "screenmirror standalone signaling server on http://{}",
        bound
    );
    axum::serve(listener, app).await?;
    Ok(())
}
