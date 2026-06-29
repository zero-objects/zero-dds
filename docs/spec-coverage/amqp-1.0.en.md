# OASIS AMQP 1.0 — Spec Coverage

**Spec:** [OASIS AMQP Version 1.0 — OASIS Standard →](https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-overview-v1.0-os.html) (overview/types/transport/messaging/security).

**Context:** AMQP 1.0 is the OASIS enterprise-messaging spec
(RabbitMQ/ActiveMQ/Solace). ZeroDDS implements the **type system** (spec
`amqp-1.0-types`), the **frame format** (`amqp-1.0-transport` §2.3), the
**performatives** (§2.7) and the **message sections**
(`amqp-1.0-messaging` §3) as a pure-Rust no_std+alloc library
(`crates/amqp-bridge`); the **SASL handshake** (`amqp-1.0-security` §5)
lives in `crates/amqp-endpoint`.

Implementation:

- `crates/amqp-bridge/` — type system + frame format + performatives +
  message sections, 7 modules (`types`, `extended_types`, `frame`,
  `performatives`, `sections`, `codec_profile`, `lib`).
- `crates/amqp-endpoint/src/sasl.rs` — SASL mechanisms
  (PLAIN/ANONYMOUS/EXTERNAL) + outcome codes (§5), 25 tests.

Test run: `cargo test -p zerodds-amqp-bridge` — 193 tests green, of which 87
lib inline tests (codec_profile 5, extended_types 29, frame 9, performatives
16, sections 10, types 18), 90 integration in `tests/boundary_decoders.rs`
(TS-1-Finding-7 reaction: boundary checks against mutation testing), 8 in
`tests/fuzz_smoke.rs`, 8 in `tests/proptest_roundtrip.rs`. Note: the
`codec_profile.rs` tests belong to the dds-amqp vendor profile (spec coverage
in `dds-amqp-1.0.md` §2.3/§2.4), not to OASIS AMQP 1.0.

---

## amqp-1.0-overview

### §1 Foreword + §2 Conformance

**Spec:** Overview document — describes the model layers (Network → Frame →
Performative → Message).

**Repo:** crate doc.

**Tests:** —

**Status:** n/a (informative)

---

## amqp-1.0-types

### §1.1 Type system (primitive/described/composite/restricted)

**Spec:** `amqp-1.0-types` §1.1 — four type categories.

**Repo:** all four categories covered spec-conformantly:
- **Primitive** (§1.6): complete in `types.rs::FormatCode` with all 30+ wire
  codes.
- **Described** (§1.3): a per-performative encoder in `performatives.rs`
  (`0x00` descriptor marker + descriptor value); a generic described-composite
  is not separately exposed because the DDS-AMQP bridge uses only fixed
  performatives + a type list.
- **Composite** (§1.4): a per-performative list body in `performatives.rs` +
  the composite-type mapping in `crates/amqp-bridge/src/extended_types.rs::§7.2`.
- **Restricted** (§1.5): domain-restricted types (`milliseconds`, `seconds`,
  `ietf-language-tag`, `iso-8859-1`, `symbol`) via `extended_types.rs` with
  constraint validation.

**Tests:** cross-ref §1.3-§1.6 + inline tests in `performatives.rs`,
`extended_types.rs`, `types.rs`.

**Status:** done — all four type categories spec-conformant; the generic
described/composite layer is built per-performative, not generic (a
legitimate architecture choice for a fixed-profile bridge).

### §1.2 Type encodings (fixed/variable/compound/array)

**Spec:** `amqp-1.0-types` §1.2 — format-code subcategories (§1.2.1 Fixed
Width, §1.2.2 Variable Width, §1.2.3 Compound, §1.2.4 Array).

**Repo:** `crates/amqp-bridge/src/types.rs::FormatCode::from_byte`
classifies bytes by subcategory (spec Table 1-1).

**Tests:** `types::tests::format_code_categorizes_correctly`.

**Status:** done

### §1.3 Type notation

