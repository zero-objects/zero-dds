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

L6: each entry of `tenants` (§3) is a DomainParticipant of its own on its
`domain`, with its `topics`, their DataReaders, DataWriters, send queues and
catalog. A WebSocket connection whose authenticated subject (§7.3: the
bearer subject, the JWT `sub`, the client certificate's subject DN or its
`mtls:` fingerprint, `anonymous` under `auth.mode: none`) a tenant's
`subjects` names is bound to that tenant for its whole life: its topic
paths (§4.2), `subscribe` and sample frames (§4.3, §4.4), its catalog and
`/schema/<slug>` (§5.2) are the tenant's, and only them. A connection no
tenant names uses the participant of the top-level `domain` and `topics`. A
subject is bound to one tenant at most. The ACL of a tenant's topic (§7.3)
is its own, also when a top-level topic or another tenant's has its name.
The `topic` label of a tenant's metrics (§8.2) is `<tenant>/<name>`; the
log line `connection_open` names the tenant (§8.1), the span
`ws.connection` carries `ws.tenant` (§8.3). The admin endpoint's `/catalog`
(§4.2) lists the top-level topics.

## §2 CLI surface

```
zerodds-ws-bridged [OPTIONS]

Options:
  --config <FILE>          base file (YAML subset of zerodds-config),
                           plus FILE.d/; without it: ws-bridged.yaml and
                           ws-bridged.d/ in the system directory
  --set <PATH=VALUE>       one value above all files, repeatable
  --check-config           check the configuration, print every value with
                           its origin, exit (0 valid, 1 invalid, 2 a file
                           that does not load)
  --listen <ADDR>          bind address (default 127.0.0.1:8080)
  --domain <ID>            DDS domain ID (default 0)
  --topic <NAME[:WS_PATH]> one more topic entry (repeatable); WS_PATH is the
                           path of its endpoint (§4.2), default /topics/<slug>
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
  3   DDS error (the participant does not start)
  4   TLS error (certificate or key does not load)
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--listen`, `--domain`, `--topic`, `--tls-cert`, `--tls-key`,
`--auth-token`, `--log-level` and `--metrics` are short forms for `--set`
of their file key (`--listen X` = `--set listen=X`, `--topic N` appends
`N` to `topics`). `--topic NAME:WS_PATH` is the entry `{name: NAME,
ws_path: WS_PATH}`: the single `:` that is not part of a `::` of a scoped
name separates the path, which begins with `/`; `--topic Chat::Message:/chat`
serves `Chat::Message` at `/chat`. The same form is a bare entry of
`topics` in the file. `--version` prints `zerodds-ws-bridged` and the
version of the crate. The same file may carry the runtime keys of the
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
listen: "127.0.0.1:8080"       # default; `auth.mode: none` only on loopback (§7.2)
domain: 0
log_level: info

tls:
  enabled: false
  cert_file: "/etc/zerodds/cert.pem"
  key_file: "/etc/zerodds/key.pem"
  client_ca_file: "/etc/zerodds/clients-ca.pem"   # client certificates (mTLS)

auth:
  mode: "none"                 # none|bearer|jwt|mtls
  bearer_token: "${TOKEN}"     # ENV substitution
  bearer_subject: "service-account"   # the subject of the bearer token (§7.3)
  jwt:
    public_key: "/etc/zerodds/jwt-pub.pem"
    issuer: "https://idp.example.org"
    audience: "zerodds-bridge"
    required_claim: "scope=dds.read"

topics:
  - name: "Chat::Message"
    type: "Chat::Message"
    idl_file: "/etc/zerodds/chat.idl"  # declares `type`; needed for the JSON text frames (§4.3)
    direction: "bidir"         # in|out|bidir
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
      deadline_ms: 1000        # §6: deadline_missed after 1 s without a sample
      liveliness_lease_ms: 10000   # §6: AUTOMATIC liveliness of the DDS endpoints
      partition: ["chat"]      # §6: the partitions of the DDS endpoints
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
  path: "/metrics"             # default /metrics

limits:                        # RFC 6455 §10.4, §4.5
  max_frame_size: 16777216     # octets; default 16 MiB, at least 125
  max_message_size: 16777216   # octets; default 16 MiB, at least 1

origin:                        # RFC 6455 §10.2, §4.1
  allowed: ["https://app.example.org"]   # default: empty, no check
  required: false              # true: a handshake without Origin gets 403

max_send_queue_per_connection: 1024   # §6: samples waiting per connection
max_burst_samples: 1000        # §6: samples a durable topic keeps for late subscribers
ping_interval_ms: 30000        # §6: the daemon's WebSocket Ping; 0 = off
shutdown_drain_ms: 30000       # §9.2: the bound of the drain of a shutdown
catalog_on_connect: false      # §5.2: the catalog frame first on every connection

tenants:                       # §1 L6: participants beside the one of `domain`
  - name: "plant-a"
    domain: 7                  # default: the top-level `domain`
    subjects: ["alice", "CN=gw-7,O=Example"]   # authenticated subjects bound to it
    topics:                    # as the top-level `topics`
      - name: "Plant::Status"
        direction: "out"
        acl:
          read: ["alice"]
