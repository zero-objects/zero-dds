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
| **L5 — Auth** | TLS + SASL-PLAIN/SCRAM-SHA-256/SCRAM-SHA-512/EXTERNAL/ANONYMOUS/XOAUTH2. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per AMQP connection. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

### §1.1 L6: tenants

`tenants:` (§3) lists the participants beside the daemon's own. Each
tenant has a DomainParticipant of its own on its `domain` (GUID prefix
from its `amqp.container_id`) with its `topics`, and a connection of its
own to the broker of `amqp.broker_url`. The connection is the tenant's
binding: its `open.container-id` is the tenant's `amqp.container_id`, its
`open.hostname` the tenant's `amqp.hostname` (a virtual host), its SASL
identity the tenant's `amqp.sasl.*`, its TLS client certificate the
tenant's `amqp.tls.cert_file`/`key_file`. What a tenant does not set is
the daemon's (`amqp.sasl.mechanism`, the CA, reconnect, ACL, limits); the
catalog goes to the tenant's `catalog.address`, by default
`zerodds.bridge.<container-id>.catalog` of the tenant.

Refused at start, with the entry's path and its file and line: a tenant
without `name`, `domain`, `amqp.container_id` or `topics`; two tenants of
one name (the second as `tenants[i].name`); two participants (the daemon's
own included) with one container id (the tenant's as
`tenants[i].amqp.container_id`).

Every participant runs in a thread of its own. A participant that stops
with an error (§2) stops the others; the daemon exits with the first
error in the order of the configuration, the daemon's own first. Log
lines of a tenant's participant carry `"tenant"`; its metrics (§8.2) carry
the label `tenant`. The admin endpoint's `/catalog` lists the daemon's
topics, then each tenant's with the field `"tenant"`. A SIGHUP reload
(§9.2) applies the ACL and the TLS material of each tenant from the tenant
of the same name; a tenant added or removed needs a restart, and a removed
tenant's reload is refused (`reload_refused`) while it keeps running.

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
  --sasl-mechanism <NAME>   PLAIN|SCRAM-SHA-256|SCRAM-SHA-512|EXTERNAL|ANONYMOUS|XOAUTH2
  --user <USER>             SASL user
  --password <PASS>         SASL password (or ENV $AMQP_PASSWORD)
  --tls-ca <FILE>           CA cert for broker verification
  --tls-cert <FILE>         client cert (PEM)
  --tls-key <FILE>          client key (PEM)
  --topic <DDS=ADDR>        one topic mapping (may be given several times)
  --log-level <LEVEL>       trace/debug/info/warn/error (default info)
  --metrics <ADDR>          Prometheus scrape endpoint (default off)
  --version                 version info
  --help                    help

Test hooks (accepted by the program, listed by --help and the man page):
  --once                    one connection: a failed TCP connect or the loss
                            of the connection ends the daemon with exit 2
                            instead of a retry after the back-off
  --connect-timeout-ms <N>  time-out in milliseconds of one TCP connect and
                            of each read and write of the TLS, SASL and open
                            exchange (default 5000)

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   broker connect error (TCP)
  3   DDS participant or endpoint cannot be created
  4   TLS error
  5   SASL error
  6   AMQP-Open-Refused (peer-properties incompatible)
