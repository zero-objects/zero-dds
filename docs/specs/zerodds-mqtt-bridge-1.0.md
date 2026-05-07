# `zerodds-mqtt-bridge` v1.0 — DDS↔MQTT-5-Bridge-Daemon

ZeroDDS Vendor-Spec. Spezifiziert das Verhalten eines konfigurierbaren
Daemons, der DDS-Topics mit MQTT-5-Brokern koppelt.

## Motivation

Der MQTT-5-Standard (OASIS, 2019) ist die Lingua-Franca der IoT-
Gerätewelt: Mosquitto, EMQX, HiveMQ, AWS-IoT-Core, Azure-IoT-Hub —
alle sprechen MQTT-5. ZeroDDS hingegen ist die Backbone-Technologie
hinter Real-Time-Industriedaten (Robotik, Sensorfusion, Steuerungs-
systeme). Der `zerodds-mqtt-bridged`-Daemon koppelt beide Welten:
DDS-Topics werden in MQTT-Topics übersetzt und umgekehrt, byte-genau,
mit voller QoS-Translation.

Komplementär zur Library `crates/mqtt-bridge/` (MQTT-5-Codec +
DDS-Bridge-Logik) — die Library ist Building-Block, dieser Daemon ist
das ausführbare Produkt.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Daemon spricht MQTT-5 (OASIS Standard 2019) inkl. CONNECT/PUBLISH/SUBSCRIBE/Properties; MQTT-3.1.1 als Compat-Mode. |
| **L2 — DDS** | Daemon ist gültiger DDS-DomainParticipant (SPDP/SEDP, Discovery, Liveliness). |
| **L3 — Bridging** | Bidirektional: MQTT→DDS-Publish + DDS→MQTT-Push für alle gemappten Topics. |
| **L4 — Config** | Topic-Map per YAML-Config; Hot-Reload via SIGHUP optional. |
| **L5 — Auth** | TLS (`mqtts://`) + Username/Password + Client-Cert + JWT-via-AUTH-Property. |
| **L6 — Multi-Tenant** | Mehrere DomainParticipants pro Daemon; pro MQTT-Client-Id eine Tenant-Bindung. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

```
zerodds-mqtt-bridged [OPTIONS]

Options:
  --config <FILE>              Path zur Config-File (YAML/JSON/TOML)
  --broker <URL>               MQTT-Broker-URL (mqtt://, mqtts://, ws://, wss://)
  --client-id <ID>             MQTT-Client-Id (Default: "zerodds-bridge-<host>")
  --domain <ID>                DDS-Domain-ID (Default 0)
  --username <USER>            MQTT-Username
  --password <PASS>            MQTT-Password (oder ENV $MQTT_PASSWORD)
  --tls-ca <FILE>              CA-Cert für Broker-Verification
  --tls-cert <FILE>            Client-Cert (PEM)
  --tls-key <FILE>             Client-Key (PEM)
  --topic <DDS:MQTT>           Single-Topic-Override (mehrfach erlaubt)
  --log-level <LEVEL>          trace/debug/info/warn/error (Default info)
  --metrics <ADDR>             Prometheus-Scrape-Endpoint (Default off)
  --version                    Versions-Info
  --help                       Hilfe

Exit-Codes:
  0   normaler Shutdown (SIGTERM/SIGINT)
  1   Config-Fehler
  2   Broker-Connect-Fehler
  3   DDS-Discovery-Fehler
  4   TLS-Fehler
  5   Auth-Fehler (CONNACK reason 0x86/0x87)
```

## §3 Config-File-Format

YAML-Schema (auch JSON/TOML akzeptiert):

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
  listen: "127.0.0.1:9091"
  path: "/metrics"
```

ENV-Substitution: `${VAR}` und `${VAR:-default}` werden vor Config-
Parse aufgelöst.

## §4 MQTT-Wire-Protocol

### §4.1 Handshake (CONNECT)

Der Daemon ist MQTT-5-Client gegenüber dem Broker. CONNECT-Properties:

| Property | Wert |
|----------|------|
| `Session Expiry Interval` | aus Config (`session_expiry_interval_secs`) |
| `Receive Maximum` | aus Config (`receive_maximum`) |
| `Maximum Packet Size` | aus Config (`max_packet_size`) |
| `Topic Alias Maximum` | 100 (default) |
| `Request Response Information` | 0 |
| `Request Problem Information` | 1 |
| `User Property[zerodds_version]` | `"1.0"` |
| `User Property[zerodds_role]` | `"bridge"` |
| `Authentication Method` | optional (z.B. `"SCRAM-SHA-256"`, `"OAUTHBEARER"`) |
| `Authentication Data` | optional |

CONNACK-Reason-Codes 0x80+ → Daemon-Exit mit Code 5.

### §4.2 Publish-Frame

DDS→MQTT: Daemon sendet PUBLISH mit der CDR-Payload als `payload`-
Field:

```
PUBLISH
  Topic Name:       <mqtt_topic aus config>
  Packet Identifier: <auto>
  Properties:
    Payload Format Indicator: 0     (binary)
    Content Type:             "application/x-dds-cdr2"
    User Property:            ("zerodds_type", "<DDS-Type-Name>")
    User Property:            ("zerodds_topic", "<DDS-Topic-Name>")
    User Property:            ("zerodds_flags", "<hex-flags>")  # dispose-bit etc.
    User Property:            ("zerodds_key_hash", "<32-hex>")
    User Property:            ("zerodds_source_ts_ns", "<u64>")
    Message Expiry Interval:  <abgeleitet aus Lifespan-QoS>
  Payload:
    [0x00, 0x07, 0x00, 0x00]        # XCDR2-LE Encap-Header
    <CDR-Bytes>                     # gemäß zerodds-xcdr2-bindings-conformance-1.0 §3
