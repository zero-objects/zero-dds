# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Changed

- `regression` measures two builds of `roundtrip-1us` in turn
  (`--reference <bin>`, `--reference-tag <tag>`, `--cells`, `--reps`) and
  gates the median p50 of the build beside it against the reference's
  with 25 %; it reads and writes no baselines (`--update` and
  `tests/perf/internal-bench/baselines.txt` are gone). Cells a reference
  tag cannot measure are declared with the reason (`REFERENCE_GAPS`).

- `regression --update` writes a header that names the speed host and
  `RUN_BENCH_REGRESSION_UPDATE=true` as where the baselines come from; CI
  runs the gate there (`speed-regression`), not on the shared CI runner.

### Fixed

- `roundtrip_1us` in DCPS mode ends with an error when its writer or its
  reader matches no endpoint of the peer within 5 s; it dropped the results
  of both waits and printed "matched (both directions)" either way.
  `regression` prints the ping's error output for a repetition that fails or
  gets fewer than half its samples back, and prints the tolerance as +20 %
  (it truncated 1.20 to +19 %).

- `roundtrip_1us` destroys the writer or reader it made before the runtime
  when the other one cannot be created; it destroyed the runtime with the
  endpoint still in it.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Spec references

* OMG DDSI-RTPS 2.5 §10 — sample / fragment serialisation.
* OMG DDSI-RTPS 2.5 §8.4.6 — fragmentation negotiation
  (DATA_FRAG / NACK_FRAG).
* OMG DDS-DCPS 1.4 — DataReader / DataWriter API used by typed
  roundtrip benches.

### Binaries

* `roundtrip-1us` — sub-microsecond ping/pong latency tool with
  HdrHistogram output and optional `histlog` v2 export.
* `roundtrip-typed` — full XCDR2 typed roundtrip apples-to-apples
  with Cyclone / RTI / FastDDS custom apps.

### Benches

* `transports_e2e` — Criterion bench-group for `udp_send`,
  `uds_fs_send`, `uds_abstract_send` (Linux), `shm_send`,
  `tcp_send` over a fixed 9-point payload axis.
* `rtps_fragmented` — Criterion bench measuring the DATA_FRAG
  path for samples larger than MTU (32 B → 4 MiB).

### Public API (library `zerodds_bench_suite`)

* `PAYLOAD_SIZES` — canonical 9-point payload axis.
* `make_payload(size)` — deterministic payload generator.
* `size_label(size)` — printable label for BenchmarkIDs.

### Architecture

* Layer: Tools.
* Dependencies (in): `zerodds-rtps`, `zerodds-transport`, the four
  transport backends (UDP / UDS / SHM / TCP), `zerodds-c-api`,
  `zerodds-dcps`, `zerodds-cdr`, `zerodds-types`.
* Dev-dependency: `criterion`, `hdrhistogram`.

### Stability

Public CLI of `roundtrip-1us` and `roundtrip-typed` plus the
library re-exports are RC1-stable. Breaking changes require a
major version bump.