**Spec:** `amqp-1.0-types` §1.3 — notation for the spec-internal type
documentation: §1.3.1 Primitive Type Notation, §1.3.2 Composite Type
Notation, §1.3.3 Descriptor Notation, §1.3.4 Field Notation, §1.3.5
Restricted Type Notation.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — notation convention for the spec PDF; no code
requirement.

### §1.4 Composite type representation

**Spec:** `amqp-1.0-types` §1.4 — how composite types are encoded as a list of
fields.

**Repo:** `crates/amqp-bridge/src/performatives.rs` and
`crates/amqp-bridge/src/sections.rs` use the pattern (described-composite with
descriptor + list body); see §2.7 / §3.

**Tests:** cross-ref §2.7 and §3.

**Status:** done — the composite-type encoding pattern (descriptor + list
body) is covered via performatives + message sections.

### §1.5 Descriptor values

**Spec:** `amqp-1.0-types` §1.5 — descriptor form: a ulong code or a symbolic
name. Descriptor-code allocation for spec domains.

**Repo:** `crates/amqp-bridge/src/performatives.rs` (descriptor codes 0x10-0x18
for the 9 performatives), `crates/amqp-bridge/src/sections.rs` (codes 0x70-0x78
for message sections).

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

**Spec:** `boolean` = `0x56` + 1 byte or compact `0x41`/`0x42`.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_boolean` (compact form);
the decoder accepts both.

**Tests:** `types::tests::boolean_uses_compact_format_codes`,
`round_trip_all_primitive_values`.

**Status:** done

### §1.6.3 ubyte

**Spec:** §1.6.3 — `ubyte` (`0x50`) = "Integer in the range 0 to 2^8 - 1
inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ubyte,
decode_ubyte}`.

**Tests:** `extended_types::tests::ubyte_round_trips_at_extremes`.

**Status:** done

### §1.6.4 ushort

**Spec:** §1.6.4 — `ushort` (`0x60`) = "Integer in the range 0 to 2^16 - 1
inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ushort,
decode_ushort}`.

**Tests:** `extended_types::tests::ushort_round_trips`.

**Status:** done

### §1.6.5 uint

**Spec:** §1.6.5 — `uint` with three format codes: `uint0` (`0x43`, fixed-0),
`smalluint` (`0x52`, fixed-1), `uint` (`0x70`, fixed-4). Range 0..2^32-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_uint,
decode_uint}` with compact selection.

**Tests:** `extended_types::tests::uint_uses_compact_zero_form`,
`extended_types::tests::uint_uses_smalluint_for_low_values`,
`extended_types::tests::uint_uses_full_for_high_values`.

**Status:** done

### §1.6.6 ulong

**Spec:** §1.6.6 — `ulong` with three format codes: `ulong0` (`0x44`,
fixed-0), `smallulong` (`0x53`, fixed-1), `ulong` (`0x80`, fixed-8). Range
0..2^64-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_ulong, decode_ulong}`
with compact selection.

**Tests:** `types::tests::ulong_zero_uses_ulong0_format`,
`types::tests::ulong_small_uses_smallulong_format`,
`types::tests::ulong_large_uses_full_8_byte_format`.

**Status:** done

### §1.6.7 byte

**Spec:** §1.6.7 — `byte` (`0x51`, fixed-1) = "Integer in the range -(2^7) to
2^7 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_byte,
decode_byte}`.

**Tests:** `extended_types::tests::byte_round_trips_negative`.

**Status:** done

### §1.6.8 short

**Spec:** §1.6.8 — `short` (`0x61`, fixed-2) = "Integer in the range -(2^15)
to 2^15 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_short,
decode_short}`.

**Tests:** `extended_types::tests::short_round_trips`.

**Status:** done

### §1.6.9 int

**Spec:** §1.6.9 — `int` with two format codes: `smallint` (`0x54`, fixed-1),
`int` (`0x71`, fixed-4). Range -(2^31)..2^31-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_int,
decode_int}` with compact selection.

