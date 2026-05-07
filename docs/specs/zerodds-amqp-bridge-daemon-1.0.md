# `zerodds-amqp-bridge-daemon` v1.0 — DDS↔AMQP-1.0-Bridge-Daemon

ZeroDDS Vendor-Spec. Spezifiziert das Verhalten eines konfigurierbaren
Daemons, der DDS-Topics mit AMQP-1.0-Brokern (RabbitMQ, ActiveMQ,
Apache-Qpid, Azure-ServiceBus) koppelt.

Diese Spec ist die **PSM-Daemon-Form** der OMG-Spec `DDS-AMQP-1.0`
(PIM); das logische Mapping (Topic↔Address, QoS↔Settled-Mode,
Sample↔Message) folgt der OMG-Spec, der Daemon ergänzt CLI/Config/
Lifecycle/Operations.

## Motivation

AMQP-1.0 (ISO/IEC 19464:2014) ist der Enterprise-Messaging-Standard
für Banken-Backbones, Cloud-Pub/Sub (Azure ServiceBus, Solace), und
Workflow-Queues (RabbitMQ AMQP-1.0-Plugin). DDS dominiert Real-Time-
Industriedaten; die Brücke koppelt beide ohne Custom-Plumbing.

Komplementär zu den Libraries `crates/amqp-bridge/` (DDS-AMQP-1.0
PIM-Translation) und `crates/amqp-endpoint/` (AMQP-1.0-Codec/Frame-
Stack) — beide sind Building-Blocks, dieser Daemon ist das
ausführbare Produkt.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Daemon spricht AMQP-1.0 (ISO/IEC 19464) inkl. `BEGIN`/`ATTACH`/`TRANSFER`/`DISPOSITION`/`FLOW`/`DETACH`/`END`. |
| **L2 — DDS** | Daemon ist gültiger DDS-DomainParticipant (SPDP/SEDP, Discovery, Liveliness). |
| **L3 — Bridging** | Bidirektional (Sender/Receiver-Links pro Topic), gemäß OMG-DDS-AMQP-1.0 PIM. |
| **L4 — Config** | Topic-Map per YAML-Config; Hot-Reload via SIGHUP optional. |
| **L5 — Auth** | TLS + SASL-PLAIN/SCRAM-SHA-256/EXTERNAL/ANONYMOUS/XOAUTH2. |
| **L6 — Multi-Tenant** | Mehrere DomainParticipants pro Daemon; pro AMQP-Connection eine Tenant-Bindung. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

```
zerodds-amqp-bridged [OPTIONS]

Options:
  --config <FILE>           Path zur Config-File (YAML/JSON/TOML)
  --broker <URL>            AMQP-Broker-URL (amqp://, amqps://)
  --container-id <ID>       AMQP-Container-Id (Default: "zerodds-bridge-<host>")
  --domain <ID>             DDS-Domain-ID (Default 0)
  --sasl-mechanism <NAME>   PLAIN|SCRAM-SHA-256|EXTERNAL|ANONYMOUS|XOAUTH2
  --user <USER>             SASL-User
  --password <PASS>         SASL-Password (oder ENV $AMQP_PASSWORD)
  --tls-ca <FILE>           CA-Cert für Broker-Verification
  --tls-cert <FILE>         Client-Cert (PEM)
  --tls-key <FILE>          Client-Key (PEM)
  --topic <DDS:ADDR>        Single-Topic-Override (mehrfach erlaubt)
  --log-level <LEVEL>       trace/debug/info/warn/error (Default info)
  --metrics <ADDR>          Prometheus-Scrape-Endpoint (Default off)
  --version                 Versions-Info
  --help                    Hilfe

Exit-Codes:
  0   normaler Shutdown (SIGTERM/SIGINT)
  1   Config-Fehler
  2   Broker-Connect-Fehler (TCP)
  3   DDS-Discovery-Fehler
  4   TLS-Fehler
  5   SASL-Fehler
  6   AMQP-Open-Refused (peer-properties incompatible)
```

## §3 Config-File-Format

YAML-Schema:

