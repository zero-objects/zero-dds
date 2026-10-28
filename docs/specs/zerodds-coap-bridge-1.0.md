# `zerodds-coap-bridge` v1.0 — DDS↔CoAP bridge daemon

ZeroDDS vendor spec. Specifies the behavior of a configurable
daemon that couples DDS topics with CoAP clients (RFC 7252 + RFC 7641 Observe).

## Motivation

CoAP (RFC 7252) is the protocol standard for constrained IoT devices:
6LoWPAN sensors, battery-powered wearables, LoRa gateways. The
standard is UDP-based, compact (4-byte header), and supports
Observe patterns (RFC 7641) for asynchronous sample streams.

The `zerodds-coap-bridged` daemon couples DDS topics with CoAP
resource URIs: every DDS sample is pushed as a CoAP response to an
Observe subscription; CoAP POST/PUT requests are interpreted as
DDS writes. Block-wise transfer (RFC 7959) allows
large samples; DTLS-PSK + cert auth provides the edge security.

Complementary to the library `crates/coap-bridge/` (CoAP codec +
DTLS stack + DDS bridge logic) — the library is the building block,
this daemon is the executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks CoAP (RFC 7252) including Block1/Block2 (RFC 7959), Observe (RFC 7641), No-Response option (RFC 7967). |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional: CoAP→DDS publish (POST/PUT) + DDS→CoAP push (Observe Notify). |
| **L4 — Config** | URI map via YAML config; hot reload via SIGHUP optional. |
| **L5 — Auth** | DTLS-PSK + DTLS-Cert (RFC 7252 §9), OSCORE (RFC 8613) optional. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per DTLS session. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

```
zerodds-coap-bridged [OPTIONS]

Options:
  --config <FILE>              Base file (YAML subset of zerodds-config),
                               plus FILE.d/; without it: coap-bridged.yaml and coap-bridged.d/
                               in the system directory
  --set <PATH=VALUE>           one value above all files, repeatable
  --check-config               check the configuration, print every value with its
                               origin, exit (0 valid, 1 invalid)
  --bind <ADDR>             UDP bind address (default 0.0.0.0:5683 / DTLS 5684)
  --domain <ID>             DDS domain ID (default 0)
  --dtls-psk-id <ID>        DTLS PSK identity (single-PSK mode)
  --dtls-psk <SECRET>       DTLS PSK secret (hex or Base64)
  --dtls-cert <FILE>        DTLS server cert (PEM); enables coaps://
  --dtls-key <FILE>         DTLS server key (PEM)
  --topic <DDS:URI>         single-topic override (allowed several times)
  --log-level <LEVEL>       trace/debug/info/warn/error (default info)
  --metrics <ADDR>          Prometheus scrape endpoint (default off)
  --version                 version info
  --help                    help

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   bind error (port in use)
  3   DDS discovery error
  4   security setup error (DTLS/OSCORE)
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--bind`, `--domain`, `--dtls-cert`/`--dtls-key`, `--dtls-psk-id`/`--dtls-psk`, `--topic <DDS[:URI]>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
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
# zerodds-coap-bridged.yaml
domain: 0
log_level: info

coap:
  bind: "0.0.0.0:5683"
  bind_dtls: "0.0.0.0:5684"
  max_message_size: 1152          # default RFC-7252; raise for jumbo MTU
  ack_timeout_ms: 2000
  max_retransmit: 4
  block_size: 1024                # Block1/Block2 SZX (16/32/64/128/256/512/1024)
  observe_max_age_secs: 60

  dtls:
    enabled: true
    cert_file: "/etc/zerodds/coap-cert.pem"
    key_file:  "/etc/zerodds/coap-key.pem"
    ca_file:   "/etc/zerodds/coap-ca.pem"
    psk:
      - identity: "device-001"
        secret_hex: "deadbeefcafebabe..."
      - identity: "device-002"
        secret_hex: "feedface1234..."

oscore:
  enabled: false
  master_secret_hex: "0102030405060708090a0b0c0d0e0f10"
  master_salt_hex:   "9e7ca92223786340"
  id_context_hex:    ""

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    coap_uri_path: "chat/message"
    direction: "bidir"
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    coap_uri_path: "sensors/reading"
    direction: "out"
    qos:
      reliability: "best_effort"

  - dds_name: "Actuator::Command"
    dds_type: "Actuator::Command"
    coap_uri_path: "actuator/command"
    direction: "in"
    qos:
      reliability: "reliable"
      durability: "transient_local"

content_format:
  cdr2_le_id: 65000                # Vendor-Range registration (IANA-Registry)
  cdr2_be_id: 65001
  json_id: 50                      # standard application/json

acl:
  default_deny: true
  rules:
    - subject: "device-001"
      allow_post: ["chat/+", "sensors/+"]
      allow_observe: ["chat/+"]

metrics:
  enabled: true
  listen: "127.0.0.1:9093"
  path: "/metrics"
```

