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

L1: `mqtt.protocol_version: 3.1.1` (§3) makes the daemon an MQTT 3.1.1
client (protocol level 4). CONNECT, PUBLISH and the acknowledgements then
carry no properties: the CONNECT properties of §4.1 and the PUBLISH
properties of §4.2 are not sent, an inbound PUBLISH has none; the payload is
the same. A dispose or an unregister is not published (§4.4). The CONNACK
return codes 4 and 5 are exit 5, the others exit 2 (§2). In both versions the daemon keeps its connection alive (MQTT 5.0 and
MQTT 3.1.1 §3.1.2.10): when it has sent no packet for the Keep Alive in force
(the broker's Server Keep Alive, else `mqtt.keep_alive_secs`; 0 switches it
off), it sends PINGREQ; when the PINGRESP does not come within half the Keep
Alive, it closes the connection.

L6: each entry of `tenants` (§3) is a DomainParticipant of its own on its
`domain` and a broker connection of its own under its `mqtt.client_id`, with
its `topics` and its `acl`. The client identifier is the tenant's binding on
the broker: what the broker delivers under it is written into the tenant's
domain, what the tenant's participant receives is published under it. The
top-level keys are the participant and the connection of `domain`. The
connections share the broker URL, its TLS configuration, the auth mode and
the keys of the connection (`keep_alive_secs`, `session_expiry_interval_secs`,
`reconnect`, …); a tenant's `mqtt.username` and `mqtt.password` replace the
top level's user name and password (§7.2; where the top level presents
`auth.bearer_token`, the tenant's password replaces the token). A tenant's
ACL (§7.3) holds for its topics alone, with the tenant's subject; a tenant
without `acl` has the rules of an absent `acl` section. Two tenants of one name, and
one client identifier for two connections (two tenants, or a tenant and the
top level), prevent the start: two connections under one identifier take the
session from each other (MQTT 5.0 §3.1.4).

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
  --client-id <ID>             MQTT client ID (default: "zerodds-bridge-<host>",
                               the host name of the operating system;
                               "zerodds-bridge" on a host without one)
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
  0   normal shutdown (SIGTERM/SIGINT, also while the start connects)
  1   config error (also a mqtt.session_dir the daemon cannot create or
      write, or a session state another process holds, §4.1)
  2   broker connect error: not reachable within mqtt.connect_timeout_secs
      (§9.1; also a broker that does not answer the TLS handshake in time
      or closes the connection during it), or a CONNACK refusal other than
      those of exit 5
  3   DDS discovery error
  4   TLS error: TLS material that does not load (mqtt.tls.ca_file,
      cert_file, key_file), or a handshake the broker or the daemon refuses
      (a broker certificate not trusted or not for its name, no common
      protocol)
  5   auth error: CONNACK 0x86 Bad User Name or Password, 0x87 Not
      authorized, 0x8A Banned, 0x8C Bad authentication method; MQTT 3.1.1
      return code 4 or 5; an enhanced authentication whose client side
      fails (§7.2: a SCRAM signature of the broker that does not verify)
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
  protocol_version: 5                      # §1 L1: 5 | 3.1.1
  username: "${MQTT_USER}"
  password: "${MQTT_PASSWORD}"
  keep_alive_secs: 60
  drain_timeout_secs: 30                   # §9.2: wait for QoS 1/2 acks at shutdown
  clean_start: false
  session_expiry_interval_secs: 86400
  session_dir: "/var/lib/zerodds/mqtt-bridge"   # §4.1: the client's session state
  max_packet_size: 1048576
  receive_maximum: 1000
  topic_alias_maximum: 100
  connect_timeout_secs: 30                 # §9.1: exit 2 after this; 0 = retry without end
  tls:
    enabled: true
    ca_file: "/etc/zerodds/mqtt-ca.pem"
    cert_file: "/etc/zerodds/mqtt-client.pem"
    key_file: "/etc/zerodds/mqtt-client.key"
    server_name: "mosquitto.example.com"   # default: the host of broker_url
    verify_hostname: true
    alpn: ["mqtt"]
  reconnect:
    initial_delay_ms: 500
    max_delay_ms: 30000
    factor: 2.0
  own_topic_prefix: "$zerodds"             # §5.2, §6: <prefix>/<client_id>/...

auth:                                      # §7.2
  mode: "password"                         # none | password | mtls | enhanced
  # username: "${MQTT_USER}"               # password, SCRAM-SHA-256: win over mqtt.username
  # password: "${MQTT_PASSWORD}"           #   and mqtt.password
  # bearer_token: "${MQTT_TOKEN}"          # password: the CONNECT password; enhanced: the token
  # bearer_subject: "service-account"      # the bridge's subject for the ACL (§7.3)
  # enhanced:
  #   method: "SCRAM-SHA-256"              # SCRAM-SHA-256 | OAUTHBEARER | another method

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    mqtt_topic: "chat/message"
    direction: "bidir"             # in|out|bidir
    mqtt_qos: 1                    # 0|1|2 (auto-derived from DDS-QoS if absent)
    retain: false                  # auto-derived from the durability if absent
    data_representation: "xcdr2"   # xcdr1|xcdr2: the DDS endpoints' one representation
    schema_url: "https://schema.example/chat-message.idl"   # §5.2 catalog
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
      lifespan_ms: 60000             # §6: LIFESPAN, Message Expiry Interval
      deadline_ms: 5000              # §6: deadline_missed report
      liveliness: { kind: "automatic", lease_ms: 10000 }
      # partition: ["plant-a"]       # §6: the DDS partitions of the topic

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    mqtt_topic: "sensors/reading"
    direction: "out"
    mqtt_qos: 0
    qos:
      reliability: "best_effort"
      durability: "volatile"
      history: { kind: "keep_all" }

  - dds_name: "Config::Snapshot"
    dds_type: "Config::Snapshot"
    mqtt_topic: "config/snapshot"
    direction: "in"
    retain: true
    qos:
      reliability: "reliable"
      durability: "transient_local"
      history: { kind: "keep_last", depth: 1 }

tenants:                           # §1 L6: participants and connections beside those of `domain`
  - name: "plant-a"
    domain: 7                      # default: the top-level `domain`
    mqtt:
      client_id: "zerodds-bridge-plant-a"   # required; the tenant's binding on the broker
      username: "${PLANT_A_USER}"           # default: the top level's credentials
      password: "${PLANT_A_PASSWORD}"
    topics:                        # as the top-level `topics`
      - dds_name: "Plant::Status"
        mqtt_topic: "plant-a/status"
        direction: "out"
    acl:                           # as the top-level `acl`, for the tenant's topics
      default_deny: false

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

`mqtt.client_id` defaults to `zerodds-bridge-<host>` (§2). An empty one asks
the broker to assign one (MQTT 5.0 §3.1.3.1).

`mqtt.protocol_version` is `5` (the default) or `3.1.1` (§1 L1).

`mqtt.session_expiry_interval_secs` (default 0: the session ends with the
connection), `mqtt.max_packet_size` (1 to 16777216 octets, the default),
`mqtt.receive_maximum` (1 to 65535, default 65535) and
`mqtt.topic_alias_maximum` (0 to 65535, default 100) are the CONNECT
properties of §4.1. `clean_start: false` keeps the session across a
reconnect and a restart of the daemon only with a Session Expiry Interval
above 0 (under `mqtt.protocol_version: 3.1.1` always). `mqtt.session_dir`
(default `/var/lib/zerodds/mqtt-bridge`; `/usr/local/var/lib/zerodds/mqtt-bridge`
on macOS, `%PROGRAMDATA%\ZeroDDS\mqtt-bridge` on Windows, not empty) holds the
client side of such a session: per connection a file `<client id>.session`
(the octets of the client identifier outside `A-Z a-z 0-9 . _ -` written as
`%XX`) and its lock file `<client id>.lock` (§4.1).

