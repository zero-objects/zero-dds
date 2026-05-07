# `zerodds-mqtt-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-mqtt-bridge/badge.svg)](https://docs.rs/zerodds-mqtt-bridge)

MQTT v5.0 (OASIS Standard 07 March 2019) komplettes Stack-Set: Wire-
Codec fuer alle 14 Control-Packets, alle 27 Properties (§2.2.2),
Variable-Byte-Integer-Codec, Topic-Filter mit Wildcards (`+` / `#`),
Keep-Alive-Tracker (§3.1.2.10), In-Memory-Broker mit Session-State +
Retained-Messages + Will-Messages, und einen MQTT↔DDS-Topic-Bridge.
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OASIS MQTT 5.0 | §1.5 (Data Types), §2.1 (Fixed Header), §2.2.2 (Properties), §2.4 (Reason Codes), §3 (Control Packets §3.1 - §3.15), §4.1 (Session State), §4.7 (Topic Filter Matching) |

## Was ist drin

- **Wire-Codec** mit Encoder/Decoder fuer alle 14 Control-Packet-
  Bodies, Variable-Byte-Integer (§1.5.5), UTF-8 String / Binary Data
  / Two-Byte-Int (§1.5).
- **`Property` / `PropertyId` / `property_data_type`** — alle 27
  registrierten Property-Identifiers mit `PropertyDataType`-Schema-
  Lookup (§2.2.2).
- **`Broker` / `Session` / `RetainedMessage` / `Will`** — In-Memory-
  Broker-Logic mit Session-State, Retained-Messages, Will-Delivery
  bei abnormalem Disconnect (§3 + §4.1).
- **`KeepAliveTracker`** — §3.1.2.10 Keep-Alive-Window-Tracking mit
  konfigurierbarem 1.5x-Tolerance-Faktor.
- **`topic_matches` / `validate_filter` / `validate_topic_name`** —
  §4.7 Topic-Filter mit Single-Level (`+`) und Multi-Level (`#`)
  Wildcards.
- **`ReasonCode`** — alle Reason-Codes per §2.4 mit `is_error`-
  Helper.
- **`MqttDdsBridge` / `TopicMapper` / `mqtt_qos_to_dds` /
  `dds_qos_to_mqtt`** — bidirektionales Mapping zwischen MQTT-Topics
  und DDS-Topics inkl. QoS-Translation.

## Schichten-Position

Layer 5 — Bridges. Substrat fuer DDS↔MQTT-Endpoint-Mapping (IoT-
Broker-Integration, Cloud-MQTT-Endpoints).

## Quickstart

```rust
use zerodds_mqtt_bridge::{encode_vbi, decode_vbi};

let buf = encode_vbi(268_435_455).expect("encode max VBI");
let (v, consumed) = decode_vbi(&buf).expect("decode");
assert_eq!(v, 268_435_455);
assert_eq!(consumed, 4);
```

Topic-Filter:

```rust
use zerodds_mqtt_bridge::topic_matches;

assert!(topic_matches("sensors/+/temp", "sensors/room1/temp"));
assert!(topic_matches("sensors/#", "sensors/room1/temp/f"));
assert!(!topic_matches("sensors/+/temp", "sensors/room1/humidity"));
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Wire-Format (OASIS MQTT 5.0) + Fehler-
Diskriminanten sind RC1-stabil; Breaking-Changes erfordern Major-
Bump.

## Tests

```bash
cargo test -p zerodds-mqtt-bridge
```

115 Tests grün (107 unit + 7 fuzz-smoke + 1 doc).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/mqtt-bridge.md`](../../docs/release/rc1-reviews/mqtt-bridge.md) — RC1-Review.
