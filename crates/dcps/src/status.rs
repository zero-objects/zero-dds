// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Communication-Status-Strukturen (DDS DCPS 1.4 §2.2.4.1, Tab. 2.10).
//!
//! Die Spec definiert **13 Standard-Communication-Statuses**, die als
//! Bitmask in `StatusMask` zusammengeführt werden. Jeder Status hat eine
//! zugeordnete Datenstruktur, die `get_<status>()` auf der jeweiligen
//! Entity zurückliefert. Die Bitmask-Konstanten (zur Verbindung Status ↔
//! Bit) leben in [`crate::psm_constants::status`].
//!
//! ## Klassifikation der 13 Statuses (Spec §2.2.4.1):
//!
//! - **PLAIN**-Statuses (nur `total_count` + `total_count_change`):
//!   - `INCONSISTENT_TOPIC`         (Topic)
//!   - `SAMPLE_LOST`                (DataReader)
//!   - `LIVELINESS_LOST`            (DataWriter)
//!   - `OFFERED_DEADLINE_MISSED`    (DataWriter)
//!   - `REQUESTED_DEADLINE_MISSED`  (DataReader)
//!
//! - **STATEFUL**-Statuses (mit `last_*` + Detail-Feldern):
//!   - `SAMPLE_REJECTED`            (DataReader)
//!   - `LIVELINESS_CHANGED`         (DataReader)
//!   - `PUBLICATION_MATCHED`        (DataWriter)
//!   - `SUBSCRIPTION_MATCHED`       (DataReader)
//!   - `OFFERED_INCOMPATIBLE_QOS`   (DataWriter)
//!   - `REQUESTED_INCOMPATIBLE_QOS` (DataReader)
//!
//! - **SIGNAL**-Statuses (rein als Bit, ohne Datenstruktur — wir
//!   spiegeln sie als Marker-Structs für die Vollständigkeit der
//!   Tabelle und für eine einheitliche `get_*`-API):
//!   - `DATA_AVAILABLE`             (DataReader)
//!   - `DATA_ON_READERS`            (Subscriber)
//!
//! `total_count_change` ist als `i32` getypt: die Spec sagt
//! "incremental count since the last time the listener was called or
//! the status was read", was negativ werden kann, wenn der Reader
//! Liveliness wieder gewinnt (LIVELINESS_CHANGED). Wir bleiben bei
//! `i32` für alle `*_count_change`-Felder, um spec-konform zu sein.

extern crate alloc;

use alloc::vec::Vec;

use crate::instance_handle::InstanceHandle;

// ============================================================================
// Plain-Counter-Statuses
// ============================================================================

/// `INCONSISTENT_TOPIC_STATUS` — Spec §2.2.4.1 Tab. 2.10 + §2.2.2.3.2.
///
/// "Another topic exists with the same name but different
/// characteristics." Wird auf `Topic`-Ebene gepflegt.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InconsistentTopicStatus {
    /// Total cumulative count of inconsistencies detected.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
}

/// `SAMPLE_LOST_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// "All samples that have been lost (never received) by the
/// DataReader."
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleLostStatus {
    /// Total cumulative count of all lost samples.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
}

/// `LIVELINESS_LOST_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Counter, wie oft der DataWriter aus Sicht des LIVELINESS-QoS-Vertrags
/// als "not alive" deklariert worden ist (Writer-Seite).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivelinessLostStatus {
    /// Total cumulative count of times the writer was declared
    /// not-alive.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
}

/// `OFFERED_DEADLINE_MISSED_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Counter, wie oft der Writer das offered DEADLINE-Versprechen nicht
/// einhalten konnte. Pflegt zusätzlich den `last_instance_handle`,
/// gegen den die Verletzung gezählt wurde.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OfferedDeadlineMissedStatus {
    /// Total cumulative count of offered-deadline misses.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
    /// Handle of the last instance for which the deadline was missed.
    pub last_instance_handle: InstanceHandle,
}