`mqtt.connect_timeout_secs` (default 30) bounds how long the start tries a
broker it cannot reach (§9.1): a wait that would pass it is shortened to it,
and the last attempt starts there; 0 tries without end, and so does a value
too large for the clock of the system. An attempt waits at
most 10 s for the TCP connection of each address of the broker and 5 s for
each answer of the broker (TLS, the WebSocket upgrade, the CONNACK).
`mqtt.reconnect` gives
the waits between two attempts: `initial_delay_ms` (default 500, at least 1)
after the first, each further one `factor` times the last (default 2.0, at
least 1.0), at most `max_delay_ms` (default 30000, not below
`initial_delay_ms`).

`mqtt.own_topic_prefix` (default `$zerodds`) is the topic levels in front of
`<client_id>/` in the daemon's own topics: the catalog of §5.2, the reports
and the will of §6. MQTT 5.0 §4.7.2 leaves the topics beginning with `$` to
the server: HiveMQ 4.57.0 answers a CONNECT whose will topic begins with `$`
with CONNACK 0x82 and a PUBLISH on such a topic with DISCONNECT 0x82
(measured), so against HiveMQ the prefix is one without `$`, e.g.
`zerodds`. A prefix that is empty, carries `+`, `#` or U+0000 or ends with
`/` prevents the start; a change needs a restart (§9.2).

`mqtt.tls.server_name` is the name the broker certificate is checked against
and the SNI of the handshake; the default is the host of `broker_url`.
`mqtt.tls.verify_hostname` is `true`, the default; `false` prevents the
start, since the certificate is always checked against that name.
`mqtt.tls.alpn` lists the protocol names the TLS handshake offers (RFC 7301,
each 1 to 255 octets); the default is `["mqtt"]` under `mqtts://` and
`["http/1.1"]` under `wss://` (§7.1). A `mqtts://` or `wss://` URL without
`mqtt.tls.enabled: true`, and `mqtt.tls.enabled: true` with a `mqtt://` or
`ws://` URL, prevent the start.

`auth` (§7.2): `mode` is `none`, `password`, `mtls` or `enhanced`;
`bearer`, `sasl` and `sasl_plain` are other names of `password`. Not set,
the mode follows from what is configured: `enhanced` with
`auth.enhanced.method`, `password` with a user name, a password or
`bearer_token`, `mtls` with `mqtt.tls.cert_file` under `mqtt.tls.enabled`,
else `none`. `username` and `password` are the user name and password of
`password` and of SCRAM-SHA-256 and win over `mqtt.username` and
`mqtt.password`; `bearer_token` is the CONNECT password under `password`
(`auth.password` and `bearer_token` together prevent the start) and the
token under `enhanced` (OAUTHBEARER, and the Authentication Data of another
method); `bearer_subject` names the bridge's subject for the ACL (§7.3) in
place of the one it has; `enhanced.method` is the Authentication Method
under `enhanced`. Settings that do not fit the mode (§7.2) prevent the
start (exit 1).

`acl` (§7.3): `rules`, each with `subject` (not empty) and the lists
`allow_publish` and `allow_subscribe` of MQTT topic filters (a filter MQTT
5.0 §4.7 does not allow prevents the start); `default_deny` (not set:
`true` once `rules` has an entry, `false` without rules).

`schema_url` of a topic (optional) names where its type's schema is
published; the catalog of §5.2 carries it. `data_representation` of a topic
(`xcdr1` or `xcdr2`, optional) is the one data representation its DataWriter
offers from its creation and its DataReader accepts (DDS-XTypes 1.3
§7.6.3.1): a DDS reader that accepts only that representation matches the
DataWriter before the first PUBLISH, and a DDS writer that offers only the
other representation matches no DataReader of the topic (`out`, `bidir`;
§7.6.3.1.2), so its samples do not reach MQTT. Not set, the DataWriter offers XCDR1
(the default of DDS-XTypes 1.3 §7.6.3.1.1) and takes the representation the
header of each PUBLISH names (§4.2), and the DataReader accepts XCDR1 and
XCDR2; a reader of another participant that accepts only XCDR2 and is
discovered before the first PUBLISH then does not match (the XCDR2 readers
of Cyclone DDS 0.10.5, Fast DDS 3.1 and RTI Connext DDS 7.7.0, measured). `qos.reliability` is `reliable`
(the default) or `best_effort`, `qos.durability` `volatile` (the default),
`transient_local`, `transient` or `persistent`.

`qos.lifespan_ms` (at least 1; not set: infinite) is the LIFESPAN of the
topic's DataWriter and the Message Expiry Interval of a sample whose writer
names no lifespan; `qos.deadline_ms` (at least 1; not set: infinite) the
DEADLINE its DataReader requests and its DataWriter offers;
`qos.liveliness.kind` (`automatic`, the default, `manual_by_participant`,
`manual_by_topic`) and `qos.liveliness.lease_ms` (at least 1; not set:
infinite) the LIVELINESS of both; `qos.partition` the PARTITION names of
both (not set: the default partition). A `lifespan_ms`, `deadline_ms` or
`lease_ms` of 2147483647000 or more (2^31 - 1 s, beyond the seconds of a
DDS Duration_t) is infinite as well. §6 says what they do.

`qos.history.kind` of a topic is `keep_last` (the default) or `keep_all`,
`qos.history.depth` (default 10, at least 1) the N of `keep_last`: the
HISTORY of the topic's DataWriter (`in`, `bidir`) and the samples the daemon
holds for the broker on a topic with a DataReader (`out`, `bidir`, §6).

`metrics.path` (default `/metrics`) is the path of the Prometheus metrics on
the admin endpoint at `metrics.listen` (§8.2); it begins with `/`, has no
`?` and is not `/`, `/catalog` or `/healthz`, the endpoint's own paths.

`tenants` (§1 L6): an entry has `name` (unique, without `/`), `domain`
(default: the top-level `domain`), `mqtt` with `client_id` (required, not
the one of the top level or of another tenant), `username` and `password`,
`topics` (the form of the top-level `topics`) and `acl` (the form of the
top-level `acl`). Another key in an entry prevents the start, naming file
and line. A change of `tenants` other than a tenant's `acl` takes effect
with a restart (§9.2).

## §4 MQTT wire protocol

### §4.1 Handshake (CONNECT)

The daemon is an MQTT 5 client towards the broker. CONNECT properties:

| Property | Value |
|----------|------|
| `Session Expiry Interval` | from config (`session_expiry_interval_secs`) |
| `Receive Maximum` | from config (`receive_maximum`) |
| `Maximum Packet Size` | from config (`max_packet_size`) |
| `Topic Alias Maximum` | from config (`topic_alias_maximum`, default 100) |
| `Request Response Information` | 0 |
| `Request Problem Information` | 1 |
| `User Property[zerodds_version]` | `"1.0"` |
| `User Property[zerodds_role]` | `"bridge"` |
| `Authentication Method` | optional (e.g. `"SCRAM-SHA-256"`, `"OAUTHBEARER"`) |
| `Authentication Data` | optional |

A CONNACK that refuses the credentials or the authorization (0x86 Bad User
Name or Password, 0x87 Not authorized, 0x8A Banned, 0x8C Bad authentication
method) ends the daemon with exit 5, every other refusal with exit 2 (§2).

