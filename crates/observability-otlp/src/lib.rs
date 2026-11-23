// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OTLP/HTTP/JSON exporter for ZeroDDS telemetry.
//!
//! Crate `zerodds-observability-otlp`. Safety classification: **STANDARD**.
//!
//! Spec: `docs/specs/zerodds-observability-otlp-1.0.md`.
//! Layer position: Layer 4 — core services (consumer path for
//! `foundation::tracing::{Span, Histogram}` + `foundation::observability::Event`).
//!
//! # What is exported?
//!
//! Three OTLP endpoints are served:
//!
//! * **`/v1/traces`** — spans from `foundation::tracing::Span`.
//! * **`/v1/metrics`** — histograms from `foundation::tracing::Histogram`.
//! * **`/v1/logs`** — events from `foundation::observability::Event`.
//!
//! We use the **OTLP/HTTP/JSON** format per the OpenTelemetry spec
//! v1.4 (<https://github.com/open-telemetry/opentelemetry-proto/blob/v1.4.0/docs/specification.md#otlphttp>).
//! JSON is officially supported
//! and needs no Protobuf codegen pipeline — perfect for
//! pure Rust without `prost`/`tonic`.
//!
//! Per tick all buffered spans/histograms/events are POSTed in
//! one batch request.
//!
//! # Default endpoints
//!
//! By default a local OTel collector runs on
//! `http://127.0.0.1:4318/v1/traces` etc. — see
//! `examples/otel/jaeger-compose.yml` for a local stack.

#![warn(missing_docs)]

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::Duration;

use zerodds_foundation::observability::{Component, Event, Level};
use zerodds_foundation::tracing::{Histogram, Span, SpanKind, SpanStatus};

/// Defaults — OTel collector on localhost (Jaeger compose).
pub const DEFAULT_OTLP_HOST: &str = "127.0.0.1";
/// Default OTLP/HTTP port (OTel spec).
pub const DEFAULT_OTLP_PORT: u16 = 4318;

/// Configuration for the exporter.
#[derive(Clone, Debug)]
pub struct OtlpConfig {
    /// OTel collector host: a name, an IPv4 address or an IPv6 address,
    /// with or without the brackets of a URL (`[::1]`, `::1`).
    pub host: String,
    /// OTel collector port (HTTP).
    pub port: u16,
    /// Service name for all resource attributes.
    pub service_name: String,
    /// Service version (e.g. the Cargo version).
    pub service_version: String,
    /// The timeout of the connect (per address of `host`), of each write
    /// and of each read.
    pub timeout: Duration,
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_OTLP_HOST.into(),
            port: DEFAULT_OTLP_PORT,
            service_name: "zerodds".into(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            timeout: Duration::from_secs(5),
        }
    }
}

/// The environment variable that names the collector (spec §3).
pub const OTEL_EXPORTER_OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

impl OtlpConfig {
    /// The configuration of a daemon's exporter from an endpoint written
    /// `http://host:port`, `https://host:port`, `host:port` or `host`, a
    /// trailing `/` allowed (spec §3). The text after the last `:` is the
    /// port; without one, or when it is not a number, the port is 4318.
    /// The timeout is 5 s.
    #[must_use]
    pub fn from_endpoint(service_name: &str, service_version: &str, raw: &str) -> Self {
        let trimmed = raw
            .strip_prefix("http://")
            .or_else(|| raw.strip_prefix("https://"))
            .unwrap_or(raw)
            .trim_end_matches('/');
        let (host, port) = match trimmed.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().unwrap_or(DEFAULT_OTLP_PORT)),
            None => (trimmed.to_string(), DEFAULT_OTLP_PORT),
        };
        Self {
            host,
            port,
            service_name: service_name.into(),
            service_version: service_version.into(),
            timeout: Duration::from_secs(5),
        }
    }

    /// [`OtlpConfig::from_endpoint`] of the value of
    /// [`OTEL_EXPORTER_OTLP_ENDPOINT`]; `None` while it is not set.
    #[must_use]
    pub fn from_env(service_name: &str, service_version: &str) -> Option<Self> {
        let raw = std::env::var(OTEL_EXPORTER_OTLP_ENDPOINT).ok()?;
        Some(Self::from_endpoint(service_name, service_version, &raw))
    }
}