```

ENV substitution: `${VAR}` and `${VAR:-default}` are resolved before the config parse.

`limits.max_frame_size` is the largest payload of one frame of a client,
`limits.max_message_size` the largest message after reassembly and
decompression, both in octets (§4.5). A frame limit below 125, the largest
control frame (RFC 6455 §5.5), and a message limit of 0 prevent the start.

`origin.allowed` lists the origins the daemon expects in the `Origin` field
of the opening handshake (§4.1). An entry is an origin in its ASCII
serialization (RFC 6454 §6.2: `scheme://host[:port]`, no path; a host with
characters outside ASCII in its `xn--` form), `null`, or `*` for every
origin. A host is a name of letters, digits, `-`, `.` and `_`, an IPv4
address, or an IPv6 address in brackets. `*` is an entry of its own and no
pattern: an entry with `*` inside a host (`https://*.example.org`) is no
origin, each origin is named. An entry of any other form prevents the start,
and so does `origin.required: true` without an entry.

`metrics.path` is the path of the Prometheus metrics on the admin endpoint
at `metrics.listen` (§4.2, §8.2); it begins with `/` and is not `/`,
`/catalog` or `/healthz`, the admin endpoint's own paths.

`qos.history.kind` of a topic is `keep_last` (the default) or `keep_all`;
`qos.history.depth` (default 10, at least 1) is the N of `keep_last` (§6).

`ws_path` of a topic is the path of its endpoint (§4.2) and replaces the
default path `/topics/<slug>` (§5.1). It is an absolute path without
whitespace, `?` or `#`, it is not `/topics/__catalog__` or `/healthz`, and no
two topics share one; a `ws_path` that breaks one of these prevents the start,
naming file and line.

`max_send_queue_per_connection` (default 1024, at least 1) and
`max_burst_samples` (default 1000, at least 1) count samples;
`ping_interval_ms` is 0 or at least 100 (default 30000).
`shutdown_drain_ms` (default 30000) is milliseconds from 1 to 2147483647. `qos.deadline_ms`
and `qos.liveliness_lease_ms` of a topic are milliseconds from 1 to
2147483647; `qos.partition` is a list of partition names (DDS 1.4 §2.2.3.13,
with its wildcards). §6 says what they do.

`catalog_on_connect` (default false) sends the catalog of §5.2 as the first
text frame of every WebSocket connection (§4.2 sends it on
`/topics/__catalog__` alone).

`tenants` (§1 L6) lists participants beside the one of `domain`. An entry
has `name` (unique, without `/`), `domain` (default: the top-level
`domain`), `subjects` (at least one; a subject in two tenants prevents the
start) and `topics` (the form of the top-level `topics`, with the same
checks of `ws_path` and of the schema path within the tenant). Another key
in an entry prevents the start, naming file and line. So does a top-level
topic named `<tenant>/<name>` after the topic `<name>` of a tenant: its
ACL and its metric label (§1 L6) would be those of the tenant's topic.
One topic has one `acl`: a second entry of a topic with an `acl` of its
own prevents the start as well.

