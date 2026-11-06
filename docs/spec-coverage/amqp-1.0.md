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

### §0.1, §0.3, §0.4 Einleitung, Verweise, Danksagung, Historie

**Spec:** `amqp-1.0-overview` §0.1 (Introduction, Terminology, Normative
und Non-normative References), §0.3 Acknowledgements, §0.4 Revision
History — Schichtenmodell (Network → Frame → Performative → Message),
RFC-2119-Schlüsselwörter, Verweislisten.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Überblicksprosa, Verweislisten,
Danksagung und Dokumenthistorie; die einzige Anforderung aus §0.1
(Senden und Empfangen in der Standard-Kodierung) steht im Item §0.2.

### §0.2 Conformance

**Spec:** `amqp-1.0-overview` §0.1/§0.2 — "Every compliant AMQP process
MUST be able to send and receive messages in this standard encoding." —
"A conformant implementation MUST perform protocol negotiation (see
section 2.2), and then parse, process, and produce frames in accordance
with the format and semantics defined in parts 1 through 5 of this
specification."

**Repo:** `crates/amqp-endpoint/src/client.rs` (`AMQP_HEADER`,
`SASL_HEADER`, Protokoll-Header-Austausch), `crates/amqp-endpoint/src/broker.rs`
(Protokoll-Header-Austausch), `crates/amqp-bridge/` (Parts 1–3),
`crates/amqp-endpoint/src/sasl.rs` (Part 5 SASL).

**Tests:** `crates/amqp-endpoint/src/session.rs::tests::amqp10_0_2_header_exchange_advances_to_hdr_exch`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name`; RabbitMQ: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::{amqp10_0_2_a_message_the_client_sends_is_read_by_pika, amqp10_0_2_the_client_receives_what_pika_published}`.

**Status:** partial — Protokoll-Aushandlung, Parts 1–3 und Part 5
(SASL) sind umgesetzt (Items der Parts 1–3 und 5); es fehlt Part 4 (Transactions),
eigenes Item.

### §0.2 Conformance — Part 4 Transactions

**Spec:** `amqp-1.0-overview` §0.2 — Konformität verlangt die Parts 1
bis 5; Part 4 (`amqp-core-transactions`) definiert Transaktionen
(Coordinator, `declare`, `discharge`, transaktionale Zustände).

**Repo:** —

**Tests:** —

**Status:** open — kein Transaktions-Code in `crates/amqp-bridge` oder
`crates/amqp-endpoint`; der Text von Part 4 liegt nicht im
Standards-Cache.

---

## amqp-1.0-types

### §1.1 Type System (Primitive/Described/Composite/Restricted)

**Spec:** `amqp-1.0-types` §1.1 — vier Type-Kategorien.

**Repo:** Alle vier Kategorien spec-konform abgedeckt:
- **Primitive** (§1.6): vollständig in `crates/amqp-bridge/src/types.rs::FormatCode` mit
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

**Tests:** Primitive: `crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_5_a_uint_is_uint0_smalluint_or_four_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`. Described: `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`. Composite: `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`. Restricted: `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`.

**Status:** done — alle vier Type-Kategorien spec-konform; generic
Described-/Composite-Layer ist per-Performative-eingebaut, nicht
generisch (legitime Architektur-Wahl für fixed-Profile-Bridge).

### §1.2 Type Encodings (Fixed/Variable/Compound/Array)

**Spec:** `amqp-1.0-types` §1.2 — Format-Code-Subcategories
(§1.2.1 Fixed Width, §1.2.2 Variable Width, §1.2.3 Compound,
§1.2.4 Array).

**Repo:** `crates/amqp-bridge/src/types.rs::FormatCode::from_byte`
klassifiziert Bytes nach Subkategorie (Spec Table 1-1).

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_2_the_high_nibble_names_the_subcategory`, `crates/amqp-bridge/src/types.rs::tests::amqp10_1_2_an_unassigned_code_or_a_cut_encoding_is_refused`.

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

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_3_a_sender_attach_names_its_target_address` (a described list inside the attach list), `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_4_the_properties_are_0x73_and_a_list_in_field_order`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`.

**Status:** done — Composite-Type-Encoding-Pattern (Descriptor +
List-Body) ist via Performatives + Message-Sections abgedeckt.

### §1.5 Descriptor Values

**Spec:** `amqp-1.0-types` §1.5 — Descriptor-Form: ulong-Code oder
symbolic-Name. Descriptor-Code-Allocation für Spec-Domains.

**Repo:** `crates/amqp-bridge/src/performatives.rs` (Descriptor-Codes
0x10-0x18 für die 9 Performatives), `crates/amqp-bridge/src/sections.rs`
(Codes 0x70-0x78 für Message-Sections).

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_1_the_header_is_0x70_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_9_the_footer_is_0x78_and_a_map`.

