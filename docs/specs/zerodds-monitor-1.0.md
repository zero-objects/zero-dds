# `zerodds-monitor` v1.0 — Observability-Substrate-Spec

ZeroDDS Vendor-Spec. In `crates/monitor` (`zerodds-monitor`) implementiert.

## Motivation

Es gibt keine OMG-Spec fuer DDS-Monitoring. RTI-Connext-Monitoring und Cyclone-Statistics sind Vendor-spezifisch und nicht interoperabel. ZeroDDS waehlt einen orthogonalen Pfad: **Industrie-Standards** — Prometheus-Text-Format, OpenMetrics, OpenTelemetry-Semantic-Conventions, W3C-Trace-Context — statt eines proprietaeren Topic-Schemas.

Diese Spec definiert die **Telemetry-Substrate** zwischen dem ZeroDDS-Runtime und einem Konsumenten-Stack (Grafana, Tempo, Datadog, Honeycomb).

## Ziele

- **Industrie-Standard-Output**: Prometheus-Text-Format §5 + OpenTelemetry-Spans (siehe `zerodds-observability-otlp-1.0.md`).
- **Allocation-light Hot-Path**: Counter/Gauge sind atomare u64; Histogramme nutzen die fixen log10-Buckets aus `foundation::tracing::Histogram`.
- **Cross-Process via DDS**: W3C-Trace-Context wird als RTPS-Vendor-PID `PID_VENDOR_TRACE_CONTEXT` (0x0D00) inline-QoS-propagiert, sodass Publisher- und Subscriber-Spans im selben distributed Trace landen.
- **Zero-Config-Sane-Default**: ohne Konsumenten-Wire-up bleibt das System silent — null-Sink, kein Prometheus-Server, kein Span-Emission.

## Nicht-Ziele

- Eigener Storage-Layer (Prometheus / Tempo / Loki uebernehmen das).
- Vollstaendige OTel-SDK-Reimpl — nur die Datenmodell-Schicht; OTLP-Export macht `zerodds-observability-otlp`.
- Topic-basiertes Monitoring (a la RTI-`rti/dds/monitoring/...`) — explizit verworfen, weil es OMG-incompatible Wire-Pfade einfuehrt.

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

`Histogram`, `Span`, `TraceId`, `SpanId` und `Event` leben in **`foundation`** (Layer 0), weil sie Substrate-Primitives ohne Allocator-Druck sind. `Counter`, `Gauge`, `Registry`, `Prometheus`-Exporter und Trace-Context-PID-Codec leben in **`monitor`** (Layer 4), weil sie Konsumenten-Pfad-Aufbau sind.

### §1.2 Counter

```rust
pub struct Counter {
    name: &'static str,
    labels: Labels,
    value: AtomicU64,
}

impl Counter {
    pub fn inc(&self);                  // value += 1
    pub fn add(&self, n: u64);          // value += n
    pub fn get(&self) -> u64;
}
```

- **Monoton**: `inc`/`add` darf nur wachsen. `value` ist `AtomicU64` mit `Ordering::Relaxed` (keine Cross-Thread-Causality fuer Counters).
- **Identitaet**: `(name, labels)`-Tupel ist die Identitaet. Doppel-Registrierung mit derselben Identitaet liefert die selbe Instance (siehe §1.5).

### §1.3 Gauge

```rust
pub struct Gauge {
    name: &'static str,
    labels: Labels,
    value: AtomicI64,
}

impl Gauge {
    pub fn set(&self, v: i64);
    pub fn inc(&self);
    pub fn dec(&self);
    pub fn add(&self, n: i64);
    pub fn get(&self) -> i64;
}
```

- **Bidirektional**: Gauge kann steigen/fallen.
- **AtomicI64**: signed; `Ordering::Relaxed`.

### §1.4 Histogram

`zerodds_foundation::tracing::Histogram` wird als-ist re-exportiert. Bucket-Layout ist fix log10 von `1ns` bis `10s` (11 Buckets). Spec-konform fuer alle DCPS-Hot-Path-Latenzen.

```rust
pub use zerodds_foundation::tracing::Histogram;
```

Histogram in der Registry hat zusaetzlich Labels:

```rust
pub struct LabeledHistogram {
    pub name: &'static str,
    pub labels: Labels,
    pub histogram: Mutex<Histogram>,
}
```

### §1.5 Labels

```rust
pub struct Labels {
    pairs: Vec<(&'static str, String)>,   // sortiert nach Key
}
```

