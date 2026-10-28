# `zerodds-amqp-bridge-daemon` v1.0 — DDS↔AMQP-1.0 bridge daemon

ZeroDDS vendor spec. Specifies the behaviour of a configurable
daemon that couples DDS topics with AMQP-1.0 brokers (RabbitMQ, ActiveMQ,
Apache Qpid, Azure ServiceBus).

This spec is the **PSM daemon form** of the OMG spec `DDS-AMQP-1.0`
(PIM); the logical mapping (topic↔address, QoS↔settled mode,
sample↔message) follows the OMG spec, the daemon adds CLI/config/
lifecycle/operations.

## Motivation

AMQP-1.0 (ISO/IEC 19464:2014) is the enterprise messaging standard
for bank backbones, cloud pub/sub (Azure ServiceBus, Solace), and
workflow queues (RabbitMQ AMQP-1.0 plugin). DDS dominates real-time
industrial data; the bridge couples both without custom plumbing.

Complementary to the libraries `crates/amqp-bridge/` (DDS-AMQP-1.0
PIM translation) and `crates/amqp-endpoint/` (AMQP-1.0 codec/frame
stack) — both are building blocks, this daemon is the
executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks AMQP-1.0 (ISO/IEC 19464) incl. `BEGIN`/`ATTACH`/`TRANSFER`/`DISPOSITION`/`FLOW`/`DETACH`/`END`. |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional (sender/receiver links per topic), according to the OMG DDS-AMQP-1.0 PIM. |
| **L4 — Config** | Topic map via YAML config; hot reload via SIGHUP optional. |
| **L5 — Auth** | TLS + SASL-PLAIN/SCRAM-SHA-256/EXTERNAL/ANONYMOUS/XOAUTH2. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per AMQP connection. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

```
zerodds-amqp-bridged [OPTIONS]

Options:
  --config <FILE>              base file (YAML subset of zerodds-config),
                               plus FILE.d/; without: amqp-bridged.yaml and amqp-bridged.d/
                               in the system directory
  --set <PATH=VALUE>           one value above all files, repeatable
  --check-config               check the configuration, print every value with its origin,
                               exit (0 valid, 1 invalid)
  --broker <URL>            AMQP broker URL (amqp://, amqps://)
  --container-id <ID>       AMQP container id (default: "zerodds-bridge-<host>")
  --domain <ID>             DDS domain ID (default 0)
  --sasl-mechanism <NAME>   PLAIN|SCRAM-SHA-256|EXTERNAL|ANONYMOUS|XOAUTH2
  --user <USER>             SASL user
  --password <PASS>         SASL password (or ENV $AMQP_PASSWORD)
  --tls-ca <FILE>           CA cert for broker verification
  --tls-cert <FILE>         client cert (PEM)
  --tls-key <FILE>          client key (PEM)
  --topic <DDS:ADDR>        single-topic override (may be given several times)
  --log-level <LEVEL>       trace/debug/info/warn/error (default info)
  --metrics <ADDR>          Prometheus scrape endpoint (default off)
  --version                 version info
  --help                    help

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   broker connect error (TCP)
  3   DDS discovery error
  4   TLS error
  5   SASL error
  6   AMQP-Open-Refused (peer-properties incompatible)
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--broker`, `--container-id`, `--domain`, `--sasl-mechanism`, `--user`, `--password` (or `$AMQP_PASSWORD`), `--tls-ca`, `--tls-cert`, `--tls-key`, `--topic <DDS=ADDR>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
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
    keyed: true                                     # type with @key (WithKey entity)
    type_file: "/etc/zerodds/chat-message.typeobject"   # COMPLETE TypeObject of dds_type, optional
    amqp_address: "/exchanges/amq.topic/chat.message"   # AMQP address (broker-specific, mandatory)
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
  rules:
    - topic: "Order::Event"
      read: ["*"]                                   # DDS -> AMQP, SASL subjects
      write: ["orders-service"]                     # AMQP -> DDS

catalog:
  enabled: true
  address: "/queues/zerodds.catalog"                # default: zerodds.bridge.<container-id>.catalog

metrics:
  enabled: true
  listen: "127.0.0.1:9094"
  path: "/metrics"
```

