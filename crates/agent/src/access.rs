//! Only clients from the local network may connect. Mostly matters for IPv6, where the Frame has
//! a globally routable address and only the router's firewall stands between it and the internet.
//! Allowed: loopback, private/link-local/ULA ranges, CGNAT (Tailscale) and any address in the same
//! subnet as one of the Frame's interfaces (LAN devices with global IPv6 addresses).
//! `FRAMEMATE_ALLOW_REMOTE=1` turns the check off. The token stays the actual protection.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// At most one log line per this many seconds, so a scanner can't flood the journal.
const LOG_INTERVAL_S: u64 = 10;

pub async fn local_only(
    State(allow_remote): State<bool>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let ip = peer.ip().to_canonical(); // IPv4 clients arrive as ::ffff:a.b.c.d on the dual-stack socket
    if allow_remote || is_local(ip, &interface_networks) {
        return next.run(request).await;
    }
    static LAST_LOG: AtomicU64 = AtomicU64::new(0);
    let now = crate::hub::now_ms() / 1000;
    if now.saturating_sub(LAST_LOG.swap(now, Ordering::Relaxed)) >= LOG_INTERVAL_S {
        tracing::warn!("rejected {ip}: not in the local network (FRAMEMATE_ALLOW_REMOTE=1 allows it)");
    }
    (StatusCode::FORBIDDEN, "FrameMate only accepts connections from the local network\n").into_response()
}

/// `networks` is only consulted for public addresses (reads the interfaces).
fn is_local(ip: IpAddr, networks: &dyn Fn() -> Vec<(IpAddr, u8)>) -> bool {
    let always = match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || in_network(ip, Ipv4Addr::new(100, 64, 0, 0).into(), 10)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || in_network(ip, Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0).into(), 7) // ULA
                || in_network(ip, Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0).into(), 10) // link-local
        }
    };
    always || networks().into_iter().any(|(net, prefix)| in_network(ip, net, prefix))
}

fn in_network(ip: IpAddr, net: IpAddr, prefix: u8) -> bool {
    match (ip, net) {
        (IpAddr::V4(ip), IpAddr::V4(net)) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix.min(32))).unwrap_or(0);
            u32::from(ip) & mask == u32::from(net) & mask
        }
        (IpAddr::V6(ip), IpAddr::V6(net)) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(prefix.min(128))).unwrap_or(0);
            u128::from(ip) & mask == u128::from(net) & mask
        }
        _ => false,
    }
}

/// (address, prefix length) of every interface address, via getifaddrs(3).
fn interface_networks() -> Vec<(IpAddr, u8)> {
    let mut out = Vec::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs allocates the list, freed below.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return out;
    }
    let mut node = list;
    while !node.is_null() {
        // SAFETY: nodes stay valid until freeifaddrs.
        let ifa = unsafe { &*node };
        if let (Some(addr), Some(mask)) = (sockaddr_ip(ifa.ifa_addr), sockaddr_ip(ifa.ifa_netmask)) {
            let prefix = match mask {
                IpAddr::V4(m) => u32::from(m).count_ones(),
                IpAddr::V6(m) => u128::from(m).count_ones(),
            };
            out.push((addr, prefix as u8));
        }
        node = ifa.ifa_next;
    }
    // SAFETY: the list from getifaddrs above.
    unsafe { libc::freeifaddrs(list) };
    out
}

fn sockaddr_ip(sa: *const libc::sockaddr) -> Option<IpAddr> {
    if sa.is_null() {
        return None;
    }
    // SAFETY: sa_family says which sockaddr variant this is.
    unsafe {
        match i32::from((*sa).sa_family) {
            libc::AF_INET => {
                let sin = &*(sa as *const libc::sockaddr_in);
                Some(Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)).into())
            }
            libc::AF_INET6 => Some(Ipv6Addr::from((*(sa as *const libc::sockaddr_in6)).sin6_addr.s6_addr).into()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(ip: &str) -> bool {
        // The Frame: 192.168.178.130/24 and a global IPv6 address in 2001:db8:1:2::/64.
        let lan = || vec![("192.168.178.130".parse().unwrap(), 24), ("2001:db8:1:2::abcd".parse().unwrap(), 64)];
        is_local(ip.parse::<IpAddr>().unwrap().to_canonical(), &lan)
    }

    #[test]
    fn allows_the_local_network() {
        for ip in ["127.0.0.1", "::1", "192.168.178.22", "10.1.2.3", "172.20.0.5", "169.254.1.1", "100.101.102.103",
            "fd7a:115c:a1e3::1", "fe80::1", "::ffff:192.168.178.22", "2001:db8:1:2::77"] {
            assert!(local(ip), "{ip} should be allowed");
        }
    }

    #[test]
    fn rejects_public_addresses() {
        for ip in ["8.8.8.8", "::ffff:1.1.1.1", "2001:db8:9:9::1", "2a00:1450:4001::200e"] {
            assert!(!local(ip), "{ip} should be rejected");
        }
    }

    #[test]
    fn reads_interfaces() {
        assert!(interface_networks().iter().any(|(ip, _)| ip.is_loopback()));
    }
}
