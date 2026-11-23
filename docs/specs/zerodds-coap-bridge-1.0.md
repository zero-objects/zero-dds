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

L6: each entry of `tenants` (§3) is a DomainParticipant of its own on its
`domain`, with its `topics`, its observations and its catalog. A DTLS
session whose subject (§7.1: `psk:<identity>`, the certificate's subject
DN or its `mtls:` fingerprint, `rpk:<ni URI>`) a tenant's `subjects` names
is bound to that tenant for its whole life: its requests address the
tenant's resources, and only them. Plaintext requests, multicast requests
and a DTLS session no tenant names address the participant of the
top-level `domain` and `topics`. A subject is bound to one tenant at most.
The `topic` label of a tenant's metrics (§8.2) is `<tenant>/<dds_name>`.

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
  max_message_size: 1152          # largest datagram, RFC 7252 §4.6; raise for jumbo MTU
  max_body_size: 65536            # largest body a Block1 transfer assembles
  ack_timeout_ms: 2000
  max_retransmit: 4
  block_size: 1024                # Block1/Block2 SZX (16/32/64/128/256/512/1024)
  observe_max_age_secs: 60        # longest an observer goes without confirming
  keep_alive_secs: 0              # the ping to each observer, §6; 0 = none
  multicast:                      # RFC 7252 §8, see §4.6
    enabled: false
    groups: ["224.0.1.187"]       # default: the all-CoAP-nodes groups
    interface_v4: "0.0.0.0"
    interface_v6: 0
    leisure_ms: 5000
    resources: [".well-known/core"]
    unauthenticated_resources: [".well-known/core"]

  dtls:
    enabled: true
    cert_file: "/etc/zerodds/coap-cert.pem"
    key_file:  "/etc/zerodds/coap-key.pem"
    ca_file:   "/etc/zerodds/coap-ca.pem"
    psk:
      - identity: "device-001"
        secret_hex: "deadbeefcafebabe0011223344556677"
      - identity: "device-002"
        secret_hex: "feedface123456789abcdef001234567"
    rpk_key_file: "/etc/zerodds/coap-rpk.pem"     # RFC 7250 raw public key
    rpk_trusted:                                  # RFC 6920 names of client keys
      - "ni:///sha-256-120;UyaQV-Ev4rdLoHyJJWCi"

proxy:                            # RFC 7252 §5.7, see §4.7
  forward: false                  # Proxy-Uri / Proxy-Scheme requests
  schemes: ["coap", "coaps"]
  names: ["bridge.example"]       # host names that name this daemon
  timeout_ms: 10000               # then 5.04 Gateway Timeout
  reverse:
    - path: "far"                 # resources under /far ...
      origin: "coap://[2001:db8::7]/base"   # ... are those under this URI
  cache_entries: 256
  require_dtls_for_coaps: true
  max_hops: 16                    # RFC 8768 initial Hop-Limit
  multicast_wait_ms: 5000
  dtls:                           # for coaps destinations
    ca_file: "/etc/zerodds/coap-origin-ca.pem"
    cert_file: "/etc/zerodds/coap-proxy-cert.pem"
    key_file: "/etc/zerodds/coap-proxy-key.pem"
    # or: psk_identity: "proxy", psk_secret_hex: "..."
    # or: rpk_key_file: "...", rpk_trusted: ["nih:sha-256-120;...;c"]

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
    idl_file: "/etc/zerodds/idl/chat.idl"   # the type: every form of §4.5, §5.2
    data_representation: "xcdr2"    # what the DDS writer sends; xcdr1 needs idl_file
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
      lifespan_ms: 30000            # Max-Age of a sample, §6
      deadline_ms: 5000             # 5.03 after 5 s without a sample, §6
      liveliness_lease_ms: 10000
      partition: ["site-a"]

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    coap_uri_path: "sensors/reading"
    direction: "out"
    extensibility: "final"          # without idl_file: the XCDR2 header, §4.5
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
  cdr1_le_id: 65002
  json_id: 50                      # standard application/json
  cbor_id: 60                      # standard application/cbor

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

tenants:                          # §1 L6: participants beside the one of `domain`
  - name: "plant-a"
    domain: 7
    subjects: ["psk:device-001"]  # DTLS subjects bound to it
    topics:
      - dds_name: "Plant::Status"
        coap_uri_path: "plant/status"
