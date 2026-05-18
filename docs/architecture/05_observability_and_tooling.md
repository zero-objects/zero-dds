# Observability, Tooling und Live-Insights

> **Status:** Draft v0.2
> **Abhängigkeiten:** `02_architecture.md`

## 1 Philosophie

Die etablierten DDS-Anbieter haben Observability historisch als nachgelagerte Admin-Funktion behandelt. Das Ergebnis: jeder Vendor hat ein proprietäres Monitoring-Tool, das nicht in moderne Observability-Stacks integriert. OpenTelemetry-Unterstützung kam bei RTI erst 2023, bei anderen gar nicht.

Unsere Strategie: **Observability ist ein First-Class-Feature, nicht ein Nachgedanke.** Jedes relevante Ereignis emittiert strukturierte Telemetrie in offenen, interoperablen Formaten. Das gibt Betreibern die Freiheit, ihre bestehenden Stacks (Grafana, Datadog, Honeycomb, Tempo, Loki, Prometheus) zu verwenden, statt proprietäre Tools lernen zu müssen.

## 2 Observability-Architektur

Zwei parallele Datenpfade:

**Live-Telemetrie-Pfad:**
```
DDS-Node (instrumentiert)
    → OpenTelemetry SDK (in-process)
    → OTel Collector (sidecar oder zentral)
    → Backends: Prometheus (metrics), Tempo/Jaeger (traces), Loki (logs)
    → Grafana + Alerts + eigener Tauri-Dashboard
```

**Wire-Capture-Pfad:**
```
DDS-Node (Wire-Probe)
    → Wire-Recorder (filter-based, deterministic)
    → Sample-Archive (Object-Storage-kompatibel)
    → Replay-Engine
    → Replay-Target oder Analysis-UI
```

Beide Pfade konvergieren im Operator-UI, das sowohl Live-Monitoring als auch historisches Replay unterstützt.

## 3 Metrik-Katalog

Das `zerodds-monitor` Crate exportiert Metriken im Prometheus-Textformat und als OTLP. Die folgende Liste definiert die Kernmetriken; jede Metrik folgt OpenMetrics-Konventionen.

### 3.1 Transport-Ebene

| Metrik | Typ | Labels | Bedeutung |
|---|---|---|---|
| `dds_transport_packets_sent_total` | Counter | `transport`, `domain_id`, `local_guid` | Versandte RTPS-Pakete |
| `dds_transport_packets_received_total` | Counter | `transport`, `domain_id`, `local_guid` | Empfangene RTPS-Pakete |
| `dds_transport_bytes_sent_total` | Counter | dto. | Gesendete Bytes |
| `dds_transport_bytes_received_total` | Counter | dto. | Empfangene Bytes |
| `dds_transport_send_errors_total` | Counter | `transport`, `error_kind` | Send-Fehler (E.g., EWOULDBLOCK, ENETUNREACH) |
| `dds_transport_socket_buffer_bytes` | Gauge | `transport`, `direction` | Aktuelle Socket-Buffer-Auslastung |

### 3.2 RTPS-Protokoll-Ebene

| Metrik | Typ | Labels | Bedeutung |
|---|---|---|---|
| `dds_rtps_heartbeats_sent_total` | Counter | `writer_guid` | Gesendete Heartbeats |
| `dds_rtps_acknacks_received_total` | Counter | `writer_guid`, `reader_guid` | Empfangene Acknacks |
| `dds_rtps_retransmits_total` | Counter | `writer_guid`, `reader_guid` | Retransmissions |
| `dds_rtps_samples_dropped_total` | Counter | `writer_guid`, `reason` | Samples gedropped (z.B. History-Limit) |
| `dds_rtps_fragmented_samples_total` | Counter | `writer_guid` | Fragmentierte Samples |
| `dds_rtps_unknown_submessages_total` | Counter | `vendor_id` | Unbekannte Submessage-Kinds (Interop-Indikator) |

### 3.3 DCPS-Ebene

