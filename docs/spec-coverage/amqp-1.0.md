# OASIS AMQP 1.0 — Spec-Coverage

**Spec:** [OASIS AMQP Version 1.0 — OASIS Standard →](https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-overview-v1.0-os.html) (overview/types/transport/messaging/security).

**Kontext:** AMQP 1.0 ist die OASIS-Enterprise-Messaging-Spec
(RabbitMQ/ActiveMQ/Solace). ZeroDDS implementiert den **Type System**
(`amqp-1.0-types`), das **Frame-Format** (`amqp-1.0-transport` §2.3),
die **Performatives** (§2.7) und die **Message-Sektionen**
(`amqp-1.0-messaging` §3) als pure-Rust no_std+alloc Library
(`crates/amqp-bridge`); **Verbindung, Sessions und Links**
(`amqp-1.0-transport` §2.2, §2.4 bis §2.6), **Knoten, Delivery-States,
Source und Target** (`amqp-1.0-messaging` §3.3 bis §3.5) und der
**SASL-Handshake** (`amqp-1.0-security` §5) liegen in
`crates/amqp-endpoint` und `tools/amqp-dds-endpoint`, ebenso die
**Transaktionen** (`amqp-1.0-transactions`, Part 4) mit beiden Rollen.

Implementation:

- `crates/amqp-bridge/` — Type System, Descriptoren, Frame-Format,
  Performatives, Message-Sektionen (`types`, `extended_types`,
  `descriptors`, `frame`, `performatives`, `sections`, `codec_profile`).
- `crates/amqp-endpoint/src/peer.rs` — die antwortende Seite einer
  Verbindung als Zustandsmaschine ohne I/O (Part 2 §2.4 bis §2.6, §2.8);
  `terminus.rs` — Fehler, Delivery-States, Source und Target (Part 2
  §2.8.14, Part 3 §3.4, §3.5).
- `tools/amqp-dds-endpoint/` — der Listener darauf (`handler.rs`,
  `frame_io.rs`, `nodes.rs`).
- `crates/amqp-endpoint/src/{broker.rs, client.rs}` — der Client.
- `crates/amqp-endpoint/src/{sasl.rs, scram.rs}` — SASL (§5).
- `crates/amqp-endpoint/src/{transactions.rs, txn_resource.rs, txn.rs}` —
  die Typen von Part 4, die Transaktionen einer Ressource, der
  Controller; die Ressource im Listener (`handler.rs`).

Test-Lauf auf dem Linux-Testhost (`cargo nextest run --profile merge -p
<crate> --all-targets`): `zerodds-amqp-bridge` 234 grün, 1 übersprungen;
`zerodds-amqp-endpoint` 408 grün, 15 übersprungen (`#[ignore]`);
`amqp-dds-endpoint` 180 grün, 8 übersprungen (`#[ignore]`). Die
übersprungenen Tests mit `--run-ignored all`: `--test proton_e2e` 6 von 6
und `--test proton_txn_e2e` 2 von 2 mit python3-qpid-proton 0.37.0;
`--test artemis_txn_e2e` 3 von 3 gegen Apache ActiveMQ Artemis 2.44.0 auf
OpenJDK 21; `--test rabbitmq_amqp10_e2e` 3 von 3 und
`--test bridge_rabbitmq_e2e` 7 von 9 gegen RabbitMQ 4.0.5 mit pika 1.3.2,
die zwei nicht grünen (`ddsamqp_7_7_2_an_amqp_dispose_reaches_a_cyclone_reader`,
`ddsamqp_7_7_2_an_amqp_dispose_reaches_a_foreign_reader`) brauchen die
Python-Bindung von Cyclone DDS und `ZERODDS_DISPOSE_READER`, die auf dem
Host fehlen (Job `vendor-live-dispose-bridges`). Hinweis: die
`codec_profile.rs`-Tests gehören zum dds-amqp-Vendor-Profile
(`dds-amqp-1.0.md` §2.3/§2.4), nicht zur OASIS-AMQP-1.0.

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

**Repo:** Antwortende Seite (TCP-Server): `tools/amqp-dds-endpoint/src/handler.rs::{handle_connection, do_amqp_phase}` auf
`crates/amqp-endpoint/src/peer.rs::Peer` (Part 2) mit
`crates/amqp-endpoint/src/terminus.rs` (Part 3 §3.4, §3.5) und
`tools/amqp-dds-endpoint/src/nodes.rs` (Part 3 §3.3). Client:
`crates/amqp-endpoint/src/broker.rs`, `crates/amqp-endpoint/src/client.rs`.
Codec: `crates/amqp-bridge/` (Parts 1 bis 3). Part 5:
`crates/amqp-endpoint/src/{sasl.rs, scram.rs}`, `tools/amqp-dds-endpoint/src/tls.rs`.

**Tests:** `crates/amqp-endpoint/src/session.rs::tests::amqp10_0_2_header_exchange_advances_to_hdr_exch`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_version_is_answered_with_the_supported_header`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`; Qpid Proton gegen den Listener: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`, `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_14_proton_sends_a_message_above_the_frame_size`; RabbitMQ gegen den Client: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::{amqp10_0_2_a_message_the_client_sends_is_read_by_pika, amqp10_0_2_the_client_receives_what_pika_published}`.

**Status:** partial — gebaut sind die Protokoll-Aushandlung (§2.2),
Part 1, Part 2 auf der antwortenden Seite und als Client, Part 3, Part 4 und
Part 5 mit SASL und TLS von Beginn der Verbindung an; die Items der
Parts nennen den Stand je Abschnitt. Es fehlen: was die Items
§2.5.1 bis §2.5.3, §2.6.1, §2.6.3, §2.6.7 bis §2.6.10, §2.6.13, §2.8.14, §3.3, §3.4.1, §3.4.5, §3.5.1/§3.5.2, §3.5.3, §3.5.5/§3.5.6, §3.5.9 bis §3.5.13, §5.2 als fehlend nennen; SCRAM-SHA-1 (§0.2: "SHOULD support the PLAIN
[RFC4616] and SCRAM-SHA1 [RFC5802] mechanisms"; gebaut sind PLAIN und
SCRAM-SHA-256). Part 4 (Transactions) hat eigene Items, §4.1 bis §4.5.8.

---

## amqp-1.0-types

### §1.1 Type System (Primitive/Described/Composite/Restricted)

**Spec:** `amqp-1.0-types` §1.1 — vier Type-Kategorien.

**Repo:** Alle vier Kategorien spec-konform abgedeckt:
- **Primitive** (§1.6): vollständig in `crates/amqp-bridge/src/types.rs::FormatCode` mit
  allen 30+ Wire-Codes.
- **Described** (§1.3): `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::{Described,
  DescribedSymbol}` (`0x00` + Descriptor + Wert, Descriptor als `ulong`
  oder als `symbol`, Encode und Decode); die Performatives und
  Sektionen in `performatives.rs` und `sections.rs` setzen darauf auf.
- **Composite** (§1.4): per-Performative-List-Body in
  `performatives.rs` + Composite-Type-Mapping in
  `crates/amqp-bridge/src/extended_types.rs::§7.2`.
- **Restricted** (§1.5): Domain-Restricted-Types
  (`milliseconds`, `seconds`, `ietf-language-tag`,
  `iso-8859-1`, `symbol`) via `extended_types.rs` mit
  Constraint-Validierung.

**Tests:** Primitive: `crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_5_a_uint_is_uint0_smalluint_or_four_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`. Described: `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbolic_descriptor_is_read`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbol_of_the_specification_is_its_numeric_descriptor`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`, `crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`. Composite: `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`. Restricted: `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`.

**Status:** done — alle vier Kategorien werden geschrieben und
gelesen; ein beschriebener Wert mit numerischem oder symbolischem
Descriptor ist ein eigener Wert des Typsystems.

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
symbolic-Name. Descriptor-Code-Allocation für Spec-Domains. "Descriptor
values other than symbolic (symbol) or numeric (ulong) are, while not
syntactically invalid, reserved". §1.3.3: jeder Typ der Spezifikation
hat beide Descriptoren (`amqp:open:list` und `0x00000000:0x00000010`).

**Repo:** `crates/amqp-bridge/src/descriptors.rs` (`AMQP_DESCRIPTORS`,
`code_of`, `symbol_of`: symbolischer und numerischer Descriptor der 35
beschriebenen Typen der Parts 2, 3 und 5),
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::{Described,
DescribedSymbol}` (ein empfangener Descriptor ist `ulong` oder `symbol`;
ein Symbol der Tabelle wird zum numerischen Descriptor desselben Typs,
ein anderes Symbol bleibt `DescribedSymbol`; jeder andere Descriptor-Typ
ist `TypeError::ReservedDescriptor`),
`crates/amqp-bridge/src/performatives.rs::decode_performative` (Frame-Body
mit beiden Formen; unbekanntes Symbol `TypeError::UnknownDescriptor`),
`crates/amqp-bridge/src/sections.rs::MessageSection::decode` (über
`decode_performative`). Geschrieben wird die numerische Form
(Descriptor-Codes 0x10-0x18 der Performatives, 0x70-0x78 der Sektionen).

**Tests:** `crates/amqp-bridge/src/descriptors.rs::tests::amqp10_1_5_every_descriptor_is_in_the_amqp_domain_and_named_once`, `crates/amqp-bridge/src/descriptors.rs::tests::amqp10_1_5_a_symbol_and_its_code_name_the_same_type`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbolic_descriptor_is_read`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_symbol_of_the_specification_is_its_numeric_descriptor`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_5_a_descriptor_of_another_type_is_refused_as_reserved`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_open_by_its_symbol_is_open_by_its_code`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_frame_body_under_an_unknown_symbol_is_refused_by_name`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_1_5_a_section_by_its_symbol_is_the_section_by_its_code`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44`, `crates/amqp-bridge/src/performatives.rs::tests::amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_1_the_header_is_0x70_and_a_list`, `crates/amqp-bridge/src/sections.rs::tests::amqp10_3_2_9_the_footer_is_0x78_and_a_map`.

**Status:** done — beide Descriptor-Formen werden gelesen, die
numerische wird geschrieben.

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

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_decimal32,
decode_decimal32}`, `AmqpExtValue::Decimal32` (die vier Oktette des
BID-Layouts, wie sie auf der Leitung stehen; Encode und Decode).

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_decimal32_is_read_back_octet_for_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_cut_decimal_is_refused`, `crates/amqp-bridge/tests/boundary_decoders.rs::amqp10_1_6_13_the_decimal_decoders_hold_their_bounds`.

**Status:** done — der Wert wird geschrieben und gelesen;
Decimal-Arithmetik auf den Oktetten ist Sache des Aufrufers.

### §1.6.14 decimal64