A change of `limits`, `origin`, `metrics`, `catalog_on_connect`, `tenants`
(other than the `acl` of a tenant's topic) or of a key of §6 takes effect
with a restart (§9.2).

## §4 WebSocket wire protocol

### §4.1 Handshake

Standard RFC 6455 upgrade handshake. The daemon answers with
`Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0` if the client sends this
subprotocol header (optional). Each subprotocol name is a token (RFC 6455
§4.1, the `token` of RFC 7230 §3.2.6), so it carries no `/`.

Auth header (bearer and jwt mode): `Authorization: Bearer <token>` → 401 on
failed auth, 403 on missing scope. Credentials that authenticate no one (no
header, an unknown bearer token, a JWT whose signature, time, issuer or
audience does not hold) get `401 Unauthorized` with `WWW-Authenticate: Bearer
realm="zerodds-ws"`, and `error="invalid_token"` when a token came (RFC 6750
§3, §3.1). A JWT that verifies and lacks `auth.jwt.required_claim` gets `403
Forbidden` with `WWW-Authenticate: Bearer realm="zerodds-ws",
error="insufficient_scope", scope="<value>"`. The body names the status only;
the reason is in the log. The daemon closes the connection after either.

Origin (RFC 6455 §10.2, §4.2.2 /origin/): with a list in `origin.allowed`
(§3) the daemon checks the `Origin` header field of the request. An origin
outside the list is answered with `403 Forbidden` and the daemon closes the
connection; the response does not repeat the field. Origins are compared as
RFC 6454 §5 says: scheme, host and port, scheme and host without case, the
default port of `http`, `https`, `ws` and `wss` equal to no port. `null` is
expected only when the list names it; `*` in the list expects every origin.
A field with several origins (RFC 6454 §7.1), or a second `Origin` field,
passes only when every origin in it is expected. A request without `Origin`
is accepted, since a client outside a browser may leave the field out (RFC
6455 §4.1 item 8); with `origin.required: true` it gets 403 as well. The
check comes before the authentication. Without a list the daemon accepts
every origin.

A request that is no valid opening handshake is answered with its HTTP
status, and the daemon closes the connection (RFC 6455 §4.2.1, §4.2.2, §4.4):
405 with `Allow: GET` for a method other than GET, 426 with
`Sec-WebSocket-Version: 13` for a version other than 13, 400 for every other
invalid request (below HTTP/1.1, without `Host`, `Upgrade: websocket` or
`Connection: Upgrade`, a `Sec-WebSocket-Key` that is not 16 octets, octets
that are no UTF-8, a request head of more than 65536 octets). The request
head has 10 seconds after the TCP connection to arrive; a client that has
not sent it by then gets `408 Request Timeout` and the daemon closes the
connection (RFC 9110 §15.5.9).

`permessage-deflate` (RFC 7692): the daemon agrees to the first valid offer
in the client's `Sec-WebSocket-Extensions` and names the agreed parameters in
its response; the messages of the connection are then compressed in both
directions. An offer with an unknown, invalid or repeated parameter is
declined and the next one is looked at. With `server_max_window_bits=8` the
daemon agrees and sends its own messages uncompressed. Without a valid offer
the response names no extension and the connection is not compressed.

### §4.2 Path routing

The WebSocket listener (`listen`) serves:
- `/topics/<slug>` — the endpoint of a topic without `ws_path` (slug per
  §5.1).
- `ws_path` — the endpoint of a topic with `ws_path` (§3); the topic is
  served at this path alone, not at its default path as well.
- `/topics/__catalog__` — meta endpoint, the catalog of §5.2 with the topics
  the client may read or write (§7.3): over WebSocket as the first text frame
  of the connection, which stays open for control messages (§4.4); to a plain
  HTTP GET as the body of a `200` response with `Content-Type:
  application/json`. Origin (§4.1) and credentials are checked as for a
  topic path.
- `/healthz` — to a plain HTTP GET: `200` with `OK` while the DCPS runtime
  runs, `503` otherwise. It needs no credentials and answers whether or not
  the metrics endpoint is on.
- `/schema/<slug>` — to a plain HTTP GET: the type of a topic (§5.2).

A plain HTTP GET is a request without an `Upgrade` field; any other path of
one is answered as an invalid opening handshake (§4.1). An opening handshake
to a path that is no topic path and not the catalog opens a connection
without a topic; the client subscribes with control messages (§4.4).

With `metrics.enabled` or `metrics.listen` the admin endpoint at
`metrics.listen` serves the Prometheus metrics (§8.2) at `metrics.path`
(default `/metrics`), the catalog at `/catalog` and `/healthz`.

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
- Flags: bit0 = `dispose`, bit1 = `coherent_set_member`, bit2 = `unregister`, bit3-7 reserved.
  The daemon sets bit1 on a sample, a dispose or an unregister that its writer
  wrote inside a coherent set (DDS 1.4 §2.2.2.4.1.8; `PID_COHERENT_SET`,
  DDSI-RTPS 2.5 §8.7.6); the end of the set carries no data and is no frame
- Encap header: the encapsulation header the sample came under (DDSI-RTPS 2.5
  §10.5), e.g. `[0x00, 0x07, 0x00, 0x00]` (XCDR2-LE) per
  `zerodds-xcdr2-bindings-conformance-1.0` §3, `[0x00, 0x09, 0x00, 0x00]`
  (D_CDR2-LE) for an `@appendable` type
- CDR payload: according to the topic type, byte-exact as over RTPS. For a
  dispose or an unregister: the instance's key holder (DDS-XTypes 1.3 §7.6.8)
  under `[0x00, 0x06, 0x00, 0x00]` (PLAIN_CDR2 big endian), empty when the
  marker carried only the key hash
- A client sends samples in binary frames (flags 0); it disposes and
  unregisters with JSON sample frames. A binary frame from a client with any
  flag bit set (bit0, bit1, bit2 or a reserved bit) is answered with an error
  400 (§4.4) and writes nothing
- The daemon's DataWriter writes a client's sample under the header of its
  frame. The writer of a topic with `idl_file` offers XCDR2 alone, in the
  format of the type's extensibility (DDS-XTypes 1.3 §7.6.3.1.1, Table 60:
  PLAIN_CDR2, DELIMITED_CDR or PL_CDR2); a frame on such a topic may name
  either byte order of that format, any other header (XCDR1, another XCDR2
  format) is answered with an error 400 and writes nothing. The connections
  of a topic share its DataWriter; the samples of two clients that write at
  the same time each go out under their own header
- A binary message without the magic is answered with an error 400, whatever
  it holds (§4.4)

**Text frames** (optional, if the client requests `Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0+json`):
```json
{
  "topic": "Chat::Message",
  "op": "publish",        // publish|dispose|register|unregister
  "key_hash": "a1d0c6e8...",
  "data": { /* JSON representation of the sample */ },
  "timestamp_ns": 1730000000000000000
}
```

- `key_hash`: the instance's KeyHash (DDSI-RTPS 2.5 §9.6.4.8) as 32 lowercase
  hexadecimal digits; left out for a topic without key.