| Metrik | Typ | Labels | Bedeutung |
|---|---|---|---|
| `dds_dcps_samples_written_total` | Counter | `topic`, `writer_guid` | Geschriebene Samples |
| `dds_dcps_samples_read_total` | Counter | `topic`, `reader_guid` | Gelesene Samples |
| `dds_dcps_samples_lost_total` | Counter | `topic`, `reader_guid` | Nicht-empfangbare Samples (SAMPLE_LOST Status) |
| `dds_dcps_deadline_missed_total` | Counter | `topic`, `entity_guid` | Deadline-Misses |
| `dds_dcps_liveliness_lost_total` | Counter | `topic`, `writer_guid` | Liveliness-Lost-Events |
| `dds_dcps_subscription_matched_total` | Counter | `topic`, `reader_guid` | Neue Matches |
| `dds_dcps_subscription_unmatched_total` | Counter | dto. | Verlorene Matches |
| `dds_dcps_incompatible_qos_total` | Counter | `topic`, `entity_guid`, `policy_id` | QoS-Inkompatibilitäten |
| `dds_dcps_sample_latency_seconds` | Histogram | `topic` | End-to-End-Latency (wall-clock) |
| `dds_dcps_sample_size_bytes` | Histogram | `topic` | Sample-Größen |

### 3.4 Discovery-Ebene

| Metrik | Typ | Labels | Bedeutung |
|---|---|---|---|
| `dds_discovery_participants_known` | Gauge | `domain_id` | Bekannte Participants |
| `dds_discovery_endpoints_known` | Gauge | `domain_id`, `kind` | Bekannte Endpoints (Writer/Reader) |
| `dds_discovery_spdp_announcements_sent_total` | Counter | `domain_id` | SPDP-Announcements |
| `dds_discovery_sedp_updates_total` | Counter | `domain_id`, `kind` | SEDP-Updates |
| `dds_discovery_type_lookups_total` | Counter | `domain_id` | TypeLookup-Requests |

### 3.5 Security-Ebene

| Metrik | Typ | Labels | Bedeutung |
|---|---|---|---|
| `dds_security_auth_attempts_total` | Counter | `result` | Authentication-Versuche (success/failure) |
| `dds_security_access_denied_total` | Counter | `operation`, `topic` | Access-Control-Denials |
| `dds_security_crypto_operations_total` | Counter | `operation` (encrypt/decrypt/sign/verify) | Crypto-Operationen |
| `dds_security_crypto_latency_seconds` | Histogram | `operation` | Crypto-Latenz |

## 4 Tracing-Schema

OpenTelemetry-Spans werden mit konsistenten Attributen emittiert. Wir folgen den Semantic Conventions wo möglich und ergänzen DDS-spezifische Attribute unter dem `dds.`-Namespace.

### 4.1 Wichtige Span-Typen

| Span | Attribut-Kern | Parent |
|---|---|---|
| `dds.publish` | `dds.topic`, `dds.writer_guid`, `dds.sample_size` | Client-Code |
| `dds.sample.serialize` | `dds.representation`, `dds.size_bytes` | `dds.publish` |
| `dds.sample.transmit` | `dds.transport`, `dds.destination`, `dds.fragments` | `dds.publish` |
| `dds.sample.receive` | `dds.reader_guid`, `dds.source_guid`, `dds.transport` | (root oder parent via W3C Trace Context) |
| `dds.sample.deserialize` | `dds.representation` | `dds.sample.receive` |
| `dds.sample.deliver` | `dds.topic`, `dds.reader_guid` | `dds.sample.receive` |
| `dds.rtps.reliable.nack` | `dds.writer_guid`, `dds.reader_guid`, `dds.missing_sn_count` | Timer-basiert oder eingehender HB |
| `dds.discovery.match` | `dds.topic`, `dds.local_entity`, `dds.remote_entity` | SEDP-Event |
| `dds.security.authenticate` | `dds.remote_guid`, `dds.identity_ca`, `dds.result` | SPDP-Event |

### 4.2 W3C Trace Context im Wire-Format

Ein optionales RTPS-Parameter-List-Element transportiert den W3C Trace Context zwischen Nodes:

```
PID_VENDOR_TRACE_CONTEXT (PID 0x0D00, Vendor-spezifisch)
    traceparent: 00-{trace-id-32-hex}-{parent-id-16-hex}-{flags}
    tracestate: dds=...
```

Wenn auf Publisher-Seite ein aktiver Span existiert, wird `traceparent` mit gepublishten Samples mitgesendet. Empfangsseitig wird daraus ein `dds.sample.receive`-Span mit `follows-from`-Relationship zum Publisher-Span erstellt. Damit ergibt sich End-to-End-Tracing über das verteilte System.

Verhalten konfigurierbar:
- `emit_trace_context = always | sampled | never` (default: `sampled`, respektiert OTel Sampling-Decision)
- Receive-Side: `accept_trace_context = true | false` (default: `true`)

## 5 Structured Logging

Alle internen Logs nutzen das `tracing`-Crate (nicht `log`) mit strukturierten Feldern. Log-Level werden konservativ verwendet:

| Level | Verwendung |
|---|---|
| `ERROR` | Unrecoverable Errors, die manuelle Intervention benötigen |
| `WARN` | Degradierte Operation (z.B. Retransmits-Rate hoch, QoS-Mismatch) |
| `INFO` | Lifecycle-Events (Participant-Start, Security-Auth-Success, Match) |
| `DEBUG` | Detail-Flow für Development, nicht für Production |
| `TRACE` | Wire-Level-Details |

Log-Export-Ziele:
- stdout/stderr mit `tracing-subscriber` (Default)
- OTLP als OpenTelemetry-Logs
- JSON für Log-Shipping (Loki, Elasticsearch)

## 6 Wire-Recorder

### 6.1 Anforderungen

- **Deterministic Replay:** ein aufgezeichneter Stream muss bit-genau reproduzierbar sein, einschließlich Timing.
- **Predicate-basiertes Filtering:** Aufzeichnung selektiv, um Storage-Kosten zu kontrollieren.
- **Tamper-Evidence:** aufgezeichnete Streams sind Signatur-verifiziert.
- **Compact Binary Format:** eigenes Format für Effizienz, mit Konvertern zu pcap und MCAP.
- **Index-Support:** schnelle Random-Access zu Sample-Zeitpunkten.

### 6.2 Recording-Format

Pro Recording-Session wird ein Container-File erzeugt:

```
Header:
    Magic: "DDSR"
    Format-Version
    Recording-Metadata (Start-Timestamp, Hostname, Domain-ID, Recorder-Version)
    Signer-Public-Key

Index:
    Zeit-Index: Timestamp → File-Offset
    Topic-Index: Topic-Name → Sequenz von Offsets
    Entity-Index: GUID → Sequenz von Offsets

Frames (eine Sequenz):
    Frame-Header:
        Timestamp (monotonic ns)
        Wall-Clock-Timestamp (ns since Unix epoch)
        Reception-Order-Marker
        NTP-Offset-Snapshot
        Source-Transport (UDP/TCP/SHM)
        Source-Locator
        Destination-Locator
        Length
    Frame-Body:
        Raw RTPS-Message bytes

Footer:
    Full-Session Hash (SHA-256)
    Signature (Ed25519)
    Frame-Count
    End-Timestamp
```

### 6.3 Predicate-Language

Recording-Filter werden in einer einfachen Expression-Language ausgedrückt:

```
topic == "VehicleTracking.TrackUpdate"
topic =~ /^Weapon\./
domain_id == 7
writer.guid == "01.0F.00.00..."
qos.reliable == true
message_size > 1024
vendor_id == 0x010F  # RTI Connext
```

Filter laufen in-process am Probe-Punkt, um Recording-Overhead zu minimieren.

### 6.4 Replay-Engine

Die Replay-Engine repliziert den aufgezeichneten Stream in verschiedenen Modi:

- **Bit-exact replay:** rekonstruiert Original-Timing und Wire-Bytes. Für Regression-Tests.
- **Time-scaled replay:** 2×, 10×, 0.1× Geschwindigkeit. Für Debug-Sessions.
- **Filtered replay:** nur ausgewählte Topics/Entities replayen. Für fokussierte Analyse.
- **Modified replay:** Wire-Bytes on-the-fly modifizieren (für Fault-Injection und Fuzz-Testing).

Replay-Ziele: echtes Netzwerk (ähnlich `tcpreplay`), simulierter Null-Network, oder direkt in Analyse-UI.

## 7 Operator-UI: Tauri-basiertes Dashboard

Das `zerodds-dashboard`-Binary ist eine Tauri-Desktop-App. Begründung für Tauri statt Electron:
- Kleinere Binaries (~10 MB vs ~150 MB)
- Native Rust-Integration direkt mit unserem Stack
- Offline-fähig, kein Cloud-Zwang
- Cross-Platform (Linux, Windows, macOS)

### 7.1 Feature-Set

**Live-View:**
- Discovery-Graph: topologische Visualisierung aller bekannten Participants, Endpoints, Matches
- Per-Topic-Heatmap: Sample-Rate, Latency-Percentiles, Drop-Rate
- QoS-Mismatch-Alerts: inkompatible Publisher/Subscriber-Paare sichtbar
- Live-Log-Stream mit Filter- und Suchfunktion

**Historical-View:**
- Replay-Browser: Recordings öffnen, timeline-scrubben, einzelne Frames inspizieren
- Wire-Decoder: RTPS-Struktur visualisieren (Submessages, Submessage-Elements, Payload)
- Sample-Inspector: deserialisierter Sample-Inhalt strukturiert darstellen (XTypes-aware)

