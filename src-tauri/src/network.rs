use if_addrs::{get_if_addrs, IfAddr};
use std::net::Ipv4Addr;

pub const IGNORED_INTERFACE_PREFIXES: &[&str] = &[
    "bridge", "docker", "veth", "utun", "tun", "tap", "awdl", "llw", "gif", "stf", "ap", "virbr",
    "vboxnet",
];

pub const PREFERRED_INTERFACE_PREFIXES: &[&str] =
    &["en", "wlan", "wlp", "eth", "Wi-Fi", "WLAN", "Ethernet"];

#[inline]
pub fn is_ignored_interface(name: &str) -> bool {
    IGNORED_INTERFACE_PREFIXES
        .iter()
        .any(|p| name.starts_with(p))
}

#[inline]
pub fn is_preferred_interface(name: &str) -> bool {
    PREFERRED_INTERFACE_PREFIXES
        .iter()
        .any(|p| name.starts_with(p))
}

#[inline]
pub fn is_valid_lan_ipv4(ip: &Ipv4Addr) -> bool {
    !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified()
}

pub fn select_best_lan_ip<'a, I>(ifaces: I) -> Option<String>
where
    I: IntoIterator<Item = (&'a str, Ipv4Addr)>,
{
    let mut best_candidate: Option<(u8, String)> = None;

    for (name, ip) in ifaces {
        if is_ignored_interface(name) || !is_valid_lan_ipv4(&ip) {
            continue;
        }

        let is_pref = is_preferred_interface(name);
        let is_priv = ip.is_private();

        // Higher rank is better:
        // 4: Preferred interface + private IP
        // 3: Non-preferred interface + private IP
        // 2: Preferred interface + non-private IP
        // 1: Non-preferred interface + non-private IP
        let rank = match (is_pref, is_priv) {
            (true, true) => 4,
            (false, true) => 3,
            (true, false) => 2,
            (false, false) => 1,
        };

        match &best_candidate {
            Some((best_rank, _)) if *best_rank >= rank => {}
            _ => {
                best_candidate = Some((rank, ip.to_string()));
            }
        }
    }

    best_candidate.map(|(_, ip)| ip)
}

pub fn get_lan_ip(cli_override: Option<&str>) -> Option<String> {
    if let Some(ip) = cli_override {
        return Some(ip.to_string());
    }
    if let Ok(ifaces) = get_if_addrs() {
        let v4_list: Vec<(String, Ipv4Addr)> = ifaces
            .into_iter()
            .filter_map(|iface| match iface.addr {
                IfAddr::V4(v4) => Some((iface.name, v4.ip)),
                _ => None,
            })
            .collect();

        select_best_lan_ip(v4_list.iter().map(|(n, ip)| (n.as_str(), *ip)))
    } else {
        None
    }
}

pub fn is_wifi_connected() -> bool {
    if let Ok(ifaces) = get_if_addrs() {
        for iface in ifaces {
            if is_ignored_interface(&iface.name) {
                continue;
            }
            if let IfAddr::V4(v4) = iface.addr {
                if is_valid_lan_ipv4(&v4.ip) {
                    return true;
                }
            }
        }
    }
    false
}
