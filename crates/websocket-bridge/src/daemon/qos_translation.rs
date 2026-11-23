// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → WebSocket behavior translation.
//!
//! The one map of Spec `zerodds-ws-bridge-1.0.md` §6 from the QoS of a
//! topic to what the daemon does on its connections:
//!
//! * `Reliability::Reliable` → a full send queue makes the pump wait for
//!   the client (backpressure, nothing dropped); `BestEffort` → a full send
//!   queue drops its oldest sample. The queue holds at most
//!   `max_send_queue_per_connection` samples.
//! * `Durability` other than `Volatile` → the daemon keeps the last N
//!   samples of each instance (`KEEP_LAST(N)`) or all (`KEEP_ALL`), at most
//!   `max_burst_samples`, and sends them to a connection that subscribes.
//! * `Deadline` → `{"op":"deadline_missed"}` to the subscribers when an
//!   instance has no sample for a period.
//!
//! [`topic_qos`] builds the DDS QoS of a configured topic, which the
//! daemon's DataReader and DataWriter of the topic get as well (deadline,
//! liveliness, partition); [`behavior_for`] maps it.

use zerodds_qos::{
    DurabilityKind, HistoryKind, HistoryQosPolicy, LivelinessKind, ReaderQos, ReliabilityKind,
    WriterQos,
};

use super::config::TopicConfig;

/// Backpressure behavior on a full send queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureMode {
    /// Reliable — wait until a queue slot becomes free (Spec §6).
    Block,
    /// BestEffort — drop the oldest queued sample, non-blocking.
    DropOldest,
}

/// Replay-cache strategy for newly connecting subscribers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Volatile — no replay.
    None,
    /// Durable with KEEP_LAST — the last N samples of each instance.
    LastN(u32),
    /// Durable with KEEP_ALL — every sample, at most `max_burst_samples`.
    All,
}

/// Deadline-watcher behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineMode {
    /// No deadline tracking.
    None,
    /// Emit `op:deadline_missed` when exceeded.
    Emit {
        /// Deadline period in milliseconds.
        period_ms: u64,
    },
}

/// Complete behavior derived from a QoS set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsBehavior {
    /// Backpressure on the connection send queue.
    pub backpressure: BackpressureMode,
    /// Replay for newly connecting subscribers.
    pub replay: ReplayMode,
    /// Deadline tracking + notification.
    pub deadline: DeadlineMode,
    /// Per-connection send-queue bound (number of samples).
    pub send_queue_capacity: usize,
}

/// The daemon-wide bounds of §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueLimits {
    /// `max_send_queue_per_connection`.
    pub max_send_queue: usize,
    /// `max_burst_samples`.
    pub max_burst: usize,
}

/// The default of `max_send_queue_per_connection`.
pub const DEFAULT_MAX_SEND_QUEUE: usize = 1024;
/// The default of `max_burst_samples`.
pub const DEFAULT_MAX_BURST: usize = 1000;

impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            max_send_queue: DEFAULT_MAX_SEND_QUEUE,
            max_burst: DEFAULT_MAX_BURST,
        }
    }
}

impl Default for WsBehavior {
    fn default() -> Self {
        Self {
            backpressure: BackpressureMode::Block,
            replay: ReplayMode::None,
            deadline: DeadlineMode::None,
            send_queue_capacity: DEFAULT_MAX_SEND_QUEUE,
        }
    }
}

impl WsBehavior {
    /// Spec-conformant default behavior for a topic without overriding
    /// QoS — based on `WriterQos::default()` (Reliable, Volatile,
    /// Deadline=infinite).
    #[must_use]
    pub fn default_for_topic() -> Self {
        let writer = WriterQos::default();
        let reader = ReaderQos::default();
        dds_qos_to_ws_behavior(&writer, &reader, QueueLimits::default())
    }
}

