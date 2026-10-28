# `zerodds-monitor` v1.1 — observability substrate spec

ZeroDDS vendor spec. Implemented in `crates/monitor` (`zerodds-monitor`).
Supersedes `zerodds-monitor-1.0` (wire-compatible minor bump: identical metrics,
identical PID `0x0D00`, identical Prometheus text format; new are a
dyn-free redaction/cardinality policy API, more precise counts and a
stricter test requirement).

## Motivation

There is no OMG spec for DDS monitoring. RTI Connext Monitoring and
Cyclone Statistics are vendor-specific and not interoperable. ZeroDDS chooses
an orthogonal path: **industry standards** — Prometheus text format,
OpenMetrics, OpenTelemetry semantic conventions, W3C Trace Context — instead of a
proprietary topic schema.

This spec defines the **telemetry substrate** between the ZeroDDS runtime and
a consumer stack (Grafana, Tempo, Datadog, Honeycomb).

## Changes compared with 1.0

| # | 1.0 | 1.1 |
|---|---|---|
| 1 | §9 hooks as `Option<Box<dyn Fn(...)>>` | **dyn-free enum policies** (`TopicLabelPolicy`, `GuidLabelPolicy`) in `MonitorConfig` — conformant with `forbid(unsafe)`/`no_dyn_in_safe` and `MonitorConfig: Clone` |
| 2 | §9 names `MonitorConfig::guid_obfuscator`, the §6.2 struct does not list it | §6.2 `MonitorConfig` and §9 **unified** — the policy fields are in the struct |
| 3 | §2: "23 counters, 4 gauges, 4 histograms" | corrected: **25 counters, 3 gauges, 3 histograms** (sum 31) |
| 4 | §1.4/§3.1: "11 buckets" vs 11+`+Inf` | made precise: **11 finite buckets + `+Inf`** |
| 5 | §4.1: questionable reasoning for the PID range | **honest PID note** (standard range caveat, wire stability, vendor range as a 2.0 option) |
| 6 | §7 normative, without a test binding | §7 stays normative and must be proven by the **§10 cross-layer test** |

Wire compatibility: the default of all new policies is `Full` (= behaviour of
1.0). A 1.0 configuration behaves unchanged under 1.1.

## Goals

- **Industry-standard output**: Prometheus text format (§3) + OpenTelemetry spans
  (see `zerodds-observability-otlp-1.0.md`).
- **Allocation-light hot path**: counters/gauges are atomic integers; histograms
  use the fixed log10 buckets from `foundation::tracing::Histogram`.
- **Cross-process via DDS**: W3C Trace Context as RTPS vendor PID
  `PID_VENDOR_TRACE_CONTEXT` (0x0D00), propagated as inline QoS.
- **Zero-config sane default**: without consumer wiring the system stays silent.
- **Data protection by policy (new in 1.1)**: topic cardinality and GUID exposure
  can be controlled via declarative, dyn-free policies — without closures in the
  configuration.

## Non-goals

- Our own storage layer (Prometheus / Tempo / Loki take care of that).
- Complete OTel SDK reimplementation — only the data model layer; OTLP export is done by
  `zerodds-observability-otlp`.
- Topic-based monitoring (like RTI `rti/dds/monitoring/...`) — explicitly
  rejected, because it introduces OMG-incompatible wire paths.
- **Closure/dyn-based hooks** — deliberately rejected (architecture:
  `forbid(unsafe_code)`, `no_dyn_in_safe`); control is declarative (§9).

## §1 Data model

### §1.1 Layers

```
+-----------------------------------------+
|  zerodds-observability-otlp             |  (OTLP/HTTP/JSON)
|  zerodds-monitor::prometheus            |  (Prometheus text)
+-----------------------------------------+
|  zerodds-monitor::Registry              |  (Counter / Gauge / Histogram)
+-----------------------------------------+
|  foundation::tracing                    |  (Histogram + Span + TraceId/SpanId)
|  foundation::observability              |  (Event + Sink trait)
+-----------------------------------------+
```

`Histogram`, `Span`, `TraceId`, `SpanId`, `Event` live in **`foundation`**
(layer 0); `Counter`, `Gauge`, `Registry`, the Prometheus exporter and the
trace context PID codec in **`monitor`** (layer 4).

### §1.2 Counter

```rust
pub struct Counter { name: &'static str, labels: Labels, value: AtomicU64 }
impl Counter { pub fn inc(&self); pub fn add(&self, n: u64); pub fn get(&self) -> u64; }
```

- **Monotonic**, `AtomicU64`/`Relaxed`.
- **Identity** = `(name, labels)`; double registration returns the same instance (§1.6).

