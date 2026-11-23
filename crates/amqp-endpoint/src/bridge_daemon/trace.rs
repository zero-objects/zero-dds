// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OTLP spans of a TRANSFER round trip (spec §8.3): the root
//! `amqp.transfer` with one DDS child.
//!
//! * AMQP → DDS: `amqp.transfer` (`SpanKind::Server`) from the transfer
//!   read from the connection to its disposition sent; the child
//!   `dds.write` (`SpanKind::Client`) around the write of the sample.
//! * DDS → AMQP: `amqp.transfer` (`SpanKind::Client`) from the sample
//!   taken from the DDS reader to the broker's disposition, or to the
//!   transfer sent when the link sends settled; the child `dds.read`
//!   (`SpanKind::Internal`) at the moment the sample was taken.
//!
//! The pump of a participant runs in one thread; the tracer of the
//! participant is that thread's ([`set_tracer`]), as its log context is.

use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::vec::Vec;

use zerodds_observability_otlp::OtlpExporter;

/// Sends the spans of the transfers to an OTLP exporter; without one it
/// sends nothing.
#[derive(Clone, Default)]
pub struct Tracer {
    exporter: Option<Arc<OtlpExporter>>,
}

impl Tracer {
    /// A tracer sending to `exporter`.
    #[must_use]
    pub fn new(exporter: Option<Arc<OtlpExporter>>) -> Self {
        Self { exporter }
    }

    /// Whether spans go anywhere.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.exporter.is_some()
    }
}

/// The direction of a round trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    /// A transfer of the broker written to DDS.
    In,
    /// A DDS sample sent to the broker.
    Out,
}

/// One TRANSFER round trip, its times in nanoseconds since the Unix epoch.
#[derive(Debug, Clone)]
pub(super) struct Transfer<'a> {
    pub(super) direction: Direction,
    /// In: the transfer read; out: the sample taken.
    pub(super) start_ns: u64,
    /// The disposition sent (in) or received (out); the transfer sent when
    /// the link sends settled.
    pub(super) end_ns: u64,
    /// The DDS child: in, the write from, to; out, the take at (from = to).
    pub(super) dds: Option<(u64, u64)>,
    pub(super) dds_topic: &'a str,
    pub(super) amqp_address: &'a str,
    pub(super) link_name: &'a str,
    pub(super) delivery_id: u32,
    /// `accepted`, `rejected`, `released`, `modified`, or `settled` for a
    /// transfer sent settled, which has no outcome.
    pub(super) outcome: &'a str,
    /// The error condition of a rejection.
    pub(super) error: Option<String>,
}

std::thread_local! {
    static TRACER: std::cell::RefCell<Tracer> = std::cell::RefCell::new(Tracer::default());
}

/// The tracer of the participant this thread runs.
pub(super) fn set_tracer(t: Tracer) {
    TRACER.with(|c| *c.borrow_mut() = t);
}

/// Whether this thread's tracer sends spans anywhere.
pub(super) fn tracing() -> bool {
    TRACER.with(|c| c.borrow().enabled())
}

/// Sends the spans of `x` with this thread's tracer.
pub(super) fn record(x: &Transfer<'_>) {
    TRACER.with(|c| {
        if let Some(exporter) = &c.borrow().exporter {
            for span in spans(x) {
                exporter.add_span(span);
            }
        }
    });
}