```

When each code applies: a TCP connect that fails is retried after the
back-off of §9.3 without a limit (the broker may not be up yet); exit 2 only
with the test hook `--once`, which also ends the daemon with 2 when an
established connection is lost. A TLS handshake, SASL exchange or open that
fails before the first connection stood is a fault of the configuration that
a retry does not cure: exit 4, 5 or 6. A reset or end of stream in these
stages counts as a refusal (RabbitMQ 4.0.5 resets the connection after the
open of an unknown virtual host, measured); only a time-out there is retried.
Once a connection stood, every failure of a later connect is retried after
the back-off. Exit 6 also when the broker's open allows fewer channels
than the bridge has links (§4.2). Exit 3 when the DDS participant or one of
its endpoints cannot be created (for example a domain id without a UDP port
under the port mapping).

The default container id is `zerodds-bridge-<host>`: the host name
(`gethostname`; `COMPUTERNAME` on Windows) with its dots as hyphens, at most 32
characters; `zerodds-bridge-pid<pid>` only on a host without a name. The
container id also seeds the GUID prefix of the DDS participant, so two daemons
on one host need two container ids.

Precedence (lowest first): built-in defaults, environment, files,
flags. `--broker`, `--container-id`, `--domain`, `--sasl-mechanism`, `--user`, `--password` (or `$AMQP_PASSWORD`), `--tls-ca`, `--tls-cert`, `--tls-key`, `--topic <DDS=ADDR>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
and therefore win over the files. `$AMQP_CBS_SECRET` is the environment
layer of `amqp.cbs.secret`. `--check-config` shows `***` in place of the
values of `amqp.sasl.password` and `amqp.cbs.secret`, also in the origin of
a `--set` of them. The same file may carry the keys of the
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
    mechanism: "SCRAM-SHA-256"          # PLAIN | SCRAM-SHA-256 | SCRAM-SHA-512 | EXTERNAL | ANONYMOUS | XOAUTH2
    username: "${AMQP_USER}"
    password: "${AMQP_PASSWORD}"
  tls:
    enabled: true
    ca_file: "/etc/zerodds/amqp-ca.pem"
    cert_file: "/etc/zerodds/amqp-client.pem"
    key_file: "/etc/zerodds/amqp-client.key"
    verify_hostname: true
    alpn: ["amqp"]
  cbs:                       # claims-based security, §7.2
    enabled: false
    key_name: "RootManageSharedAccessKey"
    secret: "${AMQP_CBS_SECRET}"
    ttl_s: 3600
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
      rcv_settle_mode: 0                            # first | 1 second
      snd_settle_mode: 0                            # unsettled
    receiver:
      credit: 256
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
      deadline_ms: 2000                             # optional
      liveliness_lease_ms: 10000                    # optional
      partition: ["site-a"]                         # optional

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

tenants:                                            # §1.1, optional
  - name: "plant-b"
    domain: 7
    amqp:
      container_id: "zerodds-bridge-plant-b"        # mandatory, unique in the daemon
      hostname: "vhost:plant-b"
      sasl:
        username: "${PLANT_B_USER}"
        password: "${PLANT_B_PASSWORD}"
      tls:
        cert_file: "/etc/zerodds/plant-b.pem"
        key_file: "/etc/zerodds/plant-b.key"
    catalog:
      address: "/queues/plant-b.catalog"
    topics:
      - dds_name: "Order::Event"
        amqp_address: "/queues/plant-b.orders"
        direction: "out"

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
`zerodds-zenoh-bridged`. Only a keyed topic takes it; on an unkeyed topic
the key is rejected with a reason. For a keyed
topic in the direction AMQP → DDS, the writer uses it to read the instance
keys: a dispose or unregister message names
its instance only via `dds:instance-handle`, and with the type the
DDS marker carries the `serializedKey` (from the instance that the endpoint wrote
or carried to AMQP, or from a KeyHash that contains the key itself).
Without `type_file` the marker goes out only with `PID_KEY_HASH`; Cyclone
11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 then do not set the instance to
disposed. For a keyed topic in the direction DDS → AMQP, the reader uses it
to compute the KeyHash of a sample whose writer leaves `PID_KEY_HASH` out
(DDSI-RTPS 2.5 §9.6.4.8; §8.7.10 has a reader that receives none compute it
from the data; Cyclone 11.0.1 does not send it),
which becomes `dds:instance-handle` and `group-id`, and to read the key of
a dispose or unregister from its `serializedKey`. Without `type_file`, or
when the body or the key does not decode as the type, such a sample and such
a dispose or unregister go out with the nil instance handle
(`dds:instance-handle` of zeros, no `group-id`), never with octets that name
no instance; the daemon logs `key_hash_unavailable` (level `warn`, fields
`dds_topic`, `writer`, `type_file`) and counts `errors_total` once per
writer. The type
is not announced via SEDP; peers keep matching via the type name. Reader: `crates/amqp-endpoint/src/bridge_daemon/config.rs`
(`DaemonConfig::from_config`); the type: `key_type_support` in
`crates/amqp-endpoint/src/bridge_daemon.rs`.

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
- `snd-settle-mode` / `rcv-settle-mode` from config (`sender.*`, both
  links of a topic)
- `properties`: map with `{ "zerodds_topic": "<DDS-Topic>", "zerodds_type": "<DDS-Type>" }`
- `desired-capabilities`: `["dds.cdr2"]`