/// `REQUESTED_DEADLINE_MISSED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader-Seite. Counter, wie oft der Reader keine Sample innerhalb des
/// requested DEADLINE bekommen hat.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestedDeadlineMissedStatus {
    /// Total cumulative count of requested-deadline misses.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
    /// Handle of the last instance for which the deadline was missed.
    pub last_instance_handle: InstanceHandle,
}

// ============================================================================
// Stateful-Statuses (mit `last_*` + Detail-Feldern)
// ============================================================================

/// `SampleRejectedStatusKind` — Reason warum der letzte Sample rejected
/// wurde. Spec §2.2.4.1 Tab. 2.10 (kind enum unter `SAMPLE_REJECTED`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleRejectedStatusKind {
    /// Kein Sample wurde rejected (Default).
    #[default]
    NotRejected,
    /// Reader-Resource-Limit `max_instances` überschritten.
    RejectedByInstancesLimit,
    /// Reader-Resource-Limit `max_samples` überschritten.
    RejectedBySamplesLimit,
    /// Reader-Resource-Limit `max_samples_per_instance` überschritten.
    RejectedBySamplesPerInstanceLimit,
}

/// `SAMPLE_REJECTED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Wird ausgelöst, wenn der Reader ein Sample wegen einer
/// RESOURCE_LIMITS-Verletzung verworfen hat.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRejectedStatus {
    /// Total cumulative count of rejected samples.
    pub total_count: i32,
    /// Increment since last read.
    pub total_count_change: i32,
    /// Reason for the most recent rejection.
    pub last_reason: SampleRejectedStatusKind,
    /// Handle of the instance that was the target of the most recent
    /// rejection.
    pub last_instance_handle: InstanceHandle,
}

/// `LIVELINESS_CHANGED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader-Seite: "Reports the status of the liveliness of one or more
/// `DataWriter` objects that are matched with the `DataReader`."
///
/// Anders als die Plain-Counter-Statuses dürfen `*_count_change` hier
/// **negativ** werden, wenn z.B. ein als "alive" deklarierter Writer
/// jetzt als "not_alive" zählt (übersprungen von `alive_count` zu
/// `not_alive_count`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivelinessChangedStatus {
    /// Number of currently-alive matched writers.
    pub alive_count: i32,
    /// Number of currently-not-alive matched writers.
    pub not_alive_count: i32,
    /// Change in `alive_count` since the last read.
    pub alive_count_change: i32,
    /// Change in `not_alive_count` since the last read.
    pub not_alive_count_change: i32,
    /// Handle of the last writer that triggered the change.
    pub last_publication_handle: InstanceHandle,
}

/// `PUBLICATION_MATCHED_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Writer-Seite: "Reports the discovery of a new compatible
/// DataReader / the loss of one."
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicationMatchedStatus {
    /// Total cumulative count of compatible DataReaders that have been
    /// discovered so far (monoton steigend).
    pub total_count: i32,
    /// Change in `total_count` since the last read.
    pub total_count_change: i32,
    /// Currently matched DataReaders (kann fallen, wenn Reader weggeht).
    pub current_count: i32,
    /// Change in `current_count` since the last read (darf negativ
    /// werden).
    pub current_count_change: i32,
    /// Handle of the last DataReader that matched the writer.
    pub last_subscription_handle: InstanceHandle,
}

/// `SUBSCRIPTION_MATCHED_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader-Seite: spiegelt PublicationMatched. Felder gleichlaufend.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionMatchedStatus {
    /// Total cumulative count of compatible DataWriters discovered.
    pub total_count: i32,
    /// Change in `total_count` since last read.
    pub total_count_change: i32,
    /// Currently matched DataWriters.
    pub current_count: i32,
    /// Change in `current_count` since last read.
    pub current_count_change: i32,
    /// Handle of the last DataWriter that matched.
    pub last_publication_handle: InstanceHandle,
}

/// `QosPolicyCount` — Sub-Element von `*IncompatibleQosStatus`.
///
/// Pro QoS-Policy-Id ein Counter, wie oft genau diese Policy zur
/// Inkompatibilität geführt hat. Spec §2.2.4.1 Tab. 2.10.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QosPolicyCount {
    /// Policy-Id (siehe [`crate::psm_constants::qos_policy_id`]).
    pub policy_id: u32,
    /// Wie oft diese Policy inkompatibel war.
    pub count: i32,
}