**Status:** done

### §1.6.1 null

**Spec:** `null` = `0x40` (fixed-0).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_null`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_1_null_is_0x40`.

**Status:** done

### §1.6.2 boolean

**Spec:** `boolean` = `0x56` + 1 byte oder kompakt `0x41`/`0x42`.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_boolean` (kompakte
Form); Decoder akzeptiert beide.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_2_a_boolean_is_0x41_0x42_or_0x56_and_an_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_whose_compact_encodings_differ_share_the_wide_one`.

**Status:** done

### §1.6.3 ubyte

**Spec:** §1.6.3 — `ubyte` (`0x50`) = "Integer in the range 0 to
2^8 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ubyte,
decode_ubyte}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_3_a_ubyte_is_0x50_and_one_octet`.

**Status:** done

### §1.6.4 ushort

**Spec:** §1.6.4 — `ushort` (`0x60`) = "Integer in the range 0 to
2^16 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ushort,
decode_ushort}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_4_a_ushort_is_0x60_and_two_octets`.

**Status:** done

### §1.6.5 uint

**Spec:** §1.6.5 — `uint` mit drei Format-Codes: `uint0` (`0x43`,
fixed-0), `smalluint` (`0x52`, fixed-1), `uint` (`0x70`, fixed-4).
Range 0..2^32-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_uint,
decode_uint}` mit Compact-Selection.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_5_a_uint_is_uint0_smalluint_or_four_octets`.

**Status:** done

### §1.6.6 ulong

**Spec:** §1.6.6 — `ulong` mit drei Format-Codes: `ulong0` (`0x44`,
fixed-0), `smallulong` (`0x53`, fixed-1), `ulong` (`0x80`, fixed-8).
Range 0..2^64-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_ulong, decode_value}` mit Compact-Selection.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`.

**Status:** done

### §1.6.7 byte

**Spec:** §1.6.7 — `byte` (`0x51`, fixed-1) = "Integer in the range
-(2^7) to 2^7 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_byte,
decode_byte}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_7_a_byte_is_0x51_and_one_signed_octet`.

**Status:** done

### §1.6.8 short

**Spec:** §1.6.8 — `short` (`0x61`, fixed-2) = "Integer in the range
-(2^15) to 2^15 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_short,
decode_short}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_8_a_short_is_0x61_and_two_octets`.

**Status:** done

### §1.6.9 int

**Spec:** §1.6.9 — `int` mit zwei Format-Codes: `smallint` (`0x54`,
fixed-1), `int` (`0x71`, fixed-4). Range -(2^31)..2^31-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_int,
decode_int}` mit Compact-Selection.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_9_an_int_is_smallint_or_four_octets`.

**Status:** done

### §1.6.10 long

**Spec:** §1.6.10 — `long` mit zwei Format-Codes: `smalllong`
(`0x55`, fixed-1), `long` (`0x81`, fixed-8). Range -(2^63)..2^63-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_long, decode_value}` mit Compact-Selection.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_10_a_long_is_smalllong_or_eight_octets`.

**Status:** done

### §1.6.11 float

**Spec:** §1.6.11 — `float` (`0x72`, fixed-4) = IEEE 754 binary32.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_float,
decode_float}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_11_a_float_is_0x72_and_binary32`.

**Status:** done

### §1.6.12 double

**Spec:** §1.6.12 — `double` (`0x82`, fixed-8) = IEEE 754 binary64.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_double,
decode_double}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_12_a_double_is_0x82_and_binary64`.

**Status:** done

### §1.6.13 decimal32

**Spec:** §1.6.13 — `decimal32` (`0x74`, fixed-4) = IEEE 754
decimal32 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal32`
(Encoder, BID-Layout verbatim; Decimal-Arithmetik ist Caller-Layer).

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`.

