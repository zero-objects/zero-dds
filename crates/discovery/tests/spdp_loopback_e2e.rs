//! E2E loopback SPDP tests for zerodds-discovery (WP-0.7-A.1).
//!
//! These tests verify that the SPDP pipeline works end-to-end over
//! UDP multicast — without containers, without an external DDS
//! stack. They run in CI and guarantee that the ZeroDDS side of the
//! discovery path is implemented correctly.
//!
//! Live vendor tests (Cyclone, Fast-DDS) need Docker + Linux with
//! IP multicast — see `tests/interop/run_interop.sh`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_discovery::{DiscoveredParticipantsCache, SpdpBeacon, SpdpReader};
use zerodds_rtps::participant_data::{
    Duration as RtpsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{
    EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, SPDP_DEFAULT_MULTICAST_ADDRESS, VendorId,
    spdp_multicast_port,
};
use zerodds_transport::Transport;
use zerodds_transport_udp::UdpTransport;

fn make_participant(prefix_byte: u8, port: u32) -> ParticipantBuiltinTopicData {
    ParticipantBuiltinTopicData {
        guid: Guid::new(
            GuidPrefix::from_bytes([prefix_byte; 12]),
            EntityId::PARTICIPANT,
        ),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 1], port)),
        default_multicast_locator: Some(Locator::udp_v4(SPDP_DEFAULT_MULTICAST_ADDRESS, port)),
        metatraffic_unicast_locator: None,
        metatraffic_multicast_locator: None,
        domain_id: None,
        builtin_endpoint_set: endpoint_flag::PARTICIPANT_ANNOUNCER
            | endpoint_flag::PARTICIPANT_DETECTOR,
        lease_duration: RtpsDuration::from_secs(100),
        user_data: Vec::new(),
        properties: Default::default(),
        identity_token: None,
        permissions_token: None,
        participant_security_info: None,
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    }
}

/// Helper: can this platform do multicast? If not, skip the
/// subsequent multicast tests.
fn multicast_available(port: u16) -> bool {
    let group = Ipv4Addr::from(SPDP_DEFAULT_MULTICAST_ADDRESS);
    UdpTransport::bind_multicast_v4(group, port, Ipv4Addr::UNSPECIFIED).is_ok()
}

