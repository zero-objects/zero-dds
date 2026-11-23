# OASIS AMQP 1.0 — Spec Coverage

**Spec:** [OASIS AMQP Version 1.0 — OASIS Standard →](https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-overview-v1.0-os.html) (overview/types/transport/messaging/security).

**Context:** AMQP 1.0 is the OASIS enterprise-messaging spec
(RabbitMQ/ActiveMQ/Solace). ZeroDDS implements the **type system** (spec
`amqp-1.0-types`), the **frame format** (`amqp-1.0-transport` §2.3), the
**performatives** (§2.7) and the **message sections**
(`amqp-1.0-messaging` §3) as a pure-Rust no_std+alloc library
(`crates/amqp-bridge`); **connection, sessions and links**
(`amqp-1.0-transport` §2.2, §2.4 to §2.6), **nodes, delivery states,
source and target** (`amqp-1.0-messaging` §3.3 to §3.5) and the **SASL
handshake** (`amqp-1.0-security` §5) live in `crates/amqp-endpoint` and
`tools/amqp-dds-endpoint`, as do the **transactions**
(`amqp-1.0-transactions`, Part 4) with both roles.

Implementation:

- `crates/amqp-bridge/` — type system, descriptors, frame format,
  performatives, message sections (`types`, `extended_types`,
  `descriptors`, `frame`, `performatives`, `sections`, `codec_profile`).
- `crates/amqp-endpoint/src/peer.rs` — the answering side of a connection
  as a state machine without I/O (Part 2 §2.4 to §2.6, §2.8);
  `terminus.rs` — error, delivery states, source and target (Part 2
  §2.8.14, Part 3 §3.4, §3.5).
- `tools/amqp-dds-endpoint/` — the listener on it (`handler.rs`,
  `frame_io.rs`, `nodes.rs`).
- `crates/amqp-endpoint/src/{broker.rs, client.rs}` — the client.
- `crates/amqp-endpoint/src/{sasl.rs, scram.rs}` — SASL (§5).
- `crates/amqp-endpoint/src/{transactions.rs, txn_resource.rs, txn.rs}` —
  the types of Part 4, the transactions of a resource, the controller; the
  resource in the listener (`handler.rs`).

Test run on the Linux test host (`cargo nextest run --profile merge -p
<crate> --all-targets`): `zerodds-amqp-bridge` 234 green, 1 skipped;
`zerodds-amqp-endpoint` 408 green, 15 skipped (`#[ignore]`);
`amqp-dds-endpoint` 180 green, 8 skipped (`#[ignore]`). The skipped tests
with `--run-ignored all`: `--test proton_e2e` 6 of 6 and
`--test proton_txn_e2e` 2 of 2 with python3-qpid-proton 0.37.0;
`--test artemis_txn_e2e` 3 of 3 against Apache ActiveMQ Artemis 2.44.0 on
OpenJDK 21; `--test rabbitmq_amqp10_e2e` 3 of 3 and
`--test bridge_rabbitmq_e2e` 7 of 9 against RabbitMQ 4.0.5 with pika 1.3.2,
the two that are not green
(`ddsamqp_7_7_2_an_amqp_dispose_reaches_a_cyclone_reader`,
`ddsamqp_7_7_2_an_amqp_dispose_reaches_a_foreign_reader`) need the Python
binding of Cyclone DDS and `ZERODDS_DISPOSE_READER`, which the host does
not have (job `vendor-live-dispose-bridges`). Note: the `codec_profile.rs` tests
belong to the dds-amqp vendor profile (`dds-amqp-1.0.md` §2.3/§2.4), not
to OASIS AMQP 1.0.

---

## amqp-1.0-overview

### §0.1, §0.3, §0.4 Introduction, references, acknowledgements, history

**Spec:** `amqp-1.0-overview` §0.1 (Introduction, Terminology, Normative
and Non-normative References), §0.3 Acknowledgements, §0.4 Revision
History — layer model (Network → Frame → Performative → Message),
RFC 2119 keywords, reference lists.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — overview prose, reference lists,
acknowledgements and document history; the only requirement in §0.1
(send and receive in the standard encoding) is in item §0.2.

### §0.2 Conformance

**Spec:** `amqp-1.0-overview` §0.1/§0.2 — "Every compliant AMQP process
MUST be able to send and receive messages in this standard encoding." —
"A conformant implementation MUST perform protocol negotiation (see
section 2.2), and then parse, process, and produce frames in accordance
with the format and semantics defined in parts 1 through 5 of this
specification."

**Repo:** answering side (TCP server): `tools/amqp-dds-endpoint/src/handler.rs::{handle_connection, do_amqp_phase}` on
`crates/amqp-endpoint/src/peer.rs::Peer` (Part 2) with
`crates/amqp-endpoint/src/terminus.rs` (Part 3 §3.4, §3.5) and
`tools/amqp-dds-endpoint/src/nodes.rs` (Part 3 §3.3). Client:
`crates/amqp-endpoint/src/broker.rs`, `crates/amqp-endpoint/src/client.rs`.
Codec: `crates/amqp-bridge/` (Parts 1 to 3). Part 5:
`crates/amqp-endpoint/src/{sasl.rs, scram.rs}`, `tools/amqp-dds-endpoint/src/tls.rs`.

**Tests:** `crates/amqp-endpoint/src/session.rs::tests::amqp10_0_2_header_exchange_advances_to_hdr_exch`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_version_is_answered_with_the_supported_header`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`; Qpid Proton against the listener: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`, `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_14_proton_sends_a_message_above_the_frame_size`; RabbitMQ against the client: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::{amqp10_0_2_a_message_the_client_sends_is_read_by_pika, amqp10_0_2_the_client_receives_what_pika_published}`.

**Status:** partial — built are the protocol negotiation (§2.2), Part 1,
Part 2 on the answering side and as a client, Part 3, Part 4 and Part 5 with SASL
and TLS from the start of the connection; the items of the Parts state
each section. Missing: what the items §2.5.1 to §2.5.3, §2.6.1, §2.6.3, §2.6.7 to §2.6.10, §2.6.13, §2.8.14, §3.3, §3.4.1, §3.4.5, §3.5.1/§3.5.2, §3.5.3, §3.5.5/§3.5.6, §3.5.9 to §3.5.13, §5.2 name
as missing; SCRAM-SHA-1 (§0.2: "SHOULD support the PLAIN [RFC4616] and
SCRAM-SHA1 [RFC5802] mechanisms"; PLAIN and SCRAM-SHA-256 are built);
Part 4 (Transactions) has items of its own, §4.1 to §4.5.8.

---

## amqp-1.0-types

### §1.1 Type system (primitive/described/composite/restricted)

**Spec:** `amqp-1.0-types` §1.1 — four type categories.

**Repo:** all four categories covered spec-conformantly:
- **Primitive** (§1.6): complete in `crates/amqp-bridge/src/types.rs::FormatCode` with all 30+ wire
  codes.
- **Described** (§1.3): `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::{Described,
  DescribedSymbol}` (`0x00` + descriptor + value, the descriptor a `ulong`
  or a `symbol`, encode and decode); the performatives and sections in
  `performatives.rs` and `sections.rs` build on it.
- **Composite** (§1.4): a per-performative list body in `performatives.rs` +
  the composite-type mapping in `crates/amqp-bridge/src/extended_types.rs::§7.2`.
- **Restricted** (§1.5): domain-restricted types (`milliseconds`, `seconds`,
  `ietf-language-tag`, `iso-8859-1`, `symbol`) via `extended_types.rs` with
  constraint validation.

**Tests:** Primitive: `crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_5_a_uint_is_uint0_smalluint_or_four_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`. Described: `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbolic_descriptor_is_read`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbol_of_the_specification_is_its_numeric_descriptor`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`. Composite: `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`. Restricted: `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`.

**Status:** done — all four categories are written and read; a described
value with a numeric or a symbolic descriptor is a value of the type system
of its own.

### §1.2 Type encodings (fixed/variable/compound/array)

**Spec:** `amqp-1.0-types` §1.2 — format-code subcategories (§1.2.1 Fixed
Width, §1.2.2 Variable Width, §1.2.3 Compound, §1.2.4 Array).

**Repo:** `crates/amqp-bridge/src/types.rs::FormatCode::from_byte`
classifies bytes by subcategory (spec Table 1-1).

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_2_the_high_nibble_names_the_subcategory`, `crates/amqp-bridge/src/types.rs::tests::amqp10_1_2_an_unassigned_code_or_a_cut_encoding_is_refused`.

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

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_3_a_sender_attach_names_its_target_address` (a described list inside the attach list), `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_4_the_properties_are_0x73_and_a_list_in_field_order`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`.

**Status:** done — the composite-type encoding pattern (descriptor + list
body) is covered via performatives + message sections.

### §1.5 Descriptor values

**Spec:** `amqp-1.0-types` §1.5 — descriptor form: a ulong code or a symbolic
name. Descriptor-code allocation for spec domains. "Descriptor values other
than symbolic (symbol) or numeric (ulong) are, while not syntactically
invalid, reserved". §1.3.3: every type of the specification has both
descriptors (`amqp:open:list` and `0x00000000:0x00000010`).

**Repo:** `crates/amqp-bridge/src/descriptors.rs` (`AMQP_DESCRIPTORS`,
`code_of`, `symbol_of`: the symbolic and the numeric descriptor of the 35
described types of Parts 2, 3 and 5),
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::{Described,
DescribedSymbol}` (a received descriptor is a `ulong` or a `symbol`; a symbol
of the table becomes the numeric descriptor of the same type, any other
symbol stays `DescribedSymbol`; a descriptor of any other type is
`TypeError::ReservedDescriptor`),
`crates/amqp-bridge/src/performatives.rs::decode_performative` (a frame body
in both forms; an unknown symbol is `TypeError::UnknownDescriptor`),
`crates/amqp-bridge/src/sections.rs::MessageSection::decode` (through
`decode_performative`). The numeric form is what is written (descriptor
codes 0x10-0x18 of the performatives, 0x70-0x78 of the sections).

**Tests:** `crates/amqp-bridge/src/descriptors.rs::tests::amqp10_1_5_every_descriptor_is_in_the_amqp_domain_and_named_once`, `crates/amqp-bridge/src/descriptors.rs::tests::amqp10_1_5_a_symbol_and_its_code_name_the_same_type`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbolic_descriptor_is_read`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbol_of_the_specification_is_its_numeric_descriptor`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_descriptor_of_another_type_is_refused_as_reserved`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_open_by_its_symbol_is_open_by_its_code`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_frame_body_under_an_unknown_symbol_is_refused_by_name`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_1_5_a_section_by_its_symbol_is_the_section_by_its_code`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_1_the_header_is_0x70_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_9_the_footer_is_0x78_and_a_map`.

**Status:** done — both descriptor forms are read, the numeric one is
written.

### §1.6.1 null

**Spec:** `null` = `0x40` (fixed-0).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_null`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_1_null_is_0x40`.

**Status:** done

### §1.6.2 boolean

**Spec:** `boolean` = `0x56` + 1 byte or compact `0x41`/`0x42`.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_boolean` (compact form);
the decoder accepts both.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_2_a_boolean_is_0x41_0x42_or_0x56_and_an_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_whose_compact_encodings_differ_share_the_wide_one`.

**Status:** done

### §1.6.3 ubyte

**Spec:** §1.6.3 — `ubyte` (`0x50`) = "Integer in the range 0 to 2^8 - 1
inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ubyte,
decode_ubyte}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_3_a_ubyte_is_0x50_and_one_octet`.

**Status:** done

### §1.6.4 ushort

**Spec:** §1.6.4 — `ushort` (`0x60`) = "Integer in the range 0 to 2^16 - 1
inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_ushort,
decode_ushort}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_4_a_ushort_is_0x60_and_two_octets`.

**Status:** done

### §1.6.5 uint

**Spec:** §1.6.5 — `uint` with three format codes: `uint0` (`0x43`, fixed-0),
`smalluint` (`0x52`, fixed-1), `uint` (`0x70`, fixed-4). Range 0..2^32-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_uint,
decode_uint}` with compact selection.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_5_a_uint_is_uint0_smalluint_or_four_octets`.

**Status:** done

### §1.6.6 ulong

**Spec:** §1.6.6 — `ulong` with three format codes: `ulong0` (`0x44`,
fixed-0), `smallulong` (`0x53`, fixed-1), `ulong` (`0x80`, fixed-8). Range
0..2^64-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_ulong, decode_value}`
with compact selection.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`.

**Status:** done

### §1.6.7 byte

**Spec:** §1.6.7 — `byte` (`0x51`, fixed-1) = "Integer in the range -(2^7) to
2^7 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_byte,
decode_byte}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_7_a_byte_is_0x51_and_one_signed_octet`.

