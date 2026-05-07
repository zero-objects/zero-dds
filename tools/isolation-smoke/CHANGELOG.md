# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the internal `isolation-smoke` tool.

### Purpose

Runs the full ZeroDDS smoke-test matrix in isolated namespaces (Linux
network namespaces or systemd-nspawn containers) to verify correct
behavior under partition, network restart, and clock-skew conditions.

### Architecture

- Layer: Tools (internal, `publish = false`)
- Dependencies: `zerodds-dcps`, test harness, OS-level isolation tools
  (network namespaces / nspawn)

### Stability

Internal contract; output is consumed by the CI dashboard only.
