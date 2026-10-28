# `zerodds-mqtt-bridge` v1.0 — DDS↔MQTT 5 bridge daemon

ZeroDDS vendor spec. Specifies the behavior of a configurable
daemon that couples DDS topics with MQTT 5 brokers.

## Motivation

The MQTT 5 standard (OASIS, 2019) is the lingua franca of the IoT
device world: Mosquitto, EMQX, HiveMQ, AWS IoT Core, Azure IoT Hub —
all speak MQTT 5. ZeroDDS, on the other hand, is the backbone technology
behind real-time industrial data (robotics, sensor fusion, control
systems). The `zerodds-mqtt-bridged` daemon couples both worlds:
DDS topics are translated into MQTT topics and vice versa, byte-exact,
with full QoS translation.

Complementary to the library `crates/mqtt-bridge/` (MQTT 5 codec +
DDS bridge logic) — the library is the building block, this daemon is
the executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks MQTT 5 (OASIS Standard 2019) including CONNECT/PUBLISH/SUBSCRIBE/Properties; MQTT 3.1.1 as compat mode. |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional: MQTT→DDS publish + DDS→MQTT push for all mapped topics. |
| **L4 — Config** | Topic map via YAML config; hot reload via SIGHUP optional. |
| **L5 — Auth** | TLS (`mqtts://`) + username/password + client cert + JWT via AUTH property. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per MQTT client ID. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

```
zerodds-mqtt-bridged [OPTIONS]

Options:
  --config <FILE>              Base file (YAML subset of zerodds-config),
                               plus FILE.d/; without it: mqtt-bridged.yaml and mqtt-bridged.d/
                               in the system directory
  --set <PATH=VALUE>           one value above all files, repeatable
  --check-config               check the configuration, print every value with its
                               origin, exit (0 valid, 1 invalid)
  --broker <URL>               MQTT broker URL (mqtt://, mqtts://, ws://, wss://)
  --client-id <ID>             MQTT client ID (default: "zerodds-bridge-<host>")
  --domain <ID>                DDS domain ID (default 0)
  --username <USER>            MQTT username
  --password <PASS>            MQTT password (or ENV $MQTT_PASSWORD)
  --tls-ca <FILE>              CA cert for broker verification
  --tls-cert <FILE>            client cert (PEM)
  --tls-key <FILE>             client key (PEM)
  --topic <DDS:MQTT>           single-topic override (allowed several times)
  --log-level <LEVEL>          trace/debug/info/warn/error (default info)
  --metrics <ADDR>             Prometheus scrape endpoint (default off)
  --version                    version info
  --help                       help

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   broker connect error
  3   DDS discovery error
  4   TLS error
  5   auth error (CONNACK reason 0x86/0x87)
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--broker`, `--client-id`, `--domain`, `--username`, `--password`, `--tls-ca`, `--tls-cert`, `--tls-key`, `--topic <DDS[:MQTT]>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
and therefore win over the files. The same file may carry the keys of the
DDS runtime (`transport.*`, `discovery.*`, …), see
`zerodds-deployment-1.0.md` §2.1.

## §3 Config file format

YAML subset of `zerodds-config` (block lists, flat inline lists of
scalars, `include:`, drop-ins, `${VAR}`). An unknown key in
a known section and a value of the wrong type prevent the start,
with file and line; an unknown key at the top level produces
a warning, and under `--check-config` an error.

```yaml
# zerodds-mqtt-bridged.yaml
domain: 0
log_level: info