ENV substitution: `${VAR}` and `${VAR:-default}`. `amqp_address` is mandatory;
`verify_hostname` only takes `true`, `alpn` only `["amqp"]`, `metrics.path` only
`/metrics` — another value is rejected with a reason, instead of silently having
no effect. `topics[].keyed` (default `false`) determines the entity kind of reader
and writer (RTPS §9.3.1.2); it must match the type, otherwise other
vendors do not match. `topics[].type_file` (optional) is a serialized
COMPLETE TypeObject of `dds_type`, the same format as for
`zerodds-zenoh-bridged`. For a keyed topic in the direction AMQP → DDS, the
writer uses it to read the instance keys: a dispose or unregister message names
its instance only via `dds:instance-handle`, and with the type the
DDS marker carries the `serializedKey` (from the instance that the endpoint wrote
or carried to AMQP, or from a KeyHash that contains the key itself).
Without `type_file` the marker goes out only with `PID_KEY_HASH`; Cyclone
11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 then do not set the instance to
disposed. The type is not announced via SEDP; peers keep matching
via the type name. Reader: `crates/amqp-endpoint/src/bridge_daemon.rs`
(`DaemonConfig::from_config`, `key_type_support`).

## §4 AMQP wire protocol

### §4.1 Connection open

The daemon opens an AMQP-1.0 container towards the broker.
The `OPEN` performative carries:

| Field | Value |
|-------|------|
| `container-id` | from config |
| `hostname` | from config |
| `max-frame-size` | from config |
| `channel-max` | from config |
| `idle-time-out` | from config |
| `properties` | map with `{ "zerodds_version": "1.0", "zerodds_role": "bridge" }` |
| `desired-capabilities` | `["AMQP_DDS_BRIDGE"]` |
| `offered-capabilities` | `["AMQP_DDS_BRIDGE"]` |

### §4.2 Session + link setup

Each link gets its own session on its own channel. RabbitMQ 4.0
ends the whole session on an attach to a non-existent queue
(`amqp:not-found`, measured); with a separate session per link this stays
confined to that link. An ended session is answered and begun again after
the backoff (`amqp.reconnect`); its unconfirmed deliveries
go out again.

For each topic config entry, bidirectional links are opened:
- `direction=out|bidir` → sender link (`ATTACH role=sender`)
- `direction=in|bidir` → receiver link (`ATTACH role=receiver`)

`ATTACH` performative:
- `name`: `"<container-id>/<dds-topic>/<role>"`
- `source`/`target`: AMQP address from `amqp_address`
- `snd-settle-mode` / `rcv-settle-mode` from config
- `properties`: map with `{ "zerodds_topic": "<DDS-Topic>", "zerodds_type": "<DDS-Type>" }`
- `desired-capabilities`: `["dds.cdr2"]`

### §4.3 TRANSFER frame (DDS→AMQP)

One message per DDS sample according to DDS-AMQP 1.0 (`dds-amqp-1.0-beta1`) §8,
pass-through mode (§8.1.1). If larger than `max-frame-size`, it goes out as a
multi-part transfer (`more`, Part 2 §2.6.14).

```
TRANSFER:
  delivery-id:        <sequential per session>
  delivery-tag:       <16 bytes, unique per link>
  message-format:     0
  settled:            sender.settled (BEST_EFFORT: always true)

Message:
  Header:
    durable:           Durability != VOLATILE
    priority:          4
    ttl:               LIFESPAN of the writer in ms (absent if infinite)
    first-acquirer:    true
  Properties (§8.2):
    message-id:        binary(24) = writer GUID || RTPS sequence number
    to:                "<amqp_address>"
    subject:           "<DDS-Topic-Name>"
    content-type:      application/vnd.dds.xcdr2
                       (XCDR1 sample: application/vnd.dds.xcdr1, see below)
    creation-time:     source timestamp (ms)
    absolute-expiry-time: creation-time + LIFESPAN
    group-id:          from the KeyHash (§7.6), keyed only
  ApplicationProperties (§8.3):
    dds:domain-id, dds:instance-handle (KeyHash, 16 bytes),
    dds:nsec (remainder below 1 ms), dds:operation (dispose/unregister;
    absent for write), dds:bridge-id + dds:bridge-hop (§8.4)
  Data: encapsulation header (RTPS 2.5 §10.5) + CDR bytes, unchanged
```

An XCDR1 sample: DDS-AMQP 1.0 §8.1.1 defines pass-through for XCDR2.
Re-encoding to XCDR2 would need the type, which the bridge does not know; the
sample therefore goes out with its XCDR1 header and its own content type
`application/vnd.dds.xcdr1` (deviation, not relabelled).

