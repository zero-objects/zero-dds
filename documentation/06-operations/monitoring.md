# Monitoring

What to watch, where the data is, and how to alert.

## What to watch

| Signal | Source | Healthy | Alert |
|---|---|---|---|
| Discovery: peer count | `DcpsRuntime::discovered_participants_len()` | stable, matches expected fleet size | drops > 50 % within 30 s |
| Discovery: SPDP latency | event-time on `peer_discovered` events | < 6 s on first seen | > 30 s — multicast filtered? |
| Reliable writer: cache fill | `cache.stats().len` | small fraction of `max_samples` | rising trend with no eviction = readers lagging |
| Reliable writer: evicted count | `cache.stats().evicted` | 0 on `KeepAll`, growing on `KeepLast` | non-zero on `KeepAll` = data loss |
| Reliable writer: unknown ACKNACK source | `ReliableWriter::unknown_src_count` | 0 | > 5 = stale proxies / GUID spoofing |
| Deadline-missed | `user_writer_offered_deadline_missed(eid)` | 0 | > 0 — writer too slow |
| Liveliness lost | `user_writer_liveliness_lost(eid)` | 0 | > 0 |
| Heartbeat-RTT | (planned) per-proxy histogram | < deadline budget | tail > deadline budget |

## How to read it — three layers

### 1. Lock-free atomic poll

Cheapest path. A monitoring thread holds an
`Arc<HistoryCacheStats>` and reads atomics.

```rust
let stats = some_writer_cache.stats();
loop {
    let snap = stats.snapshot();
    metrics::gauge!("dds.cache.len", snap.len as f64);
    metrics::gauge!("dds.cache.evicted", snap.evicted as f64);
    if let Some(max) = snap.max_sn { /* … */ }
    std::thread::sleep(std::time::Duration::from_secs(1));
}
```

No impact on the writer hot path — atomics with `Acquire`-load.

### 2. Sink-based events

Lifecycle events emitted by `DcpsRuntime`. Inject
`StderrJsonSink::new()` and your log shipper picks them up.

```json
{"level":"info","component":"dcps","name":"user_writer.created","attrs":{"topic":"Telemetry","type":"Robot::Pose","reliable":"true"}}
```

Datadog / Loki / journald-friendly out of the box.

### 3. OTel exporter

`crates/observability-otlp` (work-in-progress) ships events to
an OTel collector via OTLP-HTTP. From there, fan out to:

- Tempo / Jaeger (traces)
- Mimir / Prometheus (metrics)
- Loki (logs)
- Datadog / Grafana Cloud / etc

## Alerting recipes

### Reliable-cache exhaustion (data loss)

```yaml
# Prometheus alerting rule
- alert: DdsReliableCacheLoss
  expr: rate(dds_history_cache_evicted{kind="KeepAll"}[5m]) > 0
  for: 1m
  annotations:
    summary: "Reliable Writer is dropping samples"
    description: "{{ $labels.topic }} on host {{ $labels.host }} is evicting from a KeepAll cache — readers are not keeping up."
```

### Discovery partition

```yaml
- alert: DdsDiscoveryPartition
  expr: dds_discovery_peers < 0.5 * dds_discovery_peers_baseline
  for: 30s
  annotations:
    summary: "DDS discovery has lost > 50% of peers"
    runbook: "Check IGMP-snooping, switch multicast forwarding, NIC binding."
```

### Latency tail spike

```yaml
- alert: DdsLatencyP99
  expr: histogram_quantile(0.99, dds_roundtrip_latency_seconds_bucket) > 0.001
  for: 5m
  annotations:
    summary: "DDS p99 latency exceeded 1ms budget"
```

(Requires the OTel-bridge or a custom metrics-Sink emitting a
histogram.)

## Dashboards

| Dashboard | Source |
|---|---|
| Live-DDS-Health | `tools/dashboard/` (Tauri) — connects to a DcpsRuntime via the built-in topic API and renders Pub/Sub graph + per-endpoint metrics |
| Grafana template | Planned. Will be exported via `documentation/grafana/zerodds-overview.json` |
| Cyclic-test plot | Generic `cyclictest --histogram` output → `cyclictest_plot` per OSADL |

## What you do *not* monitor with DDS itself

Use generic system tooling for:

- CPU usage — `top`, `pidstat`, `eBPF`
- Memory leaks — `valgrind`, `heaptrack`, `pidstat -r`
- Network saturation — `iftop`, `nload`, `bpftrace` (`tc-bpf` for
  per-flow statistics)
- Kernel-level RT spikes — `ftrace`, `perf sched`,
  `cyclictest --histogram`

ZeroDDS is "just another userspace process" from the OS's point
of view; standard Linux/Windows tooling gives you everything you
need below the DDS layer.
