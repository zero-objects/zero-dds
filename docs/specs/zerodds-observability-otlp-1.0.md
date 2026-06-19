# `zerodds-observability-otlp` v1.0 — OTLP/HTTP/JSON-Exporter-Spec

ZeroDDS Vendor-Spec. In `crates/observability-otlp` (`zerodds-observability-otlp`) implementiert.

## Motivation

[`zerodds-monitor`](./zerodds-monitor-1.1.md) liefert die Registry +
Prometheus-Text-Exposition. Fuer Konsumenten, die OpenTelemetry-Stack
(Tempo / Jaeger / DataDog / Honeycomb) statt Prometheus nutzen, ist
ein OTLP-Exporter notwendig.

Diese Spec definiert den **OTLP/HTTP/JSON**-Exporter. JSON statt
Protobuf, weil:

1. Pure-Rust ohne `prost`/`tonic`-Codegen-Pipeline.
2. JSON ist ein offiziell unterstuetzter OTLP-Encoding (OTel-Spec
   v1.4 §"OTLP/HTTP/JSON").
3. Local-Stacks (Jaeger-Compose, Grafana-Tempo) akzeptieren beide
   Encodings — wir verlieren kein Konsumenten-Spektrum.

## Ziele

- **Drei OTLP-Endpoints**: `/v1/traces`, `/v1/metrics`, `/v1/logs`.
- **Resource-Attributes**: `service.name`, `service.version`, optional
  weitere via Caller-Config.
- **Buffered Flush**: Spans/Histogramme/Events werden gesammelt und in
  einem Batch-POST verschickt.
- **Pure-Rust HTTP/1.1**: kein `hyper`/`reqwest`/`prost`/`tonic`-Dep.
- **Zero-Side-Effects ohne Use**: ohne `flush()` werden keine
  HTTP-Requests gemacht.

## Nicht-Ziele

- Vollstaendige OTel-SDK-Reimpl (Sampler, Processor, Resource-Detection,
  Context-Propagation als generic API).
- gRPC-Encoding (OTLP/gRPC) — bewusst weggelassen, weil das eine
  Protobuf-Codegen-Pipeline noetig macht. Add-on `zerodds-observability-otlp-grpc`
  ist künftig moeglich, aber nicht in diesem Spec-Scope.
- Direkte Integration mit dem `Sink`-Trait — der Caller fuettert
  `OtlpExporter::add_span/add_histogram/add_event` aus eigenen Loops.

## §1 Architektur

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

Body: OTLP-`ExportTraceServiceRequest` als JSON.

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

`SpanKind`-Mapping: `Internal=1`, `Server=2`, `Client=3`. `SpanStatus::Ok=1`, `Error=2`, `Unset=0`.

### §2.2 `/v1/metrics`

Body: OTLP-`ExportMetricsServiceRequest` mit Histogramm-Daten aus
`foundation::tracing::Histogram`.

Bucket-Bounds in Sekunden — gleicher Wert wie in `zerodds-monitor` §3.1:
```
1e-09, 1e-08, 1e-07, 1e-06, 1e-05, 1e-04, 1e-03, 1e-02, 1e-01, 1, 10
```

### §2.3 `/v1/logs`

Body: OTLP-`ExportLogsServiceRequest`. `Event` aus
`foundation::observability::Event` mappt auf einen `LogRecord`:

- `severity_number` aus `Level` (`Trace=1`, `Debug=5`, `Info=9`, `Warn=13`, `Error=17`).
- `severity_text` aus `Level::as_str()`.
- `body.string_value` aus `Event::name`.
- `attributes` aus `Event::attrs`.

## §3 Konfiguration

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
// Im Hot-Path:
exporter.add_span(span);
exporter.add_histogram(histogram_snapshot);
exporter.add_event(event);
// Periodisch (z.B. alle 5s aus einem Background-Thread):
exporter.flush()?;
```

`flush()`:
1. Drain der Buffer-Mutex.
2. POST auf jeden non-empty Endpoint.
3. Bei IO-/HTTP-Fehler: Buffers bleiben gedraint (no resend) — Fehler an Caller weitergegeben.

## §5 Bridge zu `zerodds-monitor::Registry`

Der Caller kann periodisch via `Registry::snapshot()` alle Counter/
Gauge/Histogram exportieren. Der OTLP-Exporter akzeptiert
`Histogram`-Instanzen direkt (Counter/Gauge werden aktuell
nicht ueber OTLP exportiert — sie laufen nur ueber Prometheus).

> **Designentscheidung:** Counter/Gauge sind Prometheus-native und
> werden vom OTel-Collector ohnehin ueber das `prometheus`-Receiver-Modul
> ingestiert. OTLP-Metric-Doppel-Export ist im aktuellen Scope nicht
> sinnvoll — Konsumenten konfigurieren entweder Prometheus-Scrape oder
> OTLP-Push, nicht beides.

## §6 Sicherheit

- TLS: out-of-scope fuer den built-in HTTP/1.1-Stub. Caller setzen
  einen lokalen OTel-Collector als Sidecar mit TLS-Frontend (wie bei
  Prometheus-Scrape).
- Authentication-Headers: aktuell nicht unterstuetzt; Add via Header-
  Map ist Major-additive Erweiterung.

## §7 Stabilitaet

- Public-API (`OtlpConfig`, `OtlpExporter`, `ExportError`, `add_*`,
  `flush`): stabil.
- Wire-Format: an OTel-Spec v1.4 angelehnt; JSON-Schema-Aenderung
  durch upstream-OTel-Spec ist Major-Bump.

## §8 Test-Pflicht

- Roundtrip: `OtlpConfig` → JSON-Build → Parsing-Smoketest (Schema-
  Sanity, kein vollstaendiger Decoder).
- HTTP-Fehler-Handling: Mock-TCP-Server der `500 Internal Server
  Error` antwortet; Exporter muss `ExportError::HttpStatus`
  liefern.
- Connect-Refused: `flush()` muss `ExportError::Io` liefern, nicht
  panicen.
