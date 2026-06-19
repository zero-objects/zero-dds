# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-amqp-bridge` crate.

### Spec References

- **OASIS AMQP 1.0** Part 1 (Types), Part 2 (Transport), Part 3 (Messaging): §1.6 (Primitive Types), §1.7 (Restricted Types), §2.3 (Frame Format), §2.7 (Performatives), §3 (Variable-Width Encodings), §3 Messaging (Message Format), §3.2 Messaging (Section ordering).
- **OMG DDS-AMQP 1.0** (formal/2024-08-01) §2.3 (Codec Profile), §2.4 (Codec-Lite Profile), §6.1 (Direct-Embed Topology), §7 (Type-System Mapping), §8 (Message-Section Mapping).

### Public-API

**Type System (`types` module):**
- `AmqpValue::{Null, Boolean, Ulong, Long, Str, Symbol, Binary, ...}`.
- `FormatCode` + `codes::*` Format-Code-Konstanten.
- `TypeError`.
- `decode_value(input) -> Result<(AmqpValue, usize), TypeError>`.
- `encode_null` / `encode_boolean` / `encode_long` / `encode_ulong` / `encode_string` / `encode_symbol` / `encode_binary`.

**Extended Types (`extended_types` module):**
- `AmqpExtValue` — extended variant model (all primitives + compound).
- `encode_ubyte` / `encode_ushort` / `encode_uint` / `encode_byte` / `encode_short` / `encode_int`.
- `encode_float` / `encode_double` / `encode_char`.
- `encode_decimal32` / `encode_decimal64` / `encode_decimal128`.
- `encode_timestamp` / `encode_uuid`.
- Corresponding `decode_*` functions.

**Frame Format (`frame` module):**
- `FrameHeader { size, doff, frame_type, channel }`.
- `FrameType::{Amqp, Sasl}`.
- `FrameError`.
- `encode_frame_header` / `decode_frame_header`.

**Performatives (`performatives` module):**
- `open` / `begin` / `attach` / `flow` / `transfer` / `disposition` / `detach` / `end` / `close` — builder functions.
- `encode_performative` / `decode_performative`.

**Message Sections (`sections` module):**
- `MessageSection::{Header, DeliveryAnnotations, MessageAnnotations, Properties, ApplicationProperties, AmqpValue, AmqpSequence, Data, Footer}`.
- `validate_section_sequence(sections) -> Result<(), TypeError>` — §3.2 ordering.

**Codec Profile (`codec_profile` module):**
- `CodecProfile::{Full, Lite}`.
- `active_profile() -> CodecProfile` (`const fn`; `Lite` with the Cargo feature `codec-lite`, otherwise `Full`).
- `is_codec_lite_value(&AmqpExtValue) -> bool` / `is_codec_lite_section(&MessageSection) -> bool`.

### Implementation

`AmqpValue` (in `types`) is the stable subset, `AmqpExtValue` (in `extended_types`) the complete set including compound types. Both are independent, with `From` conversions between the subset and the full set.

The compound decoders (`list8`, `list32`, `map8`, `map32`, `array8`, `array32`) track the recursion depth via `MAX_COMPOUND_DEPTH = 32`. AMQP spec §3.3.1.4 (spec-analogous) and DDS-AMQP 1.0 §6.1 (implementation note) require DoS caps on untrusted input.

Performatives are described composites: a 0x00 prefix byte signals the described format, followed by a `ulong` descriptor code (e.g. 0x10 for `open`), followed by a `list8`/`list32` body. The body contains the fields defined in the spec as positional-encoded list elements; unset optional fields are encoded as `null`.

Message sections follow the same described-composite pattern. `validate_section_sequence` enforces the §3.2 constraints: Header (optional, max 1, first), followed by Delivery-Annotations / Message-Annotations / Properties / Application-Properties / `Data` or `AmqpValue` or `AmqpSequence` / Footer (optional, max 1, last).

The frame header is 8 bytes: 4 bytes SIZE BE (including all frame bytes from the header onward), 1 byte DOFF (data offset in 32-bit words), 1 byte TYPE (0x00 = AMQP, 0x01 = SASL), 2 bytes CHANNEL BE.

`#![forbid(unsafe_code)]` is set. `extern crate alloc;`.

### Architecture

- **Layer:** 5 (Bridges).
- **Dependencies (in):** none (substrate crate). Only `core` + `alloc`.
- **Dependents (out):** `zerodds-amqp-endpoint` (DDS-AMQP 1.0 endpoint layer).
- **Feature flags:** `std` (default), `alloc` (via std), `codec-lite` (Codec-Lite-Profile marker).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by OASIS AMQP 1.0.
- Error discriminants: stable; new discriminants are major-additive.
- The Cargo feature `codec-lite` is conformance-only (no code-path difference).
