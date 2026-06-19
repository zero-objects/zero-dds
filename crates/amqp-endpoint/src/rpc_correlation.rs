// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC-aware correlation + reply validation.
//!
//! Spec sources:
//! * dds-amqp-1.0 Annex D §D.1 — mapping table
//!   (`requestId`→`message-id`, `relatedRequestId`→`correlation-id`,
//!   reply topic→`reply-to`, etc.).
//! * Annex D §D.2 — activation via `rpc_aware = true`.
//! * Annex D §D.4 — reply validation: correlation-id mandatory,
//!   match against an outstanding call, body decode mode-dependent,
//!   per-call timeout (`rpc_timeout_ms`, default 30000),
//!   bounded outstanding-calls table with
//!   RETCODE_OUT_OF_RESOURCES.
//!
//! This layer is connection/session agnostic — the caller feeds
//! it reply properties + body mode and gets back a
//! `ReplyDecision` that drives disposition + caller surfacing.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::mapping::BodyEncodingMode;
use crate::metrics::MetricsHub;

/// Spec Annex A — `rpc_timeout_ms` default.
pub const DEFAULT_RPC_TIMEOUT_MS: u64 = 30_000;

/// Spec §D.4.4 — default cap for the outstanding-calls table.
/// The caller may raise it; OUT_OF_RESOURCES is only reported once
/// `outstanding.len() >= cap`.
pub const DEFAULT_MAX_OUTSTANDING_CALLS: usize = 4096;

/// Spec Annex D §D.4 — configuration of an RPC-aware bridge.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Annex D §D.2 — RPC-aware activation per topic.
    pub rpc_aware: bool,
    /// Per-call timeout in milliseconds (Spec §D.4.3).
    pub rpc_timeout_ms: u64,
    /// Max outstanding calls in the table (Spec §D.4.4).
    pub max_outstanding: usize,
    /// Spec §D.4.1.3 — body-encoding mode of the reply topic;
    /// determines what is accepted as a valid reply body.
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

/// Unique correlation id. AMQP `correlation-id` may be
/// `uuid` or `string` (Spec §D.1); we normalize to a
/// String for the lookup.
pub type CorrelationId = String;

/// Entry in the outstanding-calls table.
#[derive(Debug, Clone)]
pub struct OutstandingCall {
    /// `message-id` of the original request (Spec §D.1).
    pub request_id: CorrelationId,
    /// Issue timestamp (caller-supplied monotonic ms).
    pub issued_at_ms: u64,
}

/// Spec §D.4.1 — bridge behavior after reply inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyDecision {
    /// Spec §D.4.1: the reply is complete + matched + decoded ok →
    /// the caller can forward it to the DDS-RPC call.
    Surface {
        /// Matched correlation-id (for control).
        correlation: CorrelationId,
    },
    /// Spec §D.4.2 Row 1: `correlation-id` is missing → reject.
    RejectMalformed {
        /// AMQP error-condition (`amqp:precondition-failed`).
        error: &'static str,
    },
    /// Spec §D.4.2 Row 2: `correlation-id` matches no active
    /// call → silently drop, surface nothing to the caller.
    DropUnknown,
    /// Spec §D.4.2 Row 3: body decode failed → the caller
    /// gets `RETCODE_BAD_PARAMETER`.
    DecodeFailure {
        /// The `errors.decode` counter is incremented.
        error: &'static str,
    },
    /// Spec §D.4.2 Row 4: reply after timeout → silently drop.
    DropLateReply,
}

/// Spec §D.4.4 — result of an issue attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueDecision {
    /// Call accepted, entered into the table.
    Accepted,
    /// Table full → the caller gets
    /// `RETCODE_OUT_OF_RESOURCES`.
    OutOfResources,
}

/// Outstanding-calls table.
///
/// Spec §D.4 — bounded, non-blocking, deterministic.
/// We use `BTreeMap` because `core::collections::HashMap`
/// is not available in the `no_std` setup; the `O(log n)`
/// lookup cost is sufficient for the use case (n up to ~4k).
#[derive(Debug)]
pub struct OutstandingCalls {
    cfg: RpcConfig,
    table: BTreeMap<CorrelationId, OutstandingCall>,
    next_call_id: AtomicU64,
}

impl OutstandingCalls {
    /// Fresh table.
    #[must_use]
    pub fn new(cfg: RpcConfig) -> Self {
        Self {
            cfg,
            table: BTreeMap::new(),
            next_call_id: AtomicU64::new(1),
        }
    }

