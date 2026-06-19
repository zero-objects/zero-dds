//! `spdp_demo`: a ZeroDDS participant discovers SPDP beacons.
//!
//! Binds a multicast receiver on the SPDP default group
//! (239.255.0.1:7400) and prints every received beacon.
//! In parallel, periodically sends its own beacons via unicast (Phase-0
//! limit: no multicast outbound; Phase 1 adds that with the
//! `socket2` crate for outbound interface selection).
//!
//! Usage:
//!
//! ```bash
//! # Terminal 1: Cyclone-Container starten
//! cd tests/interop
//! docker compose up -d
//!
//! # Terminal 2: Demo
//! cargo run --example spdp_demo --features std -p zerodds-discovery
//!
//! # Terminal 3: Stop Cyclone
//! cd tests/interop && docker compose down
//! ```
//!
//! Alternatively, without Docker: expects no Cyclone — runs as a pure
//! beacon sender, printing its own datagrams.
//!
//! zerodds-lint: allow no_dyn_in_safe (example code; `Box<dyn Error>` is
//! the idiomatic return type for main in demos here).

#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr)]

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration as StdDuration;

use zerodds_discovery::{DiscoveredParticipantsCache, SpdpBeacon, SpdpError, SpdpReader};
use zerodds_rtps::participant_data::{
    Duration as RtpsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{
    EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, SPDP_DEFAULT_MULTICAST_ADDRESS, VendorId,
    spdp_multicast_port,
};
use zerodds_transport::Transport;
use zerodds_transport_udp::UdpTransport;

const DOMAIN_ID: u32 = 0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = spdp_multicast_port(DOMAIN_ID);
    let group = Ipv4Addr::from(SPDP_DEFAULT_MULTICAST_ADDRESS);

    // Interface selection via env var (e.g. INTERFACE_IP=192.168.178.60).
    // Default: 0.0.0.0 (all interfaces, in practice often only loopback).
    let interface_ip: Ipv4Addr = std::env::var("INTERFACE_IP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Ipv4Addr::UNSPECIFIED);

    eprintln!("=== ZeroDDS SPDP-Demo ===");
    eprintln!("Multicast-Group: {group}:{port}");
    eprintln!("Interface: {interface_ip} (set INTERFACE_IP=<ip> to override)");

    // Multicast-Receiver
    let receiver = UdpTransport::bind_multicast_v4(group, port as u16, interface_ip)?
        .with_timeout(Some(StdDuration::from_millis(500)))?;
    eprintln!("Receiver bound: {:?}", receiver.local_locator());

    // Own beacon sender (unicast to 239.255.0.1; Linux/macOS routes
    // this via multicast if IGMP works correctly).
    let sender = UdpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0)?.set_multicast_ttl(32)?;

    // Generate a random GuidPrefix for this demo run
    let prefix = GuidPrefix::from_bytes([
        0xDE,
        0xAD,
        0xBE,
        0xEF,
        std::process::id() as u8,
        (std::process::id() >> 8) as u8,
        0,
        0,
        0,
        0,
        0,
        0,
    ]);
    let our_data = ParticipantBuiltinTopicData {
        guid: Guid::new(prefix, EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        participant_security_info: None,
        default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 1], 7410)),
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
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    };
    let mut beacon = SpdpBeacon::new(our_data);
    let reader = SpdpReader::new();
    let cache = Arc::new(std::sync::Mutex::new(DiscoveredParticipantsCache::new()));
    let dest = Locator::udp_v4(SPDP_DEFAULT_MULTICAST_ADDRESS, port);

    // Beacon-Thread
    let stop = Arc::new(AtomicBool::new(false));
    let stop_beacon = Arc::clone(&stop);
    let beacon_handle = thread::spawn(move || {
        while !stop_beacon.load(Ordering::Relaxed) {
            match beacon.serialize() {
                Ok(datagram) => {
                    if let Err(e) = sender.send(&dest, &datagram) {
                        eprintln!("send error: {e}");
                    } else {
                        eprintln!("→ sent SPDP beacon (sn={})", beacon.next_sn - 1);
                    }
                }
                Err(e) => eprintln!("beacon serialize error: {e}"),
            }
            thread::sleep(StdDuration::from_secs(2));
        }
    });

    // Receiver-Loop
    let stop_recv = Arc::clone(&stop);
    let cache_recv = Arc::clone(&cache);
    let recv_handle = thread::spawn(move || {
        while !stop_recv.load(Ordering::Relaxed) {
            match receiver.recv() {
                Ok(datagram) => match reader.parse_datagram(&datagram.data) {
                    Ok(p) => {
                        let mut c = cache_recv.lock().expect("cache lock");
                        if c.insert(p.clone()) {
                            println!(
                                "✓ DISCOVERED: prefix={:?}, vendor={:?}, version={:?}, locators=[unicast={:?}, multicast={:?}]",
                                p.data.guid.prefix,
                                p.sender_vendor,
                                p.data.protocol_version,
                                p.data.default_unicast_locator,
                                p.data.default_multicast_locator,
                            );
                            println!("  total known participants: {}", c.len());
                        }
                    }
                    Err(SpdpError::NotSpdp) => {
                        // Ignore non-SPDP datagrams (e.g. user DATA).
                    }
                    Err(e) => eprintln!("parse error: {e}"),
                },
                Err(e) => match e {
                    zerodds_transport::RecvError::Timeout => {} // normal
                    other => eprintln!("recv error: {other}"),
                },
            }
        }
    });

    eprintln!("Listening for SPDP beacons. Press Ctrl+C to stop.");
    eprintln!();

    // Demo runs for 30 seconds, then cleanup.
    thread::sleep(StdDuration::from_secs(30));
    stop.store(true, Ordering::Relaxed);
    let _ = beacon_handle.join();
    let _ = recv_handle.join();

    let final_cache = cache.lock().expect("cache lock");
    eprintln!();
    eprintln!("=== Final Report ===");
    eprintln!("Total participants discovered: {}", final_cache.len());
    for p in final_cache.iter() {
        eprintln!(
            "  - prefix={:?}, vendor={:?}",
            p.data.guid.prefix, p.sender_vendor
        );
    }
    Ok(())
}