mqtt:
  broker_url: "mqtts://mosquitto.example.com:8883"
  client_id: "zerodds-bridge-prod-01"
  username: "${MQTT_USER}"
  password: "${MQTT_PASSWORD}"
  keep_alive_secs: 60
  drain_timeout_secs: 30                   # §9.2: wait for QoS 1/2 acks at shutdown
  clean_start: false
  session_expiry_interval_secs: 86400
  max_packet_size: 1048576
  receive_maximum: 1000
  tls:
    enabled: true
    ca_file: "/etc/zerodds/mqtt-ca.pem"
    cert_file: "/etc/zerodds/mqtt-client.pem"
    key_file: "/etc/zerodds/mqtt-client.key"
    verify_hostname: true
    alpn: ["mqtt"]
  reconnect:
    initial_delay_ms: 500
    max_delay_ms: 30000
    factor: 2.0

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    mqtt_topic: "chat/message"
    direction: "bidir"             # in|out|bidir
    mqtt_qos: 1                    # 0|1|2 (auto-derived from DDS-QoS if absent)
    retain: false                  # auto-derived from TRANSIENT_LOCAL if absent
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    mqtt_topic: "sensors/reading"
    direction: "out"
    mqtt_qos: 0
    qos:
      reliability: "best_effort"
      durability: "volatile"

  - dds_name: "Config::Snapshot"
    dds_type: "Config::Snapshot"
    mqtt_topic: "config/snapshot"
    direction: "in"
    retain: true
    qos:
      reliability: "reliable"
      durability: "transient_local"
      history: { kind: "keep_last", depth: 1 }

acl:
  default_deny: false
  rules:
    - subject: "alice"
      allow_publish: ["chat/+"]
      allow_subscribe: ["chat/+", "sensors/+"]

metrics:
  enabled: true
  listen: "127.0.0.1:9092"
  path: "/metrics"
```

ENV substitution: `${VAR}` and `${VAR:-default}` are resolved before the config
parse.

## §4 MQTT wire protocol

### §4.1 Handshake (CONNECT)

The daemon is an MQTT 5 client towards the broker. CONNECT properties:

| Property | Value |
|----------|------|
| `Session Expiry Interval` | from config (`session_expiry_interval_secs`) |
| `Receive Maximum` | from config (`receive_maximum`) |
| `Maximum Packet Size` | from config (`max_packet_size`) |
| `Topic Alias Maximum` | 100 (default) |
| `Request Response Information` | 0 |
| `Request Problem Information` | 1 |
| `User Property[zerodds_version]` | `"1.0"` |
| `User Property[zerodds_role]` | `"bridge"` |
| `Authentication Method` | optional (e.g. `"SCRAM-SHA-256"`, `"OAUTHBEARER"`) |
| `Authentication Data` | optional |

CONNACK reason codes 0x80+ → daemon exit with code 5.

### §4.2 Publish frame

DDS→MQTT: the daemon sends PUBLISH with the CDR payload as the `payload`
field:

```
PUBLISH
  Topic Name:       <mqtt_topic from config>
  Packet Identifier: <auto>
  Properties:
    Payload Format Indicator: 0     (binary)
    Content Type:             "application/x-dds-cdr2"
    User Property:            ("zerodds_type", "<DDS-Type-Name>")
    User Property:            ("zerodds_topic", "<DDS-Topic-Name>")
    User Property:            ("zerodds_flags", "<hex-flags>")  # dispose-bit etc.
    User Property:            ("zerodds_key_hash", "<32-hex>")
    User Property:            ("zerodds_source_ts_ns", "<u64>")
    Message Expiry Interval:  <derived from the Lifespan QoS>
  Payload:
    [0x00, 0x07, 0x00, 0x00]        # XCDR2-LE encap header
    <CDR-Bytes>                     # per zerodds-xcdr2-bindings-conformance-1.0 §3
```

MQTT→DDS: receiving a PUBLISH decodes the payload as CDR (XCDR2-LE
default; XCDR2-BE if the encap header is `[0x00, 0x06, 0x00, 0x00]`),
writes it as a DDS sample.

### §4.3 Subscribe

For each `direction=in|bidir` topic in the config the daemon sends SUBSCRIBE with:
```
SUBSCRIBE
  Filter:                      "<mqtt_topic>"
  Subscription Identifier:     <per-topic counter>
  QoS:                         <derived: RELIABLE→QoS-2, BEST_EFFORT→QoS-0>
  No Local:                    1
  Retain As Published:         0
  Retain Handling:             0 (send retained on subscribe)
