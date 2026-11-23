// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OTLP spans of a call (spec §8.3): a server span per RPC, named
//! `<service>/<method>` with the `rpc.*` attributes of the OpenTelemetry
//! semantic conventions for gRPC, and a child `dds.publish` (the DDS writes
//! of a `Publish` or `PublishOne`) or `dds.subscribe` (the samples of a
//! `Subscribe` stream). A W3C `traceparent` in the call's metadata (W3C
//! Trace Context §3.2) makes the server span a child of the caller's span.
//! The spans go out every 5 s on a thread of their own and once more at
//! the end of the shutdown; a failed export is a `warn` line
//! `otlp_export_failed`.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use zerodds_foundation::observability::Attribute;
use zerodds_foundation::tracing::{Span, SpanContext, SpanId, SpanKind, SpanStatus, TraceId};
use zerodds_monitor::TraceParent;
use zerodds_observability_otlp::{IntAttribute, OtlpExporter};

use super::log::{Level, event};
use crate::status::Status;

/// Spec §8.3: the spans recorded go to the collector this often.
pub(crate) const EXPORT_INTERVAL: Duration = Duration::from_secs(5);

/// While exports keep failing, at most one `otlp_export_failed` line in
/// this time.
pub(crate) const EXPORT_WARN_EVERY: Duration = Duration::from_secs(60);

/// How often the export thread looks at the stop flag.
const STOP_POLL: Duration = Duration::from_millis(50);

/// Spec §8.3, §9.2: how long the end of the shutdown waits for the export
/// thread and the last export together, so that the drain of 30 s and this
/// stay within the 40 s the service managers give the stop.
pub(crate) const EXPORT_STOP_BUDGET: Duration = Duration::from_secs(5);

/// Sends the spans of the calls to the OTLP exporter of
/// `OTEL_EXPORTER_OTLP_ENDPOINT`; without one it records nothing.
#[derive(Clone, Default)]
pub(crate) struct Tracer {
    exporter: Option<Arc<OtlpExporter>>,
    failures: Arc<ExportFailures>,
}

/// The failed exports not yet reported by an `otlp_export_failed` line.
#[derive(Debug, Default)]
pub(crate) struct ExportFailures {
    state: Mutex<FailureState>,
}

#[derive(Debug, Default)]
struct FailureState {
    /// When the last line was written.
    last_line: Option<Instant>,
    /// The failures since then.
    unreported: u64,
}

impl ExportFailures {
    /// Records a failed export at `now`. `Some(n)` when a line is due, with
    /// the `n` failures it reports: the first failure, then at most one
    /// line per [`EXPORT_WARN_EVERY`]; with `always` (the export at the end
    /// of the shutdown) a line in any case.
    pub(crate) fn record(&self, now: Instant, always: bool) -> Option<u64> {
        let mut s = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        s.unreported += 1;
        let due = always
            || s.last_line
                .is_none_or(|t| now.duration_since(t) >= EXPORT_WARN_EVERY);
        if !due {
            return None;
        }
        s.last_line = Some(now);
        Some(std::mem::take(&mut s.unreported))
    }
}

/// The DDS side of a call: the span `dds.publish` or `dds.subscribe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DdsLeg {
    /// `dds.publish` or `dds.subscribe`.
    pub(crate) name: &'static str,
    /// The first DDS write, or when the stream's DataReader opened (Unix
    /// nanoseconds).
    pub(crate) start_ns: u64,
    /// The last DDS write, or the end of the stream.
    pub(crate) end_ns: u64,
    /// The samples written, or sent on the stream.
    pub(crate) samples: u64,
}

