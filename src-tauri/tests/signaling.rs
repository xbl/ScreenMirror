use axum::http::StatusCode;
use parking_lot::Mutex;
use screenmirror_lib::signaling::devices::ConnectedDevicesService;
use screenmirror_lib::signaling::handlers::{build_router, parse_message};
use screenmirror_lib::signaling::handlers::HostPeerMap;
use screenmirror_lib::signaling::room::Room;
use screenmirror_lib::signaling::room_id::RoomIDService;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn room_new_is_unlocked() {
    let r = Room::new("123456".into());
    assert!(!r.is_locked);
}

#[test]
fn room_toggle_lock() {
    let mut r = Room::new("123456".into());
    r.toggle_lock();
    assert!(r.is_locked);
    r.toggle_lock();
    assert!(!r.is_locked);
}

#[test]
fn parse_message_user_enter() {
    let json = r#"{"type":"USER_ENTER","payload":{"username":"alice","ip":"127.0.0.1"}}"#;
    let m = parse_message(json).expect("parse");
    assert_eq!(m.type_str(), "USER_ENTER");
}

#[test]
fn parse_message_invalid_json() {
    assert!(parse_message("not json").is_err());
}

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let room_ids = Arc::new(Mutex::new(RoomIDService::new()));
    let devices = Arc::new(Mutex::new(ConnectedDevicesService::new()));
    let viewer_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("viewer")
        .join("dist");
    let port = Arc::new(Mutex::new(0u16));
    let sinks = Arc::new(Mutex::new(HashMap::new()));
    let peers: HostPeerMap = Arc::new(Mutex::new(HashMap::new()));
    let app = build_router(
        room_ids,
        devices,
        viewer_path,
        Arc::new(Mutex::new(None)),
        port,
        sinks,
        peers,
        Arc::new(Mutex::new(())),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let url = format!("http://{}/api/health", addr);
    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
