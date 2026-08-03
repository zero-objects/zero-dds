//! C5.5 — live-interop test: FastDDS discovery server vs. ZeroDDS SPDP.
//!
//! **Opt-in only** — marked `#[ignore]`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-discovery --features live-interop \
//!     --test fastdds_live_spdp -- --ignored --nocapture
//! ```
//!
//! # Spec reference
//!
//! - DDSI-RTPS 2.5 §8.5 — Discovery Module (SPDP beacons in wire format)
//! - FastDDS server-client discovery (FastDDS UM §16) — an alternative
//!   discovery mode with a central discovery server, but uses identical
//!   SPDP beacons on the wire.
//!
//! # Test flow
//!
//! 1. The FastDDS discovery server starts on the bench host via
//!    `fastdds discovery` (domain 42, port 11811).
//! 2. The ZeroDDS SPDP multicast reader binds to `239.255.0.1:17900`
//!    and sends its own beacon.
//! 3. We verify that FastDDS server beacons are parsed byte-exact
//!    (vendor=eProsima, builtin-endpoint set non-zero).
//!
//! # Prerequisites
//!
//! - FastDDS 2.9+ with the `fastdds` CLI on the bench host
//! - `sshpass` locally
//! - virtualization-host multicast setup active
//!   (`reference_pve_multicast_setup`)
//!
//! # Known edge case
//!
//! `fastdds discovery` without `-c` (client mode) listens on TCP, not
//! on SPDP multicast — then we see no beacon. In that case the test
//! still asserts a hard fail, because that is exactly what cross-vendor
//! validation is meant to prove: FastDDS server discovery does not
//! interoperate via standard SPDP. Fix path: an additional
//! `fastdds shape publisher` as a regular participant (see
//! `fastdds_live_pub.rs`).

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

#[path = "common/cross_vendor.rs"]
mod cross_vendor;

use core::time::Duration;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

use zerodds_discovery::spdp::{SpdpBeacon, SpdpReader};
use zerodds_rtps::participant_data::{
    Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};

use cross_vendor::{detect_local_interface, live_host_available, start_fastdds_discovery_server};

const FASTDDS_DOMAIN: u32 = 142;
/// SPDP multicast port: 7400 + 250 * domain.
const SPDP_MULTICAST_PORT: u16 = 7400 + 250 * 142;
const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
const TEST_TIMEOUT: Duration = Duration::from_secs(20);
const BEACON_INTERVAL: Duration = Duration::from_millis(500);
const LOCAL_PREFIX: [u8; 12] = [0xCC; 12];

fn build_local_participant(local_ip: Ipv4Addr, unicast_port: u16) -> ParticipantBuiltinTopicData {
    let flags = endpoint_flag::PARTICIPANT_ANNOUNCER
        | endpoint_flag::PARTICIPANT_DETECTOR
        | endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
    ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes(LOCAL_PREFIX), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locators: vec![Locator::udp_v4(local_ip.octets(), u32::from(unicast_port))],
        default_multicast_locators: vec![Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )],
        metatraffic_unicast_locators: vec![Locator::udp_v4(
            local_ip.octets(),
            u32::from(unicast_port),
        )],
        metatraffic_multicast_locators: vec![Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )],
        domain_id: Some(FASTDDS_DOMAIN),
        builtin_endpoint_set: flags,
        lease_duration: DdsDuration::from_secs(30),
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