impl QosPolicyCount {
    /// Konstruktor.
    #[must_use]
    pub const fn new(policy_id: u32, count: i32) -> Self {
        Self { policy_id, count }
    }
}

/// `OFFERED_INCOMPATIBLE_QOS_STATUS` — Spec §2.2.4.1 + §2.2.2.4.2.
///
/// Writer-Seite: ein Reader wurde gefunden, dessen `requested QoS`
/// nicht zum `offered QoS` des Writers passt.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OfferedIncompatibleQosStatus {
    /// Total cumulative count of incompatible-QoS detections.
    pub total_count: i32,
    /// Change in `total_count` since last read.
    pub total_count_change: i32,
    /// Policy-Id of the *most recent* policy that caused the
    /// incompatibility.
    pub last_policy_id: u32,
    /// Per-policy counters.
    pub policies: Vec<QosPolicyCount>,
}

/// `REQUESTED_INCOMPATIBLE_QOS_STATUS` — Spec §2.2.4.1 + §2.2.2.5.6.
///
/// Reader-Seite: ein Writer wurde gefunden, dessen `offered QoS` nicht
/// zum `requested QoS` des Readers passt.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RequestedIncompatibleQosStatus {
    /// Total cumulative count of incompatible-QoS detections.
    pub total_count: i32,
    /// Change in `total_count` since last read.
    pub total_count_change: i32,
    /// Policy-Id of the *most recent* policy that caused the
    /// incompatibility.
    pub last_policy_id: u32,
    /// Per-policy counters.
    pub policies: Vec<QosPolicyCount>,
}

// ============================================================================
// Signal-Statuses (Marker-Structs)
// ============================================================================

/// `DATA_AVAILABLE_STATUS` — Spec §2.2.4.1.
///
/// Reines Signal: "new data has arrived in the DataReader". Es gibt
/// keine Spec-Datenstruktur dafür — wir spiegeln den Status als
/// leeres Marker-Struct, damit ein einheitlicher
/// `get_data_available_status()`-Pfad existiert.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataAvailableStatus;

/// `DATA_ON_READERS_STATUS` — Spec §2.2.4.1.
///
/// Reines Signal: "new data has arrived in **any** DataReader of the
/// Subscriber". Wie [`DataAvailableStatus`] ohne Datenstruktur.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataOnReadersStatus;

// ============================================================================
// Helper-Funktionen
// ============================================================================