```

A topic's `idl_file` declares its `dds_type` (`#include`s are read from the
file's directory): with it the topic takes and gives every form of §4.5,
a body is checked against the type, and `GET /schema/<slug>` returns the
IDL (§5.2). A topic without `idl_file` passes its samples through as they
are; its writer sends them under the XCDR2 header of `extensibility`
(`final`, `appendable` — the default of DDS-XTypes 1.3 §7.3.1.2.1.8 — or
`mutable`). A topic's DDS writer offers and sends XCDR2; with
`data_representation: xcdr1` (which needs `idl_file`) it offers and sends
XCDR1, for DDS readers that accept only XCDR1 (DDS-XTypes 1.3 §7.6.3.1.2:
a reader matches a writer whose representation is in its list).

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

A message that is no request (RFC 7252 §4.2, §4.3): an empty Confirmable
message (a "CoAP ping") and any other Confirmable message the daemon has
no use for get a Reset, as does a Confirmable message that cannot be
decoded when its header can be read; an Acknowledgement or a Reset is not
answered (it ends the retransmission of a separate response of the proxy,
§4.7); a Non-confirmable one is ignored.

A request seen before from the same endpoint with the same Message ID
(RFC 7252 §4.5; for EXCHANGE_LIFETIME, NON_LIFETIME for a Non-confirmable
one, at most 4096 remembered) is not processed again: a Confirmable one
gets the acknowledgement of the first, a Non-confirmable one nothing.

A request to the daemon's own resources:

- with a critical option the daemon does not process (§5.4.1; a second
  occurrence of a critical option that is not repeatable, §5.4.5) is `4.02
  Bad Option` when Confirmable and reset when Non-confirmable;
- with another method than GET, POST, PUT, DELETE, or with another than GET
  on `/.well-known/core`, is `4.05 Method Not Allowed` (§5.8);
- with an Accept of no form of §4.5, or of a form the topic cannot give (a
  topic without `idl_file` gives only the form of its sample), or other
  than 40 on `/.well-known/core`, is `4.06 Not Acceptable` (§5.10.4);
- POST or PUT with a Content-Format of no form of §4.5, or other than
  65000 on a topic without `idl_file`, is `4.15 Unsupported
  Content-Format` (§5.10.3);
- POST, PUT or DELETE on a topic without a writer (`direction: out`), GET
  on one without a reader (`direction: in`), is `4.05`;
- POST or PUT with an If-Match none of whose values is the ETag of the
  current representation (an empty value: there is one), or with
  If-None-Match while there is one, is `4.12 Precondition Failed`
  (§5.10.8); a topic has a representation once it has a sample;
- GET answers with the ETag of the current representation (8 octets of a
  hash of the sample, §5.10.6.1), and with `2.03 Valid` when the request
  names that ETag (§5.10.6.2).

### §4.2 Publish (CoAP→DDS)

`POST coap[s]://<host>:5683/<coap_uri_path>`:
- Options:
  - `Uri-Path`: one per segment of the URI path
  - `Content-Format`: a form of §4.5; without it 65000
  - `If-Match` / `If-None-Match` (optional, for the instance lifecycle)
  - `No-Response` (RFC 7967) optional for FIRE-AND-FORGET
- Payload: the sample in that form, without an encapsulation header

With `idl_file` the body is read as a sample of the type in its form and
written as XCDR2 little endian in the encoding of the type's
extensibility, keyed by the KeyHash of its instance (DDSI-RTPS 2.5
§9.6.4.8) for a keyed type; without it the XCDR2 body is written as it is.

Daemon response:
- `2.04 Changed` for an accepted POST → DDS write done
- `4.00 Bad Request` for a body that is no sample of the type
- `4.13 Request Entity Too Large` with Size1 for a datagram larger than
  `coap.max_message_size`, or a Block1 transfer larger than
  `coap.max_body_size`
- `5.00 Internal Server Error` on a DDS write error

`PUT` for idempotent sample updates (e.g. retain-style with instance key).

