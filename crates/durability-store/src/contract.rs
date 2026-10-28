// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The retention **contract** — the only thing that bounds retention
//! (ADR 0009, invariant 1). Derived from `DurabilityServiceQosPolicy`.

use core::time::Duration;

use zerodds_qos::policies::durability_service::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

/// Retention contract for a topic, derived from its
/// `DurabilityServiceQosPolicy`. The cold store MUST enforce exactly this and
/// nothing tighter (the memory budget is a separate, non-retention knob).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contract {
    /// `KEEP_LAST` (ring per instance at `history_depth`) or `KEEP_ALL`
    /// (bounded only by the `max_*` caps).
    pub history_kind: HistoryKind,
    /// `KEEP_LAST` depth per instance (`<= 0` is normalized to 1).
    pub history_depth: i32,
    /// Global sample cap (`LENGTH_UNLIMITED` = unbounded).
    pub max_samples: i32,
    /// Instance-count cap (`LENGTH_UNLIMITED` = unbounded).
    pub max_instances: i32,
    /// Per-instance sample cap under `KEEP_ALL` (`LENGTH_UNLIMITED` = unbounded).
    pub max_samples_per_instance: i32,
    /// Delay after an instance is unregistered before its samples are purged.
    pub cleanup_delay: Duration,
}

impl Contract {
    /// Derives the contract from the durability-service QoS policy.
    #[must_use]
    pub fn from_qos(qos: &DurabilityServiceQosPolicy) -> Self {
        Self {
            history_kind: qos.history_kind,
            history_depth: qos.history_depth,
            max_samples: qos.max_samples,
            max_instances: qos.max_instances,
            max_samples_per_instance: qos.max_samples_per_instance,
            cleanup_delay: cleanup_delay(qos),
        }
    }

    /// A `KEEP_ALL` contract with all caps unlimited and no cleanup delay — the
    /// sensible default for a durability service that should retain full
    /// history (the `DurabilityServiceQosPolicy` default is `KEEP_LAST(1)`,
    /// which is rarely what a service wants).
    #[must_use]
    pub fn keep_all() -> Self {
        Self {
            history_kind: HistoryKind::KeepAll,
            history_depth: 0,
            max_samples: LENGTH_UNLIMITED,
            max_instances: LENGTH_UNLIMITED,
            max_samples_per_instance: LENGTH_UNLIMITED,
            cleanup_delay: core::time::Duration::ZERO,
        }
    }

    /// Effective `KEEP_LAST` depth (`<= 0` normalized to 1, per DDS default).
    #[must_use]
    pub fn effective_depth(&self) -> usize {
        if self.history_depth <= 0 {
            1
        } else {
            self.history_depth as usize
        }
    }

    /// `true` if `max_samples` is bounded.
    #[must_use]
    pub fn samples_bounded(&self) -> bool {
        self.max_samples != LENGTH_UNLIMITED
    }

    /// `true` if `max_instances` is bounded.
    #[must_use]
    pub fn instances_bounded(&self) -> bool {
        self.max_instances != LENGTH_UNLIMITED
    }

    /// `true` if `max_samples_per_instance` is bounded.
    #[must_use]
    pub fn per_instance_bounded(&self) -> bool {
        self.max_samples_per_instance != LENGTH_UNLIMITED
    }

    /// Serializes to the seven primitive fields a persisted store row holds a
    /// contract as: `(history_kind, history_depth, max_samples,
    /// max_instances, max_samples_per_instance, cleanup_delay_secs,
    /// cleanup_delay_subsec_nanos)`. Shared by every SQL-backed adapter
    /// (sqlite/lakehouse/postgres) so the column layout and its
    /// encode/decode exist exactly once, not once per adapter.
    #[must_use]
    pub fn to_row(&self) -> (u32, i32, i32, i32, i32, u64, u32) {
        (
            self.history_kind as u32,
            self.history_depth,
            self.max_samples,
            self.max_instances,
            self.max_samples_per_instance,
            self.cleanup_delay.as_secs(),
            self.cleanup_delay.subsec_nanos(),
        )
    }

    /// Inverse of [`to_row`](Self::to_row). `None` if `history_kind` is not a
    /// valid [`HistoryKind`] discriminator — a corrupt or foreign row, which
    /// the caller must refuse loudly (an `Err`), never default silently to a
    /// made-up contract.
    #[must_use]
    pub fn from_row(
        history_kind: u32,
        history_depth: i32,
        max_samples: i32,
        max_instances: i32,
        max_samples_per_instance: i32,
        cleanup_delay_secs: u64,
        cleanup_delay_subsec_nanos: u32,
    ) -> Option<Self> {
        Some(Self {
            history_kind: HistoryKind::try_from_u32(history_kind)?,
            history_depth,
            max_samples,
            max_instances,
            max_samples_per_instance,
            cleanup_delay: Duration::new(cleanup_delay_secs, cleanup_delay_subsec_nanos),
        })
    }
}

impl Default for Contract {
    /// `KEEP_LAST(1)`, all caps unlimited, zero cleanup delay — matches the
    /// `DurabilityServiceQosPolicy` default.
    fn default() -> Self {
        Self::from_qos(&DurabilityServiceQosPolicy::default())
    }
}

/// Service-cleanup-delay (QoS Duration) → `core::time::Duration`. The QoS
/// fraction is `2^-32 s`, not nanoseconds.
fn cleanup_delay(qos: &DurabilityServiceQosPolicy) -> Duration {
    // Rounded to the nearest nanosecond, a whole second carrying.
    let (secs, nanos) = qos.service_cleanup_delay.to_sec_nanosec();
    Duration::new(u64::try_from(secs.max(0)).unwrap_or(0), nanos)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn row_round_trips_keep_last() {
        let c = Contract {
            history_kind: HistoryKind::KeepLast,
            history_depth: 5,
            max_samples: 100,
            max_instances: 10,
            max_samples_per_instance: 3,
            cleanup_delay: Duration::new(7, 42),
        };
        let (a, b, cc, d, e, f, g) = c.to_row();
        assert_eq!(Contract::from_row(a, b, cc, d, e, f, g), Some(c));
    }

    #[test]
    fn row_round_trips_keep_all_unlimited() {
        let c = Contract::keep_all();
        let (a, b, cc, d, e, f, g) = c.to_row();
        assert_eq!(Contract::from_row(a, b, cc, d, e, f, g), Some(c));
    }

    #[test]
    fn from_row_rejects_unknown_history_kind() {
        assert_eq!(Contract::from_row(99, 0, -1, -1, -1, 0, 0), None);
    }
}
