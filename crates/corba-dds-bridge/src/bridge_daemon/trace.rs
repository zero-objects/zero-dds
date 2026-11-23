// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OTLP spans of a GIOP exchange (spec §8.3): `giop.request`
//! (`SpanKind::Server`) with the children `dds.publish` (the request
//! sample written), `dds.subscribe` (the reply sample taken) and
//! `giop.reply` (the reply written to the connection).

#[allow(unused_imports)]
use super::prelude::*;

use zerodds_observability_otlp::OtlpExporter;

/// Sends the spans of the exchanges to an OTLP exporter; without one it
/// sends nothing.
#[derive(Clone, Default)]
pub struct Tracer {
    exporter: Option<Arc<OtlpExporter>>,
}

/// One GIOP exchange, its times in nanoseconds since the Unix epoch.
#[derive(Debug, Clone, Default)]
pub(crate) struct Exchange {
    /// The request arrived.
    pub(crate) start_ns: u64,
    /// The reply left, or the request was refused.
    pub(crate) end_ns: u64,
    /// `NO_EXCEPTION`.
    pub(crate) ok: bool,
    /// `rpc.system`, `rpc.method`, `corba.repo_id`, `net.peer`,
    /// `giop.version`, `giop.reply_status`, `dds.topic`.
    pub(crate) attributes: Vec<(&'static str, String)>,
    /// The request sample: written from, to.
    pub(crate) publish: Option<(u64, u64)>,
    /// The reply sample: taken at.
    pub(crate) subscribe: Option<u64>,
    /// The reply: written from, to.
    pub(crate) reply: Option<(u64, u64)>,
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

    /// The spans of `x`.
    pub(crate) fn exchange(&self, x: &Exchange) {
        let Some(exporter) = &self.exporter else {
            return;
        };
        for span in spans(x) {
            exporter.add_span(span);
        }
    }
}

/// The spans of an exchange: the root `giop.request` and its children.
pub(crate) fn spans(x: &Exchange) -> Vec<zerodds_foundation::tracing::Span> {
    use zerodds_foundation::observability::Attribute;
    use zerodds_foundation::tracing::{Span, SpanContext, SpanKind, SpanStatus};
    let root = SpanContext::new_root(new_trace_id(), new_span_id());
    let status = if x.ok {
        SpanStatus::Ok
    } else {
        SpanStatus::Error
    };
    let attributes: Vec<Attribute> = x
        .attributes
        .iter()
        .map(|(key, value)| Attribute {
            key,
            value: value.clone(),
        })
        .collect();
    let topic: Vec<Attribute> = attributes
        .iter()
        .filter(|a| a.key == "dds.topic")
        .cloned()
        .collect();
    let child =
        |name: &str, kind: SpanKind, (start, end): (u64, u64), attributes: Vec<Attribute>| Span {
            context: SpanContext::child_of(&root, new_span_id()),
            name: name.to_string(),
            kind,
            start_unix_ns: start,
            end_unix_ns: end,
            status,
            status_description: None,
            attributes,
        };
    let mut out = Vec::new();
    if let Some(p) = x.publish {
        out.push(child("dds.publish", SpanKind::Client, p, topic.clone()));
    }
    if let Some(at) = x.subscribe {
        out.push(child("dds.subscribe", SpanKind::Internal, (at, at), topic));
    }
    if let Some(r) = x.reply {
        out.push(child("giop.reply", SpanKind::Internal, r, Vec::new()));
    }
    out.push(Span {
        context: root,
        name: "giop.request".to_string(),
        kind: SpanKind::Server,
        start_unix_ns: x.start_ns,
        end_unix_ns: x.end_ns,
        status,
        status_description: None,
        attributes,
    });
    out
}

/// Nanoseconds since the Unix epoch.
#[must_use]
pub(crate) fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// Mixed into the span ids: a collector only needs to tell them apart.
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn new_trace_id() -> zerodds_foundation::tracing::TraceId {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() || bytes.iter().all(|b| *b == 0) {
        bytes = u128::from(unix_ns()).max(1).to_be_bytes();
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
    use zerodds_foundation::tracing::SpanKind;

    fn exchange() -> Exchange {
        Exchange {
            start_ns: 100,
            end_ns: 900,
            ok: true,
            attributes: vec![
                ("rpc.method", "add".into()),
                ("dds.topic", "Calc::Add".into()),
            ],
            publish: Some((200, 300)),
            subscribe: Some(600),
            reply: Some((700, 800)),
        }
    }

    /// Spec §8.3: the root `giop.request` and its three children, in one
    /// trace, each a child of the root.
    #[test]
    fn corbabridge_8_3_an_exchange_is_a_request_span_with_three_children() {
        let spans = spans(&exchange());
        let names: Vec<&str> = spans.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["dds.publish", "dds.subscribe", "giop.reply", "giop.request"]
        );
        let root = spans.last().unwrap();
        assert!(matches!(root.kind, SpanKind::Server));
        for c in &spans[..3] {
            assert_eq!(c.context.trace_id, root.context.trace_id);
            assert_eq!(c.context.parent_span_id, Some(root.context.span_id));
        }
        assert_eq!((root.start_unix_ns, root.end_unix_ns), (100, 900));
        assert_eq!((spans[0].start_unix_ns, spans[0].end_unix_ns), (200, 300));
    }

    /// A refused request has no DDS side: the root alone.
    #[test]
    fn corbabridge_8_3_a_refused_request_is_the_root_alone() {
        let x = Exchange {
            ok: false,
            publish: None,
            subscribe: None,
            reply: None,
            ..exchange()
        };
        let spans = spans(&x);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "giop.request");
    }

    /// The DDS children carry the topic, the root every attribute.
    #[test]
    fn corbabridge_8_3_the_dds_children_carry_the_topic() {
        let spans = spans(&exchange());
        assert!(spans[0].attributes.iter().all(|a| a.key == "dds.topic"));
        assert_eq!(spans[3].attributes.len(), 2);
    }

    /// Without an exporter nothing is sent.
    #[test]
    fn corbabridge_8_3_a_tracer_without_exporter_is_off() {
        let t = Tracer::new(None);
        assert!(!t.enabled());
        t.exchange(&exchange());
    }

    /// Span ids differ within a trace.
    #[test]
    fn corbabridge_8_3_span_ids_differ() {
        let spans = spans(&exchange());
        let mut ids: Vec<_> = spans.iter().map(|s| s.context.span_id).collect();
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }
}
