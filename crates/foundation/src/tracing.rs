// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spans + Histogramme — grobgranulare Tracing-Primitive.
//!
//! Liefert die in-Crate-Primitive; der `zerodds-observability-otlp`-Crate
//! wandelt sie in OTLP-HTTP-JSON-Frames fuer Collector-Export.
//!
//! ## Design-Linie
//!
//! Wie `observability::Event` sind Spans **grobgranular** — wir
//! instrumentieren Endpoint-Lifecycle und Discovery-Phasen, **nicht**
//! pro-Sample-Latenzen. Hot-Path-Latenzen kommen aus den Histogrammen
//! im Aggregat (`Histogram` hier), nicht aus pro-Sample-Spans.
//!
//! ## Wire-Layer-Mapping
//!
//! | OTel-Konzept    | Hier                          |
//! | --------------- | ----------------------------- |
//! | TraceId         | [`TraceId`] (16 byte)         |
//! | SpanId          | [`SpanId`] (8 byte)           |
//! | Span            | [`Span`]                      |
//! | SpanContext     | [`SpanContext`]               |
//! | Histogram       | [`Histogram`]                 |
//!
//! Kein `tracing`-Crate-Dep — wir bleiben minimal. Adapter zu
//! `tracing-opentelemetry` ist eine optionale Bridge im Konsumenten.

use alloc::string::String;
use alloc::vec::Vec;

use crate::observability::Attribute;

/// 16-Byte W3C-Trace-ID. Niemals all-zero (Zero ist Reserved fuer
/// "Invalid").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TraceId(pub [u8; 16]);

impl TraceId {
    /// Zero-Sentinel, OTel-Spec: invalid.
    pub const INVALID: Self = Self([0u8; 16]);

    /// `true` wenn nicht all-zero.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|b| *b != 0)
    }

    /// Hex-Lowercase-Repr (OTLP-Wire).
    #[must_use]
    pub fn to_hex(&self) -> String {
        bytes_to_hex(&self.0)
    }
}

/// 8-Byte Span-ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanId(pub [u8; 8]);

impl SpanId {
    /// Zero-Sentinel.
    pub const INVALID: Self = Self([0u8; 8]);

    /// `true` wenn nicht all-zero.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|b| *b != 0)
    }

    /// Hex-Lowercase-Repr.
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

/// Kontext der Span-Beziehung — von welcher Trace, mit welchem
/// Parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanContext {
    /// Globale Trace-Identitaet.
    pub trace_id: TraceId,
    /// Eigene Span-ID.
    pub span_id: SpanId,
    /// Parent-Span (None = Root).
    pub parent_span_id: Option<SpanId>,
}

impl SpanContext {
    /// Root-Span: erzeugt SpanContext mit zufaellig gewaehlter
    /// `trace_id` und `span_id` (keine Parent).
    #[must_use]
    pub fn new_root(trace_id: TraceId, span_id: SpanId) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
        }
    }

    /// Child-Span unter einem existierenden Parent.
    #[must_use]
    pub fn child_of(parent: &SpanContext, span_id: SpanId) -> Self {
        Self {
            trace_id: parent.trace_id,
            span_id,
            parent_span_id: Some(parent.span_id),
        }
    }
}

/// Ergebnis-Status eines Spans (OTel-Spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanStatus {
    /// Default: ohne Status-Override.
    Unset,
    /// Span hat Erfolg signalisiert.
    Ok,
    /// Fehler — Beschreibung im Span-Description-Feld.
    Error,
}

/// Span-Kind nach OTel-Spec — wir benutzen Internal/Server/Client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    /// Default fuer in-process Operations.
    Internal,
    /// Eingehende Operation (z.B. SEDP-Discovery-Receive).
    Server,
    /// Ausgehende Operation (z.B. Pub-Write).
    Client,
}

/// Ein abgeschlossener Span.
///
/// In ZeroDDS bauen wir Spans **abgeschlossen** auf — kein
/// scope-guard-Magic. Caller misst Start/Ende manuell, der Sink
/// bekommt das Result.
#[derive(Clone, Debug)]
pub struct Span {
    /// Span-Kontext (Trace+Span-ID + Parent).
    pub context: SpanContext,
    /// Span-Name (z.B. `"dcps.write"`).
    pub name: String,
    /// Span-Kind.
    pub kind: SpanKind,
    /// Start-Time relativ zu einem monotonen Origin (ns).
    pub start_unix_ns: u64,
    /// Ende-Time relativ zum gleichen Origin.
    pub end_unix_ns: u64,
    /// Status.
    pub status: SpanStatus,
    /// Optionale Status-Beschreibung (z.B. Fehler-Message).
    pub status_description: Option<String>,
    /// Attribute, beliebig viele.
    pub attributes: Vec<Attribute>,
}

impl Span {
    /// Span-Dauer in Nanosekunden.
    #[must_use]
    pub fn duration_ns(&self) -> u64 {
        self.end_unix_ns.saturating_sub(self.start_unix_ns)
    }
}

/// Histogram-Primitive — exponentielle Buckets (Powers of 10) plus
/// Sum/Count/Min/Max. Bewusst minimalistisch: kein hdrhistogram-Dep,
/// sondern `[u64; 10]`-Buckets fuer 1ns..10s in 10x-Schritten.
///
/// Für volle p99/p999-Aufloesung kann der Konsument `hdrhistogram`
/// als Sink-Sink benutzen — wir liefern hier nur das aggregat-frei
/// summierbare Format, das OTLP `Histogram` direkt mappt.
#[derive(Clone, Debug)]
pub struct Histogram {
    /// Logischer Name (z.B. `"dds.write.latency"`).
    pub name: String,
    /// Anzahl aller Records.
    pub count: u64,
    /// Summe aller Records (in der Einheit; default: ns).
    pub sum_ns: u64,
    /// Min-Wert.
    pub min_ns: u64,
    /// Max-Wert.
    pub max_ns: u64,
    /// Bucket-Counts: `buckets[i]` = wie viele Records `<= 10^i ns`.
    /// 10 Buckets fuer 1ns (10^0) bis 10s (10^10).
    pub buckets: [u64; 11],
}

impl Histogram {
    /// Konstruktor mit Namen.
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

    /// Misst einen Wert in Nanosekunden.
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

    /// Mittelwert in Nanosekunden, oder 0 wenn count=0.
    #[must_use]
    pub fn mean_ns(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.sum_ns / self.count
        }
    }

    /// Bucket-Boundaries als Array — fuer OTLP-Export.
    /// `bounds[i]` ist der obere Cutoff von `buckets[i]` in Nanosekunden.
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

    /// Reset auf leeren Zustand.
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

/// Standardisierte Histogram-Namen (Spec-Plan F.3).
pub mod metric_name {
    /// Pub-Write-Latenz (User → Wire).
    pub const DDS_WRITE_LATENCY: &str = "dds.write.latency";
    /// Sub-Read-Latenz (Wire → User).
    pub const DDS_READ_LATENCY: &str = "dds.read.latency";
    /// Heartbeat-Roundtrip-Time.
    pub const DDS_HEARTBEAT_RTT: &str = "dds.heartbeat.rtt";
    /// SPDP→SEDP→Match-Dauer.
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