```yaml
# zerodds-amqp-bridged.yaml
domain: 0
log_level: info

amqp:
  broker_url: "amqps://rabbitmq.example.com:5671"
  container_id: "zerodds-bridge-prod-01"
  channel_max: 256
  max_frame_size: 65536
  idle_time_out_ms: 30000
  hostname: "rabbitmq.example.com"
  sasl:
    mechanism: "SCRAM-SHA-256"          # PLAIN | SCRAM-SHA-256 | EXTERNAL | ANONYMOUS | XOAUTH2
    username: "${AMQP_USER}"
    password: "${AMQP_PASSWORD}"
  tls:
    enabled: true
    ca_file: "/etc/zerodds/amqp-ca.pem"
    cert_file: "/etc/zerodds/amqp-client.pem"
    key_file: "/etc/zerodds/amqp-client.key"
    verify_hostname: true
    alpn: ["amqp"]
  reconnect:
    initial_delay_ms: 500
    max_delay_ms: 30000
    factor: 2.0

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    amqp_address: "topic://chat/message"           # AMQP-Address (broker-spezifisch)
    direction: "bidir"
    sender:
      settled: false                                # AT_LEAST_ONCE
      durable: 1                                    # configuration | unsettled-state
      rcv_settle_mode: 0                            # first
      snd_settle_mode: 0                            # unsettled
    receiver:
      credit: 256
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    amqp_address: "queue://sensors/reading"
    direction: "out"
    sender:
      settled: true                                 # AT_MOST_ONCE
    qos:
      reliability: "best_effort"

  - dds_name: "Order::Event"
    dds_type: "Order::Event"
    amqp_address: "queue://orders/events"
    direction: "in"
    receiver:
      credit: 1024
    qos:
      reliability: "reliable"
      durability: "transient_local"

acl:
  default_deny: false

metrics:
  enabled: true
  listen: "127.0.0.1:9093"
  path: "/metrics"
```

ENV-Substitution: `${VAR}` und `${VAR:-default}`.

## §4 AMQP-Wire-Protocol

### §4.1 Connection-Open

Der Daemon öffnet einen AMQP-1.0-Container gegenüber dem Broker.
`OPEN`-Performative trägt:

| Field | Wert |
|-------|------|
| `container-id` | aus Config |
| `hostname` | aus Config |
| `max-frame-size` | aus Config |
| `channel-max` | aus Config |
| `idle-time-out` | aus Config |
| `properties` | Map mit `{ "zerodds_version": "1.0", "zerodds_role": "bridge" }` |
| `desired-capabilities` | `["AMQP_DDS_BRIDGE"]` |
| `offered-capabilities` | `["AMQP_DDS_BRIDGE"]` |

### §4.2 Session + Link-Setup

Pro Topic-Config-Eintrag werden bidirektionale Links eröffnet:
- `direction=out|bidir` → Sender-Link (`ATTACH role=sender`)
- `direction=in|bidir` → Receiver-Link (`ATTACH role=receiver`)

`ATTACH`-Performative:
- `name`: `"<container-id>/<dds-topic>/<role>"`
- `source`/`target`: AMQP-Address aus `amqp_address`
- `snd-settle-mode` / `rcv-settle-mode` aus Config
- `properties`: Map mit `{ "zerodds_topic": "<DDS-Topic>", "zerodds_type": "<DDS-Type>" }`
- `desired-capabilities`: `["dds.cdr2"]`

### §4.3 TRANSFER-Frame (DDS→AMQP)

Pro DDS-Sample sendet Daemon ein `TRANSFER`-Performative + Message:

```
TRANSFER:
  delivery-id:        <auto>
  delivery-tag:       <16-byte unique>
  message-format:     0 (default AMQP)
  settled:            <bool aus sender.settled>

Message:
  Header:
    durable:           <bool aus sender.durable>
    priority:          <0..9>
    ttl:               <abgeleitet aus Lifespan-QoS>
    first-acquirer:    true
  Properties:
    message-id:        <key-hash hex>
    user-id:           <SASL-Identity>
    to:                "<amqp_address>"
    subject:           "<DDS-Topic-Name>"
    content-type:      "application/x-dds-cdr2"
    content-encoding:  null
    creation-time:     <DDS-Source-Timestamp ms>
  ApplicationProperties:
    "zerodds_type":       "<DDS-Type-Name>"
    "zerodds_topic":      "<DDS-Topic-Name>"
    "zerodds_flags":      "<hex-flags>"
    "zerodds_key_hash":   "<32-hex>"
    "zerodds_op":         "sample" | "dispose" | "unregister"
    "zerodds_source_ts":  "<u64-ns>"
  BodySection: data
    [0x00, 0x07, 0x00, 0x00]                  # XCDR2-LE Encap-Header
    <CDR-Bytes>
```

