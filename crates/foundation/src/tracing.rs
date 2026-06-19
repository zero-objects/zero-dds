// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spans + histograms — coarse-grained tracing primitives.
//!
//! Provides the in-crate primitives; the `zerodds-observability-otlp`
//! crate converts them into OTLP HTTP JSON frames for collector export.
//!
//! ## Design line
//!
//! Like `observability::Event`, spans are **coarse-grained** — we
//! instrument endpoint lifecycle and discovery phases, **not**
//! per-sample latencies. Hot-path latencies come from the histograms
//! in aggregate (`Histogram` here), not from per-sample spans.
//!
//! ## Wire-layer mapping
//!
//! | OTel concept    | Here                          |
//! | --------------- | ----------------------------- |
//! | TraceId         | [`TraceId`] (16 byte)         |
//! | SpanId          | [`SpanId`] (8 byte)           |
//! | Span            | [`Span`]                      |
//! | SpanContext     | [`SpanContext`]               |
//! | Histogram       | [`Histogram`]                 |
//!
//! No `tracing`-crate dependency — we stay minimal. An adapter to
//! `tracing-opentelemetry` is an optional bridge in the consumer.

use alloc::string::String;
use alloc::vec::Vec;

use crate::observability::Attribute;

/// 16-byte W3C trace ID. Never all-zero (zero is reserved for
/// "invalid").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TraceId(pub [u8; 16]);

impl TraceId {
    /// Zero sentinel, OTel spec: invalid.
    pub const INVALID: Self = Self([0u8; 16]);

    /// `true` if not all-zero.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|b| *b != 0)
    }

    /// Hex lowercase repr (OTLP wire).
    #[must_use]
    pub fn to_hex(&self) -> String {
        bytes_to_hex(&self.0)
    }
}

/// 8-byte span ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanId(pub [u8; 8]);

impl SpanId {
    /// Zero sentinel.
    pub const INVALID: Self = Self([0u8; 8]);

    /// `true` if not all-zero.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|b| *b != 0)
    }

    /// Hex lowercase repr.
    #[must_use]
    pub fn to_hex(&self) -> String {
        bytes_to_hex(&self.0)
    }
}

fn bytes_to_hex(b: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(b.len() * 2);
    for &x in b {
        out.push(HEX[(x >> 4) as usize] as char);
        out.push(HEX[(x & 0x0f) as usize] as char);
    }
    out
}

/// Context of the span relationship — from which trace, with which
/// parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanContext {
    /// Global trace identity.
    pub trace_id: TraceId,
    /// Own span ID.
    pub span_id: SpanId,
    /// Parent span (None = root).
    pub parent_span_id: Option<SpanId>,
}

impl SpanContext {
    /// Root span: creates a SpanContext with a randomly chosen
    /// `trace_id` and `span_id` (no parent).
    #[must_use]
    pub fn new_root(trace_id: TraceId, span_id: SpanId) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
        }
    }

    /// Child span under an existing parent.
    #[must_use]
    pub fn child_of(parent: &SpanContext, span_id: SpanId) -> Self {
        Self {
            trace_id: parent.trace_id,
            span_id,
            parent_span_id: Some(parent.span_id),
        }
    }
}

/// Result status of a span (OTel spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanStatus {
    /// Default: no status override.
    Unset,
    /// Span signaled success.
    Ok,
    /// Error — description in the span description field.
    Error,
}

/// Span kind per OTel spec — we use Internal/Server/Client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    /// Default for in-process operations.
    Internal,
    /// Inbound operation (e.g. SEDP discovery receive).
    Server,
    /// Outbound operation (e.g. pub write).
    Client,
}

/// A completed span.
///
/// In ZeroDDS we build spans **completed** — no scope-guard magic.
/// The caller measures start/end manually; the sink receives the
/// result.
#[derive(Clone, Debug)]
pub struct Span {
    /// Span context (trace+span ID + parent).
    pub context: SpanContext,
    /// Span name (e.g. `"dcps.write"`).
    pub name: String,
    /// Span kind.
    pub kind: SpanKind,
    /// Start time relative to a monotonic origin (ns).
    pub start_unix_ns: u64,
    /// End time relative to the same origin.
    pub end_unix_ns: u64,
    /// Status.
    pub status: SpanStatus,
    /// Optional status description (e.g. error message).
    pub status_description: Option<String>,
    /// Attributes, any number.
    pub attributes: Vec<Attribute>,
}

impl Span {
    /// Span duration in nanoseconds.
    #[must_use]
    pub fn duration_ns(&self) -> u64 {
        self.end_unix_ns.saturating_sub(self.start_unix_ns)
    }
}

/// Histogram primitive — exponential buckets (powers of 10) plus
/// sum/count/min/max. Deliberately minimalistic: no hdrhistogram
/// dependency, but `[u64; 10]` buckets for 1ns..10s in 10x steps.
///
/// For full p99/p999 resolution the consumer can use `hdrhistogram`
/// as a downstream sink — here we provide only the aggregate-free
/// summable format that maps directly to OTLP `Histogram`.
#[derive(Clone, Debug)]
pub struct Histogram {
    /// Logical name (e.g. `"dds.write.latency"`).
    pub name: String,
    /// Number of all records.
    pub count: u64,
    /// Sum of all records (in the unit; default: ns).
    pub sum_ns: u64,
    /// Min value.
    pub min_ns: u64,
    /// Max value.
    pub max_ns: u64,
    /// Bucket counts: `buckets[i]` = how many records `<= 10^i ns`.
    /// 10 buckets for 1ns (10^0) to 10s (10^10).
    pub buckets: [u64; 11],
}

