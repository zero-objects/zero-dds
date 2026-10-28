# `zerodds-ws-bridge` v1.0 — DDS↔WebSocket bridge daemon

ZeroDDS vendor spec. Specifies the behaviour of a configurable
daemon that couples DDS topics with WebSocket clients.

## Motivation

Browser apps and IoT devices cannot take part natively in RTPS/UDP multicast
discovery (no UDP in the browser, MTU/routing restrictions
behind NAT). The `zerodds-ws-bridged` daemon is the bridge: it is
a DDS DomainParticipant *and* a WS server at the same time — everything that
is published on a topic goes out to all subscribed WS
clients and vice versa.

Complementary to the library `crates/websocket-bridge/` (RFC 6455 codec +
DDS bridge logic) — the library is the building block, this daemon is
the executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks RFC 6455 (base framing, handshake, close) + RFC 7692 (permessage-deflate optional). |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional: WS→DDS publish + DDS→WS push for all mapped topics. |
| **L4 — Config** | Topic map via YAML/JSON config; hot reload via SIGHUP optional. |
| **L5 — Auth** | TLS (`wss://`) + token auth (Bearer/JWT) + per-topic ACL. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per WS connection. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

```
zerodds-ws-bridged [OPTIONS]

Options:
  --config <FILE>          base file (YAML subset of zerodds-config),
                           plus FILE.d/; without it: ws-bridged.yaml and
                           ws-bridged.d/ in the system directory
  --set <PATH=VALUE>       one value above all files, repeatable
  --check-config           check the configuration, print every value with
                           its origin, exit (0 valid, 1 invalid)
  --listen <ADDR>          bind address (default 0.0.0.0:8080)
  --domain <ID>            DDS domain ID (default 0)
  --topic <NAME[:KEY]>     single-topic override (repeatable)
  --tls-cert <FILE>        TLS cert (PEM); activates wss://
  --tls-key <FILE>         TLS key (PEM)
  --auth-token <SECRET>    bearer token auth (single-token-mode)
  --log-level <LEVEL>      trace/debug/info/warn/error (default info)
  --metrics <ADDR>         Prometheus scrape endpoint (default off)
  --version                version info
  --help                   help

Exit-Codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   bind error (port in use)
  3   DDS discovery error
  4   TLS error
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--listen`, `--domain`, `--topic`, `--tls-cert`, `--tls-key`,
`--auth-token`, `--log-level` and `--metrics` are short forms for `--set`
of their file key (`--listen X` = `--set listen=X`, `--topic N` appends
`N` to `topics`). The same file may carry the runtime keys of the
DDS runtime (`transport.*`, `discovery.*`, …), see
`zerodds-deployment-1.0.md` §2.1.

## §3 Config file format

YAML subset of `zerodds-config` (block lists, flat inline lists of
scalars, `include:`, drop-ins, `${VAR}`). An unknown key in
a known section (`tls.cert_fiel`) and a value of the wrong type
prevent the start, naming file and line; an unknown key at the
top level produces a warning, and under `--check-config` an
error.

```yaml
# zerodds-ws-bridged.yaml
listen: "0.0.0.0:8080"
domain: 0
log_level: info

tls:
  enabled: false
  cert_file: "/etc/zerodds/cert.pem"
  key_file: "/etc/zerodds/key.pem"

auth:
  mode: "none"                 # none|bearer|jwt|mtls
  bearer_token: "${TOKEN}"     # ENV substitution
  jwt:
    public_key: "/etc/zerodds/jwt-pub.pem"
    audience: "zerodds-bridge"
    required_claim: "scope=dds.read"

topics:
  - name: "Chat::Message"
    type: "Chat::Message"
    direction: "bidir"         # in|out|bidir
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
    ws_path: "/topics/chat"    # Optional, default = /topics/<topic-slug>
    acl:
      read: ["*"]              # list of the permitted auth subjects
      write: ["alice","bob"]

  - name: "Sensor::Reading"
    type: "Sensor::Reading"
    direction: "out"
    ws_path: "/topics/sensor"

metrics:
  enabled: true
  listen: "127.0.0.1:9091"
  path: "/metrics"
```

ENV substitution: `${VAR}` and `${VAR:-default}` are resolved before the config parse.

## §4 WebSocket wire protocol

### §4.1 Handshake