DISPOSITION-Frame mit `state=accepted` quittiert pro Settled-Mode.

### §4.4 TRANSFER-Frame (AMQP→DDS)

Receiver-Link gibt `FLOW`-Credit, Broker pusht Messages. Daemon
dekodiert die Body-Section als CDR (Encap-Header bestimmt Endianness),
schreibt als DDS-Sample, und sendet `DISPOSITION` mit
- `state=accepted` bei DDS-Write erfolgreich
- `state=rejected{error}` bei Decode-Fehler
- `state=released` bei Backpressure

### §4.5 Disposition-Mapping

| AMQP-Outcome | DDS-Verhalten |
|--------------|---------------|
| `accepted` | Sample geschrieben |
| `rejected{condition}` | Decode/ACL-Fehler, kein Replay |
| `released` | Daemon retry-able later |
| `modified{...}` | Daemon retry mit modified delivery-count |

## §5 Topic-Mapping

### §5.1 Address-Default

Topic-Name `Chat::Message` → AMQP-Address-Default:
- Broker = RabbitMQ: `topic://amq.topic/chat.message` (mit AMQP-1.0-Plugin)
- Broker = ActiveMQ: `topic://chat.message`
- Broker = Qpid-Dispatch: `chat/message`

Generischer Default: `topic://<lowercased>::-replaced(/)`.

Override per `amqp_address`-Feld im Config (immer empfohlen).

### §5.2 Type-Discovery

Beim Start veröffentlicht Daemon einen Catalog auf einer dedizierten
Address `zerodds.bridge.<container-id>.catalog`:
```json
{
  "topics": [
    {
      "dds_name": "Chat::Message",
      "dds_type": "Chat::Message",
      "amqp_address": "topic://chat/message",
      "qos": { "reliability": "reliable", "durability": "volatile" }
    }
  ]
}
```

## §6 QoS-Translation

| DDS-QoS | AMQP-Verhalten |
|---------|----------------|
| Reliability `RELIABLE` | `snd-settle-mode=unsettled` (`AT_LEAST_ONCE`) + DISPOSITION-Wait |
| Reliability `BEST_EFFORT` | `snd-settle-mode=settled` (`AT_MOST_ONCE`) |
| Durability `VOLATILE` | non-durable Message (`Header.durable=false`) |
| Durability `TRANSIENT_LOCAL` | durable Message + durable-queue |
| Durability `TRANSIENT/PERSISTENT` | durable + Daemon-Replay-Buffer beim Reconnect |
| History `KEEP_LAST(N)` | Daemon-Buffer N |
| History `KEEP_ALL` | Backpressure via FLOW-Credit |
| Lifespan | `Header.ttl` (ms) |
| Deadline | beobachtet, Daemon emittiert Annotation `zerodds_deadline_missed` |
| Liveliness | AMQP-Idle-Timeout-Frames |
| Partition | Filter im Daemon vor Sender-TRANSFER |

## §7 Security

### §7.1 TLS

`amqps://`-Mode aktiviert über `amqp.tls.enabled: true`. ALPN
`["amqp"]` per RFC-7301.

### §7.2 SASL

- `PLAIN`: User+Pass cleartext (nur über TLS!)
- `SCRAM-SHA-256` / `SCRAM-SHA-512`: salted-challenge-response (RFC 5802)
- `EXTERNAL`: TLS-Cert-DN als Identity
- `ANONYMOUS`: Dev-Mode, nur wenn Broker-URL `127.0.0.1`
- `XOAUTH2`: OAuth-2.0-Bearer-Token (z.B. Azure ServiceBus)

### §7.3 ACL

Daemon-Side Filter vor TRANSFER. Subject = SASL-Identity oder TLS-DN.

## §8 Operations + Observability

### §8.1 Logging

Strukturiertes JSON-Log auf stdout. Felder: `timestamp`, `level`,
`event`, `container_id`, `link_name`, `dds_topic`, `amqp_address`,
`delivery_id`, `bytes`, `latency_us`.

### §8.2 Prometheus-Metrics

