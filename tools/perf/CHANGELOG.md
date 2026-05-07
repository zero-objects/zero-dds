# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the internal `perf` tool.

### Purpose

Load generator + latency profiler + benchmark suite for ZeroDDS
internal performance regression tracking. Outputs Criterion-compatible
reports plus custom JSON for the dashboard's perf trendline.

### Subcommands

- `perf hw-info` — detect HW-crypto capabilities (AES-NI, ARMv8-AES,
  PCLMULQDQ, NEON).
- `perf aes-gcm` — AES-GCM throughput benchmark.
- `perf roundtrip` — DDS pub/sub roundtrip latency at a given QoS profile.

### Architecture

- Layer: Tools (internal, `publish = false`)
- Dependencies: `zerodds-dcps`, `zerodds-rtps`, `criterion`,
  `security-crypto`

### Stability

Internal contract; metric names + JSON schema may evolve.
