# `.zddsrec` v1.0 — Recording format spec

ZeroDDS vendor spec. Implemented in `crates/recorder` (`zerodds-recorder`).

## Goals

* **Compact**: no textual annotations in the hot path. CDR payloads
  remain unchanged; the header collects metadata once.
* **Stream-friendly**: frames can be parsed on their own after
  a header that is read once. Reader and writer are both
  incremental.
* **Specifiable**: byte-exact on 1 page, no reflection magic.

## File structure

Endianness: **little-endian** for all multi-byte integers.

```
+========== Header ==========+
| Magic    "ZDDS"  (4 byte)  |
| Version  u32     (LE)      |  // = 2 for this spec
| TimeBase i64     (LE) ns   |  // UNIX epoch anchor
| ParticipantCount u32 (LE)  |
| TopicCount       u32 (LE)  |
| Participants[]             |
|   Guid       16 bytes      |
|   NameLen    u32 (LE)      |
|   Name       UTF-8         |
| Topics[]                   |
|   TypeLen    u32 (LE)      |
|   TypeName   UTF-8         |
|   NameLen    u32 (LE)      |
|   Name       UTF-8         |
+========== Frames ==========+
| FrameMagic   'F'  (1 byte) |
| TimestampDelta i64 (LE) ns |
| ParticipantIdx u32 (LE)    |
| TopicIdx       u32 (LE)    |
| SampleKind     u8          |  // 0=Alive 1=Disposed 2=Unregistered
| PayloadLen     u32 (LE)    |
| Payload        bytes       |
+============================+
| FrameMagic 'F' ... further |
+============================+
```

EOF: no explicit marker — the reader stops when the cursor reaches the
end of the file.

## Indices

`ParticipantIdx` and `TopicIdx` are 0-based indices into the
corresponding header lists.

A topic that appears in the middle of the stream is appended with a `TopicAdd`
(see below). Its index continues the header list: the
first `TopicAdd` of a file has the index `TopicCount`, the second
`TopicCount + 1`. A reader that processes the `TopicAdd` entries in the order
in which they appear in the stream arrives at the same index for the same topic
as the writer.

For participants this does not exist yet — there it still holds that the
recorder must know them before the first frame.

### TopicAdd

```
+========== TopicAdd ========+
| Magic        'T'  (1 byte) |
| TypeLen      u32  (LE)     |
| TypeName     UTF-8         |
| NameLen      u32  (LE)     |
| Name         UTF-8         |
+============================+
```

Stands between frames, at any position at which a frame may also
stand. Carries no sample.

**Why it exists.** A recorder learns the type name of a topic
from the writer that publishes it. A writer that starts after the recorder
is the normal case and not an error. Without `TopicAdd` there would remain
two options, and both were bad: give up the topic, or
register a reader with a placeholder type that no typed
writer ever matches — and then record nothing for the entire runtime
without anyone noticing.

**Version.** `TopicAdd` exists from version 2 on. A version 2 file
without `TopicAdd` is byte for byte a version 1 file; a reader for version
2 reads both.

## SampleKind encoding

| Value | Spec mapping (DDS 1.4 §2.2.4.4.5) |
|-------|-----------------------------------|
| 0     | `ALIVE`                           |
| 1     | `NOT_ALIVE_DISPOSED`              |
| 2     | `NOT_ALIVE_UNREGISTERED`          |

Other values: the reader returns a `BadSampleKind(b)` error.

## Versioning

`Version = ZDDSREC_VERSION` (= 1). The reader rejects with
`UnsupportedVersion(v)` for `v > ZDDSREC_VERSION` — older
versions can be read on the reader side in the compatibility path.

Backward-incompatible changes raise the major version. Additive
changes that remain read-only backward-compatible with default values
(e.g. ignoring new frame magics) stay in the same
major version.

## Use cases

* **Crash reproduce**: a production recorder writes a 24h trace,
  a test setup plays it back with `--time-scale 60` (1h wall clock).
* **Forensic diff**: overlay two traces with identical `TimeBase`
  in time.
* **Soak replay**: the chaos test suite (F.2) injects realistic
  loss/jitter patterns on a replay.

## Tooling

| Tool                                      | Function             |
|-------------------------------------------|----------------------|
| `crates/recorder` library                 | API for apps         |
| `tools/replay` binary (`zerodds-replay`)  | inspect/dump/replay  |
| `tools/recorder-bridge`                   | live record from domain |

## Stability and roadmap

`zerodds-recorder` 1.0 covers the core format + reader + writer +
in-process `RecordingSession`. The following extensions are planned as
**additive** major-2.0 hooks, without breaking the 1.0 wire form:

* Live recording from DcpsRuntime via a `zerodds-c-api` hook (path via
  `tools/recorder-bridge`).
* Live replay → `zerodds_writer_write` re-injection.
* `IndexAddFrame` for inline discovery additions (reserve magic byte
  already attachable at the end-of-stream cursor).
* Optional `zlib`/`zstd` frame compression with a separate FrameMagic
  as an opt-in extension.

The 1.0 reader rejects unknown FrameMagic bytes with
`ReadError::UnknownFrameMagic` — additive frame types can
be added safely.