`DELETE` maps to DDS dispose of the instance the request names: by
`Uri-Query: key_hash=<32 hexadecimal digits>`, or by the key members of a
sample in the body (in the form its Content-Format names; this needs
`idl_file`). `2.02 Deleted`; a type without key is `4.05`; a DELETE that
names no instance `4.00`.

No-Response (RFC 7967 §2.1): a response of a class the option's bits
suppress (2: 2.xx, 8: 4.xx, 16: 5.xx) does not leave; a Confirmable
request gets an empty Acknowledgement instead (RFC 7252 §4.2). The proxy
(§4.7) forwards the option and applies it to its own responses.

### §4.3 Subscribe (DDS→CoAP, Observe pattern)

`GET coap[s]://<host>:5683/<coap_uri_path>` with the `Observe: 0` option:
- The daemon registers the client in the notification list for the topic;
  a registration with the endpoint and token of one it has replaces it
  (RFC 7641 §4.1).
- Initial response: `2.05 Content`, empty for VOLATILE and before the
  first sample; otherwise the oldest of the samples §6 gives a new
  observer, the others follow as notifications.
- On every new DDS sample: notify with `Observe: <seq>` (24 bits), token
  re-used.
- `2.05 Content` in the form the registration's Accept named (§4.5; without
  Accept 65000 with `idl_file`, the sample's own form without), with
  ETag, and Max-Age from the lifespan (§6).
- A notification is Confirmable on a RELIABLE topic, and on any topic once
  the observer has not acknowledged one for `observe_max_age_secs`
  (RFC 7641 §4.5 sets 24 hours as the bound; 0 leaves that bound). It is
  retransmitted after `ack_timeout_ms` with back-off, `max_retransmit`
  times (RFC 7252 §4.2); a newer notification takes the place of one in
  flight (RFC 7641 §4.5.2).

Cancel: `Observe: 1` option in the GET; a Reset to a notification (RFC
7641 §3.6); a Confirmable notification not acknowledged after the last
retransmission (§4.5); a ping not answered (§6, `keep_alive_secs`); a
missed deadline (§6).

### §4.4 Block-wise transfer

Samples > `block_size` are split into blocks automatically:
- Outbound (GET, Notify): `Block2(NUM, M, SZX)` option
- Inbound (POST): `Block1(NUM, M, SZX)` option, the daemon assembles before the DDS write

Defragmentation cap: `coap.max_body_size` (default 64 KiB). A larger
transfer is rejected with `4.13` and Size1 (RFC 7959 §2.9.3) and must be
sent via a chunked-stream topic. A block that does not continue the
transfer is `4.08 Request Entity Incomplete` (§2.9.2). `coap.max_message_size`
bounds a single datagram (default 1152, RFC 7252 §4.6).

A GET (of a topic or of `/.well-known/core`) is answered with the block
its Block2 option asks for (RFC 7959 §2.2; a block size larger than the
daemon allows scales the number, §2.4; a block past the end is `4.02 Bad
Option`). A request that is not authenticated (no DTLS session, no OSCORE,
no auth token) gets at most 8 times its own size back (RFC 7252 §11.3,
whose example answers a 10-byte request with a 64-byte block): a larger
representation goes out in Block2 blocks of the largest size within that
bound, 16 octets at least, each with the ETag of the whole representation
and the first with Size2; the first response of an observation and the
notifications to such an observer carry the first block (RFC 7959 §2.6),
the client fetching the rest with GET.

### §4.5 Content format registry

| ID | Media type | Use |
|----|------------|------------|
| 65000 | `application/x-dds-cdr2; bo=le` | XCDR2 little-endian |
| 65001 | `application/x-dds-cdr2; bo=be` | XCDR2 big-endian |
| 65002 | `application/x-dds-cdr1; bo=le` | XCDR1 little-endian (legacy) |
| 50 | `application/json` | JSON representation |
| 60 | `application/cbor` | CBOR representation |

IANA registration in the vendor range 65000-65535; `content_format:`
(§3) sets the number of each form.

A CDR form is the body of the sample without the encapsulation header, in
the encoding the type's extensibility takes (DDS-XTypes 1.3 §7.4: under
XCDR2 PLAIN_CDR2, DELIMITED_CDR or PL_CDR2; under XCDR1 PLAIN_CDR, an
appendable type included, or PL_CDR). JSON is the value of the sample as
`zerodds-wire-codec` writes it, CBOR the same value in CBOR (RFC 8949
§6.2).

