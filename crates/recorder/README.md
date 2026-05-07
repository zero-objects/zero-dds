# `zerodds-recorder`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-recorder/badge.svg)](https://docs.rs/zerodds-recorder)

Deterministic Record/Replay-Format fuer den
[ZeroDDS](https://zerodds.org)-Stack: pure-Rust `.zddsrec`-Wire-Format
mit Reader, Writer und thread-safer Live-Session-API. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| ZDDSREC 1.0 | §1-§5 (Magic + Version + Header-Layout + Frame-Layout + Sample-Kind + Index-Disziplin) |

Spec-Doc: [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md).

## Was ist drin

- **Format-Wire-Strukturen:** `Header`, `Frame`, `FrameView`, `SampleKind`, `ParticipantEntry`, `TopicEntry`, `ZDDSREC_MAGIC`, `ZDDSREC_VERSION`.
- **`RecordWriter`** — schreibt einen `.zddsrec`-Stream inkrementell in einen `std::io::Write`-Sink. `WriteError` deckt IO + Format-Verstoesse ab.
- **`RecordReader`** — parsed einen `&[u8]`-Buffer in `Header` + Frame-Sequenz. `ReadError` mit konkreten Truncation-Pfaden.
- **`RecordingSession`** — high-level Live-API mit `record_sample(topic, type, payload)`, atomare Counter, lazy-Header und Topic-Indexing.

## Schichten-Position

Layer 4 — Core Services. Pure-Rust + `alloc`, **keine** ZeroDDS-Crate-Deps. Wird von `tools/replay` (Inspect/Dump/Replay-CLI) und `tools/recorder-bridge` (Live-Recording aus DcpsRuntime) konsumiert.

## Quickstart

Schreiben:

```rust,no_run
use zerodds_recorder::{RecordWriter, Header, ParticipantEntry, TopicEntry, Frame, SampleKind};

let mut sink = Vec::new();
let mut writer = RecordWriter::new(&mut sink);
writer.write_header(&Header {
    time_base_unix_ns: 0,
    participants: vec![ParticipantEntry { guid: [0; 16], name: "talker".into() }],
    topics: vec![TopicEntry { name: "rt/chatter".into(), type_name: "std_msgs::msg::String".into() }],
}).unwrap();
writer.write_frame(&Frame {
    timestamp_delta_ns: 0,
    participant_idx: 0,
    topic_idx: 0,
    sample_kind: SampleKind::Alive,
    payload: vec![1, 2, 3],
}).unwrap();
```

Lesen:

```rust,no_run
use zerodds_recorder::RecordReader;
let bytes: &[u8] = b""; // .zddsrec stream
let reader = RecordReader::new(bytes).unwrap();
for frame in reader.frames() {
    let frame = frame.unwrap();
    println!("topic={} kind={:?} payload_len={}",
        frame.topic_idx, frame.sample_kind, frame.payload.len());
}
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library + `std::io::Write`. |
| `alloc` | ✅ via std | `Vec`/`String`. |
| `safety` | ❌ | Reserve-Hook fuer extra Defensive-Checks. |

## Stabilitaet

`1.0.0-rc.1`. Wire-Format `ZDDSREC_VERSION = 1` ist RC1-stabil; eine
inkompatible Aenderung wuerde die Version-Konstante heben (`Reader`
lehnt unbekannte Versionen ab). Additive Erweiterungen (Streaming-
Reader, IndexAddFrame, optionale Compression) sind als Major-2.0-Hooks
designt — siehe Spec §"Stabilitaet und Roadmap".

## Tests

```bash
cargo test -p zerodds-recorder
```

17 Unit-Tests: Format-Roundtrips, Reader-Truncation-Pfade, Writer-
Header-Once-Disziplin, Session-Thread-Safety.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md) — Wire-Format-Spec.
- [`tools/replay`](../../tools/replay) — `zerodds-replay inspect|dump|replay` CLI.
- [`tools/recorder-bridge`](../../tools/recorder-bridge) — Live-Recording aus DcpsRuntime.