**Status:** done

### §1.6.8 short

**Spec:** §1.6.8 — `short` (`0x61`, fixed-2) = "Integer in the range -(2^15)
to 2^15 - 1 inclusive."

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_short,
decode_short}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_8_a_short_is_0x61_and_two_octets`.

**Status:** done

### §1.6.9 int

**Spec:** §1.6.9 — `int` with two format codes: `smallint` (`0x54`, fixed-1),
`int` (`0x71`, fixed-4). Range -(2^31)..2^31-1.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_int,
decode_int}` with compact selection.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_9_an_int_is_smallint_or_four_octets`.

**Status:** done

### §1.6.10 long

**Spec:** §1.6.10 — `long` with two format codes: `smalllong` (`0x55`,
fixed-1), `long` (`0x81`, fixed-8). Range -(2^63)..2^63-1.

**Repo:** `crates/amqp-bridge/src/types.rs::{encode_long, decode_value}` with
compact selection.

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

**Spec:** §1.6.13 — `decimal32` (`0x74`, fixed-4) = IEEE 754 decimal32 in BID
layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_decimal32,
decode_decimal32}`, `AmqpExtValue::Decimal32` (the four octets of the BID
layout as they are on the wire; encode and decode).

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_decimal32_is_read_back_octet_for_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_cut_decimal_is_refused`, `crates/amqp-bridge/tests/boundary_decoders.rs::amqp10_1_6_13_the_decimal_decoders_hold_their_bounds`.

**Status:** done — the value is written and read; decimal arithmetic on the
octets is the caller's.

### §1.6.14 decimal64

**Spec:** §1.6.14 — `decimal64` (`0x84`, fixed-8) = IEEE 754 decimal64 in BID
layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_decimal64,
decode_decimal64}`, `AmqpExtValue::Decimal64`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_14_a_decimal64_is_read_back_octet_for_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_cut_decimal_is_refused`, `crates/amqp-bridge/tests/boundary_decoders.rs::amqp10_1_6_13_the_decimal_decoders_hold_their_bounds`.

**Status:** done

### §1.6.15 decimal128

**Spec:** §1.6.15 — `decimal128` (`0x94`, fixed-16) = IEEE 754 decimal128 in
BID layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_decimal128,
decode_decimal128}`, `AmqpExtValue::Decimal128`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_15_a_decimal128_is_read_back_octet_for_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_cut_decimal_is_refused`, `crates/amqp-bridge/tests/boundary_decoders.rs::amqp10_1_6_13_the_decimal_decoders_hold_their_bounds`.

**Status:** done

### §1.6.16 char

**Spec:** §1.6.16 — `char` (`0x73`, fixed-4) = "A single Unicode character" as
a UTF-32BE codepoint.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_char,
decode_char}`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_16_a_char_is_0x73_and_a_utf_32be_code_point`.

**Status:** done

### §1.6.17 Timestamp

**Spec:** `timestamp` = `0x83` + 8-byte BE signed (ms since 1970-01-01
00:00:00).

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

**Spec:** `binary` = vbin8 (`0xA0` + 1-byte len + bytes) or vbin32 (`0xB0` +
4-byte BE len + bytes).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_binary` (chooses the
compact form based on length); a decoder for both formats.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_19_binary_is_vbin8_or_vbin32`, `crates/amqp-bridge/src/types.rs::tests::amqp10_1_2_an_unassigned_code_or_a_cut_encoding_is_refused`.

**Status:** done

### §1.6.20 String (UTF-8)

**Spec:** `string` = str8-utf8 (`0xA1`) or str32-utf8 (`0xB1`).

**Repo:** `crates/amqp-bridge/src/types.rs::encode_string`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_20_a_string_is_its_utf_8_octets_after_the_length`, `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_20_octets_that_are_not_utf_8_are_refused`.

**Status:** done

### §1.6.21 Symbol (ASCII)

**Spec:** `symbol` = sym8 (`0xA3`) or sym32 (`0xB3`); MUST be ASCII.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_symbol` rejects non-ASCII
strings (a `NonAsciiSymbol` error).

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_21_a_symbol_is_ascii_after_sym8_or_sym32`.

**Status:** done

### §1.6.22 list

**Spec:** §1.6.22 — `list` with three format codes: `list0` (`0x45`,
fixed-0), `list8` (`0xC0`, compound-1), `list32` (`0xD0`, compound-4). A
generic list of polymorphic items.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::List` with
`encode/decode` and compact selection list0/list8/list32.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_22_the_empty_list_is_list0`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_22_a_list_is_list8_size_count_and_items`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`.

**Status:** done

### §1.6.23 map

**Spec:** §1.6.23 — `map` with two format codes: `map8` (`0xC1`, compound-1),
`map32` (`0xD1`, compound-4). A polymorphic map with key-value pairs.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Map` with
`encode/decode` and compact selection.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_23_a_map_is_map8_size_count_and_pairs`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_23_a_map_of_more_than_255_elements_is_map32`.

**Status:** done

### §1.6.24 array

**Spec:** §1.6.24 — `array` with two format codes: `array8` (`0xE0`,
array-1), `array32` (`0xF0`, array-4). A homogeneous sequence with a uniform
element constructor.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Array` with
`encode/decode` and homogeneity validation (`array_parts`, `decode_array`).
The element constructor is a format code or, for described elements, `0x00`
+ descriptor + the constructor of the value (§1.2, Figure 1.3), written
once. Each element is read from its own octets, whose number the
constructor states (§1.2). The elements of a constructor without octets
(`0x40` to `0x45`) are counted. The bound on reading (`within`, `charge`):
such elements, and the copies of the shared descriptor that each element of
an array of described values gets, are charged against a spare of
`size_of::<AmqpExtValue>()` octets per octet of the outermost list, map or
array, which all nested values share; a value thus decodes to no more
elements without octets than its outermost compound value has octets, and
what is beyond is refused with `LengthTooLarge`. The empty array is written
with the constructor `null` (`e0 02 00 40`): the value does not keep the
constructor it was read with.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_is_one_constructor_then_the_values`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_whose_compact_encodings_differ_share_the_wide_one`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_of_different_types_are_refused`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_more_than_255_elements_is_array32`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_described_values_shares_descriptor_and_constructor`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_nulls_is_its_count`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_nested_arrays_of_nulls_share_one_bound`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_the_copies_of_a_shared_descriptor_are_bounded`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_a_long_array_is_read_element_by_element`, `crates/amqp-bridge/tests/boundary_decoders.rs::nested_arrays_of_nulls_do_not_multiply_the_bound`, `crates/amqp-bridge/tests/boundary_decoders.rs::nested_arrays_of_nulls_one_above_the_octets_of_the_value_is_err`, `crates/amqp-bridge/tests/boundary_decoders.rs::encode_array_empty_is_the_null_constructor_alone`; a received empty array in a refused attach: `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_a_refused_attach_with_an_empty_array_is_answered`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_a_refused_attach_with_an_empty_array_leaves_the_connection_usable`.

**Status:** done

Plus DoS-cap protection on recursion depth 32 (see `dds-amqp-1.0-beta1.pdf
§6.1`): `extended_types::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`,
`extended_types::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`.

---

## amqp-1.0-transport

### §2.2 Version negotiation

**Spec:** `amqp-1.0-transport` §2.2 — the protocol header `"AMQP" %d0 major
minor revision` (1.0.0) before any frame. "If the requested protocol
version is not supported, the server MUST send a protocol header with a
supported protocol version and then close the socket." — "If the server
cannot parse the protocol header, the server MUST send a valid protocol
header with a supported protocol version and then close the socket." —
"If the incoming and outgoing protocol headers do not match, both peers
MUST close their outgoing stream and SHOULD read the incoming stream
until it is terminated."

