//! Enumeration of eligible local IPv4 interfaces for RTPS discovery locators.
//!
//! #27: on a multi-homed host, announcing a single probed source address can
//! advertise a locator that is unreachable for a given peer (the OS default-
//! route probe and the peer's segment diverge). Discovery instead announces
//! *every* usable unicast address, so a peer on any shared segment always
//! finds a reachable one. This module produces the deterministic, filtered
//! set the DCPS announce path fans out over.
//!
//! Routing/usability filtering beyond "is this a routable unicast IPv4 on an
//! UP, multicast-capable, non-loopback interface" stays in DCPS — this module
//! only enumerates and orders.

use std::net::{IpAddr, Ipv4Addr};

/// An eligible local IPv4 interface address usable as an announced unicast
/// discovery locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibleInterface {
    /// OS interface name (diagnostic + deterministic tiebreak).
    pub name: String,
    /// OS interface index.
    pub index: u32,
    /// The unicast IPv4 address configured on the interface.
    pub ipv4: Ipv4Addr,
}

/// Returns the eligible IPv4 interfaces in deterministic order.
///
/// Eligibility (DDSI-RTPS 2.5 §9.6.1 discovery locators): the interface is
/// operational (UP — `if-addrs` only returns configured, operational
/// interfaces) and not loopback; the address is a routable unicast IPv4 — not
/// loopback, not unspecified, not link-local (169.254/16), not broadcast, not
/// multicast. Exact duplicate `(name, index, ipv4)` tuples are removed. Order
/// is by IPv4 address ascending, then name, then index — stable and
/// independent of OS enumeration order.
///
/// Multicast-capability is assumed here: this set drives **unicast** locator
/// announcement (a unicast SEDP destination does not require the interface to
/// be multicast-capable). The per-interface multicast join/TX that would need
/// the actual `IFF_MULTICAST` flag is deferred discovery-hardening.
#[must_use]
pub fn eligible_ipv4_interfaces() -> Vec<EligibleInterface> {
    let ifaces = match if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    collect_eligible(ifaces.into_iter().filter_map(|iface| {
        let ipv4 = match iface.ip() {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(_) => return None,
        };
        Some(RawInterface {
            name: iface.name.clone(),
            index: iface.index.unwrap_or(0),
            // get_if_addrs returns only operational (UP) interfaces; the
            // multicast flag is not exposed, so assume it (see fn doc).
            up: true,
            multicast: true,
            loopback: iface.is_loopback(),
            v4_addrs: vec![ipv4],
        })
    }))
}

/// OS-agnostic view of one interface, so the filter/order logic is testable
/// without a live network stack.
#[derive(Debug, Clone)]
struct RawInterface {
    name: String,
    index: u32,
    up: bool,
    multicast: bool,
    loopback: bool,
    v4_addrs: Vec<Ipv4Addr>,
}

/// Filter + flatten + deterministic order + exact-duplicate removal. Split
/// from [`eligible_ipv4_interfaces`] so tests drive it with synthetic input.
fn collect_eligible(raw: impl Iterator<Item = RawInterface>) -> Vec<EligibleInterface> {
    let mut out: Vec<EligibleInterface> = raw
        .filter(|r| r.up && r.multicast && !r.loopback)
        .flat_map(|r| {
            let RawInterface {
                name,
                index,
                v4_addrs,
                ..
            } = r;
            v4_addrs
                .into_iter()
                .filter(is_eligible_v4)
                .map(move |ipv4| EligibleInterface {
                    name: name.clone(),
                    index,
                    ipv4,
                })
        })
        .collect();
    out.sort_by(|a, b| {
        a.ipv4
            .octets()
            .cmp(&b.ipv4.octets())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.index.cmp(&b.index))
    });
    out.dedup();
    out
}

/// A routable unicast IPv4 usable as an announced locator.
fn is_eligible_v4(ip: &Ipv4Addr) -> bool {
    !ip.is_loopback()
        && !ip.is_unspecified()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_multicast()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn raw(name: &str, index: u32, up: bool, mc: bool, lo: bool, addrs: &[&str]) -> RawInterface {
        RawInterface {
            name: name.to_string(),
            index,
            up,
            multicast: mc,
            loopback: lo,
            v4_addrs: addrs.iter().map(|s| s.parse().unwrap()).collect(),
        }
    }

    #[test]
    fn keeps_only_up_multicast_non_loopback() {
        let ifs = vec![
            raw("eth0", 2, true, true, false, &["192.168.1.10"]),
            raw("down0", 3, false, true, false, &["10.0.0.5"]), // down
            raw("nomc0", 4, true, false, false, &["10.0.0.6"]), // not multicast
            raw("lo", 1, true, true, true, &["127.0.0.1"]),     // loopback flag
        ];
        let got = collect_eligible(ifs.into_iter());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ipv4, Ipv4Addr::new(192, 168, 1, 10));
    }

    #[test]
    fn filters_loopback_linklocal_unspecified_multicast_broadcast_addrs() {
        let ifs = vec![raw(
            "eth0",
            2,
            true,
            true,
            false,
            &[
                "127.0.0.1",       // loopback addr
                "169.254.3.4",     // link-local
                "0.0.0.0",         // unspecified
                "239.255.0.1",     // multicast
                "255.255.255.255", // broadcast
                "10.1.2.3",        // the one keeper
            ],
        )];
        let got = collect_eligible(ifs.into_iter());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ipv4, Ipv4Addr::new(10, 1, 2, 3));
    }

    #[test]
    fn deterministic_order_by_ip_then_name_then_index() {
        // Input in scrambled order; output must be by ipv4 asc, then name, index.
        let ifs = vec![
            raw("zeth", 9, true, true, false, &["192.168.1.20"]),
            raw("aeth", 2, true, true, false, &["10.0.0.5"]),
            raw("aeth", 2, true, true, false, &["192.168.1.20"]),
        ];
        let got = collect_eligible(ifs.into_iter());
        let ips: Vec<_> = got.iter().map(|e| (e.ipv4, e.name.as_str())).collect();
        assert_eq!(
            ips,
            vec![
                (Ipv4Addr::new(10, 0, 0, 5), "aeth"),
                (Ipv4Addr::new(192, 168, 1, 20), "aeth"),
                (Ipv4Addr::new(192, 168, 1, 20), "zeth"),
            ]
        );
    }

    #[test]
    fn removes_exact_duplicate_tuples() {
        let ifs = vec![
            raw("eth0", 2, true, true, false, &["192.168.1.10"]),
            raw("eth0", 2, true, true, false, &["192.168.1.10"]), // exact dup
        ];
        let got = collect_eligible(ifs.into_iter());
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn empty_when_only_loopback() {
        let ifs = vec![raw("lo", 1, true, true, true, &["127.0.0.1"])];
        assert!(collect_eligible(ifs.into_iter()).is_empty());
    }
}