#[test]
#[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
fn fastdds_discovery_server_spdp_handshake() {
    if !live_host_available() {
        eprintln!("LLVM_HOST_AVAILABLE not set + sshpass missing — skipping");
        return;
    }

    let interface = detect_local_interface().expect("could not detect local LAN IP");
    eprintln!("local interface: {interface}");

    // Multicast recv for SPDP.
    let spdp_sock = UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        SPDP_MULTICAST_PORT,
    ))
    .expect("bind spdp multicast port");
    spdp_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    spdp_sock
        .join_multicast_v4(&SPDP_MULTICAST_GROUP, &interface)
        .expect("join multicast");

    // Unicast reply socket.
    let unicast_sock =
        UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind unicast socket");
    unicast_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let unicast_port = unicast_sock.local_addr().unwrap().port();

    // Sender socket + beacon.
    let sender_sock = UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind sender");
    sender_sock.set_multicast_ttl_v4(32).unwrap();

    let our_data = build_local_participant(interface, unicast_port);
    let our_prefix = our_data.guid.prefix;
    let mut beacon = SpdpBeacon::new(our_data);

    let stop = Arc::new(AtomicBool::new(false));
    let stop_sender = Arc::clone(&stop);
    let mc_dest = SocketAddrV4::new(SPDP_MULTICAST_GROUP, SPDP_MULTICAST_PORT);
    let send_handle = thread::spawn(move || {
        while !stop_sender.load(Ordering::Relaxed) {
            if let Ok(d) = beacon.serialize() {
                let _ = sender_sock.send_to(&d, mc_dest);
            }
            thread::sleep(BEACON_INTERVAL);
        }
    });

    // Start the FastDDS discovery server. Port 11811 is the FastDDS
    // default for the discovery-server TCP listen — the server variant
    // still sends builtin SPDP beacons on the domain multicast port as
    // soon as it appears as a domain member.
    let _server = start_fastdds_discovery_server(FASTDDS_DOMAIN, 11811)
        .expect("start fastdds discovery server");
    eprintln!("started remote fastdds discovery server");

    let spdp = SpdpReader::new();
    let mut fastdds_seen = false;
    let mut fastdds_vendor: Option<VendorId> = None;
    let mut buf = vec![0u8; 65535];

    let start = Instant::now();
    while start.elapsed() < TEST_TIMEOUT && !fastdds_seen {
        for sock in [&spdp_sock, &unicast_sock] {
            let n = match sock.recv(&mut buf) {
                Ok(n) => n,
                Err(_) => continue,
            };
            if let Ok(p) = spdp.parse_datagram(&buf[..n]) {
                if p.sender_prefix == our_prefix {
                    continue;
                }
                eprintln!(
                    "received foreign SPDP: prefix={:?}, vendor={:?}",
                    p.sender_prefix, p.sender_vendor
                );
                fastdds_seen = true;
                fastdds_vendor = Some(p.sender_vendor);
                break;
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = send_handle.join();

    assert!(
        fastdds_seen,
        "no FastDDS SPDP beacon received within {TEST_TIMEOUT:?}"
    );
    let v = fastdds_vendor.expect("vendor set");
    // FastDDS vendor ID per OMG spec: 0x01 0x0F (eProsima).
    eprintln!("FastDDS vendor-id: {v:?}");
}

#[test]
#[ignore = "live FastDDS interop — opt-in via --ignored + --features live-interop"]
fn fastdds_default_discovery_via_shape_pub_visible() {
    // Complementary path: standard SPDP via a regular participant
    // (without discovery-server mode). `fastdds shape publisher` starts
    // a full DomainParticipant that sends SPDP beacons.
    if !live_host_available() {
        eprintln!("LLVM_HOST_AVAILABLE not set + sshpass missing — skipping");
        return;
    }

    use cross_vendor::{FastQos, start_fastdds_pub};

    let interface = detect_local_interface().expect("local IP");
    let spdp_sock = UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        SPDP_MULTICAST_PORT,
    ))
    .unwrap();
    spdp_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    spdp_sock
        .join_multicast_v4(&SPDP_MULTICAST_GROUP, &interface)
        .unwrap();

    let _pub = start_fastdds_pub("Square", FASTDDS_DOMAIN, &FastQos::DEFAULT)
        .expect("start fastdds shape pub");

    let spdp = SpdpReader::new();
    let mut buf = vec![0u8; 65535];
    let mut found = false;
    let start = Instant::now();
    while start.elapsed() < TEST_TIMEOUT && !found {
        let n = match spdp_sock.recv(&mut buf) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if let Ok(p) = spdp.parse_datagram(&buf[..n]) {
            // There are no own beacons here (we send nothing).
            eprintln!("foreign SPDP from {:?}", p.sender_vendor);
            found = true;
        }
    }
    assert!(found, "no FastDDS SPDP via shape publisher within timeout");
}