/// Hängt einen `policy_id`-Counter in einen `policies`-Vec ein. Wenn
/// der Eintrag existiert, wird `count` inkrementiert; sonst neu
/// angefügt. Genutzt vom Runtime-Layer beim Verteilen eines
/// IncompatibleQos-Events.
pub fn bump_policy_count(policies: &mut Vec<QosPolicyCount>, policy_id: u32) {
    if let Some(slot) = policies.iter_mut().find(|p| p.policy_id == policy_id) {
        slot.count = slot.count.saturating_add(1);
        return;
    }
    policies.push(QosPolicyCount::new(policy_id, 1));
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::psm_constants::qos_policy_id;

    #[test]
    fn inconsistent_topic_default_is_zero() {
        let s = InconsistentTopicStatus::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.total_count_change, 0);
    }

    #[test]
    fn sample_lost_clone_roundtrip() {
        let s = SampleLostStatus {
            total_count: 5,
            total_count_change: 2,
        };
        let c = s;
        assert_eq!(s, c);
    }

    #[test]
    fn sample_rejected_status_default_kind_is_not_rejected() {
        let s = SampleRejectedStatus::default();
        assert_eq!(s.last_reason, SampleRejectedStatusKind::NotRejected);
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn sample_rejected_kind_variants_are_distinct() {
        // Vollständigkeit der Spec-Enum-Variants.
        let kinds = [
            SampleRejectedStatusKind::NotRejected,
            SampleRejectedStatusKind::RejectedByInstancesLimit,
            SampleRejectedStatusKind::RejectedBySamplesLimit,
            SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn liveliness_lost_default() {
        let s = LivelinessLostStatus::default();
        assert_eq!(s.total_count, 0);
    }

    #[test]
    fn liveliness_changed_negative_count_change_allowed() {
        // Spec §2.2.4.1: alive_count_change kann negativ sein, wenn
        // ein Writer von "alive" zu "not_alive" wechselt.
        let s = LivelinessChangedStatus {
            alive_count: 0,
            not_alive_count: 1,
            alive_count_change: -1,
            not_alive_count_change: 1,
            last_publication_handle: InstanceHandle::from_raw(42),
        };
        assert_eq!(s.alive_count_change, -1);
        assert_eq!(s.not_alive_count_change, 1);
        assert_eq!(s.last_publication_handle.as_raw(), 42);
    }

    #[test]
    fn publication_matched_with_handle_roundtrip() {
        let h = InstanceHandle::from_raw(99);
        let s = PublicationMatchedStatus {
            total_count: 3,
            total_count_change: 1,
            current_count: 2,
            current_count_change: 1,
            last_subscription_handle: h,
        };
        let c = s;
        assert_eq!(c.last_subscription_handle, h);
        assert_eq!(c.current_count, 2);
    }

    #[test]
    fn subscription_matched_with_handle_roundtrip() {
        let h = InstanceHandle::from_raw(7);
        let s = SubscriptionMatchedStatus {
            total_count: 5,
            total_count_change: 0,
            current_count: 5,
            current_count_change: 0,
            last_publication_handle: h,
        };
        assert_eq!(s.last_publication_handle, h);
    }

    #[test]
    fn offered_deadline_missed_default_handle_is_nil() {
        let s = OfferedDeadlineMissedStatus::default();
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn requested_deadline_missed_default_handle_is_nil() {
        let s = RequestedDeadlineMissedStatus::default();
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn offered_incompatible_qos_clone_roundtrip() {
        let s = OfferedIncompatibleQosStatus {
            total_count: 2,
            total_count_change: 1,
            last_policy_id: qos_policy_id::DURABILITY,
            policies: alloc::vec![QosPolicyCount::new(qos_policy_id::DURABILITY, 2)],
        };
        let c = s.clone();
        assert_eq!(c, s);
        assert_eq!(c.policies.len(), 1);
        assert_eq!(c.policies[0].policy_id, qos_policy_id::DURABILITY);
    }

    #[test]
    fn requested_incompatible_qos_clone_roundtrip() {
        let s = RequestedIncompatibleQosStatus {
            total_count: 1,
            total_count_change: 1,
            last_policy_id: qos_policy_id::RELIABILITY,
            policies: alloc::vec![QosPolicyCount::new(qos_policy_id::RELIABILITY, 1)],
        };
        let c = s.clone();
        assert_eq!(c, s);
    }

    #[test]
    fn bump_policy_count_inserts_then_increments() {
        let mut v = alloc::vec::Vec::<QosPolicyCount>::new();
        bump_policy_count(&mut v, qos_policy_id::DURABILITY);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].count, 1);
        bump_policy_count(&mut v, qos_policy_id::DURABILITY);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].count, 2);
        bump_policy_count(&mut v, qos_policy_id::RELIABILITY);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn data_available_and_data_on_readers_are_zero_sized_markers() {
        // Spec §2.2.4.1: pure Signals — keine Felder.
        // Wir checken die Marker-Semantik via Default+Eq.
        let a1 = DataAvailableStatus;
        let a2 = DataAvailableStatus;
        assert_eq!(a1, a2);
        let r1 = DataOnReadersStatus;
        let r2 = DataOnReadersStatus;
        assert_eq!(r1, r2);
        // Marker-Structs haben Größe 0 (kompakt).
        assert_eq!(core::mem::size_of::<DataAvailableStatus>(), 0);
        assert_eq!(core::mem::size_of::<DataOnReadersStatus>(), 0);
    }
}
