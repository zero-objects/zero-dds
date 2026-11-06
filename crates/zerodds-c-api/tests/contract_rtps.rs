// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps`, from its non-test sources:
//! - `wire_types::EntityId` as the writer/reader handle (`entities.rs`,
//!   `publisher_ffi.rs`), built with `EntityId::user_reader_with_key` /
//!   `user_reader_no_key` (`subscriber_ffi.rs`); `entity_kind` compared with
//!   `EntityKind::UserWriterNoKey` (`lib.rs`, `publisher_ffi.rs`) and
//!   classified with `EntityKind::topic_kind` against `TopicKind::WithKey` /
//!   `NoKey`; group entity ids asked for with `EntityKind::UserWriterGroup` /
//!   `UserReaderGroup` (`participant_ffi.rs`).
//! - `wire_types::Guid::new` + `to_bytes` for instance handles and the
//!   writer GUID (`entity_ffi.rs`, `extra_ffi.rs`), `Guid::from_bytes` for
//!   the reader GUIDs a caller passes as 16 octets (`lib.rs`),
//!   `GuidPrefix::from_bytes` for the identity-adjusted prefix and an
//!   all-zero fallback (`security_ffi.rs`, `entity_ffi.rs`),
//!   `VendorId::ZERODDS` for the security builtins (`security_ffi.rs`).
//! - `inline_qos::status_info::{DISPOSED, UNREGISTERED}` as the dispose /
//!   unregister bits it writes (`lib.rs`), `history_cache::ChangeKind`
//!   (`NotAliveDisposed`, `NotAliveUnregistered`,
//!   `NotAliveDisposedUnregistered`) matched on received samples
//!   (`subscriber_ffi.rs`).
//! - `inline_qos::SampleIdentityBytes::new(writer_guid, sn)` for the related
//!   sample identity of an RPC reply (`lib.rs`).
//! - `publication_data::data_representation::{XCDR, XCDR2}`: the offer
//!   `[XCDR2, XCDR]` (`lib.rs`) and the mapping of the C values 0 and 2
//!   (`rpc_qos_ffi.rs`).
//! - `subscription_data::ContentFilterProperty`, built field by field with
//!   `filter_class::DDSSQL` and cloned per reader (`lib.rs`,
//!   `participant_ffi.rs`, `subscriber_ffi.rs`, `entity_ffi.rs`).
//! - `writer_group::WriterGroup` shared as `Arc` by the writers of a
//!   GROUP-scope publisher: `new`, `set_writers`, `begin_coherent_set`,
//!   `end_coherent_set` (`publisher_ffi.rs`).
//! - `header_extension::HeTimestamp` as the source timestamp, converted from
//!   and to the DCPS time by `zerodds_dcps::time` (`publisher_ffi.rs`,
//!   `extra_ffi.rs`, `subscriber_ffi.rs`).
//!
//! Oracles: DDSI-RTPS 2.5 §8.2.1.2 (ChangeKind_t), §8.7.3 (content filter
//! property), §9.3.1.1/§9.3.1.2/§9.3.1.5 (GUID mapping, Table 9.1),
//! §9.3.2.1 (Time_t), §9.3.2.5 (GroupDigest_t, MD5 computed outside this
//! code), §9.3.2.10 (SequenceNumber_t), §9.6.4.3 Table 9.23 (group coherent
//! set), §9.6.4.9 (StatusInfo_t); DDS-XTypes 1.3 §7.4
//! (DataRepresentationId_t). No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use zerodds_rtps::group_digest::GroupDigest;
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::inline_qos::{SampleIdentityBytes, status_info};
use zerodds_rtps::publication_data::data_representation as dr;
use zerodds_rtps::subscription_data::{ContentFilterProperty, filter_class};
use zerodds_rtps::wire_types::{EntityId, EntityKind, Guid, GuidPrefix, TopicKind, VendorId};
use zerodds_rtps::writer_group::WriterGroup;