Standard RFC 6455 upgrade handshake. The daemon answers with
`Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0` if the client sends this
subprotocol header (optional).

Auth header (bearer mode): `Authorization: Bearer <token>` → 401 on
failed auth, 403 on missing scope.

### §4.2 Path routing

For each topic config entry a WS endpoint is created:
- `/topics/<slug>` — default path (slug = topic name lowercased + `_`-replaced).
- `ws_path` override in the config.
- `/topics/__catalog__` — meta endpoint, returns a JSON list of the available topics + schemas.
- `/healthz` — HTTP 200 if the DCPS runtime is running.
- `/metrics` — Prometheus format if activated.

### §4.3 Frame format

Every WS frame transports one sample:

**Binary frames** (default):
```
+--------+--------+--------------------+--------------------+
| Magic  | Flags  | Encap-Header (4)   | CDR-Payload (...)  |
| "ZDB1" | 1 byte | XCDR2-LE etc.      | Spec §3 conformance |
+--------+--------+--------------------+--------------------+
```
- Magic `0x5A 0x44 0x42 0x31` = "ZDB1" (ZeroDDS Binary v1)
- Flags: bit0 = `dispose`, bit1 = `coherent_set_member`, bit2-7 reserved
- Encap header: `[0x00, 0x07, 0x00, 0x00]` (XCDR2-LE) per `zerodds-xcdr2-bindings-conformance-1.0` §3
- CDR payload: according to the topic type, byte-exact as over RTPS

**Text frames** (optional, if the client requests `Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0+json`):
```json
{
  "topic": "Chat::Message",
  "op": "publish",        // publish|dispose|register|unregister
  "key_hash": "a1d0c6e8...",
  "data": { /* JSON representation of the sample */ },
  "timestamp_ns": 1730000000000000000
}
```

JSON mode reduces throughput by ~3-5x — only for debug/browser dev.

### §4.4 Control messages

Client → daemon:
```json
{ "op": "subscribe", "topics": ["Chat::Message", "Sensor::Reading"] }
{ "op": "unsubscribe", "topics": ["Chat::Message"] }
{ "op": "ping" }
```

Daemon → client:
```json
{ "op": "subscribed", "topics": [...] }
{ "op": "error", "code": 403, "message": "scope dds.write required for Chat::Message" }
{ "op": "pong" }
```

Standard RFC 6455 PING/PONG frames are supported in addition.

## §5 Topic mapping

### §5.1 Slug algorithm

Topic name `Chat::Message` → URL slug:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Other non-`[a-z0-9/_-]` → `_`
4. Result: `/topics/chat/message`

Override via `ws_path` in the config.

### §5.2 Type discovery

On connection setup, the daemon optionally sends a catalog frame:
```json
{
  "op": "catalog",
  "topics": [
    {
      "name": "Chat::Message",
      "type": "Chat::Message",
      "ws_path": "/topics/chat/message",
      "qos": { "reliability": "reliable", ... },
      "schema_url": "/schema/chat/message.idl"
    }
  ]
}
```

`/schema/<slug>` returns the IDL definition (or TypeObject XML) for
codegen on the client side.

## §6 QoS translation

WebSocket is connection-oriented (TCP) — the DDS QoS policies
map as follows:

| DDS QoS | WS behaviour |
|---------|--------------|
| Reliability `RELIABLE` | TCP guarantees suffice; the daemon drops no frames for a slow client (backpressure via WS mask) |
| Reliability `BEST_EFFORT` | the daemon drops frames if the send queue is full (`max_send_queue_per_connection` config) |
| Durability `VOLATILE` | live samples only |
| Durability `TRANSIENT_LOCAL` | the daemon keeps the last N samples + sends a burst on connect (`max_burst_samples` config) |
| History `KEEP_LAST(N)` | analogous |
| Deadline | observed, sends `{ "op": "deadline_missed", "topic": "..." }` |
| Liveliness | heartbeat via WS PING / topic liveliness mapped to connection liveness |
| Partition | partition filter in the daemon before the WS write |

## §7 Security

### §7.1 TLS

`wss://` mode activated via `tls.enabled: true`. Cert rotation via
SIGHUP (the daemon re-reads cert/key, connections keep their session).

### §7.2 Auth modes

- `none`: no auth (dev mode, only binding to 127.0.0.1 allowed)
- `bearer`: HTTP header `Authorization: Bearer <token>` against the
  `bearer_token` config value
