# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the internal `qos-matrix` tool.

### Purpose

Generates the full QoS-policy compatibility matrix (Reliability,
Durability, Deadline, History, Ownership, Liveliness, ResourceLimits,
DestinationOrder, Lifespan, ContentFilter, etc.) as Markdown + CSV. Each
cell shows whether a writer/reader pairing is compatible, and which
policy mismatches are flagged by `RequestedIncompatibleQos`.

### Spec References

- OMG DDS-DCPS 1.4 §2.2.3 — RxO QoS-compatibility table

### Architecture

- Layer: Tools (internal, `publish = false`)
- Dependencies: `zerodds-qos`, `serde_json`, `clap`

### Stability

Internal contract; output format consumed only by the docs generator.