/// Main function: derive a WS behavior from the `(Writer,Reader)` QoS.
#[must_use]
pub fn dds_qos_to_ws_behavior(
    writer: &WriterQos,
    reader: &ReaderQos,
    limits: QueueLimits,
) -> WsBehavior {
    let backpressure = match (writer.reliability.kind, reader.reliability.kind) {
        (ReliabilityKind::Reliable, _) | (_, ReliabilityKind::Reliable) => BackpressureMode::Block,
        _ => BackpressureMode::DropOldest,
    };
    let replay = replay_for(writer.durability.kind, &writer.history);
    let deadline = deadline_for(&writer.deadline);
    WsBehavior {
        backpressure,
        replay,
        deadline,
        send_queue_capacity: limits.max_send_queue.max(1),
    }
}

/// The DDS QoS of a configured topic (Spec §3, §6): reliability,
/// durability, history, deadline, liveliness (AUTOMATIC with the
/// configured lease) and partition, the same for the daemon's DataWriter
/// and DataReader.
#[must_use]
pub fn topic_qos(topic: &TopicConfig) -> (WriterQos, ReaderQos) {
    let mut w = WriterQos::default();
    let mut r = ReaderQos::default();
    let reliability = if topic.reliability == "best_effort" {
        ReliabilityKind::BestEffort
    } else {
        ReliabilityKind::Reliable
    };
    let durability = match topic.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let history = HistoryQosPolicy {
        kind: if topic.keep_all() {
            HistoryKind::KeepAll
        } else {
            HistoryKind::KeepLast
        },
        depth: topic.history_depth.max(1),
    };
    let ms = |v: u64| zerodds_qos::Duration::from_millis(i32::try_from(v).unwrap_or(i32::MAX));
    w.reliability.kind = reliability;
    r.reliability.kind = reliability;
    w.durability.kind = durability;
    r.durability.kind = durability;
    w.history = history;
    r.history = history;
    if let Some(d) = topic.deadline_ms {
        w.deadline.period = ms(d);
        r.deadline.period = ms(d);
    }
    if let Some(l) = topic.liveliness_lease_ms {
        for lv in [&mut w.liveliness, &mut r.liveliness] {
            lv.kind = LivelinessKind::Automatic;
            lv.lease_duration = ms(l);
        }
    }
    w.partition.names.clone_from(&topic.partition);
    r.partition.names.clone_from(&topic.partition);
    (w, r)
}

/// The behavior of a configured topic: [`topic_qos`] through
/// [`dds_qos_to_ws_behavior`].
#[must_use]
pub fn behavior_for(topic: &TopicConfig, limits: QueueLimits) -> WsBehavior {
    let (w, r) = topic_qos(topic);
    dds_qos_to_ws_behavior(&w, &r, limits)
}

fn replay_for(d: DurabilityKind, history: &HistoryQosPolicy) -> ReplayMode {
    if d == DurabilityKind::Volatile {
        return ReplayMode::None;
    }
    match history.kind {
        HistoryKind::KeepLast => {
            ReplayMode::LastN(u32::try_from(history.depth.max(1)).unwrap_or(1))
        }
        HistoryKind::KeepAll => ReplayMode::All,
    }
}

