# ROS 2 Delivery-Modes — Same-Host Throughput/Latency (internal)

> **INTERNAL — do NOT publish externally (website / marketing / public docs).**
> Strategic "joker card" (raw same-host ~8 GB/s, memory-bound). Quote internally
> only until we decide to release.

Measures the three `zerodds-delivery-modes-1.0` modes end-to-end through the
ROS 2 RMW layer (rclcpp), same-host: `Portable` (serialized CDR over RTPS),
`RawSameHost` (POSIX-SHM, CDR body shared), `Iceoryx` (iceoryx2).

## Setup

- Host: `codepit` (Debian 13 LXC on an AMD Ryzen Threadripper PRO 3955WX,
  DDR4-3200), ROS 2 Humble via RoboStack/micromamba. LXC sees 4 cores / 16 GiB.
- Build: `rmw-zerodds-shim` + `zerodds-c-api` `--release`, feature
  `delivery-iceoryx`; `RMW_IMPLEMENTATION=rmw_zerodds_cpp`.
- Topology: one process, one node, publisher + a normal-callback subscription on
  one topic; the subscriber runs a blocking `SingleThreadedExecutor::spin()` in a
  separate thread (exercises the event-driven `rmw_wait` doorbell + cancel path).
- Message: fixed-POD `uint8[N]` + `uint64 stamp` + `uint32 seq` (loanable;
  `can_loan_messages` = true). N ∈ {64 KiB, 1 MiB, 4 MiB}.
- Method: **lockstep one-way latency** — publish (loaned `borrow`/`publish`),
  wait until received, record `recv_time − stamp` (CLOCK_MONOTONIC, same host).
  50/30/20 warmup + 300/200/100 measured per size.
  `eff GB/s = bytes / p50_latency` (single-stream effective).
- Mode via `ZERODDS_DELIVERY_MODE` (`portable` default | `raw-same-host` | `iceoryx`).

## Results (2026-06-18, codepit)

| Size | Portable (CDR→RTPS) | RawSameHost (SHM) | Iceoryx (iceoryx2) |
|---|---|---|---|
| 64 KiB | p50 1659 µs · p99 2098 · 0.04 GB/s | p50 **20.5 µs** · p99 27 · 3.20 GB/s | p50 **20.0 µs** · p99 25 · 3.27 GB/s |
| 1 MiB  | p50 25822 µs · p99 30768 · 0.04 GB/s | p50 **128 µs** · p99 174 · 8.19 GB/s | p50 **130 µs** · p99 192 · 8.05 GB/s |
| 4 MiB  | p50 103294 µs · p99 111859 · 0.04 GB/s | p50 **593 µs** · p99 1163 · 7.07 GB/s | p50 **1176 µs** · p99 1361 · 3.57 GB/s |

All runs delivered every sample (no loss) and tore down cleanly.

## Memory-bandwidth context (why ~8 GB/s is near-optimal)

| Bound | Value |
|---|---|
| Theoretical platform peak (8-ch DDR4-3200) | 204.8 GB/s |
| Per channel | 25.6 GB/s |
| **Empirical single-thread memcpy on codepit** | **17.6 GB/s** (≈ 35 GB/s read+write traffic) |

The lockstep stream is single-core-bound, so the relevant ceiling is the
**17.6 GB/s** single-thread memcpy, not the 204.8 GB/s platform peak. The raw
path moves the payload twice per message at this ceiling: serialize struct→CDR
into the SHM slot (writer) + memcpy SHM→message (normal-callback receiver). At
1 MiB the 128 µs latency ≈ two 1-MiB passes at 17.6 GB/s — i.e. the raw modes are
**memory-bound, not transport-bound**: there is essentially no protocol overhead
left, only the necessary copies. Headroom is in *removing copies*
(`rmw_take_loaned_message` drops the receive memcpy; a struct-verbatim wire drops
the serialize), not in more bandwidth. Portable, by contrast, is ~40 MB/s —
dominated by serialization + RTPS fragmentation + the reliable ACK loop on the
loopback, three orders of magnitude off the memory ceiling.

## Reading

- **Portable** pays serialize → RTPS fragmentation → reliable ACK: ~40 MB/s,
  latency linear in size (4 MiB ≈ 103 ms).
- **RawSameHost / Iceoryx** share the CDR body via shared memory — no network, no
  writer heap staging — **80–200× faster**, 7–8 GB/s.
- `RawSameHost` ≈ `Iceoryx` at 64 K/1 M; at 4 MiB SHM is ~2× faster (the iceoryx
  receive copies into an owned buffer + the struct memcpy).
- These are *normal-callback* numbers (one receive memcpy). A loaned-take
  subscription would drop that copy too.

## Caveat / honest scope

- Single-process, same-host. Portable is the only cross-host / cross-vendor mode;
  the raw modes are same-host-only by design (`publishes_to_wire` gate).
- Effective GB/s is single-stream lockstep (one in flight) — isolates per-message
  latency (the fair mode comparison), not max pipelined throughput.

## Reproduce

`perf_msgs` (ament msg pkg) `PerfMsg{64K,1M,4M}.msg` = `uint64 stamp` +
`uint32 seq` + `uint8[N] data`; build the Python introspection generator with
`-DPython_EXECUTABLE/_INCLUDE_DIR/_LIBRARY/_NumPy_INCLUDE_DIR` hints (RoboStack).
Driver `perf_node.cpp` = templated `run<MsgT>(label, bytes, warmup, meas)` per
above. Per mode:

```
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
ZERODDS_DELIVERY_MODE=<portable|raw-same-host|iceoryx> \
LD_LIBRARY_PATH=$CONDA_PREFIX/lib:<repo>/target/release:<perf_msgs>/lib \
  ./perf_node
```

Shim built `--features rmw-zerodds-shim/delivery-iceoryx` for the iceoryx mode
(iceoryx2 runs daemonless on codepit). Kill stale `perf_node` between runs —
overlapping ROS_DOMAIN_IDs cross-talk and skew results. memcpy ceiling probe:
`gcc -O2` a 256-MiB `memcpy` loop with a `volatile` sink (else the loop is elided).

## Side-finding (fixed)

This benchmark surfaced a real RMW bug: a blocking `spin()` + `executor.cancel()`
hung (`th.join()` deadlock) — `rmw_wait` re-blocked forever on a notify-wake when
nothing was ready (the cancel interrupt-guard is not always in its array). Fixed
in `rmw_wait` (return on any notify-wake so the executor re-evaluates its own
cancel/spinning). Invisible to the `spin_some` tests. Commit
`fix(rmw-zerodds-shim): rmw_wait must return on a notify-wake`.