```

MQTT→DDS: PUBLISH-Empfang dekodiert die Payload als CDR (XCDR2-LE
default; XCDR2-BE wenn Encap-Header `[0x00, 0x06, 0x00, 0x00]`),
schreibt als DDS-Sample.

### §4.3 Subscribe

Pro `direction=in|bidir` Topic im Config sendet Daemon SUBSCRIBE mit:
```
SUBSCRIBE
  Filter:                      "<mqtt_topic>"
  Subscription Identifier:     <pro-Topic counter>
  QoS:                         <derived: RELIABLE→QoS-2, BEST_EFFORT→QoS-0>
  No Local:                    1
  Retain As Published:         0
  Retain Handling:             0 (send retained on subscribe)
```

### §4.4 Control-Properties

`User Property[zerodds_op]` Werte:
- `"sample"` (default) — normales DATA
- `"dispose"` — Instance-Dispose (DDS sendet als WriteOp::Dispose)
- `"unregister"` — Instance-Unregister
- `"register"` — Instance-Register (selten ueber Wire)

Fehlt das Property, wird `"sample"` angenommen.

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus

Topic-Name `Chat::Message` → MQTT-Topic-Default-Slug:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Andere non-`[a-z0-9/_-]` → `_`
4. Result: `chat/message`

Override per `mqtt_topic`-Feld im Config.

Beispiele:
| DDS-Topic | MQTT-Default |
|-----------|--------------|
| `Chat::Message` | `chat/message` |
| `Sensor::Reading` | `sensors/reading` (mit explizitem override) |
| `RoboticArm::Joint::Position` | `roboticarm/joint/position` |

### §5.2 Type-Discovery

Beim Connect veröffentlicht Daemon ein Catalog-Retain auf
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

## §6 QoS-Translation

| DDS-QoS | MQTT-Verhalten |
|---------|----------------|
| Reliability `RELIABLE` | MQTT-QoS 1 oder 2 (Config-Override; Default 1) |
| Reliability `BEST_EFFORT` | MQTT-QoS 0 |
| Durability `VOLATILE` | retain=false |
| Durability `TRANSIENT_LOCAL` | retain=true (letzter Sample wird auf Broker gehalten) |
| Durability `TRANSIENT/PERSISTENT` | retain=true + Daemon-eigener Replay-Buffer beim Cold-Start |
| History `KEEP_LAST(N)` | Daemon-internal Buffer N |
| History `KEEP_ALL` | Daemon-Backpressure (kein Drop) |
| Lifespan | MQTT-Property `Message Expiry Interval` (Sekunden, gerundet) |
| Deadline | beobachtet, Daemon emittiert `$zerodds/.../deadline_missed` retain |
| Liveliness | MQTT-Will-Message bei `liveliness_lost` |
| Partition | Filter im Daemon vor MQTT-Publish |

QoS-Auto-Derivation: Wenn `mqtt_qos` im Config nicht gesetzt, wird er
aus `qos.reliability` abgeleitet (RELIABLE→1, BEST_EFFORT→0). Wenn
`retain` nicht gesetzt, aus `qos.durability` abgeleitet
(TRANSIENT_LOCAL→true, sonst false).

## §7 Security

### §7.1 TLS

`mqtts://`-Mode aktiviert über `mqtt.tls.enabled: true`. ALPN-Liste
`["mqtt"]` per RFC-7301 wenn Broker es unterstützt.

Cert-Rotation via SIGHUP (Daemon liest cert/key neu, Connection bleibt
bis natürlicher Reconnect).

### §7.2 Auth-Modes

- `none`: anonyme CONNECT (Dev-Mode, nur erlaubt wenn Broker-URL `127.0.0.1`)
- `password`: CONNECT mit `Username` + `Password`
- `mtls`: Client-Cert-Auth, kein User/Pass im CONNECT
- `enhanced` (MQTT-5): `Authentication Method` + `Authentication Data` Properties (z.B. SCRAM, OAUTHBEARER, JWT-Bearer)

