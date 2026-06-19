# `zerodds-recorder`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-recorder/badge.svg)](https://docs.rs/zerodds-recorder)

Deterministic record/replay format for the
[ZeroDDS](https://zerodds.org) stack: pure-Rust `.zddsrec` wire format
with reader, writer and a thread-safe live-session API. Safety
classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| ZDDSREC 1.0 | §1-§5 (magic + version + header layout + frame layout + sample kind + index discipline) |

Spec doc: [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md).

## What's inside

- **Format wire structures:** `Header`, `Frame`, `FrameView`, `SampleKind`, `ParticipantEntry`, `TopicEntry`, `ZDDSREC_MAGIC`, `ZDDSREC_VERSION`.
- **`RecordWriter`** — writes a `.zddsrec` stream incrementally into a `std::io::Write` sink. `WriteError` covers IO + format violations.
- **`RecordReader`** — parses a `&[u8]` buffer into `Header` + frame sequence. `ReadError` with concrete truncation paths.
- **`RecordingSession`** — high-level live API with `record_sample(topic, type, payload)`, atomic counters, lazy header and topic indexing.

## Layer position

Layer 4 — core services. Pure Rust + `alloc`, **no** ZeroDDS crate deps. Consumed by `tools/replay` (inspect/dump/replay CLI) and `tools/recorder-bridge` (live recording from the DcpsRuntime).

## Quickstart

Writing:

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

Reading:

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

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Standard library + `std::io::Write`. |
| `alloc` | ✅ via std | `Vec`/`String`. |
| `safety` | ❌ | Reserve hook for extra defensive checks. |

## Stability

`1.0.0-rc.1`. Wire format `ZDDSREC_VERSION = 1` is RC1-stable; an
incompatible change would bump the version constant (the `Reader`
rejects unknown versions). Additive extensions (streaming
reader, IndexAddFrame, optional compression) are designed as
major-2.0 hooks — see spec §"Stability and roadmap".

## Tests

```bash
cargo test -p zerodds-recorder
```

17 unit tests: format roundtrips, reader truncation paths, writer
header-once discipline, session thread safety.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/specs/zddsrec-1.0.md`](../../docs/specs/zddsrec-1.0.md) — wire-format spec.
- [`tools/replay`](../../tools/replay) — `zerodds-replay inspect|dump|replay` CLI.
- [`tools/recorder-bridge`](../../tools/recorder-bridge) — live recording from the DcpsRuntime.
