# `zerodds-monitor` v1.1 — Observability-Substrate-Spec

ZeroDDS Vendor-Spec. In `crates/monitor` (`zerodds-monitor`) implementiert.
Löst `zerodds-monitor-1.0` ab (wire-kompatibler Minor-Bump: identische Metriken,
identische PID `0x0D00`, identisches Prometheus-Text-Format; neu sind eine
dyn-freie Redaction-/Kardinalitäts-Policy-API, präzisierte Zählungen und eine
geschärfte Test-Pflicht).

## Motivation

Es gibt keine OMG-Spec für DDS-Monitoring. RTI-Connext-Monitoring und
Cyclone-Statistics sind Vendor-spezifisch und nicht interoperabel. ZeroDDS wählt
einen orthogonalen Pfad: **Industrie-Standards** — Prometheus-Text-Format,
OpenMetrics, OpenTelemetry-Semantic-Conventions, W3C-Trace-Context — statt eines
proprietären Topic-Schemas.

Diese Spec definiert das **Telemetry-Substrate** zwischen dem ZeroDDS-Runtime und
einem Konsumenten-Stack (Grafana, Tempo, Datadog, Honeycomb).

## Änderungen gegenüber 1.0

| # | 1.0 | 1.1 |
|---|---|---|
| 1 | §9-Hooks als `Option<Box<dyn Fn(...)>>` | **dyn-freie Enum-Policies** (`TopicLabelPolicy`, `GuidLabelPolicy`) in `MonitorConfig` — konform mit `forbid(unsafe)`/`no_dyn_in_safe` und `MonitorConfig: Clone` |
| 2 | §9 nennt `MonitorConfig::guid_obfuscator`, §6.2-Struct listet es nicht | §6.2-`MonitorConfig` und §9 **vereinheitlicht** — die Policy-Felder stehen in der Struct |
| 3 | §2: „23 Counter, 4 Gauge, 4 Histogram" | korrigiert: **25 Counter, 3 Gauge, 3 Histogram** (Summe 31) |
| 4 | §1.4/§3.1: „11 Buckets" vs 11+`+Inf` | präzisiert: **11 endliche Buckets + `+Inf`** |
| 5 | §4.1: PID-Range-Begründung fragwürdig | **ehrliche PID-Note** (Standard-Range-Caveat, Wire-Stabilität, Vendor-Range als 2.0-Option) |
| 6 | §7 normativ, ohne Test-Bindung | §7 bleibt normativ und ist per **§10-Cross-Layer-Test** zu belegen |

Wire-Verträglichkeit: Default aller neuen Policies ist `Full` (= Verhalten von
1.0). Ein 1.0-Konfigurat verhält sich unter 1.1 unverändert.

## Ziele

- **Industrie-Standard-Output**: Prometheus-Text-Format (§3) + OpenTelemetry-Spans
  (siehe `zerodds-observability-otlp-1.0.md`).
- **Allocation-light Hot-Path**: Counter/Gauge sind atomare Integer; Histogramme
  nutzen die fixen log10-Buckets aus `foundation::tracing::Histogram`.
- **Cross-Process via DDS**: W3C-Trace-Context als RTPS-Vendor-PID
  `PID_VENDOR_TRACE_CONTEXT` (0x0D00) inline-QoS-propagiert.
- **Zero-Config-Sane-Default**: ohne Konsumenten-Wire-up bleibt das System silent.
- **Datenschutz per Policy (neu in 1.1)**: Topic-Kardinalität und GUID-Exposition
  sind über deklarative, dyn-freie Policies steuerbar — ohne Closures im
  Konfigurat.

## Nicht-Ziele

- Eigener Storage-Layer (Prometheus / Tempo / Loki übernehmen das).
- Vollständige OTel-SDK-Reimpl — nur die Datenmodell-Schicht; OTLP-Export macht
  `zerodds-observability-otlp`.
- Topic-basiertes Monitoring (à la RTI `rti/dds/monitoring/...`) — explizit
  verworfen, weil es OMG-inkompatible Wire-Pfade einführt.
