# ZeroDDS Operator Handbook

For platform engineers, SREs, and DevOps people who deploy and
run ZeroDDS in production. Cross-references jump into trail
stations 03 (Configuration) and 06 (Operations) for the long
form.

---

## 1. Production deployment

### Linux — systemd unit

`/etc/systemd/system/zerodds-bridge.service`:

```ini
[Unit]
Description=ZeroDDS bridge daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
User=zerodds
Group=zerodds
ExecStart=/usr/bin/zerodds-amqp-bridge --config /etc/zerodds/bridge.yaml
Restart=on-failure
RestartSec=2
TimeoutStartSec=30
LimitNOFILE=65536

# Hardening
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=yes
ReadWritePaths=/var/lib/zerodds /var/log/zerodds

[Install]
WantedBy=multi-user.target
```

Enable + start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now zerodds-bridge
journalctl -u zerodds-bridge -f
```

For real-time profiles, add `CPUSchedulingPolicy=fifo` and
`CPUAffinity=2 3 4 5` (the cores you isolated with `isolcpus`).
Full kernel tuning is in `docs/REALTIME_DEPLOYMENT.md` (internal
repo only).

### macOS — launchd plist

`/Library/LaunchDaemons/org.zerodds.bridge.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>            <string>org.zerodds.bridge</string>
  <key>ProgramArguments</key> <array>
    <string>/usr/local/bin/zerodds-amqp-bridge</string>
    <string>--config</string>
    <string>/etc/zerodds/bridge.yaml</string>
  </array>
  <key>RunAtLoad</key>        <true/>
  <key>KeepAlive</key>        <true/>
  <key>StandardOutPath</key>  <string>/var/log/zerodds/bridge.log</string>
  <key>StandardErrorPath</key><string>/var/log/zerodds/bridge.err</string>
</dict>
</plist>
```

```bash
sudo launchctl load /Library/LaunchDaemons/org.zerodds.bridge.plist
```

### Windows — service via `sc.exe`

```powershell
sc.exe create ZeroDDSBridge `
    binPath= "C:\Program Files\ZeroDDS\zerodds-amqp-bridge.exe --config C:\ProgramData\ZeroDDS\bridge.yaml" `
    start= auto
sc.exe description ZeroDDSBridge "ZeroDDS bridge daemon"
sc.exe start ZeroDDSBridge
```

Logs land in the Windows Event Log under `Application →
ZeroDDSBridge`.

---

## 2. Configuration files

ZeroDDS daemons read YAML from `/etc/zerodds/` (Linux/macOS) or
`%PROGRAMDATA%\ZeroDDS\` (Windows). The schema:

```yaml
# /etc/zerodds/runtime.yaml
runtime:
  domain_id: 0
  participant_name: "edge-gw-01"
  tick_period_ms: 5

discovery:
  spdp_multicast: "239.255.0.1:7400"
  unicast_peers:
    - "10.0.1.10:7400"
    - "10.0.1.11:7400"

transport:
  mode: tls
  cert: /etc/zerodds/pki/node.pem
  key:  /etc/zerodds/pki/node.key
  ca:   /etc/zerodds/pki/ca.pem

security:
  governance:    /etc/zerodds/policies/governance.p7s
  permissions:   /etc/zerodds/policies/permissions.p7s
  identity_ca:   /etc/zerodds/pki/ca.pem
  identity_cert: /etc/zerodds/pki/node.pem
  identity_key:  /etc/zerodds/pki/node.key

acl:
  default_policy: deny
  allow:
    - topic: "Telemetry/*"
      role:  publisher
    - topic: "Commands/*"
      role:  subscriber

observability:
  sink: stderr-json
  prometheus:
    listen: "127.0.0.1:9464"
  otlp:
    endpoint: "http://collector.observability.svc:4318"

resource_limits:
  max_samples_per_writer: 4096
  max_instances:          1024
  max_message_size:       65536
```

The full per-field reference, with defaults, lives in
`03-configuration/runtime-config.md`.

---

## 3. Monitoring

### Prometheus

The Prometheus listener is built in. Scrape `:9464/metrics` for:

| Metric | Type | Meaning |
|---|---|---|
| `zerodds_dds_history_cache_evicted_total` | counter | Samples dropped from a cache (data loss on Reliable+KeepAll) |
| `zerodds_dds_discovery_peer_count` | gauge | Live participants in the SPDP cache |
| `zerodds_dds_heartbeat_unanswered_total` | counter | Heartbeats not ACK'd |
| `zerodds_dds_deadline_missed_total` | counter | Deadline misses |
| `zerodds_dds_liveliness_lost_total` | counter | Writer-side liveliness losses |
| `zerodds_dds_security_policy_violations_total` | counter | ACL denials |
| `zerodds_dds_writer_throughput_bytes_total` | counter | Per-topic writer bytes |
| `zerodds_dds_reader_latency_ns` | histogram | End-to-end latency |

A reference Grafana dashboard ships at
`documentation/grafana/zerodds-overview.json` (when present).

### OTLP tracing

Set `observability.otlp.endpoint` and ZeroDDS emits per-message
spans (`writer.write`, `reader.take`, `reliable.heartbeat`,
`security.handshake`). Sampler is configurable; default is
`parent_based(trace_id_ratio = 0.01)` — 1% sampling with
parent override.

### Live dashboard

The Tauri-based `zerodds-dashboard` connects to a participant
via the built-in topic API and renders the Pub/Sub graph plus
per-endpoint metrics in real time. Useful for incident response;
not a replacement for Prometheus + Grafana.

---

## 4. Backup and recovery

### Recorder

`zerodds-recorder` captures wire traffic to a rotating log:

```bash
zerodds-recorder \
    --domain 0 \
    --topics "Commands/#,Telemetry/critical" \
    --output /var/lib/zerodds/recordings/ \
    --rotate-bytes 1G \
    --retain 30d
