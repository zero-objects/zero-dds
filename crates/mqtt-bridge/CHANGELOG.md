# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-mqtt-bridge` crate.

### Spec references

- **OASIS MQTT 5.0** (07 March 2019): §1.5 (Data Types), §2.1 (Fixed Header), §2.2.2 (Properties — all 27 identifiers), §2.4 (Reason Codes), §3 (Control Packets §3.1-§3.15: all 14 packet types), §4.1 (Session State), §4.7 (Topic Filter Matching).

### Public API

**Wire codec (`codec` + `control_packets` + `data_types` + `vbi` + `packet`):**
- `encode_publish` / `decode_publish` / `CodecError`.
- `FixedHeader`, `ControlPacketType` (all 14 values).
- Encoders/decoders for all 14 control-packet bodies: `connect` / `connack` / `ack` (PUBACK/PUBREC/PUBREL/PUBCOMP) / `subscribe` / `suback` / `unsubscribe` / `unsuback` / `disconnect` / `auth`.
- `Subscription`, `AckBody`, `AuthBody`, etc.
- `encode_vbi` / `decode_vbi` / `vbi_size` (§1.5.5 Variable Byte Integer).
- `encode_utf8_string` / `decode_utf8_string` / `encode_binary_data` / `decode_binary_data` / `encode_two_byte_int` / `decode_two_byte_int` (§1.5).

**Properties (`properties` module + `control_packets::property_data_type`):**
- `Property`, `PropertyId`, `PropertyValueKind`.
- `PropertyDataType` (Byte / Two-Byte-Int / Four-Byte-Int / VBI / UTF8-String / UTF8-String-Pair / Binary-Data).
- `property_data_type(id) -> Option<PropertyDataType>` — schema lookup over all 27 identifiers.

**Broker (`broker` module):**
- `Broker::{new, handle_connect, handle_publish, handle_subscribe, handle_unsubscribe, handle_disconnect, deliver_pending}`.
- `Session`, `BrokerSubscription`, `RetainedMessage`, `Will`, `DeliveryEnvelope`, `QoS::{AtMostOnce, AtLeastOnce, ExactlyOnce}`.

**Keep-alive (`keep_alive` module):**
- `KeepAliveTracker::{new, on_packet, is_expired}` with a 1.5x tolerance factor (§3.1.2.10).

**Topic filter (`topic_filter` module):**
- `topic_matches(filter, topic) -> bool` — `+` (single-level) / `#` (multi-level) wildcards.
- `validate_filter` / `validate_topic_name` / `TopicFilterError`.

**DDS bridge (`dds_bridge` module):**
- `MqttDdsBridge::{new, on_mqtt_publish, on_dds_sample}`.
- `TopicMapper::{add_mapping, lookup_mqtt, lookup_dds}`.
- `BridgeStats { mqtt_to_dds_count, dds_to_mqtt_count }`.
- `mqtt_qos_to_dds(qos) -> (DdsReliability, DdsDurability)`, `dds_qos_to_mqtt`.
- `forward_user_properties(props) -> Vec<...>`.

**Reason codes (`reason_codes` module):**
- `ReasonCode::{Success, ...}` with `is_error` (>= 0x80).

### Implementation

`codec` adheres strictly to the §2.1 + §3.3 PUBLISH layout: fixed header (type/flags) + VBI remaining length + variable header (topic name + packet ID if QoS > 0 + properties) + payload. `control_packets` does this exactly per §3.x for the other 13 packet types.

`broker` is a pure-Rust in-memory implementation with `BTreeMap` state; suitable for tests, embedded and edge brokers. The session state holds subscriptions, unthreaded pending acks, and the last will per client. Retained messages are delivered directly on SUBSCRIBE.

`topic_filter::topic_matches` implements the §4.7 match rules recursively: `+` matches exactly one level, `#` matches 0+ levels (only allowed at the end of the filter; `validate_filter` rejects `#` at another position).

`#![forbid(unsafe_code)]` is set. `extern crate alloc;`.

### Architecture

- **Layer:** 5 (bridges).
- **Dependencies (in):** none (substrate crate).
- **Dependents (out):** (planned) caller layer for DDS↔MQTT workflows.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by OASIS MQTT 5.0.
- Error discriminants: stable; new discriminants are major-additive.

### Added — daemon wireup

- Cross-cutting daemon runtime: the `daemon` feature enables
  Prometheus metrics (§8.2), a catalog/healthz/metrics admin endpoint
  (§5.2), a signal watcher for graceful shutdown (§9.2), and an
  OTLP span exporter (§8.3).
- Bridge security: TLS connector (rustls 0.23 ClientConnection) +
  SASL-PLAIN + bearer token + topic ACL via `zerodds-bridge-security`
  (bridge spec §7.1/§7.2/§7.3).
- Backoff for broker reconnect with exponential backoff +
  a cross-vendor interop module.
- DDS QoS → MQTT behavior translation in `qos_translation`.
