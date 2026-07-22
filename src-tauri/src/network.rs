use if_addrs::{get_if_addrs, IfAddr};

const WIFI_PREFIXES: &[&str] = &["en", "wlan", "wlp", "Wi-Fi", "WLAN"];

pub fn get_lan_ip(cli_override: Option<&str>) -> Option<String> {
    if let Some(ip) = cli_override {
        return Some(ip.to_string());
    }
    let mut wifi_ip: Option<String> = None;
    let mut other_ip: Option<String> = None;
    if let Ok(ifaces) = get_if_addrs() {
        for iface in ifaces {
            let name = iface.name;
            if name.starts_with("bridge") || name.starts_with("docker") || name.starts_with("veth")
            {
                continue;
            }
            if let IfAddr::V4(v4) = iface.addr {
                if v4.ip.is_loopback() {
                    continue;
                }
                let ip = v4.ip.to_string();
                let is_wifi = WIFI_PREFIXES.iter().any(|p| name.starts_with(p));
                if is_wifi && wifi_ip.is_none() {
                    wifi_ip = Some(ip);
                } else if !is_wifi && other_ip.is_none() {
                    other_ip = Some(ip);
                }
            }
        }
    }
    wifi_ip.or(other_ip)
}

pub fn is_wifi_connected() -> bool {
    if let Ok(ifaces) = get_if_addrs() {
        for iface in ifaces {
            let name = iface.name;
            if name.starts_with("bridge") {
                continue;
            }
            if let IfAddr::V4(v4) = iface.addr {
                if !v4.ip.is_loopback() {
                    return true;
                }
            }
        }
    }
    false
}
