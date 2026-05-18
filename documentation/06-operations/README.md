# 06 – Operations

Running ZeroDDS in production. This station is a pointer-collection
to the operations material that already exists.

## Sub-stations

- [Deployment](deployment.md) — packaging, systemd, container
  layouts, multi-host topology.
- [Monitoring](monitoring.md) — what to watch, where the metrics
  come from, how to alert.
- [Troubleshooting](troubleshooting.md) — common symptoms, root
  causes, diagnostic commands.

## Cross-references

| Topic | Authoritative document |
|---|---|
| Native packages (.deb / .rpm / .msi / .pkg) | [`../../docs/PACKAGING.md`](../../docs/PACKAGING.md) |
| Real-time tuning (isolcpus, preempt_rt) | [`../../docs/REALTIME_DEPLOYMENT.md`](../../docs/REALTIME_DEPLOYMENT.md) |
| Multi-host interop tests | [`../../docs/interop/`](../../docs/interop/) |
| CI matrix + Apex.AI plugin | [`../../docs/ci/`](../../docs/ci/) |
| Performance baselines | [`../../docs/perf/`](../../docs/perf/) |
| QoS reference | [`../03-configuration/qos-policies.md`](../03-configuration/qos-policies.md) |
| Security configuration | [`../03-configuration/security.md`](../03-configuration/security.md) |
| Observability | [`../03-configuration/observability.md`](../03-configuration/observability.md) |

## What you watch in production

| Signal | Source | Alert if |
|---|---|---|
| `dds.history_cache.evicted` | `HistoryCacheStats` | > 0 on a Reliable+KeepAll writer (data loss) |
| `dds.discovery.peer_count` | SPDP cache | drops by > 50% in 30 s (network partition) |
| `dds.heartbeat.unanswered` | `ReliableWriter::unknown_src_count` | > 5 (stale proxies) |
| `dds.deadline.missed` | `offered_deadline_missed_count` | > 0 (writer too slow) |
| `dds.liveliness.lost` | `liveliness_lost_count` | > 0 (writer-side detection) |
| `dds.security.policy_violations` | LoggingPlugin | any (SOC investigation) |

Every metric above is reachable lock-free via the atomic stats
plus the observability sink — no impact on the hot path.

## Operational discipline

- Run `zerodds-perf hw-info` on every host at deploy time, log the
  output. It tells you whether you got the AES-NI / ARMv8-AES
  acceleration you expected.
- Snapshot `zerodds-admin --topics --participants` periodically
  (every minute) to a structured log — that's your discovery
  health-check.
- For RT deployments, run `cyclictest` in parallel with the load
  for at least 24 h before declaring the deployment certified.
  See [`../../docs/REALTIME_DEPLOYMENT.md`](../../docs/REALTIME_DEPLOYMENT.md) §7.

## Future operations work

| Topic | Status | Trail station |
|---|---|---|
| OTel exporter (OTLP-HTTP) | in progress | this station's [monitoring.md](monitoring.md) |
| Tauri-Dashboard | in progress | future |
| Live interop matrix | in progress | future |
