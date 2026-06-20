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