/// The spans of a round trip: the DDS child, then the root `amqp.transfer`.
pub(super) fn spans(x: &Transfer<'_>) -> Vec<zerodds_foundation::tracing::Span> {
    use zerodds_foundation::observability::Attribute;
    use zerodds_foundation::tracing::{Span, SpanContext, SpanKind, SpanStatus};
    let root = SpanContext::new_root(new_trace_id(), new_span_id());
    let ok = x.outcome == "accepted" || x.outcome == "settled";
    let status = if ok {
        SpanStatus::Ok
    } else {
        SpanStatus::Error
    };
    let attr = |key: &'static str, value: String| Attribute { key, value };
    let mut attributes = vec![
        attr("messaging.system", "amqp".into()),
        attr(
            "messaging.operation",
            match x.direction {
                Direction::In => "receive",
                Direction::Out => "publish",
            }
            .into(),
        ),
        attr("messaging.destination.name", x.amqp_address.to_string()),
        attr("amqp.link_name", x.link_name.to_string()),
        attr("amqp.delivery_id", x.delivery_id.to_string()),
        attr("amqp.outcome", x.outcome.to_string()),
        attr("dds.topic", x.dds_topic.to_string()),
    ];
    if let Some(e) = &x.error {
        attributes.push(attr("amqp.error_condition", e.clone()));
    }
    let mut out = Vec::new();
    if let Some((from, to)) = x.dds {
        let (name, kind) = match x.direction {
            Direction::In => ("dds.write", SpanKind::Client),
            Direction::Out => ("dds.read", SpanKind::Internal),
        };
        out.push(Span {
            context: SpanContext::child_of(&root, new_span_id()),
            name: name.to_string(),
            kind,
            start_unix_ns: from,
            end_unix_ns: to,
            status,
            status_description: None,
            attributes: vec![attr("dds.topic", x.dds_topic.to_string())],
        });
    }
    out.push(Span {
        context: root,
        name: "amqp.transfer".to_string(),
        kind: match x.direction {
            Direction::In => SpanKind::Server,
            Direction::Out => SpanKind::Client,
        },
        start_unix_ns: x.start_ns,
        end_unix_ns: x.end_ns.max(x.start_ns),
        status,
        status_description: x.error.clone(),
        attributes,
    });
    out
}

/// Nanoseconds since the Unix epoch.
#[must_use]
pub(super) fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// Mixed into the span ids: a collector only needs to tell them apart.
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn new_trace_id() -> zerodds_foundation::tracing::TraceId {
    let mut bytes = [0u8; 16];
    let random = zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(&mut bytes)
        .is_ok();
    if !random || bytes.iter().all(|b| *b == 0) {
        bytes = u128::from(unix_ns())
            .wrapping_add(u128::from(SPAN_COUNTER.load(Ordering::Relaxed)))
            .max(1)
            .to_be_bytes();
    }
    zerodds_foundation::tracing::TraceId(bytes)
}