**Tests:** `extended_types::tests::int_uses_compact_when_in_byte_range`,
`extended_types::tests::int_uses_full_for_high_values`.

**Status:** done

### §1.6.10 long

**Spec:** §1.6.10 — `long` with two format codes: `smalllong` (`0x55`,
fixed-1), `long` (`0x81`, fixed-8). Range -(2^63)..2^63-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_long, decode_long}` with
compact selection.

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

**Spec:** §1.6.13 — `decimal32` (`0x74`, fixed-4) = IEEE 754 decimal32 in BID
layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal32`
(encoder, BID layout verbatim; decimal arithmetic is caller-layer).

**Tests:** `extended_types::tests::decimal32_64_128_have_correct_lengths`.

**Status:** done — wire format correct; decimal arithmetic (e.g.
exact-decimal round-trips between values) is the caller's responsibility.

### §1.6.14 decimal64

**Spec:** §1.6.14 — `decimal64` (`0x84`, fixed-8) = IEEE 754 decimal64 in BID
layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal64`.

**Tests:** `extended_types::tests::decimal32_64_128_have_correct_lengths`.

**Status:** done

### §1.6.15 decimal128

**Spec:** §1.6.15 — `decimal128` (`0x94`, fixed-16) = IEEE 754 decimal128 in
BID layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_decimal128`.

**Tests:** `extended_types::tests::decimal32_64_128_have_correct_lengths`.

**Status:** done

### §1.6.16 char

**Spec:** §1.6.16 — `char` (`0x73`, fixed-4) = "A single Unicode character" as
a UTF-32BE codepoint.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_char,
decode_char}`.

**Tests:** `extended_types::tests::char_round_trips_ascii_and_unicode`.

**Status:** done

### §1.6.17 Timestamp

**Spec:** `timestamp` = `0x83` + 8-byte BE signed (ms since 1970-01-01
00:00:00).

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

**Spec:** `binary` = vbin8 (`0xA0` + 1-byte len + bytes) or vbin32 (`0xB0` +
4-byte BE len + bytes).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_binary` (chooses the
compact form based on length); a decoder for both formats.

**Tests:** `types::tests::binary_short_uses_vbin8_format`,
`binary_long_uses_vbin32_format`, `round_trip_all_primitive_values`,
`truncated_inputs_yield_error`.

**Status:** done

### §1.6.20 String (UTF-8)

**Spec:** `string` = str8-utf8 (`0xA1`) or str32-utf8 (`0xB1`).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_string`.

**Tests:** `types::tests::string_short_uses_str8_format`,
`string_unicode_round_trip`, `invalid_utf8_in_str_yields_error`.

**Status:** done

### §1.6.21 Symbol (ASCII)

**Spec:** `symbol` = sym8 (`0xA3`) or sym32 (`0xB3`); MUST be ASCII.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_symbol` rejects non-ASCII
strings (a `NonAsciiSymbol` error).

**Tests:** `types::tests::symbol_short_uses_sym8_format`,
`symbol_rejects_non_ascii`.

**Status:** done

### §1.6.22 list

**Spec:** §1.6.22 — `list` with three format codes: `list0` (`0x45`,
fixed-0), `list8` (`0xC0`, compound-1), `list32` (`0xD0`, compound-4). A
generic list of polymorphic items.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::List` with
`encode/decode` and compact selection list0/list8/list32.

**Tests:** `extended_types::tests::empty_list_uses_list0`,
`extended_types::tests::small_list_round_trips`,
`extended_types::tests::nested_list_round_trips`,
`extended_types::tests::deeply_nested_list_exceeds_dos_cap`.

**Status:** done

### §1.6.23 map

**Spec:** §1.6.23 — `map` with two format codes: `map8` (`0xC1`, compound-1),
`map32` (`0xD1`, compound-4). A polymorphic map with key-value pairs.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Map` with
`encode/decode` and compact selection.