/// Starts the thread `thread_name` that flushes `exporter` every
/// `interval` and once more when `stop` is set (spec §4). The thread looks
/// at `stop` every `stop_poll` (at most every `interval`) and ends after
/// that last flush. Export errors are dropped: a collector that is away
/// does not stop the daemon.
///
/// # Errors
/// The start of the thread.
pub fn spawn_flush_loop(
    exporter: std::sync::Arc<OtlpExporter>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    interval: Duration,
    stop_poll: Duration,
    thread_name: &str,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    use std::sync::atomic::Ordering;
    std::thread::Builder::new()
        .name(thread_name.into())
        .spawn(move || {
            let step = interval.min(stop_poll);
            let mut next = std::time::Instant::now() + interval;
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(step);
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if std::time::Instant::now() < next {
                    continue;
                }
                next = std::time::Instant::now() + interval;
                let _ = exporter.flush();
            }
            let _ = exporter.flush();
        })
}

/// A span attribute with an integer value: an OTLP `AnyValue` with
/// `intValue`, for an attribute the OpenTelemetry semantic conventions
/// define as `int` (such as `rpc.grpc.status_code` or `network.peer.port`).
/// The attributes of [`Span::attributes`] are `stringValue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntAttribute {
    /// The attribute key.
    pub key: &'static str,
    /// The value, written as `{"intValue":"<decimal>"}` (the proto3 JSON
    /// mapping of an int64).
    pub value: i64,
}

/// Exporter handle. Buffers spans/histograms/events and flushes
/// them via POST to the OTel collector.
pub struct OtlpExporter {
    cfg: OtlpConfig,
    buf: Mutex<ExporterBuffers>,
}

#[derive(Default)]
struct ExporterBuffers {
    /// Each span with its integer attributes.
    spans: Vec<(Span, Vec<IntAttribute>)>,
    histograms: Vec<Histogram>,
    events: Vec<Event>,
}

/// Error during export.
#[derive(Debug)]
pub enum ExportError {
    /// Connect/Write/Read failed.
    Io(std::io::Error),
    /// HTTP-Status != 2xx.
    HttpStatus {
        /// Status-Code.
        code: u16,
        /// Body-Snippet (max 256 byte).
        body_snippet: String,
    },
    /// Mutex poisoned.
    Poisoned,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::HttpStatus { code, body_snippet } => {
                write!(f, "http {code}: {body_snippet}")
            }
            Self::Poisoned => write!(f, "exporter mutex poisoned"),
        }
    }
}

impl std::error::Error for ExportError {}

impl OtlpExporter {
    /// New exporter with the given config.
    #[must_use]
    pub fn new(cfg: OtlpConfig) -> Self {
        Self {
            cfg,
            buf: Mutex::new(ExporterBuffers::default()),
        }
    }

    /// Adds a span to the pending queue.
    pub fn add_span(&self, span: Span) {
        self.add_span_with_ints(span, Vec::new());
    }

    /// Adds a span with attributes of an integer value beside its string
    /// attributes (`intValue` after the `stringValue` attributes).
    pub fn add_span_with_ints(&self, span: Span, ints: Vec<IntAttribute>) {
        if let Ok(mut b) = self.buf.lock() {
            b.spans.push((span, ints));
        }
    }

    /// Adds a histogram (snapshot).
    pub fn add_histogram(&self, h: Histogram) {
        if let Ok(mut b) = self.buf.lock() {
            b.histograms.push(h);
        }
    }

    /// Adds an event.
    pub fn add_event(&self, e: Event) {
        if let Ok(mut b) = self.buf.lock() {
            b.events.push(e);
        }
    }

