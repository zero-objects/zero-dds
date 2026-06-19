// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → gRPC behavior translation.
//!
//! gRPC runs over HTTP/2 (TCP) — the reliability layer is intrinsically
//! reliable. Mapping:
//!
//! * `Reliability::Reliable`     → reliable-only mode (HTTP/2 stream).
//! * `Reliability::BestEffort`   → reliable-only + warning log (gRPC
//!   has no notion of BestEffort).
//! * `Durability::Volatile`      → no buffer replay.
//! * `Durability::TransientLocal+` → buffer replay (server-side cache N).
//! * `Deadline::period`          → gRPC `grpc-timeout` header.

use zerodds_qos::{DurabilityKind, HistoryKind, ReaderQos, ReliabilityKind, WriterQos};

/// gRPC mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcReliabilityMode {
    /// Reliable.
    ReliableOnly,
    /// Reliable + warning log (BestEffort QoS on a reliable transport).
    ReliableWithBestEffortWarning,
}

/// Behavior for a topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrpcBehavior {
    /// Reliability mode.
    pub reliability: GrpcReliabilityMode,
    /// Buffer replay depth for new subscribers.
    pub replay_depth: u32,
    /// `grpc-timeout` (ms).
    pub timeout_ms: Option<u32>,
}

impl Default for GrpcBehavior {
    fn default() -> Self {
        Self {
            reliability: GrpcReliabilityMode::ReliableOnly,
            replay_depth: 0,
            timeout_ms: None,
        }
    }
}

impl GrpcBehavior {
    /// Defaults from `WriterQos::default()`.
    #[must_use]
    pub fn default_for_topic() -> Self {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        dds_qos_to_grpc_behavior(&w, &r)
    }
}

/// Main mapping function.
#[must_use]
pub fn dds_qos_to_grpc_behavior(writer: &WriterQos, reader: &ReaderQos) -> GrpcBehavior {
    // Note: HTTP/2 is intrinsically reliable; we only mark the mode.
    // gRPC uses HTTP/2 = always reliable. We only log a warning when the
    // writer is explicitly BestEffort; a BestEffort reader against a
    // reliable writer is DDS-compliant (a best-effort reader accepts a
    // reliable stream just as well).
    let reliability = if matches!(writer.reliability.kind, ReliabilityKind::Reliable) {
        GrpcReliabilityMode::ReliableOnly
    } else {
        GrpcReliabilityMode::ReliableWithBestEffortWarning
    };
    let _ = reader;
    let replay_depth = match writer.durability.kind {
        DurabilityKind::Volatile => 0,
        DurabilityKind::TransientLocal => match writer.history.kind {
            HistoryKind::KeepLast => writer.history.depth.max(1) as u32,
            HistoryKind::KeepAll => 1024,
        },
        DurabilityKind::Transient | DurabilityKind::Persistent => 1024,
    };
    let timeout_ms = ttl_for(&writer.deadline);
    GrpcBehavior {
        reliability,
        replay_depth,
        timeout_ms,
    }
}

fn ttl_for(d: &zerodds_qos::DeadlineQosPolicy) -> Option<u32> {
    if d.period == zerodds_qos::Duration::INFINITE || d.period == zerodds_qos::Duration::ZERO {
        return None;
    }
    let frac_ms = ((d.period.fraction as u64) * 1000) >> 32;
    let total_ms = (d.period.seconds.max(0) as u64) * 1000 + frac_ms;
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
    fn reliable_yields_reliable_only() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_grpc_behavior(&w, &r);
        assert_eq!(b.reliability, GrpcReliabilityMode::ReliableOnly);
    }

    #[test]
    fn best_effort_writer_yields_warning_mode() {
        let mut w = WriterQos::default();
        w.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..w.reliability
        };
        let r = ReaderQos::default();
        let b = dds_qos_to_grpc_behavior(&w, &r);
        assert_eq!(
            b.reliability,
            GrpcReliabilityMode::ReliableWithBestEffortWarning
        );
    }

    #[test]
    fn transient_local_yields_replay() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::TransientLocal;
        w.history.depth = 8;
        let r = ReaderQos::default();
        let b = dds_qos_to_grpc_behavior(&w, &r);
        assert_eq!(b.replay_depth, 8);
    }

    #[test]
    fn volatile_no_replay() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_grpc_behavior(&w, &r);
        assert_eq!(b.replay_depth, 0);
    }

    #[test]
    fn deadline_yields_timeout_ms() {
        let mut w = WriterQos::default();
        w.deadline.period = Duration::from_millis(750);
        let r = ReaderQos::default();
        let b = dds_qos_to_grpc_behavior(&w, &r);
        assert_eq!(b.timeout_ms, Some(750));
    }

    #[test]
    fn deadline_infinite_no_timeout() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_grpc_behavior(&w, &r);
        assert_eq!(b.timeout_ms, None);
    }

    #[test]
    fn default_for_topic_reliable_only() {
        let b = GrpcBehavior::default_for_topic();
        assert_eq!(b.reliability, GrpcReliabilityMode::ReliableOnly);
    }
}