**Spec:** §1.6.14 — `decimal64` (`0x84`, fixed-8) = IEEE 754
decimal64 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_decimal64,
decode_decimal64}`, `AmqpExtValue::Decimal64`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_14_a_decimal64_is_read_back_octet_for_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_cut_decimal_is_refused`, `crates/amqp-bridge/tests/boundary_decoders.rs::amqp10_1_6_13_the_decimal_decoders_hold_their_bounds`.

**Status:** done

### §1.6.15 decimal128

**Spec:** §1.6.15 — `decimal128` (`0x94`, fixed-16) = IEEE 754
decimal128 in BID-Layout.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::{encode_decimal128,
decode_decimal128}`, `AmqpExtValue::Decimal128`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_15_a_decimal128_is_read_back_octet_for_octet`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_a_cut_decimal_is_refused`, `crates/amqp-bridge/tests/boundary_decoders.rs::amqp10_1_6_13_the_decimal_decoders_hold_their_bounds`.

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
mit `encode/decode` und Homogenitäts-Validation (`array_parts`,
`decode_array`). Der Element-Constructor ist ein Format-Code oder, für
beschriebene Elemente, `0x00` + Descriptor + Constructor des Werts
(§1.2, Bild 1.3), einmal geschrieben. Jedes Element wird aus seinen
eigenen Oktetten gelesen, deren Zahl der Constructor nennt (§1.2).
Elemente eines Constructors ohne Oktette (`0x40` bis `0x45`) werden
gezählt. Grenze beim Lesen (`within`, `charge`): solche Elemente und die
Kopien des gemeinsamen Descriptors, die jedes Element eines Arrays
beschriebener Werte bekommt, werden einem Guthaben von
`size_of::<AmqpExtValue>()` Oktetten je Oktett der äußersten Liste, Map
oder des äußersten Arrays belastet, das alle verschachtelten Werte
teilen; ein Wert ergibt damit höchstens so viele Elemente ohne Oktette,
wie sein äußerster zusammengesetzter Wert Oktette hat, und was darüber
liegt, wird mit `LengthTooLarge` abgelehnt. Das leere Array wird mit dem
Constructor `null` geschrieben (`e0 02 00 40`): der Wert behält den
Constructor nicht, mit dem er gelesen wurde.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_is_one_constructor_then_the_values`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_whose_compact_encodings_differ_share_the_wide_one`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_elements_of_different_types_are_refused`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_more_than_255_elements_is_array32`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_described_values_shares_descriptor_and_constructor`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_an_array_of_nulls_is_its_count`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_nested_arrays_of_nulls_share_one_bound`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_the_copies_of_a_shared_descriptor_are_bounded`, `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_24_a_long_array_is_read_element_by_element`, `crates/amqp-bridge/tests/boundary_decoders.rs::nested_arrays_of_nulls_do_not_multiply_the_bound`, `crates/amqp-bridge/tests/boundary_decoders.rs::nested_arrays_of_nulls_one_above_the_octets_of_the_value_is_err`, `crates/amqp-bridge/tests/boundary_decoders.rs::encode_array_empty_is_the_null_constructor_alone`; ein empfangenes leeres Array in einem abgelehnten Attach: `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_a_refused_attach_with_an_empty_array_is_answered`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_a_refused_attach_with_an_empty_array_leaves_the_connection_usable`.

**Status:** done

Plus DoS-Cap-Schutz auf Recursion-Depth 32 (siehe
`dds-amqp-1.0-beta1.pdf §6.1`):
`extended_types::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`,
`extended_types::tests::amqp10_1_6_22_a_list_nested_past_the_bound_is_refused`.

---

## amqp-1.0-transport

### §2.2 Version Negotiation

**Spec:** `amqp-1.0-transport` §2.2 — Protokoll-Header `"AMQP" %d0 major
minor revision` (1.0.0) vor jedem Frame. "If the requested protocol
version is not supported, the server MUST send a protocol header with a
supported protocol version and then close the socket." — "If the server
cannot parse the protocol header, the server MUST send a valid protocol
header with a supported protocol version and then close the socket." —
"If the incoming and outgoing protocol headers do not match, both peers
MUST close their outgoing stream and SHOULD read the incoming stream
until it is terminated."

**Repo:** Server: `tools/amqp-dds-endpoint/src/handler.rs::{handle_connection,
refuse_header}` (ein Header mit anderer Version, anderer Protokoll-Id
oder ohne `AMQP` wird mit dem unterstützten Header beantwortet: dem
SASL-Header, wenn SASL verlangt ist, sonst `AMQP%d0.1.0.0`; danach
wird der ausgehende Strom geschlossen und der eingehende bis zu seinem
Ende gelesen, höchstens 1 s und 64 KiB),
`tools/amqp-dds-endpoint/src/frame_io.rs::{read_protocol_header,
write_protocol_header, Transport}` (Version ist major, minor und
revision). Client: `crates/amqp-endpoint/src/broker.rs::connect` (sendet
seinen Header zuerst; ein abweichender Header des Brokers beendet den
Verbindungsaufbau mit dem empfangenen Header in der Fehlermeldung).

**Tests:** `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_version_is_answered_with_the_supported_header`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_octets_that_are_no_header_are_answered_with_the_header`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_endpoint_that_requires_sasl_answers_with_the_sasl_header`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_header_after_sasl_is_answered_with_the_amqp_header`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_2_the_amqp_protocol_header_is_amqp_0_1_0_0`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_2_another_revision_is_another_version`, `tools/amqp-dds-endpoint/src/handler.rs::tests::amqp10_2_2_a_refused_header_closes_the_connection_in_the_metrics`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name`.

**Status:** done

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

### §2.4.1, §2.4.2 Opening a Connection, Pipelined Open

**Spec:** `amqp-1.0-transport` §2.4.1 — "each peer MUST send an open frame before sending any other frames"; vor der Aushandlung gelten Frame-Größe 512 und Kanal 0; danach die für beide annehmbaren Grenzen. §2.4.2: Frames dürfen hinter dem eigenen `open` gesendet werden, bevor das des Partners da ist.

**Repo:** Antwortende Seite: `crates/amqp-endpoint/src/peer.rs::Peer` (`on_open`: `open` nur auf Kanal 0, `container-id` Pflicht, `max-frame-size` unter 512 abgelehnt; die Antwort nennt die Grenzen aus `PeerConfig`; es gilt das Kleinere beider Seiten; ein Frame vor `open` und ein zweites `open` schließen mit `amqp:illegal-state`), im Listener `tools/amqp-dds-endpoint/src/handler.rs::do_amqp_phase`. Die Grenzen des Listeners kommen aus `<limits>` der Konfiguration (`tools/amqp-dds-endpoint/src/server.rs::ServerConfig::apply_limits`): `max-frame-size`, `max-sessions-per-connection` minus eins als `channel-max`, `max-links-per-session` minus eins als `begin.handle-max`, `idle-timeout-ms`; eine Verbindung über `max-connections` hinaus wird beim Annehmen geschlossen. Frames, die ein Client hinter seinem `open` ohne Warten sendet, werden in ihrer Reihenfolge verarbeitet. Client: `crates/amqp-endpoint/src/broker.rs::open_connection`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_a_frame_before_open_closes_with_illegal_state`, `tools/amqp-dds-endpoint/src/handler.rs::tests::ddsamqp_2_1_handle_connection_open_close_round_trip`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener`, `tools/amqp-dds-endpoint/src/server.rs::tests::ddsamqp_a_limits_that_admit_nothing_are_refused`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`; RabbitMQ (Client): `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::amqp10_0_2_a_message_the_client_sends_is_read_by_pika`.

**Status:** done

### §2.4.3, §2.4.4 Closing a Connection, Simultaneous Close

**Spec:** `amqp-1.0-transport` §2.4.3 — "each peer MUST write a close frame with a code indicating the reason for closing. This frame MUST be the last thing ever written onto a connection." §2.4.4: beide Seiten können gleichzeitig schließen.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`close` des Partners wird mit `close` beantwortet, danach wird nichts mehr geschrieben; `Peer::close` und `fail_connection` schreiben `close` mit dem Fehler als letzten Frame; ein `close` des Partners, das das eigene kreuzt, beendet die Verbindung); `do_amqp_phase` wartet nach dem eigenen `close` höchstens 2 s auf das des Partners. Die Links der Verbindung gehen an `Container::detached` zurück. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::close`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_3_close_is_answered_with_close_and_ends_the_connection`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_6_after_a_close_with_an_error_frames_are_discarded_until_close`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_3_a_lost_connection_hands_the_links_back`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`.

**Status:** done

### §2.4.5 Idle Timeout of a Connection

**Spec:** `amqp-1.0-transport` §2.4.5 — jede Seite nennt in `open.idle-time-out` die Pause, die sie vom Partner duldet ("SHOULD be half the peer's actual timeout threshold"); wer nichts zu senden hat, sendet einen leeren Frame; bei Überschreitung "a peer SHOULD try to gracefully close the connection using a close frame with an error".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`tick`: leerer Frame auf Kanal 0, wenn seit der Hälfte des `idle-time-out` des Partners nichts gesendet wurde; `close` mit `amqp:resource-limit-exceeded`, wenn für `PeerConfig::idle_time_out_ms` kein Frame kam; `open` nennt die Hälfte davon; leere Frames des Partners zählen auf jedem Kanal). Listener: `HandlerConfig::idle_time_out` (aus `ServerConfig::read_timeout`, das `<limits idle-timeout-ms>` der Konfiguration setzt; 0: keine Grenze), `poll_interval` 20 ms. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::heartbeat`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_5_idle_time_out_is_kept_in_both_directions`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_4_5_the_listener_sends_heartbeats_and_closes_a_silent_connection`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_4_5_proton_with_an_idle_time_out_is_kept_alive_by_the_endpoint`.

**Status:** done

### §2.4.6, §2.4.7 Connection States

**Spec:** `amqp-1.0-transport` §2.4.6/§2.4.7 — Zustände START bis END; DISCARDING: nach einem `close` mit Fehler "any incoming frames on the connection MUST be silently discarded until the peer's close frame is received".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`ConnState`: `AwaitOpen` ist HDR_EXCH, `Opened`, `Discarding` für CLOSE_SENT und DISCARDING, `End`; in HDR_EXCH wird vor einem `close` das eigene `open` gesendet, weil der Zustand nur `open` als Senden zulässt), `crates/amqp-endpoint/src/session.rs::{ConnectionState, advance_connection}`. Die Zustände OPEN_PIPE, OC_PIPE und CLOSE_PIPE gehören einer Seite, die selbst vorab sendet; der Listener sendet sein `open` als Antwort.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_6_after_a_close_with_an_error_frames_are_discarded_until_close`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_a_frame_before_open_closes_with_illegal_state`, `crates/amqp-endpoint/src/session.rs::tests::amqp10_0_2_header_exchange_advances_to_hdr_exch`.