- **Key**: `&'static str` (compile-time-stable Label-Name).
- **Value**: `String` (runtime-Werte: `topic`, `transport`, `error_kind`, `policy_id`).
- **Kardinalitaet**: Caller verantwortet Bound — kein automatischer Limit. Spec-Empfehlung: max 10 Werte pro Key.
- **Sortierung**: alphabetisch nach Key — fuer deterministische Prometheus-Text-Ausgabe.

### §1.6 Registry

```rust
pub struct Registry {
    counters:   Mutex<HashMap<MetricKey, Arc<Counter>>>,
    gauges:     Mutex<HashMap<MetricKey, Arc<Gauge>>>,
    histograms: Mutex<HashMap<MetricKey, Arc<LabeledHistogram>>>,
}

impl Registry {
    pub fn counter(&self, name: &'static str, labels: Labels) -> Arc<Counter>;
    pub fn gauge(&self, name: &'static str, labels: Labels) -> Arc<Gauge>;
    pub fn histogram(&self, name: &'static str, labels: Labels) -> Arc<LabeledHistogram>;
    pub fn render_prometheus(&self) -> String;
    pub fn snapshot(&self) -> RegistrySnapshot;
}
```

- **Single-Source-of-Truth**: pro Process eine `Registry` per `Arc`.
- **Idempotente Lookup**: zweiter Aufruf von `counter("x", labels)` liefert dieselbe `Arc<Counter>`.
- **Default-Registry** als globaler `OnceLock<Arc<Registry>>` ueber `default_registry()`.

## §2 Standard-Metric-Naming

Alle ZeroDDS-Metriken folgen OpenMetrics-Konventionen:

```
dds_<domain>_<thing>[_<unit>][_total]
```

- `<domain>` ∈ `transport`, `rtps`, `dcps`, `discovery`, `security`.
- `<unit>` ∈ `seconds`, `bytes`, ggf. weglassbar.
- `_total`-Suffix fuer monotone Counter (Prometheus-Konvention).

### §2.1 Transport-Domain (6 Metrics)

| Metric | Kind | Labels | Beschreibung |
|---|---|---|---|
| `dds_transport_packets_sent_total` | Counter | `transport`, `domain_id` | RTPS-Pakete gesendet |
| `dds_transport_packets_received_total` | Counter | `transport`, `domain_id` | RTPS-Pakete empfangen |
| `dds_transport_bytes_sent_total` | Counter | `transport`, `domain_id` | Bytes gesendet |
| `dds_transport_bytes_received_total` | Counter | `transport`, `domain_id` | Bytes empfangen |
| `dds_transport_send_errors_total` | Counter | `transport`, `error_kind` | Send-Fehler |
| `dds_transport_socket_buffer_bytes` | Gauge | `transport`, `direction` | Socket-Buffer-Auslastung |

### §2.2 RTPS-Domain (6 Metrics)

| Metric | Kind | Labels | Beschreibung |
|---|---|---|---|
| `dds_rtps_heartbeats_sent_total` | Counter | `writer_kind` | Heartbeats gesendet |
| `dds_rtps_acknacks_received_total` | Counter | `writer_kind` | Acknacks empfangen |
| `dds_rtps_retransmits_total` | Counter | `writer_kind` | Retransmissions |
| `dds_rtps_samples_dropped_total` | Counter | `writer_kind`, `reason` | Samples gedropped |
| `dds_rtps_fragmented_samples_total` | Counter | `writer_kind` | Fragmentierte Samples |
| `dds_rtps_unknown_submessages_total` | Counter | `vendor_id` | Unbekannte Submessage-Kinds |

### §2.3 DCPS-Domain (10 Metrics)

| Metric | Kind | Labels | Beschreibung |
|---|---|---|---|
| `dds_dcps_samples_written_total` | Counter | `topic` | Geschriebene Samples |
| `dds_dcps_samples_read_total` | Counter | `topic` | Gelesene Samples |
| `dds_dcps_samples_lost_total` | Counter | `topic` | SAMPLE_LOST-Status |
| `dds_dcps_deadline_missed_total` | Counter | `topic`, `entity_kind` | Deadline-Misses |
| `dds_dcps_liveliness_lost_total` | Counter | `topic` | Liveliness-Lost-Events |
| `dds_dcps_subscription_matched_total` | Counter | `topic` | Neue Matches |
| `dds_dcps_subscription_unmatched_total` | Counter | `topic` | Verlorene Matches |
| `dds_dcps_incompatible_qos_total` | Counter | `topic`, `policy_id` | QoS-Inkompatibilitaeten |
| `dds_dcps_sample_latency_seconds` | Histogram | `topic` | E2E-Latency (wall-clock) |
| `dds_dcps_sample_size_bytes` | Histogram | `topic` | Sample-Groessen |