Settlement (Part 2 §2.8.3): on a sending link the daemon follows the
broker's dispositions, not the `rcv-settle-mode` of the broker's attach.
An outcome the broker sends unsettled (its receiver settle mode is
`second`) the daemon settles (`disposition` role sender, settled), and the
broker settles after it; an outcome the broker sends settled ends the
delivery. A state that is not an outcome (`received`) leaves the delivery
unsettled. On a receiving link with `rcv-settle-mode` 1 the daemon is the
receiver: it sends its outcome unsettled and forgets the delivery once the
broker has settled it; until then the delivery takes up credit (at most
`receiver.credit` wait).

Flow control (Part 2 §2.6.7, §2.7.4): on a sending link the credit is the
broker's delivery-count plus its link-credit less the link's
delivery-count, none when that limit lies behind the link's count (RFC 1982
serial numbers); a flow without delivery-count counts from the
initial-delivery-count 0 of the daemon's attach, from which the link counts
again on every attach. A sending link the broker asks to drain sends what
is queued while the credit lasts, then advances its delivery-count by the
credit left and sends a flow that states it (link-credit 0, `available` the
messages queued for credit, `drain` set); a message that waits for the
session window is available, and the link drains once it has gone out. On a
receiving link the broker's delivery-count is the one the daemon counts
from: credit a count moved without deliveries consumed is granted again once
half of it is used (§4.4). A flow with `echo` is answered with the state of
its link, one without a handle with the state of the session. The two links
of CBS (§7.2) do the same.

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

A dispose or unregister goes out with the same header and properties:
message-id = GUID of the disposing writer || its RTPS sequence number,
creation-time = the source timestamp of the marker.

Samples, disposes and unregisters that this bridge's writer itself wrote
(from AMQP) are recognized by the GUID prefix and not sent back. Without credit, a
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
- `state=released` on backpressure: the DDS writer refuses the sample (its
  RESOURCE_LIMITS, `OUT_OF_RESOURCES`, or another write error); the broker may
  deliver the message again

### §4.5 Disposition mapping

The outcome the broker gives a message the daemon sent (AMQP 1.0 Part 3
§3.4); the rows of §4.4 are the daemon's own outcomes for received messages.

| AMQP outcome | Daemon behaviour |
|--------------|------------------|
| `accepted` | The delivery ends. |
| `rejected{error}` | The delivery ends; counted in `dispositions_rejected_total{reason}` and `errors_total`, logged `rejected_by_broker`; not sent again. |
| `released` | Sent again first on the link, `header.first-acquirer` false, `header.delivery-count` unchanged (Part 3 §3.2.1, §3.4.4). |
| `modified{delivery-failed, undeliverable-here, message-annotations}` | Like `released`; with `delivery-failed` the `header.delivery-count` is one more; `message-annotations` are combined with the message's (same key replaced, others added). With `undeliverable-here` the message is not sent again on the link (§3.4.5), the only one of its topic: it is discarded, counted in `errors_total` and logged `undeliverable_here`. |

A message given back with `released` or `modified` five times is
discarded, counted in `errors_total` and logged `released_too_often`.
`released` and `modified` count in `dispositions_released_total`.
Measured against RabbitMQ 4.0.5: a message to an exchange that routes it
to no queue is `released`; a queue at `x-max-length` with `x-overflow:
reject-publish` answers `rejected` without an error.

## §5 Topic mapping

### §5.1 Address

`amqp_address` is mandatory; the form depends on the broker:
- RabbitMQ 4.x (AMQP 1.0 native, addresses v2): `/queues/<queue>`,
  `/exchanges/<exchange>/<routing-key>`; the queue must exist.
- ActiveMQ Artemis: `<address>`, `queue://<name>`, `topic://<name>` (the
  prefixes when the acceptor maps them: `anycastPrefix=queue://;
  multicastPrefix=topic://`).
- Qpid Dispatch: `<prefix>/<name>`.
- Solace PubSub+: `<queue>`, `queue://<name>`, `topic://<name>`; a
  queue must exist (Solace attaches a sender to any queue name and
  rejects each message for a queue that does not exist).