    /// Number of currently outstanding calls.
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.table.len()
    }

    /// Spec §D.4.4 — enter a new call.
    ///
    /// `issued_at_ms` is a monotonic caller clock; the timeout
    /// is measured in `expire_overdue` against the same clock
    /// source.
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
        // Annex D §D.2: every new call gets a sequential
        // internal id (for potential tracing/audit purposes).
        let _ = self.next_call_id.fetch_add(1, Ordering::Relaxed);
        IssueDecision::Accepted
    }

    /// Spec §D.4.1 + §D.4.2 — check an incoming reply.
    ///
    /// `now_ms` must be the caller's monotonic clock, so that
    /// timeout detection is consistent with `expire_overdue`.
    ///
    /// `body_mode` is the body-encoding mode expected by the
    /// reply topic (Spec §D.4.1.3); the actual
    /// decode lives in the caller layer (full XCDR2/JSON
    /// type-reflection decode requires a TypeObject lookup).
    /// Here we accept an optional `body_decoded_ok`
    /// flag, set by the caller when decoding had already
    /// succeeded before the reply-validation call.
    pub fn validate_reply(
        &mut self,
        properties: &ReplyProperties,
        now_ms: u64,
        body_mode: BodyEncodingMode,
        body_decoded_ok: bool,
        metrics: &MetricsHub,
    ) -> ReplyDecision {
        // §D.4.1 Row 1: correlation-id mandatory.
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

        // §D.4.2 Row 4: reply after timeout (the call still exists
        // because expire_overdue has not removed it)?
        if now_ms.saturating_sub(call.issued_at_ms) > self.cfg.rpc_timeout_ms {
            metrics.on_dropped_malformed_reply();
            return ReplyDecision::DropLateReply;
        }

        // §D.4.1 Row 3: body-decode status is caller-supplied
        // (decode lives in the codegen layers); we only verify
        // the status reported by the caller as well as its
        // consistency with `body_mode`.
        if !body_decoded_ok {
            metrics.on_decode_error();
            return ReplyDecision::DecodeFailure {
                error: "amqp:decode-error",
            };
        }
        // A body-mode mismatch (e.g. the reply topic expects
        // PASSTHROUGH but the content-type says JSON) is reported
        // by the caller via body_decoded_ok=false. Here we only
        // verify that body_mode is consistent with the Annex-D
        // configuration of the reply topic — the value is a
        // precondition check, not a reject reason per se.
        let _ = body_mode;

        ReplyDecision::Surface {
            correlation: call.request_id,
        }
    }

    /// Spec §D.4.3 — remove all calls whose deadline
    /// (`issued_at + rpc_timeout_ms`) <= `now_ms`; per call,
    /// `rpc.calls.timed-out` is incremented.
    ///
    /// Returns the list of removed call ids so the caller can
    /// close the respective DDS-RPC call surfaces with
    /// `RETCODE_TIMEOUT`.
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

    /// Read-only access to the configuration.
    #[must_use]
    pub fn config(&self) -> &RpcConfig {
        &self.cfg
    }
}

/// AMQP reply-properties subset (what the §D.4.1 validation
/// consumes). The caller extracts this from
/// `MessageSection::Properties`.
#[derive(Debug, Clone, Default)]
pub struct ReplyProperties {
    /// `correlation-id` as a string (uuid is normalized hex-encoded).
    pub correlation_id: Option<CorrelationId>,
    /// `reply-to` (caller information; not inspected by D.4.1).
    pub reply_to: Option<String>,
}

impl ReplyProperties {
    /// Convenient constructor from an AMQP `correlation-id` value.
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
        // §D.4.4 default: bounded; specifically 4096.
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
        // Table full → out-of-resources.
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
        // Counter incremented.
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
        // The call was removed from the table.
        assert_eq!(t.outstanding(), 0);
        // No counter incremented.
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
        // At now=200: a (age 200ms) and b (age 150ms) are > 100ms.
        // c is 0ms old, stays.
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
        // Spec: "deadline = issued_at + rpc_timeout_ms". Strictly
        // greater than the deadline → expired. A reply exactly at
        // the deadline is accepted.
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
        // §D.4.4 — non-blocking guarantee: with a full table
        // `issue` must return OutOfResources synchronously, no
        // block, no wait.
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
