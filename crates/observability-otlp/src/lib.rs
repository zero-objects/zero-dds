// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OTLP/HTTP/JSON-Exporter fuer ZeroDDS-Telemetrie.
//!
//! Crate `zerodds-observability-otlp`. Safety classification: **STANDARD**.
//!
//! Spec: `docs/specs/zerodds-observability-otlp-1.0.md`.
//! Schichten-Position: Layer 4 — Core Services (Konsumenten-Pfad fuer
//! `foundation::tracing::{Span, Histogram}` + `foundation::observability::Event`).
//!
//! # Was wird exportiert?
//!
//! Drei OTLP-Endpunkte werden bedient:
//!
//! * **`/v1/traces`** — Spans aus [`foundation::tracing::Span`].
//! * **`/v1/metrics`** — Histograms aus [`foundation::tracing::Histogram`].
//! * **`/v1/logs`** — Events aus [`foundation::observability::Event`].
//!
//! Wir benutzen das **OTLP/HTTP/JSON**-Format laut OpenTelemetry-Spec
//! v1.4 (https://github.com/open-telemetry/opentelemetry-proto/blob/v1.4.0/
//! docs/specification.md#otlphttp). JSON ist offiziell unterstuetzt
//! und braucht keine Protobuf-Codegen-Pipeline — perfekt fuer
//! pure-Rust ohne `prost`/`tonic`.
//!
//! Pro Tick werden alle gepufferten Spans/Histogramme/Events in
//! einer Batch-Request POSTed.
//!
//! # Default-Endpoints
//!
//! Standardmaessig laeuft ein lokaler OTel-Collector auf
//! `http://127.0.0.1:4318/v1/traces` etc. — siehe
//! `examples/otel/jaeger-compose.yml` fuer einen Local-Stack.

#![warn(missing_docs)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use zerodds_foundation::observability::{Component, Event, Level};
use zerodds_foundation::tracing::{Histogram, Span, SpanKind, SpanStatus};

/// Defaults — OTel-Collector auf localhost (Jaeger-compose).
pub const DEFAULT_OTLP_HOST: &str = "127.0.0.1";
/// Default OTLP/HTTP-Port (OTel-Spec).
pub const DEFAULT_OTLP_PORT: u16 = 4318;

/// Konfiguration fuer den Exporter.
#[derive(Clone, Debug)]
pub struct OtlpConfig {
    /// OTel-Collector-Host.
    pub host: String,
    /// OTel-Collector-Port (HTTP).
    pub port: u16,
    /// Service-Name fuer alle Resource-Attributes.
    pub service_name: String,
    /// Service-Version (z.B. Cargo-Version).
    pub service_version: String,
    /// Connect/Write-Timeout.
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

/// Exporter-Handle. Buffert Spans/Histogramme/Events und flush'd
/// sie via POST an den OTel-Collector.
pub struct OtlpExporter {
    cfg: OtlpConfig,
    buf: Mutex<ExporterBuffers>,
}

#[derive(Default)]
struct ExporterBuffers {
    spans: Vec<Span>,
    histograms: Vec<Histogram>,
    events: Vec<Event>,
}

/// Fehler beim Export.
#[derive(Debug)]
pub enum ExportError {
    /// Connect/Write/Read fehlgeschlagen.
    Io(std::io::Error),
    /// HTTP-Status != 2xx.
    HttpStatus {
        /// Status-Code.
        code: u16,
        /// Body-Snippet (max 256 byte).
        body_snippet: String,
    },
    /// Mutex vergiftet.
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
    /// Neuer Exporter mit der gegebenen Config.
    #[must_use]
    pub fn new(cfg: OtlpConfig) -> Self {
        Self {
            cfg,
            buf: Mutex::new(ExporterBuffers::default()),
        }
    }

    /// Fuegt einen Span zur Pending-Queue hinzu.
    pub fn add_span(&self, span: Span) {
        if let Ok(mut b) = self.buf.lock() {
            b.spans.push(span);
        }
    }

    /// Fuegt ein Histogram (Snapshot) hinzu.
    pub fn add_histogram(&self, h: Histogram) {
        if let Ok(mut b) = self.buf.lock() {
            b.histograms.push(h);
        }
    }

    /// Fuegt ein Event hinzu.
    pub fn add_event(&self, e: Event) {
        if let Ok(mut b) = self.buf.lock() {
            b.events.push(e);
        }
    }

    /// Flush + POST. Blockiert bis Collector geantwortet hat oder
    /// Timeout erreicht ist. Leert die Buffer auch bei Fehler.
    ///
    /// # Errors
    /// IO/HTTP-Fehler aus dem POST-Request.
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

    /// Direct HTTP-POST nach `path`. JSON content-type.
    fn post(&self, path: &str, body: &str) -> Result<(), ExportError> {
        let addr = format!("{}:{}", self.cfg.host, self.cfg.port);
        let mut stream = TcpStream::connect(&addr).map_err(ExportError::Io)?;
        stream.set_write_timeout(Some(self.cfg.timeout)).ok();
        stream.set_read_timeout(Some(self.cfg.timeout)).ok();
        let req = format!(
            "POST {} HTTP/1.1\r\n\
             Host: {}\r\n\
             User-Agent: zerodds-otlp/0.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            path,
            self.cfg.host,
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
// JSON-Builder — minimaler Serializer nach OTLP/HTTP/JSON-Spec.
// ============================================================================

fn build_traces_json(cfg: &OtlpConfig, spans: &[Span]) -> String {
    // Top-level: { resourceSpans: [{ resource, scopeSpans: [{ scope, spans }] }] }
    let mut out = String::with_capacity(512 + spans.len() * 256);
    out.push_str(r#"{"resourceSpans":[{"resource":"#);
    push_resource(&mut out, cfg);
    out.push_str(r#","scopeSpans":[{"scope":{"name":"zerodds","version":""#);
    push_str_escaped(&mut out, &cfg.service_version);
    out.push_str(r#""},"spans":["#);
    for (i, s) in spans.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_span(&mut out, s);
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

fn push_span(out: &mut String, s: &Span) {
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
    if !s.attributes.is_empty() {
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
        let body = build_traces_json(&cfg, &[span]);
        assert!(body.contains(r#""traceId":"01010101010101010101010101010101""#));
        assert!(body.contains(r#""spanId":"0202020202020202""#));
        assert!(body.contains(r#""name":"dcps.write""#));
        assert!(body.contains(r#""kind":3"#)); // Client
        assert!(body.contains(r#""service.name""#));
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

    #[test]
    fn flush_drains_buffers_even_with_no_collector() {
        let cfg = OtlpConfig {
            host: "127.0.0.1".into(),
            port: 1, // bewusst unreachable
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
        // Erwartet: IO-Fehler beim Connect.
        assert!(r.is_err());
        // Buffer wurde dennoch geleert.
        let r2 = exp.flush();
        assert!(r2.is_ok(), "second flush with empty buffers should be ok");
    }
}
