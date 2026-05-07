# `zerodds-observability-otlp`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-observability-otlp/badge.svg)](https://docs.rs/zerodds-observability-otlp)

OTLP/HTTP/JSON-Exporter fuer [ZeroDDS](https://zerodds.org) —
buffered Span/Histogram/Event-Push an einen OpenTelemetry-Collector
ohne `prost`/`tonic`/`hyper`-Dep. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| ZeroDDS-Observability-OTLP 1.0 | §1 (Architektur), §2.1-§2.3 (Endpoints), §3 (Konfiguration), §4 (Lifecycle), §5 (Bridge zu monitor::Registry) |
| OpenTelemetry Protocol 1.4 | OTLP/HTTP/JSON Encoding |

## Was ist drin

- **`OtlpExporter`** — Buffered Span/Histogram/Event-Sammler mit `flush()`-getriggertem Batch-POST.
- **`OtlpConfig`** — Host/Port/Service-Name/Service-Version/Timeout (Defaults: 127.0.0.1:4318).
- **Drei Endpoints:** `/v1/traces`, `/v1/metrics`, `/v1/logs` als JSON.
- **`ExportError`** — Io / HttpStatus / Poisoned.

## Schichten-Position

Layer 4 — Core Services. Companion zu [`zerodds-monitor`](../monitor) (Prometheus-Pfad).

## Quickstart

```rust,no_run
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};
use zerodds_foundation::tracing::{Histogram, Span, SpanKind, SpanStatus, SpanContext, TraceId, SpanId};

let exp = OtlpExporter::new(OtlpConfig::default());

// Hot-Path: Spans/Histogramme akkumulieren
let span = Span {
    context: SpanContext::new_root(TraceId([1; 16]), SpanId([2; 8])),
    name: "dds.publish".into(),
    kind: SpanKind::Client,
    start_unix_ns: 0,
    end_unix_ns: 1_000,
    status: SpanStatus::Ok,
    status_description: None,
    attributes: vec![],
};
exp.add_span(span);
exp.add_histogram(Histogram::new("dds.write.latency"));

// Periodisch (z.B. alle 5s)
let _ = exp.flush();
```

## Stabilitaet

`1.0.0-rc.1`. Wire-Format an OTel-Spec v1.4 angelehnt — Aenderung
durch upstream-OTel ist Major-Bump.

## Tests

```bash
cargo test -p zerodds-observability-otlp
```

## Lizenz

Apache-2.0.

## Siehe auch

- [`docs/specs/zerodds-observability-otlp-1.0.md`](../../docs/specs/zerodds-observability-otlp-1.0.md)
- [`zerodds-monitor`](../monitor) — Counter/Gauge/Histogram-Registry + Prometheus-Exporter.
- [`zerodds-foundation`](../foundation) — `tracing::Span` / `tracing::Histogram` / `observability::Event`.