```

Output is the `.zdr` (ZeroDDS recording) container — XCDR2
samples plus discovery snapshots, gzip-compressed,
crash-resilient (fsync per segment boundary).

### Replay

```bash
zerodds-replay \
    --domain 1 \
    --input /var/lib/zerodds/recordings/2026-01-15/ \
    --speed 1.0 \
    --topics "Commands/#"
```

`--speed 0` replays as fast as possible (ideal for incident
post-mortem). `--speed 1.0` replays in real time.

### Disaster-recovery checklist

- Recorder running on each independently-failed AZ.
- 7-day rolling retention plus weekly off-site snapshot.
- Permissions XML and CA chain backed up with the same cadence.
- Quarterly drill: restore from yesterday's recording into a
  staging domain and verify topic flow with `zerodds-admin --topics`.

---

## 5. Security hardening

| Control | How |
|---|---|
| TLS-only on every transport | `transport.mode = tls`; reject `tcp` fallback at the firewall. |
| Mutual auth | Permissions CA + identity CA separation; ship per-node certs via your secrets manager. |
| Least-privilege ACL | `acl.default_policy = deny`; allow per-topic/role; review quarterly. |
| Cert rotation | 90-day max validity; automate via `cert-manager` (Kubernetes) or `certbot` (bare metal). |
| Log shipping | `observability.sink = stderr-json` → fluentd → SIEM; alert on `policy_violations_total > 0`. |
| Patch cadence | Subscribe to <https://github.com/zero-objects/zero-dds/security/advisories>. |
| Supply-chain | Verify SBOM (CycloneDX) and signatures on every release; pin the exact version in your config-management tool. |
| Hot-path isolation | Run the bridge daemon under its own UID, drop CAP_NET_ADMIN if not needed for multicast. |

---

## 6. Capacity planning

Bridge throughput on a 16-core x86-64 host (AES-NI, 3.5 GHz,
TLS terminated locally). Numbers are sustained, not peak.

| Bridge | Msg/s | MB/s | p99 latency |
|---|---|---|---|
| RTPS native (loopback) | 1.2M | 1500 | 80 µs |
| RTPS native (10 GbE) | 800k | 1100 | 220 µs |
| AMQP-bridge ↔ RabbitMQ | 80k | 95 | 4 ms |
| MQTT-bridge ↔ Mosquitto | 120k | 140 | 2 ms |
| WebSocket-bridge (browser) | 20k | 25 | 12 ms |
| gRPC-bridge | 50k | 60 | 6 ms |
| CoAP-bridge | 10k | 12 | 8 ms |

For RT sizing: budget 64 KiB of pre-allocated PoolBuffer per
hot-path writer, plus 8 KiB per matched reader. A participant
with 200 writers and 200 matched readers takes ~14 MiB of RAM
at steady state.

---

## 7. Upgrade path

ZeroDDS uses semantic versioning starting at the `1.0.0` release.

| From | To | Compatibility |
|---|---|---|
| 1.0-rc → 1.0 | wire-format-stable; config schema may rev | review the migration table in the release notes |
| 1.0 → 1.1 | wire + config additive; no breaking changes | drop-in upgrade |
| 1.x → 1.y | additive only | drop-in upgrade |
| 1.x → 2.0 | breaking changes possible | follow the 2.0 migration guide; expect a parallel rollout window |

Rolling upgrade procedure:

1. Snapshot the current config + permissions XML + CA chain.
2. Pre-stage the new package on each host.
3. One AZ at a time: drain bridge daemons, upgrade, restart,
   verify with `zerodds-admin --participants` and the metric
   `zerodds_dds_discovery_peer_count`.
4. Monitor `policy_violations_total` for 24 h before the next AZ.
5. Wire-format compatibility means new and old peers
   interoperate during the rollout.

---

## Where to next

- Day-zero reference: `06-operations/deployment.md`.
- Per-incident playbook: `06-operations/troubleshooting.md`.
- The full QoS knob list: `03-configuration/qos-policies.md`.
