# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-monitor` crate.

### Spec references

- **`docs/specs/zerodds-monitor-1.0.md`** §1 (data model), §2 (standard naming, 31 metrics), §3 (Prometheus text format), §4 (PID 0x0D00 W3C trace context), §5 (span schema, 9 span types), §6 (lifecycle), §7 (hook table), §8 (stability), §9 (security), §10 (test requirement).
- **Industry standards:** Prometheus text format v0.0.4, OpenMetrics, W3C-Trace-Context 1.0, OpenTelemetry semantic conventions.

### Public API

**Metric types:**
- `Counter` (AtomicU64), `Gauge` (AtomicI64), `LabeledHistogram` (Mutex<foundation::Histogram>).
- `Labels` (sorted, deduplicating), `MetricKey`.

**Registry:**
- `Registry::{new, counter, gauge, histogram, set_help, snapshot, render_prometheus}`.
- `default_registry()` — globaler `OnceLock<Arc<Registry>>`.
- `RegistrySnapshot`.

**Prometheus text:**
- `render_prometheus(&snapshot) -> String` — OpenMetrics v0.0.4 exposition.
- `serve_prometheus(addr, registry)` (feature `prometheus-server`) — mini-HTTP `/metrics` endpoint.

**Trace context:**
- `PID_VENDOR_TRACE_CONTEXT = 0x0D00`.
- `TraceContextPid::{encode_inline_qos, decode_inline_qos, from_span_context, to_span_context}`.
- `TraceParent`, `TraceState`, `TraceContextError`.

**Constants:**
- `metric_names::*` — 31 standard metric names (transport / RTPS / DCPS / discovery / security).
- `span_names::*` — 9 standard span names.
- `span_names::attr::*` — 16 DDS attribute keys.

**Configuration:**
- `MonitorConfig`, `TraceContextEmission::{Always, Sampled, Never}`.

**Re-exports:**
- `zerodds_foundation::tracing::Histogram` as `Histogram`.

### Implementation

`Counter` and `Gauge` are `AtomicU64`/`AtomicI64` with `Ordering::Relaxed` (no cross-thread causality for counters). `LabeledHistogram` is a `Mutex<Histogram>` because `Histogram` has mutating methods. `Registry` holds a separate `Mutex<HashMap<MetricKey, Arc<...>>>` for counters/gauges/histograms. Idempotent lookup: a second `counter("x", labels)` returns the same instance.

The Prometheus render does a deterministic sort: by metric name (BTreeMap), per metric by labels (cmp). Histograms are converted to seconds (foundation counts in ns), bucket bounds are log10 from `1e-09` to `10` seconds, plus `+Inf`. Label escaping per the Prometheus spec (`\`, `"`, `\n`).

PID 0x0D00 wire format: two CDR strings (length+bytes+NUL+padding-to-4-byte). `traceparent` parses the W3C format `00-{32hex}-{16hex}-{2hex}` and rejects all-zero trace/span IDs (W3C spec).

`serve_prometheus` is a mini-HTTP server based on `TcpListener` without a `hyper` dep — sufficient for a Prometheus scrape. Pre-RC1 this was a stub `// TODO: implementation to follow` (12 LOC) — the full RC1 materialization is introduced with this version.

`forbid(unsafe_code)` is set (via workspace lints).

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** `zerodds-foundation` (Span/Histogram/TraceId/SpanId/Event data model). No further ZeroDDS crate deps.
- **Dependents (out):** `zerodds-rtps` (feature `metrics`), `zerodds-discovery` (feature `metrics`), `zerodds-dcps` (feature `metrics`), `zerodds-transport-udp`, `zerodds-security-crypto` (feature `metrics`), `tools/dashboard`, `tools/perf`.
- **Feature flags:** `std` (default), `alloc` (via std), `prometheus-server` (default on).

### Stability

- Public API: RC1-stable.
- Metric names + label keys: stable; label-key extension is major-additive (Prometheus selectors do not break).
- PID 0x0D00 wire format: stable from RC1; a change would be RTPS-wire-breaking.
- Span names: follow OTel semantic conventions; releases track the semconv versions.