With `idl_file` a topic takes and gives all five; a DDS sample in any CDR
representation is converted. Without it a topic takes 65000 and gives the
form of each DDS sample's representation identifier (XCDR2 little endian
65000, big endian 65001, XCDR1 little endian 65002); a sample in XCDR1
big endian has no form there, a request for it is `4.06` and an observer
gets no notification of it (logged).

### §4.6 Multicast (RFC 7252 §8)

With `coap.multicast.enabled` the daemon joins `coap.multicast.groups`
(default: the all-CoAP-nodes group of the bind address's family,
`224.0.1.187`, or `ff02::fd` and `ff05::fd`; an IPv6 group with an IPv4
bind address takes a second, IPv6-only socket on the same port; an IPv4
group needs an IPv4 bind address). It learns the destination address of
every datagram (`IP_PKTINFO`, `IPV6_RECVPKTINFO`, RFC 3542). A request
whose destination is a multicast group:

- is answered only when it is Non-confirmable and has a token (§8.1); a
  Confirmable one is ignored, never answered with Reset;
- is answered only for `coap.multicast.resources` and, without OSCORE,
  only for `coap.multicast.unauthenticated_resources` (§11.3; both
  `.well-known/core` by default, the service discovery of §7.1);
- gets no error and no empty response (§8.2); a `/.well-known/core`
  whose query filter (RFC 6690 §4.1) matches no link gets none;
- is answered by a Non-confirmable unicast message with its own Message
  ID at a random point of a leisure period of `coap.multicast.leisure_ms`
  (default 5 s, `DEFAULT_LEISURE`); a further response for the same group
  takes the next period at the earliest after the previous one; at most
  1024 responses wait, more are not sent;
- registers no observation (an Observe option is ignored).

A request to a resource the daemon does not have gets `4.04 Not Found`
by unicast; a Uri-Path `.` or `..` gets `4.00 Bad Request` (RFC 7252
§5.10.1). The resource name is formed as RFC 7252 §6.5 step 6 says, a
`/` inside one Uri-Path segment percent-encoded, and both the topic map
and the ACL take that name.

### §4.7 Proxy (RFC 7252 §5.7)

The daemon is a CoAP-to-CoAP proxy.

- **Forward** (`proxy.forward`, §5.7.2): a request with `Proxy-Uri`, or
  with `Proxy-Scheme` and the Uri-* options (§5.10.2, §6.5), is carried out
  against the URI it names, for the schemes of `proxy.schemes`. A URI whose
  host is one of `proxy.names`, the request's destination address or the
  bound address (a loopback address with an unspecified bind) and whose
  port is the daemon's port of that scheme is the daemon itself: the
  request is served locally. Any other proxy request without
  `proxy.forward`, or for another scheme, is `5.05 Proxying Not
  Supported`.
- **Reverse** (`proxy.reverse`, §5.7.3): the resources under `path` are
  those under `origin`; the rest of the path and the query are appended.
  A path may not cover a topic or `.well-known/core` (start refused). A
  Location-Path of the origin under its path is mapped back.
- **Options** (§5.7.1): Proxy-Uri, Proxy-Scheme, the Uri-* options and the
  auth token (§7.2) are consumed; If-Match, ETag, If-None-Match, OSCORE,
  Content-Format, Accept, Size1 and Size2 are forwarded; an unrecognized
  Safe-to-Forward option is forwarded, an unrecognized Unsafe one is `4.02
  Bad Option`, a second occurrence of a critical non-repeatable option too
  (§5.4.5). An unrecognized Unsafe option in a response, a response that
  cannot be decoded or a Reset of the destination is `5.02 Bad Gateway`;
  no response within `proxy.timeout_ms` is `5.04 Gateway Timeout`. The
  request to the destination has a token of 64 random bits, a Uri-Host for
  a host that is no IP address, and is Confirmable, retransmitted after
  ACK_TIMEOUT with back-off (§4.2).
