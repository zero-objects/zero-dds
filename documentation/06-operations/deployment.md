# Deployment

How to ship ZeroDDS to a fleet.

## Single-host

```bash
# Linux (after `apt install zerodds-tools`):
systemctl --user start your-zerodds-app.service
```

systemd unit (user or system):

```ini
[Unit]
Description=My ZeroDDS application
After=network.target

[Service]
ExecStart=/usr/local/bin/your-app
Environment=RUST_LOG=info
Environment=ZERODDS_DOMAIN=0
Restart=on-failure
RestartSec=5

# For RT:
AmbientCapabilities=CAP_SYS_NICE
LimitRTPRIO=99
LimitMEMLOCK=infinity

[Install]
WantedBy=multi-user.target
```

### Real-time privileges

`zerodds-rt-linux`'s `SchedulerProfile::apply_to_current_thread()` and the
CPU pinning need these privileges (`crates/rt-linux/src/scheduler.rs`):

| Profile | Linux policy | Privilege |
|---|---|---|
| `Default` | `SCHED_OTHER` | none |
| `RealtimeFifo { priority }` | `SCHED_FIFO` | `CAP_SYS_NICE` for a priority above 0; for 0 depending on `RLIMIT_RTPRIO` |
| `RealtimeRoundRobin { priority }` | `SCHED_RR` | as `RealtimeFifo` |
| `Deadline { runtime_ns, deadline_ns, period_ns }` | `SCHED_DEADLINE` | always `CAP_SYS_NICE`; `runtime ≤ deadline ≤ period`, `EBUSY` when the kernel's bandwidth reservation does not fit |
| CPU pinning | `sched_setaffinity` | none, within the process's own CPU set |

Without the privilege the call fails with `EPERM`. Give the binary the
capability instead of running it with `sudo`
(`sudo setcap 'cap_sys_nice=eip' /usr/local/bin/your-app`), or use the unit
above. Kernel parameters, `PREEMPT_RT`, IRQ affinity and the `cyclictest`
baseline are in
[`tests/perf/rt-tuning/host-config-checklist.md`](../../tests/perf/rt-tuning/host-config-checklist.md).

## Container (Docker / Podman)

```dockerfile
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

# Bring the .deb you built earlier:
COPY zerodds-tools_*.deb /tmp/
RUN dpkg -i /tmp/zerodds-tools_*.deb && rm /tmp/zerodds-tools_*.deb

COPY ./your-app /usr/local/bin/your-app
EXPOSE 7400-7900/udp                  # SPDP + ephemeral RTPS

ENTRYPOINT ["/usr/local/bin/your-app"]
```

**Networking modes**:

- `--network host` — easiest, multicast just works.
- `--network bridge` — needs explicit port mapping for ephemeral
  ports + multicast configuration. SPDP on Docker bridges is
  flaky; consider unicast static peer-list (planned config).
- `--network macvlan` — assigns the container a MAC on the host
  L2; multicast works as if bare-metal.

## Kubernetes

DDS in K8s is non-trivial — multicast is filtered out of pod
networks by default. Recommended approach:

- Deploy the DDS-bridge sidecar in `hostNetwork: true` mode (it
  sees the L2 multicast).
- Pod talks to the bridge via UDS / SHM.

A reference Helm chart lives at
`pkg/k8s/zerodds-bridge/Chart.yaml` (planned).

## Multi-host topology

```
+---------+      multicast     +---------+
| Host A  |  239.255.0.1:7400  | Host B  |
| (peer)  |  +================>|  (peer) |
+---------+                    +---------+
     |                                 |
     | unicast user-data port 7401-7900|
     +=================================+
```

Required:

- IGMP-snooping switch (or static multicast forwarding).
- Same domain ID on both peers.
- No NAT between peers (or fall back to TCP transport via
  `transport-tcp` and a unicast static peer-list).

## Cloud / VLAN without multicast

