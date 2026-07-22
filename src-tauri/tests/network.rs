use screenmirror_lib::network::*;

#[test]
fn get_lan_ip_with_override() {
    let ip = get_lan_ip(Some("10.0.0.1"));
    assert_eq!(ip.as_deref(), Some("10.0.0.1"));
}

#[test]
fn get_lan_ip_without_override_returns_some_or_none() {
    let _ip: Option<String> = get_lan_ip(None);
}

#[test]
fn is_wifi_connected_doesnt_panic() {
    let _b: bool = is_wifi_connected();
}