### §1.3 Gauge

```rust
pub struct Gauge { name: &'static str, labels: Labels, value: AtomicI64 }
impl Gauge { pub fn set(&self, v: i64); pub fn inc(&self); pub fn dec(&self); pub fn add(&self, n: i64); pub fn get(&self) -> i64; }
```

- **Bidirectional**, `AtomicI64`/`Relaxed`.

### §1.4 Histogram

`zerodds_foundation::tracing::Histogram` is re-exported as-is. The bucket layout
is fixed log10: **11 finite buckets** (`1e-9 s` … `10 s`) **plus `+Inf`** (see
§3.1).

```rust
pub use zerodds_foundation::tracing::Histogram;

pub struct LabeledHistogram { pub name: &'static str, pub labels: Labels, pub histogram: Mutex<Histogram> }
```

### §1.5 Labels

```rust
pub struct Labels { pairs: Vec<(&'static str, String)> }  // sorted by key
```

- **Key** = `&'static str`; **value** = `String` (`topic`, `transport`, `error_kind`, `policy_id`, …).
- **Sorting** alphabetically by key (deterministic Prometheus output).
- **Cardinality**: in 1.0 purely the caller's responsibility; in 1.1 it can additionally
  be limited declaratively via `TopicLabelPolicy` (§9).

### §1.6 Registry

```rust
pub struct Registry { /* Mutex<HashMap<MetricKey, Arc<…>>> per kind */ }
impl Registry {
    pub fn counter(&self, name: &'static str, labels: Labels) -> Arc<Counter>;
    pub fn gauge(&self, name: &'static str, labels: Labels) -> Arc<Gauge>;
    pub fn histogram(&self, name: &'static str, labels: Labels) -> Arc<LabeledHistogram>;
    pub fn render_prometheus(&self) -> String;
    pub fn snapshot(&self) -> RegistrySnapshot;
}
```

- **Single source of truth** per process; **idempotent lookup**; **default registry**
  as a global `OnceLock<Arc<Registry>>` via `default_registry()`.

## §2 Standard metric naming

```
dds_<domain>_<thing>[_<unit>][_total]
```

- `<domain>` ∈ `transport`, `rtps`, `dcps`, `discovery`, `security`.
- `<unit>` ∈ `seconds`, `bytes`, may be omitted where appropriate; `_total` suffix for monotonic counters.

### §2.1 Transport domain (6)

| Metric | Kind | Labels |
|---|---|---|
| `dds_transport_packets_sent_total` | Counter | `transport`, `domain_id` |
| `dds_transport_packets_received_total` | Counter | `transport`, `domain_id` |
| `dds_transport_bytes_sent_total` | Counter | `transport`, `domain_id` |
| `dds_transport_bytes_received_total` | Counter | `transport`, `domain_id` |
| `dds_transport_send_errors_total` | Counter | `transport`, `error_kind` |
| `dds_transport_socket_buffer_bytes` | Gauge | `transport`, `direction` |

### §2.2 RTPS domain (6)

| Metric | Kind | Labels |
|---|---|---|
| `dds_rtps_heartbeats_sent_total` | Counter | `writer_kind` |
| `dds_rtps_acknacks_received_total` | Counter | `writer_kind` |
| `dds_rtps_retransmits_total` | Counter | `writer_kind` |
| `dds_rtps_samples_dropped_total` | Counter | `writer_kind`, `reason` |
| `dds_rtps_fragmented_samples_total` | Counter | `writer_kind` |
| `dds_rtps_unknown_submessages_total` | Counter | `vendor_id` |

### §2.3 DCPS domain (10)

| Metric | Kind | Labels |
|---|---|---|
| `dds_dcps_samples_written_total` | Counter | `topic` |
| `dds_dcps_samples_read_total` | Counter | `topic` |
| `dds_dcps_samples_lost_total` | Counter | `topic` |
| `dds_dcps_deadline_missed_total` | Counter | `topic`, `entity_kind` |
| `dds_dcps_liveliness_lost_total` | Counter | `topic` |
| `dds_dcps_subscription_matched_total` | Counter | `topic` |
| `dds_dcps_subscription_unmatched_total` | Counter | `topic` |
| `dds_dcps_incompatible_qos_total` | Counter | `topic`, `policy_id` |
| `dds_dcps_sample_latency_seconds` | Histogram | `topic` |
| `dds_dcps_sample_size_bytes` | Histogram | `topic` |

The `topic` label of this domain is subject to the `TopicLabelPolicy` (§9).

### §2.4 Discovery domain (5)