Measured: RabbitMQ 4.0.5 (`tests/bridge_rabbitmq_e2e.rs`); Artemis 2.44.0
with `<address>`, `queue://<name>` and `topic://<name>` (two receivers of
one multicast address, each gets every sample; `tests/bridge_artemis_e2e.rs`); Qpid
Dispatch 1.19.0 with `zd/<name>` under a balanced prefix `zd`
(`tests/bridge_dispatch_e2e.rs`); Solace PubSub+ Standard 10.26.0.9219
with `<queue>`, `queue://<name>` and `topic://<name>` (two receivers, each
gets every sample; `tests/bridge_solace_e2e.rs`).

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
| Durability `TRANSIENT_LOCAL` | durable message (`Header.durable=true`) to a durable node: the target of the sender link has terminus `durable` 1 (configuration) unless `sender.durable` is set; the source of the receiver link stays 0 unless `sender.durable` is set (it applies to both links) |
| Durability `TRANSIENT/PERSISTENT` | as `TRANSIENT_LOCAL`; queued and unsettled messages survive a reconnect (§9.3) |
| History `KEEP_LAST(N)` | daemon buffer N per sender link while the broker grants no credit; the oldest is discarded, counted in `errors_total` and logged `queue_full_dropped_oldest` |
| History `KEEP_ALL` | queue up to 65,536 without credit, beyond that the oldest is discarded and counted |
| Lifespan | `Header.ttl` (ms) from the writer's LIFESPAN |
| Deadline | `qos.deadline_ms`: requested by the DDS reader, offered by the DDS writer (a DDS writer that offers a longer period, or none, does not match, DDS 1.4 §2.2.3.7). A rise of the reader's REQUESTED_DEADLINE_MISSED total is logged (`deadline_missed`), counted in `errors_total`, and the next message of the topic carries the total as message annotation `x-opt-zerodds-deadline-missed` (ulong). The `x-opt-` prefix: AMQP 1.0 Part 3 §3.2.10 reserves keys without `x-`, and a receiver may ignore an unknown `x-opt-` key. |
| Liveliness | `qos.liveliness_lease_ms`: the lease of the DDS endpoints (AUTOMATIC); towards the broker AMQP idle time-out frames, a heartbeat at half the broker's value |
| Partition | `qos.partition` (list, wildcards as DDS 1.4 §2.2.3.13): the partitions of the DDS reader and writer. A message carries the PARTITION of the sample's writer in `dds:partition` (DDS-AMQP 1.0 §8.3). A received message whose `dds:partition` matches none of the topic's partitions is `rejected` (`amqp:precondition-failed`), counted and logged `partition_mismatch`; one without `dds:partition` is written into the topic's partitions. |

Keys: `topics[].qos.deadline_ms` and `topics[].qos.liveliness_lease_ms`
(milliseconds, 1 to 2147483647; infinite without the key) and
`topics[].qos.partition` (a list of names; the default partition without
the key). Another value prevents the start with file and line.

## §7 Security

### §7.1 TLS

`amqps://` mode enabled via `amqp.tls.enabled: true`. ALPN
`["amqp"]` per RFC-7301.

### §7.2 SASL

The mechanism is `amqp.sasl.mechanism`; the broker must offer it, there is
no fallback to another one (a refusal is exit 5, §2).

- `PLAIN` (RFC 4616): user and password in the clear; refused without
  TLS towards a broker that is not on the loopback interface (`localhost`,
  `127.0.0.0/8`, `::1`).
- `SCRAM-SHA-256` (RFC 7677) and `SCRAM-SHA-512` (the SCRAM family of
  RFC 5802 over SHA-512): salted challenge-response; the iteration count
  must be at least 4096, the server nonce must extend the client's, and
  the outcome must carry the server signature (`v=`), otherwise the
  exchange is refused.
- `EXTERNAL` (RFC 4422 App. A): the identity is the TLS client
  certificate (`amqp.tls.cert_file`); needs TLS.
- `ANONYMOUS` (RFC 4505): only towards a loopback broker, or over TLS
  with claims-based security below.
- `XOAUTH2`: an OAuth 2.0 bearer token, `amqp.sasl.password`, sent as
  `user=<username>^Aauth=Bearer <token>^A^A`. A broker that does not
  accept the token sends its error status as a challenge; the bridge
  answers with an empty response and the refusal names that error.

