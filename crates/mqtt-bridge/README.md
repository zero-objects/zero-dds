# `zerodds-mqtt-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-mqtt-bridge/badge.svg)](https://docs.rs/zerodds-mqtt-bridge)

MQTT v5.0 (OASIS Standard 07 March 2019) complete stack set: wire
codec for all 14 control packets, all 27 properties (§2.2.2),
a variable-byte-integer codec, topic filters with wildcards (`+` / `#`),
a keep-alive tracker (§3.1.2.10), an in-memory broker with session state +
retained messages + will messages, and an MQTT↔DDS topic bridge.
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OASIS MQTT 5.0 | §1.5 (Data Types), §2.1 (Fixed Header), §2.2.2 (Properties), §2.4 (Reason Codes), §3 (Control Packets §3.1 - §3.15), §4.1 (Session State), §4.7 (Topic Filter Matching) |

## What's inside

- **Wire codec** with encoder/decoder for all 14 control-packet
  bodies, variable byte integer (§1.5.5), UTF-8 String / Binary Data
  / Two-Byte-Int (§1.5).
- **`Property` / `PropertyId` / `property_data_type`** — all 27
  registered property identifiers with a `PropertyDataType` schema
  lookup (§2.2.2).
- **`Broker` / `Session` / `RetainedMessage` / `Will`** — in-memory
  broker logic with session state, retained messages, will delivery
  on abnormal disconnect (§3 + §4.1).
- **`KeepAliveTracker`** — §3.1.2.10 keep-alive window tracking with a
  configurable 1.5x tolerance factor.
- **`topic_matches` / `validate_filter` / `validate_topic_name`** —
  §4.7 topic filters with single-level (`+`) and multi-level (`#`)
  wildcards.
- **`ReasonCode`** — all reason codes per §2.4 with an `is_error`
  helper.
- **`MqttDdsBridge` / `TopicMapper` / `mqtt_qos_to_dds` /
  `dds_qos_to_mqtt`** — bidirectional mapping between MQTT topics
  and DDS topics incl. QoS translation.

## Layer position

Layer 5 — bridges. Substrate for DDS↔MQTT endpoint mapping (IoT
broker integration, cloud MQTT endpoints).

## Quickstart

```rust
use zerodds_mqtt_bridge::{encode_vbi, decode_vbi};

let buf = encode_vbi(268_435_455).expect("encode max VBI");
let (v, consumed) = decode_vbi(&buf).expect("decode");
assert_eq!(v, 268_435_455);
assert_eq!(consumed, 4);
```

Topic filter:

```rust
use zerodds_mqtt_bridge::topic_matches;

assert!(topic_matches("sensors/+/temp", "sensors/room1/temp"));
assert!(topic_matches("sensors/#", "sensors/room1/temp/f"));
assert!(!topic_matches("sensors/+/temp", "sensors/room1/humidity"));
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1`. The public API + wire format (OASIS MQTT 5.0) + error
discriminants are RC1-stable; breaking changes require a major
bump.

## Tests

```bash
cargo test -p zerodds-mqtt-bridge
```

115 tests green (107 unit + 7 fuzz-smoke + 1 doc).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/release/rc1-reviews/mqtt-bridge.md`](../../docs/release/rc1-reviews/mqtt-bridge.md) — RC1 review.
