# OASIS AMQP 1.0 — Spec-Coverage

**Spec:** [OASIS AMQP Version 1.0 — OASIS Standard →](https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-overview-v1.0-os.html) (overview/types/transport/messaging/security).

**Kontext:** AMQP 1.0 ist die OASIS-Enterprise-Messaging-Spec
(RabbitMQ/ActiveMQ/Solace). ZeroDDS implementiert den **Type System**
(`amqp-1.0-types`), das **Frame-Format** (`amqp-1.0-transport` §2.3),
die **Performatives** (§2.7) und die **Message-Sektionen**
(`amqp-1.0-messaging` §3) als pure-Rust no_std+alloc Library
(`crates/amqp-bridge`); der **SASL-Handshake** (`amqp-1.0-security` §5)
liegt in `crates/amqp-endpoint`.

Implementation:

- `crates/amqp-bridge/` — Type System + Frame-Format + Performatives +
  Message-Sektionen, 7 Module (`types`, `extended_types`, `frame`,
  `performatives`, `sections`, `codec_profile`, `lib`).
- `crates/amqp-endpoint/src/sasl.rs` — SASL-Mechanismen
  (PLAIN/ANONYMOUS/EXTERNAL) + Outcome-Codes (§5), 25 Tests.

Test-Lauf: `cargo test -p zerodds-amqp-bridge` — 193 Tests grün, davon
87 lib-Inline-Tests (codec_profile 5, extended_types 29, frame 9,
performatives 16, sections 10, types 18), 90 Integration in
`tests/boundary_decoders.rs` (TS-1-Finding-7-Reaktion: Boundary-
Checks gegen Mutation-Testing), 8 in `tests/fuzz_smoke.rs`, 8 in
`tests/proptest_roundtrip.rs`. Hinweis: `codec_profile.rs`-Tests
gehören zum dds-amqp-Vendor-Profile (Spec-Coverage in
`dds-amqp-1.0.md` §2.3/§2.4), nicht zur OASIS-AMQP-1.0.

---

## amqp-1.0-overview

### §1 Foreword + §2 Conformance

**Spec:** Overview Document — beschreibt die Modell-Schichten
(Network → Frame → Performative → Message).

**Repo:** Crate-Doc.

**Tests:** —

**Status:** n/a (informative)

---

## amqp-1.0-types

### §1.1 Type System (Primitive/Described/Composite/Restricted)

**Spec:** `amqp-1.0-types` §1.1 — vier Type-Kategorien.

**Repo:** Alle vier Kategorien spec-konform abgedeckt:
- **Primitive** (§1.6): vollständig in `types.rs::FormatCode` mit
  allen 30+ Wire-Codes.
- **Described** (§1.3): per-Performative-Encoder in
  `performatives.rs` (`0x00`-Descriptor-Marker + Descriptor-Value);
  generic Described-Composite ist nicht eigenständig exponiert,
  weil DDS-AMQP-Bridge nur fixed Performatives + Type-Liste nutzt.
- **Composite** (§1.4): per-Performative-List-Body in
  `performatives.rs` + Composite-Type-Mapping in
  `crates/amqp-bridge/src/extended_types.rs::§7.2`.
- **Restricted** (§1.5): Domain-Restricted-Types
  (`milliseconds`, `seconds`, `ietf-language-tag`,
  `iso-8859-1`, `symbol`) via `extended_types.rs` mit
  Constraint-Validierung.

**Tests:** Cross-Ref §1.3-§1.6 + Inline-Tests in `performatives.rs`,
`extended_types.rs`, `types.rs`.

**Status:** done — alle vier Type-Kategorien spec-konform; generic
Described-/Composite-Layer ist per-Performative-eingebaut, nicht
generisch (legitime Architektur-Wahl für fixed-Profile-Bridge).

### §1.2 Type Encodings (Fixed/Variable/Compound/Array)

**Spec:** `amqp-1.0-types` §1.2 — Format-Code-Subcategories
(§1.2.1 Fixed Width, §1.2.2 Variable Width, §1.2.3 Compound,
§1.2.4 Array).

**Repo:** `crates/amqp-bridge/src/types.rs::FormatCode::from_byte`
klassifiziert Bytes nach Subkategorie (Spec Table 1-1).

**Tests:** `types::tests::format_code_categorizes_correctly`.

**Status:** done

### §1.3 Type Notation