| Metric | Kind | Labels |
|---|---|---|
| `dds_discovery_participants_known` | Gauge | `domain_id` |
| `dds_discovery_endpoints_known` | Gauge | `domain_id`, `kind` |
| `dds_discovery_spdp_announcements_sent_total` | Counter | `domain_id` |
| `dds_discovery_sedp_updates_total` | Counter | `domain_id`, `kind` |
| `dds_discovery_type_lookups_total` | Counter | `domain_id` |

### §2.5 Security domain (4)

| Metric | Kind | Labels |
|---|---|---|
| `dds_security_auth_attempts_total` | Counter | `result` |
| `dds_security_access_denied_total` | Counter | `operation`, `topic` |
| `dds_security_crypto_operations_total` | Counter | `operation` |
| `dds_security_crypto_latency_seconds` | Histogram | `operation` |

**Total: 31 metrics = 25 counters, 3 gauges, 3 histograms.**

## §3 Prometheus text format encoding

Per Prometheus exposition format v0.0.4 (`text/plain; version=0.0.4; charset=utf-8`).
Render output with deterministically sorted metric names and labels;
empty registry → empty string.

### §3.1 Bucket convention

Histograms are exported in **seconds** (foundation counts in nanoseconds,
the exporter converts). Cumulative buckets, `+Inf` as the last bucket — **11
finite `le` values plus `+Inf`**:

```
1e-09, 1e-08, 1e-07, 1e-06, 1e-05, 1e-04, 1e-03, 1e-02, 1e-01, 1, 10, +Inf
```

### §3.2 Label escaping

`\\` → `\\\\`, `"` → `\\"`, `\n` → `\\n` (Prometheus spec).

### §3.3 Render output

`Registry::render_prometheus(&self) -> String` — full exposition, sorted.

## §4 W3C Trace Context as RTPS vendor PID

### §4.1 PID allocation

```
PID_VENDOR_TRACE_CONTEXT = 0x0D00  (relevant for OctetsToInlineQos)
```

> **Honest note (1.1):** `0x0D00` lies in the **standard PID range**
> (< `0x8000`), not in the vendor range. This is a deliberate 1.0 decision,
> but carries the risk of a future collision with an OMG standard PID. Receivers
> that do not understand the PID ignore it (RTPS 2.5 §9.6.3.2 —
> `IGNORE` path for unknown PIDs), so it is transparent to other vendors (Cyclone,
> RTI, Fast-DDS). The PID + its wire format stay
> **wire-stable** in 1.1; a move into the vendor range (`0x8000+`) is earmarked as
> a **2.0 major** (it would be RTPS-wire-breaking).

### §4.2 Wire format

ParameterList element payload with two CDR strings: `traceparent` (W3C Trace
Context 1.0, `00-{trace-id}-{parent-id}-{flags}`) + optional `tracestate`
(§3.3 W3C, vendor key values). Unchanged compared with 1.0.

### §4.3 Encoding/decoding

```rust
pub struct TraceContextPid { pub traceparent: TraceParent, pub tracestate: Option<TraceState> }
impl TraceContextPid {
    pub fn encode_inline_qos(&self, out: &mut Vec<u8>);
    pub fn decode_inline_qos(bytes: &[u8]) -> Result<Self, TraceContextError>;
    pub fn from_span_context(ctx: &SpanContext, vendor_state: Option<&str>) -> Self;
    pub fn to_span_context(&self) -> SpanContext;
}
```

### §4.4 Lifecycle

- **Publisher**: `DataWriter::write` reads the current span and encodes the PID
  as inline QoS according to `emit_trace_context` (`always | sampled | never`,
  default `sampled`).
- **Subscriber**: `DataReader` extracts the PID and creates a
  `dds.sample.receive` span with `parent = traceparent`, according to
  `accept_trace_context` (default `true`).
- ZeroDDS respects the caller's OTel sampler decision (does not enforce its own).

## §5 Span schema

Spans use `zerodds_foundation::tracing::Span` and follow the OTel semantic
conventions plus the `dds.` namespace. Span types + attributes unchanged compared with
1.0 (`dds.publish`, `dds.sample.*`, `dds.rtps.reliable.nack`,
`dds.discovery.match`, `dds.security.authenticate`).

> **Data protection relevance:** several span attributes carry GUIDs (`dds.writer_guid`,
> `dds.reader_guid`, `dds.remote_guid`, `dds.source_guid`). Their exposure
> during OTLP export is controlled via `GuidLabelPolicy` (§9).

## §6 Lifecycle and configuration

### §6.1 Default registry

```rust
pub fn default_registry() -> Arc<Registry>;   // OnceLock, lazy
```

### §6.2 Configuration (1.1 — extended)

