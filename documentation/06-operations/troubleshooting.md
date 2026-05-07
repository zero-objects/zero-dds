# Troubleshooting

Common symptoms, root causes, diagnostic commands.

## Discovery

### Symptom: `wait_for_matched(1, 5s)` times out

Pub and sub start, but never see each other.

| Cause | Diagnosis | Fix |
|---|---|---|
| Different domain IDs | `zerodds-admin --participants` on each host shows different domains | match the IDs |
| Multicast filtered | `tcpdump -i any -nn 'udp and port 7400'` shows no SPDP | enable IGMP-snooping or use unicast peer-list |
| Different multicast group | `RuntimeConfig.spdp_multicast_group` overridden | match the group |
| L2 broadcast domains differ | participants on different VLANs / VPCs | route SPDP via static unicast peer-list |
| ZeroDDS port collision | `ss -uln \| grep 7400` shows another DDS process | change domain ID |

Quick smoke test:

```bash
# Host A:
zerodds-admin --participants

# Host B:
zerodds-admin --participants

# Each should list the other after ~5 s. If not, check tcpdump.
```

## Wire issues

### Symptom: parse errors in the log

```
WARN zerodds_rtps: header.parse failed: BadVendorId
WARN zerodds_rtps: submessage 0x80 with reader_id zero — discarded
```

| Cause | Fix |
|---|---|
| Foreign vendor with RTPS 2.1 sending 0x80 (= ZeroDDS HeaderExtension) | Already handled — gated on `protocol_version >= 2.5`. Ignore the warning if it doesn't repeat. |
| Truly malformed packet | Run `zerodds-traceability <pcap>` to dump submessage details. |

## Reliable delivery

### Symptom: writer cache grows unbounded

`HistoryCacheStats.len` keeps climbing.

| Cause | Fix |
|---|---|
| Slow reader holding back acknowledgement | Investigate the reader — `zerodds-admin --readers` shows match status |
| `KeepAll` cache + `max_samples` too high | Switch to `KeepLast(N)` to bound the cache |
| Multicast packet loss | Check `evicted_count` and `unknown_src_count` |

### Symptom: occasional duplicate samples

Reader sees the same SN twice.

| Cause | Fix |
|---|---|
| Multicast loopback on top of unicast | Bind to a specific interface in `RuntimeConfig.multicast_interface` |
| Two peers with same GUID prefix | Verify `zerodds-admin --participants` shows distinct prefixes |

## Real-time / latency

### Symptom: p99 latency spikes every ~10 ms

Tick interrupts on the RT-CPU.

```bash
cat /proc/interrupts | awk '$NF=="3" { print }'   # CPU 3 has IRQs
```

Fix: enable `nohz_full` on your RT cores. See
`docs/REALTIME_DEPLOYMENT.md` §3 (internal repo only).

### Symptom: p99 spikes every ~1 s

RCU callbacks scheduled on the RT core.

Fix: `rcu_nocbs` on the RT cores.

### Symptom: SCHED_DEADLINE returns EBUSY

Bandwidth reservation full.

```bash
cat /proc/sys/kernel/sched_rt_runtime_us
sysctl -w kernel.sched_rt_runtime_us=950000
```

## Security

### Symptom: handshake fails immediately on first peer

```
ERROR zerodds_security_pki: cert chain validation failed: subject mismatch
```

| Cause | Fix |
|---|---|
| Identity cert subject does not match permissions XML `<subject_name>` | Re-issue cert or update permissions XML |
| Cert expired | Re-issue (we recommend 90-day validity max) |
| Identity CA cert not installed on peer | Distribute the CA bundle |

### Symptom: discovery succeeds but topic never matches under security

| Cause | Fix |
|---|---|
| Permissions XML denies the topic | Check `<allow_rule>` covers the topic + domain |
| Governance protection mismatch | Both peers must agree on `data_protection_kind`; mixed `NONE` and `ENCRYPT` peers won't match |

## Build / install

### Symptom: `cargo build` fails on `ring` 0.17

Rust toolchain too old.

```bash
rustup update stable    # need >= 1.85 (workspace pin)
```

### Symptom: `dpkg-buildpackage` fails on `dh-cargo`

Multi-binary workspace not handled by stock dh-cargo.

Use the local `pkg/debian/rules` — it does manual `dh_install`
calls instead of relying on dh-cargo for splitting.

## Log levels

ZeroDDS uses standard `log` / `tracing` macros. Enable:

```bash
RUST_LOG=zerodds_dcps=debug,zerodds_rtps=info,zerodds_discovery=info ./your-app
```

Per-component tuning:

| Module | What you see at `debug` |
|---|---|
| `zerodds_dcps::runtime` | Endpoint lifecycle, SEDP-match decisions |
| `zerodds_rtps::reliable_writer` | Per-sample fanout, ACKNACK handling |
| `zerodds_discovery::sedp` | Per-publication / subscription announce |
| `dds_security_*` | Handshake state machine, gate decisions |

## Diagnostic CLI commands

```bash
zerodds-admin --participants               # peer-list with GUID
zerodds-admin --topics                     # topic catalogue
zerodds-admin --readers --writers          # endpoint listing
zerodds-admin --listen --topic <name>      # tail samples on a topic
zerodds-perf hw-info                       # CPU-feature audit
zerodds-traceability <pcap>                # decode RTPS bytes
zerodds-chaos packet-loss --rate 0.1       # inject loss for testing
```

## When all else fails

1. Reproduce with `RUST_LOG=trace`.
2. Capture a `tcpdump` of the discovery + data flow.
3. Run `zerodds-traceability` over the pcap.
4. File an issue at the GitLab project including the trace + the
   exact `RuntimeConfig` you ran with.