**Spec:** `amqp-1.0-types` §1.3 — Notation für Spec-internes
Type-Documentation: §1.3.1 Primitive Type Notation,
§1.3.2 Composite Type Notation, §1.3.3 Descriptor Notation,
§1.3.4 Field Notation, §1.3.5 Restricted Type Notation.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Notations-Konvention für die
Spec-PDF; keine Code-Pflicht.

### §1.4 Composite Type Representation

**Spec:** `amqp-1.0-types` §1.4 — wie composite-types als List-
of-fields encoded werden.

**Repo:** `crates/amqp-bridge/src/performatives.rs` und
`crates/amqp-bridge/src/sections.rs` nutzen das Pattern (described-
composite mit Descriptor + List-Body); siehe §2.7 / §3.

**Tests:** Cross-Ref §2.7 und §3.

**Status:** done — Composite-Type-Encoding-Pattern (Descriptor +
List-Body) ist via Performatives + Message-Sections abgedeckt.

### §1.5 Descriptor Values

**Spec:** `amqp-1.0-types` §1.5 — Descriptor-Form: ulong-Code oder
symbolic-Name. Descriptor-Code-Allocation für Spec-Domains.

**Repo:** `crates/amqp-bridge/src/performatives.rs` (Descriptor-Codes
0x10-0x18 für die 9 Performatives), `crates/amqp-bridge/src/sections.rs`
(Codes 0x70-0x78 für Message-Sections).

**Tests:** `performatives::tests::open_descriptor_is_0x10`,
`begin_descriptor_is_0x11`, `all_nine_descriptors_have_unique_values`,
`sections::tests::all_seven_sections_have_unique_order_indexes`.

**Status:** done

### §1.6.1 null

**Spec:** `null` = `0x40` (fixed-0).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_null`.

**Tests:** `types::tests::null_encodes_to_single_byte_0x40`,
`round_trip_all_primitive_values`.

**Status:** done

### §1.6.2 boolean

**Spec:** `boolean` = `0x56` + 1 byte oder kompakt `0x41`/`0x42`.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_boolean` (kompakte
Form); Decoder akzeptiert beide.

**Tests:** `types::tests::boolean_uses_compact_format_codes`,
`round_trip_all_primitive_values`.

**Status:** done

### §1.6.3 ubyte

**Spec:** §1.6.3 — `ubyte` (`0x50`) = "Integer in the range 0 to
2^8 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ubyte,
decode_ubyte}`.

**Tests:** `extended_types::tests::ubyte_round_trips_at_extremes`.

**Status:** done

### §1.6.4 ushort

**Spec:** §1.6.4 — `ushort` (`0x60`) = "Integer in the range 0 to
2^16 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ushort,
decode_ushort}`.

**Tests:** `extended_types::tests::ushort_round_trips`.

**Status:** done

### §1.6.5 uint

**Spec:** §1.6.5 — `uint` mit drei Format-Codes: `uint0` (`0x43`,
fixed-0), `smalluint` (`0x52`, fixed-1), `uint` (`0x70`, fixed-4).
Range 0..2^32-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_uint,
decode_uint}` mit Compact-Selection.

**Tests:** `extended_types::tests::uint_uses_compact_zero_form`,
`extended_types::tests::uint_uses_smalluint_for_low_values`,
`extended_types::tests::uint_uses_full_for_high_values`.

**Status:** done

### §1.6.6 ulong

**Spec:** §1.6.6 — `ulong` mit drei Format-Codes: `ulong0` (`0x44`,
fixed-0), `smallulong` (`0x53`, fixed-1), `ulong` (`0x80`, fixed-8).
Range 0..2^64-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_ulong,
decode_ulong}` mit Compact-Selection.

**Tests:** `types::tests::ulong_zero_uses_ulong0_format`,
`types::tests::ulong_small_uses_smallulong_format`,
`types::tests::ulong_large_uses_full_8_byte_format`.

**Status:** done

### §1.6.7 byte

**Spec:** §1.6.7 — `byte` (`0x51`, fixed-1) = "Integer in the range
-(2^7) to 2^7 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_byte,
decode_byte}`.

**Tests:** `extended_types::tests::byte_round_trips_negative`.

**Status:** done

### §1.6.8 short

**Spec:** §1.6.8 — `short` (`0x61`, fixed-2) = "Integer in the range
-(2^15) to 2^15 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_short,
decode_short}`.

**Tests:** `extended_types::tests::short_round_trips`.

**Status:** done

### §1.6.9 int