**Status:** done

### §2.5.1 bis §2.5.3 Establishing and Ending a Session

**Spec:** `amqp-1.0-transport` §2.5.1 — "The remote-channel field of a begin frame MUST be empty for a locally initiated session, and MUST be set when announcing the endpoint created as a result of a remotely initiated session"; niedrigster freier Kanal empfohlen. §2.5.2/§2.5.3: `end`, auch gleichzeitig von beiden Seiten.

**Repo:** Antwortende Seite: `crates/amqp-endpoint/src/peer.rs::Peer` (`on_begin`: Antwort auf dem niedrigsten freien ausgehenden Kanal mit `remote-channel`, Fenstern und `handle-max` aus `PeerConfig`; ein Kanal über `channel-max` schließt mit `amqp:connection:framing-error`; `on_end`: `end` wird mit `end` beantwortet, der Kanal ist wieder frei). Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::{begin, end}`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_1_begin_is_answered_on_the_lowest_free_channel_with_remote_channel`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`.

**Status:** partial — jede Seite kann nur eine Rolle: `Peer` beginnt selbst keine Session und schließt bei einem `begin` mit `remote-channel` mit `amqp:not-allowed`; der Client (`broker.rs`) beantwortet ein `begin`, das der Broker von sich aus sendet, nicht, sondern meldet einen Protokollfehler.

### §2.5.4, §2.5.5 Session Errors, Session States

**Spec:** `amqp-1.0-transport` §2.5.4 — "When a session is unable to process input, it MUST indicate this by issuing an END with an appropriate error indicating the cause of the problem. It MUST then proceed to discard all incoming frames from the remote endpoint until receiving the remote endpoint's corresponding end frame." §2.5.5: Zustände UNMAPPED bis DISCARDING.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`fail_session`: `end` mit dem Fehler, der ausgehende Kanal ist frei, die Frames der Session werden bis zum `end` des Partners verworfen, ihre Links gehen an `Container::detached`; andere Sessions der Verbindung bleiben).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_4_a_session_error_ends_the_session_and_discards_until_end`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation`.

**Status:** done

### §2.5.6 Session Flow Control

**Spec:** `amqp-1.0-transport` §2.5.6 — next-incoming-id, incoming-window, next-outgoing-id, outgoing-window, remote-incoming-window, remote-outgoing-window und ihre Fortschreibung beim Senden und Empfangen von `transfer` und `flow`.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_transfer`, `on_flow`, `send`, `session_windows`; das eingehende Fenster wird wieder geöffnet, sobald die Hälfte verbraucht ist; ein `transfer` über das Fenster hinaus beendet die Session mit `amqp:session:window-violation`; das Fenster des Partners zählt je `transfer`-Frame: `send` beginnt eine Zustellung, sobald das Fenster nicht null ist, schreibt die Frames, die es fasst, und `resume_transfers` schreibt die übrigen, wenn ein `flow` es öffnet; bei Fenster null `SendError::Window`). Listener: `ConnectionNodes::pump`. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::{transfer, apply_session_flow, remote_incoming_window}`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_6_the_windows_follow_transfers_and_flows`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_6_a_delivery_above_the_window_is_continued_after_each_flow`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_5_6_the_listener_sends_a_message_above_the_window_in_steps`, `crates/amqp-endpoint/src/broker.rs::tests::a_full_session_window_refuses_the_transfer`.

**Status:** done

### §2.6.1, §2.6.2 Naming a Link, Link Handles

**Spec:** `amqp-1.0-transport` §2.6.1 — der Name bezeichnet den Link zwischen zwei Containern je Richtung; "the second attach succeeds and the first attach MUST then be closed with a link error of stolen". §2.6.2: jede Seite vergibt ihr eigenes Handle.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_attach`: eigenes Handle ist das niedrigste freie; ein Link, der mit gleichem Namen und gleicher Richtung auf derselben Verbindung noch einmal angehängt wird, wird mit `amqp:link:stolen` geschlossen; ein belegtes Handle beendet die Session mit `amqp:session:handle-in-use`, eines über `handle-max` die Verbindung mit `amqp:connection:framing-error`).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_1_a_link_attached_again_is_stolen`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_7_2_a_handle_above_handle_max_closes_with_framing_error`.

**Status:** partial — `stolen` gilt innerhalb einer Verbindung; ein Link gleichen Namens, den derselbe Container über eine zweite Verbindung anhängt, lässt den ersten bestehen (die Verbindungen eines Listeners teilen kein Link-Verzeichnis).

### §2.6.3 Establishing or Resuming a Link

**Spec:** `amqp-1.0-transport` §2.6.3 — `attach` wird mit `attach` beantwortet; ohne Terminus: "the session endpoint will still create a link endpoint and issue an attach indicating that the link endpoint has no associated local terminus. In this case, the session endpoint MUST immediately detach the newly created link endpoint." — "the endpoint MUST report the actual properties of the terminus as created" — "If either end of the link is already associated with a terminus, the attach frame MUST include its unsettled delivery state."

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_attach` mit `Container::attach`: Antwort mit gleichem Namen, der anderen Rolle, den Termini wie angelegt, `initial-delivery-count` als Sender, `max-message-size`; ein abgelehnter Attach wird mit `attach` ohne den eigenen Terminus und einem schließenden `detach` mit dem Fehler beantwortet; auf einen Attach mit `unsettled` antwortet die leere Map). Listener: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::attach` (Adresse ohne Knoten `amqp:not-found`, Zugriffsprüfung `amqp:unauthorized-access`, Terminus ohne Adresse und ein Ziel, das kein `target` ist, `amqp:not-implemented`). Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::attach`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_a_refused_attach_is_answered_with_a_null_terminus_and_a_closing_detach`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `tools/amqp-dds-endpoint/src/handler.rs::tests::ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_3_proton_sees_a_refused_attach_as_not_found`.

**Status:** partial — Anlegen ist gebaut. Wiederaufnahme: der Listener hält keinen Terminus über das Ende eines Links hinaus (er meldet `expiry-policy` `link-detach`), seine `unsettled`-Map ist deshalb immer leer; der Client sendet `attach.unsettled` nie und kann einen Link nicht wiederaufnehmen.

### §2.6.4 bis §2.6.6 Detaching, Link Errors, Closing a Link

**Spec:** `amqp-1.0-transport` §2.6.5 — "When an error occurs at a link endpoint, the endpoint MUST be detached with appropriate error information supplied in the error field of the detach frame. The link endpoint MUST then be destroyed. If any input (other than a detach) related to the endpoint ... be received, the session MUST be terminated with an errant-link session-error." — "Receipt of a pipelined attach MUST result in the session being terminated with an errant-link session-error." §2.6.4/§2.6.6: `detach`, schließend oder nicht.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_detach`: Antwort auf dem eigenen Handle, schließend, wenn der Partner schließt; beide Handles sind wieder frei; `detach` und `fail_link`: schließender `detach` mit Fehler, danach beendet jede Eingabe für das Handle außer `detach` die Session mit `amqp:session:errant-link`, ebenso ein `disposition`, dessen Bereich eine Delivery-Id nennt, die der Link beim Zerstören in einer der beiden Richtungen unerledigt hielt oder mitten in der Übertragung hatte (`on_disposition`; die Ids gelten bis zum `detach` des Partners); ein Attach mit `unsettled` auf dem Handle ist die Wiederaufnahme). Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::detach`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_6_detach_is_answered_and_frees_the_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_a_disposition_for_a_link_detached_with_an_error_ends_the_session`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`.

**Status:** done

### §2.6.7 bis §2.6.10 Flow Control, Synchronous Get, Asynchronous Notification, Stopping a Link

**Spec:** `amqp-1.0-transport` §2.6.7 — `link-credit(snd) := delivery-count(rcv) + link-credit(rcv) - delivery-count(snd)`; "If the link-credit is less than or equal to zero ... a sender MUST NOT send more messages"; "If the sender's drain flag is set and there are no available messages, the sender MUST advance its delivery-count until link-credit is zero, and send its updated flow state to the receiver." §2.6.8 bis §2.6.10: die Muster daraus (ein Kredit, `drain`, Kreditfenster, Kredit 0 mit `echo`).

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` als Sender: `on_flow` (Kredit nach der Formel, ohne `delivery-count` mit dem des eigenen Attach), `send` (`SendError::NoCredit` ohne Kredit), `drained` (Zähler um den Restkredit vor, `flow` mit Kredit 0; nicht, solange eine Zustellung des Links auf das Session-Fenster wartet), `echo` wird mit `flow` beantwortet; `flow.available` (§2.7.4) ist in jedem `flow` eines sendenden Links die Zahl aus `Container::available`. Als Empfänger: Kredit `PeerConfig::link_credit` mit dem Attach, erneuert bei halbem Verbrauch; ein `transfer` ohne Kredit löst den Link mit `amqp:link:transfer-limit-exceeded`. Listener: `ConnectionNodes::pump` (ein Link mit `drain` wird erst geleert gemeldet, wenn sein Knoten nichts mehr für ihn hat), `ConnectionNodes::available` (die Nachrichten des Knotens, die der Link bekommen darf: `Nodes::available`). Client: `crates/amqp-endpoint/src/broker.rs::{BrokerWriter::flow, BrokerWriter::sender_flow, BrokerWriter::session_flow, sender_credit}` (Kredit nach der Formel; liegt die Grenze hinter dem eigenen `delivery-count` (RFC 1982), ist der Kredit 0; ohne `delivery-count` vom `initial-delivery-count` 0 des eigenen Attach). Daemon: `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::on_link_frame` (sendender Link: Kredit, `drain`, `echo`; ein wieder angehängter Link zählt ab 0; empfangender Link: `on_sender_flow` übernimmt den `delivery-count` des Brokers, verbrauchter Kredit wird bei halbem Verbrauch erneuert, `echo` wird mit `flow` beantwortet; `echo` ohne Handle mit `BrokerWriter::session_flow`), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::{drain, answer_flow}` (ohne verfügbare Nachricht Zähler um den Restkredit vor, `flow` mit Kredit 0, `available` und `drain`; nicht, solange eine Nachricht auf das Session-Fenster wartet; nach dem Stopp von §9.2 ist keine Nachricht verfügbar), `crates/amqp-endpoint/src/bridge_daemon/cbs.rs::Cbs::on_inbound` (dasselbe für den Sender und den Empfänger von `$cbs`). Transaktions-Controller: `crates/amqp-endpoint/src/txn.rs::TxnController::on_event` (der Steuerlink sendet nur auf Aufruf, hat also nie eine wartende Nachricht: `drain` verbraucht den Kredit, `echo` wird beantwortet), `crates/amqp-endpoint/src/txn.rs::TxnClient` (`dispatch`: sendende Links leeren und beantworten `echo`, empfangende beantworten `echo`, `echo` ohne Handle mit dem Zustand der Session; `transfer`: ein Link, dessen Nachricht auf Kredit wartet, leert erst nach ihr).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_sender_sends_within_the_credit_and_drains`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_transfer_without_credit_detaches_with_transfer_limit_exceeded`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_receiver_restores_the_credit_when_half_is_used`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_3_4_the_outcomes_act_on_the_node_and_drain_consumes_the_credit`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_7_4_a_flow_of_the_sender_states_the_messages_that_wait_for_credit`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_link_in_the_middle_of_a_delivery_is_not_drained`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_4_the_listener_states_the_messages_that_wait_for_credit`, `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::{amqp10_2_6_7_a_sending_link_asked_to_drain_consumes_the_credit_it_cannot_use, amqp10_2_7_4_a_flow_with_echo_is_answered_with_the_state_of_the_sending_link, amqp10_2_6_7_a_flow_behind_the_deliveries_sent_grants_no_credit, amqp10_2_6_7_a_link_attached_again_counts_from_its_initial_delivery_count, amqp10_2_7_4_a_flow_with_echo_is_answered_with_the_state_of_the_receiving_link, amqp10_2_6_7_a_receiving_link_takes_the_delivery_count_of_the_brokers_flow, amqpbridge_12_1_link_state_credit_follows_part_2_2_6_7, amqpbridge_9_2_a_link_asked_to_drain_after_the_stop_consumes_its_credit}`, `crates/amqp-endpoint/src/bridge_daemon/cbs.rs::tests::{amqp10_2_6_7_the_cbs_sender_drains_and_answers_an_echo, amqp10_2_6_7_the_cbs_receiver_answers_an_echo_and_follows_the_brokers_count}`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_2_6_7_the_control_link_drains_and_answers_an_echo`, `crates/amqp-endpoint/tests/txn_scripted.rs::amqp10_2_6_7_the_txn_client_drains_its_sending_link_and_answers_echoes`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`. Linux-Testhost: grün.