When multicast is disabled (typical AWS / GCP / Azure VPCs):

1. Configure unicast static peer-list (planned `RuntimeConfig`
   field; today via custom SPDP injection).
2. Use TCP transport instead of UDP.
3. Run the discovery server pattern (one peer with a known
   address that all others phone home to).

## Hardware ROM

| Aspect | Recommendation |
|---|---|
| CPU | x86_64 with AES-NI + PCLMULQDQ, or ARMv8 with FEAT_AES + FEAT_PMULL. Verify with `zerodds-perf hw-info` |
| RAM | 256 MiB minimum — ZeroDDS itself is ~10 MiB, the rest is your application + history caches |
| Network | 1 GbE for general use, 10 GbE for high-throughput data + isolated NIC for RT deployments |
| Disk | None required — ZeroDDS is RAM-only by default unless you enable `Persistent` durability or recording |

## Durability service (TRANSIENT and PERSISTENT)

`TRANSIENT_LOCAL` needs no service: the writer keeps its own history.
`TRANSIENT` and `PERSISTENT` (DDS 1.4 §2.2.3.4) are served by the daemon
`zerodds-durability-svc` (ADR 0009), one per DDS domain. It keeps the history
of the topics it serves and replays it to readers that join later, after the
writer's process is gone; with a durable store the history survives a restart
of the daemon. The options are in `zerodds-durability-svc(1)` and
`zerodds-durability-svc.yaml(5)`.

```bash
# every TRANSIENT/PERSISTENT topic on domain 0, sqlite store
zerodds-durability-svc --domain 0 --store sqlite \
    --path /var/lib/zerodds/durability/dur.db --auto

# a fixed set of topics, file store
zerodds-durability-svc --domain 0 --store file \
    --path /var/lib/zerodds/durability --topic SensorData --topic Commands
```

| `--store` | `--path` | Build |
|---|---|---|
| `sqlite` (default) | a database file; omitted: in memory, lost on restart | default |
| `file` | a directory (required) | default |
| `lakehouse` (DuckDB) | a database file | `--features lakehouse` |
| `postgres` | a libpq connection string (required) | `--features postgres` |

- systemd: `packaging/linux/systemd/zerodds-durability-svc.service`
  (`Type=notify`; domain, store and path from the `ZERODDS_DURABILITY_*`
  lines; the store directory is the only `ReadWritePaths`). One unit per
  domain, each with its own `--path`.
- Docker: `packaging/docker/durability-svc/Dockerfile` (default build:
  `sqlite` and `file`), with `--network host` and a volume on
  `/var/lib/zerodds/durability`.
- Backup: the store is the durable state. sqlite: `sqlite3 dur.db
  ".backup '/backup/dur.db'"` while running, or the `.db` with `-wal` and
  `-shm` while idle; file: copy the directory. At start the daemon serves what
  the store holds, so a restored backup resumes that history; a committed
  sample survives `kill -9` (the integration scenario
  `tests/integration/tests/services.rs::durability_outlives_writer_and_daemon`).
- Two daemons on the same domain and topic both replay; readers may see
  duplicates.

## Deployment checklist

- [ ] Native package installed; `zerodds-admin --version` runs.
- [ ] `zerodds-perf hw-info` shows expected backend (`aes-ni+pclmulqdq` or
      `armv8-aes+pmull`).
- [ ] systemd unit + capabilities configured.
- [ ] `RuntimeConfig.observability` wired to your log shipper.
- [ ] Domain ID + multicast group documented in your runbook.
- [ ] Security: governance + permissions XML deployed; CA chain
      in place; certs renewed within 90 days.
- [ ] For RT: kernel cmdline (isolcpus + nohz_full + rcu_nocbs)
      verified; cyclictest baseline captured.

## Reading further

- [Monitoring](monitoring.md) — what to watch in production.
- [Troubleshooting](troubleshooting.md) — symptoms and fixes.