```

### §4.4 Control properties

`User Property[zerodds_op]` values:
- `"sample"` (default) — normal DATA
- `"dispose"` — instance dispose (DDS sends as WriteOp::Dispose)
- `"unregister"` — instance unregister
- `"register"` — instance register (rarely over the wire)

If the property is missing, `"sample"` is assumed.

## §5 Topic mapping

### §5.1 Slug algorithm

Topic name `Chat::Message` → default MQTT topic slug:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Other non-`[a-z0-9/_-]` → `_`
4. Result: `chat/message`

Override via the `mqtt_topic` field in the config.

Examples:
| DDS topic | MQTT default |
|-----------|--------------|
| `Chat::Message` | `chat/message` |
| `Sensor::Reading` | `sensors/reading` (with explicit override) |
| `RoboticArm::Joint::Position` | `roboticarm/joint/position` |

### §5.2 Type discovery

On connect the daemon publishes a retained catalog on
`$zerodds/<client_id>/catalog`:
```json
{
  "topics": [
    {
      "dds_name": "Chat::Message",
      "dds_type": "Chat::Message",
      "mqtt_topic": "chat/message",
      "qos": { "reliability": "reliable", "durability": "volatile" },
      "schema_url": "https://schema.example/chat-message.idl"
    }
  ]
}
```

## §6 QoS translation

| DDS QoS | MQTT behavior |
|---------|----------------|
| Reliability `RELIABLE` | MQTT QoS 1 or 2 (config override; default 1) |
| Reliability `BEST_EFFORT` | MQTT QoS 0 |
| Durability `VOLATILE` | retain=false |
| Durability `TRANSIENT_LOCAL` | retain=true (the last sample is held on the broker) |
| Durability `TRANSIENT/PERSISTENT` | retain=true + the daemon's own replay buffer on cold start |
| History `KEEP_LAST(N)` | daemon-internal buffer N |
| History `KEEP_ALL` | daemon backpressure (no drop) |
| Lifespan | MQTT property `Message Expiry Interval` (seconds, rounded) |
| Deadline | observed, the daemon emits `$zerodds/.../deadline_missed` retain |
| Liveliness | MQTT will message on `liveliness_lost` |
| Partition | filter in the daemon before the MQTT publish |

QoS auto-derivation: if `mqtt_qos` is not set in the config, it is
derived from `qos.reliability` (RELIABLE→1, BEST_EFFORT→0). If
`retain` is not set, it is derived from `qos.durability`
(TRANSIENT_LOCAL→true, otherwise false).

## §7 Security

### §7.1 TLS

`mqtts://` mode is enabled via `mqtt.tls.enabled: true`. ALPN list
`["mqtt"]` per RFC 7301 if the broker supports it. Under `wss://`
the TLS handshake offers `["http/1.1"]`: the channel first carries the
HTTP/1.1 upgrade (RFC 6455 §4). `mqtt.tls.enabled: true` with an
`mqtt://` or `ws://` URL is rejected.

`ws://` and `wss://` (MQTT 5.0 §6): MQTT in WebSocket binary frames,
subprotocol `mqtt`, path from the URL, otherwise `/mqtt`; default ports 80
and 443.

Cert rotation via SIGHUP (the daemon re-reads cert/key, the connection stays
until the natural reconnect).

### §7.2 Auth modes

- `none`: anonymous CONNECT (dev mode, only allowed if the broker URL is `127.0.0.1`)
- `password`: CONNECT with `Username` + `Password`
- `mtls`: client cert auth, no user/pass in the CONNECT
- `enhanced` (MQTT 5): `Authentication Method` + `Authentication Data` properties (e.g. SCRAM, OAUTHBEARER, JWT bearer)

### §7.3 ACL (daemon side, before MQTT)

```yaml
acl:
  default_deny: true
  rules:
    - subject: "*"
      allow_publish: ["sensors/+/public/#"]
      allow_subscribe: ["sensors/+/public/#"]
    - subject: "alice"
      allow_publish: ["chat/+"]
```

Subject resolution: from the TLS cert DN, MQTT username, or JWT `sub`
claim (in this order).

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout (per `--log-level`). Fields:
`timestamp`, `level`, `event`, `mqtt_topic`, `dds_topic`, `bytes`,
`mqtt_qos`, `latency_us`, `connect_state`.

### §8.2 Prometheus metrics

