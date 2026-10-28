// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → CORBA behavior translation.
//!
//! Mapping per spec `zerodds-corba-bridge-1.0.md` §6:
//!
//! * `Reliability::Reliable`     → TwoWay operation (reply expected).
//! * `Reliability::BestEffort`   → OneWay operation (no reply).
//! * `Durability`                → ignored (CORBA has no persistence
//!   translation; persistence layers would be CORBA Persistent Service /
//!   OAS Naming Service bindings, outside the bridge surface).
//! * `Deadline::period`          → IIOP `request-timeout` (spec §4.7).

use zerodds_qos::{ReaderQos, ReliabilityKind, WriterQos};

/// CORBA operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorbaOpKind {
    /// TwoWay — reply expected (Reliable).
    TwoWay,
    /// OneWay — fire-and-forget (BestEffort).
    OneWay,
}

/// Behavior for a BridgeMapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorbaBehavior {
    /// Operation kind.
    pub op_kind: CorbaOpKind,
    /// Request timeout (ms, None = unset).
    pub request_timeout_ms: Option<u32>,
}

impl Default for CorbaBehavior {
    fn default() -> Self {
        Self {
            op_kind: CorbaOpKind::TwoWay,
            request_timeout_ms: None,
        }
    }
}

impl CorbaBehavior {
    /// Defaults from `WriterQos::default()` (Reliable).
    #[must_use]
    pub fn default_for_topic() -> Self {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        dds_qos_to_corba_behavior(&w, &r)
    }
}

/// Main mapping function.
#[must_use]
pub fn dds_qos_to_corba_behavior(writer: &WriterQos, reader: &ReaderQos) -> CorbaBehavior {
    let op_kind = if matches!(writer.reliability.kind, ReliabilityKind::Reliable) {
        CorbaOpKind::TwoWay
    } else {
        CorbaOpKind::OneWay
    };
    let _ = reader;
    let request_timeout_ms = ttl_for(&writer.deadline);
    CorbaBehavior {
        op_kind,
        request_timeout_ms,
    }
}

fn ttl_for(d: &zerodds_qos::DeadlineQosPolicy) -> Option<u32> {
    if d.period == zerodds_qos::Duration::INFINITE || d.period == zerodds_qos::Duration::ZERO {
        return None;
    }
    // Rounded to the nearest millisecond: 300 ms is 299.999… ms in
    // 2^-32 s and was cut to 299.
    let total_ms = u64::try_from(d.period.to_millis()).unwrap_or(u64::MAX);
    if total_ms == 0 {
        None
    } else if total_ms > u32::MAX as u64 {
        Some(u32::MAX)
    } else {
        Some(total_ms as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_qos::{Duration, ReliabilityQosPolicy};

    #[test]
    fn reliable_yields_twoway() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_corba_behavior(&w, &r);
        assert_eq!(b.op_kind, CorbaOpKind::TwoWay);
    }

    #[test]
    fn best_effort_yields_oneway() {
        let mut w = WriterQos::default();
        w.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..w.reliability
        };
        let r = ReaderQos::default();
        let b = dds_qos_to_corba_behavior(&w, &r);
        assert_eq!(b.op_kind, CorbaOpKind::OneWay);
    }

    #[test]
    fn deadline_yields_request_timeout() {
        let mut w = WriterQos::default();
        w.deadline.period = Duration::from_millis(1500);
        let r = ReaderQos::default();
        let b = dds_qos_to_corba_behavior(&w, &r);
        assert_eq!(b.request_timeout_ms, Some(1500));
    }

    #[test]
    fn deadline_infinite_no_timeout() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_corba_behavior(&w, &r);
        assert_eq!(b.request_timeout_ms, None);
    }

    #[test]
    fn default_for_topic_twoway() {
        let b = CorbaBehavior::default_for_topic();
        assert_eq!(b.op_kind, CorbaOpKind::TwoWay);
    }
}