**Status:** partial — die antwortende Seite ist auf beiden Seiten gebaut: der Listener (`Peer`), die Links des Daemons (Topic-Links und `$cbs`) und die Links des Transaktions-Controllers leeren auf `drain` und beantworten `echo`, mit und ohne Handle. Offen: kein Client setzt `flow.echo`, das Anhalten mit Bestätigung (§2.6.10) ist auf der Client-Seite nicht möglich; `flow.drain` setzt nur `TxnClient::receive` (Abruf mit Zeitgrenze, §2.6.8), die empfangenden Links des Daemons fragen nie danach. Gegen einen Broker, der selbst `drain` oder `echo` sendet, ist das Verhalten nur mit dem geskripteten Broker gemessen, nicht gegen einen Fremd-Broker.

### §2.6.11, §2.6.12 Messages, Transferring a Message

**Spec:** `amqp-1.0-transport` §2.6.12 — jede Zustellung hat ein `delivery-tag`; sie ist unerledigt, bis sie erledigt (settled) wird; `disposition` trägt Zustand und Erledigung; Empfänger-Modus `first` oder `second` (§2.8.3).

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` als Empfänger: `on_transfer` übergibt die Zustellung an `Container::deliver` und sendet dessen Ergebnis als `disposition`, erledigt im Modus `first`, unerledigt im Modus `second` bis zur Erledigung durch den Sender; ein erledigter `transfer` bekommt keine. Als Sender: `send` (erledigt oder nicht nach `snd-settle-mode`), `on_disposition` meldet den Zustand an `Container::disposition` und erledigt einen Endzustand, den der Empfänger offen ließ. Client: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::{transfer, disposition}`, `crates/amqp-endpoint/src/client.rs::AmqpClient::send_to_accepted`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_12_an_unsettled_delivery_is_answered_with_its_outcome`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_12_a_disposition_for_what_this_endpoint_sent_is_settled`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `crates/amqp-endpoint/src/client.rs::tests::amqp10_3_4_2_send_to_accepted_returns_on_the_accepting_disposition`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`.

**Status:** done

### §2.6.13 Resuming Deliveries

**Spec:** `amqp-1.0-transport` §2.6.13 — nach dem Wiederanhängen gleichen beide Seiten ihre unerledigten Zustellungen ab; eine mit `resume` gesendete Zustellung, die nur die Quelle für unerledigt hält, "MUST be ignored by the receiver".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_transfer`: ein `transfer` mit `resume`, dessen Tag der Empfänger nicht unerledigt hält, wird nicht zugestellt; `LinkInfo::remote_unsettled` gibt die Map des Partners an den Container).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_13_a_resumed_delivery_this_endpoint_does_not_hold_is_ignored`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session`.

**Status:** partial — keine Seite hält unerledigte Zustellungen über einen Link hinaus: der Listener gibt sie beim Ende des Links an den Knoten zurück (Default-Outcome `released`), der Client sendet weder `attach.unsettled` noch `transfer.resume`. Der Abgleich zweier nicht leerer Maps und `incomplete-unsettled` sind nicht gebaut.

### §2.6.14 Transferring Large Messages

**Spec:** `amqp-1.0-transport` §2.6.14 — eine Nachricht über mehrere `transfer`-Frames (`more`); "messages transferred along a single link MUST NOT be interleaved"; bei `aborted` "the receiver MUST discard the message data that was transferred prior to the abort".

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (`on_transfer`: Frames einer Zustellung werden zusammengesetzt, Folgeframes dürfen `delivery-id` und Tag weglassen, `aborted` verwirft, eine zweite Zustellung vor dem Ende der ersten löst den Link mit `amqp:not-allowed`, eine Nachricht über `max-message-size` mit `amqp:link:message-size-exceeded`; `send` teilt nach der Frame-Größe des Partners; fasst das Session-Fenster des Partners nicht alle Frames, folgen die übrigen nach dem nächsten `flow` (§2.5.6), und bis dahin nimmt der Link keine andere Zustellung an: `SendError::InProgress`). Client: `crates/amqp-endpoint/src/broker.rs::{BrokerWriter::transfer, BrokerReader::next_event}`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_14_a_message_in_several_transfers_is_one_delivery`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_14_a_message_above_the_frame_size_is_sent_in_several_transfers`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_18_a_message_above_max_message_size_detaches_the_link`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_6_a_delivery_above_the_window_is_continued_after_each_flow`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_5_6_the_listener_sends_a_message_above_the_window_in_steps`, `crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_14_proton_sends_a_message_above_the_frame_size`.

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

### §2.8.1 bis §2.8.13 Definitions

**Spec:** `amqp-1.0-transport` §2.8.1 bis §2.8.13 — `role`, `sender-settle-mode`, `receiver-settle-mode`, `handle`, `seconds`, `milliseconds`, `delivery-tag` (bis 32 Oktette), `delivery-number`, `transfer-number`, `sequence-no` (RFC 1982), `message-format`, `ietf-language-tag`, `fields`.

**Repo:** `crates/amqp-endpoint/src/peer.rs::Peer` (Rolle als boolean, Erledigungs-Modi 0 bis 2 und 0 bis 1 mit Ablehnung anderer Werte, Handles, Tag über 32 Oktette abgelehnt, Zähler mit Überlauf nach RFC 1982 in `serial_le` und `wrapping_add`, `message-format` wird durchgereicht). `ietf-language-tag`: die Endpunkte senden keine `outgoing-locales`, es gilt die Vorgabe `en-US`.

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_12_a_disposition_for_what_this_endpoint_sent_is_settled`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_14_a_message_above_the_frame_size_is_sent_in_several_transfers`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_7_a_sender_sends_within_the_credit_and_drains`.

**Status:** done

### §2.8.14 Error

**Spec:** `amqp-1.0-transport` §2.8.14 — `error` (0x1d): `condition` (Pflicht), `description`, `info`.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{AmqpError, encode_error, decode_error}`.

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_2_8_14_an_error_is_0x1d_condition_and_description`.

**Status:** partial — `info` wird nicht gelesen und nicht geschrieben; `AmqpError` hat kein Feld dafür.

### §2.8.15 bis §2.8.18 Error Conditions

**Spec:** `amqp-1.0-transport` §2.8.15 bis §2.8.18 — die Symbole der gemeinsamen, der Verbindungs-, Session- und Link-Fehler.

