// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A1 — optional discovery-server mode (without the Action breakage).
//!
//! ROS-2 painpoint (ros.discourse #19900): eProsima's Discovery Server breaks
//! ROS-2 Actions because it proxies SEDP (endpoint discovery), and the proxy
//! mishandles the dynamically-created Action endpoints.
//!
//! ZeroDDS' discovery server ([`RuntimeConfig::discovery_server`]) relays **only
//! SPDP** (participant-locator exchange): clients point only at the server and
//! learn each other's participant locators through it, then run **SEDP directly
//! peer-to-peer**. Because the server never touches SEDP, dynamic endpoints
//! (incl. Actions) discover exactly as they do in the full mesh — no breakage.
//!
//! This test proves the core: a server S relays between clients A and B that
//! know ONLY S (multicast off, `initial_peers = [S]`, NOT each other). A must
//! discover B through S, then A↔B user data flows directly.
//!
//! macOS: ignored — same-host SPDP is unreliable on the macOS loopback (as with
//! the other e2e tests); runs on Linux / the CI bench.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::{GuidPrefix, Locator};

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: None,
    }
}

fn reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        group_data: vec![],
        topic_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

/// Multicast-free client config that knows ONLY the server's SPDP locator.
fn client_cfg(server: &Locator) -> RuntimeConfig {
    let loopback = Locator::udp_v4([127, 0, 0, 1], server.port);
    RuntimeConfig {
        spdp_multicast_send: false,
        initial_peers: vec![loopback],
        ..RuntimeConfig::default()
    }
}

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn clients_discover_each_other_through_the_server() {
    let domain = 95;
    let host = [0x71u8; 4];
    let mk = |tag: u8| {
        let mut p = [tag; 12];
        p[..4].copy_from_slice(&host);
        GuidPrefix::from_bytes(p)
    };

    // The server: multicast-free + SPDP relay on. No user endpoints.
    let server = DcpsRuntime::start(domain, mk(0x51), RuntimeConfig::discovery_server()).unwrap();
    let server_loc = server.spdp_unicast_locator();

    // Two clients that know ONLY the server (not each other).
    let a = DcpsRuntime::start(domain, mk(0xA1), client_cfg(&server_loc)).unwrap();
    let b = DcpsRuntime::start(domain, mk(0xB2), client_cfg(&server_loc)).unwrap();

    // A must discover B's participant through the server's SPDP relay.
    let b_prefix = b.guid_prefix;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut found = false;
    while Instant::now() < deadline {
        if a.discovered_participants()
            .iter()
            .any(|dp| dp.sender_prefix == b_prefix)
        {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        found,
        "client A never discovered client B through the discovery server (relay broken)"
    );

    // And the relay must work the other way too.
    let a_prefix = a.guid_prefix;
    let mut found_back = false;
    while Instant::now() < deadline {
        if b.discovered_participants()
            .iter()
            .any(|dp| dp.sender_prefix == a_prefix)
        {
            found_back = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        found_back,
        "client B never discovered client A through the server"
    );

    // SEDP happens directly A<->B: a writer on A reaches a reader on B.
    let topic = "A1DiscServer";
    let w = a.register_user_writer(writer_cfg(topic)).unwrap();
    let (r, rx) = b.register_user_reader(reader_cfg(topic)).unwrap();

    let mdl = Instant::now() + Duration::from_secs(15);
    while Instant::now() < mdl
        && (b.user_reader_matched_count(r) < 1 || a.user_writer_matched_count(w) < 1)
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        b.user_reader_matched_count(r) >= 1 && a.user_writer_matched_count(w) >= 1,
        "A<->B did not match directly via SEDP after discovery-server bridging"
    );

    a.write_user_sample(w, vec![0xA1, 0x5E, 0x12, 0x34])
        .expect("write");
    let got = rx.recv_timeout(Duration::from_secs(3));
    assert!(
        got.is_ok(),
        "user data did not flow A->B after discovery-server bridging"
    );
}
