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
    let secs = u64::try_from(qos.service_cleanup_delay.seconds.max(0)).unwrap_or(0);
    let frac = u64::from(qos.service_cleanup_delay.fraction);
    let nanos = (frac.saturating_mul(1_000_000_000) >> 32) as u32;
    Duration::new(secs, nanos)
}
