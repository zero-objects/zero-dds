// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OTLP spans of the daemon (spec §8.3), sent through
//! `zerodds-observability-otlp` when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
//!
//! Every WebSocket connection is one trace. Its root `ws.connection`
//! (`SpanKind::Server`) runs from the start of the opening handshake to the
//! end of the connection and is sent when the connection ends. Each message
//! of the connection is a child of the root, sent when it is done:
//!
//! * `ws.frame.in` (`SpanKind::Server`): a client's message, from the
//!   reassembled message to the end of its handling; when the message wrote
//!   into DDS, its child `dds.write` (`SpanKind::Client`) covers the write
//!   of the DataWriter.
//! * `ws.frame.out` (`SpanKind::Client`): a message of the daemon, from
//!   taking it from the connection's send queue (or building it) to the end
//!   of its socket write.
//!
//! Ping, Pong and Close frames are no span.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use zerodds_foundation::observability::Attribute;
use zerodds_foundation::tracing::{Span, SpanContext, SpanId, SpanKind, SpanStatus, TraceId};
use zerodds_observability_otlp::OtlpExporter;

/// Sends the spans of the connections to an OTLP exporter; without one it
/// sends nothing and costs nothing.
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

    /// Sends what the exporter holds (the end of a shutdown, after the last
    /// connection ended).
    pub fn flush(&self) {
        if let Some(exporter) = &self.exporter {
            // A collector that is not there is no failure of the daemon.
            let _ = exporter.flush();
        }
    }

    /// The span of a connection whose opening handshake started at
    /// `start_ns`; `None` without an exporter.
    #[must_use]
    pub(crate) fn connection(&self, start_ns: u64) -> Option<Arc<ConnectionSpan>> {
        let exporter = Arc::clone(self.exporter.as_ref()?);
        Some(Arc::new(ConnectionSpan {
            exporter,
            context: SpanContext::new_root(new_trace_id(), new_span_id()),
            start_ns,
            state: Mutex::new(ConnectionState::default()),
        }))
    }
}

/// What a connection span learns while the connection runs.
#[derive(Debug, Default)]
struct ConnectionState {
    attributes: Vec<Attribute>,
    /// The reason the connection failed or was refused.
    error: Option<String>,
    /// Sent once.
    ended: bool,
}

/// The root span of one connection; its messages are its children.
pub(crate) struct ConnectionSpan {
    exporter: Arc<OtlpExporter>,
    context: SpanContext,
    start_ns: u64,
    state: Mutex<ConnectionState>,
}

/// A client's message, its times in nanoseconds since the Unix epoch.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameIn {
    /// The message was reassembled.
    pub(crate) start_ns: u64,
    /// Its handling ended.
    pub(crate) end_ns: u64,
    /// The DDS topic of its sample; empty for a message of no topic.
    pub(crate) topic: String,
    /// The payload octets (before compression).
    pub(crate) bytes: usize,
    /// A text message.
    pub(crate) text: bool,
    /// The write into DDS: from, to.
    pub(crate) dds_write: Option<(u64, u64)>,
    /// The reason of the error frame the daemon answered it with.
    pub(crate) refused: Option<String>,
}

/// A message of the daemon.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameOut {
    /// Taken from the send queue (or built).
    pub(crate) start_ns: u64,
    /// Its socket write ended.
    pub(crate) end_ns: u64,
    /// The DDS topic of its sample; empty for a message of no topic.
    pub(crate) topic: String,
    /// The payload octets (before compression).
    pub(crate) bytes: usize,
    /// A text message.
    pub(crate) text: bool,
    /// The write failed.
    pub(crate) failed: Option<String>,
}

impl ConnectionSpan {
    /// Adds attributes to the root (`net.peer`, `ws.path`, …).
    pub(crate) fn set(&self, key: &'static str, value: &str) {
        let mut state = lock(&self.state);
        state.attributes.retain(|a| a.key != key);
        state.attributes.push(Attribute {
            key,
            value: value.to_string(),
        });
    }

    /// The connection failed or its handshake was refused; the first reason
    /// is kept.
    pub(crate) fn fail(&self, reason: &str) {
        let mut state = lock(&self.state);
        if state.error.is_none() {
            state.error = Some(reason.to_string());
        }
    }

    /// The spans of a client's message, sent.
    pub(crate) fn frame_in(&self, x: &FrameIn) {
        for span in frame_in_spans(&self.context, x) {
            self.exporter.add_span(span);
        }
    }

    /// The span of a message of the daemon, sent.
    pub(crate) fn frame_out(&self, x: &FrameOut) {
        self.exporter.add_span(frame_out_span(&self.context, x));
    }