impl Histogram {
    /// Constructor with name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            count: 0,
            sum_ns: 0,
            min_ns: u64::MAX,
            max_ns: 0,
            buckets: [0; 11],
        }
    }

    /// Records a value in nanoseconds.
    pub fn record_ns(&mut self, ns: u64) {
        self.count = self.count.saturating_add(1);
        self.sum_ns = self.sum_ns.saturating_add(ns);
        if ns < self.min_ns {
            self.min_ns = ns;
        }
        if ns > self.max_ns {
            self.max_ns = ns;
        }
        let idx = bucket_index(ns);
        self.buckets[idx] = self.buckets[idx].saturating_add(1);
    }

    /// Mean in nanoseconds, or 0 if count=0.
    #[must_use]
    pub fn mean_ns(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.sum_ns / self.count
        }
    }

    /// Bucket boundaries as an array — for OTLP export.
    /// `bounds[i]` is the upper cutoff of `buckets[i]` in nanoseconds.
    #[must_use]
    pub fn bucket_bounds() -> [u64; 11] {
        [
            1,              // 10^0
            10,             // 10^1
            100,            // 10^2
            1_000,          // 10^3
            10_000,         // 10^4
            100_000,        // 10^5
            1_000_000,      // 10^6
            10_000_000,     // 10^7
            100_000_000,    // 10^8
            1_000_000_000,  // 10^9
            10_000_000_000, // 10^10
        ]
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.count = 0;
        self.sum_ns = 0;
        self.min_ns = u64::MAX;
        self.max_ns = 0;
        self.buckets = [0; 11];
    }
}

fn bucket_index(ns: u64) -> usize {
    let bounds = Histogram::bucket_bounds();
    for (i, b) in bounds.iter().enumerate() {
        if ns <= *b {
            return i;
        }
    }
    bounds.len() - 1
}

/// Standardized histogram names (spec plan F.3).
pub mod metric_name {
    /// Pub write latency (user → wire).
    pub const DDS_WRITE_LATENCY: &str = "dds.write.latency";
    /// Sub read latency (wire → user).
    pub const DDS_READ_LATENCY: &str = "dds.read.latency";
    /// Heartbeat round-trip time.
    pub const DDS_HEARTBEAT_RTT: &str = "dds.heartbeat.rtt";
    /// SPDP→SEDP→match duration.
    pub const DDS_DISCOVERY_MATCH_DURATION: &str = "dds.discovery.match.duration";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_invalid_is_zero() {
        assert!(!TraceId::INVALID.is_valid());
        assert!(TraceId([1u8; 16]).is_valid());
    }

    #[test]
    fn span_id_hex_format() {
        let id = SpanId([0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04]);
        assert_eq!(id.to_hex(), "deadbeef01020304");
    }

    #[test]
    fn trace_id_hex_format() {
        let id = TraceId([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]);
        assert_eq!(id.to_hex(), "00112233445566778899aabbccddeeff");
    }

    #[test]
    fn span_context_child_inherits_trace_id() {
        let parent = SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8]));
        let child = SpanContext::child_of(&parent, SpanId([3u8; 8]));
        assert_eq!(child.trace_id, parent.trace_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id));
    }

    #[test]
    fn histogram_records_into_correct_bucket() {
        let mut h = Histogram::new("test");
        h.record_ns(500); // 10^2..10^3 → bucket 3 (≤1_000)
        h.record_ns(50_000); // 10^4..10^5 → bucket 5 (≤100_000)
        h.record_ns(2_000_000_000); // ≤10^10 → bucket 10
        assert_eq!(h.count, 3);
        assert_eq!(h.buckets[3], 1);
        assert_eq!(h.buckets[5], 1);
        assert_eq!(h.buckets[10], 1);
    }

    #[test]
    fn histogram_min_max_mean() {
        let mut h = Histogram::new("test");
        h.record_ns(100);
        h.record_ns(200);
        h.record_ns(300);
        assert_eq!(h.min_ns, 100);
        assert_eq!(h.max_ns, 300);
        assert_eq!(h.mean_ns(), 200);
    }

    #[test]
    fn histogram_reset_clears_state() {
        let mut h = Histogram::new("test");
        h.record_ns(50);
        h.reset();
        assert_eq!(h.count, 0);
        assert_eq!(h.min_ns, u64::MAX);
        assert_eq!(h.max_ns, 0);
        assert_eq!(h.buckets, [0u64; 11]);
    }

    #[test]
    fn histogram_clamps_to_top_bucket_for_huge_values() {
        let mut h = Histogram::new("test");
        h.record_ns(u64::MAX);
        assert_eq!(h.buckets[10], 1);
    }

    #[test]
    fn span_duration() {
        let s = Span {
            context: SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8])),
            name: "x".into(),
            kind: SpanKind::Internal,
            start_unix_ns: 1_000_000,
            end_unix_ns: 1_500_000,
            status: SpanStatus::Ok,
            status_description: None,
            attributes: Vec::new(),
        };
        assert_eq!(s.duration_ns(), 500_000);
    }
}
