use screenmirror_lib::commands::CaptureTargetArgs;
use screenmirror_lib::webrtc::{CaptureKind, CaptureSourceInfo, CaptureTarget};

#[test]
fn capture_target_clones_its_optional_stable_source_id() {
    let target = CaptureTarget {
        kind: CaptureKind::Screen,
        id: 3,
        source_id: Some("screen:69733440".into()),
        quality: 0.75,
    };

    let cloned = target.clone();
    assert_eq!(cloned.source_id.as_deref(), Some("screen:69733440"));
}

#[test]
fn capture_source_info_serializes_the_source_identifier() {
    let source = CaptureSourceInfo {
        id: "screen:0".into(),
        source_id: "screen:69733440".into(),
        name: "Studio Display".into(),
        kind: "screen".into(),
        width: 5120,
        height: 2880,
    };

    let value = serde_json::to_value(source).expect("capture source serializes");
    assert_eq!(value["id"], "screen:0");
    assert_eq!(value["sourceId"], "screen:69733440");
    assert_eq!(value["kind"], "screen");
}

#[test]
fn capture_target_args_accepts_the_frontend_source_id_name() {
    let args: CaptureTargetArgs = serde_json::from_value(serde_json::json!({
        "kind": "screen",
        "id": 0,
        "sourceId": "screen:69733440"
    }))
    .expect("camelCase command arguments deserialize");

    assert_eq!(args.source_id.as_deref(), Some("screen:69733440"));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn enumerate_sources_is_empty_on_non_macos() {
    assert!(screenmirror_lib::webrtc::enumerate_sources()
        .expect("non-macOS stub succeeds")
        .is_empty());
}
