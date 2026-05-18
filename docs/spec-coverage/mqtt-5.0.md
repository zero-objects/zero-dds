# OASIS MQTT v5.0 — Spec-Coverage

**Spec:** `docs/standards/cache/oasis/mqtt-5.0.html` (OASIS Standard
07 March 2019).

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** MQTT 5.0 ist die OASIS-Pub-Sub-Spec fuer IoT/M2M.
ZeroDDS implementiert den **Wire-Codec** (Spec §1.5 Data Types,
§2.1 Fixed Header, §2.2 Variable Header + Properties, §3.3 PUBLISH)
als pure-Rust no_std+alloc Library im Crate `crates/mqtt-bridge/`.
Broker-Logic (Session-State, Topic-Filter-Matching, QoS-Retransmission)
ist Caller-Layer.

Implementation: `crates/mqtt-bridge/` (5 Module, 36 Tests gruen).

---

## §1 Introduction

### §1.1-§1.4 Background

**Spec:** §1, S. 7-10 (Spec) — Abstract, Status, Status of this Doc.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Hintergrund; ohne Code-Mapping.

### §1.5 Data Types

**Spec:** §1.5 — alle MQTT-Datatypes:
* §1.5.1 Bits — implizit via `u8`.
* §1.5.2 Two Byte Integer (BE u16).
* §1.5.3 Four Byte Integer (BE u32) — implizit via `u32::to_be_bytes`.
* §1.5.4 UTF-8 Encoded String (u16 BE length + UTF-8).
* §1.5.5 Variable Byte Integer (1-4 bytes, 7-bit-groups +
  continuation-bit, max 268,435,455).
* §1.5.6 Binary Data (u16 BE length + bytes).
* §1.5.7 UTF-8 String Pair — Composition aus 2 §1.5.4-Strings (Caller
  baut via 2 `encode_utf8_string`-Aufrufen).

**Repo:** `crates/mqtt-bridge/src/data_types.rs` +
`crates/mqtt-bridge/src/vbi.rs`.

**Tests:** `data_types::tests::two_byte_int_round_trip`,
`utf8_string_round_trip`, `utf8_string_starts_with_be_length`,
`utf8_string_decode_invalid_utf8`, `utf8_string_decode_truncated`,
`binary_data_round_trip`; `vbi::tests::*` (10 Tests, alle Boundaries
0/127/128/16383/16384/2097151/2097152/MAX, 5-byte-Reject).

**Status:** done

---

## §2 MQTT Control Packet Format

### §2.1 Structure of an MQTT Control Packet

**Spec:** §2.1, S. 19-21 — Fixed Header (Type-Nibble + Flags-Nibble +
Remaining Length VBI), Variable Header, Payload.

**Repo:** `crates/mqtt-bridge/src/packet.rs::FixedHeader`.

**Tests:** `packet::tests::publish_header_decodes_dup_qos_retain_flags`,
`codec::tests::fixed_header_first_byte_layout_for_publish`.

**Status:** done

### §2.1.2 Control Packet Type (Table 2-1)

**Spec:** §2.1.2 — 15 Packet-Types (CONNECT=1, ..., AUTH=15).

**Repo:** `crates/mqtt-bridge/src/packet.rs::ControlPacketType` mit
allen 15 Werten.

**Tests:** `packet::tests::packet_type_round_trip_via_bits`,
`packet_type_zero_is_reserved`.

**Status:** done

### §2.1.4 Remaining Length

**Spec:** §2.1.4 — VBI; Bytes nach Fixed Header bis Ende des Pakets.

**Repo:** Cross-Ref VBI-Modul.

**Tests:** `codec::tests::large_payload_encodes_multibyte_remaining_length`,
`truncated_remaining_length_decode_fails`.

**Status:** done

### §2.2 Variable Header

**Spec:** §2.2, S. 21 — Per-Packet-Type Variable-Header-Inhalt.

**Repo:** PUBLISH-Variable-Header in
`crates/mqtt-bridge/src/codec.rs::encode_publish/decode_publish`.

**Tests:** Cross-Ref `publish_qos*_round_trip`.

**Status:** done — fuer PUBLISH; weitere Packet-Types analog
implementierbar (siehe `.open.md`).

### §2.2.2 Properties

**Spec:** §2.2.2, S. 23-25 — VBI-Length-prefixed Property-Block;
Property-Identifier (Spec Table 2-4) + Wert.

**Repo:** `crates/mqtt-bridge/src/properties.rs::{PropertyId, Property}`
mit 15 named-IDs + `Other(u32)`-Variante. Codec serialisiert
Property-Block-Body als opaque Bytes mit VBI-Length-Prefix.

