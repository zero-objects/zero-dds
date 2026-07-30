// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TSN live-transport e2e harness binary (feature `tsn-live`, Linux).
//!
//! Starts a DcpsRuntime with `user_transport = Tsn` (AF_PACKET,
//! interface via `ZERODDS_TSN_IFACE`) and runs a pub/sub roundtrip.
//! Two instances run in separate network namespaces (connected by a
//! veth pair) — see `tests/interop/tsn_netns_e2e.sh`.
//!
//! Discovery (SPDP/SEDP) runs over UDP multicast (must cross the veth
//! pair); the user traffic goes over TSN Ethernet frames (0x88B5).
//!
//! Invocation: `tsn_pingpong ping` (writer) or `tsn_pingpong pong` (reader).
//! Exit 0 = roundtrip succeeded, !=0 = error/timeout.

// Example binary: console output is intentional here (analogous to the
// other dcps examples).
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::unreachable
)]

#[cfg(not(all(feature = "tsn-live", target_os = "linux")))]
fn main() {
    eprintln!("tsn_pingpong needs --features tsn-live on Linux");
    std::process::exit(2);
}

#[cfg(all(feature = "tsn-live", target_os = "linux"))]
fn main() {
    std::process::exit(real_main());
}

#[cfg(all(feature = "tsn-live", target_os = "linux"))]
fn real_main() -> i32 {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use zerodds_dcps::runtime::{
        DcpsRuntime, RuntimeConfig, UserReaderConfig, UserTransportKind, UserWriterConfig,
    };
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
    };
    use zerodds_rtps::wire_types::GuidPrefix;

    let role = std::env::args().nth(1).unwrap_or_default();
    let domain = 140;
    let topic = "TsnPingPong";
    let payload = b"tsn-roundtrip-0123456789";

    // Distinct host_id prefixes (different first 4 bytes).
    let prefix = match role.as_str() {
        "ping" => GuidPrefix::from_bytes([0xA1, 0x11, 0x00, 0x01, 1, 2, 3, 4, 5, 6, 7, 8]),
        "pong" => GuidPrefix::from_bytes([0xB2, 0x22, 0x00, 0x02, 8, 7, 6, 5, 4, 3, 2, 1]),
        other => {
            eprintln!("unknown role '{other}' (expected: ping|pong)");
            return 2;
        }
    };

    let cfg = RuntimeConfig {
        user_transport: Some(UserTransportKind::Tsn),
        ..RuntimeConfig::default()
    };
    let rt = match DcpsRuntime::start(domain, prefix, cfg) {
        Ok(rt) => Arc::new(rt),
        Err(e) => {
            eprintln!("DcpsRuntime::start failed: {e:?}");
            return 1;
        }
    };

    let writer_cfg = |topic: &str| UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
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
    };
    let reader_cfg = |topic: &str| UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    };

    let deadline = Instant::now() + Duration::from_secs(20);
    match role.as_str() {
        "ping" => {
            let weid = rt.register_user_writer(writer_cfg(topic)).expect("writer");
            while Instant::now() < deadline {
                if rt.user_writer_matched_count(weid) >= 1 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if rt.user_writer_matched_count(weid) < 1 {
                eprintln!("ping: no reader matched via SEDP (TSN discovery?)");
                return 1;
            }
            // Send multiple times — reliable recovery may need a few ticks.
            for _ in 0..20 {
                let _ = rt.write_user_sample(weid, payload.to_vec());
                std::thread::sleep(Duration::from_millis(100));
            }
            println!("TSN-PING-DONE");
            0
        }
        "pong" => {
            let (reid, rx) = rt.register_user_reader(reader_cfg(topic)).expect("reader");
            while Instant::now() < deadline {
                if rt.user_reader_matched_count(reid) >= 1 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(zerodds_dcps::runtime::UserSample::Alive { payload: bytes, .. }) => {
                    if bytes.as_slice() == payload {
                        println!("TSN-PONG-OK");
                        0
                    } else {
                        eprintln!("pong: payload mismatch");
                        1
                    }
                }
                Ok(_) => {
                    eprintln!("pong: unexpected sample (not Alive)");
                    1
                }
                Err(_) => {
                    eprintln!("pong: timeout — no sample received over TSN");
                    1
                }
            }
        }
        _ => unreachable!(),
    }
}