The daemon holds what its CONNECT announces (MQTT 5.0 §3.1.2.11, §4.9,
§4.13): a packet of the broker above Maximum Packet Size ends the connection
with DISCONNECT 0x95 before it is read whole, a Topic Alias above Topic
Alias Maximum with 0x94 (an alias within it is resolved to its topic), more
unreleased QoS 2 messages than Receive Maximum with 0x93, a packet that does
not decode with 0x81 Malformed Packet (MQTT 5.0 §4.13.1). It holds what the
CONNACK announces: Receive Maximum, Maximum Packet Size, Maximum QoS and
Retain Available bound its PUBLISH packets; the PUBLISH of a sample they
forbid is not sent, the daemon logs it and counts it as an error, and names
each topic whose QoS or retain flag the broker forbids once after the
CONNACK. The broker's Server Keep Alive replaces `keep_alive_secs`. Under
`mqtt.protocol_version: 3.1.1` the CONNECT carries no properties (§1 L1).

Session State (MQTT 5.0 §4.1): under `clean_start: false` with a Session
Expiry Interval above 0 (MQTT 3.1.1: `clean_start: false`) the session
outlives the connection, and the client MUST store its side of it after the
connection closed ([MQTT-3.1.2-23]). The daemon keeps it — its QoS 1/2
PUBLISH packets the broker has not completed, and the broker's QoS 2
PUBLISH packets it answered with PUBREC whose PUBREL has not come — in its
file in `mqtt.session_dir` (§3), written before the packet it concerns goes
out, so that it also outlives a restart or a crash of the daemon; the file
is not synced to the disk, and a crash of the operating system can lose its
last records. A record the disk takes only in part (a full disk) is cut off
again; a file that ends inside its last record (the daemon ended while
writing it) gives the records before it, with the warning
`session_file_truncated`. On a CONNACK with Session Present 1 the daemon
resends those PUBLISH packets with DUP 1 and the PUBREL packets, with their packet
identifiers and in their order (MQTT 5.0 §4.4, §4.6), as many as the
broker's Receive Maximum lets through, the rest as acknowledgements come; a
new PUBLISH takes an identifier not in use. A QoS 2 PUBLISH of the broker it
answered before is not written into DDS a second time. On Session Present 0
it discards that state and logs how many message flows it discarded
([MQTT-3.2.2-5]). A broker that resumes a session the daemon holds no state
of (Session Present 1 without a file) gets its connection closed, and the
daemon begins a new session with Clean Start 1 ([MQTT-3.2.2-4]; under MQTT
3.1.1 a third CONNECT with Clean Session 0 then begins the lasting session);
Session Present 1 to a CONNECT with Clean Start 1 is a protocol error
(DISCONNECT 0x82, exit 2). `clean_start: true` removes the file of an
earlier session ([MQTT-3.1.2-4]). A `mqtt.session_dir` the daemon cannot
create or write, a file another process holds, or one it cannot read
prevents the start (exit 1). A file of the other protocol level holds no
state of this connection. With an empty `mqtt.client_id` no state is kept:
the broker assigns a new identifier at each CONNECT, so no session is
resumed.

### §4.2 Publish frame

DDS→MQTT: the daemon sends PUBLISH with the CDR payload as the `payload`
field:

```
PUBLISH
  Topic Name:       <mqtt_topic from config>
  Packet Identifier: <auto, QoS 1 and 2>
  Properties:                       # in the order of MQTT 5.0 Table 2-4
    Payload Format Indicator: 0     (binary)
    Message Expiry Interval:  <the writer's Lifespan QoS in seconds, rounded,
                               at least 1; absent for an infinite lifespan>
    Content Type:             "application/x-dds-cdr2"   (XCDR2 headers 00 06 to 00 0b)
                              "application/x-dds-cdr"    (XCDR1 headers 00 00 to 00 03)
    User Property:            ("zerodds_type", "<DDS-Type-Name>")
    User Property:            ("zerodds_topic", "<DDS-Topic-Name>")
    User Property:            ("zerodds_flags", "0x<2 hex digits>")  # PID_STATUS_INFO flags:
                                                       # 0x00 sample, 0x01 disposed, 0x02 unregistered
    User Property:            ("zerodds_key_hash", "<32-hex>")   # when the sample has a key hash
    User Property:            ("zerodds_source_ts_ns", "<u64>")  # when it carries a source timestamp
  Payload:
    <encapsulation header>          # the header the sample came under, e.g.
                                    # [0x00, 0x07, 0x00, 0x00] (XCDR2-LE)
    <CDR-Bytes>                     # per zerodds-xcdr2-bindings-conformance-1.0 §3
```

The header is the one of DDSI-RTPS 2.5 §10.5 and DDS-XTypes 1.3 Table 60 the
sample arrived with; the CDR bytes go on unchanged, so the identifier names
their XCDR version, their format (plain, delimited, parameter list) and
their byte order. `zerodds_source_ts_ns` is the source timestamp in
nanoseconds since the Unix epoch.
A dispose and an unregister of an instance are a PUBLISH of their own with
`zerodds_op` (§4.4).

MQTT→DDS: receiving a PUBLISH reads the payload as an encapsulation header
and the CDR bytes behind it; the header names the representation, the format
and the byte order of the DDS write, a header that is no CDR representation
is refused. On a topic with `data_representation` (§3) the DataWriter's
representation does not change (DDS-XTypes 1.3 §7.6.3.1): a header of the
other XCDR version is refused. `zerodds_key_hash` makes the write one of that instance,
`zerodds_source_ts_ns` gives it its source timestamp. A Payload Format
Indicator 1, a Content Type other than the two above or one that names the
other XCDR version than the header, a key hash that is not 32 hex digits and
a timestamp that is no number of nanoseconds an RTPS time holds are refused:
the daemon logs the PUBLISH and counts it as an error, nothing is written. A
PUBLISH without properties (an MQTT 3.1.1 client, `mosquitto_pub` without
`-D`) is the payload alone.

### §4.3 Subscribe

For each topic filter of the `direction=in|bidir` topics in the config the
daemon sends one SUBSCRIBE with:
```
SUBSCRIBE
  Filter:                      "<mqtt_topic>"
  Subscription Identifier:     <per-filter counter, from 1 in the order of the config>
  QoS:                         <§6: mqtt_qos, else RELIABLE→1, BEST_EFFORT→0>
  No Local:                    1
  Retain As Published:         0
  Retain Handling:             0 (send retained on subscribe)
```

Topics with the same `mqtt_topic` share one subscription, its QoS the
highest of theirs; a topic whose subscription the ACL refuses (§7.3) is not
subscribed. The start waits for the SUBACK of each SUBSCRIBE (5 s, else
exit 2). A subscription the broker refuses (Reason Code 0x80 or above) is
logged with its filter and Reason Code and counted as an error; its topics
get no message. A granted QoS below the one asked for is logged. A broker
whose CONNACK says Subscription Identifier Available 0 gets no identifier;
a filter with `+` or `#` is not sent to a broker with Wildcard Subscription
Available 0 and is logged as refused (MQTT 5.0 §3.2.2.3).

A PUBLISH of the broker goes to the topics its Subscription Identifiers
name, each whose filter also matches its topic name; one without an
identifier goes to every topic whose filter matches its topic name (MQTT
5.0 §4.7). A PUBLISH that matches no subscribed topic is not written: the
daemon counts it as an error and logs its topic name once.

On a bidir topic the bridge's own messages do not come back: the broker
does not send the bridge's PUBLISH back to it (No Local), and the DataReader
of the topic does not publish the samples of the bridge's own DataWriter,
which came from the broker. Under `mqtt.protocol_version: 3.1.1` a
SUBSCRIBE has no properties and its options byte is the QoS alone (MQTT
3.1.1 §3.8.3.1): the broker sends the bridge's PUBLISH on a bidir topic
back, and the daemon recognises the copy by its topic and payload (within
30 s of the PUBLISH) and does not write it into DDS.