- `jwt`: JWT token validation (RS256/ES256), claims against the per-topic
  `acl`
- `mtls`: client certificate auth, subject DN as identity

### §7.3 ACL

Per topic in the config:
```yaml
acl:
  read: ["alice", "bob", "*group:engineers*"]
  write: ["alice"]
```

Rules:
- `*` = all
- `<name>` = exact match
- `*group:<n>*` = JWT claim `groups` contains `<n>`

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout (per `--log-level`). Fields:
`timestamp`, `level`, `event`, `connection_id`, `topic`, `bytes`,
`peer`, `latency_us`.

### §8.2 Prometheus metrics

If `metrics.enabled: true`:
```
zerodds_ws_bridge_connections_total      counter
zerodds_ws_bridge_connections_active     gauge
zerodds_ws_bridge_frames_in_total        counter{topic, direction}
zerodds_ws_bridge_bytes_in_total         counter{topic, direction}
zerodds_ws_bridge_frames_out_total       counter{topic, direction}
zerodds_ws_bridge_bytes_out_total        counter{topic, direction}
zerodds_ws_bridge_send_queue_drops_total counter{connection_id}
zerodds_ws_bridge_dds_samples_received   counter{topic}
zerodds_ws_bridge_dds_samples_published  counter{topic}
zerodds_ws_bridge_auth_failures_total    counter{reason}
```

### §8.3 OTLP spans (optional)

If the `OTEL_EXPORTER_OTLP_ENDPOINT` env is set: the daemon emits
`zerodds-observability-otlp` spans per connection lifecycle and per
frame.

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation (fail-fast on faulty YAML).
2. TLS cert load if activated.
3. DCPS DomainParticipant init on `domain`.
4. Per topic: register reader+writer (according to `direction`: `in` = WebSocket clients → DDS, one DataWriter; `out` = DDS → WebSocket clients, one DataReader; `bidir` = both). The daemon answers a `publish` on a topic without a DataWriter and a `subscribe` on one without a DataReader with an error frame (`code` 404, `reason` `not-bridged-into-dds` or `not-bridged-out-of-dds`).
5. WS server bind + listen.
6. SIGHUP/SIGTERM/SIGINT handler.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s, configurable):
- Stop accepting new WS-Connections.
- Drain pending DDS-samples (publish queue).
- Send `{ "op": "shutdown" }` to all WS-Connections.
- Close WS-Connections with code 1001 (going away).
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart).

## §10 Cross-vendor

The daemon is a normal RTPS peer — Cyclone/RTI/Fast-DDS subscribers
receive the samples that a WS client publishes, and vice versa.
Verified in `crates/websocket-bridge/tests/cross_vendor.rs`.

## §11 Packaging

According to the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-ws-bridged` (statically linked recommended)
- Config default: `/etc/zerodds/ws-bridged.yaml` (Linux), `/usr/local/etc/zerodds/` (Mac), `%PROGRAMDATA%\ZeroDDS\` (Win)
- Systemd unit: `zerodds-ws-bridged.service`
- launchd plist: `org.zerodds.ws-bridged.plist`
- Win service: `ZeroDDSWSBridge`
- Docker: `zerodds/ws-bridged:1.0`

Manual: `man 1 zerodds-ws-bridged` + `man 5 zerodds-ws-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `topic_map`, `auth`, `frame_codec`,
`dds_pump`) ≥ 5 tests in `crates/websocket-bridge/src/bin/`.

### §12.2 Integration tests

`crates/websocket-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-ws-bridged` as a subprocess.
- WS client connects, publishes.
- DDS subscriber in the test process receives.
- Verify the byte-exact roundtrip.

### §12.3 Multi-vendor

`tests/cross_vendor.rs` (cargo-feature-gated): Cyclone subscriber in
Docker Compose, the ZeroDDS WS bridge publishes, the Cyclone sub prints.

## §13 Cross-references

- Library: `crates/websocket-bridge/`
- Codec spec: `zerodds-ws-bridge`-specific extensions to RFC 6455.
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-mqtt-bridge-1.0`, `zerodds-coap-bridge-1.0`,
  `zerodds-amqp-bridge-daemon-1.0`, `zerodds-grpc-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch updates for bug fixes, minor for additive config
fields, major for wire protocol changes.