**Repo:** `crates/amqp-endpoint/src/peer.rs::condition` (alle 25 Symbole). `Peer` sendet: `amqp:decode-error` (Frame-Body nicht dekodierbar oder keine Liste), `amqp:not-implemented` (unbekannter Frame-Body), `amqp:illegal-state`, `amqp:not-allowed`, `amqp:invalid-field`, `amqp:resource-limit-exceeded`, `amqp:connection:framing-error` (Kanal, Handle, Frame-Größe, Frame-Typ), die vier Session-Fehler, `amqp:link:transfer-limit-exceeded`, `amqp:link:message-size-exceeded`, `amqp:link:stolen`; der Listener dazu `amqp:not-found`, `amqp:unauthorized-access`, `amqp:internal-error` und `amqp:resource-limit-exceeded` für eine Knotengrenze (§3.3).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_15_an_undecodable_frame_body_closes_with_its_condition`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_18_a_message_above_max_message_size_detaches_the_link`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_5_4_a_session_error_ends_the_session_and_discards_until_end`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_an_oversized_frame_closes_with_framing_error`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_3_1_a_frame_cut_by_a_read_timeout_is_kept_until_its_rest_arrives`.

**Status:** done

### §2.8.19 Constant Definitions

**Spec:** `amqp-1.0-transport` §2.8.19 — PORT 5672, SECURE-PORT 5671, MAJOR 1, MINOR 0, REVISION 0, MIN-MAX-FRAME-SIZE 512.

**Repo:** `crates/amqp-endpoint/src/peer.rs::MIN_MAX_FRAME_SIZE`, `tools/amqp-dds-endpoint/src/frame_io.rs::protocol::{MAJOR, MINOR, REVISION}`, `tools/amqp-dds-endpoint/src/server.rs::ServerConfig::default_listen` (Port 5672; der Port eines TLS-Listeners kommt aus der Konfiguration).

**Tests:** `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_2_8_19_the_frame_size_is_never_below_512`, `tools/amqp-dds-endpoint/src/frame_io.rs::tests::amqp10_2_2_the_amqp_protocol_header_is_amqp_0_1_0_0`, `tools/amqp-dds-endpoint/src/server.rs::tests::server_config_default_has_sensible_values`.

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

### §3.3 Distribution Nodes

**Spec:** `amqp-1.0-messaging` §3.3 — ein Knoten hält Nachrichten und gibt sie über Links ab; die Zustände einer Nachricht am Knoten (AVAILABLE, ACQUIRED, ARCHIVED).

**Repo:** `tools/amqp-dds-endpoint/src/nodes.rs::Nodes` (Warteschlangen je Adresse für alle Verbindungen eines Listeners: eine Nachricht ist verfügbar, bis ein Link sie nimmt, dann bei diesem Link, bis ein Endzustand sie entfernt oder zurücklegt), `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes`. Die Knoten sind begrenzt (`crates/amqp-endpoint/src/limits.rs::NodeLimits`, Element `<nodes>` der Konfiguration: `max-nodes` 1024 durch Attach angelegte Knoten, `max-messages` 10 000 und `max-octets` 64 MiB je Knoten): ein Attach, der einen Knoten mehr anlegen würde, wird mit `amqp:resource-limit-exceeded` abgelehnt, ein `transfer` an einen vollen Knoten mit demselben Fehler zurückgewiesen (`rejected`); eine freigegebene Nachricht kommt in ihren Knoten zurück, was immer er hält.

**Tests:** `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_3_a_queue_holds_messages_in_order_until_a_link_takes_them`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_2_8_15_nodes_are_bounded_in_number_messages_and_octets`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_7_1_the_limits_of_the_configuration_file_bound_the_listener`.

**Status:** partial — gebaut sind AVAILABLE und ACQUIRED an einer Warteschlange, die jede Nachricht an einen Link abgibt (`move`). Der Zustand ARCHIVED fehlt: es gibt keine Knotenart, die eine Nachricht nach der Abgabe behält (`copy`, §3.5.2).

### §3.4.1, §3.4.6 Received, Resuming Deliveries Using Delivery States

**Spec:** `amqp-1.0-messaging` §3.4.1 — `received` (0x23): `section-number`, `section-offset`. §3.4.6: Wiederaufnahme einer teilweise übertragenen Nachricht ab dieser Stelle.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{DeliveryState, encode_state, decode_state}` (Codec).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_4_1_received_is_0x23_section_number_and_offset`.

**Status:** partial — nur der Codec. Kein Endpunkt sendet `received`, und keiner setzt eine Übertragung an einer gemeldeten Stelle fort (siehe §2.6.13).

### §3.4.2 bis §3.4.5 Accepted, Rejected, Released, Modified

**Spec:** `amqp-1.0-messaging` §3.4.2 bis §3.4.5 — `accepted` (0x24), `rejected` (0x25, `error`), `released` (0x26), `modified` (0x27: `delivery-failed`, `undeliverable-here`, `message-annotations`) und ihre Wirkung an der Quelle.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{Outcome, encode_outcome, decode_outcome}`; Wirkung am Knoten in `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::disposition`: `accepted` und `rejected` entfernen die Nachricht, `released` legt sie zurück, `modified` mit `delivery-failed` zählt `header.delivery-count` hoch (`nodes.rs::Message::count_failed_delivery`), mit `undeliverable-here` bekommt der Link sie nicht wieder; eine Erledigung ohne Endzustand gilt als `released` (Default-Outcome der Quelle).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::amqp10_3_4_the_outcomes_are_their_described_lists`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_3_4_the_outcomes_act_on_the_node_and_drain_consumes_the_credit`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_4_5_an_undeliverable_here_message_skips_that_link`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_4_5_a_failed_delivery_is_counted_in_the_header`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_3_4_proton_releases_modifies_and_rejects_against_the_endpoint`.

**Status:** partial — `modified.message-annotations` wird nicht gelesen und nicht geschrieben (`Outcome::Modified` hat kein Feld dafür); alles andere ist gebaut.

### §3.5.1, §3.5.2, §3.5.7, §3.5.8 Filtering, Distribution Modes, Filter Set

**Spec:** `amqp-1.0-messaging` §3.5.1/§3.5.8 — `filter-set`: benannte Prädikate; "the sending endpoint sets the filter actually in place". §3.5.2/§3.5.7: `move` und `copy`.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::Source::{filter, distribution_mode}` (Codec; ein Filter ist ein beschriebener Wert mit numerischem oder symbolischem Descriptor). Der Listener bietet `move` und keinen Filter und meldet beides in seiner Quelle, was immer der Empfänger wünscht (`ConnectionNodes::attach`); der Empfänger entscheidet (§3.5.3: "The receiving endpoint MUST check that the filter in place meets its needs"). Der Client kann weder Filter noch Verteilungsart anfordern (§3.5.3).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_source_is_0x28_and_its_fields_in_order`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_2_6_proton_sends_receives_settles_and_drains_against_the_endpoint`.

**Status:** partial — gebaut sind der Codec und `move` ohne Filter, so gemeldet. Es fehlen: die Verteilungsart `copy` (eine Knotenart, die Nachrichten nach der Abgabe behält, mit dem Zustand ARCHIVED aus §3.3) und jede Auswertung eines Filters; ein angeforderter Filter und `copy` werden nicht gewährt (der Test `amqp10_2_6_12_…` fordert beides an und prüft, dass `move` und kein Filter gemeldet werden).

### §3.5.3, §3.5.4 Source, Target

**Spec:** `amqp-1.0-messaging` §3.5.3 — `source` (0x28) mit elf Feldern; §3.5.4 — `target` (0x29) mit sieben; `dynamic`: "In this case the address field MUST NOT be set"; der Name eines angelegten Knotens "SHOULD include the link name and the container-id of the remote container".

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{Source, Target, Terminus}` (alle Felder, Encode und Decode; ein Feld mit falschem Typ und `dynamic-node-properties` ohne `dynamic` werden mit dem Feldnamen abgelehnt). `Peer::on_attach` lehnt den Attach mit einer solchen Quelle oder einem solchen Ziel (Descriptor `0x28`, `0x29`) mit `amqp:invalid-field` und dem Text des Decoders ab; nur ein beschriebener Wert anderen Typs im Feld `target` (Part 4: ein Koordinator) geht unverändert an den Container. Listener: `ConnectionNodes::attach` (Quelle mit `default-outcome` `released` und den vier `outcomes`; dynamischer Knoten `dynamic/<container>/<link>/<n>`; `dynamic` mit Adresse `amqp:invalid-field`).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_source_is_0x28_and_its_fields_in_order`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_an_unset_field_is_its_default`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_4_a_target_is_0x29_and_its_fields_in_order`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_field_of_the_wrong_type_is_refused_by_name`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_3_5_3_a_dynamic_node_is_named_after_container_and_link`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_3_5_4_a_malformed_target_is_refused_with_the_field`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_e2e.rs::amqp10_3_5_3_proton_gets_a_dynamic_node_from_the_endpoint`.

**Status:** partial — der Client (`broker.rs::LinkSpec`) sendet von Quelle und Ziel nur `address` und `durable`; `expiry-policy`, `timeout`, `dynamic`, `distribution-mode`, `filter`, `default-outcome`, `outcomes` und `capabilities` kann er nicht anfordern und liest sie aus der Antwort des Brokers nicht.

### §3.5.5, §3.5.6 Terminus Durability, Terminus Expiry Policy

**Spec:** `amqp-1.0-messaging` §3.5.5 — `none` (0), `configuration` (1), `unsettled-state` (2). §3.5.6 — `link-detach`, `session-end`, `connection-close`, `never`.

**Repo:** `crates/amqp-endpoint/src/terminus.rs::{Terminus::durable, ExpiryPolicy}` (Codec, andere Werte abgelehnt), `crates/amqp-endpoint/src/link.rs::TerminusDurability`. Der Listener hält keinen Terminus über den Link hinaus und meldet deshalb `durable` `none` und `expiry-policy` `link-detach`, was immer gewünscht war (§2.6.3: die Eigenschaften wie angelegt). Client: `LinkSpec::durable`.

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_6_the_expiry_policies_are_their_four_symbols`, `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_3_a_field_of_the_wrong_type_is_refused_by_name`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_12_a_message_sent_to_a_node_reaches_a_receiver_of_another_connection`, `crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_2_terminus_durability_from_wire`.

**Status:** partial — gebaut sind der Codec und der nicht dauerhafte Terminus, der mit dem Link endet (`durable` `none`, `expiry-policy` `link-detach`, `timeout` 0, so gemeldet). Es fehlen am Listener: `durable` `configuration` und `unsettled-state`, die Ablaufregeln `session-end`, `connection-close` und `never` und `timeout`; dafür müsste ein Terminus das Ende seines Links überdauern, was auch die Wiederaufnahme aus §2.6.3 und §2.6.13 trägt. Der Client kann nur `durable` anfordern (§3.5.3).

### §3.5.9 bis §3.5.13 Node Properties, Lifetime Policies

**Spec:** `amqp-1.0-messaging` §3.5.9 — `node-properties`: `lifetime-policy`, `supported-dist-modes`. §3.5.10 bis §3.5.13 — `delete-on-close` (0x2b), `delete-on-no-links` (0x2c), `delete-on-no-messages` (0x2d), `delete-on-no-links-or-messages` (0x2e).

**Repo:** `crates/amqp-endpoint/src/terminus.rs::Terminus::dynamic_node_properties` (Codec), `crates/amqp-bridge/src/descriptors.rs` (die vier Descriptoren). Listener: ein dynamischer Knoten lebt so lange wie der Link, der ihn anlegte, und wird mit ihm gelöscht (`delete-on-close`, so gemeldet).

**Tests:** `crates/amqp-endpoint/src/terminus.rs::tests::amqp10_3_5_4_a_target_is_0x29_and_its_fields_in_order`, `tools/amqp-dds-endpoint/tests/link_engine_e2e.rs::amqp10_2_6_3_an_unknown_address_is_not_found_and_a_dynamic_source_is_created`, `crates/amqp-bridge/src/descriptors.rs::tests::amqp10_1_5_a_symbol_and_its_code_name_the_same_type`.

**Status:** partial — nur `delete-on-close` ist gebaut; wer eine der drei anderen Lebensdauern wünscht, bekommt `delete-on-close` gemeldet. `supported-dist-modes` wird nicht gemeldet.

---

## amqp-1.0-transactions

Part 4 (`amqp-core-transactions`, im Standards-Cache als
`internal/standards/cache/oasis/amqp-1.0-transactions.html`). Beide
Rollen sind gebaut: der Listener `tools/amqp-dds-endpoint` ist die
transaktionale Ressource, `crates/amqp-endpoint/src/txn.rs` der
Transaktions-Controller auf der Client-Verbindung. Die Typen liegen in
`crates/amqp-endpoint/src/transactions.rs` (no_std), die Descriptoren
`0x30` bis `0x34` in `crates/amqp-bridge/src/descriptors.rs`.

### §4.1 Transactional Messaging

