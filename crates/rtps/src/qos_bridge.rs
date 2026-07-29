// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bridge from SEDP BuiltinTopicData (wire) to zerodds-qos policies.
//!
//! `as_writer_qos()` / `as_reader_qos()` lift the wire-carried QoS
//! fields up to a full `WriterQos`/`ReaderQos` form; the remaining
//! policies stay at their defaults.

use crate::publication_data::PublicationBuiltinTopicData;
use crate::subscription_data::SubscriptionBuiltinTopicData;

use zerodds_qos::{DurabilityQosPolicy, ReaderQos, WriterQos};

// ---------- BuiltinTopicData → Qos-Aggregate ----------
//
// **Important:** `PublicationBuiltinTopicData` /
// `SubscriptionBuiltinTopicData` currently carry only a subset of the
// QoS on the wire (durability, reliability, presentation). The remaining
// policies (deadline, liveliness, partition, ownership, …) stay at the
// zerodds-qos defaults if they are not set explicitly.
//
// Effect on `zerodds_qos::check_compatibility`: if a peer actually
// requests a strict deadline but we assume the default INFINITE, the
// check reports "Compatible" even though the real wire connection
// would trigger the OFFERED_INCOMPATIBLE_QOS listener.
//
// For locally constructed QoS (e.g. in the DCPS layer), applications
// can use the `with_*` helpers to carry a full QoS along in the bridge
// types.

impl PublicationBuiltinTopicData {
    /// Builds a `WriterQos` from the wire fields.
    ///
    /// **Limitation:** only durability + reliability + presentation are
    /// taken from `self`; all other policies stay at their
    /// `WriterQos::default()` values. Applications that want to match
    /// against the discovered peer must be aware of this limitation —
    /// see the module documentation.
    #[must_use]
    pub fn as_writer_qos(&self) -> WriterQos {
        WriterQos {
            durability: DurabilityQosPolicy {
                kind: self.durability,
            },
            reliability: self.reliability,
            presentation: self.presentation,
            ..WriterQos::default()
        }
    }

    /// Applies a full `WriterQos` to this builtin-topic-data payload,
    /// as far as the wire fields allow.
    /// Policies not (yet) serialized are lost.
    #[must_use]
    pub fn with_writer_qos(mut self, qos: &WriterQos) -> Self {
        self.durability = qos.durability.kind;
        self.reliability = qos.reliability;
        self.presentation = qos.presentation;
        self
    }
}

impl SubscriptionBuiltinTopicData {
    /// Analogous to [`PublicationBuiltinTopicData::as_writer_qos`] for readers.
    ///
    /// **Limitation:** only durability + reliability + presentation; the
    /// remaining policies at `ReaderQos::default()`.
    #[must_use]
    pub fn as_reader_qos(&self) -> ReaderQos {
        ReaderQos {
            durability: DurabilityQosPolicy {
                kind: self.durability,
            },
            reliability: self.reliability,
            presentation: self.presentation,
            ..ReaderQos::default()
        }
    }

    /// Analogous to [`PublicationBuiltinTopicData::with_writer_qos`] for readers.
    #[must_use]
    pub fn with_reader_qos(mut self, qos: &ReaderQos) -> Self {
        self.durability = qos.durability.kind;
        self.reliability = qos.reliability;
        self.presentation = qos.presentation;
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::unreachable, clippy::panic)]
mod tests {
    use super::*;
    use crate::publication_data::{DurabilityKind, ReliabilityKind, ReliabilityQos};
    use crate::wire_types::{EntityId, Guid, GuidPrefix};
    use zerodds_qos::Duration;

    #[test]
    fn durability_kind_is_reexport_not_duplicate() {
        // The test pins this invariant — if someone wanted to duplicate
        // the types again, it breaks here.
        fn assert_same_type<T>(_a: &T, _b: &T) {}
        let rtps = DurabilityKind::Transient;
        let qos = zerodds_qos::DurabilityKind::Transient;
        assert_same_type(&rtps, &qos);
        assert_eq!(rtps, qos);
    }

