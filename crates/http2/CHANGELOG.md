# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-http2`-Crate.

### Spec-Referenzen

- **RFC 9113** (HTTP/2): §3.4 (Connection-Preface), §4 (Frame-Layer Format), §5.1 (Stream-States), §5.2 (Flow-Control), §6.1-§6.10 (`DATA` / `HEADERS` / `PRIORITY` / `RST_STREAM` / `SETTINGS` / `PUSH_PROMISE` / `PING` / `GOAWAY` / `WINDOW_UPDATE` / `CONTINUATION`), §6.5 (SETTINGS-Codec mit Defaults), §6.9 (`WINDOW_UPDATE`-Frame), §7 (Error-Codes).
- **Vorgaenger:** RFC 7540 wurde von RFC 9113 abgeloest. Wire-Format und §-Nummern bleiben weitgehend identisch; 9113 entfernt einige ungenutzte Features (Priority-Hint-Direktiven) und klaert mehrere Edge-Cases. Diese Crate folgt 9113.

### Public-API

**Frame-Layer:**
- `FrameType::{Data, Headers, Priority, RstStream, Settings, PushPromise, Ping, GoAway, WindowUpdate, Continuation}` + `FrameType::from_u8`.
- `Flags(pub u8)` + Konstanten (`END_STREAM`, `END_HEADERS`, `PADDED`, `PRIORITY`, `ACK`).
- `FrameHeader { length, frame_type, flags, stream_id }`.
- `Frame<'a> { header, payload: &'a [u8] }` (zero-copy).
- `FRAME_HEADER_LEN`, `DEFAULT_MAX_FRAME_SIZE`.
- `encode_frame(header, payload, out, max_frame_size) -> Result<usize, Http2Error>`.
- `decode_frame(input, max_frame_size) -> Result<(Frame<'_>, usize), Http2Error>`.

**Connection-Preface:**
- `CLIENT_PREFACE: &'static [u8; 24]` (`PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`).
- `check_preface(input) -> Result<usize, Http2Error>`.

**SETTINGS (`settings`-Modul):**
- `SettingId::{HeaderTableSize, EnablePush, MaxConcurrentStreams, InitialWindowSize, MaxFrameSize, MaxHeaderListSize}` + `from_u16`.
- `Setting { id, value }`.
- `Settings::{default, apply}`.
- `decode_settings(payload) -> Result<Vec<Setting>, Http2Error>`.
- `encode_settings(settings) -> Vec<u8>`.

**Streams (`stream`-Modul):**
- `StreamId = u32` (Type-Alias).
- `StreamState::{Idle, ReservedLocal, ReservedRemote, Open, HalfClosedLocal, HalfClosedRemote, Closed}`.
- `StreamEvent` (Trigger-Set fuer Transitions).
- `transition(state, event) -> Result<StreamState, Http2Error>` — §5.1-Diagramm.
- `is_client_initiated(stream_id)`, `is_server_initiated(stream_id)`.

**Flow-Control (`flow`-Modul):**
- `FlowControl::{new, window, consume, apply_window_update, apply_initial_window_size_change}`.
- `encode_window_update(increment) -> [u8; 4]`.
- `decode_window_update(payload) -> Result<u32, Http2Error>`.
- `INITIAL_WINDOW_SIZE: i64 = 65_535`.

**Errors:**
- `ErrorCode::{NoError, ProtocolError, InternalError, FlowControlError, SettingsTimeout, StreamClosed, FrameSizeError, RefusedStream, Cancel, CompressionError, ConnectError, EnhanceYourCalm, InadequateSecurity, Http11Required}` + `from_u32` + `as_u32`.
- `Http2Error::{ShortFrameHeader, ShortPayload, FrameTooLarge { got, max }, UnknownFrameType(u8), InvalidPreface, InvalidSetting(u16), InvalidStreamTransition, FlowControlOverflow, FlowControlWindowZero, ShortSettingsPayload, ShortWindowUpdate}` + `Display` + `std::error::Error` (Feature `std`).

### Implementierung

`encode_frame`/`decode_frame` arbeiten direkt auf Byte-Slices ohne Heap-Allocation. R-Bit (MSB des Stream-Id-Worts) wird beim Decode strip'ed (Spec §4.1: "Implementations MUST ignore this bit"). `max_frame_size`-Parameter erzwingt §6.5.2-`SETTINGS_MAX_FRAME_SIZE`-Konformitaet sowohl beim Encode als auch beim Decode.

`FlowControl` haelt das Window als `i64` (Spec §5.2.1: das Window kann durch `INITIAL_WINDOW_SIZE`-Adjustments theoretisch negativ werden bis zu `-2^31 + 1`). `consume` lehnt `bytes > window` ab; `increment` lehnt Overflows ab und mappt `WINDOW_UPDATE` mit Increment 0 als `FlowControlError` per §6.9.

Stream-State-Machine implementiert das §5.1-Diagramm exakt: alle 14 erlaubten Transitions plus die `RST_STREAM`-Sofort-zu-`Closed`-Pfade. Ungueltige Transitions liefern `InvalidStreamTransition`.

`#![no_std]` + `extern crate alloc;`. `#![forbid(unsafe_code)]` ist gesetzt (per Workspace-Lints + lokal).

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** keine (Substrat-Crate). Nur `core` + `alloc`.
- **Dependents (out):** `zerodds-grpc-bridge` (HTTP/2-Connection-Path), `zerodds-conformance` (Cross-Vendor-Test-Harness).
- **Feature-Flags:** `std` (default, aktiviert `std::error::Error`-Impls), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch RFC 9113 fixiert.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.
- Modul-Pfade `error`, `flow`, `frame`, `preface`, `settings`, `stream` sind explizit `pub` und Teil des stabilen Surface (Caller darf z.B. `frame::DEFAULT_MAX_FRAME_SIZE` direkt nutzen).
