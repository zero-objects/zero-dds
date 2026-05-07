// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bridge von SEDP-BuiltinTopicData (Wire) zu zerodds-qos-Policies.
//!
//! `as_writer_qos()` / `as_reader_qos()` ziehen die wire-getragenen
//! QoS-Felder auf eine vollständige `WriterQos`/`ReaderQos`-Form hoch;
//! restliche Policies bleiben auf Default.

use crate::publication_data::PublicationBuiltinTopicData;
use crate::subscription_data::SubscriptionBuiltinTopicData;

use zerodds_qos::{DurabilityQosPolicy, ReaderQos, WriterQos};

// ---------- BuiltinTopicData → Qos-Aggregate ----------
//
// **Wichtig:** `PublicationBuiltinTopicData` /
// `SubscriptionBuiltinTopicData` tragen aktuell nur eine Teilmenge
// der QoS auf Wire (Durability, Reliability). Die übrigen Policies
// (Deadline, Liveliness, Partition, Ownership, …) bleiben auf den
// zerodds-qos-Defaults, wenn sie nicht explizit gesetzt werden.
//
// Effekt auf `zerodds_qos::check_compatibility`: wenn ein Peer real
// eine strenge Deadline requestet, wir aber den Default INFINITE
// annehmen, meldet der Check "Compatible", obwohl die reale Wire-
// Verbindung den OFFERED_INCOMPATIBLE_QOS-Listener triggern würde.
//
// Für lokal konstruierte QoS (z.B. im DCPS-Layer) können Anwendungen
// die `with_*`-Helpers nutzen, um vollständige QoS in den Bridge-
// Typen mitzuführen.

impl PublicationBuiltinTopicData {
    /// Baut aus den Wire-Felder eine `WriterQos`.
    ///
    /// **Einschraenkung:** Nur Durability + Reliability werden aus
    /// `self` uebernommen; alle anderen Policies bleiben auf ihren
    /// `WriterQos::default()`-Werten. Anwendungen, die gegen den
    /// discovered Peer matchen wollen, muessen dieser Einschraenkung
    /// bewusst sein — siehe Modul-Dokumentation.
    #[must_use]
    pub fn as_writer_qos(&self) -> WriterQos {
        WriterQos {
            durability: DurabilityQosPolicy {
                kind: self.durability,
            },
            reliability: self.reliability,
            ..WriterQos::default()
        }
    }

    /// Wendet eine vollstaendige `WriterQos` auf diesen Builtin-Topic-
    /// Data-Payload an, soweit Wire-Felder es erlauben.
    /// Policies, die (noch) nicht serialisiert werden, gehen verloren.
    #[must_use]
    pub fn with_writer_qos(mut self, qos: &WriterQos) -> Self {
        self.durability = qos.durability.kind;
        self.reliability = qos.reliability;
        self
    }
}

impl SubscriptionBuiltinTopicData {
    /// Analog [`PublicationBuiltinTopicData::as_writer_qos`] fuer Reader.
    ///
    /// **Einschraenkung:** Nur Durability + Reliability; uebrige
    /// Policies auf `ReaderQos::default()`.
    #[must_use]
    pub fn as_reader_qos(&self) -> ReaderQos {
        ReaderQos {
            durability: DurabilityQosPolicy {
                kind: self.durability,
            },
            reliability: self.reliability,
            ..ReaderQos::default()
        }
    }

    /// Analog [`PublicationBuiltinTopicData::with_writer_qos`] fuer Reader.
    #[must_use]
    pub fn with_reader_qos(mut self, qos: &ReaderQos) -> Self {
        self.durability = qos.durability.kind;
        self.reliability = qos.reliability;
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
        // Nach der // Der Test haelt diese Invariante fest — wenn jemand die
        // Typen wieder duplizieren wollte, bricht er hier.
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
        };
        let wq = pub_data.as_writer_qos();
        let rq = sub_data.as_reader_qos();
        assert!(zerodds_qos::check_compatibility(&wq, &rq).is_compatible());
    }

    /// #24 Round-2-Review: bridged negative-compatibility test.
    ///
    /// BestEffort-Writer (discovered) + Reliable-Reader (lokal) darf nach
    /// Bridge-Konvertierung NICHT kompatibel sein — sonst verschleiert die
    /// Bridge einen echten QoS-Mismatch.
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
        };
        let wq = pub_data.as_writer_qos();
        let rq = sub_data.as_reader_qos();
        let res = zerodds_qos::check_compatibility(&wq, &rq);
        assert!(!res.is_compatible());
    }
}
