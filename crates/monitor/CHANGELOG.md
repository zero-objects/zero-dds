# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-monitor`-Crate.

### Spec-Referenzen

- **`docs/specs/zerodds-monitor-1.0.md`** §1 (Datenmodell), §2 (Standard-Naming, 31 Metriken), §3 (Prometheus-Text-Format), §4 (PID 0x0D00 W3C-Trace-Context), §5 (Span-Schema, 9 Span-Typen), §6 (Lifecycle), §7 (Hook-Tabelle), §8 (Stabilitaet), §9 (Sicherheit), §10 (Test-Pflicht).
- **Industrie-Standards:** Prometheus-Text-Format v0.0.4, OpenMetrics, W3C-Trace-Context 1.0, OpenTelemetry-Semantic-Conventions.

### Public-API

**Metric-Typen:**
- `Counter` (AtomicU64), `Gauge` (AtomicI64), `LabeledHistogram` (Mutex<foundation::Histogram>).
- `Labels` (sortiert, dedupend), `MetricKey`.

**Registry:**
- `Registry::{new, counter, gauge, histogram, set_help, snapshot, render_prometheus}`.
- `default_registry()` — globaler `OnceLock<Arc<Registry>>`.
- `RegistrySnapshot`.

**Prometheus-Text:**
- `render_prometheus(&snapshot) -> String` — OpenMetrics v0.0.4-Exposition.
- `serve_prometheus(addr, registry)` (Feature `prometheus-server`) — Mini-HTTP-`/metrics`-Endpoint.

**Trace-Context:**
- `PID_VENDOR_TRACE_CONTEXT = 0x0D00`.
- `TraceContextPid::{encode_inline_qos, decode_inline_qos, from_span_context, to_span_context}`.
- `TraceParent`, `TraceState`, `TraceContextError`.

**Konstanten:**
- `metric_names::*` — 31 Standard-Metric-Namen (Transport / RTPS / DCPS / Discovery / Security).
- `span_names::*` — 9 Standard-Span-Namen.
- `span_names::attr::*` — 16 DDS-Attribut-Keys.

**Konfiguration:**
- `MonitorConfig`, `TraceContextEmission::{Always, Sampled, Never}`.

**Re-Exports:**
- `zerodds_foundation::tracing::Histogram` als `Histogram`.

### Implementierung

`Counter` und `Gauge` sind `AtomicU64`/`AtomicI64` mit `Ordering::Relaxed` (keine Cross-Thread-Causality fuer Counter). `LabeledHistogram` ist `Mutex<Histogram>` weil `Histogram` Mutate-Methoden hat. `Registry` haelt `Mutex<HashMap<MetricKey, Arc<...>>>` fuer Counter/Gauge/Histogram getrennt. Idempotente Lookup: zweiter `counter("x", labels)` liefert dieselbe Instance.

Prometheus-Render macht eine deterministische Sortierung: nach Metric-Name (BTreeMap), pro Metric nach Labels (cmp). Histogramme werden in Sekunden konvertiert (foundation zaehlt in ns), Bucket-Bounds sind log10 von `1e-09` bis `10` Sekunden, plus `+Inf`. Label-Escape per Prometheus-Spec (`\`, `"`, `\n`).

PID 0x0D00 Wire-Format: zwei CDR-Strings (length+bytes+NUL+padding-zu-4-byte). `traceparent` parst W3C-Format `00-{32hex}-{16hex}-{2hex}` und lehnt all-zero Trace-/Span-IDs ab (W3C-Spec).

`serve_prometheus` ist ein Mini-HTTP-Server auf `TcpListener`-Basis ohne `hyper`-Dep — reicht fuer Prometheus-Scrape. Pre-RC1 lieferte das ein Stub `// TODO: Implementierung folgt` (12 LOC) — die volle RC1-Materialisierung ist mit dieser Version eingefuehrt.

`forbid(unsafe_code)` ist gesetzt (per Workspace-Lints).

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-foundation` (Span/Histogram/TraceId/SpanId/Event-Datenmodell). Keine weiteren ZeroDDS-Crate-Deps.
- **Dependents (out):** `zerodds-rtps` (Feature `metrics`), `zerodds-discovery` (Feature `metrics`), `zerodds-dcps` (Feature `metrics`), `zerodds-transport-udp`, `zerodds-security-crypto` (Feature `metrics`), `tools/dashboard`, `tools/perf`.
- **Feature-Flags:** `std` (default), `alloc` (via std), `prometheus-server` (default an).

### Stabilitaet

- Public-API: RC1-stabil.
- Metric-Namen + Label-Keys: stabil; Label-Keys-Erweiterung ist Major-additive (Prometheus-Selectors brechen nicht).
- PID 0x0D00 Wire-Format: stabil ab RC1; Aenderung waere RTPS-Wire-Breaking.
- Span-Namen: folgen OTel-Semantic-Conventions; Releases verfolgen die Semconv-Versionen.