**Tests:** `properties::tests::property_id_round_trip_for_all_named`,
`property_id_unknown_yields_other_variant`,
`well_known_property_ids_match_spec_values`,
`codec::tests::empty_properties_round_trips_as_empty_vec`,
`non_empty_properties_round_trip_preserves_bytes`.

**Status:** done — Identifier-Registry + Wire-Length-Prefix +
`PropertyValueKind`-Tabelle nach Spec §2.2.2.2 Table 2-4 mit
Decode-Helpers (`decode_two_byte_int`, `decode_four_byte_int`,
`decode_utf8_string`, `decode_binary_data`) und
`PropertyId::value_kind()` als zentralem Dispatch.

---

## §3 MQTT Control Packets

### §3.1 CONNECT

**Spec:** §3.1, S. 30-46 — Verbindungsaufbau mit Will/Properties/
Authentication.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs::{encode_connect_body, decode_connect_body}` (`ConnectBody`).

**Tests:** `control_packets::tests::connect_round_trip`.

**Status:** done

### §3.2 CONNACK

**Spec:** §3.2 — Connect Acknowledgment.

**Repo:** `control_packets::{encode_connack_body, decode_connack_body}` (`ConnackBody`).

**Tests:** `control_packets::tests::connack_round_trip`.

**Status:** done

### §3.3 PUBLISH — Voll abgedeckt

**Spec:** §3.3, S. 48-58 — Vollstaendige PUBLISH-Packet-Spec mit
DUP/QoS/RETAIN-Flags, Topic-Name, Packet-Identifier (bei QoS>0),
Properties, Payload.

**Repo:** `crates/mqtt-bridge/src/codec.rs::encode_publish/
decode_publish` voll implementiert; QoS-3 wird abgelehnt
(MalformedPacket), Packet-Identifier-Pflicht bei QoS>0 enforced.

**Tests:** `codec::tests::publish_qos0_no_packet_id_round_trip`,
`publish_qos1_includes_packet_id_round_trip`,
`publish_qos2_round_trip`, `invalid_qos_3_rejected_on_encode`,
`missing_packet_id_at_qos1_rejected`,
`wrong_packet_type_rejected_on_decode`,
`fixed_header_first_byte_layout_for_publish`,
`empty_properties_round_trips_as_empty_vec`,
`non_empty_properties_round_trip_preserves_bytes`,
`truncated_remaining_length_decode_fails`,
`empty_input_decode_fails`,
`large_payload_encodes_multibyte_remaining_length`.

**Status:** done

### §3.4 PUBACK

**Spec:** §3.4 — Acknowledgment für QoS-1-PUBLISH.

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`
(gemeinsame `AckBody`-Struktur).

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.5 PUBREC

