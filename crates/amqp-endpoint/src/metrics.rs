// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mandatory-metrics counter hub.
//!
//! Spec source: dds-amqp-1.0 §7.9.2 + §7.9.2.1 — all 13
//! mandatory metrics that an endpoint MUST expose on `$metrics`.
//! We hold them as atomic counters in a
//! `MetricsHub`, which the caller increments at strategic points
//! (connection open, transfer send, loop drop, etc.).
//!
//! The `$metrics` receiver producer (a separate module, still
//! pending) reads snapshots of this hub and produces
//! AMQP messages with a map body
//! `{name, value, unit, timestamp}` per §7.9.2.

use core::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Spec §7.9.2.1 — mandatory-metric identifier (string key
/// for the `name` field of the `$metrics` sample-body map).
pub mod names {
    /// `connections.active` — currently open AMQP connections.
    pub const CONNECTIONS_ACTIVE: &str = "connections.active";
    /// `connections.total` — cumulative accepted since start.
    pub const CONNECTIONS_TOTAL: &str = "connections.total";
    /// `transfers.received` — cumulative received transfers.
    pub const TRANSFERS_RECEIVED: &str = "transfers.received";
    /// `transfers.sent` — cumulative sent transfers.
    pub const TRANSFERS_SENT: &str = "transfers.sent";
    /// `transfers.unsettled` — currently unsettled deliveries.
    pub const TRANSFERS_UNSETTLED: &str = "transfers.unsettled";
    /// `transfers.rate` — sliding-window throughput (per second).
    pub const TRANSFERS_RATE: &str = "transfers.rate";
    /// `errors.decode` — cumulative `amqp:decode-error` cases.
    pub const ERRORS_DECODE: &str = "errors.decode";
    /// `errors.unauthorized` — cumulative `amqp:unauthorized-access` cases.
    pub const ERRORS_UNAUTHORIZED: &str = "errors.unauthorized";
    /// `topics.exposed` — current catalog entries.
    pub const TOPICS_EXPOSED: &str = "topics.exposed";
    /// `transfers.dropped.loop` — bridge-coexistence self-tag drops.
    pub const TRANSFERS_DROPPED_LOOP: &str = "transfers.dropped.loop";
    /// `transfers.dropped.hop-cap` — hop-cap-exceeded drops.
    pub const TRANSFERS_DROPPED_HOP_CAP: &str = "transfers.dropped.hop-cap";
    /// `transfers.dropped.malformed-reply` — RPC reply-validation drops.
    pub const TRANSFERS_DROPPED_MALFORMED_REPLY: &str = "transfers.dropped.malformed-reply";
    /// `rpc.calls.timed-out` — RPC-aware calls that exceeded `rpc_timeout_ms`.
    pub const RPC_CALLS_TIMED_OUT: &str = "rpc.calls.timed-out";
    /// `transfers.dropped.reconnect-overflow` — KEEP_LAST eviction
    /// during a bridge reconnect.
    pub const TRANSFERS_DROPPED_RECONNECT_OVERFLOW: &str = "transfers.dropped.reconnect-overflow";
}

/// Spec §7.9.2 — unit identifier for the `unit` field.
pub mod units {
    /// Countable quantity (default for counters).
    pub const COUNT: &str = "count";
    /// Bytes.
    pub const BYTES: &str = "bytes";
    /// Milliseconds.
    pub const MILLISECONDS: &str = "milliseconds";
    /// Per second (rate).
    pub const PER_SECOND: &str = "per-second";
}

/// Spec §7.9.2.1 — complete mandatory-metric list for
/// test/catalog iteration. The order matches the table in
/// the spec.
pub const MANDATORY_METRIC_NAMES: [&str; 14] = [
    names::CONNECTIONS_ACTIVE,
    names::CONNECTIONS_TOTAL,
    names::TRANSFERS_RECEIVED,
    names::TRANSFERS_SENT,
    names::TRANSFERS_UNSETTLED,
    names::TRANSFERS_RATE,
    names::ERRORS_DECODE,
    names::ERRORS_UNAUTHORIZED,
    names::TOPICS_EXPOSED,
    names::TRANSFERS_DROPPED_LOOP,
    names::TRANSFERS_DROPPED_HOP_CAP,
    names::TRANSFERS_DROPPED_MALFORMED_REPLY,
    names::RPC_CALLS_TIMED_OUT,
    names::TRANSFERS_DROPPED_RECONNECT_OVERFLOW,
];

/// Process-global counter hub for all 13 mandatory metrics.
///
/// Spec §7.9.2 — counter values are monotonically non-negative
/// (cumulative, gauge or rate); we use 64-bit atomic
/// counters, and the caller signs the calls itself.
#[derive(Debug, Default)]
pub struct MetricsHub {
    /// `connections.active` — gauge.
    pub connections_active: AtomicI64,
    /// `connections.total` — cumulative.
    pub connections_total: AtomicU64,
    /// `transfers.received` — cumulative.
    pub transfers_received: AtomicU64,
    /// `transfers.sent` — cumulative.
    pub transfers_sent: AtomicU64,
    /// `transfers.unsettled` — gauge (signed because
    /// settling before sending is possible and needs `saturating_sub`).
    pub transfers_unsettled: AtomicI64,
    /// `transfers.rate` — sliding-window throughput per second
    /// (caller computes from delta).
    pub transfers_rate: AtomicI64,
    /// `errors.decode` — cumulative.
    pub errors_decode: AtomicU64,
    /// `errors.unauthorized` — cumulative.
    pub errors_unauthorized: AtomicU64,
    /// `topics.exposed` — gauge.
    pub topics_exposed: AtomicI64,
    /// `transfers.dropped.loop` — cumulative.
    pub transfers_dropped_loop: AtomicU64,
    /// `transfers.dropped.hop-cap` — cumulative.
    pub transfers_dropped_hop_cap: AtomicU64,
    /// `transfers.dropped.malformed-reply` — cumulative.
    pub transfers_dropped_malformed_reply: AtomicU64,
    /// `rpc.calls.timed-out` — cumulative.
    pub rpc_calls_timed_out: AtomicU64,
    /// `transfers.dropped.reconnect-overflow` — cumulative.
    pub transfers_dropped_reconnect_overflow: AtomicU64,
}

impl MetricsHub {
    /// Fresh hub with all counters at 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ---------------- Counter API ----------------

    /// Connection opened.
    pub fn on_connection_open(&self) {
        self.connections_active.fetch_add(1, Ordering::Relaxed);
        self.connections_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Connection closed.
    pub fn on_connection_close(&self) {
        self.connections_active.fetch_sub(1, Ordering::Relaxed);
    }

    /// Transfer received.
    pub fn on_transfer_received(&self) {
        self.transfers_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Transfer sent (unsettled tracking via `settle_started`).
    pub fn on_transfer_sent(&self, settle_started: bool) {
        self.transfers_sent.fetch_add(1, Ordering::Relaxed);
        if settle_started {
            self.transfers_unsettled.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Disposition received → lower the unsettled counter.
    pub fn on_settled(&self) {
        self.transfers_unsettled.fetch_sub(1, Ordering::Relaxed);
    }

    /// Decode error counted.
    pub fn on_decode_error(&self) {
        self.errors_decode.fetch_add(1, Ordering::Relaxed);
    }

    /// Unauthorized error counted.
    pub fn on_unauthorized(&self) {
        self.errors_unauthorized.fetch_add(1, Ordering::Relaxed);
    }

    /// Topic exposed (catalog add) — gauge raise.
    pub fn on_topic_added(&self) {
        self.topics_exposed.fetch_add(1, Ordering::Relaxed);
    }

    /// Topic removed — gauge fall.
    pub fn on_topic_removed(&self) {
        self.topics_exposed.fetch_sub(1, Ordering::Relaxed);
    }

    /// Loop drop (bridge-coexistence self-tag).
    pub fn on_dropped_loop(&self) {
        self.transfers_dropped_loop.fetch_add(1, Ordering::Relaxed);
    }

    /// Hop-cap drop.
    pub fn on_dropped_hop_cap(&self) {
        self.transfers_dropped_hop_cap
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Malformed-reply drop (RPC-aware).
    pub fn on_dropped_malformed_reply(&self) {
        self.transfers_dropped_malformed_reply
            .fetch_add(1, Ordering::Relaxed);
    }

    /// RPC call timeout.
    pub fn on_rpc_timeout(&self) {
        self.rpc_calls_timed_out.fetch_add(1, Ordering::Relaxed);
    }

    /// KEEP_LAST eviction during reconnect.
    pub fn on_reconnect_overflow(&self) {
        self.transfers_dropped_reconnect_overflow
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Read the current value by name (for the
    /// `$metrics` sample producer).
    ///
    /// Returns `Some(value)` for all mandatory-metric names from
    /// [`MANDATORY_METRIC_NAMES`] and `None` for unknown
    /// names.
    #[must_use]
    pub fn snapshot(&self, name: &str) -> Option<i64> {
        let v = match name {
            names::CONNECTIONS_ACTIVE => self.connections_active.load(Ordering::Relaxed),
            names::CONNECTIONS_TOTAL => {
                i64_from_u64(self.connections_total.load(Ordering::Relaxed))
            }
            names::TRANSFERS_RECEIVED => {
                i64_from_u64(self.transfers_received.load(Ordering::Relaxed))
            }
            names::TRANSFERS_SENT => i64_from_u64(self.transfers_sent.load(Ordering::Relaxed)),
            names::TRANSFERS_UNSETTLED => self.transfers_unsettled.load(Ordering::Relaxed),
            names::TRANSFERS_RATE => self.transfers_rate.load(Ordering::Relaxed),
            names::ERRORS_DECODE => i64_from_u64(self.errors_decode.load(Ordering::Relaxed)),
            names::ERRORS_UNAUTHORIZED => {
                i64_from_u64(self.errors_unauthorized.load(Ordering::Relaxed))
            }
            names::TOPICS_EXPOSED => self.topics_exposed.load(Ordering::Relaxed),
            names::TRANSFERS_DROPPED_LOOP => {
                i64_from_u64(self.transfers_dropped_loop.load(Ordering::Relaxed))
            }
            names::TRANSFERS_DROPPED_HOP_CAP => {
                i64_from_u64(self.transfers_dropped_hop_cap.load(Ordering::Relaxed))
            }
            names::TRANSFERS_DROPPED_MALFORMED_REPLY => i64_from_u64(
                self.transfers_dropped_malformed_reply
                    .load(Ordering::Relaxed),
            ),
            names::RPC_CALLS_TIMED_OUT => {
                i64_from_u64(self.rpc_calls_timed_out.load(Ordering::Relaxed))
            }
            names::TRANSFERS_DROPPED_RECONNECT_OVERFLOW => i64_from_u64(
                self.transfers_dropped_reconnect_overflow
                    .load(Ordering::Relaxed),
            ),
            _ => return None,
        };
        Some(v)
    }

    /// Spec §7.9.2 `unit` field per metric.
    #[must_use]
    pub const fn unit_of(name: &str) -> Option<&'static str> {
        match name.as_bytes() {
            b"transfers.rate" => Some(units::PER_SECOND),
            b"connections.active"
            | b"connections.total"
            | b"transfers.received"
            | b"transfers.sent"
            | b"transfers.unsettled"
            | b"errors.decode"
            | b"errors.unauthorized"
            | b"topics.exposed"
            | b"transfers.dropped.loop"
            | b"transfers.dropped.hop-cap"
            | b"transfers.dropped.malformed-reply"
            | b"rpc.calls.timed-out"
            | b"transfers.dropped.reconnect-overflow" => Some(units::COUNT),
            _ => None,
        }
    }
}

const fn i64_from_u64(v: u64) -> i64 {
    if v > i64::MAX as u64 {
        i64::MAX
    } else {
        v as i64
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_metric_count_is_14() {
        // Spec §7.9.2.1 enumerates 14 metrics (incl. the three
        // dropped.* and rpc.calls.timed-out metrics from the
        // sealed-profile iterations + reconnect-overflow).
        assert_eq!(MANDATORY_METRIC_NAMES.len(), 14);
    }

    #[test]
    fn fresh_hub_all_zero() {
        let h = MetricsHub::new();
        for name in MANDATORY_METRIC_NAMES {
            assert_eq!(h.snapshot(name), Some(0), "metric {name} initial != 0");
        }
    }

    #[test]
    fn unknown_metric_yields_none() {
        let h = MetricsHub::new();
        assert!(h.snapshot("not.a.metric").is_none());
    }

    #[test]
    fn connection_open_close_balances() {
        let h = MetricsHub::new();
        h.on_connection_open();
        h.on_connection_open();
        assert_eq!(h.snapshot(names::CONNECTIONS_ACTIVE), Some(2));
        assert_eq!(h.snapshot(names::CONNECTIONS_TOTAL), Some(2));
        h.on_connection_close();
        assert_eq!(h.snapshot(names::CONNECTIONS_ACTIVE), Some(1));
        // total is cumulative — not reset.
        assert_eq!(h.snapshot(names::CONNECTIONS_TOTAL), Some(2));
    }

    #[test]
    fn transfer_unsettled_tracking() {
        let h = MetricsHub::new();
        h.on_transfer_sent(true); // unsettled
        h.on_transfer_sent(true);
        assert_eq!(h.snapshot(names::TRANSFERS_UNSETTLED), Some(2));
        h.on_settled();
        assert_eq!(h.snapshot(names::TRANSFERS_UNSETTLED), Some(1));
    }

    #[test]
    fn pre_settled_transfer_does_not_count_unsettled() {
        let h = MetricsHub::new();
        h.on_transfer_sent(false); // settled / pre-settled
        assert_eq!(h.snapshot(names::TRANSFERS_SENT), Some(1));
        assert_eq!(h.snapshot(names::TRANSFERS_UNSETTLED), Some(0));
    }

    #[test]
    fn loop_and_hop_cap_drops_count_separately() {
        let h = MetricsHub::new();
        h.on_dropped_loop();
        h.on_dropped_loop();
        h.on_dropped_hop_cap();
        assert_eq!(h.snapshot(names::TRANSFERS_DROPPED_LOOP), Some(2));
        assert_eq!(h.snapshot(names::TRANSFERS_DROPPED_HOP_CAP), Some(1));
    }

    #[test]
    fn rpc_failure_counters_independent() {
        let h = MetricsHub::new();
        h.on_rpc_timeout();
        h.on_dropped_malformed_reply();
        h.on_dropped_malformed_reply();
        assert_eq!(h.snapshot(names::RPC_CALLS_TIMED_OUT), Some(1));
        assert_eq!(
            h.snapshot(names::TRANSFERS_DROPPED_MALFORMED_REPLY),
            Some(2)
        );
    }

    #[test]
    fn unit_of_known_metrics() {
        assert_eq!(
            MetricsHub::unit_of(names::CONNECTIONS_ACTIVE),
            Some(units::COUNT)
        );
        assert_eq!(
            MetricsHub::unit_of(names::TRANSFERS_RATE),
            Some(units::PER_SECOND)
        );
        assert_eq!(MetricsHub::unit_of("not.a.metric"), None);
    }

    #[test]
    fn topics_added_removed_balances() {
        let h = MetricsHub::new();
        h.on_topic_added();
        h.on_topic_added();
        h.on_topic_added();
        assert_eq!(h.snapshot(names::TOPICS_EXPOSED), Some(3));
        h.on_topic_removed();
        assert_eq!(h.snapshot(names::TOPICS_EXPOSED), Some(2));
    }

    #[test]
    fn reconnect_overflow_counter() {
        let h = MetricsHub::new();
        h.on_reconnect_overflow();
        h.on_reconnect_overflow();
        assert_eq!(
            h.snapshot(names::TRANSFERS_DROPPED_RECONNECT_OVERFLOW),
            Some(2)
        );
    }
}
