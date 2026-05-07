// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Jaeger-Talker Demo — Pumpt echte Spans + Histogramme + Events
//! ueber `OtlpExporter` an einen lokalen Jaeger-Backend.
//!
//! ## Run
//!
//! Erst Jaeger via Docker hochfahren (siehe
//! `examples/demos/otel/jaeger-compose.yml`):
//!
//! ```bash
//! cd examples/demos/otel
//! docker compose -f jaeger-compose.yml up -d
//! ```
//!
//! Dann den Talker laufen lassen:
//!
//! ```bash
//! cargo run --example jaeger_talker -p zerodds-observability-otlp
//! ```
//!
//! Verifikation: oeffne `http://localhost:16686` (Jaeger UI),
//! waehle Service `zerodds-talker`, klicke "Find Traces".
//! Du solltest 50 Spans `dcps.write` und 50 Spans `dcps.read`
//! sehen plus 1 Histogramm `dcps.write_latency_ns` mit ~50 Samples.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::expect_used)]

use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zerodds_foundation::observability::{Attribute, Component, Event, Level};
use zerodds_foundation::tracing::{
    Histogram, Span, SpanContext, SpanId, SpanKind, SpanStatus, TraceId,
};

fn attr(key: &'static str, value: impl Into<String>) -> Attribute {
    Attribute {
        key,
        value: value.into(),
    }
}
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = OtlpConfig {
        service_name: "zerodds-talker".into(),
        service_version: env!("CARGO_PKG_VERSION").into(),
        ..OtlpConfig::default()
    };

    println!(
        "[talker] OTLP-Endpoint: http://{}:{}/v1/traces",
        cfg.host, cfg.port
    );
    let exporter = OtlpExporter::new(cfg);

    // 50 simulierte DCPS-write-Spans + Sub-Spans + Latenz-Histogramm.
    let mut hist = Histogram::new("dcps.write_latency_ns");
    for i in 0..50 {
        let trace = TraceId([i as u8; 16]);
        let span_id_pub = SpanId([(i as u8).wrapping_add(0x10); 8]);
        let span_id_sub = SpanId([(i as u8).wrapping_add(0x20); 8]);

        let start = now_ns();
        // Simulate latency.
        sleep(Duration::from_millis(2));
        let end = now_ns();
        let latency_ns = end - start;
        hist.record_ns(latency_ns);

        // Publisher-Span (Client-side).
        exporter.add_span(Span {
            context: SpanContext::new_root(trace, span_id_pub),
            name: "dcps.write".into(),
            kind: SpanKind::Client,
            start_unix_ns: start,
            end_unix_ns: end,
            status: SpanStatus::Ok,
            status_description: None,
            attributes: vec![
                attr("topic", "Chat::Message"),
                attr("payload_size", "128"),
                attr("sample_id", i.to_string()),
            ],
        });

        // Subscriber-Span (Server-side).
        exporter.add_span(Span {
            context: SpanContext::child_of(&SpanContext::new_root(trace, span_id_pub), span_id_sub),
            name: "dcps.read".into(),
            kind: SpanKind::Server,
            start_unix_ns: end,
            end_unix_ns: end + 500_000,
            status: SpanStatus::Ok,
            status_description: None,
            attributes: vec![
                attr("topic", "Chat::Message"),
                attr("subscriber_id", "alice"),
            ],
        });

        if i % 10 == 9 {
            println!("[talker] sent {} sample-pairs", i + 1);
        }
    }

    exporter.add_histogram(hist);

    exporter.add_event(Event {
        level: Level::Info,
        component: Component::Dcps,
        name: "talker.demo.flushed",
        attrs: vec![attr("samples", "50")],
    });

    print!("[talker] flushing batch to OTLP-Collector ... ");
    match exporter.flush() {
        Ok(()) => {
            println!("OK");
            println!("[talker] verify: open http://localhost:16686");
            println!("[talker]         service=zerodds-talker, click 'Find Traces'");
            Ok(())
        }
        Err(e) => {
            eprintln!("FAILED: {e}");
            eprintln!(
                "[talker] ist Jaeger gestartet? cd examples/demos/otel && docker compose -f jaeger-compose.yml up -d"
            );
            Err(Box::new(e))
        }
    }
}