**Spec:** `amqp-1.0-transactions` §4.1 — ein Container ist die transaktionale Ressource, der andere der Controller; der Controller baut den Kontroll-Link auf und sendet darüber `declare` und `discharge`; "No transactional work is allowed on the control link"; die Arbeit einer Transaktion kann auf jedem Link der Session laufen, mit der Capability auf jedem Link der Verbindung; "If the control link is closed while there exist non-discharged transactions it created, then all such transactions are immediately rolled back, and attempts to perform further transactional work on them will lead to failure."

**Repo:** Ressource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes` (`attach_control`, `coordinate`; das Ende eines Kontroll-Links rollt dessen Transaktionen zurück, ihre Kennungen sind danach unbekannt) mit `crates/amqp-endpoint/src/txn_resource.rs::TxnTable` (Kennungen, die Arbeit je Transaktion, Besitzer ist der Kontroll-Link). Eine Transaktion gilt auf allen Links aller Sessions der Verbindung. Controller: `crates/amqp-endpoint/src/txn.rs::{TxnController, TxnClient}`; ein Kontroll-Link, den die Ressource beendet, schließt den Controller mit dem Fehler des Detach.

**Tests:** `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_1_closing_the_control_link_rolls_back_its_transactions`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_1_closing_the_control_link_rolls_back_its_transactions`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error` (transaktionale Arbeit auf dem Kontroll-Link: Detach mit `amqp:not-allowed`), `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_7_several_transactions_on_a_session_and_one_across_sessions`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_1_the_controller_learns_that_its_control_link_was_terminated`.

**Status:** done — auf dem Linux-Testhost grün.

### §4.2 Declaring a Transaction

**Spec:** `amqp-1.0-transactions` §4.2 — die Ressource stellt einen Koordinator als Ziel bereit; "links to the coordinator cannot be resumed"; `declare` in einer einzigen `amqp-value`-Sektion, andere Sektionen "SHOULD be ignored"; "This message MUST NOT be sent settled"; eine erledigt gesendete Nachricht: "it SHOULD detach with an amqp:illegal-state error"; Erfolg ist `declared` mit der Kennung; ein Fehler ist ein `transaction-error`, als `rejected`, wenn die Quelle des Links `rejected` unterstützt, sonst im Detach des Links; "Transaction controllers SHOULD establish a control link that allows the rejected outcome."

**Repo:** Ressource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::{attach_control, coordinate}`. Der Attach mit dem Ziel `coordinator` wird mit dem Koordinator und seinen Capabilities beantwortet (§4.5.1); ein Kontroll-Link, auf dem der Listener senden sollte, wird mit `amqp:not-implemented` abgelehnt, einer ohne Recht (ACL, Adresse des Koordinators) mit `amqp:unauthorized-access`, mit `<transactions max-transactions="0">` jeder mit `amqp:not-implemented`. Ein `declare` wird mit `declared` beantwortet (`TxnTable::declare`, je Transaktion eine eigene Kennung, höchstens `max-transactions`, darüber `amqp:resource-limit-exceeded`); ein `declare` mit `global-id` wird abgelehnt (§4.5.2). Die Nachricht liest `crates/amqp-endpoint/src/transactions.rs::coordinator_request` (Header, Annotationen, Properties und Footer werden übergangen; kein oder mehr als ein `amqp-value`, ein anderer Körper: `amqp:not-implemented`; nicht dekodierbar: `amqp:decode-error`). Erledigt gesendet: Detach mit `amqp:illegal-state`. Ob ein Fehler als `rejected` oder als Detach geht, entscheidet die Liste `outcomes` der Quelle des Kontroll-Links. Ein Kontroll-Link wird nicht wieder aufgenommen: ein neuer Attach unter demselben Namen kennt keine Transaktion des alten. Controller: `crates/amqp-endpoint/src/txn.rs::TxnController::{link_spec, attach, declare, declare_global}` und `crates/amqp-endpoint/src/broker.rs::LinkSpec::coordinator` (Ziel `coordinator` mit den gewünschten Capabilities, Quelle mit den Outcomes einschließlich `rejected`; `declare` unerledigt in einer `amqp-value`-Sektion; ein Partner, der keinen Koordinator anhängt, ist keine Ressource: Detach mit `amqp:not-implemented`). `TxnClient::connect` prüft die Capabilities des Koordinators gegen die gewünschten (§4.5.1: "it is the responsibility of the transaction controller to verify"): fehlt eine, schließt er die Verbindung und meldet `io::ErrorKind::Unsupported` mit den fehlenden Symbolen. Ohne `amqp:multi-txns-per-ssn` sendet `TxnController` kein zweites `declare`, solange eine Transaktion offen oder ein `declare` unbeantwortet ist (§4.5.7).

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_is_answered_with_declared_and_the_identifier`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_the_transactions_element_of_the_configuration_bounds_the_coordinator`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_1_closing_the_control_link_rolls_back_its_transactions`, `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_2_a_message_without_a_declare_or_discharge_is_no_request`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_2_a_delivery_is_answered_with_an_error_of_the_link_or_later`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_2_the_attach_of_a_control_link_has_a_coordinator_as_its_target`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_the_control_link_sends_unsettled_to_a_coordinator`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_declare_is_sent_unsettled_and_answered_with_the_txn_id`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_detach_of_the_control_link_carries_the_error_of_the_request`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_partner_without_a_coordinator_is_no_transactional_resource`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_2_the_controller_declares_and_discharges_against_the_coordinator`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_1_the_controller_refuses_a_coordinator_without_a_desired_capability`. Qpid Proton als Controller: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint`. ActiveMQ Artemis als Ressource: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_4_artemis_declares_a_txn_id_of_36_octets`.

**Status:** done — auf dem Linux-Testhost grün, die Tests gegen Qpid Proton (python3-qpid-proton 0.37.0) und Apache ActiveMQ Artemis 2.44.0 (OpenJDK 21) mit `--run-ignored all`.

### §4.3 Discharging a Transaction

**Spec:** `amqp-1.0-transactions` §4.3 — `discharge` in einer `amqp-value`-Sektion, `fail` wählt Commit oder Rollback, erledigt gesendet ist ein Fehler; kann der Koordinator nicht abschließen, geht ein `transaction-error` als `rejected` oder im Detach des Kontroll-Links; "the coordinator MUST always be able to complete a discharge where the fail flag is set to true".

**Repo:** Ressource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::{coordinate, commit, rollback}` mit `crates/amqp-endpoint/src/txn_resource.rs::TxnTable::discharge`: Commit wendet die Arbeit an und wird mit `accepted` beantwortet; kann der Listener sie nicht mehr anwenden, wird die Transaktion zurückgerollt und der Commit mit `amqp:transaction:rollback` abgelehnt; `fail` rollt immer zurück und wird mit `accepted` beantwortet, auch für eine abgelaufene oder verurteilte Transaktion; eine Kennung, die keine Transaktion dieses Kontroll-Links nennt, ist `amqp:transaction:unknown-id`. Controller: `crates/amqp-endpoint/src/txn.rs::TxnController::discharge`, `TxnClient::{commit, rollback}`; ein `rejected` mit Fehler oder der Detach des Kontroll-Links wird zu `TxnError::{UnknownId, Rollback, Timeout, Refused, Closed}`.

**Tests:** `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_3_a_discharge_hands_back_the_work_to_apply_or_to_undo`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_3_a_control_link_without_rejected_is_detached_with_the_error`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_3_a_discharge_of_an_unknown_id_is_rejected_with_unknown_id`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_3_a_discharge_names_the_transaction_and_is_answered_with_accepted`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_3_a_rejected_request_carries_the_transaction_error`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_2_the_controller_declares_and_discharges_against_the_coordinator`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_3_proton_reads_the_rollback_of_a_commit_the_endpoint_refuses`.

**Status:** done — auf dem Linux-Testhost grün, der Proton-Test mit `--run-ignored all`.

### §4.4, §4.4.1 Transactional Work, Transactional Posting

**Spec:** `amqp-1.0-transactions` §4.4 — Arbeit ist Einstellen (`transfer`), Erwerben (`flow`) und Abschließen (`disposition`); die Kennungen in Rahmen von der Ressource an den Controller "MUST correctly indicate with which (if any) transaction this work is associated". §4.4.1 — ein eingestellter `transfer` trägt `transactional-state`; "if delivery is split across several transfer frames then all frames MUST be explicitly associated with the same transaction"; ein `discharge` bei unvollständiger Lieferung beendet den Kontroll-Link mit `transaction-rollback`; die Nachricht wird erst mit dem Commit am Knoten verfügbar; eine unerledigte Lieferung wird mit `transactional-state` derselben Transaktion und dem vorläufigen Outcome beantwortet, bevor der Commit gelingen kann.

**Repo:** Ressource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::post` (Nachricht bei der Transaktion gehalten, Antwort `transactional-state{txn-id, accepted}`; jeder Rahmen muss die Transaktion nennen: fehlt sie auf einem späteren Rahmen, ist die Antwort `rejected` mit `amqp:invalid-field` in der Transaktion; nennt erst ein späterer Rahmen eine Transaktion, wird die Lieferung nicht eingestellt und mit `amqp:invalid-field` außerhalb jeder Transaktion abgelehnt, erledigt gesendet wird der Link mit dem Fehler getrennt; eine Transaktion, die nur ein Teil der Rahmen nennt, kann nicht mehr committen; eine Lieferung ist teilweise gegen jede Transaktion eingestellt, die einer ihrer bisherigen Rahmen nennt; `discharge` bei offener Lieferung: die Transaktionen des Kontroll-Links werden zurückgerollt, dann folgt sein Detach mit `amqp:transaction:rollback`), `tools/amqp-dds-endpoint/src/nodes.rs::Nodes::put_all` (die Nachrichten einer Transaktion kommen alle oder keine an ihre Knoten), `crates/amqp-endpoint/src/peer.rs` (der Zustand des ersten Rahmens geht an den Container, ebenso jede Transaktion, die ein späterer Rahmen nennt: `Container::delivery_named`, `Delivery::txns`; die Antwort geht an den Sender). Controller: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::transfer_with_state` (der Zustand auf jedem Rahmen), `crates/amqp-endpoint/src/txn.rs::{posted, TxnClient::send, TxnClient::send_settled}`.

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_posted_message_reaches_the_node_only_with_the_commit`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_rolled_back_posting_reaches_no_node`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_discharge_with_a_partial_delivery_terminates_the_control_link`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_every_frame_of_a_posted_delivery_names_the_transaction`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_delivery_whose_later_frame_names_the_transaction_is_rejected`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_1_a_transaction_a_later_frame_names_has_a_partial_delivery`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_control_link_terminated_for_a_partial_delivery_goes_after_the_abort`, `tools/amqp-dds-endpoint/src/nodes.rs::tests::amqp10_4_4_1_the_messages_of_a_transaction_reach_their_nodes_all_or_none`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_1_the_state_of_a_transfer_reaches_the_container_and_its_answer_the_sender`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_4_1_a_posted_message_names_the_transaction_on_every_frame`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_4_the_states_of_transactional_work_name_the_transaction`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_1_the_controller_posts_in_a_transaction_and_commits_or_rolls_back`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_1_a_message_of_several_frames_is_posted_whole`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_4_1_a_committed_send_is_in_the_artemis_queue_and_a_rolled_back_one_is_not`.