### §2.4 Discovery-Domain (5 Metrics)

| Metric | Kind | Labels | Beschreibung |
|---|---|---|---|
| `dds_discovery_participants_known` | Gauge | `domain_id` | Bekannte Participants |
| `dds_discovery_endpoints_known` | Gauge | `domain_id`, `kind` | Bekannte Endpoints |
| `dds_discovery_spdp_announcements_sent_total` | Counter | `domain_id` | SPDP-Announcements |
| `dds_discovery_sedp_updates_total` | Counter | `domain_id`, `kind` | SEDP-Updates |
| `dds_discovery_type_lookups_total` | Counter | `domain_id` | TypeLookup-Requests |

### §2.5 Security-Domain (4 Metrics)

| Metric | Kind | Labels | Beschreibung |
|---|---|---|---|
| `dds_security_auth_attempts_total` | Counter | `result` | Authentication-Versuche |
| `dds_security_access_denied_total` | Counter | `operation`, `topic` | Access-Control-Denials |
| `dds_security_crypto_operations_total` | Counter | `operation` | Crypto-Operationen |
| `dds_security_crypto_latency_seconds` | Histogram | `operation` | Crypto-Latenz |

Summe: **31 Metriken**, davon 23 Counter, 4 Gauge, 4 Histogram.

## §3 Prometheus-Text-Format-Encoding

Per Prometheus-Exposition-Format v0.0.4 (`text/plain; version=0.0.4; charset=utf-8`):

```
# HELP dds_dcps_samples_written_total Geschriebene Samples
# TYPE dds_dcps_samples_written_total counter
dds_dcps_samples_written_total{topic="VehicleTracking.TrackUpdate"} 1234
dds_dcps_samples_written_total{topic="Telemetry.Heartbeat"} 5678

# HELP dds_dcps_sample_latency_seconds E2E-Latency
# TYPE dds_dcps_sample_latency_seconds histogram
dds_dcps_sample_latency_seconds_bucket{topic="VehicleTracking.TrackUpdate",le="1e-09"} 0
dds_dcps_sample_latency_seconds_bucket{topic="VehicleTracking.TrackUpdate",le="1e-08"} 12
...
dds_dcps_sample_latency_seconds_bucket{topic="VehicleTracking.TrackUpdate",le="+Inf"} 5000
dds_dcps_sample_latency_seconds_sum{topic="VehicleTracking.TrackUpdate"} 0.001234
dds_dcps_sample_latency_seconds_count{topic="VehicleTracking.TrackUpdate"} 5000
```

### §3.1 Bucket-Konvention

Histogramme werden in **Sekunden** exportiert (foundation-Histogramme zaehlen in Nanosekunden — der Exporter konvertiert). Prometheus-Konvention: cumulative buckets, `+Inf` als letzter Bucket.

ZeroDDS-Buckets in Sekunden:
```
1e-09, 1e-08, 1e-07, 1e-06, 1e-05, 1e-04, 1e-03, 1e-02, 1e-01, 1, 10, +Inf
```

### §3.2 Label-Escaping

Per Prometheus-Spec:
- `\\` → `\\\\`
- `"` → `\\"`
- `\n` → `\\n`

### §3.3 Render-Output

`Registry::render_prometheus(&self) -> String` liefert die volle Exposition als String, mit deterministisch sortierten Metric-Names und Labels.

## §4 W3C-Trace-Context als RTPS-Vendor-PID

### §4.1 PID-Allocation

```
PID_VENDOR_TRACE_CONTEXT = 0x0D00 (vendor-PID, OctetsToInlineQos-relevant)
```

PID 0x0D00 ist im RTPS-Vendor-PID-Range (`0x8000` bit gesetzt → vendor; aber `0x0D00` ist im Standard-PID-Range, daher als "ZeroDDS-Optional-Standard-Konformes-Vendor-Extension" deklariert; faellt unter den `IGNORE_UNKNOWN_PID`-Pfad, wenn der Receiver es nicht versteht).

> **Anmerkung:** RTPS 2.5 §9.6.3.2.4 erlaubt Vendor-spezifische PIDs. Die Wahl `0x0D00` (statt `0x80nn`) ist absichtlich, weil das Feature semantisch eine Standard-Erweiterung ist — andere Vendoren koennen denselben PID adoptieren ohne Vendor-Konflikt. Cyclone und RTI ignorieren den PID transparent.

### §4.2 Wire-Format

PID_VENDOR_TRACE_CONTEXT ist eine ParameterList-Element-Payload mit zwei CDR-Strings:

```
+---- PID_VENDOR_TRACE_CONTEXT (PID 0x0D00) Value ----+
| 0x00 | u32       | traceparent.length              |
| 0x04 | utf-8[]   | traceparent (NUL-terminated)    |
|      | u32       | tracestate.length               |
|      | utf-8[]   | tracestate (NUL-terminated)     |
+----------------------------------------------------+
```

`traceparent` folgt W3C-Trace-Context-1.0:
```
00-{trace-id-32-hex}-{parent-id-16-hex}-{flags-2-hex}
```
Beispiel: `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`.

`tracestate` folgt W3C-Trace-Context-1.0 §3.3 (vendor-spezifische Key-Value-Pairs, kommasepariert):
```
dds=topic:VehicleTracking.TrackUpdate;version:1.0
```

### §4.3 Encoding/Decoding

```rust
pub struct TraceContextPid {
    pub traceparent: TraceParent,
    pub tracestate: Option<TraceState>,
}

impl TraceContextPid {
    pub fn encode_inline_qos(&self, out: &mut Vec<u8>);
    pub fn decode_inline_qos(bytes: &[u8]) -> Result<Self, TraceContextError>;

    pub fn from_span_context(ctx: &SpanContext, vendor_state: Option<&str>) -> Self;
    pub fn to_span_context(&self) -> SpanContext;
}
```

### §4.4 Lifecycle

- **Publisher-Side**: `DataWriter::write` prueft den aktuellen `tracing::Span` (ueber Thread-Local-Slot in `monitor::current_span()`) und encodet `PID_VENDOR_TRACE_CONTEXT` als InlineQoS-Element in das ausgehende DATA-Submessage. Konfigurierbar:
  - `emit_trace_context = always | sampled | never` (default: `sampled`).

- **Subscriber-Side**: `DataReader` extrahiert `PID_VENDOR_TRACE_CONTEXT` aus inline_qos und erzeugt einen neuen `dds.sample.receive`-Span mit `parent = traceparent` (Cross-Process-Continuity). Konfigurierbar:
  - `accept_trace_context = true | false` (default: `true`).

- **Sampling-Kompatibilitaet**: ein Receiver darf Spans verwerfen, wenn sein lokales Sampler `sampled=false` zurueckgibt. ZeroDDS forciert keine Sampling-Decision, sondern respektiert die Caller-OTel-Sampler-Decision.

## §5 Span-Schema

Alle Spans nutzen `zerodds_foundation::tracing::Span`-Datentyp und folgen OpenTelemetry-Semantic-Conventions plus DDS-spezifischer `dds.`-Namespace-Erweiterung.

### §5.1 Span-Typen

| Span-Name | Kind | Pflicht-Attrs | Optional |
|---|---|---|---|
| `dds.publish` | Producer | `dds.topic`, `dds.writer_guid` | `dds.sample_size`, `dds.qos.reliability` |
| `dds.sample.serialize` | Internal | `dds.representation` | `dds.size_bytes` |
| `dds.sample.transmit` | Internal | `dds.transport`, `dds.destination` | `dds.fragments` |
| `dds.sample.receive` | Consumer | `dds.reader_guid`, `dds.source_guid`, `dds.transport` | `dds.sample_size` |
| `dds.sample.deserialize` | Internal | `dds.representation` | `dds.size_bytes` |
| `dds.sample.deliver` | Internal | `dds.topic`, `dds.reader_guid` | `dds.qos.reliability` |
| `dds.rtps.reliable.nack` | Internal | `dds.writer_guid`, `dds.reader_guid` | `dds.missing_sn_count` |
| `dds.discovery.match` | Internal | `dds.topic`, `dds.local_entity`, `dds.remote_entity` | `dds.is_compatible` |
| `dds.security.authenticate` | Internal | `dds.remote_guid` | `dds.identity_ca`, `dds.result` |

### §5.2 Span-Hierarchie

`dds.publish` ist Root oder Child eines User-Spans. `dds.sample.serialize` und `dds.sample.transmit` sind Children von `dds.publish`. `dds.sample.receive` kann ueber `traceparent`-Inline-QoS einen `follows-from`-Link zum Publisher-`dds.publish` haben (cross-process). `dds.sample.deserialize` und `dds.sample.deliver` sind Children von `dds.sample.receive`.

## §6 Lifecycle und Konfiguration

### §6.1 Default-Registry

```rust
pub fn default_registry() -> Arc<Registry>;
```

`OnceLock<Arc<Registry>>` — beim ersten Aufruf inititalisiert.

### §6.2 Konfiguration