**Spec:** §3.5 — QoS-2 Receive-Acknowledgment (Schritt 1 des
4-Way-Handshake).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`
(gleiche AckBody-Struktur).

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.6 PUBREL

**Spec:** §3.6 — QoS-2 Publish-Release (Schritt 2).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.7 PUBCOMP

**Spec:** §3.7 — QoS-2 Publish-Complete (Schritt 3, finaler ACK).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.8 SUBSCRIBE

**Spec:** §3.8 — Client-zu-Server-Subscription-Request mit
Topic-Filter-Liste.

**Repo:** `control_packets::{encode_subscribe_body,
decode_subscribe_body}`.

**Tests:** `control_packets::tests::subscribe_round_trip`.

**Status:** done

### §3.9 SUBACK

**Spec:** §3.9 — Server-zu-Client SUBSCRIBE-Acknowledgment mit
Reason-Codes pro Subscription.

**Repo:** `control_packets::{encode_suback_body,
decode_suback_body}`.

**Tests:** `control_packets::tests::suback_round_trip`.

**Status:** done

### §3.10 UNSUBSCRIBE

**Spec:** §3.10 — Client-Unsubscribe-Request.

**Repo:** `control_packets::{encode_unsubscribe_body,
decode_unsubscribe_body}`.

**Tests:** `control_packets::tests::unsubscribe_round_trip`.

**Status:** done

### §3.11 UNSUBACK

**Spec:** §3.11 — Server-UNSUBSCRIBE-Acknowledgment mit Reason-Codes.

**Repo:** `control_packets::{encode_unsuback_body,
decode_unsuback_body}`.

**Tests:** `control_packets::tests::unsuback_round_trip`.

**Status:** done

### §3.12 PINGREQ

**Spec:** §3.12 — 2-byte Heartbeat (Type-Byte + Remaining=0).

**Repo:** Konstante `0xC0`; Encode trivial (1 Byte 0xC0 + VBI(0)).

**Tests:** `packet::tests::packet_type_round_trip_via_bits`.

**Status:** done

### §3.13 PINGRESP

**Spec:** §3.13 — 2-byte Heartbeat-Reply (Type-Byte + Remaining=0).

**Repo:** Konstante `0xD0`; Encode trivial (1 Byte 0xD0 + VBI(0)).

**Tests:** `packet::tests::packet_type_round_trip_via_bits`.

**Status:** done

### §3.14 DISCONNECT

**Spec:** §3.14 — Verbindungsabbau (mit Reason-Code in 5.0 neu).

**Repo:** `control_packets::{encode_disconnect_body,
decode_disconnect_body}`.

**Tests:** `control_packets::tests::disconnect_round_trip`.

**Status:** done

### §3.15 AUTH

**Spec:** §3.15 — Erweiterte Authentifizierung (neu in 5.0,
multi-step SASL-like challenge/response).

**Repo:** `control_packets::{encode_auth_body, decode_auth_body}`.

**Tests:** `control_packets::tests::auth_round_trip`.

**Status:** done

---

## §4 Operational Behavior

### §4.1-§4.13 Storing State + Network Connections + QoS Levels

**Spec:** §4 — Session-State, Topic-Filter-Matching, QoS-Protokoll-
Flow, Keep-Alive-Timer, Message-Ordering, etc.

**Repo:** `crates/mqtt-bridge/src/broker.rs::{Broker, Session,
Subscription, RetainedMessage, DeliveryEnvelope, QoS, Will}` mit
`connect`/`disconnect`/`subscribe`/`retained_for`/Packet-ID-Tracking.
Topic-Filter-Matching in `crates/mqtt-bridge/src/topic_filter.rs`.
Keep-Alive-Timer als Wall-Clock-Tick in
`crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker` mit
`record_packet`/`is_expired`/`remaining` und Spec-§3.1.2.10-konformer
1.5×-Period-Expiry-Regel.

**Tests:** Inline (~99 inline `#[test]` über mqtt-bridge inkl.
`keep_alive::tests::*` (5 Tests)).

**Status:** done — Session-Manager, Subscription-Tree, Retained-
Messages, Will-Handling, QoS-Levels (0/1/2) + Keep-Alive-Timer
spec-konform implementiert.

---

## §5 Security

### §5 Security Considerations

**Spec:** §5 — Security-Hinweise (TLS, Authentication, Authorization,
Privacy); informativ, kein Wire-Mapping.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Security-Considerations sind
informativ; konkrete TLS-Bindings sind Caller-Layer.

## §6 Web Socket Support

### §6 WebSocket-Transport für MQTT

**Spec:** §6 — MQTT 5.0 über WebSocket (RFC 6455, Sub-Protocol
`mqtt`).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — WebSocket-Layer ist eigene Crate
(`crates/websocket-bridge/`, siehe `websocket-rfc-6455.md`); MQTT-
über-WebSocket ist Caller-Layer-Composition (Frame-Codec von
`crates/mqtt-bridge` durch WebSocket-Frames durchreichen).

## §7 Conformance

### §7 Conformance Targets

**Spec:** §7 — Drei Conformance-Targets: MQTT-Client, MQTT-Server,
MQTT-Server-with-MQTT-Bridge-Functionality.

**Repo:** Subset-Implementation: Wire-Codec + Broker-Logik im
`mqtt-bridge`-Crate. Volle Conformance-Marker am Crate-Niveau
nicht ausgewiesen.

**Tests:** —

**Status:** `n/a (informative)` — Conformance-Statement ist
deklarativ; eine konkrete Implementation kann Conformance gegen
einen oder mehrere Targets claimen; aktuell wird kein expliziter
Conformance-Claim am Crate ausgewiesen.

---

## Audit-Status

22 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-mqtt-bridge` — 94 lib-Inline-Tests +
7 Integration-Tests = 101 Tests grün, 0 failed. Module mit Tests:
`broker`, `codec`, `control_packets`, `data_types`, `dds_bridge`,
`packet`, `properties`, `reason_codes`, `topic_filter`, `vbi`.

Offene Punkte: siehe `mqtt-5.0.open.md`. Restaufwand für die
Partials zusammen ca. 1 PW (§2.2.2 Property-Decode-Helper +
§4 Keep-Alive-Timer-Tick).
