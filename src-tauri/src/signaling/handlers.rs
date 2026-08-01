use crate::signaling::devices::ConnectedDevicesService;
use crate::signaling::room_id::RoomIDService;
use crate::webrtc::HostPeer;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WireMessage {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub payload: Value,
}

impl WireMessage {
    pub fn type_str(&self) -> &str {
        &self.type_
    }
}

pub fn parse_message(raw: &str) -> Result<WireMessage, serde_json::Error> {
    serde_json::from_str(raw)
}

/// Map of room_id → sender to push messages to the connected viewer.
pub type ViewerSinkMap =
    Arc<Mutex<std::collections::HashMap<String, mpsc::UnboundedSender<String>>>>;
pub type HostPeerMap = Arc<Mutex<std::collections::HashMap<String, Arc<HostPeer>>>>;

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct AppState {
    pub room_ids: Arc<Mutex<RoomIDService>>,
    pub devices: Arc<Mutex<ConnectedDevicesService>>,
    pub viewer_path: PathBuf,
    /// Active host peer per viewer connection. Multiple peers may share a room.
    pub host_peers: HostPeerMap,
    /// Per-connection viewer sink. When host creates an answer, push it here.
    pub viewer_sinks: ViewerSinkMap,
    /// Optional capture target selected by the host UI for this session.
    pub capture_target: Arc<Mutex<Option<crate::webrtc::CaptureTarget>>>,
    /// Shared with the Tauri command so joining peers cannot miss a target
    /// switch that is taking its active-peer snapshot.
    pub capture_target_switch_lock: Arc<Mutex<()>>,
    /// Bound signaling port (after fallback). Used by `/api/host-info` so the
    /// host UI can build a complete QR URL from a single HTTP fetch.
    pub port: Arc<Mutex<u16>>,
}

pub fn build_router(
    room_ids: Arc<Mutex<RoomIDService>>,
    devices: Arc<Mutex<ConnectedDevicesService>>,
    viewer_path: PathBuf,
    capture_target: Arc<Mutex<Option<crate::webrtc::CaptureTarget>>>,
    port: Arc<Mutex<u16>>,
    viewer_sinks: ViewerSinkMap,
    host_peers: HostPeerMap,
    capture_target_switch_lock: Arc<Mutex<()>>,
) -> Router {
    let state = AppState {
        room_ids,
        devices,
        viewer_path: viewer_path.clone(),
        host_peers,
        viewer_sinks,
        capture_target,
        capture_target_switch_lock,
        port,
    };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/host-info", get(host_info))
        .route("/api/ws", get(ws_handler))
        // Serve assets at /assets/{filename} from viewer/dist/assets/
        .route("/assets/:filename", get(serve_asset))
        // Serve root index.html
        .route("/", get(serve_root))
        // All other paths fall back to SPA index.html
        .fallback(spa_fallback)
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Returns enough info for a QR code: the LAN IP the host is bound to, the
/// current port, and the list of connected / pending devices. Used by the E2E
/// harness and any headless consumer that wants the same data the host UI
/// builds its QR from.
async fn host_info(State(state): State<AppState>) -> impl IntoResponse {
    let lan_ip = crate::network::get_lan_ip(None).unwrap_or_else(|| "127.0.0.1".to_string());
    let port = *state.port.lock();
    let pending = state.devices.lock().get_pending();
    let connected: Vec<String> = state
        .devices
        .lock()
        .get_devices()
        .into_iter()
        .map(|d| d.id)
        .collect();
    let body = serde_json::json!({
        "lan_ip": lan_ip,
        "port": port,
        "pending_device": pending.map(|d| d.id),
        "connected_devices": connected,
    });
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        body.to_string(),
    )
}

async fn spa_fallback(State(state): State<AppState>, path: Option<Path<String>>) -> Response {
    let path_str = path.map(|p| p.0).unwrap_or_default();
    let requested = state.viewer_path.join(&path_str);
    if !path_str.is_empty() && requested.is_file() {
        if let Ok(bytes) = tokio::fs::read(&requested).await {
            return (
                StatusCode::OK,
                [("content-type", guess_mime(&path_str))],
                bytes,
            )
                .into_response();
        }
    }
    let index = state.viewer_path.join("index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => (
            StatusCode::OK,
            [("content-type", "text/html"), ("cache-control", "no-store")],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "viewer bundle missing").into_response(),
    }
}

async fn serve_root(State(state): State<AppState>) -> Response {
    let index = state.viewer_path.join("index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => (
            StatusCode::OK,
            [("content-type", "text/html"), ("cache-control", "no-store")],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "viewer dist not found").into_response(),
    }
}