### §4.4 Control properties

`User Property[zerodds_op]` values:
- `"sample"` (default) — normal DATA
- `"dispose"` — instance dispose (DDS sends as WriteOp::Dispose)
- `"unregister"` — instance unregister
- `"register"` — instance register (rarely over the wire)

If the property is missing, `"sample"` is assumed.

DDS→MQTT: a dispose or an unregister of an instance that the DataReader of
an `out|bidir` topic receives becomes a PUBLISH on the topic's
`mqtt_topic`, with the QoS and the retain flag of its samples (§6; on a
retained topic the retained message is the last change of the topic) and
in the history of its instance:

```
PUBLISH
  Properties:
    Payload Format Indicator: 0
    Content Type:             "application/x-dds-cdr2"
    User Property:            ("zerodds_type", "<DDS-Type-Name>")
    User Property:            ("zerodds_topic", "<DDS-Topic-Name>")
    User Property:            ("zerodds_op", "dispose" | "unregister")
    User Property:            ("zerodds_flags", "0x01" | "0x02" | "0x03")
    User Property:            ("zerodds_key_hash", "<32-hex>")
    User Property:            ("zerodds_source_ts_ns", "<u64>")  # when it carries one
  Payload:
    00 06 00 00                     # CDR2_BE
    <key holder>                    # DDS-XTypes 1.3 §7.6.8, big-endian PLAIN_CDR2,
                                    # when the DataReader recovered one
```

A dispose is `dispose` with 0x01, an unregister `unregister` with 0x02, a
dispose and unregister in one change `dispose` with 0x03. A sample carries
no `zerodds_op`.

Under `mqtt.protocol_version: 3.1.1` (§1 L1) a PUBLISH has no properties
and cannot carry `zerodds_op`; its header and key holder would be taken for
a sample. The daemon does not publish a dispose or an unregister then: it
logs the first one of a topic and counts each in `errors_total`; in its
turn in the history of its instance it removes the instance from the
replay buffer (§6). Every inbound PUBLISH is a sample.

MQTT→DDS: `dispose` and `unregister` name their instance by
`zerodds_key_hash`; the payload is empty, the header `00 06 00 00` alone,
or the header and the key holder. They are written as the dispose (with
0x03 the dispose and unregister) or the unregister of that instance by the
topic's DataWriter. `register` writes nothing: DDS sends no message for the
registration of an instance (DDS 1.4 §2.2.2.4.2.5). `zerodds_flags`, when
present, agrees with the operation (0x00 `sample`, 0x01 or 0x03 `dispose`,
0x02 `unregister`). Another value of `zerodds_op`, flags that contradict
it, a dispose or unregister without key hash or with a key holder under
another header are refused: the daemon logs the PUBLISH and counts it as an
error, nothing is written.

## §5 Topic mapping

### §5.1 Slug algorithm

Topic name `Chat::Message` → default MQTT topic slug:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Other non-`[a-z0-9/_-]` → `_`
4. Result: `chat/message`

Override via the `mqtt_topic` field in the config.

The `mqtt_topic` of an `out` or `bidir` topic is a topic name, the daemon
publishes on it: it has no wildcard `+` or `#` (MQTT 5.0 §4.7.1,
[MQTT-3.3.2-2]). The one of an `in` topic is a topic filter the daemon
subscribes to: `+` and `#` each fill a level, `#` is the last one (MQTT 5.0
§4.7.1). Another value prevents the start (exit 1), naming the entry with
file and line.

Examples:
| DDS topic | MQTT default |
|-----------|--------------|
| `Chat::Message` | `chat/message` |
| `Sensor::Reading` | `sensors/reading` (with explicit override) |
| `RoboticArm::Joint::Position` | `roboticarm/joint/position` |

### §5.2 Type discovery

On connect (after each CONNACK) the daemon publishes a retained catalog on
`<own_topic_prefix>/<client_id>/catalog` (`<own_topic_prefix>` is
`mqtt.own_topic_prefix`, default `$zerodds`, §3; the same holds for the
topics of §6 and §7.3):
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

`<client_id>` is the client identifier of the connection, the one the
broker assigned when it assigned one. With an empty `mqtt.client_id` the
broker may assign another identifier at each connect: after a reconnect
under a new one the daemon clears the catalog of the identifier before (a
retained PUBLISH without payload), so the bridge is listed once. The PUBLISH has QoS 1 (0 when the
broker's Maximum QoS is 0), Payload Format Indicator 1 and Content Type
`application/json`. `qos` names the reliability and the durability as they
apply (`reliable` and `volatile` when not set); `schema_url` is the topic's
key of that name and is left out when the topic has none. The connection of
a tenant (§1 L6) publishes the catalog of the tenant's topics; `/catalog` on
the admin endpoint (§8.2) serves the document of the top level. At the
shutdown (§9.2) the daemon clears the retained catalog with a retained
PUBLISH without payload before its DISCONNECT, so a stopped bridge is not
listed; after a crash the catalog stays. A client identifier with `+` or
`#` cannot be a topic level: the daemon then publishes no catalog and logs
it, as it logs and counts a catalog the broker's CONNACK forbids (Retain
Available 0). MQTT 5.0 §4.7.2 leaves the topics beginning with `$` to the
server; Mosquitto 2.0.21, EMQX 5.8.6 and Aedes 1.0.2 take and keep the
catalog, HiveMQ 4.57.0 takes it under a prefix without `$` (measured, §10).

## §6 QoS translation

| DDS QoS | MQTT behavior |
|---------|----------------|
| Reliability `RELIABLE` | MQTT QoS 1 or 2 (config override; default 1) |
| Reliability `BEST_EFFORT` | MQTT QoS 0 |
| Durability `VOLATILE` | retain=false |
| Durability `TRANSIENT_LOCAL` | retain=true (the last sample is held on the broker) |
| Durability `TRANSIENT/PERSISTENT` | retain=true + the daemon's own replay buffer on cold start |
| History `KEEP_LAST(N)` | daemon-internal buffer N per instance |
| History `KEEP_ALL` | daemon backpressure (no drop) |
| Lifespan | MQTT property `Message Expiry Interval` (seconds, rounded) |
| Deadline | observed, the daemon emits `<own_topic_prefix>/.../deadline_missed` retain |
| Liveliness | MQTT will message on `liveliness_lost` |
| Partition | filter in the daemon before the MQTT publish |

History: the daemon publishes the samples of a topic one at a time, in the
order they arrived. While a QoS 1/2 PUBLISH waits for the broker's
acknowledgements (its Receive Maximum is reached) or the connection is lost
(§9.3), the samples that arrive wait in the daemon: under `KEEP_LAST(N)` the
newest N of each instance (an instance is a key hash; the samples without
one are one instance), the oldest of an instance is dropped and counted in
`zerodds_mqtt_bridge_dds_samples_dropped_total`; under `KEEP_ALL` every sample, and
the PUBLISH waits as long as the broker acknowledges nothing. A sample the
broker's CONNACK forbids (§4.1) is not sent under either kind.