**Status:** done — Wire-Format korrekt; Decimal-Arithmetik (z.B.
exact-decimal-roundtrips zwischen Werten) ist Caller-Verantwortung.

### §1.6.14 decimal64

**Spec:** §1.6.14 — `decimal64` (`0x84`, fixed-8) = IEEE 754
decimal64 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal64`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`.

**Status:** done

### §1.6.15 decimal128

**Spec:** §1.6.15 — `decimal128` (`0x94`, fixed-16) = IEEE 754
decimal128 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal128`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`.

**Status:** done

### §1.6.16 char

**Spec:** §1.6.16 — `char` (`0x73`, fixed-4) = "A single Unicode
character" als UTF-32BE-Codepoint.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_char,
decode_char}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_16_a_char_is_0x73_and_a_utf_32be_code_point`.

**Status:** done

### §1.6.17 Timestamp

**Spec:** `timestamp` = `0x83` + 8-byte BE signed (ms since
1970-01-01 00:00:00).

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_timestamp,
decode_timestamp}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`.

**Status:** done

### §1.6.18 Uuid

**Spec:** `uuid` = `0x98` + 16 bytes RFC 4122.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_uuid,
decode_uuid}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets`.

**Status:** done

### §1.6.19 Binary

**Spec:** `binary` = vbin8 (`0xA0` + 1-byte len + bytes) oder vbin32
(`0xB0` + 4-byte BE len + bytes).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_binary` (wählt
Compact-Form basierend auf Length); Decoder für beide Formate.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_19_binary_is_vbin8_or_vbin32`, `crates/amqp-bridge/src/types.rs::tests::amqp10_1_2_an_unassigned_code_or_a_cut_encoding_is_refused`.

**Status:** done

### §1.6.20 String (UTF-8)

**Spec:** `string` = str8-utf8 (`0xA1`) oder str32-utf8 (`0xB1`).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_string`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_20_a_string_is_its_utf_8_octets_after_the_length`, `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_20_octets_that_are_not_utf_8_are_refused`.

**Status:** done

### §1.6.21 Symbol (ASCII)

**Spec:** `symbol` = sym8 (`0xA3`) oder sym32 (`0xB3`); MUST be ASCII.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_symbol` lehnt
non-ASCII-Strings ab (`NonAsciiSymbol`-Error).

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_21_a_symbol_is_ascii_after_sym8_or_sym32`.

**Status:** done

### §1.6.22 list

**Spec:** §1.6.22 — `list` mit drei Format-Codes: `list0` (`0x45`,
fixed-0), `list8` (`0xC0`, compound-1), `list32` (`0xD0`,
compound-4). Generische Liste polymorpher Items.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::
List` mit `encode/decode` und Compact-Selection list0/list8/list32.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_22_the_empty_list_is_list0`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_22_a_list_is_list8_size_count_and_items`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`.

**Status:** done

### §1.6.23 map

**Spec:** §1.6.23 — `map` mit zwei Format-Codes: `map8` (`0xC1`,
compound-1), `map32` (`0xD1`, compound-4). Polymorpher Map mit
Key-Value-Pairs.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Map`
mit `encode/decode` und Compact-Selection.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_23_a_map_is_map8_size_count_and_pairs`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_23_a_map_of_more_than_255_elements_is_map32`.

**Status:** done

### §1.6.24 array

