use screenmirror_lib::network::*;
use std::net::Ipv4Addr;

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

#[test]
fn test_is_ignored_interface() {
    let ignored = [
        "utun0", "utun4", "tun0", "tap0", "awdl0", "llw0", "gif0", "stf0", "ap0", "bridge0",
        "docker0", "veth01ab", "virbr0", "vboxnet0",
    ];
    for name in &ignored {
        assert!(
            is_ignored_interface(name),
            "interface {name} should be ignored"
        );
    }

    let allowed = ["en0", "en1", "eth0", "wlan0", "wlp2s0", "Wi-Fi"];
    for name in &allowed {
        assert!(
            !is_ignored_interface(name),
            "interface {name} should NOT be ignored"
        );
    }
}

#[test]
fn test_is_valid_lan_ipv4() {
    assert!(!is_valid_lan_ipv4(&Ipv4Addr::new(127, 0, 0, 1))); // loopback
    assert!(!is_valid_lan_ipv4(&Ipv4Addr::new(169, 254, 10, 20))); // link-local
    assert!(!is_valid_lan_ipv4(&Ipv4Addr::new(0, 0, 0, 0))); // unspecified

    assert!(is_valid_lan_ipv4(&Ipv4Addr::new(192, 168, 1, 100)));
    assert!(is_valid_lan_ipv4(&Ipv4Addr::new(10, 0, 0, 15)));
    assert!(is_valid_lan_ipv4(&Ipv4Addr::new(172, 16, 0, 5)));
}

#[test]
fn test_select_best_lan_ip_ranking() {
    let ifaces = vec![
        ("utun4", Ipv4Addr::new(10, 8, 0, 2)),          // VPN - ignored
        ("en0", Ipv4Addr::new(169, 254, 5, 5)),         // link-local - ignored
        ("awdl0", Ipv4Addr::new(192, 168, 99, 1)),      // macOS peer-to-peer - ignored
        ("other0", Ipv4Addr::new(10, 0, 0, 2)),         // non-preferred private (rank 3)
        ("en1", Ipv4Addr::new(192, 168, 1, 50)),        // preferred private (rank 4)
    ];

    let chosen = select_best_lan_ip(ifaces.into_iter());
    assert_eq!(chosen, Some("192.168.1.50".to_string()));
}

#[test]
fn test_select_best_lan_ip_fallback() {
    let ifaces = vec![
        ("utun0", Ipv4Addr::new(10, 200, 1, 1)),
        ("custom0", Ipv4Addr::new(172, 20, 0, 10)),
    ];

    let chosen = select_best_lan_ip(ifaces.into_iter());
    assert_eq!(chosen, Some("172.20.0.10".to_string()));
}
