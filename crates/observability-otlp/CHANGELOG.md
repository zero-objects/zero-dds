# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-observability-otlp`-Crate.

### Spec-Referenzen

- **`docs/specs/zerodds-observability-otlp-1.0.md`** §1-§8 (komplett).
- **OpenTelemetry Protocol v1.4** — OTLP/HTTP/JSON Encoding.

### Public-API

- `OtlpConfig` — Host/Port/Service-Name/Service-Version/Timeout.
- `OtlpExporter::{new, add_span, add_histogram, add_event, flush}`.
- `ExportError::{Io, HttpStatus, Poisoned}`.
- `DEFAULT_OTLP_HOST = "127.0.0.1"`, `DEFAULT_OTLP_PORT = 4318`.

### Implementierung

Pure-Rust HTTP/1.1 ohne `hyper`/`reqwest`/`prost`/`tonic`. JSON-Encoding manuell, weil OTLP/HTTP/JSON ein offizielles Encoding von OTel-Spec v1.4 ist und so eine vollstaendige Codegen-Pipeline (Protobuf → Rust) entfaellt. `add_span` / `add_histogram` / `add_event` sind Mutex-buffered; `flush()` drained und POSTet einen Batch pro non-empty Endpoint.

`SpanKind`-Mapping: `Internal=1`, `Server=2`, `Client=3`. `SpanStatus`: `Unset=0`, `Ok=1`, `Error=2`. `Level` → severity_number per OpenTelemetry-Logs-Convention (`Trace=1`, `Debug=5`, `Info=9`, `Warn=13`, `Error=17`).

Histogram-Bucket-Bounds in Sekunden: `1e-09 .. 10` log10-Schritten — identisch mit `zerodds-monitor::prometheus`.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-foundation` (Span/Histogram/Event-Datenmodell). Keine weiteren ZeroDDS-Crate-Deps.
- **Dependents (out):** End-User-Builds direkt.
- **Feature-Flags:** keine.

### Stabilitaet

Public-API + Wire-Format RC1-stabil. OTel-Spec-v1.5+ Wire-Aenderung wuerde Major-Bump erfordern.