ENV substitution: `${VAR}` and `${VAR:-default}` are resolved before the config
parse.

## §4 CoAP wire protocol

### §4.1 Frame format

Standard RFC 7252 header (4 bytes) + token + options + payload:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|Ver| T |  TKL  |      Code     |          Message ID           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Token (if any, TKL bytes) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Options (if any) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|1 1 1 1 1 1 1 1|    Payload (CDR bytes) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### §4.2 Publish (CoAP→DDS)

`POST coap[s]://<host>:5683/<coap_uri_path>`:
- Options:
  - `Uri-Path`: one per segment of the URI path
  - `Content-Format`: 65000 (XCDR2-LE) or 65001 (XCDR2-BE) or 50 (JSON)
  - `If-Match` / `If-None-Match` (optional, for the instance lifecycle)
  - `No-Response` (RFC 7967) optional for FIRE-AND-FORGET
- Payload: CDR bytes (or JSON if Content-Format=50)

Daemon response:
- `2.04 Changed` for an accepted POST → DDS write done
- `4.00 Bad Request` on a decode error
- `4.13 Request Entity Too Large` if without Block1
- `5.00 Internal Server Error` on a DDS write error

`PUT` for idempotent sample updates (e.g. retain-style with instance key).

`DELETE` maps to DDS dispose.

### §4.3 Subscribe (DDS→CoAP, Observe pattern)

`GET coap[s]://<host>:5683/<coap_uri_path>` with the `Observe: 0` option:
- The daemon registers the client in the notification list for the topic.
- Initial response: current sample (or `2.05 Content` empty for VOLATILE).
- On every new DDS sample: notify with `Observe: <seq>`, token re-used.
- `2.05 Content` with `Content-Format: 65000` + CDR payload.

Cancel: `Observe: 1` option in the GET, or RST reset on the token, or
Observe timeout `observe_max_age_secs`.

### §4.4 Block-wise transfer

Samples > `block_size` are split into blocks automatically:
- Outbound (Notify): `Block2(NUM, M, SZX)` option
- Inbound (POST): `Block1(NUM, M, SZX)` option, the daemon assembles before the DDS write

Defragmentation cap: configurable via `coap.max_message_size`
(default 64KB). Larger samples are rejected with `4.13` and
must be sent via a chunked-stream topic.

### §4.5 Content format registry

| ID | Media type | Use |
|----|------------|------------|
| 65000 | `application/x-dds-cdr2; bo=le` | XCDR2 little-endian |
| 65001 | `application/x-dds-cdr2; bo=be` | XCDR2 big-endian |
| 65002 | `application/x-dds-cdr1; bo=le` | XCDR1 little-endian (legacy) |
| 50 | `application/json` | JSON representation (debug) |
| 60 | `application/cbor` | CBOR representation (optional, future) |

IANA registration in the vendor range 65000-65535.

## §5 Topic mapping

### §5.1 Slug algorithm

Topic name `Chat::Message` → default CoAP URI path:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Other non-`[a-z0-9/_-]` → `_`
4. Result: `chat/message`

Override via the `coap_uri_path` field in the config.

### §5.2 Type discovery

`GET /.well-known/core` (RFC 6690) returns the resource catalog:
```
</chat/message>;rt="dds.topic";ct=65000;type="Chat::Message",
</sensors/reading>;rt="dds.topic";ct=65000;type="Sensor::Reading",
</actuator/command>;rt="dds.topic";ct=65000;type="Actuator::Command"
```

`GET /schema/<slug>` returns the IDL definition for codegen on the
client side.

## §6 QoS translation

| DDS QoS | CoAP behavior |
|---------|----------------|
| Reliability `RELIABLE` | CoAP `CON` messages (Confirmable) with retransmit |
| Reliability `BEST_EFFORT` | CoAP `NON` messages (Non-Confirmable) |
| Durability `VOLATILE` | live notify only; no replay on Observe re-register |
| Durability `TRANSIENT_LOCAL` | initial notify with the last sample |
| History `KEEP_LAST(N)` | analogous (Observe re-register sends the last N) |
| Lifespan | `Max-Age` option in the notify |
| Deadline | observed, the daemon emits `5.03 Service Unavailable` with `Max-Age=0` |
| Liveliness | CoAP ping (`0.00 EMPTY` as Confirmable heartbeat every `keep_alive_secs`) |
| Partition | filter in the daemon before the notify push |

## §7 Security

### §7.1 DTLS

`coaps://` mode is enabled via `coap.dtls.enabled: true`. Recommended
cipher suites:
- `TLS_PSK_WITH_AES_128_CCM_8` (PSK mode, RFC 7252 §9.1.3)
- `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` (cert mode)
- `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` (hybrid)

Cert rotation via SIGHUP (the daemon re-reads cert/key, sessions are kept).

### §7.2 OSCORE (RFC 8613)