**Security-View:**
- Zertifikats-Chain-Inspector
- Permissions-Dokument-Browser
- Crypto-Operationen-Timeline

**Performance-View:**
- Throughput-Graphen pro Topic/Endpoint
- Latency-Percentile-Charts
- Resource-Usage des DDS-Runtime (CPU, Memory, Socket-Buffer)

### 7.2 Datenquellen

Das Dashboard verbindet sich mit drei Datenquellen:

1. **Direkter DDS-Connect:** Dashboard selbst ist ein DDS-Participant, kann Built-in-Topics und Custom-Topics konsumieren.
2. **OTel-Endpoint:** holt Metriken und Traces via OTLP aus Backends (Prometheus/Tempo).
3. **Recording-Files:** öffnet lokale oder Object-Store-gehostete Recordings.

## 8 Performance-Tooling

Das `zerodds-perf`-Binary stellt Lasten-und Messwerkzeuge bereit.

### 8.1 Load-Generatoren

- `zerodds-perf publish`: konfigurable Publisher mit Ziel-Rate, Sample-Größe, QoS
- `zerodds-perf subscribe`: Subscriber mit Latenz-Messung und Drop-Detection
- `zerodds-perf roundtrip`: Request/Reply-Tester mit Statistik

Konfiguration über CLI und Config-Files:

```yaml
scenario: throughput_stress
participants:
  - role: publisher
    count: 4
    topic: LoadTest
    rate_hz: 1000
    payload_bytes: 256
    qos:
      reliability: RELIABLE
      history: KEEP_LAST
      depth: 10
  - role: subscriber
    count: 16
    topic: LoadTest
    qos:
      reliability: RELIABLE
duration_seconds: 60
metrics_output: results.prom
```

### 8.2 Benchmark-Suite

Criterion.rs-basierte Benchmarks für alle Hot-Path-Funktionen. Baseline wird bei jedem Release archiviert, Regressionen führen zu CI-Failures wenn >5%.

Benchmark-Kategorien:
- CDR Encode/Decode pro Typ-Komplexität
- RTPS Submessage Parse
- Discovery-Match-Computation
- QoS-Compatibility-Check
- End-to-End-Latenz in kontrollierter Umgebung

### 8.3 Flamegraph-Support

Integration mit `cargo-flamegraph` und `perf` für Production-Profiling. Dashboards können Flamegraph-Snapshots aus laufenden Systemen aufnehmen.

## 9 Admin-CLI

Das `zerodds-admin`-Binary bietet Command-Line-Tools für Operators:

```
zerodds-admin participants list [--domain 0]
zerodds-admin topics list --domain 0
zerodds-admin qos validate --file my-profiles.xml
zerodds-admin discovery graph --format dot --output graph.dot
zerodds-admin security verify-cert --cert node.pem --ca ca.pem
zerodds-admin recording start --filter 'topic =~ /^Critical\./' --output session.ddsr
zerodds-admin recording inspect session.ddsr
zerodds-admin recording replay session.ddsr --rate 2.0
```

## 10 Integration mit Standard-Stacks

Out-of-the-Box-Integration mit gängigen Observability-Stacks:

- **Grafana Dashboards:** vorgefertigte JSON-Dashboards im Release-Artefakt, importierbar via Grafana UI
- **Prometheus Alerts:** vorgefertigte Alerting-Rules für typische Probleme (Discovery-Loss, High-Retransmit-Rate, Deadline-Violations)
- **OpenTelemetry Collector Config:** Beispiel-Config für Sidecar-Deployment neben DDS-Prozessen
- **Loki Labels:** strukturierte Log-Labels konsistent zu Metrik-Labels

## 11 Datenschutz und Retention

Observability-Daten können sensibel sein. Out-of-the-Box-Safeguards:

- **Payload-Redaction:** Wire-Recorder kann Sample-Payloads pseudonymisieren (SHA-256 statt Rohdaten).
- **GUID-Hashing:** wenn Privacy-Anforderungen bestehen, können GUIDs gehasht werden bevor sie in Metriken/Traces exportiert werden.
- **Retention-Policies:** Metriken und Traces haben konfigurierbare Retention. Defaults: 30 Tage Metriken, 7 Tage Traces, 90 Tage Logs.
- **Recording-Encryption:** Recording-Files können At-Rest verschlüsselt werden (AES-256-GCM mit vom Operator verwaltetem Key).