- **Hop-Limit** (RFC 8768): a request's Hop-Limit is decremented, one
  without gets `proxy.max_hops`; 0 or more than 255 is `4.00`, 0 after the
  decrement `5.08 Hop Limit Reached` with the daemon's identifier (its
  first name, else its bound address) in the diagnostic payload. A `5.08`
  of the destination is passed on with the identifier put in front; one
  that already names the daemon is a loop and becomes `5.02`.
- **Cache** (§5.6, §5.7.1, §11.2): a `2.05` to a GET without OSCORE and
  without an ETag of the client is kept for its Max-Age (60 s by
  default), at most `proxy.cache_entries` responses; a cached response
  goes out with the Max-Age reduced by its time in the cache. A stale
  response with an ETag is validated with it; `2.03 Valid` refreshes it
  with the new Max-Age. `2.01`, `2.02` and `2.04` for a URI make its
  responses stale. The response to a `coaps` request is kept only for a
  client with a DTLS identity and served only to that identity.
- **coaps destinations** (§5.7, §9): over DTLS 1.2, with the certificate
  checked against `proxy.dtls.ca_file` and the host name, optionally
  presenting `cert_file`/`key_file`; with `psk_identity` and
  `psk_secret_hex`; or with raw public keys, `rpk_key_file` and the RFC 6920
  names of the destinations' keys in `rpk_trusted`. Without any of them
  they are `5.05`. With
  `proxy.require_dtls_for_coaps` (default) the client must come over DTLS,
  else `4.01`.
- **Multicast destinations** (§8.2.2): the request goes out once,
  Non-confirmable; every response within `proxy.multicast_wait_ms` goes
  back to the client, the first as the response, the others as
  Non-confirmable messages with the same token; none is `5.04`. They are
  not cached.
- **Blocks** (RFC 7959 §2.10): a Block1 body is collected (`2.31
  Continue`) and forwarded whole, in 1024-octet Block1 blocks when it is
  larger; Block2 blocks of the destination are fetched and joined (at
  most 1 MiB). A GET answer is cut into Block2 blocks when the client asks
  for a block, when it exceeds 1024 octets, or when the client is not
  authenticated and it exceeds 8 times the request (§11.3, as §4.4); the
  further blocks come from the cache.
- **Observe**: the proxy registers no observation; the response carries no
  Observe option (RFC 7641 §2).
- **Message layer** (§4, §5.2.2): access is that of §7.2 (auth token or
  DTLS identity), `4.01` otherwise. A response that is not there within
  one second is preceded by an empty Acknowledgement and sent as a
  Confirmable message, retransmitted until the client acknowledges or
  resets it; a retransmitted request is carried out once and gets the
  same acknowledgement for EXCHANGE_LIFETIME (§4.5). At most 64 proxy
  requests run at a time, beyond that `5.03` with Max-Age 5.

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
</chat/message>;rt="dds.topic";ct="65000 65001 65002 50 60";type="Chat::Message",
</schema/chat/message>;rt="dds.schema";ct=0,
</sensors/reading>;rt="dds.topic";ct=65000;type="Sensor::Reading",
</actuator/command>;rt="dds.topic";ct=65000;type="Actuator::Command"
```

`ct` lists the forms of §4.5 the topic gives: all five with `idl_file`,
65000 without.

`GET /schema/<slug>` returns the IDL definition for codegen on the
client side: the text of the topic's `idl_file` with its `#include`s
expanded, Content-Format 0 (`text/plain; charset=utf-8`); the ACL's read
permission of the topic applies. A topic without `idl_file` has none,
`4.04`.

## §6 QoS translation

| DDS QoS | CoAP behavior |
|---------|----------------|
| Reliability `RELIABLE` | CoAP `CON` messages (Confirmable) with retransmit |
| Reliability `BEST_EFFORT` | CoAP `NON` messages (Non-Confirmable) |
| Durability `VOLATILE` | live notify only; no replay on Observe re-register |
| Durability `TRANSIENT_LOCAL` | initial notify with the last sample |
| History `KEEP_LAST(N)` | analogous (Observe re-register sends the last N); `KEEP_ALL` the last 1024 |
| Lifespan | `Max-Age` option in the notify and the GET response: what is left of `lifespan_ms`; an expired sample is no representation |
| Deadline | observed, the daemon emits `5.03 Service Unavailable` with `Max-Age=0`: a GET while `deadline_ms` has passed without a sample, and once a notification to every observer, which ends its observation (RFC 7641 §4.2) |
| Liveliness | CoAP ping (`0.00 EMPTY` as Confirmable heartbeat every `keep_alive_secs`) to each endpoint with an observation; an endpoint that does not answer after the retransmissions loses its observations; `liveliness_lease_ms` is the lease of the topic's DDS endpoints |
| Partition | filter in the daemon before the notify push: the topic's DDS endpoints are in `partition`, a sample of another partition does not reach the daemon |