- **Closure-/dyn-basierte Hooks** — bewusst verworfen (Architektur:
  `forbid(unsafe_code)`, `no_dyn_in_safe`); Steuerung erfolgt deklarativ (§9).

## §1 Datenmodell

### §1.1 Schichten

```
+-----------------------------------------+
|  zerodds-observability-otlp             |  (OTLP/HTTP/JSON)
|  zerodds-monitor::prometheus            |  (Prometheus-Text)
+-----------------------------------------+
|  zerodds-monitor::Registry              |  (Counter / Gauge / Histogram)
+-----------------------------------------+
|  foundation::tracing                    |  (Histogram + Span + TraceId/SpanId)
|  foundation::observability              |  (Event + Sink-Trait)
+-----------------------------------------+
```

`Histogram`, `Span`, `TraceId`, `SpanId`, `Event` leben in **`foundation`**
(Layer 0); `Counter`, `Gauge`, `Registry`, Prometheus-Exporter und
Trace-Context-PID-Codec in **`monitor`** (Layer 4).

### §1.2 Counter

```rust
pub struct Counter { name: &'static str, labels: Labels, value: AtomicU64 }
impl Counter { pub fn inc(&self); pub fn add(&self, n: u64); pub fn get(&self) -> u64; }
```

- **Monoton**, `AtomicU64`/`Relaxed`.
- **Identität** = `(name, labels)`; Doppel-Registrierung liefert dieselbe Instance (§1.6).

### §1.3 Gauge

```rust
pub struct Gauge { name: &'static str, labels: Labels, value: AtomicI64 }
impl Gauge { pub fn set(&self, v: i64); pub fn inc(&self); pub fn dec(&self); pub fn add(&self, n: i64); pub fn get(&self) -> i64; }
```

- **Bidirektional**, `AtomicI64`/`Relaxed`.

### §1.4 Histogram

`zerodds_foundation::tracing::Histogram` wird as-is re-exportiert. Bucket-Layout
ist fix log10: **11 endliche Buckets** (`1e-9 s` … `10 s`) **plus `+Inf`** (siehe
§3.1).

```rust
pub use zerodds_foundation::tracing::Histogram;

pub struct LabeledHistogram { pub name: &'static str, pub labels: Labels, pub histogram: Mutex<Histogram> }
```

### §1.5 Labels

```rust
pub struct Labels { pairs: Vec<(&'static str, String)> }  // sortiert nach Key
```

- **Key** = `&'static str`; **Value** = `String` (`topic`, `transport`, `error_kind`, `policy_id`, …).
- **Sortierung** alphabetisch nach Key (deterministische Prometheus-Ausgabe).
- **Kardinalität**: in 1.0 reine Caller-Verantwortung; in 1.1 zusätzlich über
  `TopicLabelPolicy` (§9) deklarativ begrenzbar.

### §1.6 Registry

```rust
pub struct Registry { /* Mutex<HashMap<MetricKey, Arc<…>>> je Kind */ }
impl Registry {
    pub fn counter(&self, name: &'static str, labels: Labels) -> Arc<Counter>;
    pub fn gauge(&self, name: &'static str, labels: Labels) -> Arc<Gauge>;
    pub fn histogram(&self, name: &'static str, labels: Labels) -> Arc<LabeledHistogram>;
    pub fn render_prometheus(&self) -> String;
    pub fn snapshot(&self) -> RegistrySnapshot;
}
```

- **Single-Source-of-Truth** pro Process; **idempotenter Lookup**; **Default-Registry**
  als globaler `OnceLock<Arc<Registry>>` über `default_registry()`.

## §2 Standard-Metric-Naming

```
dds_<domain>_<thing>[_<unit>][_total]
```

- `<domain>` ∈ `transport`, `rtps`, `dcps`, `discovery`, `security`.
- `<unit>` ∈ `seconds`, `bytes`, ggf. weglassbar; `_total`-Suffix für monotone Counter.