Claims-based security (`amqp.cbs`, the `put-token` exchange of the AMQP
CBS working draft as Azure Service Bus uses it): with
`amqp.cbs.enabled: true` the daemon begins a session of its own after the
links' sessions, attaches a sender to the node `$cbs` and a receiver from
`$cbs` whose target `<container-id>/$cbs/reply` is the `reply-to` of the
requests, and puts one token per AMQP address of its links (the catalog's
included) before it begins the links. A request carries the message-id,
`to` `$cbs`, the `reply-to`, the application properties `operation`
`put-token`, `type` `servicebus.windows.net:sastoken` and `name` (the
audience `amqp://<host>/<address>`, `<host>` from `amqp.hostname` or the
broker URL, the address without a leading `/`), and the token as an
`amqp-value` string: `SharedAccessSignature sr=<audience URL-encoded>&
sig=<signature>&se=<expiry>&skn=<amqp.cbs.key_name>`, the signature the
Base64 HMAC-SHA256 with the UTF-8 octets of `amqp.cbs.secret` over the encoded
audience, a line feed and the expiry (seconds since the Unix epoch), then
URL-encoded. The links follow when every answer (`correlation-id` = the
request's message-id) has `status-code` 200 or 202 (log `cbs_authorized`);
any other status ends the connection with that status, and the daemon
connects again after its back-off. The tokens live `amqp.cbs.ttl_s`
seconds (10 to 86400, default 3600) and are put again on the same links at
four fifths of that (log `cbs_token_renewed`). A round, the first or a
renewal, ends within the connect time-out (`--connect-timeout-ms`, default
5000 ms) from its start: the `$cbs` links attached, credit granted, every
token put and answered. A round that has not ended by then ends the
connection (`connection_lost`, `cbs: put-token round not answered within
<n> ms`), and the daemon connects again after its back-off (§9.3).
`amqp.cbs.secret` may come from `$AMQP_CBS_SECRET`.

### §7.3 ACL

Daemon-side filter before TRANSFER: a direction the ACL denies at start
gets neither its DDS endpoint nor its link; a right a reload withdraws
stops the samples of the topic (DDS → AMQP: a new sample is not sent, a
message still queued for credit or released by the broker is dropped, each
with the `acl_denied` line) or settles each transfer `rejected` with
`amqp:unauthorized-access` (AMQP → DDS).

Subject: the SASL user of PLAIN, SCRAM and XOAUTH2; with EXTERNAL the
subject DN (RFC 4514, `CN=bridge-a,O=Example`) of the first certificate
of `amqp.tls.cert_file`, and that certificate's fingerprint
`mtls:<SHA-256 hex>` as a second name, the form of the other ZeroDDS
bridges. ANONYMOUS has no subject; only `*` admits it. A reload that
brings another client certificate brings its subject.

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout, one object per line. Every line begins
with `timestamp` (RFC 3339, UTC, milliseconds), `level` and `event`; the
lines of a participant carry its `tenant` (the lines of a tenant's
participant, §1.1) and its `container_id`. Then the fields of the event:
a link event (`attached`, `attach_refused`, `link_detached`,
`link_down`) names `link_name`, `dds_topic` and `amqp_address`; other events name
`dds_topic`, `link_name`, `broker` or `err` as they apply. Text values are
JSON strings with every control character escaped (RFC 8259 §7); counts
and times are JSON numbers.

`log_level` (`--log-level`) is the threshold. A line per transfer is
`debug`:

- `transfer_out`: a DDS sample sent to the broker; `delivery_id`, `bytes`.
- `disposition`: the round trip of a sample ends; `delivery_id`,
  `outcome` (`accepted`, `rejected`, `released`, `modified`, or `settled`
  on a link that sends settled, where the round trip ends with the
  transfer), `latency_us` from the sample taken from the DDS reader to the
  broker's disposition (to the transfer on a settled link).
- `transfer_in`: a transfer of the broker; `delivery_id`, `bytes`,
  `outcome` (what the bridge settled it with), `latency_us` from the
  transfer read from the connection to the bridge's disposition, the DDS
  write included.

The program's own lines (`startup`, `admin_endpoint`,
`admin_endpoint_bind_failed`, `signal_watcher_init_failed`) go the same
way. Only the error that ends the program goes to stderr, as
`error: <reason>`, beside the exit code of §2.

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
AMQP error condition if it is one AMQP 1.0 defines (Part 2 §2.8.15 to
§2.8.18, Part 4 §4.5.8), otherwise `other`, also for a rejection without
an error included. Every family also carries the label `tenant` (its
name) for the series of a tenant's participant (§1.1); the daemon's own
series have no `tenant` label. Label values come from the configuration
or from that closed set, never verbatim from the broker. In addition, the
generic families `zerodds_amqp_*` remain.