    #[test]
    fn reliability_kind_is_reexport_not_duplicate() {
        let rtps = ReliabilityKind::Reliable;
        let qos = zerodds_qos::ReliabilityKind::Reliable;
        assert_eq!(rtps, qos);
    }

    #[test]
    fn duration_is_reexport_not_duplicate() {
        let d = Duration::from_secs(7);
        let qd = zerodds_qos::Duration::from_secs(7);
        assert_eq!(d, qd);
    }

    #[test]
    fn writer_reader_qos_match_by_defaults() {
        let pub_data = PublicationBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0, 0, 1]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: alloc::string::String::from("T"),
            type_name: alloc::string::String::from("X"),
            durability: DurabilityKind::TransientLocal,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration {
                    seconds: 0,
                    fraction: 0,
                },
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec![2],
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let sub_data = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0, 0, 2]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
            topic_name: alloc::string::String::from("T"),
            type_name: alloc::string::String::from("X"),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::BestEffort,
                max_blocking_time: Duration {
                    seconds: 0,
                    fraction: 0,
                },
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec![2],
            content_filter: None,
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let wq = pub_data.as_writer_qos();
        let rq = sub_data.as_reader_qos();
        assert!(zerodds_qos::check_compatibility(&wq, &rq).is_compatible());
    }

    /// #24 Round-2-Review: bridged negative-compatibility test.
    ///
    /// A BestEffort writer (discovered) + reliable reader (local) must
    /// NOT be compatible after bridge conversion — otherwise the bridge
    /// masks a real QoS mismatch.
    #[test]
    fn besteffort_writer_reliable_reader_is_incompatible_via_bridge() {
        let pub_data = PublicationBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0, 0, 1]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: alloc::string::String::from("T"),
            type_name: alloc::string::String::from("X"),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::BestEffort,
                max_blocking_time: Duration {
                    seconds: 0,
                    fraction: 0,
                },
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec![2],
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let sub_data = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0, 0, 2]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
            topic_name: alloc::string::String::from("T"),
            type_name: alloc::string::String::from("X"),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration {
                    seconds: 0,
                    fraction: 0,
                },
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec![2],
            content_filter: None,
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let wq = pub_data.as_writer_qos();
        let rq = sub_data.as_reader_qos();
        let res = zerodds_qos::check_compatibility(&wq, &rq);
        assert!(!res.is_compatible());
        match res {
            zerodds_qos::CompatibilityResult::Incompatible(reasons) => {
                assert!(
                    reasons.contains(&zerodds_qos::IncompatibleReason::Reliability),
                    "expected Reliability reason, got {reasons:?}"
                );
            }
            zerodds_qos::CompatibilityResult::Compatible => {
                unreachable!("BestEffort writer vs Reliable reader must not match")
            }
        }
    }

    /// Durability-Mismatch: Volatile-Writer vs TransientLocal-Reader →
    /// durability-Reason.
    #[test]
    fn volatile_writer_transient_local_reader_incompatible_via_bridge() {
        let pub_data = PublicationBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0, 0, 1]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: alloc::string::String::from("T"),
            type_name: alloc::string::String::from("X"),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration {
                    seconds: 0,
                    fraction: 0,
                },
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec![2],
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let sub_data = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0, 0, 2]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
            topic_name: alloc::string::String::from("T"),
            type_name: alloc::string::String::from("X"),
            durability: DurabilityKind::TransientLocal,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::BestEffort,
                max_blocking_time: Duration {
                    seconds: 0,
                    fraction: 0,
                },
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec![2],
            content_filter: None,
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let wq = pub_data.as_writer_qos();
        let rq = sub_data.as_reader_qos();
        let res = zerodds_qos::check_compatibility(&wq, &rq);
        assert!(!res.is_compatible());
    }
}