### §2.1 Transport-Domain (6)

| Metric | Kind | Labels |
|---|---|---|
| `dds_transport_packets_sent_total` | Counter | `transport`, `domain_id` |
| `dds_transport_packets_received_total` | Counter | `transport`, `domain_id` |
| `dds_transport_bytes_sent_total` | Counter | `transport`, `domain_id` |
| `dds_transport_bytes_received_total` | Counter | `transport`, `domain_id` |
| `dds_transport_send_errors_total` | Counter | `transport`, `error_kind` |
| `dds_transport_socket_buffer_bytes` | Gauge | `transport`, `direction` |

### §2.2 RTPS-Domain (6)

| Metric | Kind | Labels |
|---|---|---|
| `dds_rtps_heartbeats_sent_total` | Counter | `writer_kind` |
| `dds_rtps_acknacks_received_total` | Counter | `writer_kind` |
| `dds_rtps_retransmits_total` | Counter | `writer_kind` |
| `dds_rtps_samples_dropped_total` | Counter | `writer_kind`, `reason` |
| `dds_rtps_fragmented_samples_total` | Counter | `writer_kind` |
| `dds_rtps_unknown_submessages_total` | Counter | `vendor_id` |

### §2.3 DCPS-Domain (10)

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

Das `topic`-Label dieser Domain unterliegt der `TopicLabelPolicy` (§9).

### §2.4 Discovery-Domain (5)

| Metric | Kind | Labels |
|---|---|---|
| `dds_discovery_participants_known` | Gauge | `domain_id` |
| `dds_discovery_endpoints_known` | Gauge | `domain_id`, `kind` |
| `dds_discovery_spdp_announcements_sent_total` | Counter | `domain_id` |
| `dds_discovery_sedp_updates_total` | Counter | `domain_id`, `kind` |
| `dds_discovery_type_lookups_total` | Counter | `domain_id` |

### §2.5 Security-Domain (4)

| Metric | Kind | Labels |
|---|---|---|
| `dds_security_auth_attempts_total` | Counter | `result` |
| `dds_security_access_denied_total` | Counter | `operation`, `topic` |
| `dds_security_crypto_operations_total` | Counter | `operation` |
| `dds_security_crypto_latency_seconds` | Histogram | `operation` |

**Summe: 31 Metriken = 25 Counter, 3 Gauge, 3 Histogram.**

## §3 Prometheus-Text-Format-Encoding

Per Prometheus-Exposition-Format v0.0.4 (`text/plain; version=0.0.4; charset=utf-8`).
Render-Output mit deterministisch sortierten Metric-Names und Labels;
leere Registry → leerer String.

### §3.1 Bucket-Konvention

Histogramme werden in **Sekunden** exportiert (foundation zählt in Nanosekunden,
der Exporter konvertiert). Cumulative Buckets, `+Inf` als letzter Bucket — **11
endliche `le`-Werte plus `+Inf`**:

```
1e-09, 1e-08, 1e-07, 1e-06, 1e-05, 1e-04, 1e-03, 1e-02, 1e-01, 1, 10, +Inf
```

### §3.2 Label-Escaping

`\\` → `\\\\`, `"` → `\\"`, `\n` → `\\n` (Prometheus-Spec).

### §3.3 Render-Output

`Registry::render_prometheus(&self) -> String` — volle Exposition, sortiert.

## §4 W3C-Trace-Context als RTPS-Vendor-PID

### §4.1 PID-Allocation

```
PID_VENDOR_TRACE_CONTEXT = 0x0D00  (OctetsToInlineQos-relevant)
```