#[test]
fn rtps_9_3_1_2_entity_kinds_the_api_compares_have_the_values_of_table_9_1() {
    for (kind, octet) in [
        (EntityKind::UserWriterNoKey, 0x03),
        (EntityKind::UserWriterWithKey, 0x02),
        (EntityKind::UserReaderNoKey, 0x04),
        (EntityKind::UserReaderWithKey, 0x07),
        (EntityKind::UserWriterGroup, 0x08),
        (EntityKind::UserReaderGroup, 0x09),
    ] {
        assert_eq!(kind.to_byte(), octet, "{kind:?}");
        assert_eq!(EntityKind::from_byte(octet), kind);
    }
    assert_eq!(
        EntityId::user_reader_with_key([9, 9, 9]).to_bytes(),
        [9, 9, 9, 0x07]
    );
    assert_eq!(
        EntityId::user_reader_no_key([9, 9, 9]).to_bytes(),
        [9, 9, 9, 0x04]
    );
}

#[test]
fn rtps_9_3_1_2_topic_kind_follows_the_key_part_of_the_entity_kind() {
    assert_eq!(
        EntityId::user_reader_with_key([1, 2, 3])
            .entity_kind
            .topic_kind(),
        Some(TopicKind::WithKey)
    );
    assert_eq!(
        EntityId::user_reader_no_key([1, 2, 3])
            .entity_kind
            .topic_kind(),
        Some(TopicKind::NoKey)
    );
    assert_eq!(
        EntityId::user_writer_no_key([1, 2, 3])
            .entity_kind
            .topic_kind(),
        Some(TopicKind::NoKey)
    );
    // A group is neither: the API treats it as "not keyed".
    assert_eq!(EntityKind::UserWriterGroup.topic_kind(), None);
    assert_eq!(EntityKind::UserReaderGroup.topic_kind(), None);
}