/// One call that ended, its times in nanoseconds since the Unix epoch.
#[derive(Debug, Clone)]
pub(crate) struct CallSpan<'a> {
    pub(crate) service: &'a str,
    pub(crate) method: &'a str,
    /// Empty for a call without a DDS topic.
    pub(crate) dds_topic: &'a str,
    /// The client's address, `ip:port`.
    pub(crate) peer: &'a str,
    pub(crate) status: Status,
    pub(crate) message: Option<&'a str>,
    pub(crate) start_ns: u64,
    pub(crate) end_ns: u64,
    /// The caller's span, from the `traceparent` of the call.
    pub(crate) parent: Option<&'a TraceParent>,
    pub(crate) dds: Option<DdsLeg>,
}

impl Tracer {
    /// A tracer sending to `exporter`.
    pub(crate) fn new(exporter: Option<Arc<OtlpExporter>>) -> Self {
        Self {
            exporter,
            failures: Arc::default(),
        }
    }

    /// Whether spans go anywhere.
    pub(crate) fn enabled(&self) -> bool {
        self.exporter.is_some()
    }

    /// Records the spans of `call`, unless its caller did not sample it.
    pub(crate) fn call(&self, call: &CallSpan<'_>) {
        let Some(exporter) = &self.exporter else {
            return;
        };
        if !sampled(call.parent) {
            return;
        }
        for (span, ints) in spans(call) {
            exporter.add_span_with_ints(span, ints);
        }
    }

    /// Spec §8.3: the export every `interval` until `stop` is set, on a
    /// thread of its own; `None` without an exporter. Once `stop` is set
    /// the thread ends without a further export: the last one is
    /// [`Self::finish`], after the thread has been joined, so no export of
    /// the thread is still in flight when the daemon exits.
    ///
    /// # Errors
    /// The thread could not be started.
    pub(crate) fn spawn_export_loop(
        &self,
        stop: Arc<AtomicBool>,
        interval: Duration,
    ) -> std::io::Result<Option<JoinHandle<()>>> {
        if self.exporter.is_none() {
            return Ok(None);
        }
        let tracer = self.clone();
        std::thread::Builder::new()
            .name("zerodds-grpc-otlp".into())
            .spawn(move || {
                'export: loop {
                    let due = Instant::now() + interval;
                    while Instant::now() < due {
                        if stop.load(Ordering::SeqCst) {
                            break 'export;
                        }
                        std::thread::sleep(
                            STOP_POLL.min(due.saturating_duration_since(Instant::now())),
                        );
                    }
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    tracer.export(false);
                }
            })
            .map(Some)
    }

    /// Spec §8.3, §9.2: the last export, at the end of the shutdown; a
    /// failure is a line in any case.
    pub(crate) fn finish(&self) {
        self.export(true);
    }

    /// Spec §8.3, §9.2: the end of the shutdown — waits for the export
    /// thread to end (it has no export in flight then), then makes the last
    /// export ([`Self::finish`]), both until `deadline`. When the deadline
    /// passes first, the export still in flight is left to the exit of the
    /// process, its spans are lost, and an `otlp_export_failed` line says
    /// so; `false` then.
    pub(crate) fn finish_by(&self, thread: Option<JoinHandle<()>>, deadline: Instant) -> bool {
        if self.exporter.is_none() {
            return true;
        }
        let done = self.join_by(thread, deadline) && self.last_export_by(deadline);
        if !done && let Some(failures) = self.failures.record(Instant::now(), true) {
            event(Level::Warn, "otlp_export_failed")
                .text(
                    "err",
                    "the export did not end within the stop budget; its spans are lost",
                )
                .num("failures", failures)
                .emit();
        }
        done
    }

    /// Joins `thread` if it ends before `deadline`.
    fn join_by(&self, thread: Option<JoinHandle<()>>, deadline: Instant) -> bool {
        let Some(handle) = thread else {
            return true;
        };
        while !handle.is_finished() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            std::thread::sleep(STOP_POLL.min(left));
        }
        handle.join().is_ok()
    }

    /// The last export on a thread of its own, waited for until `deadline`.
    fn last_export_by(&self, deadline: Instant) -> bool {
        let tracer = self.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("zerodds-grpc-otlp-last".into())
            .spawn(move || {
                tracer.finish();
                // The receiver is gone when the deadline passed.
                let _ = tx.send(());
            });
        if spawned.is_err() {
            // Without a thread the export runs here, bounded by the
            // exporter's own timeouts.
            self.finish();
            return true;
        }
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_ok()
    }

    /// Sends what is recorded. The spans of a failed export are lost (the
    /// exporter clears its buffer); the failure is an `otlp_export_failed`
    /// line at `warn` with `err` and `failures`, at most one per
    /// [`EXPORT_WARN_EVERY`] unless `always`.
    fn export(&self, always: bool) {
        let Some(exporter) = &self.exporter else {
            return;
        };
        if let Err(e) = exporter.flush()
            && let Some(failures) = self.failures.record(Instant::now(), always)
        {
            event(Level::Warn, "otlp_export_failed")
                .text("err", &e.to_string())
                .num("failures", failures)
                .emit();
        }
    }
}

