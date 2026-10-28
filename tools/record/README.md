# `zerodds-record`

CLI frontend for the ZeroDDS capture format `.zddsrec`. Writes wire
snapshots of active DDS topics into a version-stable binary file and
reads them back for inspection.

Spec: [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md) +
[`docs/specs/zerodds-deployment-1.0.md`](../../docs/specs/zerodds-deployment-1.0.md) §1.1.

## Install

```bash
cargo install zerodds-record
# or as part of the workspace build:
cargo build --release --bin zerodds-record
```

## Subcommands

### `record` — start a capture

```bash
zerodds-record record \
    --output capture.zddsrec \
    --domain 0 \
    --topic Sensor \
    --topic Heartbeat \
    --duration 30s
```

| Flag | Default | Description |
|---|---|---|
| `-o`, `--output FILE` | `capture-<ts>.zddsrec` | Output path |
| `-d`, `--domain ID` | `0` | DDS domain ID |
| `-t`, `--topic NAME` | `*` | Topic filter (repeatable) |
| `--duration DUR` | indefinite | `5`, `30s`, `2m`, `1h` |
| `--max-sample-bytes N` | `1048576` | DoS cap per sample |
| `--decode` | off | Decode samples to typed values (needs `--type-file`) |
| `--type-file IDL` | — | Out-of-band IDL providing the types |
| `--map TOPIC=Type` | — | Override which IDL type decodes a topic (repeatable) |
| `--out-json FILE` | — | Write decoded samples as NDJSON |
| `--out-sqlite FILE` | — | Write decoded samples to per-topic SQLite |

#### Decoded output (`--decode`)

With `--decode --type-file <IDL>` each captured CDR sample is additionally
decoded to typed values (via the reflective XTypes codec) and fanned out to a
per-topic relational SQLite DB (`--out-sqlite`) and/or NDJSON (`--out-json`).
The opaque `.zddsrec` is **always** written too — decode/write failures warn but
never abort the capture. Each decoded sink retains the raw CDR bytes per sample,
so `zerodds-replay` can re-publish byte-exact from any format.

The IDL type per topic defaults to the writer's discovered DDS type name; use
`--map TopicName=Pkg::Type` to override. The SQLite schema uses `sample_id` as a
record marker, with child tables (joined by `sample_id`) for scalar
sequences/arrays; `_types` holds the full IDL per topic.

```bash
zerodds-record record -t TrackTopic \
    --decode --type-file tracks.idl --map TrackTopic=cuas::Track \
    --out-sqlite tracks.db --out-json tracks.ndjson
```

**Type-following:** before writing the `.zddsrec` header, the `record` subcommand
discovers the real IDL type name of every writer of the requested
topics via SEDP (short settle phase) and attaches an opaque reader per
`(topic, type)`. This way **typed topics** (e.g. `cuas::Track`) are captured —
a generic `RawBytes` reader would not match a typed writer and would
record nothing. Topics without a discovered writer fall back (with a warning) to
`zerodds::RawBytes`. The payload is stored losslessly as raw CDR bytes
(replay-capable); the header carries the real type name.

### `info <FILE>` — inspect the header

```bash
zerodds-record info capture.zddsrec
# zddsrec header: capture.zddsrec
#   time-base (unix-ns): 1700000000000000000
#   participants:        2
#   topics (3):
#     - Sensor
#     - Heartbeat
#     - LatReq
```

### `list <FILE>` — count frames per topic

```bash
zerodds-record list capture.zddsrec
# topic-frame counts: capture.zddsrec
#   total frames: 1024
#         512  Sensor
#         480  Heartbeat
#          32  LatReq
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 2 | CLI parse error |
| 3 | I/O or format error while reading |

## Spec / wire format

`.zddsrec` is binary, length-prefixed, version-stable. The header carries
`time_base_unix_ns` as the time anchor; all frame timestamps are deltas
relative to it. Backend crate: [`zerodds-recorder`](../../crates/recorder).

## License

Apache-2.0. See [`LICENSE`](../../LICENSE).