**Spec:** §1.6.9 — `int` mit zwei Format-Codes: `smallint` (`0x54`,
fixed-1), `int` (`0x71`, fixed-4). Range -(2^31)..2^31-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_int,
decode_int}` mit Compact-Selection.

**Tests:** `extended_types::tests::int_uses_compact_when_in_byte_range`,
`extended_types::tests::int_uses_full_for_high_values`.

**Status:** done

### §1.6.10 long

**Spec:** §1.6.10 — `long` mit zwei Format-Codes: `smalllong`
(`0x55`, fixed-1), `long` (`0x81`, fixed-8). Range -(2^63)..2^63-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_long,
decode_long}` mit Compact-Selection.

**Tests:** `types::tests::long_small_uses_smalllong_format`,
`types::tests::long_large_uses_full_8_byte_format`.

**Status:** done

### §1.6.11 float

**Spec:** §1.6.11 — `float` (`0x72`, fixed-4) = IEEE 754 binary32.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_float,
decode_float}`.

**Tests:** `extended_types::tests::float_round_trips`.

**Status:** done

### §1.6.12 double

**Spec:** §1.6.12 — `double` (`0x82`, fixed-8) = IEEE 754 binary64.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_double,
decode_double}`.

**Tests:** `extended_types::tests::double_round_trips`.

**Status:** done

### §1.6.13 decimal32

**Spec:** §1.6.13 — `decimal32` (`0x74`, fixed-4) = IEEE 754
decimal32 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal32`
(Encoder, BID-Layout verbatim; Decimal-Arithmetik ist Caller-Layer).

**Tests:** `extended_types::tests::decimal32_64_128_have_correct_lengths`.

**Status:** done — Wire-Format korrekt; Decimal-Arithmetik (z.B.
exact-decimal-roundtrips zwischen Werten) ist Caller-Verantwortung.

### §1.6.14 decimal64

**Spec:** §1.6.14 — `decimal64` (`0x84`, fixed-8) = IEEE 754
decimal64 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal64`.

**Tests:** `extended_types::tests::decimal32_64_128_have_correct_lengths`.

**Status:** done

### §1.6.15 decimal128

**Spec:** §1.6.15 — `decimal128` (`0x94`, fixed-16) = IEEE 754
decimal128 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal128`.

**Tests:** `extended_types::tests::decimal32_64_128_have_correct_lengths`.

**Status:** done

### §1.6.16 char

**Spec:** §1.6.16 — `char` (`0x73`, fixed-4) = "A single Unicode
character" als UTF-32BE-Codepoint.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_char,
decode_char}`.

**Tests:** `extended_types::tests::char_round_trips_ascii_and_unicode`.

**Status:** done

### §1.6.17 Timestamp

**Spec:** `timestamp` = `0x83` + 8-byte BE signed (ms since
1970-01-01 00:00:00).

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_timestamp,
decode_timestamp}`.

**Tests:** `extended_types::tests::timestamp_round_trips_negative`.

**Status:** done

### §1.6.18 Uuid

**Spec:** `uuid` = `0x98` + 16 bytes RFC 4122.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_uuid,
decode_uuid}`.

**Tests:** `extended_types::tests::uuid_round_trips`.

**Status:** done

### §1.6.19 Binary

**Spec:** `binary` = vbin8 (`0xA0` + 1-byte len + bytes) oder vbin32
(`0xB0` + 4-byte BE len + bytes).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_binary` (wählt
Compact-Form basierend auf Length); Decoder für beide Formate.

**Tests:** `types::tests::binary_short_uses_vbin8_format`,
`binary_long_uses_vbin32_format`,
`round_trip_all_primitive_values`,
`truncated_inputs_yield_error`.

**Status:** done

### §1.6.20 String (UTF-8)

**Spec:** `string` = str8-utf8 (`0xA1`) oder str32-utf8 (`0xB1`).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_string`.

**Tests:** `types::tests::string_short_uses_str8_format`,
`string_unicode_round_trip`, `invalid_utf8_in_str_yields_error`.

**Status:** done

### §1.6.21 Symbol (ASCII)

**Spec:** `symbol` = sym8 (`0xA3`) oder sym32 (`0xB3`); MUST be ASCII.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_symbol` lehnt
non-ASCII-Strings ab (`NonAsciiSymbol`-Error).

**Tests:** `types::tests::symbol_short_uses_sym8_format`,
`symbol_rejects_non_ascii`.

**Status:** done

### §1.6.22 list

**Spec:** §1.6.22 — `list` mit drei Format-Codes: `list0` (`0x45`,
fixed-0), `list8` (`0xC0`, compound-1), `list32` (`0xD0`,
compound-4). Generische Liste polymorpher Items.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::
List` mit `encode/decode` und Compact-Selection list0/list8/list32.