fn deadline_for(d: &zerodds_qos::DeadlineQosPolicy) -> DeadlineMode {
    if d.period == zerodds_qos::Duration::INFINITE || d.period == zerodds_qos::Duration::ZERO {
        return DeadlineMode::None;
    }
    // Rounded to the nearest millisecond: 300 ms is 299.999… ms in
    // 2^-32 s and was cut to 299.
    let total_ms = u64::try_from(d.period.to_millis()).unwrap_or(u64::MAX);
    if total_ms == 0 {
        DeadlineMode::None
    } else {
        DeadlineMode::Emit {
            period_ms: total_ms,
        }
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
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
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
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        assert_eq!(b.backpressure, BackpressureMode::DropOldest);
    }

    #[test]
    fn transient_local_keep_last_yields_replay_lastn() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::TransientLocal;
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 7;
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        assert_eq!(b.replay, ReplayMode::LastN(7));
    }

    #[test]
    fn volatile_yields_no_replay() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::Volatile;
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        assert_eq!(b.replay, ReplayMode::None);
    }

    #[test]
    fn transient_yields_the_replay_of_its_history() {
        let mut w = WriterQos::default();
        w.durability.kind = DurabilityKind::Transient;
        w.history.kind = HistoryKind::KeepAll;
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        assert_eq!(b.replay, ReplayMode::All);
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 3;
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        assert_eq!(b.replay, ReplayMode::LastN(3));
    }

    /// Spec §6 with §3: the keys of a topic are its DDS QoS and its
    /// behavior; the queue bound is `max_send_queue_per_connection`.
    #[test]
    fn wsbridge_6_the_topic_keys_are_the_qos_and_the_behavior() {
        let topic = TopicConfig {
            name: "T".into(),
            reliability: "best_effort".into(),
            durability: "transient_local".into(),
            history_kind: "keep_last".into(),
            history_depth: 4,
            deadline_ms: Some(250),
            liveliness_lease_ms: Some(2000),
            partition: vec!["plant-a".into(), "line*".into()],
            ..TopicConfig::default()
        };
        let (w, r) = topic_qos(&topic);
        assert_eq!(w.reliability.kind, ReliabilityKind::BestEffort);
        assert_eq!(r.durability.kind, DurabilityKind::TransientLocal);
        assert_eq!(r.history.depth, 4);
        assert_eq!(w.deadline.period, Duration::from_millis(250));
        assert_eq!(r.deadline.period, Duration::from_millis(250));
        assert_eq!(w.liveliness.lease_duration, Duration::from_millis(2000));
        assert_eq!(r.liveliness.kind, LivelinessKind::Automatic);
        assert_eq!(w.partition.names, ["plant-a", "line*"]);
        assert_eq!(r.partition.names, w.partition.names);
        let b = behavior_for(
            &topic,
            QueueLimits {
                max_send_queue: 7,
                max_burst: 9,
            },
        );
        assert_eq!(
            b,
            WsBehavior {
                backpressure: BackpressureMode::DropOldest,
                replay: ReplayMode::LastN(4),
                deadline: DeadlineMode::Emit { period_ms: 250 },
                send_queue_capacity: 7,
            }
        );
        // The defaults: reliable, volatile, no deadline.
        let plain = TopicConfig {
            name: "P".into(),
            ..TopicConfig::default()
        };
        assert_eq!(
            behavior_for(&plain, QueueLimits::default()),
            WsBehavior::default()
        );
        let all = TopicConfig {
            durability: "persistent".into(),
            history_kind: "keep_all".into(),
            ..plain
        };
        assert_eq!(
            behavior_for(&all, QueueLimits::default()).replay,
            ReplayMode::All
        );
    }

    #[test]
    fn deadline_set_yields_emit_mode() {
        let mut w = WriterQos::default();
        w.deadline.period = Duration::from_millis(500);
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        match b.deadline {
            DeadlineMode::Emit { period_ms } => assert!(period_ms >= 1),
            DeadlineMode::None => panic!("expected emit"),
        }
    }

    #[test]
    fn deadline_infinite_yields_none() {
        let w = WriterQos::default();
        let r = ReaderQos::default();
        let b = dds_qos_to_ws_behavior(&w, &r, QueueLimits::default());
        assert_eq!(b.deadline, DeadlineMode::None);
    }

    #[test]
    fn wsbridge_12_1_default_for_topic_uses_writer_defaults() {
        let b = WsBehavior::default_for_topic();
        // The writer default is Reliable.
        assert_eq!(b.backpressure, BackpressureMode::Block);
        // The default durability is Volatile.
        assert_eq!(b.replay, ReplayMode::None);
    }
}
