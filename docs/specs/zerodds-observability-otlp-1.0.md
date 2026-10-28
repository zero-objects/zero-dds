# `zerodds-observability-otlp` v1.0 — OTLP/HTTP/JSON exporter spec

ZeroDDS vendor spec. Implemented in `crates/observability-otlp` (`zerodds-observability-otlp`).

## Motivation

[`zerodds-monitor`](./zerodds-monitor-1.1.md) provides the registry +
Prometheus text exposition. For consumers that use an OpenTelemetry stack
(Tempo / Jaeger / DataDog / Honeycomb) instead of Prometheus,
an OTLP exporter is necessary.

This spec defines the **OTLP/HTTP/JSON** exporter. JSON instead of
Protobuf, because:

1. Pure Rust without a `prost`/`tonic` codegen pipeline.
2. JSON is an officially supported OTLP encoding (OTel spec
   v1.4 §"OTLP/HTTP/JSON").
3. Local stacks (Jaeger Compose, Grafana Tempo) accept both
   encodings — we lose no part of the consumer spectrum.

## Goals

- **Three OTLP endpoints**: `/v1/traces`, `/v1/metrics`, `/v1/logs`.
- **Resource attributes**: `service.name`, `service.version`, optionally
  more via caller config.
- **Buffered flush**: spans/histograms/events are collected and sent in
  one batch POST.
- **Pure-Rust HTTP/1.1**: no `hyper`/`reqwest`/`prost`/`tonic` dependency.
- **Zero side effects without use**: without `flush()` no
  HTTP requests are made.

## Non-goals

- Complete OTel SDK reimplementation (sampler, processor, resource detection,
  context propagation as a generic API).
- gRPC encoding (OTLP/gRPC) — deliberately left out, because it requires a
  Protobuf codegen pipeline. An add-on `zerodds-observability-otlp-grpc`
  is possible in future, but not within the scope of this spec.
- Direct integration with the `Sink` trait — the caller feeds
  `OtlpExporter::add_span/add_histogram/add_event` from its own loops.

## §1 Architecture

```
+-------------------------+         +------------------+
| ZeroDDS-Runtime         |         |  OTel-Collector  |
|  + monitor::Registry    |         |   /v1/traces     |
|  + tracing::Span        | ─────►  |   /v1/metrics    |
|  + observability::Event |  POST   |   /v1/logs       |
+-------------------------+         +------------------+
       │                                   │
       │ add_span / add_histogram          │
       │ add_event                         │
       ▼                                   │
+-------------------------+                ▼
| OtlpExporter            |        +-----------+
|  ExporterBuffers        |        | Tempo /   |
|  (Mutex)                |        | Jaeger /  |
|                         |        | Datadog   |
|  flush() → POST         |        +-----------+
+-------------------------+
```

## §2 Endpoints

### §2.1 `/v1/traces`

Body: OTLP `ExportTraceServiceRequest` as JSON.

```json
{
  "resourceSpans": [{
    "resource": {
      "attributes": [
        {"key":"service.name","value":{"stringValue":"zerodds"}},
        {"key":"service.version","value":{"stringValue":"1.0.0-rc.1"}}
      ]
    },
    "scopeSpans": [{
      "scope": {"name":"zerodds-observability-otlp","version":"1.0.0-rc.1"},
      "spans": [
        {
          "traceId":"4bf92f3577b34da6a3ce929d0e0e4736",
          "spanId":"00f067aa0ba902b7",
          "parentSpanId":"...",
          "name":"dds.publish",
          "kind":3,
          "startTimeUnixNano":"1714915200000000000",
          "endTimeUnixNano":"1714915200001234567",
          "attributes":[...],
          "status":{"code":1}
        }
      ]
    }]
  }]
}
```

`SpanKind` mapping: `Internal=1`, `Server=2`, `Client=3`. `SpanStatus::Ok=1`, `Error=2`, `Unset=0`.

### §2.2 `/v1/metrics`

Body: OTLP `ExportMetricsServiceRequest` with histogram data from
`foundation::tracing::Histogram`.

Bucket bounds in seconds — the same value as in `zerodds-monitor` §3.1:
```
1e-09, 1e-08, 1e-07, 1e-06, 1e-05, 1e-04, 1e-03, 1e-02, 1e-01, 1, 10
```

### §2.3 `/v1/logs`

Body: OTLP `ExportLogsServiceRequest`. `Event` from
`foundation::observability::Event` maps to a `LogRecord`:

- `severity_number` from `Level` (`Trace=1`, `Debug=5`, `Info=9`, `Warn=13`, `Error=17`).
- `severity_text` from `Level::as_str()`.
- `body.string_value` from `Event::name`.
- `attributes` from `Event::attrs`.

## §3 Configuration

```rust
pub struct OtlpConfig {
    pub host: String,                 // default 127.0.0.1
    pub port: u16,                    // default 4318 (OTel-Spec)
    pub service_name: String,         // default "zerodds"
    pub service_version: String,      // default = CARGO_PKG_VERSION
    pub timeout: Duration,            // default 5s
}
```

## §4 Lifecycle

```rust
let exporter = OtlpExporter::new(OtlpConfig::default());
// In the hot path:
exporter.add_span(span);
exporter.add_histogram(histogram_snapshot);
exporter.add_event(event);
// Periodically (e.g. every 5s from a background thread):
exporter.flush()?;
```

`flush()`:
1. Drain of the buffer mutex.
2. POST to every non-empty endpoint.
3. On an IO/HTTP error: the buffers stay drained (no resend) — the error is passed on to the caller.

## §5 Bridge to `zerodds-monitor::Registry`

The caller can periodically export all counters/
gauges/histograms via `Registry::snapshot()`. The OTLP exporter accepts
`Histogram` instances directly (counters/gauges are currently
not exported via OTLP — they only go through Prometheus).

> **Design decision:** Counters/gauges are Prometheus-native and
> are ingested by the OTel Collector anyway via the `prometheus` receiver module.
> A double OTLP metric export does not make sense in the current scope
> — consumers configure either a Prometheus scrape or
> an OTLP push, not both.

## §6 Security

- TLS: out of scope for the built-in HTTP/1.1 stub. Callers put
  a local OTel Collector as a sidecar with a TLS front end (as with the
  Prometheus scrape).
- Authentication headers: currently not supported; adding them via a header
  map is a major-additive extension.

## §7 Stability

- Public API (`OtlpConfig`, `OtlpExporter`, `ExportError`, `add_*`,
  `flush`): stable.
- Wire format: based on OTel spec v1.4; a JSON schema change
  by the upstream OTel spec is a major bump.

## §8 Test obligation

- Roundtrip: `OtlpConfig` → JSON build → parsing smoke test (schema
  sanity, not a complete decoder).
- HTTP error handling: a mock TCP server that answers `500 Internal Server
  Error`; the exporter must return
  `ExportError::HttpStatus`.
- Connect refused: `flush()` must return `ExportError::Io`, not
  panic.
