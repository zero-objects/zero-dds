// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Additional tests from the WP-1.7 review (B6).
//!
//! The tests live bundled here instead of in individual modules, so
//! the review findings (#19–#25) are easy to follow; in a "normal"
//! refactoring pass they would move into the respective modules.

#![allow(
    clippy::unwrap_used,
    clippy::unreachable,
    clippy::field_reassign_with_default
)]

// ---------- #21 WriterQos-Default pinning ----------

#[test]
fn writer_qos_default_is_reliable() {
    use crate::policies::{ReliabilityKind, WriterQos};
    assert_eq!(
        WriterQos::default().reliability.kind,
        ReliabilityKind::Reliable
    );
}

#[test]
fn reader_qos_default_is_best_effort() {
    use crate::policies::{ReaderQos, ReliabilityKind};
    assert_eq!(
        ReaderQos::default().reliability.kind,
        ReliabilityKind::BestEffort
    );
}

// ---------- #19 BE-Endianness-Parity ----------

#[test]
fn durability_roundtrip_both_endian() {
    use crate::policies::{DurabilityKind, DurabilityQosPolicy};
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

    for endian in [Endianness::Little, Endianness::Big] {
        let p = DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        };
        let mut w = BufferWriter::new(endian);
        p.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, endian);
        assert_eq!(DurabilityQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}

#[test]
fn history_roundtrip_big_endian() {
    use crate::policies::{HistoryKind, HistoryQosPolicy};
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

    let p = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 0x1234_5678,
    };
    let mut w = BufferWriter::new(Endianness::Big);
    p.encode_into(&mut w).unwrap();
    let bytes = w.into_bytes();
    let mut r = BufferReader::new(&bytes, Endianness::Big);
    assert_eq!(HistoryQosPolicy::decode_from(&mut r).unwrap(), p);
}

// ---------- #20 Partition-Mismatch-Aggregate + Stable Reasons ----------

#[test]
fn partition_mismatch_in_aggregate() {
    use crate::compatibility::{CompatibilityResult, IncompatibleReason};
    use crate::policies::{PartitionQosPolicy, ReaderQos, WriterQos, check_compatibility};
    use alloc::string::String;

    let mut writer = WriterQos::default();
    writer.partition = PartitionQosPolicy {
        names: alloc::vec![String::from("sensor_data")],
    };
    let mut reader = ReaderQos::default();
    reader.partition = PartitionQosPolicy {
        names: alloc::vec![String::from("telemetry")],
    };
    let res = check_compatibility(&writer, &reader);
    match res {
        CompatibilityResult::Incompatible(reasons) => {
            assert!(reasons.contains(&IncompatibleReason::Partition));
        }
        CompatibilityResult::Compatible => {
            // If both are non-empty but disjoint, they do not match.
            unreachable!("disjoint partitions must not be compatible");
        }
    }
}

#[test]
fn reasons_are_deduplicated_and_stable() {
    use crate::compatibility::{CompatibilityResult, IncompatibleReason};

    let r = CompatibilityResult::from_reasons(alloc::vec![
        IncompatibleReason::Reliability,
        IncompatibleReason::Durability,
        IncompatibleReason::Reliability,
        IncompatibleReason::Durability,
    ]);
    match r {
        CompatibilityResult::Incompatible(reasons) => {
            assert_eq!(reasons.len(), 2);
            // Canonical order: Durability < Reliability (decl order).
            assert_eq!(reasons[0], IncompatibleReason::Durability);
            assert_eq!(reasons[1], IncompatibleReason::Reliability);
        }
        CompatibilityResult::Compatible => unreachable!(),
    }
}

// ---------- #22 Duration negative ----------

#[test]
fn duration_from_millis_negative_half_second() {
    use crate::duration::Duration;

    // -500 ms: seconds=-1, fraction=0.5 → (-1 + 0.5) = -0.5.
    let d = Duration::from_millis(-500);
    assert_eq!(d.seconds, -1);
    // 0.5 * 2^32 = 2_147_483_648
    assert_eq!(d.fraction, 2_147_483_648);
}

#[test]
fn duration_from_millis_negative_1500() {
    use crate::duration::Duration;

    // -1500 ms: seconds=-2, fraction=0.5 → (-2 + 0.5) = -1.5.
    let d = Duration::from_millis(-1500);
    assert_eq!(d.seconds, -2);
    assert_eq!(d.fraction, 2_147_483_648);
}

#[test]
fn duration_from_millis_exact_negative_second() {
    use crate::duration::Duration;

    let d = Duration::from_millis(-1000);
    assert_eq!(d.seconds, -1);
    assert_eq!(d.fraction, 0);
}

// ---------- #23 try_from_u32 OOB extreme values ----------