#[test]
fn rtps_9_3_1_5_guid_is_the_prefix_then_the_entity_id() {
    let prefix = GuidPrefix::from_bytes([0x01, 0xF0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    let eid = EntityId::user_writer_with_key([0xA, 0xB, 0xC]);
    let bytes = Guid::new(prefix, eid).to_bytes();
    assert_eq!(bytes[..12], prefix.to_bytes());
    assert_eq!(bytes[12..], [0xA, 0xB, 0xC, 0x02]);
    let back = Guid::from_bytes(bytes);
    assert_eq!((back.prefix, back.entity_id), (prefix, eid));
}

#[test]
fn rtps_9_3_1_1_the_all_zero_fallback_prefix_is_guidprefix_unknown() {
    assert_eq!(GuidPrefix::from_bytes([0; 12]), GuidPrefix::UNKNOWN);
    assert_eq!(GuidPrefix::from_bytes([7; 12]).to_bytes(), [7; 12]);
}

#[test]
fn the_security_builtins_announce_the_zerodds_vendor_id() {
    // The interim value ZeroDDS announces until the OMG assigns one; it
    // must not be another vendor's assigned id.
    assert_eq!(VendorId::ZERODDS.to_bytes(), [0x01, 0xF0]);
    for other in [
        VendorId::RTI_CONNEXT,
        VendorId::OPENDDS,
        VendorId::FASTDDS,
        VendorId::CYCLONE,
    ] {
        assert_ne!(VendorId::ZERODDS, other);
    }
}

#[test]
fn rtps_9_6_4_9_dispose_and_unregister_are_the_d_and_u_flags() {
    assert_eq!(status_info::DISPOSED, 0x0000_0001);
    assert_eq!(status_info::UNREGISTERED, 0x0000_0002);
    assert_eq!(
        status_info::DISPOSED | status_info::UNREGISTERED,
        0x0000_0003
    );
}

#[test]
fn rtps_8_2_1_2_the_not_alive_change_kinds_are_told_apart() {
    let kinds = [
        ChangeKind::Alive,
        ChangeKind::NotAliveDisposed,
        ChangeKind::NotAliveUnregistered,
        ChangeKind::NotAliveDisposedUnregistered,
    ];
    for (i, a) in kinds.iter().enumerate() {
        for (j, b) in kinds.iter().enumerate() {
            assert_eq!(i == j, a == b, "{a:?} vs {b:?}");
        }
    }
}

#[test]
fn rtps_9_3_2_10_sample_identity_keeps_the_writer_guid_and_the_64_bit_number() {
    let guid = [0x11; 16];
    let sn: u64 = (3 << 32) | 7;
    let identity = SampleIdentityBytes::new(guid, sn);
    assert_eq!(identity.writer_guid, guid);
    assert_eq!(identity.sequence_number, sn);
    // On the wire: the GUID, then SequenceNumber_t {long high; unsigned
    // long low} with sequence_number = low + high * 2^32.
    let be = identity.to_bytes(false);
    assert_eq!(be[..16], guid);
    assert_eq!(be[16..20], 3_i32.to_be_bytes());
    assert_eq!(be[20..24], 7_u32.to_be_bytes());
    let le = identity.to_bytes(true);
    assert_eq!(le[16..20], 3_i32.to_le_bytes());
    assert_eq!(le[20..24], 7_u32.to_le_bytes());
}

#[test]
fn xtypes_7_4_data_representation_ids_are_0_1_2() {
    let xcdr: i16 = dr::XCDR;
    let xcdr2: i16 = dr::XCDR2;
    assert_eq!((xcdr, dr::XML, xcdr2), (0, 1, 2));
}

#[test]
fn rtps_8_7_3_content_filter_property_carries_the_five_fields_and_ddssql() {
    assert_eq!(filter_class::DDSSQL, "DDSSQL");
    let property = ContentFilterProperty {
        content_filtered_topic_name: "SquareFiltered".into(),
        related_topic_name: "Square".into(),
        filter_class_name: filter_class::DDSSQL.into(),
        filter_expression: "x > %0".into(),
        expression_parameters: vec!["10".into()],
    };
    let per_reader = property.clone();
    assert_eq!(per_reader, property);
    assert_eq!(per_reader.expression_parameters, ["10"]);
}

#[test]
fn rtps_9_6_4_3_writer_group_numbers_follow_table_9_23() {
    let writers = [
        EntityId::user_writer_with_key([0, 0, 1]),
        EntityId::user_writer_with_key([0, 1, 0]),
    ];
    let group = Arc::new(WriterGroup::new());
    group.set_writers(&writers);
    for _ in 0..10 {
        let _ = group.stamp_change();
    }
    group.begin_coherent_set();
    let shared = Arc::clone(&group);
    let gsns: Vec<i64> =
        std::thread::spawn(move || (0..3).map(|_| shared.stamp_change().gsn).collect())
            .join()
            .unwrap();
    assert_eq!(gsns, [11, 12, 13]);
    let end = group.end_coherent_set().expect("a set with three changes");
    assert_eq!((end.first_gsn, end.ecs_gsn), (11, 14));
    // §9.3.2.5: MD5 of the CDR big-endian EntityIdSet_t, ids sorted as
    // little-endian int32: {00 01 00 02, 00 00 01 02}; computed with
    // Python's hashlib, leading four octets.
    assert_eq!(end.writer_set, GroupDigest([0xd4, 0xfa, 0xc0, 0x30]));
}

#[test]
fn a_group_coherent_set_without_a_change_has_no_end() {
    let group = WriterGroup::new();
    group.begin_coherent_set();
    assert_eq!(group.end_coherent_set(), None);
    assert_eq!(
        group.end_coherent_set(),
        None,
        "and no set is open after it"
    );
}

#[test]
fn rtps_9_3_2_1_source_timestamp_fraction_counts_two_to_the_minus_32_seconds() {
    use zerodds_dcps::time::{Time, he_timestamp_to_time, time_to_he_timestamp};
    let ts = time_to_he_timestamp(Time::new(1, 500_000_000));
    assert_eq!((ts.seconds, ts.fraction), (1, 1 << 31));
    assert_eq!(he_timestamp_to_time(ts), Time::new(1, 500_000_000));
    // TIME_INVALID {0xffffffff, 0xffffffff}: what a missing timestamp maps to.
    let invalid = zerodds_rtps::header_extension::HeTimestamp::TIME_INVALID;
    assert_eq!((invalid.seconds, invalid.fraction), (u32::MAX, u32::MAX));
    assert_eq!(time_to_he_timestamp(Time::INVALID), invalid);
}

#[test]
fn the_handles_the_api_keeps_are_plain_shareable_values() {
    fn value<T: Copy + Eq + std::fmt::Debug + Send + Sync + 'static>() {}
    value::<EntityId>();
    value::<Guid>();
    value::<GuidPrefix>();
    value::<ChangeKind>();
    value::<zerodds_rtps::header_extension::HeTimestamp>();
    fn shared<T: Send + Sync>() {}
    shared::<WriterGroup>();
}
