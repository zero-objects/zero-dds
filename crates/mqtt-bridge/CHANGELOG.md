# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-mqtt-bridge`-Crate.

### Spec-Referenzen

- **OASIS MQTT 5.0** (07 March 2019): §1.5 (Data Types), §2.1 (Fixed Header), §2.2.2 (Properties — alle 27 Identifiers), §2.4 (Reason Codes), §3 (Control Packets §3.1-§3.15: alle 14 Packet-Types), §4.1 (Session State), §4.7 (Topic Filter Matching).

### Public-API

**Wire-Codec (`codec` + `control_packets` + `data_types` + `vbi` + `packet`):**
- `encode_publish` / `decode_publish` / `CodecError`.
- `FixedHeader`, `ControlPacketType` (alle 14 Werte).
- Encoder/Decoder fuer alle 14 Control-Packet-Bodies: `connect` / `connack` / `ack` (PUBACK/PUBREC/PUBREL/PUBCOMP) / `subscribe` / `suback` / `unsubscribe` / `unsuback` / `disconnect` / `auth`.
- `Subscription`, `AckBody`, `AuthBody`, etc.
- `encode_vbi` / `decode_vbi` / `vbi_size` (§1.5.5 Variable Byte Integer).
- `encode_utf8_string` / `decode_utf8_string` / `encode_binary_data` / `decode_binary_data` / `encode_two_byte_int` / `decode_two_byte_int` (§1.5).

**Properties (`properties`-Modul + `control_packets::property_data_type`):**
- `Property`, `PropertyId`, `PropertyValueKind`.
- `PropertyDataType` (Byte / Two-Byte-Int / Four-Byte-Int / VBI / UTF8-String / UTF8-String-Pair / Binary-Data).
- `property_data_type(id) -> Option<PropertyDataType>` — Schema-Lookup ueber alle 27 Identifiers.

**Broker (`broker`-Modul):**
- `Broker::{new, handle_connect, handle_publish, handle_subscribe, handle_unsubscribe, handle_disconnect, deliver_pending}`.
- `Session`, `BrokerSubscription`, `RetainedMessage`, `Will`, `DeliveryEnvelope`, `QoS::{AtMostOnce, AtLeastOnce, ExactlyOnce}`.

**Keep-Alive (`keep_alive`-Modul):**
- `KeepAliveTracker::{new, on_packet, is_expired}` mit 1.5x-Tolerance-Faktor (§3.1.2.10).

**Topic-Filter (`topic_filter`-Modul):**
- `topic_matches(filter, topic) -> bool` — `+` (single-level) / `#` (multi-level) Wildcards.
- `validate_filter` / `validate_topic_name` / `TopicFilterError`.

**DDS-Bridge (`dds_bridge`-Modul):**
- `MqttDdsBridge::{new, on_mqtt_publish, on_dds_sample}`.
- `TopicMapper::{add_mapping, lookup_mqtt, lookup_dds}`.
- `BridgeStats { mqtt_to_dds_count, dds_to_mqtt_count }`.
- `mqtt_qos_to_dds(qos) -> (DdsReliability, DdsDurability)`, `dds_qos_to_mqtt`.
- `forward_user_properties(props) -> Vec<...>`.

**Reason-Codes (`reason_codes`-Modul):**
- `ReasonCode::{Success, ...}` mit `is_error` (>= 0x80).

### Implementierung

`codec` haelt sich strikt an §2.1 + §3.3 PUBLISH-Layout: Fixed-Header (Type/Flags) + VBI Remaining-Length + Variable-Header (Topic-Name + Packet-ID falls QoS > 0 + Properties) + Payload. `control_packets` macht das pro §3.x exakt fuer die anderen 13 Packet-Types.

`broker` ist eine pure-Rust In-Memory-Implementation mit `BTreeMap`-State; eignet sich fuer Tests, embedded und Edge-Broker. Session-State haelt Subscriptions, ungefaedelte Pending-Acks, und das letzte Will pro Client. Retained-Messages werden auf SUBSCRIBE direkt zugestellt.

`topic_filter::topic_matches` implementiert die §4.7-Match-Regeln rekursiv: `+` matcht genau ein Level, `#` matcht 0+ Levels (nur am Ende des Filters erlaubt; `validate_filter` lehnt `#` an anderer Position ab).

`#![forbid(unsafe_code)]` ist gesetzt. `extern crate alloc;`.

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** keine (Substrat-Crate).
- **Dependents (out):** (vorgesehen) Caller-Layer fuer DDS↔MQTT-Workflows.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch OASIS MQTT 5.0 fixiert.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.

### Added — Daemon-Wireup

- Cross-Cutting Daemon-Runtime: `daemon`-Feature aktiviert
  Prometheus-Metrics (§8.2), Catalog/Healthz/Metrics-Admin-Endpoint
  (§5.2), Signal-Watcher fuer Graceful-Shutdown (§9.2), und
  OTLP-Span-Exporter (§8.3).
- Bridge-Security: TLS-Connector (rustls 0.23 ClientConnection) +
  SASL-PLAIN + Bearer-Token + Topic-ACL via `zerodds-bridge-security`
  (Bridge-Spec §7.1/§7.2/§7.3).
- Backoff fuer Broker-Reconnect mit Exponential-Backoff +
  Cross-Vendor-Interop-Modul.
- DDS-QoS → MQTT-Behavior-Translation in `qos_translation`.