fn new_span_id() -> zerodds_foundation::tracing::SpanId {
    let n = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n ^ (u64::from(std::process::id()).rotate_left(32));
    let mut bytes = mixed.to_be_bytes();
    if bytes.iter().all(|b| *b == 0) {
        bytes[7] = 1;
    }
    zerodds_foundation::tracing::SpanId(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_foundation::tracing::{SpanKind, SpanStatus};

    fn inbound() -> Transfer<'static> {
        Transfer {
            direction: Direction::In,
            start_ns: 100,
            end_ns: 900,
            dds: Some((300, 400)),
            dds_topic: "Chat::Message",
            amqp_address: "/queues/chat",
            link_name: "zd/Chat::Message/receiver",
            delivery_id: 7,
            outcome: "accepted",
            error: None,
        }
    }

    fn value<'s>(s: &'s zerodds_foundation::tracing::Span, key: &str) -> Option<&'s str> {
        s.attributes
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.as_str())
    }

    /// §8.3: an inbound transfer is the root `amqp.transfer` from the
    /// transfer to its disposition, with the child `dds.write` around the
    /// write, both in one trace.
    #[test]
    fn amqpbridge_8_3_an_inbound_transfer_is_a_span_with_a_dds_write_child() {
        let s = spans(&inbound());
        let names: Vec<&str> = s.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["dds.write", "amqp.transfer"]);
        let (child, root) = (&s[0], &s[1]);
        assert!(matches!(root.kind, SpanKind::Server));
        assert!(matches!(child.kind, SpanKind::Client));
        assert_eq!(child.context.trace_id, root.context.trace_id);
        assert_eq!(child.context.parent_span_id, Some(root.context.span_id));
        assert_eq!(root.context.parent_span_id, None);
        assert_eq!((root.start_unix_ns, root.end_unix_ns), (100, 900));
        assert_eq!((child.start_unix_ns, child.end_unix_ns), (300, 400));
        assert!(matches!(root.status, SpanStatus::Ok));
        assert_eq!(value(root, "messaging.system"), Some("amqp"));
        assert_eq!(value(root, "messaging.operation"), Some("receive"));
        assert_eq!(
            value(root, "messaging.destination.name"),
            Some("/queues/chat")
        );
        assert_eq!(value(root, "amqp.delivery_id"), Some("7"));
        assert_eq!(value(root, "amqp.outcome"), Some("accepted"));
        assert_eq!(value(root, "dds.topic"), Some("Chat::Message"));
        assert_eq!(value(child, "dds.topic"), Some("Chat::Message"));
    }

    /// §8.3: an outbound transfer runs from the sample taken to the
    /// broker's disposition; its child `dds.read` is the moment of the take.
    #[test]
    fn amqpbridge_8_3_an_outbound_transfer_ends_with_its_disposition() {
        let x = Transfer {
            direction: Direction::Out,
            start_ns: 50,
            end_ns: 700,
            dds: Some((50, 50)),
            link_name: "zd/Chat::Message/sender",
            ..inbound()
        };
        let s = spans(&x);
        assert_eq!(s[0].name, "dds.read");
        assert_eq!((s[0].start_unix_ns, s[0].end_unix_ns), (50, 50));
        assert!(matches!(s[1].kind, SpanKind::Client));
        assert_eq!((s[1].start_unix_ns, s[1].end_unix_ns), (50, 700));
        assert_eq!(value(&s[1], "messaging.operation"), Some("publish"));
        // A transfer sent settled has no outcome; it is not an error.
        let settled = spans(&Transfer {
            outcome: "settled",
            ..x
        });
        assert!(matches!(settled[1].status, SpanStatus::Ok));
    }

    /// A rejected, released or modified transfer has status `ERROR`; the
    /// rejection's condition is the status description.
    #[test]
    fn amqpbridge_8_3_a_rejected_transfer_has_status_error() {
        for outcome in ["rejected", "released", "modified"] {
            let s = spans(&Transfer {
                outcome,
                error: (outcome == "rejected").then(|| "amqp:decode-error".to_string()),
                ..inbound()
            });
            assert!(matches!(s[1].status, SpanStatus::Error), "{outcome}");
            assert!(matches!(s[0].status, SpanStatus::Error), "{outcome}");
            if outcome == "rejected" {
                assert_eq!(
                    s[1].status_description.as_deref(),
                    Some("amqp:decode-error")
                );
                assert_eq!(
                    value(&s[1], "amqp.error_condition"),
                    Some("amqp:decode-error")
                );
            }
        }
        // A transfer rejected before any write has no DDS child.
        let s = spans(&Transfer {
            outcome: "rejected",
            dds: None,
            ..inbound()
        });
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "amqp.transfer");
    }

    /// Without an exporter nothing is sent and the thread's tracer is off.
    #[test]
    fn amqpbridge_8_3_a_tracer_without_exporter_is_off() {
        let t = Tracer::new(None);
        assert!(!t.enabled());
        set_tracer(t);
        assert!(!tracing());
        record(&inbound());
        let on = Tracer::new(Some(Arc::new(OtlpExporter::new(Default::default()))));
        assert!(on.enabled());
        set_tracer(on);
        assert!(tracing());
        set_tracer(Tracer::default());
    }

    /// Span ids differ, within a trace and between traces; the traces of
    /// two transfers differ.
    #[test]
    fn amqpbridge_8_3_span_ids_differ() {
        let a = spans(&inbound());
        let b = spans(&inbound());
        let mut ids: Vec<_> = a.iter().chain(&b).map(|s| s.context.span_id.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4);
        assert_ne!(a[1].context.trace_id, b[1].context.trace_id);
    }
}