Samples that this bridge writer itself wrote (from AMQP) are
recognized by the GUID prefix and not sent back. Without credit, a
sample waits in the link's queue: KEEP_LAST(n) holds n, KEEP_ALL up to 65,536; beyond that
the oldest one is discarded and counted (`errors_total`,
log `queue_full_dropped_oldest`).

### §4.4 TRANSFER frame (AMQP→DDS)

The receiver link grants `FLOW` credit (`receiver.credit`, topped up at
half), the broker pushes messages. The daemon reads the data sections (all, in
order); the encapsulation header determines representation and
byte order of the writer, `dds:instance-handle` the KeyHash, and
`dds:operation` dispose/unregister a lifecycle sample. A message with
its own `dds:bridge-id` is accepted and not written
(DDS-AMQP 1.0 §8.4, log `dropped_own_message`). `DISPOSITION` with
- `state=accepted` if the DDS write succeeded
- `state=rejected{error}` on a decode error
- `state=released` on backpressure

### §4.5 Disposition mapping

| AMQP outcome | DDS behaviour |
|--------------|---------------|
| `accepted` | Sample written |
| `rejected{condition}` | Decode/ACL error, no replay |
| `released` | Daemon retry-able later |
| `modified{...}` | like `released`; discarded and counted after 5 attempts |

## §5 Topic mapping

### §5.1 Address

`amqp_address` is mandatory; the form depends on the broker:
- RabbitMQ 4.x (AMQP 1.0 native, addresses v2): `/queues/<queue>`,
  `/exchanges/<exchange>/<routing-key>`; the queue must exist.
- ActiveMQ Artemis: `<address>`, `queue://<name>`, `topic://<name>`.
- Qpid Dispatch: `<prefix>/<name>`.

Measured: RabbitMQ 4.0.5 (`tests/bridge_rabbitmq_e2e.rs`).

### §5.2 Type discovery

Per connection the daemon publishes a catalog (`application/json`,
settled) on `catalog.address`, default `zerodds.bridge.<container-id>.catalog`
(not a valid address on RabbitMQ 4; set `/queues/...` there).
`catalog.enabled: false` switches it off. If the broker rejects the address,
the rest keeps running:
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

## §6 QoS translation

| DDS QoS | AMQP behaviour |
|---------|----------------|
| Reliability `RELIABLE` | `snd-settle-mode=unsettled` (`AT_LEAST_ONCE`) + DISPOSITION wait |
| Reliability `BEST_EFFORT` | `snd-settle-mode=settled` (`AT_MOST_ONCE`) |
| Durability `VOLATILE` | non-durable message (`Header.durable=false`) |
| Durability `TRANSIENT_LOCAL` | durable message + durable queue |
| Durability `TRANSIENT/PERSISTENT` | durable + daemon replay buffer on reconnect |
| History `KEEP_LAST(N)` | daemon buffer N |
| History `KEEP_ALL` | queue up to 65,536 without credit, beyond that the oldest is discarded and counted |
| Lifespan | `Header.ttl` (ms) |
| Deadline | open: no annotation `zerodds_deadline_missed` |
| Liveliness | AMQP idle timeout frames (heartbeat at half the broker's value) |
| Partition | open: no partition configuration per topic |

## §7 Security

### §7.1 TLS

`amqps://` mode enabled via `amqp.tls.enabled: true`. ALPN
`["amqp"]` per RFC-7301.

### §7.2 SASL

- `PLAIN`: user+pass cleartext (only over TLS!)
- `SCRAM-SHA-256`: salted challenge-response (RFC 5802/7677), the
  server signature is checked; `SCRAM-SHA-512` is not built
- `EXTERNAL`: TLS cert DN as identity
- `ANONYMOUS`: dev mode, only if the broker URL is `127.0.0.1`
- `XOAUTH2`: OAuth-2.0 bearer token (e.g. Azure ServiceBus)

### §7.3 ACL

Daemon-side filter before TRANSFER. Subject = SASL identity or TLS DN.

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout. Fields: `timestamp`, `level`,
`event`, `container_id`, `link_name`, `dds_topic`, `amqp_address`,
`delivery_id`, `bytes`, `latency_us`.

### §8.2 Prometheus metrics

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

`state` is `established`, `failed` (connect failed) or `lost`
(an existing connection ended). `role` is `sender` or `receiver`.
Dispositions count in both directions: those of the broker for sent
transfers and those of the bridge for received ones. `reason` is the
AMQP error condition if it is an `amqp:` symbol, otherwise `other`. Label
values come from the configuration, not from the broker. In addition, the
generic families `zerodds_amqp_*` remain.

### §8.3 OTLP spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → spans per TRANSFER round trip
(`amqp.transfer` + `dds.write` child).

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation.
2. TLS cert load if enabled.
3. DCPS DomainParticipant init on `domain`.
4. Per topic: register reader+writer (according to `direction`: `in` = broker (receiver or sender link) → DDS, one DataWriter; `out` = DDS → broker (receiver or sender link), one DataReader; `bidir` = both).
5. AMQP connection open (+ SASL + TLS).
6. Per topic: SESSION + ATTACH.
7. Install SIGHUP/SIGTERM/SIGINT handlers.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Stop accepting new DDS samples.
- Drain pending DISPOSITIONs (settled-pending).
- Send `DETACH` for all links + `END` for sessions + `CLOSE`.
- Clean up DDS entities.
- Exit 0.

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart). Open: the daemon sets the reload flag, but rereads
nothing.