### §8.3 OTLP spans

With `OTEL_EXPORTER_OTLP_ENDPOINT` (an OTLP/HTTP collector, `http://host:port`)
every TRANSFER round trip is one trace, sent as OTLP/HTTP JSON to
`/v1/traces` every 5 s and once more when the program ends:

- AMQP → DDS: the root `amqp.transfer` (kind `SERVER`) from the transfer
  read from the connection to the bridge's disposition, with the child
  `dds.write` (kind `CLIENT`) around the write of the sample. A message
  the bridge rejects before a write (not a DDS sample, a withdrawn write
  right) has no child.
- DDS → AMQP: the root `amqp.transfer` (kind `CLIENT`) from the sample
  taken from the DDS reader to the broker's disposition, or to the
  transfer on a link that sends settled, with the child `dds.read` (kind
  `INTERNAL`) at the moment of the take. A message the broker gives back
  (`released`, `modified`) and the bridge sends again is one more round
  trip.

Attributes of the root: `messaging.system` = `amqp`,
`messaging.operation` (`receive` or `publish`),
`messaging.destination.name` (the `amqp_address`), `amqp.link_name`,
`amqp.delivery_id`, `amqp.outcome` (as the `outcome` of §8.1),
`dds.topic`, and `amqp.error_condition` for a rejection. The children
carry `dds.topic`. The status is `OK` for `accepted` and `settled`,
`ERROR` otherwise, with the error condition as its message. The span
kinds are those of the OTLP exporter of ZeroDDS, which has no `PRODUCER`
and `CONSUMER`.

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation. The SIGHUP/SIGTERM/SIGINT handlers (on
   Windows the console control handler) are installed right after it, so
   that a stop during the steps below — a TLS load, the DDS start, a
   connect or its back-off — is the shutdown of §9.2 and not the default
   action of the signal.
2. TLS cert load if enabled (log `tls_loaded`).
3. DCPS DomainParticipant init on `domain` (log `participant_started` with
   `domain` and `guid_prefix`).
4. Per topic: register reader+writer (according to `direction`: `in` = broker (receiver or sender link) → DDS, one DataWriter; `out` = DDS → broker (receiver or sender link), one DataReader; `bidir` = both); log `endpoints_registered` with the counts `readers` and `writers`.
5. AMQP connection open (+ SASL + TLS); log `connected`.
6. Per topic: SESSION + ATTACH; log `attached` per link.

The DDS endpoints stand before the broker is contacted: a DDS peer matches
them while the broker is unreachable. TLS material that does not load
ends the daemon with exit 1 before step 3.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Stop accepting new DDS samples. A sample the DDS reader delivers after
  the stop is not sent.
- Drain pending DISPOSITIONs (settled-pending): the daemon waits until
  the broker has settled every delivery in flight, at most 30 s. It does
  not wait for credit; a message still queued for credit is not sent,
  also when the broker grants credit during the drain. A delivery the
  broker gives back with `released` or `modified` during the drain is
  settled and waits in the queue; it is not sent again. A link the broker
  asks to drain has no message available and consumes its credit (§4.2).
- Send `DETACH` for all links + `END` for sessions + `CLOSE`.
- Clean up DDS entities.
- Exit 0.

The drain begins with the line `drain_started`, which names the
deliveries in flight (`in_flight`) and the messages queued for credit
(`queued`); from that line on the daemon takes no DDS sample.
What the stop leaves behind is counted in `errors_total` and logged:
deliveries still unsettled after 30 s (`shutdown_with_unsettled`, `count`),
messages queued for credit and samples delivered after the stop
(`shutdown_with_queued`, `count` and `not_taken`). A stop while no broker
connection stands counts the queued and unsettled messages the same way
and ends at once.

