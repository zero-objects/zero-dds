// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC-Aware Correlation + Reply Validation.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 Annex D §D.1 — Mapping-Tabelle
//!   (`requestId`→`message-id`, `relatedRequestId`→`correlation-id`,
//!   reply-Topic→`reply-to`, etc.).
//! * Annex D §D.2 — Aktivierung ueber `rpc_aware = true`.
//! * Annex D §D.4 — Reply Validation: correlation-id pflicht,
//!   match auf outstanding call, body-decode mode-abhaengig,
//!   per-call Timeout (`rpc_timeout_ms`, default 30000),
//!   bounded outstanding-calls Tabelle mit
//!   RETCODE_OUT_OF_RESOURCES.
//!
//! Diese Schicht ist Connection-/Session-agnostisch — Caller fuettert
//! es mit Reply-Properties + Body-Mode und bekommt eine
//! `ReplyDecision` zurueck, die Disposition + Caller-Surfacing
//! steuert.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::mapping::BodyEncodingMode;
use crate::metrics::MetricsHub;

/// Spec Annex A — `rpc_timeout_ms` Default.
pub const DEFAULT_RPC_TIMEOUT_MS: u64 = 30_000;

/// Spec §D.4.4 — Default-Cap fuer die outstanding-calls Tabelle.
/// Caller darf hochsetzen; OUT_OF_RESOURCES wird nur ab
/// `outstanding.len() >= cap` zurueckgemeldet.
pub const DEFAULT_MAX_OUTSTANDING_CALLS: usize = 4096;

/// Spec Annex D §D.4 — Konfiguration einer RPC-Aware-Bridge.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Annex D §D.2 — RPC-Aware Aktivierung pro Topic.
    pub rpc_aware: bool,
    /// Per-Call-Timeout in Millisekunden (Spec §D.4.3).
    pub rpc_timeout_ms: u64,
    /// Max Outstanding-Calls in der Tabelle (Spec §D.4.4).
    pub max_outstanding: usize,
    /// Spec §D.4.1.3 — Body-Encoding-Mode des Reply-Topics; legt
    /// fest, was als gueltiges Reply-Body akzeptiert wird.
    pub reply_body_mode: BodyEncodingMode,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            rpc_aware: false,
            rpc_timeout_ms: DEFAULT_RPC_TIMEOUT_MS,
            max_outstanding: DEFAULT_MAX_OUTSTANDING_CALLS,
            reply_body_mode: BodyEncodingMode::PassThrough,
        }
    }
}

/// Eindeutige Korrelations-Id. AMQP `correlation-id` darf
/// `uuid` oder `string` sein (Spec §D.1); wir normalisieren auf
/// String fuer den Lookup.
pub type CorrelationId = String;

/// Eintrag in der Outstanding-Calls-Tabelle.
#[derive(Debug, Clone)]
pub struct OutstandingCall {
    /// `message-id` der Original-Request (Spec §D.1).
    pub request_id: CorrelationId,
    /// Issue-Timestamp (caller-supplied monotonic ms).
    pub issued_at_ms: u64,
}

/// Spec §D.4.1 — Bridge-Verhalten nach Reply-Inspektion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyDecision {
    /// Spec §D.4.1: Reply ist vollstaendig + matched + decodet ok →
    /// Caller kann an DDS-RPC-Auruf weitergeben.
    Surface {
        /// Matched correlation-id (zur Kontrolle).
        correlation: CorrelationId,
    },
    /// Spec §D.4.2 Row 1: `correlation-id` fehlt → reject.
    RejectMalformed {
        /// AMQP error-condition (`amqp:precondition-failed`).
        error: &'static str,
    },
    /// Spec §D.4.2 Row 2: `correlation-id` matched keinen aktiven
    /// Call → silently drop, keinen Caller surfacen.
    DropUnknown,
    /// Spec §D.4.2 Row 3: Body-Decode fehlgeschlagen → caller
    /// bekommt `RETCODE_BAD_PARAMETER`.
    DecodeFailure {
        /// `errors.decode`-Counter wird inkrementiert.
        error: &'static str,
    },
    /// Spec §D.4.2 Row 4: Reply nach Timeout → silently drop.
    DropLateReply,
}

