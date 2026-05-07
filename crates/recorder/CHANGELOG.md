# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-recorder`-Crate.

### Spec-Referenzen

- **`docs/specs/zddsrec-1.0.md`** §1-§5 — Wire-Format-Spec.

### Public-API

**Wire-Format:**
- `Header`, `Frame`, `FrameView`, `SampleKind`, `ParticipantEntry`, `TopicEntry`.
- Konstanten: `ZDDSREC_MAGIC = "ZDDS"`, `ZDDSREC_VERSION = 1`, `FRAME_MAGIC = b'F'`.

**Writer:**
- `RecordWriter::{new, write_header, write_frame, finish}`.
- `WriteError::{Io, HeaderAlreadyWritten, FrameBeforeHeader, …}`.

**Reader:**
- `RecordReader::{new, header, frames}`.
- `ReadError::{TruncatedHeader, BadMagic, UnsupportedVersion, UnknownFrameMagic, BadSampleKind, …}`.

**Session:**
- `RecordingSession::{new, record_sample, frames_count, bytes_written, finish}`.
- `SessionOptions`, `TopicKey`, `SessionError`.

### Implementierung

`Header`-Struktur traegt Magic + Version + UNIX-Time-Base + Participant-Array + Topic-Array, alle Multi-Byte-Felder little-endian. Pro Sample wird ein `Frame` mit `TimestampDelta` (relativ zum `time_base_unix_ns`), Participant-/Topic-Indizes und `SampleKind` geschrieben. Frames sind nach einmal gelesenem Header eigenstaendig parsebar — Reader und Writer arbeiten beide inkrementell.

`RecordingSession` baut darauf eine high-level Live-API: lazy-Header-Schreiben, atomare Frame-/Byte-Counter fuer Dashboard-Telemetrie, Mutex-protected Writer-Ingress fuer thread-safen Multi-Producer-Use.

`forbid(unsafe_code)` ist gesetzt (per Workspace-Lints).

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** keine (pure-Rust + `alloc`).
- **Dependents (out):** `tools/replay`, `tools/recorder-bridge`, end-user-Builds direkt.
- **Feature-Flags:** `std` (default), `alloc` (via std), `safety` (Reserve-Hook).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format `ZDDSREC_VERSION = 1`: stabil; inkompatible Aenderung wuerde Major-Bump und Versionswechsel erfordern.
- Reader lehnt unbekannte FrameMagic-Bytes mit `ReadError::UnknownFrameMagic` ab — additive Frame-Typen (z.B. `IndexAddFrame`, `CompressedFrame`) koennen sicher in einer 2.0-Major hinzugefuegt werden.
