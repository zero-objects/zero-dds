# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

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