    /// Flush + POST. Blocks until the collector has responded or
    /// the timeout is reached. Clears the buffer even on error.
    ///
    /// # Errors
    /// IO/HTTP error from the POST request.
    pub fn flush(&self) -> Result<(), ExportError> {
        let (spans, histograms, events) = {
            let mut b = self.buf.lock().map_err(|_| ExportError::Poisoned)?;
            (
                std::mem::take(&mut b.spans),
                std::mem::take(&mut b.histograms),
                std::mem::take(&mut b.events),
            )
        };
        if !spans.is_empty() {
            let body = build_traces_json(&self.cfg, &spans);
            self.post("/v1/traces", &body)?;
        }
        if !histograms.is_empty() {
            let body = build_metrics_json(&self.cfg, &histograms);
            self.post("/v1/metrics", &body)?;
        }
        if !events.is_empty() {
            let body = build_logs_json(&self.cfg, &events);
            self.post("/v1/logs", &body)?;
        }
        Ok(())
    }

    /// The connection to the collector: each address of `host:port` in
    /// turn, each connect within `timeout` (a collector that drops the SYN
    /// does not hold the export for the kernel's retries). An IPv6 address
    /// in brackets, as a URL writes it (RFC 3986 §3.2.2), is resolved
    /// without them.
    fn connect(&self) -> Result<TcpStream, ExportError> {
        let addrs = (self.bare_host(), self.cfg.port)
            .to_socket_addrs()
            .map_err(ExportError::Io)?;
        let mut last = std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            format!("{}:{} names no address", self.cfg.host, self.cfg.port),
        );
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, self.cfg.timeout) {
                Ok(stream) => return Ok(stream),
                Err(e) => last = e,
            }
        }
        Err(ExportError::Io(last))
    }

    /// `host` without the brackets of an IPv6 address.
    fn bare_host(&self) -> &str {
        self.cfg
            .host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(&self.cfg.host)
    }

    /// Direct HTTP POST to `path`. JSON content-type; the `Host` field
    /// writes an IPv6 address in brackets (RFC 9110 §7.2, RFC 3986 §3.2.2).
    fn post(&self, path: &str, body: &str) -> Result<(), ExportError> {
        let mut stream = self.connect()?;
        stream
            .set_write_timeout(Some(self.cfg.timeout))
            .map_err(ExportError::Io)?;
        stream
            .set_read_timeout(Some(self.cfg.timeout))
            .map_err(ExportError::Io)?;
        let host = self.bare_host();
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        let req = format!(
            "POST {} HTTP/1.1\r\n\
             Host: {}\r\n\
             User-Agent: zerodds-otlp/0.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            path,
            host,
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).map_err(ExportError::Io)?;
        let mut resp = Vec::new();
        let _ = stream.read_to_end(&mut resp);
        let resp_str = String::from_utf8_lossy(&resp);
        // Parse status-line.
        let (code, body_start) = parse_http_status(&resp_str);
        if !(200..300).contains(&code) {
            let snippet: String = resp_str[body_start.min(resp_str.len())..]
                .chars()
                .take(256)
                .collect();
            return Err(ExportError::HttpStatus {
                code,
                body_snippet: snippet,
            });
        }
        Ok(())
    }
}

fn parse_http_status(resp: &str) -> (u16, usize) {
    if let Some(line_end) = resp.find('\n') {
        let first_line = &resp[..line_end];
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            let code: u16 = parts[1].parse().unwrap_or(0);
            let body_start = resp.find("\r\n\r\n").map(|i| i + 4).unwrap_or(resp.len());
            return (code, body_start);
        }
    }
    (0, resp.len())
}

// ============================================================================
// JSON builder — minimal serializer per the OTLP/HTTP/JSON spec.
// ============================================================================