/// W3C Trace Context §3.2: the `traceparent` of a call's metadata. `None`
/// without one, with more than one (their RFC 9110 §5.3 combination is no
/// valid value) and for one that does not parse: the caller's trace is
/// ignored and the call starts a trace of its own (§3.2.2.3, §3.2.2.4,
/// §3.2.4). The header name is matched in any case (§3.2.1).
pub(crate) fn traceparent_of(metadata: &[(String, String)]) -> Option<TraceParent> {
    let mut values = metadata
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("traceparent"))
        .map(|(_, value)| value);
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    value.trim_matches([' ', '\t']).parse().ok()
}

/// W3C Trace Context §3.2.2.5.1: a call whose caller did not set the
/// sampled flag is not recorded (the parent-based decision of
/// OpenTelemetry); a call without a caller's trace is.
pub(crate) fn sampled(parent: Option<&TraceParent>) -> bool {
    parent.is_none_or(TraceParent::is_sampled)
}

/// The span status of a gRPC server span (OpenTelemetry semantic
/// conventions for gRPC): `Error` for UNKNOWN, DEADLINE_EXCEEDED,
/// UNIMPLEMENTED, INTERNAL, UNAVAILABLE and DATA_LOSS, unset for the other
/// codes.
pub(crate) fn span_status(status: Status) -> SpanStatus {
    match status {
        Status::Unknown
        | Status::DeadlineExceeded
        | Status::Unimplemented
        | Status::Internal
        | Status::Unavailable
        | Status::DataLoss => SpanStatus::Error,
        _ => SpanStatus::Unset,
    }
}

/// The spans of a call, each with its attributes of an integer value: the
/// DDS child, when the call has one, then the server span.
pub(crate) fn spans(call: &CallSpan<'_>) -> Vec<(Span, Vec<IntAttribute>)> {
    let root = match call.parent {
        Some(p) => SpanContext::child_of(
            &SpanContext::new_root(p.trace_id, p.parent_id),
            new_span_id(),
        ),
        None => SpanContext::new_root(new_trace_id(), new_span_id()),
    };
    let status = span_status(call.status);
    let description = (status == SpanStatus::Error)
        .then(|| call.message.map(str::to_string))
        .flatten();
    // The OpenTelemetry semantic conventions define `rpc.grpc.status_code`
    // and `network.peer.port` as `int`, the others as `string`.
    let mut attributes = vec![
        attr("rpc.system", "grpc"),
        attr("rpc.service", call.service),
        attr("rpc.method", call.method),
    ];
    let mut ints = vec![int("rpc.grpc.status_code", i64::from(call.status.code()))];
    if !call.dds_topic.is_empty() {
        attributes.push(attr("dds.topic", call.dds_topic));
    }
    if let Ok(peer) = call.peer.parse::<SocketAddr>() {
        attributes.push(attr("network.peer.address", &peer.ip().to_string()));
        ints.push(int("network.peer.port", i64::from(peer.port())));
    }
    let mut out = Vec::with_capacity(2);
    if let Some(leg) = call.dds {
        let leg_attributes = if call.dds_topic.is_empty() {
            Vec::new()
        } else {
            vec![attr("dds.topic", call.dds_topic)]
        };
        out.push((
            Span {
                context: SpanContext::child_of(&root, new_span_id()),
                name: leg.name.to_string(),
                kind: if leg.name == "dds.publish" {
                    SpanKind::Client
                } else {
                    SpanKind::Internal
                },
                start_unix_ns: leg.start_ns,
                end_unix_ns: leg.end_ns,
                status,
                status_description: None,
                attributes: leg_attributes,
            },
            vec![int(
                "dds.samples",
                i64::try_from(leg.samples).unwrap_or(i64::MAX),
            )],
        ));
    }
    out.push((
        Span {
            context: root,
            name: format!("{}/{}", call.service, call.method),
            kind: SpanKind::Server,
            start_unix_ns: call.start_ns,
            end_unix_ns: call.end_ns,
            status,
            status_description: description,
            attributes,
        },
        ints,
    ));
    out
}

