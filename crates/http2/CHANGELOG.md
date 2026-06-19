# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-http2` crate.

### Spec references

- **RFC 9113** (HTTP/2): §3.4 (connection preface), §4 (frame-layer format), §5.1 (stream states), §5.2 (flow control), §6.1-§6.10 (`DATA` / `HEADERS` / `PRIORITY` / `RST_STREAM` / `SETTINGS` / `PUSH_PROMISE` / `PING` / `GOAWAY` / `WINDOW_UPDATE` / `CONTINUATION`), §6.5 (SETTINGS codec with defaults), §6.9 (`WINDOW_UPDATE` frame), §7 (error codes).
- **Predecessor:** RFC 7540 was superseded by RFC 9113. The wire format and §-numbers remain largely identical; 9113 removes some unused features (priority-hint directives) and clarifies several edge cases. This crate follows 9113.

### Public API

**Frame layer:**
- `FrameType::{Data, Headers, Priority, RstStream, Settings, PushPromise, Ping, GoAway, WindowUpdate, Continuation}` + `FrameType::from_u8`.
- `Flags(pub u8)` + constants (`END_STREAM`, `END_HEADERS`, `PADDED`, `PRIORITY`, `ACK`).
- `FrameHeader { length, frame_type, flags, stream_id }`.
- `Frame<'a> { header, payload: &'a [u8] }` (zero-copy).
- `FRAME_HEADER_LEN`, `DEFAULT_MAX_FRAME_SIZE`.
- `encode_frame(header, payload, out, max_frame_size) -> Result<usize, Http2Error>`.
- `decode_frame(input, max_frame_size) -> Result<(Frame<'_>, usize), Http2Error>`.

**Connection preface:**
- `CLIENT_PREFACE: &'static [u8; 24]` (`PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`).
- `check_preface(input) -> Result<usize, Http2Error>`.

**SETTINGS (`settings` module):**
- `SettingId::{HeaderTableSize, EnablePush, MaxConcurrentStreams, InitialWindowSize, MaxFrameSize, MaxHeaderListSize}` + `from_u16`.
- `Setting { id, value }`.
- `Settings::{default, apply}`.
- `decode_settings(payload) -> Result<Vec<Setting>, Http2Error>`.
- `encode_settings(settings) -> Vec<u8>`.

**Streams (`stream` module):**
- `StreamId = u32` (type alias).
- `StreamState::{Idle, ReservedLocal, ReservedRemote, Open, HalfClosedLocal, HalfClosedRemote, Closed}`.
- `StreamEvent` (trigger set for transitions).
- `transition(state, event) -> Result<StreamState, Http2Error>` — §5.1 diagram.
- `is_client_initiated(stream_id)`, `is_server_initiated(stream_id)`.

**Flow control (`flow` module):**
- `FlowControl::{new, window, consume, apply_window_update, apply_initial_window_size_change}`.
- `encode_window_update(increment) -> [u8; 4]`.
- `decode_window_update(payload) -> Result<u32, Http2Error>`.
- `INITIAL_WINDOW_SIZE: i64 = 65_535`.

**Errors:**
- `ErrorCode::{NoError, ProtocolError, InternalError, FlowControlError, SettingsTimeout, StreamClosed, FrameSizeError, RefusedStream, Cancel, CompressionError, ConnectError, EnhanceYourCalm, InadequateSecurity, Http11Required}` + `from_u32` + `as_u32`.
- `Http2Error::{ShortFrameHeader, ShortPayload, FrameTooLarge { got, max }, UnknownFrameType(u8), InvalidPreface, InvalidSetting(u16), InvalidStreamTransition, FlowControlOverflow, FlowControlWindowZero, ShortSettingsPayload, ShortWindowUpdate}` + `Display` + `std::error::Error` (Feature `std`).

### Implementation

`encode_frame`/`decode_frame` operate directly on byte slices without heap allocation. The R bit (MSB of the stream-id word) is stripped on decode (Spec §4.1: "Implementations MUST ignore this bit"). The `max_frame_size` parameter enforces §6.5.2 `SETTINGS_MAX_FRAME_SIZE` conformance on both encode and decode.

`FlowControl` holds the window as `i64` (Spec §5.2.1: the window can theoretically become negative — down to `-2^31 + 1` — through `INITIAL_WINDOW_SIZE` adjustments). `consume` rejects `bytes > window`; `increment` rejects overflows and maps `WINDOW_UPDATE` with increment 0 to `FlowControlError` per §6.9.

The stream state machine implements the §5.1 diagram exactly: all 14 permitted transitions plus the `RST_STREAM` immediate-to-`Closed` paths. Invalid transitions return `InvalidStreamTransition`.

`#![no_std]` + `extern crate alloc;`. `#![forbid(unsafe_code)]` is set (via workspace lints + locally).

### Architecture

- **Layer:** 5 (Bridges).
- **Dependencies (in):** none (substrate crate). Only `core` + `alloc`.
- **Dependents (out):** `zerodds-grpc-bridge` (HTTP/2 connection path), `zerodds-conformance` (cross-vendor test harness).
- **Feature flags:** `std` (default, enables `std::error::Error` impls), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by RFC 9113.
- Error discriminants: stable; new discriminants are major-additive.
- The module paths `error`, `flow`, `frame`, `preface`, `settings`, `stream` are explicitly `pub` and part of the stable surface (a caller may use e.g. `frame::DEFAULT_MAX_FRAME_SIZE` directly).