**Status:** done — auf dem Linux-Testhost grün, die Tests gegen Proton und Artemis mit `--run-ignored all`.

### §4.4.2 Transactional Retirement

**Spec:** `amqp-1.0-transactions` §4.4.2 — der Controller verbindet das Outcome einer Lieferung über eine `disposition` mit `transactional-state` mit einer Transaktion; "the delivery MUST NOT be associated with a different non-discharged transaction than the outcome. If this happens then the control link MUST be terminated with a transaction-rollback error"; mit dem Commit wird das Outcome angewendet, die Lieferungen können erledigt werden; nach einem Rollback bleibt die Lieferung beim Controller.

**Repo:** Ressource: `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::retire` (das Outcome wird bei der Transaktion vermerkt, die Lieferung bleibt unerledigt; mit dem Commit wendet der Listener es an und erledigt die Lieferung mit ihm; nach einem Rollback bleibt sie beim Controller; eine Lieferung in einer zweiten offenen Transaktion beendet den Kontroll-Link mit `amqp:transaction:rollback`; ein Outcome in einer unbekannten Transaktion trennt den Link der Lieferung mit `amqp:transaction:unknown-id`). `crates/amqp-endpoint/src/peer.rs` lässt eine Lieferung mit transaktionaler `disposition` unerledigt, bis sie erledigt wird. Controller: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::disposition_with_state`, `crates/amqp-endpoint/src/txn.rs::{retired, TxnClient::retire, TxnClient::settled}`.

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_an_outcome_in_a_transaction_applies_with_the_commit`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_a_rolled_back_outcome_is_not_applied`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_an_outcome_in_another_transaction_terminates_the_control_link`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_an_outcome_in_an_unknown_transaction_detaches_the_link_with_unknown_id`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_2_a_transactional_disposition_leaves_the_delivery_unsettled_until_it_is_settled`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_4_dispositions_and_flows_name_the_transaction`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_2_the_controller_retires_in_a_transaction`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_proton_declares_posts_aborts_retires_and_commits_against_the_endpoint`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_4_2_a_rolled_back_accept_is_delivered_again_by_artemis`.

**Status:** done — auf dem Linux-Testhost grün, die Tests gegen Proton und Artemis mit `--run-ignored all`.

### §4.4.3 Transactional Acquisition

**Spec:** `amqp-1.0-transactions` §4.4.3 — ein `txn-id` in `flow.properties` legt fest, in welcher Transaktion ein sendender Link Nachrichten erwirbt; mit dem `discharge` wird er gelöscht; ein Link ohne transaktionales Erwerben "MUST be terminated with a not-implemented error"; der Controller "SHOULD ensure there is no outstanding credit at the sender before it discharges the transaction"; ist eine erworbene Nachricht bei einem Rollback nur teilweise gesendet, "the resource MUST abort or complete sending the remainder of the message before completing the discharge".

**Repo:** Ressource: `tools/amqp-dds-endpoint/src/handler.rs` (der `txn-id` eines `flow` setzt die Transaktion des Links, `crates/amqp-endpoint/src/transactions.rs::flow_txn_id`; erworbene Nachrichten gehen mit `transactional-state{txn-id}`; ein Rollback legt sie in ihrer Reihenfolge zurück und erledigt die Lieferungen als `released`; ein Commit lässt das Erwerben stehen und wendet ein Outcome derselben Transaktion an; ein Outcome außerhalb der Transaktion für eine darin erworbene Lieferung beendet den Kontroll-Link mit `amqp:transaction:rollback`; ein unbekannter `txn-id` trennt den Link mit `amqp:transaction:unknown-id`; eine teilweise gesendete Nachricht wird vor der Antwort auf den Rollback abgebrochen, ebenso vor dem Detach des Kontroll-Links, wenn ein abgelehnter Commit ohne `rejected` in der Quelle oder ein `amqp:transaction:rollback` nach §4.4.1 oder §4.4.2 den Link beendet (`ConnectionNodes::{end_control, roll_back_control}`; `Action::Detach` wartet wie `Action::Answer`); `ConnectionNodes::clear_acquiring` löscht die Transaktion der Links mit dem `discharge`). Der Listener erwirbt auf jedem sendenden Link transaktional, `not-implemented` tritt nicht ein. Controller: `crates/amqp-endpoint/src/broker.rs::BrokerWriter::flow_in_txn`, `crates/amqp-endpoint/src/txn.rs::TxnClient::receive` (sendet den `txn-id`, wenn er sich ändert, und leert den Link mit `drain`, wenn keine Nachricht kommt, damit kein Kredit beim Sender bleibt).

**Tests:** `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_messages_acquired_in_a_transaction_return_with_the_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_committed_acquisition_stands_and_its_outcome_applies`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_acquisition_in_an_unknown_transaction_and_an_outcome_outside_it_are_refused`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_message_sent_in_part_is_aborted_before_the_rollback_is_answered`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_refused_commit_detaches_the_control_link_after_the_abort`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_a_control_link_terminated_for_a_partial_delivery_goes_after_the_abort`, `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_4_3_the_txn_id_of_a_flow_is_an_entry_of_its_properties`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_3_the_txn_id_of_a_flow_reaches_the_container`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_4_3_a_delivery_sent_with_a_state_carries_it_on_every_frame_and_can_be_aborted`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_4_3_the_controller_acquires_in_a_transaction`.

**Status:** done — auf dem Linux-Testhost grün. Gegen eine fremde Implementierung ist das transaktionale Erwerben nicht gemessen: die Klasse `Transaction` von Qpid Proton (Python) bietet Einstellen und Abschließen, kein Erwerben; gegen Artemis ist es nicht geprüft.

### §4.4.4 Interaction of Settlement with Transactions

**Spec:** `amqp-1.0-transactions` §4.4.4 — Einstellen: vom Controller erledigt gesendet, verfügbar erst mit dem Commit, kann die Ressource sie nicht verarbeiten, "it MUST NOT allow the successful discharge"; unerledigt gesendet, das Outcome muss vor dem Commit mit derselben Transaktion gemeldet sein, kann die Ressource es nicht mehr anwenden, darf der Commit nicht gelingen; "The controller SHOULD settle any outstanding unsettled deliveries in a timely fashion". Abschließen: vom Controller erledigt, nach einem Rollback "the resource MUST apply the default outcome"; nicht erledigt, nach einem Rollback der Zustand von vorher. Erwerben: mit dem Rollback werden Erwerben und Outcome zurückgenommen; ein Outcome vor dem `discharge` "MUST be associated with the same transaction".

**Repo:** `tools/amqp-dds-endpoint/src/handler.rs::ConnectionNodes::{post, commit, rollback, retire}` mit `crates/amqp-endpoint/src/txn_resource.rs::TxnTable` (eine verurteilte Transaktion lehnt den Commit mit dem Grund ab und nimmt den Rollback an): eine erledigt gesendete Lieferung, die der Knoten nicht nehmen kann, verurteilt die Transaktion; ein Commit, dessen Nachrichten die Knoten nicht mehr fassen, wird mit `amqp:transaction:rollback` abgelehnt und nichts erreicht einen Knoten; eine vom Controller mit transaktionalem Outcome erledigte Lieferung bekommt nach dem Rollback das Default-Outcome der Quelle, `released`; eine nicht erledigte bleibt beim Controller; erworbene Nachrichten kommen mit dem Rollback zurück. Controller: `crates/amqp-endpoint/src/txn.rs::TxnClient::send` erledigt eine eingestellte Lieferung nach der Antwort der Ressource.

**Tests:** `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_4_4_a_doomed_transaction_refuses_the_commit_and_takes_the_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_4_a_commit_the_nodes_can_no_longer_take_is_rejected_with_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_2_a_rolled_back_outcome_is_not_applied`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_messages_acquired_in_a_transaction_return_with_the_rollback`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_4_3_acquisition_in_an_unknown_transaction_and_an_outcome_outside_it_are_refused`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_8_the_controller_reads_the_transaction_errors`. Qpid Proton: `tools/amqp-dds-endpoint/tests/proton_txn_e2e.rs::amqp10_4_3_proton_reads_the_rollback_of_a_commit_the_endpoint_refuses`.

**Status:** done — auf dem Linux-Testhost grün, der Proton-Test mit `--run-ignored all`.

### §4.5.1 Coordinator

**Spec:** `amqp-1.0-transactions` §4.5.1 — `coordinator` (0x30, `amqp:coordinator:list`), ein Ziel mit dem Feld `capabilities` (Symbole): vom Controller die gewünschten, von der Ressource die vorhandenen.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::Coordinator` (Encode und Decode, numerischer oder symbolischer Descriptor), `crates/amqp-bridge/src/descriptors.rs` (`0x30` bis `0x34`), `crates/amqp-endpoint/src/peer.rs` (ein Ziel `coordinator` geht als `LinkInfo::coordinator` an den Container, `Attached::coordinator` in die Antwort), `crates/amqp-endpoint/src/broker.rs` (Attach und Lesen auf der Client-Seite). Der Listener meldet `amqp:local-transactions`, `amqp:multi-txns-per-ssn` und `amqp:multi-ssns-per-txn`. Controller: `TxnClient::connect` vergleicht die Capabilities des Koordinators mit den gewünschten (`Coordinator::lacking`); fehlt eine, schließt er die Verbindung und meldet `io::ErrorKind::Unsupported`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_1_a_coordinator_is_0x30_and_its_capabilities`, `crates/amqp-endpoint/tests/peer_engine.rs::amqp10_4_5_1_a_coordinator_target_is_answered_with_the_capabilities_of_the_container`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_is_answered_with_declared_and_the_identifier`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_1_the_controller_refuses_a_coordinator_without_a_desired_capability`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_4_1_a_committed_send_is_in_the_artemis_queue_and_a_rolled_back_one_is_not` (der Koordinator von Artemis meldet `amqp:local-transactions`), `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_7_two_transactions_on_one_session_with_artemis` (Artemis 2.44.0 meldet `amqp:local-transactions`, `amqp:multi-txns-per-ssn` und `amqp:multi-ssns-per-txn`; ein Controller, der `amqp:distributed-transactions` wünscht, verbindet sich nicht).

**Status:** done — auf dem Linux-Testhost grün, der Artemis-Test mit `--run-ignored all`.

### §4.5.2 Declare

**Spec:** `amqp-1.0-transactions` §4.5.2 — `declare` (0x31) mit dem optionalen Feld `global-id`; "This field MUST NOT be set if the coordinator does not have the distributed-transactions capability."

**Repo:** `crates/amqp-endpoint/src/transactions.rs::{Declare, declare_message, coordinator_request}`. Controller: `TxnController::declare_global` sendet `global-id` nur an einen Koordinator mit `amqp:distributed-transactions`, sonst `InvalidInput`. Listener: hat diese Capability nicht und lehnt ein `declare` mit `global-id` mit `amqp:not-implemented` ab (verteilte Transaktionen beschreibt Part 6, den OASIS nicht veröffentlicht hat).

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_2_declare_is_0x31_with_an_optional_global_id`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_2_a_declare_is_sent_unsettled_and_answered_with_the_txn_id`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_2_a_declare_the_coordinator_cannot_perform_is_rejected_with_the_error`.