Built: SIGTERM/SIGINT stops accepting DDS samples, waits up to 30 s for the
dispositions of the deliveries in flight (remainder counted, log
`shutdown_with_unsettled`), then DETACH, END, CLOSE.

### §9.3 Reconnect

Connection drop → exponential backoff. Unsettled deliveries are
re-attempted on reconnect (provided the broker holds durable state).

## §10 Cross-vendor

The daemon is a normal RTPS peer. AMQP side tested against RabbitMQ
(AMQP-1.0 plugin), ActiveMQ Artemis, Qpid Dispatch Router, Solace,
Azure ServiceBus.

Measured against RabbitMQ 4.0.5 (Linux test host) in
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, with `pika` (AMQP 0.9.1)
as an independent counterpart: DDS→AMQP, AMQP→DDS, an unreadable body is
rejected, multi-part transfers in both directions, loop prevention,
reconnect after the broker drops the connection, a rejected address next to
running links, catalog, TTL. SCRAM-SHA-256, EXTERNAL, TLS/ALPN against a
test server in the crate (`tests/broker_tls_sasl.rs`), since RabbitMQ does not
offer SCRAM. ActiveMQ Artemis, Qpid, Solace, Azure ServiceBus: not measured.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-amqp-bridged`
- Config default: `/etc/zerodds/amqp-bridged.yaml`
- Systemd unit: `zerodds-amqp-bridged.service`
- launchd plist: `org.zerodds.amqp-bridged.plist`
- Windows service: `ZeroDDSAMQPBridge`
- Docker: `zerodds/amqp-bridged:1.0`

Manual: `man 1 zerodds-amqp-bridged` + `man 5 zerodds-amqp-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `amqp_codec`, `link_state`, `disposition`,
`sasl`, `dds_pump`, `acl`) ≥ 5 tests in `crates/amqp-bridge/`
+ `crates/amqp-endpoint/`.

### §12.2 Integration tests

- `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (against RabbitMQ 4.0 on
  127.0.0.1:5672, user `zerodds`; `#[ignore]`, `--ignored`): the bridge in the
  test process, a second DDS participant, `pika` as the counterpart; the cases
  from §10.
- `crates/amqp-endpoint/tests/broker_tls_sasl.rs`: TLS with a mandatory
  client certificate and ALPN `amqp`, SCRAM-SHA-256 (also wrong password),
  EXTERNAL, mechanism not offered.
- `crates/amqp-endpoint/tests/bridge_e2e.rs`: the binary — exit codes, a
  peer without a SASL layer (exit 5), PLAIN without TLS against a remote
  broker (exit 1).

### §12.3 Multi-vendor

Open: a Cyclone DDS subscriber behind the bridge and the rest of the
broker matrix (ActiveMQ Artemis 2.x, Qpid Dispatch 1.x, Solace PubSub+ 10.x,
Azure ServiceBus); RabbitMQ 4.0.5 is measured.

## §13 Cross-references

- Library: `crates/amqp-bridge/` + `crates/amqp-endpoint/`
- OMG spec: DDS-AMQP-1.0 (PIM, formal/2026-01-01).
- AMQP-1.0 standard: ISO/IEC 19464:2014.
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-mqtt-bridge-1.0`, `zerodds-grpc-bridge-1.0`,
  `zerodds-corba-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch updates for bug fixes, minor for additive config
fields, major for wire protocol changes (e.g. AMQP-1.0 → 2).