> **Ehrliche Note (1.1):** `0x0D00` liegt im **Standard-PID-Range**
> (< `0x8000`), nicht im Vendor-Range. Das ist eine bewusste 1.0-Entscheidung,
> birgt aber das Risiko einer künftigen OMG-Standard-PID-Kollision. Empfänger,
> die den PID nicht verstehen, ignorieren ihn (RTPS 2.5 §9.6.3.2 —
> `IGNORE`-Pfad für unbekannte PIDs), daher ist er für Fremd-Vendoren (Cyclone,
> RTI, Fast-DDS) transparent. Der PID + sein Wire-Format bleiben in 1.1
> **wire-stabil**; ein Wechsel in den Vendor-Range (`0x8000+`) ist als
> **2.0-Major** vorgemerkt (wäre RTPS-wire-breaking).

### §4.2 Wire-Format

ParameterList-Element-Payload mit zwei CDR-Strings: `traceparent` (W3C-Trace-
Context 1.0, `00-{trace-id}-{parent-id}-{flags}`) + optional `tracestate`
(§3.3 W3C, vendor-Key-Values). Unverändert gegenüber 1.0.

### §4.3 Encoding/Decoding

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

- **Publisher**: `DataWriter::write` liest den aktuellen Span und encodet den PID
  als InlineQoS gemäß `emit_trace_context` (`always | sampled | never`,
  Default `sampled`).
- **Subscriber**: `DataReader` extrahiert den PID und erzeugt einen
  `dds.sample.receive`-Span mit `parent = traceparent`, gemäß
  `accept_trace_context` (Default `true`).
- ZeroDDS respektiert die Caller-OTel-Sampler-Decision (keine eigene erzwungen).

## §5 Span-Schema

Spans nutzen `zerodds_foundation::tracing::Span` und folgen den OTel-Semantic-
Conventions plus `dds.`-Namespace. Span-Typen + Attribute unverändert gegenüber
1.0 (`dds.publish`, `dds.sample.*`, `dds.rtps.reliable.nack`,
`dds.discovery.match`, `dds.security.authenticate`).

> **Datenschutz-Bezug:** mehrere Span-Attribute tragen GUIDs (`dds.writer_guid`,
> `dds.reader_guid`, `dds.remote_guid`, `dds.source_guid`). Deren Exposition
> beim OTLP-Export wird über `GuidLabelPolicy` (§9) gesteuert.

## §6 Lifecycle und Konfiguration

### §6.1 Default-Registry

```rust
pub fn default_registry() -> Arc<Registry>;   // OnceLock, lazy
```

### §6.2 Konfiguration (1.1 — erweitert)

```rust
#[derive(Clone, Debug)]
pub struct MonitorConfig {
    pub emit_trace_context: TraceContextEmission,   // always | sampled | never
    pub accept_trace_context: bool,
    pub enable_metrics: bool,
    pub topic_label_policy: TopicLabelPolicy,        // NEU 1.1, Default Full
    pub guid_label_policy: GuidLabelPolicy,          // NEU 1.1, Default Full
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceContextEmission { Always, Sampled, Never }   // Default Sampled
```

Alle Felder sind `Clone + Debug`; **keine `dyn`/Closures**.

### §6.3 Prometheus-Server (optional)

```rust
pub fn serve_prometheus(addr: SocketAddr, registry: Arc<Registry>) -> Result<JoinHandle<()>, ServeError>;
```

Mini-HTTP-Server (TcpListener, kein hyper-Dep); `GET /metrics` → Prometheus-Text,
unbekannte Pfade → 404.

## §7 Hook-Point-Tabelle (normativ)

Welche Crate emittiert welche Metric/Span? Diese Tabelle ist **normativ** — jeder
Hook-Point ist im Coverage-Audit als Cross-Layer-Finding nachweisbar und durch
den §10-Cross-Layer-Test gegen die Default-Registry zu belegen.

| Crate | Metric/Span | Wire-Up |
|---|---|---|
| `transport-udp/-tcp/-shm/-uds` | `dds_transport_*` | `Transport::send`/`recv` |
| `rtps` | `dds_rtps_*` | `WriterCache`/`ReaderCache`/`fragment_assembler` |
| `discovery` | `dds_discovery_*` | `SpdpStack`/`SedpStack`/`TypeLookupService` |
| `dcps` | alle 10 DCPS-Metriken + Spans `dds.publish`, `dds.sample.deliver` | `DataWriter::write`/`DataReader::take`/`Subscriber::deliver` |
| `security`/`security-crypto` | `dds_security_*`, Span `dds.security.authenticate` | `AuthHandshake`/`CryptoPlugin::*` |

