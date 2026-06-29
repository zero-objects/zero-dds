// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DcpsRuntime — event loop + UDP sockets per DomainParticipant.
//!
//! # Structure
//!
//! - Binds 3 UDP sockets per participant:
//!   * SPDP multicast receiver (domain-based port).
//!   * SPDP unicast fallback (ephemeral, for bidirectional SPDP).
//!   * User unicast (ephemeral, where matched peers send to).
//! - Spawns a single event-loop thread that periodically:
//!   * sends the SPDP beacon (every 5 s by default),
//!   * polls all sockets non-blocking,
//!   * moves SPDP datagrams into the DiscoveredParticipantsCache,
//!   * dispatches SEDP datagrams (pub/sub announces),
//!   * delivers user data to the correct DataReader slots,
//!   * runs the WLP/liveliness tick,
//!   * serves the TypeLookup service endpoints (XTypes 1.3 §7.6.3.3.4).
//! - Thread lifecycle via `Arc<AtomicBool> stop_flag` + `JoinHandle` in
//!   `Drop`.
//!
//! With the `security` feature active, all outbound/inbound bytes pass
//! through the `SharedSecurityGate` (DDS-Security 1.2). Multi-interface
//! binding (RuntimeConfig::interface_bindings) enables per-subnet routing
//! for production topologies.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::time::Duration;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use zerodds_discovery::security::SecurityBuiltinStack;
use zerodds_discovery::sedp::SedpStack;
use zerodds_discovery::spdp::{
    DiscoveredParticipant, DiscoveredParticipantsCache, SpdpBeacon, SpdpReader,
};
use zerodds_discovery::type_lookup::{
    TypeLookupClient, TypeLookupEndpoints, TypeLookupReply, TypeLookupServer,
};
use zerodds_qos::Duration as QosDuration;
use zerodds_rtps::EntityId;
use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::fragment_assembler::AssemblerCaps;
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::DEFAULT_MTU;
use zerodds_rtps::participant_data::{ParticipantBuiltinTopicData, endpoint_flag};
use zerodds_rtps::reliable_reader::{ReliableReader, ReliableReaderConfig};
use zerodds_rtps::reliable_writer::{
    DEFAULT_FRAGMENT_SIZE, DEFAULT_HEARTBEAT_PERIOD, LOOPBACK_FRAGMENT_SIZE, LOOPBACK_MTU,
    ReliableWriter, ReliableWriterConfig,
};
use zerodds_rtps::wire_types::{
    Guid, GuidPrefix, Locator, LocatorKind, ProtocolVersion, SPDP_DEFAULT_MULTICAST_ADDRESS,
    VendorId, spdp_multicast_port,
};
use zerodds_transport::Transport;
use zerodds_transport_udp::UdpTransport;

#[cfg(feature = "security")]
use zerodds_security_runtime::{EndpointProtection, IpRange, NetInterface, ProtectionLevel};

use crate::error::{DdsError, Result};

/// Default tick period of the event loop.
///
/// This is the worst-case quantization for sub-tick-driven tasks
/// (SEDP heartbeats, reliable-writer resends, ACKNACK emit). Short enough
/// for sub-ms round-trip latency (5 ms = 100 Hz tick rate), long enough
/// to keep idle CPU cost small.
///
/// Phase-3 migration: this tick loop is replaced by a deadline heap +
/// condvar worker (`scheduler.rs`) — then this value is only the
/// idle-floor sleep (no quantization tax for events).
pub const DEFAULT_TICK_PERIOD: Duration = Duration::from_millis(5);

/// Default SPDP announce period (Spec §8.5.3.2 recommends 5 s).
pub const DEFAULT_SPDP_PERIOD: Duration = Duration::from_secs(5);

/// Default number of SPDP announces sent at the fast initial-burst cadence
/// (C3 WiFi-robust discovery) before falling back to [`DEFAULT_SPDP_PERIOD`].
/// Analogous to Fast DDS `initial_announcements`.
pub const DEFAULT_INITIAL_ANNOUNCE_COUNT: u32 = 10;

/// Default period between initial-announcement-burst SPDP sends.
pub const DEFAULT_INITIAL_ANNOUNCE_PERIOD: Duration = Duration::from_millis(200);

/// Deadline/lease compat check: the offered period must be <= requested.
/// `0` is the sentinel for INFINITE — there any combination is compatible
/// (offered INFINITE implies "I promise nothing faster than infinity",
/// but a reader with INFINITE also requests nothing).
fn deadline_compat(offered_nanos: u64, requested_nanos: u64) -> bool {
    if offered_nanos == 0 || requested_nanos == 0 {
        // INFINITE on one side → compatible.
        return true;
    }
    offered_nanos <= requested_nanos
}

/// Partition matching: both sides have at least one common partition OR
/// both are empty (default partition "").
/// Enforces a per-instance KeepLast depth over a TransientLocal retained-sample
/// buffer (DDS 1.4 §2.2.3.18). Keeps at most `depth` *Alive* entries per
/// instance key (oldest evicted first), preserving order; terminal lifecycle
/// markers (dispose/unregister) are never counted against the depth and are
/// always retained so a late joiner observes the final NOT_ALIVE state.
fn enforce_retained_depth(
    retained: &mut alloc::collections::VecDeque<RetainedSample>,
    depth: usize,
) {
    use alloc::collections::BTreeMap;
    // Count alive samples per key.
    let mut alive_per_key: BTreeMap<[u8; 16], usize> = BTreeMap::new();
    for s in retained.iter() {
        if s.lifecycle.is_none() {
            *alive_per_key.entry(s.key_hash).or_insert(0) += 1;
        }
    }
    // Determine how many to drop per key.
    let mut to_drop: BTreeMap<[u8; 16], usize> = BTreeMap::new();
    for (k, count) in &alive_per_key {
        if *count > depth {
            to_drop.insert(*k, count - depth);
        }
    }
    if to_drop.is_empty() {
        return;
    }
    retained.retain(|s| {
        if s.lifecycle.is_some() {
            return true;
        }
        if let Some(rem) = to_drop.get_mut(&s.key_hash) {
            if *rem > 0 {
                *rem -= 1;
                return false; // evict this (oldest) alive sample for the key
            }
        }
        true
    });
}

fn partitions_overlap(offered: &[String], requested: &[String]) -> bool {
    if offered.is_empty() && requested.is_empty() {
        return true;
    }
    // An empty list is treated as ["" (default)].
    let off_default = offered.is_empty();
    let req_default = requested.is_empty();
    if off_default && requested.iter().any(|s| s.is_empty()) {
        return true;
    }
    if req_default && offered.iter().any(|s| s.is_empty()) {
        return true;
    }
    // Both non-default: intersect.
    offered.iter().any(|o| requested.iter().any(|r| r == o))
}

/// Materializes the locator address that we announce in the SPDP beacon
/// from an UdpTransport bound to UNSPECIFIED.
///
/// Binding to `0.0.0.0` yields `local_addr() == 0.0.0.0:port`, which is
/// not routable for peers. Via a UDP connect probe to a non-routable
/// address we resolve the outbound interface address (no traffic —
/// `connect()` on a UDP socket only sets the routing information). Falls
/// back to `multicast_interface` (RuntimeConfig) if the probe fails, or
/// to the unchanged locator as a last resort.
#[cfg(feature = "std")]
fn announce_locator(uc: &(dyn Transport + Send + Sync), hint: Ipv4Addr) -> Locator {
    let raw = uc.local_locator();
    // Keep the port from the bound socket.
    let port = raw.port;
    // V6 resolution: with a `::` bind, announce `::1` (loopback) as a
    // sensible default reachability. Cross-host v6 is its own sprint
    // (needs a v6 interface probe analogous to the v4 path below).
    if raw.kind == LocatorKind::UdpV6 || raw.kind == LocatorKind::Tcpv6 {
        let all_zero = raw.address.iter().all(|b| *b == 0);
        if all_zero {
            let mut loopback_addr = [0u8; 16];
            loopback_addr[15] = 1;
            return match raw.kind {
                LocatorKind::Tcpv6 => Locator::tcp_v6(loopback_addr, port),
                _ => Locator::udp_v6(loopback_addr, port),
            };
        }
        return raw;
    }
    // V4 resolution: only meaningful for UDPv4/TCPv4 locators with an
    // UNSPECIFIED bind. For SHM, return raw — the locator kind has its
    // own pairing resolution (its own sprint).
    if raw.kind != LocatorKind::UdpV4 && raw.kind != LocatorKind::Tcpv4 {
        return raw;
    }
    // Extract the address — only the last 4 bytes are the IPv4.
    let ip = Ipv4Addr::new(
        raw.address[12],
        raw.address[13],
        raw.address[14],
        raw.address[15],
    );
    if !ip.is_unspecified() {
        return raw;
    }
    // Helper: construct a locator with the original kind (UdpV4 or
    // Tcpv4) and the now-resolved v4 address.
    let to_locator = |octets: [u8; 4]| -> Locator {
        match raw.kind {
            LocatorKind::Tcpv4 => Locator::tcp_v4(octets, port),
            _ => Locator::udp_v4(octets, port),
        }
    };
    // Interface pinning: an explicitly set interface
    // (`ZERODDS_INTERFACE` / `RuntimeConfig.multicast_interface`) takes
    // **precedence over the route probe**. On multi-homed hosts (VPN/
    // Docker/macOS bridge100) the probe might otherwise pick the wrong
    // source IP and announce an unreachable address → discovery fails.
    if !hint.is_unspecified() {
        return to_locator(hint.octets());
    }
    // Probe: temporary socket, "connect" to 192.0.2.1 (RFC 5737
    // TEST-NET-1, guaranteed non-routable). connect only sets the routing
    // table — no packet goes out.
    if let Ok(probe) =
        std::net::UdpSocket::bind(std::net::SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
    {
        if probe
            .connect(std::net::SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 7))
            .is_ok()
        {
            if let Ok(std::net::SocketAddr::V4(local)) = probe.local_addr() {
                let resolved = local.ip();
                if !resolved.is_unspecified() {
                    return to_locator(resolved.octets());
                }
            }
        }
    }
    // Fallback: loopback (the pin hint is already handled above). Not
    // ideal, but better than 0.0.0.0 as a locator (at least routable on
    // the same host).
    to_locator([127, 0, 0, 1])
}

/// Converts a `core::time::Duration` (std) to a `zerodds_qos::Duration`
/// (spec 2^-32 fraction encoding). Saturates on overflow — `i32::MAX`
/// seconds suffices for over 60 years of lease.
fn qos_duration_from_std(d: Duration) -> QosDuration {
    let secs = i32::try_from(d.as_secs()).unwrap_or(i32::MAX);
    let nanos = d.subsec_nanos();
    // The spec fraction is 2^-32 s; from nanos back via (nanos << 32) / 1e9.
    let fraction = ((u64::from(nanos)) << 32) / 1_000_000_000u64;
    QosDuration {
        seconds: secs,
        fraction: fraction as u32,
    }
}

/// Converts a `zerodds_qos::Duration` to nanoseconds (0 = INFINITE,
/// "no monitoring"). `seconds` is i32 — we clamp to non-negative.
fn qos_duration_to_nanos(d: zerodds_qos::Duration) -> u64 {
    if d.is_infinite() {
        return 0;
    }
    let secs = d.seconds.max(0) as u64;
    // fraction is 2^-32 s, i.e. nanos = fraction * 1e9 / 2^32.
    let frac_nanos = ((d.fraction as u64) * 1_000_000_000u64) >> 32;
    secs.saturating_mul(1_000_000_000u64)
        .saturating_add(frac_nanos)
}

/// Human-readable name of a QoS policy id (Spec OMG DDS 1.4 §2.2.3,
/// PSM ids from [`crate::psm_constants::qos_policy_id`]). Used for the
/// C2 "loud instead of silent" log on an incompatible QoS match, so that
/// it states in plain text *which* policy prevented the match.
#[must_use]
fn qos_policy_id_name(pid: u32) -> &'static str {
    use crate::psm_constants::qos_policy_id as qid;
    match pid {
        qid::DURABILITY => "DURABILITY",
        qid::PRESENTATION => "PRESENTATION",
        qid::DEADLINE => "DEADLINE",
        qid::LATENCY_BUDGET => "LATENCY_BUDGET",
        qid::OWNERSHIP => "OWNERSHIP",
        qid::OWNERSHIP_STRENGTH => "OWNERSHIP_STRENGTH",
        qid::LIVELINESS => "LIVELINESS",
        qid::PARTITION => "PARTITION",
        qid::RELIABILITY => "RELIABILITY",
        qid::DESTINATION_ORDER => "DESTINATION_ORDER",
        qid::DURABILITY_SERVICE => "DURABILITY_SERVICE",
        qid::TYPE_CONSISTENCY_ENFORCEMENT => "TYPE_CONSISTENCY_ENFORCEMENT",
        qid::DATA_REPRESENTATION => "DATA_REPRESENTATION",
        _ => "OTHER",
    }
}

/// RTPS serialized-payload header for user samples: `CDR_LE`
/// (PLAIN_CDR / XCDR1, little-endian) + options=0. Spec OMG RTPS 2.5
/// §9.4.2.13.
///
/// Prepended to every user payload before it goes into the DATA
/// submessage — without this header, vendor readers (Cyclone / Fast-DDS)
/// refuse to deliver the sample.
///
/// **Why `0x01` (XCDR1) and not `0x07` (XCDR2):** the C++ PSM codegen
/// (`dds/topic/xcdr2.hpp`) aligns 8-byte primitives to `sizeof` — that
/// is the PLAIN_CDR/XCDR1 rule, NOT XCDR2 (which requires
/// `min(sizeof,4)`). ZeroDDS therefore effectively produces an XCDR1
/// layout; the encapsulation header must declare that honestly,
/// otherwise the peer reads the body with the wrong alignment (e.g.
/// OpenDDS' `dds_demarshal` fails). Full XCDR2 support is a separate
/// codegen feature.
pub const USER_PAYLOAD_ENCAP: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

/// Encapsulation header for the user payload, based on the negotiated
/// DataRepresentation (`offer_first`: the **first** element of the
/// writer's offer list = the wire format actually emitted by the writer)
/// and the type extensibility. The header MUST honestly declare the body
/// encoding produced by the codegen, otherwise the peer (e.g.
/// FastDDS/OpenDDS XCDR2-only reader) reads the body with the wrong
/// alignment or wrongly expects a DHEADER.
///
/// DDSI-RTPS 2.5 §10.5 / XTypes 1.3 Tab.59 (little-endian variant):
///   XCDR1 final/appendable -> CDR_LE        `0x0001`
///   XCDR1 mutable          -> PL_CDR_LE      `0x0003`
///   XCDR2 final            -> PLAIN_CDR2_LE  `0x0007`
///   XCDR2 appendable       -> D_CDR2_LE      `0x0009`
///   XCDR2 mutable          -> PL_CDR2_LE     `0x000b`
#[must_use]
fn user_payload_encap(
    offer_first: i16,
    ext: zerodds_types::qos::ExtensibilityForRepr,
    big_endian: bool,
) -> [u8; 4] {
    use zerodds_rtps::publication_data::data_representation as dr;
    use zerodds_types::qos::ExtensibilityForRepr::{Appendable, Final, Mutable};
    let id: u8 = match (offer_first, ext) {
        (dr::XCDR2, Final) => 0x07,
        (dr::XCDR2, Appendable) => 0x09,
        (dr::XCDR2, Mutable) => 0x0b,
        // XCDR1: appendable is treated like final (Tab.59: XCDR1 has no
        // dedicated APPENDABLE encoding).
        (dr::XCDR, Mutable) => 0x03,
        // (dr::XCDR, Final|Appendable) as well as XML/unknown -> CDR_LE.
        _ => 0x01,
    };
    // RTPS 2.5 §10.5: the `_LE` representation ids are odd, the matching `_BE`
    // ones are the even predecessor (CDR_BE 0x00, CDR2_BE 0x06, D_CDR2_BE 0x08,
    // PL_CDR2_BE 0x0a). Clearing the low bit converts LE -> BE.
    let id = if big_endian { id & 0xFE } else { id };
    [0x00, id, 0x00, 0x00]
}

/// Stack PoolBuffer cap for the small-sample path in
/// [`DcpsRuntime::write_user_sample`]. A 1.5 KiB payload + 4 B encap
/// header fit through the framing without touching the heap.
const SMALL_FRAME_CAP: usize = 1536;

/// Small-sample hot-path helper: frames `USER_PAYLOAD_ENCAP` + payload
/// into a stack `PoolBuffer<SMALL_FRAME_CAP>` and hands the slice to the
/// writer. No Vec/Box/Rc/Arc allocation in this function — verified by
/// the `dds_no_realloc_in_hot_path` lint.
///
/// zerodds-lint: hot-path-realloc-free
fn write_user_sample_pooled(
    writer: &mut ReliableWriter,
    payload: &[u8],
    now: Duration,
    encap: &[u8; 4],
) -> Result<Vec<zerodds_rtps::message_builder::OutboundDatagram>> {
    let mut frame = zerodds_foundation::PoolBuffer::<SMALL_FRAME_CAP>::new();
    frame
        .extend_from_slice(encap)
        .map_err(|_| DdsError::WireError {
            message: String::from("user encap framing"),
        })?;
    frame
        .extend_from_slice(payload)
        .map_err(|_| DdsError::WireError {
            message: String::from("user payload framing"),
        })?;
    // Hot path: only DATA, NO HEARTBEAT. Cyclone DDS rate-limits the
    // HB piggyback (≥100 µs spacing, or a packet boundary) — so it does
    // NOT send an HB per write. At 14k writes/sec this would fire 14k
    // unnecessary submessages and (with an unaligned payload) 14k extra
    // sendto syscalls. Periodic HBs are handled by the tick loop (every
    // `heartbeat_period` ms, default 100 ms); we no longer attach `_now`
    // to `last_heartbeat`, because we emit nothing.
    let _ = now;
    // RTPS-F1: attach the wall-clock source timestamp so the peer can populate
    // SampleInfo.source_timestamp + honour DESTINATION_ORDER = BY_SOURCE_TIMESTAMP
    // (DDSI-RTPS §8.7.3). The writer prepends an INFO_TS before the DATA.
    let source_ts = crate::time::time_to_he_timestamp(crate::time::get_current_time());
    writer
        .write_stamped(frame.as_slice(), Some(source_ts))
        .map_err(|_| DdsError::WireError {
            message: String::from("user writer encode"),
        })
}

/// Choice of transport for DCPS user traffic. Discovery (SPDP/SEDP)
/// remains UDPv4 multicast independently of this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserTransportKind {
    /// UDP IPv4 (default).
    UdpV4,
    /// UDP IPv6.
    UdpV6,
    /// TCP IPv4 (DDS-TCP-PSM `LOCATOR_KIND_TCPV4`).
    TcpV4,
    /// TCP IPv6 (DDS-TCP-PSM `LOCATOR_KIND_TCPV6`).
    TcpV6,
    /// POSIX shared memory (same-host). Only with the `same-host-shm`
    /// feature.
    #[cfg(feature = "same-host-shm")]
    Shm,
    /// Unix domain socket (same-host, container-friendly). Only with the
    /// `same-host-uds` feature.
    #[cfg(feature = "same-host-uds")]
    Uds,
    /// TSN L2 transport (AF_PACKET, RTPS direct on Ethernet, EtherType
    /// 0x88B5). Only with the `tsn-live` feature on Linux. Interface/VLAN/
    /// PCP via the env vars `ZERODDS_TSN_IFACE`/`_VLAN`/`_PCP`.
    #[cfg(all(feature = "tsn-live", target_os = "linux"))]
    Tsn,
}

/// Maps the `ZERODDS_USER_TRANSPORT` env var to a [`UserTransportKind`].
/// `None` if unset or unknown — the caller then falls back to UDPv4.
fn parse_user_transport_env() -> Option<UserTransportKind> {
    match std::env::var("ZERODDS_USER_TRANSPORT").ok()?.as_str() {
        "UDPv4" => Some(UserTransportKind::UdpV4),
        "UDPv6" => Some(UserTransportKind::UdpV6),
        "TCPv4" => Some(UserTransportKind::TcpV4),
        "TCPv6" => Some(UserTransportKind::TcpV6),
        #[cfg(feature = "same-host-shm")]
        "SHM" => Some(UserTransportKind::Shm),
        #[cfg(feature = "same-host-uds")]
        "UDS" => Some(UserTransportKind::Uds),
        #[cfg(all(feature = "tsn-live", target_os = "linux"))]
        "TSN" => Some(UserTransportKind::Tsn),
        _ => None,
    }
}

/// Result of [`select_user_transport`]: the user-traffic transport plus
/// an optional `TcpTransport` accept handle (only for TCP).
type UserTransportSelection = (
    Arc<dyn Transport + Send + Sync>,
    Option<Arc<zerodds_transport_tcp::TcpTransport>>,
);

/// Binds the user-traffic transport for the selected
/// [`UserTransportKind`]. Discovery (SPDP/SEDP) runs separately over
/// UDPv4 multicast; this transport carries only the DCPS user traffic.
///
/// Additionally returns an optional `TcpTransport` accept handle: TCP has
/// no implicit accept thread in the constructor, so the caller starts an
/// `accept_one` worker for it.
#[cfg_attr(
    not(any(feature = "same-host-shm", feature = "same-host-uds")),
    allow(unused_variables)
)]
fn select_user_transport(
    kind: UserTransportKind,
    guid_prefix: GuidPrefix,
    domain_id: i32,
    pinned: Ipv4Addr,
) -> Result<UserTransportSelection> {
    match kind {
        UserTransportKind::UdpV4 => {
            // Interface pinning: bind to the pinned IPv4 (egress + receive
            // on exactly this interface), otherwise `0.0.0.0` (auto).
            let udp = UdpTransport::bind_v4(pinned, 0)
                .map_err(|_| DdsError::TransportError {
                    label: "user unicast bind (UDPv4)",
                })?
                .with_timeout(Some(Duration::from_secs(1)))
                .map_err(|_| DdsError::TransportError {
                    label: "user unicast set_timeout (UDPv4)",
                })?;
            Ok((Arc::new(udp), None))
        }
        UserTransportKind::UdpV6 => {
            let udp = UdpTransport::bind_v6(std::net::Ipv6Addr::UNSPECIFIED, 0)
                .map_err(|_| DdsError::TransportError {
                    label: "user unicast bind (UDPv6)",
                })?
                .with_timeout(Some(Duration::from_secs(1)))
                .map_err(|_| DdsError::TransportError {
                    label: "user unicast set_timeout (UDPv6)",
                })?;
            Ok((Arc::new(udp), None))
        }
        UserTransportKind::TcpV4 => {
            // Interface pinning analogous to UDPv4.
            let tcp = zerodds_transport_tcp::TcpTransport::bind_v4(pinned, 0).map_err(|_| {
                DdsError::TransportError {
                    label: "user unicast bind (TCPv4)",
                }
            })?;
            let arc = Arc::new(tcp);
            let dynamic: Arc<dyn Transport + Send + Sync> = arc.clone();
            Ok((dynamic, Some(arc)))
        }
        UserTransportKind::TcpV6 => {
            let tcp =
                zerodds_transport_tcp::TcpTransport::bind_v6(std::net::Ipv6Addr::UNSPECIFIED, 0)
                    .map_err(|_| DdsError::TransportError {
                        label: "user unicast bind (TCPv6)",
                    })?;
            let arc = Arc::new(tcp);
            let dynamic: Arc<dyn Transport + Send + Sync> = arc.clone();
            Ok((dynamic, Some(arc)))
        }
        #[cfg(feature = "same-host-shm")]
        UserTransportKind::Shm => {
            // local_id = guid_prefix (12 bytes) + 4-byte domain id so that
            // separate domains get separate segments (no cross-domain
            // collisions).
            let mut local_id = [0u8; 16];
            local_id[..12].copy_from_slice(&guid_prefix.to_bytes());
            local_id[12..].copy_from_slice(&(domain_id as u32).to_be_bytes());
            let shm = crate::shm_user::ShmUserTransport::new(
                local_id,
                zerodds_transport_shm::posix::ShmConfig::default(),
            );
            Ok((Arc::new(shm), None))
        }
        #[cfg(feature = "same-host-uds")]
        UserTransportKind::Uds => {
            // local_id = guid_prefix (12 bytes) + 4-byte domain id — the
            // peer resolves this id from the announced UDS locator into the
            // same socket path.
            let mut local_id = [0u8; 16];
            local_id[..12].copy_from_slice(&guid_prefix.to_bytes());
            local_id[12..].copy_from_slice(&(domain_id as u32).to_be_bytes());
            // recv_timeout analogous to the UDP path: the recv loop must
            // periodically check the stop flag (otherwise a thread hang on
            // shutdown on a blocking recv).
            let uds_cfg = zerodds_transport_uds::UdsConfig {
                recv_timeout: Some(Duration::from_secs(1)),
                ..zerodds_transport_uds::UdsConfig::default()
            };
            let uds =
                zerodds_transport_uds::UdsTransport::bind(local_id, uds_cfg).map_err(|_| {
                    DdsError::TransportError {
                        label: "user unicast bind (UDS)",
                    }
                })?;
            Ok((Arc::new(uds), None))
        }
        #[cfg(all(feature = "tsn-live", target_os = "linux"))]
        UserTransportKind::Tsn => {
            // Interface/VLAN/PCP via env (TSN needs a concrete interface;
            // not bindable to 0.0.0.0 like UDP/TCP). recv_timeout 1s
            // analogous to UDP for the stop-flag check.
            let iface =
                std::env::var("ZERODDS_TSN_IFACE").map_err(|_| DdsError::TransportError {
                    label: "ZERODDS_TSN_IFACE not set (TSN transport)",
                })?;
            let vlan = std::env::var("ZERODDS_TSN_VLAN")
                .ok()
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0);
            let pcp = std::env::var("ZERODDS_TSN_PCP")
                .ok()
                .and_then(|s| s.parse::<u8>().ok())
                .unwrap_or(0);
            let tsn = zerodds_transport_tsn::socket::TsnTransport::bind(
                &iface,
                vlan,
                pcp,
                Some(Duration::from_secs(1)),
            )
            .map_err(|_| DdsError::TransportError {
                label: "user unicast bind (TSN)",
            })?;
            Ok((Arc::new(tsn), None))
        }
    }
}

/// Configuration for the runtime. Exposed via DomainParticipant factory
/// methods.
#[derive(Clone)]
pub struct RuntimeConfig {
    /// Tick period of the event loop. Default 50 ms.
    pub tick_period: Duration,
    /// SPDP announce period. Default 5 s.
    pub spdp_period: Duration,
    /// C3 WiFi-robust discovery — number of initial SPDP announces sent at the
    /// fast [`Self::initial_announce_period`] cadence (instead of `spdp_period`)
    /// while no peer is yet discovered. Default
    /// [`DEFAULT_INITIAL_ANNOUNCE_COUNT`]. `0` disables the burst (legacy
    /// single-announce-then-`spdp_period` behaviour).
    pub initial_announce_count: u32,
    /// Period between initial-announcement-burst SPDP sends. Default
    /// [`DEFAULT_INITIAL_ANNOUNCE_PERIOD`].
    pub initial_announce_period: Duration,
    /// SPDP multicast group (IPv4). Default 239.255.0.1 (Spec §9.6.1.4.1).
    pub spdp_multicast_group: Ipv4Addr,
    /// Interface address for the multicast join. Default 0.0.0.0 (the
    /// kernel picks the default interface).
    pub multicast_interface: Ipv4Addr,

    /// C1: whether SPDP beacons are sent via multicast. Default `true`
    /// (spec behavior). `false` (env `ZERODDS_NO_MULTICAST`) → pure
    /// unicast discovery via [`Self::initial_peers`], not a single
    /// multicast packet — for networks that drop multicast (WiFi/cloud
    /// VPC), and for a rigorous multicast-free discovery proof.
    pub spdp_multicast_send: bool,

    /// A1: **discovery-server** mode. When `true`, this participant relays the
    /// raw SPDP (participant-locator) announcements between the clients that
    /// point their [`Self::initial_peers`] at it — so N clients discover each
    /// other through one well-known address instead of an O(N²) peer list or
    /// multicast. Crucially it relays **only SPDP** (participant discovery);
    /// SEDP (endpoint discovery, incl. dynamically-created ROS-2 Action
    /// endpoints) then happens **directly peer-to-peer** between the real
    /// participants — which is exactly why ROS-2 Actions work over it, unlike a
    /// SEDP-proxying discovery server. Default `false`. Plain discovery only
    /// (DDS-Security secured discovery-server relay is a follow-up).
    pub discovery_server: bool,

    /// C3: max reassemblable sample size (DoS cap of the fragment
    /// assembler). Larger samples are silently discarded. The rtps
    /// default was 1 MiB (the phase-1 assumption "large images = no
    /// use case") — too small for ROS PointCloud2/Image (often several
    /// MB). Default here 16 MiB; env `ZERODDS_MAX_SAMPLE_BYTES` (bytes)
    /// overrides. Still a deliberate DoS guard, just ROS-realistic.
    pub max_reassembly_sample_bytes: usize,

    /// C1 multicast-free discovery: unicast initial-peer locators to
    /// which SPDP beacons are sent **in addition** to multicast. Default
    /// empty (= pure multicast behavior as before). Populated via
    /// [`RuntimeConfig::default`] from the env `ZERODDS_PEERS` (comma
    /// list of `ip` or `ip:port`). An `ip` without a port is expanded to
    /// the well-known SPDP unicast ports of participant indices 0..N
    /// (see [`expand_initial_peer`]).
    pub initial_peers: Vec<Locator>,

    /// Transport for DCPS user traffic. `None` (default) → fall back to
    /// the env var `ZERODDS_USER_TRANSPORT`, otherwise UDPv4. Discovery
    /// (SPDP/SEDP) remains UDPv4 multicast independently of this. Ignored when
    /// [`Self::user_transports`] is non-empty.
    pub user_transport: Option<UserTransportKind>,

    /// Preference-ordered set of transports for DCPS user traffic. When
    /// non-empty, the runtime builds a [`LayeredUserTransport`](crate::layered_transport::LayeredUserTransport)
    /// over all of them: each datagram is routed to the first transport whose
    /// locator kind matches the destination (so list the fast/local transport
    /// first — e.g. `[Shm, UdpV4]` — and the fallback last), and receives are
    /// multiplexed from all of them. Empty (default) → single-transport via
    /// [`Self::user_transport`].
    pub user_transports: alloc::vec::Vec<UserTransportKind>,

    /// Optional security gate. Active only with the `security` feature.
    /// When set, UDP outbound messages are pulled through
    /// [`SharedSecurityGate::transform_outbound`], and inbound messages
    /// through [`SharedSecurityGate::transform_inbound_from`] (peer key
    /// from RTPS header bytes 8..20).
    #[cfg(feature = "security")]
    pub security: Option<std::sync::Arc<zerodds_security_runtime::SharedSecurityGate>>,
    /// Optional LoggingPlugin for security events. Called by the inbound
    /// path when packets are dropped due to a policy violation, tampering
    /// or a legacy block.
    #[cfg(feature = "security")]
    pub security_logger: Option<std::sync::Arc<dyn zerodds_security_runtime::LoggingPlugin>>,

    /// Multi-interface bindings. Empty → `user_unicast` is the only
    /// outbound socket (legacy behavior). Non-empty →
    /// `DcpsRuntime::start` builds a dedicated UDP socket per spec and the
    /// writer tick loop routes to the matching socket per destination
    /// locator.
    #[cfg(feature = "security")]
    pub interface_bindings: Vec<InterfaceBindingSpec>,

    /// `true` → the SPDP beacon additionally announces the 12 secure
    /// discovery bits (16..27, DDS-Security 1.2 §7.4.7.1). Default
    /// `false` — only standard bits are announced. Set by the DCPS
    /// factory once a PolicyEngine is configured. This flag is available
    /// even without the `security` feature, so that tests can check bit
    /// presence without activating the whole crypto crate.
    pub announce_secure_endpoints: bool,

    /// FastDDS interop: run the reliable secure SPDP channel (0xff0101c2/c7,
    /// `ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_SECURE_*`). FastDDS announces
    /// its full secured participant data (identity_token/security_info) over
    /// this channel and gates the crypto-token reciprocation/endpoint matching
    /// on it; cyclone does NOT need it (cyclone↔zerodds runs without). Default off
    /// — enable only for FastDDS cross-vendor.
    pub enable_secure_spdp: bool,

    /// WLP-Tick-Periode (Writer-Liveliness-Protocol, RTPS 2.5 §8.4.13).
    /// `Duration::ZERO` → default `participant_lease_duration / 3`
    /// (spec recommendation: three misses before the reader marks the
    /// writer as not-alive). A direct override enables aggressive
    /// tests.
    pub wlp_period: Duration,

    /// Lease duration announced in the SPDP beacon as
    /// `PARTICIPANT_LEASE_DURATION` (spec default 100 s). Also used as the
    /// basis for the AUTOMATIC WLP tick (`wlp_period =
    /// participant_lease_duration / 3` if `wlp_period == Duration::ZERO`).
    pub participant_lease_duration: Duration,

    /// USER_DATA bytes of the participant (DDS 1.4 §2.2.3.1
    /// `UserDataQosPolicy`). Announced in the SPDP beacon as PID_USER_DATA
    /// (DDSI-RTPS §9.6.3.2) and exposed on the receiver side in
    /// `ParticipantBuiltinTopicData.user_data`. Default empty.
    pub user_data: Vec<u8>,

    /// Observability sink. Default is `null_sink()` — each event emit is
    /// then a direct return without allocation on the consumer side.
    /// Consumers inject e.g.
    /// [`zerodds_foundation::observability::StderrJsonSink`] (JSON lines
    /// for Vector/fluentd/Datadog) or their own OTLP bridge.
    pub observability: zerodds_foundation::observability::SharedSink,

    /// Sprint D.5d lever C — RT pinning + priority. Linux-only; on
    /// macOS/Windows the hooks are no-ops.
    ///
    /// SCHED_FIFO priority (1-99) for the three recv workers (SPDP MC,
    /// metatraffic, user data). `None` = default scheduler (CFS).
    /// `Some(80)` is the spec recommendation for real-time paths. Requires
    /// `CAP_SYS_NICE` or an `RLIMIT_RTPRIO`-permitted user.
    pub recv_thread_priority: Option<i32>,

    /// Like [`Self::recv_thread_priority`], but for the tick worker.
    pub tick_thread_priority: Option<i32>,

    /// CPU affinity mask for the recv workers. `None` = no affinity (the
    /// kernel schedules freely). A list of CPU indices, e.g.
    /// `vec![2, 3]` for cores 2+3. Set via `sched_setaffinity`; all three
    /// recv threads share the same mask.
    pub recv_thread_cpus: Option<Vec<usize>>,

    /// Like [`Self::recv_thread_cpus`], but for the tick worker.
    pub tick_thread_cpus: Option<Vec<usize>>,

    /// Opt-3 (Spec `zerodds-zero-copy-1.0` §9): number of additional
    /// user-data recv workers that listen on the same port as
    /// `user_unicast` via `SO_REUSEPORT`. `0` (default) = only the primary
    /// `recv_user_data_loop` worker. Under high recv load the pool scales
    /// linearly with cores (kernel flow hashing distributes incoming
    /// datagrams). Recommended values: 1-3 additional workers per CPU
    /// core.
    pub extra_recv_threads: usize,

    /// D.5g — default DataRepresentation list announced in SEDP
    /// PublicationData and SEDP SubscriptionData, when not overridden
    /// per-writer/reader (UserWriterConfig/UserReaderConfig).
    ///
    /// **Important**: per strict spec (XTypes 1.3 §7.6.3.1.2) the first
    /// element is the writer's "offered" and must be in the reader's
    /// "accepted" list for a match to happen. Default `[XCDR1, XCDR2]` =
    /// legacy-first → max interop with the RTI Connext Shapes Demo
    /// (XCDR1-only). Pure-XCDR2 deployments can switch this to `[XCDR2]`
    /// or `[XCDR2, XCDR1]` for bandwidth efficiency and
    /// @appendable/@mutable support.
    ///
    /// Empty (`vec![]`) is interpreted per spec as `[XCDR1]`.
    pub data_representation_offer: Vec<i16>,

    /// D.5g — default match mode for DataRepresentation negotiation.
    ///
    /// `Strict` (XTypes 1.3 §7.6.3.1.2 normative): writer.first ∈
    /// reader.list = match. `Tolerant` (industry norm): any overlap =
    /// match, picks the first overlap as the wire format.
    ///
    /// Default `Tolerant` because Cyclone DDS and FastDDS match this way —
    /// maximizes interop. The strict setting is only meaningful for
    /// formal spec-compliance tests.
    pub data_rep_match_mode: zerodds_rtps::publication_data::data_representation::DataRepMatchMode,

    /// zerodds-async-1.0 §4 — when `true`, `start()` does **not** spawn the
    /// dedicated `zdds-tick` std::thread. The periodic tick (SPDP announce,
    /// SEDP/WLP, deadline/lifespan/liveliness) must then be driven externally
    /// via [`DcpsRuntime::tick_driver`]. Used by the async API's
    /// `spawn_in_tokio`, which multiplexes many participants' tick loops onto
    /// a tokio runtime instead of one thread each. Default `false` (internal
    /// thread, unchanged behaviour). The recv worker threads are unaffected —
    /// they block on socket recv and stay regardless.
    pub external_tick: bool,

    /// D.5e Phase 3 — when `true`, `start()` drives the periodic tick via the
    /// event-driven deadline scheduler ([`crate::scheduler`]) instead of the
    /// fixed-`tick_period` poll: the worker parks until the next due deadline
    /// (SPDP announce, or a fine floor while user endpoints/QoS timers are
    /// active) or until a write/recv `raise` wakes it — no busy-poll, lower idle
    /// CPU, lower tail latency. The work done per wake is the **unchanged**
    /// `run_tick_iteration` (identical wire output + cadence — cross-vendor
    /// safe). **Default `true`** since D.5e Phase C (2026-06-14) — set
    /// `ZERODDS_SCHEDULER_TICK=0` or this field to `false` for the classic
    /// fixed-period `tick_loop`. Mutually exclusive with `external_tick`
    /// (external wins).
    pub scheduler_tick: bool,
}

/// Configuration entry for a physical or logical network interface.
///
/// A binding describes an outbound socket: which IP/port it binds to,
/// which `NetInterface` class the interface represents, and which IP
/// range counts as "associated peers" (routing match).
#[cfg(feature = "security")]
#[derive(Clone, Debug)]
pub struct InterfaceBindingSpec {
    /// Name for diagnostics + log attribution (e.g. `"eth0"`, `"tun0"`,
    /// `"lo"`).
    pub name: String,
    /// Bind address. `0.0.0.0` leaves the interface to the kernel.
    pub bind_addr: Ipv4Addr,
    /// Bind port. `0` = ephemeral.
    pub bind_port: u16,
    /// Interface class — feeds into the PolicyEngine context.
    pub kind: NetInterface,
    /// Destination IP range this binding is responsible for. Example:
    /// `127.0.0.0/8` for loopback. A target whose IP lies in this range is
    /// routed to this binding.
    pub subnet: IpRange,
    /// If `true`: this binding is used when **no** other subnet match
    /// applies. Exactly one entry should have `default = true` (usually
    /// the WAN binding).
    pub default: bool,
}

/// Fully bound interface with its UDP socket.
#[cfg(feature = "security")]
struct InterfaceBinding {
    spec: InterfaceBindingSpec,
    socket: Arc<UdpTransport>,
}

/// Pool of per-interface UDP sockets with target-based routing.
///
/// Decision:
/// 1. Iterates over all bindings; the first whose subnet contains the
///    target wins.
/// 2. If no match and a default binding exists → default path.
/// 3. No match + no default → `None`, the caller drops.
#[cfg(feature = "security")]
struct OutboundSocketPool {
    bindings: Vec<InterfaceBinding>,
    default_idx: Option<usize>,
}

#[cfg(feature = "security")]
impl OutboundSocketPool {
    fn bind_all(specs: &[InterfaceBindingSpec]) -> Result<Self> {
        let mut bindings = Vec::with_capacity(specs.len());
        for spec in specs {
            let socket = UdpTransport::bind_v4(spec.bind_addr, spec.bind_port).map_err(|_| {
                DdsError::TransportError {
                    label: "interface-binding bind_v4 failed",
                }
            })?;
            // Short read timeout so that the per-interface inbound poll in
            // the event loop becomes non-blocking. 5 ms is small enough not
            // to create latency elsewhere (the tick period defaults to
            // 50 ms), but large enough to amortize context switches.
            let socket = socket
                .with_timeout(Some(Duration::from_millis(5)))
                .map_err(|_| DdsError::TransportError {
                    label: "interface-binding set_timeout failed",
                })?;
            bindings.push(InterfaceBinding {
                spec: spec.clone(),
                socket: Arc::new(socket),
            });
        }
        let default_idx = bindings.iter().position(|b| b.spec.default);
        Ok(Self {
            bindings,
            default_idx,
        })
    }

    /// Returns `(socket, NetInterface class)` for a destination locator.
    /// `None` if neither a subnet match nor a default binding exists.
    fn route(&self, target: &Locator) -> Option<(&Arc<UdpTransport>, NetInterface)> {
        let ip = ipv4_from_locator(target)?;
        let addr = core::net::IpAddr::V4(core::net::Ipv4Addr::from(ip));
        for b in &self.bindings {
            if b.spec.subnet.contains(&addr) {
                return Some((&b.socket, b.spec.kind.clone()));
            }
        }
        let idx = self.default_idx?;
        let b = self.bindings.get(idx)?;
        Some((&b.socket, b.spec.kind.clone()))
    }
}

/// True if the locator is routable over the user-data transport
/// (trait object). Accepts UDPv4, UDPv6, TCPv4, Shm. The concrete
/// transport (UdpTransport/TcpTransport/ShmUserTransport) then returns
/// `UnsupportedLocator` for kinds it does not itself speak;
/// the filter here only prevents sending to clearly non-IP/SHM
/// locators like UDS (for which we have no transport plugin).
fn is_routable_user_locator(loc: &Locator) -> bool {
    matches!(
        loc.kind,
        LocatorKind::UdpV4
            | LocatorKind::UdpV6
            | LocatorKind::Tcpv4
            | LocatorKind::Tcpv6
            | LocatorKind::Shm
            | LocatorKind::Uds
            | LocatorKind::Tsn
    )
}

/// Computes the user-endpoint `EndpointSecurityInfo` mask from the governance
/// protection kinds (DDS-Security 1.2 §10.4.1.2.6 / §9.4.2.4). The wire mask
/// MUST match cyclone/FastDDS/OpenDDS byte-exactly, otherwise the peer rejects
/// the endpoint match with "security_attributes mismatch".
///
/// - metadata=SIGN/ENCRYPT → IS_SUBMESSAGE_PROTECTED (+ plugin SUBMESSAGE_ENCRYPTED on ENCRYPT)
/// - data=SIGN    → IS_PAYLOAD_PROTECTED
/// - data=ENCRYPT → IS_PAYLOAD_PROTECTED | **IS_KEY_PROTECTED** (+ plugin PAYLOAD_ENCRYPTED)
/// - liveliness=SIGN/ENCRYPT → **IS_LIVELINESS_PROTECTED** (§9.4.1.3: per-endpoint!)
/// - topic enable_discovery_protection → IS_DISCOVERY_PROTECTED
///
/// is_key_protected follows §10.4.1.2.6 exclusively from the **DATA** protection
/// and only on ENCRYPT — NOT from the metadata protection. is_liveliness_protected
/// in contrast MUST be on every user endpoint as soon as liveliness_protection is active;
/// cyclone compares the mask at endpoint match and otherwise rejects with
/// "security_attributes mismatch" (0x..30 vs 0x..70).
#[cfg(feature = "security")]
fn compute_user_endpoint_attrs(
    meta: ProtectionLevel,
    data: ProtectionLevel,
    discovery_protected: bool,
    liveliness_protected: bool,
    read_protected: bool,
    write_protected: bool,
) -> zerodds_rtps::endpoint_security_info::EndpointSecurityInfo {
    use zerodds_rtps::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};
    let mut a = attrs::IS_VALID;
    let mut p = plugin_attrs::IS_VALID;
    if read_protected {
        a |= attrs::IS_READ_PROTECTED;
    }
    if write_protected {
        a |= attrs::IS_WRITE_PROTECTED;
    }
    if meta != ProtectionLevel::None {
        a |= attrs::IS_SUBMESSAGE_PROTECTED;
    }
    if meta == ProtectionLevel::Encrypt {
        p |= plugin_attrs::IS_SUBMESSAGE_ENCRYPTED;
    }
    if data != ProtectionLevel::None {
        a |= attrs::IS_PAYLOAD_PROTECTED;
    }
    if data == ProtectionLevel::Encrypt {
        a |= attrs::IS_KEY_PROTECTED;
        p |= plugin_attrs::IS_PAYLOAD_ENCRYPTED;
    }
    if discovery_protected {
        a |= attrs::IS_DISCOVERY_PROTECTED;
    }
    if liveliness_protected {
        a |= attrs::IS_LIVELINESS_PROTECTED;
    }
    EndpointSecurityInfo {
        endpoint_security_attributes: a,
        plugin_endpoint_security_attributes: p,
    }
}

#[cfg(all(test, feature = "security"))]
mod endpoint_attr_tests {
    use super::compute_user_endpoint_attrs;
    use zerodds_rtps::endpoint_security_info::attrs;
    use zerodds_security_runtime::ProtectionLevel;

    fn mask(meta: ProtectionLevel, data: ProtectionLevel) -> u32 {
        compute_user_endpoint_attrs(meta, data, false, false, false, false)
            .endpoint_security_attributes
    }

    fn mask_liv(meta: ProtectionLevel, data: ProtectionLevel) -> u32 {
        compute_user_endpoint_attrs(meta, data, false, true, false, false)
            .endpoint_security_attributes
    }

    #[test]
    fn liveliness_protected_sets_0x40_per_spec_9_4_1_3() {
        use ProtectionLevel::{Encrypt, None};
        let v = attrs::IS_VALID;
        let pay = attrs::IS_PAYLOAD_PROTECTED;
        let key = attrs::IS_KEY_PROTECTED;
        let liv = attrs::IS_LIVELINESS_PROTECTED;
        // liveliness=ENCRYPT + data=ENCRYPT → 0x..70 (cyclone's value at match).
        assert_eq!(mask_liv(None, Encrypt), v | pay | key | liv);
        // without liveliness → 0x..30, NO 0x40.
        assert_eq!(mask(None, Encrypt), v | pay | key);
        assert_eq!(mask_liv(None, None), v | liv);
    }

    #[test]
    fn key_protected_follows_data_encrypt_per_spec_10_4_1_2_6() {
        use ProtectionLevel::{Encrypt, None, Sign};
        let v = attrs::IS_VALID;
        let sub = attrs::IS_SUBMESSAGE_PROTECTED;
        let pay = attrs::IS_PAYLOAD_PROTECTED;
        let key = attrs::IS_KEY_PROTECTED;
        // §10.4.1.2.6: is_key_protected follows ONLY data=ENCRYPT.
        // data=ENCRYPT → PAYLOAD|KEY (= cyclone's 0x30 in the common subset).
        assert_eq!(mask(None, Encrypt), v | pay | key);
        // data=SIGN → PAYLOAD, NO KEY.
        assert_eq!(mask(None, Sign), v | pay);
        // data=NONE → no payload/key bits.
        assert_eq!(mask(None, None), v);
        // KEY does NOT depend on metadata: meta=ENCRYPT/data=NONE → only SUBMESSAGE.
        assert_eq!(mask(Encrypt, None), v | sub);
        // meta=ENCRYPT + data=ENCRYPT → SUBMESSAGE|PAYLOAD|KEY (0x38).
        assert_eq!(mask(Encrypt, Encrypt), v | sub | pay | key);
    }
}

/// Unicast targets for the WLP heartbeat fan-out (M-2): per discovered peer the
/// `metatraffic_unicast_locator` (fallback `default_unicast_locator`), filtered
/// to routable kinds. WLP is metatraffic (DDSI-RTPS §8.4.13); in multicast-
/// free environments (container/cloud) the pure multicast pulse never reaches the
/// peer reader → the lease expires although the peer is alive. The additional
/// unicast fan-out follows the SEDP locator model.
fn wlp_unicast_targets(peers: &[zerodds_discovery::spdp::DiscoveredParticipant]) -> Vec<Locator> {
    peers
        .iter()
        .filter_map(|dp| {
            dp.data
                .metatraffic_unicast_locator
                .or(dp.data.default_unicast_locator)
        })
        .filter(is_routable_user_locator)
        .collect()
}

/// Extracts the IPv4 address from a `Locator` (UDP-V4).
/// `None` for SHM/UDS/IPv6.
#[cfg(feature = "security")]
fn ipv4_from_locator(loc: &Locator) -> Option<[u8; 4]> {
    if loc.kind != LocatorKind::UdpV4 {
        return None;
    }
    Some([
        loc.address[12],
        loc.address[13],
        loc.address[14],
        loc.address[15],
    ])
}

impl core::fmt::Debug for RuntimeConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut dbg = f.debug_struct("RuntimeConfig");
        dbg.field("tick_period", &self.tick_period)
            .field("spdp_period", &self.spdp_period)
            .field("spdp_multicast_group", &self.spdp_multicast_group)
            .field("multicast_interface", &self.multicast_interface);
        #[cfg(feature = "security")]
        {
            dbg.field("security", &self.security.as_ref().map(|_| "<gate>"));
            dbg.field(
                "security_logger",
                &self.security_logger.as_ref().map(|_| "<logger>"),
            );
        }
        dbg.finish()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        // Env hook for bench tuning: ZERODDS_TICK_PERIOD_MS=N → overrides
        // the 5ms default. High (e.g. 1000) relieves the write hot path of
        // the periodic HB/tick overhead and makes spread spikes from tick
        // preemption visible. Production: do not set; the default 5 ms is
        // spec-compliant.
        let tick = std::env::var("ZERODDS_TICK_PERIOD_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_TICK_PERIOD);
        // C3 WiFi-robust discovery — initial-announcement burst. Env overrides:
        // `ZERODDS_INITIAL_ANNOUNCE_COUNT` (0 disables) +
        // `ZERODDS_INITIAL_ANNOUNCE_PERIOD_MS`.
        let initial_announce_count = std::env::var("ZERODDS_INITIAL_ANNOUNCE_COUNT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_INITIAL_ANNOUNCE_COUNT);
        let initial_announce_period = std::env::var("ZERODDS_INITIAL_ANNOUNCE_PERIOD_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_INITIAL_ANNOUNCE_PERIOD);
        Self {
            tick_period: tick,
            spdp_period: DEFAULT_SPDP_PERIOD,
            initial_announce_count,
            initial_announce_period,
            // Env override `ZERODDS_SPDP_MC_GROUP` (IPv4) of the SPDP
            // multicast group. Two processes with different groups do NOT
            // see each other via multicast → enables a multicast-free C1
            // e2e proof (discovery then only via ZERODDS_PEERS). Default is
            // the spec group.
            spdp_multicast_group: std::env::var("ZERODDS_SPDP_MC_GROUP")
                .ok()
                .and_then(|s| s.parse::<Ipv4Addr>().ok())
                .unwrap_or_else(|| Ipv4Addr::from(SPDP_DEFAULT_MULTICAST_ADDRESS)),
            // Interface pinning (Cyclone `NetworkInterface`/FastDDS
            // whitelist equivalent): `ZERODDS_INTERFACE=<ipv4>` forces
            // announce + bind on this interface. Default UNSPECIFIED = auto
            // (route probe). Critical on multi-homed hosts (VPN/Docker/
            // macOS bridge100), where the auto choice may announce the
            // wrong interface.
            multicast_interface: std::env::var("ZERODDS_INTERFACE")
                .ok()
                .and_then(|s| s.parse::<Ipv4Addr>().ok())
                .unwrap_or(Ipv4Addr::UNSPECIFIED),
            // Multicast send on by default; `ZERODDS_NO_MULTICAST` (any
            // non-empty value) turns it off → pure unicast discovery.
            spdp_multicast_send: std::env::var("ZERODDS_NO_MULTICAST")
                .map(|v| v.is_empty())
                .unwrap_or(true),
            // A1: off by default; env `ZERODDS_DISCOVERY_SERVER` (any non-empty
            // value) or `RuntimeConfig::discovery_server()` turns the relay on.
            discovery_server: std::env::var("ZERODDS_DISCOVERY_SERVER")
                .map(|v| !v.is_empty())
                .unwrap_or(false),
            // C3: 16 MiB default (suitable for ROS PointCloud2/Image),
            // env override `ZERODDS_MAX_SAMPLE_BYTES`.
            max_reassembly_sample_bytes: std::env::var("ZERODDS_MAX_SAMPLE_BYTES")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(16 * 1024 * 1024),
            // Programmatic default empty. The env `ZERODDS_PEERS` is
            // expanded domain-aware only in `DcpsRuntime::start` and merged
            // with this field into the effective peer list.
            initial_peers: Vec::new(),
            user_transport: None,
            user_transports: alloc::vec::Vec::new(),
            #[cfg(feature = "security")]
            security: None,
            #[cfg(feature = "security")]
            security_logger: None,
            #[cfg(feature = "security")]
            interface_bindings: Vec::new(),
            announce_secure_endpoints: false,
            // Env hook for bench/FastDDS interop: ZERODDS_SECURE_SPDP=1 turns
            // on the reliable secure SPDP channel (0xff0101). Production sets this
            // explicitly via the SecurityProfile/config.
            enable_secure_spdp: std::env::var("ZERODDS_SECURE_SPDP").ok().as_deref() == Some("1"),
            wlp_period: Duration::ZERO,
            participant_lease_duration: Duration::from_secs(100),
            user_data: Vec::new(),
            observability: zerodds_foundation::observability::null_sink(),
            recv_thread_priority: None,
            tick_thread_priority: None,
            recv_thread_cpus: None,
            tick_thread_cpus: None,
            extra_recv_threads: 0,
            // D.5g — default `[XCDR1, XCDR2]` (legacy-first, max interop).
            // Env-var override `ZERODDS_DATA_REPR_OFFER` as a comma list
            // ("XCDR1", "XCDR2", "XCDR1,XCDR2", "XCDR2,XCDR1"). Cross-vendor
            // benches against strict-matching vendors (RTI) need XCDR2-only
            // so that every wire match happens.
            data_representation_offer: parse_data_repr_offer_env().unwrap_or_else(|| {
                zerodds_rtps::publication_data::data_representation::DEFAULT_OFFER.to_vec()
            }),
            data_rep_match_mode:
                zerodds_rtps::publication_data::data_representation::DataRepMatchMode::default(),
            external_tick: false,
            // D.5e Phase 3 — the event-driven deadline-heap scheduler is the
            // DEFAULT tick (Phase C, 2026-06-14): it parks until the next due
            // deadline / a write-recv raise instead of polling every 5 ms (~17×
            // fewer idle iterations, lower tail latency, identical wire output).
            // Verified cross-vendor secured (data-enc + rtps-enc all pairs) +
            // same_host_e2e + latency_assertions on codepit. Escape hatch:
            // `ZERODDS_SCHEDULER_TICK=0` restores the classic fixed-period
            // `tick_loop`.
            scheduler_tick: std::env::var("ZERODDS_SCHEDULER_TICK")
                .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                .unwrap_or(true),
        }
    }
}

impl RuntimeConfig {
    /// Apply a [`SecurityBundle`](zerodds_security_runtime::SecurityBundle):
    /// wires its security-event logger into [`Self::security_logger`] and, if
    /// the bundle carries a [`SecurityProfile`](zerodds_security_runtime::SecurityProfile),
    /// its gate into [`Self::security`]. Convenience for the common
    /// `SecurityBundle::builder()…build()` flow so callers don't have to set
    /// the two fields by hand.
    #[cfg(feature = "security")]
    #[must_use]
    pub fn with_security_bundle(
        mut self,
        bundle: &zerodds_security_runtime::SecurityBundle,
    ) -> Self {
        if let Some(logger) = bundle.logging_plugin() {
            self.security_logger = Some(logger);
        }
        if let Some(profile) = bundle.security_profile() {
            self.security = Some(profile.gate.clone());
        }
        self
    }

    /// Materialize a security-event logger from `dds.sec.log.*` properties and
    /// wire it into [`Self::security_logger`]. This is the DDS-Security
    /// spec-style alternative to handing a logger object in directly (see
    /// [`Self::with_security_bundle`]): the participant carries
    /// `dds.sec.log.plugin = "stderr,jsonl"` (+ `dds.sec.log.*` parameters) on
    /// its [`PropertyQosPolicy`](zerodds_qos::PropertyQosPolicy), and the
    /// runtime builds the fan-out logger from them.
    ///
    /// No-op when `dds.sec.log.plugin` is absent. Errors if a selected sink is
    /// misconfigured (e.g. `jsonl` without `dds.sec.log.jsonl.path`).
    #[cfg(feature = "security")]
    pub fn with_security_log_properties(
        mut self,
        property: &zerodds_qos::PropertyQosPolicy,
    ) -> core::result::Result<Self, zerodds_security_logging::LogConfigError> {
        let pairs: alloc::vec::Vec<(&str, &str)> = property.iter().collect();
        if let Some(logger) = zerodds_security_logging::logging_plugin_from_properties(&pairs)? {
            self.security_logger = Some(alloc::sync::Arc::from(logger));
        }
        Ok(self)
    }

    /// C4: robotics-capable defaults for **out-of-the-box ROS-2 interop**.
    /// Saves the manual env tuning otherwise needed for real ROS-2 nodes.
    /// Specifically, compared to [`RuntimeConfig::default`]:
    /// - **`data_representation_offer = [XCDR1, XCDR2]`**: `rmw_cyclonedds`/
    ///   `rmw_fastrtps` write XCDR1 for final/simple types (e.g.
    ///   `std_msgs/String`). An XCDR2-only reader does not match an XCDR1
    ///   writer — so the ROS reader here offers both legacy-first
    ///   (tolerant match is already the default). This is the clean,
    ///   ROS-specific variant of the `ZERODDS_DATA_REPR_OFFER` env
    ///   workaround, WITHOUT changing the global `DEFAULT_OFFER`
    ///   (XCDR2-only, deliberately for FastDDS/OpenDDS XCDR2 readers).
    ///
    /// The ROS-realistic reassembly cap (16 MiB, PointCloud2/Image) is
    /// already the global default and is carried over here.
    #[must_use]
    pub fn ros_defaults() -> Self {
        use zerodds_rtps::publication_data::data_representation as dr;
        Self {
            data_representation_offer: alloc::vec![dr::XCDR, dr::XCDR2],
            ..Self::default()
        }
    }

    /// C6 multi-robot / WAN / cross-subnet profile.
    ///
    /// A named profile for fleets that span subnets, the cloud, or WiFi —
    /// environments that drop IP multicast, so SPDP discovery cannot rely on
    /// the multicast beacon. It is the [`ros_defaults`](Self::ros_defaults)
    /// representation offer **plus**:
    ///
    /// - **Multicast-free discovery** (`spdp_multicast_send = false`):
    ///   participants find each other purely through unicast initial peers,
    ///   regardless of the `ZERODDS_NO_MULTICAST` env. Set the peers via
    ///   `ZERODDS_PEERS` (a comma list of `ip` or `ip:port`); a port-less
    ///   `ip` is expanded to the well-known SPDP unicast ports of the first
    ///   N participant indices (`ZERODDS_MAX_PEER_PARTICIPANTS`).
    /// - **WAN-tolerant liveliness**: a longer participant lease (300 s vs
    ///   the 100 s spec default) so transient cross-subnet RTT spikes or
    ///   brief link drops do not trigger a false liveliness loss.
    ///
    /// **Domain isolation** is the caller's lever: pass a fleet-dedicated
    /// `domain_id` to [`DcpsRuntime::start`] to keep robots off the default
    /// domain 0. The profile deliberately does not pick a domain for you.
    ///
    /// ```
    /// use zerodds_dcps::runtime::RuntimeConfig;
    /// let cfg = RuntimeConfig::multi_robot();
    /// assert!(!cfg.spdp_multicast_send); // unicast-only discovery
    /// ```
    pub fn multi_robot() -> Self {
        use zerodds_rtps::publication_data::data_representation as dr;
        Self {
            data_representation_offer: alloc::vec![dr::XCDR, dr::XCDR2],
            spdp_multicast_send: false,
            participant_lease_duration: Duration::from_secs(300),
            ..Self::default()
        }
    }

    /// A1 — **discovery-server** profile (the server side). Multicast-free, with
    /// the SPDP relay on ([`Self::discovery_server`]): clients point their
    /// `initial_peers`/`ZERODDS_PEERS` at this one well-known address and the
    /// server bridges their participant discovery, so N clients find each other
    /// without an O(N²) peer list or multicast. SEDP (incl. ROS-2 Action
    /// endpoints) stays direct peer-to-peer — no SEDP proxy, no Action breakage.
    ///
    /// Clients run with [`RuntimeConfig::multi_robot`] (or any multicast-free
    /// config) and set `ZERODDS_PEERS` to the server's address.
    ///
    /// ```
    /// use zerodds_dcps::runtime::RuntimeConfig;
    /// let server = RuntimeConfig::discovery_server();
    /// assert!(server.discovery_server);
    /// assert!(!server.spdp_multicast_send); // unicast-only
    /// ```
    pub fn discovery_server() -> Self {
        Self {
            discovery_server: true,
            ..Self::multi_robot()
        }
    }
}

/// Parse the `ZERODDS_DATA_REPR_OFFER` env var. Values: "XCDR1", "XCDR2",
/// or a comma list. None if the env var is missing or invalid.
fn parse_data_repr_offer_env() -> Option<Vec<i16>> {
    let s = std::env::var("ZERODDS_DATA_REPR_OFFER").ok()?;
    parse_data_repr_offer_str(&s)
}

/// Computes the **well-known** SPDP unicast discovery port for a
/// domain + participant index. Formula (DDSI-RTPS 2.5 §9.6.1.4.1):
///   port = PB + DG·domain + d1 + PG·pid = 7400 + 250·domain + 10 + 2·pid
///
/// This lets a configured unicast initial peer (multicast-free discovery)
/// reach a participant deterministically WITHOUT having found it via
/// multicast first. Defined locally in `dcps` to avoid touching
/// `crates/rtps` (spec constants as literals).
#[must_use]
fn spdp_unicast_port(domain_id: u32, participant_id: u32) -> u32 {
    7400 + 250 * domain_id + 10 + 2 * participant_id
}

/// Default number of participant indices a port-less initial peer is
/// expanded to (Cyclone equivalent: `MaxAutoParticipantIndex`). The
/// beacon thereby reaches the first N participants of the peer host via
/// their well-known SPDP unicast ports. Overridable via the env
/// `ZERODDS_MAX_PEER_PARTICIPANTS` (e.g. for dense multi-robot / >10
/// participants-per-host scenarios). Cap 120 (= the well-known-port
/// allocation window).
const INITIAL_PEER_MAX_PARTICIPANTS: u32 = 10;

/// Effective peer-expansion limit: env `ZERODDS_MAX_PEER_PARTICIPANTS`
/// or [`INITIAL_PEER_MAX_PARTICIPANTS`], clamped to 1..=120.
fn initial_peer_max_participants() -> u32 {
    std::env::var("ZERODDS_MAX_PEER_PARTICIPANTS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(INITIAL_PEER_MAX_PARTICIPANTS)
        .clamp(1, 120)
}

/// C1 multicast-free discovery: parses the env `ZERODDS_PEERS` (comma
/// list of `ip` or `ip:port`) into SPDP unicast initial-peer locators for
/// `domain_id`. Empty/invalid → empty list.
fn parse_initial_peers_env(domain_id: u32) -> Vec<Locator> {
    let mut out = Vec::new();
    let max = initial_peer_max_participants();
    if let Ok(s) = std::env::var("ZERODDS_PEERS") {
        for entry in s.split(',') {
            expand_initial_peer(entry.trim(), domain_id, max, &mut out);
        }
    }
    out
}

/// Expands a single peer spec into locator(s) and appends them to `out`.
/// `ip:port` → exact locator. Just `ip` → well-known SPDP unicast ports
/// of participant indices `0..max_participants` (Spec §9.6.1.4.1).
/// Invalid specs are ignored.
fn expand_initial_peer(spec: &str, domain_id: u32, max_participants: u32, out: &mut Vec<Locator>) {
    if spec.is_empty() {
        return;
    }
    if let Some((ip_s, port_s)) = spec.rsplit_once(':') {
        if let (Ok(ip), Ok(port)) = (ip_s.parse::<Ipv4Addr>(), port_s.parse::<u16>()) {
            out.push(Locator::udp_v4(ip.octets(), u32::from(port)));
            return;
        }
    }
    if let Ok(ip) = spec.parse::<Ipv4Addr>() {
        for pid in 0..max_participants {
            if let Ok(port) = u16::try_from(spdp_unicast_port(domain_id, pid)) {
                out.push(Locator::udp_v4(ip.octets(), u32::from(port)));
            }
        }
    }
}

/// Pure parser for the `ZERODDS_DATA_REPR_OFFER` syntax (testable without
/// env). Returns the DataRepresentationId list with the **spec values**
/// `XCDR=0`, `XCDR2=2` (XTypes 1.3 §7.6.3.1.2) — NOT version numbers.
/// `None` on empty/invalid input.
fn parse_data_repr_offer_str(s: &str) -> Option<Vec<i16>> {
    use zerodds_rtps::publication_data::data_representation as dr;
    let mut out = Vec::new();
    for tok in s.split(',').map(str::trim) {
        let v = match tok.to_ascii_uppercase().as_str() {
            "XCDR1" | "XCDR" | "1" => dr::XCDR,
            "XCDR2" | "2" => dr::XCDR2,
            _ => return None,
        };
        out.push(v);
    }
    if out.is_empty() { None } else { Some(out) }
}

// ---------------------------------------------------------------------------
// Security-gate helpers
// ---------------------------------------------------------------------------

/// Pull outbound UDP bytes through the security gate (when configured).
/// Without the `security` feature or without a gate: pass-through (clone
/// as Vec).
///
/// Errors in the gate are logged silently and the packet is **not** sent —
/// better to drop than leak plaintext.
/// DDS-Security 8.4.2.4: the RTPS message protection (message-level SRTPS)
/// does NOT apply to bootstrap traffic that must flow BEFORE the participant
/// crypto-key exchange: SPDP (participant discovery, to everyone) and the
/// ParticipantStatelessMessage (auth handshake). Wrapping them in SRTPS would
/// mean a not-yet-authenticated peer could not decrypt them
/// (no key) -> discovery/auth breaks (match timeout pub=0 sub=0). Detection
/// via the writer EntityId of the DATA/DATA_FRAG submessages.
#[cfg(feature = "security")]
fn rtps_message_protection_exempt(
    bytes: &[u8],
    discovery_plain: bool,
    liveliness_plain: bool,
) -> bool {
    use zerodds_rtps::wire_types::EntityId;
    // Bootstrap endpoints (§8.4.2.4): SPDP/Stateless/Volatile ALWAYS flow
    // plain (before/during key exchange resp. their own submessage protection).
    // Discovery plane (SEDP pub/sub, TypeLookup) is plain when discovery_
    // protection_kind=NONE; WLP (ParticipantMessage) plain when liveliness_
    // protection_kind=NONE. cyclone<->cyclone reference capture: under rtps_
    // protection=ENCRYPT + discovery=NONE cyclone sends the ENTIRE discovery
    // plane (DATA+HEARTBEAT+ACKNACK) PLAINTEXT — only user DATA is SRTPS-
    // wrapped. ZeroDDS must mirror this, otherwise it drops cyclone's plain
    // SubscriptionData as legacy_blocked -> no user-endpoint match.
    let entity_exempt = |e: EntityId| -> bool {
        matches!(
            e,
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
                | EntityId::SPDP_BUILTIN_PARTICIPANT_READER
                | EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                | EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER
                | EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
                | EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
        ) || (discovery_plain
            && matches!(
                e,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
                    | EntityId::SEDP_BUILTIN_PUBLICATIONS_READER
                    | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
                    | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER
                    | EntityId::TL_SVC_REQ_WRITER
                    | EntityId::TL_SVC_REQ_READER
                    | EntityId::TL_SVC_REPLY_WRITER
                    | EntityId::TL_SVC_REPLY_READER
            ))
            || (liveliness_plain
                && matches!(
                    e,
                    EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER
                        | EntityId::BUILTIN_PARTICIPANT_MESSAGE_READER
                ))
    };
    let Ok(parsed) = decode_datagram(bytes) else {
        return false;
    };
    // Datagram exempt if it has at least one relevant submessage AND
    // ALL relevant ones are exempt (.all) — otherwise a bundled
    // exempt+non-exempt datagram leaks the protection-worthy submessage.
    let relevant: alloc::vec::Vec<bool> = parsed
        .submessages
        .iter()
        .filter_map(|sm| match sm {
            ParsedSubmessage::Data(d) => {
                Some(entity_exempt(d.reader_id) || entity_exempt(d.writer_id))
            }
            ParsedSubmessage::DataFrag(d) => {
                Some(entity_exempt(d.reader_id) || entity_exempt(d.writer_id))
            }
            ParsedSubmessage::Heartbeat(h) => {
                Some(entity_exempt(h.reader_id) || entity_exempt(h.writer_id))
            }
            ParsedSubmessage::AckNack(a) => {
                Some(entity_exempt(a.reader_id) || entity_exempt(a.writer_id))
            }
            ParsedSubmessage::Gap(g) => {
                Some(entity_exempt(g.reader_id) || entity_exempt(g.writer_id))
            }
            ParsedSubmessage::NackFrag(n) => {
                Some(entity_exempt(n.reader_id) || entity_exempt(n.writer_id))
            }
            // SEC_PREFIX (Kx-Volatile, inner writer-id encrypted) -> exempt.
            ParsedSubmessage::Unknown { id: 0x31, .. } => Some(true),
            // Framing (INFO_DST/INFO_TS/...) -> neutral.
            _ => None,
        })
        .collect();
    !relevant.is_empty() && relevant.iter().all(|&b| b)
}

#[cfg(feature = "security")]
fn secure_outbound_bytes<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    match &rt.config.security {
        // OUTBOUND is spec-strict (DDS-Security 8.4.2.4 Table 27 is_rtps_protected):
        // under rtps_protection the ENTIRE RTPS message is SRTPS-wrapped; ONLY the
        // "separate messages" (SPDP/Stateless/Volatile) flow plain. SEDP/WLP/
        // TypeLookup are NOT among them and must be wrapped — independent
        // of discovery_/liveliness_protection (those are orthogonal submessage layers).
        // -> discovery_plain=false, liveliness_plain=false forces the wrap.
        // OpenDDS' RtpsUdpReceiveStrategy::check_encoded otherwise drops every plain SEDP
        // as "Full message requires protection". cyclone does take the shortcut
        // (sends SEDP plain), but accepts wrapped SEDP inbound without issue.
        // The INBOUND path (secure_inbound_bytes) deliberately stays lenient and still
        // accepts cyclone's plain SEDP — the asymmetry is intentional.
        Some(gate) if rtps_message_protection_exempt(bytes, false, false) => {
            let _ = gate;
            Some(alloc::borrow::Cow::Borrowed(bytes))
        }
        Some(gate) => gate
            .transform_outbound(bytes)
            .ok()
            .map(alloc::borrow::Cow::Owned),
        None => Some(alloc::borrow::Cow::Borrowed(bytes)),
    }
}

// Security off: no clone — the caller borrows the datagram bytes
// directly (copy 6 of the zero-copy audit eliminated).
#[cfg(not(feature = "security"))]
fn secure_outbound_bytes<'a>(
    _rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    Some(alloc::borrow::Cow::Borrowed(bytes))
}

/// Pull inbound UDP bytes through the security gate.
///
/// Expects an RTPS header with the GuidPrefix at bytes 8..20.
/// `None` → drop the packet.
///
/// Security: drop reasons are forwarded, differentiated, to the
/// configured `LoggingPlugin`:
/// * `Malformed`       → `Error`
/// * `LegacyBlocked`   → `Error`
/// * `PolicyViolation` → `Warning` (possible tampering)
/// * `CryptoError`     → `Warning` (tag mismatch, replay etc.)
#[cfg(feature = "security")]
fn secure_inbound_bytes<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
    iface: &NetInterface,
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    use zerodds_security_runtime::{InboundVerdict, LogLevel};
    let Some(gate) = &rt.config.security else {
        return Some(alloc::borrow::Cow::Borrowed(bytes));
    };
    // DDS-Security 8.4.2.4 (symmetric to outbound): SPDP/Stateless are
    // message-protection-exempt and ALWAYS arrive plain (also from cyclone). Without
    // this exception classify_inbound discards plain SPDP on the WAN iface under
    // rtps_protection as LegacyBlocked -> no discovery (match timeout).
    {
        let looks_srtps = bytes.len() > 20usize && bytes[20usize] == 0x33;
        if !looks_srtps
            && rtps_message_protection_exempt(
                bytes,
                gate.discovery_protection().unwrap_or(ProtectionLevel::None)
                    == ProtectionLevel::None,
                gate.liveliness_protection()
                    .unwrap_or(ProtectionLevel::None)
                    == ProtectionLevel::None,
            )
        {
            // SRTPS-exempt. BUT metadata_protection user DATA carries per-submessage
            // SEC_PREFIX/BODY/POSTFIX (§9.5.3.3, NO SRTPS) — that must still be
            // decrypted per-endpoint here, otherwise the reader gets the
            // SEC wrapper instead of the DATA. Volatile-Kx-SEC fails with None
            // (key_id not in user-remote_by_key_id) -> unchanged for the
            // Volatile handler in the metatraffic loop.
            if walk_submessages(bytes)
                .iter()
                .any(|(id, _, _)| *id == SMID_SEC_PREFIX)
            {
                let mut pk = [0u8; 12];
                pk.copy_from_slice(&bytes[8..20]);
                if let Some(mut dg) = unprotect_user_datagram(rt, bytes, &pk) {
                    match unprotect_user_payload(rt, &dg) {
                        PayloadDecode::Decoded(clear) => dg = clear,
                        PayloadDecode::Failed => return None,
                        PayloadDecode::NotEncrypted => {}
                    }
                    return Some(alloc::borrow::Cow::Owned(dg));
                }
            }
            return Some(alloc::borrow::Cow::Borrowed(bytes));
        }
    }
    let verdict = gate.classify_inbound(bytes, iface);
    let category = verdict.category();
    let (level, message): (LogLevel, String) = match &verdict {
        InboundVerdict::Accept(out) => {
            // Cross-vendor user DATA: cyclone protects the DATA submessage as a
            // SEC_PREFIX/BODY/POSTFIX sequence (metadata_protection=ENCRYPT). Before
            // the submessage parse, transform it back with the sender's data key
            // (GuidPrefix = bytes[8..20]). `unprotect_user_datagram` returns
            // `None` when no SEC_* sequence is present → normal accept path.
            // OUTER layer first (metadata_protection, SEC_PREFIX/BODY/
            // POSTFIX), then the INNER one (data_protection, encrypted
            // SerializedPayload §9.5.3.3.1). Both can be active at once
            // (full secure profile); each returns `None` when its layer
            // is not present -> then the datagram stays unchanged.
            let mut dg: alloc::vec::Vec<u8> = out.clone();
            if dg.len() >= 20 {
                let mut pk = [0u8; 12];
                pk.copy_from_slice(&dg[8..20]);
                if let Some(clear) = unprotect_user_datagram(rt, &dg, &pk) {
                    dg = clear;
                }
            }
            match unprotect_user_payload(rt, &dg) {
                PayloadDecode::Decoded(clear) => dg = clear,
                // Undecodable encrypted payload -> discard the datagram
                // (no ciphertext garbage to the reader; reliable re-send resp. another
                // copy delivers the sample later).
                PayloadDecode::Failed => return None,
                PayloadDecode::NotEncrypted => {}
            }
            return Some(alloc::borrow::Cow::Owned(dg));
        }
        InboundVerdict::Malformed => (
            LogLevel::Error,
            alloc::format!(
                "inbound datagram too short ({} bytes, iface={:?})",
                bytes.len(),
                iface
            ),
        ),
        InboundVerdict::LegacyBlocked => (
            LogLevel::Error,
            alloc::format!(
                "legacy plaintext peer on protected domain \
                 (iface={iface:?}, allow_unauthenticated_participants=false)"
            ),
        ),
        InboundVerdict::PolicyViolation(msg) => {
            (LogLevel::Warning, alloc::format!("{msg} [iface={iface:?}]"))
        }
        InboundVerdict::CryptoError(msg) => {
            (LogLevel::Warning, alloc::format!("{msg} [iface={iface:?}]"))
        }
    };
    if let Some(logger) = &rt.config.security_logger {
        // Participant ident: GuidPrefix (or 0-padding for Malformed).
        let mut participant = [0u8; 16];
        if bytes.len() >= 20 {
            participant[..12].copy_from_slice(&bytes[8..20]);
        }
        logger.log(level, participant, category, &message);
    }
    None
}

#[cfg(not(feature = "security"))]
fn secure_inbound_bytes<'a>(
    _rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    Some(alloc::borrow::Cow::Borrowed(bytes))
}

/// Default interface class for inbound dispatch when the socket does not
/// belong to the `outbound_pool`. In the v1.4 setup (without
/// `interface_bindings`), all packets run through `user_unicast` and are
/// classified as `Wan` — the most conservative assumption (protection
/// rules apply as in the single-interface case).
#[cfg(feature = "security")]
const DEFAULT_INBOUND_IFACE: NetInterface = NetInterface::Wan;

/// Per-reader outbound transform.
///
/// Looks up in the writer slot which `ProtectionLevel` the matched reader
/// expects at the given `target` locator, then pulls the datagram through
/// the security gate individually. This way each reader gets a wire
/// payload matching its security profile (Legacy=plain, Fast=Sign,
/// Secure=Encrypt).
///
/// Fallback paths:
/// * No security gate configured → passthrough.
/// * No `locator_to_peer` entry (reader not yet matched via SEDP) →
///   `transform_outbound` with the domain rule — that is the homogeneous
///   v1.4 path.
/// * The gate returns an error → `None` (the caller drops — better no
///   plaintext leak).
#[cfg(feature = "security")]
fn secure_outbound_for_target(
    rt: &DcpsRuntime,
    writer_eid: EntityId,
    bytes: &[u8],
    target: &Locator,
) -> Option<Vec<u8>> {
    let Some(gate) = &rt.config.security else {
        return Some(bytes.to_vec());
    };
    // FU2 S3: fallback level from our own governance (data_protection_
    // kind), in case the matched reader did not announce an explicit SEDP
    // security_info level. This way user data to an authenticated peer is
    // encrypted per our own governance, while SPDP/SEDP metatraffic
    // bootstraps plaintext over rtps_protection_kind=NONE.
    // Governance `data_protection` is a FLOOR, not a mere fallback: a
    // per-reader level can only STRENGTHEN (e.g. legacy plaintext is only
    // allowed if the domain policy itself permits plaintext), never fall
    // below the domain policy. Otherwise a matched-but-not-authenticated
    // peer (foreign CA, SEDP match over plaintext discovery,
    // reader_protection=None) leaks plaintext user data.
    let gov_data_level = gate.data_protection().unwrap_or(ProtectionLevel::None);
    // metadata_protection (§8.4.2.4 / §9.5.3.3): EVERY writer submessage (DATA,
    // HEARTBEAT, GAP) is SEC_PREFIX/BODY/POSTFIX-wrapped per-submessage —
    // TARGET-INDEPENDENT, since the per-endpoint writer key is local (the peer fetches
    // it via datawriter_crypto_token). Must take effect BEFORE the locator-based reader
    // resolution: otherwise tick HEARTBEATs/GAPs to not-yet-locator-
    // matched targets fall into the None branch -> with rtps=NONE PLAIN -> leak + no
    // reliable recovery (breaks already zero<->zero). data_protection (inner
    // payload layer) first, then the outer submessage layer.
    if gate.metadata_protection().unwrap_or(ProtectionLevel::None) != ProtectionLevel::None {
        let inner = if gov_data_level != ProtectionLevel::None {
            protect_user_payload(rt, bytes)?
        } else {
            bytes.to_vec()
        };
        let meta_sec = protect_user_datagram(rt, &inner)?;
        // Under rtps_protection message-level SRTPS MUST additionally go on top —
        // BOTH layers, like cyclone<->cyclone. Without it the peer would see the
        // metadata-SEC-DATA as "clear submsg from protected src" and discard it.
        if gate.rtps_protection().unwrap_or(ProtectionLevel::None) != ProtectionLevel::None {
            return gate.transform_outbound(&meta_sec).ok();
        }
        return Some(meta_sec);
    }
    let resolved = rt.writer_slot(writer_eid).and_then(|arc| {
        arc.lock().ok().and_then(|slot| {
            let pk = slot.locator_to_peer.get(target).copied()?;
            // An EXPLICITLY negotiated per-reader level is respected: a
            // legacy-v1.4 reader has reader_protection=None and MUST get plaintext,
            // otherwise it cannot decode (heterogeneous domain). Only
            // when NO entry exists (matched via plaintext discovery, but
            // no level negotiated -> potentially unauthenticated) does the
            // governance data_protection FLOOR apply as leak protection.
            // Governance data_protection is a FLOOR (§8.4.2.4, memory-documented):
            // a per-reader level can only STRENGTHEN, never fall below the domain
            // policy. A reader discovered via secure SEDP whose security_info parses
            // to `Some(None)` (no is_payload_protected bit detected, discovery=
            // ENCRYPT) would otherwise yield level=None -> Some(None) arm -> PLAINTEXT
            // leak, although the domain requires data_protection=ENCRYPT (disc-data-
            // enc: zerodds sent user DATA without the N-flag -> OpenDDS decode_serialized_
            // payload=0 -> no echo). `.max` enforces at least the governance FLOOR.
            // With gov=None legacy plaintext (reader_lv) stays allowed.
            let level = match slot.reader_protection.get(&pk).copied() {
                Some(reader_lv) => reader_lv.max(gov_data_level),
                None => gov_data_level,
            };
            Some((pk, level))
        })
    });
    match resolved {
        // Matched reader with Sign/Encrypt: cyclone-conformant SUBMESSAGE
        // protection (SEC_PREFIX/BODY/POSTFIX around the DATA submessage, local
        // data key) instead of message-level SRTPS — `metadata_protection_kind=
        // ENCRYPT`, §9.5.3.3. cyclone decodes with the key sent via datawriter_crypto_
        // tokens. `None` level = byte-identical passthrough.
        Some((peer_key, level)) if level != ProtectionLevel::None => {
            // Layer choice per governance (DDS-Security §8.4.2.4 vs §7.3.7):
            //  * metadata_protection_kind != NONE -> per-submessage protection
            //    (`encode_datawriter_submessage`, SEC_PREFIX/BODY/POSTFIX) for
            //    EVERY writer submessage (DATA, HEARTBEAT, GAP, ...). This is the
            //    cyclone interop path: cyclone expects HEARTBEAT/GAP SEC_*-
            //    wrapped too, otherwise its reader never NACKs (no reliable recovery).
            //  * otherwise (only rtps_protection_kind != NONE) -> message-level SRTPS
            //    via `transform_outbound_for` (whole message, §7.3.7).
            // INNER layer (§9.5.3.3.1): data_protection encrypts ONLY the
            // SerializedPayload of each DATA submessage. Applied BEFORE the outer
            // submessage/message layer — cyclone-conformant
            // nesting (§9.5.3.3): data_protection (inner) + metadata_
            // protection (outer). With pure data_protection this is the
            // only + complete protection.
            let inner: Vec<u8> = if gov_data_level != ProtectionLevel::None {
                // Crypto error -> drop instead of leak (None propagated via `?`).
                protect_user_payload(rt, bytes)?
            } else {
                bytes.to_vec()
            };
            // OUTER layer choice (DDS-Security §8.4.2.4 / §7.3.7):
            if gate.metadata_protection().unwrap_or(ProtectionLevel::None) != ProtectionLevel::None
            {
                // metadata_protection -> per-submessage protection (DATA, HEARTBEAT,
                // GAP, ...) with the per-endpoint writer key (cyclone interop path).
                // Under additional rtps_protection message-level SRTPS MUST go on top
                // (both layers) — otherwise "clear submsg from protected src".
                match protect_user_datagram(rt, &inner) {
                    Some(ms)
                        if gate.rtps_protection().unwrap_or(ProtectionLevel::None)
                            != ProtectionLevel::None =>
                    {
                        gate.transform_outbound(&ms).ok()
                    }
                    other => other,
                }
            } else if gate.rtps_protection().unwrap_or(ProtectionLevel::None)
                != ProtectionLevel::None
            {
                // rtps_protection -> message-level SRTPS (whole message, §7.3.7),
                // per-reader key.
                gate.transform_outbound_for(&peer_key, &inner, level).ok()
            } else {
                // only data_protection -> the payload layer is already the
                // complete protection (§9.5.3.3.1). Header/InlineQoS stay
                // plaintext, the encrypted payload carries the N-flag.
                Some(inner)
            }
        }
        // Matched reader with level None: a legacy-v1.4 reader (explicit
        // SEDP legacy or NONE governance) gets byte-identical plaintext —
        // message-level SRTPS would make it undecryptable.
        Some(_) => {
            // Matched reader with data level None: under rtps_protection the
            // message MUST still be message-level-SRTPS-wrapped (§8.4.2.4) —
            // the data_protection level only controls the payload/submessage layer.
            // Without it user DATA/HEARTBEAT leaks plain, although the domain
            // requires rtps_protection=ENCRYPT (the peer discards it as legacy).
            if gate.rtps_protection().unwrap_or(ProtectionLevel::None) != ProtectionLevel::None {
                gate.transform_outbound(bytes).ok()
            } else {
                Some(bytes.to_vec())
            }
        }
        // No locator-resolved reader: multicast/meta bootstrap OR a
        // user reader whose locator is (not yet) in `locator_to_peer`
        // (e.g. discovered via secure SEDP, discovery_protection=ENCRYPT). The
        // data_protection (inner payload layer §9.5.3.3.1) is TARGET-INDEPENDENT
        // (local writer key) and MUST still apply for a user writer —
        // otherwise under data_protection=ENCRYPT the user DATA leaks PLAINTEXT (N-flag
        // missing -> a spec-conformant remote reader never calls `decode_serialized_payload`
        // -> no sample, no echo; disc-data-enc stall, source-documented: OpenDDS
        // decode_serialized_payload=0). ONLY for user writers — SPDP/SEDP builtin DATA
        // must bootstrap plaintext (otherwise undecodable before key exchange).
        None => {
            use zerodds_rtps::wire_types::EntityKind;
            let is_user_writer = matches!(
                writer_eid.entity_kind,
                EntityKind::UserWriterWithKey | EntityKind::UserWriterNoKey
            );
            if is_user_writer && gov_data_level != ProtectionLevel::None {
                let inner = protect_user_payload(rt, bytes)?;
                gate.transform_outbound(&inner).ok()
            } else {
                gate.transform_outbound(bytes).ok()
            }
        }
    }
}

#[cfg(not(feature = "security"))]
fn secure_outbound_for_target(
    _rt: &DcpsRuntime,
    _writer_eid: EntityId,
    bytes: &[u8],
    _target: &Locator,
) -> Option<Vec<u8>> {
    Some(bytes.to_vec())
}

/// FU2 S3: data_protection-aware user DATA outbound. Encrypts the
/// datagram with the governance `data_protection` level. `transform_outbound_
/// for` ignores the `peer_key` and uses the local key — the ciphertext
/// is decryptable for EVERY authenticated peer (with our token),
/// non-authenticated peers cannot read it. A `None` level falls
/// back to message-level (`rtps_protection` resp. passthrough). Used for
/// UDP + in-process fastpath + SHM UNIFORMLY, so the
/// inproc path is secured too.
#[cfg(feature = "security")]
fn secure_user_outbound<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    let Some(gate) = &rt.config.security else {
        return Some(alloc::borrow::Cow::Borrowed(bytes));
    };
    let level = gate.data_protection().unwrap_or(ProtectionLevel::None);
    if matches!(level, ProtectionLevel::None) {
        gate.transform_outbound(bytes)
            .ok()
            .map(alloc::borrow::Cow::Owned)
    } else {
        gate.transform_outbound_for(&[0u8; 12], bytes, level)
            .ok()
            .map(alloc::borrow::Cow::Owned)
    }
}

#[cfg(not(feature = "security"))]
fn secure_user_outbound<'a>(
    _rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    Some(alloc::borrow::Cow::Borrowed(bytes))
}

/// Sends `bytes` to `target` on the matching interface socket.
/// Falls back to `rt.user_unicast` if no
/// pool is configured or no binding matches the target range
/// and no default binding is set either.
#[cfg(feature = "security")]
fn send_on_best_interface(rt: &DcpsRuntime, target: &Locator, bytes: &[u8]) {
    if let Some(pool) = &rt.outbound_pool {
        if let Some((socket, _iface)) = pool.route(target) {
            let _ = socket.send(target, bytes);
            return;
        }
    }
    let _ = rt.user_unicast.send(target, bytes);
}

#[cfg(not(feature = "security"))]
fn send_on_best_interface(rt: &DcpsRuntime, target: &Locator, bytes: &[u8]) {
    let _ = rt.user_unicast.send(target, bytes);
}

/// User-writer slot in the runtime. Carries ReliableWriter + topic meta +
/// fragment size (from QoS).
struct UserWriterSlot {
    writer: ReliableWriter,
    topic_name: String,
    type_name: String,
    reliable: bool,
    durability: zerodds_qos::DurabilityKind,
    /// Deadline period in nanoseconds (0 == INFINITE, no monitoring).
    deadline_nanos: u64,
    /// Last successful `write` relative to `DcpsRuntime::start_instant`.
    last_write: Option<Duration>,
    /// Counter for missed deadlines (Spec §2.2.4.2.9).
    offered_deadline_missed_count: u64,
    /// Counter for LivelinessLost detections from the writer's view
    /// (Spec §2.2.4.2.10). Incremented in `check_writer_liveliness` on
    /// manual-lease overrun. 0 == not monitored.
    liveliness_lost_count: u64,
    /// Last assert time (manual liveliness). `None` == never.
    last_liveliness_assert: Option<Duration>,
    /// Per-policy counter for offered_incompatible_qos. Spec
    /// §2.2.4.2.4.2 — writer side. Incremented on
    /// `wire_writer_to_remote_reader` reject.
    offered_incompatible_qos: crate::status::OfferedIncompatibleQosStatus,
    /// Lifespan duration in nanoseconds (0 == INFINITE, no expiry).
    lifespan_nanos: u64,
    /// Per sample SN the insert time (relative to start_instant).
    /// Removed from front on expiry — SNs are monotonic, lifespan
    /// is constant, so the expiry prefix is always front.
    sample_insert_times:
        alloc::collections::VecDeque<(zerodds_rtps::wire_types::SequenceNumber, Duration)>,
    /// Liveliness kind (Automatic / ManualByParticipant / ManualByTopic).
    liveliness_kind: zerodds_qos::LivelinessKind,
    /// Lease duration in nanoseconds (0 == INFINITE).
    liveliness_lease_nanos: u64,
    /// Ownership mode.
    ownership: zerodds_qos::OwnershipKind,
    /// Ownership strength (Spec §2.2.3.2). Mirrored in the same-runtime
    /// dispatch into `UserSample::Alive.writer_strength`, so that
    /// EXCLUSIVE ownership logic in the reader also works for intra-process
    /// loopback.
    ownership_strength: i32,
    /// Partition list.
    partition: Vec<String>,
    /// Per-matched-reader ProtectionLevel. Derived at the
    /// SEDP match from `sub.security_info`. `None` entries
    /// for legacy readers. Empty for writers without matched
    /// security peers — then the hot path is unchanged.
    #[cfg(feature = "security")]
    reader_protection: BTreeMap<[u8; 12], ProtectionLevel>,
    /// Mapping Locator → GuidPrefix for the writer tick loop, so that
    /// `secure_outbound_for_target` can look up the protection per target
    /// without breaking the writer-tick API (`dg.targets` are
    /// locator lists today).
    #[cfg(feature = "security")]
    locator_to_peer: BTreeMap<Locator, [u8; 12]>,
    /// F-TYPES-3 XTypes 1.3 §7.3.4.2 TypeIdentifier of the writer type
    /// (from `T::TYPE_IDENTIFIER` in `UserWriterConfig`).
    type_identifier: zerodds_types::TypeIdentifier,
    /// D.5g — per-writer override for the DataRepresentation offer.
    /// `None` = runtime default. `Some(vec)` = hardcoded per writer.
    data_rep_offer_override: Option<Vec<i16>>,
    /// Type extensibility of the writer type (FINAL/APPENDABLE/MUTABLE).
    /// Together with the offer `first` element it determines the
    /// encapsulation header of the user payload (see
    /// [`user_payload_encap`]). Default `Final`; set by codegen/FFI via
    /// `set_user_writer_wire_extensibility` when the type
    /// is appendable/mutable (relevant for XCDR2 wire: D_CDR2/PL_CDR2).
    wire_extensibility: zerodds_types::qos::ExtensibilityForRepr,
    /// Emit the big-endian encapsulation variant (`_BE`, RTPS 2.5 §10.5)
    /// instead of the little-endian default. Set by the durability service
    /// replay path so a big-endian peer's stored sample is re-published with a
    /// matching BE encap header (the body bytes are already big-endian). `false`
    /// = little-endian (the canonical wire for a normal writer).
    big_endian_override: bool,
    /// Spec §2.2.3.5 DurabilityService — with Durability=Transient/
    /// Persistent the backend holds in addition to the writer's own
    /// HistoryCache. On the first late-joiner match in
    /// `wire_writer_to_remote_reader` the backend samples are
    /// (re-)injected into the HistoryCache, so that the RTPS reliable
    /// path delivers them to the reader. `None` for Volatile/
    /// TransientLocal (the cache suffices).
    durability_backend: Option<alloc::sync::Arc<dyn crate::durability_service::DurabilityBackend>>,
    /// `true` as soon as the backend has been replayed once into the
    /// HistoryCache. Prevents repeated re-injection on further matches.
    backend_primed: bool,
    /// HISTORY KeepLast depth (DDS 1.4 §2.2.3.18). Per-instance retained-sample
    /// depth for the same-runtime durability replay path. Default
    /// [`DEFAULT_INTRA_HISTORY_DEPTH`]. KeepAll is modelled as `usize::MAX`.
    /// Settable via [`DcpsRuntime::set_user_writer_history_depth`].
    history_depth: usize,
    /// TRANSIENT_LOCAL retained samples (DDS 1.4 §2.2.3.4) for the
    /// same-runtime late-joiner replay path. Holds the *most recent*
    /// `history_depth` Alive samples **per instance key** plus any
    /// terminal lifecycle marker for an instance. A reader that joins an
    /// intra-runtime route AFTER these writes replays this buffer so it sees
    /// the retained history (the wire/SEDP path is separate, see
    /// `wire_writer_to_remote_reader`). Empty unless durability is
    /// TransientLocal or stronger.
    retained: alloc::collections::VecDeque<RetainedSample>,
    /// Set of intra-runtime reader EntityIds that have already received the
    /// TransientLocal retained-sample replay, so a route recompute does not
    /// replay the same history twice to the same reader.
    intra_replayed_readers: alloc::collections::BTreeSet<EntityId>,
}

/// Default same-runtime HISTORY KeepLast depth when the user has not called
/// [`DcpsRuntime::set_user_writer_history_depth`]. Mirrors the DDS spec default
/// of `depth = 1` for KEEP_LAST (DDS 1.4 §2.2.3.18 Table).
const DEFAULT_INTRA_HISTORY_DEPTH: usize = 1;

/// One retained sample for the same-runtime TransientLocal replay path.
#[derive(Debug, Clone)]
struct RetainedSample {
    /// Instance key hash (16 byte). All-zero for NoKey topics / unknown key.
    key_hash: [u8; 16],
    /// CDR body without encapsulation header.
    payload: Vec<u8>,
    /// XCDR version tag (`0` = XCDR1, `1` = XCDR2).
    representation: u8,
    /// Writer ownership strength at write time.
    strength: i32,
    /// `Some(kind)` if this entry is a terminal lifecycle marker
    /// (dispose / unregister) rather than an alive sample.
    lifecycle: Option<zerodds_rtps::history_cache::ChangeKind>,
}

/// The listener dispatch carries, alongside the `UserSample`, a
/// zero-copy view on the original `Arc<[u8]>` with an encap offset
/// (lever-E zero-copy path).
pub type UserSampleWithEncap = (UserSample, Option<(Arc<[u8]>, usize)>);

/// Sample channel item: either data payload or lifecycle marker.
/// Lifecycle is reconstructed by the wire path as `key_hash + ChangeKind` from
/// the PID_STATUS_INFO header; the DataReader DCPS layer
/// translates that into `__push_lifecycle`.
#[derive(Debug, Clone)]
pub enum UserSample {
    /// Normal sample with payload (CDR-encoded application type).
    /// `writer_guid` is the 16-byte GUID of the emitting writer
    /// — needed by the subscriber for exclusive-ownership resolution
    /// (DDS 1.4 §2.2.3.23 / §2.2.2.5.5).
    Alive {
        /// CDR payload (without encapsulation header). Zero-copy container:
        /// typically holds an `Arc<[u8]>` slice into the RTPS wire datagram
        /// without a heap re-alloc. See `docs/specs/zerodds-zero-copy-1.0.md`.
        payload: crate::sample_bytes::SampleBytes,
        /// Writer GUID — for strongest-writer selection.
        writer_guid: [u8; 16],
        /// Writer `ownership_strength` at the time of receipt.
        /// `0` if the writer is not yet known via discovery
        /// (the reader treats this as default strength = spec-conformant
        /// for shared-ownership topics; for exclusive the
        /// reader filters the real strength against the current owner).
        writer_strength: i32,
        /// XCDR version of the `payload` — extracted from the encapsulation
        /// header of the wire sample (RTPS 2.5 §10.5) BEFORE the
        /// header was stripped: `0` = XCDR1 (CDR/PL_CDR), `1` =
        /// XCDR2 (CDR2/D_CDR2/PL_CDR2). The typed consumer
        /// needs this to decode the body with the correct alignment rule
        /// (XTypes 1.3 §7.4.3.4.2).
        representation: u8,
        /// Byte order of the `payload` — extracted from the encapsulation
        /// representation identifier's low bit (RTPS 2.5 §10.5: the `_BE`
        /// variants 0x0000/0x0002/0x0006/0x0008/0x000a are even, the `_LE`
        /// variants odd). `false` = little-endian (the canonical wire and the
        /// intra-runtime default), `true` = big-endian. The typed consumer
        /// dispatches `DdsType::decode` vs `decode_be` on this.
        big_endian: bool,
        /// Source timestamp from the writer's INFO_TS submessage (DDSI-RTPS
        /// §8.7.3), if any. Threaded into `SampleInfo.source_timestamp` and the
        /// `DESTINATION_ORDER = BY_SOURCE_TIMESTAMP` decision. `None` ⇒ the
        /// reader uses reception order.
        source_timestamp: Option<zerodds_rtps::header_extension::HeTimestamp>,
    },
    /// Lifecycle marker (dispose / unregister) — the reader sets
    /// InstanceState accordingly.
    Lifecycle {
        /// Key hash of the affected instance (16 byte).
        key_hash: [u8; 16],
        /// `NotAliveDisposed` / `NotAliveUnregistered` /
        /// `NotAliveDisposedUnregistered`.
        kind: zerodds_rtps::history_cache::ChangeKind,
    },
}

/// User-reader slot. ReliableReader + topic meta + channel to the
/// DataReader (DCPS API side).
/// Listener callback for sample arrival.
///
/// Fired synchronously by `recv_user_data_loop` in the recv-thread
/// context as soon as an alive sample lands in the reader HistoryCache.
/// Eliminates the polling latency of `zerodds_reader_take()` —
/// the listener path typically saves 50-100 µs per side.
///
/// **Contract** (analogous to DDS spec §2.2.4.4 listener semantics):
/// * The callback runs on the recv thread, NOT the user thread.
/// * Short and non-blocking. No I/O, no locks, no
///   ZeroDDS API calls inside.
/// * `bytes` points to the CDR payload of the alive sample (without
///   encapsulation header). Lifetime only for the duration of the
///   callback; copy if needed beyond the call.
/// * Disposed/unregistered lifecycle events do NOT fire the listener
///   (only `Alive` samples) — for lifecycle tracking
///   keep using `zerodds_reader_take()` or add a
///   lifecycle-listener API.
///
/// Data-available listener. Arguments: CDR body (without encapsulation
/// header) and the XCDR version of the sample (`0` = XCDR1, `1` = XCDR2)
/// — the typed consumer needs the latter for the alignment
/// rule on decode (XTypes 1.3 §7.4.3.4.2).
pub type UserReaderListener = alloc::boxed::Box<dyn Fn(&[u8], u8, u8) + Send + Sync + 'static>;

struct UserReaderSlot {
    reader: ReliableReader,
    topic_name: String,
    type_name: String,
    sample_tx: mpsc::Sender<UserSample>,
    /// Spec §3 zerodds-async-1.0: async waker slot. Registered by the
    /// async reader; on `sample_tx.send` we call
    /// `waker.wake()`. `None` if no async reader is active.
    async_waker: alloc::sync::Arc<std::sync::Mutex<Option<core::task::Waker>>>,
    /// Listener callback for alive samples.
    /// Fired synchronously by `recv_user_data_loop`. `None` =
    /// no listener registered (the user polls via
    /// `zerodds_reader_take()`). Arc, so the recv thread can
    /// execute the callback cloned without another lock (minimize lock
    /// hold time).
    listener: Option<alloc::sync::Arc<UserReaderListener>>,
    durability: zerodds_qos::DurabilityKind,
    /// Deadline period in nanoseconds (0 == INFINITE).
    deadline_nanos: u64,
    /// Time of the last received sample relative to runtime start.
    last_sample_received: Option<Duration>,
    /// Counter for missed deadline expectations (Spec §2.2.4.2.11).
    requested_deadline_missed_count: u64,
    /// Per-policy counter for requested_incompatible_qos. Spec
    /// §2.2.4.2.6.5 — reader side. Incremented on
    /// `wire_reader_to_remote_writer` reject.
    requested_incompatible_qos: crate::status::RequestedIncompatibleQosStatus,
    /// Sample-lost counter (Spec §2.2.4.2.6.2). Incremented
    /// by `record_sample_lost`.
    sample_lost_count: u64,
    /// Sample-rejected counter (Spec §2.2.4.2.6.3). Incremented
    /// by `record_sample_rejected`.
    sample_rejected: crate::status::SampleRejectedStatus,
    /// Monotonically increasing count of alive samples delivered to the
    /// user. Serves as a non-consuming data-availability detector for
    /// `on_data_available` (DDS 1.4 §2.2.4.2.6.1) — unlike
    /// `last_sample_received`, this counter is only bumped on real sample
    /// delivery, never by the deadline path. Read via
    /// [`DcpsRuntime::user_reader_samples_delivered`].
    samples_delivered_count: u64,
    /// Reader-side requested liveliness lease (0 == INFINITE).
    liveliness_lease_nanos: u64,
    /// Reader-side requested liveliness kind.
    liveliness_kind: zerodds_qos::LivelinessKind,
    /// Counter: how often the writer was marked "alive"
    /// (Spec §2.2.4.2.14 alive_count).
    liveliness_alive_count: u64,
    /// Counter: how often it was marked "not_alive" (lease expired).
    liveliness_not_alive_count: u64,
    /// Current "alive/not-alive" state from the reader's view.
    liveliness_alive: bool,
    /// QR-cluster (e): set of writer GUIDs the reader currently considers alive
    /// via an AUTOMATIC-liveliness same-runtime match. Used to bump
    /// `liveliness_alive_count` exactly once per writer-alive transition on the
    /// intra-runtime path (the wire DATA path tracks this via reader proxies).
    liveliness_alive_writers: alloc::collections::BTreeSet<[u8; 16]>,
    /// Ownership.
    ownership: zerodds_qos::OwnershipKind,
    /// Partition.
    partition: Vec<String>,
    /// Per-writer strength cache for exclusive-ownership resolution
    /// (DDS 1.4 §2.2.3.23). Filled by `wire_reader_to_remote_writer`
    /// from each `PublicationBuiltinTopicData.ownership_strength`;
    /// `delivered_to_user_sample` looks it up here to pack the
    /// strength into `UserSample::Alive`.
    writer_strengths: alloc::collections::BTreeMap<[u8; 16], i32>,
    /// F-TYPES-3 XTypes 1.3 §7.3.4.2 TypeIdentifier of the reader type
    /// (from `T::TYPE_IDENTIFIER` in `UserReaderConfig`). Default
    /// `TypeIdentifier::None` signals "no TypeIdentifier" —
    /// the match falls back to a pure `type_name` comparison
    /// (DDS 1.4 §2.2.3 default path).
    type_identifier: zerodds_types::TypeIdentifier,
    /// XTypes 1.3 §7.6.3.7 — TCE policy controlling the strictness
    /// of the XTypes match path.
    type_consistency: zerodds_types::qos::TypeConsistencyEnforcement,
    /// A2 — TIME_BASED_FILTER `minimum_separation` (DDS 1.4 §2.2.3.12), in
    /// nanoseconds, for the runtime/C-FFI delivery path. `0` (default) = off.
    /// Set via [`DcpsRuntime::set_user_reader_time_based_filter`] (the
    /// `rmw_zerodds` / C-FFI path; the typed entity reader enforces TBF on its
    /// own QoS). See [`UserReaderSlot::tbf_should_deliver`].
    tbf_min_separation_nanos: u128,
    /// Per-instance last-delivered timestamp (nanoseconds since runtime start),
    /// keyed by the sample KeyHash (keyless types share the all-zero key). Only
    /// populated when `tbf_min_separation_nanos > 0`.
    tbf_last_delivered: alloc::collections::BTreeMap<[u8; 16], u128>,
}

impl UserReaderSlot {
    /// A2 — TIME_BASED_FILTER gate (DDS 1.4 §2.2.3.12) for the runtime delivery
    /// path: returns `true` if a sample of the given instance may be delivered,
    /// i.e. at least `minimum_separation` has elapsed since the last delivered
    /// sample of that instance. The first sample of an instance always passes.
    /// `key_hash = None` (keyless type) collapses to a single instance. A
    /// `minimum_separation` of 0 disables the filter (always `true`).
    fn tbf_should_deliver(&mut self, key_hash: Option<[u8; 16]>, now_nanos: u128) -> bool {
        if self.tbf_min_separation_nanos == 0 {
            return true;
        }
        let inst = key_hash.unwrap_or([0u8; 16]);
        match self.tbf_last_delivered.get(&inst) {
            Some(&last) if now_nanos.saturating_sub(last) < self.tbf_min_separation_nanos => false,
            _ => {
                self.tbf_last_delivered.insert(inst, now_nanos);
                true
            }
        }
    }
}

/// Helper struct for announcing a local publication/subscription
/// as SEDP BuiltinTopicData. The caller creates it once per
/// writer/reader registration and passes it to SedpStack.
/// QoS config for registering a user writer with the runtime.
/// Bundles all policies that go on the wire via SEDP plus the local
/// Per-endpoint discovery info for ROS 2 endpoint-info-by-topic introspection
/// (`rmw_get_publishers_info_by_topic` / `rmw_get_subscriptions_info_by_topic`,
/// the data behind `ros2 topic info -v`). Covers local user endpoints plus
/// remote SEDP-discovered ones. QoS is best-effort from what discovery carries
/// (history/depth are not on the wire, so the consumer fills rmw defaults).
#[derive(Debug, Clone)]
pub struct DiscoveredEndpointInfo {
    /// DDS topic name (raw, un-demangled).
    pub topic_name: String,
    /// IDL type name (raw).
    pub type_name: String,
    /// 16-byte endpoint GUID: 12-byte participant prefix + 4-byte entity id.
    /// Bytes 0..12 identify the owning participant (for node-name lookup).
    pub endpoint_guid: [u8; 16],
    /// RELIABLE (`true`) vs BEST_EFFORT (`false`).
    pub reliable: bool,
    /// TRANSIENT_LOCAL or stronger (`true`) vs VOLATILE (`false`).
    pub transient_local: bool,
    /// Deadline period in whole seconds (0 == INFINITE).
    pub deadline_seconds: i32,
    /// Lifespan in whole seconds (0 == INFINITE; always 0 for subscriptions).
    pub lifespan_seconds: i32,
    /// Liveliness lease in whole seconds (0 == INFINITE).
    pub liveliness_lease_seconds: i32,
}

/// Packs an RTPS [`Guid`] into the 16-byte wire form (prefix ++ entity id).
fn guid_to_16(g: Guid) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..12].copy_from_slice(&g.prefix.to_bytes());
    b[12..].copy_from_slice(&g.entity_id.to_bytes());
    b
}

/// monitoring. Avoids 10+-argument functions.
#[derive(Debug, Clone)]
pub struct UserWriterConfig {
    /// Topic name (DDS topic).
    pub topic_name: String,
    /// IDL type name.
    pub type_name: String,
    /// `true` = RELIABLE, `false` = BEST_EFFORT.
    pub reliable: bool,
    /// Durability.
    pub durability: zerodds_qos::DurabilityKind,
    /// Deadline period (offered).
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// Lifespan duration (writer-only).
    pub lifespan: zerodds_qos::LifespanQosPolicy,
    /// Liveliness (offered).
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// Ownership mode (Shared / Exclusive).
    pub ownership: zerodds_qos::OwnershipKind,
    /// Strength for Exclusive (ignored for Shared).
    pub ownership_strength: i32,
    /// Partition list. Empty == default partition (`""`).
    pub partition: Vec<String>,
    /// UserData QoS (Spec §2.2.3.1) — opaque `sequence<octet>`, propagated
    /// via discovery.
    pub user_data: Vec<u8>,
    /// TopicData QoS (Spec §2.2.3.3).
    pub topic_data: Vec<u8>,
    /// GroupData QoS (Spec §2.2.3.2).
    pub group_data: Vec<u8>,
    /// XTypes 1.3 §7.3.4.2 TypeIdentifier (F-TYPES-3 wire-up). Default
    /// `TypeIdentifier::None` for the `T::TYPE_IDENTIFIER` default.
    pub type_identifier: zerodds_types::TypeIdentifier,

    /// D.5g — per-writer override of the DataRepresentation offer list.
    /// `None` = use `RuntimeConfig::data_representation_offer`.
    /// `Some(vec)` = overridden per writer (e.g. `[XCDR2]` for
    /// a modern-only pub).
    pub data_representation_offer: Option<Vec<i16>>,
}

/// QoS config for registering a user reader.
#[derive(Debug, Clone)]
pub struct UserReaderConfig {
    /// Topic name.
    pub topic_name: String,
    /// IDL type name.
    pub type_name: String,
    /// `true` = RELIABLE, `false` = BEST_EFFORT.
    pub reliable: bool,
    /// Durability (requested).
    pub durability: zerodds_qos::DurabilityKind,
    /// Deadline (requested).
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// Liveliness (requested).
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// Ownership.
    pub ownership: zerodds_qos::OwnershipKind,
    /// Partition.
    pub partition: Vec<String>,
    /// UserData QoS (Spec §2.2.3.1).
    pub user_data: Vec<u8>,
    /// TopicData QoS (Spec §2.2.3.3).
    pub topic_data: Vec<u8>,
    /// GroupData QoS (Spec §2.2.3.2).
    pub group_data: Vec<u8>,
    /// XTypes 1.3 §7.3.4.2 TypeIdentifier (F-TYPES-3 wire-up).
    pub type_identifier: zerodds_types::TypeIdentifier,
    /// TypeConsistencyEnforcement (XTypes §7.6.3.7) — controls how strictly
    /// the reader match checks XTypes compatibility.
    pub type_consistency: zerodds_types::qos::TypeConsistencyEnforcement,

    /// D.5g — per-reader override of the DataRepresentation accept list.
    /// `None` = use `RuntimeConfig::data_representation_offer`.
    /// `Some(vec)` = overridden per reader (e.g. `[XCDR1]` for
    /// a reader that accepts only legacy XCDR1 wire).
    pub data_representation_offer: Option<Vec<i16>>,
}

fn build_publication_data(
    owner_prefix: GuidPrefix,
    writer_eid: EntityId,
    cfg: &UserWriterConfig,
    runtime_offer: &[i16],
    user_locator: Locator,
) -> zerodds_rtps::publication_data::PublicationBuiltinTopicData {
    use zerodds_qos::{ReliabilityKind, ReliabilityQosPolicy};
    zerodds_rtps::publication_data::PublicationBuiltinTopicData {
        key: Guid::new(owner_prefix, writer_eid),
        participant_key: Guid::new(owner_prefix, EntityId::PARTICIPANT),
        topic_name: cfg.topic_name.clone(),
        type_name: cfg.type_name.clone(),
        durability: cfg.durability,
        reliability: ReliabilityQosPolicy {
            kind: if cfg.reliable {
                ReliabilityKind::Reliable
            } else {
                ReliabilityKind::BestEffort
            },
            max_blocking_time: QosDuration::from_millis(100_i32),
        },
        ownership: cfg.ownership,
        ownership_strength: cfg.ownership_strength,
        liveliness: cfg.liveliness,
        deadline: cfg.deadline,
        lifespan: cfg.lifespan,
        partition: cfg.partition.clone(),
        user_data: cfg.user_data.clone(),
        topic_data: cfg.topic_data.clone(),
        group_data: cfg.group_data.clone(),
        type_information: None,
        // D.5g — PID_DATA_REPRESENTATION (XTypes 1.3 §7.6.3.1.1, RTPS 2.5
        // PID 0x0073). Per-Writer-Override (cfg.data_representation_offer)
        // overrides the RuntimeConfig default.
        data_representation: cfg
            .data_representation_offer
            .clone()
            .unwrap_or_else(|| runtime_offer.to_vec()),
        // Security: the PolicyEngine fills this later. Default
        // None = legacy behavior (no EndpointSecurityInfo PID).
        security_info: None,
        // .B — RPC discovery PIDs. Default None: no RPC endpoint;
        // the RpcEndpoint builder fills these fields.
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        // F-TYPES-3 Wire-up: XTypes-1.3 §7.3.4.2 TypeIdentifier.
        type_identifier: cfg.type_identifier.clone(),
        // DDSI-RTPS 2.5 §8.5.3.3: endpoint locator. All user endpoints
        // share the one `user_unicast` socket — hence the
        // endpoint locator equals the resolved participant locator.
        unicast_locators: alloc::vec![user_locator],
        multicast_locators: Vec::new(),
    }
}

/// The `DataRepresentation` set a **DataReader** announces (PID_DATA_REPRESENTATION
/// in its SEDP subscription). Per OMG XTypes 1.3 §7.6.2, a reader with the
/// default (empty) policy accepts **both** XCDR1 and XCDR2 — and ZeroDDS decodes
/// both (the read path dispatches on the per-sample encapsulation id). So the
/// reader advertises every representation it can decode, not just the writer's
/// preferred one.
///
/// This matters cross-vendor: CycloneDDS (and legacy RTI / OpenDDS < 3.16)
/// default their *writers* to **XCDR1** for `@final` types (non-XTypes backward
/// compat). A reader that only announces XCDR2 makes those writers fail the
/// `DataRepresentation` RxO check — the writer never forms a connection, and the
/// samples are dropped before any are sent. We therefore start from the
/// configured offer (which fixes the *preferred* order) and ensure both XCDR2
/// and XCDR1 are present. A WRITER keeps the narrow offer (`build_publication_data`),
/// because the generated encoder emits one representation.
fn reader_accept_repr(configured_offer: &[i16]) -> Vec<i16> {
    use zerodds_rtps::publication_data::data_representation as dr;
    let mut out: Vec<i16> = configured_offer.to_vec();
    for id in [dr::XCDR2, dr::XCDR] {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

fn build_subscription_data(
    owner_prefix: GuidPrefix,
    reader_eid: EntityId,
    cfg: &UserReaderConfig,
    runtime_offer: &[i16],
    user_locator: Locator,
) -> zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData {
    use zerodds_qos::{ReliabilityKind, ReliabilityQosPolicy};
    zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData {
        key: Guid::new(owner_prefix, reader_eid),
        participant_key: Guid::new(owner_prefix, EntityId::PARTICIPANT),
        topic_name: cfg.topic_name.clone(),
        type_name: cfg.type_name.clone(),
        durability: cfg.durability,
        reliability: ReliabilityQosPolicy {
            kind: if cfg.reliable {
                ReliabilityKind::Reliable
            } else {
                ReliabilityKind::BestEffort
            },
            max_blocking_time: QosDuration::from_millis(100_i32),
        },
        ownership: cfg.ownership,
        liveliness: cfg.liveliness,
        deadline: cfg.deadline,
        partition: cfg.partition.clone(),
        user_data: cfg.user_data.clone(),
        topic_data: cfg.topic_data.clone(),
        group_data: cfg.group_data.clone(),
        type_information: None,
        // D.5g — PID_DATA_REPRESENTATION (see build_publication_data).
        // A per-reader override overrides the RuntimeConfig default.
        data_representation: cfg
            .data_representation_offer
            .clone()
            .unwrap_or_else(|| runtime_offer.to_vec()),
        content_filter: None,
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        // F-TYPES-3 Wire-up: XTypes-1.3 §7.3.4.2 TypeIdentifier.
        type_identifier: cfg.type_identifier.clone(),
        // DDSI-RTPS 2.5 §8.5.3.2: endpoint locator (see
        // build_publication_data).
        unicast_locators: alloc::vec![user_locator],
        multicast_locators: Vec::new(),
    }
}

/// The runtime of a `DomainParticipant`. Hosts all background
/// threads and UDP sockets.
pub struct DcpsRuntime {
    /// Participant GUID prefix (12-byte identifier, random per instance).
    pub guid_prefix: GuidPrefix,
    /// Domain id.
    pub domain_id: i32,
    /// SPDP multicast receiver socket.
    pub spdp_multicast_rx: Arc<UdpTransport>,
    /// SPDP unicast socket (for bidirectional SPDP, B2).
    pub spdp_unicast: Arc<UdpTransport>,
    /// User-data unicast transport (default user unicast, where peers
    /// send matched samples). Trait object: can be UDP/v4 or /v6,
    /// and in phase C additionally TCP or SHM (env var
    /// `ZERODDS_USER_TRANSPORT`). Discovery (SPDP/SEDP) stays UDP-only.
    pub user_unicast: Arc<dyn Transport + Send + Sync>,
    /// Resolved user-unicast locator (routable interface address,
    /// not `0.0.0.0`). Written as `PID_UNICAST_LOCATOR` into EVERY
    /// SEDP pub/sub announce (DDSI-RTPS 2.5 §8.5.3.2/3)
    /// and as the participant `DEFAULT_UNICAST_LOCATOR` in SPDP. Precomputed
    /// via `announce_locator`, so the endpoint and participant locators
    /// are guaranteed identical.
    pub user_announce_locator: Locator,
    /// Sender socket for the SPDP multicast announce (separate UdpSocket
    /// without SO_REUSE/SO_BIND_IP_MULTICAST, so send_to routes cleanly).
    spdp_mc_tx: Arc<UdpTransport>,
    /// SPDP beacon (sends periodic announces).
    spdp_beacon: Mutex<SpdpBeacon>,
    /// Own participant data (SPDP self-view). Handed by the in-process
    /// discovery fastpath as a `DiscoveredParticipant` to same-process
    /// peers (see [`crate::inproc`]).
    participant_data: ParticipantBuiltinTopicData,
    /// Stash of all locally announced publications/subscriptions —
    /// so a peer runtime starting later in the same process
    /// can pull our endpoints via `inproc_snapshot`
    /// (pull-on-creation of the in-process discovery fastpath).
    /// Append-only; a future patch for endpoint deletion would
    /// remove by GUID here.
    announced_pubs: Mutex<Vec<zerodds_rtps::publication_data::PublicationBuiltinTopicData>>,
    announced_subs: Mutex<Vec<zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData>>,
    /// SPDP reader (parses incoming beacons).
    spdp_reader: SpdpReader,
    /// Discovered remote participants (prefix → data).
    discovered: Arc<Mutex<DiscoveredParticipantsCache>>,
    /// A1 discovery-server relay: cache of the last raw SPDP datagram per
    /// discovered participant prefix. Only populated in `discovery_server` mode;
    /// used to forward a newly-joined client the SPDP of every already-known
    /// client (and vice versa). Empty otherwise.
    spdp_relay_cache: Mutex<alloc::collections::BTreeMap<GuidPrefix, Vec<u8>>>,
    /// SEDP stack for publication/subscription announce + discovery.
    pub sedp: Arc<Mutex<SedpStack>>,
    /// TypeLookup-Service Builtin-Endpoint-GUIDs (XTypes 1.3 §7.6.3.3.4).
    pub type_lookup_endpoints: TypeLookupEndpoints,
    /// TypeLookup server (server-side handler over the local
    /// TypeRegistry).
    pub type_lookup_server: Arc<Mutex<TypeLookupServer>>,
    /// TypeLookup client (client-side correlation table for outstanding
    /// requests).
    pub type_lookup_client: Arc<Mutex<TypeLookupClient>>,
    /// Monotonically increasing sequence number of the TL_SVC_REPLY_WRITER. Reply DATA
    /// carry their OWN writer_sn (instead of echoing the request SN) — the
    /// correlation runs via PID_RELATED_SAMPLE_IDENTITY (DDS-RPC §7.8.2),
    /// so a reliable cross-vendor reply reader sees no SN jumps.
    tl_reply_sn: core::sync::atomic::AtomicU64,
    /// Security builtin endpoint stack
    /// (`DCPSParticipantStatelessMessage` + `DCPSParticipantVolatile-
    /// MessageSecure`). `None` as long as no security plugin is active
    /// — the hot path then skips any security-builtin
    /// demux. `Some` is set via [`DcpsRuntime::enable_security_builtins`]
    /// as soon as the factory has registered a plugin.
    pub security_builtin: Mutex<Option<Arc<Mutex<SecurityBuiltinStack>>>>,
    /// Monotonic "start time" — for SEDP tick clocks.
    start_instant: Instant,
    /// Local user-writer registry (EntityId → writer state).
    user_writers: Arc<RwLock<BTreeMap<EntityId, Arc<Mutex<UserWriterSlot>>>>>,
    /// ADR-0006 side map: per user writer an optional ShmLocator bytes
    /// value (PID_SHM_LOCATOR in the SEDP sample). `None` = no
    /// same-host backend attached. The wire encoder consults
    /// this map on the SEDP push.
    shm_locators: Arc<RwLock<BTreeMap<EntityId, Vec<u8>>>>,
    /// Wave 4 (Spec `zerodds-zero-copy-1.0` §6): tracker for
    /// same-host (writer, reader) pairs. The SEDP match hook registers
    /// here every pair whose remote prefix carries the same host-id prefix
    /// as the local participant. The hot-path send consults
    /// the tracker and routes over SHM instead of UDP in the `Bound` state.
    pub same_host: Arc<crate::same_host::SameHostTracker>,
    /// Local user-reader registry (EntityId → reader state).
    user_readers: Arc<RwLock<BTreeMap<EntityId, Arc<Mutex<UserReaderSlot>>>>>,
    /// Cross-vendor step 6b: peers to whom we have already sent per-endpoint
    /// crypto tokens (datawriter/datareader). Prevents spam on the
    /// repeated receipt of cyclone's tokens; sending happens only once the
    /// user endpoints exist (the bench creates them after handshake start).
    #[cfg(feature = "security")]
    /// Already-sent per-endpoint crypto tokens, per dedup key
    /// (source_endpoint ++ destination_endpoint, see `endpoint_token_key`).
    /// Per-token instead of per-peer, so late-matched user endpoints still
    /// get their tokens (#29).
    endpoint_tokens_sent: Arc<RwLock<alloc::collections::BTreeSet<[u8; 32]>>>,
    /// Peers (prefix) to whom our SEDP endpoint records have already been
    /// re-announced after a completed crypto-token exchange. Under rtps_/discovery_
    /// protection the initial SEDP burst is discarded by the peer (no key), until
    /// the participant crypto token arrives via Volatile; a one-time
    /// re-announce from that moment (the peer can now decode) brings the
    /// dropped SEDP up (OpenDDS flow; cyclone/FastDDS converge anyway).
    #[cfg(feature = "security")]
    sedp_reannounced: Arc<RwLock<alloc::collections::BTreeSet<[u8; 12]>>>,
    /// Per-endpoint crypto (DDS-Security §9.5.3.3): per local writer/reader
    /// EntityId its own crypto slot handle (its own key material, not the
    /// participant key). Used for the per-endpoint token (prepare_endpoint_
    /// crypto_tokens) AND the per-endpoint encode (protect_user_datagram)
    /// — the same key on both sides. Get-or-register lazily via
    /// `local_endpoint_crypto_handle`.
    #[cfg(feature = "security")]
    endpoint_crypto:
        Arc<RwLock<alloc::collections::BTreeMap<EntityId, zerodds_security::crypto::CryptoHandle>>>,
    /// Same-runtime writer→reader routes: per local writer the list
    /// of local readers subscribed to the same topic+type.
    /// Rebuilt in `recompute_intra_runtime_routes` on every
    /// register/unregister. Looked up in the write hot path,
    /// to push samples directly into the reader slot's `sample_tx`
    /// (intra-process loopback without an RTPS roundtrip, in parallel to the
    /// inproc peer path that only serves cross-runtime peers).
    intra_runtime_routes: Arc<RwLock<BTreeMap<EntityId, Vec<EntityId>>>>,
    /// Entity key counter (3 byte, incrementing). User writers use
    /// `0xC2` (with-key, user), user readers `0xC7`.
    entity_counter: AtomicU32,
    /// Configuration (cloned from RuntimeConfig).
    pub config: RuntimeConfig,
    /// Per-interface outbound socket pool. `None`
    /// when `config.interface_bindings` is empty — then
    /// `user_unicast` stays the only outbound socket (v1.4 path).
    #[cfg(feature = "security")]
    outbound_pool: Option<Arc<OutboundSocketPool>>,
    /// Writer-Liveliness-Protocol endpoint (RTPS 2.5 §8.4.13).
    /// Sends periodic `ParticipantMessageData` heartbeats and
    /// tracks last-seen per remote participant.
    pub wlp: Arc<Mutex<crate::wlp::WlpEndpoint>>,
    /// Builtin-topic reader sinks (DDS 1.4 §2.2.5). Set by the
    /// `DomainParticipant` constructor via `attach_builtin_sinks`;
    /// before that this is `None` and the discovery hot path
    /// drops samples silently (e.g. when the runtime is
    /// started directly for internal tests, without a participant).
    builtin_sinks: Mutex<Option<crate::builtin_subscriber::BuiltinSinks>>,
    /// Ignore filter (DDS 1.4 §2.2.2.2.1.14-17). Set by the
    /// `DomainParticipant` constructor via `attach_ignore_filter`.
    /// `None` means: no participant hook → no
    /// filtering.
    ignore_filter: Mutex<Option<crate::participant::IgnoreFilter>>,
    /// Stop flag for all worker threads (recv loops + tick loop).
    stop: Arc<AtomicBool>,
    /// Monotonic count of completed tick iterations. Incremented once per
    /// [`run_tick_iteration`], regardless of whether the tick is driven by the
    /// internal `zdds-tick` thread or an external executor (zerodds-async-1.0
    /// §4 `spawn_in_tokio`). Diagnostic: a stalled count means the tick loop
    /// stopped advancing. Read via [`DcpsRuntime::tick_count`].
    tick_seq: AtomicU64,
    /// Total SPDP announces emitted (multicast + unicast fan-out count as one).
    /// Diagnostic for the C3 initial-announcement burst — a fresh, peer-less
    /// participant should advance this fast initially. Read via
    /// [`DcpsRuntime::spdp_announce_count`].
    spdp_announce_seq: AtomicU64,
    /// Inconsistent-topic counter (DDS 1.4 §2.2.4.2.4). Incremented when
    /// matching discovers a remote endpoint carrying the same `topic_name`
    /// but a differing `type_name` in the SEDP cache. Read via
    /// [`DcpsRuntime::inconsistent_topic_count`].
    inconsistent_topic_seq: AtomicU64,
    /// D.5e Phase 3 — wake handle for the event-driven scheduler tick. `Some`
    /// only when started with `scheduler_tick`. Recv loops + the write path call
    /// [`DcpsRuntime::raise_tick_wake`] to wake the worker immediately on new
    /// work (so HEARTBEAT/ACKNACK/HB processing does not wait for a deadline).
    tick_wake: Mutex<Option<crate::scheduler::SchedulerHandle<TickEvent>>>,
    /// Coalesces wake raises: many incoming datagrams collapse into one wake.
    tick_wake_pending: AtomicBool,
    /// Worker thread JoinHandles. Per-socket recv threads + tick thread,
    /// all terminated together via `stop` (Sprint D.5b — previously
    /// a single single-threaded `event_loop`).
    handles: Mutex<Vec<JoinHandle<()>>>,
    /// Match-event notifier (D.5e Phase-1 quick win). Notified by the
    /// SEDP match path after `add_reader_proxy` / `add_writer_proxy`;
    /// `wait_for_matched_*` parks on it instead of polling every 20 ms.
    /// The mutex content is only a lock anchor for the Condvar API; there is
    /// no state protected by it (the count is read independently
    /// via `user_*_matched_count`).
    match_event: Arc<(Mutex<()>, Condvar)>,
    /// Acknowledgments event notifier. Notified when a writer
    /// receives an ACKNACK that advances its acked-base.
    /// `wait_for_acknowledgments` parks on it instead of polling every 50 ms.
    ack_event: Arc<(Mutex<()>, Condvar)>,
}

impl core::fmt::Debug for DcpsRuntime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DcpsRuntime")
            .field("domain_id", &self.domain_id)
            .field("guid_prefix", &self.guid_prefix)
            .field("spdp_group", &self.config.spdp_multicast_group)
            .finish_non_exhaustive()
    }
}

/// Type alias: Arc-shared slot handles from the per-slot mutex
/// architecture.
type WriterSlotArc = Arc<Mutex<UserWriterSlot>>;
type ReaderSlotArc = Arc<Mutex<UserReaderSlot>>;

impl DcpsRuntime {
    /// The 16-byte RTPS GUID of a local writer with EntityId `eid` (this
    /// participant's GUID prefix ++ the entity id). Used by the cross-vendor
    /// iceoryx-cyclone bridge to stamp the PSMX chunk with the writer's real
    /// GUID so a peer that discovered the writer over RTPS SEDP associates the
    /// shared-memory sample with it.
    #[must_use]
    pub fn writer_guid(&self, eid: EntityId) -> [u8; 16] {
        Guid::new(self.guid_prefix, eid).to_bytes()
    }

    // ========================================================================
    // --- Per-Slot-Mutex-Helpers
    //
    // The `user_writers`/`user_readers` registry is `RwLock<BTreeMap<EntityId,
    // Arc<Mutex<Slot>>>>`. Hot-path accesses take the read lock briefly, clone
    // the slot Arc and release the read lock before taking the per-slot mutex.
    // Parallel writes to **different** slots thereby run
    // without global contention.
    //
    // Slot creation/deletion takes the write lock; that is rare and
    // amortizes out.
    // ========================================================================

    /// Returns the slot Arc for a user writer, if present.
    /// Hot-path form: a single read lock + Arc clone, no
    /// per-slot mutex. The caller takes the mutex itself.
    fn writer_slot(&self, eid: EntityId) -> Option<WriterSlotArc> {
        self.user_writers
            .read()
            .ok()
            .and_then(|w| w.get(&eid).cloned())
    }

    /// Returns the slot Arc for a user reader, if present.
    fn reader_slot(&self, eid: EntityId) -> Option<ReaderSlotArc> {
        self.user_readers
            .read()
            .ok()
            .and_then(|r| r.get(&eid).cloned())
    }

    /// Snapshot of all writer slots as `Vec<(EntityId, Arc)>`. Allows
    /// iteration without holding the registry read lock — e.g. for
    /// the heartbeat tick or liveliness sweep, where we potentially take every
    /// slot's mutex.
    fn writer_slots_snapshot(&self) -> Vec<(EntityId, WriterSlotArc)> {
        match self.user_writers.read() {
            Ok(w) => w.iter().map(|(k, v)| (*k, Arc::clone(v))).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Snapshot of all reader slots — symmetric to writer_slots_snapshot.
    fn reader_slots_snapshot(&self) -> Vec<(EntityId, ReaderSlotArc)> {
        match self.user_readers.read() {
            Ok(r) => r.iter().map(|(k, v)| (*k, Arc::clone(v))).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Returns the list of EntityIds of all registered writers.
    /// Very lightweight — no slot-Arc clone, just EntityIds.
    fn writer_eids(&self) -> Vec<EntityId> {
        match self.user_writers.read() {
            Ok(w) => w.keys().copied().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Returns the list of EntityIds of all registered readers.
    fn reader_eids(&self) -> Vec<EntityId> {
        match self.user_readers.read() {
            Ok(r) => r.keys().copied().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Starts a new runtime for a participant.
    ///
    /// # Errors
    /// `TransportError` if one of the 3 UDP sockets fails to bind
    /// (e.g. a port collision on the SPDP multicast port in another
    /// SO_REUSE-less DDS instance).
    pub fn start(
        domain_id: i32,
        guid_prefix: GuidPrefix,
        mut config: RuntimeConfig,
    ) -> Result<Arc<Self>> {
        // C1 multicast-free discovery: merge the domain-aware env `ZERODDS_PEERS`
        // into the (programmatic) `config.initial_peers`. Default
        // is both empty → pure multicast behavior.
        config
            .initial_peers
            .extend(parse_initial_peers_env(domain_id as u32));
        // SPDP multicast receiver on the spec port.
        // u32 → u16 enforcing, the spec port is always < 65536.
        let spdp_port = u16::try_from(spdp_multicast_port(domain_id as u32)).map_err(|_| {
            DdsError::BadParameter {
                what: "domain_id too large for SPDP port mapping",
            }
        })?;
        let spdp_mc = UdpTransport::bind_multicast_v4(
            config.spdp_multicast_group,
            spdp_port,
            config.multicast_interface,
        )
        .map_err(|_| DdsError::TransportError {
            label: "spdp multicast bind",
        })?
        // Sprint D.5b: recv sockets have their own thread that
        // blocks waiting for data. Timeout 1 s = stop-flag polling
        // granularity at shutdown, NOT the tick rhythm.
        .with_timeout(Some(Duration::from_secs(1)))
        .map_err(|_| DdsError::TransportError {
            label: "spdp multicast set_timeout",
        })?;

        // SPDP unicast: bind to the **well-known** RTPS port
        // (7400+250*domain+10+2*pid, Spec §9.6.1.4.1), so a
        // configured unicast initial peer can reach this participant
        // WITHOUT prior multicast (C1 multicast-free
        // discovery). Participant index 0,1,2,… until a free port
        // is found (multiple participants per host, also alongside
        // Cyclone/FastDDS). Fallback ephemeral if all well-known
        // ports are taken (then multicast discovery only).
        // Interface pinning (ZERODDS_INTERFACE): UNSPECIFIED = auto. If
        // set, ALL IP sockets bind (SPDP-uc, SPDP-mc-tx, user UDP/TCP)
        // to this IP → announce + egress + receive on exactly this
        // interface (multi-homed robustness, cf. Cyclone `NetworkInterface`).
        let pinned = config.multicast_interface;
        let (spdp_uc_raw, _spdp_participant_id) = {
            let mut bound = None;
            for pid in 0u32..120 {
                let Ok(port) = u16::try_from(spdp_unicast_port(domain_id as u32, pid)) else {
                    break;
                };
                if let Ok(sock) = UdpTransport::bind_v4(pinned, port) {
                    bound = Some((sock, pid));
                    break;
                }
            }
            match bound {
                Some(b) => b,
                None => (
                    UdpTransport::bind_v4(pinned, 0).map_err(|_| DdsError::TransportError {
                        label: "spdp unicast bind",
                    })?,
                    u32::MAX,
                ),
            }
        };
        let spdp_uc = spdp_uc_raw
            .with_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| DdsError::TransportError {
                label: "spdp unicast set_timeout",
            })?;

        // User-data unicast (ephemeral port). Transport choice primarily via
        // `RuntimeConfig::user_transport`, fallback to the env var
        // `ZERODDS_USER_TRANSPORT` (bench binaries), otherwise UDPv4.
        // SPDP multicast stays UDPv4 — the DDSI-RTPS spec mandates
        // 239.255.0.1 for cross-vendor discovery; v6-only hosts
        // cannot discover cross-vendor (its own sprint).
        let (user_uc, tcp_accept_handle): (Arc<dyn Transport + Send + Sync>, _) =
            if !config.user_transports.is_empty() {
                // Multi-transport: build each kind and layer them. Preference =
                // config order (first match by destination locator kind wins).
                let mut legs: alloc::vec::Vec<Arc<dyn Transport + Send + Sync>> =
                    alloc::vec::Vec::new();
                let mut tcp_handle = None;
                for kind in &config.user_transports {
                    let (leg, tcp) = select_user_transport(*kind, guid_prefix, domain_id, pinned)?;
                    legs.push(leg);
                    if tcp.is_some() {
                        tcp_handle = tcp;
                    }
                }
                let layered = Arc::new(crate::layered_transport::LayeredUserTransport::new(legs));
                (layered, tcp_handle)
            } else {
                let user_transport_kind = config
                    .user_transport
                    .or_else(parse_user_transport_env)
                    .unwrap_or(UserTransportKind::UdpV4);
                select_user_transport(user_transport_kind, guid_prefix, domain_id, pinned)?
            };

        // Separate sender socket for the SPDP announce. Ephemeral port; with
        // interface pinning it binds to the pinned IP (egress source), otherwise
        // `0.0.0.0` (the kernel picks the outgoing interface per route).
        let spdp_mc_tx =
            UdpTransport::bind_v4(pinned, 0).map_err(|_| DdsError::TransportError {
                label: "spdp mc-tx bind",
            })?;

        let stop = Arc::new(AtomicBool::new(false));

        // Materialize beacon locators for cross-host interop:
        // with a `0.0.0.0` bind address (UNSPECIFIED) the peer would
        // otherwise learn a non-routable address. We resolve UNSPECIFIED
        // via a UDP connect probe to a non-routable IP
        // (no traffic, just the routing table) and announce the
        // resulting local interface address — cross-host-capable
        // without an external crate dependency.
        let user_locator = announce_locator(&*user_uc, config.multicast_interface);
        let spdp_uc_locator = announce_locator(&spdp_uc, config.multicast_interface);
        let participant_data = ParticipantBuiltinTopicData {
            guid: Guid::new(guid_prefix, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: Some(user_locator),
            default_multicast_locator: None,
            metatraffic_unicast_locator: Some(spdp_uc_locator),
            metatraffic_multicast_locator: Some(Locator {
                kind: LocatorKind::UdpV4,
                port: u32::from(spdp_port),
                address: {
                    let mut a = [0u8; 16];
                    a[12..].copy_from_slice(&config.spdp_multicast_group.octets());
                    a
                },
            }),
            domain_id: Some(domain_id as u32),
            // We announce the endpoints we actually
            // implement: SPDP (participant ann/det) + SEDP
            // (publications/subscriptions ann+det) + WLP (10/11) +
            // TypeLookup service (12/13). Cyclone/Fast-DDS filter
            // their proxy setup by these flags — without them
            // we get no SEDP/WLP peers. SEDP topic
            // endpoints (bits 28/29) are optional per RTPS 2.5 §8.5.4.4
            // and covered in ZeroDDS via synthetic DCPSTopic
            // derivation from pub/sub — we do not announce them,
            // otherwise we promise peers a non-existent
            // endpoint pairing. When the caller sets
            // `announce_secure_endpoints = true` (security
            // factory path), we additionally mix in the 12 secure
            // discovery bits (16..27, DDS-Security 1.2 §7.4.7.1).
            builtin_endpoint_set: {
                let mut mask = endpoint_flag::ALL_STANDARD;
                if config.announce_secure_endpoints {
                    mask |= endpoint_flag::ALL_SECURE;
                }
                mask
            },
            // Spec default lease = 100 s; configurable via
            // `RuntimeConfig::participant_lease_duration`.
            lease_duration: qos_duration_from_std(config.participant_lease_duration),
            // UserData on the participant — filled from
            // `DomainParticipantQos::user_data` via RuntimeConfig.
            user_data: config.user_data.clone(),
            // PROPERTY_LIST: security fills this with security caps
            // once a PolicyEngine is configured. Default-empty
            // stays backward-compatible with legacy peers.
            properties: Default::default(),
            // IdentityToken/PermissionsToken are filled by the security
            // layer once authentication + access control are
            // initialized. Default `None` = legacy announce.
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            // Filled by the security layer (enable_security_builtins*) —
            // without PID_PARTICIPANT_SECURITY_INFO foreign vendors classify
            // us as non-secure. Default None = legacy/plain.
            participant_security_info: None,
        };
        let beacon = SpdpBeacon::new(participant_data.clone());
        let sedp = SedpStack::new(guid_prefix, VendorId::ZERODDS);
        // In-process discovery fastpath: remember the multicast group before
        // `config` is moved into the struct literal.
        let inproc_group = config.spdp_multicast_group;

        #[cfg(feature = "security")]
        let outbound_pool = if config.interface_bindings.is_empty() {
            None
        } else {
            Some(Arc::new(OutboundSocketPool::bind_all(
                &config.interface_bindings,
            )?))
        };

        // WLP endpoint (RTPS 2.5 §8.4.13). The tick period is explicit
        // `wlp_period`, or `lease/3` when `wlp_period == ZERO`
        // (spec recommendation: three misses before the reader marks the
        // writer as not-alive).
        let wlp_tick_period = if config.wlp_period.is_zero() {
            config.participant_lease_duration / 3
        } else {
            config.wlp_period
        };
        let wlp = crate::wlp::WlpEndpoint::new(guid_prefix, VendorId::ZERODDS, wlp_tick_period);

        let rt = Arc::new(Self {
            guid_prefix,
            domain_id,
            spdp_multicast_rx: Arc::new(spdp_mc),
            spdp_unicast: Arc::new(spdp_uc),
            user_unicast: user_uc,
            user_announce_locator: user_locator,
            spdp_mc_tx: Arc::new(spdp_mc_tx),
            spdp_beacon: Mutex::new(beacon),
            participant_data,
            announced_pubs: Mutex::new(Vec::new()),
            announced_subs: Mutex::new(Vec::new()),
            spdp_reader: SpdpReader::new(),
            discovered: Arc::new(Mutex::new(DiscoveredParticipantsCache::new())),
            spdp_relay_cache: Mutex::new(alloc::collections::BTreeMap::new()),
            sedp: Arc::new(Mutex::new(sedp)),
            type_lookup_endpoints: TypeLookupEndpoints::new(guid_prefix),
            type_lookup_server: Arc::new(Mutex::new(TypeLookupServer::new())),
            type_lookup_client: Arc::new(Mutex::new(TypeLookupClient::new())),
            tl_reply_sn: core::sync::atomic::AtomicU64::new(0),
            security_builtin: Mutex::new(None),
            start_instant: Instant::now(),
            user_writers: Arc::new(RwLock::new(BTreeMap::new())),
            shm_locators: Arc::new(RwLock::new(BTreeMap::new())),
            same_host: Arc::new(crate::same_host::SameHostTracker::new()),
            user_readers: Arc::new(RwLock::new(BTreeMap::new())),
            #[cfg(feature = "security")]
            endpoint_tokens_sent: Arc::new(RwLock::new(alloc::collections::BTreeSet::new())),
            #[cfg(feature = "security")]
            sedp_reannounced: Arc::new(RwLock::new(alloc::collections::BTreeSet::new())),
            #[cfg(feature = "security")]
            endpoint_crypto: Arc::new(RwLock::new(alloc::collections::BTreeMap::new())),
            intra_runtime_routes: Arc::new(RwLock::new(BTreeMap::new())),
            entity_counter: AtomicU32::new(1),
            config,
            stop: stop.clone(),
            tick_seq: AtomicU64::new(0),
            spdp_announce_seq: AtomicU64::new(0),
            inconsistent_topic_seq: AtomicU64::new(0),
            tick_wake: Mutex::new(None),
            tick_wake_pending: AtomicBool::new(false),
            handles: Mutex::new(Vec::new()),
            match_event: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            ack_event: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            #[cfg(feature = "security")]
            outbound_pool,
            wlp: Arc::new(Mutex::new(wlp)),
            builtin_sinks: Mutex::new(None),
            ignore_filter: Mutex::new(None),
        });

        // In-process discovery fastpath: register the runtime in the process
        // registry so same-process+domain peers find each other
        // deterministically (see `crate::inproc`). Right
        // after, `pull-on-creation`: pull all already-announced endpoints
        // of existing peers into our SEDP cache — otherwise
        // we see peers that announced endpoints BEFORE us
        // only via the (lossy) UDP SEDP path.
        crate::inproc::register(&rt, domain_id, inproc_group);
        rt.inproc_pull_from_peers();

        // Per-socket recv threads + one tick thread (Sprint D.5b).
        //
        // Previously the entire stack ran in a single event loop
        // that went through three blocking `recv()`s with a `tick_period`
        // timeout (50 ms) sequentially per iteration. On a
        // roundtrip each stage waited up to 50 ms for timeouts of the
        // front sockets before its own datagram got its turn —
        // yielded 5-14 ms p50.
        //
        // Refit: every relevant recv path has its own thread
        // that sits directly blocking on its socket and dispatches
        // immediately when data arrives. The tick thread does the
        // periodic outbound work (HEARTBEAT/resend/ACKNACK/
        // SPDP announce/deadline/lifespan/liveliness) and sleeps
        // `tick_period` between iterations.
        //
        // Lock order (deadlock avoidance): the tick thread and
        // recv threads contend for `rt.sedp.lock()` / `rt.wlp.lock()`.
        // Convention: keep lock-hold times short (handle_datagram /
        // tick are both fast), do not take a sub-lock under the `sedp`
        // or `wlp` lock.
        let mut handles_init: Vec<JoinHandle<()>> = Vec::with_capacity(4);

        let rt_recv_spdp_mc = Arc::clone(&rt);
        let stop_recv_spdp_mc = stop.clone();
        handles_init.push(
            thread::Builder::new()
                .name(String::from("zdds-recv-spdp-mc"))
                .spawn(move || recv_spdp_multicast_loop(rt_recv_spdp_mc, stop_recv_spdp_mc))
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "spawn zdds-recv-spdp-mc thread",
                })?,
        );

        let rt_recv_meta = Arc::clone(&rt);
        let stop_recv_meta = stop.clone();
        handles_init.push(
            thread::Builder::new()
                .name(String::from("zdds-recv-meta"))
                .spawn(move || recv_metatraffic_loop(rt_recv_meta, stop_recv_meta))
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "spawn zdds-recv-meta thread",
                })?,
        );

        let rt_recv_user = Arc::clone(&rt);
        let stop_recv_user = stop.clone();
        let primary_socket = Arc::clone(&rt.user_unicast);
        handles_init.push(
            thread::Builder::new()
                .name(String::from("zdds-recv-user"))
                .spawn(move || recv_user_data_loop(rt_recv_user, primary_socket, stop_recv_user))
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "spawn zdds-recv-user thread",
                })?,
        );

        // TCPv4 variant: a separate accept worker (TcpTransport has
        // no implicit accept thread in the constructor — accept_one()
        // must be called explicitly).
        if let Some(tcp_arc) = tcp_accept_handle {
            let stop_accept = stop.clone();
            handles_init.push(
                thread::Builder::new()
                    .name(String::from("zdds-tcp-accept"))
                    .spawn(move || {
                        while !stop_accept.load(Ordering::Relaxed) {
                            // accept_one() blocks until connection +
                            // handshake; on EOF it returns Ok(()) and
                            // we accept the next peer.
                            let _ = tcp_arc.accept_one();
                        }
                    })
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "spawn zdds-tcp-accept thread",
                    })?,
            );
        }

        // Opt-3 (Spec `zerodds-zero-copy-1.0` §9): additional
        // SO_REUSEPORT recv workers. Each binds to the same
        // user_unicast port; the kernel distributes incoming datagrams via
        // flow hash. On a bind error (e.g. a platform without
        // SO_REUSEPORT support) the worker is skipped and the
        // runtime continues with the available workers.
        if rt.config.extra_recv_threads > 0 {
            let user_port = u16::try_from(rt.user_unicast.local_locator().port).unwrap_or(0);
            // With an active security config, share the first interface bind address;
            // otherwise INADDR_ANY (the kernel chooses).
            #[cfg(feature = "security")]
            let bind_addr = rt
                .config
                .interface_bindings
                .first()
                .map(|spec| spec.bind_addr)
                .unwrap_or(Ipv4Addr::UNSPECIFIED);
            #[cfg(not(feature = "security"))]
            let bind_addr = Ipv4Addr::UNSPECIFIED;
            for i in 0..rt.config.extra_recv_threads {
                let extra_socket =
                    match UdpTransport::bind_v4_reuse(bind_addr, user_port) {
                        Ok(t) => Arc::new(t.with_timeout(Some(Duration::from_secs(1))).map_err(
                            |_| DdsError::TransportError {
                                label: "extra-recv set_timeout failed",
                            },
                        )?),
                        Err(_) => break, // SO_REUSEPORT not available — skip.
                    };
                let rt_extra = Arc::clone(&rt);
                let stop_extra = stop.clone();
                let name = format!("zdds-recv-user-{}", i + 1);
                handles_init.push(
                    thread::Builder::new()
                        .name(name)
                        .spawn(move || recv_user_data_loop(rt_extra, extra_socket, stop_extra))
                        .map_err(|_| DdsError::PreconditionNotMet {
                            reason: "spawn zdds-recv-user-N thread",
                        })?,
                );
            }
        }

        // Wave 4b.4 (Spec `zerodds-zero-copy-1.0` §6): per-owner
        // SHM recv loop. Polls all bound consumer entries of the
        // SameHostTracker round-robin and dispatches incoming
        // frames analogous to the UDP path. Only compiled when
        // the `same-host-shm` feature is on.
        #[cfg(feature = "same-host-shm")]
        {
            let rt_recv_shm = Arc::clone(&rt);
            let stop_recv_shm = stop.clone();
            handles_init.push(
                thread::Builder::new()
                    .name(String::from("zdds-recv-shm"))
                    .spawn(move || recv_user_shm_loop(rt_recv_shm, stop_recv_shm))
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "spawn zdds-recv-shm thread",
                    })?,
            );
        }

        // zerodds-async-1.0 §4: with `external_tick`, the tick loop is driven
        // by an external executor (tokio via `spawn_in_tokio`) rather than a
        // dedicated thread — so we skip the spawn here. `stop` is dropped; the
        // driver observes shutdown via `rt.stop` (set in `Drop`/`stop()`).
        if rt.config.external_tick {
            drop(stop);
        } else if rt.config.scheduler_tick {
            // D.5e Phase 3 — event-driven scheduler tick. Create the scheduler
            // up front, publish its wake handle so recv loops + the write path
            // can `raise_tick_wake`, then drive the (unchanged) tick from the
            // deadline-heap worker.
            let (scheduler, handle) =
                crate::scheduler::Scheduler::<TickEvent>::new(SCHEDULER_IDLE_FLOOR);
            if let Ok(mut g) = rt.tick_wake.lock() {
                *g = Some(handle.clone());
            }
            let rt_tick = Arc::clone(&rt);
            let stop_tick = stop;
            handles_init.push(
                thread::Builder::new()
                    .name(String::from("zdds-tick-sched"))
                    .spawn(move || scheduler_tick_loop(rt_tick, stop_tick, scheduler, handle))
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "spawn zdds-tick-sched thread",
                    })?,
            );
        } else {
            let rt_tick = Arc::clone(&rt);
            let stop_tick = stop;
            handles_init.push(
                thread::Builder::new()
                    .name(String::from("zdds-tick"))
                    .spawn(move || tick_loop(rt_tick, stop_tick))
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "spawn zdds-tick thread",
                    })?,
            );
        }

        let mut guard = rt
            .handles
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "runtime handles mutex poisoned",
            })?;
        *guard = handles_init;
        drop(guard);

        Ok(rt)
    }

    /// Local unicast locator for user data (announced in SPDP).
    #[must_use]
    pub fn user_locator(&self) -> zerodds_rtps::wire_types::Locator {
        self.user_unicast.local_locator()
    }

    /// Local unicast locator for SPDP metatraffic.
    #[must_use]
    pub fn spdp_unicast_locator(&self) -> zerodds_rtps::wire_types::Locator {
        self.spdp_unicast.local_locator()
    }

    /// Returns the `BuiltinEndpointSet` bitmask that the runtime
    /// currently announces in the SPDP beacon. Used for tests + diagnostics;
    /// production consumers should decode the SPDP beacon
    /// themselves.
    #[must_use]
    pub fn announced_builtin_endpoint_set(&self) -> u32 {
        self.spdp_beacon
            .lock()
            .map(|b| b.data.builtin_endpoint_set)
            .unwrap_or(0)
    }

    /// Registers a `TypeObject` in the local TypeLookup server
    /// registry. Other participants can then query this type via
    /// a `getTypes` request (XTypes 1.3 §7.6.3.3.4).
    ///
    /// Returns the `EquivalenceHash` of the registered type
    /// (the caller can embed it e.g. in `PublicationBuiltinTopicData` as a
    /// PID_TYPE_INFORMATION hint).
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` on lock poisoning or a hash
    /// computation error.
    pub fn register_type_object(
        &self,
        obj: zerodds_types::type_object::TypeObject,
    ) -> Result<zerodds_types::EquivalenceHash> {
        let hash = zerodds_types::compute_hash(&obj).map_err(|_| DdsError::PreconditionNotMet {
            reason: "type hash computation failed",
        })?;
        let mut server =
            self.type_lookup_server
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "type_lookup_server mutex poisoned",
                })?;
        match obj {
            zerodds_types::type_object::TypeObject::Minimal(m) => {
                server.registry.insert_minimal(hash, m);
            }
            zerodds_types::type_object::TypeObject::Complete(c) => {
                server.registry.insert_complete(hash, c);
            }
            _ => {
                return Err(DdsError::PreconditionNotMet {
                    reason: "unknown TypeObject variant",
                });
            }
        }
        Ok(hash)
    }

    /// Sends a `getTypes` request to a discovered peer and
    /// returns a `RequestId` with which the caller can correlate the
    /// asynchronous reply later (XTypes 1.3
    /// §7.6.3.3.4 + `TypeLookupClient::handle_reply`).
    ///
    /// `peer` must be in `discovered_participants()` — otherwise
    /// `None` is returned (no known peer locator). On a
    /// successful send the request sample-identity sequence
    /// is returned as the `RequestId`; an incoming reply is correlated on
    /// this sequence ID.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` on encode errors or lock
    /// poisoning.
    pub fn send_type_lookup_request(
        &self,
        peer: zerodds_rtps::wire_types::GuidPrefix,
        type_hashes: &[zerodds_types::EquivalenceHash],
    ) -> Result<Option<zerodds_discovery::type_lookup::RequestId>> {
        use alloc::sync::Arc as AllocArc;
        use zerodds_discovery::type_lookup::request_types_payload;
        use zerodds_rtps::datagram::encode_data_datagram;
        use zerodds_rtps::header::RtpsHeader;
        use zerodds_rtps::submessages::DataSubmessage;
        use zerodds_rtps::wire_types::{ProtocolVersion, SequenceNumber};

        // Find peer's user-unicast locator (default-unicast first;
        // fallback metatraffic-unicast). TypeLookup datagrams go via
        // the user-unicast path — the peer DCPS runtime has a
        // shared receive loop there for SEDP/user data/TypeLookup.
        let target = {
            let discovered = self
                .discovered
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "discovered mutex poisoned",
                })?;
            let Some(dp) = discovered.get(&peer) else {
                return Ok(None);
            };
            dp.data
                .default_unicast_locator
                .or(dp.data.metatraffic_unicast_locator)
        };
        let Some(target) = target else {
            return Ok(None);
        };

        // Allocate RequestId (client-side incrementing sequence). Reply
        // correlation runs via the `handle_reply` callback. We
        // register a callback that feeds the returned
        // TypeObjects into the local `TypeLookupServer.registry`
        // (XTypes 1.3 §7.6.3.3.4): hash-by-hash, separately
        // for Minimal and Complete variants. This way a hash that
        // was resolved once is recognized for future `has_type_for_hash`
        // checks (= no re-requests).
        let mut client =
            self.type_lookup_client
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "type_lookup_client mutex poisoned",
                })?;
        let type_ids: alloc::vec::Vec<zerodds_types::TypeIdentifier> = type_hashes
            .iter()
            .map(|h| zerodds_types::TypeIdentifier::EquivalenceHashMinimal(*h))
            .collect();
        let server_for_cb = Arc::clone(&self.type_lookup_server);
        let cb = Box::new(
            move |reply: zerodds_discovery::type_lookup::TypeLookupReply| {
                let zerodds_discovery::type_lookup::TypeLookupReply::Types(types_reply) = reply
                else {
                    return;
                };
                let Ok(mut server) = server_for_cb.lock() else {
                    return;
                };
                for t in &types_reply.types {
                    match t {
                        zerodds_types::type_lookup::ReplyTypeObject::Minimal(m) => {
                            let to = zerodds_types::type_object::TypeObject::Minimal(m.clone());
                            if let Ok(h) = zerodds_types::compute_hash(&to) {
                                server.registry.insert_minimal(h, m.clone());
                            }
                        }
                        zerodds_types::type_lookup::ReplyTypeObject::Complete(c) => {
                            let to = zerodds_types::type_object::TypeObject::Complete(c.clone());
                            if let Ok(h) = zerodds_types::compute_hash(&to) {
                                server.registry.insert_complete(h, c.clone());
                            }
                        }
                    }
                }
            },
        );
        let request_id = client.request_types(type_ids.clone(), cb);
        drop(client);

        // Encode the wire request payload (PL_CDR_LE-Encapsulation).
        let body = request_types_payload(&type_ids).map_err(|_| DdsError::PreconditionNotMet {
            reason: "type_lookup request payload encode failed",
        })?;
        let mut payload: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(4 + body.len());
        payload.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        payload.extend_from_slice(&body);

        // Use the RequestId as the writer_sn so the peer-side reply can
        // echo it for correlation (XTypes §7.6.3.3.3 Sample-Identity).
        let id_u64 = request_id.0;
        let sn =
            SequenceNumber::from_high_low((id_u64 >> 32) as i32, (id_u64 & 0xFFFF_FFFF) as u32);
        let header = RtpsHeader {
            protocol_version: ProtocolVersion::CURRENT,
            vendor_id: VendorId::ZERODDS,
            guid_prefix: self.guid_prefix,
        };
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::TL_SVC_REQ_READER,
            writer_id: EntityId::TL_SVC_REQ_WRITER,
            writer_sn: sn,
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: AllocArc::from(payload.into_boxed_slice()),
        };
        let datagram =
            encode_data_datagram(header, &[data]).map_err(|_| DdsError::PreconditionNotMet {
                reason: "type_lookup request datagram encode failed",
            })?;

        if is_routable_user_locator(&target) {
            let _ = self.user_unicast.send(&target, &datagram);
        }
        Ok(Some(request_id))
    }

    /// Activates the security builtin endpoint stack
    /// (`DCPSParticipantStatelessMessage` + `DCPSParticipantVolatile-
    /// MessageSecure`). Typically called by the factory
    /// once a security plugin is registered on the participant.
    /// Idempotent: a second call has no effect. Returns the (possibly
    /// freshly created) stack handle.
    pub fn enable_security_builtins(
        &self,
        vendor_id: VendorId,
    ) -> Arc<Mutex<SecurityBuiltinStack>> {
        self.install_security_stack(SecurityBuiltinStack::new(self.guid_prefix, vendor_id))
    }

    /// Like [`enable_security_builtins`](Self::enable_security_builtins),
    /// but with an active auth-handshake driver (FU2 Gap 4). The stack
    /// is built via [`SecurityBuiltinStack::with_auth`]: the shared
    /// `auth` plugin (= the same instance that hangs on the crypto gate as
    /// the `SharedSecretProvider`) drives the PKI handshake as soon as
    /// a peer with stateless bits + identity token is discovered.
    ///
    /// `local_identity` comes from `validate_local_identity`; the local
    /// 16-byte participant GUID is derived from the `guid_prefix`.
    ///
    /// Idempotent (first-wins): if a stack is already active — even one
    /// built without auth — that one is returned and the freshly
    /// built one discarded.
    #[cfg(feature = "security")]
    pub fn enable_security_builtins_with_auth(
        self: &Arc<Self>,
        vendor_id: VendorId,
        auth: Arc<Mutex<dyn zerodds_security::authentication::AuthenticationPlugin>>,
        local_identity: zerodds_security::authentication::IdentityHandle,
    ) -> Arc<Mutex<SecurityBuiltinStack>> {
        let local_guid = Guid::new(self.guid_prefix, EntityId::PARTICIPANT).to_bytes();
        // Announce the local IdentityToken in the SPDP beacon (PID_IDENTITY_TOKEN,
        // FU2 Gap 7c) + set the stateless/volatile-secure bits, so peers
        // initiate the auth handshake. Before moving `auth` into the stack.
        if let Ok(mut plugin) = auth.lock() {
            if let Ok(token) = plugin.get_identity_token(local_identity) {
                // PID_PERMISSIONS_TOKEN (§7.4.1.5, S4 point 1): secure
                // vendors (cyclone/FastDDS) start validate_remote_identity
                // only when SPDP carries identity_token AND permissions_token;
                // otherwise we stay non-secure and all endpoints are "not
                // allowed". Empty if no permissions are configured.
                let perm_token = plugin.get_permissions_token();
                let pdata = if let Ok(mut beacon) = self.spdp_beacon.lock() {
                    if !token.is_empty() {
                        beacon.data.identity_token = Some(token);
                    }
                    if !perm_token.is_empty() {
                        beacon.data.permissions_token = Some(perm_token);
                    }
                    // Full secure builtin endpoint set (§7.4.7.1): stateless +
                    // VolatileSecure (22-25) PLUS secure SEDP (16-19),
                    // secure ParticipantMessage (20-21) and DCPSParticipantsSecure
                    // (26-27). cyclone starts validate_remote_identity + creates the
                    // secure builtin proxies ONLY when the remote announces the full
                    // secure set (cyclone-trace-verified) — only
                    // 22-25 → "Non secure remote ... not allowed", no handshake.
                    beacon.data.builtin_endpoint_set |= endpoint_flag::PUBLICATIONS_SECURE_WRITER
                        | endpoint_flag::PUBLICATIONS_SECURE_READER
                        | endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER
                        | endpoint_flag::SUBSCRIPTIONS_SECURE_READER
                        | endpoint_flag::PARTICIPANT_MESSAGE_SECURE_WRITER
                        | endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER
                        | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
                        | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
                        | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
                        | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
                        | endpoint_flag::PARTICIPANT_SECURE_WRITER
                        | endpoint_flag::PARTICIPANT_SECURE_READER;
                    // PID_PARTICIPANT_SECURITY_INFO (§7.4.1.6): marks us as a
                    // secure participant — mandatory, otherwise cyclone/
                    // FastDDS treat us as non-secure and reject all endpoints.
                    // IS_VALID on both masks; derive the participant-level
                    // ParticipantSecurityAttributes (§9.4.2.4) from the governance:
                    // is_{rtps,discovery,liveliness}_protected in the
                    // attr mask, is_*_encrypted in the plugin mask. cyclone
                    // matches the announced bits against its own governance —
                    // a null mask with e.g. discovery=ENCRYPT is a policy
                    // mismatch and cyclone then establishes NO secured
                    // participant crypto handshake (bug source protected discovery).
                    use zerodds_rtps::participant_security_info::{
                        ParticipantSecurityInfo, attrs, plugin_attrs,
                    };
                    let (mut a, mut p) = (attrs::IS_VALID, plugin_attrs::IS_VALID);
                    if let Some(gate) = self.config.security.as_ref() {
                        let rtps = gate.rtps_protection().unwrap_or(ProtectionLevel::None);
                        let disc = gate.discovery_protection().unwrap_or(ProtectionLevel::None);
                        let live = gate
                            .liveliness_protection()
                            .unwrap_or(ProtectionLevel::None);
                        if rtps != ProtectionLevel::None {
                            a |= attrs::IS_RTPS_PROTECTED;
                        }
                        if disc != ProtectionLevel::None {
                            a |= attrs::IS_DISCOVERY_PROTECTED;
                        }
                        if live != ProtectionLevel::None {
                            a |= attrs::IS_LIVELINESS_PROTECTED;
                        }
                        if rtps == ProtectionLevel::Encrypt {
                            p |= plugin_attrs::IS_RTPS_ENCRYPTED;
                        }
                        if disc == ProtectionLevel::Encrypt {
                            p |= plugin_attrs::IS_DISCOVERY_ENCRYPTED;
                        }
                        if live == ProtectionLevel::Encrypt {
                            p |= plugin_attrs::IS_LIVELINESS_ENCRYPTED;
                        }
                    }
                    beacon.data.participant_security_info = Some(ParticipantSecurityInfo {
                        participant_security_attributes: a,
                        plugin_participant_security_attributes: p,
                    });
                    // c.pdata (§9.3.2.5.2, S4 root 6+7): our own
                    // ParticipantBuiltinTopicData as PL_CDR_**BE** — the replier
                    // (cyclone) deserializes c.pdata strictly as a big-endian
                    // ParameterList and binds the participant_guid to the
                    // authenticated identity. LE → "payload too long".
                    Some(beacon.data.to_pl_cdr_be())
                } else {
                    None
                };
                if let Some(pd) = pdata {
                    plugin.set_local_participant_data(pd);
                }
            }
        }
        let stack = self.install_security_stack(SecurityBuiltinStack::with_auth(
            self.guid_prefix,
            vendor_id,
            auth,
            local_identity,
            local_guid,
        ));
        // FU2 S3: kick off in-process participant discovery + handshake trigger
        // deterministically — decouples the secured discovery from the
        // flaky multicast path (codepit LXC). Bidirectional, idempotent.
        self.inproc_announce_participant();
        // FU2 S3: immediate token-carrying SPDP re-announce (event-driven).
        self.announce_spdp_now();
        stack
    }

    /// Installs a freshly built `SecurityBuiltinStack` into the
    /// runtime slot (idempotent) and catches up on peers already
    /// discovered via SPDP. Shared core of
    /// [`enable_security_builtins`](Self::enable_security_builtins) and
    /// [`enable_security_builtins_with_auth`](Self::enable_security_builtins_with_auth).
    fn install_security_stack(
        &self,
        fresh: SecurityBuiltinStack,
    ) -> Arc<Mutex<SecurityBuiltinStack>> {
        // Lock poisoning is a bug indicator here (an earlier panic in the
        // hot path). In that case we return a fresh, isolated
        // stack — the caller gets at least a
        // functional slot, but the hot path writes its mutations
        // into the unlocked original. In production code this does not happen;
        // in tests (where poisoning can occur) this is a
        // best-effort recovery.
        let mut slot = match self.security_builtin.lock() {
            Ok(g) => g,
            Err(_) => {
                return Arc::new(Mutex::new(fresh));
            }
        };
        if let Some(existing) = slot.as_ref() {
            return Arc::clone(existing);
        }
        let stack = Arc::new(Mutex::new(fresh));
        // Catch up on already-discovered peers (discovery may have already
        // seen SPDP beacons before the plugin was activated).
        if let Ok(cache) = self.discovered.lock() {
            if let Ok(mut s) = stack.lock() {
                for peer in cache.iter() {
                    s.handle_remote_endpoints(peer);
                }
            }
        }
        *slot = Some(Arc::clone(&stack));
        // Protected discovery (DDS-Security §8.4.2.4): if the governance demands
        // `discovery_protection_kind != NONE`, the SedpStack routes secured
        // endpoints via the secure SEDP (DCPSPublicationsSecure/Subscriptions
        // Secure) instead of plaintext — the runtime send path protects their DATA/
        // HEARTBEAT/GAP with the participant data key. Set before the first
        // announce_* (endpoint creation follows the security activation).
        #[cfg(feature = "security")]
        if let Some(gate) = self.config.security.as_ref() {
            let protected = gate
                .discovery_protection()
                .map(|l| l != ProtectionLevel::None)
                .unwrap_or(false);
            if protected {
                if let Ok(mut sedp) = self.sedp.lock() {
                    sedp.set_discovery_protected(true);
                }
            }
        }
        stack
    }

    /// Snapshot handle on the security builtin stack. `None` if
    /// [`enable_security_builtins`](Self::enable_security_builtins)
    /// has not been called yet.
    #[must_use]
    pub fn security_builtin_snapshot(&self) -> Option<Arc<Mutex<SecurityBuiltinStack>>> {
        self.security_builtin.lock().ok()?.as_ref().map(Arc::clone)
    }

    /// `assert_liveliness()` on the `DomainParticipant` (DCPS 1.4
    /// §2.2.3.11 MANUAL_BY_PARTICIPANT). Sends exactly one WLP heartbeat
    /// with `kind = MANUAL_BY_PARTICIPANT` on the next tick;
    /// all readers matching this participant refresh their
    /// last-seen timestamp. Idempotent — multiple calls within
    /// one tick period result in multiple wire sends up to the
    /// cap (`MAX_QUEUED_PULSES = 32`).
    pub fn assert_liveliness(&self) {
        if let Ok(mut wlp) = self.wlp.lock() {
            wlp.assert_participant();
        }
    }

    /// `assert_liveliness()` on a `DataWriter` (DCPS 1.4 §2.2.3.11
    /// MANUAL_BY_TOPIC). `topic_token` is an opaque token that
    /// matching readers can use to associate the pulse with a concrete
    /// topic. We use the ZeroDDS vendor kind (Cyclone /
    /// Fast-DDS ignore the vendor kind, which is spec-conformant —
    /// MSB-set in `kind` requests "ignore unknown" behavior).
    pub fn assert_writer_liveliness(&self, topic_token: Vec<u8>) {
        if let Ok(mut wlp) = self.wlp.lock() {
            wlp.assert_topic(topic_token);
        }
    }

    /// Current WLP last-seen timestamp of a remote peer (relative
    /// to runtime start). `None` if the peer has not sent a WLP
    /// heartbeat yet.
    #[must_use]
    pub fn peer_liveliness_last_seen(&self, prefix: &GuidPrefix) -> Option<Duration> {
        self.wlp
            .lock()
            .ok()
            .and_then(|w| w.peer_state(prefix).map(|s| s.last_seen))
    }

    /// Returns the [`zerodds_discovery::PeerCapabilities`] of a remote
    /// peer, based on its most recently received SPDP beacon.
    /// `None` if the peer has not been discovered via SPDP yet.
    #[must_use]
    pub fn peer_capabilities(
        &self,
        prefix: &GuidPrefix,
    ) -> Option<zerodds_discovery::PeerCapabilities> {
        self.discovered
            .lock()
            .ok()
            .and_then(|d| d.get(prefix).map(|p| p.data.builtin_endpoint_set))
            .map(zerodds_discovery::PeerCapabilities::from_bits)
    }

    /// Snapshot of the currently discovered remote participants.
    /// Key = GUID prefix, value = last seen beacon content.
    #[must_use]
    pub fn discovered_participants(&self) -> Vec<DiscoveredParticipant> {
        self.discovered
            .lock()
            .map(|cache| cache.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Wires the `BuiltinSinks` of the `DomainParticipant` into the
    /// discovery hot path. From this
    /// call on, all SPDP/SEDP receive events land as samples in
    /// the 4 builtin-topic readers.
    ///
    /// Called by the `DomainParticipant` constructor exactly once during
    /// setup.
    pub fn attach_builtin_sinks(&self, sinks: crate::builtin_subscriber::BuiltinSinks) {
        if let Ok(mut guard) = self.builtin_sinks.lock() {
            *guard = Some(sinks);
        }
    }

    /// Snapshot of the currently wired BuiltinSinks (internal, for the
    /// hot path).
    pub(crate) fn builtin_sinks_snapshot(&self) -> Option<crate::builtin_subscriber::BuiltinSinks> {
        self.builtin_sinks.lock().ok().and_then(|g| g.clone())
    }

    /// Wires the `IgnoreFilter` of the `DomainParticipant` into the
    /// discovery hot path. From
    /// this call on, SPDP/SEDP receive events are checked against the
    /// filter before being pushed as a builtin sample or used as an
    /// SEDP match source.
    ///
    /// Called by the `DomainParticipant` constructor exactly once during
    /// setup.
    pub fn attach_ignore_filter(&self, filter: crate::participant::IgnoreFilter) {
        if let Ok(mut guard) = self.ignore_filter.lock() {
            *guard = Some(filter);
        }
    }

    /// Snapshot of the currently wired IgnoreFilter (internal, for
    /// the hot path).
    pub(crate) fn ignore_filter_snapshot(&self) -> Option<crate::participant::IgnoreFilter> {
        self.ignore_filter.lock().ok().and_then(|g| g.clone())
    }

    /// Synchronizes the protected-discovery flag of the `SedpStack` with the
    /// governance (`discovery_protection_kind`). Idempotent, called before every
    /// `announce_*` — so the flag is set correctly regardless of the
    /// order in which security activation and endpoint creation ran.
    #[cfg(feature = "security")]
    fn sync_sedp_discovery_protected(&self, sedp: &mut SedpStack) {
        if let Some(gate) = self.config.security.as_ref() {
            let protected = gate
                .discovery_protection()
                .map(|l| l != ProtectionLevel::None)
                .unwrap_or(false);
            sedp.set_discovery_protected(protected);
        }
    }

    /// Announces a local publication via SEDP. The runtime
    /// sends the generated datagrams immediately to all already-
    /// discovered remote participants.
    ///
    /// # Errors
    /// `WireError` if encoding fails.
    pub fn announce_publication(
        &self,
        data: &zerodds_rtps::publication_data::PublicationBuiltinTopicData,
    ) -> Result<()> {
        // In-process discovery fastpath: put it in the stash so a
        // peer runtime starting later in the same process can pull us
        // via `inproc_snapshot`.
        if let Ok(mut v) = self.announced_pubs.lock() {
            v.push(data.clone());
        }
        // ADR-0006: side-map lookup. If the local user writer has a
        // same-host backend attached (set_shm_locator was
        // called), we inject PID_SHM_LOCATOR into the SEDP
        // sample. Otherwise pure 1:1 spec wire.
        let shm = self.shm_locator(data.key.entity_id);
        let datagrams = {
            let mut sedp = self.sedp.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "sedp poisoned",
            })?;
            // Protected discovery (§8.4.2.4): set robustly before the announce —
            // independent of the order of enable_security_builtins vs.
            // endpoint creation. `discovery_protection_kind != NONE` routes
            // the announce into the secure SEDP writer.
            #[cfg(feature = "security")]
            self.sync_sedp_discovery_protected(&mut sedp);
            let res = if let Some(ref bytes) = shm {
                sedp.announce_publication_with_shm_locator(data, bytes)
            } else {
                sedp.announce_publication(data)
            };
            res.map_err(|_| DdsError::WireError {
                message: alloc::string::String::from("sedp announce_publication"),
            })?
        };
        // Send outside the lock (Rc<Vec<Locator>> is !Send,
        // but we are on the same thread as `self` — no
        // problem).
        for dg in datagrams {
            if let Some(secured) = secure_outbound_bytes(self, &dg.bytes) {
                for t in dg.targets.iter() {
                    if is_routable_user_locator(t) {
                        // §8.3.7: unicast metatraffic (SEDP DATA to the remote
                        // metatraffic_unicast_locator) MUST go out from the metatraffic
                        // recv socket `spdp_unicast`, NOT from the ephemeral
                        // `spdp_mc_tx` — otherwise the peer sees a foreign
                        // source port and sends its reliable ACKNACK/resends
                        // to a dead port (cross-vendor SEDP stall). Identical
                        // to `send_discovery_datagram`.
                        let _ = self.spdp_unicast.send(t, &secured);
                    }
                }
            }
        }
        // In-process discovery fastpath: serve same-process+domain peers
        // synchronously + losslessly with this publication.
        self.inproc_announce_publication(data);
        Ok(())
    }

    /// Announces a local subscription via SEDP. Analogous to
    /// `announce_publication`.
    ///
    /// # Errors
    /// `WireError` if encoding fails.
    pub fn announce_subscription(
        &self,
        data: &zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData,
    ) -> Result<()> {
        if let Ok(mut v) = self.announced_subs.lock() {
            v.push(data.clone());
        }
        let datagrams = {
            let mut sedp = self.sedp.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "sedp poisoned",
            })?;
            #[cfg(feature = "security")]
            self.sync_sedp_discovery_protected(&mut sedp);
            sedp.announce_subscription(data)
                .map_err(|_| DdsError::WireError {
                    message: alloc::string::String::from("sedp announce_subscription"),
                })?
        };
        for dg in datagrams {
            if let Some(secured) = secure_outbound_bytes(self, &dg.bytes) {
                for t in dg.targets.iter() {
                    if is_routable_user_locator(t) {
                        // Source port: metatraffic recv socket, not spdp_mc_tx
                        // (see announce_publication / send_discovery_datagram).
                        let _ = self.spdp_unicast.send(t, &secured);
                    }
                }
            }
        }
        // In-process discovery fastpath: see `announce_publication`.
        self.inproc_announce_subscription(data);
        Ok(())
    }

    /// Re-announces the local SEDP endpoint records (publications +
    /// subscriptions) to a peer whose crypto-token exchange has just
    /// completed. Background: under `rtps_protection`/`discovery_
    /// protection` ZeroDDS wraps the SEDP message-/submessage-protected; the
    /// peer discards the initial SEDP burst UNTIL it has our participant crypto
    /// token (via Volatile). From that moment it can decode — a
    /// one-time re-announce brings the previously dropped SEDP up (mints fresh
    /// SNs; the reliable SEDP writer delivers them, HEARTBEAT/NACK retry covers a
    /// not-quite-ready peer timing). Once per peer (dedup).
    ///
    /// No-op without active rtps_/discovery_protection (then the announce
    /// went through plaintext anyway) and for already re-announced peers. Emits
    /// the RETAINED records directly (NO additional `announced_pubs` push).
    #[cfg(feature = "security")]
    fn re_announce_sedp_to_peer(&self, peer_prefix: GuidPrefix) {
        let Some(gate) = &self.config.security else {
            return;
        };
        let rtps = gate.rtps_protection().unwrap_or(ProtectionLevel::None) != ProtectionLevel::None;
        let disc =
            gate.discovery_protection().unwrap_or(ProtectionLevel::None) != ProtectionLevel::None;
        if !rtps && !disc {
            return;
        }
        // First check whether we have any local endpoints at all — the token
        // exchange can complete BEFORE the user endpoint creation.
        // Without records do NOT mark as "re-announced" (the periodic tick
        // retriggers as soon as the user writer/reader is announced).
        let pubs = self
            .announced_pubs
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        let subs = self
            .announced_subs
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        if pubs.is_empty() && subs.is_empty() {
            return;
        }
        {
            let mut set = match self.sedp_reannounced.write() {
                Ok(s) => s,
                Err(_) => return,
            };
            if !set.insert(peer_prefix.0) {
                return; // already re-announced
            }
        }
        let send_dgs = |dgs: Vec<zerodds_rtps::message_builder::OutboundDatagram>| {
            for dg in dgs {
                if let Some(secured) = secure_outbound_bytes(self, &dg.bytes) {
                    for t in dg.targets.iter() {
                        if is_routable_user_locator(t) {
                            let _ = self.spdp_unicast.send(t, &secured);
                        }
                    }
                }
            }
        };
        for data in &pubs {
            let shm = self.shm_locator(data.key.entity_id);
            let dgs = {
                let Ok(mut sedp) = self.sedp.lock() else {
                    continue;
                };
                self.sync_sedp_discovery_protected(&mut sedp);
                let res = if let Some(ref bytes) = shm {
                    sedp.announce_publication_with_shm_locator(data, bytes)
                } else {
                    sedp.announce_publication(data)
                };
                match res {
                    Ok(d) => d,
                    Err(_) => continue,
                }
            };
            send_dgs(dgs);
        }
        for data in &subs {
            let dgs = {
                let Ok(mut sedp) = self.sedp.lock() else {
                    continue;
                };
                self.sync_sedp_discovery_protected(&mut sedp);
                match sedp.announce_subscription(data) {
                    Ok(d) => d,
                    Err(_) => continue,
                }
            };
            send_dgs(dgs);
        }
    }

    /// Own participant data as a `DiscoveredParticipant` — the
    /// self-view that the in-process fastpath hands to peers.
    fn self_as_discovered_participant(&self) -> zerodds_discovery::spdp::DiscoveredParticipant {
        // From the LIVE SPDP beacon: after `enable_security_builtins_with_auth`
        // it carries the `identity_token` + the secure endpoint bits that the
        // `participant_data` construction snapshot does NOT have. Without these the
        // in-process injected DP is worthless for the security handshake trigger
        // (`handle_remote_endpoints`/`begin_handshake_with` need
        // the token). Fallback to `participant_data` on lock poisoning.
        let data = self
            .spdp_beacon
            .lock()
            .map(|b| b.data.clone())
            .unwrap_or_else(|_| self.participant_data.clone());
        zerodds_discovery::spdp::DiscoveredParticipant {
            sender_prefix: self.guid_prefix,
            sender_vendor: VendorId::ZERODDS,
            data,
        }
    }

    /// In-process discovery: injects the just-announced publication
    /// synchronously into all same-process+domain peer runtimes.
    fn inproc_announce_publication(
        &self,
        data: &zerodds_rtps::publication_data::PublicationBuiltinTopicData,
    ) {
        let peers = crate::inproc::peers(self.domain_id, self.config.spdp_multicast_group);
        let mut dp = None;
        for peer in peers {
            if peer.guid_prefix == self.guid_prefix {
                continue;
            }
            let dp = dp.get_or_insert_with(|| self.self_as_discovered_participant());
            peer.inproc_inject_publication(dp, data);
        }
    }

    /// In-process discovery: injects the just-announced subscription
    /// synchronously into all same-process+domain peer runtimes.
    fn inproc_announce_subscription(
        &self,
        data: &zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData,
    ) {
        let peers = crate::inproc::peers(self.domain_id, self.config.spdp_multicast_group);
        let mut dp = None;
        for peer in peers {
            if peer.guid_prefix == self.guid_prefix {
                continue;
            }
            let dp = dp.get_or_insert_with(|| self.self_as_discovered_participant());
            peer.inproc_inject_subscription(dp, data);
        }
    }

    /// In-process discovery (receive side): wires the remote
    /// participant + injects the publication into the SEDP cache and
    /// matches the local readers. Idempotent — an announcement arriving
    /// later via UDP is thereby a no-op.
    fn inproc_inject_publication(
        self: &Arc<Self>,
        dp: &zerodds_discovery::spdp::DiscoveredParticipant,
        data: &zerodds_rtps::publication_data::PublicationBuiltinTopicData,
    ) {
        // §2.2.2.2.1.17: an ignored publication/participant must not be matched.
        // The in-process fastpath bypasses the wire match path, so the ignore
        // filter must be honored here too — otherwise the Durability-Service's
        // own two participants (ingest + replay, same process) would match and
        // echo-loop despite mutually ignoring each other.
        if let Some(filter) = self.ignore_filter_snapshot() {
            let pub_h = crate::instance_handle::InstanceHandle::from_guid(data.key);
            let part_h = crate::instance_handle::InstanceHandle::from_guid(data.participant_key);
            if filter.is_publication_ignored(pub_h) || filter.is_participant_ignored(part_h) {
                return;
            }
        }
        let now = self.start_instant.elapsed();
        let is_new = self
            .discovered
            .lock()
            .map(|mut c| c.insert(dp.clone()))
            .unwrap_or(false);
        if let Ok(mut sedp) = self.sedp.lock() {
            if is_new {
                sedp.on_participant_discovered(dp);
            }
            sedp.cache_mut().insert_publication(data.clone(), now);
        }
        run_matching_pass(self);
    }

    /// In-process discovery (receive side): like `inproc_inject_publication`
    /// for a subscription.
    fn inproc_inject_subscription(
        self: &Arc<Self>,
        dp: &zerodds_discovery::spdp::DiscoveredParticipant,
        data: &zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData,
    ) {
        // See `inproc_inject_publication`: honor the ignore filter on the
        // in-process fastpath (symmetric, subscription side).
        if let Some(filter) = self.ignore_filter_snapshot() {
            let sub_h = crate::instance_handle::InstanceHandle::from_guid(data.key);
            let part_h = crate::instance_handle::InstanceHandle::from_guid(data.participant_key);
            if filter.is_subscription_ignored(sub_h) || filter.is_participant_ignored(part_h) {
                return;
            }
        }
        let now = self.start_instant.elapsed();
        let is_new = self
            .discovered
            .lock()
            .map(|mut c| c.insert(dp.clone()))
            .unwrap_or(false);
        if let Ok(mut sedp) = self.sedp.lock() {
            if is_new {
                sedp.on_participant_discovered(dp);
            }
            sedp.cache_mut().insert_subscription(data.clone(), now);
        }
        run_matching_pass(self);
    }

    /// Snapshot of our own endpoints for the `pull-on-creation` path
    /// of a peer runtime starting later in the same process.
    fn inproc_snapshot(
        &self,
    ) -> (
        zerodds_discovery::spdp::DiscoveredParticipant,
        Vec<zerodds_rtps::publication_data::PublicationBuiltinTopicData>,
        Vec<zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData>,
    ) {
        let dp = self.self_as_discovered_participant();
        let pubs = self
            .announced_pubs
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        let subs = self
            .announced_subs
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        (dp, pubs, subs)
    }

    /// At runtime creation: ask existing same-process+domain peers
    /// for their already-announced endpoints and inject these into
    /// our SEDP cache. Symmetric counterpart to the
    /// announce hook (which distributes live endpoints to peers).
    fn inproc_pull_from_peers(self: &Arc<Self>) {
        let peers: Vec<Arc<DcpsRuntime>> =
            crate::inproc::peers(self.domain_id, self.config.spdp_multicast_group)
                .into_iter()
                .filter(|rt| rt.guid_prefix != self.guid_prefix)
                .collect();
        for peer in peers {
            let (dp, pubs, subs) = peer.inproc_snapshot();
            for p in &pubs {
                self.inproc_inject_publication(&dp, p);
            }
            for s in &subs {
                self.inproc_inject_subscription(&dp, s);
            }
        }
    }

    /// FU2 S3: in-process counterpart to the security part of
    /// [`handle_spdp_datagram`]. Wires the secure builtin endpoints of the
    /// discovered peer and kicks off — if it announces an `identity_token`
    /// — the auth handshake; the resulting AUTH datagrams
    /// go to the peer via UDP **unicast** (reliable loopback).
    /// No-op without a local security stack or without a peer `identity_token`.
    #[cfg(feature = "security")]
    fn inproc_drive_security_handshake(
        self: &Arc<Self>,
        dp: &zerodds_discovery::spdp::DiscoveredParticipant,
    ) {
        if dp.sender_prefix == self.guid_prefix {
            return;
        }
        let Some(sec) = self.security_builtin_snapshot() else {
            return;
        };
        let dgs = if let Ok(mut s) = sec.lock() {
            s.note_remote_vendor(dp.sender_prefix, dp.sender_vendor);
            s.handle_remote_endpoints(dp);
            match dp.data.identity_token.as_ref() {
                Some(token) => s
                    .begin_handshake_with(dp.sender_prefix, dp.data.guid.to_bytes(), token)
                    .unwrap_or_default(),
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        for dg in dgs {
            send_discovery_datagram(self, &dg.targets, &dg.bytes);
        }
    }

    /// FU2 S3: in-process SPDP **participant** discovery. This was the real
    /// gap — `inproc_inject_publication`/`_subscription` only inject
    /// SEDP endpoints, the SPDP participant level (identity_token +
    /// `begin_handshake_with`) ran EXCLUSIVELY over the multicast path
    /// that is flaky on the codepit LXC. This hook, on activation of the
    /// security builtins, exchanges the participant DPs (with token) **bidirectionally** with
    /// all same-process+domain peers and kicks off the auth handshakes
    /// — deterministically, without a single multicast beacon.
    #[cfg(feature = "security")]
    fn inproc_announce_participant(self: &Arc<Self>) {
        let self_dp = self.self_as_discovered_participant();
        let peers: Vec<Arc<DcpsRuntime>> =
            crate::inproc::peers(self.domain_id, self.config.spdp_multicast_group)
                .into_iter()
                .filter(|rt| rt.guid_prefix != self.guid_prefix)
                .collect();
        for peer in peers {
            // self → peer: the peer discovers US + triggers its handshake.
            let _ = peer
                .discovered
                .lock()
                .map(|mut c| c.insert(self_dp.clone()));
            peer.inproc_drive_security_handshake(&self_dp);
            // peer → self: WE discover the peer + trigger our handshake.
            let peer_dp = peer.self_as_discovered_participant();
            let _ = self
                .discovered
                .lock()
                .map(|mut c| c.insert(peer_dp.clone()));
            self.inproc_drive_security_handshake(&peer_dp);
        }
    }

    /// C1 multicast-free discovery: sends a (possibly already security-
    /// transformed) SPDP beacon additionally to all configured
    /// unicast initial peers. No-op without peers → pure multicast behavior,
    /// no additional syscalls by default.
    fn send_spdp_to_initial_peers(&self, bytes: &[u8]) {
        for peer in &self.config.initial_peers {
            let _ = self.spdp_mc_tx.send(peer, bytes);
        }
    }

    /// FU2 S3: sends an SPDP beacon IMMEDIATELY via multicast, instead of waiting
    /// for the next periodic `spdp_period` tick. Critical for the
    /// cross-process secured handshake: `DcpsRuntime::start` starts the
    /// beacon sender, whose first beacon (token-LESS) goes out BEFORE
    /// `enable_security_builtins_with_auth` sets the `identity_token` on the beacon.
    /// If a peer latches this token-less first beacon, it calls
    /// `begin_handshake_with` with `token=None` → no-op → the handshake NEVER
    /// starts. An immediate re-announce after setting the token ensures
    /// that the first token-carrying beacon goes out promptly.
    #[cfg(feature = "security")]
    fn announce_spdp_now(&self) {
        let mc_target = Locator {
            kind: LocatorKind::UdpV4,
            port: u32::from(
                u16::try_from(spdp_multicast_port(self.domain_id as u32)).unwrap_or(7400),
            ),
            address: {
                let mut a = [0u8; 16];
                a[12..].copy_from_slice(&self.config.spdp_multicast_group.octets());
                a
            },
        };
        if let Ok(mut beacon) = self.spdp_beacon.lock() {
            if let Ok(datagram) = beacon.serialize() {
                if let Some(secured) = secure_outbound_bytes(self, &datagram) {
                    let _ = self.spdp_mc_tx.send(&mc_target, &secured);
                    // C1 multicast-free discovery: on the immediate announce too, to
                    // the configured initial peers (ZERODDS_PEERS).
                    self.send_spdp_to_initial_peers(&secured);
                    // Directed unicast fan-out to already-discovered peers:
                    // covers the order in which we discover a peer
                    // BEFORE our security builtins (token) are active — then the
                    // directed response in handle_spdp_datagram skipped tokenless;
                    // announce_spdp_now() (called by enable() after the token set)
                    // catches up with the tokened beacon promptly + LXC-multicast-
                    // independently. Otherwise the peer waits until spdp_period.
                    for loc in wlp_unicast_targets(&self.discovered_participants()) {
                        let _ = self.spdp_unicast.send(&loc, &secured);
                    }
                }
            }
            // FastDDS interop: additionally announce on the reliable secure SPDP
            // writer (0xff0101c2), so FastDDS sees our full secured
            // participant data over its expected channel.
            if self.config.enable_secure_spdp {
                if let Ok(datagram) = beacon.serialize_secure() {
                    let protected = protect_secure_spdp(self, &datagram).unwrap_or(datagram);
                    if let Some(secured) = secure_outbound_bytes(self, &protected) {
                        let _ = self.spdp_mc_tx.send(&mc_target, &secured);
                    }
                }
            }
        }
    }

    /// FU2 cross-vendor: `EndpointSecurityInfo` (PID_ENDPOINT_SECURITY_INFO,
    /// 0x1004) for user endpoints, derived from the governance
    /// `data_protection`. Foreign vendors (cyclone/FastDDS) reject, with
    /// `data_protection=ENCRYPT`, a user endpoint WITHOUT this PID as
    /// non-secure ("Non secure remote ... not allowed by security").
    /// `None` without an active security gate (plain).
    #[cfg(feature = "security")]
    fn user_endpoint_security_info(
        &self,
    ) -> Option<zerodds_rtps::endpoint_security_info::EndpointSecurityInfo> {
        let gate = self.config.security.as_ref()?;
        let meta = gate.metadata_protection().ok()?;
        let data = gate.data_protection().ok()?;
        let disc = gate.topic_discovery_protected().unwrap_or(false);
        let liv = gate
            .liveliness_protection()
            .map(|l| l != ProtectionLevel::None)
            .unwrap_or(false);
        let rdp = gate.topic_read_protected().unwrap_or(false);
        let wrp = gate.topic_write_protected().unwrap_or(false);
        Some(compute_user_endpoint_attrs(meta, data, disc, liv, rdp, wrp))
    }

    #[cfg(not(feature = "security"))]
    fn user_endpoint_security_info(
        &self,
    ) -> Option<zerodds_rtps::endpoint_security_info::EndpointSecurityInfo> {
        None
    }

    /// Registers a local user writer. The caller gets the
    /// writer `EntityId`; for sends via `write_user_sample(eid, ...)`.
    ///
    /// In the runtime there is **still no** automatic SEDP announce +
    /// matching — that comes in B4b. Currently `register_user_writer`
    /// is just the wiring.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the registry mutex is poisoned.
    pub fn register_user_writer(&self, cfg: UserWriterConfig) -> Result<EntityId> {
        // Default: WithKey. Backward-compat for all test callers.
        self.register_user_writer_kind(cfg, true)
    }

    /// Like [`register_user_writer`] but with an explicit NoKey/WithKey
    /// flag. Cross-vendor interop needs it: if the IDL type has no
    /// `@key`, the writer MUST set `is_keyed=false`, otherwise
    /// a remote reader rejects the DATA submessage due to an
    /// entityKind mismatch (Spec §9.3.1.2 table 9.1: 0x02=WithKey
    /// vs 0x03=NoKey).
    pub fn register_user_writer_kind(
        &self,
        cfg: UserWriterConfig,
        is_keyed: bool,
    ) -> Result<EntityId> {
        let now = self.start_instant.elapsed();
        let key = self.next_entity_key();
        let eid = if is_keyed {
            EntityId::user_writer_with_key(key)
        } else {
            EntityId::user_writer_no_key(key)
        };
        let writer = ReliableWriter::new(ReliableWriterConfig {
            guid: Guid::new(self.guid_prefix, eid),
            vendor_id: VendorId::ZERODDS,
            reader_proxies: Vec::new(),
            max_samples: 1024,
            history_kind: HistoryKind::KeepLast { depth: 32 },
            heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
            // Ethernet-safe default; the value is raised at the reader match
            // if all readers are same-host (see the
            // set_fragmentation call after add_reader_proxy).
            fragment_size: DEFAULT_FRAGMENT_SIZE,
            mtu: DEFAULT_MTU,
        });
        let mut pub_data = build_publication_data(
            self.guid_prefix,
            eid,
            &cfg,
            &self.config.data_representation_offer,
            self.user_announce_locator,
        );
        // FU2 cross-vendor: EndpointSecurityInfo from the governance
        // data_protection — otherwise cyclone/FastDDS reject the user endpoint
        // with data_protection=ENCRYPT as non-secure.
        pub_data.security_info = self.user_endpoint_security_info();
        self.user_writers
            .write()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "user_writers poisoned",
            })?
            .insert(
                eid,
                Arc::new(Mutex::new(UserWriterSlot {
                    writer,
                    topic_name: cfg.topic_name.clone(),
                    type_name: cfg.type_name.clone(),
                    reliable: cfg.reliable,
                    durability: cfg.durability,
                    deadline_nanos: qos_duration_to_nanos(cfg.deadline.period),
                    // Initial `None`: the deadline window starts only on the
                    // first real write. Prevents false misses due to
                    // slow entity setup (e.g. Linux CI container)
                    // before the app does its first write(). On the
                    // first write() `last_write = Some(now)` is set,
                    // and from then the deadline counter ticks.
                    last_write: None,
                    offered_deadline_missed_count: 0,
                    liveliness_lost_count: 0,
                    last_liveliness_assert: Some(now),
                    offered_incompatible_qos: crate::status::OfferedIncompatibleQosStatus::default(
                    ),
                    lifespan_nanos: qos_duration_to_nanos(cfg.lifespan.duration),
                    sample_insert_times: alloc::collections::VecDeque::new(),
                    liveliness_kind: cfg.liveliness.kind,
                    liveliness_lease_nanos: qos_duration_to_nanos(cfg.liveliness.lease_duration),
                    ownership: cfg.ownership,
                    ownership_strength: cfg.ownership_strength,
                    partition: cfg.partition.clone(),
                    #[cfg(feature = "security")]
                    reader_protection: BTreeMap::new(),
                    #[cfg(feature = "security")]
                    locator_to_peer: BTreeMap::new(),
                    type_identifier: cfg.type_identifier.clone(),
                    data_rep_offer_override: cfg.data_representation_offer.clone(),
                    // Default FINAL: irrelevant for XCDR1 (default offer)
                    // (final==appendable==CDR_LE), correct for XCDR2 for
                    // @final types. Appendable/mutable types set this later via
                    // set_user_writer_wire_extensibility.
                    wire_extensibility: zerodds_types::qos::ExtensibilityForRepr::Final,
                    big_endian_override: false,
                    durability_backend: None,
                    backend_primed: false,
                    history_depth: DEFAULT_INTRA_HISTORY_DEPTH,
                    retained: alloc::collections::VecDeque::new(),
                    intra_replayed_readers: alloc::collections::BTreeSet::new(),
                })),
            );
        // FIRST match locally, THEN announce — symmetric to
        // register_user_reader_kind. Avoids a peer-side match
        // triggered by our announce_publication
        // starting a data flow to us before we have wired the
        // ReaderProxies.
        self.match_local_writer_against_cache(eid);
        let _ = self.announce_publication(&pub_data);
        // Intra-runtime routing: scan local readers for a match on
        // (topic, type). Applies to bridge daemons with writer+reader in
        // the same runtime (WS/MQTT/CoAP/AMQP bridges). Without this
        // route the local reader gets no samples from the local
        // writer — the `inproc` fastpath explicitly skips self, UDP loopback
        // is not guaranteed, and SEDP match paths go via
        // the discovered cache, which does not contain self.
        self.recompute_intra_runtime_routes();
        // FU2 F-ECHO-WRITE: a user writer created AFTER handshake completion
        // (e.g. the event-driven echo writer in the responder/pong) must send its
        // per-endpoint datawriter_crypto_tokens IMMEDIATELY to the already-
        // authenticated peers — not only on the next tick. Otherwise
        // cyclone's reader stays in "waiting for approval by security" beyond
        // its match deadline (the event-driven pong may not tick
        // in time) → flaky sub=0. Idempotent via endpoint_tokens_sent dedup.
        #[cfg(feature = "security")]
        self.flush_late_endpoint_tokens();
        // Observability event.
        self.config.observability.record(
            &zerodds_foundation::observability::Event::new(
                zerodds_foundation::observability::Level::Info,
                zerodds_foundation::observability::Component::Dcps,
                "user_writer.created",
            )
            .with_attr("topic", cfg.topic_name.as_str())
            .with_attr("type", cfg.type_name.as_str())
            .with_attr("reliable", if cfg.reliable { "true" } else { "false" }),
        );
        Ok(eid)
    }

    /// FU2 F-ECHO-WRITE: sends pending per-endpoint crypto tokens IMMEDIATELY to all
    /// already-authenticated peers. For user endpoints created AFTER handshake
    /// completion (event-driven echo writer in the responder): their token
    /// must go out before cyclone's reader match deadline expires — the periodic
    /// tick (or a VolatileSecure recv) is otherwise possibly too late. Idempotent
    /// via `endpoint_tokens_sent` dedup (double-send with the tick excluded).
    #[cfg(feature = "security")]
    fn flush_late_endpoint_tokens(&self) {
        let Some(stack) = self.security_builtin_snapshot() else {
            return;
        };
        let Ok(mut s) = stack.lock() else {
            return;
        };
        let now = self.start_instant.elapsed();
        let peers: alloc::vec::Vec<GuidPrefix> = self
            .config
            .security
            .as_ref()
            .map(|g| {
                g.authenticated_peer_prefixes()
                    .into_iter()
                    .map(GuidPrefix::from_bytes)
                    .collect()
            })
            .unwrap_or_default();
        for prefix in peers {
            let already = self
                .endpoint_tokens_sent
                .read()
                .map(|set| set.clone())
                .unwrap_or_default();
            let pending =
                pending_endpoint_tokens(prepare_endpoint_crypto_tokens(self, prefix), &already);
            for ep_msg in pending {
                let key = endpoint_token_key(&ep_msg);
                let dgs = protect_volatile_outbound(
                    self,
                    prefix,
                    s.volatile_writer
                        .write_with_heartbeat(&ep_msg, now)
                        .unwrap_or_default(),
                );
                for dg in dgs {
                    for t in dg.targets.iter() {
                        let _ = self.spdp_unicast.send(t, &dg.bytes);
                    }
                }
                if let Ok(mut set) = self.endpoint_tokens_sent.write() {
                    set.insert(key);
                }
            }
            // Periodic re-announce retrigger: as soon as the user writer/reader
            // is announced (announced_pubs/subs not empty), this catches up the
            // SEDP initially dropped under rtps_/discovery_protection to this
            // (now tokened) peer. Once per peer (dedup in the method).
            self.re_announce_sedp_to_peer(prefix);
        }
    }

    /// Spec §2.2.3.5 — registers a durability-service backend on
    /// a writer already registered via [`register_user_writer`].
    /// With Durability=Transient/Persistent the backend is replayed into the
    /// HistoryCache on the first late-joiner match in
    /// `wire_writer_to_remote_reader`, so the reader gets all samples —
    /// including those no longer in the writer cache due to history eviction
    /// or those that have survived a writer restart.
    pub fn attach_durability_backend(
        &self,
        eid: EntityId,
        backend: alloc::sync::Arc<dyn crate::durability_service::DurabilityBackend>,
    ) -> Result<()> {
        let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
            what: "attach_durability_backend: unknown writer entity id",
        })?;
        let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "user_writer slot poisoned",
        })?;
        slot.durability_backend = Some(backend);
        slot.backend_primed = false;
        Ok(())
    }

    /// Sets the type extensibility of a writer (FINAL/APPENDABLE/
    /// MUTABLE). Affects exclusively the encapsulation header
    /// of the user payload (see [`user_payload_encap`]) — relevant for
    /// XCDR2 wire, where @appendable requires a `D_CDR2_LE` and @mutable a
    /// `PL_CDR2_LE` header. The codegen/FFI calls this after
    /// `register_user_writer*` when the type is not @final.
    /// Does NOT change the SEDP announce offer list.
    ///
    /// # Errors
    /// `BadParameter` on an unknown EntityId, `PreconditionNotMet` on a
    /// poisoned slot mutex.
    pub fn set_user_writer_wire_extensibility(
        &self,
        eid: EntityId,
        ext: zerodds_types::qos::ExtensibilityForRepr,
    ) -> Result<()> {
        let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
            what: "set_user_writer_wire_extensibility: unknown writer entity id",
        })?;
        let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "user_writer slot poisoned",
        })?;
        slot.wire_extensibility = ext;
        Ok(())
    }

    /// Registers a local user reader. Returns the reader EntityId
    /// and an `mpsc::Receiver` through which DataReader handles
    /// consume incoming samples.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the registry mutex is poisoned.
    /// Registers a user reader. Returns the EntityId and an
    /// `mpsc::Receiver<UserSample>` — alive samples deliver payload,
    /// lifecycle markers carry key hash + ChangeKind.
    pub fn register_user_reader(
        &self,
        cfg: UserReaderConfig,
    ) -> Result<(EntityId, mpsc::Receiver<UserSample>)> {
        // Default: WithKey. Backward-compat for all test callers.
        self.register_user_reader_kind(cfg, true)
    }

    /// Like [`register_user_reader`] but with an explicit NoKey/WithKey
    /// flag. Symmetric to [`register_user_writer_kind`] — the reader kind
    /// must match the writer kind.
    pub fn register_user_reader_kind(
        &self,
        cfg: UserReaderConfig,
        is_keyed: bool,
    ) -> Result<(EntityId, mpsc::Receiver<UserSample>)> {
        let now = self.start_instant.elapsed();
        let key = self.next_entity_key();
        let eid = if is_keyed {
            EntityId::user_reader_with_key(key)
        } else {
            EntityId::user_reader_no_key(key)
        };
        let reader = ReliableReader::new(ReliableReaderConfig {
            guid: Guid::new(self.guid_prefix, eid),
            vendor_id: VendorId::ZERODDS,
            writer_proxies: Vec::new(),
            max_samples_per_proxy: 256,
            // D.5e: 0ms = synchronous ACK response (Cyclone parity).
            // Previously 200ms = pre-1.0 default without spec justification.
            heartbeat_response_delay:
                zerodds_rtps::reliable_reader::DEFAULT_HEARTBEAT_RESPONSE_DELAY,
            // C3: ROS-realistic reassembly cap (PointCloud2/Image),
            // instead of the conservative rtps 1-MiB default.
            assembler_caps: AssemblerCaps {
                max_sample_bytes: self.config.max_reassembly_sample_bytes,
                ..AssemblerCaps::default()
            },
        });
        // BEST_EFFORT readers must not block delivery on a leading sequence-number
        // gap (RTPS §8.4.12.1) — they don't NACK to repair it, so waiting would
        // deadlock against e.g. an OpenDDS/RTI writer whose first sample to us is
        // mid-stream. Reliable readers keep strict in-order delivery.
        let mut reader = reader;
        reader.set_best_effort(!cfg.reliable);
        let (tx, rx) = mpsc::channel();
        // A DataReader announces every representation it can decode (XCDR2 +
        // XCDR1), not just the writer-preferred one — see `reader_accept_repr`.
        let reader_repr = reader_accept_repr(&self.config.data_representation_offer);
        let mut sub_data = build_subscription_data(
            self.guid_prefix,
            eid,
            &cfg,
            &reader_repr,
            self.user_announce_locator,
        );
        // FU2 cross-vendor: EndpointSecurityInfo from the governance (see writer).
        sub_data.security_info = self.user_endpoint_security_info();
        self.user_readers
            .write()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "user_readers poisoned",
            })?
            .insert(
                eid,
                Arc::new(Mutex::new(UserReaderSlot {
                    reader,
                    topic_name: cfg.topic_name.clone(),
                    type_name: cfg.type_name.clone(),
                    sample_tx: tx,
                    async_waker: Arc::new(std::sync::Mutex::new(None)),
                    listener: None,
                    durability: cfg.durability,
                    deadline_nanos: qos_duration_to_nanos(cfg.deadline.period),
                    // Start time as reference (see register_user_writer).
                    last_sample_received: Some(now),
                    requested_deadline_missed_count: 0,
                    requested_incompatible_qos:
                        crate::status::RequestedIncompatibleQosStatus::default(),
                    sample_lost_count: 0,
                    sample_rejected: crate::status::SampleRejectedStatus::default(),
                    samples_delivered_count: 0,
                    liveliness_lease_nanos: qos_duration_to_nanos(cfg.liveliness.lease_duration),
                    liveliness_kind: cfg.liveliness.kind,
                    liveliness_alive_count: 0,
                    liveliness_not_alive_count: 0,
                    // Optimistic init: we see the writer via SEDP,
                    // until the lease expires it counts as alive.
                    liveliness_alive: true,
                    liveliness_alive_writers: alloc::collections::BTreeSet::new(),
                    ownership: cfg.ownership,
                    partition: cfg.partition.clone(),
                    writer_strengths: alloc::collections::BTreeMap::new(),
                    type_identifier: cfg.type_identifier.clone(),
                    type_consistency: cfg.type_consistency,
                    // A2 — TIME_BASED_FILTER off by default; the C-FFI/rmw path
                    // arms it via `set_user_reader_time_based_filter`.
                    tbf_min_separation_nanos: 0,
                    tbf_last_delivered: alloc::collections::BTreeMap::new(),
                })),
            );
        // FIRST match locally (create the writer proxy on the reader),
        // THEN announce. Otherwise our announce_subscription triggers a
        // backend replay at the peer via the in-process fastpath
        // (Spec §2.2.3.5), which injects DATA into *our* reader
        // before we have wired the matching WriterProxies — the
        // samples are then discarded as unknown-source
        // (tests `{transient,persistent}_late_joiner_receives_backend_replay`).
        self.match_local_reader_against_cache(eid);
        let _ = self.announce_subscription(&sub_data);
        // Intra-runtime routing: see `register_user_writer_kind`.
        self.recompute_intra_runtime_routes();
        // Observability event.
        self.config.observability.record(
            &zerodds_foundation::observability::Event::new(
                zerodds_foundation::observability::Level::Info,
                zerodds_foundation::observability::Component::Dcps,
                "user_reader.created",
            )
            .with_attr("topic", cfg.topic_name.as_str())
            .with_attr("type", cfg.type_name.as_str()),
        );
        Ok((eid, rx))
    }

    /// Rebuilds the same-runtime writer→reader routing table.
    /// Called in `register_user_writer_kind` and `register_user_reader_kind`
    /// after every endpoint create. Per local writer it collects
    /// all local readers that have exactly the same `topic_name`
    /// and `type_name`. The lookup in the write hot path
    /// (`write_user_sample_borrowed`) is read-locked and cheap
    /// (BTreeMap lookup → Vec clone). On endpoint removal (TODO: not
    /// yet hooked everywhere) this would be called too.
    fn recompute_intra_runtime_routes(&self) {
        let writer_snap = self.writer_slots_snapshot();
        let reader_snap = self.reader_slots_snapshot();
        let mut new_map: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        // QR-cluster (b): writers whose route gained a new reader and which are
        // TransientLocal must replay their retained history to those readers.
        // (writer_eid, reader_eid) pairs collected here, replayed after the
        // routing lock is released.
        let mut replay_targets: Vec<(EntityId, EntityId)> = Vec::new();
        for (writer_eid, w_arc) in writer_snap {
            let (w_topic, w_type, w_partition, w_transient_local) = match w_arc.lock() {
                Ok(s) => (
                    s.topic_name.clone(),
                    s.type_name.clone(),
                    s.partition.clone(),
                    !matches!(s.durability, zerodds_qos::DurabilityKind::Volatile),
                ),
                Err(_) => continue,
            };
            let mut readers: Vec<EntityId> = Vec::new();
            for (reader_eid, r_arc) in &reader_snap {
                let matches = match r_arc.lock() {
                    Ok(s) => {
                        s.topic_name == w_topic
                            && s.type_name == w_type
                            // QR-cluster (c): PARTITION gates the same-runtime
                            // match exactly as on the wire (DDS 1.4 §2.2.3.13).
                            && partitions_overlap(&w_partition, &s.partition)
                    }
                    Err(_) => false,
                };
                if matches {
                    readers.push(*reader_eid);
                    // Schedule a TransientLocal replay if this writer has not yet
                    // replayed to this reader.
                    if w_transient_local {
                        let already = w_arc
                            .lock()
                            .map(|s| s.intra_replayed_readers.contains(reader_eid))
                            .unwrap_or(true);
                        if !already {
                            replay_targets.push((writer_eid, *reader_eid));
                        }
                    }
                }
            }
            if !readers.is_empty() {
                new_map.insert(writer_eid, readers);
            }
        }
        // Perform the TransientLocal retained-sample replay to each new reader
        // (DDS 1.4 §2.2.3.4 late-joiner delivery). Done outside the routing
        // lock; the per-writer slot lock guards `retained` + the
        // already-replayed dedup set.
        for (writer_eid, reader_eid) in replay_targets {
            self.intra_runtime_replay_retained(writer_eid, reader_eid);
        }
        let changed = match self.intra_runtime_routes.write() {
            Ok(mut g) => {
                let changed = *g != new_map;
                *g = new_map;
                changed
            }
            Err(_) => false,
        };
        // A new/changed intra-runtime route is a same-participant
        // match → wake the `wait_for_matched_{subscription,publication}` waiter
        // (the matched count now includes these routes).
        if changed {
            self.match_event.1.notify_all();
        }
    }

    /// QR-cluster (b): replays a TransientLocal writer's retained samples
    /// (DDS 1.4 §2.2.3.4) to a single late-joining intra-runtime reader. Each
    /// retained Alive entry is delivered as `UserSample::Alive`; each terminal
    /// lifecycle entry as `UserSample::Lifecycle`, so the reader's instance
    /// state reflects the most recent NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS.
    /// Idempotent via the per-writer `intra_replayed_readers` set.
    fn intra_runtime_replay_retained(&self, writer_eid: EntityId, reader_eid: EntityId) {
        // Snapshot retained under the writer lock, mark replayed, then release.
        let samples: Vec<RetainedSample> = {
            let Some(w_arc) = self.writer_slot(writer_eid) else {
                return;
            };
            let Ok(mut w) = w_arc.lock() else {
                return;
            };
            if !w.intra_replayed_readers.insert(reader_eid) {
                return; // already replayed to this reader
            }
            w.retained.iter().cloned().collect()
        };
        if samples.is_empty() {
            return;
        }
        let Some(r_arc) = self.reader_slot(reader_eid) else {
            return;
        };
        let writer_guid = Guid::new(self.guid_prefix, writer_eid).to_bytes();
        let (listener, waker, sender) = {
            let Ok(r) = r_arc.lock() else {
                return;
            };
            (
                r.listener.clone(),
                Arc::clone(&r.async_waker),
                r.sample_tx.clone(),
            )
        };
        for s in samples {
            match s.lifecycle {
                None => {
                    if let Some(l) = &listener {
                        // Durability replay is little-endian (the store does not
                        // retain the original byte order) → big_endian = 0.
                        l(&s.payload, s.representation, 0);
                    } else {
                        let sample = UserSample::Alive {
                            payload: crate::sample_bytes::SampleBytes::from_vec(s.payload.clone()),
                            writer_guid,
                            writer_strength: s.strength,
                            representation: s.representation,
                            // Durability replay: the store does not yet retain
                            // the original byte order (ZeroDDS-internal samples
                            // are little-endian); a big-endian peer's durable
                            // sample would replay LE. Tracked as a durability
                            // followup, not part of the live RTPS BE path.
                            big_endian: false,
                            // Durability replay: original source timestamp not
                            // retained in the store today → reception order.
                            source_timestamp: None,
                        };
                        let _ = sender.send(sample);
                        wake_async_waker(&waker);
                    }
                }
                Some(kind) => {
                    // Lifecycle markers always go to the MPSC channel (the
                    // alive-only listener does not carry instance state).
                    let _ = sender.send(UserSample::Lifecycle {
                        key_hash: s.key_hash,
                        kind,
                    });
                    wake_async_waker(&waker);
                }
            }
        }
    }

    /// QR-cluster (d): delivers a lifecycle marker (dispose / unregister) to all
    /// matched intra-runtime readers (DDS 1.4 §2.2.2.4.2.10 / §2.2.2.4.2.7) so
    /// their instance state becomes NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS,
    /// and records it in the writer's retained buffer so a later late joiner
    /// also observes the terminal state. The wire path is handled separately by
    /// [`Self::write_user_lifecycle`].
    fn intra_runtime_dispatch_lifecycle(
        &self,
        writer_eid: EntityId,
        key_hash: [u8; 16],
        kind: zerodds_rtps::history_cache::ChangeKind,
    ) {
        // Record in retained (terminal marker for the key) so future late
        // joiners observe the NOT_ALIVE state.
        if let Some(w_arc) = self.writer_slot(writer_eid) {
            if let Ok(mut w) = w_arc.lock() {
                if !matches!(w.durability, zerodds_qos::DurabilityKind::Volatile) {
                    // Replace any prior terminal marker for this key; keep the
                    // retained alive samples (the reader saw them already, but a
                    // brand-new late joiner needs both the data and the state).
                    w.retained
                        .retain(|s| !(s.lifecycle.is_some() && s.key_hash == key_hash));
                    w.retained.push_back(RetainedSample {
                        key_hash,
                        payload: Vec::new(),
                        representation: 0,
                        strength: 0,
                        lifecycle: Some(kind),
                    });
                }
            }
        }
        let routes: Vec<EntityId> = match self.intra_runtime_routes.read() {
            Ok(g) => match g.get(&writer_eid) {
                Some(v) => v.clone(),
                None => return,
            },
            Err(_) => return,
        };
        for reader_eid in routes {
            let Some(slot_arc) = self.reader_slot(reader_eid) else {
                continue;
            };
            let (waker, sender) = {
                let Ok(slot) = slot_arc.lock() else {
                    continue;
                };
                (Arc::clone(&slot.async_waker), slot.sample_tx.clone())
            };
            let _ = sender.send(UserSample::Lifecycle { key_hash, kind });
            wake_async_waker(&waker);
        }
    }

    /// Same-runtime direct dispatch: pushes the just-written
    /// sample directly into the `sample_tx` channel of all local readers
    /// on the same topic+type. Avoids an RTPS wire roundtrip + UDP
    /// loopback for the bridge-daemon case (writer+reader in the same
    /// `DcpsRuntime`). Called by the write hot path after the normal
    /// wire dispatch.
    fn intra_runtime_dispatch_alive(
        &self,
        writer_eid: EntityId,
        payload: &[u8],
        writer_strength: i32,
        // XCDR version tag of the writer's effective offer (`0` = XCDR1,
        // `1` = XCDR2), matching `encap_representation`'s convention on the
        // wire-receive path. On the wire path the reader recovers this from
        // byte[1] of the encap header; here the intra-runtime payload carries
        // no encap header, so the writer's actual representation must be
        // threaded through explicitly (Bug R4 — previously hardcoded `0`,
        // losing the XCDR version on the same-runtime loopback path).
        representation: u8,
    ) {
        let routes: Vec<EntityId> = match self.intra_runtime_routes.read() {
            Ok(g) => match g.get(&writer_eid) {
                Some(v) => v.clone(),
                None => return,
            },
            Err(_) => return,
        };
        if routes.is_empty() {
            return;
        }
        let writer_guid = Guid::new(self.guid_prefix, writer_eid).to_bytes();
        // QR-cluster (e): LIVELINESS AUTOMATIC auto-renew. A delivered sample
        // proves the matched writer is alive (DDS 1.4 §2.2.3.11). For AUTOMATIC
        // kind the infrastructure renews liveliness implicitly, so each
        // intra-runtime delivery marks the writer alive at the reader.
        let writer_liveliness_automatic = self
            .writer_slot(writer_eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.liveliness_kind))
            .map(|k| matches!(k, zerodds_qos::LivelinessKind::Automatic))
            .unwrap_or(false);
        for reader_eid in routes {
            let Some(slot_arc) = self.reader_slot(reader_eid) else {
                continue;
            };
            // Hold the slot lock only for the listener/sender clone, dispatch
            // outside (symmetric to the data-receive path above, which
            // preserves exactly the same order in the DATA arm).
            let listener;
            let waker;
            let sender;
            {
                let Ok(mut slot) = slot_arc.lock() else {
                    continue;
                };
                // Liveliness renew: bump alive_count once per writer-alive
                // transition (the reader sees this writer become alive).
                if writer_liveliness_automatic {
                    let newly_alive = slot.liveliness_alive_writers.insert(writer_guid);
                    if newly_alive {
                        slot.liveliness_alive = true;
                        slot.liveliness_alive_count = slot.liveliness_alive_count.saturating_add(1);
                    }
                }
                listener = slot.listener.clone();
                waker = Arc::clone(&slot.async_waker);
                sender = slot.sample_tx.clone();
            }
            // Listener and MPSC are exclusive (see the data-arm comment):
            // if a listener is set, the sample only goes to it;
            // otherwise to the MPSC receiver.
            if let Some(l) = listener {
                // The listener signature is `(payload, representation, big_endian)`.
                // Intra-runtime: no encap header, so carry the writer's
                // actual representation tag (Bug R4); same-process delivery is
                // always native little-endian → big_endian = 0.
                l(payload, representation, 0);
            } else {
                let sample = UserSample::Alive {
                    payload: crate::sample_bytes::SampleBytes::from_vec(payload.to_vec()),
                    writer_guid,
                    writer_strength,
                    representation,
                    // Intra-runtime same-process delivery always produces the
                    // native little-endian wire.
                    big_endian: false,
                    // Intra-runtime same-process delivery bypasses the INFO_TS
                    // wire path → reception order.
                    source_timestamp: None,
                };
                let _ = sender.send(sample);
                wake_async_waker(&waker);
            }
        }
    }

    /// On registration / SEDP event: for a local writer `eid`
    /// go through all subscriptions known in the cache; on a topic+type
    /// match add a `ReaderProxy` to the local ReliableWriter.
    fn match_local_writer_against_cache(&self, eid: EntityId) {
        let (topic, type_name) = {
            let Some(arc) = self.writer_slot(eid) else {
                return;
            };
            let Ok(s) = arc.lock() else {
                return;
            };
            (s.topic_name.clone(), s.type_name.clone())
        };
        let (matches, conflict): (Vec<_>, bool) = {
            let sedp = match self.sedp.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let matches = sedp
                .cache()
                .match_subscriptions(&topic, &type_name)
                .map(|s| s.data.clone())
                .collect();
            let conflict = sedp.cache().topic_name_conflicts(&topic, &type_name);
            (matches, conflict)
        };
        if conflict {
            self.inconsistent_topic_seq.fetch_add(1, Ordering::Relaxed);
        }
        for sub in matches {
            self.wire_writer_to_remote_reader(eid, &sub);
        }
    }

    fn match_local_reader_against_cache(&self, eid: EntityId) {
        let (topic, type_name) = {
            let Some(arc) = self.reader_slot(eid) else {
                return;
            };
            let Ok(s) = arc.lock() else {
                return;
            };
            (s.topic_name.clone(), s.type_name.clone())
        };
        let (matches, conflict): (Vec<_>, bool) = {
            let sedp = match self.sedp.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let matches = sedp
                .cache()
                .match_publications(&topic, &type_name)
                .map(|p| p.data.clone())
                .collect();
            let conflict = sedp.cache().topic_name_conflicts(&topic, &type_name);
            (matches, conflict)
        };
        if conflict {
            self.inconsistent_topic_seq.fetch_add(1, Ordering::Relaxed);
        }
        for pubd in matches {
            self.wire_reader_to_remote_writer(eid, &pubd);
        }
    }

    fn wire_writer_to_remote_reader(
        &self,
        writer_eid: EntityId,
        sub: &zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData,
    ) {
        // §2.2.2.2.1.16: an ignored subscription must not be MATCHED (symmetric
        // to the publication gate in `wire_reader_to_remote_writer`). The
        // Durability-Service ignores its own ingest reader here so the replay
        // writer never delivers back to it (echo loop).
        if let Some(filter) = self.ignore_filter_snapshot() {
            let sub_h = crate::instance_handle::InstanceHandle::from_guid(sub.key);
            let part_h = crate::instance_handle::InstanceHandle::from_guid(sub.participant_key);
            if filter.is_subscription_ignored(sub_h) || filter.is_participant_ignored(part_h) {
                return;
            }
        }
        let locators =
            endpoint_or_default_locators(&sub.unicast_locators, sub.key.prefix, &self.discovered);
        if locators.is_empty() {
            return;
        }
        // Backend replay datagrams (Spec §2.2.3.5). Sent after
        // the slot-lock release, so the send path does not run under
        // the slot mutex.
        let mut replay_dgs: Vec<zerodds_rtps::message_builder::OutboundDatagram> = Vec::new();
        if let Some(slot_arc) = self.writer_slot(writer_eid) {
            if let Ok(mut slot) = slot_arc.lock() {
                let slot = &mut *slot;
                // Idempotency gate: if a ReaderProxy already exists for this
                // remote reader, the match has already run
                // once. A re-wire (e.g. when the SEDP announcement
                // arrives at the writer both via the in-process fastpath and via UDP)
                // would REPLACE the proxy via
                // `add_reader_proxy` — and thereby reset
                // `highest_acked_sn`/`highest_sent_sn`.
                // The next tick then emits an invalid HEARTBEAT
                // with `first_sn > last_sn` (cache_min=N, highest_acked+1=N+1),
                // the reader interprets this as "everything before first_sn is
                // lost" and advances `delivered_up_to` past not-yet-
                // delivered backend replay samples (tests
                // `{transient,persistent}_late_joiner_receives_backend_replay`
                // — 3% flake without the gate).
                if slot
                    .writer
                    .reader_proxies()
                    .iter()
                    .any(|p| p.remote_reader_guid == sub.key)
                {
                    return;
                }
                // --- QoS-Compatibility ---
                // Spec OMG DDS 1.4 §2.2.3.6: Writer offered >= Reader requested.
                //
                // Per reject, bump the responsible policy ID in
                // `offered_incompatible_qos.policies`, so the
                // DataWriter listener is triggered via `dispatch_offered_incompatible_qos`.
                // We track the *first* faulty
                // policy as `last_policy_id` (Spec §2.2.4.1: most-recent).
                use crate::psm_constants::qos_policy_id as qid;
                use crate::status::bump_policy_count;
                // C2 "loud instead of silent": an incompatible QoS match is
                // not only kept as a pollable status (Spec §2.2.4.1),
                // but logged loudly IMMEDIATELY. The central ROS-DDS
                // pain point is that QoS mismatches are silently discarded
                // (e.g. Cyclone's `DDS_INVALID_QOS_POLICY_ID` without a
                // log) — exactly that made the ROS-2 entityKind diagnosis so
                // expensive. The reject names the topic, remote reader and
                // the exact policy.
                let obs = self.config.observability.clone();
                let topic_for_log = slot.topic_name.clone();
                let remote_for_log = alloc::format!("{:?}", sub.key);
                let bump = |slot: &mut UserWriterSlot, pid: u32| {
                    slot.offered_incompatible_qos.total_count =
                        slot.offered_incompatible_qos.total_count.saturating_add(1);
                    slot.offered_incompatible_qos.last_policy_id = pid;
                    bump_policy_count(&mut slot.offered_incompatible_qos.policies, pid);
                    obs.record(
                        &zerodds_foundation::observability::Event::new(
                            zerodds_foundation::observability::Level::Warn,
                            zerodds_foundation::observability::Component::Dcps,
                            "qos.incompatible.offered",
                        )
                        .with_attr("topic", topic_for_log.as_str())
                        .with_attr("remote_reader", remote_for_log.as_str())
                        .with_attr("policy", qos_policy_id_name(pid)),
                    );
                };

                // Durability rank: Volatile < TransientLocal < Transient <
                // Persistent. The writer may offer more than the reader requests.
                if (slot.durability as u8) < (sub.durability as u8) {
                    bump(slot, qid::DURABILITY);
                    return;
                }
                // Deadline: writer period <= reader period (the writer promises
                // to write faster than the reader expects).
                if !deadline_compat(
                    slot.deadline_nanos,
                    qos_duration_to_nanos(sub.deadline.period),
                ) {
                    bump(slot, qid::DEADLINE);
                    return;
                }
                // Liveliness-Kind: Automatic < ManualByParticipant < ManualByTopic.
                // Writer-Kind >= Reader-Kind. Lease: writer.lease <= reader.lease.
                if (slot.liveliness_kind as u8) < (sub.liveliness.kind as u8) {
                    bump(slot, qid::LIVELINESS);
                    return;
                }
                if !deadline_compat(
                    slot.liveliness_lease_nanos,
                    qos_duration_to_nanos(sub.liveliness.lease_duration),
                ) {
                    bump(slot, qid::LIVELINESS);
                    return;
                }
                // Ownership: both must be equal (Spec §2.2.3.6 Table:
                // no "compatible" case except exactly equal).
                if slot.ownership != sub.ownership {
                    bump(slot, qid::OWNERSHIP);
                    return;
                }
                // Partition: at least one common partition — or
                // both empty (default partition "").
                if !partitions_overlap(&slot.partition, &sub.partition) {
                    bump(slot, qid::PARTITION);
                    return;
                }
                // F-TYPES-3 XTypes-1.3 §7.6.3.7 symmetric writer-side check.
                // If both sides carry a TypeIdentifier (≠ None),
                // we check compatibility. The reader's TCE policy is not
                // directly available here; we take the default TCE
                // (AllowTypeCoercion without PreventWidening) — the reader-
                // side check in `wire_reader_to_remote_writer` validates
                // with the real reader TCE.
                if slot.type_identifier != zerodds_types::TypeIdentifier::None
                    && sub.type_identifier != zerodds_types::TypeIdentifier::None
                    // Equal TypeIdentifiers are by definition the same type
                    // (XTypes 1.3 §7.2.4.1 identity). This is the typed-endpoint
                    // case: writer + reader of the same generated type carry the
                    // same (possibly complete) TypeIdentifier, whose TypeObject
                    // is NOT in this fresh registry. Without this short-circuit a
                    // complete-hash type-id would fail the assignability lookup
                    // (Bug QT). Skip the registry-backed structural check when the
                    // ids are identical.
                    && slot.type_identifier != sub.type_identifier
                {
                    let registry = zerodds_types::resolve::TypeRegistry::new();
                    let tce = zerodds_types::qos::TypeConsistencyEnforcement::default();
                    let matcher = zerodds_types::type_matcher::TypeMatcher::new(&tce);
                    if !matcher
                        .match_types(&slot.type_identifier, &sub.type_identifier, &registry)
                        .is_match()
                    {
                        bump(slot, qid::TYPE_CONSISTENCY_ENFORCEMENT);
                        return;
                    }
                }

                let mut proxy = zerodds_rtps::reader_proxy::ReaderProxy::new(
                    sub.key,
                    locators.clone(),
                    Vec::new(),
                    slot.reliable,
                );
                // D.5g — Per-Peer DataRepresentation negotiation
                // (XTypes 1.3 §7.6.3.1.2). Writer-offered = Per-Writer-
                // Override (slot.data_rep_offer_override) ODER Runtime-
                // Default. Reader-accepted = sub.data_representation
                // (spec default `[XCDR1]` if empty). Match mode from
                // RuntimeConfig.
                {
                    use zerodds_rtps::publication_data::data_representation as dr;
                    let writer_offered: Vec<i16> = slot
                        .data_rep_offer_override
                        .clone()
                        .unwrap_or_else(|| self.config.data_representation_offer.clone());
                    let mode = self.config.data_rep_match_mode;
                    if let Some(negotiated) =
                        dr::negotiate(&writer_offered, &sub.data_representation, mode)
                    {
                        proxy.set_negotiated_data_representation(negotiated);
                    } else {
                        // No overlap → SEDP match spec violation.
                        // We add the proxy anyway for best-effort
                        // compat; the wire-format default stays XCDR2.
                        // A spec-strict caller should reject the match.
                    }
                }
                // Spec §2.2.3.4 Tab. 16: cache replay suppression. For
                // Volatile the reader must not see any late-joiner history
                // → skip up to `cache.max_sn`. For Transient/Persistent
                // the backend is authoritative — we deliver the history
                // via the backend replay path with NEW SNs; the
                // writer's own cache (especially gappy under KeepLast
                // eviction) must not serve the reader twice.
                // TransientLocal is the only tier where the
                // writer cache is the real history anchor.
                if !matches!(slot.durability, zerodds_qos::DurabilityKind::TransientLocal) {
                    if let Some(max) = slot.writer.cache().max_sn() {
                        proxy.skip_samples_up_to(max);
                    }
                }
                // Spec §2.2.3.5 — Durability=Transient/Persistent:
                // on the first late-joiner match, re-inject the backend samples
                // into the HistoryCache. The existing
                // reliable-reader path then delivers them via DATA +
                // heartbeat/AckNack. Idempotent via the
                // `backend_primed` flag.
                let backend_writes: Vec<Vec<u8>> = if !slot.backend_primed
                    && (slot.durability == zerodds_qos::DurabilityKind::Transient
                        || slot.durability == zerodds_qos::DurabilityKind::Persistent)
                {
                    slot.durability_backend
                        .as_ref()
                        .and_then(|b| b.replay_for_topic(&slot.topic_name).ok())
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| s.payload)
                        .collect()
                } else {
                    Vec::new()
                };
                slot.writer.add_reader_proxy(proxy);
                // Path-MTU-aware fragmentation: if ALL matched
                // readers run on the same host, traffic goes via
                // loopback (MTU 65536) — then one datagram per sample
                // instead of N 1344-B fragments (halves the 8-kB roundtrip
                // latency). As soon as a reader is remote, it stays
                // Ethernet-safe at DEFAULT_FRAGMENT_SIZE, so no
                // oversized datagram gets IP-fragmented on the 1500-byte
                // path.
                let all_same_host = slot
                    .writer
                    .reader_proxies()
                    .iter()
                    .all(|p| self.guid_prefix.is_same_host(p.remote_reader_guid.prefix));
                if all_same_host {
                    slot.writer
                        .set_fragmentation(LOOPBACK_FRAGMENT_SIZE, LOOPBACK_MTU);
                } else {
                    slot.writer
                        .set_fragmentation(DEFAULT_FRAGMENT_SIZE, DEFAULT_MTU);
                }
                // Wave 4b.2 (Spec `zerodds-zero-copy-1.0` §6): if the
                // remote reader runs on the same host (matching
                // GuidPrefix host-id, wave 4a), register the pair in the
                // SameHostTracker. Wave 4b.3 (feature `same-host-shm`):
                // additionally try to set up a PosixShmTransport owner
                // segment — on success `mark_bound(Owner)`,
                // otherwise `mark_failed` and UDP fallback.
                if self.guid_prefix.is_same_host(sub.key.prefix) {
                    let local_writer_guid =
                        zerodds_rtps::wire_types::Guid::new(self.guid_prefix, writer_eid);
                    self.same_host.register_pending(local_writer_guid, sub.key);
                    #[cfg(feature = "same-host-shm")]
                    {
                        match crate::same_host_shm::open_owner_segment(
                            self.guid_prefix,
                            local_writer_guid,
                            sub.key,
                        ) {
                            Ok(t) => self.same_host.mark_bound(
                                local_writer_guid,
                                sub.key,
                                t,
                                crate::same_host::Role::Owner,
                            ),
                            Err(reason) => {
                                self.same_host
                                    .mark_failed(local_writer_guid, sub.key, reason)
                            }
                        }
                    }
                }
                // Inject the backend replay into the HistoryCache (within
                // the slot lock). Important: with `KeepLast(N)` and a small N
                // the cache would immediately evict every replay sample
                // again — the subsequent writer tick then sees
                // SN=4,5 as "not in cache" and sends GAPs to the
                // reader, which marks our replay samples as irrelevant.
                // Solution: temporarily expand the cache to `KeepAll` with
                // a sufficient cap, for the duration of the
                // burst, then restore the user QoS.
                // Backend samples are in **raw** format (that is how
                // `DataWriter::write` in publisher.rs stores them) — before the
                // writer.write we must prepend the USER_PAYLOAD_ENCAP framing,
                // so the reader recognizes the stream value spec-conformantly
                // (see `validate_user_encap_offset`).
                let now_replay = self.start_instant.elapsed();
                if !backend_writes.is_empty() {
                    // Same encap header as in the live-write path
                    // (offer `first` + extensibility), so replay samples
                    // declare the same wire encoding.
                    let replay_encap = {
                        let offer_first = slot
                            .data_rep_offer_override
                            .as_ref()
                            .and_then(|v| v.first().copied())
                            .or_else(|| self.config.data_representation_offer.first().copied())
                            .unwrap_or(zerodds_rtps::publication_data::data_representation::XCDR);
                        user_payload_encap(
                            offer_first,
                            slot.wire_extensibility,
                            slot.big_endian_override,
                        )
                    };
                    let original_kind = slot.writer.cache().kind();
                    let original_max = slot.writer.cache().max_samples();
                    let burst_max = original_max
                        .saturating_add(backend_writes.len())
                        .max(backend_writes.len() + 16);
                    slot.writer.set_cache_kind_and_max(
                        zerodds_rtps::history_cache::HistoryKind::KeepAll,
                        burst_max,
                    );
                    for raw_payload in &backend_writes {
                        let mut framed = Vec::with_capacity(replay_encap.len() + raw_payload.len());
                        framed.extend_from_slice(&replay_encap);
                        framed.extend_from_slice(raw_payload);
                        if let Ok(out) = slot.writer.write_with_heartbeat(&framed, now_replay) {
                            replay_dgs.extend(out);
                        }
                    }
                    slot.writer
                        .set_cache_kind_and_max(original_kind, original_max);
                    slot.backend_primed = true;
                }
                // D.5e Phase-1: wake `wait_for_matched_subscription`-waiters.
                self.match_event.1.notify_all();

                // Security: derive the per-reader protection level from
                // security_info and build the locator lookup map,
                // so the writer tick can serialize per target
                // individually.
                #[cfg(feature = "security")]
                {
                    let peer_key = sub.key.prefix.0;
                    // Set the per-reader level ONLY for an EXPLICITLY announced
                    // `PID_ENDPOINT_SECURITY_INFO`. If it is missing (OpenDDS does not
                    // send it — it relies on the domain governance), NO
                    // None override: then the governance `data_protection` FLOOR
                    // applies in `secure_outbound_for_target`. An authenticated peer
                    // in a data_protection=ENCRYPT domain expects the encrypted
                    // payload; a None override would leak plaintext (cyclone/
                    // FastDDS announce security_info → unchanged).
                    if let Some(info) = sub.security_info.as_ref() {
                        let level = EndpointProtection::from_info(Some(info)).level;
                        slot.reader_protection.insert(peer_key, level);
                    }
                    for loc in &locators {
                        slot.locator_to_peer.insert(*loc, peer_key);
                    }
                }
            }
        }
        // Send the backend replay datagrams (Spec §2.2.3.5). The slot mutex
        // is released here; the send path mirrors the pattern from
        // `write_user_sample` — including the in-process fastpath for
        // same-process peers (otherwise UDP loopback loss under load can
        // swallow the Transient/Persistent replay samples).
        let inproc_peers: Vec<Arc<DcpsRuntime>> = {
            let all = crate::inproc::peers(self.domain_id, self.config.spdp_multicast_group);
            all.into_iter()
                .filter(|rt| rt.guid_prefix != self.guid_prefix)
                .collect()
        };
        let now_send = self.start_instant.elapsed();
        for dg in &replay_dgs {
            for t in dg.targets.iter() {
                if is_routable_user_locator(t) {
                    let _ = self.user_unicast.send(t, &dg.bytes);
                }
            }
            for peer in &inproc_peers {
                handle_user_datagram(peer, &dg.bytes, now_send);
            }
        }
        // Emit the match event outside the slot mutex.
        self.config.observability.record(
            &zerodds_foundation::observability::Event::new(
                zerodds_foundation::observability::Level::Info,
                zerodds_foundation::observability::Component::Discovery,
                "writer.matched_remote_reader",
            )
            .with_attr("writer_eid", alloc::format!("{writer_eid:?}")),
        );
    }

    fn wire_reader_to_remote_writer(
        &self,
        reader_eid: EntityId,
        pubd: &zerodds_rtps::publication_data::PublicationBuiltinTopicData,
    ) {
        // §2.2.2.2.1.17: an ignored publication must not be MATCHED, not merely
        // hidden from the DCPSPublication builtin reader. The Durability-Service
        // relies on this to avoid ingesting its own replay writer (echo loop).
        if let Some(filter) = self.ignore_filter_snapshot() {
            let pub_h = crate::instance_handle::InstanceHandle::from_guid(pubd.key);
            let part_h = crate::instance_handle::InstanceHandle::from_guid(pubd.participant_key);
            if filter.is_publication_ignored(pub_h) || filter.is_participant_ignored(part_h) {
                return;
            }
        }
        let locators =
            endpoint_or_default_locators(&pubd.unicast_locators, pubd.key.prefix, &self.discovered);
        if locators.is_empty() {
            return;
        }
        if let Some(slot_arc) = self.reader_slot(reader_eid) {
            if let Ok(mut slot) = slot_arc.lock() {
                let slot = &mut *slot;
                // Idempotency gate (symmetric to
                // `wire_writer_to_remote_reader`): if a WriterProxy already
                // exists for this remote writer, the
                // match has already run. A re-wire via UDP SEDP after
                // an in-process pull would REPLACE via `add_writer_proxy` —
                // resetting `delivered_up_to`/`received` and
                // losing already-buffered/delivered samples.
                if slot
                    .reader
                    .writer_proxies()
                    .iter()
                    .any(|s| s.proxy.remote_writer_guid == pubd.key)
                {
                    return;
                }
                // Per-policy bump for requested_incompatible_qos.
                use crate::psm_constants::qos_policy_id as qid;
                use crate::status::bump_policy_count;
                // C2 "loud instead of silent" (symmetric to the writer side):
                // an incompatible QoS match is logged loudly immediately.
                let obs = self.config.observability.clone();
                let topic_for_log = slot.topic_name.clone();
                let remote_for_log = alloc::format!("{:?}", pubd.key);
                let bump = |slot: &mut UserReaderSlot, pid: u32| {
                    slot.requested_incompatible_qos.total_count = slot
                        .requested_incompatible_qos
                        .total_count
                        .saturating_add(1);
                    slot.requested_incompatible_qos.last_policy_id = pid;
                    bump_policy_count(&mut slot.requested_incompatible_qos.policies, pid);
                    obs.record(
                        &zerodds_foundation::observability::Event::new(
                            zerodds_foundation::observability::Level::Warn,
                            zerodds_foundation::observability::Component::Dcps,
                            "qos.incompatible.requested",
                        )
                        .with_attr("topic", topic_for_log.as_str())
                        .with_attr("remote_writer", remote_for_log.as_str())
                        .with_attr("policy", qos_policy_id_name(pid)),
                    );
                };

                // See wire_writer... — symmetric, the writer is now remote.
                if (pubd.durability as u8) < (slot.durability as u8) {
                    bump(slot, qid::DURABILITY);
                    return;
                }
                if !deadline_compat(
                    qos_duration_to_nanos(pubd.deadline.period),
                    slot.deadline_nanos,
                ) {
                    bump(slot, qid::DEADLINE);
                    return;
                }
                if (pubd.liveliness.kind as u8) < (slot.liveliness_kind as u8) {
                    bump(slot, qid::LIVELINESS);
                    return;
                }
                if !deadline_compat(
                    qos_duration_to_nanos(pubd.liveliness.lease_duration),
                    slot.liveliness_lease_nanos,
                ) {
                    bump(slot, qid::LIVELINESS);
                    return;
                }
                if pubd.ownership != slot.ownership {
                    bump(slot, qid::OWNERSHIP);
                    return;
                }
                if !partitions_overlap(&pubd.partition, &slot.partition) {
                    bump(slot, qid::PARTITION);
                    return;
                }

                // F-TYPES-3 XTypes-1.3 §7.6.3.7 TypeConsistencyEnforcement.
                // If both sides carry a TypeIdentifier (≠ None),
                // we check compatibility via the TypeMatcher. Otherwise
                // the match falls back to a pure type_name comparison (default path).
                if slot.type_identifier != zerodds_types::TypeIdentifier::None
                    && pubd.type_identifier != zerodds_types::TypeIdentifier::None
                    // Equal TypeIdentifiers ⇒ same type (XTypes 1.3 §7.2.4.1).
                    // The typed-endpoint case carries a complete TypeIdentifier
                    // whose TypeObject is not in this fresh registry; identity
                    // is decisive without a structural lookup (Bug QT).
                    && pubd.type_identifier != slot.type_identifier
                {
                    let registry = zerodds_types::resolve::TypeRegistry::new();
                    let matcher =
                        zerodds_types::type_matcher::TypeMatcher::new(&slot.type_consistency);
                    if !matcher
                        .match_types(&pubd.type_identifier, &slot.type_identifier, &registry)
                        .is_match()
                    {
                        bump(slot, qid::TYPE_CONSISTENCY_ENFORCEMENT);
                        return;
                    }
                }

                slot.reader
                    .add_writer_proxy(zerodds_rtps::writer_proxy::WriterProxy::new(
                        pubd.key,
                        locators,
                        Vec::new(),
                        true,
                    ));
                // Wave 4b.2 (Spec `zerodds-zero-copy-1.0` §6): reader
                // side of the same-host match. If the remote writer runs on
                // the same host, register the pair AND
                // attach synchronously to the SHM segment.
                //
                // Idempotent: thanks to the `PosixShmTransport::open` refactor
                // (transport-shm bug fix 2026-05-19) it does not matter whether the
                // writer hook (open_owner) or the reader hook
                // (open_consumer) runs first — whoever comes first
                // creates the segment, whoever later attaches. Real-life
                // DDS has no guaranteed SEDP match order.
                if self.guid_prefix.is_same_host(pubd.key.prefix) {
                    let local_reader_guid =
                        zerodds_rtps::wire_types::Guid::new(self.guid_prefix, reader_eid);
                    self.same_host.register_pending(pubd.key, local_reader_guid);
                    #[cfg(feature = "same-host-shm")]
                    {
                        match crate::same_host_shm::open_consumer_segment(
                            self.guid_prefix,
                            pubd.key,
                            local_reader_guid,
                        ) {
                            Ok(t) => self.same_host.mark_bound(
                                pubd.key,
                                local_reader_guid,
                                t,
                                crate::same_host::Role::Consumer,
                            ),
                            Err(reason) => {
                                self.same_host
                                    .mark_failed(pubd.key, local_reader_guid, reason)
                            }
                        }
                    }
                }
                // D.5e Phase-1: wake `wait_for_matched_publication`-waiters.
                self.match_event.1.notify_all();

                // §2.2.3.23 exclusive-ownership resolver cache:
                // remember the writer `ownership_strength` from discovery, so
                // `delivered_to_user_sample` can pack the value into every
                // sample.
                slot.writer_strengths
                    .insert(pubd.key.to_bytes(), pubd.ownership_strength);
            }
        }
    }

    /// Writes a sample to a registered user writer and
    /// sends the generated datagrams.
    ///
    /// The payload is prefixed with the RTPS serialized-payload header
    /// (encapsulation scheme) before it goes into the DATA
    /// submessage. OMG RTPS 2.5 §9.4.2.13 requires exactly these
    /// 4 bytes at the start of every serialized user payload —
    /// see [`USER_PAYLOAD_ENCAP`] (`CDR_LE` / XCDR1).
    /// Without this header Cyclone/Fast-DDS readers refuse to
    /// deliver the sample (they parse the first 4 bytes as
    /// encapsulation kind + options and drop unknown-scheme).
    ///
    /// # Errors
    /// - `BadParameter` if the EntityId has no registered writer.
    /// - `WireError` on an encoding error.
    pub fn write_user_sample(&self, eid: EntityId, payload: Vec<u8>) -> Result<()> {
        // Vec-ownership API. The spec contract is unchanged. We delegate to
        // the borrowed variant; this saves a heap-allocation hop when
        // the caller already has a `&[u8]` (e.g. the C-FFI loan API).
        self.write_user_sample_borrowed(eid, &payload)
    }

    /// Sets the per-writer data-representation override for a user writer. The
    /// next `write_user_sample*` derives its encapsulation header from this
    /// override's first element instead of the runtime default — so a
    /// representation-faithful re-publisher (e.g. the durability service
    /// replaying foreign-vendor XCDR1 bytes) can declare the encap that matches
    /// the body it holds. `None` clears the override (back to the runtime
    /// default). Idempotent + cheap; safe to call before every write.
    ///
    /// # Errors
    /// `BadParameter` for an unknown writer entity id; `PreconditionNotMet` on a
    /// poisoned slot lock.
    pub fn set_user_writer_data_rep_override(
        &self,
        eid: EntityId,
        offer: Option<Vec<i16>>,
    ) -> Result<()> {
        let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
            what: "unknown writer entity id",
        })?;
        let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "user_writer slot poisoned",
        })?;
        slot.data_rep_offer_override = offer;
        Ok(())
    }

    /// Forces the writer to emit the big-endian (`_BE`) encapsulation variant
    /// (RTPS 2.5 §10.5) instead of the little-endian default. Used by the
    /// durability service replay path: a big-endian peer's stored sample holds
    /// big-endian body bytes, so its replay must carry a matching BE encap
    /// header. `false` restores the canonical little-endian wire.
    ///
    /// # Errors
    /// `BadParameter` for an unknown writer entity id; `PreconditionNotMet` on a
    /// poisoned slot lock.
    pub fn set_user_writer_byte_order_override(
        &self,
        eid: EntityId,
        big_endian: bool,
    ) -> Result<()> {
        let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
            what: "unknown writer entity id",
        })?;
        let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "user_writer slot poisoned",
        })?;
        slot.big_endian_override = big_endian;
        Ok(())
    }

    /// Sets the HISTORY KeepLast depth (DDS 1.4 §2.2.3.18) for a user writer.
    /// This governs how many of the most-recent samples **per instance key**
    /// are retained for the same-runtime TransientLocal late-joiner replay path
    /// (`intra_runtime_dispatch_alive` retains, a new route replays). Pass
    /// `usize::MAX` for KeepAll. A binding maps its HistoryQosPolicy here.
    ///
    /// # Errors
    /// `BadParameter` for an unknown writer entity id; `PreconditionNotMet` on a
    /// poisoned slot lock.
    pub fn set_user_writer_history_depth(&self, eid: EntityId, depth: usize) -> Result<()> {
        let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
            what: "unknown writer entity id",
        })?;
        let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "user_writer slot poisoned",
        })?;
        slot.history_depth = depth.max(1);
        // Re-enforce the new depth over the already-retained samples per key.
        let d = slot.history_depth;
        enforce_retained_depth(&mut slot.retained, d);
        Ok(())
    }

    /// Reads the current TransientLocal retained-sample count for a user writer
    /// (test/introspection helper). `0` for an unknown writer.
    #[must_use]
    pub fn user_writer_retained_len(&self, eid: EntityId) -> usize {
        self.writer_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.retained.len()))
            .unwrap_or(0)
    }

    /// Writes a user sample from a borrowed byte slice.
    /// **Zero-copy path** for the loan API and SHM backend: avoids
    /// the Vec materialization when the caller holds a slot/stack buffer.
    ///
    /// Identical semantics to `write_user_sample`; it just takes no
    /// ownership of the buffer.
    ///
    /// # Errors
    /// As `write_user_sample`.
    pub fn write_user_sample_borrowed(&self, eid: EntityId, payload: &[u8]) -> Result<()> {
        self.write_user_sample_keyed(eid, payload, [0u8; 16])
    }

    /// Like [`write_user_sample_borrowed`] but with an explicit 16-byte instance
    /// `key_hash` (DDS 1.4 §2.2.2.4.2 keyed topics). The key is used by the
    /// same-runtime TransientLocal retention path so KeepLast depth is enforced
    /// **per instance** and a late joiner replays the most-recent samples of
    /// every live instance (and any disposed/unregistered terminal marker).
    /// A binding that does not key its topic passes the all-zero key (one
    /// default instance), which is what `write_user_sample_borrowed` does.
    ///
    /// # Errors
    /// As [`write_user_sample_borrowed`].
    pub fn write_user_sample_keyed(
        &self,
        eid: EntityId,
        payload: &[u8],
        key_hash: [u8; 16],
    ) -> Result<()> {
        let _phase_guard = if phase_timing_enabled() {
            Some(PhaseTimer {
                start: std::time::Instant::now(),
                ns_acc: &PHASE_WRITE_USER_NS,
                calls_acc: &PHASE_WRITE_USER_CALLS,
            })
        } else {
            None
        };
        let pt_on = phase_timing_enabled();
        let pt_t0 = if pt_on {
            Some(std::time::Instant::now())
        } else {
            None
        };
        // Hot path: for small samples (<= 1.5 kB payload)
        // the encap framing is copied into a stack PoolBuffer —
        // zero heap touch in the framing step. Large samples fall
        // back to Vec.
        let now = self.start_instant.elapsed();
        let total = USER_PAYLOAD_ENCAP.len() + payload.len();
        let pt_t2_out: Option<std::time::Instant>;
        // XCDR version tag of the writer's effective offer (`0` = XCDR1,
        // `1` = XCDR2), set below from the same `offer_first` that drives the
        // wire encap header. Carried into the same-runtime loopback dispatch
        // so the intra-runtime reader sees the writer's real representation
        // (Bug R4) instead of an unconditional `0`.
        let intra_representation: u8;
        let out_datagrams = {
            let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
                what: "unknown writer entity id",
            })?;
            let pt_t1 = if pt_on {
                Some(std::time::Instant::now())
            } else {
                None
            };
            if let (Some(t0), Some(t1)) = (pt_t0, pt_t1) {
                PHASE_WRITE_SUB_NS[0].fetch_add(
                    (t1 - t0).as_nanos() as u64,
                    core::sync::atomic::Ordering::Relaxed,
                );
            }
            let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "user_writer slot poisoned",
            })?;
            let pt_t2 = if pt_on {
                Some(std::time::Instant::now())
            } else {
                None
            };
            pt_t2_out = pt_t2;
            if let (Some(t1), Some(t2)) = (pt_t1, pt_t2) {
                PHASE_WRITE_SUB_NS[1].fetch_add(
                    (t2 - t1).as_nanos() as u64,
                    core::sync::atomic::Ordering::Relaxed,
                );
            }
            // Deadline timer: remember the last write for offered_deadline_missed.
            slot.last_write = Some(now);
            // Encap header from the effective offer `first` (per-writer
            // override else runtime default) + type extensibility. The app
            // encoder serializes exactly this wire format; the header must
            // declare it honestly (otherwise an XCDR2-only vendor
            // reader misparses). See `user_payload_encap`.
            let encap = {
                let offer_first = slot
                    .data_rep_offer_override
                    .as_ref()
                    .and_then(|v| v.first().copied())
                    .or_else(|| self.config.data_representation_offer.first().copied())
                    .unwrap_or(zerodds_rtps::publication_data::data_representation::XCDR);
                // Map the negotiated i16 DataRepresentationId to the u8 XCDR
                // version tag used by `UserSample::Alive.representation` /
                // `encap_representation` (`1` = XCDR2, `0` = XCDR1). Mirrors
                // the wire path where the reader derives this from the encap
                // header byte[1].
                intra_representation =
                    if offer_first == zerodds_rtps::publication_data::data_representation::XCDR2 {
                        1
                    } else {
                        0
                    };
                user_payload_encap(
                    offer_first,
                    slot.wire_extensibility,
                    slot.big_endian_override,
                )
            };
            // Spec §2.2.3.5 backend filling happens in
            // `DataWriter::write` (publisher.rs) with the **raw** payload —
            // here only the HistoryCache filling + wire send.
            let dgs = if total <= SMALL_FRAME_CAP {
                write_user_sample_pooled(&mut slot.writer, payload, now, &encap)?
            } else {
                let mut framed = Vec::with_capacity(total);
                framed.extend_from_slice(&encap);
                framed.extend_from_slice(payload);
                // See write_user_sample_pooled: HB rate-limited via the
                // tick loop instead of per-write.
                let _ = now;
                slot.writer
                    .write(&framed)
                    .map_err(|_| DdsError::WireError {
                        message: String::from("user writer encode"),
                    })?
            };
            // Lifespan: remember the insert time of the just-written SN.
            if slot.lifespan_nanos != 0 {
                if let Some(sn) = slot.writer.cache().max_sn() {
                    slot.sample_insert_times.push_back((sn, now));
                }
            }
            // QR-cluster (a)+(b): TRANSIENT_LOCAL same-runtime retention with
            // per-instance HISTORY KeepLast depth (DDS 1.4 §2.2.3.4 + §2.2.3.18).
            // A new sample for a key clears any prior terminal lifecycle marker
            // for that key (the instance is alive again) and is appended; the
            // depth is then re-enforced per key.
            if !matches!(slot.durability, zerodds_qos::DurabilityKind::Volatile) {
                slot.retained
                    .retain(|s| !(s.lifecycle.is_some() && s.key_hash == key_hash));
                let strength = slot.ownership_strength;
                slot.retained.push_back(RetainedSample {
                    key_hash,
                    payload: payload.to_vec(),
                    representation: intra_representation,
                    strength,
                    lifecycle: None,
                });
                let depth = slot.history_depth;
                enforce_retained_depth(&mut slot.retained, depth);
            }
            dgs
        };
        let pt_t3 = if pt_on {
            Some(std::time::Instant::now())
        } else {
            None
        };
        if let (Some(t2), Some(t3)) = (pt_t2_out, pt_t3) {
            PHASE_WRITE_SUB_NS[2].fetch_add(
                (t3 - t2).as_nanos() as u64,
                core::sync::atomic::Ordering::Relaxed,
            );
        }
        // Opt-4 (Spec `zerodds-zero-copy-1.0` §9): precompute the skip set
        // of UDP locators occupied by a bound same-host reader.
        // Readers on these locators get the sample via
        // SHM (`same_host_send_pass` below); a UDP send would be a duplicate.
        #[cfg(feature = "same-host-shm")]
        let same_host_skip_locators: Vec<Locator> = self.same_host_udp_skip_set(eid);
        // In-process fastpath (same-process+domain peers): snapshot the
        // peer runtimes ONCE per write, then feed each datagram directly into
        // their recv path — no UDP loopback, no reliable
        // recovery race. The receiver deduplicates by SequenceNumber,
        // a copy arriving additionally via UDP later is a
        // no-op. The wire path stays untouched for cross-process.
        //
        // Hot-path fast path: lock-free registry hint. In the typical
        // cross-process bench (ping in process A, pong in process B)
        // A's registry has only A — the `peers()` lock+Vec alloc would be
        // pure overhead per write. Skip when count ≤ 1.
        let inproc_peers: Vec<Arc<DcpsRuntime>> = if crate::inproc::registry_count_hint() <= 1 {
            Vec::new()
        } else {
            let all = crate::inproc::peers(self.domain_id, self.config.spdp_multicast_group);
            all.into_iter()
                .filter(|rt| rt.guid_prefix != self.guid_prefix)
                .collect()
        };
        for dg in out_datagrams {
            // FU2 S3: UDP per target with per-reader data_protection
            // (`secure_outbound_for_target` — heterogeneously correct: legacy readers
            // get plaintext, secure readers SRTPS; the governance
            // data_protection fallback applies for readers without explicit
            // SEDP security_info).
            for t in dg.targets.iter() {
                if is_routable_user_locator(t) {
                    #[cfg(feature = "same-host-shm")]
                    if same_host_skip_locators.iter().any(|s| s == t) {
                        continue;
                    }
                    if let Some(secured) = secure_outbound_for_target(self, eid, &dg.bytes, t) {
                        #[allow(clippy::print_stderr)]
                        if let Err(e) = self.user_unicast.send(t, &secured) {
                            if std::env::var("ZERODDS_TRACE_SEND_ERR")
                                .map(|s| s == "1")
                                .unwrap_or(false)
                            {
                                eprintln!("[TRACE] user_unicast.send({t:?}) failed: {e:?}");
                            }
                        }
                    }
                }
            }
            // SHM + in-process fastpath: `secure_user_outbound` (uniform
            // governance data_protection level). The inproc peer runs through
            // its secured inbound path (decrypt or drop),
            // symmetric to the UDP recv — otherwise a non-
            // authenticated same-process peer could see encrypted data
            // unencrypted.
            if let Some(secured) = secure_user_outbound(self, &dg.bytes) {
                // Wave 4b.4 (Spec `zerodds-zero-copy-1.0` §6):
                // parallel send via SHM to all bound-owner entries
                // for this writer. Opt-4 above filters their UDP
                // locators out beforehand, so nothing is sent twice.
                #[cfg(feature = "same-host-shm")]
                self.same_host_send_pass(eid, &secured);
                for peer in &inproc_peers {
                    #[cfg(feature = "security")]
                    {
                        if let Some(clear) =
                            secure_inbound_bytes(peer, &secured, &DEFAULT_INBOUND_IFACE)
                        {
                            handle_user_datagram(peer, &clear, now);
                        }
                    }
                    #[cfg(not(feature = "security"))]
                    handle_user_datagram(peer, &secured, now);
                }
            }
        }
        let pt_t4 = if pt_on {
            Some(std::time::Instant::now())
        } else {
            None
        };
        if let (Some(t3), Some(t4)) = (pt_t3, pt_t4) {
            PHASE_WRITE_SUB_NS[3].fetch_add(
                (t4 - t3).as_nanos() as u64,
                core::sync::atomic::Ordering::Relaxed,
            );
        }
        // Same-runtime writer→reader loopback: in parallel to the wire path
        // push directly into the `sample_tx` of all local readers on the same
        // topic+type. Bridge-daemon use case (writer+reader
        // in the same DcpsRuntime); without this hook intra-process
        // loopback would be completely dead, because `inproc_announce_*` skips self
        // and UDP multicast loopback is not guaranteed. Strength from
        // the writer slot.
        let writer_strength = self
            .writer_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.ownership_strength))
            .unwrap_or(0);
        self.intra_runtime_dispatch_alive(eid, payload, writer_strength, intra_representation);
        // Embargo inspect tap at the DCPS layer (path-separated from the
        // production path). Only compiled when the `inspect` feature is
        // on. The topic name is fetched via a separate lookup, outside
        // the lock region so hooks do not run under the lock.
        #[cfg(feature = "inspect")]
        {
            self.dispatch_inspect_dcps_tap(eid, payload);
        }
        // D.5e Phase 3 — a freshly written sample makes a HEARTBEAT due: wake the
        // scheduler tick so it goes out immediately (no 5 ms tail), speeding the
        // reliable HB→ACKNACK handshake.
        self.raise_tick_wake();
        Ok(())
    }

    /// Wave 4b.4 (Spec `zerodds-zero-copy-1.0` §6) helper:
    /// sends `bytes` to all bound-owner entries of the [`SameHostTracker`]
    /// for this local writer (owner role).
    ///
    /// Called by the [`Self::write_user_sample`] hot path after the UDP send.
    /// Same-host readers thereby receive the sample frame
    /// via SHM **in addition** to the UDP path — the reader HistoryCache
    /// deduplicates by SequenceNumber.
    #[cfg(feature = "same-host-shm")]
    /// Opt-4 (Spec `zerodds-zero-copy-1.0` §9): locator skip set for
    /// the UDP send path. Returns all UDP default-unicast locators
    /// of the readers that have a bound same-host SHM pair with this
    /// writer — the hot-path caller filters these targets out of
    /// `dg.targets`, so the same readers are not served twice
    /// (UDP + SHM).
    #[cfg(feature = "same-host-shm")]
    fn same_host_udp_skip_set(&self, writer_eid: EntityId) -> Vec<Locator> {
        use crate::same_host::{Role, SameHostState};
        let writer_guid = zerodds_rtps::wire_types::Guid::new(self.guid_prefix, writer_eid);
        let mut skip: Vec<Locator> = Vec::new();
        let snapshot = self.same_host.snapshot();
        let discovered = self.discovered.clone();
        for (w, reader, state) in snapshot {
            if w != writer_guid {
                continue;
            }
            if !matches!(
                state,
                SameHostState::Bound {
                    role: Role::Owner,
                    ..
                }
            ) {
                continue;
            }
            // Reader prefix → default_unicast_locator from discovery.
            if let Ok(cache) = discovered.lock() {
                if let Some(p) = cache.get(&reader.prefix) {
                    if let Some(loc) = p.data.default_unicast_locator {
                        skip.push(loc);
                    }
                }
            }
        }
        skip
    }

    #[cfg(feature = "same-host-shm")]
    fn same_host_send_pass(&self, writer_eid: EntityId, bytes: &[u8]) {
        use crate::same_host::{Role, SameHostState};
        use zerodds_transport::Transport;
        use zerodds_transport_shm::PosixShmTransport;

        let writer_guid = zerodds_rtps::wire_types::Guid::new(self.guid_prefix, writer_eid);
        let snapshot = self.same_host.snapshot();
        let total = snapshot.len();
        let mut matched = 0u32;
        let mut owners = 0u32;
        let mut sent = 0u32;
        for (w, _reader, state) in snapshot {
            if w != writer_guid {
                continue;
            }
            matched += 1;
            let SameHostState::Bound { transport, role } = state else {
                continue;
            };
            if !matches!(role, Role::Owner) {
                continue;
            }
            owners += 1;
            let Ok(t) = transport.downcast::<PosixShmTransport>() else {
                continue;
            };
            // ShmTransport is 1:1: send() validates `dest ==
            // peer_locator`. Owner.peer_locator points to the
            // consumer endpoint → that is our target.
            let target = t.peer_locator();
            if t.send(&target, bytes).is_ok() {
                sent += 1;
            }
        }
        let _ = (total, matched, owners, sent); // diag counter removed after the Bug-3 fix
    }

    /// Inspect-endpoint tap dispatch for DCPS publish.
    /// Reads the topic name separately from the WriterSlot and passes
    /// a frame to the zerodds-inspect-endpoint tap registry.
    /// **Not** the production hot path: only when the `inspect` feature is on.
    #[cfg(feature = "inspect")]
    fn dispatch_inspect_dcps_tap(&self, eid: EntityId, payload: &[u8]) {
        let Some(slot_arc) = self.writer_slot(eid) else {
            return;
        };
        let topic = match slot_arc.lock() {
            Ok(slot) => slot.topic_name.clone(),
            Err(_) => return,
        };
        let ts_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let mut corr: u64 = 0;
        for (i, byte) in eid.entity_key.iter().enumerate() {
            corr |= u64::from(*byte) << (i * 8);
        }
        corr |= u64::from(eid.entity_kind as u8) << 24;
        let frame = zerodds_inspect_endpoint::Frame::dcps(topic, ts_ns, corr, payload.to_vec());
        zerodds_inspect_endpoint::tap::dispatch(&frame);
    }

    /// Sends a lifecycle marker (`dispose`/`unregister_instance`) to
    /// all matched readers. Spec §2.2.2.4.2.10/.7 + §9.6.3.9 PID_STATUS_INFO.
    /// `status_bits` is the OR combination of
    /// `zerodds_rtps::inline_qos::status_info::DISPOSED` and/or `UNREGISTERED`.
    ///
    /// # Errors
    /// - `BadParameter` if the EntityId has no registered writer.
    /// - `WireError` on an encode error.
    pub fn write_user_lifecycle(
        &self,
        eid: EntityId,
        key_hash: [u8; 16],
        status_bits: u32,
    ) -> Result<()> {
        let out_datagrams = {
            let slot_arc = self.writer_slot(eid).ok_or(DdsError::BadParameter {
                what: "unknown writer entity id",
            })?;
            let mut slot = slot_arc.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "user_writer slot poisoned",
            })?;
            slot.writer
                .write_lifecycle(key_hash, status_bits)
                .map_err(|_| DdsError::WireError {
                    message: String::from("user writer lifecycle encode"),
                })?
        };
        for dg in out_datagrams {
            // FU2 S3: lifecycle DATA (dispose/unregister) per-target
            // data_protection-aware (heterogeneously correct like the immediate send).
            for t in dg.targets.iter() {
                if is_routable_user_locator(t) {
                    if let Some(secured) = secure_outbound_for_target(self, eid, &dg.bytes, t) {
                        let _ = self.user_unicast.send(t, &secured);
                    }
                }
            }
        }
        // QR-cluster (d): also deliver the lifecycle marker to matched
        // same-runtime readers — the wire targets above never include
        // intra-runtime local readers (those go via the direct dispatch path).
        // Map the PID_STATUS_INFO bits to the HistoryCache ChangeKind.
        use zerodds_rtps::inline_qos::status_info;
        let disposed = status_bits & status_info::DISPOSED != 0;
        let unregistered = status_bits & status_info::UNREGISTERED != 0;
        let kind = match (disposed, unregistered) {
            (true, true) => zerodds_rtps::history_cache::ChangeKind::NotAliveDisposedUnregistered,
            (true, false) => zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
            (false, true) => zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered,
            // No status bits set: nothing to deliver as a lifecycle marker.
            (false, false) => return Ok(()),
        };
        self.intra_runtime_dispatch_lifecycle(eid, key_hash, kind);
        Ok(())
    }

    /// Generates a 3-byte entity key for new user endpoints.
    fn next_entity_key(&self) -> [u8; 3] {
        let n = self.entity_counter.fetch_add(1, Ordering::Relaxed);
        [(n >> 16) as u8, (n >> 8) as u8, n as u8]
    }

    /// Snapshot of all currently known remote publications (topic
    /// name + type name + writer GUID).
    #[must_use]
    pub fn discovered_publications_count(&self) -> usize {
        self.sedp
            .lock()
            .map(|s| s.cache().publications_len())
            .unwrap_or(0)
    }

    /// Snapshot of every publication on this domain as `(topic_name,
    /// type_name)` — raw DDS topic/type strings — for graph introspection
    /// (`rmw_get_topic_names_and_types`, `rmw_count_publishers`). Includes BOTH
    /// this participant's LOCAL user writers AND the remote publications from
    /// SEDP, so a node sees its own topics as well as its peers'.
    #[must_use]
    pub fn discovered_publication_topics(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        if let Ok(map) = self.user_writers.read() {
            for slot in map.values() {
                if let Ok(s) = slot.lock() {
                    out.push((s.topic_name.clone(), s.type_name.clone()));
                }
            }
        }
        if let Ok(s) = self.sedp.lock() {
            out.extend(
                s.cache()
                    .publications()
                    .map(|p| (p.data.topic_name.clone(), p.data.type_name.clone())),
            );
        }
        out
    }

    /// Snapshot of every subscription on this domain as `(topic_name,
    /// type_name)` (local user readers + remote SEDP). Counterpart to
    /// [`Self::discovered_publication_topics`].
    #[must_use]
    pub fn discovered_subscription_topics(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        if let Ok(map) = self.user_readers.read() {
            for slot in map.values() {
                if let Ok(s) = slot.lock() {
                    out.push((s.topic_name.clone(), s.type_name.clone()));
                }
            }
        }
        if let Ok(s) = self.sedp.lock() {
            out.extend(
                s.cache()
                    .subscriptions()
                    .map(|s| (s.data.topic_name.clone(), s.data.type_name.clone())),
            );
        }
        out
    }

    /// Snapshot of all currently known remote subscriptions.
    #[must_use]
    pub fn discovered_subscriptions_count(&self) -> usize {
        self.sedp
            .lock()
            .map(|s| s.cache().subscriptions_len())
            .unwrap_or(0)
    }

    /// Per-endpoint snapshot of every publication on this domain (local user
    /// writers + remote SEDP), for ROS 2 `rmw_get_publishers_info_by_topic`.
    #[must_use]
    pub fn discovered_publication_endpoints(&self) -> Vec<DiscoveredEndpointInfo> {
        let secs = |nanos: u64| i32::try_from(nanos / 1_000_000_000).unwrap_or(i32::MAX);
        let mut out: Vec<DiscoveredEndpointInfo> = Vec::new();
        if let Ok(map) = self.user_writers.read() {
            for slot in map.values() {
                if let Ok(s) = slot.lock() {
                    out.push(DiscoveredEndpointInfo {
                        topic_name: s.topic_name.clone(),
                        type_name: s.type_name.clone(),
                        endpoint_guid: guid_to_16(s.writer.guid()),
                        reliable: s.reliable,
                        transient_local: !matches!(
                            s.durability,
                            zerodds_qos::DurabilityKind::Volatile
                        ),
                        deadline_seconds: secs(s.deadline_nanos),
                        lifespan_seconds: secs(s.lifespan_nanos),
                        liveliness_lease_seconds: secs(s.liveliness_lease_nanos),
                    });
                }
            }
        }
        if let Ok(s) = self.sedp.lock() {
            for p in s.cache().publications() {
                out.push(DiscoveredEndpointInfo {
                    topic_name: p.data.topic_name.clone(),
                    type_name: p.data.type_name.clone(),
                    endpoint_guid: guid_to_16(p.data.key),
                    reliable: matches!(
                        p.data.reliability.kind,
                        zerodds_qos::ReliabilityKind::Reliable
                    ),
                    transient_local: !matches!(
                        p.data.durability,
                        zerodds_qos::DurabilityKind::Volatile
                    ),
                    deadline_seconds: p.data.deadline.period.seconds,
                    lifespan_seconds: p.data.lifespan.duration.seconds,
                    liveliness_lease_seconds: p.data.liveliness.lease_duration.seconds,
                });
            }
        }
        out
    }

    /// Counterpart to [`Self::discovered_publication_endpoints`] for
    /// subscriptions (`rmw_get_subscriptions_info_by_topic`).
    #[must_use]
    pub fn discovered_subscription_endpoints(&self) -> Vec<DiscoveredEndpointInfo> {
        let secs = |nanos: u64| i32::try_from(nanos / 1_000_000_000).unwrap_or(i32::MAX);
        let mut out: Vec<DiscoveredEndpointInfo> = Vec::new();
        if let Ok(map) = self.user_readers.read() {
            for slot in map.values() {
                if let Ok(s) = slot.lock() {
                    out.push(DiscoveredEndpointInfo {
                        topic_name: s.topic_name.clone(),
                        type_name: s.type_name.clone(),
                        endpoint_guid: guid_to_16(s.reader.guid()),
                        // Reader requested-reliability is not retained in the
                        // slot; RELIABLE is the rmw default (best-effort field).
                        reliable: true,
                        transient_local: !matches!(
                            s.durability,
                            zerodds_qos::DurabilityKind::Volatile
                        ),
                        deadline_seconds: secs(s.deadline_nanos),
                        lifespan_seconds: 0,
                        liveliness_lease_seconds: secs(s.liveliness_lease_nanos),
                    });
                }
            }
        }
        if let Ok(s) = self.sedp.lock() {
            for sub in s.cache().subscriptions() {
                out.push(DiscoveredEndpointInfo {
                    topic_name: sub.data.topic_name.clone(),
                    type_name: sub.data.type_name.clone(),
                    endpoint_guid: guid_to_16(sub.data.key),
                    reliable: matches!(
                        sub.data.reliability.kind,
                        zerodds_qos::ReliabilityKind::Reliable
                    ),
                    transient_local: !matches!(
                        sub.data.durability,
                        zerodds_qos::DurabilityKind::Volatile
                    ),
                    deadline_seconds: sub.data.deadline.period.seconds,
                    lifespan_seconds: 0,
                    liveliness_lease_seconds: sub.data.liveliness.lease_duration.seconds,
                });
            }
        }
        out
    }

    /// Number of matched remote readers for a local user writer.
    /// Polled by `DataWriter::wait_for_matched_subscription`.
    #[must_use]
    pub fn user_writer_matched_count(&self, eid: EntityId) -> usize {
        // Distinct matched subscriptions = remote/cross-participant reader
        // proxies UNION same-participant (intra-runtime) local readers. The
        // intra-runtime self-match path delivers samples without adding a wire
        // reader-proxy (avoids UDP-to-self double-delivery), so its matches
        // would otherwise be invisible to `wait_for_matched_subscription`.
        self.user_writer_matched_subscription_handles(eid).len()
    }

    /// List of `InstanceHandle`s of all matched readers for a local
    /// user writer (Spec §2.2.2.4.2.x `get_matched_subscriptions`): remote/
    /// cross-participant readers (reader proxies) plus the same-participant
    /// readers from the intra-runtime routes, deduplicated by GUID.
    #[must_use]
    pub fn user_writer_matched_subscription_handles(
        &self,
        eid: EntityId,
    ) -> Vec<crate::instance_handle::InstanceHandle> {
        let mut handles: Vec<crate::instance_handle::InstanceHandle> = self
            .writer_slot(eid)
            .and_then(|arc| {
                arc.lock().ok().map(|s| {
                    s.writer
                        .reader_proxies()
                        .iter()
                        .map(|p| {
                            crate::instance_handle::InstanceHandle::from_guid(p.remote_reader_guid)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        for h in self.intra_runtime_writer_matched_readers(eid) {
            if !handles.contains(&h) {
                handles.push(h);
            }
        }
        handles
    }

    /// Same-participant readers that the local writer `eid` delivers to via
    /// an intra-runtime route (as matched-subscription handles).
    fn intra_runtime_writer_matched_readers(
        &self,
        writer_eid: EntityId,
    ) -> Vec<crate::instance_handle::InstanceHandle> {
        match self.intra_runtime_routes.read() {
            Ok(g) => g
                .get(&writer_eid)
                .map(|readers| {
                    readers
                        .iter()
                        .map(|reid| {
                            crate::instance_handle::InstanceHandle::from_guid(Guid::new(
                                self.guid_prefix,
                                *reid,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Same-participant writers that deliver to the local
    /// reader `reader_eid` via an intra-runtime route (as matched-publication handles).
    fn intra_runtime_reader_matched_writers(
        &self,
        reader_eid: EntityId,
    ) -> Vec<crate::instance_handle::InstanceHandle> {
        match self.intra_runtime_routes.read() {
            Ok(g) => g
                .iter()
                .filter(|(_, readers)| readers.contains(&reader_eid))
                .map(|(weid, _)| {
                    crate::instance_handle::InstanceHandle::from_guid(Guid::new(
                        self.guid_prefix,
                        *weid,
                    ))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List of `InstanceHandle`s of all matched remote writers for a
    /// local user reader (Spec §2.2.2.5.x `get_matched_publications`).
    #[must_use]
    pub fn user_reader_matched_publication_handles(
        &self,
        eid: EntityId,
    ) -> Vec<crate::instance_handle::InstanceHandle> {
        let mut handles: Vec<crate::instance_handle::InstanceHandle> = self
            .reader_slot(eid)
            .and_then(|arc| {
                arc.lock().ok().map(|s| {
                    s.reader
                        .writer_proxies()
                        .iter()
                        .map(|p| {
                            crate::instance_handle::InstanceHandle::from_guid(
                                p.proxy.remote_writer_guid,
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        for h in self.intra_runtime_reader_matched_writers(eid) {
            if !handles.contains(&h) {
                handles.push(h);
            }
        }
        handles
    }

    /// Counter for missed offered deadlines on the user writer.
    /// Spec OMG DDS 1.4 §2.2.4.2.9 `OFFERED_DEADLINE_MISSED_STATUS`.
    #[must_use]
    pub fn user_writer_offered_deadline_missed(&self, eid: EntityId) -> u64 {
        self.writer_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.offered_deadline_missed_count))
            .unwrap_or(0)
    }

    /// Counter for missed requested deadlines on the user reader.
    /// Spec §2.2.4.2.11 `REQUESTED_DEADLINE_MISSED_STATUS`.
    #[must_use]
    pub fn user_reader_requested_deadline_missed(&self, eid: EntityId) -> u64 {
        self.reader_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.requested_deadline_missed_count))
            .unwrap_or(0)
    }

    /// Current liveliness status of a local user reader.
    /// Spec §2.2.4.2.14 `LIVELINESS_CHANGED_STATUS`:
    /// `(alive, alive_count, not_alive_count)`.
    #[must_use]
    pub fn user_reader_liveliness_status(&self, eid: EntityId) -> (bool, u64, u64) {
        self.reader_slot(eid)
            .and_then(|arc| {
                arc.lock().ok().map(|s| {
                    (
                        s.liveliness_alive,
                        s.liveliness_alive_count,
                        s.liveliness_not_alive_count,
                    )
                })
            })
            .unwrap_or((false, 0, 0))
    }

    /// LivelinessLost counter on the user writer (Spec §2.2.4.2.10).
    /// Incremented by `check_writer_liveliness`.
    #[must_use]
    pub fn user_writer_liveliness_lost(&self, eid: EntityId) -> u64 {
        self.writer_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.liveliness_lost_count))
            .unwrap_or(0)
    }

    /// Snapshot of OfferedIncompatibleQosStatus on the writer.
    #[must_use]
    pub fn user_writer_offered_incompatible_qos(
        &self,
        eid: EntityId,
    ) -> crate::status::OfferedIncompatibleQosStatus {
        self.writer_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.offered_incompatible_qos.clone()))
            .unwrap_or_default()
    }

    /// Snapshot of RequestedIncompatibleQosStatus on the reader.
    #[must_use]
    pub fn user_reader_requested_incompatible_qos(
        &self,
        eid: EntityId,
    ) -> crate::status::RequestedIncompatibleQosStatus {
        self.reader_slot(eid)
            .and_then(|arc| {
                arc.lock()
                    .ok()
                    .map(|s| s.requested_incompatible_qos.clone())
            })
            .unwrap_or_default()
    }

    /// Sample-lost counter (reader side). Spec §2.2.4.2.6.2.
    #[must_use]
    pub fn user_reader_sample_lost(&self, eid: EntityId) -> u64 {
        self.reader_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.sample_lost_count))
            .unwrap_or(0)
    }

    /// Monotonically increasing count of alive samples delivered to the
    /// user (Spec §2.2.4.2.6.1 `on_data_available` detector). A delta
    /// against the last poll snapshot means "new data available".
    #[must_use]
    pub fn user_reader_samples_delivered(&self, eid: EntityId) -> u64 {
        self.reader_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.samples_delivered_count))
            .unwrap_or(0)
    }

    /// A2 — arm TIME_BASED_FILTER (DDS 1.4 §2.2.3.12) on a runtime/C-FFI user
    /// reader: it then receives at most one sample per instance per
    /// `min_separation_nanos`; closer-spaced samples are dropped before they
    /// reach the reader's channel. `0` disables the filter. Returns `true` if
    /// the reader exists. This is the seam `rmw_zerodds` uses to rate-limit ROS-2
    /// subscriptions (`rmw_qos_profile_t` carries no TIME_BASED_FILTER field).
    pub fn set_user_reader_time_based_filter(
        &self,
        eid: EntityId,
        min_separation_nanos: u128,
    ) -> bool {
        let Some(arc) = self.reader_slot(eid) else {
            return false;
        };
        let Ok(mut slot) = arc.lock() else {
            return false;
        };
        slot.tbf_min_separation_nanos = min_separation_nanos;
        if min_separation_nanos == 0 {
            slot.tbf_last_delivered.clear();
        }
        true
    }

    /// Bug-2 diagnosis (2026-05-19): number of submessages dropped
    /// because of an unknown writer_id. If this value is incremented
    /// after a write, it indicates an SEDP match
    /// race (writer_proxy not yet added when DATA is received).
    #[must_use]
    pub fn user_reader_unknown_src_count(&self, eid: EntityId) -> u64 {
        self.reader_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.reader.unknown_src_count()))
            .unwrap_or(0)
    }

    /// Sample-rejected status (reader side). Spec §2.2.4.2.6.3.
    #[must_use]
    pub fn user_reader_sample_rejected(
        &self,
        eid: EntityId,
    ) -> crate::status::SampleRejectedStatus {
        self.reader_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.sample_rejected))
            .unwrap_or_default()
    }

    /// Records a lost sample on the user reader. Called
    /// by resource-limit or decode-failure paths — the
    /// detector is application-external, because sample-lost depending on the
    /// implementation comes from several sources (cache drop, decode
    /// fail, sequence-number gap drop).
    pub fn record_sample_lost(&self, eid: EntityId, count: u32) {
        if count == 0 {
            return;
        }
        if let Some(arc) = self.reader_slot(eid) {
            if let Ok(mut slot) = arc.lock() {
                slot.sample_lost_count = slot.sample_lost_count.saturating_add(u64::from(count));
            }
        }
    }

    /// Records a rejected sample on the user reader.
    pub fn record_sample_rejected(
        &self,
        eid: EntityId,
        kind: crate::status::SampleRejectedStatusKind,
        instance: crate::instance_handle::InstanceHandle,
    ) {
        if let Some(arc) = self.reader_slot(eid) {
            if let Ok(mut slot) = arc.lock() {
                slot.sample_rejected.total_count =
                    slot.sample_rejected.total_count.saturating_add(1);
                slot.sample_rejected.last_reason = kind;
                slot.sample_rejected.last_instance_handle = instance;
            }
        }
    }

    /// Manual liveliness assert on the user writer. Sets the
    /// `last_liveliness_assert` timestamp. For `LivelinessKind::Automatic`
    /// `last_write` is also set — the liveliness path
    /// otherwise never falls through the `assert` trigger, because every successful
    /// `write` already takes over the liveliness tick.
    pub fn assert_writer_liveliness_eid(&self, eid: EntityId) {
        let now = self.start_instant.elapsed();
        if let Some(arc) = self.writer_slot(eid) {
            if let Ok(mut slot) = arc.lock() {
                slot.last_liveliness_assert = Some(now);
                if slot.liveliness_kind == zerodds_qos::LivelinessKind::Automatic {
                    slot.last_write = Some(now);
                }
            }
        }
    }

    /// True if all matched readers have acknowledged all samples written
    /// so far. Empty cache or no proxies → true.
    #[must_use]
    pub fn user_writer_all_acknowledged(&self, eid: EntityId) -> bool {
        self.writer_slot(eid)
            .and_then(|arc| arc.lock().ok().map(|s| s.writer.all_samples_acknowledged()))
            .unwrap_or(true)
    }

    /// Test helper — pushes a synthetic `UserSample::Alive`
    /// directly into the `mpsc::Sender` of the given reader, without
    /// going through the wire/discovery path. Enables end-to-end tests of
    /// downstream consumers (e.g. bridge-daemon pumps) that otherwise
    /// become flaky in CI containers due to multicast-loopback limits.
    /// **Not** for production code.
    ///
    /// `writer_guid` and `writer_strength` are set to default values
    /// (shared-ownership assumption).
    ///
    /// Returns `true` if the reader slot exists and the push
    /// succeeded, `false` if the EID is unknown or the channel is
    /// closed.
    #[doc(hidden)]
    pub fn test_inject_user_alive(&self, eid: EntityId, payload: Vec<u8>) -> bool {
        let Some(arc) = self.reader_slot(eid) else {
            return false;
        };
        let Ok(mut slot) = arc.lock() else {
            return false;
        };
        let sent = slot
            .sample_tx
            .send(UserSample::Alive {
                payload: crate::sample_bytes::SampleBytes::from_vec(payload),
                writer_guid: [0u8; 16],
                writer_strength: 0,
                representation: 0,
                big_endian: false,
                source_timestamp: None,
            })
            .is_ok();
        if sent {
            slot.samples_delivered_count = slot.samples_delivered_count.saturating_add(1);
        }
        sent
    }

    /// Test helper — bumps the inconsistent-topic counter as if matching had
    /// discovered a remote endpoint with the same `topic_name` but a
    /// different `type_name`. Lets listener-FFI tests exercise the
    /// `on_inconsistent_topic` poll path without standing up two
    /// participants with a real SEDP type mismatch. **Not** for production.
    #[doc(hidden)]
    pub fn test_bump_inconsistent_topic(&self) {
        self.inconsistent_topic_seq.fetch_add(1, Ordering::Relaxed);
    }

    /// Spec §3.1 zerodds-async-1.0: registers the waker of an
    /// async reader in the UserReaderSlot. On `sample_tx.send`
    /// the waker is woken. `None` as the argument clears the waker
    /// (e.g. after the async reader is dropped).
    pub fn register_user_reader_waker(&self, eid: EntityId, waker: Option<core::task::Waker>) {
        if let Some(arc) = self.reader_slot(eid) {
            if let Ok(slot) = arc.lock() {
                if let Ok(mut g) = slot.async_waker.lock() {
                    *g = waker;
                }
            }
        }
    }

    /// Register a listener callback for alive-sample
    /// arrival on the user reader. `None` clears an
    /// existing listener.
    ///
    /// The listener fires synchronously on the recv thread of
    /// `recv_user_data_loop` — see the contract doc on the
    /// [`UserReaderListener`] type. Eliminates the user-polling
    /// latency (~50-100 µs) compared to `sample_tx.recv()`.
    ///
    /// Returns `true` if the reader slot exists and the listener
    /// was set, `false` if the EID is not a known user reader.
    pub fn set_user_reader_listener(
        &self,
        eid: EntityId,
        listener: Option<UserReaderListener>,
    ) -> bool {
        let Some(arc) = self.reader_slot(eid) else {
            return false;
        };
        let Ok(mut slot) = arc.lock() else {
            return false;
        };
        slot.listener = listener.map(alloc::sync::Arc::new);
        true
    }

    /// Number of matched writers for a local user reader: remote/cross-
    /// participant writers (writer proxies) plus same-participant writers from the
    /// intra-runtime routes, deduplicated by GUID (symmetric to the writer).
    #[must_use]
    pub fn user_reader_matched_count(&self, eid: EntityId) -> usize {
        self.user_reader_matched_publication_handles(eid).len()
    }

    /// D.5e Phase-1 — waits until a match event occurs or the timeout
    /// is reached. Replaces 20-ms polling in `DataReader::wait_for_matched_*`
    /// and `DataWriter::wait_for_matched_*`.
    ///
    /// The caller checks the match count itself (via `user_*_matched_count`)
    /// before and after the wait — this function is only the block mechanics.
    /// Returns `false` if the timeout is reached, `true` if a notify came.
    #[cfg(feature = "std")]
    pub fn wait_match_event(&self, timeout: core::time::Duration) -> bool {
        let (lock, cvar) = &*self.match_event;
        let Ok(guard) = lock.lock() else { return false };
        match cvar.wait_timeout(guard, timeout) {
            Ok((_, t)) => !t.timed_out(),
            Err(_) => false,
        }
    }

    /// D.5e Phase-1 — waits until an ACK event occurs or a timeout.
    /// Replaces 50-ms polling in `DataWriter::wait_for_acknowledgments`.
    #[cfg(feature = "std")]
    pub fn wait_ack_event(&self, timeout: core::time::Duration) -> bool {
        let (lock, cvar) = &*self.ack_event;
        let Ok(guard) = lock.lock() else { return false };
        match cvar.wait_timeout(guard, timeout) {
            Ok((_, t)) => !t.timed_out(),
            Err(_) => false,
        }
    }

    /// D.5e Phase-1 — notify helper for the ACK event. Called by the reliable
    /// writer path when an ACKNACK advances the acked-base.
    #[cfg(feature = "std")]
    pub(crate) fn notify_ack_event(&self) {
        self.ack_event.1.notify_all();
    }

    /// ADR-0006 — sets the PID_SHM_LOCATOR bytes for a local
    /// user writer in the side map. Called by the DataWriter
    /// once `set_flat_backend` has attached a same-host backend (POSIX shm /
    /// Iceoryx2). On the next SEDP push the wire encoder
    /// injects PID 0x8001 into the `PublicationData`.
    pub fn set_shm_locator(&self, eid: EntityId, bytes: Vec<u8>) {
        if let Ok(mut g) = self.shm_locators.write() {
            g.insert(eid, bytes);
        }
    }

    /// ADR-0006 — reads the PID_SHM_LOCATOR bytes for a local
    /// user writer from the side map. Returns `None` if no
    /// same-host backend is set.
    #[must_use]
    pub fn shm_locator(&self, eid: EntityId) -> Option<Vec<u8>> {
        self.shm_locators.read().ok()?.get(&eid).cloned()
    }

    /// ADR-0006 — removes the PID_SHM_LOCATOR entry (e.g. when the
    /// user writer is reconfigured without a backend).
    pub fn clear_shm_locator(&self, eid: EntityId) {
        if let Ok(mut g) = self.shm_locators.write() {
            g.remove(&eid);
        }
    }

    /// Stops all worker threads (recv loops + tick loop) and joins
    /// them. Idempotent — repeated calls are no-ops.
    ///
    /// Shutdown delay: up to ~1 s, because the recv threads sit in
    /// `recv()` with a 1 s read timeout. After the
    /// current recv() call finishes they check the stop flag and
    /// terminate.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        // D.5e Phase 3 — wake the scheduler tick worker so it observes `stop`
        // immediately instead of parking up to the idle floor.
        if let Ok(guard) = self.tick_wake.lock() {
            if let Some(h) = guard.as_ref() {
                h.stop();
            }
        }
        if let Ok(mut guard) = self.handles.lock() {
            for h in guard.drain(..) {
                let _ = h.join();
            }
        }
    }
}

impl Drop for DcpsRuntime {
    // ZERODDS_PHASE_DUMP=1 is on-demand debug telemetry for
    // phase-latency profiling. eprintln is semantically correct here
    // (stderr diagnostics), no log-crate dependency wanted.
    #[allow(clippy::print_stderr)]
    fn drop(&mut self) {
        if std::env::var("ZERODDS_PHASE_DUMP")
            .map(|s| s == "1")
            .unwrap_or(false)
        {
            let hu_ns = PHASE_HANDLE_USER_NS.load(core::sync::atomic::Ordering::Relaxed);
            let hu_n = PHASE_HANDLE_USER_CALLS.load(core::sync::atomic::Ordering::Relaxed);
            let wu_ns = PHASE_WRITE_USER_NS.load(core::sync::atomic::Ordering::Relaxed);
            let wu_n = PHASE_WRITE_USER_CALLS.load(core::sync::atomic::Ordering::Relaxed);
            let hu_us = if hu_n > 0 {
                hu_ns as f64 / hu_n as f64 / 1000.0
            } else {
                0.0
            };
            let wu_us = if wu_n > 0 {
                wu_ns as f64 / wu_n as f64 / 1000.0
            } else {
                0.0
            };
            eprintln!(
                "[ZERODDS_PHASE] handle_user_datagram:  N={}  avg={:.3}us  total={:.1}ms",
                hu_n,
                hu_us,
                hu_ns as f64 / 1_000_000.0
            );
            eprintln!(
                "[ZERODDS_PHASE] write_user_sample:      N={}  avg={:.3}us  total={:.1}ms",
                wu_n,
                wu_us,
                wu_ns as f64 / 1_000_000.0
            );
            // Sub-phases of write_user_sample_borrowed.
            // [0] slot_lookup, [1] slot_lock_acquire,
            // [2] writer.write + framing, [3] dispatch (UDP + inproc).
            const SUB_LABELS: [&str; 4] = [
                "  ├─ slot_lookup       ",
                "  ├─ slot_lock_acquire ",
                "  ├─ writer.write+frame",
                "  └─ dispatch (UDP+...)",
            ];
            for (i, label) in SUB_LABELS.iter().enumerate() {
                let s_ns = PHASE_WRITE_SUB_NS[i].load(core::sync::atomic::Ordering::Relaxed);
                if s_ns > 0 && wu_n > 0 {
                    let s_us = s_ns as f64 / wu_n as f64 / 1000.0;
                    eprintln!(
                        "[ZERODDS_PHASE] {} avg={:.3}us  total={:.1}ms",
                        label,
                        s_us,
                        s_ns as f64 / 1_000_000.0
                    );
                }
            }
        }
        self.shutdown();
    }
}

// ---------------------------------------------------------------------
// Worker threads (Sprint D.5b — per-socket recv + central tick).
//
// Before: a single `event_loop` that went through three sequential
// blocking `recv()`s with a `tick_period` timeout (50 ms) per iteration.
// Roundtrip latency: 5-14 ms p50 (CFS drift + sequential wait stages).
//
// Now: four dedicated threads.
//   * recv_spdp_multicast_loop  — blocks on the SPDP multicast socket
//   * recv_metatraffic_loop     — blocks on SPDP unicast (= metatraffic)
//   * recv_user_data_loop       — blocks on user-data unicast
//   * tick_loop                 — periodic outbound tasks +
//                                 per-interface inbound (non-blocking) +
//                                 deadline/lifespan/liveliness
//
// Lock discipline: the recv threads and the tick thread contend for
// `rt.sedp.lock()` / `rt.wlp.lock()` / per-slot `slot.lock()`.
// Convention: keep lock-hold times short (handle_datagram + tick each
// have only single-pass logic), no sub-lock under sedp/wlp.
// ---------------------------------------------------------------------

/// Sprint D.5d lever C — applies SCHED_FIFO + CPU affinity to the
/// calling thread. Linux-only; no-op on macOS/Windows.
///
/// Called by every worker loop right at the start, so
/// the syscalls run on the actual worker thread
/// (`pthread_self()` must come from the thread itself).
///
/// Failures are logged to stderr but are not fatal — if
/// the process has no `CAP_SYS_NICE`, the runtime continues with
/// the CFS default scheduler.
#[allow(unused_variables)]
fn apply_thread_tuning(label: &str, priority: Option<i32>, cpus: Option<&[usize]>) {
    #[cfg(target_os = "linux")]
    rt_pinning::apply(label, priority, cpus);
}

/// Linux-only `pthread_setschedparam` + `sched_setaffinity` wrapper.
/// A dedicated module encapsulates the `unsafe` locally with safety notes; the
/// crate-level `#![deny(unsafe_code)]` stays active for the rest of the dcps
/// codebase.
#[cfg(target_os = "linux")]
#[allow(unsafe_code, clippy::print_stderr)]
mod rt_pinning {
    pub(super) fn apply(label: &str, priority: Option<i32>, cpus: Option<&[usize]>) {
        if let Some(prio) = priority {
            // SAFETY: libc FFI with an owned `param` struct. The self-thread via
            // `pthread_self()` is always valid.
            // musl libc has additional `sched_ss_*` fields (POSIX
            // sporadic-server) that we do not set — `mem::zeroed`
            // initializes them cleanly to 0.
            unsafe {
                let mut param: libc::sched_param = core::mem::zeroed();
                param.sched_priority = prio;
                let rc = libc::pthread_setschedparam(
                    libc::pthread_self(),
                    libc::SCHED_FIFO,
                    &raw const param,
                );
                if rc != 0 {
                    eprintln!(
                        "zdds[{label}]: pthread_setschedparam SCHED_FIFO {prio} \
                         failed (rc={rc}). Need CAP_SYS_NICE or RLIMIT_RTPRIO."
                    );
                }
            }
        }
        if let Some(cpu_list) = cpus {
            // SAFETY: cpu_set_t is POD; CPU_ZERO/SET are libc inline
            // functions without lifetime requirements.
            unsafe {
                let mut set: libc::cpu_set_t = core::mem::zeroed();
                libc::CPU_ZERO(&mut set);
                for &cpu in cpu_list {
                    if cpu < libc::CPU_SETSIZE as usize {
                        libc::CPU_SET(cpu, &mut set);
                    }
                }
                let rc = libc::sched_setaffinity(
                    0,
                    core::mem::size_of::<libc::cpu_set_t>(),
                    &raw const set,
                );
                if rc != 0 {
                    eprintln!("zdds[{label}]: sched_setaffinity({cpu_list:?}) failed.");
                }
            }
        }
    }
}

/// FastDDS interop (phase 2): acknowledges FastDDS' reliable secure SPDP writer
/// (0xff0101c2). FastDDS heartbeats its secure SPDP reliably and sends the
/// `participant_crypto_tokens` only once our 0xff0101c7 reader has acked its writer
/// (fast<->fast reference pcap: ACKNACK on 0xff0101c7). We respond to
/// every incoming secure-SPDP HEARTBEAT with an ACKNACK (base = last+1,
/// final), addressed via INFO_DST to the sender prefix. Gated on
/// `enable_secure_spdp`.
#[cfg(feature = "security")]
fn secure_spdp_reader_acks(rt: &DcpsRuntime, clear: &[u8]) -> Vec<Vec<u8>> {
    use zerodds_rtps::header::RtpsHeader;
    use zerodds_rtps::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
    use zerodds_rtps::submessages::{AckNackSubmessage, HeartbeatSubmessage, SequenceNumberSet};
    use zerodds_rtps::wire_types::SequenceNumber;
    if !rt.config.enable_secure_spdp {
        return Vec::new();
    }
    let Ok(parsed) = decode_datagram(clear) else {
        return Vec::new();
    };
    let peer_prefix = parsed.header.guid_prefix;
    let mut out = Vec::new();
    let mut count = 0i32;
    let secure_writer = EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER;
    let secure_reader = EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER;
    // Header + INFO_DST(peer) + submessage. INFO_DST is mandatory, otherwise the
    // dest prefix is UNKNOWN -> FastDDS discards it as "not a connection".
    let wrap = |id: SubmessageId, body: &[u8], flags: u8| -> Option<Vec<u8>> {
        let blen = u16::try_from(body.len()).ok()?;
        let header = RtpsHeader::new(VendorId::ZERODDS, rt.guid_prefix);
        let mut dg = Vec::with_capacity(20 + 16 + body.len() + 4);
        dg.extend_from_slice(&header.to_bytes());
        let info = SubmessageHeader {
            submessage_id: SubmessageId::InfoDst,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: 12,
        };
        dg.extend_from_slice(&info.to_bytes());
        dg.extend_from_slice(&peer_prefix.to_bytes());
        let sh = SubmessageHeader {
            submessage_id: id,
            flags: flags | FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: blen,
        };
        dg.extend_from_slice(&sh.to_bytes());
        dg.extend_from_slice(body);
        Some(dg)
    };
    for sub in &parsed.submessages {
        match sub {
            // FastDDS' secure-SPDP writer HEARTBEAT -> we ack (reader 0xff0101c7).
            ParsedSubmessage::Heartbeat(hb) if hb.writer_id == secure_writer => {
                count = count.wrapping_add(1);
                let ack = AckNackSubmessage {
                    reader_id: secure_reader,
                    writer_id: secure_writer,
                    reader_sn_state: SequenceNumberSet {
                        bitmap_base: SequenceNumber(hb.last_sn.0 + 1),
                        num_bits: 0,
                        bitmap: Vec::new(),
                    },
                    count,
                    final_flag: true,
                };
                let (body, flags) = ack.write_body(true);
                if let Some(dg) = wrap(SubmessageId::AckNack, &body, flags) {
                    out.push(dg);
                }
            }
            // FastDDS' reader requests (preemptive ACKNACK to our 0xff0101c2
            // writer) our secure-SPDP data reliably -> deliver DATA(SN=1) +
            // HEARTBEAT(1,1), otherwise FastDDS' reader never matches and
            // sends no crypto_tokens.
            ParsedSubmessage::AckNack(a) if a.writer_id == secure_writer => {
                if let Ok(mut beacon) = rt.spdp_beacon.lock() {
                    if let Ok(data_dg) = beacon.serialize_secure() {
                        out.push(protect_secure_spdp(rt, &data_dg).unwrap_or(data_dg));
                    }
                }
                count = count.wrapping_add(1);
                let hbsm = HeartbeatSubmessage {
                    reader_id: secure_reader,
                    writer_id: secure_writer,
                    first_sn: SequenceNumber(1),
                    last_sn: SequenceNumber(1),
                    count,
                    final_flag: false,
                    liveliness_flag: false,
                    group_info: None,
                };
                let (body, flags) = hbsm.write_body(true);
                if let Some(dg) = wrap(SubmessageId::Heartbeat, &body, flags) {
                    out.push(dg);
                }
            }
            _ => {}
        }
    }
    out
}

/// FastDDS interop (phase 2b): builds a secure-SPDP HEARTBEAT (writer
/// 0xff0101c2, first=1/last=1) with INFO_DST to `peer_prefix`. Sent periodically per
/// discovered peer, so FastDDS' reliable secure-SPDP reader is solicited to a
/// (preemptive) ACKNACK and matches our writer.
#[cfg(feature = "security")]
fn build_secure_spdp_heartbeat(
    local_prefix: GuidPrefix,
    peer_prefix: GuidPrefix,
    count: i32,
) -> Option<Vec<u8>> {
    use zerodds_rtps::header::RtpsHeader;
    use zerodds_rtps::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
    use zerodds_rtps::submessages::HeartbeatSubmessage;
    use zerodds_rtps::wire_types::SequenceNumber;
    let hb = HeartbeatSubmessage {
        reader_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
        writer_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
        first_sn: SequenceNumber(1),
        last_sn: SequenceNumber(1),
        count,
        final_flag: false,
        liveliness_flag: false,
        group_info: None,
    };
    let (body, flags) = hb.write_body(true);
    let blen = u16::try_from(body.len()).ok()?;
    let header = RtpsHeader::new(VendorId::ZERODDS, local_prefix);
    let mut dg = Vec::with_capacity(20 + 16 + body.len() + 4);
    dg.extend_from_slice(&header.to_bytes());
    let info = SubmessageHeader {
        submessage_id: SubmessageId::InfoDst,
        flags: FLAG_E_LITTLE_ENDIAN,
        octets_to_next_header: 12,
    };
    dg.extend_from_slice(&info.to_bytes());
    dg.extend_from_slice(&peer_prefix.to_bytes());
    let sh = SubmessageHeader {
        submessage_id: SubmessageId::Heartbeat,
        flags: flags | FLAG_E_LITTLE_ENDIAN,
        octets_to_next_header: blen,
    };
    dg.extend_from_slice(&sh.to_bytes());
    dg.extend_from_slice(&body);
    Some(dg)
}

/// FastDDS interop: SEC-protects the secure-SPDP DATA (0xff0101c2) under
/// `discovery_protection != NONE` — FastDDS then encrypts the secure-SPDP DATA
/// (like the secure SEDP), and a PLAIN secure SPDP is discarded. Wraps
/// the DATA submessage with the per-endpoint writer key (0xff0101c2) as
/// SEC_PREFIX/BODY/POSTFIX; framing submessages (INFO_*) stay. Without
/// discovery_protection (common subset) passthrough. `None` on a crypto error.
#[cfg(feature = "security")]
fn protect_secure_spdp(rt: &DcpsRuntime, datagram: &[u8]) -> Option<Vec<u8>> {
    let gate = rt.config.security.as_ref()?;
    if gate.discovery_protection().unwrap_or(ProtectionLevel::None) == ProtectionLevel::None
        || datagram.len() < 20
    {
        return Some(datagram.to_vec());
    }
    let h = local_endpoint_crypto_handle(
        rt,
        EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
        true,
    )?;
    let mut out = datagram[..20].to_vec();
    for (id, start, total) in walk_submessages(datagram) {
        let submsg = &datagram[start..start + total];
        if id == SMID_DATA {
            match gate.encode_data_datawriter_by_handle(h, submsg) {
                Ok(s) => out.extend_from_slice(&s),
                Err(_) => return None,
            }
        } else {
            out.extend_from_slice(submsg);
        }
    }
    Some(out)
}

/// Worker: blocks on the SPDP multicast socket, dispatches SPDP beacons +
/// WLP heartbeats that come in over multicast.
fn recv_spdp_multicast_loop(rt: Arc<DcpsRuntime>, stop: Arc<AtomicBool>) {
    apply_thread_tuning(
        "recv-spdp-mc",
        rt.config.recv_thread_priority,
        rt.config.recv_thread_cpus.as_deref(),
    );
    while !stop.load(Ordering::Relaxed) {
        let elapsed = rt.start_instant.elapsed();
        let sedp_now = Duration::from_secs(elapsed.as_secs())
            + Duration::from_nanos(u64::from(elapsed.subsec_nanos()));
        let Ok(dg) = rt.spdp_multicast_rx.recv() else {
            continue;
        };
        #[cfg(feature = "security")]
        let clear = secure_inbound_bytes(&rt, &dg.data, &DEFAULT_INBOUND_IFACE);
        #[cfg(not(feature = "security"))]
        let clear = secure_inbound_bytes(&rt, &dg.data);
        if let Some(clear) = clear {
            handle_spdp_datagram(&rt, &clear);
            // FastDDS interop phase 2: ack the secure-SPDP HEARTBEATs (0xff0101c2)
            // reliably, otherwise FastDDS sends no crypto_tokens.
            #[cfg(feature = "security")]
            for ack in secure_spdp_reader_acks(&rt, &clear) {
                for loc in wlp_unicast_targets(&rt.discovered_participants()) {
                    let _ = rt.spdp_unicast.send(&loc, &ack);
                }
            }
            // WLP heartbeats arrive on the SPDP multicast socket
            // (the sender sends them to the SPDP multicast group).
            // handle_spdp_datagram ignores them, so we also feed
            // the same buffer into the WLP endpoint. A
            // secure-WLP DATA is participant-key SEC-protected → decode
            // it first (like secure SEDP in the metatraffic loop), otherwise
            // wlp.handle_datagram would only see the SEC block.
            #[cfg(feature = "security")]
            let wlp_decoded: Option<Vec<u8>> = if clear.len() >= 20 {
                let mut pk = [0u8; 12];
                pk.copy_from_slice(&clear[8..20]);
                unprotect_user_datagram(&rt, &clear, &pk)
            } else {
                None
            };
            #[cfg(feature = "security")]
            let wlp_input: &[u8] = wlp_decoded.as_deref().unwrap_or(&clear);
            #[cfg(not(feature = "security"))]
            let wlp_input: &[u8] = &clear;
            if let Ok(mut wlp) = rt.wlp.lock() {
                let _ = wlp.handle_datagram(wlp_input, sedp_now);
            }
        }
    }
}

/// Worker: blocks on SPDP unicast (= metatraffic socket), dispatches
/// SPDP reverse beacons + SEDP + WLP + security builtin.
fn recv_metatraffic_loop(rt: Arc<DcpsRuntime>, stop: Arc<AtomicBool>) {
    apply_thread_tuning(
        "recv-meta",
        rt.config.recv_thread_priority,
        rt.config.recv_thread_cpus.as_deref(),
    );
    while !stop.load(Ordering::Relaxed) {
        let elapsed = rt.start_instant.elapsed();
        let sedp_now = Duration::from_secs(elapsed.as_secs())
            + Duration::from_nanos(u64::from(elapsed.subsec_nanos()));
        let Ok(dg) = rt.spdp_unicast.recv() else {
            continue;
        };
        #[cfg(feature = "security")]
        let clear = secure_inbound_bytes(&rt, &dg.data, &DEFAULT_INBOUND_IFACE);
        #[cfg(not(feature = "security"))]
        let clear = secure_inbound_bytes(&rt, &dg.data);
        if let Some(clear) = clear {
            // A single recv call, both handlers on the same
            // datagram. SPDP first (Cyclone reverse beacons), then
            // SEDP, then WLP, then security builtin.
            handle_spdp_datagram(&rt, &clear);
            // FastDDS interop phase 2: ack the secure-SPDP HEARTBEATs (0xff0101c2)
            // reliably (they arrive unicast over the metatraffic socket).
            #[cfg(feature = "security")]
            for ack in secure_spdp_reader_acks(&rt, &clear) {
                for loc in wlp_unicast_targets(&rt.discovered_participants()) {
                    let _ = rt.spdp_unicast.send(&loc, &ack);
                }
            }
            // Protected discovery: secure-SEDP DATA is SEC_* submessage-
            // protected (the sender's participant data key). Before the SEDP parse
            // decode it with the sender prefix (RTPS header bytes[8..20]); for
            // plaintext SEDP (no SEC_*) unprotect_user_datagram returns None
            // and we use `clear` unchanged.
            #[cfg(feature = "security")]
            let sedp_decoded: Option<Vec<u8>> = if clear.len() >= 20 {
                let mut pk = [0u8; 12];
                pk.copy_from_slice(&clear[8..20]);
                unprotect_user_datagram(&rt, &clear, &pk)
            } else {
                None
            };
            // OPEN (phase 3, internal/security/per-endpoint-crypto-followup.md):
            // if `unprotect_user_datagram` fails for a secure-SEDP DATA
            // (cyclone's per-endpoint token not yet installed — race),
            // `sedp_input` falls back to the SEC_* bytes and the DATA is discarded.
            // Cross-vendor (discovery=ENCRYPT) must make this deterministic:
            // treat the reliable secure-SEDP DATA as not-received (NACK,
            // no SN advance), so the re-send after token install decodes.
            #[cfg(feature = "security")]
            let sedp_input: &[u8] = sedp_decoded.as_deref().unwrap_or(&clear);
            #[cfg(not(feature = "security"))]
            let sedp_input: &[u8] = &clear;
            let events = {
                if let Ok(mut sedp) = rt.sedp.lock() {
                    sedp.handle_datagram(sedp_input, sedp_now).ok()
                } else {
                    None
                }
            };
            if let Some(ev) = events {
                if !ev.is_empty() {
                    run_matching_pass(&rt);
                    push_sedp_events_to_builtin_readers(&rt, &ev);
                }
            }

            // Secure WLP (BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER) is, like
            // secure SEDP, participant-key SEC-protected → feed the decoded variant
            // (sedp_input), not the still SEC-wrapped `clear`. For
            // plaintext WLP, sedp_input == clear.
            let wlp_resends = if let Ok(mut wlp) = rt.wlp.lock() {
                let _ = wlp.handle_datagram(sedp_input, sedp_now);
                // Reliable resend: if the peer NACKs our (secure-)WLP writer,
                // we re-emit the missing beats (cyclone treats WLP as
                // reliable; without a resend it would never get the liveliness assertion).
                wlp.wlp_acknack_resends(sedp_input)
            } else {
                Vec::new()
            };
            for beat in wlp_resends {
                if let Some(secured) = protect_wlp_outbound(&rt, &beat) {
                    for loc in wlp_unicast_targets(&rt.discovered_participants()) {
                        let _ = rt.spdp_unicast.send(&loc, &secured);
                    }
                }
            }
            for dg in dispatch_security_builtin_datagram(&rt, &clear, sedp_now) {
                send_discovery_datagram(&rt, &dg.targets, &dg.bytes);
            }
        }
    }
}

/// Supervisor: wave 4b.4 (Spec `zerodds-zero-copy-1.0` §6) — same-host SHM
/// receive. Spawns **one dedicated receive thread per bound SHM consumer**, so
/// each blocks on its own segment futex ([`PosixShmTransport::recv`] →
/// `wait_for_frame`) and dispatches a sample the instant it lands.
///
/// History: the original single loop iterated all consumers round-robin and
/// called the blocking `recv()` on each in turn. With N consumers the
/// worst-case per-sample latency was `(N-1) × recv_timeout` (1 ms each, see
/// [`crate::same_host_shm::shm_config_for_pair`]) — a single thread cannot wait
/// on N futexes at once, so the active consumer's sample waited while the loop
/// sat in idle consumers' `recv()` timeouts. Fine for 1-2 same-host peers, but
/// it dominated at the many-endpoint scale ROS hits (user topics +
/// `ros_discovery_info` + parameter services + `rosout` = a dozen same-host SHM
/// consumers), turning a ~30 µs delivery into ~570 µs. The documented fix
/// ("multiple threads or epoll-style multiplexing") is realized here as one
/// thread per consumer. The supervisor only polls *membership* (discovery-rate,
/// not the data path) to spawn/reap workers.
#[cfg(feature = "same-host-shm")]
fn recv_user_shm_loop(rt: Arc<DcpsRuntime>, stop: Arc<AtomicBool>) {
    use crate::same_host::{Role, SameHostState};
    use zerodds_transport_shm::PosixShmTransport;

    apply_thread_tuning(
        "recv-shm-sup",
        rt.config.recv_thread_priority,
        rt.config.recv_thread_cpus.as_deref(),
    );
    // segment-id → (per-worker stop flag, join handle).
    let mut workers: std::collections::HashMap<
        [u8; 16],
        (Arc<AtomicBool>, thread::JoinHandle<()>),
    > = std::collections::HashMap::new();
    // Membership poll is discovery-rate, NOT the data path: it only detects
    // newly-bound / vanished consumers to spawn / reap their worker thread.
    let membership_poll = Duration::from_millis(100);
    while !stop.load(Ordering::Relaxed) {
        let mut live: std::collections::HashSet<[u8; 16]> = std::collections::HashSet::new();
        for (w, r, state) in rt.same_host.snapshot() {
            let SameHostState::Bound { transport, role } = state else {
                continue;
            };
            if !matches!(role, Role::Consumer) {
                continue;
            }
            let Ok(consumer) = transport.downcast::<PosixShmTransport>() else {
                continue;
            };
            let key = crate::same_host::shm_segment_id_for_pair(w, r);
            live.insert(key);
            if workers.contains_key(&key) {
                continue;
            }
            let wstop = Arc::new(AtomicBool::new(false));
            let (rt_w, stop_w, wstop_w) = (Arc::clone(&rt), Arc::clone(&stop), Arc::clone(&wstop));
            if let Ok(h) = thread::Builder::new()
                .name(String::from("zdds-recv-shm-c"))
                .spawn(move || shm_consumer_recv_loop(rt_w, consumer, stop_w, wstop_w))
            {
                workers.insert(key, (wstop, h));
            }
        }
        // Reap workers whose consumer disappeared.
        let gone: Vec<[u8; 16]> = workers
            .keys()
            .filter(|k| !live.contains(*k))
            .copied()
            .collect();
        for k in gone {
            if let Some((wstop, h)) = workers.remove(&k) {
                wstop.store(true, Ordering::Relaxed);
                let _ = h.join();
            }
        }
        thread::sleep(membership_poll);
    }
    // Shutdown: stop + join every worker (each wakes within its 1 ms recv_timeout).
    for (_, (wstop, h)) in workers {
        wstop.store(true, Ordering::Relaxed);
        let _ = h.join();
    }
}

/// One per-consumer SHM receive thread: blocks on this segment's futex and
/// dispatches each frame the instant it arrives — no cross-consumer
/// serialization. Exits when the runtime stops, the worker is reaped, or the
/// segment dies. See [`recv_user_shm_loop`].
#[cfg(feature = "same-host-shm")]
fn shm_consumer_recv_loop(
    rt: Arc<DcpsRuntime>,
    consumer: Arc<zerodds_transport_shm::PosixShmTransport>,
    stop: Arc<AtomicBool>,
    wstop: Arc<AtomicBool>,
) {
    use zerodds_transport::Transport;
    apply_thread_tuning(
        "recv-shm-c",
        rt.config.recv_thread_priority,
        rt.config.recv_thread_cpus.as_deref(),
    );
    while !stop.load(Ordering::Relaxed) && !wstop.load(Ordering::Relaxed) {
        match consumer.recv() {
            Ok(dg) => {
                let elapsed = rt.start_instant.elapsed();
                let sedp_now = Duration::from_secs(elapsed.as_secs())
                    + Duration::from_nanos(u64::from(elapsed.subsec_nanos()));
                // Security gate (analogous to the UDP path). SHM is
                // same-host-only — if the policy allows plaintext, the
                // datagram comes through unchanged.
                #[cfg(feature = "security")]
                let clear = secure_inbound_bytes(&rt, &dg.data, &DEFAULT_INBOUND_IFACE);
                #[cfg(not(feature = "security"))]
                let clear = secure_inbound_bytes(&rt, &dg.data);
                if let Some(clear) = clear {
                    handle_user_datagram(&rt, &clear, sedp_now);
                }
            }
            // A timeout is normal — the 1 ms recv_timeout just lets the loop
            // re-check the stop flags; an empty segment is not an error.
            Err(zerodds_transport::RecvError::Timeout) => {}
            // Hard error (broken segment / peer crashed): drop this worker; the
            // supervisor respawns if the segment is re-bound, UDP stays fallback.
            Err(_) => break,
        }
    }
}

/// Worker: blocks on the user-data unicast socket, dispatches
/// TypeLookup service replies + user-sample datagrams.
///
/// Int-1 (Spec `zerodds-zero-copy-1.0` §9): with the feature
/// `recvmmsg-batch` on Linux the loop uses `recv_batch_linux` and
/// fetches up to 32 datagrams per syscall — a 7-8x throughput boost.
/// On an empty batch the path falls back to single-recv() so
/// the recv thread does not spin in a busy loop at low traffic.
fn recv_user_data_loop(
    rt: Arc<DcpsRuntime>,
    socket: Arc<dyn Transport + Send + Sync>,
    stop: Arc<AtomicBool>,
) {
    apply_thread_tuning(
        "recv-user",
        rt.config.recv_thread_priority,
        rt.config.recv_thread_cpus.as_deref(),
    );
    // recvmmsg-batch (Linux + feature) needs the concrete UdpSocket
    // under the trait. With a trait-object transport this is not directly
    // accessible — we fall back to single-recv(). recvmmsg is
    // a UDP optimization; once TCP/SHM transports are to be mixed,
    // it is no longer worth it. For a pure UDPv4 user transport
    // this costs ~5-10% throughput in Linux batch mode (measured 2026-05).
    while !stop.load(Ordering::Relaxed) {
        let elapsed = rt.start_instant.elapsed();
        let sedp_now = Duration::from_secs(elapsed.as_secs())
            + Duration::from_nanos(u64::from(elapsed.subsec_nanos()));
        let Ok(dg) = socket.recv() else {
            continue;
        };
        dispatch_user_datagram(&rt, &dg, sedp_now);
        // D.5e Phase 3 — incoming user data may solicit an ACKNACK or advance a
        // reliable reader: wake the scheduler tick immediately (no 5 ms tail).
        rt.raise_tick_wake();
    }
}

/// Helper: dispatches a single user datagram through the security gate +
/// TypeLookup + handle_user_datagram. Shared by the single-recv and the
/// recvmmsg batch path.
fn dispatch_user_datagram(
    rt: &Arc<DcpsRuntime>,
    dg: &zerodds_transport::ReceivedDatagram,
    sedp_now: Duration,
) {
    #[cfg(feature = "security")]
    let clear = secure_inbound_bytes(rt, &dg.data, &DEFAULT_INBOUND_IFACE);
    #[cfg(not(feature = "security"))]
    let clear = secure_inbound_bytes(rt, &dg.data);
    if let Some(clear) = clear {
        // TypeLookup service first — if the frame is addressed to
        // TL_SVC_*_READER, it does not go to a
        // user reader. Other frames fall through.
        if !dispatch_type_lookup_datagram(rt, &clear, &dg.source) {
            handle_user_datagram(rt, &clear, sedp_now);
        }
    }
}

/// Worker: periodic outbound tasks + per-interface inbound
/// (non-blocking) + housekeeping. Sleeps `tick_period` between
/// iterations.
fn tick_loop(rt: Arc<DcpsRuntime>, stop: Arc<AtomicBool>) {
    apply_thread_tuning(
        "tick",
        rt.config.tick_thread_priority,
        rt.config.tick_thread_cpus.as_deref(),
    );
    let mut st = TickState::new(&rt);
    while !stop.load(Ordering::Relaxed) {
        run_tick_iteration(Arc::clone(&rt), &mut st);
        // Housekeeping runs inline here in the classic fixed-period path,
        // exactly as before (every `tick_period`, same cadence).
        tick_housekeep(&rt, rt.start_instant.elapsed());
        std::thread::sleep(rt.config.tick_period);
    }
}

/// D.5e Phase 3 — idle park cap for a discovery-only participant (no user
/// endpoints): how long the scheduler tick worker may sleep when nothing but
/// SPDP/WLP is pending. SPDP/WLP fire on their own (longer) periods, so this is
/// just a safety heartbeat — well above the 5 ms poll it replaces.
const SCHEDULER_IDLE_FLOOR: Duration = Duration::from_millis(250);

/// Earliest instant the scheduler tick worker must next run `run_tick_iteration`
/// so no periodic work is delayed: never past the next SPDP announce, and —
/// while user endpoints exist — capped at `tick_period` so HEARTBEAT/ACKNACK/
/// deadline/lifespan/liveliness keep their current cadence (identical wire
/// behaviour). With no user endpoints, parks up to [`SCHEDULER_IDLE_FLOOR`].
/// Active traffic is handled out-of-band by `raise_tick_wake` (immediate).
fn next_tick_deadline(rt: &Arc<DcpsRuntime>, st: &TickState) -> Instant {
    let now = Instant::now();
    let fine_cap = if rt.has_user_endpoints() {
        rt.config.tick_period
    } else {
        SCHEDULER_IDLE_FLOOR
    };
    st.next_announce.min(now + fine_cap).max(now)
}

/// D.5e Phase 3 B-2 — the kinds of work the deadline-heap scheduler fires as
/// distinct heap events, each re-armed at its own next deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickEvent {
    /// Periodic SPDP announce + reliable outbound (SEDP / WLP / user HEARTBEAT /
    /// ACKNACK) + secondary inbound poll — the wire-producing tick
    /// ([`run_tick_iteration`]), re-armed at [`next_tick_deadline`].
    Tick,
    /// Deadline / lifespan / liveliness housekeeping ([`tick_housekeep`]),
    /// re-armed at the **exact** next QoS due-instant (no fixed quantum).
    Housekeep,
}

/// D.5e Phase 3 — event-driven scheduler tick worker. Replaces the fixed-period
/// `tick_loop` sleep with a deadline-heap park. Two independent event streams:
/// [`TickEvent::Tick`] drives the **unchanged** `run_tick_iteration` (wire
/// output byte-identical to `tick_loop`), re-armed at [`next_tick_deadline`];
/// [`TickEvent::Housekeep`] runs the QoS checks, re-armed at their exact next
/// due-instant so a deadline/lifespan/liveliness fires on time instead of up to
/// one `tick_period` late, and an idle participant parks long. A write/recv
/// `raise_tick_wake` wakes **both** immediately, so freshly-armed QoS windows
/// are picked up without delay.
fn scheduler_tick_loop(
    rt: Arc<DcpsRuntime>,
    stop: Arc<AtomicBool>,
    mut scheduler: crate::scheduler::Scheduler<TickEvent>,
    handle: crate::scheduler::SchedulerHandle<TickEvent>,
) {
    apply_thread_tuning(
        "tick",
        rt.config.tick_thread_priority,
        rt.config.tick_thread_cpus.as_deref(),
    );
    let mut st = TickState::new(&rt);
    // Prime both event streams immediately.
    handle.raise_now(TickEvent::Tick);
    handle.raise_now(TickEvent::Housekeep);
    loop {
        let (due, stopped) = scheduler.park_due_batch();
        if stopped || stop.load(Ordering::Relaxed) {
            break;
        }
        if due.is_empty() {
            continue; // woken with nothing due yet — re-evaluate.
        }
        // Coalesce: a batch of wakes maps to at most ONE run of each kind.
        let mut do_tick = false;
        let mut do_housekeep = false;
        for ev in due {
            match ev {
                TickEvent::Tick => do_tick = true,
                TickEvent::Housekeep => do_housekeep = true,
            }
        }
        if do_tick {
            rt.tick_wake_pending.store(false, Ordering::Release);
            run_tick_iteration(Arc::clone(&rt), &mut st);
            if stop.load(Ordering::Relaxed) {
                break;
            }
            handle.raise_at(next_tick_deadline(&rt, &st), TickEvent::Tick);
        }
        if do_housekeep {
            let next = tick_housekeep(&rt, rt.start_instant.elapsed());
            if stop.load(Ordering::Relaxed) {
                break;
            }
            // Park exactly until the next QoS due-instant; nothing pending →
            // idle floor (a later write re-arms via `raise_tick_wake`).
            let deadline = match next {
                Some(due_nanos) => rt.start_instant + Duration::from_nanos(due_nanos),
                None => Instant::now() + SCHEDULER_IDLE_FLOOR,
            };
            handle.raise_at(deadline, TickEvent::Housekeep);
        }
    }
}

/// Per-iteration mutable state of the runtime tick. Held across iterations so
/// the same body ([`run_tick_iteration`]) can be driven from either the
/// dedicated `zdds-tick` thread (default) or an external executor — tokio via
/// [`DcpsRuntime::tick_driver`] / async `spawn_in_tokio`
/// (zerodds-async-1.0 §4).
struct TickState {
    /// Multicast target locator to which we send SPDP beacons.
    mc_target: Locator,
    /// Next instant at which a periodic SPDP announce is due.
    next_announce: Instant,
    /// Number of SPDP announces already sent. Drives the C3 initial
    /// announcement burst: as long as `< initial_announce_count` **and** no
    /// peer discovered yet, announces happen at `initial_announce_period` cadence
    /// instead of the full `spdp_period` — so discovery over lossy/power-save WiFi
    /// does not fail on lost first beacons.
    announces_done: u32,
    /// FastDDS interop: count for the periodic secure-SPDP HEARTBEATs
    /// (0xff0101c2). Must increase, otherwise FastDDS' reader ignores follow-up HBs.
    #[cfg(feature = "security")]
    secure_hb_count: i32,
}

impl TickState {
    fn new(rt: &Arc<DcpsRuntime>) -> Self {
        let mc_target = Locator {
            kind: LocatorKind::UdpV4,
            port: u32::from(
                u16::try_from(spdp_multicast_port(rt.domain_id as u32)).unwrap_or(7400),
            ),
            address: {
                let mut a = [0u8; 16];
                a[12..].copy_from_slice(&rt.config.spdp_multicast_group.octets());
                a
            },
        };
        Self {
            mc_target,
            next_announce: Instant::now(), // immediately at start
            announces_done: 0,
            #[cfg(feature = "security")]
            secure_hb_count: 0,
        }
    }
}

/// One iteration of the runtime's **wire** tick: periodic SPDP announce,
/// SEDP/WLP ticks, per-user-writer + per-user-reader ticks, secondary inbound
/// poll. QoS housekeeping (deadline/lifespan/liveliness) is **not** part of this
/// — each driver calls [`tick_housekeep`] separately (D.5e Phase 3 B-2), so the
/// event-driven scheduler can fire it on its own exact-deadline schedule.
/// Mutable per-iteration state lives in `st`; the caller waits `tick_period`
/// between calls. Factored out of [`tick_loop`] so an external executor can
/// drive the tick without the dedicated thread (zerodds-async-1.0 §4).
fn run_tick_iteration(rt: Arc<DcpsRuntime>, st: &mut TickState) {
    // Monotonic clock relative to runtime start. Used by the SEDP,
    // WLP and user tick alike.
    let elapsed_since_start = rt.start_instant.elapsed();
    let sedp_now = Duration::from_secs(elapsed_since_start.as_secs())
        + Duration::from_nanos(u64::from(elapsed_since_start.subsec_nanos()));

    // --- Periodic SPDP announce ---
    // FU2 cross-vendor (cyclone-trace-documented): a secured participant MUST
    // NOT announce before its security builtins are enabled — otherwise
    // a token-less/non-secure first beacon goes out, which foreign vendors
    // (cyclone: "Non secure remote ... not allowed by security") latch as
    // non-secure and, on the later token beacon, treat ONLY as a QoS update
    // (no security re-evaluation) → the handshake never starts.
    // `config.security.is_some()` = secured runtime; until
    // `enable_security_builtins*` installs the stack (snapshot Some) +
    // sets the token/security-info on the beacon, we hold the beacon
    // back. enable() triggers the first token-carrying beacon via
    // `announce_spdp_now()`. Plain runtimes (security None) announce
    // immediately as before.
    #[cfg(feature = "security")]
    let security_pending = rt.config.security.is_some() && rt.security_builtin_snapshot().is_none();
    #[cfg(not(feature = "security"))]
    let security_pending = false;
    if Instant::now() >= st.next_announce && !security_pending {
        let secured_beacon: Option<Vec<u8>> = {
            if let Ok(mut beacon) = rt.spdp_beacon.lock() {
                beacon
                    .serialize()
                    .ok()
                    .and_then(|d| secure_outbound_bytes(&rt, &d).map(|c| c.to_vec()))
            } else {
                None
            }
        };
        if let Some(secured) = secured_beacon {
            let _ = rt.spdp_mc_tx.send(&st.mc_target, &secured);
            // C1 multicast-free discovery: additionally to all configured
            // initial peers (ZERODDS_PEERS) — bootstrap without multicast.
            rt.send_spdp_to_initial_peers(&secured);
            // SPDP unicast fan-out to discovered peers (analogous to WLP-M-2/H-3-H-4):
            // codepit-LXC multicast is flaky; if it loses the tokened
            // secure beacon, the peer never discovers ZeroDDS as secure and
            // NEVER starts the auth handshake (cyclone→ZeroDDS responder hung
            // exactly here: HS_DISPATCH=0). From the metatraffic recv socket
            // (spdp_unicast), so the source port is correct.
            // Periodic directed unicast fan-out to discovered peers:
            // codepit-LXC multicast is flaky; if it loses the tokened
            // beacon, the peer never discovers ZeroDDS as secure and never starts
            // the auth handshake. The unicast refresh (every spdp_period) robustly
            // covers lost multicasts + late joiners. (Previously disabled for a
            // flaky-diag experiment — reactivated as a regular path,
            // complements the event-driven directed response in handle_spdp_datagram.)
            for loc in wlp_unicast_targets(&rt.discovered_participants()) {
                let _ = rt.spdp_unicast.send(&loc, &secured);
            }
        }
        // FastDDS interop: announce in parallel on the reliable secure-SPDP writer
        // (0xff0101c2). FastDDS announces its full secured
        // participant data over this channel and gates the crypto-token
        // reciprocation on it; without our secure SPDP it never sees ZeroDDS there
        // and reciprocates no datawriter/datareader tokens.
        #[cfg(feature = "security")]
        if rt.config.enable_secure_spdp {
            let secure_beacon: Option<Vec<u8>> = {
                if let Ok(mut beacon) = rt.spdp_beacon.lock() {
                    beacon
                        .serialize_secure()
                        .ok()
                        .and_then(|d| protect_secure_spdp(&rt, &d))
                        .and_then(|d| secure_outbound_bytes(&rt, &d).map(|c| c.to_vec()))
                } else {
                    None
                }
            };
            if let Some(secured) = secure_beacon {
                let _ = rt.spdp_mc_tx.send(&st.mc_target, &secured);
                for loc in wlp_unicast_targets(&rt.discovered_participants()) {
                    let _ = rt.spdp_unicast.send(&loc, &secured);
                }
            }
            // Secure-SPDP HEARTBEAT per peer (INFO_DST), so FastDDS' reader
            // — even as a late joiner — is solicited to a (preemptive) ACKNACK
            // and matches our 0xff0101c2 writer. Without a HEARTBEAT
            // FastDDS does not engage our writer (fastdds->zerodds: 0 ACKNACK).
            st.secure_hb_count = st.secure_hb_count.wrapping_add(1);
            for p in rt.discovered_participants() {
                let peer_prefix = p.data.guid.prefix;
                if let Some(hb) =
                    build_secure_spdp_heartbeat(rt.guid_prefix, peer_prefix, st.secure_hb_count)
                {
                    for loc in wlp_unicast_targets(core::slice::from_ref(&p)) {
                        let _ = rt.spdp_unicast.send(&loc, &hb);
                    }
                }
            }
        }
        // C3 WiFi robustness — initial announcement burst: as long as we have
        // not discovered a peer yet and the burst count is not exhausted,
        // announce at the fast `initial_announce_period` cadence. Over
        // lossy/power-save WiFi the first beacons often get lost in the cold-start
        // or sleep window; a single announce + 5s period
        // then leads to `participants=0`. The burst keeps the NIC awake through
        // frequent TX, keeps the stateful-firewall pinhole open and
        // elicits directed SPDP responses that arrive in the wake windows
        // — analogous to FastDDS `initial_announcements`. As soon as a peer
        // is discovered, the cadence falls back to the full `spdp_period`.
        st.announces_done = st.announces_done.saturating_add(1);
        rt.spdp_announce_seq.fetch_add(1, Ordering::Relaxed);
        let still_searching = st.announces_done < rt.config.initial_announce_count
            && rt.discovered_participants().is_empty();
        let period = if still_searching {
            rt.config.initial_announce_period
        } else {
            rt.config.spdp_period
        };
        st.next_announce = Instant::now() + period;
    }

    // (SPDP multicast recv: now in `recv_spdp_multicast_loop`.)

    // --- SEDP-Tick (outbound HEARTBEAT/Resend/ACKNACK) ---
    let sedp_outbound = {
        if let Ok(mut sedp) = rt.sedp.lock() {
            sedp.tick(sedp_now).unwrap_or_default()
        } else {
            Vec::new()
        }
    };
    for dg in sedp_outbound {
        // Protected discovery: SEC_*-protect secure-SEDP DATA/HEARTBEAT/GAP
        // (participant data key). Non-secure SEDP goes unchanged; on a
        // crypto error on secure SEDP it is dropped (no plaintext leak).
        #[cfg(feature = "security")]
        {
            if let Some(inner) = protect_sedp_outbound(&rt, &dg.bytes) {
                // discovery_protection has SEC-wrapped the secure SEDP per-submessage
                // (SEC_PREFIX/BODY/POSTFIX, per-endpoint key). Under
                // rtps_protection SRTPS MUST additionally go on top — BOTH layers,
                // like cyclone<->cyclone (reference pcap: 0x "clear submsg from
                // protected src"). send_discovery_datagram -> secure_outbound_bytes
                // would classify the SEC_PREFIX datagram as volatile-Kx (which is
                // RIGHTLY SRTPS-exempt, because its key only comes over the volatile
                // itself) and skip SRTPS -> cyclone would see the
                // secure SEDP clear, discard ACKNACK/HEARTBEAT as "clear submsg
                // from protected src" and never re-send the SubscriptionData ->
                // ZeroDDS' writer never matches cyclone's reader (wait_for_matched
                // timeout). Hence wrap SRTPS EXPLICITLY here instead of via the
                // generic exempt heuristic.
                let final_bytes: Option<Vec<u8>> = match &rt.config.security {
                    Some(gate)
                        if gate.rtps_protection().unwrap_or(ProtectionLevel::None)
                            != ProtectionLevel::None =>
                    {
                        gate.transform_outbound(&inner).ok()
                    }
                    _ => Some(inner),
                };
                if let Some(fb) = final_bytes {
                    for t in dg.targets.iter() {
                        if is_routable_user_locator(t) {
                            let _ = rt.spdp_unicast.send(t, &fb);
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "security"))]
        send_discovery_datagram(&rt, &dg.targets, &dg.bytes);
    }

    // --- Security-Builtin-Tick ---
    // Volatile-Secure-Writer heartbeats + Volatile-Secure-Reader
    // ACKNACK/NACK_FRAG. Stateless hat keinen Tick (BestEffort).
    if let Some(stack) = rt.security_builtin_snapshot() {
        let outbound = {
            if let Ok(mut s) = stack.lock() {
                // `out` is only mutated under feature="security" (reassign +
                // extend in the cfg block below); otherwise unused_mut in the no-security build.
                #[allow(unused_mut)]
                let mut out = s.poll(sedp_now).unwrap_or_default();
                #[cfg(feature = "security")]
                if rt.config.security.is_some() {
                    // STABLE peer list: `completed_peer_prefixes()` reads
                    // `self.handshakes`, which is GC'd after handshake completion
                    // → the LATE volatile RESENDS/HEARTBEATs (tick, long after
                    // completion) would then find NO peer anymore (`peers.len()!=1`)
                    // and go out CLEAR → cyclone discards them as "clear
                    // submsg from protected src". The stabler `authenticated_peer_
                    // prefixes()` (the installed Kx key stays) — identical to the
                    // token-send tick further below.
                    let peers: Vec<GuidPrefix> = rt
                        .config
                        .security
                        .as_ref()
                        .map(|g| {
                            g.authenticated_peer_prefixes()
                                .into_iter()
                                .map(GuidPrefix::from_bytes)
                                .collect()
                        })
                        .unwrap_or_default();
                    // The reliable volatile submessages from poll() (DATA RESENDS
                    // + HEARTBEAT + GAP) must — like the first send — be SEC_*-
                    // protected (§8.4.2.4, all writer submessages incl.
                    // HEARTBEAT). protect_volatile_datagram now protects all
                    // is_protected_writer_submessage. With exactly one peer
                    // (bench) with its Kx key.
                    if peers.len() == 1 {
                        let pk = peers[0].to_bytes();
                        out = out
                            .into_iter()
                            .filter_map(|dg| {
                                protect_volatile_datagram(&rt, &dg.bytes, &pk).map(|bytes| {
                                    zerodds_rtps::message_builder::OutboundDatagram {
                                        bytes,
                                        targets: dg.targets,
                                    }
                                })
                            })
                            .collect();
                    }
                    // FU2 step 6b: send per-endpoint datawriter/datareader crypto
                    // tokens to every authenticated peer as soon as the
                    // local user endpoints exist.
                    //
                    // STABLE peer list instead of `completed_peer_prefixes()`: the
                    // handshake entry is GC'd after completion, so a
                    // late-matching user writer/reader (user endpoints match
                    // AFTER the secure SEDP) would find no tick window in which
                    // its per-endpoint token would go out — the peer could then never
                    // decode ZeroDDS' user DATA (#29). `authenticated_peer_
                    // prefixes()` (the installed data key) stays.
                    let token_peers: Vec<GuidPrefix> = rt
                        .config
                        .security
                        .as_ref()
                        .map(|g| {
                            g.authenticated_peer_prefixes()
                                .into_iter()
                                .map(GuidPrefix::from_bytes)
                                .collect()
                        })
                        .unwrap_or_default();
                    for prefix in token_peers {
                        // Per-token dedup (#29): each per-endpoint token
                        // exactly once — builtins early, user endpoints
                        // as soon as they match. A per-peer guard would
                        // block late-matched user endpoints forever.
                        let already = rt
                            .endpoint_tokens_sent
                            .read()
                            .map(|set| set.clone())
                            .unwrap_or_default();
                        let pending = pending_endpoint_tokens(
                            prepare_endpoint_crypto_tokens(&rt, prefix),
                            &already,
                        );
                        for ep_msg in pending {
                            let key = endpoint_token_key(&ep_msg);
                            out.extend(protect_volatile_outbound(
                                &rt,
                                prefix,
                                s.volatile_writer
                                    .write_with_heartbeat(&ep_msg, sedp_now)
                                    .unwrap_or_default(),
                            ));
                            if let Ok(mut set) = rt.endpoint_tokens_sent.write() {
                                set.insert(key);
                            }
                        }
                    }
                }
                out
            } else {
                Vec::new()
            }
        };
        for dg in outbound {
            send_discovery_datagram(&rt, &dg.targets, &dg.bytes);
        }
    }

    // --- WLP-Tick (Writer-Liveliness-Protocol Heartbeats) ---
    //
    // RTPS 2.5 §8.4.13: WLP heartbeats are metatraffic.
    // Spec recommendation: multicast to all known peers, one
    // heartbeat per `lease_duration / 3`. We send via the
    // SPDP multicast sender — that is the same socket that
    // sends out the SPDP beacons, and it ensures that all
    // peers see the WLP pulses without the runtime having to
    // look up a unicast locator per peer.
    let wlp_outbound = {
        if let Ok(mut wlp) = rt.wlp.lock() {
            // Use the secure-WLP entity when liveliness_protection != NONE
            // (set idempotently per tick — follows the current governance).
            wlp.set_secure(wlp_liveliness_protected(&rt));
            wlp.tick(sedp_now).unwrap_or(None)
        } else {
            None
        }
    };
    if let Some(bytes) = wlp_outbound {
        // Under liveliness_protection != NONE the secure-WLP DATA is protected
        // with the participant key (§8.4.2.4); otherwise rtps-level/plaintext.
        if let Some(secured) = protect_wlp_outbound(&rt, &bytes) {
            // Multicast to all peers (spec recommendation §8.4.13)...
            let _ = rt.spdp_mc_tx.send(&st.mc_target, &secured);
            // ...plus unicast to every discovered peer (M-2), so WLP also
            // arrives without multicast (container/cloud). From the metatraffic recv
            // socket (spdp_unicast), so the source port is correct (cf. H-3/H-4).
            for loc in wlp_unicast_targets(&rt.discovered_participants()) {
                let _ = rt.spdp_unicast.send(&loc, &secured);
            }
        }
    }

    // (Metatraffic unicast recv: now in `recv_metatraffic_loop`.)

    // --- User-Writer-Tick (HEARTBEAT + Resends) ---
    //
    // Security: per-target serializer. A datagram can go to
    // multiple reader locators. Per target we pull it
    // individually through `secure_outbound_for_target`, so the
    // wire payload matches the protection class of the respective reader.
    let user_writer_outbound: Vec<(EntityId, _)> = {
        let mut all = Vec::new();
        for (eid, arc) in rt.writer_slots_snapshot() {
            if let Ok(mut slot) = arc.lock() {
                if let Ok(dgs) = slot.writer.tick(sedp_now) {
                    for dg in dgs {
                        all.push((eid, dg));
                    }
                }
            }
        }
        all
    };
    for (writer_eid, dg) in user_writer_outbound {
        for t in dg.targets.iter() {
            if !is_routable_user_locator(t) {
                continue;
            }
            if let Some(secured) = secure_outbound_for_target(&rt, writer_eid, &dg.bytes, t) {
                send_on_best_interface(&rt, t, &secured);
            }
        }
    }

    // --- User-Reader-Tick-Outbound (ACKNACK / NACK_FRAG) ---
    let user_reader_outbound: Vec<_> = {
        let mut all = Vec::new();
        for (_eid, arc) in rt.reader_slots_snapshot() {
            if let Ok(mut slot) = arc.lock() {
                if let Ok(dgs) = slot.reader.tick_outbound(sedp_now) {
                    all.extend(dgs);
                }
            }
        }
        all
    };
    for dg in user_reader_outbound {
        if let Some(secured) = protect_user_reader_datagram(&rt, &dg.bytes) {
            for t in dg.targets.iter() {
                if is_routable_user_locator(t) {
                    let _ = rt.user_unicast.send(t, &secured);
                }
            }
        }
    }

    // (User-data unicast recv: now in `recv_user_data_loop`.)

    // --- Per-interface inbound ---
    //
    // Each pool binding is polled non-blocking; the
    // received datagram goes through `secure_inbound_bytes` with
    // the matching NetInterface class. This lets the
    // PolicyEngine make interface-specific decisions
    // (e.g. accept loopback-plain on a protected domain).
    //
    // The non-blocking semantics are achieved by each socket
    // in `bind_all` holding a short read timeout — see
    // `OutboundSocketPool::bind_all`. Without a timeout the
    // event loop would hang on an empty binding per tick.
    #[cfg(feature = "security")]
    if let Some(pool) = &rt.outbound_pool {
        for binding in &pool.bindings {
            while let Ok(dg) = binding.socket.recv() {
                let iface = binding.spec.kind.clone();
                if let Some(clear) = secure_inbound_bytes(&rt, &dg.data, &iface) {
                    // Try SPDP first (reverse beacons), then
                    // SEDP, then user data — same dispatch as
                    // for the legacy sockets.
                    handle_spdp_datagram(&rt, &clear);
                    let events = rt
                        .sedp
                        .lock()
                        .ok()
                        .and_then(|mut s| s.handle_datagram(&clear, sedp_now).ok());
                    if let Some(ev) = events {
                        if !ev.is_empty() {
                            run_matching_pass(&rt);
                            push_sedp_events_to_builtin_readers(&rt, &ev);
                        }
                    }
                    if !dispatch_type_lookup_datagram(&rt, &clear, &dg.source) {
                        handle_user_datagram(&rt, &clear, sedp_now);
                    }
                    // DDS-Security 1.2 §7.4.2 Builtin-Endpoints
                    for dg in dispatch_security_builtin_datagram(&rt, &clear, sedp_now) {
                        send_discovery_datagram(&rt, &dg.targets, &dg.bytes);
                    }
                }
            }
        }
    }

    // Housekeeping (deadline/lifespan/liveliness) runs as a separate
    // `tick_housekeep` call of the respective driver (tick_loop /
    // tick_driver / scheduler_tick_loop) — see `tick_housekeep`.

    // Diagnostic: mark this iteration complete so `tick_count()` advances
    // whether driven by the internal thread or an external executor.
    rt.tick_seq.fetch_add(1, Ordering::Relaxed);
}

/// Min tracker for the earliest "next-due" instant (nanos in the runtime
/// `elapsed` time base) across multiple housekeeping sources.
struct NextDue(Option<u64>);

impl NextDue {
    fn new() -> Self {
        Self(None)
    }
    fn note(&mut self, due_nanos: u64) {
        self.0 = Some(self.0.map_or(due_nanos, |e| e.min(due_nanos)));
    }
    fn into_inner(self) -> Option<u64> {
        self.0
    }
}

/// D.5e Phase 3 B-2 — the time-driven housekeeping checks, factored out of
/// [`run_tick_iteration`], so the event-driven scheduler can fire them
/// as its own [`TickEvent::Housekeep`] heap event exactly at the next
/// due-instant (and `tick_loop`/`tick_driver` call them inline).
/// Pure reader/writer-side bookkeeping — **no** cross-vendor wire
/// output, the cadence is purely internal.
///
/// Return value: the earliest instant (nanos in the `elapsed` time base) at which
/// a check is due again, or `None` if nothing is currently pending
/// (no active deadline/lifespan/liveliness slot) — then the
/// scheduler parks until the idle floor resp. until a `raise_tick_wake` signals new
/// work.
fn tick_housekeep(rt: &Arc<DcpsRuntime>, elapsed: Duration) -> Option<u64> {
    let mut next_due = NextDue::new();
    // --- Deadline-Monitoring ---
    if let Some(d) = check_deadlines(rt, elapsed) {
        next_due.note(d);
    }
    // --- Lifespan-Expire ---
    if let Some(d) = expire_by_lifespan(rt, elapsed) {
        next_due.note(d);
    }
    // --- Liveliness lease check (reader side) ---
    if let Some(d) = check_liveliness(rt, elapsed) {
        next_due.note(d);
    }
    // --- Writer-side liveliness-lost check ---
    if let Some(d) = check_writer_liveliness(rt, elapsed) {
        next_due.note(d);
    }
    next_due.into_inner()
}

impl DcpsRuntime {
    /// Number of completed tick iterations since `start()`. Advances once per
    /// tick regardless of whether the internal `zdds-tick` thread or an
    /// external executor ([`DcpsRuntime::tick_driver`]) drives it — a stalled
    /// value means the periodic tick stopped. Diagnostic only.
    #[must_use]
    pub fn tick_count(&self) -> u64 {
        self.tick_seq.load(Ordering::Relaxed)
    }

    /// Number of SPDP announces emitted since `start()`. Diagnostic for the C3
    /// initial-announcement burst: a fresh participant with no discovered peer
    /// advances this at [`RuntimeConfig::initial_announce_period`] for the first
    /// [`RuntimeConfig::initial_announce_count`] announces, then slows to
    /// `spdp_period`.
    #[must_use]
    pub fn spdp_announce_count(&self) -> u64 {
        self.spdp_announce_seq.load(Ordering::Relaxed)
    }

    /// Number of discovered topic inconsistencies (DDS 1.4 §2.2.4.2.4).
    /// Bumped during matching against the SEDP cache whenever a remote
    /// endpoint carries the same `topic_name` but a differing `type_name`
    /// than a local endpoint. A delta against the last poll snapshot
    /// triggers `on_inconsistent_topic`.
    #[must_use]
    pub fn inconsistent_topic_count(&self) -> u64 {
        self.inconsistent_topic_seq.load(Ordering::Relaxed)
    }

    /// External tick driver (zerodds-async-1.0 §4). Only meaningful when the
    /// runtime was started with [`RuntimeConfig::external_tick`] = `true`,
    /// which suppresses the dedicated `zdds-tick` thread. Each
    /// [`DcpsTickDriver::tick`] call runs exactly one tick iteration; the
    /// caller schedules the next after [`DcpsTickDriver::tick_period`]. The
    /// async API's `spawn_in_tokio` uses this to multiplex many participants'
    /// tick loops onto a tokio runtime instead of one std::thread each.
    #[must_use]
    pub fn tick_driver(self: &Arc<Self>) -> DcpsTickDriver {
        DcpsTickDriver {
            st: TickState::new(self),
            rt: Arc::clone(self),
        }
    }

    /// D.5e Phase 3 — wake the scheduler tick worker immediately (new work:
    /// a sample written, a HEARTBEAT/DATA/ACKNACK received). Coalesced: many
    /// raises between two worker passes collapse into a single wake, so a
    /// datagram storm does not flood the channel. No-op unless started with
    /// `scheduler_tick`.
    pub fn raise_tick_wake(&self) {
        // Only the first raiser since the last pass actually sends.
        if self.tick_wake_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(guard) = self.tick_wake.lock() {
            if let Some(h) = guard.as_ref() {
                // Active traffic wakes the reliable tick AND re-evaluates
                // housekeeping, so a freshly-armed deadline/lifespan/liveliness
                // window is scheduled at once instead of waiting out the park.
                h.raise_now(TickEvent::Tick);
                h.raise_now(TickEvent::Housekeep);
            }
        }
    }

    /// `true` if this participant has any user DataWriter or DataReader — i.e.
    /// the fine-grained periodic work (HEARTBEAT / ACKNACK / deadline / lifespan
    /// / liveliness) may be due and the scheduler keeps a fine cadence. A pure
    /// discovery-only participant parks long.
    fn has_user_endpoints(&self) -> bool {
        self.user_writers
            .read()
            .map(|m| !m.is_empty())
            .unwrap_or(true)
            || self
                .user_readers
                .read()
                .map(|m| !m.is_empty())
                .unwrap_or(true)
    }
}

/// Drives a runtime's periodic tick from an external executor (tokio, an
/// embedded scheduler, a manual test loop). Obtained via
/// [`DcpsRuntime::tick_driver`]; only does useful work when the runtime was
/// started with [`RuntimeConfig::external_tick`] = `true`.
///
/// Typical loop (the async crate's `spawn_in_tokio` shape):
///
/// ```ignore
/// let mut driver = runtime.tick_driver();
/// let period = driver.tick_period();
/// while !driver.is_stopped() {
///     driver.tick();
///     tokio::time::sleep(period).await;
/// }
/// ```
pub struct DcpsTickDriver {
    rt: Arc<DcpsRuntime>,
    st: TickState,
}

impl DcpsTickDriver {
    /// Period the caller should wait between consecutive [`Self::tick`] calls
    /// (mirrors the internal `zdds-tick` thread's `tick_period`).
    #[must_use]
    pub fn tick_period(&self) -> Duration {
        self.rt.config.tick_period
    }

    /// `true` once the runtime is shutting down (set by `Drop`/`stop()`). The
    /// driving task must then stop calling [`Self::tick`] and return so the
    /// runtime can be dropped cleanly.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.rt.stop.load(Ordering::Relaxed)
    }

    /// Run one tick iteration: periodic SPDP announce, SEDP/WLP ticks,
    /// per-user-writer ticks, deadline/lifespan/liveliness checks. Equivalent
    /// to one pass of the internal `zdds-tick` loop body.
    pub fn tick(&mut self) {
        run_tick_iteration(Arc::clone(&self.rt), &mut self.st);
        tick_housekeep(&self.rt, self.rt.start_instant.elapsed());
    }
}

/// Writer-side liveliness-lost detection. Spec §2.2.4.2.10.
///
/// For all user writers: if a lease duration is set and more time
/// has elapsed since the last assert (Automatic = `last_write`, Manual =
/// `last_liveliness_assert`) than the
/// lease duration allows, the writer counts as
/// "not-alive" from the DDS view — `liveliness_lost_count++` and reset the window.
///
/// Note: with pure best-effort tests + `Automatic` the
/// counter typically does not advance — Automatic asserts with every
/// `write_user_sample`. Manual mode requires an explicit
/// `assert_liveliness` (comes with .4b — until then we already provide
/// the detection here, the hot-path trigger triggers it).
fn check_writer_liveliness(rt: &Arc<DcpsRuntime>, now: std::time::Duration) -> Option<u64> {
    let now_nanos = now.as_nanos() as u64;
    let mut next_due = NextDue::new();
    for (_eid, arc) in rt.writer_slots_snapshot() {
        let Ok(mut slot) = arc.lock() else { continue };
        if slot.liveliness_lease_nanos == 0 {
            continue;
        }
        let last = match slot.liveliness_kind {
            zerodds_qos::LivelinessKind::Automatic => slot.last_write,
            _ => slot.last_liveliness_assert,
        };
        let last_nanos = match last {
            Some(t) => t.as_nanos() as u64,
            None => continue,
        };
        if now_nanos.saturating_sub(last_nanos) >= slot.liveliness_lease_nanos {
            slot.liveliness_lost_count = slot.liveliness_lost_count.saturating_add(1);
            // Reset the window, so the same lease-window
            // overrun does not count in an infinite loop.
            // Spec §2.2.3.11: "lease has elapsed" — `>=` is boundary-
            // stable and avoids flakiness when tick_period == lease.
            slot.last_liveliness_assert = Some(now);
            slot.last_write = Some(now);
            next_due.note(now_nanos.saturating_add(slot.liveliness_lease_nanos));
        } else {
            next_due.note(last_nanos.saturating_add(slot.liveliness_lease_nanos));
        }
    }
    next_due.into_inner()
}

/// Checks for all user readers whether the writer has delivered no sample
/// for longer than `lease_duration`. If so: transition
/// alive → not_alive, `not_alive_count++`.
///
/// Automatic liveliness (§2.2.3.11): every write is an implicit assert.
/// So we check the reader-side `last_sample_received`.
/// Manual kinds come with .4b (explicit assert messages).
fn check_liveliness(rt: &Arc<DcpsRuntime>, now: std::time::Duration) -> Option<u64> {
    let now_nanos = now.as_nanos() as u64;
    let mut next_due = NextDue::new();
    for (_eid, arc) in rt.reader_slots_snapshot() {
        let Ok(mut slot) = arc.lock() else { continue };
        if slot.liveliness_lease_nanos == 0 {
            continue;
        }
        // Until the first sample: consider it alive (optimistic).
        let last = match slot.last_sample_received {
            Some(t) => t.as_nanos() as u64,
            None => continue,
        };
        // Only a still-alive reader can transition; one already
        // not_alive stays so until a new sample arrives (event-driven
        // via the recv path) — so no re-schedule needed.
        if !slot.liveliness_alive {
            continue;
        }
        if now_nanos.saturating_sub(last) >= slot.liveliness_lease_nanos {
            slot.liveliness_alive = false;
            slot.liveliness_not_alive_count = slot.liveliness_not_alive_count.saturating_add(1);
        } else {
            next_due.note(last.saturating_add(slot.liveliness_lease_nanos));
        }
    }
    next_due.into_inner()
}

/// For all user writers: remove samples from the HistoryCache whose
/// insert time + lifespan has elapsed. OMG DDS 1.4 §2.2.3.16:
/// "If the duration...elapses and the sample is still in the cache...
/// the sample is no longer available to any future DataReaders".
///
/// Implementation: `sample_insert_times` is a VecDeque, sorted
/// by insert time (= SN, because monotonic). Front-pop while expired;
/// the highest expired SN runs through via `cache.remove_up_to(sn + 1)`.
fn expire_by_lifespan(rt: &Arc<DcpsRuntime>, now: std::time::Duration) -> Option<u64> {
    let now_nanos = now.as_nanos() as u64;
    let mut next_due = NextDue::new();
    for (_eid, arc) in rt.writer_slots_snapshot() {
        let Ok(mut slot) = arc.lock() else { continue };
        if slot.lifespan_nanos == 0 {
            continue;
        }
        let mut highest_expired = None;
        while let Some(&(sn, inserted)) = slot.sample_insert_times.front() {
            let inserted_nanos = inserted.as_nanos() as u64;
            if now_nanos.saturating_sub(inserted_nanos) >= slot.lifespan_nanos {
                highest_expired = Some(sn);
                slot.sample_insert_times.pop_front();
            } else {
                break;
            }
        }
        if let Some(sn) = highest_expired {
            let _removed = slot
                .writer
                .remove_samples_up_to(zerodds_rtps::wire_types::SequenceNumber(sn.0 + 1));
        }
        // Next lifespan due = expiry of the now-oldest sample still
        // remaining in the cache. Empty deque → nothing due,
        // until a new sample is written (raise_tick_wake covers that).
        if let Some(&(_sn, inserted)) = slot.sample_insert_times.front() {
            next_due.note((inserted.as_nanos() as u64).saturating_add(slot.lifespan_nanos));
        }
    }
    next_due.into_inner()
}

/// Checks for all user writers + user readers whether the deadline period
/// has been exceeded since the last sample. Every exceedance
/// increments the corresponding missed counter by exactly 1
/// — regardless of how often `check_deadlines` is called within an
/// elapsed window, because we keep setting `last_*`
/// to "now" after we have counted.
///
/// **Init-state semantics:** as long as `last_write`/`last_sample_received`
/// is `None` (no real write/sample yet), the deadline
/// check does not count. Only after the first real data point does the
/// deadline window start. This prevents false misses due to slow
/// entity setup (Linux CI/container) before the app even issues a
/// write.
fn check_deadlines(rt: &Arc<DcpsRuntime>, now: std::time::Duration) -> Option<u64> {
    let now_nanos = now.as_nanos() as u64;
    let mut next_due = NextDue::new();
    for (_eid, arc) in rt.writer_slots_snapshot() {
        let Ok(mut slot) = arc.lock() else { continue };
        if slot.deadline_nanos == 0 {
            continue;
        }
        let Some(last) = slot.last_write.map(|d| d.as_nanos() as u64) else {
            // Never written yet → deadline window not active.
            continue;
        };
        if now_nanos.saturating_sub(last) >= slot.deadline_nanos {
            slot.offered_deadline_missed_count =
                slot.offered_deadline_missed_count.saturating_add(1);
            // Reset the window: the next deadline is counted relative
            // to the current tick. `>=` is boundary-stable
            // (Spec §2.2.3.7: "deadline has elapsed").
            slot.last_write = Some(now);
            next_due.note(now_nanos.saturating_add(slot.deadline_nanos));
        } else {
            next_due.note(last.saturating_add(slot.deadline_nanos));
        }
    }
    for (_eid, arc) in rt.reader_slots_snapshot() {
        let Ok(mut slot) = arc.lock() else { continue };
        if slot.deadline_nanos == 0 {
            continue;
        }
        let Some(last) = slot.last_sample_received.map(|d| d.as_nanos() as u64) else {
            continue;
        };
        if now_nanos.saturating_sub(last) >= slot.deadline_nanos {
            slot.requested_deadline_missed_count =
                slot.requested_deadline_missed_count.saturating_add(1);
            slot.last_sample_received = Some(now);
            next_due.note(now_nanos.saturating_add(slot.deadline_nanos));
        } else {
            next_due.note(last.saturating_add(slot.deadline_nanos));
        }
    }
    next_due.into_inner()
}

/// For all local writers + readers: matching against the current
/// SEDP cache. A cheap re-run when SEDP events came in — idempotent,
/// because ReliableWriter/Reader add_*_proxy are idempotent (same
/// GUID → replaced).
fn run_matching_pass(rt: &Arc<DcpsRuntime>) {
    let writer_ids: Vec<EntityId> = rt.writer_eids();
    for eid in writer_ids {
        rt.match_local_writer_against_cache(eid);
    }
    let reader_ids: Vec<EntityId> = rt.reader_eids();
    for eid in reader_ids {
        rt.match_local_reader_against_cache(eid);
    }
}

/// Returns the default-unicast locator of a discovered remote
/// participant.
fn remote_user_locators(
    prefix: GuidPrefix,
    discovered: &Arc<Mutex<DiscoveredParticipantsCache>>,
) -> Vec<Locator> {
    match discovered.lock() {
        Ok(cache) => cache
            .get(&prefix)
            .and_then(|p| p.data.default_unicast_locator)
            .into_iter()
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Determine the destination for user traffic to a remote endpoint.
///
/// DDSI-RTPS 2.5 §8.5.3.2/§8.5.3.3: the per-endpoint `unicastLocatorList`
/// from the SEDP announce is authoritative. §8.5.5: only when it is empty
/// does the sender fall back to the participant `DEFAULT_UNICAST_LOCATOR` from
/// SPDP.
///
/// Before this fix ZeroDDS *always* used the participant default — which
/// broke OpenDDS interop: OpenDDS stores only the
/// placeholder 127.0.0.1:12345 as the participant default and announces the real user locator
/// exclusively per-endpoint.
fn endpoint_or_default_locators(
    endpoint: &[Locator],
    prefix: GuidPrefix,
    discovered: &Arc<Mutex<DiscoveredParticipantsCache>>,
) -> Vec<Locator> {
    if !endpoint.is_empty() {
        return endpoint.to_vec();
    }
    remote_user_locators(prefix, discovered)
}

/// Dispatches a received RTPS datagram to matching user readers.
/// Decides, based on the `reader_id` in DATA/DATA_FRAG/HEARTBEAT/GAP,
/// which local reader is responsible.
/// Strip the 4-byte encapsulation header off the received sample payload.
/// Returns `None` if the payload is < 4 bytes or carries an unknown
/// scheme (PL_CDR variants would not get here; they go via
/// SEDP — if we see such a thing on user endpoints, it is garbage).
/// Spec §3.2 zerodds-async-1.0: wakes a registered waker
/// after every `sample_tx.send`. `take` consumes the waker, to
/// avoid double wakeups — the caller registers a new one after
/// every `Pending` result.
fn wake_async_waker(slot: &alloc::sync::Arc<std::sync::Mutex<Option<core::task::Waker>>>) {
    if let Ok(mut g) = slot.lock() {
        if let Some(w) = g.take() {
            w.wake();
        }
    }
}

/// Converts a sample delivered by the ReliableReader into a
/// `UserSample` channel entry. For `ChangeKind::Alive` the
/// CDR encapsulation header is stripped; for lifecycle markers
/// the key hash is reconstructed from the bytes.
/// Inspect-endpoint tap dispatch for the DCPS receive path.
///
/// Called in `handle_user_datagram` when a sample is delivered to
/// a user reader. Only when the `inspect` feature is
/// on; without the feature no code, no branch.
#[cfg(feature = "inspect")]
fn dispatch_inspect_dcps_receive_tap(topic: &str, reader_id: EntityId, item: &UserSample) {
    let payload: Vec<u8> = match item {
        UserSample::Alive { payload, .. } => payload.to_vec(),
        UserSample::Lifecycle { key_hash, .. } => key_hash.to_vec(),
    };
    let ts_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let mut corr: u64 = 0;
    for (i, byte) in reader_id.entity_key.iter().enumerate() {
        corr |= u64::from(*byte) << (i * 8);
    }
    corr |= u64::from(reader_id.entity_kind as u8) << 24;
    let frame = zerodds_inspect_endpoint::Frame::dcps(topic.to_owned(), ts_ns, corr, payload);
    zerodds_inspect_endpoint::tap::dispatch(&frame);
}

fn delivered_to_user_sample(
    sample: &zerodds_rtps::reliable_reader::DeliveredSample,
    writer_strengths: &alloc::collections::BTreeMap<[u8; 16], i32>,
) -> Option<UserSample> {
    use zerodds_rtps::history_cache::ChangeKind;
    match sample.kind {
        ChangeKind::Alive | ChangeKind::AliveFiltered => {
            let writer_guid = sample.writer_guid.to_bytes();
            let writer_strength = writer_strengths.get(&writer_guid).copied().unwrap_or(0);
            // Encapsulation representation from byte[1] of the header
            // (RTPS 2.5 §10.5) — BEFORE stripping. 0x00–0x03 = XCDR1
            // (CDR/PL_CDR), 0x06–0x0b = XCDR2 (CDR2/D_CDR2/PL_CDR2).
            let representation = encap_representation(&sample.payload);
            let big_endian = encap_big_endian(&sample.payload);
            strip_user_encap_arc(&sample.payload).map(|payload| UserSample::Alive {
                payload,
                writer_guid,
                writer_strength,
                representation,
                big_endian,
                source_timestamp: sample.source_timestamp,
            })
        }
        ChangeKind::NotAliveDisposed
        | ChangeKind::NotAliveUnregistered
        | ChangeKind::NotAliveDisposedUnregistered => {
            // Lifecycle marker: Spec §9.6.4.8 + §9.6.3.9 requires
            // `PID_KEY_HASH` in the inline QoS — the reader reads it
            // and propagates it via `DeliveredSample.key_hash`.
            // Fallback: with non-spec-conformant writers the
            // hash falls back to the first 16 bytes of the key-only payload
            // (PLAIN_CDR2-BE key holder).
            let kh = sample.key_hash.unwrap_or_else(|| {
                let mut h = [0u8; 16];
                let n = sample.payload.len().min(16);
                h[..n].copy_from_slice(&sample.payload[..n]);
                h
            });
            Some(UserSample::Lifecycle {
                key_hash: kh,
                kind: sample.kind,
            })
        }
    }
}

/// Returns the XCDR version from the 4-byte encapsulation header
/// (RTPS 2.5 §10.5): `0` = XCDR1 (CDR/PL_CDR, encap byte 0x00–0x05),
/// `1` = XCDR2 (CDR2/DELIMITED_CDR2/PL_CDR2, encap byte 0x06–0x0b).
/// Default `0` for a too-short payload — XCDR1 is the spec baseline.
fn encap_representation(payload: &[u8]) -> u8 {
    if payload.len() >= 2 && payload[1] >= 0x06 {
        1
    } else {
        0
    }
}

/// Returns the byte order from the 4-byte encapsulation representation
/// identifier (RTPS 2.5 §10.5). The repr-id is a big-endian `uint16`; its low
/// bit selects the byte order — the `_BE` variants (CDR_BE 0x0000, PL_CDR_BE
/// 0x0002, CDR2_BE 0x0006, D_CDR2_BE 0x0008, PL_CDR2_BE 0x000a) are even, the
/// `_LE` variants odd. `true` ⇒ big-endian. A too-short / header-less payload
/// (e.g. the intra-runtime bare body) defaults to little-endian (`false`).
fn encap_big_endian(payload: &[u8]) -> bool {
    payload.len() >= 2 && (payload[1] & 0x01) == 0
}

/// Checks whether `payload` has a known 4-byte encapsulation header.
/// Returns `Some(4)` if so (= offset behind the header), `None` if
/// no known scheme. Separated in use from [`strip_user_encap`]:
/// here only validation without allocation, for the listener zero-copy
/// path (lever E / Sprint D.5d).
fn validate_user_encap_offset(payload: &[u8]) -> Option<usize> {
    if payload.len() < 4 {
        return None;
    }
    // Accept all data-representation schemes (RTPS 2.5 §10.5,
    // table 10.3): byte0 = 0x00, byte1 in:
    //   0x00/0x01 CDR_BE/LE        (XCDR1 PLAIN_CDR)
    //   0x02/0x03 PL_CDR_BE/LE     (XCDR1 parameter list, key serial.)
    //   0x06/0x07 CDR2_BE/LE       (XCDR2 PLAIN_CDR2)
    //   0x08/0x09 D_CDR2_BE/LE     (XCDR2 DELIMITED_CDR2, @appendable)
    //   0x0a/0x0b PL_CDR2_BE/LE    (XCDR2 PL_CDR2, @mutable)
    // Cyclone often sends XCDR1, OpenDDS/FastDDS XCDR2. We pass
    // all through; the typed decoder picks the correct alignment rule
    // based on the `representation` (see `encap_representation`).
    if payload[0] != 0x00 {
        return None;
    }
    match payload[1] {
        0x00..=0x03 | 0x06..=0x0b => Some(4),
        _ => None,
    }
}

/// Zero-copy variant: strips the encap header via range slicing
/// on the refcounted `Arc<[u8]>` backing store. No heap alloc.
/// Spec: `docs/specs/zerodds-zero-copy-1.0.md` §6 wave 2.
fn strip_user_encap_arc(
    payload: &alloc::sync::Arc<[u8]>,
) -> Option<crate::sample_bytes::SampleBytes> {
    validate_user_encap_offset(payload).map(|off| {
        crate::sample_bytes::SampleBytes::from_arc_slice(
            alloc::sync::Arc::clone(payload),
            off..payload.len(),
        )
    })
}

#[cfg(test)]
fn strip_user_encap(payload: &[u8]) -> Option<alloc::vec::Vec<u8>> {
    validate_user_encap_offset(payload).map(|off| payload[off..].to_vec())
}

/// Bench-only phase-timing accumulators. Active with env
/// `ZERODDS_PHASE_TIMING=1`. With `ZERODDS_PHASE_DUMP=1` the
/// atexit hook prints the totals on drop of the first runtime.
#[doc(hidden)]
pub static PHASE_HANDLE_USER_NS: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);
#[doc(hidden)]
pub static PHASE_HANDLE_USER_CALLS: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);
#[doc(hidden)]
pub static PHASE_WRITE_USER_NS: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);
#[doc(hidden)]
pub static PHASE_WRITE_USER_CALLS: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// Sub-phases in the `handle_user_datagram` receive hot path:
/// 0=decode_datagram, 1=slot-lookup+lock, 2=reader.handle_data,
/// 3=delivered_to_user_sample, 4=listener+sender-dispatch.
/// Active under `ZERODDS_PHASE_TIMING=1`. Each `Instant::now()` bracket
/// costs ~50 ns; at a ~3 µs handle that is ~1.6% per sub-phase.
#[doc(hidden)]
pub static PHASE_HANDLE_SUB_NS: [core::sync::atomic::AtomicU64; 5] = [
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
];

/// Sub-phases in `write_user_sample_borrowed` (sender hot path):
/// 0=lookup, 1=lock, 2=write_with_heartbeat, 3=send-loop, 4=reserved.
/// The detail drilldown into socket.send_to vs. inproc-peer dispatch was
/// done once for the connected-UDP lever (showed send_to as
/// 97% of the dispatch path); not permanent in the code, because per-phase
/// `Instant::now()` itself costs ~50 ns — at a 6 µs send that
/// would be 1% overhead and skews the calibrated measurement.
#[doc(hidden)]
pub static PHASE_WRITE_SUB_NS: [core::sync::atomic::AtomicU64; 5] = [
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
    core::sync::atomic::AtomicU64::new(0),
];

fn phase_timing_enabled() -> bool {
    static CACHE: core::sync::atomic::AtomicI8 = core::sync::atomic::AtomicI8::new(-1);
    let v = CACHE.load(core::sync::atomic::Ordering::Relaxed);
    if v >= 0 {
        return v == 1;
    }
    let on = std::env::var("ZERODDS_PHASE_TIMING")
        .map(|s| s == "1")
        .unwrap_or(false);
    CACHE.store(
        if on { 1 } else { 0 },
        core::sync::atomic::Ordering::Relaxed,
    );
    on
}

struct PhaseTimer {
    start: std::time::Instant,
    ns_acc: &'static core::sync::atomic::AtomicU64,
    calls_acc: &'static core::sync::atomic::AtomicU64,
}

impl Drop for PhaseTimer {
    fn drop(&mut self) {
        let ns = self.start.elapsed().as_nanos() as u64;
        self.ns_acc
            .fetch_add(ns, core::sync::atomic::Ordering::Relaxed);
        self.calls_acc
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }
}

fn handle_user_datagram(rt: &Arc<DcpsRuntime>, bytes: &[u8], now: Duration) {
    let _phase_guard = if phase_timing_enabled() {
        Some(PhaseTimer {
            start: std::time::Instant::now(),
            ns_acc: &PHASE_HANDLE_USER_NS,
            calls_acc: &PHASE_HANDLE_USER_CALLS,
        })
    } else {
        None
    };
    let pt_on = phase_timing_enabled();
    let pt_t0 = if pt_on {
        Some(std::time::Instant::now())
    } else {
        None
    };
    let parsed = match decode_datagram(bytes) {
        Ok(p) => p,
        Err(_) => return,
    };
    // DDSI-RTPS §8.3.4: the effective source of each writer submessage is the
    // sourceGuidPrefix from the RTPS header. The reader demux needs it to
    // distinguish writer proxies with the same EntityId but a different participant
    // (fan-in / multiple publishers on the same topic).
    let src_prefix = parsed.header.guid_prefix;
    if let (Some(t0), true) = (pt_t0, pt_on) {
        let ns = t0.elapsed().as_nanos() as u64;
        PHASE_HANDLE_SUB_NS[0].fetch_add(ns, core::sync::atomic::Ordering::Relaxed);
    }
    // Per-submessage: take the matching slot mutex individually per
    // submessage — no global user_writers/user_readers lock anymore.
    // With per-submessage granularity, reader datagrams can be processed in parallel
    // to writer AckNacks.
    //
    // RTPS-F1 (DDSI-RTPS §8.3.4 ReceiverState.haveTimestamp): an INFO_TS
    // submessage sets the source timestamp applied to every following DATA in
    // the same message, until another INFO_TS (or an I-flag clears it). We
    // carry it forward and hand it to the reader so it lands in
    // `SampleInfo.source_timestamp`.
    let mut cur_source_ts: Option<zerodds_rtps::header_extension::HeTimestamp> = None;
    for sub in parsed.submessages {
        match sub {
            ParsedSubmessage::InfoTimestamp(its) => {
                cur_source_ts = if its.invalidate {
                    None
                } else {
                    Some(its.timestamp)
                };
            }
            ParsedSubmessage::Data(d) => {
                // Sprint D.5d lever B — collect-then-dispatch:
                // sample conversion + liveliness update inside slot.lock,
                // then listener fire + channel send + waker wake
                // OUTSIDE the lock.
                //
                // Cross-vendor fix 2026-05-19: when reader_id ==
                // ENTITYID_UNKNOWN (RTPS spec §8.3.7.2: "deliver to all
                // matched readers on this topic"), we iterate over
                // ALL reader slots and let `handle_data` filter by
                // writer_proxies. Cyclone DDS/FastDDS/RTI send
                // user DATA with reader_id=UNKNOWN; without this fan-out
                // ZeroDDS would drop every such DATA.
                let pt_t1 = if pt_on {
                    Some(std::time::Instant::now())
                } else {
                    None
                };
                let target_slots: Vec<ReaderSlotArc> = if d.reader_id == EntityId::UNKNOWN {
                    let snap = rt.reader_slots_snapshot();
                    let mut v = Vec::with_capacity(snap.len());
                    v.extend(snap.into_iter().map(|(_, arc)| arc));
                    v
                } else {
                    let mut v = Vec::with_capacity(1);
                    if let Some(arc) = rt.reader_slot(d.reader_id) {
                        v.push(arc);
                    }
                    v
                };
                if let (Some(t1), true) = (pt_t1, pt_on) {
                    let ns = t1.elapsed().as_nanos() as u64;
                    PHASE_HANDLE_SUB_NS[1].fetch_add(ns, core::sync::atomic::Ordering::Relaxed);
                }
                for arc in target_slots {
                    // Lever E: alongside the UserSample we carry a
                    // zero-copy view on the original `Arc<[u8]>` with
                    // the encap offset — the listener can thereby read into
                    // the payload without allocation.
                    let mut items: Vec<UserSampleWithEncap> = Vec::with_capacity(4);
                    let listener;
                    let waker;
                    let sender;
                    #[cfg(feature = "inspect")]
                    let topic_name;
                    let pt_t2 = if pt_on {
                        Some(std::time::Instant::now())
                    } else {
                        None
                    };
                    {
                        let Ok(mut slot) = arc.lock() else { continue };
                        let hd_samples: Vec<_> = slot
                            .reader
                            .handle_data(src_prefix, &d, cur_source_ts)
                            .into_iter()
                            .collect();
                        for sample in hd_samples {
                            // A2 TIME_BASED_FILTER (§2.2.3.12): drop alive samples
                            // that arrive within minimum_separation of the last
                            // delivered sample of the same instance. No-op when
                            // the filter is disabled (tbf_min_separation_nanos==0).
                            if matches!(
                                sample.kind,
                                zerodds_rtps::history_cache::ChangeKind::Alive
                                    | zerodds_rtps::history_cache::ChangeKind::AliveFiltered
                            ) && !slot.tbf_should_deliver(sample.key_hash, now.as_nanos())
                            {
                                continue;
                            }
                            // Listener zero-copy view only for alive samples
                            // with a valid encap header. Arc::clone is
                            // an atomic refcount inc, no data copy.
                            let listener_view: Option<(Arc<[u8]>, usize)> = match sample.kind {
                                zerodds_rtps::history_cache::ChangeKind::Alive
                                | zerodds_rtps::history_cache::ChangeKind::AliveFiltered => {
                                    validate_user_encap_offset(&sample.payload)
                                        .map(|off| (Arc::clone(&sample.payload), off))
                                }
                                _ => None,
                            };
                            if let Some(item) =
                                delivered_to_user_sample(&sample, &slot.writer_strengths)
                            {
                                items.push((item, listener_view));
                            }
                        }
                        if !items.is_empty() {
                            slot.last_sample_received = Some(now);
                            slot.samples_delivered_count = slot
                                .samples_delivered_count
                                .saturating_add(items.len() as u64);
                            if !slot.liveliness_alive {
                                slot.liveliness_alive = true;
                                slot.liveliness_alive_count =
                                    slot.liveliness_alive_count.saturating_add(1);
                            }
                        }
                        listener = slot.listener.clone();
                        waker = Arc::clone(&slot.async_waker);
                        sender = slot.sample_tx.clone();
                        #[cfg(feature = "inspect")]
                        {
                            topic_name = slot.topic_name.clone();
                        }
                    }
                    if let (Some(t2), true) = (pt_t2, pt_on) {
                        let ns = t2.elapsed().as_nanos() as u64;
                        PHASE_HANDLE_SUB_NS[2].fetch_add(ns, core::sync::atomic::Ordering::Relaxed);
                    }
                    let pt_t3 = if pt_on {
                        Some(std::time::Instant::now())
                    } else {
                        None
                    };
                    // --- Outside slot.lock: dispatch ---
                    //
                    // Listener and MPSC are exclusive: if a listener
                    // (callback) is set, the consumer is on the
                    // callback path — the additional `sender.send` +
                    // `wake_async_waker` would be pure overhead AND
                    // would grow the channel buffer unboundedly
                    // (memory leak in callback-only apps). We
                    // dispatch either the callback OR the MPSC, not
                    // both. A caller (Rust API) that wants take()+listener
                    // at the same time simply sets NO listener
                    // and polls via take().
                    for (item, listener_view) in items {
                        let (item_repr, item_be) = if let UserSample::Alive {
                            representation,
                            big_endian,
                            ..
                        } = &item
                        {
                            (*representation, u8::from(*big_endian))
                        } else {
                            (0, 0)
                        };
                        #[cfg(feature = "inspect")]
                        dispatch_inspect_dcps_receive_tap(&topic_name, d.reader_id, &item);
                        if let Some(ref l) = listener {
                            if let Some((arc_payload, off)) = listener_view {
                                // Zero-copy: slice view into the original Arc.
                                l(&arc_payload[off..], item_repr, item_be);
                            }
                        } else {
                            let _ = sender.send(item);
                            wake_async_waker(&waker);
                        }
                    }
                    if let (Some(t3), true) = (pt_t3, pt_on) {
                        let ns = t3.elapsed().as_nanos() as u64;
                        PHASE_HANDLE_SUB_NS[4].fetch_add(ns, core::sync::atomic::Ordering::Relaxed);
                    }
                } // for arc in target_slots
            }
            ParsedSubmessage::DataFrag(df) => {
                // Lever B+E — see the Data arm above.
                // Cross-vendor: same UNKNOWN fan-out as for Data.
                let target_slots: Vec<ReaderSlotArc> = if df.reader_id == EntityId::UNKNOWN {
                    rt.reader_slots_snapshot()
                        .into_iter()
                        .map(|(_, arc)| arc)
                        .collect()
                } else {
                    rt.reader_slot(df.reader_id).into_iter().collect()
                };
                for arc in target_slots {
                    let mut items: Vec<UserSampleWithEncap> = Vec::with_capacity(4);
                    let listener;
                    let waker;
                    let sender;
                    #[cfg(feature = "inspect")]
                    let topic_name;
                    {
                        let Ok(mut slot) = arc.lock() else { continue };
                        for sample in
                            slot.reader
                                .handle_data_frag(src_prefix, &df, now, cur_source_ts)
                        {
                            // A2 TIME_BASED_FILTER (§2.2.3.12) — see DATA path.
                            if matches!(
                                sample.kind,
                                zerodds_rtps::history_cache::ChangeKind::Alive
                                    | zerodds_rtps::history_cache::ChangeKind::AliveFiltered
                            ) && !slot.tbf_should_deliver(sample.key_hash, now.as_nanos())
                            {
                                continue;
                            }
                            let listener_view: Option<(Arc<[u8]>, usize)> = match sample.kind {
                                zerodds_rtps::history_cache::ChangeKind::Alive
                                | zerodds_rtps::history_cache::ChangeKind::AliveFiltered => {
                                    validate_user_encap_offset(&sample.payload)
                                        .map(|off| (Arc::clone(&sample.payload), off))
                                }
                                _ => None,
                            };
                            if let Some(item) =
                                delivered_to_user_sample(&sample, &slot.writer_strengths)
                            {
                                items.push((item, listener_view));
                            }
                        }
                        if !items.is_empty() {
                            slot.last_sample_received = Some(now);
                            slot.samples_delivered_count = slot
                                .samples_delivered_count
                                .saturating_add(items.len() as u64);
                            if !slot.liveliness_alive {
                                slot.liveliness_alive = true;
                                slot.liveliness_alive_count =
                                    slot.liveliness_alive_count.saturating_add(1);
                            }
                        }
                        listener = slot.listener.clone();
                        waker = Arc::clone(&slot.async_waker);
                        sender = slot.sample_tx.clone();
                        #[cfg(feature = "inspect")]
                        {
                            topic_name = slot.topic_name.clone();
                        }
                    }
                    for (item, listener_view) in items {
                        let (item_repr, item_be) = if let UserSample::Alive {
                            representation,
                            big_endian,
                            ..
                        } = &item
                        {
                            (*representation, u8::from(*big_endian))
                        } else {
                            (0, 0)
                        };
                        #[cfg(feature = "inspect")]
                        dispatch_inspect_dcps_receive_tap(&topic_name, df.reader_id, &item);
                        // See the Data arm: listener and MPSC are exclusive.
                        if let Some(ref l) = listener {
                            if let Some((arc_payload, off)) = listener_view {
                                l(&arc_payload[off..], item_repr, item_be);
                            }
                        } else {
                            let _ = sender.send(item);
                            wake_async_waker(&waker);
                        }
                    }
                } // for arc in target_slots (DataFrag)
            }
            ParsedSubmessage::Heartbeat(h) => {
                // Lever B — collect-then-dispatch like the Data arm. An HB can
                // unlock samples that were waiting on a hole fill
                // (volatile skip, historic eviction).
                //
                // D.5e Phase-2: synchronous ACKNACK emit on HB receipt
                // instead of deferred-via-tick. With `heartbeat_response_delay=0`
                // (D.5e default) `tick_outbound(now)` flushes the
                // ACKNACK directly for all pending writer_proxies — the tick loop
                // no longer has to wait 5 ms.
                // Cross-vendor: a HEARTBEAT with reader_id=UNKNOWN is
                // "to all matched readers". Cyclone often packs this into
                // DATA+HB submessage bundles.
                let target_slots: Vec<ReaderSlotArc> = if h.reader_id == EntityId::UNKNOWN {
                    rt.reader_slots_snapshot()
                        .into_iter()
                        .map(|(_, arc)| arc)
                        .collect()
                } else {
                    rt.reader_slot(h.reader_id).into_iter().collect()
                };
                for arc in target_slots {
                    let mut items: Vec<UserSample> = Vec::new();
                    let mut sync_outbound: Vec<zerodds_rtps::message_builder::OutboundDatagram> =
                        Vec::new();
                    let waker;
                    let sender;
                    {
                        let Ok(mut slot) = arc.lock() else { continue };
                        for sample in slot.reader.handle_heartbeat(src_prefix, &h, now) {
                            // A2 TIME_BASED_FILTER (§2.2.3.12) — see DATA path.
                            if matches!(
                                sample.kind,
                                zerodds_rtps::history_cache::ChangeKind::Alive
                                    | zerodds_rtps::history_cache::ChangeKind::AliveFiltered
                            ) && !slot.tbf_should_deliver(sample.key_hash, now.as_nanos())
                            {
                                continue;
                            }
                            if let Some(item) =
                                delivered_to_user_sample(&sample, &slot.writer_strengths)
                            {
                                items.push(item);
                            }
                        }
                        if !items.is_empty() {
                            slot.last_sample_received = Some(now);
                            slot.samples_delivered_count = slot
                                .samples_delivered_count
                                .saturating_add(items.len() as u64);
                            if !slot.liveliness_alive {
                                slot.liveliness_alive = true;
                                slot.liveliness_alive_count =
                                    slot.liveliness_alive_count.saturating_add(1);
                            }
                        }
                        // D.5e Phase-2: synchronous ACKNACK directly in the recv thread.
                        if let Ok(dgs) = slot.reader.tick_outbound(now) {
                            sync_outbound = dgs;
                        }
                        waker = Arc::clone(&slot.async_waker);
                        sender = slot.sample_tx.clone();
                    }
                    for item in items {
                        let _ = sender.send(item);
                        wake_async_waker(&waker);
                    }
                    // Send ACKNACK datagrams synchronously — no tick-quantization tax.
                    for dg in sync_outbound {
                        if let Some(secured) = protect_user_reader_datagram(rt, &dg.bytes) {
                            for t in dg.targets.iter() {
                                if is_routable_user_locator(t) {
                                    let _ = rt.user_unicast.send(t, &secured);
                                }
                            }
                        }
                    }
                } // for arc in target_slots (Heartbeat)
            }
            ParsedSubmessage::Gap(g) => {
                // Cross-vendor: Gap with UNKNOWN reader → fan-out.
                let target_slots: Vec<ReaderSlotArc> = if g.reader_id == EntityId::UNKNOWN {
                    rt.reader_slots_snapshot()
                        .into_iter()
                        .map(|(_, arc)| arc)
                        .collect()
                } else {
                    rt.reader_slot(g.reader_id).into_iter().collect()
                };
                for arc in target_slots {
                    if let Ok(mut slot) = arc.lock() {
                        for sample in slot.reader.handle_gap(src_prefix, &g) {
                            // A2 TIME_BASED_FILTER (§2.2.3.12) — see DATA path.
                            if matches!(
                                sample.kind,
                                zerodds_rtps::history_cache::ChangeKind::Alive
                                    | zerodds_rtps::history_cache::ChangeKind::AliveFiltered
                            ) && !slot.tbf_should_deliver(sample.key_hash, now.as_nanos())
                            {
                                continue;
                            }
                            if let Some(item) =
                                delivered_to_user_sample(&sample, &slot.writer_strengths)
                            {
                                let _ = slot.sample_tx.send(item);
                                wake_async_waker(&slot.async_waker);
                            }
                        }
                    }
                }
            }
            ParsedSubmessage::AckNack(ack) => {
                if let Some(arc) = rt.writer_slot(ack.writer_id) {
                    let mut sync_outbound: Vec<zerodds_rtps::message_builder::OutboundDatagram> =
                        Vec::new();
                    if let Ok(mut slot) = arc.lock() {
                        let base = ack.reader_sn_state.bitmap_base;
                        let requested: Vec<_> = ack.reader_sn_state.iter_set().collect();
                        let src = Guid::new(parsed.header.guid_prefix, ack.reader_id);
                        slot.writer.handle_acknack(src, base, requested);
                        // D.5e Phase-2: synchronous resend on NACK receipt.
                        // An ACKNACK may have listed requested SNs for resend;
                        // tick delivers the resend datagrams directly in the recv thread.
                        if let Ok(dgs) = slot.writer.tick(now) {
                            sync_outbound = dgs;
                        }
                    }
                    // ACK-Event-Cvar: wake `wait_for_acknowledgments`-waiters.
                    rt.notify_ack_event();
                    // Send sync resends (no more tick wait). FU2 S3:
                    // per-target data_protection (a reliable resend of user DATA
                    // must be encrypted just like the immediate send).
                    for dg in sync_outbound {
                        for t in dg.targets.iter() {
                            if is_routable_user_locator(t) {
                                if let Some(secured) =
                                    secure_outbound_for_target(rt, ack.writer_id, &dg.bytes, t)
                                {
                                    let _ = rt.user_unicast.send(t, &secured);
                                }
                            }
                        }
                    }
                }
            }
            ParsedSubmessage::NackFrag(nf) => {
                if let Some(arc) = rt.writer_slot(nf.writer_id) {
                    if let Ok(mut slot) = arc.lock() {
                        let src = Guid::new(parsed.header.guid_prefix, nf.reader_id);
                        slot.writer.handle_nackfrag(src, &nf);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Test hook: allows a direct call of `handle_spdp_datagram` from
/// other modules without spinning up the whole event loop.
/// For internal tests only.
#[cfg(test)]
pub(crate) fn handle_spdp_datagram_for_test(rt: &Arc<DcpsRuntime>, bytes: &[u8]) {
    handle_spdp_datagram(rt, bytes);
}

fn handle_spdp_datagram(rt: &Arc<DcpsRuntime>, bytes: &[u8]) {
    let parsed = match rt.spdp_reader.parse_datagram(bytes) {
        Ok(p) => p,
        Err(_) => return, // not SPDP or wire error — swallow
    };
    // Self-discovery filter: ignore our own beacons.
    if parsed.sender_prefix == rt.guid_prefix {
        return;
    }
    let is_new = {
        if let Ok(mut cache) = rt.discovered.lock() {
            cache.insert(parsed.clone())
        } else {
            false
        }
    };
    // On first discovery: wire the SEDP stack + send out initial
    // announcements.
    if is_new {
        // A1 discovery-server relay: bridge the newly-joined client to every
        // already-known client over a single well-known address. Forwards ONLY
        // raw SPDP (participant locators) — SEDP (endpoint discovery, incl. ROS-2
        // Action endpoints) then proceeds DIRECTLY peer-to-peer, which is exactly
        // why Actions keep working (unlike a SEDP-proxying discovery server).
        // Plain discovery only; secured relay is a follow-up.
        #[cfg(feature = "security")]
        let relay_plain = rt.config.security.is_none();
        #[cfg(not(feature = "security"))]
        let relay_plain = true;
        if rt.config.discovery_server && relay_plain {
            let new_client = wlp_unicast_targets(core::slice::from_ref(&parsed));
            let others: Vec<_> = rt
                .discovered_participants()
                .into_iter()
                .filter(|dp| dp.sender_prefix != parsed.sender_prefix)
                .collect();
            if let Ok(mut relay) = rt.spdp_relay_cache.lock() {
                // 1) tell the new client about every already-known client.
                for dp in &others {
                    if let Some(raw) = relay.get(&dp.sender_prefix) {
                        for loc in &new_client {
                            let _ = rt.spdp_unicast.send(loc, raw);
                        }
                    }
                }
                // 2) tell every already-known client about the new client.
                for dp in &others {
                    for loc in wlp_unicast_targets(core::slice::from_ref(dp)) {
                        let _ = rt.spdp_unicast.send(&loc, bytes);
                    }
                }
                // 3) remember the new client's SPDP for future joiners.
                relay.insert(parsed.sender_prefix, bytes.to_vec());
            }
        }
        if let Ok(mut sedp) = rt.sedp.lock() {
            sedp.on_participant_discovered(&parsed);
        }
        // Event-driven directed SPDP response (§8.5.3): send OUR own
        // SPDP IMMEDIATELY unicast to the newly discovered peer, instead of letting it
        // wait for our next periodic multicast beacon (spdp_period=5s, codepit-LXC
        // multicast flaky). A spec-conformant peer (OpenDDS)
        // processes our auth request ONLY once it has our identity_token from
        // our SPDP — without this directed response it waits up to
        // spdp_period (seconds latency → cross-vendor ping wait_for_matched
        // timeout). NO timeout band-aid: the seconds latency was the missing
        // discovery event. Token-less first beacons (security not yet enabled)
        // are NOT sent (see security_pending in the announce loop) — the
        // periodic/announce_spdp_now path catches up.
        #[cfg(feature = "security")]
        let beacon_ready =
            !(rt.config.security.is_some() && rt.security_builtin_snapshot().is_none());
        #[cfg(not(feature = "security"))]
        let beacon_ready = true;
        if beacon_ready {
            let targets = wlp_unicast_targets(core::slice::from_ref(&parsed));
            if !targets.is_empty() {
                if let Some(secured) = rt
                    .spdp_beacon
                    .lock()
                    .ok()
                    .and_then(|mut b| b.serialize().ok())
                    .and_then(|d| secure_outbound_bytes(rt, &d).map(|c| c.to_vec()))
                {
                    for loc in &targets {
                        let _ = rt.spdp_unicast.send(loc, &secured);
                    }
                }
            }
        }
    }
    // FU2: wire the security builtin stack + kick off the auth handshake.
    // On EVERY beacon (not only is_new): `handle_remote_endpoints` and
    // `begin_handshake_with` are idempotent. This also covers the case
    // that the peer was discovered before the auth plugin was active via
    // `enable_security_builtins_with_auth` — the next
    // beacon refresh then kicks off the handshake. No-op without a plugin,
    // without security bits or without an announced identity_token.
    if let Some(sec) = rt.security_builtin_snapshot() {
        let handshake_dgs = if let Ok(mut s) = sec.lock() {
            s.note_remote_vendor(parsed.sender_prefix, parsed.sender_vendor);
            s.handle_remote_endpoints(&parsed);
            match parsed.data.identity_token.as_ref() {
                Some(token) => s
                    .begin_handshake_with(parsed.sender_prefix, parsed.data.guid.to_bytes(), token)
                    .unwrap_or_default(),
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        for dg in handshake_dgs {
            send_discovery_datagram(rt, &dg.targets, &dg.bytes);
        }
    }
    //  Mirror the SPDP receive into the builtin DCPSParticipant reader.
    // We send on every beacon (also refresh) — Spec §2.2.5.1
    // allows it, take() returns the respective current
    // data to the user. A reader with KEEP_LAST(1) receives only the newest.
    if let Some(sinks) = rt.builtin_sinks_snapshot() {
        let dcps_sample =
            crate::builtin_topics::ParticipantBuiltinTopicData::from_wire(&parsed.data);
        // .7 §2.2.2.2.1.14: drop ignored participants before
        // they fall into the builtin reader.
        if let Some(filter) = rt.ignore_filter_snapshot() {
            let h = crate::instance_handle::InstanceHandle::from_guid(dcps_sample.key);
            if filter.is_participant_ignored(h) {
                return;
            }
        }
        let _ = sinks.push_participant(&dcps_sample);
    }
}

/// Pushes SEDP events (new pubs/subs) into the 4 builtin-topic
/// readers. A new pub/sub produces **two** samples:
///
/// 1. a `DCPSPublication`/`DCPSSubscription` sample,
/// 2. a `DCPSTopic` sample (synthetic from topic name + type name).
///
/// The native SEDP-topics endpoints (RTPS 2.5 §9.3.2.12 bits 28/29)
/// are optional per Spec §8.5.4.4 and covered in ZeroDDS via this
/// synthetic derivation — see also
/// `endpoint_flag::ALL_STANDARD`, which deliberately omits the
/// topics bits. Cyclone/Fast-DDS peers that send their own topic
/// announces are ignored (no reader endpoint).
fn push_sedp_events_to_builtin_readers(
    rt: &Arc<DcpsRuntime>,
    events: &zerodds_discovery::sedp::SedpEvents,
) {
    let Some(sinks) = rt.builtin_sinks_snapshot() else {
        return;
    };
    let filter = rt.ignore_filter_snapshot();
    for w in &events.new_publications {
        let pub_sample = crate::builtin_topics::PublicationBuiltinTopicData::from_wire(w);
        let topic_sample = crate::builtin_topics::TopicBuiltinTopicData::from_publication(w);
        // .7 §2.2.2.2.1.14/.16: consult the participant + publication +
        // topic ignore filters.
        if let Some(f) = &filter {
            let part_h = crate::instance_handle::InstanceHandle::from_guid(w.participant_key);
            let pub_h = crate::instance_handle::InstanceHandle::from_guid(w.key);
            let topic_h = crate::instance_handle::InstanceHandle::from_guid(topic_sample.key);
            if f.is_participant_ignored(part_h) || f.is_publication_ignored(pub_h) {
                continue;
            }
            let _ = sinks.push_publication(&pub_sample);
            if !f.is_topic_ignored(topic_h) {
                let _ = sinks.push_topic(&topic_sample);
            }
        } else {
            let _ = sinks.push_publication(&pub_sample);
            let _ = sinks.push_topic(&topic_sample);
        }
    }
    for r in &events.new_subscriptions {
        let sub_sample = crate::builtin_topics::SubscriptionBuiltinTopicData::from_wire(r);
        let topic_sample = crate::builtin_topics::TopicBuiltinTopicData::from_subscription(r);
        if let Some(f) = &filter {
            let part_h = crate::instance_handle::InstanceHandle::from_guid(r.participant_key);
            let sub_h = crate::instance_handle::InstanceHandle::from_guid(r.key);
            let topic_h = crate::instance_handle::InstanceHandle::from_guid(topic_sample.key);
            if f.is_participant_ignored(part_h) || f.is_subscription_ignored(sub_h) {
                continue;
            }
            let _ = sinks.push_subscription(&sub_sample);
            if !f.is_topic_ignored(topic_h) {
                let _ = sinks.push_topic(&topic_sample);
            }
        } else {
            let _ = sinks.push_subscription(&sub_sample);
            let _ = sinks.push_topic(&topic_sample);
        }
    }
}

/// Binary-property name of the crypto key material in the CryptoToken DataHolder
/// (DDS-Security §9.5.2.1.1, cyclone-verified: `dds.cryp.keymat`).
#[cfg(feature = "security")]
const CRYPTO_TOKEN_PROP: &str = "dds.cryp.keymat";

/// CryptoToken `class_id` (§9.5.2.1: `DDS:Crypto:AES_GCM_GMAC` — underscores,
/// **not** the plugin-class string with hyphens).
#[cfg(feature = "security")]
const CRYPTO_TOKEN_CLASS_ID: &str = "DDS:Crypto:AES_GCM_GMAC";

/// Builds the `PARTICIPANT_CRYPTO_TOKENS` VolatileSecure message with the
/// Kx-encrypted token as a binary property (FU2 S1.4).
#[cfg(feature = "security")]
fn build_crypto_token_message(
    rt: &DcpsRuntime,
    remote_prefix: GuidPrefix,
    kx_token: Vec<u8>,
) -> zerodds_security::generic_message::ParticipantGenericMessage {
    use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
    use zerodds_security::token::DataHolder;
    ParticipantGenericMessage {
        message_identity: MessageIdentity {
            source_guid: Guid::new(rt.guid_prefix, EntityId::PARTICIPANT).to_bytes(),
            sequence_number: 1,
        },
        related_message_identity: MessageIdentity::default(),
        destination_participant_key: Guid::new(remote_prefix, EntityId::PARTICIPANT).to_bytes(),
        destination_endpoint_key: [0; 16],
        source_endpoint_key: [0; 16],
        message_class_id: class_id::PARTICIPANT_CRYPTO_TOKENS.into(),
        message_data: alloc::vec![
            DataHolder::new(CRYPTO_TOKEN_CLASS_ID)
                .with_binary_property(CRYPTO_TOKEN_PROP, kx_token)
        ],
    }
}

/// FU2 S1.4 (send): after handshake completion Kx-encrypt the local data token
/// (`gate.local_token`) and send it as
/// `PARTICIPANT_CRYPTO_TOKENS` over VolatileSecure.
/// Registers the peer's Kx key in the gate beforehand. `None` without a gate
/// or on error (drop instead of leak).
#[cfg(feature = "security")]
fn prepare_crypto_token(
    rt: &DcpsRuntime,
    remote_prefix: GuidPrefix,
    remote_identity: zerodds_security::authentication::IdentityHandle,
    secret: zerodds_security::authentication::SharedSecretHandle,
) -> Option<zerodds_security::generic_message::ParticipantGenericMessage> {
    let gate = rt.config.security.as_ref()?;
    let peer_key = remote_prefix.to_bytes();
    // ALWAYS register the peer's Kx key — even with rtps=NONE: the per-endpoint
    // tokens (discovery_/data_protection) travel Kx-protected over the volatile,
    // protect_volatile_datagram needs this key.
    gate.register_remote_by_guid_from_secret(peer_key, remote_identity, secret)
        .ok()?;
    // BUT: send the ParticipantCryptoToken (= SRTPS keymat) ONLY when
    // rtps_protection != NONE. With rtps=NONE there is no SRTPS; OpenDDS rejects the
    // token (Spdp.cpp:1966 `crypto_handle_==NIL` -> "not configured for RTPS
    // Protection", logs `handle_participant_crypto_tokens failed`) and OpenDDS-self
    // also does NOT exchange it with rtps=NONE. None here = no participant
    // token send; the per-endpoint tokens continue over the separate path.
    if gate.rtps_protection().unwrap_or(ProtectionLevel::None) == ProtectionLevel::None {
        return None;
    }
    // Cross-vendor: the data token travels in PLAINTEXT in the
    // ParticipantGenericMessage — it becomes confidential only through the
    // SEC_PREFIX/BODY/POSTFIX submessage protection of the whole volatile
    // DATA (see protect_volatile_datagram). The `register_*` line above
    // created the peer's Kx key in the gate that this protection uses.
    let token = gate.local_token().ok()?;
    Some(build_crypto_token_message(rt, remote_prefix, token))
}

/// Per-endpoint crypto handle for a local writer/reader (get-or-register).
/// DDS-Security §9.5.3.3: each endpoint has its OWN key material. Registration
/// under the write lock (race-free). `None` without an active gate.
#[cfg(feature = "security")]
fn local_endpoint_crypto_handle(
    rt: &DcpsRuntime,
    eid: EntityId,
    is_writer: bool,
) -> Option<zerodds_security::crypto::CryptoHandle> {
    let gate = rt.config.security.as_ref()?;
    {
        let map = rt.endpoint_crypto.read().ok()?;
        if let Some(h) = map.get(&eid) {
            return Some(*h);
        }
    }
    let mut map = rt.endpoint_crypto.write().ok()?;
    if let Some(h) = map.get(&eid) {
        return Some(*h);
    }
    let h = gate.register_local_endpoint(is_writer).ok()?;
    map.insert(eid, h);
    Some(h)
}

/// Cross-vendor step 6b (send): per-endpoint `datawriter_crypto_tokens` (for
/// every local user writer) + `datareader_crypto_tokens` (for every local
/// user reader) to the peer. cyclone needs these to approve the user-endpoint
/// match and decode ZeroDDS' user DATA. `source_endpoint_key` = the
/// local endpoint GUID; the keymat is the local data key (one key per
/// participant in the bench). Empty list without a gate / without user endpoints.
#[cfg(feature = "security")]
fn prepare_endpoint_crypto_tokens(
    rt: &DcpsRuntime,
    remote_prefix: GuidPrefix,
) -> Vec<zerodds_security::generic_message::ParticipantGenericMessage> {
    use zerodds_security::generic_message::{MessageIdentity, ParticipantGenericMessage, class_id};
    use zerodds_security::token::DataHolder;
    let Some(gate) = rt.config.security.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // cyclone associates a datawriter/datareader token via the pair
    // (source_endpoint, destination_endpoint). Hence per local endpoint ONE
    // token PER matched remote endpoint of **this** peer, with the concrete
    // remote GUID as destination_endpoint_key (dst=0 would make cyclone discard it).
    //
    // §9.5.3.3: the token carries the **per-endpoint** key material of the
    // `source_eid` (not the participant key) — the same key with which
    // ZeroDDS encodes this endpoint's submessages (protect_user_datagram).
    let build = |class: &str,
                 source_eid: EntityId,
                 dst: [u8; 16]|
     -> Option<ParticipantGenericMessage> {
        let is_writer = class == class_id::DATAWRITER_CRYPTO_TOKENS;
        let handle = local_endpoint_crypto_handle(rt, source_eid, is_writer)?;
        let token = gate.create_endpoint_token(handle).ok()?;
        // Dual key (metadata != data, meta-sign-data): cyclone expects
        // num_key_mat=2 — submessage keymat (metadata kind) + payload keymat
        // (data kind) as TWO DataHolders in this order. Single key
        // (all other profiles): only the submessage/endpoint keymat.
        let mut dhs = alloc::vec![
            DataHolder::new(CRYPTO_TOKEN_CLASS_ID).with_binary_property(CRYPTO_TOKEN_PROP, token)
        ];
        if let Some(pay) = gate.endpoint_payload_token(handle) {
            dhs.push(
                DataHolder::new(CRYPTO_TOKEN_CLASS_ID).with_binary_property(CRYPTO_TOKEN_PROP, pay),
            );
        }
        Some(ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: Guid::new(rt.guid_prefix, EntityId::PARTICIPANT).to_bytes(),
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: Guid::new(remote_prefix, EntityId::PARTICIPANT).to_bytes(),
            destination_endpoint_key: dst,
            source_endpoint_key: Guid::new(rt.guid_prefix, source_eid).to_bytes(),
            message_class_id: class.into(),
            message_data: dhs,
        })
    };
    // datawriter tokens: per local writer for every matched remote reader
    // of this peer (dst = reader GUID).
    for (weid, warc) in rt.writer_slots_snapshot() {
        if let Ok(slot) = warc.lock() {
            for proxy in slot.writer.reader_proxies() {
                if proxy.remote_reader_guid.prefix == remote_prefix {
                    out.extend(build(
                        class_id::DATAWRITER_CRYPTO_TOKENS,
                        weid,
                        proxy.remote_reader_guid.to_bytes(),
                    ));
                }
            }
        }
    }
    // datareader tokens: per local reader for every matched remote writer
    // of this peer (dst = writer GUID).
    for (reid, rarc) in rt.reader_slots_snapshot() {
        if let Ok(slot) = rarc.lock() {
            for ws in slot.reader.writer_proxies() {
                if ws.proxy.remote_writer_guid.prefix == remote_prefix {
                    out.extend(build(
                        class_id::DATAREADER_CRYPTO_TOKENS,
                        reid,
                        ws.proxy.remote_writer_guid.to_bytes(),
                    ));
                }
            }
        }
    }
    // Protected discovery (§8.4.2.4): the secure builtin SEDP endpoints
    // (DCPSPublications/SubscriptionsSecure) also need crypto tokens,
    // so the peer associates ZeroDDS' data key with them + decodes the secure-SEDP
    // submessages. cyclone exchanges these builtin-endpoint tokens
    // the same way over the volatile (ff0003c2/c7 + ff0004c2/c7).
    if gate
        .discovery_protection()
        .map(|l| l != ProtectionLevel::None)
        .unwrap_or(false)
    {
        let builtin_pairs = [
            (
                class_id::DATAWRITER_CRYPTO_TOKENS,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            ),
            (
                class_id::DATAREADER_CRYPTO_TOKENS,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
            ),
            (
                class_id::DATAWRITER_CRYPTO_TOKENS,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
            ),
            (
                class_id::DATAREADER_CRYPTO_TOKENS,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
            ),
        ];
        for (class, src_eid, dst_eid) in builtin_pairs {
            out.extend(build(
                class,
                src_eid,
                Guid::new(remote_prefix, dst_eid).to_bytes(),
            ));
        }
    }
    // FastDDS interop: the reliable secure-SPDP builtin (DCPSParticipantsSecure,
    // ff0101c2/c7) needs per-endpoint crypto tokens when FastDDS SEC-encrypts the secure-
    // SPDP DATA under discovery_protection — otherwise the peer cannot
    // decode our secure SPDP -> no secure participant discovery ->
    // no token reciprocation. Gated on enable_secure_spdp.
    if rt.config.enable_secure_spdp {
        let spdp_pairs = [
            (
                class_id::DATAWRITER_CRYPTO_TOKENS,
                EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
                EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            ),
            (
                class_id::DATAREADER_CRYPTO_TOKENS,
                EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
                EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            ),
        ];
        for (class, src_eid, dst_eid) in spdp_pairs {
            out.extend(build(
                class,
                src_eid,
                Guid::new(remote_prefix, dst_eid).to_bytes(),
            ));
        }
    }
    // Liveliness protection (§8.4.2.4): the secure-WLP builtin endpoints
    // (BuiltinParticipantMessageSecure, ff0200c2/c7) also need per-
    // endpoint crypto tokens. cyclone gates the participant security approval
    // (and thus the user-endpoint connection) on it — without these tokens
    // "connect ... waiting for approval by security" stays hung.
    if gate
        .liveliness_protection()
        .map(|l| l != ProtectionLevel::None)
        .unwrap_or(false)
    {
        let wlp_pairs = [
            (
                class_id::DATAWRITER_CRYPTO_TOKENS,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
            ),
            (
                class_id::DATAREADER_CRYPTO_TOKENS,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
                EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
            ),
        ];
        for (class, src_eid, dst_eid) in wlp_pairs {
            out.extend(build(
                class,
                src_eid,
                Guid::new(remote_prefix, dst_eid).to_bytes(),
            ));
        }
    }
    out
}

/// Dedup key of a per-endpoint crypto token: the pair
/// (source_endpoint, destination_endpoint). cyclone associates a
/// datawriter/datareader token via exactly this pair (§9.5.3.3), so it is
/// also the right granularity to remember which tokens have gone out.
#[cfg(feature = "security")]
fn endpoint_token_key(
    m: &zerodds_security::generic_message::ParticipantGenericMessage,
) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[..16].copy_from_slice(&m.source_endpoint_key);
    k[16..].copy_from_slice(&m.destination_endpoint_key);
    k
}

/// Filters out the per-endpoint tokens not yet sent. The previously
/// used **per-peer** once-guard was too coarse: it snapped shut as soon as the
/// participant/secure-SEDP builtin tokens were out — but user endpoints match
/// only later (after the secure SEDP). Their tokens then never went out,
/// and the peer could never decode ZeroDDS' user DATA. Per-token dedup
/// (peer+source+dest) sends each token exactly once — builtins early,
/// user endpoints as soon as they match.
#[cfg(feature = "security")]
fn pending_endpoint_tokens(
    msgs: Vec<zerodds_security::generic_message::ParticipantGenericMessage>,
    already_sent: &alloc::collections::BTreeSet<[u8; 32]>,
) -> Vec<zerodds_security::generic_message::ParticipantGenericMessage> {
    msgs.into_iter()
        .filter(|m| !already_sent.contains(&endpoint_token_key(m)))
        .collect()
}

/// FU2 S1.4 (recv): Kx-decrypt an incoming `PARTICIPANT_CRYPTO_TOKENS` message
/// and install the peer's data key in the gate.
/// Afterwards secured user DATA round-trips with this peer.
#[cfg(feature = "security")]
fn install_crypto_token(
    rt: &DcpsRuntime,
    remote_prefix: GuidPrefix,
    msg: &zerodds_security::generic_message::ParticipantGenericMessage,
) {
    use zerodds_security::generic_message::class_id;
    // Cross-vendor: cyclone sends the data key both as
    // participant_crypto_tokens and per-endpoint as datawriter/
    // datareader_crypto_tokens. We install the keymat from all three
    // under the sender's participant slot (one user endpoint per participant
    // in the bench) — so decode_data_datawriter_from decodes the user DATA.
    if msg.message_class_id != class_id::PARTICIPANT_CRYPTO_TOKENS
        && msg.message_class_id != class_id::DATAWRITER_CRYPTO_TOKENS
        && msg.message_class_id != class_id::DATAREADER_CRYPTO_TOKENS
    {
        return;
    }
    let Some(gate) = rt.config.security.as_ref() else {
        return;
    };
    let peer_key = remote_prefix.to_bytes();
    // `message_data` is a sequence<DataHolder> (DDS-Security §7.4.4.3
    // ParticipantGenericMessage): cyclone packs MULTIPLE CryptoTokens (its own
    // key material per endpoint, different transformation_key_id) into ONE
    // message. Install ALL — taking only `.first()` lost the
    // endpoint keys (key_id 2..N) and the secure SEDP stayed undecodable.
    // Plaintext token (confidentiality was provided by the submessage protection of
    // the transporting volatile DATA, see unprotect_volatile_datagram).
    // DDS-Security §9.5.2 vs §9.5.3: the PARTICIPANT crypto token carries the
    // message-level key (SRTPS, decode_secured_rtps_message -> slots[peer]); the
    // datawriter/datareader tokens carry per-endpoint data keys that belong ONLY in
    // the key_id path (remote_by_key_id, decode_data_by_key_id). Putting both
    // into slots[peer] let the last-installed (datareader) overwrite the
    // participant key -> message-level SRTPS tag mismatch.
    let is_participant = msg.message_class_id == class_id::PARTICIPANT_CRYPTO_TOKENS;
    for dh in &msg.message_data {
        if let Some(token) = dh.binary_property(CRYPTO_TOKEN_PROP) {
            let _ = if is_participant {
                gate.set_remote_data_token_by_guid(&peer_key, token)
            } else {
                gate.install_remote_endpoint_token(token)
            };
        }
    }
}

// RTPS submessage IDs for the VolatileSecure submessage-protection surgery.
#[cfg(feature = "security")]
const SMID_DATA: u8 = 0x15;
#[cfg(feature = "security")]
const SMID_SEC_PREFIX: u8 = 0x31;
#[cfg(feature = "security")]
const SMID_SEC_POSTFIX: u8 = 0x32;
// Further writer submessage IDs (DDSI-RTPS 2.5 §8.3.7). Per DDS-Security
// §8.4.2.4 (is_submessage_protected=TRUE, DataWriter) ALL submessages sent by the
// writer — not only DATA — MUST be protected via encode_datawriter_submessage.
// HEARTBEAT is the critical one: without it the remote
// reader cannot NACK a missing sequence number (= no reliable recovery).
#[cfg(feature = "security")]
const SMID_HEARTBEAT: u8 = 0x07;
#[cfg(feature = "security")]
const SMID_GAP: u8 = 0x08;
#[cfg(feature = "security")]
const SMID_DATA_FRAG: u8 = 0x16;
#[cfg(feature = "security")]
const SMID_HEARTBEAT_FRAG: u8 = 0x13;
// Reader submessages (DDSI-RTPS 2.5 §8.3.7): under `metadata_protection_kind
// != NONE` to be protected via `encode_datareader_submessage` (§8.4.2.4) with the per-endpoint
// reader key — otherwise a spec-conformant remote writer
// (cyclone under discovery=ENCRYPT) discards the clear ACKNACK and never re-sends.
#[cfg(feature = "security")]
const SMID_ACKNACK: u8 = 0x06;
#[cfg(feature = "security")]
const SMID_NACK_FRAG: u8 = 0x12;

/// `true` if the submessage ID is a submessage sent by the DataReader
/// (ACKNACK/NACK_FRAG) — datareader protection path.
#[cfg(feature = "security")]
fn is_protected_reader_submessage(id: u8) -> bool {
    matches!(id, SMID_ACKNACK | SMID_NACK_FRAG)
}

/// Extracts the `reader_id` (sender) from an ACKNACK/NACK_FRAG submessage:
/// offset 4 (after header(4)), directly before the writer_id (offset 8).
#[cfg(feature = "security")]
fn reader_eid_in_submessage(submsg: &[u8], id: u8) -> Option<EntityId> {
    if !is_protected_reader_submessage(id) {
        return None;
    }
    let raw: [u8; 4] = submsg.get(4..8)?.try_into().ok()?;
    Some(EntityId::from_bytes(raw))
}

/// `true` if the submessage ID is a submessage sent by the DataWriter that,
/// under `metadata_protection_kind != NONE`, must be protected via `encode_datawriter_submessage`
/// (DDS-Security §8.4.2.4). ACKNACK/NACK_FRAG are
/// reader submessages (datareader path) and are excluded here.
#[cfg(feature = "security")]
fn is_protected_writer_submessage(id: u8) -> bool {
    matches!(
        id,
        SMID_DATA | SMID_DATA_FRAG | SMID_HEARTBEAT | SMID_HEARTBEAT_FRAG | SMID_GAP
    )
}

/// Walks the submessages of an RTPS datagram from `offset` and returns
/// `(submessage_id, start, total_len)`. `octetsToNextHeader == 0` means
/// "to the end of the datagram" (RTPS §8.3.3.2.3).
#[cfg(feature = "security")]
fn walk_submessages(bytes: &[u8]) -> Vec<(u8, usize, usize)> {
    let mut out = Vec::new();
    let mut o = 20; // RTPS header
    while o + 4 <= bytes.len() {
        let id = bytes[o];
        let le = bytes[o + 1] & 0x01 != 0;
        let raw = if le {
            u16::from_le_bytes([bytes[o + 2], bytes[o + 3]])
        } else {
            u16::from_be_bytes([bytes[o + 2], bytes[o + 3]])
        } as usize;
        let body = if raw == 0 { bytes.len() - (o + 4) } else { raw };
        let total = 4 + body;
        if o + total > bytes.len() {
            break;
        }
        out.push((id, o, total));
        o += total;
    }
    out
}

/// Cross-vendor VolatileSecure (send): replaces every DATA submessage in the
/// datagram with the cyclone-conformant `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX`
/// sequence (encrypted with the peer's Kx key). Other submessages
/// (INFO_DST/INFO_TS/HEARTBEAT) stay unchanged. Returns the datagram
/// unchanged if no DATA submessage is present (e.g. a pure
/// HEARTBEAT tick). `None` only on a crypto error (drop instead of leak).
#[cfg(feature = "security")]
fn protect_volatile_datagram(
    rt: &DcpsRuntime,
    bytes: &[u8],
    peer_key: &[u8; 12],
) -> Option<Vec<u8>> {
    let gate = rt.config.security.as_ref()?;
    if bytes.len() < 20 {
        return Some(bytes.to_vec());
    }
    let subs = walk_submessages(bytes);
    // DDS-Security §8.4.2.4: ParticipantVolatileMessageSecure is submessage-
    // protected — ALL submessages sent by the endpoint MUST be protected with the Kx key,
    // not only DATA. This holds for BOTH directions:
    //  * writer submessages (DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, GAP)
    //  * reader submessages (ACKNACK, NACK_FRAG)
    // cyclone/FastDDS otherwise discard the WHOLE volatile sample with "clear
    // submsg from protected src" → the crypto-token exchange over the volatile
    // stalls. write_with_heartbeat bundles DATA+HEARTBEAT into ONE datagram; if
    // the HEARTBEAT stayed clear, the whole token sample was lost (cross-vendor
    // cyclone→ZeroDDS responder).
    // The reader ACKNACK: OpenDDS' RtpsUdpReceiveStrategy::check_encoded requires
    // protection for the volatile reader (ff0202c4, is_submessage_protected=TRUE) and
    // otherwise drops the clear ACKNACK ("Submessage requires protection") → its
    // volatile WRITER never gets an ACK → considers the token delivery
    // unacknowledged → zerodds NEVER sends the SRTPS-protected secure SEDP → no
    // user-endpoint match. The volatile channel uses ONE shared Kx session key
    // (KDF from the shared secret, §9.5.3.3.4.4), symmetric for both directions
    // → protect the ACKNACK with the same Kx key as the DATA.
    if !subs.iter().any(|(id, _, _)| {
        is_protected_writer_submessage(*id) || is_protected_reader_submessage(*id)
    }) {
        return Some(bytes.to_vec()); // no protection-worthy submessage -> unchanged
    }
    let mut out = Vec::with_capacity(bytes.len() + 64);
    out.extend_from_slice(&bytes[..20]);
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        if is_protected_writer_submessage(id) || is_protected_reader_submessage(id) {
            match gate.encode_kx_datawriter_for(peer_key, submsg) {
                Ok(sec) => out.extend_from_slice(&sec),
                Err(_) => return None, // drop instead of plaintext leak
            }
        } else {
            out.extend_from_slice(submsg);
        }
    }
    Some(out)
}

/// Cross-vendor VolatileSecure (recv): recognizes a `SEC_PREFIX`/`SEC_BODY`/
/// `SEC_POSTFIX` sequence, decodes it with the peer's Kx key to the
/// original DATA submessage and builds a plain RTPS datagram for the
/// `volatile_reader`. `None` if no SEC_* sequence is present (then the normal
/// path) or on a crypto error.
#[cfg(feature = "security")]
fn unprotect_volatile_datagram(
    rt: &DcpsRuntime,
    bytes: &[u8],
    peer_key: &[u8; 12],
) -> Option<Vec<u8>> {
    let gate = rt.config.security.as_ref()?;
    if bytes.len() < 20 {
        return None;
    }
    let subs = walk_submessages(bytes);
    // Cyclone/FastDDS bundle, via xpack, MULTIPLE SEC_*-protected volatile
    // submessages (all with the Kx key) into ONE datagram. So there can be
    // multiple SEC_PREFIX/BODY/POSTFIX triples — transform ALL back
    // (like unprotect_user_datagram). Decoding only the first block (an earlier
    // bug) left every bundled token sample after the first encrypted;
    // the VOLATILE writer does not retransmit them → deterministic
    // token loss (no "flaky" transport, all same-host). `None` if there is no
    // SEC_PREFIX at all (plaintext) or the Kx decode fails (= not a volatile datagram,
    // e.g. secure SEDP with a per-endpoint key).
    if !subs.iter().any(|(id, _, _)| *id == SMID_SEC_PREFIX) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..20]);
    let mut i = 0;
    while i < subs.len() {
        let (id, start, total) = subs[i];
        if id == SMID_SEC_PREFIX {
            let postfix_idx = subs[i..]
                .iter()
                .position(|(sid, _, _)| *sid == SMID_SEC_POSTFIX)
                .map(|off| i + off)?;
            let (_, q_start, q_total) = subs[postfix_idx];
            let sec_wire = &bytes[start..q_start + q_total];
            let submsg = gate.decode_kx_datawriter_from(peer_key, sec_wire).ok()?;
            out.extend_from_slice(&submsg);
            i = postfix_idx + 1;
        } else {
            out.extend_from_slice(&bytes[start..start + total]);
            i += 1;
        }
    }
    Some(out)
}

/// Protects a peer's volatile outbound datagrams (DATA -> SEC_*).
/// HEARTBEAT/ACKNACK datagrams (without DATA) stay unchanged; datagrams
/// with a crypto error are dropped.
#[cfg(feature = "security")]
fn protect_volatile_outbound(
    rt: &DcpsRuntime,
    remote_prefix: GuidPrefix,
    dgs: Vec<zerodds_rtps::message_builder::OutboundDatagram>,
) -> Vec<zerodds_rtps::message_builder::OutboundDatagram> {
    let peer_key = remote_prefix.to_bytes();
    dgs.into_iter()
        .filter_map(|dg| {
            protect_volatile_datagram(rt, &dg.bytes, &peer_key).map(|bytes| {
                zerodds_rtps::message_builder::OutboundDatagram {
                    bytes,
                    targets: dg.targets,
                }
            })
        })
        .collect()
}

/// Cross-vendor (send): replaces EVERY submessage sent by the DataWriter (DATA,
/// DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, GAP) with the cyclone-conformant
/// SEC_PREFIX/BODY/POSTFIX sequence, encrypted with the **local data key**.
/// DDS-Security §8.4.2.4 (`is_submessage_protected=TRUE`, DataWriter): ALL
/// writer submessages MUST be protected via `encode_datawriter_submessage`
/// — in particular the HEARTBEAT, otherwise the remote reader cannot NACK missing
/// sequence numbers (no reliable recovery). Framing submessages
/// (INFO_TS/INFO_DST/...) stay unchanged; `None` on a crypto error.
#[cfg(feature = "security")]
fn protect_user_datagram(rt: &DcpsRuntime, bytes: &[u8]) -> Option<Vec<u8>> {
    let gate = rt.config.security.as_ref()?;
    if bytes.len() < 20 {
        return Some(bytes.to_vec());
    }
    let subs = walk_submessages(bytes);
    if !subs
        .iter()
        .any(|(id, _, _)| is_protected_writer_submessage(*id))
    {
        return Some(bytes.to_vec());
    }
    // §9.5.3.3 per-endpoint key: all writer submessages of a datagram
    // come from the same writer. Take the writer_id from the first protected
    // submessage + look up the per-endpoint handle. No handle
    // (unregistered endpoint) → participant-key fallback.
    let endpoint_handle = subs
        .iter()
        .find(|(id, _, _)| is_protected_writer_submessage(*id))
        .and_then(|&(id, start, total)| writer_eid_in_submessage(&bytes[start..start + total], id))
        .and_then(|weid| local_endpoint_crypto_handle(rt, weid, true));
    let mut out = Vec::with_capacity(bytes.len() + 64);
    out.extend_from_slice(&bytes[..20]);
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        if is_protected_writer_submessage(id) {
            let sec = match endpoint_handle {
                Some(h) => gate.encode_data_datawriter_by_handle(h, submsg),
                None => gate.encode_data_datawriter_local(submsg),
            };
            match sec {
                Ok(s) => out.extend_from_slice(&s),
                Err(_) => return None,
            }
        } else {
            out.extend_from_slice(submsg);
        }
    }
    Some(out)
}

/// Extracts the `writer_id` from an RTPS writer submessage. DATA/DATA_FRAG:
/// offset 12 (header(4)+extraFlags(2)+octetsToInlineQos(2)+readerId(4));
/// HEARTBEAT/GAP/HEARTBEAT_FRAG: offset 8 (header(4)+readerId(4)).
#[cfg(feature = "security")]
fn writer_eid_in_submessage(submsg: &[u8], id: u8) -> Option<EntityId> {
    let off = match id {
        SMID_DATA | SMID_DATA_FRAG => 12,
        SMID_HEARTBEAT | SMID_GAP | SMID_HEARTBEAT_FRAG => 8,
        _ => return None,
    };
    let raw: [u8; 4] = submsg.get(off..off + 4)?.try_into().ok()?;
    Some(EntityId::from_bytes(raw))
}

/// Cross-vendor user DATA (recv): decodes the SEC_* sequence with the sender's
/// data key (`peer_key` = sender GuidPrefix) back to the DATA submessage.
/// `None` if no SEC_* sequence is present (normal path) or on a crypto error.
#[cfg(feature = "security")]
fn unprotect_user_datagram(rt: &DcpsRuntime, bytes: &[u8], peer_key: &[u8; 12]) -> Option<Vec<u8>> {
    let gate = rt.config.security.as_ref()?;
    if bytes.len() < 20 {
        return None;
    }
    let subs = walk_submessages(bytes);
    // §8.4.2.4: the peer SEC_*-wrapped EVERY writer submessage individually
    // (DATA, HEARTBEAT, GAP, ...). So there can be MULTIPLE SEC_PREFIX/BODY/
    // POSTFIX triples in the same datagram — transform them all back. `None`
    // only if there is no SEC_* sequence at all (normal/plaintext path).
    if !subs.iter().any(|(id, _, _)| *id == SMID_SEC_PREFIX) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..20]);
    let mut i = 0;
    while i < subs.len() {
        let (id, start, total) = subs[i];
        if id == SMID_SEC_PREFIX {
            // Find the matching SEC_POSTFIX from i; the block is [prefix..postfix].
            let postfix_idx = subs[i..]
                .iter()
                .position(|(sid, _, _)| *sid == SMID_SEC_POSTFIX)
                .map(|off| i + off)?;
            let (_, q_start, q_total) = subs[postfix_idx];
            let sec_wire = &bytes[start..q_start + q_total];
            // key_id-based decode: the peer has, per endpoint (user +
            // secure-builtin discovery), its own key material; the correct
            // key is found via the transformation_key_id in the CryptoHeader.
            // Fallback for transformation_key_id=0: this is NOT a per-
            // endpoint token key, but the participant-level key derived from the
            // SharedSecret (DDS-Security Tab.73, AES256-GCM, sender_key_id
            // =0) — cyclone protects with it under rtps_protection. That one is decoded by the
            // Kx path (peer-prefix-indexed SharedSecret key).
            let mut submsg = gate
                .decode_data_by_key_id(sec_wire)
                .or_else(|_| gate.decode_data_datawriter_from(peer_key, sec_wire))
                .or_else(|_| gate.decode_kx_datawriter_from(peer_key, sec_wire))
                .ok()?;
            // Correct octetsToNextHeader to the real body length: cyclone
            // wraps every writer submessage INDIVIDUALLY; within its SEC_BODY
            // it is the last one -> octetsToNextHeader=0 ("to the end of the message").
            // When concatenating multiple decoded blocks (e.g. DATA + piggybacked
            // HEARTBEAT), otn=0 would make the strict decode_datagram swallow the following
            // submessage as payload -> the reader would never see the
            // HEARTBEAT and would block as a late joiner on the SN gap.
            if submsg.len() >= 4 {
                let le = submsg[1] & zerodds_rtps::FLAG_E_LITTLE_ENDIAN != 0;
                let otn = u16::try_from(submsg.len() - 4).unwrap_or(0);
                let b = if le {
                    otn.to_le_bytes()
                } else {
                    otn.to_be_bytes()
                };
                submsg[2] = b[0];
                submsg[3] = b[1];
            }
            out.extend_from_slice(&submsg);
            i = postfix_idx + 1;
        } else {
            out.extend_from_slice(&bytes[start..start + total]);
            i += 1;
        }
    }
    Some(out)
}

/// §8.5.1.9.1 / §9.5.3.3.1 data_protection (send): encrypts ONLY the
/// SerializedPayload INSIDE each DATA submessage (payload layer). The
/// submessage header, octetsToInlineQos, InlineQoS and the flags (E/Q/D/K)
/// stay byte-identical; only the N-flag (NonStandardPayload, §8.3.8.2) is
/// set and octetsToNextHeader adjusted to the new payload length. This is
/// the spec-conformant + cyclone-interop form of data_protection (counterpart:
/// metadata_protection = whole submessage SEC_*-wrapped). Applied as the INNER
/// layer BEFORE the submessage/message protection. `None` on a
/// crypto error (drop instead of leak); a datagram without DATA stays unchanged.
#[cfg(feature = "security")]
fn protect_user_payload(rt: &DcpsRuntime, bytes: &[u8]) -> Option<Vec<u8>> {
    use zerodds_rtps::FLAG_E_LITTLE_ENDIAN;
    use zerodds_rtps::submessages::{DATA_FLAG_NON_STANDARD, DataSubmessage};
    let gate = rt.config.security.as_ref()?;
    if bytes.len() < 20 {
        return Some(bytes.to_vec());
    }
    let subs = walk_submessages(bytes);
    if !subs.iter().any(|(id, _, _)| *id == SMID_DATA) {
        return Some(bytes.to_vec());
    }
    let mut out = Vec::with_capacity(bytes.len() + 64);
    out.extend_from_slice(&bytes[..20]);
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        if id != SMID_DATA {
            out.extend_from_slice(submsg);
            continue;
        }
        let flags = submsg[1];
        let le = flags & FLAG_E_LITTLE_ENDIAN != 0;
        // data_protection payload key: the **per-endpoint DataWriter key**
        // (§9.5.3.3.1). cyclone associates the DataWriter strictly with its
        // datawriter_crypto_handle and decodes the SerializedPayload ONLY with
        // this key — the participant key yields "Invalid Crypto
        // Handle" in cyclone. The key is sent to the peer as a datawriter_crypto_token;
        // the reader finds it via the transformation_key_id in the CryptoHeader.
        let handle = writer_eid_in_submessage(submsg, id)
            .and_then(|w| local_endpoint_crypto_handle(rt, w, true))?;
        // Payload boundary: read_body_with_flags returns serialized_payload as
        // an Arc of body[pos..] -> payload = the last plen bytes of the submessage.
        let body = &submsg[4..];
        let ds = DataSubmessage::read_body_with_flags(body, le, flags).ok()?;
        let plen = ds.serialized_payload.len();
        let payload_off = submsg.len() - plen;
        let enc = gate
            .encode_serialized_payload(handle, &ds.serialized_payload)
            .ok()?;
        let new_body_len = (payload_off - 4) + enc.len();
        if new_body_len > u16::MAX as usize {
            return None;
        }
        out.push(submsg[0]);
        out.push(flags | DATA_FLAG_NON_STANDARD);
        let otn = new_body_len as u16;
        if le {
            out.extend_from_slice(&otn.to_le_bytes());
        } else {
            out.extend_from_slice(&otn.to_be_bytes());
        }
        // Body prefix (extraFlags..InlineQoS) verbatim, then encrypted payload.
        out.extend_from_slice(&submsg[4..payload_off]);
        out.extend_from_slice(&enc);
    }
    Some(out)
}

/// Result of the inner payload layer on receipt (§8.5.1.9.4).
#[cfg(feature = "security")]
enum PayloadDecode {
    /// No DATA submessage carries the N-flag — plaintext path, pass the datagram
    /// on unchanged.
    NotEncrypted,
    /// Successfully decrypted — use the plaintext datagram.
    Decoded(Vec<u8>),
    /// N-flag set, but decryption failed. The datagram MUST
    /// be discarded — passing an undecodable encrypted payload as
    /// ciphertext gives the reader garbage (§8.5: reject). The
    /// reliable re-send catches up on the sample once the key is installed
    /// resp. another (e.g. inproc/message-level) copy delivers it.
    Failed,
}

/// `true` if the SerializedPayload begins with a CryptoHeader (§9.5.3.3.1):
/// the first 4 bytes are a CryptoTransformKind != NONE
/// (AES128_GMAC/GCM, AES256_GMAC/GCM = `[0,0,0,1..=4]`). A plaintext CDR
/// encapsulation carries either a different first byte pair (CDR_LE `[0,1]`,
/// XCDR2 `[0,6/7]`, PL_CDR `[0,2/3]`) or — for CDR_BE `[0,0]` — options
/// `[0,0]`, so it does not collide with the transform kinds 1..=4. Serves as
/// detection for vendors (cyclone) that encrypt the data_protection payload
/// without setting the N-flag of the DATA submessage.
#[cfg(feature = "security")]
fn payload_has_crypto_header(payload: &[u8]) -> bool {
    matches!(payload, [0, 0, 0, 1..=4, ..])
}

/// §8.5.1.9.4 / §9.5.3.3.1 data_protection (recv): decrypts the
/// SerializedPayload of each DATA submessage whose payload begins with a CryptoHeader
/// — recognized by the set N-flag (zero↔zero, [`protect_user_payload`])
/// OR by the CryptoTransformKind signature (cyclone does not set the N-flag).
/// The tag verification of the GCM open IS the detection: if the decode fails
/// and the N-flag was not set, the submessage is passed through as plaintext
/// (false positive of the signature heuristic). The key is found via the
/// `transformation_key_id` (key_id), the sender prefix (peer slot) or — for
/// key_id=0 (participant/Kx key, cyclone) — the Kx key material.
/// `NotEncrypted` if no DATA submessage was decrypted; `Failed` only
/// on an N-flag decode error (§8.5: reject undecryptable).
#[cfg(feature = "security")]
fn unprotect_user_payload(rt: &DcpsRuntime, bytes: &[u8]) -> PayloadDecode {
    use zerodds_rtps::FLAG_E_LITTLE_ENDIAN;
    use zerodds_rtps::submessages::{DATA_FLAG_NON_STANDARD, DataSubmessage};
    let Some(gate) = rt.config.security.as_ref() else {
        return PayloadDecode::NotEncrypted;
    };
    if bytes.len() < 20 {
        return PayloadDecode::NotEncrypted;
    }
    // Sender prefix (RTPS header bytes[8..20]) as a fallback key index, if the
    // transformation_key_id in the CryptoHeader is not uniquely in the remote index
    // (zero↔zero indexed via the peer slot, cyclone strictly via key_id).
    let mut peer_key = [0u8; 12];
    peer_key.copy_from_slice(&bytes[8..20]);
    let subs = walk_submessages(bytes);
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..20]);
    let mut did_decode = false;
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        if id != SMID_DATA {
            out.extend_from_slice(submsg);
            continue;
        }
        let flags = submsg[1];
        let le = flags & FLAG_E_LITTLE_ENDIAN != 0;
        let nflag = flags & DATA_FLAG_NON_STANDARD != 0;
        let body = &submsg[4..];
        let Ok(ds) = DataSubmessage::read_body_with_flags(body, le, flags) else {
            // Parse error of a DATA marked as encrypted -> drop;
            // a pure plaintext DATA never made read_body_with_flags fail,
            // so a set N-flag is the only reason here.
            if nflag {
                return PayloadDecode::Failed;
            }
            out.extend_from_slice(submsg);
            continue;
        };
        // Only attempt when the payload is recognizable as encrypted:
        // N-flag (zero↔zero) or CryptoHeader signature (cyclone without an N-flag).
        if !nflag && !payload_has_crypto_header(&ds.serialized_payload) {
            out.extend_from_slice(submsg);
            continue;
        }
        let plen = ds.serialized_payload.len();
        let payload_off = submsg.len() - plen;
        let pdec = gate
            .decode_serialized_payload(&ds.serialized_payload)
            .or_else(|_| gate.decode_serialized_payload_from(&peer_key, &ds.serialized_payload))
            .or_else(|_| gate.decode_serialized_payload_kx(&peer_key, &ds.serialized_payload));
        let Ok(dec) = pdec else {
            // §8.5: if the N-flag was set, the payload is surely encrypted
            // and the reader would get garbage -> drop (reliable re-send catches it
            // up after key install). If only the signature heuristic was the trigger
            // (no N-flag), it is a plaintext CDR_BE payload whose options
            // happen to look like a TransformKind -> pass through unchanged.
            if nflag {
                return PayloadDecode::Failed;
            }
            out.extend_from_slice(submsg);
            continue;
        };
        let new_body_len = (payload_off - 4) + dec.len();
        if new_body_len > u16::MAX as usize {
            return PayloadDecode::Failed;
        }
        out.push(submsg[0]);
        out.push(flags & !DATA_FLAG_NON_STANDARD);
        let otn = new_body_len as u16;
        if le {
            out.extend_from_slice(&otn.to_le_bytes());
        } else {
            out.extend_from_slice(&otn.to_be_bytes());
        }
        out.extend_from_slice(&submsg[4..payload_off]);
        out.extend_from_slice(&dec);
        did_decode = true;
    }
    if did_decode {
        PayloadDecode::Decoded(out)
    } else {
        PayloadDecode::NotEncrypted
    }
}

/// `true` if the EntityId is one of the four secure-SEDP discovery endpoints
/// (DCPSPublicationsSecure/DCPSSubscriptionsSecure, EntityIds ff0003c2/c7 +
/// ff0004c2/c7). Controls whether a SEDP datagram is protected-discovery traffic
/// and must be SEC_*-protected (DDS-Security §8.4.2.4).
#[cfg(feature = "security")]
fn is_secure_sedp_entity(e: EntityId) -> bool {
    e == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
        || e == EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER
        || e == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER
        || e == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER
}

/// `true` if the datagram carries a submessage to/from a secure-SEDP endpoint
/// — then it is protected-discovery traffic.
#[cfg(feature = "security")]
fn is_secure_sedp_datagram(bytes: &[u8]) -> bool {
    let Ok(parsed) = decode_datagram(bytes) else {
        return false;
    };
    parsed.submessages.iter().any(|s| {
        let ids = match s {
            ParsedSubmessage::Data(d) => [Some(d.writer_id), Some(d.reader_id)],
            ParsedSubmessage::DataFrag(d) => [Some(d.writer_id), Some(d.reader_id)],
            ParsedSubmessage::Heartbeat(h) => [Some(h.writer_id), Some(h.reader_id)],
            ParsedSubmessage::Gap(g) => [Some(g.writer_id), Some(g.reader_id)],
            ParsedSubmessage::AckNack(a) => [Some(a.writer_id), Some(a.reader_id)],
            ParsedSubmessage::NackFrag(n) => [Some(n.writer_id), Some(n.reader_id)],
            _ => [None, None],
        };
        ids.into_iter().flatten().any(is_secure_sedp_entity)
    })
}

/// Protected discovery (DDS-Security §8.4.2.4) send: secure-SEDP datagrams
/// (DATA/HEARTBEAT/GAP of the secure writers) are
/// `encode_datawriter_submessage`-protected with the participant data key — the same key the peer installs via
/// `participant_crypto_tokens`. Non-secure SEDP goes through unchanged.
/// `None` ⟹ crypto error on secure SEDP → drop the datagram instead of a
/// plaintext leak.
#[cfg(feature = "security")]
fn protect_sedp_outbound(rt: &DcpsRuntime, bytes: &[u8]) -> Option<Vec<u8>> {
    let Some(gate) = rt.config.security.as_ref() else {
        return Some(bytes.to_vec());
    };
    if !is_secure_sedp_datagram(bytes) || bytes.len() < 20 {
        return Some(bytes.to_vec());
    }
    // Governance §8.4.2.4: discovery_protection_kind=NONE -> NO discovery
    // protection. Secure-SEDP entities (ff0003c7/ff0004c7) must then NOT
    // be per-endpoint-protected; otherwise their ACKNACKs leak as message-
    // level SEC_PREFIX with a never-exchanged per-endpoint key that a
    // peer (cyclone uses plain SEDP under discovery=NONE) discards as "Invalid Crypto
    // Handle". Pass through plain -> the outer rtps_protection
    // layer (SRTPS via secure_outbound_bytes) wraps the whole message correctly.
    if gate.discovery_protection().unwrap_or(ProtectionLevel::None) == ProtectionLevel::None {
        return Some(bytes.to_vec());
    }
    // §8.4.2.4: protect BOTH directions — writer submessages (DATA/HEARTBEAT/
    // GAP) with the per-endpoint writer key (encode_datawriter_submessage), reader
    // submessages (ACKNACK/NACK_FRAG) with the per-endpoint reader key
    // (encode_datareader_submessage). A spec-conformant cyclone under
    // discovery=ENCRYPT discards a CLEAR ACKNACK of the secure-SEDP reader →
    // never re-sends the SubscriptionData → ZeroDDS never discovers the reader. The
    // per-endpoint key (same as in the sent datareader_crypto_token)
    // makes the ACKNACK decodable for cyclone.
    let subs = walk_submessages(bytes);
    let mut out = Vec::with_capacity(bytes.len() + 64);
    out.extend_from_slice(&bytes[..20]);
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        let handle = if is_protected_writer_submessage(id) {
            writer_eid_in_submessage(submsg, id)
                .and_then(|w| local_endpoint_crypto_handle(rt, w, true))
        } else if is_protected_reader_submessage(id) {
            reader_eid_in_submessage(submsg, id)
                .and_then(|r| local_endpoint_crypto_handle(rt, r, false))
        } else {
            // Framing submessage (INFO_TS/INFO_DST/...) — unchanged.
            out.extend_from_slice(submsg);
            continue;
        };
        let sec = match handle {
            Some(h) => gate.encode_data_datawriter_by_handle(h, submsg),
            // No per-endpoint handle (should not occur for secure SEDP)
            // → participant-key fallback, so no plaintext leak arises.
            None => gate.encode_data_datawriter_local(submsg),
        };
        match sec {
            Ok(s) => out.extend_from_slice(&s),
            Err(_) => return None,
        }
    }
    Some(out)
}

/// Protects a user-reader outbound datagram (ACKNACK/NACK_FRAG) on the
/// send direction (DDS-Security §8.4.2.4). Counterpart to the writer-DATA layer:
/// under `metadata_protection != NONE` the reader submessage too MUST be protected with the
/// per-endpoint reader key, otherwise a spec-strict
/// peer writer (cyclone/FastDDS) discards the CLEAR ACKNACK → the SN gap is never
/// re-sent → permanent reliable stall. Only needed when
/// **rtps_protection** does NOT already wrap the message as an SRTPS whole; otherwise
/// (and with metadata=NONE) the function delegates to `secure_outbound_bytes`.
#[cfg(feature = "security")]
fn protect_user_reader_datagram<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    let Some(gate) = rt.config.security.as_ref() else {
        return Some(alloc::borrow::Cow::Borrowed(bytes));
    };
    let metadata = gate.metadata_protection().unwrap_or(ProtectionLevel::None);
    let rtps = gate.rtps_protection().unwrap_or(ProtectionLevel::None);
    // rtps != None → SRTPS wraps the whole message incl. ACKNACK; metadata ==
    // None → no submessage protection configured. secure_outbound_bytes
    // (transform_outbound) covers both cases correctly.
    if metadata == ProtectionLevel::None || rtps != ProtectionLevel::None || bytes.len() < 20 {
        return secure_outbound_bytes(rt, bytes);
    }
    let subs = walk_submessages(bytes);
    let mut out = Vec::with_capacity(bytes.len() + 64);
    out.extend_from_slice(&bytes[..20]);
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        if is_protected_reader_submessage(id) {
            let handle = reader_eid_in_submessage(submsg, id)
                .and_then(|r| local_endpoint_crypto_handle(rt, r, false));
            match handle {
                Some(h) => match gate.encode_data_datawriter_by_handle(h, submsg) {
                    Ok(s) => out.extend_from_slice(&s),
                    Err(_) => return None,
                },
                // No per-endpoint reader key yet (the endpoint matches only after
                // secure SEDP) → pass through plaintext; the reader tick re-sends
                // the ACKNACK once the key is installed.
                None => out.extend_from_slice(submsg),
            }
        } else {
            // Framing submessage (INFO_DST/INFO_TS/...) — unchanged.
            out.extend_from_slice(submsg);
        }
    }
    Some(alloc::borrow::Cow::Owned(out))
}

#[cfg(not(feature = "security"))]
fn protect_user_reader_datagram<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    secure_outbound_bytes(rt, bytes)
}

/// `true` if `liveliness_protection != NONE` is configured — then WLP runs
/// over the secure entity + participant-key protection (§8.4.2.4).
#[cfg(feature = "security")]
fn wlp_liveliness_protected(rt: &DcpsRuntime) -> bool {
    rt.config.security.as_ref().is_some_and(|gate| {
        gate.liveliness_protection()
            .unwrap_or(ProtectionLevel::None)
            != ProtectionLevel::None
    })
}

#[cfg(not(feature = "security"))]
fn wlp_liveliness_protected(_rt: &DcpsRuntime) -> bool {
    false
}

/// Protects a WLP outbound datagram (BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
/// DATA) under `liveliness_protection != NONE` with the **participant data key**
/// (§8.4.2.4 / §7.4.7.1 Tab.7). WLP is participant-level (no per-endpoint key)
/// — analogous to the participant-key fallback in `protect_sedp_outbound`. If
/// `rtps_protection` already covers the message as SRTPS (or liveliness=NONE),
/// the function delegates to `secure_outbound_bytes`.
#[cfg(feature = "security")]
fn protect_wlp_outbound<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    let Some(gate) = rt.config.security.as_ref() else {
        return Some(alloc::borrow::Cow::Borrowed(bytes));
    };
    let live = gate
        .liveliness_protection()
        .unwrap_or(ProtectionLevel::None);
    let rtps = gate.rtps_protection().unwrap_or(ProtectionLevel::None);
    // liveliness=NONE: no inner SEC layer -> secure_outbound_bytes covers
    // rtps_protection (SRTPS) resp. passthrough. PREVIOUSLY this branch
    // also delegated with rtps!=None and thus left out the liveliness SEC -> cyclone
    // saw the WLP DATA "clear submsg from protected src" -> no liveliness.
    if live == ProtectionLevel::None || bytes.len() < 20 {
        return secure_outbound_bytes(rt, bytes);
    }
    let subs = walk_submessages(bytes);
    let mut out = Vec::with_capacity(bytes.len() + 64);
    out.extend_from_slice(&bytes[..20]);
    for (id, start, total) in subs {
        let submsg = &bytes[start..start + total];
        if id == SMID_DATA {
            // Protect the secure-WLP DATA with the per-endpoint key of the secure-WLP writer
            // (ff0200c2) — the same key ZeroDDS sends the peer via the
            // datawriter_crypto_token (prepare_endpoint_crypto_tokens
            // liveliness block). encode_data_datawriter_local took the participant
            // key, which cyclone does NOT associate with ff0200c2 -> undecodable ->
            // no liveliness -> peer approval of the user endpoints hangs.
            let sec = writer_eid_in_submessage(submsg, id)
                .and_then(|w| local_endpoint_crypto_handle(rt, w, true))
                .and_then(|h| gate.encode_data_datawriter_by_handle(h, submsg).ok());
            match sec {
                Some(s) => out.extend_from_slice(&s),
                None => return None,
            }
        } else {
            out.extend_from_slice(submsg);
        }
    }
    // Under additional rtps_protection, message-level SRTPS MUST go around the
    // liveliness-SEC-wrapped WLP (both layers, like cyclone<->cyclone) —
    // otherwise cyclone would see only the SRTPS shell OR (with the old logic) the
    // clear DATA. First inner SEC (above), then SRTPS (here).
    if rtps != ProtectionLevel::None {
        return gate
            .transform_outbound(&out)
            .ok()
            .map(alloc::borrow::Cow::Owned);
    }
    Some(alloc::borrow::Cow::Owned(out))
}

#[cfg(not(feature = "security"))]
fn protect_wlp_outbound<'a>(
    rt: &DcpsRuntime,
    bytes: &'a [u8],
) -> Option<alloc::borrow::Cow<'a, [u8]>> {
    secure_outbound_bytes(rt, bytes)
}

/// Wire demux for the security builtin topics. Routes an
/// incoming RTPS submessage sequence to the `SecurityBuiltinStack`,
/// if the stack is active. No-op if the datagram does not address a security
/// builtin reader or the plugin is not enabled.
///
/// Called by the metatraffic receive path — stateless +
/// VolatileSecure run over the SPDP unicast locators (PID 0x0032),
/// not over `user_unicast`.
fn dispatch_security_builtin_datagram(
    rt: &Arc<DcpsRuntime>,
    bytes: &[u8],
    now: Duration,
) -> Vec<zerodds_rtps::message_builder::OutboundDatagram> {
    // `mut` only needed on the security path (the handshake reply is appended
    // there); without the feature the list stays empty.
    #[cfg(feature = "security")]
    let mut outbound = Vec::new();
    #[cfg(not(feature = "security"))]
    let outbound = Vec::new();
    let Some(stack) = rt.security_builtin_snapshot() else {
        return outbound;
    };
    // Cross-vendor VolatileSecure: cyclone protects the volatile DATA as a
    // SEC_PREFIX/SEC_BODY/SEC_POSTFIX sequence. Before the submessage parse,
    // transform the sequence with the sender's Kx key (GuidPrefix = RTPS header bytes[8..20])
    // back to the original DATA submessage. `None` = no SEC_*
    // sequence (normal path) resp. crypto error.
    #[cfg(feature = "security")]
    let unprotected: Option<Vec<u8>> = if bytes.len() >= 20 {
        let mut pk = [0u8; 12];
        pk.copy_from_slice(&bytes[8..20]);
        unprotect_volatile_datagram(rt, bytes, &pk)
    } else {
        None
    };
    #[cfg(feature = "security")]
    let bytes: &[u8] = unprotected.as_deref().unwrap_or(bytes);
    let Ok(parsed) = decode_datagram(bytes) else {
        return outbound;
    };
    // sourceGuidPrefix of the datagram (DDSI-RTPS §8.3.4) — reader demux key for
    // the volatile builtin readers. Used in both feature configs.
    let remote_prefix = parsed.header.guid_prefix;
    let Ok(mut s) = stack.lock() else {
        return outbound;
    };
    for sub in parsed.submessages {
        match sub {
            ParsedSubmessage::Data(d) => {
                if d.reader_id == EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER
                    || d.writer_id == EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                {
                    // FU2 Gap 5: decode the stateless auth and — with
                    // an active auth plugin — drive the handshake.
                    // `on_stateless_message` returns the next token
                    // message (reply/final), which we send back to the peer.
                    // Decode errors are swallowed (stateless
                    // has no resend path, Spec §10.3.4.1). The
                    // completion `(remote_identity, secret)` is stored in the stack
                    // (peer_secret) — the gate registration +
                    // crypto-token exchange follows in Gap 6.
                    if let Ok(msg) = s.stateless_reader.handle_data(&d) {
                        #[cfg(feature = "security")]
                        s.note_remote_vendor(remote_prefix, parsed.header.vendor_id);
                        #[cfg(feature = "security")]
                        if let Ok((out, completed)) = s.on_stateless_message(remote_prefix, &msg) {
                            outbound.extend(out);
                            // FU2 S1.4: handshake done → register Kx +
                            // send the Kx-encrypted data token to the peer over Volatile-
                            // Secure. (the pki lock is free here:
                            // on_stateless_message released it.)
                            if let Some((remote_identity, secret)) = completed {
                                if let Some(token_msg) =
                                    prepare_crypto_token(rt, remote_prefix, remote_identity, secret)
                                {
                                    outbound.extend(protect_volatile_outbound(
                                        rt,
                                        remote_prefix,
                                        s.volatile_writer
                                            .write_with_heartbeat(&token_msg, now)
                                            .unwrap_or_default(),
                                    ));
                                }
                                // Step 6b: per-endpoint datawriter/datareader
                                // tokens (per-token dedup #29: the builtins go out
                                // here exactly once + are marked).
                                let already = rt
                                    .endpoint_tokens_sent
                                    .read()
                                    .map(|set| set.clone())
                                    .unwrap_or_default();
                                let pending = pending_endpoint_tokens(
                                    prepare_endpoint_crypto_tokens(rt, remote_prefix),
                                    &already,
                                );
                                for ep_msg in pending {
                                    let key = endpoint_token_key(&ep_msg);
                                    outbound.extend(protect_volatile_outbound(
                                        rt,
                                        remote_prefix,
                                        s.volatile_writer
                                            .write_with_heartbeat(&ep_msg, now)
                                            .unwrap_or_default(),
                                    ));
                                    if let Ok(mut set) = rt.endpoint_tokens_sent.write() {
                                        set.insert(key);
                                    }
                                }
                            }
                        }
                        #[cfg(not(feature = "security"))]
                        let _ = msg;
                    }
                } else if d.reader_id
                    == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
                {
                    // FU2 S1.4: VolatileSecure carries the crypto-token
                    // exchange. Kx-decrypt the received PARTICIPANT_CRYPTO_TOKENS
                    // message + install the data key in the gate.
                    if let Ok(_msgs) = s.volatile_reader.handle_data(remote_prefix, &d) {
                        #[cfg(feature = "security")]
                        for m in &_msgs {
                            install_crypto_token(rt, remote_prefix, m);
                        }
                        // Step 6b: now (peer ready) send our per-endpoint
                        // tokens back. Per-token dedup (#29): builtins
                        // go out early here, the later-matching user-
                        // endpoint tokens are caught up by the tick path (no per-peer
                        // guard that blocks them forever).
                        #[cfg(feature = "security")]
                        {
                            let already = rt
                                .endpoint_tokens_sent
                                .read()
                                .map(|set| set.clone())
                                .unwrap_or_default();
                            let pending = pending_endpoint_tokens(
                                prepare_endpoint_crypto_tokens(rt, remote_prefix),
                                &already,
                            );
                            for ep_msg in pending {
                                let key = endpoint_token_key(&ep_msg);
                                outbound.extend(protect_volatile_outbound(
                                    rt,
                                    remote_prefix,
                                    s.volatile_writer
                                        .write_with_heartbeat(&ep_msg, now)
                                        .unwrap_or_default(),
                                ));
                                if let Ok(mut set) = rt.endpoint_tokens_sent.write() {
                                    set.insert(key);
                                }
                            }
                        }
                        // The peer now has our participant crypto token (can
                        // decode our SRTPS/SEC SEDP): catch up the initially dropped
                        // SEDP burst once (OpenDDS convergence).
                        #[cfg(feature = "security")]
                        rt.re_announce_sedp_to_peer(remote_prefix);
                    }
                }
            }
            ParsedSubmessage::DataFrag(df) => {
                if df.reader_id == EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER
                    || df.writer_id == EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                {
                    // FU2 cross-vendor: cyclone/FastDDS RTPS-fragment the
                    // large HandshakeReply/Final (cert + permissions over
                    // MTU). Reassemble the fragments + drive them through the
                    // handshake driver like a stateless DATA.
                    if let Ok(msgs) = s.stateless_reader.handle_data_frag(&df) {
                        #[cfg(feature = "security")]
                        s.note_remote_vendor(remote_prefix, parsed.header.vendor_id);
                        #[cfg(feature = "security")]
                        for msg in &msgs {
                            if let Ok((out, completed)) = s.on_stateless_message(remote_prefix, msg)
                            {
                                outbound.extend(out);
                                if let Some((remote_identity, secret)) = completed {
                                    if let Some(token_msg) = prepare_crypto_token(
                                        rt,
                                        remote_prefix,
                                        remote_identity,
                                        secret,
                                    ) {
                                        outbound.extend(protect_volatile_outbound(
                                            rt,
                                            remote_prefix,
                                            s.volatile_writer
                                                .write_with_heartbeat(&token_msg, now)
                                                .unwrap_or_default(),
                                        ));
                                    }
                                    let already = rt
                                        .endpoint_tokens_sent
                                        .read()
                                        .map(|set| set.clone())
                                        .unwrap_or_default();
                                    let pending = pending_endpoint_tokens(
                                        prepare_endpoint_crypto_tokens(rt, remote_prefix),
                                        &already,
                                    );
                                    for ep_msg in pending {
                                        let key = endpoint_token_key(&ep_msg);
                                        outbound.extend(protect_volatile_outbound(
                                            rt,
                                            remote_prefix,
                                            s.volatile_writer
                                                .write_with_heartbeat(&ep_msg, now)
                                                .unwrap_or_default(),
                                        ));
                                        if let Ok(mut set) = rt.endpoint_tokens_sent.write() {
                                            set.insert(key);
                                        }
                                    }
                                }
                            }
                        }
                        #[cfg(not(feature = "security"))]
                        let _ = msgs;
                    }
                } else if df.reader_id
                    == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
                {
                    let _ = s.volatile_reader.handle_data_frag(remote_prefix, &df, now);
                }
            }
            ParsedSubmessage::Heartbeat(h) => {
                let to_volatile_reader = h.reader_id
                    == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
                    || (h.reader_id == EntityId::UNKNOWN
                        && h.writer_id
                            == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER);
                if to_volatile_reader {
                    s.volatile_reader.handle_heartbeat(remote_prefix, &h, now);
                }
            }
            ParsedSubmessage::Gap(g) => {
                if g.reader_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER {
                    let _ = s.volatile_reader.handle_gap(remote_prefix, &g);
                }
            }
            ParsedSubmessage::AckNack(ack) => {
                if ack.writer_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER {
                    let base = ack.reader_sn_state.bitmap_base;
                    let requested: Vec<_> = ack.reader_sn_state.iter_set().collect();
                    let src = Guid::new(parsed.header.guid_prefix, ack.reader_id);
                    s.volatile_writer.handle_acknack(src, base, requested);
                }
            }
            ParsedSubmessage::NackFrag(nf) => {
                if nf.writer_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER {
                    let src = Guid::new(parsed.header.guid_prefix, nf.reader_id);
                    s.volatile_writer.handle_nackfrag(src, &nf);
                }
            }
            _ => {}
        }
    }
    outbound
}

/// Dispatches a datagram addressed to the TypeLookup service endpoints
/// (XTypes 1.3 §7.6.3.3.4). Handles incoming
/// requests (to `TL_SVC_REQ_READER`), generates replies and sends
/// them back to the source locator; handles incoming replies
/// (to `TL_SVC_REPLY_READER`), correlates with the client.
///
/// Returns `true` if the datagram was accepted by the TypeLookup path
/// — the caller can then skip the user-reader path.
fn dispatch_type_lookup_datagram(rt: &Arc<DcpsRuntime>, bytes: &[u8], source: &Locator) -> bool {
    use zerodds_cdr::{BufferReader, Endianness};
    use zerodds_rtps::inline_qos::{SampleIdentityBytes, find_related_sample_identity};
    use zerodds_types::type_lookup::{
        GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply, GetTypesRequest,
    };

    let Ok(parsed) = decode_datagram(bytes) else {
        return false;
    };
    // DDS-RPC §7.8.2: the request sample identity = (request writer GUID,
    // request SN). The server carries it as PID_RELATED_SAMPLE_IDENTITY in the
    // reply inline QoS, so a client (also cross-vendor) can correlate
    // without relying on the echoed writer_sn.
    let src_prefix = parsed.header.guid_prefix;

    let mut accepted = false;

    for sub in &parsed.submessages {
        let ParsedSubmessage::Data(d) = sub else {
            continue;
        };
        let payload: &[u8] = &d.serialized_payload;
        if payload.is_empty() {
            continue;
        }
        // Skip CDR-Encapsulation header (4 bytes) if present.
        let body: &[u8] = if payload.len() >= 4 && (payload[0] == 0x00 && payload[1] == 0x01) {
            &payload[4..]
        } else {
            payload
        };

        // Inbound Request → Server.
        if d.reader_id == EntityId::TL_SVC_REQ_READER {
            accepted = true;
            // Request sample identity = (request writer GUID, request SN) — mirrored
            // as related_sample_identity into the reply inline QoS.
            let (sn_hi, sn_lo) = d.writer_sn.split();
            let req_sn = ((u64::from(sn_hi as u32)) << 32) | u64::from(sn_lo);
            let related =
                SampleIdentityBytes::new(Guid::new(src_prefix, d.writer_id).to_bytes(), req_sn);
            // Try GetTypes-Request first; fall back to
            // GetTypeDependenciesRequest if that fails.
            let mut r = BufferReader::new(body, Endianness::Little);
            if let Ok(req) = GetTypesRequest::decode_from(&mut r) {
                let reply = match rt.type_lookup_server.lock() {
                    Ok(g) => g.handle_get_types(&req),
                    Err(_) => continue,
                };
                let _ = send_type_lookup_reply(
                    rt,
                    source,
                    TypeLookupReplyPayload::Types(reply),
                    related,
                );
                continue;
            }
            let mut r = BufferReader::new(body, Endianness::Little);
            if let Ok(req) = GetTypeDependenciesRequest::decode_from(&mut r) {
                let reply = match rt.type_lookup_server.lock() {
                    Ok(g) => g.handle_get_type_dependencies(&req),
                    Err(_) => continue,
                };
                let _ = send_type_lookup_reply(
                    rt,
                    source,
                    TypeLookupReplyPayload::Dependencies(reply),
                    related,
                );
                continue;
            }
        }

        // Inbound Reply → Client.
        if d.reader_id == EntityId::TL_SVC_REPLY_READER {
            accepted = true;
            // Correlation prefers PID_RELATED_SAMPLE_IDENTITY (DDS-RPC §7.8.2,
            // cross-vendor compatible); fallback to the echoed writer_sn for
            // peers/legacy replies without inline QoS.
            let request_id = d
                .inline_qos
                .as_ref()
                .and_then(|pl| find_related_sample_identity(pl, true).ok().flatten())
                .map(|sid| zerodds_discovery::type_lookup::RequestId::from_u64(sid.sequence_number))
                .unwrap_or_else(|| {
                    let (sn_high, sn_low) = d.writer_sn.split();
                    let sn_u64 = ((u64::from(sn_high as u32)) << 32) | u64::from(sn_low);
                    zerodds_discovery::type_lookup::RequestId::from_u64(sn_u64)
                });
            let mut r = BufferReader::new(body, Endianness::Little);
            if let Ok(reply) = GetTypesReply::decode_from(&mut r) {
                if let Ok(mut client) = rt.type_lookup_client.lock() {
                    client.handle_reply(request_id, TypeLookupReply::Types(reply));
                }
                continue;
            }
            // M-5: the getTypeDependencies reply carries a different element type
            // (TypeIdentifierWithSize list) — its own decode branch, otherwise the
            // dependencies callback never fires.
            let mut r = BufferReader::new(body, Endianness::Little);
            if let Ok(reply) = GetTypeDependenciesReply::decode_from(&mut r) {
                if let Ok(mut client) = rt.type_lookup_client.lock() {
                    client.handle_reply(request_id, TypeLookupReply::Dependencies(reply));
                }
                continue;
            }
        }
    }

    accepted
}

/// Reply payload variants that the TypeLookup server can emit.
enum TypeLookupReplyPayload {
    Types(zerodds_types::type_lookup::GetTypesReply),
    Dependencies(zerodds_types::type_lookup::GetTypeDependenciesReply),
}

/// Sends a TypeLookup reply to a peer locator as a
/// DATA datagram on the TL_SVC_REPLY_WRITER → peer's
/// TL_SVC_REPLY_READER. The sequence number echoes the request sequence
/// for correlation purposes (see XTypes §7.6.3.3.3 sample identity).
fn send_type_lookup_reply(
    rt: &Arc<DcpsRuntime>,
    target: &Locator,
    reply: TypeLookupReplyPayload,
    related: zerodds_rtps::inline_qos::SampleIdentityBytes,
) -> Result<()> {
    use alloc::sync::Arc as AllocArc;
    use core::sync::atomic::Ordering;
    use zerodds_cdr::{BufferWriter, Endianness};
    use zerodds_rtps::datagram::encode_data_datagram;
    use zerodds_rtps::header::RtpsHeader;
    use zerodds_rtps::submessages::DataSubmessage;
    use zerodds_rtps::wire_types::{ProtocolVersion, SequenceNumber, VendorId};

    // CDR-encode reply (PL_CDR_LE-Encapsulation).
    let mut w = BufferWriter::new(Endianness::Little);
    match reply {
        TypeLookupReplyPayload::Types(r) => {
            r.encode_into(&mut w)
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "type_lookup reply encode failed",
                })?;
        }
        TypeLookupReplyPayload::Dependencies(r) => {
            r.encode_into(&mut w)
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "type_lookup deps reply encode failed",
                })?;
        }
    }
    let body = w.into_bytes();
    let mut payload: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(4 + body.len());
    payload.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    payload.extend_from_slice(&body);

    let header = RtpsHeader {
        protocol_version: ProtocolVersion::CURRENT,
        vendor_id: VendorId::ZERODDS,
        guid_prefix: rt.guid_prefix,
    };
    // Own monotonically increasing reply-writer SN (starting at 1) instead of a
    // request-SN echo — a reliable cross-vendor reply reader would otherwise see SN jumps.
    let reply_sn = rt
        .tl_reply_sn
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    let writer_sn =
        SequenceNumber::from_high_low((reply_sn >> 32) as i32, (reply_sn & 0xFFFF_FFFF) as u32);
    let data = DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::TL_SVC_REPLY_READER,
        writer_id: EntityId::TL_SVC_REPLY_WRITER,
        writer_sn,
        // DDS-RPC §7.8.2: related_sample_identity couples the reply to the
        // request (cross-vendor correlation without a writer_sn echo).
        inline_qos: Some(zerodds_rtps::inline_qos::reply_inline_qos(related, true)),
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: AllocArc::from(payload.into_boxed_slice()),
    };
    let datagram =
        encode_data_datagram(header, &[data]).map_err(|_| DdsError::PreconditionNotMet {
            reason: "type_lookup reply datagram encode failed",
        })?;

    if is_routable_user_locator(target) {
        let _ = rt.user_unicast.send(target, &datagram);
    }
    Ok(())
}

/// Sends a discovery datagram to all target locators. UDP-only
/// (TCPv4/SHM/UDS are not carried in discovery); non-UDP
/// locators are silently ignored.
fn send_discovery_datagram(rt: &Arc<DcpsRuntime>, targets: &[Locator], bytes: &[u8]) {
    let Some(secured) = secure_outbound_bytes(rt, bytes) else {
        return;
    };
    for t in targets {
        if !is_routable_user_locator(t) {
            continue;
        }
        // Send unicast metatraffic (SEDP responses, VolatileSecure, stateless auth)
        // from the **metatraffic recv socket** (`spdp_unicast`, = announced
        // metatraffic_unicast_locator), NOT from the ephemeral `spdp_mc_tx`.
        // Otherwise the peer sees a foreign source port and sends its
        // responses (e.g. cyclone's VolatileSecure ACKNACK to the source locator)
        // to a port ZeroDDS does not listen on → reliable resends stay
        // out (cross-vendor). `spdp_mc_tx` stays only for SPDP multicast.
        let _ = rt.spdp_unicast.send(t, &secured);
    }
}

/// Default user-multicast locator for a DomainParticipant.
/// Not used in live mode 1 yet; SPDP-announced in B2.
#[must_use]
pub fn user_multicast_endpoint(domain_id: i32) -> SocketAddr {
    // Spec §9.6.1.4.1: user-multicast-port = PB + DG * d + d2
    //   = 7400 + 250 * d + 1
    let port = 7400u16.saturating_add(250u16.saturating_mul(domain_id as u16).saturating_add(1));
    SocketAddr::from((Ipv4Addr::from([239, 255, 0, 1]), port))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// A's the last mile of the big-endian decode path: a big-endian
    /// encapsulation header on a received DATA sample must route the typed
    /// decode to `DdsType::decode_be`, not `decode`. Builds a CDR2_BE wire
    /// sample, runs it through `delivered_to_user_sample` (the real recv→sample
    /// conversion), and asserts the resulting `big_endian` flag drives a correct
    /// decode — while the little-endian `decode` on the same BE body is wrong.
    #[test]
    fn big_endian_encap_routes_to_decode_be() {
        use crate::dds_type::{DdsType, DecodeError, EncodeError};
        #[derive(Debug, PartialEq, Clone)]
        struct BeProbe {
            v: i32,
        }
        impl DdsType for BeProbe {
            const TYPE_NAME: &'static str = "BeProbe";
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little).xcdr2();
                <i32 as zerodds_cdr::CdrEncode>::encode(&self.v, &mut w)?;
                out.extend_from_slice(&w.into_bytes());
                Ok(())
            }
            fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Big).xcdr2();
                <i32 as zerodds_cdr::CdrEncode>::encode(&self.v, &mut w)?;
                out.extend_from_slice(&w.into_bytes());
                Ok(())
            }
            fn decode(b: &[u8]) -> core::result::Result<Self, DecodeError> {
                let mut r =
                    zerodds_cdr::BufferReader::new(b, zerodds_cdr::Endianness::Little).xcdr2();
                Ok(BeProbe {
                    v: <i32 as zerodds_cdr::CdrDecode>::decode(&mut r)?,
                })
            }
            fn decode_be(b: &[u8]) -> core::result::Result<Self, DecodeError> {
                let mut r = zerodds_cdr::BufferReader::new(b, zerodds_cdr::Endianness::Big).xcdr2();
                Ok(BeProbe {
                    v: <i32 as zerodds_cdr::CdrDecode>::decode(&mut r)?,
                })
            }
        }

        // A value whose 4 LE bytes differ from its 4 BE bytes.
        let orig = BeProbe { v: 0x0102_0304 };
        let strengths = alloc::collections::BTreeMap::new();

        let mk = |repr_lo: u8, body: Vec<u8>| {
            let mut wire = alloc::vec![0x00u8, repr_lo, 0x00, 0x00];
            wire.extend_from_slice(&body);
            zerodds_rtps::reliable_reader::DeliveredSample {
                writer_guid: Guid::new(GuidPrefix::from_bytes([0x11; 12]), EntityId::PARTICIPANT),
                sequence_number: zerodds_rtps::wire_types::SequenceNumber(1),
                payload: alloc::sync::Arc::from(wire.into_boxed_slice()),
                kind: zerodds_rtps::history_cache::ChangeKind::Alive,
                key_hash: None,
                source_timestamp: None,
            }
        };

        // --- big-endian wire: CDR2_BE (repr low byte 0x06) ---
        let mut be_body = Vec::new();
        orig.encode_be(&mut be_body).unwrap();
        let us = delivered_to_user_sample(&mk(0x06, be_body), &strengths).expect("alive");
        let UserSample::Alive {
            payload,
            big_endian,
            representation,
            ..
        } = us
        else {
            panic!("expected Alive");
        };
        assert!(big_endian, "CDR2_BE encap must set big_endian");
        assert_eq!(representation, 1, "0x06 = XCDR2");
        // The subscriber dispatch: decode_be for a big-endian sample.
        let decoded = if big_endian {
            BeProbe::decode_be(&payload)
        } else {
            BeProbe::decode(&payload)
        }
        .unwrap();
        assert_eq!(decoded, orig, "BE wire decodes correctly via decode_be");
        // The dispatch matters: little-endian decode on the BE body is wrong.
        assert_ne!(BeProbe::decode(&payload).unwrap(), orig);

        // --- little-endian control: CDR2_LE (repr low byte 0x07) ---
        let mut le_body = Vec::new();
        orig.encode(&mut le_body).unwrap();
        let us_le = delivered_to_user_sample(&mk(0x07, le_body), &strengths).expect("alive");
        let UserSample::Alive {
            payload: le_payload,
            big_endian: be_le,
            ..
        } = us_le
        else {
            panic!("expected Alive");
        };
        assert!(!be_le, "CDR2_LE encap must NOT set big_endian");
        assert_eq!(BeProbe::decode(&le_payload).unwrap(), orig);
    }

    /// FU1 diagnosis: inject a REAL FastDDS-3.6 SPDP datagram (domain 205,
    /// codepit capture 2026-05-29) directly into handle_spdp_datagram
    /// — does the runtime register FastDDS as a peer? Separates the
    /// receive problem (socket) from the handle problem (parse/insert/filter).
    #[test]
    fn handle_spdp_registers_real_fastdds_participant() {
        fn hx(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }
        const FASTDDS_SPDP: &str = "525450530203010f010fa72bbbebc90100000000090108006850196a57e882b41505b80100001000000100c7000100c2000000000100000000030000150004000203000016000400010f000000800400030601000f000400cd00000050001000010fa72bbbebc90100000000000001c107800400010000000380280021000000653830653630353335646339343432636231306537323239313038653135666600000000320018000100000024e50000000000000000000000000000c0a8b273310018000100000025e50000000000000000000000000000c0a8b273020008001400000000000000580004003ffc0f006200140010000000525450535061727469636970616e74005900cc0004000000110000005041525449434950414e545f54595045000000000700000053494d504c4500001b000000666173746464732e706879736963616c5f646174612e686f73740000210000006538306536303533356463393434326362313065373232393130386531356666000000001b000000666173746464732e706879736963616c5f646174612e75736572000005000000726f6f74000000001e000000666173746464732e706879736963616c5f646174612e70726f636573730000000600000036303334370000000100000080013800010000001ae50000000000000000000000000000efff00016850196a72ca8cb4020000000000000030040000000000000000000000000000";
        let bytes = hx(FASTDDS_SPDP);
        let prefix = GuidPrefix::from_bytes([0x99; 12]);
        let rt =
            Arc::new(DcpsRuntime::start(205, prefix, RuntimeConfig::default()).expect("rt start"));
        assert_eq!(rt.discovered_participants().len(), 0, "fresh: no peers");
        handle_spdp_datagram_for_test(&rt, &bytes);
        let n = rt.discovered_participants().len();
        assert_eq!(
            n, 1,
            "FastDDS must be registered after handle_spdp_datagram (got {n})"
        );
    }

    #[test]
    fn select_user_transport_tcpv4_yields_tcpv4_locator() {
        let prefix = GuidPrefix::from_bytes([1u8; 12]);
        let (t, accept) =
            select_user_transport(UserTransportKind::TcpV4, prefix, 0, Ipv4Addr::UNSPECIFIED)
                .expect("TcpV4 transport");
        assert_eq!(t.local_locator().kind, LocatorKind::Tcpv4);
        assert!(accept.is_some(), "TCP needs an accept handle");
    }

    #[test]
    fn select_user_transport_udpv4_default_kind() {
        let prefix = GuidPrefix::from_bytes([2u8; 12]);
        let (t, accept) =
            select_user_transport(UserTransportKind::UdpV4, prefix, 0, Ipv4Addr::UNSPECIFIED)
                .expect("UdpV4 transport");
        assert_eq!(t.local_locator().kind, LocatorKind::UdpV4);
        assert!(accept.is_none(), "UDP needs no accept handle");
    }

    #[cfg(feature = "same-host-uds")]
    #[test]
    fn select_user_transport_uds_yields_uds_locator() {
        let prefix = GuidPrefix::from_bytes([3u8; 12]);
        let (t, accept) =
            select_user_transport(UserTransportKind::Uds, prefix, 0, Ipv4Addr::UNSPECIFIED)
                .expect("Uds transport");
        assert_eq!(t.local_locator().kind, LocatorKind::Uds);
        assert!(accept.is_none(), "UDS needs no accept handle");
    }

    #[test]
    fn strip_user_encap_xcdr2_le() {
        let payload = [0x00, 0x07, 0x00, 0x00, 1, 2, 3];
        assert_eq!(strip_user_encap(&payload), Some(alloc::vec![1, 2, 3]));
    }

    #[test]
    fn strip_user_encap_xcdr1_le() {
        // Cyclone default for simple types.
        let payload = [0x00, 0x01, 0x00, 0x00, 0xAA];
        assert_eq!(strip_user_encap(&payload), Some(alloc::vec![0xAA]));
    }

    #[test]
    fn strip_user_encap_rejects_unknown_scheme() {
        let payload = [0xFF, 0xFF, 0x00, 0x00, 1];
        assert_eq!(strip_user_encap(&payload), None);
    }

    #[test]
    fn strip_user_encap_rejects_short() {
        assert_eq!(strip_user_encap(&[0x00, 0x07]), None);
    }

    #[test]
    fn user_payload_encap_is_cdr_le() {
        // CDR_LE (PLAIN_CDR / XCDR1, Little-Endian) — ehrliche
        // Declaration of the body encoding generated by codegen.
        assert_eq!(USER_PAYLOAD_ENCAP, [0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn data_repr_offer_str_uses_spec_ids() {
        use zerodds_rtps::publication_data::data_representation as dr;
        // XCDR1 -> Spec-Id 0 (NICHT 1 = XML); XCDR2 -> 2.
        assert_eq!(parse_data_repr_offer_str("XCDR1"), Some(vec![dr::XCDR]));
        assert_eq!(parse_data_repr_offer_str("XCDR2"), Some(vec![dr::XCDR2]));
        assert_eq!(parse_data_repr_offer_str("xcdr2"), Some(vec![dr::XCDR2]));
        assert_eq!(
            parse_data_repr_offer_str("XCDR2,XCDR1"),
            Some(vec![dr::XCDR2, dr::XCDR])
        );
        assert_eq!(parse_data_repr_offer_str("bogus"), None);
        assert_eq!(parse_data_repr_offer_str(""), None);
        // XCDR1 must NOT map to the XML id (1).
        assert_ne!(parse_data_repr_offer_str("XCDR1"), Some(vec![dr::XML]));
    }

    /// A DataReader announces every representation it can decode (XCDR2 + XCDR1)
    /// — XTypes 1.3 §7.6.2: the default reader policy accepts both. CycloneDDS
    /// (and legacy RTI / OpenDDS < 3.16) default their writers to XCDR1 for
    /// `@final` types; without XCDR1 in the reader's announced set those writers
    /// fail the DataRepresentation RxO check and never deliver. Regression for
    /// Bug DR1.
    #[test]
    fn reader_accept_repr_always_includes_both_representations() {
        use zerodds_rtps::publication_data::data_representation as dr;
        // Default writer offer [XCDR2] -> reader must also accept XCDR1.
        let widened = reader_accept_repr(&[dr::XCDR2]);
        assert!(widened.contains(&dr::XCDR2));
        assert!(widened.contains(&dr::XCDR));
        // XCDR2 stays first (the preferred / generated encoding).
        assert_eq!(widened[0], dr::XCDR2);
        // Already-both list is preserved (idempotent, order kept).
        assert_eq!(
            reader_accept_repr(&[dr::XCDR, dr::XCDR2]),
            alloc::vec![dr::XCDR, dr::XCDR2]
        );
        // Empty config still yields both.
        let from_empty = reader_accept_repr(&[]);
        assert!(from_empty.contains(&dr::XCDR2) && from_empty.contains(&dr::XCDR));
    }

    #[test]
    fn user_payload_encap_maps_repr_and_extensibility() {
        use zerodds_rtps::publication_data::data_representation as dr;
        use zerodds_types::qos::ExtensibilityForRepr as Ext;
        // DDSI-RTPS 2.5 §10.5 / XTypes 1.3 Tab.59 Encapsulation-IDs
        // (2-byte repr-id BE + 2-byte options=0), little-endian variant:
        //   XCDR1 final/appendable -> CDR_LE        0x0001
        //   XCDR1 mutable          -> PL_CDR_LE      0x0003
        //   XCDR2 final            -> PLAIN_CDR2_LE  0x0007
        //   XCDR2 appendable       -> D_CDR2_LE      0x0009
        //   XCDR2 mutable          -> PL_CDR2_LE     0x000b
        assert_eq!(
            user_payload_encap(dr::XCDR, Ext::Final, false),
            [0x00, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR, Ext::Appendable, false),
            [0x00, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR, Ext::Mutable, false),
            [0x00, 0x03, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR2, Ext::Final, false),
            [0x00, 0x07, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR2, Ext::Appendable, false),
            [0x00, 0x09, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR2, Ext::Mutable, false),
            [0x00, 0x0b, 0x00, 0x00]
        );
        // The default const is exactly the (XCDR1, Final) case.
        assert_eq!(
            user_payload_encap(dr::XCDR, Ext::Final, false),
            USER_PAYLOAD_ENCAP
        );
        // Unknown/XML repr falls back safely to CDR_LE.
        assert_eq!(
            user_payload_encap(dr::XML, Ext::Final, false),
            [0x00, 0x01, 0x00, 0x00]
        );
        // big_endian=true selects the `_BE` variant (the even predecessor of
        // the odd `_LE` id): CDR_BE 0x00, PL_CDR_BE 0x02, PLAIN_CDR2_BE 0x06,
        // D_CDR2_BE 0x08, PL_CDR2_BE 0x0a (RTPS 2.5 §10.5). Used by the
        // durability service to replay a big-endian peer's stored sample.
        assert_eq!(
            user_payload_encap(dr::XCDR, Ext::Final, true),
            [0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR, Ext::Mutable, true),
            [0x00, 0x02, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR2, Ext::Final, true),
            [0x00, 0x06, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR2, Ext::Appendable, true),
            [0x00, 0x08, 0x00, 0x00]
        );
        assert_eq!(
            user_payload_encap(dr::XCDR2, Ext::Mutable, true),
            [0x00, 0x0a, 0x00, 0x00]
        );
    }

    #[test]
    fn observability_sink_records_writer_and_reader_creation() {
        // VecSink injizieren, Writer + Reader erzeugen,
        // check that both events arrive.
        use std::sync::Arc as StdArc;
        use zerodds_foundation::observability::{Component, Level, VecSink};

        let sink = StdArc::new(VecSink::new());
        let cfg = RuntimeConfig {
            observability: sink.clone(),
            ..RuntimeConfig::default()
        };
        let rt =
            DcpsRuntime::start(7, GuidPrefix::from_bytes([0xAA; 12]), cfg).expect("start runtime");
        let _ = rt.register_user_writer(UserWriterConfig {
            topic_name: "ObsTopic".into(),
            type_name: "ObsType".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::Volatile,
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            data_representation_offer: None,
        });
        let _ = rt.register_user_reader(UserReaderConfig {
            topic_name: "ObsTopic".into(),
            type_name: "ObsType".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::Volatile,
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: None,
        });
        rt.shutdown();

        let events = sink.snapshot();
        assert!(
            events.iter().any(|e| e.name == "user_writer.created"
                && e.component == Component::Dcps
                && e.level == Level::Info),
            "writer-event missing: got {:?}",
            events.iter().map(|e| e.name).collect::<Vec<_>>()
        );
        assert!(
            events
                .iter()
                .any(|e| e.name == "user_reader.created" && e.component == Component::Dcps),
            "reader-event missing"
        );
        // The topic attribute must hang on the writer.created event.
        let writer_event = events
            .iter()
            .find(|e| e.name == "user_writer.created")
            .expect("writer event");
        assert!(
            writer_event
                .attrs
                .iter()
                .any(|a| a.key == "topic" && a.value == "ObsTopic"),
            "topic attr missing"
        );
    }

    #[test]
    fn user_endpoint_entity_kind_follows_keyedness() {
        // Regression (ROS-2 cross-vendor): the entityKind of a user
        // endpoint MUST follow the type keyedness (Spec §9.3.1.2). A
        // a keyless type yields NoKey (Writer 0x03 / Reader 0x04), a
        // keyed type WithKey (0x02 / 0x07). If this does not match the
        // peer, CycloneDDS/ROS 2 silently rejects the endpoint match
        // (DDS_INVALID_QOS_POLICY_ID, no log). create_datawriter/
        // create_datareader derive `is_keyed` from `DdsType::HAS_KEY`.
        use zerodds_rtps::wire_types::EntityKind;
        let rt = DcpsRuntime::start(
            11,
            GuidPrefix::from_bytes([0xBC; 12]),
            RuntimeConfig::default(),
        )
        .expect("start runtime");
        let mk_w = || UserWriterConfig {
            topic_name: "KindTopic".into(),
            type_name: "KindType".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::Volatile,
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            data_representation_offer: None,
        };
        let mk_r = || UserReaderConfig {
            topic_name: "KindTopic".into(),
            type_name: "KindType".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::Volatile,
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: None,
        };
        // keyless (HAS_KEY=false) -> NoKey
        let w_nokey = rt.register_user_writer_kind(mk_w(), false).expect("writer");
        assert_eq!(w_nokey.entity_kind, EntityKind::UserWriterNoKey);
        let (r_nokey, _) = rt.register_user_reader_kind(mk_r(), false).expect("reader");
        assert_eq!(r_nokey.entity_kind, EntityKind::UserReaderNoKey);
        // keyed (HAS_KEY=true) -> WithKey
        let w_key = rt.register_user_writer_kind(mk_w(), true).expect("writer");
        assert_eq!(w_key.entity_kind, EntityKind::UserWriterWithKey);
        let (r_key, _) = rt.register_user_reader_kind(mk_r(), true).expect("reader");
        assert_eq!(r_key.entity_kind, EntityKind::UserReaderWithKey);
        rt.shutdown();
    }

    #[test]
    fn incompatible_qos_match_emits_loud_warning() {
        // C2 "loud instead of silent": an incompatible QoS match is logged as a
        // warn event with topic + policy, not silently discarded.
        // Setup: writer Volatile + reader TransientLocal on the same
        // Topic (reader requests more durability than the writer offers)
        // → intra-runtime match fails with policy DURABILITY.
        use std::sync::Arc as StdArc;
        use zerodds_foundation::observability::{Component, Level, VecSink};

        let sink = StdArc::new(VecSink::new());
        let cfg_a = RuntimeConfig {
            observability: sink.clone(),
            tick_period: Duration::from_millis(5),
            ..RuntimeConfig::default()
        };
        let cfg_b = RuntimeConfig {
            tick_period: Duration::from_millis(5),
            ..RuntimeConfig::default()
        };
        // Two same-process runtimes, same domain → inproc discovery.
        let rt = DcpsRuntime::start(13, GuidPrefix::from_bytes([0xCE; 12]), cfg_a)
            .expect("start runtime a");
        let rt_b = DcpsRuntime::start(13, GuidPrefix::from_bytes([0xCF; 12]), cfg_b)
            .expect("start runtime b");
        let _w = rt
            .register_user_writer(UserWriterConfig {
                topic_name: "QT".into(),
                type_name: "QType".into(),
                reliable: false,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: alloc::vec![],
                user_data: alloc::vec![],
                topic_data: alloc::vec![],
                group_data: alloc::vec![],
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .expect("writer");
        let _r = rt_b
            .register_user_reader(UserReaderConfig {
                topic_name: "QT".into(),
                type_name: "QType".into(),
                reliable: false,
                durability: zerodds_qos::DurabilityKind::TransientLocal,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                partition: alloc::vec![],
                user_data: alloc::vec![],
                topic_data: alloc::vec![],
                group_data: alloc::vec![],
                type_identifier: zerodds_types::TypeIdentifier::None,
                type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                data_representation_offer: None,
            })
            .expect("reader");
        // Await the match pass.
        let mut found = false;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(25));
            let events = sink.snapshot();
            if events.iter().any(|e| {
                (e.name == "qos.incompatible.offered" || e.name == "qos.incompatible.requested")
                    && e.component == Component::Dcps
                    && e.level == Level::Warn
                    && e.attrs.iter().any(|a| a.key == "topic" && a.value == "QT")
                    && e.attrs
                        .iter()
                        .any(|a| a.key == "policy" && a.value == "DURABILITY")
            }) {
                found = true;
                break;
            }
        }
        rt.shutdown();
        rt_b.shutdown();
        assert!(
            found,
            "expected a loud qos.incompatible warn event with policy DURABILITY"
        );
    }

    #[test]
    fn spdp_unicast_port_follows_rtps_formula() {
        // Spec §9.6.1.4.1: PB + DG*domain + d1 + PG*pid = 7400+250*d+10+2*pid.
        assert_eq!(super::spdp_unicast_port(0, 0), 7410);
        assert_eq!(spdp_unicast_port(0, 1), 7412);
        assert_eq!(spdp_unicast_port(1, 0), 7660);
        assert_eq!(spdp_unicast_port(7, 0), 9160);
    }

    #[test]
    fn announce_locator_pins_interface_over_route_probe() {
        // Interface pinning: a set interface takes precedence over the
        // route probe (multi-homed robustness, cf. Cyclone NetworkInterface).
        let udp = UdpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0).expect("bind");
        let pin = Ipv4Addr::new(10, 11, 12, 13);
        let loc = super::announce_locator(&udp, pin);
        assert_eq!(loc.kind, zerodds_rtps::wire_types::LocatorKind::UdpV4);
        assert_eq!(loc.address[12..], [10, 11, 12, 13]);
        // Without a pin (UNSPECIFIED) → probe/fallback does NOT return the pin IP.
        let auto = super::announce_locator(&udp, Ipv4Addr::UNSPECIFIED);
        assert_ne!(auto.address[12..], [10, 11, 12, 13]);
    }

    #[test]
    fn expand_initial_peer_ip_only_yields_well_known_port_range() {
        let m = super::INITIAL_PEER_MAX_PARTICIPANTS;
        let mut out = Vec::new();
        super::expand_initial_peer("127.0.0.1", 0, m, &mut out);
        assert_eq!(out.len(), m as usize);
        assert_eq!(out[0].port, 7410);
        assert_eq!(out[1].port, 7412);
        // Larger limit → more ports (C1 dense multi-robot scenarios).
        let mut wide = Vec::new();
        super::expand_initial_peer("127.0.0.1", 0, 30, &mut wide);
        assert_eq!(wide.len(), 30);
        assert_eq!(wide[29].port, 7410 + 2 * 29);
        // ip:port -> exactly one exact locator.
        let mut one = Vec::new();
        super::expand_initial_peer("10.0.0.5:7410", 0, m, &mut one);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].port, 7410);
        assert_eq!(one[0].address[12..], [10, 0, 0, 5]);
        // Garbage is ignored.
        let mut none = Vec::new();
        super::expand_initial_peer("not-an-ip", 0, m, &mut none);
        assert!(none.is_empty());
    }

    #[test]
    #[ignore = "heavy multi-runtime scaling test (12 runtimes); explicit: cargo test -- --ignored"]
    #[allow(clippy::print_stdout)]
    fn multicast_free_discovery_scales_to_many_participants() {
        // C1 scaling: N participants, each with its own multicast group
        // (→ separate inproc buckets) AND multicast send off → pure
        // Unicast discovery via an explicit well-known-port peer list. Evidence,
        // that multicast-free all-to-all discovery works beyond 2 participants
        // (the "N²-multicast-storm" pain cluster, but unicast).
        // N via env (ZERODDS_SCALE_N, default 12) for >50 perf demos.
        let n: u32 = std::env::var("ZERODDS_SCALE_N")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(12)
            .clamp(2, 120);
        let domain = 21;
        let peers: Vec<Locator> = (0..n)
            .map(|pid| Locator::udp_v4([127, 0, 0, 1], super::spdp_unicast_port(domain, pid)))
            .collect();
        let mut rts = Vec::new();
        for i in 0..n {
            let cfg = RuntimeConfig {
                tick_period: Duration::from_millis(10),
                spdp_period: Duration::from_millis(40),
                // Own group per runtime → no inproc, no multicast.
                spdp_multicast_group: Ipv4Addr::new(239, 255, 21, (i + 1) as u8),
                spdp_multicast_send: false,
                initial_peers: peers.clone(),
                ..RuntimeConfig::default()
            };
            // Unique prefix even for n>47 (two-byte index).
            let mut pb = [0xD0u8; 12];
            pb[0] = (i & 0xff) as u8;
            pb[1] = (i >> 8) as u8;
            let prefix = GuidPrefix::from_bytes(pb);
            rts.push(DcpsRuntime::start(domain as i32, prefix, cfg).expect("start"));
        }
        // Wait until each participant has discovered all n-1 others.
        // Grosszuegiges Fenster: viele Runtimes konkurrieren um CPU; break-early.
        let started = std::time::Instant::now();
        let mut all_full = false;
        for _ in 0..1200 {
            std::thread::sleep(Duration::from_millis(25));
            if rts
                .iter()
                .all(|rt| rt.discovered_participants().len() >= (n as usize - 1))
            {
                all_full = true;
                break;
            }
        }
        let elapsed = started.elapsed();
        let min_seen = rts
            .iter()
            .map(|rt| rt.discovered_participants().len())
            .min()
            .unwrap_or(0);
        for rt in &rts {
            rt.shutdown();
        }
        println!(
            "C1-Scaling: {n} Participants multicast-frei all-to-all in {:.2}s (min={min_seen}/{})",
            elapsed.as_secs_f64(),
            n - 1
        );
        assert!(
            all_full,
            "multicast-free all-to-all discovery does not scale: min seen = {min_seen}/{}",
            n - 1
        );
    }

    #[test]
    fn default_reassembly_cap_is_ros_realistic() {
        // C3 regression: the DCPS reassembly cap must be ROS-PointCloud2/
        // Image-capable (several MB), not the conservative
        // rtps 1-MiB default that silently discards large samples.
        let cfg = RuntimeConfig::default();
        assert!(
            cfg.max_reassembly_sample_bytes >= 8 * 1024 * 1024,
            "reassembly cap too small for ROS PointCloud2/Image: {}",
            cfg.max_reassembly_sample_bytes
        );
    }

    #[test]
    fn ros_defaults_offers_xcdr1_for_ros_writers() {
        // C4: the ROS profile offers [XCDR1, XCDR2] (matches ROS/Cyclone
        // XCDR1 writer) + keeps the ROS-realistic reassembly cap.
        use zerodds_rtps::publication_data::data_representation as dr;
        let cfg = RuntimeConfig::ros_defaults();
        assert_eq!(
            cfg.data_representation_offer,
            alloc::vec![dr::XCDR, dr::XCDR2]
        );
        assert!(cfg.max_reassembly_sample_bytes >= 8 * 1024 * 1024);
    }

    #[test]
    fn multicast_free_discovery_via_initial_peers() {
        // C1: two runtimes with DIFFERENT multicast groups lie
        // in different inproc buckets AND cannot see each other via
        // multicast — so they discover each other EXCLUSIVELY via
        // the unicast initial peers (well-known SPDP ports on 127.0.0.1).
        let domain = 7;
        let mut peers = Vec::new();
        super::expand_initial_peer(
            "127.0.0.1",
            domain as u32,
            super::INITIAL_PEER_MAX_PARTICIPANTS,
            &mut peers,
        );
        let mk = |group: [u8; 4]| RuntimeConfig {
            tick_period: Duration::from_millis(10),
            spdp_period: Duration::from_millis(40),
            spdp_multicast_group: Ipv4Addr::from(group),
            // Multicast send fully off → rigorous unicast-only proof.
            spdp_multicast_send: false,
            initial_peers: peers.clone(),
            ..RuntimeConfig::default()
        };
        let a = DcpsRuntime::start(
            domain,
            GuidPrefix::from_bytes([0xA1; 12]),
            mk([239, 255, 7, 1]),
        )
        .expect("a");
        let b = DcpsRuntime::start(
            domain,
            GuidPrefix::from_bytes([0xB2; 12]),
            mk([239, 255, 7, 2]),
        )
        .expect("b");
        let mut discovered = false;
        for _ in 0..160 {
            std::thread::sleep(Duration::from_millis(25));
            if !a.discovered_participants().is_empty() && !b.discovered_participants().is_empty() {
                discovered = true;
                break;
            }
        }
        a.shutdown();
        b.shutdown();
        assert!(
            discovered,
            "multicast-freie Discovery via Unicast-Initial-Peers fehlgeschlagen"
        );
    }

    #[test]
    fn multi_robot_profile_is_multicast_free_and_wan_tolerant() {
        // C6: the named profile must be unicast-only with ROS reprs and a
        // WAN-tolerant lease, independent of any env.
        let cfg = RuntimeConfig::multi_robot();
        assert!(
            !cfg.spdp_multicast_send,
            "multi_robot() must disable multicast send"
        );
        assert_eq!(
            cfg.data_representation_offer,
            alloc::vec![
                zerodds_rtps::publication_data::data_representation::XCDR,
                zerodds_rtps::publication_data::data_representation::XCDR2
            ],
            "multi_robot() must offer the ROS XCDR1+XCDR2 reprs"
        );
        assert_eq!(
            cfg.participant_lease_duration,
            Duration::from_secs(300),
            "multi_robot() must use the WAN-tolerant 300s lease"
        );
    }

    #[test]
    fn multi_robot_profile_discovers_via_unicast() {
        // C6 e2e: two runtimes started from the `multi_robot()` profile (whose
        // `spdp_multicast_send = false` is the field under test) sit in
        // different multicast buckets and can ONLY find each other through the
        // unicast initial peers — proving the profile drives multicast-free
        // discovery end-to-end. Only test-timing + the peer list are
        // overridden; `spdp_multicast_send` comes from the profile.
        let domain = 9;
        let mut peers = Vec::new();
        super::expand_initial_peer(
            "127.0.0.1",
            domain as u32,
            super::INITIAL_PEER_MAX_PARTICIPANTS,
            &mut peers,
        );
        let mk = |group: [u8; 4]| RuntimeConfig {
            tick_period: Duration::from_millis(10),
            spdp_period: Duration::from_millis(40),
            spdp_multicast_group: Ipv4Addr::from(group),
            initial_peers: peers.clone(),
            ..RuntimeConfig::multi_robot()
        };
        let a = DcpsRuntime::start(
            domain,
            GuidPrefix::from_bytes([0xC6; 12]),
            mk([239, 255, 9, 1]),
        )
        .expect("a");
        let b = DcpsRuntime::start(
            domain,
            GuidPrefix::from_bytes([0xD7; 12]),
            mk([239, 255, 9, 2]),
        )
        .expect("b");
        let mut discovered = false;
        for _ in 0..160 {
            std::thread::sleep(Duration::from_millis(25));
            if !a.discovered_participants().is_empty() && !b.discovered_participants().is_empty() {
                discovered = true;
                break;
            }
        }
        a.shutdown();
        b.shutdown();
        assert!(
            discovered,
            "multi_robot() profile failed to discover via unicast initial peers"
        );
    }

    #[test]
    fn intra_runtime_writer_to_reader_loopback_delivers_sample() {
        // Bridge daemon use case: writer and reader in the SAME
        // DcpsRuntime, same topic+type. Before the same-runtime loopback
        // hook, a write() produced NO sample at the local reader,
        // because `inproc_announce_*` explicitly skips self and UDP multicast
        // loopback is not guaranteed.
        let rt = DcpsRuntime::start(
            17,
            GuidPrefix::from_bytes([0x42; 12]),
            RuntimeConfig::default(),
        )
        .expect("start runtime");
        let writer_eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: "IntraTopic".into(),
                type_name: "IntraType".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: alloc::vec![],
                user_data: alloc::vec![],
                topic_data: alloc::vec![],
                group_data: alloc::vec![],
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .expect("register writer");
        let (_reader_eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: "IntraTopic".into(),
                type_name: "IntraType".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                partition: alloc::vec![],
                user_data: alloc::vec![],
                topic_data: alloc::vec![],
                group_data: alloc::vec![],
                type_identifier: zerodds_types::TypeIdentifier::None,
                type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                data_representation_offer: None,
            })
            .expect("register reader");

        rt.write_user_sample(writer_eid, b"hello-intra-runtime".to_vec())
            .expect("write");

        // Same-runtime loopback is synchronous in the write_user_sample_borrowed
        // path — `recv_timeout` needs only microseconds, not the
        // wire roundtrip.
        let sample = rx
            .recv_timeout(core::time::Duration::from_millis(100))
            .expect("intra-runtime reader should receive sample");
        match sample {
            UserSample::Alive { payload, .. } => {
                assert_eq!(payload.as_ref(), b"hello-intra-runtime");
            }
            other => panic!("expected Alive, got {other:?}"),
        }
        rt.shutdown();
    }

    /// Bug R4 (#63): the same-runtime writer→reader loopback path
    /// (`intra_runtime_dispatch_alive`) used to hardcode the XCDR
    /// data-representation tag = `0`, so a DataWriter and DataReader sharing
    /// one `DcpsRuntime` lost the writer's real representation. Asserts the
    /// tag is carried through: default offer (`[XCDR2]`) → `1`, and an
    /// explicit `[XCDR1]` per-writer override → `0`. Also confirms a typed
    /// sample (XCDR2-framed body) round-trips intact alongside the tag.
    #[test]
    fn intra_runtime_loopback_preserves_representation_tag() {
        use zerodds_rtps::publication_data::data_representation as dr;

        fn run_case(domain: i32, prefix: u8, offer: Option<Vec<i16>>, expected_rep: u8) {
            let rt = DcpsRuntime::start(
                domain,
                GuidPrefix::from_bytes([prefix; 12]),
                RuntimeConfig::default(),
            )
            .expect("start runtime");
            let writer_eid = rt
                .register_user_writer(UserWriterConfig {
                    topic_name: "RepTopic".into(),
                    type_name: "RepType".into(),
                    reliable: true,
                    durability: zerodds_qos::DurabilityKind::Volatile,
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    lifespan: zerodds_qos::LifespanQosPolicy::default(),
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    ownership_strength: 0,
                    partition: alloc::vec![],
                    user_data: alloc::vec![],
                    topic_data: alloc::vec![],
                    group_data: alloc::vec![],
                    type_identifier: zerodds_types::TypeIdentifier::None,
                    data_representation_offer: offer,
                })
                .expect("register writer");
            let (_reader_eid, rx) = rt
                .register_user_reader(UserReaderConfig {
                    topic_name: "RepTopic".into(),
                    type_name: "RepType".into(),
                    reliable: true,
                    durability: zerodds_qos::DurabilityKind::Volatile,
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    partition: alloc::vec![],
                    user_data: alloc::vec![],
                    topic_data: alloc::vec![],
                    group_data: alloc::vec![],
                    type_identifier: zerodds_types::TypeIdentifier::None,
                    type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                    data_representation_offer: None,
                })
                .expect("register reader");

            // Typed sample: a `struct { long seq; }` XCDR2-aligned body
            // (little-endian 4-byte long). The intra-runtime path carries the
            // RAW body (no encap header), so the representation tag is the
            // only carrier of the wire version — exactly the lost signal.
            let seq: i32 = 0x0A0B_0C0D;
            let typed_payload = seq.to_le_bytes().to_vec();

            rt.write_user_sample(writer_eid, typed_payload.clone())
                .expect("write");

            let sample = rx
                .recv_timeout(core::time::Duration::from_millis(100))
                .expect("intra-runtime reader should receive sample");
            match sample {
                UserSample::Alive {
                    payload,
                    representation,
                    ..
                } => {
                    assert_eq!(
                        representation, expected_rep,
                        "intra-runtime loopback must carry the writer's XCDR \
                         version tag (offer→rep), not a hardcoded 0"
                    );
                    // Typed round-trip: the recovered body decodes to the
                    // original long.
                    assert_eq!(payload.as_ref(), typed_payload.as_slice());
                    let recovered =
                        i32::from_le_bytes(payload.as_ref()[..4].try_into().expect("4-byte long"));
                    assert_eq!(recovered, seq, "typed sample must round-trip");
                }
                other => panic!("expected Alive, got {other:?}"),
            }
            rt.shutdown();
        }

        // Default offer is `[XCDR2]` → tag `1`.
        run_case(19, 0x60, None, 1);
        // Explicit per-writer XCDR1 override → tag `0` (proves the value is
        // actually carried from the writer, not constant).
        run_case(20, 0x61, Some(alloc::vec![dr::XCDR]), 0);
        // Explicit per-writer XCDR2 override → tag `1`.
        run_case(21, 0x62, Some(alloc::vec![dr::XCDR2]), 1);
    }

    #[test]
    fn intra_runtime_loopback_not_matched_on_different_topic() {
        // Negative test: writer on TopicA, reader on TopicB — no
        // intra-runtime match, no sample. Prevents the
        // routing table from topic-blindly merging everything.
        let rt = DcpsRuntime::start(
            18,
            GuidPrefix::from_bytes([0x43; 12]),
            RuntimeConfig::default(),
        )
        .expect("start runtime");
        let writer_eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: "TopicA".into(),
                type_name: "TypeA".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: alloc::vec![],
                user_data: alloc::vec![],
                topic_data: alloc::vec![],
                group_data: alloc::vec![],
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .expect("register writer");
        let (_reader_eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: "TopicB".into(),
                type_name: "TypeB".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                partition: alloc::vec![],
                user_data: alloc::vec![],
                topic_data: alloc::vec![],
                group_data: alloc::vec![],
                type_identifier: zerodds_types::TypeIdentifier::None,
                type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                data_representation_offer: None,
            })
            .expect("register reader");

        rt.write_user_sample(writer_eid, b"should-not-arrive".to_vec())
            .expect("write");

        match rx.recv_timeout(core::time::Duration::from_millis(50)) {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => { /* expected */ }
            other => panic!("reader on different topic must not receive: got {other:?}"),
        }
        rt.shutdown();
    }

    #[test]
    fn runtime_starts_and_shuts_down_cleanly() {
        let rt = DcpsRuntime::start(
            42,
            GuidPrefix::from_bytes([7; 12]),
            RuntimeConfig::default(),
        )
        .expect("start runtime");
        assert_eq!(rt.domain_id, 42);
        // Wave 4b.2 (Spec `zerodds-zero-copy-1.0` §6): the SameHostTracker
        // must be initially empty and a same-host match (manually
        // simulated, without SEDP setup) must produce a `Pending`
        // entry. The real SEDP hook trigger is the job of the E2E
        // test in wave 4c — here only a smoke test of the wiring point.
        assert!(rt.same_host.is_empty(), "fresh runtime: no same-host pairs");
        let local_writer = zerodds_rtps::wire_types::Guid::new(
            rt.guid_prefix,
            zerodds_rtps::wire_types::EntityId::user_writer_with_key([1, 2, 3]),
        );
        let same_host_reader = zerodds_rtps::wire_types::Guid::new(
            rt.guid_prefix,
            zerodds_rtps::wire_types::EntityId::user_reader_with_key([4, 5, 6]),
        );
        rt.same_host
            .register_pending(local_writer, same_host_reader);
        assert_eq!(rt.same_host.len(), 1);
        assert!(matches!(
            rt.same_host.lookup(local_writer, same_host_reader),
            Some(crate::same_host::SameHostState::Pending)
        ));
        // Shutdown is idempotent.
        rt.shutdown();
        rt.shutdown();
    }

    #[test]
    fn spdp_announces_standard_bits_by_default() {
        // Default config (without security): standard bits + WLP bits 10/11
        // + TypeLookup bits 12/13 must be announced along;
        // secure bits 16..27 + SEDP-topics bits 28/29 must NOT
        // be set. Topics bits are optional per RTPS 2.5 §8.5.4.4
        // — ZeroDDS does not implement the native topic endpoints
        // (synthetic DCPSTopic derivation from pub/sub covers the
        // end-user need), so we do not announce the capability
        // either.
        let rt = DcpsRuntime::start(
            5,
            GuidPrefix::from_bytes([0xC; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let mask = rt.announced_builtin_endpoint_set();
        // Standard bits + WLP + TypeLookup.
        assert_ne!(mask & endpoint_flag::PARTICIPANT_ANNOUNCER, 0);
        assert_ne!(mask & endpoint_flag::PARTICIPANT_DETECTOR, 0);
        assert_ne!(mask & endpoint_flag::PUBLICATIONS_ANNOUNCER, 0);
        assert_ne!(mask & endpoint_flag::SUBSCRIPTIONS_DETECTOR, 0);
        assert_ne!(mask & endpoint_flag::PARTICIPANT_MESSAGE_DATA_WRITER, 0);
        assert_ne!(mask & endpoint_flag::PARTICIPANT_MESSAGE_DATA_READER, 0);
        assert_ne!(mask & endpoint_flag::TYPE_LOOKUP_REQUEST, 0);
        assert_ne!(mask & endpoint_flag::TYPE_LOOKUP_REPLY, 0);
        // Do NOT set the SEDP-topics bits — covered synthetically.
        assert_eq!(mask & endpoint_flag::TOPICS_ANNOUNCER, 0);
        assert_eq!(mask & endpoint_flag::TOPICS_DETECTOR, 0);
        // No secure bits without explicit announce_secure_endpoints.
        assert_eq!(mask & endpoint_flag::ALL_SECURE, 0);
    }

    #[test]
    fn spdp_announces_secure_bits_when_configured() {
        // With announce_secure_endpoints=true all 12 secure
        // bits (16..27) must be set.
        let config = RuntimeConfig {
            announce_secure_endpoints: true,
            ..Default::default()
        };
        let rt = DcpsRuntime::start(6, GuidPrefix::from_bytes([0xD; 12]), config).expect("start");
        let mask = rt.announced_builtin_endpoint_set();
        for bit in 16u32..=27 {
            assert!(
                mask & (1u32 << bit) != 0,
                "secure bit {bit} missing in the SPDP announce"
            );
        }
        // Standard bits must still be set.
        assert_eq!(
            mask & endpoint_flag::ALL_STANDARD,
            endpoint_flag::ALL_STANDARD
        );
    }

    #[test]
    fn spdp_lease_duration_is_configurable() {
        // Default 100 s (spec). The override of 17 s must arrive in the beacon.
        let config = RuntimeConfig {
            participant_lease_duration: Duration::from_secs(17),
            ..Default::default()
        };
        let rt = DcpsRuntime::start(7, GuidPrefix::from_bytes([0xE; 12]), config).expect("start");
        let secs = rt
            .spdp_beacon
            .lock()
            .map(|b| b.data.lease_duration.seconds)
            .unwrap_or(0);
        assert_eq!(secs, 17);
    }

    #[test]
    fn user_locator_is_udp_v4_127_0_0_x() {
        let rt = DcpsRuntime::start(
            0,
            GuidPrefix::from_bytes([0xA; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let loc = rt.user_locator();
        assert_eq!(loc.kind, zerodds_rtps::wire_types::LocatorKind::UdpV4);
        // Port > 0 (ephemeral).
        assert!(loc.port > 0);
    }

    #[test]
    fn two_runtimes_on_same_domain_can_coexist() {
        // The SPDP multicast port is SO_REUSE in our bind.
        let a = DcpsRuntime::start(
            3,
            GuidPrefix::from_bytes([0xA; 12]),
            RuntimeConfig::default(),
        )
        .expect("a");
        let b = DcpsRuntime::start(
            3,
            GuidPrefix::from_bytes([0xB; 12]),
            RuntimeConfig::default(),
        )
        .expect("b");
        assert_eq!(a.domain_id, b.domain_id);
    }

    #[test]
    fn peer_capabilities_unknown_peer_returns_none() {
        let rt = DcpsRuntime::start(
            10,
            GuidPrefix::from_bytes([0x60; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        // A fresh runtime has discovered no peer.
        let caps = rt.peer_capabilities(&GuidPrefix::from_bytes([0xEE; 12]));
        assert!(caps.is_none());
    }

    #[test]
    fn assert_liveliness_enqueues_wlp_pulse_without_panic() {
        // Smoke test: assert_liveliness() must not poison the lock
        // and must return synchronously.
        let rt = DcpsRuntime::start(
            8,
            GuidPrefix::from_bytes([0xF; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        rt.assert_liveliness();
        rt.assert_writer_liveliness(alloc::vec![0xDE, 0xAD]);
        // The lock must stay usable.
        let count = rt.wlp.lock().map(|w| w.peer_count()).unwrap_or(usize::MAX);
        assert_eq!(count, 0, "no peer announced itself → 0");
    }

    #[test]
    fn wlp_period_default_is_lease_over_three() {
        // With the default lease of 100 s → wlp_period = 33.33 s.
        let rt = DcpsRuntime::start(
            9,
            GuidPrefix::from_bytes([0x10; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        // We cannot read the value directly; but we
        // know: tick_period > 30 s means the default lease was
        // used. Enqueue a pulse and tick — it must fire,
        // the next AUTOMATIC comes only in 33 s.
        let mut wlp = rt.wlp.lock().unwrap();
        wlp.assert_participant();
        let now0 = Duration::from_secs(0);
        let dg = wlp.tick(now0).unwrap();
        assert!(dg.is_some(), "pulse is emitted immediately");
    }

    // Multicast loopback is unreliable on macOS (no auto-
    // interface-join with bind_multicast_v4(0.0.0.0)). On Linux
    // it works out of the box; there the test will run in CI.
    #[cfg(target_os = "linux")]
    #[test]
    fn two_runtimes_exchange_wlp_heartbeat_via_multicast() {
        // .D-e: A sends periodic WLP heartbeats. B must
        // know its own WLP endpoint with A's prefix as a peer
        // within ~3 tick periods.
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            // Aggressive WLP period for fast tests.
            wlp_period: Duration::from_millis(80),
            participant_lease_duration: Duration::from_millis(240),
            ..RuntimeConfig::default()
        };
        let _a = DcpsRuntime::start(2, GuidPrefix::from_bytes([0x40; 12]), cfg.clone()).expect("a");
        let _b = DcpsRuntime::start(2, GuidPrefix::from_bytes([0x41; 12]), cfg).expect("b");

        let a_prefix = GuidPrefix::from_bytes([0x40; 12]);
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            if _b.peer_liveliness_last_seen(&a_prefix).is_some() {
                return;
            }
        }
        panic!("B did not see A's WLP heartbeat within 3 s");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn two_runtimes_assert_liveliness_reaches_peer() {
        // The Manual-By-Participant pulse must arrive at the peer, the
        // last-seen timestamp must reset compared to purely Automatic
        // beats. Since the pulse goes out synchronously on the next
        // tick, a short wait suffices.
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            // WLP period large enough that no AUTOMATIC beat comes
            // in between within the test. The manual pulse queue
            // is processed before the AUTOMATIC slot.
            wlp_period: Duration::from_secs(3600),
            ..RuntimeConfig::default()
        };
        let a = DcpsRuntime::start(4, GuidPrefix::from_bytes([0x50; 12]), cfg.clone()).expect("a");
        let b = DcpsRuntime::start(4, GuidPrefix::from_bytes([0x51; 12]), cfg).expect("b");

        a.assert_liveliness();
        let a_prefix = GuidPrefix::from_bytes([0x50; 12]);
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            if b.peer_liveliness_last_seen(&a_prefix).is_some() {
                return;
            }
        }
        // In case of multicast-loopback problems, at least check A's
        // own pulse counter.
        panic!("B did not see A's manual liveliness assert within 3 s");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn two_runtimes_exchange_sedp_publication_announce() {
        // E2E smoke: A announces a publication, B sees it
        // via SEDP. Assumes SPDP works (so that
        // the SEDP peer proxies get wired).
        use zerodds_qos::{DurabilityKind, ReliabilityKind};
        use zerodds_rtps::publication_data::PublicationBuiltinTopicData;

        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        // Own domain, so the test does not collide with the SPDP-only test
        // on domain 0 over the multicast port.
        let a = DcpsRuntime::start(1, GuidPrefix::from_bytes([0xCC; 12]), cfg.clone()).expect("a");
        let b = DcpsRuntime::start(1, GuidPrefix::from_bytes([0xDD; 12]), cfg).expect("b");

        // Wait until both see each other via SPDP.
        for _ in 0..40 {
            thread::sleep(Duration::from_millis(50));
            if !a.discovered_participants().is_empty() && !b.discovered_participants().is_empty() {
                break;
            }
        }
        assert!(
            !a.discovered_participants().is_empty(),
            "no SPDP discovery a"
        );

        // A announces a publication for topic "Chatter" with type "RawBytes".
        let pub_data = PublicationBuiltinTopicData {
            key: Guid::new(
                a.guid_prefix,
                EntityId::user_writer_with_key([0x01, 0x02, 0x03]),
            ),
            participant_key: Guid::new(a.guid_prefix, EntityId::PARTICIPANT),
            topic_name: "Chatter".into(),
            type_name: "zerodds::RawBytes".into(),
            durability: DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: QosDuration::from_millis(100_i32),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            partition: Vec::new(),
            user_data: Vec::new(),
            topic_data: Vec::new(),
            group_data: Vec::new(),
            type_information: None,
            data_representation: Vec::new(),
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
        };
        a.announce_publication(&pub_data).expect("announce");

        // B should have the publication in the cache within ~3 s.
        // CI on shared runners has more jitter, 1 s was too tight.
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            if b.discovered_publications_count() > 0 {
                return;
            }
        }
        panic!(
            "B did not receive SEDP publication within 3 s (pub_count={})",
            b.discovered_publications_count()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn two_runtimes_e2e_user_data_match_and_transfer() {
        // E2E smoke: kompletter Pfad
        //   Runtime-A register_user_writer(topic, type)
        //   Runtime-B register_user_reader(topic, type)
        //   SEDP match, writer add_reader_proxy, reader add_writer_proxy
        //   A.write_user_sample(payload) → UDP → B's mpsc::Receiver
        //
        // Eigene Domain (2) um Kollisionen zu vermeiden.
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        let a = DcpsRuntime::start(2, GuidPrefix::from_bytes([0xEE; 12]), cfg.clone()).expect("a");
        let b = DcpsRuntime::start(2, GuidPrefix::from_bytes([0xFF; 12]), cfg).expect("b");

        // SPDP mutual — 3 s Budget.
        let mut spdp_ok = false;
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            if !a.discovered_participants().is_empty() && !b.discovered_participants().is_empty() {
                spdp_ok = true;
                break;
            }
        }
        assert!(spdp_ok, "SPDP mutual discovery did not complete in 3 s");

        // Register endpoints. A publish, B subscribe.
        let wid = a
            .register_user_writer(UserWriterConfig {
                topic_name: "Chatter".into(),
                type_name: "zerodds::RawBytes".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .expect("wid");
        let (_rid, rx) = b
            .register_user_reader(UserReaderConfig {
                topic_name: "Chatter".into(),
                type_name: "zerodds::RawBytes".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                data_representation_offer: None,
            })
            .expect("rid");

        // SEDP match + User-Data-Flow. `add_reader_proxy` triggert
        // a heartbeat immediately (RTPS §8.4.15.4), so ~tick_period
        // (20 ms) + response-delay (200 ms) + resend ≈ 300 ms in
        // idle state. A 4 s budget suffices even with CI jitter.
        let mut attempts = 0;
        loop {
            thread::sleep(Duration::from_millis(50));
            let _ = a.write_user_sample(wid, alloc::vec![0xAA, 0xBB, 0xCC]);
            if let Ok(sample) = rx.recv_timeout(Duration::from_millis(50)) {
                match sample {
                    UserSample::Alive { payload, .. } => {
                        assert_eq!(payload.as_slice(), &[0xAA, 0xBB, 0xCC][..]);
                        return;
                    }
                    other => panic!("expected Alive sample, got {other:?}"),
                }
            }
            attempts += 1;
            if attempts > 80 {
                panic!("no sample delivered within 4 s");
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn two_runtimes_discover_each_other_via_spdp() {
        // We use a tight SPDP period so the test does not wait 5 s.
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            ..RuntimeConfig::default()
        };
        // Eigene Domain 3 (SEDP=1, E2E=2) um Cross-Test-Kollision zu vermeiden.
        let a = DcpsRuntime::start(3, GuidPrefix::from_bytes([0xAA; 12]), cfg.clone()).expect("a");
        let b = DcpsRuntime::start(3, GuidPrefix::from_bytes([0xBB; 12]), cfg).expect("b");

        // Give the loop time for 2-3 beacon rounds. Multicast on
        // loopback is somewhat timing-sensitive when parallel tests
        // share the multicast group — hence 60 iterations of 50 ms
        // = 3 s budget instead of 1 s.
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            let a_sees_b = a
                .discovered_participants()
                .iter()
                .any(|p| p.sender_prefix == GuidPrefix::from_bytes([0xBB; 12]));
            let b_sees_a = b
                .discovered_participants()
                .iter()
                .any(|p| p.sender_prefix == GuidPrefix::from_bytes([0xAA; 12]));
            if a_sees_b && b_sees_a {
                return;
            }
        }
        panic!(
            "mutual SPDP discovery failed within 3 s (a={} b={})",
            a.discovered_participants().len(),
            b.discovered_participants().len()
        );
    }

    // =======================================================================
    // Security: Writer-Side Per-Reader-Serializer
    // =======================================================================

    #[cfg(feature = "security")]
    #[test]
    fn per_target_serializer_produces_different_wire_per_reader() {
        use zerodds_security_crypto::AesGcmCryptoPlugin;
        use zerodds_security_permissions::parse_governance_xml;
        use zerodds_security_runtime::{
            PeerCapabilities, ProtectionLevel as SecProtectionLevel, SharedSecurityGate,
        };

        // The governance enforces ENCRYPT on domain 0 — the default
        // path (transform_outbound) wraps too. A per-reader override
        // can still deliver plaintext if the reader is legacy.
        const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );

        let cfg = RuntimeConfig {
            security: Some(std::sync::Arc::new(gate)),
            ..RuntimeConfig::default()
        };
        let rt =
            DcpsRuntime::start(0, GuidPrefix::from_bytes([0xE4; 12]), cfg).expect("start runtime");

        let wid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: "HeteroTopic".into(),
                type_name: "zerodds::RawBytes".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .expect("register writer");

        // Drei fiktive Reader-Targets — eines pro Protection-Klasse.
        let legacy_loc = Locator::udp_v4([127, 0, 0, 11], 40001);
        let fast_loc = Locator::udp_v4([127, 0, 0, 12], 40002);
        let secure_loc = Locator::udp_v4([127, 0, 0, 13], 40003);
        let legacy_peer: [u8; 12] = [0x11; 12];
        let fast_peer: [u8; 12] = [0x22; 12];
        let secure_peer: [u8; 12] = [0x33; 12];

        // Simulates the SEDP match: populate the writer-slot maps.
        {
            let arc = rt.writer_slot(wid).unwrap();
            let mut slot = arc.lock().unwrap();
            slot.reader_protection
                .insert(legacy_peer, SecProtectionLevel::None);
            slot.reader_protection
                .insert(fast_peer, SecProtectionLevel::Sign);
            slot.reader_protection
                .insert(secure_peer, SecProtectionLevel::Encrypt);
            slot.locator_to_peer.insert(legacy_loc, legacy_peer);
            slot.locator_to_peer.insert(fast_loc, fast_peer);
            slot.locator_to_peer.insert(secure_loc, secure_peer);
        }

        // Fiktive Writer-Datagram-Bytes (RTPS-Header + User-Payload).
        let mut msg = Vec::new();
        msg.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        msg.extend_from_slice(&[0xE4; 12]); // GuidPrefix
        msg.extend_from_slice(b"HELLO-HETERO");

        let wire_legacy =
            secure_outbound_for_target(&rt, wid, &msg, &legacy_loc).expect("legacy path");
        let wire_fast = secure_outbound_for_target(&rt, wid, &msg, &fast_loc).expect("fast path");
        let wire_secure =
            secure_outbound_for_target(&rt, wid, &msg, &secure_loc).expect("secure path");

        // Spec §8.4.2.4: under rtps_protection_kind=ENCRYPT EVERY message MUST
        // be SRTPS-wrapped — even a legacy reader (data-level None) may
        // get NO plaintext, otherwise user DATA leaks on a protected
        // domain. The per-reader data level only controls the inner payload/
        // submessage layer, not the outer rtps_protection.
        assert_ne!(
            wire_legacy, msg,
            "legacy under rtps_protection=ENCRYPT MUST be SRTPS-wrapped (no plaintext leak)"
        );
        assert_ne!(wire_fast, msg, "fast reader must be protected");
        assert_ne!(wire_secure, msg, "secure reader must be protected");

        // Heterogeneity proof: the three wires are pairwise
        // different (each with its own nonce/session counter in SRTPS).
        assert_ne!(wire_legacy, wire_fast);
        assert_ne!(wire_legacy, wire_secure);
        assert_ne!(wire_fast, wire_secure);

        // Without a locator match the fallback must take the domain-rule path
        // — this governance requires ENCRYPT, so SRTPS-wrapped.
        let unknown_loc = Locator::udp_v4([127, 0, 0, 99], 40099);
        let wire_unknown =
            secure_outbound_for_target(&rt, wid, &msg, &unknown_loc).expect("fallback path");
        assert_ne!(
            wire_unknown, msg,
            "unknown target should be protected via the domain rule"
        );

        // The absence of the PeerCapabilities type is a compile check:
        // the import shows that the entire per-reader structure
        // is available in the dcps integration.
        let _unused: PeerCapabilities = PeerCapabilities::default();

        rt.shutdown();
    }

    // =======================================================================
    // Security: Reader-Side Per-Writer-Validator + Logging
    // =======================================================================

    #[cfg(feature = "security")]
    #[derive(Default, Clone)]
    struct CapturingLogger {
        inner: std::sync::Arc<
            std::sync::Mutex<Vec<(zerodds_security_runtime::LogLevel, String, String)>>,
        >,
    }

    #[cfg(feature = "security")]
    impl CapturingLogger {
        fn events(&self) -> Vec<(zerodds_security_runtime::LogLevel, String, String)> {
            self.inner.lock().map(|g| g.clone()).unwrap_or_default()
        }
    }

    #[cfg(feature = "security")]
    impl zerodds_security_runtime::LoggingPlugin for CapturingLogger {
        fn log(
            &self,
            level: zerodds_security_runtime::LogLevel,
            _participant: [u8; 16],
            category: &str,
            message: &str,
        ) {
            if let Ok(mut g) = self.inner.lock() {
                g.push((level, category.to_string(), message.to_string()));
            }
        }
        fn plugin_class_id(&self) -> &str {
            "zerodds.test.capturing_logger"
        }
    }

    #[cfg(feature = "security")]
    fn build_runtime_with(
        gov_xml: &str,
        logger: std::sync::Arc<CapturingLogger>,
    ) -> std::sync::Arc<DcpsRuntime> {
        use zerodds_security_crypto::AesGcmCryptoPlugin;
        use zerodds_security_permissions::parse_governance_xml;
        use zerodds_security_runtime::{LoggingPlugin, SharedSecurityGate};
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(gov_xml).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let logger_dyn: std::sync::Arc<dyn LoggingPlugin> = logger;
        let cfg = RuntimeConfig {
            security: Some(std::sync::Arc::new(gate)),
            security_logger: Some(logger_dyn),
            ..RuntimeConfig::default()
        };
        DcpsRuntime::start(0, GuidPrefix::from_bytes([0xE7; 12]), cfg).expect("start rt")
    }

    #[cfg(feature = "security")]
    #[test]
    fn inbound_plain_on_encrypt_domain_drops_with_error_event() {
        // DoD plan §stage 5: writer sends plain, policy expects
        // ENCRYPT → Reader droppt. Ohne allow_unauthenticated ist
        // this a "LegacyBlocked" → error level (not warning) per
        // the plan spec "missing-caps = Error".
        const GOV_ENCRYPT: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let logger = std::sync::Arc::new(CapturingLogger::default());
        let rt = build_runtime_with(GOV_ENCRYPT, std::sync::Arc::clone(&logger));

        // Plain-RTPS-Datagram (header + body).
        let mut plain = Vec::new();
        plain.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        plain.extend_from_slice(&[0x77; 12]); // attacker guid_prefix
        plain.extend_from_slice(b"plaintext-on-encrypted-domain");

        let out = secure_inbound_bytes(&rt, &plain, &NetInterface::Wan);
        assert!(out.is_none(), "tampering packet must be dropped");

        let events = logger.events();
        assert_eq!(events.len(), 1, "exactly one log event expected");
        let (level, category, _msg) = &events[0];
        assert_eq!(
            *level,
            zerodds_security_runtime::LogLevel::Error,
            "plain-on-protected-domain without allow_unauth = Error (LegacyBlocked)"
        );
        assert_eq!(category, "inbound.legacy_blocked");
        rt.shutdown();
    }

    #[cfg(feature = "security")]
    #[test]
    fn inbound_legacy_peer_accepted_when_governance_allows_unauth() {
        // DoD plan §stage 5: the legacy peer can keep talking to the reader,
        // when the governance sets allow_unauthenticated_participants=true.
        const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <allow_unauthenticated_participants>TRUE</allow_unauthenticated_participants>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let logger = std::sync::Arc::new(CapturingLogger::default());
        let rt = build_runtime_with(GOV, std::sync::Arc::clone(&logger));

        let mut plain = Vec::new();
        plain.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        plain.extend_from_slice(&[0x88; 12]);
        plain.extend_from_slice(b"legacy-but-allowed");

        let out = secure_inbound_bytes(&rt, &plain, &NetInterface::Wan)
            .expect("legacy peer must be accepted");
        assert_eq!(out, plain, "output is byte-identical (no crypto unwrap)");
        assert!(
            logger.events().is_empty(),
            "no log event on the accept path"
        );
        rt.shutdown();
    }

    #[cfg(feature = "security")]
    #[test]
    fn inbound_malformed_drops_and_logs_error() {
        const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>NONE</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let logger = std::sync::Arc::new(CapturingLogger::default());
        let rt = build_runtime_with(GOV, std::sync::Arc::clone(&logger));

        let out = secure_inbound_bytes(&rt, &[1, 2, 3, 4], &NetInterface::Wan);
        assert!(out.is_none());
        let events = logger.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, zerodds_security_runtime::LogLevel::Error);
        assert_eq!(events[0].1, "inbound.malformed");
        rt.shutdown();
    }

    #[cfg(feature = "security")]
    #[test]
    fn inbound_without_security_gate_bypasses_classify_and_logger() {
        // Without a security gate: passthrough, no log event.
        let logger = std::sync::Arc::new(CapturingLogger::default());
        let logger_dyn: std::sync::Arc<dyn zerodds_security_runtime::LoggingPlugin> =
            std::sync::Arc::clone(&logger) as _;
        let cfg = RuntimeConfig {
            security_logger: Some(logger_dyn),
            ..RuntimeConfig::default()
        };
        let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([0xE8; 12]), cfg).unwrap();
        let msg = vec![0xAAu8; 40];
        let out = secure_inbound_bytes(&rt, &msg, &NetInterface::Wan).unwrap();
        assert_eq!(out, msg);
        assert!(
            logger.events().is_empty(),
            "the logger must NOT be called without a gate"
        );
        rt.shutdown();
    }

    // =======================================================================
    // Security: Interface-Routing (Multi-Socket-Binding)
    // =======================================================================

    #[cfg(feature = "security")]
    fn lo_range(third: u8) -> zerodds_security_runtime::IpRange {
        zerodds_security_runtime::IpRange {
            base: core::net::IpAddr::V4(core::net::Ipv4Addr::new(127, 0, 0, third)),
            prefix_len: 32,
        }
    }

    #[cfg(feature = "security")]
    #[test]
    fn outbound_pool_routes_target_to_matching_binding() {
        let specs = vec![
            InterfaceBindingSpec {
                name: "lo-a".into(),
                bind_addr: Ipv4Addr::new(127, 0, 0, 1),
                bind_port: 0,
                kind: zerodds_security_runtime::NetInterface::Loopback,
                subnet: lo_range(11),
                default: false,
            },
            InterfaceBindingSpec {
                name: "lo-b".into(),
                bind_addr: Ipv4Addr::new(127, 0, 0, 1),
                bind_port: 0,
                kind: zerodds_security_runtime::NetInterface::Wan,
                subnet: lo_range(22),
                default: true,
            },
        ];
        let pool = OutboundSocketPool::bind_all(&specs).expect("pool");

        // Exact match on the first subnet -> lo-a.
        let t1 = Locator::udp_v4([127, 0, 0, 11], 40000);
        let (sock1, iface1) = pool.route(&t1).expect("route 1");
        assert_eq!(iface1, zerodds_security_runtime::NetInterface::Loopback);

        // Exact match on the second subnet -> lo-b.
        let t2 = Locator::udp_v4([127, 0, 0, 22], 40000);
        let (sock2, iface2) = pool.route(&t2).expect("route 2");
        assert_eq!(iface2, zerodds_security_runtime::NetInterface::Wan);

        // The two sockets must have different local ports.
        let p1 = sock1.local_locator().port;
        let p2 = sock2.local_locator().port;
        assert_ne!(p1, p2);
    }

    #[cfg(feature = "security")]
    #[test]
    fn outbound_pool_falls_back_to_default_when_no_subnet_matches() {
        let specs = vec![
            InterfaceBindingSpec {
                name: "lo-specific".into(),
                bind_addr: Ipv4Addr::new(127, 0, 0, 1),
                bind_port: 0,
                kind: zerodds_security_runtime::NetInterface::Loopback,
                subnet: lo_range(33),
                default: false,
            },
            InterfaceBindingSpec {
                name: "wan-default".into(),
                bind_addr: Ipv4Addr::new(127, 0, 0, 1),
                bind_port: 0,
                kind: zerodds_security_runtime::NetInterface::Wan,
                subnet: zerodds_security_runtime::IpRange {
                    base: core::net::IpAddr::V4(core::net::Ipv4Addr::UNSPECIFIED),
                    prefix_len: 0,
                },
                default: true,
            },
        ];
        let pool = OutboundSocketPool::bind_all(&specs).unwrap();
        let unknown = Locator::udp_v4([192, 168, 7, 7], 12345);
        let (_sock, iface) = pool.route(&unknown).expect("default fallback");
        assert_eq!(iface, zerodds_security_runtime::NetInterface::Wan);
    }

    #[cfg(feature = "security")]
    #[test]
    fn outbound_pool_returns_none_when_no_match_and_no_default() {
        let specs = vec![InterfaceBindingSpec {
            name: "only-lo".into(),
            bind_addr: Ipv4Addr::new(127, 0, 0, 1),
            bind_port: 0,
            kind: zerodds_security_runtime::NetInterface::Loopback,
            subnet: lo_range(44),
            default: false,
        }];
        let pool = OutboundSocketPool::bind_all(&specs).unwrap();
        assert!(pool.route(&Locator::udp_v4([8, 8, 8, 8], 53)).is_none());
    }

    #[cfg(feature = "security")]
    #[test]
    fn outbound_pool_skips_non_v4_locators() {
        let specs = vec![InterfaceBindingSpec {
            name: "lo".into(),
            bind_addr: Ipv4Addr::new(127, 0, 0, 1),
            bind_port: 0,
            kind: zerodds_security_runtime::NetInterface::Loopback,
            subnet: lo_range(55),
            default: true,
        }];
        let pool = OutboundSocketPool::bind_all(&specs).unwrap();
        // SHM locator (no IPv4) → no match; without a default it would be None,
        // here default=true and subnet-contains does not apply
        // because ipv4_from_locator returns None.
        let shm = Locator {
            kind: zerodds_rtps::wire_types::LocatorKind::Shm,
            port: 0,
            address: [0u8; 16],
        };
        assert!(pool.route(&shm).is_none());
    }

    #[cfg(feature = "security")]
    #[test]
    fn dod_plaintext_lo_vs_srtps_wan_via_sniffer() {
        // Spec §8.4.2.4 (spec wins vs DoD loopback plaintext): under
        // rtps_protection_kind=ENCRYPT means bytes are SRTPS-wrapped on EVERY
        // interface — including loopback. The test proves that the
        // per-interface routing serves both targets AND both outputs
        // are spec-conformantly protected (no plaintext leak, regardless of which
        // binding).
        //
        // Setup:
        //  * 2 sniffer UDP sockets, one simulates a legacy
        //    loopback peer (expects plaintext), the other a
        //    WAN secure peer (expects SRTPS).
        //  * DcpsRuntime with a security gate (governance = ENCRYPT) and
        //    two interface bindings: lo-binding on 127.0.0.100,
        //    wan-binding auf 127.0.0.200.
        //  * 1 writer, 2 matched_readers with different protection
        //    (Legacy=None, Secure=Encrypt) and the respective sniffer
        //    Socket address as the locator_to_peer target.
        //  * `send_on_best_interface(rt, target, bytes)` is triggered
        //    manually; the sniffer per target receives and checks
        //    the wire format.
        use std::net::{SocketAddrV4, UdpSocket};
        use zerodds_security_crypto::AesGcmCryptoPlugin;
        use zerodds_security_permissions::parse_governance_xml;
        use zerodds_security_runtime::{NetInterface as SecIf, SharedSecurityGate};

        const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        // Two sniffer sockets on ephemeral loopback ports (independent
        // from our bindings; they act as "peer receivers").
        let lo_sniffer =
            UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0)).expect("lo sniffer");
        lo_sniffer
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let wan_sniffer = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0))
            .expect("wan sniffer");
        wan_sniffer
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let lo_port = lo_sniffer.local_addr().unwrap().port();
        let wan_port = wan_sniffer.local_addr().unwrap().port();
        let lo_target = Locator::udp_v4([127, 0, 0, 1], u32::from(lo_port));
        let wan_target = Locator::udp_v4([127, 0, 0, 1], u32::from(wan_port));

        // Two bindings, subnet-matched to exactly these ports. Since
        // IpRange currently matches only on IP, we use two
        // different /32 host ranges as a trick:
        // we set both bindings to the same IP/32, but because
        // `route` takes the first subnet match, I list them such
        // that "lo-bind" comes first and then the default.
        //
        // Correct: both sniffers share 127.0.0.1/32 and the pool would
        // pick the first binding. To distinguish cleanly, we map
        // the binding decision by *target port* — that works
        // not today. So: we work around this subtlety by
        // calling `send_on_best_interface` directly for different targets
        // and assigning the binding by IP range —
        // the DoD checks the routing at the binding level, not the
        // socket layer.
        //
        // Pragmatically: we test end-to-end that the pool actually
        // picks the right interface socket for the target and
        // processes the bytes differently (plain vs SRTPS).
        // The target locators differ only in the port, but
        // `send_on_best_interface` gets them separately each. The
        // decisive point is: both bindings send **and** the
        // sniffer socket receives — proving the routing in combination
        // with the per-reader serializer from stage 4.

        let bindings = vec![InterfaceBindingSpec {
            name: "lo-for-legacy".into(),
            bind_addr: Ipv4Addr::new(127, 0, 0, 1),
            bind_port: 0,
            kind: SecIf::Loopback,
            subnet: zerodds_security_runtime::IpRange {
                base: core::net::IpAddr::V4(core::net::Ipv4Addr::new(127, 0, 0, 1)),
                prefix_len: 32,
            },
            default: true,
        }];
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let cfg = RuntimeConfig {
            security: Some(std::sync::Arc::new(gate)),
            interface_bindings: bindings,
            ..RuntimeConfig::default()
        };
        let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([0xF0; 12]), cfg).expect("rt");

        let wid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: "HeteroRouting".into(),
                type_name: "zerodds::RawBytes".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .unwrap();

        // Peer protection setup: Legacy=None for lo_target,
        // Encrypt for wan_target.
        let legacy_peer: [u8; 12] = [0x01; 12];
        let secure_peer: [u8; 12] = [0x02; 12];
        {
            let arc = rt.writer_slot(wid).unwrap();
            let mut slot = arc.lock().unwrap();
            slot.reader_protection
                .insert(legacy_peer, ProtectionLevel::None);
            slot.reader_protection
                .insert(secure_peer, ProtectionLevel::Encrypt);
            slot.locator_to_peer.insert(lo_target, legacy_peer);
            slot.locator_to_peer.insert(wan_target, secure_peer);
        }

        // Fiktives Datagram.
        let mut msg = Vec::new();
        msg.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        msg.extend_from_slice(&[0xF0; 12]);
        msg.extend_from_slice(b"DOD-ROUTING-PAYLOAD");

        // Generate the per-target wire + route via send_on_best_interface.
        let plain_wire = secure_outbound_for_target(&rt, wid, &msg, &lo_target).unwrap();
        let secure_wire = secure_outbound_for_target(&rt, wid, &msg, &wan_target).unwrap();
        assert_ne!(
            plain_wire, msg,
            "lo-target under rtps_protection=ENCRYPT also SRTPS (no plaintext leak)"
        );
        assert_ne!(secure_wire, msg, "wan-target: SRTPS-wrapped");

        send_on_best_interface(&rt, &lo_target, &plain_wire);
        send_on_best_interface(&rt, &wan_target, &secure_wire);

        // sniffer receive and compare.
        let mut buf = [0u8; 4096];
        let (n1, _) = lo_sniffer.recv_from(&mut buf).expect("lo snif got");
        assert_ne!(
            &buf[..n1],
            &msg[..],
            "loopback sniffer must see SRTPS (spec wins, no plaintext on a protected domain)"
        );
        assert_eq!(buf[20], 0x33, "lo output must begin with SRTPS_PREFIX");
        let (n2, _) = wan_sniffer.recv_from(&mut buf).expect("wan snif got");
        assert_ne!(&buf[..n2], &msg[..], "WAN sniffer must see SRTPS-wrapped");
        // Additionally: SRTPS marker at the 20th byte (after the RTPS header).
        // SRTPS_PREFIX-Submessage-Id = 0x33 (Spec §7.3.6.3).
        assert_eq!(
            buf[20], 0x33,
            "WAN output must begin with an SRTPS_PREFIX submessage"
        );

        rt.shutdown();
    }

    #[cfg(feature = "security")]
    #[test]
    fn inbound_loopback_accepts_plain_on_protected_domain() {
        // Plan §stage 6: the inbound dispatcher should accept plaintext
        // for loopback packets even on a protected domain
        // (bytes do not leave the host). That is
        // exactly the `NetInterface` consultation in classify_inbound.
        use zerodds_security_runtime::NetInterface as SecIf;
        const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let logger = std::sync::Arc::new(CapturingLogger::default());
        let rt = build_runtime_with(GOV, std::sync::Arc::clone(&logger));

        let mut plain = Vec::new();
        plain.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        plain.extend_from_slice(&[0x99; 12]);
        plain.extend_from_slice(b"loopback-plain-is-ok");

        // Accepted on loopback — no log event.
        let out = secure_inbound_bytes(&rt, &plain, &SecIf::Loopback)
            .expect("loopback plain must be accepted");
        assert_eq!(out, plain);
        assert!(logger.events().is_empty());

        // On WAN the same content → drop + error event.
        let out_wan = secure_inbound_bytes(&rt, &plain, &SecIf::Wan);
        assert!(out_wan.is_none());
        let evs = logger.events();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].0, zerodds_security_runtime::LogLevel::Error);
        assert!(
            evs[0].2.contains("iface=Wan"),
            "log message must carry iface"
        );
        rt.shutdown();
    }

    #[cfg(feature = "security")]
    #[test]
    fn dod_inbound_per_interface_receive_via_pool_socket() {
        // Plan §stage 6 inbound DoD: each pool binding has its
        // own receive path, and the NetInterface class is
        // reflected in the log event (iface=<class>).
        //
        // Setup:
        //  * DcpsRuntime with 1 InterfaceBinding (kind=Loopback,
        //    subnet=127.0.0.0/8)
        //  * Protected Governance + CapturingLogger
        //  * We bind an external UDP socket and send two
        //    plain packets:
        //      a) to the pool socket (the event loop polls it and
        //         classifies as loopback → accept without log)
        //      b) we trigger secure_inbound_bytes directly with Wan
        //         → error log with iface=Wan
        //
        // This proves that the per-interface receive path
        // exists and the iface class flows through the decision.
        use std::net::{SocketAddrV4, UdpSocket};
        use zerodds_security_crypto::AesGcmCryptoPlugin;
        use zerodds_security_permissions::parse_governance_xml;
        use zerodds_security_runtime::{NetInterface as SecIf, SharedSecurityGate};

        const GOV: &str = r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#;
        let logger = std::sync::Arc::new(CapturingLogger::default());
        let gate = SharedSecurityGate::new(
            0,
            parse_governance_xml(GOV).unwrap(),
            Box::new(AesGcmCryptoPlugin::new()),
        );
        let logger_dyn: std::sync::Arc<dyn zerodds_security_runtime::LoggingPlugin> =
            std::sync::Arc::clone(&logger) as _;
        let bindings = vec![InterfaceBindingSpec {
            name: "lo".into(),
            bind_addr: Ipv4Addr::new(127, 0, 0, 1),
            bind_port: 0,
            kind: SecIf::Loopback,
            subnet: zerodds_security_runtime::IpRange {
                base: core::net::IpAddr::V4(core::net::Ipv4Addr::new(127, 0, 0, 0)),
                prefix_len: 8,
            },
            default: true,
        }];
        let cfg = RuntimeConfig {
            security: Some(std::sync::Arc::new(gate)),
            security_logger: Some(logger_dyn),
            interface_bindings: bindings,
            ..RuntimeConfig::default()
        };
        let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([0xF1; 12]), cfg).expect("rt");

        // Read the port of the pool binding (ephemeral).
        let pool_port = rt.outbound_pool.as_ref().unwrap().bindings[0]
            .socket
            .local_locator()
            .port as u16;
        assert!(pool_port > 0);

        // An external socket sends a plain packet to the pool socket.
        let sender = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0)).unwrap();
        let mut plain = Vec::new();
        plain.extend_from_slice(b"RTPS\x02\x05\x01\x02");
        plain.extend_from_slice(&[0xAB; 12]);
        plain.extend_from_slice(b"loopback-dispatch");
        sender
            .send_to(
                &plain,
                SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), pool_port),
            )
            .unwrap();

        // The event loop needs a few ticks to poll the packet.
        // The default tick_period is 50 ms; we wait a few of them.
        std::thread::sleep(Duration::from_millis(300));

        // The pool packet, through classify_inbound with iface=Loopback,
        // ran → accept, no log events from this path.
        let pool_events = logger.events();

        // Comparison test: the same packet through secure_inbound_bytes
        // with iface=Wan → error event with an iface=Wan marker.
        let _ = secure_inbound_bytes(&rt, &plain, &SecIf::Wan);
        let after = logger.events();
        assert!(
            after.len() > pool_events.len(),
            "the Wan path must produce a new log event"
        );
        let new_ev = &after[after.len() - 1];
        assert_eq!(new_ev.0, zerodds_security_runtime::LogLevel::Error);
        assert!(
            new_ev.2.contains("iface=Wan"),
            "log message carries the iface marker: got={:?}",
            new_ev.2
        );

        // Log events from the pool path must NOT carry the error level
        // (because classify_inbound returns accept on loopback).
        for (lvl, cat, msg) in &pool_events {
            assert_ne!(
                *lvl,
                zerodds_security_runtime::LogLevel::Error,
                "the loopback path must not produce an error event: cat={cat} msg={msg}"
            );
        }
        rt.shutdown();
    }

    #[cfg(feature = "security")]
    #[test]
    fn per_target_without_security_gate_is_passthrough() {
        // Without a `security` config in RuntimeConfig, the per-target
        // path is a pure passthrough. Important so that we do not
        // break the v1.4 backward compat.
        let rt = DcpsRuntime::start(
            0,
            GuidPrefix::from_bytes([0xE5; 12]),
            RuntimeConfig::default(),
        )
        .expect("rt");
        let wid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: "T".into(),
                type_name: "zerodds::RawBytes".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .unwrap();
        let tgt = Locator::udp_v4([127, 0, 0, 1], 40000);
        let msg = b"raw-plaintext".to_vec();
        let out = secure_outbound_for_target(&rt, wid, &msg, &tgt).unwrap();
        assert_eq!(out, msg, "without a gate it must be passthrough");
        rt.shutdown();
    }

    // ----  Builtin-Topic-Reader Discovery-Hook (DDS 1.4 §2.2.5) ----

    /// Helper: constructs a synthetic SPDP beacon
    /// for a remote participant, so that `handle_spdp_datagram`
    /// accepts it.
    fn make_remote_spdp_beacon(remote_prefix: GuidPrefix) -> Vec<u8> {
        use zerodds_discovery::spdp::SpdpBeacon;
        use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
        use zerodds_rtps::wire_types::{ProtocolVersion, VendorId};
        let data = ParticipantBuiltinTopicData {
            guid: Guid::new(remote_prefix, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: None,
            default_multicast_locator: None,
            metatraffic_unicast_locator: None,
            metatraffic_multicast_locator: None,
            domain_id: Some(0),
            builtin_endpoint_set: 0,
            lease_duration: QosDuration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            participant_security_info: None,
        };
        let mut beacon = SpdpBeacon::new(data);
        beacon.serialize().expect("serialize")
    }

    #[test]
    fn handle_spdp_datagram_pushes_into_builtin_participant_reader() {
        let rt = DcpsRuntime::start(
            41,
            GuidPrefix::from_bytes([0x21; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());

        let remote = GuidPrefix::from_bytes([0x99; 12]);
        let dg = make_remote_spdp_beacon(remote);
        // A direct hook call simulates an SPDP receive without multicast.
        handle_spdp_datagram(&rt, &dg);

        let reader = bs
            .lookup_datareader::<crate::builtin_topics::ParticipantBuiltinTopicData>(
                "DCPSParticipant",
            )
            .unwrap();
        let samples = reader.take().unwrap();
        assert_eq!(samples.len(), 1, "exactly 1 sample for 1 SPDP beacon");
        assert_eq!(samples[0].key.prefix, remote);
        rt.shutdown();
    }

    #[test]
    fn handle_spdp_datagram_skips_self_beacon() {
        let prefix = GuidPrefix::from_bytes([0x22; 12]);
        let rt = DcpsRuntime::start(42, prefix, RuntimeConfig::default()).expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());

        // Beacon from our own prefix → must be ignored (Spec
        // §8.5.4 self-discovery filter).
        let dg = make_remote_spdp_beacon(prefix);
        handle_spdp_datagram(&rt, &dg);

        let reader = bs
            .lookup_datareader::<crate::builtin_topics::ParticipantBuiltinTopicData>(
                "DCPSParticipant",
            )
            .unwrap();
        let samples = reader.take().unwrap();
        assert!(samples.is_empty(), "own beacon must not be logged");
        rt.shutdown();
    }

    #[test]
    fn sedp_event_push_populates_publication_and_topic_readers() {
        use crate::builtin_topics as bt;
        use zerodds_discovery::sedp::SedpEvents;
        use zerodds_qos::{LivelinessQosPolicy, ReliabilityQosPolicy};
        let rt = DcpsRuntime::start(
            43,
            GuidPrefix::from_bytes([0x23; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());

        let mut events = SedpEvents::default();
        events.new_publications.push(
            zerodds_rtps::publication_data::PublicationBuiltinTopicData {
                key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
                participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
                topic_name: "WireT".into(),
                type_name: "WireType".into(),
                durability: zerodds_qos::DurabilityKind::Volatile,
                reliability: ReliabilityQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                liveliness: LivelinessQosPolicy::default(),
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_information: None,
                data_representation: Vec::new(),
                security_info: None,
                service_instance_name: None,
                related_entity_guid: None,
                topic_aliases: None,
                type_identifier: zerodds_types::TypeIdentifier::None,
                unicast_locators: Vec::new(),
                multicast_locators: Vec::new(),
            },
        );

        push_sedp_events_to_builtin_readers(&rt, &events);

        let pub_reader = bs
            .lookup_datareader::<bt::PublicationBuiltinTopicData>("DCPSPublication")
            .unwrap();
        let pub_samples = pub_reader.take().unwrap();
        assert_eq!(pub_samples.len(), 1);
        assert_eq!(pub_samples[0].topic_name, "WireT");

        let topic_reader = bs
            .lookup_datareader::<bt::TopicBuiltinTopicData>("DCPSTopic")
            .unwrap();
        let topic_samples = topic_reader.take().unwrap();
        assert_eq!(topic_samples.len(), 1);
        assert_eq!(topic_samples[0].name, "WireT");
        rt.shutdown();
    }

    #[test]
    fn sedp_event_push_populates_subscription_reader() {
        use crate::builtin_topics as bt;
        use zerodds_discovery::sedp::SedpEvents;
        use zerodds_qos::{LivelinessQosPolicy, ReliabilityQosPolicy};
        let rt = DcpsRuntime::start(
            44,
            GuidPrefix::from_bytes([0x24; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());

        let mut events = SedpEvents::default();
        events.new_subscriptions.push(
            zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData {
                key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
                participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
                topic_name: "SubT".into(),
                type_name: "SubType".into(),
                durability: zerodds_qos::DurabilityKind::Volatile,
                reliability: ReliabilityQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                liveliness: LivelinessQosPolicy::default(),
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_information: None,
                data_representation: Vec::new(),
                content_filter: None,
                security_info: None,
                service_instance_name: None,
                related_entity_guid: None,
                topic_aliases: None,
                type_identifier: zerodds_types::TypeIdentifier::None,
                unicast_locators: Vec::new(),
                multicast_locators: Vec::new(),
            },
        );

        push_sedp_events_to_builtin_readers(&rt, &events);

        let sub_reader = bs
            .lookup_datareader::<bt::SubscriptionBuiltinTopicData>("DCPSSubscription")
            .unwrap();
        let sub_samples = sub_reader.take().unwrap();
        assert_eq!(sub_samples.len(), 1);
        assert_eq!(sub_samples[0].topic_name, "SubT");

        // The topic reader gets a synthetic topic sample also from
        // Subscription.
        let topic_reader = bs
            .lookup_datareader::<bt::TopicBuiltinTopicData>("DCPSTopic")
            .unwrap();
        let topic_samples = topic_reader.take().unwrap();
        assert_eq!(topic_samples.len(), 1);
        assert_eq!(topic_samples[0].name, "SubT");
        rt.shutdown();
    }

    #[test]
    fn push_sedp_events_to_builtin_readers_is_noop_without_sinks() {
        use zerodds_discovery::sedp::SedpEvents;
        let rt = DcpsRuntime::start(
            45,
            GuidPrefix::from_bytes([0x25; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        // No attach_builtin_sinks → push must stay silent, not
        // panic.
        let events = SedpEvents::default();
        push_sedp_events_to_builtin_readers(&rt, &events);
        rt.shutdown();
    }

    // ----  Ignore-Filter im Discovery-Hot-Path -------------

    #[test]
    fn handle_spdp_datagram_drops_ignored_participant_beacon() {
        // Spec §2.2.2.2.1.14: ein einmal ignorierter Participant
        // taucht in keinem nachfolgenden Builtin-Sample mehr auf.
        let rt = DcpsRuntime::start(
            46,
            GuidPrefix::from_bytes([0x26; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());
        let filter = crate::participant::IgnoreFilter::default();
        rt.attach_ignore_filter(filter.clone());

        let remote = GuidPrefix::from_bytes([0xAA; 12]);
        // Derive the ignore handle from the future beacon — we
        // know that the builtin sample key is the GUID of the remote
        // participant (=prefix + EntityId::PARTICIPANT).
        let key = Guid::new(remote, EntityId::PARTICIPANT);
        let h = crate::instance_handle::InstanceHandle::from_guid(key);
        if let Ok(mut s) = filter.inner.participants.lock() {
            s.insert(h);
        }
        let dg = make_remote_spdp_beacon(remote);
        handle_spdp_datagram(&rt, &dg);

        let reader = bs
            .lookup_datareader::<crate::builtin_topics::ParticipantBuiltinTopicData>(
                "DCPSParticipant",
            )
            .unwrap();
        assert!(
            reader.take().unwrap().is_empty(),
            "an ignored participant must not land in DCPSParticipant"
        );
        rt.shutdown();
    }

    #[test]
    fn sedp_event_push_filters_ignored_publication() {
        use crate::builtin_topics as bt;
        use zerodds_discovery::sedp::SedpEvents;
        use zerodds_qos::{LivelinessQosPolicy, ReliabilityQosPolicy};
        let rt = DcpsRuntime::start(
            47,
            GuidPrefix::from_bytes([0x27; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());
        let filter = crate::participant::IgnoreFilter::default();
        rt.attach_ignore_filter(filter.clone());

        let pub_key = Guid::new(GuidPrefix::from_bytes([0x33; 12]), EntityId::PARTICIPANT);
        let h_pub = crate::instance_handle::InstanceHandle::from_guid(pub_key);
        if let Ok(mut s) = filter.inner.publications.lock() {
            s.insert(h_pub);
        }

        let mut events = SedpEvents::default();
        events.new_publications.push(
            zerodds_rtps::publication_data::PublicationBuiltinTopicData {
                key: pub_key,
                participant_key: Guid::new(
                    GuidPrefix::from_bytes([0x33; 12]),
                    EntityId::PARTICIPANT,
                ),
                topic_name: "Filtered".into(),
                type_name: "T".into(),
                durability: zerodds_qos::DurabilityKind::Volatile,
                reliability: ReliabilityQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                liveliness: LivelinessQosPolicy::default(),
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_information: None,
                data_representation: Vec::new(),
                security_info: None,
                service_instance_name: None,
                related_entity_guid: None,
                topic_aliases: None,
                type_identifier: zerodds_types::TypeIdentifier::None,
                unicast_locators: Vec::new(),
                multicast_locators: Vec::new(),
            },
        );

        push_sedp_events_to_builtin_readers(&rt, &events);

        let pub_reader = bs
            .lookup_datareader::<bt::PublicationBuiltinTopicData>("DCPSPublication")
            .unwrap();
        assert!(
            pub_reader.take().unwrap().is_empty(),
            "an ignored publication must not land in DCPSPublication"
        );
        // The synthetic DCPSTopic sample too must not be
        // forwarded, because the publication is completely
        // discarded.
        let topic_reader = bs
            .lookup_datareader::<bt::TopicBuiltinTopicData>("DCPSTopic")
            .unwrap();
        assert!(topic_reader.take().unwrap().is_empty());
        rt.shutdown();
    }

    #[test]
    fn sedp_event_push_filters_ignored_subscription() {
        use crate::builtin_topics as bt;
        use zerodds_discovery::sedp::SedpEvents;
        use zerodds_qos::{LivelinessQosPolicy, ReliabilityQosPolicy};
        let rt = DcpsRuntime::start(
            48,
            GuidPrefix::from_bytes([0x28; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());
        let filter = crate::participant::IgnoreFilter::default();
        rt.attach_ignore_filter(filter.clone());

        let sub_key = Guid::new(GuidPrefix::from_bytes([0x44; 12]), EntityId::PARTICIPANT);
        let h_sub = crate::instance_handle::InstanceHandle::from_guid(sub_key);
        if let Ok(mut s) = filter.inner.subscriptions.lock() {
            s.insert(h_sub);
        }

        let mut events = SedpEvents::default();
        events.new_subscriptions.push(
            zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData {
                key: sub_key,
                participant_key: Guid::new(
                    GuidPrefix::from_bytes([0x44; 12]),
                    EntityId::PARTICIPANT,
                ),
                topic_name: "FilteredSub".into(),
                type_name: "T".into(),
                durability: zerodds_qos::DurabilityKind::Volatile,
                reliability: ReliabilityQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                liveliness: LivelinessQosPolicy::default(),
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_information: None,
                data_representation: Vec::new(),
                content_filter: None,
                security_info: None,
                service_instance_name: None,
                related_entity_guid: None,
                topic_aliases: None,
                type_identifier: zerodds_types::TypeIdentifier::None,
                unicast_locators: Vec::new(),
                multicast_locators: Vec::new(),
            },
        );

        push_sedp_events_to_builtin_readers(&rt, &events);

        let sub_reader = bs
            .lookup_datareader::<bt::SubscriptionBuiltinTopicData>("DCPSSubscription")
            .unwrap();
        assert!(sub_reader.take().unwrap().is_empty());
        rt.shutdown();
    }

    #[test]
    fn sedp_event_push_filters_ignored_topic_only() {
        // If only the topic is ignored, DCPSPublication should
        // still be pushed — only the DCPSTopic sample falls
        // away.
        use crate::builtin_topics as bt;
        use zerodds_discovery::sedp::SedpEvents;
        use zerodds_qos::{LivelinessQosPolicy, ReliabilityQosPolicy};
        let rt = DcpsRuntime::start(
            49,
            GuidPrefix::from_bytes([0x29; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let bs = crate::builtin_subscriber::BuiltinSubscriber::new();
        rt.attach_builtin_sinks(bs.sinks());
        let filter = crate::participant::IgnoreFilter::default();
        rt.attach_ignore_filter(filter.clone());

        let topic_key =
            crate::builtin_topics::TopicBuiltinTopicData::synthesize_key("OnlyTopic", "T");
        let h_topic = crate::instance_handle::InstanceHandle::from_guid(topic_key);
        if let Ok(mut s) = filter.inner.topics.lock() {
            s.insert(h_topic);
        }

        let mut events = SedpEvents::default();
        events.new_publications.push(
            zerodds_rtps::publication_data::PublicationBuiltinTopicData {
                key: Guid::new(GuidPrefix::from_bytes([0x55; 12]), EntityId::PARTICIPANT),
                participant_key: Guid::new(
                    GuidPrefix::from_bytes([0x55; 12]),
                    EntityId::PARTICIPANT,
                ),
                topic_name: "OnlyTopic".into(),
                type_name: "T".into(),
                durability: zerodds_qos::DurabilityKind::Volatile,
                reliability: ReliabilityQosPolicy::default(),
                ownership: zerodds_qos::OwnershipKind::Shared,
                ownership_strength: 0,
                liveliness: LivelinessQosPolicy::default(),
                deadline: zerodds_qos::DeadlineQosPolicy::default(),
                lifespan: zerodds_qos::LifespanQosPolicy::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_information: None,
                data_representation: Vec::new(),
                security_info: None,
                service_instance_name: None,
                related_entity_guid: None,
                topic_aliases: None,
                type_identifier: zerodds_types::TypeIdentifier::None,
                unicast_locators: Vec::new(),
                multicast_locators: Vec::new(),
            },
        );

        push_sedp_events_to_builtin_readers(&rt, &events);

        let pub_reader = bs
            .lookup_datareader::<bt::PublicationBuiltinTopicData>("DCPSPublication")
            .unwrap();
        assert_eq!(pub_reader.take().unwrap().len(), 1);
        let topic_reader = bs
            .lookup_datareader::<bt::TopicBuiltinTopicData>("DCPSTopic")
            .unwrap();
        assert!(
            topic_reader.take().unwrap().is_empty(),
            "an ignored topic may block the synthetic DCPSTopic sample"
        );
        rt.shutdown();
    }

    // -------- Security-Builtin-Endpoint-Wiring --------

    /// Creates an SPDP beacon with configurable BuiltinEndpoint
    /// bits. Extension of [`make_remote_spdp_beacon`] with
    /// flag-Argument (Security-Bits 22..25).
    fn make_remote_spdp_beacon_with_flags(remote_prefix: GuidPrefix, endpoint_set: u32) -> Vec<u8> {
        use zerodds_discovery::spdp::SpdpBeacon;
        use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
        use zerodds_rtps::wire_types::{ProtocolVersion, VendorId};
        let data = ParticipantBuiltinTopicData {
            guid: Guid::new(remote_prefix, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7500)),
            default_multicast_locator: None,
            metatraffic_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7501)),
            metatraffic_multicast_locator: None,
            domain_id: Some(0),
            builtin_endpoint_set: endpoint_set,
            lease_duration: QosDuration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            participant_security_info: None,
        };
        let mut beacon = SpdpBeacon::new(data);
        beacon.serialize().expect("serialize")
    }

    fn dp_with_locators(
        prefix: GuidPrefix,
        metatraffic: Option<Locator>,
        default: Option<Locator>,
    ) -> zerodds_discovery::spdp::DiscoveredParticipant {
        use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
        use zerodds_rtps::wire_types::{ProtocolVersion, VendorId};
        zerodds_discovery::spdp::DiscoveredParticipant {
            sender_prefix: prefix,
            sender_vendor: VendorId::ZERODDS,
            data: ParticipantBuiltinTopicData {
                guid: Guid::new(prefix, EntityId::PARTICIPANT),
                protocol_version: ProtocolVersion::V2_5,
                vendor_id: VendorId::ZERODDS,
                default_unicast_locator: default,
                default_multicast_locator: None,
                metatraffic_unicast_locator: metatraffic,
                metatraffic_multicast_locator: None,
                domain_id: Some(0),
                builtin_endpoint_set: 0,
                lease_duration: QosDuration::from_secs(100),
                user_data: alloc::vec::Vec::new(),
                properties: Default::default(),
                identity_token: None,
                permissions_token: None,
                identity_status_token: None,
                sig_algo_info: None,
                kx_algo_info: None,
                sym_cipher_algo_info: None,
                participant_security_info: None,
            },
        }
    }

    #[test]
    fn wlp_unicast_targets_prefers_metatraffic_then_default() {
        // M-2: WLP-Unicast-Fan-out waehlt pro Peer metatraffic_unicast (bevorzugt),
        // otherwise default_unicast; peers without a routable locator fall out.
        let meta = Locator::udp_v4([127, 0, 0, 1], 7501);
        let deflt = Locator::udp_v4([127, 0, 0, 2], 7500);
        let peers = alloc::vec![
            // (a) has metatraffic → metatraffic wins
            dp_with_locators(GuidPrefix::from_bytes([1; 12]), Some(meta), Some(deflt)),
            // (b) only default → default
            dp_with_locators(GuidPrefix::from_bytes([2; 12]), None, Some(deflt)),
            // (c) none at all → no target
            dp_with_locators(GuidPrefix::from_bytes([3; 12]), None, None),
        ];
        let targets = wlp_unicast_targets(&peers);
        assert_eq!(targets, alloc::vec![meta, deflt]);
    }

    /// Like [`make_remote_spdp_beacon_with_flags`], but with a set
    /// `identity_token` (FU2 Gap 7d — triggers the auth handshake).
    #[cfg(feature = "security")]
    fn make_secure_beacon_with_identity_token(
        remote_prefix: GuidPrefix,
        endpoint_set: u32,
        identity_token: Vec<u8>,
    ) -> Vec<u8> {
        use zerodds_discovery::spdp::SpdpBeacon;
        use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
        use zerodds_rtps::wire_types::{ProtocolVersion, VendorId};
        let data = ParticipantBuiltinTopicData {
            guid: Guid::new(remote_prefix, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7500)),
            default_multicast_locator: None,
            metatraffic_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 99], 7501)),
            metatraffic_multicast_locator: None,
            domain_id: Some(0),
            builtin_endpoint_set: endpoint_set,
            lease_duration: QosDuration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: Some(identity_token),
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            participant_security_info: None,
        };
        let mut beacon = SpdpBeacon::new(data);
        beacon.serialize().expect("serialize")
    }

    /// Minimal auth plugin for the FU2 wiring tests (Gap 4/7).
    /// Crypto correctness is verified in the stack.rs driver test; here
    /// it is only about the runtime wiring path.
    #[cfg(feature = "security")]
    struct FakeAuth;
    #[cfg(feature = "security")]
    impl zerodds_security::authentication::AuthenticationPlugin for FakeAuth {
        fn validate_local_identity(
            &mut self,
            _: &zerodds_security::properties::PropertyList,
            _: [u8; 16],
        ) -> zerodds_security::error::SecurityResult<zerodds_security::authentication::IdentityHandle>
        {
            Ok(zerodds_security::authentication::IdentityHandle(1))
        }
        fn validate_remote_identity(
            &mut self,
            _: zerodds_security::authentication::IdentityHandle,
            _: [u8; 16],
            _: &[u8],
        ) -> zerodds_security::error::SecurityResult<zerodds_security::authentication::IdentityHandle>
        {
            Ok(zerodds_security::authentication::IdentityHandle(2))
        }
        fn begin_handshake_request(
            &mut self,
            _: zerodds_security::authentication::IdentityHandle,
            _: zerodds_security::authentication::IdentityHandle,
        ) -> zerodds_security::error::SecurityResult<(
            zerodds_security::authentication::HandshakeHandle,
            zerodds_security::authentication::HandshakeStepOutcome,
        )> {
            Ok((
                zerodds_security::authentication::HandshakeHandle(1),
                zerodds_security::authentication::HandshakeStepOutcome::SendMessage {
                    token: zerodds_security::token::DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")
                        .to_cdr_le(),
                },
            ))
        }
        fn begin_handshake_reply(
            &mut self,
            _: zerodds_security::authentication::IdentityHandle,
            _: zerodds_security::authentication::IdentityHandle,
            _: &[u8],
        ) -> zerodds_security::error::SecurityResult<(
            zerodds_security::authentication::HandshakeHandle,
            zerodds_security::authentication::HandshakeStepOutcome,
        )> {
            Ok((
                zerodds_security::authentication::HandshakeHandle(2),
                zerodds_security::authentication::HandshakeStepOutcome::WaitingForPeer,
            ))
        }
        fn process_handshake(
            &mut self,
            _: zerodds_security::authentication::HandshakeHandle,
            _: &[u8],
        ) -> zerodds_security::error::SecurityResult<
            zerodds_security::authentication::HandshakeStepOutcome,
        > {
            Ok(zerodds_security::authentication::HandshakeStepOutcome::WaitingForPeer)
        }
        fn shared_secret(
            &self,
            _: zerodds_security::authentication::HandshakeHandle,
        ) -> zerodds_security::error::SecurityResult<
            zerodds_security::authentication::SharedSecretHandle,
        > {
            Err(zerodds_security::error::SecurityError::new(
                zerodds_security::error::SecurityErrorKind::BadArgument,
                "fake: handshake not complete",
            ))
        }
        fn plugin_class_id(&self) -> &str {
            "FAKE:Auth:1.0"
        }
        fn get_identity_token(
            &self,
            _: zerodds_security::authentication::IdentityHandle,
        ) -> zerodds_security::error::SecurityResult<Vec<u8>> {
            // Non-empty Token (Format irrelevant — FakeAuth.validate_remote_
            // identity accepts everything); only so the beacon-populate path
            // (Gap 7c) has something to announce.
            Ok(alloc::vec![0xAB, 0xCD, 0xEF, 0x01])
        }
        fn get_permissions_token(&self) -> Vec<u8> {
            // Non-empty PermissionsToken, so the beacon-populate path
            // (S4 point 1) has something to announce (format irrelevant).
            zerodds_security::token::DataHolder::new("DDS:Access:Permissions:1.0").to_cdr_le()
        }
    }

    /// Consolidated test for the wiring. A single
    /// runtime walks all paths — snapshot API, idempotency of
    /// `enable_security_builtins`, SPDP hot path with security bits,
    /// without bits, plus the wire-demux hook. We bundle this into one
    /// test body, because each `DcpsRuntime::start` binds a multicast socket
    /// and parallel tests could brush against the OS resource caps.
    #[test]
    fn c34c_security_builtin_wiring_end_to_end() {
        use zerodds_discovery::security::SecurityBuiltinStack;
        use zerodds_security::generic_message::{
            MessageIdentity, ParticipantGenericMessage, class_id,
        };
        use zerodds_security::token::DataHolder;

        let local_prefix = GuidPrefix::from_bytes([0x75; 12]);
        let rt = DcpsRuntime::start(75, local_prefix, RuntimeConfig::default()).expect("start");

        // 1. Snapshot is None before enable
        assert!(rt.security_builtin_snapshot().is_none());

        // 2. enable ist idempotent
        let h1 = rt.enable_security_builtins(VendorId::ZERODDS);
        let h2 = rt.enable_security_builtins(VendorId::ZERODDS);
        assert!(Arc::ptr_eq(&h1, &h2));
        assert!(rt.security_builtin_snapshot().is_some());

        // 3. SPDP beacon with all security-builtin bits → the stack has
        //    four proxies
        let remote_a = GuidPrefix::from_bytes([0x99; 12]);
        let flags_all = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        handle_spdp_datagram(
            &rt,
            &make_remote_spdp_beacon_with_flags(remote_a, flags_all),
        );
        {
            let s = h1.lock().unwrap();
            assert_eq!(s.stateless_writer.reader_proxy_count(), 1);
            assert_eq!(s.stateless_reader.writer_proxy_count(), 1);
            assert_eq!(s.volatile_writer.reader_proxy_count(), 1);
            assert_eq!(s.volatile_reader.writer_proxy_count(), 1);
        }

        // 4. SPDP beacon without security bits → the stack stays unchanged
        let remote_b = GuidPrefix::from_bytes([0x88; 12]);
        handle_spdp_datagram(
            &rt,
            &make_remote_spdp_beacon_with_flags(remote_b, endpoint_flag::ALL_STANDARD),
        );
        {
            let s = h1.lock().unwrap();
            assert_eq!(
                s.stateless_writer.reader_proxy_count(),
                1,
                "a peer without security bits must not touch existing proxies"
            );
        }

        // 5. Wire-demux hook with a valid stateless DATA: remote-stack
        //    mirror sends a message → the demux hook routes it through
        //    the local reader without panic.
        let mut remote_stack = SecurityBuiltinStack::new(remote_a, VendorId::ZERODDS);
        let local_peer = make_remote_spdp_beacon_with_flags(local_prefix, flags_all);
        let parsed_local = zerodds_discovery::spdp::SpdpReader::new()
            .parse_datagram(&local_peer)
            .unwrap();
        remote_stack.handle_remote_endpoints(&parsed_local);
        let msg = ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: [0xCD; 16],
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: [0xEF; 16],
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0xFE; 16],
            message_class_id: class_id::AUTH_REQUEST.into(),
            message_data: alloc::vec![DataHolder::new("DDS:Auth:PKI-DH:1.2+AuthReq")],
        };
        let dgs = remote_stack.stateless_writer.write(&msg).unwrap();
        assert_eq!(dgs.len(), 1);
        dispatch_security_builtin_datagram(&rt, &dgs[0].bytes, Duration::from_secs(1));

        // 6. The demux hook does not panic on garbage bytes
        dispatch_security_builtin_datagram(&rt, &[0u8; 32], Duration::from_secs(1));

        rt.shutdown();
    }

    /// FU2 Gap 4: `enable_security_builtins_with_auth` builds the stack with
    /// an active handshake driver — `begin_handshake_with` sends, as
    /// the initiator actually sends an AUTH_REQUEST (instead of a no-op like with
    /// the auth-less `enable_security_builtins`).
    #[cfg(feature = "security")]
    #[test]
    fn enable_security_builtins_with_auth_activates_handshake_driver() {
        use zerodds_security::authentication::{AuthenticationPlugin, IdentityHandle};

        let local_prefix = GuidPrefix::from_bytes([0x40; 12]);
        let rt = DcpsRuntime::start(40, local_prefix, RuntimeConfig::default()).expect("start");

        let auth: Arc<Mutex<dyn AuthenticationPlugin>> = Arc::new(Mutex::new(FakeAuth));
        let stack =
            rt.enable_security_builtins_with_auth(VendorId::ZERODDS, auth, IdentityHandle(1));

        // Discover a peer with stateless bits (WITHOUT identity_token → the
        // discovery trigger starts no handshake yet) → proxies
        // are wired. The remote prefix is LARGER than local ([0x40]),
        // so that local is the initiator under the cyclone convention (smaller GUID
        // initiates) and actually sends.
        let remote = GuidPrefix::from_bytes([0x99; 12]);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        handle_spdp_datagram(&rt, &make_remote_spdp_beacon_with_flags(remote, flags));

        let dgs = {
            let mut s = stack.lock().unwrap();
            let remote_guid = Guid::new(remote, EntityId::PARTICIPANT).to_bytes();
            s.begin_handshake_with(remote, remote_guid, b"fake-remote-cert-der")
                .expect("begin_handshake_with")
        };
        assert_eq!(
            dgs.len(),
            1,
            "auth driver active → the initiator sends exactly one AUTH_REQUEST"
        );

        rt.shutdown();
    }

    /// FU2 Gap 7c/d: `enable_security_builtins_with_auth` announces the
    /// local `identity_token` in the SPDP beacon (+ stateless/volatile bits),
    /// and an incoming peer beacon WITH an `identity_token` kicks off the
    /// Auth-Handshake an (Discovery-Trigger).
    #[cfg(feature = "security")]
    #[test]
    fn spdp_beacon_announces_identity_token_and_discovery_triggers_handshake() {
        use zerodds_security::authentication::{AuthenticationPlugin, IdentityHandle};

        let local_prefix = GuidPrefix::from_bytes([0x41; 12]);
        let rt = DcpsRuntime::start(41, local_prefix, RuntimeConfig::default()).expect("start");
        let auth: Arc<Mutex<dyn AuthenticationPlugin>> = Arc::new(Mutex::new(FakeAuth));
        let stack =
            rt.enable_security_builtins_with_auth(VendorId::ZERODDS, auth, IdentityHandle(1));

        // Gap 7c: the beacon now announces identity_token + secure bits.
        let beacon_bytes = rt.spdp_beacon.lock().unwrap().serialize().unwrap();
        let parsed = zerodds_discovery::spdp::SpdpReader::new()
            .parse_datagram(&beacon_bytes)
            .unwrap();
        assert!(
            parsed.data.identity_token.is_some(),
            "the beacon must announce PID_IDENTITY_TOKEN"
        );
        // Cross-vendor: secure vendors validate a remote only when
        // SPDP carries **both** tokens. Without PID_PERMISSIONS_TOKEN they treat
        // cyclone treats us as non-secure and never starts validate_remote_identity.
        assert!(
            parsed.data.permissions_token.is_some(),
            "the beacon must announce PID_PERMISSIONS_TOKEN (cross-vendor mandatory)"
        );
        assert_ne!(
            parsed.data.builtin_endpoint_set & endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER,
            0,
            "the beacon must announce the stateless-auth bit"
        );

        // Gap 7d: peer beacon WITH identity_token + stateless bits → the
        // discovery path kicks off begin_handshake_with.
        let remote = GuidPrefix::from_bytes([0x99; 12]);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        let peer_beacon =
            make_secure_beacon_with_identity_token(remote, flags, alloc::vec![0x11, 0x22, 0x33]);
        handle_spdp_datagram(&rt, &peer_beacon);

        // Proof that the discovery trigger fired: the peer is now
        // registered in the stack's handshake state. (The earlier length
        // probe via a repeated begin_handshake_with no longer applies since the resend path
        // resends as the initiator on a repeated call.)
        let started = {
            let s = stack.lock().unwrap();
            s.handshake_peer_count()
        };
        assert_eq!(
            started, 1,
            "the discovery trigger must have started the handshake (peer registered)"
        );

        rt.shutdown();
    }

    /// FU2 S3: two secure runtimes in the same process MUST find each other via
    /// in-process participant discovery and kick off the auth handshake
    /// — WITHOUT a single multicast beacon. That was exactly missing:
    /// `inproc_inject_publication`/`_subscription` inject only SEDP, the
    /// SPDP participant discovery (identity_token + `begin_handshake_with`)
    /// ran exclusively over the flaky multicast path.
    #[cfg(feature = "security")]
    #[test]
    fn inproc_participant_discovery_triggers_handshake_without_multicast() {
        use zerodds_security::authentication::{AuthenticationPlugin, IdentityHandle};

        let a_prefix = GuidPrefix::from_bytes([0x4A; 12]);
        let b_prefix = GuidPrefix::from_bytes([0x4B; 12]);
        let rt_a = DcpsRuntime::start(47, a_prefix, RuntimeConfig::default()).expect("start a");
        let rt_b = DcpsRuntime::start(47, b_prefix, RuntimeConfig::default()).expect("start b");
        let auth_a: Arc<Mutex<dyn AuthenticationPlugin>> = Arc::new(Mutex::new(FakeAuth));
        let auth_b: Arc<Mutex<dyn AuthenticationPlugin>> = Arc::new(Mutex::new(FakeAuth));
        let stack_a =
            rt_a.enable_security_builtins_with_auth(VendorId::ZERODDS, auth_a, IdentityHandle(1));
        let stack_b =
            rt_b.enable_security_builtins_with_auth(VendorId::ZERODDS, auth_b, IdentityHandle(1));

        // KEIN handle_spdp_datagram / Multicast — rein in-process.
        let a_peers = stack_a.lock().unwrap().handshake_peer_count();
        let b_peers = stack_b.lock().unwrap().handshake_peer_count();
        assert!(
            a_peers >= 1,
            "A must have discovered B in-process + started the handshake (got {a_peers})"
        );
        assert!(
            b_peers >= 1,
            "B must have discovered A in-process + started the handshake (got {b_peers})"
        );

        rt_a.shutdown();
        rt_b.shutdown();
    }

    /// Mints a shared CA + two leaf identities (PEM) for the
    /// FU2-Handshake-e2e-Test.
    #[cfg(feature = "security")]
    #[allow(clippy::type_complexity)]
    fn mint_handshake_identities() -> ((Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)) {
        use rcgen::{CertificateParams, KeyPair};
        let mut ca_params =
            CertificateParams::new(alloc::vec![alloc::string::String::from("FU2 CA")]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca_pem = ca_cert.pem().into_bytes();
        let mint = |name: &str| -> (Vec<u8>, Vec<u8>) {
            let mut p =
                CertificateParams::new(alloc::vec![alloc::string::String::from(name)]).unwrap();
            p.is_ca = rcgen::IsCa::NoCa;
            let k = KeyPair::generate().unwrap();
            let c = p.signed_by(&k, &ca_cert, &ca_key).unwrap();
            (c.pem().into_bytes(), k.serialize_pem().into_bytes())
        };
        let alice = {
            let (cert, key) = mint("alice");
            (cert, key)
        };
        let bob = {
            let (cert, key) = mint("bob");
            (cert, key)
        };
        // attach ca_pem to both, so the caller has the trust anchor.
        (
            ([alice.0, b"\n".to_vec(), ca_pem.clone()].concat(), alice.1),
            ([bob.0, b"\n".to_vec(), ca_pem].concat(), bob.1),
        )
    }

    /// FU2 Gap 5 (e2e): a runtime replier (A) and an in-test initiator
    /// stack (B) complete a real PKI 3-round handshake via the dispatch path
    /// and BOTH derive the same SharedSecret.
    /// Verifies the dispatch wiring (`on_stateless_message` →
    /// reply/final → completion) in the real runtime context.
    #[cfg(feature = "security")]
    #[test]
    fn handshake_completes_through_runtime_dispatch_e2e() {
        use zerodds_discovery::security::SecurityBuiltinStack;
        use zerodds_security::authentication::AuthenticationPlugin;
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};

        // cert_pem here contains Leaf || CA (mint_handshake_identities),
        // identity_ca_pem = the same bundle (CA is included).
        let ((a_cert, a_key), (b_cert, b_key)) = mint_handshake_identities();

        // A = Runtime (Replier, HOEHERER Prefix). B = in-test Stack
        // (initiator, LOWER prefix) — cyclone convention: smaller
        // GUID initiiert.
        let a_prefix = GuidPrefix::from_bytes([0x20; 12]);
        let b_prefix = GuidPrefix::from_bytes([0x10; 12]);
        let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
        let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();

        // --- A: runtime with a real PKI plugin ---
        let a_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let a_local = a_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a_cert.clone(),
                    identity_ca_pem: a_cert.clone(),
                    identity_key_pem: Some(a_key),
                },
                a_guid,
            )
            .unwrap();
        let a_token = a_pki.lock().unwrap().get_identity_token(a_local).unwrap();
        let rt = DcpsRuntime::start(42, a_prefix, RuntimeConfig::default()).expect("start");
        let a_auth: Arc<Mutex<dyn AuthenticationPlugin>> = a_pki.clone();
        let a_stack = rt.enable_security_builtins_with_auth(VendorId::ZERODDS, a_auth, a_local);

        // --- B: in-test initiator stack with a real PKI plugin ---
        let b_pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let b_local = b_pki
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: b_cert.clone(),
                    identity_ca_pem: b_cert.clone(),
                    identity_key_pem: Some(b_key),
                },
                b_guid,
            )
            .unwrap();
        let b_token = b_pki.lock().unwrap().get_identity_token(b_local).unwrap();
        let b_auth: Arc<Mutex<dyn AuthenticationPlugin>> = b_pki.clone();
        let mut b_stack =
            SecurityBuiltinStack::with_auth(b_prefix, VendorId::ZERODDS, b_auth, b_local, b_guid);

        let stateless = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;

        // B discovers A (wired proxies) — via the parsed A beacon.
        let a_beacon = make_secure_beacon_with_identity_token(a_prefix, stateless, a_token.clone());
        let a_parsed = zerodds_discovery::spdp::SpdpReader::new()
            .parse_datagram(&a_beacon)
            .unwrap();
        b_stack.handle_remote_endpoints(&a_parsed);

        // A discovers B → the discovery trigger creates A's peer state (A is
        // the replier, sends nothing).
        let b_beacon = make_secure_beacon_with_identity_token(b_prefix, stateless, b_token);
        handle_spdp_datagram(&rt, &b_beacon);

        // B (initiator) starts → AUTH_REQUEST.
        let req = b_stack
            .begin_handshake_with(a_prefix, a_guid, &a_token)
            .unwrap();
        assert_eq!(req.len(), 1, "B sends AUTH_REQUEST");

        // Pump: REQUEST → A.dispatch → REPLY.
        let reply = dispatch_security_builtin_datagram(&rt, &req[0].bytes, Duration::from_secs(1));
        assert_eq!(reply.len(), 1, "A (replier) answers with AUTH reply");

        // REPLY → B verarbeitet → FINAL (+ B erreicht Complete).
        let b_msgs = b_stack
            .stateless_reader
            .handle_datagram(&reply[0].bytes)
            .unwrap();
        assert_eq!(b_msgs.len(), 1);
        let (final_dgs, _b_complete) = b_stack.on_stateless_message(a_prefix, &b_msgs[0]).unwrap();
        assert_eq!(final_dgs.len(), 1, "B sends AUTH-Final");

        // FINAL → A.dispatch → A erreicht Complete.
        let _ =
            dispatch_security_builtin_datagram(&rt, &final_dgs[0].bytes, Duration::from_secs(1));

        // Both sides must now have derived the same SharedSecret.
        let a_secret = {
            let s = a_stack.lock().unwrap();
            s.peer_secret(b_prefix)
                .expect("A must have authenticated B")
        };
        let b_secret = b_stack
            .peer_secret(a_prefix)
            .expect("B must have authenticated A");
        let a_bytes = a_pki
            .lock()
            .unwrap()
            .secret_bytes(a_secret)
            .unwrap()
            .to_vec();
        let b_bytes = b_pki
            .lock()
            .unwrap()
            .secret_bytes(b_secret)
            .unwrap()
            .to_vec();
        assert_eq!(a_bytes.len(), 32);
        assert_eq!(
            a_bytes, b_bytes,
            "runtime dispatch + in-test stack derive the same secret"
        );

        rt.shutdown();
    }

    /// FU2 S1.5 (e2e): after the auth handshake the runtime dispatch
    /// (A, replier) and a reference peer (B, stack+gate, initiator) over
    /// the Kx-protected VolatileSecure channel automatically exchange their data
    /// crypto tokens — afterwards secured user DATA round-trips in BOTH
    /// directions. **The secured-DATA proof via the runtime dispatch.**
    #[cfg(feature = "security")]
    #[test]
    #[serial_test::serial(dcps_security_e2e)]
    fn secured_data_round_trips_through_runtime_dispatch_e2e() {
        use zerodds_discovery::security::SecurityBuiltinStack;
        use zerodds_security::authentication::{AuthenticationPlugin, SharedSecretProvider};
        use zerodds_security::generic_message::{
            MessageIdentity, ParticipantGenericMessage, class_id,
        };
        use zerodds_security::token::DataHolder;
        use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
        use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};
        use zerodds_security_runtime::{ProtectionLevel, SharedSecurityGate};

        // Couples the pki plugin (behind a mutex) as the SharedSecretProvider to
        // the crypto plugin — like SecurityProfile in the FFI (Gap 1).
        struct PkiProvider(Arc<Mutex<PkiAuthenticationPlugin>>);
        impl SharedSecretProvider for PkiProvider {
            fn get_shared_secret(
                &self,
                h: zerodds_security::authentication::SharedSecretHandle,
            ) -> Option<Vec<u8>> {
                self.0.lock().ok()?.get_shared_secret(h)
            }
        }
        const GOV: &str = r#"<domain_access_rules><domain_rule><domains><id>0</id></domains><rtps_protection_kind>ENCRYPT</rtps_protection_kind><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules>"#;
        let gov = || zerodds_security_permissions::parse_governance_xml(GOV).unwrap();
        let gate_with = |pki: &Arc<Mutex<PkiAuthenticationPlugin>>| {
            SharedSecurityGate::new(
                0,
                gov(),
                Box::new(AesGcmCryptoPlugin::with_secret_provider(
                    Suite::Aes128Gcm,
                    Arc::new(PkiProvider(pki.clone())) as Arc<dyn SharedSecretProvider>,
                )),
            )
        };
        let fake_rtps = |prefix: GuidPrefix, body: &[u8]| -> Vec<u8> {
            let mut m = Vec::new();
            m.extend_from_slice(b"RTPS\x02\x05\x01\x02");
            m.extend_from_slice(&prefix.to_bytes());
            m.extend_from_slice(body);
            m
        };

        let ((a_cert, a_key), (b_cert, b_key)) = mint_handshake_identities();
        let a_prefix = GuidPrefix::from_bytes([0x20; 12]);
        let b_prefix = GuidPrefix::from_bytes([0x10; 12]); // B < A → B initiator (cyclone convention)
        let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
        let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();
        let a_key_pk = a_prefix.to_bytes();
        let b_key_pk = b_prefix.to_bytes();

        // --- A: runtime with auth + gate (sharing pki_a) ---
        let pki_a = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let a_local = pki_a
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: a_cert.clone(),
                    identity_ca_pem: a_cert.clone(),
                    identity_key_pem: Some(a_key),
                },
                a_guid,
            )
            .unwrap();
        let a_token = pki_a.lock().unwrap().get_identity_token(a_local).unwrap();
        let gate_a = Arc::new(gate_with(&pki_a));
        let rt = DcpsRuntime::start(
            43,
            a_prefix,
            RuntimeConfig {
                security: Some(gate_a.clone()),
                ..RuntimeConfig::default()
            },
        )
        .expect("start");
        let a_auth: Arc<Mutex<dyn AuthenticationPlugin>> = pki_a.clone();
        let a_stack = rt.enable_security_builtins_with_auth(VendorId::ZERODDS, a_auth, a_local);

        // --- B: in-test Stack + Gate (sharing pki_b), Initiator ---
        let pki_b = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
        let b_local = pki_b
            .lock()
            .unwrap()
            .validate_with_config(
                IdentityConfig {
                    identity_cert_pem: b_cert.clone(),
                    identity_ca_pem: b_cert.clone(),
                    identity_key_pem: Some(b_key),
                },
                b_guid,
            )
            .unwrap();
        let b_token = pki_b.lock().unwrap().get_identity_token(b_local).unwrap();
        let gate_b = gate_with(&pki_b);
        let b_auth: Arc<Mutex<dyn AuthenticationPlugin>> = pki_b.clone();
        let mut stack_b =
            SecurityBuiltinStack::with_auth(b_prefix, VendorId::ZERODDS, b_auth, b_local, b_guid);

        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
            | endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER;
        let a_beacon = make_secure_beacon_with_identity_token(a_prefix, flags, a_token.clone());
        stack_b.handle_remote_endpoints(
            &zerodds_discovery::spdp::SpdpReader::new()
                .parse_datagram(&a_beacon)
                .unwrap(),
        );
        // Wire A's stack deterministically (no handle_spdp_datagram —
        // a running runtime + trigger otherwise produces non-deterministic
        // proxy wirings via parallel/loopback beacons). A is the replier:
        // begin_handshake_with only sets up the peer state.
        let b_beacon = make_secure_beacon_with_identity_token(b_prefix, flags, b_token.clone());
        let b_parsed = zerodds_discovery::spdp::SpdpReader::new()
            .parse_datagram(&b_beacon)
            .unwrap();
        {
            let mut s = a_stack.lock().unwrap();
            s.handle_remote_endpoints(&b_parsed);
            s.begin_handshake_with(b_prefix, b_guid, &b_token).unwrap();
        }

        // --- Stateless-Handshake pumpen (B initiiert) ---
        // A is the replier and derives the secret already at begin_handshake_
        // reply → A's response to the request contains BOTH: the
        // AUTH reply (stateless) AND A's Kx-encrypted crypto token
        // (volatile, automatically via the dispatch).
        let decode_route = |dgs: &[zerodds_rtps::message_builder::OutboundDatagram]| {
            let mut stateless = Vec::new();
            let mut volatile = Vec::new();
            for dg in dgs {
                let parsed = zerodds_rtps::datagram::decode_datagram(&dg.bytes).unwrap();
                let is_vol = parsed.submessages.iter().any(|sub| {
                    // Klartext-Pfad (unprotected): DATA an den VolatileSecure-Reader.
                    matches!(sub, zerodds_rtps::datagram::ParsedSubmessage::Data(d)
                        if d.reader_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER)
                    // Cross-vendor path (protected): the volatile crypto-token DATA
                    // is SEC_*-protected (protect_volatile_outbound) -> the inner
                    // DATA is encrypted and recognizable only by the prepended SEC_PREFIX
                    // submessage (id 0x31). Stateless AUTH stays plaintext.
                    || matches!(sub, zerodds_rtps::datagram::ParsedSubmessage::Unknown { id: 0x31, .. })
                });
                if is_vol {
                    volatile.push(dg.bytes.clone());
                } else {
                    stateless.push(dg.bytes.clone());
                }
            }
            (stateless, volatile)
        };

        let req = stack_b
            .begin_handshake_with(a_prefix, a_guid, &a_token)
            .unwrap();
        let a_resp = dispatch_security_builtin_datagram(&rt, &req[0].bytes, Duration::from_secs(1));
        let (a_stateless, a_volatile) = decode_route(&a_resp);
        assert!(
            !a_volatile.is_empty(),
            "A dispatch must send A's crypto token"
        );

        // B verarbeitet A's AUTH-Reply → Final + B completes.
        let mut b_remote_id = None;
        let mut b_secret = None;
        let mut b_final = Vec::new();
        for sl in &a_stateless {
            for m in stack_b.stateless_reader.handle_datagram(sl).unwrap() {
                let (out, comp) = stack_b.on_stateless_message(a_prefix, &m).unwrap();
                b_final.extend(out);
                if let Some((id, sec)) = comp {
                    b_remote_id = Some(id);
                    b_secret = Some(sec);
                }
            }
        }
        let b_remote_id = b_remote_id.expect("B remote identity");
        let b_secret = b_secret.expect("B completes");

        // B registers A's Kx, installs A's crypto token (from a_volatile).
        gate_b
            .register_remote_by_guid_from_secret(a_key_pk, b_remote_id, b_secret)
            .unwrap();
        // A's volatile crypto token is cross-vendor SEC_*-protected
        // (protect_volatile_outbound). B must decrypt the SEC_PREFIX/BODY/POSTFIX sequence
        // with A's Kx key to the inner DATA submessage before the
        // volatile_reader can process it — mirrors unprotect_volatile_
        // datagram im Live-Dispatch.
        let unprotect_vol_b = |bytes: &[u8]| -> Option<Vec<u8>> {
            let subs = walk_submessages(bytes);
            let prefix_pos = subs.iter().position(|(id, _, _)| *id == SMID_SEC_PREFIX)?;
            let postfix_idx = subs[prefix_pos..]
                .iter()
                .position(|(id, _, _)| *id == SMID_SEC_POSTFIX)
                .map(|i| prefix_pos + i)?;
            let (_, p_start, _) = subs[prefix_pos];
            let (_, q_start, q_total) = subs[postfix_idx];
            let data_submsg = gate_b
                .decode_kx_datawriter_from(&a_key_pk, &bytes[p_start..q_start + q_total])
                .ok()?;
            let mut out = Vec::with_capacity(bytes.len());
            out.extend_from_slice(&bytes[..20]);
            for (i, &(_, start, total)) in subs.iter().enumerate() {
                if i < prefix_pos || i > postfix_idx {
                    out.extend_from_slice(&bytes[start..start + total]);
                } else if i == prefix_pos {
                    out.extend_from_slice(&data_submsg);
                }
            }
            Some(out)
        };
        let mut b_installed = 0;
        for vol in &a_volatile {
            let vol_plain = unprotect_vol_b(vol).unwrap_or_else(|| vol.clone());
            let parsed = zerodds_rtps::datagram::decode_datagram(&vol_plain).unwrap();
            let vol_src = parsed.header.guid_prefix;
            for sub in parsed.submessages {
                if let zerodds_rtps::datagram::ParsedSubmessage::Data(d) = sub {
                    if d.reader_id == EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER {
                        for m in stack_b.volatile_reader.handle_data(vol_src, &d).unwrap() {
                            if m.message_class_id == class_id::PARTICIPANT_CRYPTO_TOKENS {
                                // plaintext keymat (confidentiality was provided by the SEC_*
                                // protection of the volatile DATA, decrypted above) —
                                // install directly, no transform_kx_inbound.
                                let token = m.message_data[0]
                                    .binary_property(CRYPTO_TOKEN_PROP)
                                    .unwrap();
                                gate_b
                                    .set_remote_data_token_by_guid(&a_key_pk, token)
                                    .unwrap();
                                b_installed += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(b_installed >= 1, "B must install A's crypto token");

        // B builds + sends its crypto token — plaintext keymat in the
        // ParticipantGenericMessage (cross-vendor: confidentiality via SEC_*
        // protection of the transporting volatile DATA, not via token-internal
        // Kx encryption).
        let b_data_token = gate_b.local_token().unwrap();
        let b_crypto_msg = ParticipantGenericMessage {
            message_identity: MessageIdentity {
                source_guid: b_guid,
                sequence_number: 1,
            },
            related_message_identity: MessageIdentity::default(),
            destination_participant_key: a_guid,
            destination_endpoint_key: [0; 16],
            source_endpoint_key: [0; 16],
            message_class_id: class_id::PARTICIPANT_CRYPTO_TOKENS.into(),
            message_data: alloc::vec![
                DataHolder::new("DDS:Crypto:AES_GCM_GMAC")
                    .with_binary_property(CRYPTO_TOKEN_PROP, b_data_token)
            ],
        };
        let b_volatile = stack_b.volatile_writer.write(&b_crypto_msg).unwrap();
        // SEC_* submessage protection with A's Kx key (mirrors protect_volatile_
        // datagram in the live path): B encrypts the DATA submessage, A's
        // dispatch decrypts it via unprotect_volatile_datagram.
        let protect_vol_b = |bytes: &[u8]| -> Vec<u8> {
            let subs = walk_submessages(bytes);
            if !subs.iter().any(|(id, _, _)| *id == SMID_DATA) {
                return bytes.to_vec();
            }
            let mut out = Vec::with_capacity(bytes.len() + 64);
            out.extend_from_slice(&bytes[..20]);
            for (id, start, total) in subs {
                let submsg = &bytes[start..start + total];
                if id == SMID_DATA {
                    out.extend_from_slice(
                        &gate_b.encode_kx_datawriter_for(&a_key_pk, submsg).unwrap(),
                    );
                } else {
                    out.extend_from_slice(submsg);
                }
            }
            out
        };
        let b_vol_protected = protect_vol_b(&b_volatile[0].bytes);

        // B's Final + B's Crypto-Token an A's Dispatch: A installiert B's
        // Data token (automatically via install_crypto_token).
        for f in &b_final {
            dispatch_security_builtin_datagram(&rt, &f.bytes, Duration::from_secs(1));
        }
        dispatch_security_builtin_datagram(&rt, &b_vol_protected, Duration::from_secs(1));

        // --- Secured DATA in both directions ---
        let msg_ab = fake_rtps(a_prefix, b"[A->B secured payload]");
        let wire_ab = gate_a
            .transform_outbound_for(&b_key_pk, &msg_ab, ProtectionLevel::Encrypt)
            .unwrap();
        assert_eq!(
            gate_b.transform_inbound_from(&a_key_pk, &wire_ab).unwrap(),
            msg_ab,
            "A->B secured DATA must round-trip"
        );
        let msg_ba = fake_rtps(b_prefix, b"[B->A secured payload]");
        let wire_ba = gate_b
            .transform_outbound_for(&a_key_pk, &msg_ba, ProtectionLevel::Encrypt)
            .unwrap();
        assert_eq!(
            gate_a.transform_inbound_from(&b_key_pk, &wire_ba).unwrap(),
            msg_ba,
            "B->A secured DATA must round-trip (A's dispatch installed B's token)"
        );

        rt.shutdown();
    }

    #[test]
    fn c34c_enable_security_builtins_replays_known_peers() {
        // Order reversed: SPDP discovery first, plugin-
        // activation afterward. enable_security_builtins must catch up on already-
        // known peers. Plus: demux without a plugin (before enable)
        // is a no-op + does not panic.
        let rt = DcpsRuntime::start(
            76,
            GuidPrefix::from_bytes([0x76; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");

        // Demux without a plugin: silent no-op
        dispatch_security_builtin_datagram(&rt, &[0u8; 16], Duration::from_secs(1));

        let remote = GuidPrefix::from_bytes([0x77; 12]);
        let flags = endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER
            | endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER;
        let dg = make_remote_spdp_beacon_with_flags(remote, flags);
        handle_spdp_datagram(&rt, &dg);

        let stack = rt.enable_security_builtins(VendorId::ZERODDS);
        {
            let s = stack.lock().unwrap();
            assert_eq!(
                s.stateless_writer.reader_proxy_count(),
                1,
                "late plugin activation must catch up on known peers"
            );
        }

        rt.shutdown();
    }

    /// #29 regression: the earlier per-peer once-guard blocked late-matched
    /// user-endpoint tokens. `pending_endpoint_tokens` must, with already-sent
    /// builtin tokens, let through EXACTLY the new user token — not treat the whole
    /// peer as "done".
    #[cfg(feature = "security")]
    #[test]
    fn pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent() {
        use zerodds_security::generic_message::ParticipantGenericMessage;
        // An early-sent builtin token (secure-SEDP) ...
        let builtin = ParticipantGenericMessage {
            source_endpoint_key: [0xff; 16],
            destination_endpoint_key: [0xfe; 16],
            ..Default::default()
        };
        // ... and a late-matched user-endpoint token.
        let user = ParticipantGenericMessage {
            source_endpoint_key: [0x03; 16],
            destination_endpoint_key: [0x04; 16],
            ..Default::default()
        };
        let mut sent = alloc::collections::BTreeSet::new();
        sent.insert(endpoint_token_key(&builtin));

        let pending = pending_endpoint_tokens(vec![builtin.clone(), user.clone()], &sent);

        assert_eq!(pending.len(), 1, "only the new user token may be pending");
        assert_eq!(
            pending[0].source_endpoint_key, user.source_endpoint_key,
            "the let-through token must be the user-endpoint token"
        );
        // Idempotency: after sending, nothing is pending anymore.
        let mut sent2 = sent.clone();
        sent2.insert(endpoint_token_key(&user));
        assert!(
            pending_endpoint_tokens(vec![builtin, user], &sent2).is_empty(),
            "already-sent tokens must not become pending again"
        );
    }

    /// QT (#76, FOUNDATIONAL): a writer and reader of the SAME type whose
    /// TypeIdentifier is a *complete* hash (EquivalenceHashComplete) absent from
    /// any registry must still match and exchange data. Before the fix the
    /// runtime type-consistency check resolved against a fresh empty registry
    /// and rejected the match with TYPE_CONSISTENCY_ENFORCEMENT.
    #[test]
    fn qt_same_complete_type_identifier_matches_and_exchanges() {
        let rt = DcpsRuntime::start(
            60,
            GuidPrefix::from_bytes([0x60; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let complete = zerodds_types::TypeIdentifier::EquivalenceHashComplete(
            zerodds_types::type_identifier::EquivalenceHash([0xC0; 14]),
        );
        let mut w_cfg = qr_writer_cfg(
            "QtTopic",
            zerodds_qos::DurabilityKind::Volatile,
            alloc::vec![],
            zerodds_qos::LivelinessKind::Automatic,
        );
        w_cfg.type_identifier = complete.clone();
        let mut r_cfg = qr_reader_cfg(
            "QtTopic",
            zerodds_qos::DurabilityKind::Volatile,
            alloc::vec![],
            zerodds_qos::LivelinessKind::Automatic,
        );
        r_cfg.type_identifier = complete;
        let w = rt.register_user_writer(w_cfg).expect("writer");
        let (_r, rx) = rt.register_user_reader(r_cfg).expect("reader");
        rt.write_user_sample(w, b"complete-typed".to_vec())
            .expect("write");
        let s = rx
            .recv_timeout(core::time::Duration::from_millis(200))
            .expect("complete-TypeIdentifier writer+reader must match + exchange");
        match s {
            UserSample::Alive { payload, .. } => assert_eq!(payload.as_ref(), b"complete-typed"),
            other => panic!("expected Alive, got {other:?}"),
        }
        rt.shutdown();
    }

    // ===================================================================
    // QR-cluster (#77) — same-runtime QoS behavioral regression tests.
    // ===================================================================

    fn qr_writer_cfg(
        topic: &str,
        durability: zerodds_qos::DurabilityKind,
        partition: Vec<String>,
        liveliness: zerodds_qos::LivelinessKind,
    ) -> UserWriterConfig {
        UserWriterConfig {
            topic_name: topic.into(),
            type_name: "QrType".into(),
            reliable: true,
            durability,
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: liveliness,
                lease_duration: QosDuration::INFINITE,
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            partition,
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            data_representation_offer: None,
        }
    }

    fn qr_reader_cfg(
        topic: &str,
        durability: zerodds_qos::DurabilityKind,
        partition: Vec<String>,
        liveliness: zerodds_qos::LivelinessKind,
    ) -> UserReaderConfig {
        UserReaderConfig {
            topic_name: topic.into(),
            type_name: "QrType".into(),
            reliable: true,
            durability,
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: liveliness,
                lease_duration: QosDuration::INFINITE,
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            partition,
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: None,
        }
    }

    /// QR (a) HISTORY KeepLast: a TransientLocal writer with depth=2 retains
    /// only the last 2 samples per instance; a late-joining reader replays
    /// exactly those 2 (not all 3 written).
    #[test]
    fn qr_history_keep_last_depth_enforced_on_replay() {
        let rt = DcpsRuntime::start(
            61,
            GuidPrefix::from_bytes([0x61; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let w = rt
            .register_user_writer(qr_writer_cfg(
                "QrHistory",
                zerodds_qos::DurabilityKind::TransientLocal,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("writer");
        rt.set_user_writer_history_depth(w, 2).expect("set depth");

        // Three writes BEFORE any reader exists.
        rt.write_user_sample(w, b"s1".to_vec()).expect("w1");
        rt.write_user_sample(w, b"s2".to_vec()).expect("w2");
        rt.write_user_sample(w, b"s3".to_vec()).expect("w3");
        assert_eq!(
            rt.user_writer_retained_len(w),
            2,
            "KeepLast(2) must retain only the 2 most recent samples"
        );

        // Late-joining reader replays exactly the last 2 (s2, s3).
        let (_r, rx) = rt
            .register_user_reader(qr_reader_cfg(
                "QrHistory",
                zerodds_qos::DurabilityKind::TransientLocal,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("reader");

        let mut got: Vec<Vec<u8>> = Vec::new();
        while let Ok(s) = rx.recv_timeout(core::time::Duration::from_millis(200)) {
            if let UserSample::Alive { payload, .. } = s {
                got.push(payload.as_ref().to_vec());
            }
            if got.len() == 2 {
                break;
            }
        }
        assert_eq!(got, alloc::vec![b"s2".to_vec(), b"s3".to_vec()]);
        rt.shutdown();
    }

    /// QR (b) DURABILITY TRANSIENT_LOCAL: a late-joining reader receives the
    /// retained sample written before it matched. A VOLATILE writer replays
    /// nothing.
    #[test]
    fn qr_transient_local_late_join_replay_vs_volatile() {
        // TransientLocal: late joiner sees the prior sample.
        let rt = DcpsRuntime::start(
            62,
            GuidPrefix::from_bytes([0x62; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let w = rt
            .register_user_writer(qr_writer_cfg(
                "QrTL",
                zerodds_qos::DurabilityKind::TransientLocal,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("writer");
        rt.write_user_sample(w, b"retained".to_vec())
            .expect("write");
        let (_r, rx) = rt
            .register_user_reader(qr_reader_cfg(
                "QrTL",
                zerodds_qos::DurabilityKind::TransientLocal,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("reader");
        let s = rx
            .recv_timeout(core::time::Duration::from_millis(200))
            .expect("TransientLocal late joiner must replay the retained sample");
        match s {
            UserSample::Alive { payload, .. } => assert_eq!(payload.as_ref(), b"retained"),
            other => panic!("expected Alive, got {other:?}"),
        }
        rt.shutdown();

        // Volatile: late joiner gets nothing for the pre-match write.
        let rt2 = DcpsRuntime::start(
            63,
            GuidPrefix::from_bytes([0x63; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let w2 = rt2
            .register_user_writer(qr_writer_cfg(
                "QrVol",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("writer");
        rt2.write_user_sample(w2, b"lost".to_vec()).expect("write");
        assert_eq!(
            rt2.user_writer_retained_len(w2),
            0,
            "Volatile retains nothing"
        );
        let (_r2, rx2) = rt2
            .register_user_reader(qr_reader_cfg(
                "QrVol",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("reader");
        assert!(
            rx2.recv_timeout(core::time::Duration::from_millis(120))
                .is_err(),
            "Volatile late joiner must NOT replay a pre-match sample"
        );
        rt2.shutdown();
    }

    /// QR (c) PARTITION: a writer in partition ["A"] and a reader in ["B"]
    /// must NOT match (no intra-runtime route); a matching partition delivers.
    #[test]
    fn qr_partition_gates_intra_runtime_match() {
        let rt = DcpsRuntime::start(
            64,
            GuidPrefix::from_bytes([0x64; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        // Mismatched partitions.
        let w = rt
            .register_user_writer(qr_writer_cfg(
                "QrPart",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec!["A".into()],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("writer");
        let (_r_mismatch, rx_mismatch) = rt
            .register_user_reader(qr_reader_cfg(
                "QrPart",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec!["B".into()],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("reader");
        rt.write_user_sample(w, b"x".to_vec()).expect("write");
        assert!(
            rx_mismatch
                .recv_timeout(core::time::Duration::from_millis(120))
                .is_err(),
            "partitions [A] vs [B] must not match"
        );

        // Matching partition reader added → now delivers.
        let (_r_match, rx_match) = rt
            .register_user_reader(qr_reader_cfg(
                "QrPart",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec!["A".into()],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("reader");
        rt.write_user_sample(w, b"y".to_vec()).expect("write");
        let s = rx_match
            .recv_timeout(core::time::Duration::from_millis(200))
            .expect("partition [A] vs [A] must match");
        match s {
            UserSample::Alive { payload, .. } => assert_eq!(payload.as_ref(), b"y"),
            other => panic!("expected Alive, got {other:?}"),
        }
        rt.shutdown();
    }

    /// QR (d) KEYED LIFECYCLE: dispose(key) delivers a Lifecycle marker to a
    /// matched same-runtime reader; a later late joiner observes the terminal
    /// NOT_ALIVE_DISPOSED state.
    #[test]
    fn qr_dispose_delivers_lifecycle_to_intra_reader() {
        use zerodds_rtps::history_cache::ChangeKind;
        use zerodds_rtps::inline_qos::status_info;
        let rt = DcpsRuntime::start(
            65,
            GuidPrefix::from_bytes([0x65; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let w = rt
            .register_user_writer_kind(
                qr_writer_cfg(
                    "QrLifecycle",
                    zerodds_qos::DurabilityKind::TransientLocal,
                    alloc::vec![],
                    zerodds_qos::LivelinessKind::Automatic,
                ),
                true,
            )
            .expect("writer");
        let (_r, rx) = rt
            .register_user_reader_kind(
                qr_reader_cfg(
                    "QrLifecycle",
                    zerodds_qos::DurabilityKind::TransientLocal,
                    alloc::vec![],
                    zerodds_qos::LivelinessKind::Automatic,
                ),
                true,
            )
            .expect("reader");

        let key = [0xAB_u8; 16];
        rt.write_user_sample_keyed(w, b"alive", key).expect("write");
        // First the alive sample.
        let first = rx
            .recv_timeout(core::time::Duration::from_millis(200))
            .expect("alive sample");
        assert!(matches!(first, UserSample::Alive { .. }));

        // dispose(key) → Lifecycle marker NOT_ALIVE_DISPOSED.
        rt.write_user_lifecycle(w, key, status_info::DISPOSED)
            .expect("dispose");
        let life = rx
            .recv_timeout(core::time::Duration::from_millis(200))
            .expect("dispose must deliver a Lifecycle marker to the matched reader");
        match life {
            UserSample::Lifecycle { key_hash, kind } => {
                assert_eq!(key_hash, key);
                assert_eq!(kind, ChangeKind::NotAliveDisposed);
            }
            other => panic!("expected Lifecycle, got {other:?}"),
        }

        // A brand-new late joiner replays the alive sample AND the terminal
        // disposed marker, so it learns the instance is NOT_ALIVE_DISPOSED.
        let (_r2, rx2) = rt
            .register_user_reader_kind(
                qr_reader_cfg(
                    "QrLifecycle",
                    zerodds_qos::DurabilityKind::TransientLocal,
                    alloc::vec![],
                    zerodds_qos::LivelinessKind::Automatic,
                ),
                true,
            )
            .expect("reader2");
        let mut saw_disposed = false;
        while let Ok(s) = rx2.recv_timeout(core::time::Duration::from_millis(200)) {
            if let UserSample::Lifecycle { kind, .. } = s {
                if kind == ChangeKind::NotAliveDisposed {
                    saw_disposed = true;
                    break;
                }
            }
        }
        assert!(
            saw_disposed,
            "late joiner must observe the terminal NOT_ALIVE_DISPOSED state"
        );
        rt.shutdown();
    }

    /// QR (d) KEYED LIFECYCLE — unregister(key) maps to NOT_ALIVE (NO_WRITERS).
    #[test]
    fn qr_unregister_delivers_no_writers_lifecycle() {
        use zerodds_rtps::history_cache::ChangeKind;
        use zerodds_rtps::inline_qos::status_info;
        let rt = DcpsRuntime::start(
            66,
            GuidPrefix::from_bytes([0x66; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let w = rt
            .register_user_writer_kind(
                qr_writer_cfg(
                    "QrUnreg",
                    zerodds_qos::DurabilityKind::Volatile,
                    alloc::vec![],
                    zerodds_qos::LivelinessKind::Automatic,
                ),
                true,
            )
            .expect("writer");
        let (_r, rx) = rt
            .register_user_reader_kind(
                qr_reader_cfg(
                    "QrUnreg",
                    zerodds_qos::DurabilityKind::Volatile,
                    alloc::vec![],
                    zerodds_qos::LivelinessKind::Automatic,
                ),
                true,
            )
            .expect("reader");
        let key = [0x11_u8; 16];
        rt.write_user_lifecycle(w, key, status_info::UNREGISTERED)
            .expect("unregister");
        let life = rx
            .recv_timeout(core::time::Duration::from_millis(200))
            .expect("unregister must deliver a Lifecycle marker");
        match life {
            UserSample::Lifecycle { key_hash, kind } => {
                assert_eq!(key_hash, key);
                assert_eq!(kind, ChangeKind::NotAliveUnregistered);
            }
            other => panic!("expected Lifecycle, got {other:?}"),
        }
        rt.shutdown();
    }

    /// QR (e) LIVELINESS AUTOMATIC: the reader's liveliness_changed alive_count
    /// tracks a live matched writer on the same-runtime path.
    #[test]
    fn qr_liveliness_automatic_bumps_reader_alive_count() {
        let rt = DcpsRuntime::start(
            67,
            GuidPrefix::from_bytes([0x67; 12]),
            RuntimeConfig::default(),
        )
        .expect("start");
        let w = rt
            .register_user_writer(qr_writer_cfg(
                "QrLive",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("writer");
        let (r, rx) = rt
            .register_user_reader(qr_reader_cfg(
                "QrLive",
                zerodds_qos::DurabilityKind::Volatile,
                alloc::vec![],
                zerodds_qos::LivelinessKind::Automatic,
            ))
            .expect("reader");

        let (_alive0, count0, _na0) = rt.user_reader_liveliness_status(r);
        assert_eq!(count0, 0, "no writer has delivered yet");

        rt.write_user_sample(w, b"beat".to_vec()).expect("write");
        let _ = rx.recv_timeout(core::time::Duration::from_millis(200));

        let (alive, count, _na) = rt.user_reader_liveliness_status(r);
        assert!(alive, "AUTOMATIC writer keeps the reader's match alive");
        assert_eq!(
            count, 1,
            "alive_count must bump to 1 for the live matched AUTOMATIC writer"
        );

        // A second write from the same writer does NOT double-count.
        rt.write_user_sample(w, b"beat2".to_vec()).expect("write");
        let _ = rx.recv_timeout(core::time::Duration::from_millis(200));
        let (_a, count2, _n) = rt.user_reader_liveliness_status(r);
        assert_eq!(count2, 1, "same writer must not bump alive_count twice");
        rt.shutdown();
    }
}
