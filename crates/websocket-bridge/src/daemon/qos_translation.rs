// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS-QoS → WebSocket-Behavior-Translation.
//!
//! Mapping per Spec `zerodds-ws-bridge-1.0.md` §6:
//!
//! * `Reliability::Reliable`   → backpressure-Mode (write-block bei
//!   voller Send-Queue) + per-conn-credit; `BestEffort` → drop-on-full.
//! * `Durability::TransientLocal/Transient/Persistent` → replay-cache:
//!   neu-verbindende Subscriber bekommen die letzten N Samples (gemaess
//!   `history_depth`) wiederholt.
//! * `Deadline` → emit `op:deadline-missed` wenn ein Sample nicht
//!   innerhalb des Deadline-Budgets ausgeliefert wurde.

use zerodds_qos::{
    DurabilityKind, HistoryKind, HistoryQosPolicy, ReaderQos, ReliabilityKind, WriterQos,
};

/// Backpressure-Verhalten bei voller Send-Queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureMode {
    /// Reliable — block bis Queue Slot frei wird (Spec §6.1).
    Block,
    /// BestEffort — drop oldest, non-blocking.
    DropOldest,
}

/// Replay-Cache-Strategie fuer neu verbindende Subscriber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Volatile — keine Wiederholung.
    None,
    /// TransientLocal — letzte N Samples wiederholen.
    LastN(u32),
    /// Transient/Persistent — alles aus dem Topic-Cache.
    All,
}

/// Deadline-Watcher-Verhalten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineMode {
    /// Kein Deadline-Tracking.
    None,
    /// Emit `op:deadline-missed` wenn ueberschritten.
    Emit {
        /// Deadline-Periode in Millisekunden.
        period_ms: u64,
    },
}

/// Vollstaendiges Behavior das aus einem QoS-Set abgeleitet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsBehavior {
    /// Backpressure auf der Connection-Send-Queue.
    pub backpressure: BackpressureMode,
    /// Replay fuer neu verbindende Subscriber.
    pub replay: ReplayMode,
    /// Deadline-Tracking + Notification.
    pub deadline: DeadlineMode,
    /// Per-Connection-Send-Queue-Bound (Anzahl Frames).
    pub send_queue_capacity: usize,
}

impl Default for WsBehavior {
    fn default() -> Self {
        Self {
            backpressure: BackpressureMode::Block,
            replay: ReplayMode::None,
            deadline: DeadlineMode::None,
            send_queue_capacity: 1024,
        }
    }
}

impl WsBehavior {
    /// Spec-konformes Default-Behavior fuer Topic ohne ueberlagerndes
    /// QoS — basiert auf `WriterQos::default()` (Reliable, Volatile,
    /// Deadline=infinite).
    #[must_use]
    pub fn default_for_topic() -> Self {
        let writer = WriterQos::default();
        let reader = ReaderQos::default();
        dds_qos_to_ws_behavior(&writer, &reader)
    }
}

/// Hauptfunktion: leite ein WS-Behavior aus dem `(Writer,Reader)`-QoS ab.
#[must_use]
pub fn dds_qos_to_ws_behavior(writer: &WriterQos, reader: &ReaderQos) -> WsBehavior {
    let backpressure = match (writer.reliability.kind, reader.reliability.kind) {
        (ReliabilityKind::Reliable, _) | (_, ReliabilityKind::Reliable) => BackpressureMode::Block,
        _ => BackpressureMode::DropOldest,
    };
    let replay = replay_for(writer.durability.kind, &writer.history);
    let deadline = deadline_for(&writer.deadline);
    let cap = capacity_for(&writer.history);
    WsBehavior {
        backpressure,
        replay,
        deadline,
        send_queue_capacity: cap,
    }
}

fn replay_for(d: DurabilityKind, history: &HistoryQosPolicy) -> ReplayMode {
    match d {
        DurabilityKind::Volatile => ReplayMode::None,
        DurabilityKind::TransientLocal => match history.kind {
            HistoryKind::KeepLast => ReplayMode::LastN(history.depth.max(1) as u32),
            HistoryKind::KeepAll => ReplayMode::All,
        },
        DurabilityKind::Transient | DurabilityKind::Persistent => ReplayMode::All,
    }
}

fn deadline_for(d: &zerodds_qos::DeadlineQosPolicy) -> DeadlineMode {
    if d.period == zerodds_qos::Duration::INFINITE || d.period == zerodds_qos::Duration::ZERO {
        return DeadlineMode::None;
    }
    // ms = seconds*1000 + fraction*1000/2^32.
    let frac_ms = ((d.period.fraction as u64) * 1000) >> 32;
    let total_ms = (d.period.seconds.max(0) as u64) * 1000 + frac_ms;
    if total_ms == 0 {
        DeadlineMode::None
    } else {
        DeadlineMode::Emit {
            period_ms: total_ms,
        }
    }
}

fn capacity_for(history: &HistoryQosPolicy) -> usize {
    match history.kind {
        HistoryKind::KeepLast => (history.depth.max(1) as usize).max(16),
        HistoryKind::KeepAll => 8192,
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_qos::{Duration, ReliabilityQosPolicy};

    #[test]
    fn reliable_writer_yields_block_backpressure() {
        let mut w = WriterQos::default();
        let r = ReaderQos::default();
        w.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            ..w.reliability
        };
        let b = dds_qos_to_ws_behavior(&w, &r);
        assert_eq!(b.backpressure, BackpressureMode::Block);
    }

    #[test]
    fn best_effort_yields_drop_oldest() {
        let mut w = WriterQos::default();
        w.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..w.reliability
        };
        let mut r = ReaderQos::default();
        r.reliability = ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            ..r.reliability
        };
        let b = dds_qos_to_ws_behavior(&w, &r);
        assert_eq!(b.backpressure, BackpressureMode::DropOldest);
    }

    #[test]
    fn transient_local_keep_last_yields_replay_lastn() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::TransientLocal;
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 7;
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r);
        assert_eq!(b.replay, ReplayMode::LastN(7));
    }

    #[test]
    fn volatile_yields_no_replay() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::Volatile;
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r);
        assert_eq!(b.replay, ReplayMode::None);
    }

    #[test]
    fn transient_yields_all_replay() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::Transient;
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r);
        assert_eq!(b.replay, ReplayMode::All);
    }

    #[test]
    fn deadline_set_yields_emit_mode() {
        let mut w = WriterQos::default();
        w.deadline.period = Duration::from_millis(500);
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r);
        match b.deadline {
            DeadlineMode::Emit { period_ms } => assert!(period_ms >= 1),
            DeadlineMode::None => panic!("expected emit"),
        }
    }

    #[test]
    fn deadline_infinite_yields_none() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r);
        assert_eq!(b.deadline, DeadlineMode::None);
    }

    #[test]
    fn default_for_topic_uses_writer_defaults() {
        let b = WsBehavior::default_for_topic();
        // Writer-Default ist Reliable.
        assert_eq!(b.backpressure, BackpressureMode::Block);
        // Default-Durability ist Volatile.
        assert_eq!(b.replay, ReplayMode::None);
    }
}