Replay buffer (`TRANSIENT`, `PERSISTENT`): for an `out` or `bidir` topic of
these durabilities the daemon keeps the PUBLISH of the newest sample of
each instance that went out; a dispose or an unregister (§4.4) removes the
instance when it goes out. After each CONNACK (the start and each
reconnect, §9.3) it publishes them again, oldest first, with the QoS and
the retain flag of the topic: a broker that restarted without its retained
messages gets them again. The samples that waited in the history while
the connection was lost follow, each once and in their order.
`TRANSIENT` keeps them in memory; `PERSISTENT` also in the file
`<client id>.replay` in `mqtt.session_dir` (the octets of the identifier
outside `A-Z a-z 0-9 . _ -` as `%XX`), so a restarted daemon publishes them
again. A sample whose Message Expiry Interval has passed since it arrived
is not published again; the interval of one that is, is reduced by the
time since. A record the disk takes only in part (a full disk) is cut off
again, so that the next record does not follow a broken one. A file that
ends inside its last record (the daemon ended while writing it) gives the
records before it, with the warning `replay_file_truncated`, and is written
again without it. A file the daemon cannot read otherwise prevents the start
(exit 1). With an empty `mqtt.client_id` a persistent topic is kept in
memory only.

Lifespan: the Message Expiry Interval of a PUBLISH is the lifespan of the
sample's writer, else `qos.lifespan_ms` (seconds, rounded, at least 1, at
most 4294967295; none for an infinite lifespan), less the time since the
sample arrived: a sample
that waits in the daemon (History above, §9.3) goes out with the time left,
one whose lifespan has passed is not published and is counted in
`zerodds_mqtt_bridge_dds_samples_dropped_total`. `qos.lifespan_ms` is the LIFESPAN of the topic's
DataWriter, which the samples written from MQTT carry. DDS has no lifespan
per sample: the Message Expiry Interval of an inbound PUBLISH does not
reach DDS; the broker does not deliver a message whose interval has passed
(MQTT 5.0 §3.3.2.3.3).

Deadline: `qos.deadline_ms` is the DEADLINE the DataReader of the topic
requests and its DataWriter offers. When a DDS writer misses it
(REQUESTED_DEADLINE_MISSED of the DataReader) or the MQTT publishers do
(OFFERED_DEADLINE_MISSED of the DataWriter), the daemon publishes, retained,
on `<own_topic_prefix>/<client_id>/deadline_missed/<slug>`
(`<own_topic_prefix>` is `mqtt.own_topic_prefix`, default `$zerodds`, §3;
`<slug>` the slug of the DDS topic name, §5.1):

```json
{"dds_topic":"Chat::Message","endpoint":"reader","total_count":3,"key_hash":"<32-hex>"}
```

`endpoint` is `reader` or `writer`, `total_count` the count of the status,
`key_hash` the instance that missed it last (`null` when not known).
`<client_id>` here and in the liveliness report below is the client
identifier of the connection in force, the one the broker assigned when it
assigned one (§5.2).

Liveliness: the CONNECT carries a will on
`<own_topic_prefix>/<client_id>/liveliness_lost` (QoS 0, not retained, Payload Format
Indicator 1, Content Type `application/json`, the payload
`{"client_id":"<client_id>"}`): when the connection ends without DISCONNECT
the broker publishes it (MQTT 5.0 §3.1.2.5). There is no will with an empty
`mqtt.client_id` or one with `+` or `#`. `qos.liveliness` is the LIVELINESS
of both endpoints of the topic; when a DDS writer of the DataReader loses
its liveliness (the `not_alive_count` of LIVELINESS_CHANGED grows) the
daemon publishes, not retained, on
`<own_topic_prefix>/<client_id>/liveliness_lost/<slug>`:

```json
{"dds_topic":"Chat::Message","alive_count":0,"not_alive_count":1}
```

Partition: `qos.partition` is the PARTITION of the topic's DataReader and
DataWriter. The DataReader matches only DDS writers of its partitions
(DDS 1.4 §2.2.3.13): a sample of another partition does not reach the
daemon and is not published; the samples written from MQTT are in the
partitions of the DataWriter.

The reports of deadline and liveliness have Payload Format Indicator 1,
Content Type `application/json` and QoS 1 (0 when the broker's Maximum QoS
is 0). The daemon looks at the statuses every 200 ms.

QoS auto-derivation: if `mqtt_qos` is not set in the config, it is
derived from `qos.reliability` (RELIABLE→1, BEST_EFFORT→0). If
`retain` is not set, it is derived from `qos.durability`
(TRANSIENT_LOCAL, TRANSIENT and PERSISTENT→true, otherwise false). A value
that is set is kept, also against the derivation: `mqtt_qos: 0` on a
reliable topic stays 0.

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
until the natural reconnect): a reload reads `mqtt.tls.ca_file`,
`cert_file` and `key_file` again and takes `server_name` and `alpn`; each
connection that begins after it (§9.3) uses them, the one that stands keeps
its own. TLS material that does not load refuses the reload and keeps the
settings of before. A change of `mqtt.tls.enabled` or `mqtt.broker_url`
needs a restart.

### §7.2 Auth modes

`auth.mode` (§3) says how the daemon authenticates towards the broker:

- `none`: anonymous CONNECT (dev mode), without User Name, Password and
  client certificate. Only for a broker on the loopback interface: a host of
  `mqtt.broker_url` other than `localhost`, an address of 127.0.0.0/8 or
  `::1`, and a user name, password, token or client certificate in the
  configuration prevent the start (exit 1).
- `password` (also `bearer`, `sasl`, `sasl_plain`): CONNECT with `User Name`
  (`auth.username`, else `mqtt.username`) and `Password` (`auth.password`,
  else `auth.bearer_token`, else `mqtt.password`); one of the two is needed.
  Under `mqtt.protocol_version: 3.1.1` a password needs a user name (MQTT
  3.1.1 §3.1.2.9).
- `mtls`: client cert auth, no user/pass in the CONNECT: `mqtt.tls.enabled`
  with `cert_file` and `key_file` is needed; a configured user name,
  password or token prevents the start.
- `enhanced` (MQTT 5.0 §4.12): `Authentication Method`
  (`auth.enhanced.method`) and `Authentication Data` in the CONNECT, no User
  Name and no Password; it needs `mqtt.protocol_version: 5`.
  - `SCRAM-SHA-256` (RFC 5802, RFC 7677): the data of the CONNECT is the
    client-first-message with the user name (`auth.username`, else
    `mqtt.username`) and a nonce of 24 random octets; the broker's AUTH
    0x18 with the server-first-message is answered with AUTH 0x18 and the
    client-final-message (password `auth.password`, else `mqtt.password`);
    the CONNACK carries the server-final-message, whose signature the daemon
    checks. User name and password are prepared with SASLprep (RFC 4013);
    the iteration count is at least 4096 (RFC 7677 §4) and at most
    10 000 000. Other SCRAM methods prevent the start.
  - `OAUTHBEARER` (RFC 7628): the initial client response with the host and
    the port of the broker and `auth=Bearer <auth.bearer_token>`; an error
    challenge (AUTH 0x18 with the status as JSON) is answered with `%x01`
    (RFC 7628 §3.2.3), and the refusal that follows names the status.
  - another method (a JWT bearer, for example): `auth.bearer_token` is the
    Authentication Data of the CONNECT, one message; an AUTH 0x18 of the
    broker ends the connection.

  Until the CONNACK the daemon sends no packet other than AUTH
  ([MQTT-3.1.2-30]). An AUTH or a CONNACK whose Authentication Method is
  not the one of the CONNECT ([MQTT-4.12.0-5]), and an AUTH or an
  Authentication Method in the CONNACK after a CONNECT without one
  ([MQTT-4.12.0-6]), end the connection with DISCONNECT 0x82. When the
  daemon's side of the exchange fails (a server-first-message it cannot
  read, a server signature that does not verify: the broker does not know
  the password), it sends DISCONNECT 0x80, and the start ends with exit 5.

  Re-authentication (MQTT 5.0 §4.12.1): a SIGHUP reload (§9.2) that changes
  the credentials and keeps `auth.mode` and `auth.enhanced.method` makes
  each connection with enhanced authentication send AUTH 0x19 with the
  method and the first data of the new credentials; each AUTH 0x18 of the
  broker is answered with AUTH 0x18, its AUTH 0x00 ends the exchange (under
  SCRAM-SHA-256 with the server signature checked). Meanwhile the other
  packets flow. A failure on the daemon's side ends the connection with
  DISCONNECT 0x80; the daemon connects again (§9.3) with the new
  credentials. An AUTH of the broker outside a re-authentication is a
  protocol error (DISCONNECT 0x82).