async fn serve_asset(
    State(state): State<AppState>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Response {
    let requested = state.viewer_path.join("assets").join(&filename);
    match tokio::fs::read(&requested).await {
        Ok(bytes) => (
            StatusCode::OK,
            [("content-type", guess_mime(&filename))],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "asset not found").into_response(),
    }
}

fn guess_mime(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let room_id = q.get("roomId").cloned().unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, state, room_id))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, room_id: String) {
    // Throttle: 500ms delay before checking roomId (anti-malicious connections).
    tracing::info!("WS handler entered for room={}", room_id);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    tracing::info!("WS handler past throttle for room={}", room_id);

    if !state.room_ids.lock().is_taken(&room_id) {
        tracing::info!("WS rejected: room {} not taken", room_id);
        let _ = socket
            .send(Message::Text(r#"{"type":"NOT_ALLOWED"}"#.into()))
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let _ = socket.close().await;
        return;
    }

    let connection_id = format!(
        "{}-{}",
        room_id,
        NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
    );

    // Per-connection outbound mpsc — host pushes (ANSWER/ICE) here directly.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    state
        .viewer_sinks
        .lock()
        .insert(connection_id.clone(), out_tx.clone());

    while let Some(msg) = socket.recv().await {
        tracing::info!(
            "WS recv: {:?}",
            msg.as_ref().ok().map(|m| format!("{:?}", m))
        );
        match msg {
            Ok(Message::Text(text)) => {
                let parsed = match parse_message(&text) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                match parsed.type_str() {
                    "PING" => {
                        tracing::info!("PING received");
                        let _ = socket
                            .send(Message::Text(r#"{"type":"PONG"}"#.into()))
                            .await;
                    }
                    "GET_MY_IP" => {
                        let _ = socket
                            .send(Message::Text(
                                r#"{"type":"MY_IP","payload":{"ip":"127.0.0.1"}}"#.into(),
                            ))
                            .await;
                    }
                    "OFFER" => {
                        tracing::info!("OFFER received, room_id={}", room_id);
                        if let Some(sdp) = parsed.payload.get("sdp").and_then(|v| v.as_str()) {
                            let (answer_result, peer_opt) = {
                                let _target_switch = state.capture_target_switch_lock.lock();
                                let peer_entry = {
                                    let mut peers = state.host_peers.lock();
                                    peers
                                                .entry(connection_id.clone())
                                                .or_insert_with(|| {
                                                    let p = Arc::new(HostPeer::new());
                                                    let bind_ip = std::env::var("SCREENMIRROR_HOST_IP")
                                                        .ok()
                                                        .and_then(|s| s.parse::<std::net::IpAddr>().ok())
                                                        .or_else(|| {
                                                            crate::network::get_lan_ip(None)
                                                                .and_then(|s| s.parse::<std::net::IpAddr>().ok())
                                                        })
                                                        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
                                                    tracing::info!("HostPeer::init bind_ip={:?} (env SCREENMIRROR_HOST_IP={:?})", bind_ip, std::env::var("SCREENMIRROR_HOST_IP").ok());
                                                    if let Err(e) = p.init(bind_ip) {
                                                        tracing::error!("HostPeer::init: {e}");
                                                    }
                                                    p
                                                })
                                                .clone()
                                };
                                let res = peer_entry.accept_offer(sdp);
                                if res.is_ok() {
                                    let target =
                                        state.capture_target.lock().clone().or_else(|| {
                                            tracing::warn!(
                                            "no capture target selected; defaulting to screen 0"
                                        );
                                            Some(crate::webrtc::CaptureTarget {
                                                kind: crate::webrtc::CaptureKind::Screen,
                                                id: 0,
                                                source_id: None,
                                                quality: 0.75,
                                            })
                                        });
                                    if let Some(target) = target {
                                        let fps = crate::webrtc::profile_fps(target.quality);
                                        if let Err(error) = peer_entry.clone().start_sharing(
                                            target,
                                            fps,
                                            state.capture_target_switch_lock.clone(),
                                        ) {
                                            tracing::error!("start_sharing: {error}");
                                        }
                                    }
                                }
                                (res, Some(peer_entry))
                            };
                            match answer_result {
                                Ok(answer) => {
                                    tracing::info!("sending ANSWER ({} bytes)", answer.len());
                                    if let Some(sink) =
                                        state.viewer_sinks.lock().get(&connection_id).cloned()
                                    {
                                        let _ = sink.send(
                                            serde_json::json!({"type": "ANSWER", "payload": {"sdp": answer}})
                                                .to_string(),
                                        );
                                    }
                                    // LAN viewers are auto-approved; each connection is
                                    // tracked independently so multiple clients can join.
                                    let viewer_label =
                                        std::env::var("SCREENMIRROR_E2E_VIEWER_NAME")
                                            .unwrap_or_else(|_| "Browser Viewer".into());
                                    let device = crate::signaling::devices::Device {
                                        id: connection_id.clone(),
                                        name: viewer_label,
                                        ip: "127.0.0.1".into(),
                                        os: "browser".into(),
                                        browser: "Chrome".into(),
                                        room_id: room_id.clone(),
                                        sharing_session_id: room_id.clone(),
                                    };
                                    state.devices.lock().add_device(device).ok();
                                    if let Some(sink) =
                                        state.viewer_sinks.lock().get(&connection_id).cloned()
                                    {
                                        let _ = sink.send(
                                            serde_json::json!({"type": "ALLOWED_TO_CONNECT"})
                                                .to_string(),
                                        );
                                    }
                                    let _ = peer_opt;
                                }
                                Err(e) => {
                                    tracing::warn!("accept_offer failed: {e}");
                                    let _ = socket
                                        .send(Message::Text(
                                            serde_json::json!({"type": "ERROR", "payload": {"message": e}})
                                                .to_string(),
                                        ))
                                        .await;
                                }
                            }
                        }
                    }
                    "ICE_CANDIDATE" => {
                        tracing::debug!("received trickle ICE candidate; candidates are gathered in the SDP offer");
                    }
                    _ => {}
                }
            }
            Ok(Message::Binary(_)) => {}
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }

        while let Ok(msg) = out_rx.try_recv() {
            let _ = socket.send(Message::Text(msg)).await;
        }
    }

    state.viewer_sinks.lock().remove(&connection_id);
    state.devices.lock().release_device(&connection_id);
    let _target_switch = state.capture_target_switch_lock.lock();
    if let Some(peer) = state.host_peers.lock().remove(&connection_id) {
        peer.stop();
    }
}