fn build_traces_json(cfg: &OtlpConfig, spans: &[(Span, Vec<IntAttribute>)]) -> String {
    // Top-level: { resourceSpans: [{ resource, scopeSpans: [{ scope, spans }] }] }
    let mut out = String::with_capacity(512 + spans.len() * 256);
    out.push_str(r#"{"resourceSpans":[{"resource":"#);
    push_resource(&mut out, cfg);
    out.push_str(r#","scopeSpans":[{"scope":{"name":"zerodds","version":""#);
    push_str_escaped(&mut out, &cfg.service_version);
    out.push_str(r#""},"spans":["#);
    for (i, (s, ints)) in spans.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_span(&mut out, s, ints);
    }
    out.push_str("]}]}]}");
    out
}

fn build_metrics_json(cfg: &OtlpConfig, hists: &[Histogram]) -> String {
    let mut out = String::with_capacity(512 + hists.len() * 256);
    out.push_str(r#"{"resourceMetrics":[{"resource":"#);
    push_resource(&mut out, cfg);
    out.push_str(r#","scopeMetrics":[{"scope":{"name":"zerodds","version":""#);
    push_str_escaped(&mut out, &cfg.service_version);
    out.push_str(r#""},"metrics":["#);
    for (i, h) in hists.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_histogram(&mut out, h);
    }
    out.push_str("]}]}]}");
    out
}

fn build_logs_json(cfg: &OtlpConfig, events: &[Event]) -> String {
    let mut out = String::with_capacity(512 + events.len() * 200);
    out.push_str(r#"{"resourceLogs":[{"resource":"#);
    push_resource(&mut out, cfg);
    out.push_str(r#","scopeLogs":[{"scope":{"name":"zerodds","version":""#);
    push_str_escaped(&mut out, &cfg.service_version);
    out.push_str(r#""},"logRecords":["#);
    for (i, e) in events.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_event(&mut out, e);
    }
    out.push_str("]}]}]}");
    out
}

fn push_resource(out: &mut String, cfg: &OtlpConfig) {
    out.push_str(r#"{"attributes":[{"key":"service.name","value":{"stringValue":""#);
    push_str_escaped(out, &cfg.service_name);
    out.push_str(r#""}},{"key":"service.version","value":{"stringValue":""#);
    push_str_escaped(out, &cfg.service_version);
    out.push_str(r#""}}]}"#);
}

fn push_span(out: &mut String, s: &Span, ints: &[IntAttribute]) {
    out.push_str(r#"{"traceId":""#);
    out.push_str(&s.context.trace_id.to_hex());
    out.push_str(r#"","spanId":""#);
    out.push_str(&s.context.span_id.to_hex());
    out.push_str(r#"","name":""#);
    push_str_escaped(out, &s.name);
    out.push_str(r#"","kind":"#);
    out.push_str(match s.kind {
        SpanKind::Internal => "1",
        SpanKind::Server => "2",
        SpanKind::Client => "3",
    });
    out.push_str(r#","startTimeUnixNano":""#);
    push_u64(out, s.start_unix_ns);
    out.push_str(r#"","endTimeUnixNano":""#);
    push_u64(out, s.end_unix_ns);
    out.push('"');
    if let Some(p) = s.context.parent_span_id {
        out.push_str(r#","parentSpanId":""#);
        out.push_str(&p.to_hex());
        out.push('"');
    }
    if let Some(d) = &s.status_description {
        out.push_str(r#","status":{"code":"#);
        out.push_str(match s.status {
            SpanStatus::Unset => "0",
            SpanStatus::Ok => "1",
            SpanStatus::Error => "2",
        });
        out.push_str(r#","message":""#);
        push_str_escaped(out, d);
        out.push_str(r#""}"#);
    } else {
        out.push_str(r#","status":{"code":"#);
        out.push_str(match s.status {
            SpanStatus::Unset => "0",
            SpanStatus::Ok => "1",
            SpanStatus::Error => "2",
        });
        out.push('}');
    }
    if !s.attributes.is_empty() || !ints.is_empty() {
        out.push_str(r#","attributes":["#);
        for (i, a) in s.attributes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(r#"{"key":""#);
            push_str_escaped(out, a.key);
            out.push_str(r#"","value":{"stringValue":""#);
            push_str_escaped(out, &a.value);
            out.push_str(r#""}}"#);
        }
        for (i, a) in ints.iter().enumerate() {
            if i > 0 || !s.attributes.is_empty() {
                out.push(',');
            }
            out.push_str(r#"{"key":""#);
            push_str_escaped(out, a.key);
            out.push_str(r#"","value":{"intValue":""#);
            {
                use std::fmt::Write as _;
                let _ = write!(out, "{}", a.value);
            }
            out.push_str(r#""}}"#);
        }
        out.push(']');
    }
    out.push('}');
}

fn push_histogram(out: &mut String, h: &Histogram) {
    out.push_str(r#"{"name":""#);
    push_str_escaped(out, &h.name);
    out.push_str(r#"","unit":"ns","histogram":{"aggregationTemporality":2,"dataPoints":[{"#);
    out.push_str(r#""startTimeUnixNano":"0","timeUnixNano":"0","count":""#);
    push_u64(out, h.count);
    out.push_str(r#"","sum":"#);
    push_u64(out, h.sum_ns);
    out.push_str(r#","min":"#);
    push_u64(out, if h.count == 0 { 0 } else { h.min_ns });
    out.push_str(r#","max":"#);
    push_u64(out, h.max_ns);
    out.push_str(r#","explicitBounds":["#);
    let bounds = Histogram::bucket_bounds();
    // OTLP wants strictly-increasing bucket boundaries; use ALL but
    // last (overflow bucket has no upper bound).
    for (i, b) in bounds.iter().take(bounds.len() - 1).enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_u64(out, *b);
    }
    out.push_str(r#"],"bucketCounts":["#);
    for (i, c) in h.buckets.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        push_u64(out, *c);
        out.push('"');
    }
    out.push_str("]}]}}");
}

fn push_event(out: &mut String, e: &Event) {
    out.push_str(r#"{"timeUnixNano":"0","severityNumber":"#);
    out.push_str(match e.level {
        Level::Info => "9",
        Level::Warn => "13",
        Level::Error => "17",
    });
    out.push_str(r#","severityText":""#);
    out.push_str(match e.level {
        Level::Info => "INFO",
        Level::Warn => "WARN",
        Level::Error => "ERROR",
    });
    out.push_str(r#"","body":{"stringValue":""#);
    push_str_escaped(out, e.name);
    out.push_str(r#""},"attributes":[{"key":"component","value":{"stringValue":""#);
    out.push_str(component_str(e.component));
    out.push_str(r#""}}"#);
    for a in &e.attrs {
        out.push_str(r#",{"key":""#);
        push_str_escaped(out, a.key);
        out.push_str(r#"","value":{"stringValue":""#);
        push_str_escaped(out, &a.value);
        out.push_str(r#""}}"#);
    }
    out.push_str("]}");
}

fn component_str(c: Component) -> &'static str {
    c.as_str()
}

fn push_u64(out: &mut String, v: u64) {
    use std::fmt::Write as _;
    let _ = write!(out, "{v}");
}

fn push_str_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r"\\"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_foundation::observability::Event;
    use zerodds_foundation::tracing::{Span, SpanContext, SpanId, TraceId};

    #[test]
    fn config_default_points_to_localhost() {
        let c = OtlpConfig::default();
        assert_eq!(c.host, "127.0.0.1");
        assert_eq!(c.port, 4318);
        assert_eq!(c.service_name, "zerodds");
    }

    #[test]
    fn traces_json_roundtrip_shape() {
        let span = Span {
            context: SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8])),
            name: "dcps.write".into(),
            kind: SpanKind::Client,
            start_unix_ns: 1_700_000_000_000_000_000,
            end_unix_ns: 1_700_000_000_001_500_000,
            status: SpanStatus::Ok,
            status_description: None,
            attributes: Vec::new(),
        };
        let cfg = OtlpConfig::default();
        let body = build_traces_json(&cfg, &[(span, Vec::new())]);
        assert!(body.contains(r#""traceId":"01010101010101010101010101010101""#));
        assert!(body.contains(r#""spanId":"0202020202020202""#));
        assert!(body.contains(r#""name":"dcps.write""#));
        assert!(body.contains(r#""kind":3"#)); // Client
        assert!(body.contains(r#""service.name""#));
    }

    /// An attribute of an integer value is an `AnyValue` with `intValue`,
    /// the int64 as a decimal string (proto3 JSON mapping), after the
    /// string attributes; a span with integer attributes only has them too.
    #[test]
    fn traces_json_writes_int_attributes_as_int_value() {
        let span = |attributes| Span {
            context: SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8])),
            name: "svc/Call".into(),
            kind: SpanKind::Server,
            start_unix_ns: 1,
            end_unix_ns: 2,
            status: SpanStatus::Unset,
            status_description: None,
            attributes,
        };
        let ints = vec![
            IntAttribute {
                key: "rpc.grpc.status_code",
                value: 0,
            },
            IntAttribute {
                key: "network.peer.port",
                value: 40_000,
            },
            IntAttribute {
                key: "x.negative",
                value: -7,
            },
        ];
        let with_string = span(vec![zerodds_foundation::observability::Attribute {
            key: "rpc.system",
            value: "grpc".into(),
        }]);
        let body = build_traces_json(&OtlpConfig::default(), &[(with_string, ints.clone())]);
        assert!(
            body.contains(
                r#""attributes":[{"key":"rpc.system","value":{"stringValue":"grpc"}},{"key":"rpc.grpc.status_code","value":{"intValue":"0"}},{"key":"network.peer.port","value":{"intValue":"40000"}},{"key":"x.negative","value":{"intValue":"-7"}}]"#
            ),
            "{body}"
        );
        let body = build_traces_json(&OtlpConfig::default(), &[(span(Vec::new()), ints)]);
        assert!(
            body.contains(
                r#""attributes":[{"key":"rpc.grpc.status_code","value":{"intValue":"0"}},"#
            ),
            "{body}"
        );
        // Through the exporter's buffer: `add_span` has no integer
        // attributes, `add_span_with_ints` keeps them with its span.
        let exp = OtlpExporter::new(OtlpConfig::default());
        exp.add_span(span(Vec::new()));
        exp.add_span_with_ints(span(Vec::new()), vec![IntAttribute { key: "n", value: 3 }]);
        let buffered = exp.buf.lock().map(|b| b.spans.clone()).unwrap_or_default();
        assert_eq!(buffered.len(), 2);
        assert!(buffered[0].1.is_empty());
        assert_eq!(buffered[1].1, vec![IntAttribute { key: "n", value: 3 }]);
    }

    #[test]
    fn metrics_json_roundtrip_shape() {
        let mut h = Histogram::new("dds.write.latency");
        h.record_ns(500);
        h.record_ns(50_000);
        let cfg = OtlpConfig::default();
        let body = build_metrics_json(&cfg, &[h]);
        assert!(body.contains(r#""name":"dds.write.latency""#));
        assert!(body.contains(r#""unit":"ns""#));
        assert!(body.contains(r#""count":"2""#));
        assert!(body.contains(r#""explicitBounds":[1,10,100,1000,10000,100000,"#));
    }

    #[test]
    fn logs_json_roundtrip_shape() {
        let e = Event::new(Level::Info, Component::Dcps, "writer.created").with_attr("topic", "/x");
        let cfg = OtlpConfig::default();
        let body = build_logs_json(&cfg, &[e]);
        assert!(body.contains(r#""severityNumber":9"#));
        assert!(body.contains(r#""body":{"stringValue":"writer.created"}"#));
        assert!(body.contains(r#""key":"topic""#));
    }

    #[test]
    fn json_escape_handles_quotes_and_newlines() {
        let mut s = String::new();
        push_str_escaped(&mut s, "a\"b\nc\\d");
        assert_eq!(s, r#"a\"b\nc\\d"#);
    }

    #[test]
    fn parse_http_status_extracts_code() {
        let r = "HTTP/1.1 200 OK\r\nServer: x\r\n\r\n{}";
        let (code, body_start) = parse_http_status(r);
        assert_eq!(code, 200);
        assert_eq!(&r[body_start..], "{}");
    }

    #[test]
    fn parse_http_status_handles_500() {
        let r = "HTTP/1.1 500 Internal Server Error\r\n\r\nboom";
        let (code, _) = parse_http_status(r);
        assert_eq!(code, 500);
    }

    /// The connect to the collector waits at most `timeout`: a listener
    /// whose accept queue is full drops the SYN (Linux), and the export
    /// fails within the timeout instead of the kernel's SYN retries.
    #[cfg(target_os = "linux")]
    #[test]
    #[allow(clippy::unwrap_used)]
    fn flush_connects_within_the_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Connections the listener never accepts, until one connect waits.
        let mut held = Vec::new();
        while let Ok(s) = TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
            held.push(s);
            assert!(held.len() < 10_000, "the accept queue does not fill");
        }
        let exp = OtlpExporter::new(OtlpConfig {
            host: "127.0.0.1".into(),
            port: addr.port(),
            timeout: Duration::from_millis(300),
            ..OtlpConfig::default()
        });
        exp.add_span(Span {
            context: SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8])),
            name: "x".into(),
            kind: SpanKind::Internal,
            start_unix_ns: 0,
            end_unix_ns: 1,
            status: SpanStatus::Unset,
            status_description: None,
            attributes: Vec::new(),
        });
        let started = std::time::Instant::now();
        let r = exp.flush();
        assert!(
            matches!(&r, Err(ExportError::Io(e)) if e.kind() == std::io::ErrorKind::TimedOut),
            "{r:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        drop(held);
    }

    #[test]
    fn flush_drains_buffers_even_with_no_collector() {
        let cfg = OtlpConfig {
            host: "127.0.0.1".into(),
            port: 1, // deliberately unreachable
            timeout: Duration::from_millis(50),
            ..OtlpConfig::default()
        };
        let exp = OtlpExporter::new(cfg);
        exp.add_span(Span {
            context: SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8])),
            name: "x".into(),
            kind: SpanKind::Internal,
            start_unix_ns: 0,
            end_unix_ns: 1,
            status: SpanStatus::Unset,
            status_description: None,
            attributes: Vec::new(),
        });
        let r = exp.flush();
        // Expected: IO error on connect.
        assert!(r.is_err());
        // Buffer was emptied nonetheless.
        let r2 = exp.flush();
        assert!(r2.is_ok(), "second flush with empty buffers should be ok");
    }

    /// A collector of the test on `listener`: the request head of each of
    /// `n` connections, each answered with 200.
    #[allow(clippy::unwrap_used)]
    fn heads_of(listener: std::net::TcpListener, n: usize) -> std::thread::JoinHandle<Vec<String>> {
        std::thread::spawn(move || {
            let mut heads = Vec::new();
            for _ in 0..n {
                let (mut s, _) = listener.accept().unwrap();
                s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut req = Vec::new();
                let mut buf = [0u8; 4096];
                let body_at = loop {
                    let got = s.read(&mut buf).unwrap();
                    assert!(got > 0, "the request ended before its head");
                    req.extend_from_slice(&buf[..got]);
                    if let Some(i) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8_lossy(&req[..body_at]).to_string();
                let length: usize = head
                    .lines()
                    .find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length")
                            .then(|| v.trim().parse().ok())?
                    })
                    .unwrap();
                while req.len() < body_at + length {
                    let got = s.read(&mut buf).unwrap();
                    assert!(got > 0, "the request ended before its body");
                    req.extend_from_slice(&buf[..got]);
                }
                s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                    .unwrap();
                heads.push(head);
            }
            heads
        })
    }

    /// An IPv6 collector: `host` `[::1]`, as `OTEL_EXPORTER_OTLP_ENDPOINT`
    /// `http://[::1]:<port>` gives it, and `::1` reach the listener on the
    /// IPv6 loopback, and the `Host` field carries the address in brackets
    /// (RFC 9110 §7.2).
    #[test]
    #[allow(clippy::unwrap_used)]
    fn flush_reaches_a_collector_at_an_ipv6_address() {
        let listener = std::net::TcpListener::bind("[::1]:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let collector = heads_of(listener, 2);
        for host in ["[::1]", "::1"] {
            let exp = OtlpExporter::new(OtlpConfig {
                host: host.into(),
                port,
                timeout: Duration::from_secs(2),
                ..OtlpConfig::default()
            });
            exp.add_span(Span {
                context: SpanContext::new_root(TraceId([1u8; 16]), SpanId([2u8; 8])),
                name: "x".into(),
                kind: SpanKind::Internal,
                start_unix_ns: 0,
                end_unix_ns: 1,
                status: SpanStatus::Unset,
                status_description: None,
                attributes: Vec::new(),
            });
            let r = exp.flush();
            assert!(r.is_ok(), "host {host}: {r:?}");
        }
        let heads = collector.join().unwrap();
        assert_eq!(heads.len(), 2);
        for head in &heads {
            assert!(head.starts_with("POST /v1/traces HTTP/1.1\r\n"), "{head}");
            assert!(head.contains("\r\nHost: [::1]\r\n"), "{head}");
        }
    }

    /// Spec §3: an endpoint with or without scheme, port and trailing `/`;
    /// without a port, or with one that is not a number, 4318.
    #[test]
    fn config_from_endpoint_reads_scheme_host_and_port() {
        let c = OtlpConfig::from_endpoint("svc", "1.2.3", "http://collector.local:4319/");
        assert_eq!(
            (c.host.as_str(), c.port, c.service_name.as_str()),
            ("collector.local", 4319, "svc")
        );
        assert_eq!(c.service_version, "1.2.3");
        assert_eq!(c.timeout, Duration::from_secs(5));
        let c = OtlpConfig::from_endpoint("svc", "1", "https://otel.svc:4318");
        assert_eq!((c.host.as_str(), c.port), ("otel.svc", 4318));
        let c = OtlpConfig::from_endpoint("svc", "1", "host:9999");
        assert_eq!((c.host.as_str(), c.port), ("host", 9999));
        let c = OtlpConfig::from_endpoint("svc", "1", "host");
        assert_eq!((c.host.as_str(), c.port), ("host", 4318));
        let c = OtlpConfig::from_endpoint("svc", "1", "host:http");
        assert_eq!((c.host.as_str(), c.port), ("host", 4318));
    }

    /// Spec §4: the loop flushes after `interval`, and once more after the
    /// stop flag is set; then the thread ends.
    #[test]
    #[allow(clippy::unwrap_used)]
    fn flush_loop_flushes_each_interval_and_at_stop() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let collector = heads_of(listener, 2);
        let exp = std::sync::Arc::new(OtlpExporter::new(OtlpConfig {
            port,
            timeout: Duration::from_secs(2),
            ..OtlpConfig::default()
        }));
        let span = |n: u8| Span {
            context: SpanContext::new_root(TraceId([n; 16]), SpanId([n; 8])),
            name: "x".into(),
            kind: SpanKind::Internal,
            start_unix_ns: 0,
            end_unix_ns: 1,
            status: SpanStatus::Unset,
            status_description: None,
            attributes: Vec::new(),
        };
        exp.add_span(span(1));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handle = spawn_flush_loop(
            std::sync::Arc::clone(&exp),
            std::sync::Arc::clone(&stop),
            Duration::from_millis(50),
            Duration::from_millis(10),
            "zerodds-test-otlp",
        )
        .unwrap();
        let start = std::time::Instant::now();
        while exp.buf.lock().unwrap().spans.len() == 1 {
            assert!(start.elapsed() < Duration::from_secs(5), "no flush");
            std::thread::sleep(Duration::from_millis(5));
        }
        exp.add_span(span(2));
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        handle.join().unwrap();
        assert!(exp.buf.lock().unwrap().spans.is_empty());
        let heads = collector.join().unwrap();
        assert_eq!(heads.len(), 2);
        assert!(
            heads
                .iter()
                .all(|h| h.starts_with("POST /v1/traces HTTP/1.1\r\n"))
        );
    }
}