fn attr(key: &'static str, value: &str) -> Attribute {
    Attribute {
        key,
        value: value.to_string(),
    }
}

fn int(key: &'static str, value: i64) -> IntAttribute {
    IntAttribute { key, value }
}

/// Nanoseconds since the Unix epoch.
pub(crate) fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// The fallback of the identifiers when the system has no random source.
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn fallback_id() -> u64 {
    let n = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    (n ^ unix_ns().rotate_left(17) ^ u64::from(std::process::id()).rotate_left(40)).max(1)
}

/// A random trace id (W3C Trace Context §8.2), never all zeros.
fn new_trace_id() -> TraceId {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() || bytes.iter().all(|b| *b == 0) {
        bytes[..8].copy_from_slice(&fallback_id().to_be_bytes());
        bytes[8..].copy_from_slice(&fallback_id().to_be_bytes());
    }
    TraceId(bytes)
}

/// A random span id, never all zeros.
fn new_span_id() -> SpanId {
    let mut bytes = [0u8; 8];
    if getrandom::getrandom(&mut bytes).is_err() || bytes.iter().all(|b| *b == 0) {
        bytes = fallback_id().to_be_bytes();
    }
    SpanId(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    fn publish() -> CallSpan<'static> {
        CallSpan {
            service: "zerodds.chat.v1.ChatMessageStream",
            method: "Publish",
            dds_topic: "Chat::Message",
            peer: "127.0.0.1:40000",
            status: Status::Ok,
            message: None,
            start_ns: 100,
            end_ns: 900,
            parent: None,
            dds: Some(DdsLeg {
                name: "dds.publish",
                start_ns: 200,
                end_ns: 700,
                samples: 3,
            }),
        }
    }

    /// The string attribute `key` of a span.
    fn value<'a>(span: &'a (Span, Vec<IntAttribute>), key: &str) -> Option<&'a str> {
        span.0
            .attributes
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.as_str())
    }

    /// The integer attribute `key` of a span.
    fn int_value(span: &(Span, Vec<IntAttribute>), key: &str) -> Option<i64> {
        span.1.iter().find(|a| a.key == key).map(|a| a.value)
    }

    /// Spec §8.3: a `Publish` is a server span `<service>/<method>` with a
    /// child `dds.publish` over its DDS writes, in one trace.
    #[test]
    fn grpcbridge_8_3_a_publish_is_a_server_span_with_a_dds_publish_child() {
        let spans = spans(&publish());
        assert_eq!(spans.len(), 2);
        let (child, root) = (&spans[0].0, &spans[1].0);
        assert_eq!(root.name, "zerodds.chat.v1.ChatMessageStream/Publish");
        assert!(matches!(root.kind, SpanKind::Server));
        assert_eq!(root.context.parent_span_id, None);
        assert_eq!((root.start_unix_ns, root.end_unix_ns), (100, 900));
        assert_eq!(child.name, "dds.publish");
        assert!(matches!(child.kind, SpanKind::Client));
        assert_eq!(child.context.trace_id, root.context.trace_id);
        assert_eq!(child.context.parent_span_id, Some(root.context.span_id));
        assert_ne!(child.context.span_id, root.context.span_id);
        assert_eq!((child.start_unix_ns, child.end_unix_ns), (200, 700));
        assert_eq!(int_value(&spans[0], "dds.samples"), Some(3));
        assert_eq!(value(&spans[0], "dds.samples"), None);
        assert_eq!(value(&spans[0], "dds.topic"), Some("Chat::Message"));
    }

    /// Spec §8.3: the attributes of the OpenTelemetry semantic conventions
    /// for gRPC, the DDS topic and the client's address;
    /// `rpc.grpc.status_code` and `network.peer.port` of type `int`, as the
    /// conventions define them.
    #[test]
    fn grpcbridge_8_3_attributes_name_service_method_and_topic() {
        let spans = spans(&publish());
        let root = &spans[1];
        assert_eq!(value(root, "rpc.system"), Some("grpc"));
        assert_eq!(
            value(root, "rpc.service"),
            Some("zerodds.chat.v1.ChatMessageStream")
        );
        assert_eq!(value(root, "rpc.method"), Some("Publish"));
        assert_eq!(int_value(root, "rpc.grpc.status_code"), Some(0));
        assert_eq!(value(root, "rpc.grpc.status_code"), None);
        assert_eq!(value(root, "dds.topic"), Some("Chat::Message"));
        assert_eq!(value(root, "network.peer.address"), Some("127.0.0.1"));
        assert_eq!(int_value(root, "network.peer.port"), Some(40_000));
        assert_eq!(value(root, "network.peer.port"), None);
        let reflection = CallSpan {
            dds_topic: "",
            dds: None,
            ..publish()
        };
        let spans = super::spans(&reflection);
        assert_eq!(spans.len(), 1);
        assert_eq!(value(&spans[0], "dds.topic"), None);
    }

    /// Spec §8.3: the status of a server span as the semantic conventions
    /// set it: UNAVAILABLE is an error with the `grpc-message`,
    /// INVALID_ARGUMENT (the client's fault) and OK stay unset.
    #[test]
    fn grpcbridge_8_3_the_status_of_a_failed_call_is_error() {
        let failed = CallSpan {
            status: Status::Unavailable,
            message: Some("DDS write: no route"),
            ..publish()
        };
        let spans = spans(&failed);
        assert_eq!(spans[1].0.status, SpanStatus::Error);
        assert_eq!(
            spans[1].0.status_description.as_deref(),
            Some("DDS write: no route")
        );
        assert_eq!(int_value(&spans[1], "rpc.grpc.status_code"), Some(14));
        let refused = CallSpan {
            status: Status::InvalidArgument,
            message: Some("Sample: truncated"),
            ..publish()
        };
        assert_eq!(super::spans(&refused)[1].0.status, SpanStatus::Unset);
        assert_eq!(super::spans(&publish())[1].0.status, SpanStatus::Unset);
        for code in [2u8, 4, 12, 13, 14, 15] {
            assert_eq!(
                span_status(Status::from_code(code).unwrap()),
                SpanStatus::Error,
                "{code}"
            );
        }
        for code in [0u8, 1, 3, 5, 6, 7, 8, 9, 10, 11, 16] {
            assert_eq!(
                span_status(Status::from_code(code).unwrap()),
                SpanStatus::Unset,
                "{code}"
            );
        }
    }

    /// Spec §8.3: a `Subscribe` stream is one span for its life, with the
    /// `dds.subscribe` child over the samples it sent.
    #[test]
    fn grpcbridge_8_3_a_subscribe_stream_is_one_span_for_its_life() {
        let stream = CallSpan {
            method: "Subscribe",
            start_ns: 1_000,
            end_ns: 9_000_000,
            dds: Some(DdsLeg {
                name: "dds.subscribe",
                start_ns: 2_000,
                end_ns: 9_000_000,
                samples: 250,
            }),
            ..publish()
        };
        let spans = spans(&stream);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0.name, "dds.subscribe");
        assert!(matches!(spans[0].0.kind, SpanKind::Internal));
        assert_eq!(int_value(&spans[0], "dds.samples"), Some(250));
        assert_eq!(
            spans[1].0.name,
            "zerodds.chat.v1.ChatMessageStream/Subscribe"
        );
        assert_eq!(spans[1].0.duration_ns(), 8_999_000);
    }

    /// Spec §8.3: a failed export is an `otlp_export_failed` line, the
    /// first at once, then at most one per minute with the failures since
    /// the line before; the export at the end of the shutdown writes its
    /// line in any case.
    #[test]
    fn grpcbridge_8_3_failed_exports_are_reported_at_most_once_a_minute() {
        let f = ExportFailures::default();
        let t0 = Instant::now();
        assert_eq!(f.record(t0, false), Some(1));
        assert_eq!(f.record(t0 + Duration::from_secs(5), false), None);
        assert_eq!(f.record(t0 + Duration::from_secs(10), false), None);
        assert_eq!(f.record(t0 + Duration::from_secs(59), false), None);
        assert_eq!(f.record(t0 + EXPORT_WARN_EVERY, false), Some(4));
        assert_eq!(f.record(t0 + Duration::from_secs(65), false), None);
        assert_eq!(f.record(t0 + Duration::from_secs(66), true), Some(2));
        assert_eq!(f.record(t0 + Duration::from_secs(67), false), None);
    }

    /// Spec §8.3, §9.2: the export thread ends once the stop flag is set,
    /// without an export of its own; without an exporter there is no
    /// thread.
    #[test]
    fn grpcbridge_8_3_the_export_thread_ends_at_the_stop_without_exporting() {
        let none = Tracer::new(None);
        assert!(!none.enabled());
        let stop = Arc::new(AtomicBool::new(false));
        assert!(
            none.spawn_export_loop(Arc::clone(&stop), EXPORT_INTERVAL)
                .unwrap()
                .is_none()
        );
        // A collector of the test that counts its connections.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let tracer = Tracer::new(Some(Arc::new(OtlpExporter::new(
            zerodds_observability_otlp::OtlpConfig {
                host: "127.0.0.1".into(),
                port: listener.local_addr().unwrap().port(),
                timeout: Duration::from_millis(200),
                ..Default::default()
            },
        ))));
        tracer.call(&publish());
        let handle = tracer
            .spawn_export_loop(Arc::clone(&stop), Duration::from_secs(3600))
            .unwrap()
            .expect("a thread with an exporter");
        let started = Instant::now();
        stop.store(true, Ordering::SeqCst);
        handle.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(
            listener.accept().is_err(),
            "the thread exported after the stop"
        );
    }

    /// A tracer that exports to `port` on 127.0.0.1 with the exporter
    /// timeout `timeout`, with one span recorded.
    fn tracer_to(port: u16, timeout: Duration) -> Tracer {
        let tracer = Tracer::new(Some(Arc::new(OtlpExporter::new(
            zerodds_observability_otlp::OtlpConfig {
                host: "127.0.0.1".into(),
                port,
                timeout,
                ..Default::default()
            },
        ))));
        tracer.call(&publish());
        tracer
    }

    /// Spec §8.3, §9.2: the end of the shutdown waits for the export thread
    /// and the last export only until its deadline. Against a collector that
    /// takes the connection and never answers (the exporter's own timeout is
    /// 10 s) it gives up at the deadline, for an export of the thread in
    /// flight and for the last export; against a collector that answers it
    /// returns after the last export.
    #[test]
    fn grpcbridge_8_3_the_last_export_ends_within_the_stop_budget() {
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = silent.local_addr().unwrap().port();
        // An export of the thread in flight.
        let tracer = tracer_to(port, Duration::from_secs(10));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = tracer
            .spawn_export_loop(Arc::clone(&stop), Duration::from_millis(10))
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        stop.store(true, Ordering::SeqCst);
        let started = Instant::now();
        assert!(!tracer.finish_by(handle, started + Duration::from_millis(300)));
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(280) && took < Duration::from_secs(1),
            "{took:?}"
        );
        // The last export itself.
        let tracer = tracer_to(port, Duration::from_secs(10));
        let started = Instant::now();
        assert!(!tracer.finish_by(None, started + Duration::from_millis(300)));
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(280) && took < Duration::from_secs(1),
            "{took:?}"
        );
        // A collector that answers.
        let answering = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = answering.local_addr().unwrap().port();
        let collector = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut s, _) = answering.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            // The request: its head, then Content-Length octets of body.
            loop {
                let n = s.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req);
                if let Some(end) = text.find("\r\n\r\n") {
                    let len: usize = text
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap();
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
                assert!(n > 0, "the request ended early");
            }
            s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
            String::from_utf8_lossy(&req).into_owned()
        });
        let tracer = tracer_to(port, Duration::from_secs(10));
        assert!(tracer.finish_by(None, Instant::now() + EXPORT_STOP_BUDGET));
        let request = collector.join().unwrap();
        assert!(request.starts_with("POST /v1/traces "), "{request}");
        drop(silent);
    }

    /// Spec §8.3, W3C Trace Context §3.2: the server span of a call with a
    /// `traceparent` is in the caller's trace, a child of the caller's
    /// span; the DDS child hangs below it.
    #[test]
    fn grpcbridge_8_3_the_span_of_a_call_is_a_child_of_its_traceparent() {
        let metadata = vec![
            ("authorization".to_string(), "Bearer x".to_string()),
            ("TraceParent".to_string(), PARENT.to_string()),
        ];
        let parent = traceparent_of(&metadata).expect("a traceparent");
        assert!(sampled(Some(&parent)));
        let call = CallSpan {
            parent: Some(&parent),
            ..publish()
        };
        let spans = spans(&call);
        let (child, root) = (&spans[0].0, &spans[1].0);
        assert_eq!(
            root.context.trace_id.to_hex(),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert_eq!(
            root.context.parent_span_id.map(|s| s.to_hex()).as_deref(),
            Some("00f067aa0ba902b7")
        );
        assert_ne!(root.context.span_id.to_hex(), "00f067aa0ba902b7");
        assert_eq!(child.context.trace_id, root.context.trace_id);
        assert_eq!(child.context.parent_span_id, Some(root.context.span_id));
    }

    /// W3C Trace Context §3.2.2.3, §3.2.2.4, §3.2.4: a malformed
    /// `traceparent`, or two of them, is ignored and the call starts a
    /// trace of its own.
    #[test]
    fn grpcbridge_8_3_a_malformed_traceparent_starts_a_new_trace() {
        let one = |v: &str| vec![("traceparent".to_string(), v.to_string())];
        for bad in [
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-00",
            "garbage",
        ] {
            assert_eq!(traceparent_of(&one(bad)), None, "{bad}");
        }
        let mut two = one(PARENT);
        two.extend(one(PARENT));
        assert_eq!(traceparent_of(&two), None);
        assert_eq!(traceparent_of(&[]), None);
        let fresh = spans(&publish());
        assert_eq!(fresh[1].0.context.parent_span_id, None);
        assert_ne!(
            fresh[1].0.context.trace_id.to_hex(),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
    }

    /// W3C Trace Context §3.2.2.5.1: a caller that did not sample its
    /// request gets no span of the daemon.
    #[test]
    fn grpcbridge_8_3_an_unsampled_traceparent_records_no_span() {
        let parent = traceparent_of(&[(
            "traceparent".to_string(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00".to_string(),
        )])
        .expect("a valid traceparent");
        assert!(!sampled(Some(&parent)));
        assert!(sampled(None));
    }
}