**Repo:** server: `tools/amqp-dds-endpoint/src/handler.rs::{handle_connection,
refuse_header}` (a header with another version, another protocol id or
without `AMQP` is answered with the supported header: the SASL header
where SASL is required, else `AMQP%d0.1.0.0`; then the outgoing stream is
closed and the incoming one is read to its end, for at most 1 s and
64 KiB), `tools/amqp-dds-endpoint/src/frame_io.rs::{read_protocol_header,
write_protocol_header, Transport}` (the version is major, minor and
revision). Client: `crates/amqp-endpoint/src/broker.rs::connect` (sends its
header first; a different header from the broker ends the connection
attempt with the received header in the error).

**Tests:** `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_version_is_answered_with_the_supported_header`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_octets_that_are_no_header_are_answered_with_the_header`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_endpoint_that_requires_sasl_answers_with_the_sasl_header`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_header_after_sasl_is_answered_with_the_amqp_header`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_2_the_amqp_protocol_header_is_amqp_0_1_0_0`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_2_another_revision_is_another_version`, `tools/amqp-dds-endpoint/src/handler.rs::tests::amqp10_2_2_a_refused_header_closes_the_connection_in_the_metrics`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name`.

**Status:** done

### §2.3.1 Frame format

**Spec:** `amqp-1.0-transport` §2.3.1 — an 8-byte header (SIZE 4-byte BE +
DOFF + TYPE + CHANNEL 2-byte BE) + extended header + body.

**Repo:** `crates/amqp-bridge/src/frame.rs::{FrameHeader, FrameType,
encode_frame_header, decode_frame_header}`. Precondition enforcement: DOFF >=
2, SIZE >= 8, body_offset <= SIZE.

**Tests:** `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_the_smallest_frame_is_its_eight_octet_header`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_size_and_channel_are_in_network_byte_order`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_the_body_begins_doff_times_four_octets_in`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_fewer_than_eight_octets_are_no_header`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_a_doff_below_2_is_refused`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_a_size_below_8_is_refused`, `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_a_body_offset_beyond_size_is_refused`.

**Status:** done

### §2.3.1.4 Frame type (AMQP/SASL)

**Spec:** `amqp-1.0-transport` §2.3.1.4 — 0x00 AMQP, 0x01 SASL.

**Repo:** `FrameType::Amqp/Sasl/Reserved`.

**Tests:** `crates/amqp-bridge/src/frame.rs::tests::amqp10_2_3_1_the_type_octet_is_0x00_for_amqp_and_0x01_for_sasl`.

**Status:** done

### §2.4.1, §2.4.2 Opening a connection, pipelined open

**Spec:** `amqp-1.0-transport` §2.4.1 — "each peer MUST send an open frame before sending any other frames"; before the negotiation the frame size is 512 and the channel 0; after it the limits both accept. §2.4.2: frames may follow the own `open` before the partner's has arrived.

**Repo:** Answering side: `crates/amqp-endpoint/src/peer.rs::Peer` (`on_open`: `open` on channel 0 only, `container-id` mandatory, a `max-frame-size` below 512 refused; the answer states the limits of `PeerConfig`; the smaller of both sides holds; a frame before `open` and a second `open` close with `amqp:illegal-state`), in the listener `tools/amqp-dds-endpoint/src/handler.rs::do_amqp_phase`. The limits of the listener come from `<limits>` of the configuration (`tools/amqp-dds-endpoint/src/server.rs::ServerConfig::apply_limits`): `max-frame-size`, `max-sessions-per-connection` minus one as `channel-max`, `max-links-per-session` minus one as `begin.handle-max`, `idle-timeout-ms`; one connection more than `max-connections` is closed when it is accepted. Frames a client sends behind its `open` without waiting are processed in their order. Client: `crates/amqp-endpoint/src/broker.rs::open_connection`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_a_frame_before_open_closes_with_illegal_state`, `tools/amqp-dds-endpoint/src/handler.rs::tests::ddsamqp_2_1_handle_connection_open_close_round_trip`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener`, `tools/amqp-dds-endpoint/src/server.rs::tests::ddsamqp_a_limits_that_admit_nothing_are_refused`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`; RabbitMQ (Client): `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::amqp10_0_2_a_message_the_client_sends_is_read_by_pika`.

**Status:** done

### §2.4.3, §2.4.4 Closing a connection, simultaneous close

**Spec:** `amqp-1.0-transport` §2.4.3 — "each peer MUST write a close frame with a code indicating the reason for closing. This frame MUST be the last thing ever written onto a connection." §2.4.4: both sides may close at the same time.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (the partner's `close` is answered with `close`, nothing is written after it; `Peer::close` and `fail_connection` write `close` with the error as the last frame; a `close` of the partner that crosses the own one ends the connection); `do_amqp_phase` waits at most 2 s for the partner's `close` after its own. The links of the connection go back to `Container::detached`. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::close`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_3_close_is_answered_with_close_and_ends_the_connection`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_6_after_a_close_with_an_error_frames_are_discarded_until_close`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_3_a_lost_connection_hands_the_links_back`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`.

**Status:** done

### §2.4.5 Idle timeout of a connection

**Spec:** `amqp-1.0-transport` §2.4.5 — each side states in `open.idle-time-out` the pause it tolerates from its partner ("SHOULD be half the peer's actual timeout threshold"); a side with nothing to send sends an empty frame; on expiry "a peer SHOULD try to gracefully close the connection using a close frame with an error".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`tick`: an empty frame on channel 0 when nothing was sent for half the partner's `idle-time-out`; `close` with `amqp:resource-limit-exceeded` when no frame came for `PeerConfig::idle_time_out_ms`; `open` states half of it; empty frames of the partner count on any channel). Listener: `HandlerConfig::idle_time_out` (from `ServerConfig::read_timeout`, which `<limits idle-timeout-ms>` of the configuration sets; 0: no limit), `poll_interval` 20 ms. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::heartbeat`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_5_idle_time_out_is_kept_in_both_directions`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_4_5_the_listener_sends_heartbeats_and_closes_a_silent_connection`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_4_5_proton_with_an_idle_time_out_is_kept_alive_by_the_endpoint`.

**Status:** done

### §2.4.6, §2.4.7 Connection states

**Spec:** `amqp-1.0-transport` §2.4.6/§2.4.7 — the states START to END; DISCARDING: after a `close` with an error "any incoming frames on the connection MUST be silently discarded until the peer's close frame is received".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`ConnState`: `AwaitOpen` is HDR_EXCH, `Opened`, `Discarding` for CLOSE_SENT and DISCARDING, `End`; in HDR_EXCH the own `open` is sent before a `close`, since the state admits `open` only as a send), `crates/amqp-endpoint/src/session.rs::{ConnectionState, advance_connection}`. The states OPEN_PIPE, OC_PIPE and CLOSE_PIPE belong to a side that pipelines itself; the listener sends its `open` as the answer.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_6_after_a_close_with_an_error_frames_are_discarded_until_close`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_a_frame_before_open_closes_with_illegal_state`, `crates/amqp-endpoint/src/session.rs::tests::amqp10_0_2_header_exchange_advances_to_hdr_exch`.

**Status:** done

### §2.5.1 to §2.5.3 Establishing and ending a session

**Spec:** `amqp-1.0-transport` §2.5.1 — "The remote-channel field of a begin frame MUST be empty for a locally initiated session, and MUST be set when announcing the endpoint created as a result of a remotely initiated session"; the lowest free channel is recommended. §2.5.2/§2.5.3: `end`, also from both sides at once.

**Repo:** Answering side: `crates/amqp-endpoint/src/peer.rs::Peer` (`on_begin`: the answer on the lowest free outgoing channel with `remote-channel`, the windows and `handle-max` of `PeerConfig`; a channel above `channel-max` closes with `amqp:connection:framing-error`; `on_end`: `end` is answered with `end`, the channel is free again). Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::{begin, end}`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_1_begin_is_answered_on_the_lowest_free_channel_with_remote_channel`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`.

**Status:** partial — each side has one role only: `Peer` begins no session itself and closes on a `begin` with `remote-channel` with `amqp:not-allowed`; the client (`broker.rs`) does not answer a `begin` the broker initiates and reports a protocol error.

### §2.5.4, §2.5.5 Session errors, session states

**Spec:** `amqp-1.0-transport` §2.5.4 — "When a session is unable to process input, it MUST indicate this by issuing an END with an appropriate error indicating the cause of the problem. It MUST then proceed to discard all incoming frames from the remote endpoint until receiving the remote endpoint's corresponding end frame." §2.5.5: the states UNMAPPED to DISCARDING.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`fail_session`: `end` with the error, the outgoing channel is free, the frames of the session are discarded until the partner's `end`, its links go to `Container::detached`; other sessions of the connection stay).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_4_a_session_error_ends_the_session_and_discards_until_end`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation`.

**Status:** done

### §2.5.6 Session flow control

**Spec:** `amqp-1.0-transport` §2.5.6 — next-incoming-id, incoming-window, next-outgoing-id, outgoing-window, remote-incoming-window, remote-outgoing-window and their update on sending and receiving `transfer` and `flow`.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_transfer`, `on_flow`, `send`, `session_windows`; the incoming window is widened again once half of it is used; a `transfer` beyond the window ends the session with `amqp:session:window-violation`; the partner's window counts per `transfer` frame: `send` starts a delivery as soon as the window is not zero, writes the frames it admits, and `resume_transfers` writes the others when a `flow` widens it; with a window of zero `SendError::Window`). Listener: `ConnectionNodes::pump`. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::{transfer, apply_session_flow, remote_incoming_window}`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_6_the_windows_follow_transfers_and_flows`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_6_a_delivery_above_the_window_is_continued_after_each_flow`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_5_6_the_listener_sends_a_message_above_the_window_in_steps`, `crates/amqp-endpoint/src/broker.rs::tests::a_full_session_window_refuses_the_transfer`.

**Status:** done

### §2.6.1, §2.6.2 Naming a link, link handles

**Spec:** `amqp-1.0-transport` §2.6.1 — the name identifies the link between two containers per direction; "the second attach succeeds and the first attach MUST then be closed with a link error of stolen". §2.6.2: each side assigns its own handle.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_attach`: the own handle is the lowest free one; a link attached again with the same name and direction on the same connection is closed with `amqp:link:stolen`; a handle in use ends the session with `amqp:session:handle-in-use`, one above `handle-max` the connection with `amqp:connection:framing-error`).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_1_a_link_attached_again_is_stolen`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_7_2_a_handle_above_handle_max_closes_with_framing_error`.