### §7.3 ACL (Daemon-Side, vor MQTT)

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

Subject-Resolution: aus TLS-Cert-DN, MQTT-Username, oder JWT-`sub`-
Claim (in dieser Reihenfolge).

## §8 Operations + Observability

### §8.1 Logging

Strukturiertes JSON-Log auf stdout (per `--log-level`). Felder:
`timestamp`, `level`, `event`, `mqtt_topic`, `dds_topic`, `bytes`,
`mqtt_qos`, `latency_us`, `connect_state`.

### §8.2 Prometheus-Metrics

Wenn `metrics.enabled: true`:
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

### §8.3 OTLP-Spans (optional)

Wenn `OTEL_EXPORTER_OTLP_ENDPOINT`-ENV gesetzt: Daemon emittiert
`zerodds-observability-otlp` Spans pro PUBLISH-Roundtrip
(`mqtt.publish` + `dds.write` als Child-Spans).

## §9 Lifecycle

### §9.1 Startup

1. Config-Parse + Validation (fail-fast bei fehlerhafter YAML).
2. TLS-Cert-Load wenn aktiviert.
3. DCPS-DomainParticipant init auf `domain`.
4. Pro Topic: Reader+Writer registrieren (gemäß `direction`).
5. MQTT-Client-Connect mit Reconnect-Backoff.
6. Pro `direction=in|bidir` Topic: SUBSCRIBE.
7. SIGHUP/SIGTERM/SIGINT-Handler installieren.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s, konfigurierbar):
- Stop accepting new DDS-Samples.
- Drain pending MQTT-PUBLISH-Acks (QoS 1/2).
- Send DISCONNECT mit Reason-Code 0x00.
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → Config-Reload (TLS-Cert + ACL hot-update; topic-map-Änderungen
brauchen Restart).

### §9.3 Reconnect

Broker-Disconnect → Exponential-Backoff (`reconnect.initial_delay_ms`
bis `reconnect.max_delay_ms`). Session-State (Subscriptions,
Inflight-Queue) wird wiederhergestellt sofern `clean_start=false`.

## §10 Cross-Vendor

Daemon ist ein normaler RTPS-Peer auf der DDS-Seite — Cyclone/RTI/
Fast-DDS-Subscriber empfangen Samples, die ein MQTT-Client published,
und vice versa. Auf der MQTT-Seite ist der Daemon ein normaler
MQTT-5-Client — getestet gegen Mosquitto, EMQX, HiveMQ.

Verifiziert in `crates/mqtt-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Binary: `zerodds-mqtt-bridged` (statisch gelinkt empfohlen)
- Config-Default: `/etc/zerodds/mqtt-bridged.yaml` (Linux), `/usr/local/etc/zerodds/` (Mac), `%PROGRAMDATA%\ZeroDDS\` (Win)
- Systemd-Unit: `zerodds-mqtt-bridged.service`
- launchd-Plist: `org.zerodds.mqtt-bridged.plist`
- Win-Service: `ZeroDDSMQTTBridge`
- Docker: `zerodds/mqtt-bridged:1.0`

Manual: `man 1 zerodds-mqtt-bridged` + `man 5 zerodds-mqtt-bridged.yaml`.

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`config`, `mqtt_codec`, `topic_map`, `qos_translate`,
`dds_pump`, `acl`) ≥ 5 Tests in `crates/mqtt-bridge/`.

### §12.2 Integration-Tests

`crates/mqtt-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-mqtt-bridged` als Subprocess.
- Mosquitto via testcontainers (Docker).
- Publish auf MQTT-Topic, DDS-Subscriber empfaengt.
- Publish auf DDS-Topic, MQTT-Subscriber empfaengt.
- Verify byte-genauer Roundtrip.

### §12.3 Multi-Vendor

`tests/cross_vendor.rs` (cargo-feature-gated): Cyclone-DDS-Subscriber
in Docker-Compose, ZeroDDS-MQTT-Bridge als Pump zwischen MQTT-Client
und Cyclone.

Broker-Matrix: Mosquitto 2.x, EMQX 5.x, HiveMQ 4.x, Aedes (JS).

## §13 Cross-References

- Library: `crates/mqtt-bridge/`
- MQTT-5-Standard: OASIS Standard 2019.
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Verwandte Daemons: `zerodds-ws-bridge-1.0`, `zerodds-coap-bridge-1.0`,
  `zerodds-amqp-bridge-daemon-1.0`, `zerodds-grpc-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch-Updates für Bugfixes, Minor für additive Config-
Felder, Major für Wire-Protocol-Changes (z.B. MQTT-5.x → 6).
