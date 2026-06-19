# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization der `zerodds-observability-otlp`-Crate.

### Spec-Referenzen

- **`docs/specs/zerodds-observability-otlp-1.0.md`** §1-§8 (komplett).
- **OpenTelemetry Protocol v1.4** — OTLP/HTTP/JSON Encoding.

### Public-API

- `OtlpConfig` — Host/Port/Service-Name/Service-Version/Timeout.
- `OtlpExporter::{new, add_span, add_histogram, add_event, flush}`.
- `ExportError::{Io, HttpStatus, Poisoned}`.
- `DEFAULT_OTLP_HOST = "127.0.0.1"`, `DEFAULT_OTLP_PORT = 4318`.

### Implementation

Pure-Rust HTTP/1.1 without `hyper`/`reqwest`/`prost`/`tonic`. JSON encoding is manual, because OTLP/HTTP/JSON is an official encoding of OTel spec v1.4, so a full codegen pipeline (Protobuf → Rust) is unnecessary. `add_span` / `add_histogram` / `add_event` are Mutex-buffered; `flush()` drains and POSTs one batch per non-empty endpoint.

`SpanKind`-Mapping: `Internal=1`, `Server=2`, `Client=3`. `SpanStatus`: `Unset=0`, `Ok=1`, `Error=2`. `Level` → severity_number per OpenTelemetry-Logs-Convention (`Trace=1`, `Debug=5`, `Info=9`, `Warn=13`, `Error=17`).

Histogram-Bucket-Bounds in Sekunden: `1e-09 .. 10` log10-Schritten — identisch mit `zerodds-monitor::prometheus`.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-foundation` (Span/Histogram/Event data model). No further ZeroDDS crate deps.
- **Dependents (out):** End-User-Builds direkt.
- **Feature flags:** none.

### Stabilitaet

Public API + wire format RC1-stable. An OTel-spec-v1.5+ wire change would require a major bump.
