# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `RecordingSession::finish`: writes the header when no sample did and
  flushes the sink, so that a capture without samples is a header without
  frames; `RecordWriter::flush`.

### Changed

- The tests check the octets of `docs/specs/zddsrec-1.0.md`
  (`tests/zddsrec_layout.rs`) instead of the writer against the reader.


### Changed

- Format version 3: a frame carries the representation identifier of its
  sample's encapsulation header (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table
  60), `Frame::representation_identifier` / `FrameView::representation_identifier`
  (`Option<[u8; 2]>`, presence octet and two octets after `SampleKind`). The
  payload is the body without the header, and a body alone does not say
  whether it is XCDR1 or XCDR2, plain, delimited or a parameter list, or its
  byte order: a replay sent every sample under one header. Versions 1 and 2
  read with no identifier (`RecordReader::version`).
  `RecordingSession::record_sample` takes the identifier before the payload.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-recorder` crate.

### Spec references

- **`docs/specs/zddsrec-1.0.md`** §1-§5 — wire-format spec.

### Public API

**Wire format:**
- `Header`, `Frame`, `FrameView`, `SampleKind`, `ParticipantEntry`, `TopicEntry`.
- Constants: `ZDDSREC_MAGIC = "ZDDS"`, `ZDDSREC_VERSION = 1`, `FRAME_MAGIC = b'F'`.

**Writer:**
- `RecordWriter::{new, write_header, write_frame, finish}`.
- `WriteError::{Io, HeaderAlreadyWritten, FrameBeforeHeader, …}`.

**Reader:**
- `RecordReader::{new, header, frames}`.
- `ReadError::{TruncatedHeader, BadMagic, UnsupportedVersion, UnknownFrameMagic, BadSampleKind, …}`.

**Session:**
- `RecordingSession::{new, record_sample, frames_count, bytes_written, finish}`.
- `SessionOptions`, `TopicKey`, `SessionError`.

### Implementation

The `Header` structure carries magic + version + UNIX time base + participant array + topic array, all multi-byte fields little-endian. For each sample a `Frame` is written with `TimestampDelta` (relative to `time_base_unix_ns`), participant/topic indices and `SampleKind`. Frames are independently parseable once the header has been read — reader and writer both work incrementally.

`RecordingSession` builds a high-level live API on top of this: lazy header writing, atomic frame/byte counters for dashboard telemetry, mutex-protected writer ingress for thread-safe multi-producer use.

`forbid(unsafe_code)` is set (via workspace lints).

### Architecture

- **Layer:** 4 (core services).
- **Dependencies (in):** none (pure Rust + `alloc`).
- **Dependents (out):** `tools/replay`, `tools/recorder-bridge`, end-user builds directly.
- **Feature flags:** `std` (default), `alloc` (via std), `safety` (reserve hook).

### Stability

- Public API: RC1-stable.
- Wire format `ZDDSREC_VERSION = 1`: stable; an incompatible change would require a major bump and a version change.
- The reader rejects unknown FrameMagic bytes with `ReadError::UnknownFrameMagic` — additive frame types (e.g. `IndexAddFrame`, `CompressedFrame`) can be added safely in a 2.0 major.
