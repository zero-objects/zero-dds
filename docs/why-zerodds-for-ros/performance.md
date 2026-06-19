# Performance / Latency / CPU

← [Back to overview](index.md)

## The pain

Beyond outright failures, DDS in ROS 2 has performance sharp edges (**19
reports**): large messages blocking unrelated work, latency that gets *worse* at
low publish rates, and CPU/bandwidth spent on traffic that shouldn't exist.

- **Publishing a large message blocks all callback groups** — one big sample
  stalls the whole executor.
- Latency increases for low-frequency data (warm-path assumptions).
- Even alternative middlewares show high latency / lost messages at high publish
  rates, so this is not unique to one vendor.
- Stacks send unnecessary packets through the network, burning bandwidth and CPU.

### Most recent example

**[rmw_cyclonedds#559 — "Publishing large message blocks all callback
groups"](https://github.com/ros2/rmw_cyclonedds/issues/559)** (2026-03-03). A
single large publish blocks every callback group — a head-of-line stall that
couples unrelated parts of the application through the middleware.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2026-03-03 | [rmw_cyclonedds#559](https://github.com/ros2/rmw_cyclonedds/issues/559) | Large publish blocks all callback groups |
| 2025-04-12 | [cyclonedds#2256](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2256) | Latency *increases* for low-frequency data |
| 2024-06-07 | [rmw_zenoh#198](https://github.com/ros2/rmw_zenoh/issues/198) | High latency / lost messages at high pub rate (even on Zenoh) |
| 2024-04-19 | [rmw_cyclonedds#489](https://github.com/ros2/rmw_cyclonedds/issues/489) | Sends unnecessary packets through the network |

## How ZeroDDS solves it

**Low, consistent latency from an event-driven core; large data on its own path
so it doesn't stall everything else.**

- **Measured low latency.** Round-trip latency on loopback is **p50 = 40 µs /
  p99 = 83 µs** (256 B, 200 samples, 0 lost). The number is reproducible from
  the open `latency_ping` / `latency_pong` examples — not a slide.
- **Event-driven, no busy-poll.** Receive and wait paths use
  listeners/condvars/wakers, never spin loops, so there is no "latency gets worse
  when idle / low-frequency" warm-path artifact and no core burned waiting.
- **Large data on a dedicated fragmentation path.** Big samples go through the
  DATA_FRAG / NACK_FRAG path with explicit caps; they don't monopolize the
  small-message fast path, which is the mechanism behind "one large publish
  blocks everything."
- **No gratuitous traffic.** Multicast-free unicast means packets go to the peers
  that asked for them — no broadcast discovery chatter consuming bandwidth and
  CPU on every node.
- **Efficient by construction.** Pure Rust, `no_std + alloc` core (~1.6 MB on
  Cortex-M4F) — the same code path from MCU to server, without a garbage
  collector or a heavyweight runtime.

> **Honest status:** the loopback latency, WiFi throughput (10.8 MiB/s) and
> scaling numbers are verified. Head-to-head latency/throughput comparison tables
> *against each vendor on identical hardware* are still being produced — that is
> exactly the kind of measurement we want open-source validators to run and
> publish.

## Why it no longer has to be a pain

The performance cluster is *coupling* (large data stalling small data),
*warm-path assumptions* (idle latency), and *gratuitous traffic*. ZeroDDS keeps
paths decoupled, stays event-driven, and only sends unicast to real peers — so
performance is predictable rather than full of sharp edges.

## Reproduce it yourself

```bash
# Loopback RTT distribution (p50/p90/p99), 256 B, 200 samples:
crates/dcps/examples/latency_ping   # + latency_pong
```

→ [Back to overview](index.md) · Next: [Migration](migration.md)
