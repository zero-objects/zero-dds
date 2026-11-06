// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-durability-service` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`src/lib.rs`):
//! `EntityKind::topic_kind` on a discovered writer's GUID to decide whether
//! the topic is keyed (`writer_is_keyed`); `Guid::from_bytes` and
//! `SequenceNumber(i64)` for the source identity a replayed sample carries;
//! `EntityId` and `HeTimestamp` passed through to `zerodds-dcps`; and
//! `history_cache::ChangeKind` turned into `inline_qos::status_info` bits
//! (`lifecycle_status_bits`). Oracle: DDSI-RTPS 2.5 §9.3.1.2 Table 9.1,
//! §9.3.1.5 (GUID_t), §9.3.2.10 (sequence_number = low + high·2^32),
//! §9.6.4.9 (StatusInfo D and U flags).

use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
use zerodds_rtps::wire_types::{EntityKind, Guid, SequenceNumber, TopicKind};

#[test]
fn rtps_9_3_1_2_keyed_and_keyless_writer_kinds() {
    for byte in [0x02, 0xc2, 0x07, 0xc7] {
        assert_eq!(
            EntityKind::from_byte(byte).topic_kind(),
            Some(TopicKind::WithKey),
            "{byte:#04x}"
        );
    }
    for byte in [0x03, 0xc3, 0x04, 0xc4] {
        assert_eq!(
            EntityKind::from_byte(byte).topic_kind(),
            Some(TopicKind::NoKey),
            "{byte:#04x}"
        );
    }
    // Neither: participant, groups, unknown, and octets Table 9.1 does not
    // name (RTI writes 0x82 and 0x87).
    for byte in [0x00, 0xc0, 0xc1, 0x08, 0x09, 0xc8, 0xc9, 0x82, 0x87] {
        assert_eq!(
            EntityKind::from_byte(byte).topic_kind(),
            None,
            "{byte:#04x}"
        );
    }
}

#[test]
fn rtps_9_3_1_5_a_source_guid_is_prefix_then_entity_id() {
    let bytes = [0x01, 0x0f, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0x12, 0x02];
    let guid = Guid::from_bytes(bytes);
    assert_eq!(guid.prefix.0, bytes[..12]);
    assert_eq!(guid.entity_id.entity_key, [0, 0, 0x12]);
    assert_eq!(guid.entity_id.entity_kind, EntityKind::UserWriterWithKey);
    assert_eq!(guid.to_bytes(), bytes);
}

#[test]
fn rtps_9_3_2_10_the_sequence_number_is_low_plus_high_times_two_to_the_32() {
    let sn = SequenceNumber(5 * (1_i64 << 32) + 7);
    assert_eq!(sn.split(), (5, 7));
    assert_eq!(SequenceNumber::from_high_low(5, 7), sn);
    assert_eq!(
        SequenceNumber::UNKNOWN.split(),
        (-1, 0),
        "SEQUENCENUMBER_UNKNOWN"
    );
}

#[test]
fn rtps_9_6_4_9_lifecycle_kinds_and_their_status_bits() {
    assert_eq!((DISPOSED, UNREGISTERED), (0x1, 0x2));
    // The one place the service maps kinds to bits; exhaustive so that a
    // new kind fails here instead of going out as 0.
    let bits = |kind: ChangeKind| match kind {
        ChangeKind::NotAliveDisposed => DISPOSED,
        ChangeKind::NotAliveUnregistered => UNREGISTERED,
        ChangeKind::NotAliveDisposedUnregistered => DISPOSED | UNREGISTERED,
        ChangeKind::Alive | ChangeKind::AliveFiltered => 0,
    };
    assert_eq!(bits(ChangeKind::NotAliveDisposedUnregistered), 0x3);
}