- `data`: the sample as JSON (the representation of `zerodds_wire_codec::json`:
  a struct is an object by member name, a sequence an array, an enumerator its
  name). The daemon needs the topic's type for it, from `idl_file` (§3): it
  decodes a DDS sample in the format its encapsulation header names, and
  encodes a client's `data` under XCDR2 little endian in the format of the
  type's extensibility (DDS-XTypes 1.3 §7.4.2: PLAIN_CDR2, DELIMITED_CDR or
  PL_CDR2). Present for `publish`; left out for `dispose` and `unregister`
  from the daemon.
- `timestamp_ns`: the source time in nanoseconds since the Unix epoch; left
  out when the sample carried none. A client's value is the source time of the
  sample it writes.
- From the daemon: `publish` per sample, `dispose` and `unregister` per marker
  (both for a marker that is both). From a client: `publish` writes `data`;
  `dispose` and `unregister` name the instance by `key_hash` or by the key
  members in `data`; `register` has no message on the wire (DDS 1.4
  §2.2.2.4.2.5) and writes nothing.
- A JSON-mode client of a topic without `idl_file`, on its path or by
  `subscribe`, gets an error 415 and no samples of it.

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
{ "op": "shutdown" }
```

`shutdown` is the last message of a daemon that shuts down; the Close frame
with 1001 follows (§9.2). `subscribed` names the topics of the request the connection now receives; a
topic it does not receive gets an error. The control messages are text frames
in both modes. Error codes: 400 a message of another form, 403 the ACL (§7.3)
does not grant it, 404 a topic not bridged in that direction (`direction`),
415 a topic without `idl_file` in JSON mode, 422 a sample that is no sample
of the topic's type, 500 the DDS write failed (a JSON sample frame and a
binary frame alike). `subscribed` names each topic once.

Standard RFC 6455 PING/PONG frames are supported in addition.

### §4.5 Invalid data and closing

The daemon validates every frame of a client (RFC 6455 §10.7). Invalid data
ends the connection with a Close frame of its status code (RFC 6455 §7.4.1),
and nothing behind it is processed:

- 1002: a frame that is not masked, a reserved bit that no agreed extension
  defines, a reserved opcode, a payload length that is not in its shortest
  form, a control frame that is fragmented or longer than 125 octets, a
  continuation frame without a message, a new data frame inside a fragmented
  message, a Close frame with a body of one octet or a status code that must
  not be sent.
- 1007: a text message that is not UTF-8, as soon as the octet that makes it
  malformed has arrived; a Close reason that is not UTF-8.
- 1009: a frame of more than `limits.max_frame_size` octets or a message
  (after reassembly and decompression) of more than
  `limits.max_message_size` octets (§3; 16 MiB each by default); a frame is
  refused when its header has arrived.

A fragmented message is reassembled and handled as one message. A Close frame
of the client is answered with a Close frame of the same status code.

Behind every Close frame it sends (the answer to a client's Close, the Close
for invalid data, the Close 1001 of the shutdown, §9.2) the daemon ends its
sending direction, over TLS with `close_notify` before the TCP FIN, and reads
until the client closes the connection, at most 500 ms. What it reads in that
time is not processed.

## §5 Topic mapping

### §5.1 Slug algorithm

Topic name `Chat::Message` → URL slug:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Other non-`[a-z0-9/_-]` → `_`
4. Result: `/topics/chat/message`

Override via `ws_path` in the config.

### §5.2 Type discovery

On connection setup, the daemon optionally sends a catalog frame: on
`/topics/__catalog__` always (§4.2), on every connection with
`catalog_on_connect: true` (§3). It is the first text frame of the
connection and names the topics the client may read or write (§7.3):
```json
{
  "op": "catalog",
  "service": "zerodds-ws-bridged",
  "version": "<the version of the daemon>",
  "topics": [
    {
      "name": "Chat::Message",
      "type": "Chat::Message",
      "direction": "bidir",
      "ws_path": "/topics/chat/message",
      "qos": {
        "reliability": "reliable",
        "durability": "volatile",
        "history": { "kind": "keep_last", "depth": 10 }
      },
      "schema_url": "/schema/chat/message.idl"
    }
  ]
}
```

`schema_url` is present for a topic with `idl_file` (§3). The same JSON is
the body of a plain HTTP GET of `/topics/__catalog__` and of `/catalog` on
the admin endpoint (which lists every topic).

`/schema/<slug>` returns the IDL definition (or TypeObject XML) for
codegen on the client side. `<slug>` is the slug of §5.1 of the topic's
name, the default path without `/topics/` (`Chat::Message` →
`chat/message`), whatever `ws_path` the topic has; two topics with
`idl_file` whose names have one slug prevent the start. To a plain HTTP GET
on the WebSocket listener:
- `/schema/<slug>` and `/schema/<slug>.idl`: `200` with `Content-Type:
  text/plain; charset=utf-8` and the text of the topic's `idl_file`, each
  `#include` line replaced by the text of the file it names (a `"file"` from
  the directory of the including file, then from the directory of
  `idl_file`; a `<file>` from that directory; an include of a file that is
  not there stays), so that the document compiles on its own.
