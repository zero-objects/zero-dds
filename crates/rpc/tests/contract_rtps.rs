// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps`:
//! - `inline_qos::SampleIdentityBytes::new(writer_guid, sequence_number)` for
//!   the related sample identity of a reply (`src/replier.rs`,
//!   `src/requester.rs`);
//! - `publication_data::PublicationBuiltinTopicData` and
//!   `subscription_data::SubscriptionBuiltinTopicData` it fills for its
//!   endpoints (`src/endpoint.rs`), with `VendorId::ZERODDS`,
//!   `Guid::new(prefix, EntityId::PARTICIPANT)` as the participant key, and
//!   the re-exported `publication_data::DurabilityKind` and `ReliabilityQos`
//!   at their defaults. The shape of the two structs (every field the crate
//!   names in its literals) is checked by building the crate, which this
//!   target does first.
//!
//! Oracle: DDS-RPC 1.0 §7.5.1.1.1 (SampleIdentity = GUID_t + SequenceNumber_t),
//! DDSI-RTPS 2.5 §9.3.1.3 Table 9.2 (ENTITYID_PARTICIPANT), §9.3.2.10,
//! DDS 1.4 §2.2.3 (DURABILITY default VOLATILE; RELIABILITY default
//! BEST_EFFORT for a reader, max_blocking_time 100 ms).
#![allow(clippy::unwrap_used)]

use zerodds_rtps::inline_qos::SampleIdentityBytes;
use zerodds_rtps::publication_data::{DurabilityKind, ReliabilityQos};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};

#[test]
fn rpc_7_5_1_1_1_sample_identity_is_guid_then_sequence_number() {
    let guid = [7_u8; 16];
    let identity = SampleIdentityBytes::new(guid, (3_u64 << 32) | 9);
    let little = identity.to_bytes(true);
    assert_eq!(little[..16], guid);
    assert_eq!(little[16..20], 3_i32.to_le_bytes(), "high");
    assert_eq!(little[20..], 9_u32.to_le_bytes(), "low");
    let big = identity.to_bytes(false);
    assert_eq!(big[16..20], 3_i32.to_be_bytes());
    assert_eq!(
        SampleIdentityBytes::from_bytes(&big, false).unwrap(),
        identity
    );
}

#[test]
fn rtps_9_3_1_3_the_participant_key_ends_in_entityid_participant() {
    let prefix = GuidPrefix::zerodds([1, 2, 3, 4], [5, 6, 7, 8, 9, 10]);
    let key = Guid::new(prefix, EntityId::PARTICIPANT);
    assert_eq!(key.to_bytes()[12..], [0x00, 0x00, 0x01, 0xc1]);
    assert_eq!(VendorId::ZERODDS.to_bytes(), prefix.0[..2]);
}

#[test]
fn dcps_2_2_3_the_defaults_the_endpoint_data_takes() {
    assert_eq!(DurabilityKind::default(), DurabilityKind::Volatile);
    let reliability = ReliabilityQos::default();
    assert_eq!(reliability.kind, zerodds_qos::ReliabilityKind::BestEffort);
    assert_eq!(
        reliability.max_blocking_time,
        zerodds_qos::Duration::from_millis(100)
    );
}
