# `zerodds-record`

CLI-Frontend für das ZeroDDS-Capture-Format `.zddsrec`. Schreibt Wire-
Snapshots aktiver DDS-Topics in eine versionsstabile Binärdatei und
liest sie für Inspektion zurück.

Spec: [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md) +
[`docs/specs/zerodds-deployment-1.0.md`](../../docs/specs/zerodds-deployment-1.0.md) §1.1.

## Install

```bash
cargo install zerodds-record
# oder als Teil des Workspace-Builds:
cargo build --release --bin zerodds-record
```

## Sub-Commands

### `record` — Capture starten

```bash
zerodds-record record \
    --output capture.zddsrec \
    --domain 0 \
    --topic Sensor \
    --topic Heartbeat \
    --duration 30s
```

| Flag | Default | Beschreibung |
|---|---|---|
| `-o`, `--output FILE` | `capture-<ts>.zddsrec` | Output-Pfad |
| `-d`, `--domain ID` | `0` | DDS-Domain-ID |
| `-t`, `--topic NAME` | `*` | Topic-Filter (wiederholbar) |
| `--duration DUR` | indefinite | `5`, `30s`, `2m`, `1h` |
| `--max-sample-bytes N` | `1048576` | DoS-Cap pro Sample |
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

**Type-following:** der `record`-Subcommand entdeckt vor dem Festschreiben des
`.zddsrec`-Headers den echten IDL-Typnamen jedes Writers der angeforderten
Topics via SEDP (kurze Settle-Phase) und hängt sich pro `(Topic, Typ)` mit einem
opaken Reader an. So werden **typisierte Topics** (z.B. `cuas::Track`) erfasst —
ein generischer `RawBytes`-Reader würde keinen typisierten Writer matchen und
nichts aufzeichnen. Topics ohne entdeckten Writer fallen (mit Warnung) auf
`zerodds::RawBytes` zurück. Die Payload wird als rohe CDR-Bytes verlustfrei
gespeichert (Replay-fähig); der Header trägt den echten Typnamen.

### `info <FILE>` — Header inspizieren

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

### `list <FILE>` — Frames pro Topic zählen

```bash
zerodds-record list capture.zddsrec
# topic-frame counts: capture.zddsrec
#   total frames: 1024
#         512  Sensor
#         480  Heartbeat
#          32  LatReq
```

## Exit-Codes

| Code | Bedeutung |
|---|---|
| 0 | Erfolg |
| 2 | CLI-Parse-Fehler |
| 3 | I/O- oder Format-Fehler beim Lesen |

## Spec / Wire-Format

`.zddsrec` ist binär, length-prefixed, versionsstabil. Header trägt
`time_base_unix_ns` als Zeit-Anker; alle Frame-Timestamps sind Deltas
relativ dazu. Backend-Crate: [`zerodds-recorder`](../../crates/recorder).

## License

Apache-2.0. Siehe [`LICENSE`](../../LICENSE).
