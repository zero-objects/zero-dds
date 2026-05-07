# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

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
