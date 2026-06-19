# Observability, tooling and live insights

> **Status:** Draft v0.2
> **Dependencies:** `02_architecture.md`

## 1 Philosophy

The established DDS vendors have historically treated observability as a downstream admin function. The result: every vendor has a proprietary monitoring tool that does not integrate into modern observability stacks. OpenTelemetry support arrived at RTI only in 2023, at others not at all.

Our strategy: **observability is a first-class feature, not an afterthought.** Every relevant event emits structured telemetry in open, interoperable formats. This gives operators the freedom to use their existing stacks (Grafana, Datadog, Honeycomb, Tempo, Loki, Prometheus) instead of having to learn proprietary tools.

## 2 Observability architecture

Two parallel data paths:

**Live-telemetry path:**
```
DDS node (instrumented)
    → OpenTelemetry SDK (in-process)
    → OTel Collector (sidecar or central)
    → backends: Prometheus (metrics), Tempo/Jaeger (traces), Loki (logs)
    → Grafana + alerts + own Tauri dashboard
```

**Wire-capture path:**
```
DDS node (wire probe)
    → wire recorder (filter-based, deterministic)
    → sample archive (object-storage-compatible)
    → replay engine
    → replay target or analysis UI
```

Both paths converge in the operator UI, which supports both live monitoring and historical replay.

## 3 Metric catalog

The `zerodds-monitor` crate exports metrics in the Prometheus text format and as OTLP. The following list defines the core metrics; each metric follows OpenMetrics conventions.

### 3.1 Transport level

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `dds_transport_packets_sent_total` | Counter | `transport`, `domain_id`, `local_guid` | RTPS packets sent |
| `dds_transport_packets_received_total` | Counter | `transport`, `domain_id`, `local_guid` | RTPS packets received |
| `dds_transport_bytes_sent_total` | Counter | same | Bytes sent |
| `dds_transport_bytes_received_total` | Counter | same | Bytes received |
| `dds_transport_send_errors_total` | Counter | `transport`, `error_kind` | Send errors (e.g., EWOULDBLOCK, ENETUNREACH) |
| `dds_transport_socket_buffer_bytes` | Gauge | `transport`, `direction` | Current socket-buffer utilization |

### 3.2 RTPS protocol level

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `dds_rtps_heartbeats_sent_total` | Counter | `writer_guid` | Heartbeats sent |
| `dds_rtps_acknacks_received_total` | Counter | `writer_guid`, `reader_guid` | Acknacks received |
| `dds_rtps_retransmits_total` | Counter | `writer_guid`, `reader_guid` | Retransmissions |
| `dds_rtps_samples_dropped_total` | Counter | `writer_guid`, `reason` | Samples dropped (e.g. history limit) |
| `dds_rtps_fragmented_samples_total` | Counter | `writer_guid` | Fragmented samples |
| `dds_rtps_unknown_submessages_total` | Counter | `vendor_id` | Unknown submessage kinds (interop indicator) |

### 3.3 DCPS level

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `dds_dcps_samples_written_total` | Counter | `topic`, `writer_guid` | Samples written |
| `dds_dcps_samples_read_total` | Counter | `topic`, `reader_guid` | Samples read |
| `dds_dcps_samples_lost_total` | Counter | `topic`, `reader_guid` | Unreceivable samples (SAMPLE_LOST status) |
| `dds_dcps_deadline_missed_total` | Counter | `topic`, `entity_guid` | Deadline misses |
| `dds_dcps_liveliness_lost_total` | Counter | `topic`, `writer_guid` | Liveliness-lost events |
| `dds_dcps_subscription_matched_total` | Counter | `topic`, `reader_guid` | New matches |
| `dds_dcps_subscription_unmatched_total` | Counter | same | Lost matches |
| `dds_dcps_incompatible_qos_total` | Counter | `topic`, `entity_guid`, `policy_id` | QoS incompatibilities |
| `dds_dcps_sample_latency_seconds` | Histogram | `topic` | End-to-end latency (wall-clock) |
| `dds_dcps_sample_size_bytes` | Histogram | `topic` | Sample sizes |

### 3.4 Discovery level

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `dds_discovery_participants_known` | Gauge | `domain_id` | Known participants |
| `dds_discovery_endpoints_known` | Gauge | `domain_id`, `kind` | Known endpoints (writer/reader) |
| `dds_discovery_spdp_announcements_sent_total` | Counter | `domain_id` | SPDP announcements |
| `dds_discovery_sedp_updates_total` | Counter | `domain_id`, `kind` | SEDP updates |
| `dds_discovery_type_lookups_total` | Counter | `domain_id` | TypeLookup requests |

