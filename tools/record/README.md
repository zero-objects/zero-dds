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

**Hinweis:** Live-Capture braucht eine angeschlossene DCPS-Runtime;
in RC1 validiert der `record`-Subcommand die Konfiguration und
beendet sich. Der vollständige Capture-Pfad wird mit RC2 freigeschaltet.

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