**Status:** partial — `stolen` holds within one connection; a link of the same name the same container attaches over a second connection leaves the first in place (the connections of a listener share no link registry).

### §2.6.3 Establishing or resuming a link

**Spec:** `amqp-1.0-transport` §2.6.3 — `attach` is answered with `attach`; without a terminus: "the session endpoint will still create a link endpoint and issue an attach indicating that the link endpoint has no associated local terminus. In this case, the session endpoint MUST immediately detach the newly created link endpoint." — "the endpoint MUST report the actual properties of the terminus as created" — "If either end of the link is already associated with a terminus, the attach frame MUST include its unsettled delivery state."

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_attach` with `Container::attach`: the answer with the same name, the other role, the termini as created, `initial-delivery-count` as a sender, `max-message-size`; a refused attach is answered with an `attach` without the own terminus and a closing `detach` with the error; an attach with `unsettled` is answered with the empty map). Listener: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::attach` (an address without a node `amqp:not-found`, the access check `amqp:unauthorized-access`, a terminus without an address and a target that is no `target` `amqp:not-implemented`). Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::attach`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_a_refused_attach_is_answered_with_a_null_terminus_and_a_closing_detach`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `tools/amqp-dds-endpoint/src/handler.rs::tests::ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_3_proton_sees_a_refused_attach_as_not_found`.

**Status:** partial — establishing is built. Resuming: the listener keeps no terminus past the end of a link (it reports `expiry-policy` `link-detach`), so its `unsettled` map is always empty; the client never sends `attach.unsettled` and cannot resume a link.

### §2.6.4 to §2.6.6 Detaching, link errors, closing a link

**Spec:** `amqp-1.0-transport` §2.6.5 — "When an error occurs at a link endpoint, the endpoint MUST be detached with appropriate error information supplied in the error field of the detach frame. The link endpoint MUST then be destroyed. If any input (other than a detach) related to the endpoint ... be received, the session MUST be terminated with an errant-link session-error." — "Receipt of a pipelined attach MUST result in the session being terminated with an errant-link session-error." §2.6.4/§2.6.6: `detach`, closing or not.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_detach`: the answer on the own handle, closing when the partner closes; both handles are free again; `detach` and `fail_link`: a closing `detach` with the error, after which any input for the handle other than `detach` ends the session with `amqp:session:errant-link`, and so does a `disposition` whose range names a delivery-id the link held unsettled in either direction, or was in the middle of transferring, when it was destroyed (`on_disposition`; the ids hold until the partner's `detach`); an attach with `unsettled` on the handle is the resumption). Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::detach`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_6_detach_is_answered_and_frees_the_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_a_disposition_for_a_link_detached_with_an_error_ends_the_session`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`.

**Status:** done

### §2.6.7 to §2.6.10 Flow control, synchronous get, asynchronous notification, stopping a link

**Spec:** `amqp-1.0-transport` §2.6.7 — `link-credit(snd) := delivery-count(rcv) + link-credit(rcv) - delivery-count(snd)`; "If the link-credit is less than or equal to zero ... a sender MUST NOT send more messages"; "If the sender's drain flag is set and there are no available messages, the sender MUST advance its delivery-count until link-credit is zero, and send its updated flow state to the receiver." §2.6.8 to §2.6.10: the patterns built on it (one credit, `drain`, a credit window, credit 0 with `echo`).

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` as a sender: `on_flow` (the credit by the formula, without a `delivery-count` with the one of the own attach), `send` (`SendError::NoCredit` without credit), `drained` (the count advanced by the credit left, a `flow` with credit 0; not while a delivery of the link waits for the session window), `echo` is answered with a `flow`; `flow.available` (§2.7.4) is, in every `flow` of a sending link, the count of `Container::available`. As a receiver: credit `PeerConfig::link_credit` with the attach, restored when half is used; a `transfer` without credit detaches the link with `amqp:link:transfer-limit-exceeded`. Listener: `ConnectionNodes::pump` (a link with `drain` is reported drained only when its node has nothing more for it), `ConnectionNodes::available` (the messages of the node the link may get: `Nodes::available`). Client: `crates/amqp-endpoint/src/broker.rs::{BrokerWriter::flow, BrokerWriter::sender_flow, BrokerWriter::session_flow, sender_credit}` (the credit by the formula; a limit behind the own `delivery-count` (RFC 1982) is credit 0; without a `delivery-count` from the `initial-delivery-count` 0 of the own attach). Daemon: `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::on_link_frame` (sending link: credit, `drain`, `echo`; a link attached again counts from 0; receiving link: `on_sender_flow` takes the broker's `delivery-count`, consumed credit is restored when half is used, `echo` is answered with a `flow`; `echo` without a handle with `BrokerWriter::session_flow`), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::{drain, answer_flow}` (with no message available the count advanced by the credit left, a `flow` with credit 0, `available` and `drain`; not while a message waits for the session window; after the stop of §9.2 no message is available), `crates/amqp-endpoint/src/bridge_daemon/cbs.rs::Cbs::on_inbound` (the same for the sender and the receiver of `$cbs`). Transaction controller: `crates/amqp-endpoint/src/txn.rs::TxnController::on_event` (the control link sends only when called, so no message ever waits: `drain` consumes the credit, `echo` is answered), `crates/amqp-endpoint/src/txn.rs::TxnClient` (`dispatch`: sending links drain and answer `echo`, receiving links answer `echo`, `echo` without a handle with the state of the session; `transfer`: a link whose message waits for credit drains only after it).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_sender_sends_within_the_credit_and_drains`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_transfer_without_credit_detaches_with_transfer_limit_exceeded`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_receiver_restores_the_credit_when_half_is_used`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_3_4_the_outcomes_act_on_the_node_and_drain_consumes_the_credit`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_7_4_a_flow_of_the_sender_states_the_messages_that_wait_for_credit`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_link_in_the_middle_of_a_delivery_is_not_drained`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_4_the_listener_states_the_messages_that_wait_for_credit`, `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::{amqp10_2_6_7_a_sending_link_asked_to_drain_consumes_the_credit_it_cannot_use, amqp10_2_7_4_a_flow_with_echo_is_answered_with_the_state_of_the_sending_link, amqp10_2_6_7_a_flow_behind_the_deliveries_sent_grants_no_credit, amqp10_2_6_7_a_link_attached_again_counts_from_its_initial_delivery_count, amqp10_2_7_4_a_flow_with_echo_is_answered_with_the_state_of_the_receiving_link, amqp10_2_6_7_a_receiving_link_takes_the_delivery_count_of_the_brokers_flow, amqpbridge_12_1_link_state_credit_follows_part_2_2_6_7, amqpbridge_9_2_a_link_asked_to_drain_after_the_stop_consumes_its_credit}`, `crates/amqp-endpoint/src/bridge_daemon/cbs.rs::tests::{amqp10_2_6_7_the_cbs_sender_drains_and_answers_an_echo, amqp10_2_6_7_the_cbs_receiver_answers_an_echo_and_follows_the_brokers_count}`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_2_6_7_the_control_link_drains_and_answers_an_echo`, `crates/amqp-endpoint/tests/txn_scripted.rs::amqp10_2_6_7_the_txn_client_drains_its_sending_link_and_answers_echoes`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`. Linux test host: green.

**Status:** partial — the answering side is built on both sides: the listener (`Peer`), the daemon's links (topic links and `$cbs`) and the links of the transaction controller drain on `drain` and answer `echo`, with and without a handle. Open: no client sets `flow.echo`, so a stop with confirmation (§2.6.10) is not possible on the client side; only `TxnClient::receive` sets `flow.drain` (a get with a time limit, §2.6.8), the daemon's receiving links never ask for it. Against a broker that sends `drain` or `echo` itself, the behaviour is measured with the scripted broker only, not against another vendor's broker.

### §2.6.11, §2.6.12 Messages, transferring a message

**Spec:** `amqp-1.0-transport` §2.6.12 — each delivery has a `delivery-tag`; it is unsettled until it is settled; `disposition` carries state and settlement; the receiver settles `first` or `second` (§2.8.3).

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` as a receiver: `on_transfer` hands the delivery to `Container::deliver` and sends its result as a `disposition`, settled in mode `first`, unsettled in mode `second` until the sender settles; a settled `transfer` gets none. As a sender: `send` (settled or not by `snd-settle-mode`), `on_disposition` reports the state to `Container::disposition` and settles a terminal state the receiver left open. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::{transfer, disposition}`, `crates/amqp-endpoint/src/client.rs::AmqpClient::send_to_accepted`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_12_an_unsettled_delivery_is_answered_with_its_outcome`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_12_a_disposition_for_what_this_endpoint_sent_is_settled`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `crates/amqp-endpoint/src/client.rs::tests::amqp10_3_4_2_send_to_accepted_returns_on_the_accepting_disposition`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`.

**Status:** done

### §2.6.13 Resuming deliveries