**Tests:** `extended_types::tests::empty_list_uses_list0`,
`extended_types::tests::small_list_round_trips`,
`extended_types::tests::nested_list_round_trips`,
`extended_types::tests::deeply_nested_list_exceeds_dos_cap`.

**Status:** done

### §1.6.23 map

**Spec:** §1.6.23 — `map` mit zwei Format-Codes: `map8` (`0xC1`,
compound-1), `map32` (`0xD1`, compound-4). Polymorpher Map mit
Key-Value-Pairs.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Map`
mit `encode/decode` und Compact-Selection.

**Tests:** `extended_types::tests::map_round_trips_with_string_keys`,
`extended_types::tests::encode_map_uses_long_form_when_count_exceeds_u8`.

**Status:** done

### §1.6.24 array

**Spec:** §1.6.24 — `array` mit zwei Format-Codes: `array8` (`0xE0`,
array-1), `array32` (`0xF0`, array-4). Homogene Sequence mit
einheitlichem Element-Constructor.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Array`
mit `encode/decode` und Homogenitäts-Validation.

**Tests:** `extended_types::tests::array_round_trips_homogeneous_short`,
`extended_types::tests::array_homogeneous_constraint_violation_yields_error`,
`extended_types::tests::encode_array_uses_long_form_when_count_exceeds_u8`,
`extended_types::tests::decode_array_at_minimum_buffer_size`.

**Status:** done

Plus DoS-Cap-Schutz auf Recursion-Depth 32 (siehe
`dds-amqp-1.0-beta1.pdf §6.1`):
`extended_types::tests::decode_at_depth_at_cap_accepted`,
`extended_types::tests::decode_at_depth_over_cap_rejected`.

---

## amqp-1.0-transport

### §2.3.1 Frame Format

**Spec:** `amqp-1.0-transport` §2.3.1 — 8-byte Header (SIZE 4-byte BE +
DOFF + TYPE + CHANNEL 2-byte BE) + Extended-Header + Body.

**Repo:** `crates/amqp-bridge/src/frame.rs::{FrameHeader, FrameType,
encode_frame_header, decode_frame_header}`. Pre-Condition-Enforcement:
DOFF >= 2, SIZE >= 8, body_offset <= SIZE.

**Tests:** `frame::tests::frame_type_round_trip`,
`header_round_trip_minimum_size`,
`header_with_channel_42_and_size_1024`,
`body_offset_is_doff_times_4`, `header_too_short_decode_fails`,
`doff_below_2_rejected`, `size_below_8_rejected`,
`body_offset_exceeding_size_rejected`,
`sasl_frame_type_byte_is_1`.

**Status:** done

### §2.3.1.4 Frame Type (AMQP/SASL)

**Spec:** `amqp-1.0-transport` §2.3.1.4 — 0x00 AMQP, 0x01 SASL.

**Repo:** `FrameType::Amqp/Sasl/Reserved`.

**Tests:** `frame::tests::frame_type_round_trip`,
`sasl_frame_type_byte_is_1`.

**Status:** done

### §2.7 Performatives (open/begin/attach/...)

**Spec:** `amqp-1.0-transport` §2.7 — open, begin, attach, flow,
transfer, disposition, detach, end, close.

**Repo:** `crates/amqp-bridge/src/performatives.rs` — alle 9
Performatives als described composites (Descriptor-Marker `0x00` +
Ulong-Descriptor + List-Body). Convenience-Constructor pro
Performative (open, begin, attach, flow, transfer, disposition,
detach, end, close) plus generischer
`encode_performative/decode_performative`.

**Tests:** `performatives::tests::open_descriptor_is_0x10`,
`begin_descriptor_is_0x11`, `attach_carries_role_bit`,
`flow_carries_window_values`, `transfer_default_settled_false`,
`disposition_settled_round_trip`, `detach_round_trips`,
`end_and_close_are_minimal`, `arbitrary_descriptor_round_trip`,
`truncated_input_yields_error`,
`all_nine_descriptors_have_unique_values`.

**Status:** done

---

## amqp-1.0-messaging

### §3 Message Format

**Spec:** `amqp-1.0-messaging` §3 — Header/Delivery-Annotations/
Message-Annotations/Properties/Application-Properties/Body/Footer-
Sektionen.