OSCORE mode for end-to-end protection across CoAP proxies:
```yaml
oscore:
  enabled: true
  master_secret_hex: "0102..."
  master_salt_hex:   "9e7c..."
  id_context_hex:    ""
  sender_id_hex:     "01"   # sender ID of the daemon (default 01)
  replay_window:     32     # anti-replay window width (default 32)
```

The daemon is the OSCORE server: for each client `kid` a sender/
recipient context is derived via HKDF (RFC 8613 §3.2; `sender_id_hex`
is the sender ID of the daemon, clients configure it as their recipient
ID). Replay window per `kid`: 32 (default, §3.2.2).

Behavior with `enabled: true`:
- Requests with the OSCORE option are decrypted and verified
  (§8.2); replays are answered with `4.01`, verification errors with `4.00`.
- Requests without the OSCORE option are rejected with `4.01 Unauthorized`;
  only `/.well-known/core` remains reachable
  unencrypted.
- Responses are protected (§8.3, without Partial IV); Observe
  notifications carry the daemon's own Partial IV
  (§4.1.3.5.2).
- A build without the `oscore` feature refuses to start (fail-closed,
  exit 4).

### §7.3 ACL

Per topic in the config; subject = DTLS PSK identity or cert subject DN.

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout. Fields: `timestamp`, `level`,
`event`, `peer`, `coap_uri`, `dds_topic`, `mid`, `token_hex`,
`block_num`, `bytes`, `latency_us`.

### §8.2 Prometheus metrics

If `metrics.enabled: true`:
```
zerodds_coap_bridge_requests_total          counter{method, code}
zerodds_coap_bridge_observers_active        gauge{topic}
zerodds_coap_bridge_notifies_sent_total     counter{topic}
zerodds_coap_bridge_blocks_in_total         counter{topic}
zerodds_coap_bridge_blocks_out_total        counter{topic}
zerodds_coap_bridge_dtls_handshakes_total   counter{result}
zerodds_coap_bridge_oscore_replays_total    counter
zerodds_coap_bridge_dds_samples_received    counter{topic}
zerodds_coap_bridge_dds_samples_published   counter{topic}
zerodds_coap_bridge_acl_denials_total       counter{reason}
```

### §8.3 OTLP spans

`OTEL_EXPORTER_OTLP_ENDPOINT` set: the daemon emits spans per
CoAP exchange (`coap.request` + `dds.write` as child).

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation.
2. DTLS cert/PSK load if enabled.
3. DCPS DomainParticipant init on `domain`.
4. Per topic: register reader+writer (according to `direction`: `in` = CoAP clients (POST/PUT or GET/Observe) → DDS, one DataWriter; `out` = DDS → CoAP clients (POST/PUT or GET/Observe), one DataReader; `bidir` = both).
5. UDP bind 5683 (CoAP) + 5684 (CoAPs) if DTLS.
6. Install SIGHUP/SIGTERM/SIGINT handlers.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Stop accepting new CoAP requests.
- To every observer, a notification `5.03 Service Unavailable` without the
  Observe option with its token: RFC 7641 §4.2 ends an observation with
  a notification whose code is not 2.xx. (A `2.05` with
  `Observe: 1` would be an ordinary notification and would end nothing.)
- Drain pending DDS writes.
- Cleanup DDS entities.
- Exit 0.

SIGHUP → config reload (cert + ACL hot update; topic map changes
need a restart).

## §10 Cross-vendor

The daemon is a normal RTPS peer on the DDS side. The CoAP side is tested
against libcoap, californium, aiocoap, Eclipse Wakaama (the LwM2M stack
uses the CoAP layer).

Verified in `crates/coap-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-coap-bridged`
- Config default: `/etc/zerodds/coap-bridged.yaml`
- Systemd unit: `zerodds-coap-bridged.service`
- launchd plist: `org.zerodds.coap-bridged.plist`
- Win service: `ZeroDDSCoAPBridge`
- Docker: `zerodds/coap-bridged:1.0`

Manual: `man 1 zerodds-coap-bridged` + `man 5 zerodds-coap-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `coap_codec`, `block_assembler`, `observe_table`,
`dtls`, `oscore`, `dds_pump`) ≥ 5 tests in `crates/coap-bridge/`.

### §12.2 Integration tests

`crates/coap-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-coap-bridged` as a subprocess.
- libcoap test client as CoAP peer.
- POST → DDS subscriber receives.
- Observe → DDS publisher emit → CoAP notify.
- Block1/Block2 with a 4KB payload.

### §12.3 Multi-vendor

`tests/cross_vendor.rs`: Cyclone DDS subscriber + libcoap/californium
client against the ZeroDDS CoAP bridge in Docker Compose.

## §13 Cross-references

- Library: `crates/coap-bridge/`
- RFC 7252 (CoAP), RFC 7641 (Observe), RFC 7959 (Block), RFC 8613 (OSCORE).
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-mqtt-bridge-1.0`, `zerodds-ws-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch updates for bug fixes, minor for additive config
fields (e.g. new content format IDs), major for wire protocol
changes.