A tenant (§1 L6) authenticates with its own `mqtt.username` and
`mqtt.password` in the mode of the top level: they take the place of
`auth.username`/`mqtt.username` and `auth.password`/`mqtt.password`
under `password` and under SCRAM-SHA-256; where the top level presents
`auth.bearer_token` (`password` without `auth.password`, OAUTHBEARER,
another method), the tenant's `mqtt.password` is its token. A tenant's
user name or password that the mode does not use (`none`, `mtls`, a user
name under OAUTHBEARER or another token method) prevents the start
(exit 1) with a message that names the tenant.

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

Each rule grants its `subject` (`*`: every subject) MQTT topics by topic
filter (MQTT 5.0 §4.7): `allow_publish` the PUBLISH packets the daemon sends
to the broker (DDS → MQTT) on a topic name one of its filters matches;
`allow_subscribe` the subscriptions whose filter lies within one of its
filters, and the PUBLISH packets the broker delivers on a topic name one of
its filters matches. A topic name or filter that only rules of other
subjects name is refused; one that no rule names is allowed under
`default_deny: false` and refused under `true` (not set: `true` once a rule
exists, `false` without rules). A refused subscription is not sent, a
refused sample is not published, a refused PUBLISH of the broker is not
written into DDS; each refusal is counted in
`zerodds_mqtt_bridge_acl_denials_total` (`reason` `publish` or `subscribe`,
§8.2) and logged as `acl_denied` (the refused samples of a topic and the
refused PUBLISH packets on one MQTT topic once). The daemon's own topics
under `<own_topic_prefix>/<client_id>/` (`mqtt.own_topic_prefix`, default
`$zerodds`, §3: the catalog of §5.2, the reports and the will of §6) are not
subject to the ACL. A SIGHUP reload replaces the rules at
once (§9.2); a withdrawn right also holds for the replay buffer (§6).