On Windows the console control handler takes the place of the signals:
CTRL_C and CTRL_BREAK stop the daemon as SIGTERM does. Closing the
console, logoff and system shutdown set the same stop, but Windows ends
the process as soon as the handler returns, so the drain above does not
run for them (the handler returns at once; the crate forbids the unsafe
code a handler that blocks until the drain has ended would need). Windows
has no SIGHUP; a reload there is a restart.

SIGHUP → config reload. The daemon reads the configuration again from the
sources it started from and checks it; one that does not load, or TLS
material that does not load, changes nothing (`reload_refused`). Otherwise,
per participant (the daemon's own, and each tenant from the tenant of the
same name, §1.1): the ACL applies at once, a withdrawn right stops the
topic's samples and transfers; the TLS material applies to the next
connect (`reloaded`). A right the new ACL grants, changed topics, broker
URL or domain need a restart, since the DDS endpoints are made at start;
they are logged (`reload_needs_restart`).

### §9.3 Reconnect

Connection drop → exponential backoff: the first delay is
`amqp.reconnect.initial_delay_ms`, each failed attempt multiplies it by
`factor` up to `max_delay_ms`; a connection that stands resets it. Deliveries
the broker had not settled go out again on the new connection, first and in
their order, before what was queued after them (the broker may then hold a
message twice; the message-id names the sample). The same holds for a session
the broker ends (§4.2): it is begun again on its channel after the back-off of
the link.

## §10 Cross-vendor

The daemon is a normal RTPS peer. AMQP side tested against RabbitMQ
(AMQP-1.0 plugin), ActiveMQ Artemis, Qpid Dispatch Router, Solace,
Azure ServiceBus.

Measured against RabbitMQ 4.0.5 (Linux test host) in
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, with `pika` (AMQP 0.9.1)
as an independent counterpart: DDS→AMQP, AMQP→DDS, an unreadable body is
rejected, multi-part transfers in both directions, loop prevention,
reconnect after the broker drops the connection, a rejected address next to
running links, catalog, TTL, and SASL EXTERNAL with the TLS client
certificate (`rabbitmq_auth_mechanism_ssl`). SCRAM-SHA-256 and SCRAM-SHA-512
against Apache ActiveMQ Artemis (`tests/bridge_artemis_e2e.rs`, an acceptor
with `SCRAMPropertiesLoginModule`), since RabbitMQ does not offer SCRAM;
XOAUTH2 against Apache Qpid Broker-J's OAuth2 provider
(`tests/bridge_brokerj_e2e.rs`). TLS/ALPN against a test server in the crate
(`tests/broker_tls_sasl.rs`).

The broker matrix (`tests/support/broker_matrix.rs`, qpid-proton's Python
binding as the client at the broker): DDS samples arrive as pass-through
messages and each round trip ends `accepted` at the bridge; a message
reaches a DDS reader and a bad body is rejected with `amqp:decode-error`; a
20 000-byte sample at a frame size of 4096 crosses in both directions; the
bridge connects again after the broker ends the connection and delivers
the sample written meanwhile; a refused address leaves the other link
running. Measured against Apache ActiveMQ Artemis 2.44.0
(`tests/bridge_artemis_e2e.rs`; a restart of the broker for the reconnect,
an address without auto-create for the refusal) and Apache Qpid Dispatch
Router 1.19.0 (`tests/bridge_dispatch_e2e.rs`; a router without a store:
the receiver attaches first, the router passes the bridge's outcome back to
the sender; a link-routed prefix without its connection for the refusal;
1.19.0 is built against CPython 3.11, since it ends with SIGSEGV on Python
3.13) and Solace PubSub+ Standard 10.26.0.9219 (`tests/bridge_solace_e2e.rs`;
the queues created through SEMP v2, the bridge over TLS with SASL PLAIN,
SEMP's `disconnect` of the AMQP clients for the reconnect; Solace attaches
every sender, so the refused link is the receiver of a queue with the
permission `no-access`). Solace comes as a container image only and starts
with 1 GB of `/dev/shm` and a hard `nofile` limit of 1048576; with Docker's
default of 64 MB `/dev/shm`, or a hard limit of 524288, its platform audit
ends in `POST Violation [022]`. Its CI job runs it as a service container
and needs a runner with `shm_size` of at least 1 GiB and a Docker daemon
whose default hard `nofile` limit is at least 1048576. Azure Service Bus:
not measured; its emulator needs its license terms accepted. The
claims-based security Azure Service Bus authorizes links with is built
(§7.2) and tested against the scripted broker.