**Spec:** `amqp-1.0-transport` §2.6.13 — after a reattach both sides reconcile their unsettled deliveries; a delivery sent with `resume` that only the source holds unsettled "MUST be ignored by the receiver".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_transfer`: a `transfer` with `resume` whose tag the receiver does not hold unsettled is not delivered; `LinkInfo::remote_unsettled` hands the partner's map to the container).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_13_a_resumed_delivery_this_endpoint_does_not_hold_is_ignored`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session`.

**Status:** partial — neither side keeps unsettled deliveries past a link: the listener hands them back to the node when the link ends (default outcome `released`), the client sends neither `attach.unsettled` nor `transfer.resume`. Reconciling two non-empty maps and `incomplete-unsettled` are not built.

### §2.6.14 Transferring large messages

**Spec:** `amqp-1.0-transport` §2.6.14 — a message over several `transfer` frames (`more`); "messages transferred along a single link MUST NOT be interleaved"; on `aborted` "the receiver MUST discard the message data that was transferred prior to the abort".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_transfer`: the frames of a delivery are put together, continuation frames may leave out `delivery-id` and tag, `aborted` discards, a second delivery before the end of the first detaches the link with `amqp:not-allowed`, a message above `max-message-size` with `amqp:link:message-size-exceeded`; `send` splits by the partner's frame size; when the partner's session window does not admit all frames, the others follow after the next `flow` (§2.5.6), and until then the link takes no other delivery: `SendError::InProgress`). Client: `crates/amqp-endpoint/src/broker.rs::{BrokerWriter::transfer, BrokerReader::next_event}`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_14_a_message_in_several_transfers_is_one_delivery`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_14_a_message_above_the_frame_size_is_sent_in_several_transfers`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_18_a_message_above_max_message_size_detaches_the_link`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_6_a_delivery_above_the_window_is_continued_after_each_flow`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_5_6_the_listener_sends_a_message_above_the_window_in_steps`, `crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_14_proton_sends_a_message_above_the_frame_size`.

**Status:** done

### §2.7 Performatives (open/begin/attach/...)

**Spec:** `amqp-1.0-transport` §2.7 — open, begin, attach, flow, transfer,
disposition, detach, end, close.

**Repo:** `crates/amqp-bridge/src/performatives.rs` — all 9 performatives as
described composites (descriptor marker `0x00` + ulong descriptor + list
body). A convenience constructor per performative (open, begin, attach, flow,
transfer, disposition, detach, end, close) plus a generic
`encode_performative/decode_performative`.

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_1_open_is_its_container_id_and_the_fields_after_it`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_2_begin_is_channel_and_the_three_numbers`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_3_attach_is_name_handle_and_role`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_4_flow_is_the_windows_then_handle_count_and_credit`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_5_transfer_is_handle_id_tag_format_settled_then_the_message`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_6_disposition_is_role_first_last_settled_and_state`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_7_detach_end_and_close_are_their_fields`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_2_a_performative_without_the_described_prefix_is_refused`.

**Status:** done

### §2.8.1 to §2.8.13 Definitions

**Spec:** `amqp-1.0-transport` §2.8.1 to §2.8.13 — `role`, `sender-settle-mode`, `receiver-settle-mode`, `handle`, `seconds`, `milliseconds`, `delivery-tag` (up to 32 octets), `delivery-number`, `transfer-number`, `sequence-no` (RFC 1982), `message-format`, `ietf-language-tag`, `fields`.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (the role as a boolean, the settle modes 0 to 2 and 0 to 1 with other values refused, handles, a tag above 32 octets refused, counters that wrap by RFC 1982 in `serial_le` and `wrapping_add`, `message-format` passed through). `ietf-language-tag`: the endpoints send no `outgoing-locales`, the default `en-US` holds.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_12_a_disposition_for_what_this_endpoint_sent_is_settled`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_14_a_message_above_the_frame_size_is_sent_in_several_transfers`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_sender_sends_within_the_credit_and_drains`.

**Status:** done

### §2.8.14 Error

**Spec:** `amqp-1.0-transport` §2.8.14 — `error` (0x1d): `condition` (mandatory), `description`, `info`.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{AmqpError, encode_error, decode_error}`.

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_2_8_14_an_error_is_0x1d_condition_and_description`.

**Status:** partial — `info` is neither read nor written; `AmqpError` has no field for it.

### §2.8.15 to §2.8.18 Error conditions

**Spec:** `amqp-1.0-transport` §2.8.15 to §2.8.18 — the symbols of the shared, connection, session and link errors.

**Repo:** `crates/amqp-endpoint/src/peer.rs::condition` (all 25 symbols). `Peer` sends: `amqp:decode-error` (a frame body that does not decode or is no list), `amqp:not-implemented` (an unknown frame body), `amqp:illegal-state`, `amqp:not-allowed`, `amqp:invalid-field`, `amqp:resource-limit-exceeded`, `amqp:connection:framing-error` (channel, handle, frame size, frame type), the four session errors, `amqp:link:transfer-limit-exceeded`, `amqp:link:message-size-exceeded`, `amqp:link:stolen`; the listener adds `amqp:not-found`, `amqp:unauthorized-access`, `amqp:internal-error`, and `amqp:resource-limit-exceeded` for a node bound (§3.3).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_15_an_undecodable_frame_body_closes_with_its_condition`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_18_a_message_above_max_message_size_detaches_the_link`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_4_a_session_error_ends_the_session_and_discards_until_end`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_an_oversized_frame_closes_with_framing_error`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_3_1_a_frame_cut_by_a_read_timeout_is_kept_until_its_rest_arrives`.

**Status:** done

### §2.8.19 Constant definitions

**Spec:** `amqp-1.0-transport` §2.8.19 — PORT 5672, SECURE-PORT 5671, MAJOR 1, MINOR 0, REVISION 0, MIN-MAX-FRAME-SIZE 512.

**Repo:** `crates/amqp-endpoint/src/peer.rs::MIN_MAX_FRAME_SIZE`, `tools/amqp-dds-endpoint/src/frame_io.rs::protocol::{MAJOR, MINOR, REVISION}`, `tools/amqp-dds-endpoint/src/server.rs::ServerConfig::default_listen` (port 5672; the port of a TLS listener comes from the configuration).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_19_the_frame_size_is_never_below_512`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_2_the_amqp_protocol_header_is_amqp_0_1_0_0`, `tools/amqp-dds-endpoint/src/server.rs::tests::server_config_default_has_sensible_values`.

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

**Tests:** `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_1_the_header_is_0x70_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_4_the_properties_are_0x73_and_a_list_in_field_order`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_5_the_application_properties_are_0x74_and_a_map`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_6_data_is_0x75_and_a_binary`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_7_an_amqp_sequence_is_0x76_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_8_an_amqp_value_is_0x77_and_any_value`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_9_the_footer_is_0x78_and_a_map`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_the_sections_follow_in_their_order`.

**Status:** done

### §3.3 Distribution nodes

**Spec:** `amqp-1.0-messaging` §3.3 — a node holds messages and hands them out over links; the states of a message at the node (AVAILABLE, ACQUIRED, ARCHIVED).

**Repo:** `tools/amqp-dds-endpoint/src/nodes.rs::Nodes` (queues by address for all connections of a listener: a message is available until a link takes it, then with that link until a terminal state removes it or puts it back), `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes`. The nodes are bounded (`crates/amqp-endpoint/src/limits.rs::NodeLimits`, the element `<nodes>` of the configuration: `max-nodes` 1024 nodes created by an attach, `max-messages` 10 000 and `max-octets` 64 MiB per node): an attach that would create one node more is refused with `amqp:resource-limit-exceeded`, a `transfer` to a full node is rejected with the same error (`rejected`); a released message goes back to its node whatever the node holds.

**Tests:** `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_3_a_queue_holds_messages_in_order_until_a_link_takes_them`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_2_8_15_nodes_are_bounded_in_number_messages_and_octets`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener`.

**Status:** partial — built are AVAILABLE and ACQUIRED at a queue that hands each message to one link (`move`). The state ARCHIVED is missing: there is no kind of node that keeps a message after it was handed out (`copy`, §3.5.2).

### §3.4.1, §3.4.6 Received, resuming deliveries using delivery states

**Spec:** `amqp-1.0-messaging` §3.4.1 — `received` (0x23): `section-number`, `section-offset`. §3.4.6: resuming a partly transferred message from that point.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{DeliveryState, encode_state, decode_state}` (codec).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_4_1_received_is_0x23_section_number_and_offset`.

**Status:** partial — the codec only. No endpoint sends `received`, and none continues a transfer at a reported point (see §2.6.13).

### §3.4.2 to §3.4.5 Accepted, rejected, released, modified

**Spec:** `amqp-1.0-messaging` §3.4.2 to §3.4.5 — `accepted` (0x24), `rejected` (0x25, `error`), `released` (0x26), `modified` (0x27: `delivery-failed`, `undeliverable-here`, `message-annotations`) and their effect at the source.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{Outcome, encode_outcome, decode_outcome}`; the effect at the node in `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::disposition`: `accepted` and `rejected` remove the message, `released` puts it back, `modified` with `delivery-failed` counts `header.delivery-count` up (`nodes.rs::Message::count_failed_delivery`), with `undeliverable-here` the link does not get it again; a settlement without a terminal state counts as `released` (the default outcome of the source).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::amqp10_3_4_the_outcomes_are_their_described_lists`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_3_4_the_outcomes_act_on_the_node_and_drain_consumes_the_credit`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_4_5_an_undeliverable_here_message_skips_that_link`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_4_5_a_failed_delivery_is_counted_in_the_header`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_3_4_proton_releases_modifies_and_rejects_against_the_endpoint`.

**Status:** partial — `modified.message-annotations` is neither read nor written (`Outcome::Modified` has no field for it); everything else is built.

### §3.5.1, §3.5.2, §3.5.7, §3.5.8 Filtering, distribution modes, filter set