```rust
pub struct MonitorConfig {
    pub emit_trace_context: TraceContextEmission,
    pub accept_trace_context: bool,
    pub enable_metrics: bool,
}

pub enum TraceContextEmission {
    Always,
    Sampled,   // respektiert OTel-Sampler-Decision
    Never,
}
```

### §6.3 Prometheus-Server (optional)

```rust
pub fn serve_prometheus(addr: SocketAddr, registry: Arc<Registry>) -> Result<JoinHandle<()>, ServeError>;
```

Mini-HTTP-Server (TcpListener-basiert, kein hyper-Dep). Antwortet auf `GET /metrics` mit dem Prometheus-Text-Format.

## §7 Hook-Point-Tabelle

Welche Crate emittiert welche Metric? Diese Tabelle ist normativ — jeder Hook-Point ist im RC1-Audit als Cross-Layer-Finding nachweisbar.

| Crate | Metric/Span | Wire-Up |
|---|---|---|
| `transport-udp/-tcp/-shm/-uds` | `dds_transport_packets_*`, `dds_transport_bytes_*`, `dds_transport_send_errors_total` | in `Transport::send`/`recv` |
| `rtps` | `dds_rtps_heartbeats_sent_total`, `dds_rtps_acknacks_received_total`, `dds_rtps_retransmits_total`, `dds_rtps_samples_dropped_total`, `dds_rtps_fragmented_samples_total`, `dds_rtps_unknown_submessages_total` | in `WriterCache`/`ReaderCache`/`fragment_assembler` |
| `discovery` | `dds_discovery_participants_known`, `dds_discovery_endpoints_known`, `dds_discovery_spdp_announcements_sent_total`, `dds_discovery_sedp_updates_total`, `dds_discovery_type_lookups_total` | in `SpdpStack`/`SedpStack`/`TypeLookupService` |
| `dcps` | alle 10 DCPS-Metriken + Spans `dds.publish`, `dds.sample.deliver` | in `DataWriter::write`/`DataReader::take`/`Subscriber::deliver` |
| `security`/`security-crypto` | `dds_security_*`, Span `dds.security.authenticate` | in `AuthHandshake`/`CryptoPlugin::*` |

## §8 Stabilitaet

- **Public-API (`Counter`, `Gauge`, `Histogram`, `Registry`, `Labels`)**: RC1-stabil, Major-Bump bei Breaking-Changes.
- **Metric-Namen + Label-Keys**: stabil ab RC1; Label-Keys werden nur durch additive Changes erweitert (neue Labels sind Major-kompatibel, da sie Prometheus-Selectors nicht brechen).
- **PID_VENDOR_TRACE_CONTEXT (0x0D00)** und sein Wire-Format: stabil ab RC1; Format-Aenderung waere RTPS-Wire-Breaking.
- **Span-Namen + Attr-Keys**: folgen OTel-Semantic-Conventions; Releases verfolgen die Semconv-Versionen.

## §9 Sicherheit

- **Payload-Redaction**: Counter/Gauge/Histogram speichern keine Sample-Inhalte. Trace-Spans haben optionale Attribute, aber `dds.sample_size` traegt nur Groessen, keinen Content.
- **GUID-Hashing-Hook**: Caller koennen `MonitorConfig::guid_obfuscator: Option<Box<dyn Fn(&Guid) -> String>>` setzen, um GUIDs vor dem Export zu hashen.
- **Cardinality-Bounded**: Crate-Hooks enforcen pro Metric einen `Labels`-Set, der von der Crate selbst kuratiert ist. End-User-Topic-Names werden mit einem optionalen `topic_label_filter` getruncated.

## §10 Test-Pflicht

- Counter/Gauge: atomare Inkrement-Konsistenz unter parallelen Threads.
- Histogram-Re-Export: Cross-Validation mit `foundation::tracing::Histogram` (gleiche Bucket-Werte).
- Prometheus-Text: Roundtrip durch einen Prometheus-Konformen-Parser oder Golden-Vector-Test gegen einen handvalidierten String.
- Label-Escaping: alle drei Escape-Pfade (`\\`, `"`, `\n`).
- PID 0x0D00 Encode/Decode-Roundtrip mit drei `traceparent`-Beispielen aus W3C-Trace-Context-Spec.
- Cross-Layer: `dcps::DataWriter::write` schreibt `samples_written_total++`, dann `Registry::render_prometheus` enthaelt das Increment.

## §11 Coverage-Doc

`docs/spec-coverage/zerodds-monitor-1.0.md` traegt die Sektion-Per-Sektion-Mapping mit Repo+Tests+Status. Akzeptanz: alle §-Sektionen `done`; partial/open ist RC1-Blocker.
