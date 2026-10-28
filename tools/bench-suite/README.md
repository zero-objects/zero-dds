# ZeroDDS Bench-Suite

**Crate:** `zerodds-bench-suite` (tool, not runtime).
**Purpose:** citable, reproducible perf measurements for all
ZeroDDS transports over a fixed payload axis.

## What is here

- `src/lib.rs` — shared harness: `PAYLOAD_SIZES` constant (9 points
  from 32 B to 4 MiB), `make_payload()` deterministic
  payload gen, `size_label()` labels for BenchmarkIds.
- `benches/transports_e2e.rs` — 4 bench groups: `udp_send`,
  `uds_fs_send`, `uds_abstract_send` (Linux), `shm_send`. Each
  measures `transport.send(locator, payload)` over the complete
  payload axis (with a DGRAM cap at 64 KiB for UDP+UDS).
- `benches/rtps_fragmented.rs` — writer dispatch with fragmentation
  via the ReliableWriter, multiple reader proxies.
- `src/bin/roundtrip_1us.rs` — sub-µs latency bench:
  ping/pong busy-poll on UDP, histogram output with p50/p90/
  p99/p999/p9999/p99999, optional `histlog` V2 format export
  (`--hgrm out.hgrm`).

## How to produce the baseline

See `internal/perf/methodology.md` for details. Short version:

```bash
# on a bare-metal Debian 12 host (Threadripper 3955WX):
sudo ./benches/hosts/llvm/tune.sh on    # governor=performance,
                                         # net.core.*_max=32MiB, …

taskset -c 4-11 \
    cargo bench -p zerodds-bench-suite \
    --bench transports_e2e -- \
    --save-baseline v1.2-<date>

sudo ./benches/hosts/llvm/tune.sh off
```

The generated reports are under `target/criterion/` — raw JSON +
HTML plots. Baseline exports as Markdown under
`internal/perf/baseline-<host>-<date>.md`.

## Design decisions

Documented as comments in the code. Short version:

- **Fixed payload axis**: 9 points, no ad-hoc sizes.
  Reports are only comparable between runs if the axis is identical.
- **TX-only measurement**: the receiver runs in the same process/thread
  and drains silently. This way we measure the pure sender path without
  OS scheduling noise from the receiver. E2E ping/pong latency is captured by the
  separate `roundtrip-1us` binary.
- **DGRAM cap 60 KiB**: UDP + UDS DGRAM are capped by the kernel;
  sizes > 60 KiB are skipped in the DGRAM bench groups. SHM
  has no cap and runs over all 9 points.
- **SHM capacity 4×max_datagram**: prevents ring-wrap spin loops
  in the bench. In a realistic config the capacity is typically 1 MiB;
  we size it per payload size so that we do not pull wrap overheads into
  the measurement.
- **Consumer drain in the SHM bench**: the Criterion iter loop drains
  the consumer after every send, so that the ring does not fill up. This
  means the bench measures send+recv together; the pure send
  path without a drain would quickly be capped by the full ring.

## Roundtrip-1us — latency bench

Sub-µs round-trip bench for latency measurement on RT-tuned hosts.
Runs as a two-process setup:

```bash
# pong process (CPU-pinned, RT prio recommended):
taskset -c 2 chrt -f 80 \
  roundtrip-1us --role pong --addr 0.0.0.0:7400 --payload 64

# ping process (on the other machine):
taskset -c 3 chrt -f 80 \
  roundtrip-1us --role ping \
    --remote 192.0.2.10:7400 --bind 0.0.0.0:7401 \
    --payload 64 --warmup 5000 --samples 100000 \
    --hgrm out.hgrm
```

Output: stdout p50/p90/p99/p999/p9999/p99999 + min/max/n;
`--hgrm` writes the full histogram in the `histlog` V2 text format
(plottable with OSADL `cyclictest_plot`).

## Cross-vendor comparison

`tools/bench-suite` only measures the ZeroDDS path. Cross-vendor
comparisons against Cyclone DDS (`ddsperf`) + FastDDS
(`LatencyTestPublisher`) run via the live interop harness in
`internal/interop/`.

## Regression gate

After every tag, the CI is meant to:

1. Load the frozen baseline from the repo.
2. Run `cargo bench -- --baseline <tag>`.
3. Fail on regression: > 10 % degradation of p50 at any
   (bench, size) point breaks the build.

Currently manual — automation is material for a follow-up sprint.