### 3.5 Security level

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `dds_security_auth_attempts_total` | Counter | `result` | Authentication attempts (success/failure) |
| `dds_security_access_denied_total` | Counter | `operation`, `topic` | Access-control denials |
| `dds_security_crypto_operations_total` | Counter | `operation` (encrypt/decrypt/sign/verify) | Crypto operations |
| `dds_security_crypto_latency_seconds` | Histogram | `operation` | Crypto latency |

## 4 Tracing schema

OpenTelemetry spans are emitted with consistent attributes. We follow the Semantic Conventions where possible and add DDS-specific attributes under the `dds.` namespace.

### 4.1 Important span types

| Span | Attribute core | Parent |
|---|---|---|
| `dds.publish` | `dds.topic`, `dds.writer_guid`, `dds.sample_size` | Client code |
| `dds.sample.serialize` | `dds.representation`, `dds.size_bytes` | `dds.publish` |
| `dds.sample.transmit` | `dds.transport`, `dds.destination`, `dds.fragments` | `dds.publish` |
| `dds.sample.receive` | `dds.reader_guid`, `dds.source_guid`, `dds.transport` | (root or parent via W3C Trace Context) |
| `dds.sample.deserialize` | `dds.representation` | `dds.sample.receive` |
| `dds.sample.deliver` | `dds.topic`, `dds.reader_guid` | `dds.sample.receive` |
| `dds.rtps.reliable.nack` | `dds.writer_guid`, `dds.reader_guid`, `dds.missing_sn_count` | Timer-based or incoming HB |
| `dds.discovery.match` | `dds.topic`, `dds.local_entity`, `dds.remote_entity` | SEDP event |
| `dds.security.authenticate` | `dds.remote_guid`, `dds.identity_ca`, `dds.result` | SPDP event |

### 4.2 W3C Trace Context in the wire format

An optional RTPS ParameterList element transports the W3C Trace Context between nodes:

```
PID_VENDOR_TRACE_CONTEXT (PID 0x0D00, vendor-specific)
    traceparent: 00-{trace-id-32-hex}-{parent-id-16-hex}-{flags}
    tracestate: dds=...
```

When an active span exists on the publisher side, `traceparent` is sent along with published samples. On the receive side, a `dds.sample.receive` span with a `follows-from` relationship to the publisher span is created from it. This yields end-to-end tracing across the distributed system.

Behavior configurable:
- `emit_trace_context = always | sampled | never` (default: `sampled`, respects the OTel sampling decision)
- Receive side: `accept_trace_context = true | false` (default: `true`)

## 5 Structured logging

All internal logs use the `tracing` crate (not `log`) with structured fields. Log levels are used conservatively:

| Level | Use |
|---|---|
| `ERROR` | Unrecoverable errors that require manual intervention |
| `WARN` | Degraded operation (e.g. high retransmit rate, QoS mismatch) |
| `INFO` | Lifecycle events (participant start, security auth success, match) |
| `DEBUG` | Detailed flow for development, not for production |
| `TRACE` | Wire-level details |

Log export targets:
- stdout/stderr with `tracing-subscriber` (default)
- OTLP as OpenTelemetry logs
- JSON for log shipping (Loki, Elasticsearch)

## 6 Wire recorder

### 6.1 Requirements

- **Deterministic replay:** a recorded stream must be bit-exactly reproducible, including timing.
- **Predicate-based filtering:** recording selective, to control storage cost.
- **Tamper evidence:** recorded streams are signature-verified.
- **Compact binary format:** own format for efficiency, with converters to pcap and MCAP.
- **Index support:** fast random access to sample timestamps.

### 6.2 Recording format

A container file is produced per recording session:

```
Header:
    Magic: "DDSR"
    Format version
    Recording metadata (start timestamp, hostname, domain ID, recorder version)
    Signer public key

Index:
    Time index: timestamp → file offset
    Topic index: topic name → sequence of offsets
    Entity index: GUID → sequence of offsets

Frames (one sequence):
    Frame header:
        Timestamp (monotonic ns)
        Wall-clock timestamp (ns since Unix epoch)
        Reception-order marker
        NTP-offset snapshot
        Source transport (UDP/TCP/SHM)
        Source locator
        Destination locator
        Length
    Frame body:
        Raw RTPS message bytes

Footer:
    Full-session hash (SHA-256)
    Signature (Ed25519)
    Frame count
    End timestamp
```

### 6.3 Predicate language

Recording filters are expressed in a simple expression language:

```
topic == "VehicleTracking.TrackUpdate"
topic =~ /^Weapon\./
domain_id == 7
writer.guid == "01.0F.00.00..."
qos.reliable == true
message_size > 1024
vendor_id == 0x010F  # RTI Connext
```

Filters run in-process at the probe point to minimize recording overhead.

### 6.4 Replay engine

The replay engine replicates the recorded stream in various modes:

- **Bit-exact replay:** reconstructs the original timing and wire bytes. For regression tests.
- **Time-scaled replay:** 2×, 10×, 0.1× speed. For debug sessions.
- **Filtered replay:** replay only selected topics/entities. For focused analysis.
- **Modified replay:** modify wire bytes on the fly (for fault injection and fuzz testing).

Replay targets: real network (similar to `tcpreplay`), simulated null network, or directly into the analysis UI.

## 7 Operator UI: Tauri-based dashboard

The `zerodds-dashboard` binary is a Tauri desktop app. Rationale for Tauri instead of Electron:
- Smaller binaries (~10 MB vs ~150 MB)
- Native Rust integration directly with our stack
- Offline-capable, no cloud mandate
- Cross-platform (Linux, Windows, macOS)

### 7.1 Feature set

**Live view:**
- Discovery graph: topological visualization of all known participants, endpoints, matches
- Per-topic heatmap: sample rate, latency percentiles, drop rate
- QoS-mismatch alerts: incompatible publisher/subscriber pairs visible
- Live log stream with filter and search

**Historical view:**
- Replay browser: open recordings, scrub the timeline, inspect individual frames
- Wire decoder: visualize RTPS structure (submessages, submessage elements, payload)
- Sample inspector: present deserialized sample content in a structured way (XTypes-aware)

**Security view:**
- Certificate-chain inspector
- Permissions-document browser
- Crypto-operations timeline

**Performance view:**
- Throughput graphs per topic/endpoint
- Latency-percentile charts
- Resource usage of the DDS runtime (CPU, memory, socket buffer)

### 7.2 Data sources

The dashboard connects to three data sources:

1. **Direct DDS connect:** the dashboard itself is a DDS participant, can consume built-in topics and custom topics.
2. **OTel endpoint:** fetches metrics and traces via OTLP from backends (Prometheus/Tempo).
3. **Recording files:** opens local or object-store-hosted recordings.

## 8 Performance tooling

The `zerodds-perf` binary provides load and measurement tools.

### 8.1 Load generators

- `zerodds-perf publish`: configurable publisher with target rate, sample size, QoS
- `zerodds-perf subscribe`: subscriber with latency measurement and drop detection
- `zerodds-perf roundtrip`: request/reply tester with statistics

Configuration via CLI and config files:

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

### 8.2 Benchmark suite

Criterion.rs-based benchmarks for all hot-path functions. The baseline is archived on every release, regressions cause CI failures when >5%.

Benchmark categories:
- CDR encode/decode per type complexity
- RTPS submessage parse
- Discovery-match computation
- QoS-compatibility check
- End-to-end latency in a controlled environment

### 8.3 Flamegraph support

Integration with `cargo-flamegraph` and `perf` for production profiling. Dashboards can capture flamegraph snapshots from running systems.

## 9 Admin CLI

The `zerodds-admin` binary offers command-line tools for operators. It has two
families: the *live* groups join a running DDS domain over RTPS, the *offline*
groups load a DDS-XML deployment and analyze it without touching the network.
Every data-bearing command accepts `--json`.

```
# Live (joins the domain over RTPS)
zerodds-admin domain inspect 0            # participants + their writer/reader endpoints
zerodds-admin discovery snapshot 0        # raw SPDP/SEDP snapshot + pub/sub counts

# Offline (DDS-XML, no network)
zerodds-admin config inspect deploy.xml   # domain-id-centric topology of a deployment
zerodds-admin qos validate deploy.xml     # DDS-XML well-formedness + library parse
zerodds-admin qos check deploy.xml        # writer/reader RxO QoS compatibility (DDS 1.4 §2.2.3)
```

Recording and replay live in the dedicated `zerodds-record` / `zerodds-replay`
binaries; static DDS-XML validation and rendering is also available standalone
via `zerodds-xmlc`.

## 10 Integration with standard stacks

Out-of-the-box integration with common observability stacks:

- **Grafana dashboards:** prebuilt JSON dashboards in the release artifact, importable via the Grafana UI
- **Prometheus alerts:** prebuilt alerting rules for typical problems (discovery loss, high retransmit rate, deadline violations)
- **OpenTelemetry Collector config:** example config for a sidecar deployment next to DDS processes
- **Loki labels:** structured log labels consistent with metric labels

## 11 Privacy and retention

Observability data can be sensitive. Out-of-the-box safeguards:

- **Payload redaction:** the wire recorder can pseudonymize sample payloads (SHA-256 instead of raw data).
- **GUID hashing:** when privacy requirements exist, GUIDs can be hashed before they are exported into metrics/traces.
- **Retention policies:** metrics and traces have configurable retention. Defaults: 30 days metrics, 7 days traces, 90 days logs.
- **Recording encryption:** recording files can be encrypted at rest (AES-256-GCM with an operator-managed key).