```
zerodds_amqp_bridge_connections_total            counter{state}
zerodds_amqp_bridge_links_active                 gauge{role}
zerodds_amqp_bridge_transfers_in_total           counter{dds_topic}
zerodds_amqp_bridge_transfers_out_total          counter{dds_topic}
zerodds_amqp_bridge_bytes_in_total               counter{dds_topic}
zerodds_amqp_bridge_bytes_out_total              counter{dds_topic}
zerodds_amqp_bridge_dispositions_accepted_total  counter
zerodds_amqp_bridge_dispositions_rejected_total  counter{reason}
zerodds_amqp_bridge_dispositions_released_total  counter
zerodds_amqp_bridge_credit_topup_total           counter
zerodds_amqp_bridge_dds_samples_received         counter{dds_topic}
zerodds_amqp_bridge_dds_samples_published        counter{dds_topic}
zerodds_amqp_bridge_sasl_failures_total          counter{mechanism}
```

### §8.3 OTLP-Spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → Spans pro TRANSFER-Roundtrip
(`amqp.transfer` + `dds.write` Child).

## §9 Lifecycle

### §9.1 Startup

1. Config-Parse + Validation.
2. TLS-Cert-Load wenn aktiviert.
3. DCPS-DomainParticipant init auf `domain`.
4. Pro Topic: Reader+Writer registrieren (gemäß `direction`).
5. AMQP-Connection-Open (+ SASL + TLS).
6. Pro Topic: SESSION + ATTACH.
7. SIGHUP/SIGTERM/SIGINT-Handler installieren.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Stop accepting new DDS-Samples.
- Drain pending DISPOSITIONs (settled-pending).
- Send `DETACH` für alle Links + `END` für Sessions + `CLOSE`.
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → Config-Reload (TLS-Cert + ACL hot-update; topic-map-Änderungen
brauchen Restart).

### §9.3 Reconnect

Connection-Drop → Exponential-Backoff. Unsettled-Deliveries werden
beim Reconnect re-attempted (sofern Broker durable-state hält).

## §10 Cross-Vendor

Daemon ist normaler RTPS-Peer. AMQP-Seite getestet gegen RabbitMQ
(AMQP-1.0-Plugin), ActiveMQ-Artemis, Qpid-Dispatch-Router, Solace,
Azure ServiceBus.

Verifiziert in `crates/amqp-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Binary: `zerodds-amqp-bridged`
- Config-Default: `/etc/zerodds/amqp-bridged.yaml`
- Systemd-Unit: `zerodds-amqp-bridged.service`
- launchd-Plist: `org.zerodds.amqp-bridged.plist`
- Win-Service: `ZeroDDSAMQPBridge`
- Docker: `zerodds/amqp-bridged:1.0`

Manual: `man 1 zerodds-amqp-bridged` + `man 5 zerodds-amqp-bridged.yaml`.

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`config`, `amqp_codec`, `link_state`, `disposition`,
`sasl`, `dds_pump`, `acl`) ≥ 5 Tests in `crates/amqp-bridge/`
+ `crates/amqp-endpoint/`.

### §12.2 Integration-Tests

`crates/amqp-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-amqp-bridged` als Subprocess.
- RabbitMQ via testcontainers (Docker, AMQP-1.0-Plugin enabled).
- AMQP-Sender publish → DDS-Sub empfangen.
- DDS-Pub → AMQP-Receiver empfangen.
- DISPOSITION-Sequence (`accepted`/`rejected`/`released`).

### §12.3 Multi-Vendor

`tests/cross_vendor.rs`: Cyclone-DDS-Subscriber + RabbitMQ + ZeroDDS-
AMQP-Bridge im Docker-Compose.

Broker-Matrix: RabbitMQ 3.x, ActiveMQ-Artemis 2.x, Qpid-Dispatch 1.x,
Solace PubSub+ 10.x, Azure ServiceBus (via emulator).

## §13 Cross-References

- Library: `crates/amqp-bridge/` + `crates/amqp-endpoint/`
- OMG-Spec: DDS-AMQP-1.0 (PIM, formal/2026-01-01).
- AMQP-1.0-Standard: ISO/IEC 19464:2014.
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Verwandte Daemons: `zerodds-mqtt-bridge-1.0`, `zerodds-grpc-bridge-1.0`,
  `zerodds-corba-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch-Updates für Bugfixes, Minor für additive Config-
Felder, Major für Wire-Protocol-Changes (z.B. AMQP-1.0 → 2).
