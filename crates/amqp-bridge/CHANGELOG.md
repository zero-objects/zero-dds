# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-amqp-bridge`-Crate.

### Spec-Referenzen

- **OASIS AMQP 1.0** Part 1 (Types), Part 2 (Transport), Part 3 (Messaging): §1.6 (Primitive Types), §1.7 (Restricted Types), §2.3 (Frame-Format), §2.7 (Performatives), §3 (Variable-Width-Encodings), §3 Messaging (Message-Format), §3.2 Messaging (Section-Reihenfolge).
- **OMG DDS-AMQP 1.0** (formal/2024-08-01) §2.3 (Codec-Profile), §2.4 (Codec-Lite-Profile), §6.1 (Direct-Embed-Topology), §7 (Type-System-Mapping), §8 (Message-Section-Mapping).

### Public-API

**Type-System (`types`-Modul):**
- `AmqpValue::{Null, Boolean, Ulong, Long, Str, Symbol, Binary, ...}`.
- `FormatCode` + `codes::*` Format-Code-Konstanten.
- `TypeError`.
- `decode_value(input) -> Result<(AmqpValue, usize), TypeError>`.
- `encode_null` / `encode_boolean` / `encode_long` / `encode_ulong` / `encode_string` / `encode_symbol` / `encode_binary`.

**Extended Types (`extended_types`-Modul):**
- `AmqpExtValue` — erweitertes Variant-Modell (alle Primitive + Compound).
- `encode_ubyte` / `encode_ushort` / `encode_uint` / `encode_byte` / `encode_short` / `encode_int`.
- `encode_float` / `encode_double` / `encode_char`.
- `encode_decimal32` / `encode_decimal64` / `encode_decimal128`.
- `encode_timestamp` / `encode_uuid`.
- Korrespondierende `decode_*`-Funktionen.

**Frame-Format (`frame`-Modul):**
- `FrameHeader { size, doff, frame_type, channel }`.
- `FrameType::{Amqp, Sasl}`.
- `FrameError`.
- `encode_frame_header` / `decode_frame_header`.

**Performatives (`performatives`-Modul):**
- `open` / `begin` / `attach` / `flow` / `transfer` / `disposition` / `detach` / `end` / `close` — Builder-Funktionen.
- `encode_performative` / `decode_performative`.

**Message-Sections (`sections`-Modul):**
- `MessageSection::{Header, DeliveryAnnotations, MessageAnnotations, Properties, ApplicationProperties, AmqpValue, AmqpSequence, Data, Footer}`.
- `validate_section_sequence(sections) -> Result<(), TypeError>` — §3.2-Reihenfolge.

**Codec-Profile (`codec_profile`-Modul):**
- `CodecProfile::{Full, Lite}`.
- `active_profile() -> CodecProfile` (`const fn`; `Lite` mit Cargo-Feature `codec-lite`, sonst `Full`).
- `is_codec_lite_value(&AmqpExtValue) -> bool` / `is_codec_lite_section(&MessageSection) -> bool`.

### Implementierung

`AmqpValue` (in `types`) ist das stable Subset, `AmqpExtValue` (in `extended_types`) das vollstaendige Set inklusive Compound-Typen. Beide sind unabhaengig, mit `From`-Conversions zwischen dem Subset und dem Vollset.

Compound-Decoder (`list8`, `list32`, `map8`, `map32`, `array8`, `array32`) tracken die Recursion-Tiefe ueber `MAX_COMPOUND_DEPTH = 32`. AMQP-Spec §3.3.1.4 (Spec-analog) und DDS-AMQP-1.0 §6.1 (Implementation-Note) verlangen DoS-Caps bei untrusted-input.

Performatives sind described composites: ein 0x00-prefix-byte signalisiert das described-format, gefolgt von einem `ulong` Descriptor-Code (z.B. 0x10 fuer `open`), gefolgt von einem `list8`/`list32` Body. Der Body enthaelt die in der Spec definierten Felder als positional-encoded list-Elements; nicht-gesetzte Optional-Felder werden als `null` codiert.

Message-Sections folgen demselben described-composite-Pattern. `validate_section_sequence` erzwingt die §3.2-Constraints: Header (optional, max 1, zuerst), gefolgt von Delivery-Annotations / Message-Annotations / Properties / Application-Properties / `Data` oder `AmqpValue` oder `AmqpSequence` / Footer (optional, max 1, zuletzt).

Frame-Header ist 8 Bytes: 4 Bytes SIZE BE (inklusive aller Frame-Bytes ab dem Header), 1 Byte DOFF (Data-Offset in 32-bit-Words), 1 Byte TYPE (0x00 = AMQP, 0x01 = SASL), 2 Bytes CHANNEL BE.

`#![forbid(unsafe_code)]` ist gesetzt. `extern crate alloc;`.

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** keine (Substrat-Crate). Nur `core` + `alloc`.
- **Dependents (out):** `zerodds-amqp-endpoint` (DDS-AMQP-1.0 Endpoint-Layer).
- **Feature-Flags:** `std` (default), `alloc` (via std), `codec-lite` (Codec-Lite-Profile-Marker).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch OASIS AMQP 1.0 fixiert.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.
- Cargo-Feature `codec-lite` ist conformance-only (kein Code-Pfad-Unterschied).
