# TSN roundtrip latency bench

Measures the **host transport path latency** (request → echo → request) of the
live AF_PACKET transport (DDS-TSN 1.0 Annex A, EtherType `0x88B5`,
`crates/transport-tsn/src/socket.rs`) against a UDP baseline over
**the same** veth pair in the root namespace.

## What this is — and what it is not

This is **not** a TSN bounded latency measurement. The actual TSN
advantage (deterministic, tightly bounded latency + jitter) arises in the
**network**: a TSN switch with a time-aware shaper (802.1Qbv / taprio) or
ETF scheduling, plus gPTP time synchronisation. That can only be measured with
real TSN hardware.

This bench measures the **host socket path** and serves as:

- **baseline + regression guard** for the AF_PACKET transport,
- **honest comparison** AF_PACKET vs. UDP on the same link.

## Running

```bash
sudo tests/perf/tsn-latency/run.sh [COUNT]   # Default COUNT=20000
```

Needs root (`CAP_NET_RAW` + `CAP_NET_ADMIN`), Linux. Creates a veth pair
with IPs, runs echo server + latency client once each over AF_PACKET
and over UDP, and outputs one JSON object each (p50/p90/p99/max/jitter in ns).

Single modes (example binary directly):

```text
tsn_latency tsn-pong  <iface>
tsn_latency tsn-ping  <iface> <peer-mac> <count>
tsn_latency udp-pong  <bind-addr>
tsn_latency udp-ping  <bind-addr> <peer-addr> <count>
```

## Baseline (Linux test host, LXC, veth, 6.17.2, 4 vCPU, 2026-06-10)

5000 samples, 200 warmup, 64-byte payload, 0 % loss:

| Transport            | p50      | p90      | p99      | max     | Jitter (p99−p50) |
|----------------------|----------|----------|----------|---------|------------------|
| TSN / AF_PACKET 0x88B5 | 11.95 µs | 22.49 µs | 29.58 µs | 281 µs  | 17.62 µs         |
| UDP (same veth)      | 17.10 µs | 21.07 µs | 34.90 µs | 928 µs  | 17.79 µs         |

Observation: the raw Ethernet path is **below** UDP at p50, because it
bypasses the IP/UDP stack. This is expected and confirms that the
AF_PACKET transport has no overhead disadvantage compared to UDP. The
high `max` values come from the scheduling jitter of a non-isolated
LXC container without RT tuning — on isolated/RT-tuned cores
(`rt-tuning/`) considerably tighter.

**No TSN switch in the path** → no statement about deterministic
end-to-end latency. For that, see `internal/tsn/hardware-eval.md`.