**Status:** done — auf dem Linux-Testhost grün.

### §4.5.3 Discharge

**Spec:** `amqp-1.0-transactions` §4.5.3 — `discharge` (0x32) mit dem Pflichtfeld `txn-id` und `fail` (boolean).

**Repo:** `crates/amqp-endpoint/src/transactions.rs::{Discharge, discharge_message, coordinator_request}` (`fail` ohne Wert ist `false`; ohne `txn-id` oder mit falschem Typ: `amqp:invalid-field`).

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_3_discharge_is_0x32_txn_id_and_fail`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_3_a_discharge_names_the_transaction_and_is_answered_with_accepted`.

**Status:** done — auf dem Linux-Testhost grün.

### §4.5.4 Transaction Id

**Spec:** `amqp-1.0-transactions` §4.5.4 — `transaction-id`, ein `binary`: "A transaction-id can be up to 32 octets of binary data."

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TxnId` (`new` und `decode` nehmen höchstens 32 Oktette; der Listener vergibt 8). Ein längerer `txn-id` in einem `discharge` wird mit `amqp:invalid-field` abgelehnt. Apache ActiveMQ Artemis 2.44.0 vergibt 36 Oktette (den Text einer UUID); der Controller lehnt ein solches `declared` per Default als Protokollfehler ab und nimmt es erst nach `TxnClient::set_txn_id_limit` an (`BrokerReader::set_txn_id_limit`, `TxnId::decode_up_to`).

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_4_a_txn_id_is_a_binary_of_at_most_32_octets`, `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_4_a_longer_txn_id_is_read_only_up_to_the_given_bound`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_4_the_reader_takes_longer_txn_ids_only_when_told`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_3_a_discharge_of_an_unknown_id_is_rejected_with_unknown_id`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_4_artemis_declares_a_txn_id_of_36_octets`.

**Status:** done — auf dem Linux-Testhost grün, der Artemis-Test mit `--run-ignored all`.

### §4.5.5 Declared

**Spec:** `amqp-1.0-transactions` §4.5.5 — `declared` (0x33), Delivery-State und Outcome, mit dem Pflichtfeld `txn-id`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::Declared`, `crates/amqp-endpoint/src/terminus.rs::{DeliveryState::Declared, decode_state, encode_state}`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_5_declared_is_0x33_and_the_txn_id`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states`.

**Status:** done — auf dem Linux-Testhost grün.

### §4.5.6 Transactional State

**Spec:** `amqp-1.0-transactions` §4.5.6 — `transactional-state` (0x34) mit dem Pflichtfeld `txn-id` und dem vorläufigen `outcome`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TransactionalState` (ein `outcome`, das keines aus Part 3 §3.4 ist, wird abgelehnt), `crates/amqp-endpoint/src/terminus.rs::DeliveryState::Transactional`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_6_a_transactional_state_is_0x34_txn_id_and_outcome`, `crates/amqp-endpoint/src/broker.rs::tests::amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states`.

**Status:** done — auf dem Linux-Testhost grün.

### §4.5.7 Txn Capability

**Spec:** `amqp-1.0-transactions` §4.5.7 — die Symbole `amqp:local-transactions`, `amqp:distributed-transactions`, `amqp:promotable-transactions`, `amqp:multi-txns-per-ssn`, `amqp:multi-ssns-per-txn`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TxnCapability` (alle fünf; ein anderes Symbol in `coordinator.capabilities` ist ein Fehler des Decoders). Der Listener hat und meldet `amqp:local-transactions`, `amqp:multi-txns-per-ssn` (mehrere offene Transaktionen je Session) und `amqp:multi-ssns-per-txn` (eine Transaktion über die Sessions der Verbindung); `distributed-transactions` und `promotable-transactions` meldet er nicht. Controller: hat der Koordinator `amqp:multi-txns-per-ssn` nicht, sendet `TxnController` kein zweites `declare` auf der Session, solange eine Transaktion offen oder ein `declare` unbeantwortet ist (`TxnController::{open_transactions, admits_declare}`; `TxnClient::declare` meldet `TxnError::Unsupported`). Diese Sperre ist nur gegen einen Koordinator aus Testereignissen geprüft: der Listener und Artemis 2.44.0 melden `amqp:multi-txns-per-ssn`; gegen Artemis ist geprüft, dass zwei Transaktionen auf einer Session offen sein können.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_7_the_txn_capabilities_are_their_five_symbols`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_7_several_transactions_on_a_session_and_one_across_sessions`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_5_7_without_multi_txns_per_ssn_one_transaction_at_a_time`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_5_7_with_multi_txns_per_ssn_several_transactions_are_open`. ActiveMQ Artemis: `crates/amqp-endpoint/tests/artemis_txn_e2e.rs::amqp10_4_5_7_two_transactions_on_one_session_with_artemis`.

**Status:** done — auf dem Linux-Testhost grün, der Artemis-Test mit `--run-ignored all`.

### §4.5.8 Transaction Error

**Spec:** `amqp-1.0-transactions` §4.5.8 — `amqp:transaction:unknown-id`, `amqp:transaction:rollback`, `amqp:transaction:timeout`.

**Repo:** `crates/amqp-endpoint/src/transactions.rs::TransactionError`; Listener: `unknown-id` für eine Kennung ohne Transaktion, `rollback` für einen Commit, der nicht gelingen kann, und die Fehlerfälle aus §4.4.1 und §4.4.2, `timeout` nach `<transactions timeout-ms>` (Default im README von `tools/amqp-dds-endpoint`). Controller: `crates/amqp-endpoint/src/txn.rs::TxnError::from_error`.

**Tests:** `crates/amqp-endpoint/src/transactions.rs::tests::amqp10_4_5_8_the_transaction_errors_are_their_three_symbols`, `crates/amqp-endpoint/src/txn_resource.rs::tests::amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout`, `tools/amqp-dds-endpoint/tests/txn_resource_e2e.rs::amqp10_4_5_8_a_transaction_past_its_time_is_rolled_back_with_timeout`, `crates/amqp-endpoint/src/txn.rs::tests::amqp10_4_5_8_an_error_of_the_coordinator_is_read_by_its_condition`, `tools/amqp-dds-endpoint/tests/txn_controller_e2e.rs::amqp10_4_5_8_the_controller_reads_the_transaction_errors`.

**Status:** done — auf dem Linux-Testhost grün.

---

## amqp-1.0-security

### §5.1 Security Layers

**Spec:** `amqp-1.0-security` §5.1 — Sicherheitsschichten (TLS, SASL) liegen unter der AMQP-Verbindung und werden vor ihr ausgehandelt.

**Repo:** Listener: `tools/amqp-dds-endpoint/src/server.rs::serve_one` (TLS, dann `handle_connection`), `tools/amqp-dds-endpoint/src/handler.rs::{serve, do_sasl_phase}` (SASL-Header, dann AMQP-Header). Client: `crates/amqp-endpoint/src/broker.rs::connect` (TLS, SASL, `open`).

**Tests:** `tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_10_1_a_tls_client_is_offered_plain_and_authenticated_against_the_store`, `tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_2_1_an_mtls_client_authenticates_with_external`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`, `tools/amqp-dds-endpoint/tests/version_negotiation.rs::amqp10_2_2_an_unsupported_header_after_sasl_is_answered_with_the_amqp_header`.

**Status:** done

### §5.2 TLS

**Spec:** `amqp-1.0-security` §5.2 — TLS wird mit dem Protokoll-Header `AMQP%d2.1.0.0` ausgehandelt; danach folgt der TLS-Handshake auf derselben Verbindung.

**Repo:** —

**Tests:** —

**Status:** open — der Header mit Protokoll-Id 2 ist auf keiner Seite gebaut: der Listener beantwortet ihn wie jede andere nicht unterstützte Protokoll-Id mit seinem Header und schließt (§2.2), der Client sendet ihn nie. TLS gibt es nur von Beginn der Verbindung an (§5.2.1).

### §5.2.1 Alternative Establishment

**Spec:** `amqp-1.0-security` §5.2.1 — TLS von Beginn der TCP-Verbindung an (Port 5671), ohne den TLS-Protokoll-Header.

**Repo:** Listener: `tools/amqp-dds-endpoint/src/tls.rs::accept_server`, `tools/amqp-dds-endpoint/src/server.rs::serve_one`. Client: `crates/amqp-endpoint/src/broker.rs::connect` (rustls, ALPN `amqp`, Servername aus `ConnectParams::host`).

**Tests:** `tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_10_1_a_tls_client_is_offered_plain_and_authenticated_against_the_store`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`.

**Status:** done

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

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::{amqp10_5_3_3_1_the_mechanism_names_are_plain_anonymous_external, amqp10_5_plain_authenticates_on_correct_credentials, amqp10_5_plain_fails_on_wrong_credentials, amqp10_5_anonymous_authenticates_without_credentials, amqp10_5_external_uses_transport_subject, amqp10_5_3_3_6_the_sasl_codes_are_0_to_4, amqp10_5_sasl_code_unknown_value_rejected}`; `crates/amqp-bridge/src/performatives.rs::tests::{amqp10_5_3_3_2_sasl_init_is_mechanism_response_and_hostname, amqp10_5_3_3_1_the_offered_mechanisms_are_read_from_an_array_or_one_symbol, amqp10_5_3_3_5_the_outcome_code_is_read}`; `tools/amqp-dds-endpoint/src/handler.rs::tests::amqp10_5_3_3_1_no_empty_list_of_mechanisms_is_sent`; RabbitMQ: `crates/amqp-endpoint/tests/rabbitmq_amqp10_e2e.rs::amqp10_0_2_a_message_the_client_sends_is_read_by_pika` (SASL PLAIN).

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

64 done / 14 partial / 1 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document amqp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-bridge --all-targets` (Job `merge-rust-amqp-bridge`) — 67 Tests grün.
* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (Job `merge-rust-amqp-endpoint`) — 10 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (Job `integration-bridges-vendor-amqp`) — 3 Tests grün.

80 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `amqp-1.0.open.md`.