- `/schema/<slug>.xml`: `200` with `Content-Type: application/xml;
  charset=utf-8` and the XML type representation of the type (DDS-XTypes
  1.3 §7.3.2, the schema of its Annex A): `<types>` with each type the
  topic's type uses, before its first use, in the `<module>`s of its scope;
  a member of a `@mutable` struct carries its `id`. A type that needs a
  type of its own in that representation (a bitset, a sequence or map of a
  collection other than an array of sequences, a map key that is no
  primitive or string) gets `501` naming the reason and the IDL form.
- `404` for a slug of no topic and for a topic without `idl_file`.

Origin (§4.1) and credentials (§7.2) are checked as for a topic path; a
topic the ACL lets the client neither read nor write (§7.3) gets `403`. A
method other than GET is answered as an invalid opening handshake (405).

## §6 QoS translation

WebSocket is connection-oriented (TCP) — the DDS QoS policies of a topic
(§3 `qos`) map as follows. The daemon's DataReader and DataWriter of the
topic get the same reliability, durability, deadline, liveliness and
partition.

| DDS QoS | WS behaviour |
|---------|--------------|
| Reliability `RELIABLE` | the daemon drops no sample for a connected client: when the connection's send queue holds `max_send_queue_per_connection` samples, the topic waits until the client has taken one. While it waits, the samples the daemon's DataReader receives wait in the daemon, in the hand-off from the DDS runtime to the topic, which has no bound of its own; the DDS writer gets no backpressure. A client that takes no frame for `ping_interval_ms` is closed (Liveliness below), so a topic waits at most that long for one client, and the samples that wait meanwhile are its sample rate times `ping_interval_ms`; with `ping_interval_ms: 0` the wait has no bound |
| Reliability `BEST_EFFORT` | when the send queue is full, the daemon drops the oldest queued sample of a `BEST_EFFORT` topic for the new one; a queue that holds samples of `RELIABLE` topics only keeps them and drops the new sample. Each drop is counted in the send-queue drop counter (§8.2) |
| Durability `VOLATILE` | live samples only |
| Durability `TRANSIENT_LOCAL`, `TRANSIENT`, `PERSISTENT` | the daemon keeps the samples of the history below, at most `max_burst_samples`, and sends them to a connection when it subscribes (by its path or by `subscribe`), oldest first, before any live sample; behind `subscribed` (§4.4) |
| History `KEEP_LAST(N)` | the last N samples of each instance are kept (`qos.history.depth`) |
| History `KEEP_ALL` | every sample is kept, at most `max_burst_samples` |
| Deadline (`qos.deadline_ms`) | from its first sample on, an instance that gets no sample for the period is a text frame `{ "op": "deadline_missed", "topic": "..." }` (with `"key_hash"` for a topic with key, §4.3) to the topic's subscribers that the read ACL (§7.3) grants the topic, once per missed period; a dispose or an unregister ends the instance's deadline |
| Liveliness (`qos.liveliness_lease_ms`) | the lease of the AUTOMATIC liveliness of the daemon's DataWriter and DataReader; the participant asserts it while the daemon runs. The liveliness of a connection is its answer to the daemon's WebSocket Ping (RFC 6455 §5.5.2), sent every `ping_interval_ms`: a connection that has not answered a Ping with a Pong when the next is due gets a Close 1011 and is closed (§4.5). A connection that takes no octet of a frame for `ping_interval_ms` cannot get the Ping either; the daemon closes it without a Close frame |
| Partition (`qos.partition`) | the partitions of the daemon's DataReader and DataWriter: a sample reaches the clients only from a writer of a matching partition (DDS 1.4 §2.2.3.13); the clients' samples go out in these partitions |

The send queue holds the samples of one connection; a burst, the control
messages of §4.4 and the frames of this section are not counted. A
connection holds at most one burst of each topic: a `subscribe` while the
previous burst of the topic still waits replaces it. At most 1024 control
messages (`subscribed`, `deadline_missed`) wait in the queue; a client that
lets more pile up, by subscribing again and again without reading, gets a
Close 1008 and is closed (§4.5), and what waits is not sent.

The instances of KEEP_LAST(N) and of the deadline are told apart by the
KeyHash of each sample (DDSI-RTPS 2.5 §9.6.4.8), as the writer sends it in
line (`PID_KEY_HASH`). A topic without `idl_file` is a topic without key
for the daemon: samples without a KeyHash are one instance. On a topic
whose `idl_file` declares a key, a sample without a KeyHash names no
instance: it reaches the subscribers, but it is not kept for a later
subscriber and starts no deadline; the daemon counts it in its error
counter (§8.2) and logs the first one of each topic. The daemon does not
compute the KeyHash from the payload itself.

## §7 Security

### §7.1 TLS

`wss://` mode activated via `tls.enabled: true`. Cert rotation via
SIGHUP (the daemon re-reads cert/key, connections keep their session).

### §7.2 Auth modes

- `none`: no auth (dev mode). Served on a loopback `listen` address only
  (127.0.0.0/8, `::1`, or a name that resolves to loopback addresses
  alone, such as `localhost`); on any other address the daemon does not
  start (exit 1, §2), `--check-config` reports the configuration invalid
  (exit 1), and a SIGHUP reload to `none` on such an address is refused and
  changes nothing. `listen` waits for a restart (§9.2): a reload is checked
  against the address the daemon serves, whatever `listen` the file names
  now.