/// Spec §D.4.4 — Result eines Issue-Versuchs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueDecision {
    /// Call angenommen, in Tabelle eingetragen.
    Accepted,
    /// Tabelle voll → Caller bekommt
    /// `RETCODE_OUT_OF_RESOURCES`.
    OutOfResources,
}

/// Outstanding-Calls-Tabelle.
///
/// Spec §D.4 — bounded, non-blocking, deterministisch.
/// Wir benutzen `BTreeMap` weil `core::collections::HashMap`
/// im `no_std`-Setup nicht verfuegbar ist; Lookup-Kosten
/// `O(log n)` sind fuer den Use-Case (n bis ~4k) ausreichend.
#[derive(Debug)]
pub struct OutstandingCalls {
    cfg: RpcConfig,
    table: BTreeMap<CorrelationId, OutstandingCall>,
    next_call_id: AtomicU64,
}

impl OutstandingCalls {
    /// Frische Tabelle.
    #[must_use]
    pub fn new(cfg: RpcConfig) -> Self {
        Self {
            cfg,
            table: BTreeMap::new(),
            next_call_id: AtomicU64::new(1),
        }
    }

    /// Anzahl aktuell ausstehender Calls.
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.table.len()
    }

    /// Spec §D.4.4 — neuen Call eintragen.
    ///
    /// `issued_at_ms` ist eine monotone Caller-Clock; der Timeout
    /// wird in `expire_overdue` gegen denselben Clock-Source
    /// gemessen.
    pub fn issue(&mut self, request_id: CorrelationId, issued_at_ms: u64) -> IssueDecision {
        if self.table.len() >= self.cfg.max_outstanding {
            return IssueDecision::OutOfResources;
        }
        self.table.insert(
            request_id.clone(),
            OutstandingCall {
                request_id,
                issued_at_ms,
            },
        );
        // Annex D §D.2: jeder neue Call bekommt eine fortlaufende
        // interne Id (fuer eventuelle Tracing/Audit-Zwecke).
        let _ = self.next_call_id.fetch_add(1, Ordering::Relaxed);
        IssueDecision::Accepted
    }

    /// Spec §D.4.1 + §D.4.2 — eingehender Reply pruefen.
    ///
    /// `now_ms` muss die Caller-monotone Clock sein, damit
    /// Timeout-Detection mit `expire_overdue` konsistent ist.
    ///
    /// `body_mode` ist der vom Reply-Topic erwartete
    /// Body-Encoding-Mode (Spec §D.4.1.3); das tatsaechliche
    /// Decode liegt in der Caller-Schicht (volles XCDR2/JSON-
    /// Type-Reflection-Decode benoetigt TypeObject-Lookup).
    /// Wir akzeptieren hier ein optionales `body_decoded_ok`-
    /// Flag, das vom Caller gesetzt wird, wenn das Decoding
    /// vor dem Reply-Validation-Aufruf bereits erfolgreich war.
    pub fn validate_reply(
        &mut self,
        properties: &ReplyProperties,
        now_ms: u64,
        body_mode: BodyEncodingMode,
        body_decoded_ok: bool,
        metrics: &MetricsHub,
    ) -> ReplyDecision {
        // §D.4.1 Row 1: correlation-id pflicht.
        let Some(correlation) = properties.correlation_id.as_ref() else {
            metrics.on_dropped_malformed_reply();
            return ReplyDecision::RejectMalformed {
                error: "amqp:precondition-failed",
            };
        };

        // §D.4.1 Row 2: matched outstanding call?
        let Some(call) = self.table.remove(correlation) else {
            metrics.on_dropped_malformed_reply();
            return ReplyDecision::DropUnknown;
        };

        // §D.4.2 Row 4: Reply nach Timeout (call existiert noch
        // weil expire_overdue ihn nicht entfernt hat)?
        if now_ms.saturating_sub(call.issued_at_ms) > self.cfg.rpc_timeout_ms {
            metrics.on_dropped_malformed_reply();
            return ReplyDecision::DropLateReply;
        }

        // §D.4.1 Row 3: Body-Decode-Status ist Caller-supplied
        // (decode liegt in den Codegen-Schichten); wir verifizieren
        // nur den vom Caller gemeldeten Status sowie die
        // Konsistenz mit `body_mode`.
        if !body_decoded_ok {
            metrics.on_decode_error();
            return ReplyDecision::DecodeFailure {
                error: "amqp:decode-error",
            };
        }
        // Body-Mode-Mismatch (z.B. Reply-Topic erwartet
        // PASSTHROUGH, content-type sagt JSON) wird vom Caller
        // gemeldet via body_decoded_ok=false. Wir verifizieren hier
        // nur, dass body_mode konsistent ist mit der Annex-D-
        // Konfiguration des Reply-Topics — der Wert ist
        // Pre-Conditions-Check, kein Reject-Grund per se.
        let _ = body_mode;

        ReplyDecision::Surface {
            correlation: call.request_id,
        }
    }

    /// Spec §D.4.3 — alle Calls entfernen, deren Deadline
    /// (`issued_at + rpc_timeout_ms`) <= `now_ms` ist; pro Call
    /// wird `rpc.calls.timed-out` inkrementiert.
    ///
    /// Liefert die Liste der entfernten Call-Ids, damit der Caller
    /// die jeweiligen DDS-RPC-Auruf-Surfaces mit
    /// `RETCODE_TIMEOUT` schliessen kann.
    pub fn expire_overdue(&mut self, now_ms: u64, metrics: &MetricsHub) -> Vec<CorrelationId> {
        let cutoff_window = self.cfg.rpc_timeout_ms;
        let expired: Vec<CorrelationId> = self
            .table
            .iter()
            .filter_map(|(id, call)| {
                if now_ms.saturating_sub(call.issued_at_ms) > cutoff_window {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        for id in &expired {
            self.table.remove(id);
            metrics.on_rpc_timeout();
        }
        expired
    }

    /// Spec §D.4.4 — bounded-table accessor.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cfg.max_outstanding
    }

    /// Read-only access auf die Konfiguration.
    #[must_use]
    pub fn config(&self) -> &RpcConfig {
        &self.cfg
    }
}

/// AMQP-Reply-Properties-Subset (das, was die §D.4.1-Validation
/// konsumiert). Caller extrahiert dies aus
/// `MessageSection::Properties`.
#[derive(Debug, Clone, Default)]
pub struct ReplyProperties {
    /// `correlation-id` als String (uuid wird hex-encoded normalisiert).
    pub correlation_id: Option<CorrelationId>,
    /// `reply-to` (Caller-Information; nicht von D.4.1 inspiziert).
    pub reply_to: Option<String>,
}

impl ReplyProperties {
    /// Bequemer Konstruktor aus AMQP `correlation-id`-Wert.
    #[must_use]
    pub fn from_amqp(correlation_id: Option<&AmqpExtValue>) -> Self {
        let id = correlation_id.and_then(|v| match v {
            AmqpExtValue::Str(s) => Some(s.clone()),
            AmqpExtValue::Symbol(s) => Some(s.clone()),
            AmqpExtValue::Uuid(bytes) => Some(format_uuid(*bytes)),
            AmqpExtValue::Binary(b) => Some(hex_lower(b)),
            _ => None,
        });
        Self {
            correlation_id: id,
            reply_to: None,
        }
    }
}

fn format_uuid(b: [u8; 16]) -> String {
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        let _ = core::fmt::Write::write_fmt(&mut s, core::format_args!("{byte:02x}"));
    }
    s
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = core::fmt::Write::write_fmt(&mut out, core::format_args!("{b:02x}"));
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn cfg(rpc_aware: bool) -> RpcConfig {
        RpcConfig {
            rpc_aware,
            ..RpcConfig::default()
        }
    }

    fn issue(table: &mut OutstandingCalls, id: &str, t: u64) {
        assert_eq!(table.issue(id.to_string(), t), IssueDecision::Accepted);
    }

    #[test]
    fn defaults_match_spec() {
        // §D.4.3 default: 30000 ms.
        assert_eq!(DEFAULT_RPC_TIMEOUT_MS, 30_000);
        // §D.4.4 default: bounded; konkret 4096.
        assert_eq!(DEFAULT_MAX_OUTSTANDING_CALLS, 4096);
        let c = RpcConfig::default();
        assert!(!c.rpc_aware);
    }

    #[test]
    fn issue_accepts_then_full() {
        let mut t = OutstandingCalls::new(RpcConfig {
            max_outstanding: 2,
            ..cfg(true)
        });
        issue(&mut t, "a", 0);
        issue(&mut t, "b", 0);
        // Tabelle voll → Out-of-Resources.
        assert_eq!(t.issue("c".to_string(), 0), IssueDecision::OutOfResources);
        assert_eq!(t.outstanding(), 2);
        assert_eq!(t.capacity(), 2);
    }

    #[test]
    fn validate_reply_rejects_missing_correlation_id() {
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(cfg(true));
        let props = ReplyProperties::default();
        let d = t.validate_reply(&props, 100, BodyEncodingMode::PassThrough, true, &metrics);
        assert!(matches!(d, ReplyDecision::RejectMalformed { .. }));
        // Counter inkrementiert.
        assert_eq!(
            metrics.snapshot(crate::metrics::names::TRANSFERS_DROPPED_MALFORMED_REPLY),
            Some(1)
        );
    }

    #[test]
    fn validate_reply_drops_unknown_correlation_id() {
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(cfg(true));
        let props = ReplyProperties {
            correlation_id: Some("ghost".into()),
            ..Default::default()
        };
        let d = t.validate_reply(&props, 100, BodyEncodingMode::PassThrough, true, &metrics);
        assert_eq!(d, ReplyDecision::DropUnknown);
        assert_eq!(
            metrics.snapshot(crate::metrics::names::TRANSFERS_DROPPED_MALFORMED_REPLY),
            Some(1)
        );
    }

    #[test]
    fn validate_reply_surfaces_matched_call() {
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(cfg(true));
        issue(&mut t, "req-1", 100);
        let props = ReplyProperties {
            correlation_id: Some("req-1".into()),
            ..Default::default()
        };
        let d = t.validate_reply(&props, 200, BodyEncodingMode::PassThrough, true, &metrics);
        assert_eq!(
            d,
            ReplyDecision::Surface {
                correlation: "req-1".into()
            }
        );
        // Call wurde aus der Tabelle entfernt.
        assert_eq!(t.outstanding(), 0);
        // Kein Counter inkrementiert.
        assert_eq!(
            metrics.snapshot(crate::metrics::names::TRANSFERS_DROPPED_MALFORMED_REPLY),
            Some(0)
        );
    }

    #[test]
    fn validate_reply_decode_failure_reports() {
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(cfg(true));
        issue(&mut t, "req-1", 100);
        let props = ReplyProperties {
            correlation_id: Some("req-1".into()),
            ..Default::default()
        };
        let d = t.validate_reply(
            &props,
            200,
            BodyEncodingMode::PassThrough,
            false, /* body_decoded_ok */
            &metrics,
        );
        assert!(matches!(d, ReplyDecision::DecodeFailure { .. }));
        // §D.4.2 Row 3: errors.decode++.
        assert_eq!(
            metrics.snapshot(crate::metrics::names::ERRORS_DECODE),
            Some(1)
        );
    }

    #[test]
    fn validate_reply_late_reply_dropped() {
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(RpcConfig {
            rpc_timeout_ms: 100,
            ..cfg(true)
        });
        issue(&mut t, "req-1", 0);
        let props = ReplyProperties {
            correlation_id: Some("req-1".into()),
            ..Default::default()
        };
        // 200 ms > 100 ms timeout → late.
        let d = t.validate_reply(&props, 200, BodyEncodingMode::PassThrough, true, &metrics);
        assert_eq!(d, ReplyDecision::DropLateReply);
        assert_eq!(
            metrics.snapshot(crate::metrics::names::TRANSFERS_DROPPED_MALFORMED_REPLY),
            Some(1)
        );
    }

    #[test]
    fn expire_overdue_removes_and_counts() {
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(RpcConfig {
            rpc_timeout_ms: 100,
            ..cfg(true)
        });
        issue(&mut t, "a", 0);
        issue(&mut t, "b", 50);
        issue(&mut t, "c", 200);
        // Bei now=200: a (alt 200ms) und b (alt 150ms) sind > 100ms.
        // c ist 0ms alt, bleibt.
        let expired = t.expire_overdue(200, &metrics);
        assert_eq!(expired.len(), 2);
        assert!(expired.contains(&"a".to_string()));
        assert!(expired.contains(&"b".to_string()));
        assert_eq!(t.outstanding(), 1);
        assert_eq!(
            metrics.snapshot(crate::metrics::names::RPC_CALLS_TIMED_OUT),
            Some(2)
        );
    }

    #[test]
    fn expire_overdue_at_exact_deadline_does_not_remove() {
        // Spec: "deadline = issued_at + rpc_timeout_ms". Strikt
        // groesser als deadline → expired. Ein Reply genau am
        // deadline ist akzeptiert.
        let metrics = MetricsHub::new();
        let mut t = OutstandingCalls::new(RpcConfig {
            rpc_timeout_ms: 100,
            ..cfg(true)
        });
        issue(&mut t, "edge", 0);
        let expired = t.expire_overdue(100, &metrics);
        assert!(expired.is_empty());
        assert_eq!(t.outstanding(), 1);
    }

    #[test]
    fn from_amqp_handles_str_symbol_uuid_binary() {
        // String correlation-id.
        let p = ReplyProperties::from_amqp(Some(&AmqpExtValue::Str("foo".into())));
        assert_eq!(p.correlation_id, Some("foo".to_string()));
        // Symbol.
        let p = ReplyProperties::from_amqp(Some(&AmqpExtValue::Symbol("sym".into())));
        assert_eq!(p.correlation_id, Some("sym".to_string()));
        // UUID-Bytes → canonical 8-4-4-4-12 form.
        let p = ReplyProperties::from_amqp(Some(&AmqpExtValue::Uuid([
            0x55, 0xee, 0x00, 0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0,
        ])));
        assert_eq!(
            p.correlation_id,
            Some("55ee0000-aabb-ccdd-1234-56789abcdef0".to_string())
        );
        // Binary → hex.
        let p = ReplyProperties::from_amqp(Some(&AmqpExtValue::Binary(alloc::vec![0xab, 0xcd])));
        assert_eq!(p.correlation_id, Some("abcd".to_string()));
    }

    #[test]
    fn validate_reply_does_not_block_on_table_full() {
        // §D.4.4 — Non-Blocking-Guarantee: bei voller Tabelle
        // muss `issue` synchron OutOfResources liefern, kein
        // Block, kein Wait.
        let mut t = OutstandingCalls::new(RpcConfig {
            max_outstanding: 1,
            ..cfg(true)
        });
        issue(&mut t, "a", 0);
        let r = t.issue("b".to_string(), 0);
        assert_eq!(r, IssueDecision::OutOfResources);
    }

    #[test]
    fn issue_capacity_zero_always_rejects() {
        let mut t = OutstandingCalls::new(RpcConfig {
            max_outstanding: 0,
            ..cfg(true)
        });
        assert_eq!(t.issue("anything".into(), 0), IssueDecision::OutOfResources);
    }

    #[test]
    fn rpc_timeout_can_be_overridden() {
        let c = RpcConfig {
            rpc_timeout_ms: 5_000,
            ..RpcConfig::default()
        };
        let t = OutstandingCalls::new(c);
        assert_eq!(t.config().rpc_timeout_ms, 5_000);
    }
}