## §7 Security

### §7.1 DTLS

`coaps://` mode is enabled via `coap.dtls.enabled: true` (feature `dtls`:
DTLS 1.2 on OpenSSL 3.2 or later, ADR 0014). The three modes of RFC 7252
§9 can be configured together:

- **PreSharedKey** (§9.1.3.1): `coap.dtls.psk`, identity and `secret_hex`.
- **RawPublicKey** (§9.1.3.2, RFC 7250): `coap.dtls.rpk_key_file`, the PEM
  private key whose public key the daemon shows instead of a certificate;
  `coap.dtls.rpk_trusted`, the RFC 6920 names of the client keys it
  accepts (`ni:///sha-256-120;…`, `nih:sha-256-120;…;c` or the hex of the
  binary form; at least `sha-256-120`, §9.1.3.2.1). With trusted keys the
  daemon asks a client for its key; a key not in the list gets no session.
- **Certificate** (§9.1.3.3): `cert_file`, `key_file`; with `ca_file` the
  daemon asks a client for a certificate and checks it against that CA.
  Without any credential the daemon generates a self-signed certificate
  for `localhost`.

Cipher suites, in this order unless `cipher_suites` names others:
`TLS_PSK_WITH_AES_128_CCM_8` and `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` (the
suites RFC 7252 makes mandatory), then the CCM and GCM suites of the same
key exchange (an OpenSSL 3 peer with its defaults offers no CCM suite);
`TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` when configured. The subject of a
DTLS client for §7.2/§7.3 is `psk:<identity>`, the certificate's DN, or
`rpk:` and the `ni` URI (sha-256) of its raw public key.

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

Structured JSON log on stdout, one object per line. Fields: `timestamp`
(RFC 3339, UTC, milliseconds), `level`, `event`, `peer`, `coap_uri`,
`dds_topic`, `mid`, `token_hex`, `block_num`, `bytes`, `latency_us`, and
per event others (`method`, `code`, `reason`). `log_level` is the
threshold. Each exchange is an event `request`: at `debug` for a 2.xx
response, `info` for 4.xx, `warn` for 5.xx. Errors of the command line
before the configuration is read go to stderr as text.

### §8.2 Prometheus metrics

If `metrics.enabled: true`, at `metrics.listen` under `metrics.path`:
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
CoAP exchange (`coap.request` + `dds.write` as child). `coap.request` is
`SpanKind::Server` from the arrival of the request to its response, with
`coap.method`, `coap.code`, `coap.uri`, `net.peer` and `dds.topic`, status
Error for a 4.xx or 5.xx response; `dds.write` is `SpanKind::Client`, the
DDS write or dispose the request made. Flushed every 5 s and at the
shutdown.

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

Verified in `crates/coap-bridge/tests/cross_vendor.rs`: libcoap's
`coap-client` (POST, the catalog, Observe), aiocoap and Eclipse Californium
(a POST in JSON, a GET in JSON, an observation that gets the next sample as
a JSON notification), Eclipse Wakaama's LwM2M client (its registration, a
Confirmable `POST /rd` with Uri-Query and a link-format body, is read and
answered piggybacked, and Wakaama reads the answer).

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
client against the ZeroDDS CoAP bridge, with the programs
`tests/integration/provision.sh` installs, in the weekly job
`integration-bridges-vendor`. A JSON sample libcoap POSTs reaches a
Cyclone DDS reader of the type, and a sample a Cyclone DDS writer writes
reaches libcoap's observation as a JSON notification; the Cyclone DDS
participant is `tests/interop/cyclone_reading.c`, built by the test with
`idlc`.

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