The DDS side against another vendor (`tests/bridge_rabbitmq_e2e.rs`,
Cyclone DDS 11.0.1 through its Python binding): a Cyclone reader takes the
samples and the dispose the bridge writes from AMQP, and a Cyclone writer's
samples and dispose reach the queue with their payload, the content type of
their encapsulation and the instance's KeyHash in `dds:instance-handle`.
Cyclone sends no `PID_KEY_HASH`; the bridge's reader computes the KeyHash
from the topic's `type_file` (§3).

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

Per module ≥ 5 tests. The modules, under `crates/amqp-endpoint/src/`:

| Module | File |
|---|---|
| `config` | `bridge_daemon/config.rs` |
| `amqp_codec` | `broker.rs`: the frames of the client connection, on the codec of `crates/amqp-bridge/` |
| `link_state` | `bridge_daemon/link_state.rs` |
| `disposition` | `bridge_daemon/disposition.rs` |
| `sasl` | `broker/sasl.rs`: the client side of the SASL layer |
| `dds_pump` | `bridge_daemon/dds_pump.rs` |
| `acl` | `bridge_daemon/acl.rs` |

The tests of `link_state`, `disposition` and `dds_pump` read the frames a
`BrokerWriter` writes to a socket pair, without a broker; those of `sasl`
run against a broker side in the test that answers frame by frame.

### §12.2 Integration tests

- `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (against RabbitMQ 4.0 on
  127.0.0.1:5672, user `zerodds`; `#[ignore]`, `--ignored`): the bridge in the
  test process, a second DDS participant, `pika` as the counterpart; the cases
  from §10.
- `crates/amqp-endpoint/tests/broker_tls_sasl.rs`: TLS with a mandatory
  client certificate and ALPN `amqp`, SCRAM-SHA-256 (also wrong password),
  EXTERNAL, mechanism not offered.
- `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs`,
  `crates/amqp-endpoint/tests/bridge_brokerj_e2e.rs` (`#[ignore]`, in
  `integration-bridges-vendor-amqp`): SCRAM-SHA-256 and SCRAM-SHA-512 against
  Artemis (a wrong password is exit 5), XOAUTH2 against Broker-J (an unknown
  token is refused); the broker matrix of §10 against Artemis.
- `crates/amqp-endpoint/tests/bridge_dispatch_e2e.rs` (`#[ignore]`, in
  `integration-bridges-vendor-amqp`): the broker matrix of §10 through Qpid
  Dispatch Router.
- `crates/amqp-endpoint/tests/bridge_solace_e2e.rs` (`#[ignore]`, in
  `integration-bridges-vendor-amqp-solace`): the broker matrix of §10
  against Solace PubSub+, a service container of the job.
- `crates/amqp-endpoint/tests/bridge_e2e.rs`: the binary — exit codes 0 to 6
  (SIGTERM, configuration, TCP, DDS participant, TLS handshake against a
  certificate of another CA, SASL, a refused open), `--check-config` with the
  origin of each value, `--topic`.
- `crates/amqp-endpoint/tests/bridge_scripted.rs`: the bridge in the test
  process against a broker side the test drives frame by frame
  (`tests/support/scripted_broker.rs`): the fields of open and attach, one
  session per link, credit top-up, settle mode `second`, the message-id of a
  dispose, no echo of the bridge's own dispose, unsettled deliveries after a
  dropped connection and after an ended session.

### §12.3 Multi-vendor

- A Cyclone DDS subscriber behind the bridge and a Cyclone DDS publisher in
  front of it, with RabbitMQ (`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`,
  `ddsamqp_7_7_2_an_amqp_dispose_reaches_a_cyclone_reader`,
  `amqpbridge_12_3_a_cyclone_writer_reaches_the_queue` and, without
  `type_file`, `amqpbridge_12_3_a_cyclone_writer_without_type_file_gets_the_nil_handle`;
  `#[ignore]`, in `vendor-live-dispose-bridges`).
- The broker matrix (§10): RabbitMQ 4.0.5, ActiveMQ Artemis 2.44.0, Qpid
  Dispatch 1.19.0 and Solace PubSub+ Standard 10.26.0.9219 are measured;
  Azure Service Bus is not (its emulator needs its license terms
  accepted).

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