**Spec:** `amqp-1.0-messaging` §3.5.1/§3.5.8 — `filter-set`: named predicates; "the sending endpoint sets the filter actually in place". §3.5.2/§3.5.7: `move` and `copy`.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::Source::{filter, distribution_mode}` (codec; a filter is a described value with a numeric or a symbolic descriptor). The listener offers `move` and no filter and reports both in its source whatever the receiver asks for (`ConnectionNodes::attach`); the receiver decides (§3.5.3: "The receiving endpoint MUST check that the filter in place meets its needs"). The client can request neither a filter nor a distribution mode (§3.5.3).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_source_is_0x28_and_its_fields_in_order`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`.

**Status:** partial — built are the codec and `move` without a filter, reported as such. Missing: the distribution mode `copy` (a kind of node that keeps messages after they were handed out, with the state ARCHIVED of §3.3) and any evaluation of a filter; a requested filter and `copy` are not granted (the test `amqp10_2_6_12_…` requests both and checks that `move` and no filter are reported).

### §3.5.3, §3.5.4 Source, target

**Spec:** `amqp-1.0-messaging` §3.5.3 — `source` (0x28) with eleven fields; §3.5.4 — `target` (0x29) with seven; `dynamic`: "In this case the address field MUST NOT be set"; the name of a created node "SHOULD include the link name and the container-id of the remote container".

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{Source, Target, Terminus}` (all fields, encode and decode; a field of the wrong type and `dynamic-node-properties` without `dynamic` are refused with the field name). `Peer::on_attach` refuses the attach with such a source or target (descriptor `0x28`, `0x29`) with `amqp:invalid-field` and the text of the decoder; only a described value of another type in the field `target` (Part 4: a coordinator) goes to the container as it came. Listener: `ConnectionNodes::attach` (a source with `default-outcome` `released` and the four `outcomes`; a dynamic node `dynamic/<container>/<link>/<n>`; `dynamic` with an address `amqp:invalid-field`).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_source_is_0x28_and_its_fields_in_order`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_an_unset_field_is_its_default`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_4_a_target_is_0x29_and_its_fields_in_order`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_field_of_the_wrong_type_is_refused_by_name`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_5_3_a_dynamic_node_is_named_after_container_and_link`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_3_5_4_a_malformed_target_is_refused_with_the_field`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_3_5_3_proton_gets_a_dynamic_node_from_the_endpoint`.

**Status:** partial — the client (`broker.rs::LinkSpec`) sends only `address` and `durable` of source and target; it cannot ask for `expiry-policy`, `timeout`, `dynamic`, `distribution-mode`, `filter`, `default-outcome`, `outcomes` and `capabilities` and does not read them from the broker's answer.

### §3.5.5, §3.5.6 Terminus durability, terminus expiry policy

**Spec:** `amqp-1.0-messaging` §3.5.5 — `none` (0), `configuration` (1), `unsettled-state` (2). §3.5.6 — `link-detach`, `session-end`, `connection-close`, `never`.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{Terminus::durable, ExpiryPolicy}` (codec, other values refused), `crates/amqp-endpoint/src/link.rs::TerminusDurability`. The listener keeps no terminus past the link and therefore reports `durable` `none` and `expiry-policy` `link-detach` whatever was asked for (§2.6.3: the properties as created). Client: `LinkSpec::durable`.

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_6_the_expiry_policies_are_their_four_symbols`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_field_of_the_wrong_type_is_refused_by_name`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_2_terminus_durability_from_wire`.

**Status:** partial — built are the codec and the non-durable terminus that ends with its link (`durable` `none`, `expiry-policy` `link-detach`, `timeout` 0, reported as such). Missing at the listener: `durable` `configuration` and `unsettled-state`, the expiry policies `session-end`, `connection-close` and `never`, and `timeout`; for them a terminus would have to outlive its link, which also carries the resumption of §2.6.3 and §2.6.13. The client can request `durable` only (§3.5.3).

### §3.5.9 to §3.5.13 Node properties, lifetime policies

**Spec:** `amqp-1.0-messaging` §3.5.9 — `node-properties`: `lifetime-policy`, `supported-dist-modes`. §3.5.10 to §3.5.13 — `delete-on-close` (0x2b), `delete-on-no-links` (0x2c), `delete-on-no-messages` (0x2d), `delete-on-no-links-or-messages` (0x2e).

**Repo:** `crates/amqp-endpoint/src/terminus.rs::Terminus::dynamic_node_properties` (codec), `crates/amqp-bridge/src/descriptors.rs` (the four descriptors). Listener: a dynamic node lives as long as the link that created it and is deleted with it (`delete-on-close`, reported as such).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_4_a_target_is_0x29_and_its_fields_in_order`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`, `crates/amqp-bridge/src/descriptors.rs::tests::amqp10_1_5_a_symbol_and_its_code_name_the_same_type`.

**Status:** partial — only `delete-on-close` is built; a request for one of the other three lifetimes gets `delete-on-close` reported. `supported-dist-modes` is not reported.

---

## amqp-1.0-transactions

Part 4 (`amqp-core-transactions`, in the standards cache as
`internal/standards/cache/oasis/amqp-1.0-transactions.html`). Both roles
are built: the listener `tools/amqp-dds-endpoint` is the transactional
resource, `crates/amqp-endpoint/src/txn.rs` the transaction controller on
the client connection. The types are in
`crates/amqp-endpoint/src/transactions.rs` (no_std), the descriptors
`0x30` to `0x34` in `crates/amqp-bridge/src/descriptors.rs`.

### §4.1 Transactional Messaging

**Spec:** `amqp-1.0-transactions` §4.1 — one container is the transactional resource, the other the controller; the controller establishes the control link and sends `declare` and `discharge` over it; "No transactional work is allowed on the control link"; the work of a transaction can be on any link of the session, with the capability on any link of the connection; "If the control link is closed while there exist non-discharged transactions it created, then all such transactions are immediately rolled back, and attempts to perform further transactional work on them will lead to failure."

**Repo:** Resource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes` (`attach_control`, `coordinate`; the end of a control link rolls back its transactions, whose identifiers are unknown afterwards) with `crates/amqp-endpoint/src/txn_resource.rs::TxnTable` (identifiers, the work of each transaction, the control link as owner). A transaction holds on every link of every session of the connection. Controller: `crates/amqp-endpoint/src/txn.rs::{TxnController, TxnClient}`; a control link the resource terminates closes the controller with the error of the detach.

