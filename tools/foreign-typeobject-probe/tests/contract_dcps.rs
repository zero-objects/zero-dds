// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-foreign-typeobject-probe` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-dcps` (`src/main.rs`):
//! - `participant::host_id_bytes` and `participant::next_process_unique`
//!   for its prefix (`GuidPrefix::zerodds`).
//! - `runtime::DcpsRuntime::start(domain, prefix, RuntimeConfig::default())`;
//!   the public field `sedp` (the SEDP cache: a writer's `topic_name`,
//!   `type_information`, `participant_key.prefix`, `key.entity_id`); the
//!   public field `type_lookup_server` (`registry.get_complete`);
//!   `send_type_lookup_request` and `send_type_dependencies_request` to the
//!   writer's participant; `register_user_reader_kind` with a
//!   `UserReaderConfig { topic_name, type_name, reliable: true, ..Default }`
//!   of the writer's topic kind; `shutdown`.
//! - `runtime::UserSample::Alive { payload, representation, big_endian, .. }`:
//!   `representation == 1` selects the XCDR2 encodings for the decoder,
//!   `big_endian` the byte order.
//!
//! What it relies on and is checked here: the two halves of the prefix the
//! runtime gives a participant of this process — the host identifier,
//! stable within the process, and a process-unique tail of which two calls
//! never return the same while sharing the process id. Oracles: DDSI-RTPS
//! 2.5 §8.2.4.2 ("every Participant in the Domain has a unique GUID"),
//! §9.3.1.5 (vendor id in octets 0..2), the rustdoc of the generated
//! participant prefix in `participant.rs` (octets 2..6 the host id, 6..10
//! the process id, 10..12 a counter that makes the prefixes of one process
//! pairwise different) and of `host_id_bytes` (cached per process). A
//! reader config over `Default` announces no TypeIdentifier and requests
//! VOLATILE (rustdoc of `UserReaderConfig`'s `Default`; DDS 1.4 §2.2.3).
//! The live calls need a bound runtime and a foreign writer; their shape is
//! checked at compile time. No socket is opened.

use std::sync::mpsc::Receiver;

use zerodds_dcps::qos::DurabilityKind;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_rtps::wire_types::{EntityKind, GuidPrefix, VendorId};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

#[test]
fn rtps_8_2_4_2_two_participant_prefixes_of_this_process_differ() {
    let host = zerodds_dcps::participant::host_id_bytes();
    assert_eq!(zerodds_dcps::participant::host_id_bytes(), host);
    let first = zerodds_dcps::participant::next_process_unique();
    let second = zerodds_dcps::participant::next_process_unique();
    assert_ne!(first, second);
    assert_eq!(first[..4], second[..4], "the process id");
    let a = GuidPrefix::zerodds(host, first);
    let b = GuidPrefix::zerodds(host, second);
    assert_ne!(a, b);
    assert_eq!(a.0[..2], VendorId::ZERODDS.to_bytes());
    assert_eq!(a.0[2..6], host);
    assert_eq!(a.0[6..10], first[..4]);
    assert!(a.is_same_host(b));
}

#[test]
fn the_reader_config_over_default_announces_no_type_identifier() {
    let cfg = UserReaderConfig {
        topic_name: "Robot".to_string(),
        type_name: "RobotModule::Robot".to_string(),
        reliable: true,
        ..Default::default()
    };
    assert_eq!(cfg.type_identifier, TypeIdentifier::None);
    assert_eq!(cfg.durability, DurabilityKind::Volatile);
    assert!(cfg.reliable);
}

/// What the probe reads of one sample before decoding it.
fn decoding_choice(sample: &UserSample) -> Option<(bool, bool, usize)> {
    match sample {
        UserSample::Alive {
            payload,
            representation,
            big_endian,
            ..
        } => Some((*representation == 1, *big_endian, payload.len())),
        _ => None,
    }
}

#[test]
fn the_live_calls_keep_the_shape_the_probe_uses() {
    let _: fn(&UserSample) -> Option<(bool, bool, usize)> = decoding_choice;
    let _: fn(i32, GuidPrefix, RuntimeConfig) -> zerodds_dcps::Result<std::sync::Arc<DcpsRuntime>> =
        DcpsRuntime::start;
    let _ = |rt: &DcpsRuntime, topic: &str| -> Option<(GuidPrefix, bool, bool)> {
        let sedp = rt.sedp.lock().ok()?;
        sedp.cache().publications().find_map(|publication| {
            let data = &publication.data;
            if data.topic_name != topic {
                return None;
            }
            Some((
                data.participant_key.prefix,
                data.type_information.is_some(),
                data.key.entity_id.entity_kind != EntityKind::UserWriterNoKey,
            ))
        })
    };
    let _ = |rt: &DcpsRuntime, hash: &EquivalenceHash| -> bool {
        rt.type_lookup_server
            .lock()
            .is_ok_and(|server| server.registry.get_complete(hash).is_some())
    };
    let _ = |rt: &DcpsRuntime,
             peer: GuidPrefix,
             asked: &[TypeIdentifier],
             cfg: UserReaderConfig|
     -> zerodds_dcps::Result<Receiver<UserSample>> {
        let _sent: bool = rt.send_type_lookup_request(peer, asked)?.is_some();
        let _ = rt.send_type_dependencies_request(peer, asked)?;
        let (_reader, rx) = rt.register_user_reader_kind(cfg, true)?;
        rt.shutdown();
        Ok(rx)
    };
}