#[test]
fn try_from_u32_rejects_extreme_values() {
    use crate::policies::{
        DestinationOrderKind, DurabilityKind, HistoryKind, LivelinessKind, OwnershipKind,
        PresentationAccessScope, ReliabilityKind,
    };

    for v in [4u32, 99, 0x8000_0000, u32::MAX] {
        assert_eq!(DurabilityKind::try_from_u32(v), None);
    }
    for v in [0u32, 3, u32::MAX] {
        assert_eq!(ReliabilityKind::try_from_u32(v), None);
    }
    for v in [2u32, 99, u32::MAX] {
        assert_eq!(HistoryKind::try_from_u32(v), None);
    }
    for v in [3u32, 99, u32::MAX] {
        assert_eq!(LivelinessKind::try_from_u32(v), None);
    }
    for v in [2u32, u32::MAX] {
        assert_eq!(OwnershipKind::try_from_u32(v), None);
    }
    for v in [2u32, u32::MAX] {
        assert_eq!(DestinationOrderKind::try_from_u32(v), None);
    }
    for v in [3u32, u32::MAX] {
        assert_eq!(PresentationAccessScope::try_from_u32(v), None);
    }
}

#[test]
fn strict_decoder_rejects_unknown_discriminator() {
    use crate::policies::{DurabilityQosPolicy, ReliabilityQosPolicy};
    use zerodds_cdr::{BufferReader, DecodeError, Endianness};

    let bad = 99u32.to_le_bytes();
    let mut r = BufferReader::new(&bad, Endianness::Little);
    let err = DurabilityQosPolicy::decode_from(&mut r).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::InvalidEnum {
            kind: "DurabilityKind",
            value: 99,
        }
    ));

    // Reliability
    let mut bad2 = alloc::vec::Vec::new();
    bad2.extend_from_slice(&99u32.to_le_bytes());
    bad2.extend_from_slice(&[0u8; 8]); // Duration fills bytes
    let mut r = BufferReader::new(&bad2, Endianness::Little);
    let err = ReliabilityQosPolicy::decode_from(&mut r).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::InvalidEnum {
            kind: "ReliabilityKind",
            ..
        }
    ));
}

// ---------- #25 PID numeric table ----------

#[test]
fn pid_values_match_spec() {
    use crate::pid::Pid;

    // Values from RTPS 2.5 §9.6.3 Table 9.9 (or equivalent DDS 1.4).
    assert_eq!(Pid::USER_DATA, 0x002c);
    assert_eq!(Pid::TOPIC_DATA, 0x002e);
    assert_eq!(Pid::GROUP_DATA, 0x002d);
    assert_eq!(Pid::DURABILITY, 0x001d);
    assert_eq!(Pid::DURABILITY_SERVICE, 0x001e);
    assert_eq!(Pid::DEADLINE, 0x0023);
    assert_eq!(Pid::LATENCY_BUDGET, 0x0027);
    assert_eq!(Pid::LIVELINESS, 0x001b);
    assert_eq!(Pid::RELIABILITY, 0x001a);
    assert_eq!(Pid::LIFESPAN, 0x002b);
    assert_eq!(Pid::DESTINATION_ORDER, 0x0025);
    assert_eq!(Pid::HISTORY, 0x0040);
    assert_eq!(Pid::RESOURCE_LIMITS, 0x0041);
    assert_eq!(Pid::OWNERSHIP, 0x001f);
    assert_eq!(Pid::OWNERSHIP_STRENGTH, 0x0006);
    assert_eq!(Pid::PRESENTATION, 0x0021);
    assert_eq!(Pid::PARTITION, 0x0029);
    assert_eq!(Pid::TIME_BASED_FILTER, 0x0004);
    assert_eq!(Pid::TRANSPORT_PRIORITY, 0x0049);
    // Vendor flag set (MSB) for non-standardized PIDs.
    assert_eq!(Pid::READER_DATA_LIFECYCLE & 0x8000, 0x8000);
    assert_eq!(Pid::WRITER_DATA_LIFECYCLE & 0x8000, 0x8000);
    assert_eq!(Pid::SENTINEL, 0x0001);
}

// ---------- #7 Consistency-Check ----------

#[test]
fn writer_qos_consistency_catches_negative_history_depth() {
    use crate::policies::qos_set::InconsistentReason;
    use crate::policies::{HistoryKind, WriterQos};

    let mut q = WriterQos::default();
    q.history.kind = HistoryKind::KeepLast;
    q.history.depth = 0;
    assert_eq!(q.check_consistency(), Err(InconsistentReason::HistoryDepth));
}

#[test]
fn reader_qos_consistency_catches_filter_gt_deadline() {
    use crate::duration::Duration;
    use crate::policies::ReaderQos;
    use crate::policies::qos_set::InconsistentReason;

    let mut q = ReaderQos::default();
    q.deadline.period = Duration::from_secs(1);
    q.time_based_filter.minimum_separation = Duration::from_secs(5);
    assert_eq!(
        q.check_consistency(),
        Err(InconsistentReason::FilterVsDeadline)
    );
}
