# `zerodds-recorder-bridge`

> Live recorder daemon: subscribes to a configured set of DDS
> topics and writes received samples to a `.zddsrec` file.

[![Crates.io](https://img.shields.io/crates/v/zerodds-recorder-bridge.svg)](https://crates.io/crates/zerodds-recorder-bridge)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS tool that captures a running domain to disk. For each
configured `topic_name:type_name` pair, the bridge creates a
DataReader, polls in a loop, and writes received samples to a
`.zddsrec` file via
[`zerodds_recorder::RecordingSession`](../../crates/recorder/).

## Spec mapping

| Spec | Section |
| --- | --- |
| OMG DDS-DCPS 1.4 | §2.2 — DataReader API |
| ZeroDDS Recorder 1.0 | `.zddsrec` file format |

## Safety classification

**COMFORT** — operator tooling.

## Quickstart

```bash
zerodds-recorder-bridge \
  --output recording.zddsrec \
  --domain 0 \
  --topic Pose:robot::Pose \
  --topic Telemetry:robot::Telemetry \
  --duration 60s
```

Use `Ctrl+C` to stop early; the recording is closed cleanly on
SIGINT.

## Stability

`1.0.0-rc.1` — public CLI is stable. Breaking changes require a
major version bump.

## Build & test

```bash
cargo build -p zerodds-recorder-bridge
cargo test  -p zerodds-recorder-bridge
```

## Links

* [`crates/recorder/`](../../crates/recorder/) — `.zddsrec` format.
* [`tools/replay/`](../replay/) — playback / re-injection.
* [`CHANGELOG.md`](CHANGELOG.md).

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