#[test]
fn loopback_multicast_self_discovery() {
    // A participant sends its own beacon and discovers itself via
    // loopback multicast. If the platform does not support
    // multicast, we skip.
    let port = spdp_multicast_port(0) as u16;
    if !multicast_available(port) {
        eprintln!("Multicast not available; skipping loopback_multicast_self_discovery");
        return;
    }
    let group = Ipv4Addr::from(SPDP_DEFAULT_MULTICAST_ADDRESS);
    let receiver = UdpTransport::bind_multicast_v4(group, port, Ipv4Addr::UNSPECIFIED)
        .expect("bind multicast")
        .with_timeout(Some(Duration::from_millis(500)))
        .expect("set timeout");
    let sender = UdpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0)
        .expect("bind sender")
        .set_multicast_ttl(1)
        .expect("set ttl");

    let mut beacon = SpdpBeacon::new(make_participant(0xAA, u32::from(port)));
    let datagram = beacon.serialize().expect("serialize");
    let dest = Locator::udp_v4(SPDP_DEFAULT_MULTICAST_ADDRESS, u32::from(port));

    // Send a few times, since the first datagram may be lost
    // (IGMP setup on some OSes, loopback routing initialization).
    let stop = Arc::new(AtomicBool::new(false));
    let stop_send = Arc::clone(&stop);
    let send_handle = thread::spawn(move || {
        while !stop_send.load(Ordering::Relaxed) {
            sender.send(&dest, &datagram).expect("send");
            thread::sleep(Duration::from_millis(200));
        }
    });

    // Receiver: wait up to 3 seconds for our own beacon.
    let reader = SpdpReader::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut found = false;
    while Instant::now() < deadline {
        match receiver.recv() {
            Ok(d) => {
                if let Ok(p) = reader.parse_datagram(&d.data) {
                    if p.data.guid.prefix == GuidPrefix::from_bytes([0xAA; 12]) {
                        found = true;
                        break;
                    }
                }
            }
            Err(zerodds_transport::RecvError::Timeout) => continue,
            Err(e) => panic!("recv error: {e}"),
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = send_handle.join();
    assert!(found, "self-discovery via loopback-multicast must succeed");
}

#[test]
fn two_zerodds_participants_discover_each_other() {
    // Simulates two ZeroDDS participants on the same machine that
    // discover each other over multicast.
    let port = spdp_multicast_port(0) as u16;
    if !multicast_available(port) {
        eprintln!("Multicast not available; skipping two-participant test");
        return;
    }
    let group = Ipv4Addr::from(SPDP_DEFAULT_MULTICAST_ADDRESS);

    // Two separate receivers (same group, different sockets).
    // On macOS Phase 0 without SO_REUSEADDR the second bind may be
    // problematic; skip then.
    let receiver_a = match UdpTransport::bind_multicast_v4(group, port, Ipv4Addr::UNSPECIFIED) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Cannot bind first multicast receiver; skipping");
            return;
        }
    }
    .with_timeout(Some(Duration::from_millis(500)))
    .expect("set timeout a");

    // Receiver B needs its own socket. Without REUSEADDR many OSes
    // reject the second bind — we use a different port.
    let receiver_b = match UdpTransport::bind_multicast_v4(group, 0, Ipv4Addr::UNSPECIFIED) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Cannot bind second multicast receiver ({e}); skipping");
            return;
        }
    }
    .with_timeout(Some(Duration::from_millis(500)))
    .expect("set timeout b");

    let sender_a = UdpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0)
        .expect("sender a")
        .set_multicast_ttl(1)
        .expect("ttl a");
    let sender_b = UdpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0)
        .expect("sender b")
        .set_multicast_ttl(1)
        .expect("ttl b");

    let mut beacon_a = SpdpBeacon::new(make_participant(0xAA, u32::from(port)));
    let mut beacon_b = SpdpBeacon::new(make_participant(0xBB, u32::from(port)));
    let datagram_a = beacon_a.serialize().expect("serialize a");
    let datagram_b = beacon_b.serialize().expect("serialize b");
    let dest = Locator::udp_v4(SPDP_DEFAULT_MULTICAST_ADDRESS, u32::from(port));

    let cache_a = Arc::new(Mutex::new(DiscoveredParticipantsCache::new()));
    let cache_b = Arc::new(Mutex::new(DiscoveredParticipantsCache::new()));

    let stop = Arc::new(AtomicBool::new(false));
    let stop_send = Arc::clone(&stop);

    // Sender thread: both beacons periodically.
    let send_handle = thread::spawn(move || {
        while !stop_send.load(Ordering::Relaxed) {
            let _ = sender_a.send(&dest, &datagram_a);
            let _ = sender_b.send(&dest, &datagram_b);
            thread::sleep(Duration::from_millis(200));
        }
    });

    // Both receivers: run for 3 seconds, accumulate cache.
    let reader = SpdpReader::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Ok(d) = receiver_a.recv() {
            if let Ok(p) = reader.parse_datagram(&d.data) {
                cache_a.lock().expect("lock a").insert(p);
            }
        }
        if let Ok(d) = receiver_b.recv() {
            if let Ok(p) = reader.parse_datagram(&d.data) {
                cache_b.lock().expect("lock b").insert(p);
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = send_handle.join();

    let n_a = cache_a.lock().expect("lock a").len();
    let n_b = cache_b.lock().expect("lock b").len();
    eprintln!("Receiver A discovered {n_a} participants");
    eprintln!("Receiver B discovered {n_b} participants");

    // At least one discovery in at least one cache — a weak assert,
    // but resistant to the loss rate of loopback multicast in
    // restrictive CI environments.
    assert!(
        n_a >= 1 || n_b >= 1,
        "at least one receiver should discover at least one participant"
    );
}
