# `zerodds-replay`

> Inspect/replay CLI for `.zddsrec` recordings: dump frames,
> replay at scaled wallclock, optional live re-injection into a
> DCPS domain.

[![Crates.io](https://img.shields.io/crates/v/zerodds-replay.svg)](https://crates.io/crates/zerodds-replay)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS tool for offline analysis and re-injection of recordings
captured by `zerodds-recorder-bridge`. Reads the `.zddsrec` format
defined by [`zerodds-recorder`](../../crates/recorder/), exposes a
read-only inspection mode, and an optional live-injection mode that
re-publishes recorded samples into a running DDS domain through
`zerodds-c-api`.

## Spec mapping

| Spec | Section |
| --- | --- |
| ZeroDDS Recorder 1.0 | `.zddsrec` file format |
| OMG DDSI-RTPS 2.5 | §10 sample serialisation |

## Safety classification

**COMFORT** — operator tooling, not safety-critical runtime code.

## Quickstart

```bash
zerodds-replay inspect recording.zddsrec
zerodds-replay dump recording.zddsrec
zerodds-replay replay recording.zddsrec --time-scale 1.0
zerodds-replay replay recording.zddsrec \
  --inject --inject-domain 0 --topic Pose

# replay also reads the decoded formats written by `record --decode`
zerodds-replay replay tracks.ndjson --inject
zerodds-replay replay tracks.db --inject
```

## Input formats

`replay` detects the format from the file extension and re-publishes byte-exact
from the raw CDR bytes each format retains:

| Extension | Format |
| --- | --- |
| `.zddsrec` | native binary capture (streamed) |
| `.json` / `.ndjson` / `.jsonl` | decoded NDJSON (`record --out-json`) |
| `.db` / `.sqlite` / `.sqlite3` | decoded SQLite (`record --out-sqlite`) |

The NDJSON line carries the DDS `type` name and SQLite carries `_types`, so the
replay writer is created with the correct type without an out-of-band IDL.

## Sub-commands

| Command | Purpose |
| --- | --- |
| `inspect <file>` | Print recording header and per-topic frame count. |
| `dump <file>` | List every frame with timestamp, topic, length. |
| `replay <file> [--time-scale F] [--topic NAME]... [--inject [--inject-domain N]]` | Replay frames at wallclock speed (`F=1.0`) or scaled (`F>1.0` faster); whitelist topics; optionally inject into a live DCPS domain. Accepts `.zddsrec`, NDJSON and SQLite (see *Input formats*). |

`inspect` and `dump` operate on `.zddsrec` only.

## Stability

`1.0.0-rc.1` — public CLI is stable. Breaking changes require a
major version bump.

## Build & test

```bash
cargo build -p zerodds-replay
cargo test  -p zerodds-replay
```

## Links

* [`crates/recorder/`](../../crates/recorder/) — `.zddsrec` format.
* [`tools/recorder-bridge/`](../recorder-bridge/) — live recording
  counterpart.
* [`CHANGELOG.md`](CHANGELOG.md).

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
