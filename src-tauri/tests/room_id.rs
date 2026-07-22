use screenmirror_lib::signaling::room_id::RoomIDService;

#[test]
fn new_service_has_no_taken_ids() {
    let s = RoomIDService::new();
    assert!(!s.is_taken("000000"));
}

#[test]
fn generated_id_is_6_digits() {
    let s = RoomIDService::new();
    let id = s.get_simple_available_room_id();
    assert_eq!(id.len(), 6);
    assert!(id.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn mark_and_unmark_taken() {
    let mut s = RoomIDService::new();
    s.mark_taken("123456");
    assert!(s.is_taken("123456"));
    s.unmark_taken("123456");
    assert!(!s.is_taken("123456"));
}

#[test]
fn many_generated_ids_are_unique_within_50() {
    let s = RoomIDService::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..50 {
        let id = s.get_simple_available_room_id();
        assert!(seen.insert(id.clone()), "duplicate id: {id}");
    }
}