```rust
#[derive(Clone, Debug)]
pub struct MonitorConfig {
    pub emit_trace_context: TraceContextEmission,   // always | sampled | never
    pub accept_trace_context: bool,
    pub enable_metrics: bool,
    pub topic_label_policy: TopicLabelPolicy,        // NEW 1.1, default Full
    pub guid_label_policy: GuidLabelPolicy,          // NEW 1.1, default Full
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceContextEmission { Always, Sampled, Never }   // default Sampled
```

All fields are `Clone + Debug`; **no `dyn`/closures**.

### §6.3 Prometheus server (optional)

```rust
pub fn serve_prometheus(addr: SocketAddr, registry: Arc<Registry>) -> Result<JoinHandle<()>, ServeError>;
```

Mini HTTP server (TcpListener, no hyper dependency); `GET /metrics` → Prometheus text,
unknown paths → 404.

## §7 Hook point table (normative)

Which crate emits which metric/span? This table is **normative** — every
hook point is traceable in the coverage audit as a cross-layer finding and must be proven by
the §10 cross-layer test against the default registry.

| Crate | Metric/span | Wiring |
|---|---|---|
| `transport-udp/-tcp/-shm/-uds` | `dds_transport_*` | `Transport::send`/`recv` |
| `rtps` | `dds_rtps_*` | `WriterCache`/`ReaderCache`/`fragment_assembler` |
| `discovery` | `dds_discovery_*` | `SpdpStack`/`SedpStack`/`TypeLookupService` |
| `dcps` | all 10 DCPS metrics + spans `dds.publish`, `dds.sample.deliver` | `DataWriter::write`/`DataReader::take`/`Subscriber::deliver` |
| `security`/`security-crypto` | `dds_security_*`, span `dds.security.authenticate` | `AuthHandshake`/`CryptoPlugin::*` |

## §8 Stability

- **Public API** (`Counter`, `Gauge`, `Histogram`, `Registry`, `Labels`,
  `MonitorConfig`, `TopicLabelPolicy`, `GuidLabelPolicy`): stable; major bump on
  breaking changes. The 1.1 policy fields are **additive** (new fields with
  `Default`, hence no break for `MonitorConfig { .. }` construction via
  `..Default::default()`).
- **Metric names + label keys**: stable; only additive extension.
- **PID 0x0D00 + wire format**: stable (move to vendor range = 2.0, §4.1).
- **Span names + attribute keys**: follow OTel semconv.

## §9 Security (1.1 — declarative policies)

- **Payload redaction**: counters/gauges/histograms store no sample content
  (only numbers). Span attributes carry sizes (`dds.sample_size`), no content.
- **GUID exposure** via `GuidLabelPolicy` (replaces the 1.0 `dyn` hook):

  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum GuidLabelPolicy {
      Full,    // GUID as hex (default; sensible in trusted networks)
      Hashed,  // stable short hash of the GUID in span attributes
      Omit,    // omit GUID attributes
  }
  ```
  Applies to the GUID-carrying §5 span attributes during export.

- **Cardinality limiting** via `TopicLabelPolicy` (replaces the 1.0 `topic_label_filter`):

  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum TopicLabelPolicy {
      Full,            // topic name unchanged (default)
      Truncate(usize), // truncate the topic label to N characters
      Hashed,          // stable short hash instead of plain text
      Drop,            // omit the topic label entirely (max. cardinality protection)
  }
  ```
  Applies to the `topic` label of the DCPS metrics (§2.3) when the label is created.

Both policies are dyn-free (enum instead of `Box<dyn Fn>`), `Clone`, deterministic
and therefore testable; default `Full` = 1.0 behaviour.

## §10 Test requirement

- Counter/gauge: atomic increment consistency under parallel threads.
- Histogram re-export: cross-validation with `foundation::tracing::Histogram`
  (same bucket values).
- Prometheus text: golden vector test against a hand-validated string; bucket/
  `_sum`/`_count` layout.
- Label escaping: all three escape paths (`\\`, `"`, `\n`).
- PID 0x0D00 encode/decode round trip with three `traceparent` examples.
- **Cross-layer (normative, §7):** `dcps::DataWriter::write` increments
  `dds_dcps_samples_written_total`; afterwards
  `default_registry().render_prometheus()` contains the increment.
- **Policy (new in 1.1):** `TopicLabelPolicy::{Truncate,Hashed,Drop}` and
  `GuidLabelPolicy::{Hashed,Omit}` change the rendered output deterministically;
  `Full` is the identity (1.0 behaviour).

## §11 Coverage document

`docs/spec-coverage/zerodds-monitor-1.1.md` carries the section-by-section mapping
with repo + tests + status. Acceptance: all § sections `done`; partial/open is a
release blocker.
