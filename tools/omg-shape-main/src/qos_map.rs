// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Flag → QoS mapping. Wires the `shape_main` CLI flags onto ZeroDDS's
//! **existing** QoS machinery (`crates/qos/src/policies/*`,
//! `crates/dcps/src/qos.rs`) — this module writes no new QoS/wire code,
//! only the CLI-to-struct translation.

use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DurabilityKind, DurabilityQosPolicy, HistoryKind,
    HistoryQosPolicy, OwnershipKind, OwnershipQosPolicy, OwnershipStrengthQosPolicy,
    PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy, PublisherQos,
    ReliabilityKind, ReliabilityQosPolicy, SubscriberQos, TimeBasedFilterQosPolicy,
};
use zerodds_qos::Duration;

use crate::cli::{AccessScopeArg, DurabilityArg, Options};

/// `-x [1|2]` → XTypes 1.3 §7.6.3.1.2 DataRepresentation id list.
/// `1` = XCDR (id `0`), `2` = XCDR2 (id `2`) — matches the
/// `[XCDR1, XCDR2]` / `Some(vec![0_i16, 2])` convention already used
/// across the dcps crate (see `crates/dcps/src/qos.rs`
/// `data_representation_defaults_none_and_settable`).
fn data_representation_list(opt: Option<u8>) -> Option<Vec<i16>> {
    match opt {
        Some(1) => Some(vec![0_i16]),
        Some(2) => Some(vec![2_i16]),
        _ => None,
    }
}

fn durability_kind(d: Option<DurabilityArg>) -> DurabilityKind {
    match d {
        Some(DurabilityArg::V) | None => DurabilityKind::Volatile,
        Some(DurabilityArg::L) => DurabilityKind::TransientLocal,
        Some(DurabilityArg::T) => DurabilityKind::Transient,
        Some(DurabilityArg::P) => DurabilityKind::Persistent,
    }
}

fn access_scope(a: Option<AccessScopeArg>) -> PresentationAccessScope {
    match a {
        Some(AccessScopeArg::I) | None => PresentationAccessScope::Instance,
        Some(AccessScopeArg::T) => PresentationAccessScope::Topic,
        Some(AccessScopeArg::G) => PresentationAccessScope::Group,
    }
}

fn history(depth: Option<i32>) -> Option<HistoryQosPolicy> {
    depth.map(|d| {
        if d <= 0 {
            HistoryQosPolicy {
                kind: HistoryKind::KeepAll,
                depth: i32::MAX,
            }
        } else {
            HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: d,
            }
        }
    })
}

fn partition(p: &Option<String>) -> Option<PartitionQosPolicy> {
    p.as_ref().map(|name| PartitionQosPolicy {
        names: vec![name.clone()],
    })
}

/// Builds the `DataWriterQos` for `-P` from the parsed flags.
#[must_use]
pub fn writer_qos(opts: &Options) -> DataWriterQos {
    let mut qos = DataWriterQos::default();
    if opts.reliable {
        qos.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_millis(100),
        };
    } else if opts.best_effort {
        qos.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: Duration::ZERO,
        };
    }
    qos.durability = DurabilityQosPolicy {
        kind: durability_kind(opts.durability),
    };
    if let Some(h) = history(opts.history_depth) {
        qos.history = h;
    }
    if opts.deadline_ms > 0 {
        qos.deadline.period =
            Duration::from_millis(i32::try_from(opts.deadline_ms).unwrap_or(i32::MAX));
    }
    if opts.lifespan_ms > 0 {
        qos.lifespan.duration =
            Duration::from_millis(i32::try_from(opts.lifespan_ms).unwrap_or(i32::MAX));
    }
    if opts.ownership_strength >= 0 {
        qos.ownership = OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        };
        qos.ownership_strength = OwnershipStrengthQosPolicy {
            value: opts.ownership_strength,
        };
    }
    if let Some(part) = partition(&opts.partition) {
        qos.partition = part;
    }
    qos.data_representation = data_representation_list(opts.data_representation);
    qos
}

/// Builds the `DataReaderQos` for `-S` from the parsed flags.
#[must_use]
pub fn reader_qos(opts: &Options) -> DataReaderQos {
    let mut qos = DataReaderQos::default();
    if opts.reliable {
        qos.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_millis(100),
        };
    } else if opts.best_effort {
        qos.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: Duration::ZERO,
        };
    }
    qos.durability = DurabilityQosPolicy {
        kind: durability_kind(opts.durability),
    };
    if let Some(h) = history(opts.history_depth) {
        qos.history = h;
    }
    if opts.deadline_ms > 0 {
        qos.deadline.period =
            Duration::from_millis(i32::try_from(opts.deadline_ms).unwrap_or(i32::MAX));
    }
    if opts.ownership_strength >= 0 {
        qos.ownership = OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        };
    }
    if opts.time_filter_ms > 0 {
        qos.time_based_filter = TimeBasedFilterQosPolicy {
            minimum_separation: Duration::from_millis(
                i32::try_from(opts.time_filter_ms).unwrap_or(i32::MAX),
            ),
        };
    }
    if let Some(part) = partition(&opts.partition) {
        qos.partition = part;
    }
    qos.data_representation = data_representation_list(opts.data_representation);
    qos
}

/// Builds the `PublisherQos` (Presentation lives at Publisher/Subscriber
/// level per DDS 1.4 §2.2.3.6 — not per-writer/-reader).
#[must_use]
pub fn publisher_qos(opts: &Options) -> PublisherQos {
    PublisherQos {
        presentation: PresentationQosPolicy {
            access_scope: access_scope(opts.access_scope),
            coherent_access: opts.coherent,
            ordered_access: opts.ordered,
        },
        ..PublisherQos::default()
    }
}

/// Builds the `SubscriberQos` (mirrors [`publisher_qos`]).
#[must_use]
pub fn subscriber_qos(opts: &Options) -> SubscriberQos {
    SubscriberQos {
        presentation: PresentationQosPolicy {
            access_scope: access_scope(opts.access_scope),
            coherent_access: opts.coherent,
            ordered_access: opts.ordered,
        },
        ..SubscriberQos::default()
    }
}