**Repo:** `crates/amqp-bridge/src/sections.rs::MessageSection` mit
allen 7 Section-Variants (Header/DeliveryAnnotations/
MessageAnnotations/Properties/ApplicationProperties/Data/
AmqpSequence/AmqpValue/Footer) inkl. Roundtrip-Codec und
`validate_section_sequence` für Spec-§3.2-Sequencing-Constraint.

**Tests:** `sections::tests::header_section_round_trips`,
`properties_section_round_trips_with_subject`,
`application_properties_round_trips`,
`data_section_round_trips_binary`, `amqp_value_section_round_trips`,
`amqp_sequence_round_trips`, `footer_section_round_trips`,
`canonical_order_passes_validator`,
`out_of_order_fails_validator`,
`all_seven_sections_have_unique_order_indexes`.

**Status:** done

---

## amqp-1.0-security

### §5 SASL

**Spec:** `amqp-1.0-security` §5 — SASL-Mechanism-List, init,
challenge, response, outcome.

**Repo:** Server-Seite `crates/amqp-endpoint/src/sasl.rs::{SaslMechanism,
SaslState, SaslOutcome, SaslCode}` mit PLAIN/ANONYMOUS/EXTERNAL-
Mechanismen + Outcome-Codes (ok/auth/sys/sys-perm/sys-temp gemäß
§5.3.3.6) + State-Machine (`authenticate_plain`/`authenticate_anonymous`/
`authenticate_external`/`select_outbound`). **Wire-Codec der SASL-
Performatives** (§5.3) in `crates/amqp-bridge/src/performatives.rs`:
`sasl_init`/`sasl_response` (Descriptors 0x41/0x43) + `sasl_plain_response`
(RFC 4616) + `sasl_mechanisms_from_body`/`sasl_outcome_code` (Decode der
0x40/0x44-Frames). **Client-Handshake** `crates/amqp-endpoint/src/client.rs::
AmqpClient::sasl_plain` (SASL-Header → mechanisms → init(PLAIN) → outcome).

**Tests:** Inline-Tests im `sasl`-Modul + `performatives::tests::{
sasl_init_descriptor_and_roundtrip, sasl_mechanisms_extracts_symbols,
sasl_outcome_code_extracted}`; Live `crates/amqp-endpoint/tests/
rabbitmq_amqp10_e2e.rs` (SASL-PLAIN gegen RabbitMQ 4.0).

**Status:** done

---

## RabbitMQ-Interop (AMQP-1.0-Broker-Client)

### Client-Initiator + RabbitMQ-v2-Addressing

**Spec:** `amqp-1.0-transport` §2.4 (connection) + §2.6 (link) +
`amqp-1.0-messaging` §3.2.6 (Data). RabbitMQ 4.0 fährt AMQP 1.0 nativ auf
Port 5672 mit Pflicht-SASL + v2-Adressformat (`/queues/<name>`,
`/exchanges/<name>/<key>`).

**Repo:** `crates/amqp-bridge/src/performatives.rs::{attach_to,
transfer_message, flow_link_credit, disposition_accept,
data_payload_from_transfer}` (attach mit described `source`/`target`-Node
0x28/0x29 + Adresse; transfer mit delivery-tag + Data-Section) +
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Described` (§1.3.4
described type, encode+decode). `crates/amqp-endpoint/src/client.rs::
AmqpClient::{connect_plain, send_to, recv_from}` — voller Client
(SASL → open/begin → attach(sender/receiver) → transfer/flow/disposition →
detach/close).

**Tests:** `crates/amqp-bridge` Unit (`attach_to_carries_target_address`,
`transfer_message_has_tag_and_data`); Live e2e `crates/amqp-endpoint/tests/
rabbitmq_amqp10_e2e.rs` (ZeroDDS-1.0 ⇄ RabbitMQ ⇄ pika-0.9.1, beide
Richtungen) + `rabbitmq_cross_stack_e2e.rs` (ZeroDDS-1.0 ⇄ ZeroDDS-0.9.1).
Harness `crates/amqp-endpoint/competitors/rabbitmq/`.

**Status:** done

---

## Audit-Status

34 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-amqp-bridge` — 87 lib-Tests grün,
0 failed; davon 82 OASIS-AMQP-1.0-relevant
(`extended_types`/`frame`/`performatives`/`sections`/`types`, inkl. der
SASL-Performative- + attach/transfer-Codec-Tests). RabbitMQ-Interop
zusätzlich live: `cargo test -p zerodds-amqp-endpoint --test
rabbitmq_amqp10_e2e -- --ignored` (Linux bench host, `AMQP_RABBITMQ=1`).