Subject resolution: from the TLS cert DN, MQTT username, or JWT `sub`
claim (in this order). The subject is the daemon's own identity towards the
broker: the subject DN of the client certificate in `mqtt.tls.cert_file`
(under `mqtt.tls.enabled`) as an RFC 4514 string (`O=Plant,CN=bridge-01`),
else the user name it authenticates with (`password`, SCRAM-SHA-256, §7.2),
else the `sub` claim of the JWT it presents (`auth.bearer_token`, read
without checking its signature). A rule may also name the client
certificate by its fingerprint `mtls:<SHA-256 hex>`. `auth.bearer_subject`
names the subject in place of these. Without any of them the daemon has no
subject, and only the rules of `*` apply. Each tenant (§1 L6) has its own
rules and its own subject.

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout: one JSON object per line, written when its
level reaches `log_level` (`--log-level`: `trace`, `debug`, `info`, `warn`,
`error`; default `info`). A SIGHUP reload applies a changed `log_level`.
Every line has `timestamp` (RFC 3339 §5.6 in UTC with milliseconds,
`1970-01-01T00:00:00.000Z`), `level` and `event`, then the fields of its
event: `mqtt_topic`, `dds_topic` (the DDS topic of the configuration),
`bytes` (the payload octets of the PUBLISH), `mqtt_qos` (the QoS of the
PUBLISH), `latency_us`, `connect_state` (`connecting`, `connected`,
`reconnecting`, `disconnected`), `tenant` (§1 L6; only on the lines of a
tenant's connection) and the others the table names. Numbers are JSON
numbers; a Reason Code is a text such as `0x8e`, as in §8.2. Run as the
Windows service (§11), which has no stdout, the daemon appends the lines
to `%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log` (zerodds-deployment-1.0
§4.1) instead.

| Event | Level | Fields |
|-------|-------|--------|
| `start` | info | `domain`, `broker`, `topics`, `tenants` (their numbers) |
| `auth` | info | `mode` (§7.2), `acl_rules`, `broker_tls` |
| `connect` | info | `connect_state` connected, `client_id` (the one the broker assigned, when it assigned one), `protocol` (`5.0` or `3.1.1`), `keep_alive_secs`, `session_present` |
| `connect_failed` | warn | `connect_state` (`connecting` at the start, §9.1; `reconnecting` after a lost connection, §9.3), `reason`, `next_attempt_ms` (the wait before the next attempt) |
| `disconnect` | warn; info at the shutdown | `connect_state` disconnected, `reason_code` (the Reason Code of the DISCONNECT that ended the connection, or `network`, as in §8.2; `0x00` at the shutdown), `reason` |
| `subscribed` | info | `granted`, `filters` (the subscriptions the broker granted, of those sent, §4.3) |
| `subscription_refused` | warn | `mqtt_topic` (the filter), `reason_code` (of the SUBACK; none when the broker has no wildcard subscriptions), `reason` |
| `subscription_downgraded` | warn | `mqtt_topic`, `mqtt_qos` (granted), `requested_qos` |
| `topic_not_published` | warn | `dds_topic`, `mqtt_topic`, `mqtt_qos`, `reason`: the CONNACK's Maximum QoS or Retain Available forbids the topic's PUBLISH (§4.1) |
| `publish_out` | debug | `mqtt_topic`, `dds_topic`, `bytes`, `mqtt_qos`, `latency_us`: from taking the sample from the DataReader to the end of the socket write of its PUBLISH |
| `publish_in` | debug | `mqtt_topic`, `dds_topic`, `bytes`, `mqtt_qos`, `latency_us`: from reading the PUBLISH to the return of its DDS write |
| `publish_unrouted` | warn | `mqtt_topic`, `bytes`, `mqtt_qos`, `reason`: a PUBLISH of the broker that matches no subscribed topic (the first of each MQTT topic) |
| `publish_refused` | warn | `mqtt_topic`, `dds_topic`, `bytes`, `mqtt_qos`, `reason`: DDS refused the PUBLISH's sample (§4.2) |
| `publish_failed` | warn | `mqtt_topic`, `dds_topic`, `bytes`, `mqtt_qos`, `reason`: a PUBLISH that was not sent |
| `publish_refused_by_broker` | warn | `client_id`, `packet_id`, `reason_code` (of the PUBACK or PUBREC) |
| `samples_waiting` | warn | `dds_topic`, `mqtt_topic`, `waiting`, `reason`: the samples wait for the connection or for the broker's acknowledgements (§6, §9.3) |
| `sample_expired` | warn | `dds_topic`, `mqtt_topic`, `reason` (the first of each topic, §6) |
| `lifecycle_not_published` | warn | `dds_topic`, `mqtt_topic`, `reason` (§4.4; the first under MQTT 3.1.1) |
| `acl_denied` | warn | `action` (`subscribe`, `publish`, `receive`), `mqtt_topic`, `dds_topic`, `subject` (§7.3; the first of each topic) |
| `catalog_published`, `catalog_cleared` | info | `mqtt_topic` (§5.2); `topics`, `bytes`, `mqtt_qos` when published |
| `catalog_failed` | warn | `mqtt_topic` or `client_id`, `reason` |
| `replay` | info | `sent`, `kept`, `expired` (the replay buffer of §6 after a CONNACK) |
| `replay_failed` | warn | `dds_topic`, `mqtt_topic`, `reason` |
| `status_report_failed` | warn | `mqtt_topic`, `reason` (§6) |
| `reauth_started`, `reauth` | info | `client_id` and `method` for `reauth` (§7.2: the re-authentication began, ended) |
| `reauth_failed` | warn | `reason` |
| `session_resumed` | info | `client_id`, `resent` (§4.1: the flows that go out again) |
| `session_discarded` | warn | `client_id`, `discarded`, `reason` (§4.1) |
| `session_unknown` | warn | `client_id`, `reason` (§4.1: the broker resumed a session the daemon has no state of) |
| `resend_dropped` | warn | `client_id`, `packet_id`, `bytes`, `reason` (above the broker's Maximum Packet Size) |
| `session_write_failed` | warn | `client_id`, `reason` |
| `session_file_truncated`, `session_file_foreign` | warn | `path`, `reason`; `protocol_level` for `session_file_foreign` (§4.1) |
| `replay_file_truncated` | warn | `path`, `reason` (§6: the last record of the replay file is cut off) |
| `drain_incomplete` | warn | `unacknowledged` (§9.2) |
| `admin_bound` | info | `listen`, `metrics_path` (§3, §8.2) |
| `otlp_active` | info | `endpoint` (§8.3) |
| `running` | info | `admin` (the binary, once the daemon serves; empty without the admin endpoint) |
| `reload` | info | `applied` |
| `reload_refused` | warn | `reason` (the reload changes nothing) |
| `reload_needs_restart` | warn | `settings` (§9.2) |
| `shutdown`, `stopped` | info | — (the binary) |
| `start_failed` | error | `kind` (`config`, `broker`, `dds`, `tls`, `auth`, `io`), `reason`; the exit code of §2 follows, and the same text goes to stderr |
| `admin_bind_failed` | error | `listen`, `reason` |
| `signal_watcher_failed`, `otlp_failed` | error | `reason` |

`--help`, `--version` and `--check-config` print text and no log line.

### §8.2 Prometheus metrics

If `metrics.enabled: true` (or `--metrics`), on `metrics.path` of the admin
endpoint (§3):
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
zerodds_mqtt_bridge_dds_samples_dropped_total   counter{dds_topic}
zerodds_mqtt_bridge_errors_total                counter
```

`dds_topic` is a DDS topic of the configuration (`<tenant>/<topic>` for a
tenant's, §1 L6); it is empty for a PUBLISH of no topic (the catalog of
§5.2, the reports of §6) and for a PUBLISH of the broker that matches no
subscribed topic. `mqtt_qos` is the QoS of the PUBLISH. The series of a
configured topic are there from the start with 0. `connect_attempts_total`
counts each attempt to connect (§9.1, §9.3), `connect_successes_total` each
one the broker's CONNACK accepted. `publish_in_total` and `bytes_in_total`
count the PUBLISH packets of the broker and their payload octets per DDS
topic they are routed to, `publish_out_total` and `bytes_out_total` the
PUBLISH packets the daemon sends. `inflight_messages{direction="out"}` are
the daemon's QoS 1/2 PUBLISH packets the broker has not completed,
`{direction="in"}` the broker's QoS 2 PUBLISH packets whose PUBREL has not
come, summed over the connections. `dds_samples_received_total` counts the
samples of the DataReaders, `dds_samples_published_total` the samples the
DataWriters wrote into DDS. `acl_denials_total` counts the refusals of
§7.3. `broker_disconnects_total` counts the connections that ended other
than by the shutdown: `reason_code` is the Reason Code of the DISCONNECT
that ended it (the broker's, or the daemon's after a protocol error) as
`0x8e`, or `network` when the socket closed or failed or the PINGRESP did
not come. `dds_samples_dropped_total` counts the samples a `keep_last`
history dropped or whose lifespan passed while they waited (§6),
`errors_total` the PUBLISH packets and subscriptions that failed or were not
sent, the samples DDS refused, codec and socket errors.

### §8.3 OTLP spans (optional)

If the `OTEL_EXPORTER_OTLP_ENDPOINT` env is set (`http://host:port`,
`host:port` or `host`; default port 4318), the daemon sends OTLP/HTTP/JSON
spans through `zerodds-observability-otlp` to `<endpoint>/v1/traces`: every
5 s, and once more when the shutdown of §9.2 is over; the log line
`otlp_active` (§8.1) names the endpoint. A collector that is not there does
not stop the daemon. Without the variable no span is made. Every PUBLISH
roundtrip is one trace:

| Span | Kind | From – to | Attributes |
|------|------|-----------|------------|
| `mqtt.publish` (root, a PUBLISH of the daemon) | client | the sample taken from the DataReader (for a PUBLISH of the replay buffer of §6 after a CONNACK: the replay) – the end of the roundtrip: the socket write under QoS 0, the broker's PUBACK under QoS 1, its PUBCOMP under QoS 2 | `mqtt.topic`, `mqtt.qos`, `mqtt.message.bytes` (the payload octets), `dds.topic`, `mqtt.packet_id` (QoS 1 and 2) |
| `mqtt.publish` (root, a PUBLISH of the broker) | server | reading the PUBLISH – the end of its handling | `mqtt.topic`, `mqtt.qos`, `mqtt.message.bytes` |
| `dds.write` (child of the broker's `mqtt.publish`) | client | the DataWriter's write of the sample, dispose or unregister – its return; one per DDS topic the PUBLISH is routed to | `dds.topic` |

A PUBLISH of the daemon the broker refused (a PUBACK or PUBREC with a
Reason Code of 0x80 or above), or whose connection ended before the
acknowledgement, has status error with the reason. A PUBLISH of the broker
that matches no subscribed topic or that the ACL refuses (§7.3) is the
root alone with status error; a `dds.write` DDS refused has status error,
and so has its root. A `register` writes nothing (§4.4) and has no
`dds.write`. A PUBLISH of the daemon has no `dds.write`: on that path the
daemon writes nothing into DDS (the sample was written by a DataWriter of
another participant, the daemon's DataReader took it). The daemon's own
PUBLISH packets of no DDS topic (the catalog of §5.2, the reports of §6)
are no span. A PUBLISH or PUBREL a resumed session sends again after
Session Present 1 (§4.1) is no span of its own: the roundtrip of that
PUBLISH ended with status error when its connection ended.

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation (fail-fast on faulty YAML, and on auth
   settings §7.2 does not allow: exit 1).
2. Install SIGHUP/SIGTERM/SIGINT handlers: a SIGTERM or SIGINT while the
   daemon connects ends it with exit 0.
3. TLS cert load if enabled.
4. DCPS DomainParticipant init on `domain`.
5. Per topic: register reader+writer (according to `direction`: `in` = broker → DDS, one DataWriter; `out` = DDS → broker, one DataReader; `bidir` = both).
6. MQTT client connect with reconnect backoff: a broker that cannot be
   reached (TCP, no answer in time, CONNACK 0x88 Server unavailable, 0x89
   Server busy, 0x97 Quota exceeded, 0x9F Connection rate exceeded; MQTT
   3.1.1 return code 3; a TLS handshake without an answer in time or ended
   by the broker closing the connection) is tried again after the waits of
   `mqtt.reconnect` until `mqtt.connect_timeout_secs`, the last attempt at
   that time, then exit 2; a TLS handshake that the broker or the daemon
   refuses (exit 4), a refusal of the credentials (exit 5) and every other
   CONNACK refusal (exit 2) end the start at once.
7. Per `direction=in|bidir` topic: SUBSCRIBE.

Steps 4 to 7 run for the top level and then for each tenant (§1 L6); a
tenant that cannot start ends the start of the daemon.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain, at most `mqtt.drain_timeout_secs` (default
30; a value too large for the clock of the system waits without end):
- Stop accepting new DDS samples.
- Clear the retained catalog (§5.2).
- Drain pending MQTT PUBLISH acks (QoS 1/2).
- Send DISCONNECT with reason code 0x00.
- Cleanup DDS entities.
- Exit 0.

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart). The ACL applies at once with the subject of the new
settings (§7.3), the TLS settings to the next connection (§7.1); the
credentials of §7.2 apply to the next connection when `auth.mode` and
`auth.enhanced.method` stay, and when they changed, a connection with
enhanced authentication re-authenticates with them (§7.2); a reload that
keeps them (the ACL or `auth.bearer_subject` changed, for example) sends no
AUTH. A change of `auth.mode` or `auth.enhanced.method` needs a restart: the
credentials and the subject of before stay in force. What needs a restart
is logged.

### §9.3 Reconnect

Broker disconnect → exponential backoff (`reconnect.initial_delay_ms`
up to `reconnect.max_delay_ms`). The session state (subscriptions,
inflight queue) is restored if `clean_start=false`. Across a restart of the
daemon the client side of such a session comes from `mqtt.session_dir`
(§4.1).

When the connection is lost (a DISCONNECT of the broker, a closed or failed
socket, a missing PINGRESP, a packet of the broker that breaks the protocol
or does not decode, also one read while a QoS 1/2 PUBLISH waits for the
broker's Receive Maximum; that PUBLISH waits for the next connection) the
daemon connects again with the TLS settings
and the credentials in force (§7.1, §7.2), without an end: the first attempt
at once, each next one after the waits of `mqtt.reconnect`
(`initial_delay_ms`, then `factor` times the last wait, at most
`max_delay_ms`). A broker that cannot be reached, a refusal of the TLS
handshake or of the credentials and a failed subscription are tried again
after the next wait. A connection the broker ended with DISCONNECT 0x8E
Session taken over is tried again only after `max_delay_ms`: two daemons
under one client identifier would otherwise take the session from each
other at once, again and again. Each failed attempt is logged as
`connect_failed` with `connect_state` `reconnecting` and the wait in
`next_attempt_ms` (§8.1). Meanwhile the samples of the DDS side wait by the
history of their topic (§6) and `/healthz` answers 503. After the CONNACK
the daemon subscribes again (§4.3), publishes its catalog (§5.2) and its
replay buffer (§6).

## §10 Cross-vendor

The daemon is a normal RTPS peer on the DDS side — Cyclone/RTI/
Fast DDS subscribers receive samples that an MQTT client publishes,
and vice versa. On the MQTT side the daemon is a normal
MQTT 5 client — tested against Mosquitto, EMQX, HiveMQ — and an MQTT
3.1.1 client (§1 L1) of a broker that speaks only 3.1.1, such as Aedes.

Verified in `crates/mqtt-bridge/tests/cross_vendor.rs`: the clients of
Mosquitto (`mosquitto_pub`, `mosquitto_sub`) on Mosquitto 2.0.21, EMQX
5.8.6, HiveMQ 4.57.0 and Aedes 1.0.2, and on the DDS side Cyclone DDS
0.10.5 (XCDR1 and XCDR2), Fast DDS 3.1 and RTI Connext DDS 7.7.0 (XCDR2,
`data_representation: xcdr2`, §3). Per broker: both directions at QoS 0, 1
and 2, retained messages both ways, the properties of §4.2 (not under
3.1.1: Payload Format Indicator, the Message Expiry Interval of the
writer's Lifespan, Content Type and the user properties), the session across a lost connection with the will (§6, §9.3) and
the retained catalog (§5.2). Two brokers need settings: Aedes 1.0.2
refuses a CONNECT of protocol level 5, so it needs
`mqtt.protocol_version: 3.1.1`; HiveMQ 4.57.0 refuses the topics beginning
with `$` to its clients, so it needs `mqtt.own_topic_prefix` without `$`
(§3). Further Mosquitto tests: `crates/mqtt-bridge/tests/mosquitto_qos.rs`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-mqtt-bridged` (statically linked recommended)
- Config default: `/etc/zerodds/mqtt-bridged.yaml` (Linux), `/usr/local/etc/zerodds/` (Mac), `%PROGRAMDATA%\ZeroDDS\` (Win)
- Systemd unit: `zerodds-mqtt-bridged.service`
- launchd plist: `org.zerodds.mqtt-bridged.plist`
- Win service: `ZeroDDSMQTTBridge`. Started by the service control
  manager, the binary runs as this service (`zerodds_dcps::cli::run_as_service`):
  `SERVICE_START_PENDING` until it serves (`READY=1`), then
  `SERVICE_RUNNING`; the stop and shutdown controls are SIGTERM (the drain
  of §9.2), `paramchange` is SIGHUP (the reload of §9.2); when the daemon
  ends, the service is `SERVICE_STOPPED` with the exit code of §2. The
  service has no stdout: its log (§8.1) is appended to
  `%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log` (`C:\ProgramData` when
  the variable is not set), the directory made when it is missing; a file
  that cannot be opened ends the start with exit code 1. Started from a
  console, Ctrl+C, Ctrl+Break and the closing of the console are SIGTERM,
  and the log stays on stdout.
- Docker: `zerodds/mqtt-bridged:1.0`

Manual: `man 1 zerodds-mqtt-bridged` + `man 5 zerodds-mqtt-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module ≥ 5 tests in `crates/mqtt-bridge/`: the configuration
(`src/daemon/config.rs`), the MQTT codec (`src/codec.rs`), the topic map
(`src/daemon/topic_map.rs`), the QoS translation
(`src/daemon/qos_translation.rs`), the DDS pump (`src/daemon/pump.rs`) and
the ACL (`src/daemon/acl.rs`).

### §12.2 Integration tests

`tests/integration/tests/bridges.rs` (`mqtt_bridge_against_mosquitto`, the
job `integration-bridges`):
- Spawn `zerodds-mqtt-bridged` as a subprocess.
- Mosquitto as a local process (`mosquitto`, `mosquitto_sub`,
  `mosquitto_pub`).
- Publish on an MQTT topic, the DDS subscriber receives.
- Publish on a DDS topic, the MQTT subscriber receives.
- Verify a byte-exact roundtrip.

`crates/mqtt-bridge/tests/bridge_e2e.rs` runs the binary and the daemon in
the test process against the crate's own broker server and brokers of the
test (flags, exit codes, the JSON log, the reconnect);
`crates/mqtt-bridge/tests/daemon_e2e.rs` checks both directions of the
daemon in the test process, a DDS reader of the test receiving what an MQTT
client publishes.

### §12.3 Multi-vendor

`crates/mqtt-bridge/tests/cross_vendor.rs` (feature `cross-vendor-tests`,
the tests `#[ignore]`d, run by the weekly job `integration-bridges-vendor`):
a Cyclone DDS participant (`tests/interop/cyclone_reading.c`, built by the
test with `idlc` and the C compiler), the ZeroDDS MQTT bridge as a pump
between it and the MQTT clients of Mosquitto, under XCDR1 and XCDR2.

Broker matrix: Mosquitto 2.x as a process of the test; EMQX 5.x, HiveMQ
4.x and Aedes (the broker of aedes-cli on Node.js) as running brokers whose
MQTT listener the variables `ZERODDS_EMQX_MATRIX`, `ZERODDS_HIVEMQ` and
`ZERODDS_AEDES` name (the job's services). A missing broker or program
fails a test of the job.

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