**Tests:** `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_1_closing_the_control_link_rolls_back_its_transactions`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_1_closing_the_control_link_rolls_back_its_transactions`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error` (transactional work on the control link: detach with `amqp:not-allowed`), `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_7_several_transactions_on_a_session_and_one_across_sessions`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_1_the_controller_learns_that_its_control_link_was_terminated`.

**Status:** done — green on the Linux test host.

### §4.2 Declaring a Transaction

**Spec:** `amqp-1.0-transactions` §4.2 — the resource offers a coordinator as a target; "links to the coordinator cannot be resumed"; `declare` in a single `amqp-value` section, other sections "SHOULD be ignored"; "This message MUST NOT be sent settled"; a message sent settled: "it SHOULD detach with an amqp:illegal-state error"; success is `declared` with the identifier; an error is a `transaction-error`, as `rejected` when the source of the link supports `rejected`, else in the detach of the link; "Transaction controllers SHOULD establish a control link that allows the rejected outcome."

**Repo:** Resource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::{attach_control, coordinate}`. The attach with the target `coordinator` is answered with the coordinator and its capabilities (§4.5.1); a control link on which the listener would send is refused with `amqp:not-implemented`, one without the right (ACL, the coordinator's address) with `amqp:unauthorized-access`, and with `<transactions max-transactions="0">` every one with `amqp:not-implemented`. A `declare` is answered with `declared` (`TxnTable::declare`, an identifier of its own for each transaction, at most `max-transactions`, beyond that `amqp:resource-limit-exceeded`); a `declare` with `global-id` is refused (§4.5.2). `crates/amqp-endpoint/src/transactions.rs::coordinator_request` reads the message (header, annotations, properties and footer are passed over; no `amqp-value` or more than one, another body: `amqp:not-implemented`; not decodable: `amqp:decode-error`). Sent settled: detach with `amqp:illegal-state`. Whether an error goes as `rejected` or as a detach follows the `outcomes` of the control link's source. A control link is not resumed: a new attach under the same name knows no transaction of the old one. Controller: `crates/amqp-endpoint/src/txn.rs::TxnController::{link_spec, attach, declare, declare_global}` and `crates/amqp-endpoint/src/broker.rs::LinkSpec::coordinator` (target `coordinator` with the desired capabilities, a source with the outcomes including `rejected`; `declare` unsettled in an `amqp-value` section; a partner that attaches no coordinator is no resource: detach with `amqp:not-implemented`). `TxnClient::connect` checks the coordinator's capabilities against the desired ones (§4.5.1: "it is the responsibility of the transaction controller to verify"): if one is missing, it closes the connection and reports `io::ErrorKind::Unsupported` with the missing symbols. Without `amqp:multi-txns-per-ssn`, `TxnController` sends no second `declare` while a transaction is open or a `declare` is unanswered (§4.5.7).

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_is_answered_with_declared_and_the_identifier`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_the_transactions_element_of_the_configuration_bounds_the_coordinator`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_1_closing_the_control_link_rolls_back_its_transactions`, `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_2_a_message_without_a_declare_or_discharge_is_no_request`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_2_a_delivery_is_answered_with_an_error_of_the_link_or_later`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_2_the_attach_of_a_control_link_has_a_coordinator_as_its_target`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_the_control_link_sends_unsettled_to_a_coordinator`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_declare_is_sent_unsettled_and_answered_with_the_txn_id`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_detach_of_the_control_link_carries_the_error_of_the_request`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_partner_without_a_coordinator_is_no_transactional_resource`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_2_the_controller_declares_and_discharges_against_the_coordinator`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_1_the_controller_refuses_a_coordinator_without_a_desired_capability`. Qpid Proton as the controller: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint`. ActiveMQ Artemis as the resource: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_4_artemis_declares_a_txn_id_of_36_octets`.

**Status:** done — green on the Linux test host, the tests against Qpid Proton (python3-qpid-proton 0.37.0) and Apache ActiveMQ Artemis 2.44.0 (OpenJDK 21) with `--run-ignored all`.

### §4.3 Discharging a Transaction

**Spec:** `amqp-1.0-transactions` §4.3 — `discharge` in an `amqp-value` section, `fail` selects commit or rollback, sent settled is an error; when the coordinator cannot complete it, a `transaction-error` goes as `rejected` or in the detach of the control link; "the coordinator MUST always be able to complete a discharge where the fail flag is set to true".

**Repo:** Resource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::{coordinate, commit, rollback}` with `crates/amqp-endpoint/src/txn_resource.rs::TxnTable::discharge`: a commit applies the work and is answered with `accepted`; when the listener can no longer apply it, the transaction is rolled back and the commit is rejected with `amqp:transaction:rollback`; `fail` always rolls back and is answered with `accepted`, also for a transaction that timed out or is doomed; an identifier that names no transaction of this control link is `amqp:transaction:unknown-id`. Controller: `crates/amqp-endpoint/src/txn.rs::TxnController::discharge`, `TxnClient::{commit, rollback}`; a `rejected` with an error or the detach of the control link becomes `TxnError::{UnknownId, Rollback, Timeout, Refused, Closed}`.

**Tests:** `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_3_a_discharge_hands_back_the_work_to_apply_or_to_undo`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_3_a_control_link_without_rejected_is_detached_with_the_error`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_3_a_discharge_of_an_unknown_id_is_rejected_with_unknown_id`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_3_a_discharge_names_the_transaction_and_is_answered_with_accepted`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_3_a_rejected_request_carries_the_transaction_error`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_2_the_controller_declares_and_discharges_against_the_coordinator`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_3_proton_reads_the_rollback_of_a_commit_the_endpoint_refuses`.

**Status:** done — green on the Linux test host, the Proton test with `--run-ignored all`.

### §4.4, §4.4.1 Transactional Work, Transactional Posting

**Spec:** `amqp-1.0-transactions` §4.4 — work is posting (`transfer`), acquiring (`flow`) and retiring (`disposition`); the identifiers in frames from the resource to the controller "MUST correctly indicate with which (if any) transaction this work is associated". §4.4.1 — a posted `transfer` carries a `transactional-state`; "if delivery is split across several transfer frames then all frames MUST be explicitly associated with the same transaction"; a `discharge` with an incomplete delivery terminates the control link with `transaction-rollback`; the message becomes available at the node only with the commit; an unsettled delivery is answered with a `transactional-state` of the same transaction and the presumptive outcome before the commit can succeed.

**Repo:** Resource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::post` (the message is held with the transaction, the answer is `transactional-state{txn-id, accepted}`; every frame must name the transaction: if a later frame does not name it, the answer is `rejected` with `amqp:invalid-field` within the transaction; if only a later frame names a transaction, the delivery is not posted and is rejected with `amqp:invalid-field` outside any transaction, and when it was sent settled the link is detached with the error; a transaction that only some of the frames name can no longer commit; a delivery is a partial delivery posted against every transaction one of its frames so far named; a `discharge` with a delivery still open: the transactions of the control link are rolled back, then the link is detached with `amqp:transaction:rollback`), `tools/amqp-dds-endpoint/src/nodes.rs::Nodes::put_all` (the messages of a transaction reach their nodes all or none), `crates/amqp-endpoint/src/peer.rs` (the state of the first frame goes to the container, and so does every transaction a later frame names: `Container::delivery_named`, `Delivery::txns`; the answer goes to the sender). Controller: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::transfer_with_state` (the state on every frame), `crates/amqp-endpoint/src/txn.rs::{posted, TxnClient::send, TxnClient::send_settled}`.

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_posted_message_reaches_the_node_only_with_the_commit`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_rolled_back_posting_reaches_no_node`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_discharge_with_a_partial_delivery_terminates_the_control_link`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_every_frame_of_a_posted_delivery_names_the_transaction`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_delivery_whose_later_frame_names_the_transaction_is_rejected`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_transaction_a_later_frame_names_has_a_partial_delivery`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_control_link_terminated_for_a_partial_delivery_goes_after_the_abort`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_4_4_1_the_messages_of_a_transaction_reach_their_nodes_all_or_none`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_1_the_state_of_a_transfer_reaches_the_container_and_its_answer_the_sender`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_4_1_a_posted_message_names_the_transaction_on_every_frame`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_4_the_states_of_transactional_work_name_the_transaction`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_1_the_controller_posts_in_a_transaction_and_commits_or_rolls_back`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_1_a_message_of_several_frames_is_posted_whole`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_4_1_a_committed_send_is_in_the_artemis_queue_and_a_rolled_back_one_is_not`.

**Status:** done — green on the Linux test host, the tests against Proton and Artemis with `--run-ignored all`.

### §4.4.2 Transactional Retirement

**Spec:** `amqp-1.0-transactions` §4.4.2 — the controller associates the outcome of a delivery with a transaction through a `disposition` with a `transactional-state`; "the delivery MUST NOT be associated with a different non-discharged transaction than the outcome. If this happens then the control link MUST be terminated with a transaction-rollback error"; with the commit the outcome is applied and the deliveries can be settled; after a rollback the delivery stays with the controller.

**Repo:** Resource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::retire` (the outcome is recorded with the transaction, the delivery stays unsettled; with the commit the listener applies it and settles the delivery with it; after a rollback it stays with the controller; a delivery in a second open transaction terminates the control link with `amqp:transaction:rollback`; an outcome in an unknown transaction detaches the delivery's link with `amqp:transaction:unknown-id`). `crates/amqp-endpoint/src/peer.rs` leaves a delivery with a transactional `disposition` unsettled until it is settled. Controller: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::disposition_with_state`, `crates/amqp-endpoint/src/txn.rs::{retired, TxnClient::retire, TxnClient::settled}`.

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_an_outcome_in_a_transaction_applies_with_the_commit`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_a_rolled_back_outcome_is_not_applied`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_an_outcome_in_another_transaction_terminates_the_control_link`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_an_outcome_in_an_unknown_transaction_detaches_the_link_with_unknown_id`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_2_a_transactional_disposition_leaves_the_delivery_unsettled_until_it_is_settled`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_4_dispositions_and_flows_name_the_transaction`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_2_the_controller_retires_in_a_transaction`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_4_2_a_rolled_back_accept_is_delivered_again_by_artemis`.

**Status:** done — green on the Linux test host, the tests against Proton and Artemis with `--run-ignored all`.

### §4.4.3 Transactional Acquisition

**Spec:** `amqp-1.0-transactions` §4.4.3 — a `txn-id` in `flow.properties` sets the transaction in which a sending link acquires messages; the `discharge` clears it; a link without transactional acquisition "MUST be terminated with a not-implemented error"; the controller "SHOULD ensure there is no outstanding credit at the sender before it discharges the transaction"; when an acquired message has been sent only in part at a rollback, "the resource MUST abort or complete sending the remainder of the message before completing the discharge".

**Repo:** Resource: `tools/amqp-dds-endpoint/src/handler.rs` (the `txn-id` of a `flow` sets the transaction of the link, `crates/amqp-endpoint/src/transactions.rs::flow_txn_id`; acquired messages go with `transactional-state{txn-id}`; a rollback puts them back in their order and settles the deliveries as `released`; a commit lets the acquisition stand and applies an outcome of the same transaction; an outcome outside the transaction for a delivery acquired in it terminates the control link with `amqp:transaction:rollback`; an unknown `txn-id` detaches the link with `amqp:transaction:unknown-id`; a message sent in part is aborted before the rollback is answered, and before the detach of the control link when a refused commit without `rejected` in the source, or an `amqp:transaction:rollback` under §4.4.1 or §4.4.2, ends the link (`ConnectionNodes::{end_control, roll_back_control}`; `Action::Detach` waits like `Action::Answer`); `ConnectionNodes::clear_acquiring` clears the transaction of the links with the `discharge`). The listener acquires transactionally on every sending link, so `not-implemented` does not occur. Controller: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::flow_in_txn`, `crates/amqp-endpoint/src/txn.rs::TxnClient::receive` (sends the `txn-id` when it changes, and drains the link when no message comes, so that no credit stays with the sender).

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_messages_acquired_in_a_transaction_return_with_the_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_committed_acquisition_stands_and_its_outcome_applies`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_acquisition_in_an_unknown_transaction_and_an_outcome_outside_it_are_refused`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_message_sent_in_part_is_aborted_before_the_rollback_is_answered`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_refused_commit_detaches_the_control_link_after_the_abort`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_control_link_terminated_for_a_partial_delivery_goes_after_the_abort`, `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_4_3_the_txn_id_of_a_flow_is_an_entry_of_its_properties`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_3_the_txn_id_of_a_flow_reaches_the_container`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_3_a_delivery_sent_with_a_state_carries_it_on_every_frame_and_can_be_aborted`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_3_the_controller_acquires_in_a_transaction`.

**Status:** done — green on the Linux test host. Transactional acquisition is not measured against another implementation: the class `Transaction` of Qpid Proton (Python) offers posting and retiring, no acquisition; against Artemis it is not checked.

### §4.4.4 Interaction of Settlement with Transactions

**Spec:** `amqp-1.0-transactions` §4.4.4 — posting: sent settled by the controller, available only with the commit, and when the resource cannot process it "it MUST NOT allow the successful discharge"; sent unsettled, the outcome must be reported with the same transaction before the commit, and when the resource can no longer apply it the commit must not succeed; "The controller SHOULD settle any outstanding unsettled deliveries in a timely fashion". Retirement: settled by the controller, after a rollback "the resource MUST apply the default outcome"; not settled, after a rollback the state from before. Acquisition: the rollback takes back acquisition and outcome; an outcome before the `discharge` "MUST be associated with the same transaction".

**Repo:** `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::{post, commit, rollback, retire}` with `crates/amqp-endpoint/src/txn_resource.rs::TxnTable` (a doomed transaction refuses the commit with its reason and takes the rollback): a delivery sent settled that the node cannot take dooms the transaction; a commit whose messages the nodes can no longer hold is rejected with `amqp:transaction:rollback` and nothing reaches a node; a delivery the controller settled with a transactional outcome takes the source's default outcome, `released`, after the rollback; one not settled stays with the controller; acquired messages come back with the rollback. Controller: `crates/amqp-endpoint/src/txn.rs::TxnClient::send` settles a posted delivery after the resource's answer.

**Tests:** `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_4_4_a_doomed_transaction_refuses_the_commit_and_takes_the_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_4_a_commit_the_nodes_can_no_longer_take_is_rejected_with_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_a_rolled_back_outcome_is_not_applied`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_messages_acquired_in_a_transaction_return_with_the_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_acquisition_in_an_unknown_transaction_and_an_outcome_outside_it_are_refused`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_8_the_controller_reads_the_transaction_errors`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_3_proton_reads_the_rollback_of_a_commit_the_endpoint_refuses`.

**Status:** done — green on the Linux test host, the Proton test with `--run-ignored all`.

### §4.5.1 Coordinator

**Spec:** `amqp-1.0-transactions` §4.5.1 — `coordinator` (0x30, `amqp:coordinator:list`), a target with the field `capabilities` (symbols): from the controller the desired ones, from the resource the ones it has.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::Coordinator` (encode and decode, numeric or symbolic descriptor), `crates/amqp-bridge/src/descriptors.rs` (`0x30` to `0x34`), `crates/amqp-endpoint/src/peer.rs` (a `coordinator` target goes to the container as `LinkInfo::coordinator`, `Attached::coordinator` into the answer), `crates/amqp-endpoint/src/broker.rs` (attach and reading on the client side). The listener states `amqp:local-transactions`, `amqp:multi-txns-per-ssn` and `amqp:multi-ssns-per-txn`. Controller: `TxnClient::connect` compares the coordinator's capabilities with the desired ones (`Coordinator::lacking`); if one is missing, it closes the connection and reports `io::ErrorKind::Unsupported`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_1_a_coordinator_is_0x30_and_its_capabilities`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_5_1_a_coordinator_target_is_answered_with_the_capabilities_of_the_container`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_is_answered_with_declared_and_the_identifier`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_1_the_controller_refuses_a_coordinator_without_a_desired_capability`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_4_1_a_committed_send_is_in_the_artemis_queue_and_a_rolled_back_one_is_not` (Artemis's coordinator states `amqp:local-transactions`), `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_7_two_transactions_on_one_session_with_artemis` (Artemis 2.44.0 states `amqp:local-transactions`, `amqp:multi-txns-per-ssn` and `amqp:multi-ssns-per-txn`; a controller that desires `amqp:distributed-transactions` does not connect).

**Status:** done — green on the Linux test host, the Artemis test with `--run-ignored all`.

### §4.5.2 Declare

**Spec:** `amqp-1.0-transactions` §4.5.2 — `declare` (0x31) with the optional field `global-id`; "This field MUST NOT be set if the coordinator does not have the distributed-transactions capability."

**Repo:** `crates/amqp-endpoint/src/transactions.rs::{Declare, declare_message, coordinator_request}`. Controller: `TxnController::declare_global` sends `global-id` only to a coordinator with `amqp:distributed-transactions`, else `InvalidInput`. Listener: does not have that capability and refuses a `declare` with `global-id` with `amqp:not-implemented` (distributed transactions are Part 6, which OASIS has not published).

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_2_declare_is_0x31_with_an_optional_global_id`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_declare_is_sent_unsettled_and_answered_with_the_txn_id`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error`.

**Status:** done — green on the Linux test host.

### §4.5.3 Discharge

**Spec:** `amqp-1.0-transactions` §4.5.3 — `discharge` (0x32) with the mandatory field `txn-id` and `fail` (boolean).

**Repo:** `crates/amqp-endpoint/src/transactions.rs::{Discharge, discharge_message, coordinator_request}` (`fail` without a value is `false`; without `txn-id` or with the wrong type: `amqp:invalid-field`).

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_3_discharge_is_0x32_txn_id_and_fail`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_3_a_discharge_names_the_transaction_and_is_answered_with_accepted`.

**Status:** done — green on the Linux test host.

### §4.5.4 Transaction Id

**Spec:** `amqp-1.0-transactions` §4.5.4 — `transaction-id`, a `binary`: "A transaction-id can be up to 32 octets of binary data."

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TxnId` (`new` and `decode` take at most 32 octets; the listener issues 8). A longer `txn-id` in a `discharge` is rejected with `amqp:invalid-field`. Apache ActiveMQ Artemis 2.44.0 issues 36 octets (the text of a UUID); the controller refuses such a `declared` as a protocol error by default and takes it only after `TxnClient::set_txn_id_limit` (`BrokerReader::set_txn_id_limit`, `TxnId::decode_up_to`).

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_4_a_txn_id_is_a_binary_of_at_most_32_octets`, `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_4_a_longer_txn_id_is_read_only_up_to_the_given_bound`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_4_the_reader_takes_longer_txn_ids_only_when_told`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_3_a_discharge_of_an_unknown_id_is_rejected_with_unknown_id`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_4_artemis_declares_a_txn_id_of_36_octets`.

**Status:** done — green on the Linux test host, the Artemis test with `--run-ignored all`.

### §4.5.5 Declared

**Spec:** `amqp-1.0-transactions` §4.5.5 — `declared` (0x33), a delivery state and an outcome, with the mandatory field `txn-id`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::Declared`, `crates/amqp-endpoint/src/terminus.rs::{DeliveryState::Declared, decode_state, encode_state}`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_5_declared_is_0x33_and_the_txn_id`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states`.

**Status:** done — green on the Linux test host.

### §4.5.6 Transactional State

**Spec:** `amqp-1.0-transactions` §4.5.6 — `transactional-state` (0x34) with the mandatory field `txn-id` and the provisional `outcome`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TransactionalState` (an `outcome` that is none of Part 3 §3.4 is refused), `crates/amqp-endpoint/src/terminus.rs::DeliveryState::Transactional`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_6_a_transactional_state_is_0x34_txn_id_and_outcome`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states`.

**Status:** done — green on the Linux test host.

### §4.5.7 Txn Capability

**Spec:** `amqp-1.0-transactions` §4.5.7 — the symbols `amqp:local-transactions`, `amqp:distributed-transactions`, `amqp:promotable-transactions`, `amqp:multi-txns-per-ssn`, `amqp:multi-ssns-per-txn`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TxnCapability` (all five; another symbol in `coordinator.capabilities` is an error of the decoder). The listener has and states `amqp:local-transactions`, `amqp:multi-txns-per-ssn` (several open transactions per session) and `amqp:multi-ssns-per-txn` (one transaction across the sessions of the connection); it does not state `distributed-transactions` and `promotable-transactions`. Controller: when the coordinator does not have `amqp:multi-txns-per-ssn`, `TxnController` sends no second `declare` on the session while a transaction is open or a `declare` is unanswered (`TxnController::{open_transactions, admits_declare}`; `TxnClient::declare` reports `TxnError::Unsupported`). This restriction is tested only against a coordinator made of test events: the listener and Artemis 2.44.0 both state `amqp:multi-txns-per-ssn`; against Artemis the test shows that two transactions can be open on one session.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_7_the_txn_capabilities_are_their_five_symbols`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_7_several_transactions_on_a_session_and_one_across_sessions`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_5_7_without_multi_txns_per_ssn_one_transaction_at_a_time`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_5_7_with_multi_txns_per_ssn_several_transactions_are_open`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_7_two_transactions_on_one_session_with_artemis`.

**Status:** done — green on the Linux test host, the Artemis test with `--run-ignored all`.

### §4.5.8 Transaction Error

**Spec:** `amqp-1.0-transactions` §4.5.8 — `amqp:transaction:unknown-id`, `amqp:transaction:rollback`, `amqp:transaction:timeout`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TransactionError`; listener: `unknown-id` for an identifier without a transaction, `rollback` for a commit that cannot succeed and the error cases of §4.4.1 and §4.4.2, `timeout` after `<transactions timeout-ms>` (default in the README of `tools/amqp-dds-endpoint`). Controller: `crates/amqp-endpoint/src/txn.rs::TxnError::from_error`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_8_the_transaction_errors_are_their_three_symbols`, `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_5_8_an_error_of_the_coordinator_is_read_by_its_condition`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_8_the_controller_reads_the_transaction_errors`.

**Status:** done — green on the Linux test host.

---

## amqp-1.0-security

### §5.1 Security layers

**Spec:** `amqp-1.0-security` §5.1 — security layers (TLS, SASL) lie below the AMQP connection and are negotiated before it.

**Repo:** Listener: `tools/amqp-dds-endpoint/src/server.rs::serve_one` (TLS, then `handle_connection`), `tools/amqp-dds-endpoint/src/handler.rs::{serve, do_sasl_phase}` (the SASL header, then the AMQP header). Client: `crates/amqp-endpoint/src/broker.rs::connect` (TLS, SASL, `open`).

**Tests:** `tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_10_1_a_tls_client_is_offered_plain_and_authenticated_against_the_store`, `tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_2_1_an_mtls_client_authenticates_with_external`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_header_after_sasl_is_answered_with_the_amqp_header`.

**Status:** done

### §5.2 TLS

**Spec:** `amqp-1.0-security` §5.2 — TLS is negotiated with the protocol header `AMQP%d2.1.0.0`; the TLS handshake follows on the same connection.

**Repo:** —

**Tests:** —

**Status:** open — the header with protocol id 2 is built on neither side: the listener answers it like any other unsupported protocol id with its own header and closes (§2.2), the client never sends it. TLS exists from the start of the connection only (§5.2.1).

### §5.2.1 Alternative establishment

**Spec:** `amqp-1.0-security` §5.2.1 — TLS from the start of the TCP connection (port 5671), without the TLS protocol header.

**Repo:** Listener: `tools/amqp-dds-endpoint/src/tls.rs::accept_server`, `tools/amqp-dds-endpoint/src/server.rs::serve_one`. Client: `crates/amqp-endpoint/src/broker.rs::connect` (rustls, ALPN `amqp`, the server name from `ConnectParams::host`).

**Tests:** `tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_10_1_a_tls_client_is_offered_plain_and_authenticated_against_the_store`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`.

**Status:** done

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

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::{amqp10_5_3_3_1_the_mechanism_names_are_plain_anonymous_external, amqp10_5_plain_authenticates_on_correct_credentials, amqp10_5_plain_fails_on_wrong_credentials, amqp10_5_anonymous_authenticates_without_credentials, amqp10_5_external_uses_transport_subject, amqp10_5_3_3_6_the_sasl_codes_are_0_to_4, amqp10_5_sasl_code_unknown_value_rejected}`; `crates/amqp-bridge/src/performatives.rs::tests::{amqp10_5_3_3_2_sasl_init_is_mechanism_response_and_hostname, amqp10_5_3_3_1_the_offered_mechanisms_are_read_from_an_array_or_one_symbol, amqp10_5_3_3_5_the_outcome_code_is_read}`; `tools/amqp-dds-endpoint/src/handler.rs::tests::amqp10_5_3_3_1_no_empty_list_of_mechanisms_is_sent`; RabbitMQ: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::amqp10_0_2_a_message_the_client_sends_is_read_by_pika` (SASL PLAIN).

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
{connect_plain, send_to, send_to_accepted, recv_from}` — a full client (SASL → open/begin →
attach(sender/receiver) → transfer/flow/disposition → detach/close).

**Tests:** `crates/amqp-bridge/src/performatives.rs::tests::{amqp10_2_7_3_a_sender_attach_names_its_target_address, amqp10_2_7_5_transfer_is_handle_id_tag_format_settled_then_the_message, amqp10_2_7_3_attach_is_name_handle_and_role, amqp10_2_7_3_an_unsettled_sender_attach_says_so, amqp10_2_7_6_a_received_disposition_reads_its_range_and_state}`, `crates/amqp-endpoint/src/client.rs::tests::amqp10_3_4_2_send_to_accepted_returns_on_the_accepting_disposition`; RabbitMQ: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::{amqp10_0_2_a_message_the_client_sends_is_read_by_pika, amqp10_0_2_the_client_receives_what_pika_published, amqp10_3_4_2_an_accepted_send_is_in_the_queue_when_it_returns}`.

**Status:** done

---

## Audit status

64 done / 14 partial / 1 open / 0 blocked / 2 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document amqp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-bridge --all-targets` (job `merge-rust-amqp-bridge`) — 67 tests green.
* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (job `merge-rust-amqp-endpoint`) — 10 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (job `integration-bridges-vendor-amqp`) — 3 tests green.

80 cited tests green, 0 not green.

Open items: `amqp-1.0.open.en.md`.