## §8 Stabilität

- **Public-API** (`Counter`, `Gauge`, `Histogram`, `Registry`, `Labels`,
  `MonitorConfig`, `TopicLabelPolicy`, `GuidLabelPolicy`): stabil; Major-Bump bei
  Breaking-Changes. Die 1.1-Policy-Felder sind **additiv** (neue Felder mit
  `Default`, daher kein Bruch für `MonitorConfig { .. }`-Konstruktion via
  `..Default::default()`).
- **Metric-Namen + Label-Keys**: stabil; nur additive Erweiterung.
- **PID 0x0D00 + Wire-Format**: stabil (Vendor-Range-Wechsel = 2.0, §4.1).
- **Span-Namen + Attr-Keys**: folgen OTel-Semconv.

## §9 Sicherheit (1.1 — deklarative Policies)

- **Payload-Redaction**: Counter/Gauge/Histogram speichern keinen Sample-Content
  (nur Zahlen). Span-Attribute tragen Größen (`dds.sample_size`), keinen Content.
- **GUID-Exposition** über `GuidLabelPolicy` (ersetzt den 1.0-`dyn`-Hook):

  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum GuidLabelPolicy {
      Full,    // GUID als Hex (Default; sinnvoll in Trusted-Netzen)
      Hashed,  // stabiler Kurz-Hash der GUID in Span-Attributen
      Omit,    // GUID-Attribute weglassen
  }
  ```
  Wirkt auf die GUID-tragenden §5-Span-Attribute beim Export.

- **Kardinalitäts-Begrenzung** über `TopicLabelPolicy` (ersetzt den 1.0-`topic_label_filter`):

  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum TopicLabelPolicy {
      Full,            // Topic-Name unverändert (Default)
      Truncate(usize), // Topic-Label auf N Zeichen kürzen
      Hashed,          // stabiler Kurz-Hash statt Klartext
      Drop,            // topic-Label ganz weglassen (max. Kardinalitäts-Schutz)
  }
  ```
  Wirkt auf das `topic`-Label der DCPS-Metriken (§2.3) bei der Label-Erzeugung.

Beide Policies sind dyn-frei (Enum statt `Box<dyn Fn>`), `Clone`, deterministisch
und damit testbar; Default `Full` = 1.0-Verhalten.

## §10 Test-Pflicht

- Counter/Gauge: atomare Inkrement-Konsistenz unter parallelen Threads.
- Histogram-Re-Export: Cross-Validation mit `foundation::tracing::Histogram`
  (gleiche Bucket-Werte).
- Prometheus-Text: Golden-Vector-Test gegen handvalidierten String; Bucket-/
  `_sum`/`_count`-Layout.
- Label-Escaping: alle drei Escape-Pfade (`\\`, `"`, `\n`).
- PID 0x0D00 Encode/Decode-Roundtrip mit drei `traceparent`-Beispielen.
- **Cross-Layer (normativ, §7):** `dcps::DataWriter::write` inkrementiert
  `dds_dcps_samples_written_total`, danach enthält
  `default_registry().render_prometheus()` das Increment.
- **Policy (neu 1.1):** `TopicLabelPolicy::{Truncate,Hashed,Drop}` und
  `GuidLabelPolicy::{Hashed,Omit}` verändern die gerenderte Ausgabe deterministisch;
  `Full` ist Identität (1.0-Verhalten).

## §11 Coverage-Doc

`docs/spec-coverage/zerodds-monitor-1.1.md` trägt das Sektion-pro-Sektion-Mapping
mit Repo + Tests + Status. Akzeptanz: alle §-Sektionen `done`; partial/open ist
Release-Blocker.
