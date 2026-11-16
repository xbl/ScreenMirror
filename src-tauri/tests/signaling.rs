use axum::http::StatusCode;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use screenmirror_lib::signaling::devices::ConnectedDevicesService;
use screenmirror_lib::signaling::handlers::HostPeerMap;
use screenmirror_lib::signaling::handlers::{
    build_router, parse_message, uses_mobile_capture_profile,
};
use screenmirror_lib::signaling::room_id::RoomIDService;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

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

#[test]
fn mobile_capture_profile_matches_mobile_os_or_small_display() {
    assert!(uses_mobile_capture_profile("iOS", 2732));
    assert!(uses_mobile_capture_profile("Android", 1920));
    assert!(uses_mobile_capture_profile("Windows", 1280));
    assert!(!uses_mobile_capture_profile("macOS", 1920));
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
        Arc::new(screenmirror_lib::webrtc::SharedCapture::new()),
        Arc::new(screenmirror_lib::webrtc::SharedCapture::new()),
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

#[tokio::test]
async fn websocket_rejected_for_untaken_room_immediately() {
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
        Arc::new(screenmirror_lib::webrtc::SharedCapture::new()),
        Arc::new(screenmirror_lib::webrtc::SharedCapture::new()),
        Arc::new(Mutex::new(())),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    let start = std::time::Instant::now();
    let ws_url = format!("ws://{}/api/ws?roomId=untaken-123456", addr);
    let (mut ws_stream, _) = connect_async(&ws_url).await.expect("connect");

    let msg = ws_stream.next().await.expect("msg").expect("ok");
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(300),
        "rejection took {:?}, expected < 300ms without artificial 500ms sleep",
        elapsed
    );
    assert_eq!(msg.to_text().unwrap(), r#"{"type":"NOT_ALLOWED"}"#);
}

#[tokio::test]
async fn websocket_downstream_push_not_blocked_by_reading_and_cleans_up() {
    let room_ids = Arc::new(Mutex::new(RoomIDService::new()));
    let room_id = "test-room-999";
    room_ids.lock().mark_taken(room_id);

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
        sinks.clone(),
        peers,
        Arc::new(screenmirror_lib::webrtc::SharedCapture::new()),
        Arc::new(screenmirror_lib::webrtc::SharedCapture::new()),
        Arc::new(Mutex::new(())),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    let ws_url = format!("ws://{}/api/ws?roomId={}", addr, room_id);
    let (mut ws_stream, _) = connect_async(&ws_url).await.expect("connect");

    // Send PING and receive PONG immediately
    ws_stream
        .send(TungsteniteMessage::Text(r#"{"type":"PING"}"#.into()))
        .await
        .expect("send ping");
    let pong_msg = ws_stream.next().await.expect("recv pong").expect("ok");
    assert_eq!(pong_msg.to_text().unwrap(), r#"{"type":"PONG"}"#);

    // Verify connection_id is in sinks
    let (connection_id, sink) = {
        let guard = sinks.lock();
        let entry = guard
            .iter()
            .find(|(k, _)| k.starts_with(room_id))
            .expect("sink exists");
        (entry.0.clone(), entry.1.clone())
    };

    // Now push a downstream message into sink without the client sending anything
    let push_content = r#"{"type":"TEST_ASYNC_PUSH","payload":"hello"}"#;
    sink.send(push_content.to_string()).expect("sink push");

    // Client must immediately receive it without waiting for an uplink message
    let pushed_msg = tokio::time::timeout(std::time::Duration::from_millis(500), ws_stream.next())
        .await
        .expect("did not timeout waiting for push")
        .expect("msg exists")
        .expect("ok");
    assert_eq!(pushed_msg.to_text().unwrap(), push_content);

    // Close the client connection
    drop(ws_stream);

    // Give a short moment for cleanup_connection to execute
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify sink was cleaned up
    assert!(
        !sinks.lock().contains_key(&connection_id),
        "sink must be cleaned up on disconnect"
    );
}