If `metrics.enabled: true`:
```
zerodds_mqtt_bridge_connect_attempts_total      counter
zerodds_mqtt_bridge_connect_successes_total     counter
zerodds_mqtt_bridge_publish_in_total            counter{dds_topic, mqtt_qos}
zerodds_mqtt_bridge_publish_out_total           counter{dds_topic, mqtt_qos}
zerodds_mqtt_bridge_bytes_in_total              counter{dds_topic}
zerodds_mqtt_bridge_bytes_out_total             counter{dds_topic}
zerodds_mqtt_bridge_inflight_messages           gauge{direction}
zerodds_mqtt_bridge_dds_samples_received_total  counter{dds_topic}
zerodds_mqtt_bridge_dds_samples_published_total counter{dds_topic}
zerodds_mqtt_bridge_acl_denials_total           counter{reason}
zerodds_mqtt_bridge_broker_disconnects_total    counter{reason_code}
```

### §8.3 OTLP spans (optional)

If the `OTEL_EXPORTER_OTLP_ENDPOINT` ENV is set: the daemon emits
`zerodds-observability-otlp` spans per PUBLISH roundtrip
(`mqtt.publish` + `dds.write` as child spans).

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation (fail-fast on faulty YAML).
2. TLS cert load if enabled.
3. DCPS DomainParticipant init on `domain`.
4. Per topic: register reader+writer (according to `direction`: `in` = broker → DDS, one DataWriter; `out` = DDS → broker, one DataReader; `bidir` = both).
5. MQTT client connect with reconnect backoff.
6. Per `direction=in|bidir` topic: SUBSCRIBE.
7. Install SIGHUP/SIGTERM/SIGINT handlers.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s, configurable):
- Stop accepting new DDS samples.
- Drain pending MQTT PUBLISH acks (QoS 1/2).
- Send DISCONNECT with reason code 0x00.
- Cleanup DDS entities.
- Exit 0.

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart).

### §9.3 Reconnect

Broker disconnect → exponential backoff (`reconnect.initial_delay_ms`
up to `reconnect.max_delay_ms`). The session state (subscriptions,
inflight queue) is restored if `clean_start=false`.

## §10 Cross-vendor

The daemon is a normal RTPS peer on the DDS side — Cyclone/RTI/
Fast DDS subscribers receive samples that an MQTT client publishes,
and vice versa. On the MQTT side the daemon is a normal
MQTT 5 client — tested against Mosquitto, EMQX, HiveMQ.

Verified in `crates/mqtt-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-mqtt-bridged` (statically linked recommended)
- Config default: `/etc/zerodds/mqtt-bridged.yaml` (Linux), `/usr/local/etc/zerodds/` (Mac), `%PROGRAMDATA%\ZeroDDS\` (Win)
- Systemd unit: `zerodds-mqtt-bridged.service`
- launchd plist: `org.zerodds.mqtt-bridged.plist`
- Win service: `ZeroDDSMQTTBridge`
- Docker: `zerodds/mqtt-bridged:1.0`

Manual: `man 1 zerodds-mqtt-bridged` + `man 5 zerodds-mqtt-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `mqtt_codec`, `topic_map`, `qos_translate`,
`dds_pump`, `acl`) ≥ 5 tests in `crates/mqtt-bridge/`.

### §12.2 Integration tests

`crates/mqtt-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-mqtt-bridged` as a subprocess.
- Mosquitto via testcontainers (Docker).
- Publish on an MQTT topic, the DDS subscriber receives.
- Publish on a DDS topic, the MQTT subscriber receives.
- Verify a byte-exact roundtrip.

### §12.3 Multi-vendor

`tests/cross_vendor.rs` (cargo-feature-gated): Cyclone DDS subscriber
in Docker Compose, ZeroDDS MQTT bridge as a pump between the MQTT client
and Cyclone.

Broker matrix: Mosquitto 2.x, EMQX 5.x, HiveMQ 4.x, Aedes (JS).

## §13 Cross-references

- Library: `crates/mqtt-bridge/`
- MQTT 5 standard: OASIS Standard 2019.
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-ws-bridge-1.0`, `zerodds-coap-bridge-1.0`,
  `zerodds-amqp-bridge-daemon-1.0`, `zerodds-grpc-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch updates for bug fixes, minor for additive config
fields, major for wire protocol changes (e.g. MQTT 5.x → 6).