    /// The connection ended at `end_ns`: its root span, sent once.
    pub(crate) fn end(&self, end_ns: u64) {
        let span = {
            let mut state = lock(&self.state);
            if state.ended {
                return;
            }
            state.ended = true;
            connection_span(
                &self.context,
                self.start_ns,
                end_ns,
                &state.attributes,
                state.error.as_deref(),
            )
        };
        self.exporter.add_span(span);
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

fn status_of(error: Option<&str>) -> (SpanStatus, Option<String>) {
    match error {
        Some(reason) => (SpanStatus::Error, Some(reason.to_string())),
        None => (SpanStatus::Ok, None),
    }
}

fn attribute(key: &'static str, value: impl Into<String>) -> Attribute {
    Attribute {
        key,
        value: value.into(),
    }
}

/// The root span of a connection.
pub(crate) fn connection_span(
    context: &SpanContext,
    start_ns: u64,
    end_ns: u64,
    attributes: &[Attribute],
    error: Option<&str>,
) -> Span {
    let (status, status_description) = status_of(error);
    Span {
        context: *context,
        name: "ws.connection".to_string(),
        kind: SpanKind::Server,
        start_unix_ns: start_ns,
        end_unix_ns: end_ns.max(start_ns),
        status,
        status_description,
        attributes: attributes.to_vec(),
    }
}

/// The attributes of a message: `dds.topic` (when it has one),
/// `ws.message.type` (`text` or `binary`) and `ws.message.bytes`.
fn message_attributes(topic: &str, text: bool, bytes: usize) -> Vec<Attribute> {
    let mut out = Vec::with_capacity(3);
    if !topic.is_empty() {
        out.push(attribute("dds.topic", topic));
    }
    out.push(attribute(
        "ws.message.type",
        if text { "text" } else { "binary" },
    ));
    out.push(attribute("ws.message.bytes", bytes.to_string()));
    out
}

/// `ws.frame.in` and, when the message wrote into DDS, its child
/// `dds.write`; the child first.
pub(crate) fn frame_in_spans(connection: &SpanContext, x: &FrameIn) -> Vec<Span> {
    let (status, status_description) = status_of(x.refused.as_deref());
    let frame = SpanContext::child_of(connection, new_span_id());
    let mut out = Vec::with_capacity(2);
    if let Some((start, end)) = x.dds_write {
        let mut attributes = Vec::with_capacity(1);
        if !x.topic.is_empty() {
            attributes.push(attribute("dds.topic", x.topic.as_str()));
        }
        out.push(Span {
            context: SpanContext::child_of(&frame, new_span_id()),
            name: "dds.write".to_string(),
            kind: SpanKind::Client,
            start_unix_ns: start,
            end_unix_ns: end.max(start),
            status: SpanStatus::Ok,
            status_description: None,
            attributes,
        });
    }
    out.push(Span {
        context: frame,
        name: "ws.frame.in".to_string(),
        kind: SpanKind::Server,
        start_unix_ns: x.start_ns,
        end_unix_ns: x.end_ns.max(x.start_ns),
        status,
        status_description,
        attributes: message_attributes(&x.topic, x.text, x.bytes),
    });
    out
}

/// `ws.frame.out`.
pub(crate) fn frame_out_span(connection: &SpanContext, x: &FrameOut) -> Span {
    let (status, status_description) = status_of(x.failed.as_deref());
    Span {
        context: SpanContext::child_of(connection, new_span_id()),
        name: "ws.frame.out".to_string(),
        kind: SpanKind::Client,
        start_unix_ns: x.start_ns,
        end_unix_ns: x.end_ns.max(x.start_ns),
        status,
        status_description,
        attributes: message_attributes(&x.topic, x.text, x.bytes),
    }
}

/// Nanoseconds since the Unix epoch.
#[must_use]
pub(crate) fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// The time since the Unix epoch of a moment `elapsed` before now.
#[must_use]
pub(crate) fn unix_ns_since(since: std::time::Instant) -> u64 {
    let elapsed = u64::try_from(since.elapsed().as_nanos()).unwrap_or(u64::MAX);
    unix_ns().saturating_sub(elapsed)
}

/// Mixed into the span ids: a collector only needs to tell them apart.
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn new_trace_id() -> TraceId {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() || bytes.iter().all(|b| *b == 0) {
        bytes = u128::from(unix_ns()).max(1).to_be_bytes();
    }
    TraceId(bytes)
}

fn new_span_id() -> SpanId {
    let n = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n ^ (u64::from(std::process::id()).rotate_left(32));
    let mut bytes = mixed.to_be_bytes();
    if bytes.iter().all(|b| *b == 0) {
        bytes[7] = 1;
    }
    SpanId(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn root() -> SpanContext {
        SpanContext::new_root(new_trace_id(), new_span_id())
    }

    fn written() -> FrameIn {
        FrameIn {
            start_ns: 100,
            end_ns: 400,
            topic: "Chat::Message".into(),
            bytes: 42,
            text: false,
            dds_write: Some((200, 300)),
            refused: None,
        }
    }

    /// Spec §8.3: a client's message that wrote into DDS is `ws.frame.in`,
    /// a child of the connection, with the child `dds.write`, all in the
    /// connection's trace.
    #[test]
    fn wsbridge_8_3_a_frame_span_is_a_child_of_its_connection_span() {
        let connection = root();
        let spans = frame_in_spans(&connection, &written());
        let names: Vec<&str> = spans.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["dds.write", "ws.frame.in"]);
        let frame = &spans[1];
        let write = &spans[0];
        assert_eq!(frame.context.trace_id, connection.trace_id);
        assert_eq!(frame.context.parent_span_id, Some(connection.span_id));
        assert_eq!(write.context.trace_id, connection.trace_id);
        assert_eq!(write.context.parent_span_id, Some(frame.context.span_id));
        assert_eq!((frame.start_unix_ns, frame.end_unix_ns), (100, 400));
        assert_eq!((write.start_unix_ns, write.end_unix_ns), (200, 300));
        assert!(matches!(frame.kind, SpanKind::Server));
        assert!(matches!(write.kind, SpanKind::Client));
    }

    /// The message's attributes: its topic, its type and its octets; the
    /// DDS write carries the topic.
    #[test]
    fn wsbridge_8_3_a_frame_span_names_topic_type_and_octets() {
        let spans = frame_in_spans(&root(), &written());
        let value = |s: &Span, k: &str| {
            s.attributes
                .iter()
                .find(|a| a.key == k)
                .map(|a| a.value.clone())
        };
        assert_eq!(
            value(&spans[1], "dds.topic").as_deref(),
            Some("Chat::Message")
        );
        assert_eq!(
            value(&spans[1], "ws.message.type").as_deref(),
            Some("binary")
        );
        assert_eq!(value(&spans[1], "ws.message.bytes").as_deref(), Some("42"));
        assert_eq!(
            value(&spans[0], "dds.topic").as_deref(),
            Some("Chat::Message")
        );
    }

    /// A message of no topic that wrote nothing (a control message) is the
    /// frame span alone, without `dds.topic`.
    #[test]
    fn wsbridge_8_3_a_control_message_has_no_dds_write() {
        let x = FrameIn {
            topic: String::new(),
            text: true,
            dds_write: None,
            ..written()
        };
        let spans = frame_in_spans(&root(), &x);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "ws.frame.in");
        assert!(spans[0].attributes.iter().all(|a| a.key != "dds.topic"));
    }

    /// A refused message is an error span with the reason of its error
    /// frame.
    #[test]
    fn wsbridge_8_3_a_refused_message_is_an_error_span() {
        let x = FrameIn {
            dds_write: None,
            refused: Some("write on Chat::Message is not granted".into()),
            ..written()
        };
        let spans = frame_in_spans(&root(), &x);
        assert_eq!(spans[0].status, SpanStatus::Error);
        assert_eq!(
            spans[0].status_description.as_deref(),
            Some("write on Chat::Message is not granted")
        );
    }

    /// `ws.frame.out` is a child of the connection with its times.
    #[test]
    fn wsbridge_8_3_an_outgoing_message_is_a_child_of_its_connection() {
        let connection = root();
        let span = frame_out_span(
            &connection,
            &FrameOut {
                start_ns: 10,
                end_ns: 20,
                topic: "Sensor::Reading".into(),
                bytes: 7,
                text: true,
                failed: None,
            },
        );
        assert_eq!(span.name, "ws.frame.out");
        assert_eq!(span.context.parent_span_id, Some(connection.span_id));
        assert_eq!(span.context.trace_id, connection.trace_id);
        assert_eq!((span.start_unix_ns, span.end_unix_ns), (10, 20));
        assert_eq!(span.status, SpanStatus::Ok);
    }

    /// The root is a server span with its attributes; a failed connection
    /// is an error.
    #[test]
    fn wsbridge_8_3_the_connection_span_is_the_root() {
        let connection = root();
        let attributes = vec![attribute("ws.path", "/topics/chat")];
        let span = connection_span(&connection, 5, 50, &attributes, Some("1002"));
        assert_eq!(span.name, "ws.connection");
        assert!(matches!(span.kind, SpanKind::Server));
        assert_eq!(span.context.parent_span_id, None);
        assert_eq!(span.status, SpanStatus::Error);
        assert_eq!(span.attributes.len(), 1);
        let ok = connection_span(&connection, 5, 50, &attributes, None);
        assert_eq!(ok.status, SpanStatus::Ok);
    }

    /// An end before the start (a clock step) is clamped to the start.
    #[test]
    fn wsbridge_8_3_an_end_before_the_start_is_clamped() {
        let span = connection_span(&root(), 50, 5, &[], None);
        assert_eq!((span.start_unix_ns, span.end_unix_ns), (50, 50));
    }

    /// Without an exporter there is no connection span.
    #[test]
    fn wsbridge_8_3_a_tracer_without_exporter_is_off() {
        let t = Tracer::new(None);
        assert!(!t.enabled());
        assert!(t.connection(1).is_none());
        t.flush();
    }

    /// Span ids differ.
    #[test]
    fn wsbridge_8_3_span_ids_differ() {
        let connection = root();
        let mut ids: Vec<_> = frame_in_spans(&connection, &written())
            .iter()
            .map(|s| s.context.span_id)
            .chain([connection.span_id])
            .collect();
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }
}