**Tests:** `extended_types::tests::map_round_trips_with_string_keys`,
`extended_types::tests::encode_map_uses_long_form_when_count_exceeds_u8`.

**Status:** done

### §1.6.24 array

**Spec:** §1.6.24 — `array` with two format codes: `array8` (`0xE0`,
array-1), `array32` (`0xF0`, array-4). A homogeneous sequence with a uniform
element constructor.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Array` with
`encode/decode` and homogeneity validation.

**Tests:** `extended_types::tests::array_round_trips_homogeneous_short`,
`extended_types::tests::array_homogeneous_constraint_violation_yields_error`,
`extended_types::tests::encode_array_uses_long_form_when_count_exceeds_u8`,
`extended_types::tests::decode_array_at_minimum_buffer_size`.

**Status:** done

Plus DoS-cap protection on recursion depth 32 (see `dds-amqp-1.0-beta1.pdf
§6.1`): `extended_types::tests::decode_at_depth_at_cap_accepted`,
`extended_types::tests::decode_at_depth_over_cap_rejected`.

---

## amqp-1.0-transport

### §2.3.1 Frame format

**Spec:** `amqp-1.0-transport` §2.3.1 — an 8-byte header (SIZE 4-byte BE +
DOFF + TYPE + CHANNEL 2-byte BE) + extended header + body.

**Repo:** `crates/amqp-bridge/src/frame.rs::{FrameHeader, FrameType,
encode_frame_header, decode_frame_header}`. Precondition enforcement: DOFF >=
2, SIZE >= 8, body_offset <= SIZE.

**Tests:** `frame::tests::frame_type_round_trip`,
`header_round_trip_minimum_size`, `header_with_channel_42_and_size_1024`,
`body_offset_is_doff_times_4`, `header_too_short_decode_fails`,
`doff_below_2_rejected`, `size_below_8_rejected`,
`body_offset_exceeding_size_rejected`, `sasl_frame_type_byte_is_1`.

**Status:** done

### §2.3.1.4 Frame type (AMQP/SASL)

**Spec:** `amqp-1.0-transport` §2.3.1.4 — 0x00 AMQP, 0x01 SASL.

**Repo:** `FrameType::Amqp/Sasl/Reserved`.

**Tests:** `frame::tests::frame_type_round_trip`, `sasl_frame_type_byte_is_1`.

**Status:** done

### §2.7 Performatives (open/begin/attach/...)

**Spec:** `amqp-1.0-transport` §2.7 — open, begin, attach, flow, transfer,
disposition, detach, end, close.

**Repo:** `crates/amqp-bridge/src/performatives.rs` — all 9 performatives as
described composites (descriptor marker `0x00` + ulong descriptor + list
body). A convenience constructor per performative (open, begin, attach, flow,
transfer, disposition, detach, end, close) plus a generic
`encode_performative/decode_performative`.

**Tests:** `performatives::tests::open_descriptor_is_0x10`,
`begin_descriptor_is_0x11`, `attach_carries_role_bit`,
`flow_carries_window_values`, `transfer_default_settled_false`,
`disposition_settled_round_trip`, `detach_round_trips`,
`end_and_close_are_minimal`, `arbitrary_descriptor_round_trip`,
`truncated_input_yields_error`, `all_nine_descriptors_have_unique_values`.

**Status:** done

---

## amqp-1.0-messaging

### §3 Message format

**Spec:** `amqp-1.0-messaging` §3 —
Header/Delivery-Annotations/Message-Annotations/Properties/Application-Properties/Body/Footer
sections.

**Repo:** `crates/amqp-bridge/src/sections.rs::MessageSection` with all 7
section variants
(Header/DeliveryAnnotations/MessageAnnotations/Properties/ApplicationProperties/Data/AmqpSequence/AmqpValue/Footer)
incl. a round-trip codec and `validate_section_sequence` for the spec-§3.2
sequencing constraint.

**Tests:** `sections::tests::header_section_round_trips`,
`properties_section_round_trips_with_subject`,
`application_properties_round_trips`, `data_section_round_trips_binary`,
`amqp_value_section_round_trips`, `amqp_sequence_round_trips`,
`footer_section_round_trips`, `canonical_order_passes_validator`,
`out_of_order_fails_validator`,
`all_seven_sections_have_unique_order_indexes`.

**Status:** done

---

## amqp-1.0-security

### §5 SASL

**Spec:** `amqp-1.0-security` §5 — SASL mechanism list, init, challenge,
response, outcome.

**Repo:** Server side `crates/amqp-endpoint/src/sasl.rs::{SaslMechanism,
SaslState, SaslOutcome, SaslCode}` with the PLAIN/ANONYMOUS/EXTERNAL mechanisms
+ the outcome codes (ok/auth/sys/sys-perm/sys-temp per §5.3.3.6) + the state
machine (`authenticate_plain`/`authenticate_anonymous`/`authenticate_external`/
`select_outbound`). **Wire codec for the SASL performatives** (§5.3) in
`crates/amqp-bridge/src/performatives.rs`: `sasl_init`/`sasl_response`
(descriptors 0x41/0x43) + `sasl_plain_response` (RFC 4616) +
`sasl_mechanisms_from_body`/`sasl_outcome_code` (decoding the 0x40/0x44 frames).
**Client handshake** `crates/amqp-endpoint/src/client.rs::AmqpClient::sasl_plain`
(SASL header → mechanisms → init(PLAIN) → outcome).

**Tests:** inline tests in the `sasl` module + `performatives::tests::{
sasl_init_descriptor_and_roundtrip, sasl_mechanisms_extracts_symbols,
sasl_outcome_code_extracted}`; live `crates/amqp-endpoint/tests/
rabbitmq_amqp10_e2e.rs` (SASL PLAIN against RabbitMQ 4.0).

**Status:** done

---

## RabbitMQ interop (AMQP 1.0 broker client)

### Client initiator + RabbitMQ v2 addressing

**Spec:** `amqp-1.0-transport` §2.4 (connection) + §2.6 (link) +
`amqp-1.0-messaging` §3.2.6 (Data). RabbitMQ 4.0 speaks AMQP 1.0 natively on
port 5672 with mandatory SASL + the v2 address format (`/queues/<name>`,
`/exchanges/<name>/<key>`).

**Repo:** `crates/amqp-bridge/src/performatives.rs::{attach_to,
transfer_message, flow_link_credit, disposition_accept,
data_payload_from_transfer}` (attach with a described `source`/`target` node
0x28/0x29 + address; transfer with delivery tag + Data section) +
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Described` (§1.3.4
described type, encode+decode). `crates/amqp-endpoint/src/client.rs::AmqpClient::
{connect_plain, send_to, recv_from}` — a full client (SASL → open/begin →
attach(sender/receiver) → transfer/flow/disposition → detach/close).

**Tests:** `crates/amqp-bridge` unit (`attach_to_carries_target_address`,
`transfer_message_has_tag_and_data`); live e2e `crates/amqp-endpoint/tests/
rabbitmq_amqp10_e2e.rs` (ZeroDDS 1.0 ⇄ RabbitMQ ⇄ pika 0.9.1, both directions)
+ `rabbitmq_cross_stack_e2e.rs` (ZeroDDS 1.0 ⇄ ZeroDDS 0.9.1). Harness
`crates/amqp-endpoint/competitors/rabbitmq/`.

**Status:** done

---

## Audit status

34 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-amqp-bridge` — 87 lib tests green, 0 failed;
of which 82 OASIS-AMQP-1.0-relevant
(`extended_types`/`frame`/`performatives`/`sections`/`types`, including the
SASL-performative + attach/transfer codec tests). RabbitMQ interop additionally
live: `cargo test -p zerodds-amqp-endpoint --test rabbitmq_amqp10_e2e --
--ignored` (Linux bench host, `AMQP_RABBITMQ=1`).