- `bearer`: HTTP header `Authorization: Bearer <token>` against the
  `bearer_token` config value; without a value, or with an empty one (an
  unset `${VAR}` expands to nothing), the daemon does not start (exit 1)
  and `--check-config` reports the configuration invalid
- `jwt`: JWT token validation, claims against the per-topic `acl`.
  `auth.jwt.public_key` names a PEM file: an RSA key (`RSA PUBLIC KEY` or
  `PUBLIC KEY`) verifies RS256 (RFC 7518 §3.3), a P-256 `PUBLIC KEY`
  (id-ecPublicKey, prime256v1) verifies ES256 (RFC 7518 §3.4, the
  signature `R || S` of 64 octets). The `alg` of a token has to be the
  algorithm of the key; any other `alg` is refused (401). An EC key on
  another curve does not load (exit 1).
- `mtls`: client certificate auth, subject DN as identity (RFC 4514, the
  last RDN of the encoding first, e.g. `O=Example,CN=alice`); needs
  `tls.client_ca_file`, and a client without a certificate of that CA gets
  no TLS session.

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

Structured JSON log on stdout: one JSON object per line, written when its
level reaches `log_level` (`--log-level`: `trace`, `debug`, `info`, `warn`,
`error`; default `info`). A SIGHUP reload applies a changed `log_level`.
Every line has `timestamp` (RFC 3339 §5.6 in UTC with milliseconds,
`1970-01-01T00:00:00.000Z`), `level` and `event`, then the fields of its
event: `connection_id` (the daemon's number of the connection, the label of
§8.2), `topic` (`""` for a message of no topic, as in §8.2), `bytes` (the
payload octets of the message before compression), `peer` (the client's
address; for `bound`, `admin_bound` and `running` the address the daemon
serves), `latency_us`, and the others the table names. Numbers are JSON
numbers.

| Event | Level | Fields |
|-------|-------|--------|
| `start` | info | `listen`, `domain`, `topics` (their number) |
| `origin_check` | info | `allowed`, `required` (§4.1) |
| `tls_active` | info | `cert_file`, `mtls` |
| `auth` | info | `mode`, `acl_entries` |
| `bound` | info | `peer`: the WebSocket listener |
| `admin_bound` | info | `peer`, `metrics_path`: the admin endpoint (§4.2) |
| `running` | info | `peer` (the binary, once the daemon serves) |
| `connection_open` | info | `connection_id`, `peer`, `path`, `topic` (of the path, §4.2), `subject` (§7.3), `mode` (`json` or `binary`, §4.3), `tenant` (§1 L6; `""` for the participant of `domain`) |
| `connection_close` | info | `connection_id`, `peer`, `latency_us`: the time since `connection_open` |
| `subscribe`, `unsubscribe` | debug | `connection_id`, `peer`, `topic` (§4.4) |
| `frame_in` | debug | `connection_id`, `peer`, `topic`, `bytes`, `latency_us`: from the reassembled message to the return of its DDS write |
| `frame_out` | debug | `connection_id`, `peer`, `topic`, `bytes`, `latency_us`: from taking the message from the send queue to the end of its socket write |
| `frame_refused` | warn | the fields of `frame_in` and `reason`: a client message the daemon answered with an error frame (§4.4) |
| `handshake_reject` | warn | `connection_id`, `peer`, `code` (the HTTP status, §4.1), `reason` |
| `origin_reject` | warn | `connection_id`, `peer`, `code`, `origin` |
| `auth_reject` | warn | `connection_id`, `peer`, `reason` (§4.1: the response names the status only) |
| `acl_reject` | warn | `connection_id`, `peer`, `topic`, `subject` (the 403 of a topic path) |
| `schema` | debug; warn when refused (403, 404, 501) | `connection_id`, `peer`, `path`, `code` (the HTTP status of a `/schema/<slug>` request, §5.2) |
| `tls_handshake_failed` | warn | `connection_id`, `peer`, `reason` |
| `connection_failed` | warn | `connection_id`, `peer`, `code` (the status code of the Close frame, §4.5), `reason` |
| `send_queue_overflow` | warn | `connection_id`, `peer`, `queued_controls` (the bound of §6; the Close 1008 follows) |
| `sample_without_key_hash` | warn | `topic`, `reason` (the first of each topic, §6) |
| `reload` | info | `applied` |
| `reload_refused` | warn | `reason` (the reload changes nothing) |
| `reload_needs_restart` | warn | `settings` (§9.2) |
| `shutdown` | info | — |
| `shutdown_drain_bound` | warn | `drain_ms`: connections still open when `shutdown_drain_ms` passed |
| `shutdown_connections_left` | warn | `wait_ms`: connections still open after the last wait (§9.2) |
| `stopped` | info | — (the binary) |
| `start_failed` | error | `kind` (`config`, `tls`, `bind`, `dds`, `io`), `reason`; the exit code of §2 follows, and the same text goes to stderr |
| `otlp_active` | info | — (§8.3) |
| `accept_failed`, `signal_watcher_failed`, `otlp_failed` | error | `reason` |
| `admin_bind_failed` | error | `peer`, `reason` |

`--help`, `--version` and `--check-config` print text and no log line.