**Spec:** §1.6.24 — `array` mit zwei Format-Codes: `array8` (`0xE0`,
array-1), `array32` (`0xF0`, array-4). Homogene Sequence mit
einheitlichem Element-Constructor.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Array`
mit `encode/decode` und Homogenitäts-Validation.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_is_one_constructor_then_the_values`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_whose_compact_encodings_differ_share_the_wide_one`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_of_different_types_are_refused`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_more_than_255_elements_is_array32`.

**Status:** done

Plus DoS-Cap-Schutz auf Recursion-Depth 32 (siehe
`dds-amqp-1.0-beta1.pdf §6.1`):
`extended_types::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`,
`extended_types::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`.

---

## amqp-1.0-transport

### §2.3.1 Frame Format

**Spec:** `amqp-1.0-transport` §2.3.1 — 8-byte Header (SIZE 4-byte BE +
DOFF + TYPE + CHANNEL 2-byte BE) + Extended-Header + Body.

**Repo:** `crates/amqp-bridge/src/frame.rs::{FrameHeader, FrameType,
encode_frame_header, decode_frame_header}`. Pre-Condition-Enforcement:
DOFF >= 2, SIZE >= 8, body_offset <= SIZE.

**Tests:** `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_the_smallest_frame_is_its_eight_octet_header`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_size_and_channel_are_in_network_byte_order`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_the_body_begins_doff_times_four_octets_in`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_fewer_than_eight_octets_are_no_header`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_a_doff_below_2_is_refused`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_a_size_below_8_is_refused`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_a_body_offset_beyond_size_is_refused`.

**Status:** done

### §2.3.1.4 Frame Type (AMQP/SASL)

**Spec:** `amqp-1.0-transport` §2.3.1.4 — 0x00 AMQP, 0x01 SASL.

**Repo:** `FrameType::Amqp/Sasl/Reserved`.

**Tests:** `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_the_type_octet_is_0x00_for_amqp_and_0x01_for_sasl`.

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

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_1_open_is_its_container_id_and_the_fields_after_it`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_2_begin_is_channel_and_the_three_numbers`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_3_attach_is_name_handle_and_role`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_4_flow_is_the_windows_then_handle_count_and_credit`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_5_transfer_is_handle_id_tag_format_settled_then_the_message`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_6_disposition_is_role_first_last_settled_and_state`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_7_detach_end_and_close_are_their_fields`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_2_a_performative_without_the_described_prefix_is_refused`.

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

**Tests:** `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_1_the_header_is_0x70_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_4_the_properties_are_0x73_and_a_list_in_field_order`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_5_the_application_properties_are_0x74_and_a_map`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_6_data_is_0x75_and_a_binary`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_7_an_amqp_sequence_is_0x76_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_8_an_amqp_value_is_0x77_and_any_value`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_9_the_footer_is_0x78_and_a_map`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_the_sections_follow_in_their_order`.

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

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::{amqp10_5_3_3_1_the_mechanism_names_are_plain_anonymous_external, amqp10_5_plain_authenticates_on_correct_credentials, amqp10_5_plain_fails_on_wrong_credentials, amqp10_5_anonymous_authenticates_without_credentials, amqp10_5_external_uses_transport_subject, amqp10_5_3_3_6_the_sasl_codes_are_0_to_4, amqp10_5_sasl_code_unknown_value_rejected}`; `crates/amqp-bridge/src/performatives.rs::tests::{amqp10_5_3_3_2_sasl_init_is_mechanism_response_and_hostname, amqp10_5_3_3_1_the_offered_mechanisms_are_read_from_an_array_or_one_symbol, amqp10_5_3_3_5_the_outcome_code_is_read}`; RabbitMQ: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::amqp10_0_2_a_message_the_client_sends_is_read_by_pika` (SASL PLAIN).

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
AmqpClient::{connect_plain, send_to, send_to_accepted, recv_from}` — voller Client
(SASL → open/begin → attach(sender/receiver) → transfer/flow/disposition →
detach/close).

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::{amqp10_2_7_3_a_sender_attach_names_its_target_address, amqp10_2_7_5_transfer_is_handle_id_tag_format_settled_then_the_message, amqp10_2_7_3_attach_is_name_handle_and_role, amqp10_2_7_3_an_unsettled_sender_attach_says_so, amqp10_2_7_6_a_received_disposition_reads_its_range_and_state}`, `crates/amqp-endpoint/src/client.rs::tests::amqp10_3_4_2_send_to_accepted_returns_on_the_accepting_disposition`; RabbitMQ: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::{amqp10_0_2_a_message_the_client_sends_is_read_by_pika, amqp10_0_2_the_client_receives_what_pika_published, amqp10_3_4_2_an_accepted_send_is_in_the_queue_when_it_returns}`.

**Status:** done

---

## Audit-Status

34 done / 1 partial / 1 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document amqp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-bridge --all-targets` (Job `merge-rust-amqp-bridge`) — 67 Tests grün.
* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (Job `merge-rust-amqp-endpoint`) — 10 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (Job `integration-bridges-vendor-amqp`) — 3 Tests grün.

80 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `amqp-1.0.open.md`.
