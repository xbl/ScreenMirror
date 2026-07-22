use screenmirror_lib::signaling::devices::{ConnectedDevicesService, Device};

fn dev(id: &str) -> Device {
    Device {
        id: id.to_string(),
        name: "Test".into(),
        ip: "192.168.1.10".into(),
        os: "macOS".into(),
        browser: "Safari".into(),
        room_id: "123456".into(),
        sharing_session_id: "sess-1".into(),
    }
}

#[test]
fn empty_service_slot_available() {
    let s = ConnectedDevicesService::new();
    assert!(s.is_slot_available());
    assert!(s.get_devices().is_empty());
}

#[test]
fn add_device_occupies_slot() {
    let mut s = ConnectedDevicesService::new();
    s.add_device(dev("d1")).unwrap();
    assert!(!s.is_slot_available());
    assert_eq!(s.get_devices().len(), 1);
}

#[test]
fn second_device_rejected() {
    let mut s = ConnectedDevicesService::new();
    s.add_device(dev("d1")).unwrap();
    let result = s.add_device(dev("d2"));
    assert!(result.is_err());
}

#[test]
fn release_device_frees_slot() {
    let mut s = ConnectedDevicesService::new();
    s.add_device(dev("d1")).unwrap();
    assert!(s.release_device("d1"));
    assert!(s.is_slot_available());
}

#[test]
fn pending_device_can_be_set_and_reset() {
    let mut s = ConnectedDevicesService::new();
    assert!(s.get_pending().is_none());
    s.set_pending(dev("p1"));
    assert!(s.get_pending().is_some());
    s.reset_pending();
    assert!(s.get_pending().is_none());
}