### §8.2 Prometheus metrics

If `metrics.enabled: true`, at `metrics.listen` under `metrics.path`:
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
zerodds_ws_bridge_errors_total           counter
```

- `frames_in_total` and `frames_out_total` count WebSocket messages (text
  and binary, after reassembly), `bytes_in_total` and `bytes_out_total`
  their payload octets before compression; Ping, Pong and Close frames are
  not counted. `topic` is the DDS topic whose sample (or
  `deadline_missed`) the message carries: in, the topic of a sample frame
  of §4.3 (the topic of the path for a binary frame); out, the topic of
  the sample. `direction` is that topic's `direction` in the configuration
  (`in`, `out`, `bidir`). A message of no topic (a control message of §4.4,
  the catalog, an error frame) is counted with `topic=""` and
  `direction=""`, and so is a sample frame whose `topic` is no configured
  topic: a client cannot add series of its own.
- `connections_total` counts the accepted TCP connections, also one whose
  TLS handshake then fails; `connections_active` the open ones.
- `send_queue_drops_total` counts the samples a BEST_EFFORT topic dropped
  on a full send queue (§6), under the daemon's number of the connection.
- `dds_samples_received` counts the samples the DataReader of a topic
  received, `dds_samples_published` the samples, disposes and unregisters
  a client wrote into DDS (a `register` writes nothing, §4.3).
- `auth_failures_total` counts refusals by `reason`: `missing` (no
  credentials, 401), `rejected` (credentials that authenticate no one,
  401), `scope` (a verified token without `auth.jwt.required_claim`, 403),
  `acl` (a read or write the ACL does not grant: the 403 of a topic path,
  the error frame 403 of a `subscribe` or a sample frame).
- `errors_total` counts frame, codec and socket errors, failed TLS
  handshakes, refused handshakes (§4.1, origin), the `404` and `501` of
  `/schema` (§5.2), the error frames of §4.4
  the daemon answers a client message with (each once), and the samples of
  a topic with key without a key hash (§6). A refusal of the
  authentication or of the ACL is counted in `auth_failures_total` only.

### §8.3 OTLP spans (optional)

If the `OTEL_EXPORTER_OTLP_ENDPOINT` env is set (`http://host:port`,
`host:port` or `host`; default port 4318), the daemon sends OTLP/HTTP/JSON
spans through `zerodds-observability-otlp` to `<endpoint>/v1/traces`: every
5 s, and once more when the shutdown of §9.2 is over. A collector that is not
there does not stop the daemon. Every WebSocket connection is one trace:

| Span | Kind | From – to | Attributes |
|------|------|-----------|------------|
| `ws.connection` (root) | server | the start of the opening handshake – the end of the connection | `net.peer`, `ws.connection_id` (the `connection_id` of §8.1), `ws.path`, `http.status_code` (`101`), `auth.subject`, `ws.tenant` (§1 L6, for a connection bound to a tenant), `ws.mode` (`json` or `binary`), `ws.subprotocol`, `ws.extensions` (the `Sec-WebSocket-Extensions` of the response), `dds.topic` (the topic of the path, §4.2) |
| `ws.frame.in` (child of the root) | server | a client's message reassembled – its handling done | `dds.topic` (§8.2: the topic of a sample frame), `ws.message.type` (`text` or `binary`), `ws.message.bytes` (the payload octets before compression) |
| `dds.write` (child of `ws.frame.in`) | client | the DataWriter's write of the sample, dispose or unregister – its return | `dds.topic` |
| `ws.frame.out` (child of the root) | client | a message of the daemon taken from the send queue (or built) – its socket write done | `dds.topic`, `ws.message.type`, `ws.message.bytes` |

A message is a text or binary message (a client's message after reassembly);
Ping, Pong and Close frames are no span. A message the daemon refused with
an error frame (§4.4) is a `ws.frame.in` with status error and the reason as
its message; a `register` writes nothing (§4.3) and has no `dds.write`. A
connection that failed (§4.5, the Ping of §6, a write the client does not
take within `ping_interval_ms`, an error of its socket) has status error
with the reason; a message not sent because the daemon's Close frame went
out before is no failure. An opening handshake the daemon refused (§4.1: the HTTP status of
the error, the origin's 403, the 401 or 403 of the authentication or of the
topic ACL) is a `ws.connection` alone, with status error and that status in
`http.status_code`. The plain HTTP requests of the WebSocket listener
(`/healthz`, `/topics/__catalog__`, `/schema/<slug>`, §4.2, §5.2) are no
span.

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation (fail-fast on faulty YAML).
2. TLS cert load if activated.
3. DCPS DomainParticipant init on `domain`.
4. Per topic: register reader+writer (according to `direction`: `in` = WebSocket clients → DDS, one DataWriter; `out` = DDS → WebSocket clients, one DataReader; `bidir` = both). The daemon answers a `publish` on a topic without a DataWriter and a `subscribe` on one without a DataReader with an error frame (§4.4, `code` 404, the `message` naming the topic and the direction).
5. WS server bind + listen.
6. SIGHUP/SIGTERM/SIGINT handler.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain, bounded by `shutdown_drain_ms` (default
30 s):
- Stop accepting new WS-Connections.
- Drain pending DDS-samples: the samples the DataReaders received before
  the signal are handed to the send queues of the subscribed connections,
  and each connection sends what its queue holds.
- Send `{ "op": "shutdown" }` to all WS-Connections, behind those samples.
- Close WS-Connections with code 1001 (going away).
- Cleanup DDS-Entities.
- Exit 0.

Every open WS-Connection gets the text message `{"op":"shutdown"}`, then
the Close frame with 1001 and the reason `going away`, and is closed as §4.5
says. A connection whose HTTP upgrade has not finished is no WS-Connection
yet; its TCP connection is closed without a Close frame. The drain ends when
the connections have ended or when `shutdown_drain_ms` has passed; a
RELIABLE topic whose client takes no frame holds the drain until then. At
that point a connection still open gets the Close frame with 1001 without
the samples it did not take, and the daemon waits for such connections at
most 2 s more before it cleans up and exits.

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart, and so do changes of `listen`, `domain`, `tenants` (other than the `acl` of a tenant's topic), the TLS file paths,
`limits`, `origin`, `metrics`, `max_send_queue_per_connection`,
`max_burst_samples`, `ping_interval_ms`, `shutdown_drain_ms` and `catalog_on_connect`; the daemon logs which changed
settings wait for a restart; a changed `log_level` applies at once).

## §10 Cross-vendor

The daemon is a normal RTPS peer: a Cyclone DDS, Fast DDS or RTI Connext
DDS subscriber receives the samples that a WS client publishes, and a
sample their writer writes reaches the WS clients, as JSON and as a binary
frame. Verified in `crates/websocket-bridge/tests/cross_vendor.rs` (§12.3).

The DataWriter of a topic with `idl_file` offers XCDR2 alone (DDS-XTypes
1.3 §7.6.3.1.1) and sends the JSON samples of the clients under the header
of the type's extensibility, little endian (§4.3). A binary frame names its
own header, and the writer offers what that header names.

## §11 Packaging

According to the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-ws-bridged` (statically linked recommended)
- Config default: `/etc/zerodds/ws-bridged.yaml` (Linux), `/usr/local/etc/zerodds/` (Mac), `%PROGRAMDATA%\ZeroDDS\` (Win)
- Systemd unit: `zerodds-ws-bridged.service`
- launchd plist: `org.zerodds.ws-bridged.plist`
- Win service: `ZeroDDSWSBridge`. Started by the service control manager,
  the binary runs as this service: `SERVICE_START_PENDING` until it serves
  (the moment of `running` in §8.1), then `SERVICE_RUNNING`; the controls
  stop and shutdown are the SIGTERM of §9.2 (`SERVICE_STOP_PENDING`, the
  drain), `paramchange` (`sc.exe control ZeroDDSWSBridge paramchange`) is
  the SIGHUP reload; at the end `SERVICE_STOPPED` with the exit code of
  §2 (`ERROR_SERVICE_SPECIFIC_ERROR` and the code for any code but 0).
  Started from a console, Ctrl+C, Ctrl+Break and the closing of the
  console are the SIGTERM.
- Docker: `zerodds/ws-bridged:1.0`

Manual: `man 1 zerodds-ws-bridged` + `man 5 zerodds-ws-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

At least 5 unit tests per module, in its `#[cfg(test)] mod tests`: the
configuration (`src/daemon/config.rs`), the topic map and the send queues
(`src/daemon/router.rs`), the authentication and the ACL
(`src/daemon/security.rs`), the frame codec (`src/codec.rs`) and the DDS
pump, the mapping between a sample of a DataReader and the frames of §4.3
(`src/daemon/pump.rs`). The paths are relative to
`crates/websocket-bridge/`.

### §12.2 Integration tests

`crates/websocket-bridge/tests/daemon_e2e.rs`:
- Spawn `zerodds-ws-bridged` as a subprocess.
- WS client connects, publishes.
- DDS subscriber in the test process receives.
- Verify the byte-exact roundtrip: the CDR bytes and the encapsulation
  header of the client's binary frame reach the DDS reader, and a sample of
  a DDS writer of the test process reaches the client as the same bytes
  under the same header.

### §12.3 Multi-vendor

`tests/cross_vendor.rs` (cargo feature `cross-vendor-tests`, the weekly
job `integration-bridges-vendor`): the daemon in the test process against a
foreign DDS participant on the same domain, in both directions. A JSON
`publish` of a WebSocket client is taken by the foreign reader, which
prints the sample; a sample the foreign writer writes reaches a JSON-mode
and a binary-mode client; the binary frame, sent back by the client,
reaches the foreign reader again. The participants: a Cyclone DDS program
(`tests/interop/cyclone_reading.c`, built with `idlc` and the C compiler),
a Fast DDS 3 program (`tests/interop/fastdds_reading.cpp`, built with the
C++ compiler), both on the topic type `Sensor::Reading`
(`tests/interop/reading.idl`); and the `shape_main` of RTI Connext DDS
7.7.0 that the OMG `dds-rtps` suite releases, on `Square` of the suite's
`ShapeType` (`interop/shapes/omg-dds-rtps.idl`, XCDR2).

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
