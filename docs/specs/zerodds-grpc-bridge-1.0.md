# `zerodds-grpc-bridge` v1.0 — DDS↔gRPC bridge daemon

ZeroDDS vendor spec. Specifies the behaviour of a configurable
daemon that exposes DDS topics as gRPC services (HTTP/2 streaming).

## Motivation

gRPC is the de facto standard for modern microservice RPCs:
Kubernetes service mesh (Istio, Linkerd), cloud APIs (Google, AWS),
and polyglot backend stacks. ZeroDDS brings real-time industrial data;
the `zerodds-grpc-bridged` daemon provides every DDS topic as a gRPC
service `<TopicSlug>Stream` with `Publish(stream Sample)` /
`Subscribe(SubscribeReq) returns (stream Sample)`.

Complementary to the libraries `crates/grpc-bridge/`, `crates/http2/`
and `crates/hpack/` — the libraries are building blocks (HTTP/2,
HPACK, Protobuf codec, gRPC status), this daemon is the
executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks gRPC over HTTP/2 (RFC 7540) + HPACK (RFC 7541) + gRPC-PROTOCOL.md (gRPC status, gRPC encoding, gRPC timeout). |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional: gRPC client→DDS publish (client stream) + DDS→gRPC push (server stream). |
| **L4 — Config** | Topic map via YAML config; hot reload via SIGHUP optional. |
| **L5 — Auth** | TLS + ALPN(`h2`) + JWT bearer (`Authorization` metadata) + mTLS. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per gRPC connection. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

L6: each entry of `tenants` (§3) is a DomainParticipant of its own on its
`domain`, with its `topics`. A TLS connection whose client certificate a
tenant's `subjects` names (the subject DN as an RFC 4514 string, or the
fingerprint `mtls:<SHA-256 hex>`, §7.2) is bound to that tenant for its
whole life: its calls address the tenant's services, and only them; the
`Catalog` RPC and the reflection service list only them. Cleartext
connections and a TLS connection no tenant names address the participant
of the top-level `domain` and `topics`. A subject is bound to one tenant at
most. In the ACL (§7.3) and in the `dds_topic` and `service` labels of the
metrics (§8.2) a tenant's topic and service are `<tenant>/<name>`; the admin
`/catalog` lists them the same way. A tenant name contains no `/`.
Since a connection is bound by its client certificate, `tenants` needs
`grpc.tls.enabled` and `grpc.tls.client_ca_file`; without them the start
stops with a configuration error.

## §2 CLI surface

```
zerodds-grpc-bridged [OPTIONS]

Options:
  --config <FILE>              base file (YAML subset of zerodds-config),
                               plus FILE.d/; without: grpc-bridged.yaml and grpc-bridged.d/
                               in the system directory
  --set <PATH=VALUE>           one value above all files, repeatable
  --check-config               check the configuration, print every value with its origin,
                               exit (0 valid, 1 invalid)
  --bind <ADDR>             HTTP/2 bind address (default 0.0.0.0:50051)
  --domain <ID>             DDS domain ID (default 0)
  --tls-cert <FILE>         TLS cert (PEM) — mandatory for the gRPC standard (h2 ALPN)
  --tls-key <FILE>          TLS key (PEM)
  --tls-client-ca <FILE>    client CA for mTLS (optional)
  --reflection              enable the gRPC reflection service
  --topic <DDS:SVC>         single-topic override (repeatable)
  --log-level <LEVEL>       trace/debug/info/warn/error (default info)
  --metrics <ADDR>          Prometheus scrape endpoint (default off)
  --version                 version info
  --help                    help

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   bind error (port in use)
  3   DDS discovery error
  4   TLS error
```

`--check-config` refuses (exit 1) every setting the start refuses as a
configuration error, the cross-key checks of §7 among them (`auth.mode:
bearer` without `auth.bearer_token`, `jwt` without a usable key, `mtls`
without TLS and a client CA); the TLS material (exit 4) is loaded at the
start only.

Precedence (lowest first): built-in defaults, environment, files,
flags. `--bind`, `--domain`, `--tls-cert`/`--tls-key` (only together), `--reflection`, `--topic <DDS=SERVICE>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
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
# zerodds-grpc-bridged.yaml
domain: 0
log_level: info

grpc:
  bind: "0.0.0.0:50051"
  max_concurrent_streams: 1024
  max_recv_message_size: 4194304          # 4 MB
  max_send_message_size: 4194304
  initial_window_size: 1048576
  max_frame_size: 16384
  keepalive:
    time_secs: 60
    timeout_secs: 10
    permit_without_calls: false
  drain_secs: 30                           # §9.2 graceful drain
  tls:
    enabled: true
    cert_file: "/etc/zerodds/grpc-cert.pem"
    key_file:  "/etc/zerodds/grpc-key.pem"
    client_ca_file: "/etc/zerodds/grpc-client-ca.pem"   # mTLS optional
    require_client_cert: false
    alpn: ["h2", "http/1.1"]                 # http/1.1 for the IDL endpoint (§5.2)
    min_version: "1.2"                     # 1.2 | 1.3 (§7.1)
    cipher_suites: ["TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384"]   # IANA names (§7.1)
  reflection:
    enabled: true                          # gRPC server reflection (§4.6)

auth:
  mode: "jwt"                              # none | bearer | jwt | mtls (none: loopback bind only, §7.2)
  # bearer_token: "..."                    # mode bearer
  # bearer_subject: "alice"
  jwt:
    public_key: "/etc/zerodds/jwt-pub.pem"
    issuer: "https://idp.example.org"
    audience: "zerodds-grpc"
    required_claim: "scope=dds.read"

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    grpc_service: "ChatMessageStream"
    grpc_package: "zerodds.chat.v1"
    idl_file: "/etc/zerodds/idl/chat.idl"  # §5.2
    keyed: true                            # false: dds_type has no key (§10)
    direction: "bidir"
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
      deadline_ms: 1000                    # §6
      lifespan_ms: 60000                   # §6
    acl:
      publish: ["alice", "bob"]
      subscribe: ["*"]

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    grpc_service: "SensorReadingStream"
    grpc_package: "zerodds.sensor.v1"
    direction: "out"
    qos:
      reliability: "best_effort"

metrics:
  enabled: true
  listen: "127.0.0.1:9095"
  path: "/metrics"

tenants:                                   # §1 L6: participants beside the one of `domain`
  - name: "plant-a"
    domain: 7
    subjects: ["CN=plant-a,O=Example"]     # client certificates bound to it
    topics:
      - dds_name: "Plant::Status"
        grpc_service: "PlantStatusStream"
```

ENV substitution: `${VAR}` and `${VAR:-default}`.

The `grpc.*` limits apply to every connection:

- `max_concurrent_streams` (1 to 2^32-1) is the server's
  `SETTINGS_MAX_CONCURRENT_STREAMS`. A stream opened while that many are
  open (requests being received and calls being answered) is refused with
  `RST_STREAM(REFUSED_STREAM)` (RFC 9113 §5.1.2).
- `max_recv_message_size` (octets, at least 1): a request message whose
  length prefix (§4.4) names more ends its call with `RESOURCE_EXHAUSTED`
  before its octets are held; when the client has not ended its side of the
  stream, `RST_STREAM(NO_ERROR)` follows the trailers (RFC 9113 §8.1).
- `max_send_message_size` (octets, at least 1): a response message above it
  ends a unary call with `RESOURCE_EXHAUSTED` instead of the message, and a
  `Subscribe` stream with `RESOURCE_EXHAUSTED` in its trailers.
- `initial_window_size` (1 to 2^31-1) is the server's
  `SETTINGS_INITIAL_WINDOW_SIZE`; right after its SETTINGS the server opens
  the connection window to the same size with a `WINDOW_UPDATE` (the
  protocol fixes it at 65 535 otherwise). The server returns what a client
  sent with `WINDOW_UPDATE` on the connection and on the stream once half
  the window is used. A stream window below 65 535 applies once the client
  acknowledged the server's SETTINGS; until then a stream takes the 65 535
  octets the client assumes, and the acknowledgement lowers the window of
  the open streams by the difference (RFC 9113 §6.9.2, §6.9.3). DATA beyond
  the connection window is a connection error, beyond a stream's window a
  stream error, both `FLOW_CONTROL_ERROR` (RFC 9113 §6.9.1).
- `max_frame_size` (16 384 to 16 777 215) is the server's
  `SETTINGS_MAX_FRAME_SIZE`; a larger frame is a connection error. The
  server's own frames stay within the client's `SETTINGS_MAX_FRAME_SIZE`;
  a larger message goes out in several DATA frames.
- `keepalive.time_secs` (at least 1): after this long without a frame from
  the client the server sends a PING, while a call is open, or always with
  `permit_without_calls`. `keepalive.timeout_secs` (at least 1): when the
  PING is not acknowledged within it, the server sends `GOAWAY(NO_ERROR)`
  and closes the connection.
- `keepalive.permit_without_calls`: with `false` (the default) the first
  client PING while no call is open is answered, and so are two more before
  the server sends HEADERS or DATA again; the next ends the connection with
  `GOAWAY(ENHANCE_YOUR_CALM, "too_many_pings")`, as the keepalive
  enforcement of grpc-go does. A connection without a call is closed after
  5 s without a frame. With `true` every PING is answered, and a connection
  without a call stays open as long as it acknowledges the server's PINGs.
- `tls.alpn` is the list of ALPN protocols the TLS listener offers, in
  its order of preference (default `["h2", "http/1.1"]`). It names `h2`
  (gRPC over HTTP/2) and may name `http/1.1`, the IDL endpoint of §5.2;
  another entry, an entry named twice or a list without `h2` stops the
  start.
- `tls.min_version` and `tls.cipher_suites`: §7.1.
- `drain_secs` (0 to 300 seconds, default 30) is the graceful drain of
  §9.2: how long calls open at SIGTERM or SIGINT may run before they end
  with UNAVAILABLE, and the bound of the wait for the acknowledgements of
  the DDS writes. The systemd unit's `TimeoutStopSec=40` is above the
  default; a longer drain needs it raised.

A topic entry needs `dds_name`; `grpc_service` and `grpc_package` default
as §5.1 says. `dds_type` is the type name of the DDS endpoints (default
`ZeroDdsBridgeBytes`). `keyed` (default `true`) says whether `dds_type`
declares a key: the topic's DataWriter and DataReaders are WITH_KEY
endpoints, with `keyed: false` NO_KEY endpoints (DDSI-RTPS 2.5 §9.3.1.2),
as a DDS peer of a type without a key matches them (§10); a `Sample` with a
`key_hash` on such a topic is `INVALID_ARGUMENT`. `idl_file` is the IDL file that declares `dds_type`;
it is read at the start (a file that cannot be read stops it) and served
as §5.2 says; the daemon does not parse it. `qos.reliability` (default
`reliable`), `qos.durability` (default `volatile`), `qos.history`,
`qos.deadline_ms` and `qos.lifespan_ms` (1 to 2^31-1 milliseconds) act as
§6 says. `qos.history` is a map of `kind` (`keep_last` | `keep_all`) and
`depth`; `keep_all` with a `depth` stops the start with exit 1 and the
setting named, and `--check-config` refuses it.

`metrics.listen` (and `--metrics`) is the address of the admin listener, an
IP address and port (IPv6 in brackets, default `127.0.0.1:9095` with
`metrics.enabled: true`); a host name such as `localhost:9095` stops the
start with exit 1 and the value named, and `--check-config` refuses it.
`metrics.path` is the path of the Prometheus page on the admin listener; it
is absolute and none of `/`, `/catalog`, `/healthz`. `auth.bearer_token` and
`auth.bearer_subject` serve `auth.mode: bearer` (§7.2): the token in the
`authorization` metadata and the subject it stands for (default
`anonymous`). `auth.jwt.issuer`, when set, is checked against the `iss`
claim.

A changed limit, keepalive setting, `drain_secs`, `metrics.path` or
`tenants` entry needs a restart (§9.2).

## §4 gRPC wire protocol

### §4.1 HTTP/2 setup

The daemon is an HTTP/2 server with ALPN `h2`. Cleartext h2c **only** in dev
mode (`tls.enabled=false` and `bind=127.0.0.1`): with TLS off, a `grpc.bind`
that is not a loopback address (127.0.0.0/8, `::1`, `localhost`) stops the
start with exit 1, and `--check-config` refuses it. A TLS client that does
not negotiate `h2` with ALPN gets no HTTP/2: one that offers other
protocols fails the handshake, one that offers none is closed after it
(RFC 9113 §3.2).

`SETTINGS` frame with the values from the `grpc.*` config
(`MAX_CONCURRENT_STREAMS`, `INITIAL_WINDOW_SIZE`, `MAX_FRAME_SIZE`, §3) and
`MAX_HEADER_LIST_SIZE` 16 384; `GOAWAY` on shutdown.

The rest of RFC 9113 as the daemon applies it:

- Flow control in both directions (§6.9). Receiving: §3
  `initial_window_size`. Sending: every DATA frame waits for room in the
  connection window and in the stream window the client grants; the
  client's `WINDOW_UPDATE` and a change of its
  `SETTINGS_INITIAL_WINDOW_SIZE` (applied to the open streams, §6.9.2)
  release it. A `Subscribe` stream whose client does not read takes no
  further sample from its DataReader once 256 KiB of messages wait for the
  client's window, so it holds at most 256 KiB and the last message it
  took (up to `max_send_message_size`, §3).
- Frames: DATA within the client's `SETTINGS_MAX_FRAME_SIZE`, a message
  larger than a frame in several DATA frames; a header block larger than
  a frame continues in CONTINUATION frames. A smaller
  `SETTINGS_HEADER_TABLE_SIZE` of the client is announced at the start of
  the next header block (RFC 7541 §4.2).
- Receiving: a header block in HEADERS and CONTINUATION frames, without the
  padding and the priority fields of HEADERS; DATA without its padding;
  several messages in one DATA frame and one message across DATA frames. A
  header block above 1 MiB, in one HEADERS frame or with its CONTINUATION
  frames, is a connection error (`ENHANCE_YOUR_CALM`). A request whose
  decoded header list is above 16 384 octets (name, value and 32 octets per
  field, RFC 9113 §6.5.2; the server's `SETTINGS_MAX_HEADER_LIST_SIZE`, the
  hard metadata limit of gRPC core) ends with `RESOURCE_EXHAUSTED`; its
  fields are not held, but its block is decoded to the end, so the HPACK
  state of the connection stays in step (RFC 9113 §10.5.1). A dynamic table
  size update above the 4 096 octets of `SETTINGS_HEADER_TABLE_SIZE` (the
  server announces no other value) or after a header field is a connection
  error (`COMPRESSION_ERROR`, RFC 7541 §4.2, §6.3).
  Frames of an unknown type and PRIORITY are ignored; PUSH_PROMISE from a
  client is a connection error.
- A connection error sends `GOAWAY` with its error code (§5.4.1), a stream
  error `RST_STREAM` with its code (§5.4.2).

### §4.2 Service definition (auto-generated)

For each topic the daemon implicitly generates a .proto definition:
```protobuf
syntax = "proto3";
package zerodds.chat.v1;

service ChatMessageStream {
  rpc Publish(stream Sample) returns (PublishAck);
  rpc Subscribe(SubscribeReq) returns (stream Sample);
  rpc PublishOne(Sample) returns (PublishAck);
  rpc Catalog(CatalogReq) returns (CatalogResp);
}

message Sample {
  bytes  cdr_payload   = 1;       // CDR-Bytes including Encap-Header
  uint32 flags         = 2;       // dispose-bit etc.
  bytes  key_hash      = 3;
  uint64 source_ts_ns  = 4;
  string type_name     = 5;
  string topic_name    = 6;
}

message PublishAck {
  uint64 sequence_number = 1;
  uint64 acked_at_ns     = 2;
}

message SubscribeReq {
  string  partition_filter = 1;     // optional partition expression
  bool    include_dispose  = 2;
  uint32  initial_burst    = 3;     // 0 = no burst
}

message CatalogReq {}
message CatalogResp {
  repeated TopicEntry topics = 1;
}
message TopicEntry {
  string dds_name      = 1;
  string dds_type      = 2;
  string grpc_service  = 3;
  string idl_url       = 4;
}
```

The crate ships this definition, for the topic `Chat::Message` with the
defaults of §5.1, as `crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto`
(the name the reflection service gives the file, §4.6): a client generates
its stubs from it; for another topic it changes the package and the service
name, the messages stay.

### §4.3 RPC flows

**Publish (gRPC → DDS)**:
```
HEADERS (HEADERS frame)
  :method     = POST
  :path       = /zerodds.chat.v1.ChatMessageStream/Publish
  content-type = application/grpc+proto
  grpc-encoding = identity | gzip | deflate
  grpc-timeout = 30S
  authorization = Bearer <token>     (if auth=jwt)
DATA frames (Length-Prefix-Message: 5-byte header + Sample-bytes)
  ...
HEADERS (END_STREAM)
  grpc-status = 0  (OK) on success, with grpc-message on error
```

`Publish` is a client stream: the daemon writes each message to DDS as
it arrives and answers once, after the client ended its side, with one
`PublishAck` carrying the sequence number of the last sample written and
the time of the answer. A message that fails ends the call with its status
(§4.5); the messages before it are written.

`PublishOne` and `Catalog` take one request message and answer at once;
when the client has not ended its side, `RST_STREAM(NO_ERROR)` follows the
trailers. A second request message on a call of a unary request is
`INTERNAL` ("too many requests", as grpc-java answers it). A client that
ends its side without a request message sends the empty message (all
fields default).

`Sample` on `Publish`: `cdr_payload` as §4.4; `key_hash` empty or 16
octets; `source_ts_ns` (0 is none) becomes the source timestamp of the
write, and a value above 4 294 967 295 999 999 999 (the last nanosecond of
the unsigned 32-bit seconds of the RTPS timestamp, DDSI-RTPS 2.5 §9.3.2.1)
is `INVALID_ARGUMENT`; a `type_name` or `topic_name` that is set and is not the mapping's is
`INVALID_ARGUMENT`. `flags` bit 0 is a dispose, bit 1 an unregister of the
instance `key_hash` (needed; `cdr_payload` stays empty); other bits are
`INVALID_ARGUMENT`.

Request headers:

- `:method` other than `POST`: HTTP status 405 with `grpc-status` 12.
- `content-type` that does not begin with `application/grpc`: HTTP status
  415 with `grpc-status` 3 (the answer of grpc-go). `application/grpc` and
  `application/grpc+proto` (parameters after `;` allowed) are served;
  another message format is `grpc-status` 12.
- `grpc-timeout` (gRPC over HTTP/2, "Timeout") is the deadline of the call:
  a call still open at it ends with `DEADLINE_EXCEEDED`. A malformed value
  is `INTERNAL` (grpc-go).
- `grpc-encoding`, `grpc-accept-encoding`: §4.4.

**Subscribe (DDS → gRPC)**:
- The client opens a server-streaming RPC.
- The daemon registers a DataReader; every sample goes out as a DATA frame
  (length-prefix wrapper).
- `grpc-status=0` on reader cleanup.

`SubscribeReq`: `partition_filter` is the partition list of the stream's
DataReader, comma-separated, with the wildcards of DDS partition names
(empty is the default partition); `include_dispose` sends a dispose or
unregister as a `Sample` with its flag, its `key_hash` and an empty
`cdr_payload`; `initial_burst` is the number of retained samples sent
first (§6). Each `Sample` of the stream carries the encapsulation
header and the CDR bytes, the `key_hash` when the sample names one, the
source timestamp, `type_name` (`dds_type` of the mapping) and `topic_name`.

### §4.4 Length-prefix wrapper

gRPC standard:
```
+--------+----------------+----------------------+
| Compr  | Message Length | Protobuf-Sample      |
| 1 byte | 4 bytes (BE)   | (variable)           |
+--------+----------------+----------------------+
```
Compr flag: 0=identity, 1=compressed (gzip/deflate according to `grpc-encoding`).

`gzip` is RFC 1952, `deflate` the zlib format of RFC 1950 (as HTTP's
`deflate`). A gzip message is the series of its members (RFC 1952 §2.2),
inflated one after the other. The inflated message is bounded by
`max_recv_message_size` (above it: `RESOURCE_EXHAUSTED`). Octets after the
last gzip member or after the zlib stream make the message one that does
not inflate (`INTERNAL`). A message compressed with an encoding
the daemon does not speak is `UNIMPLEMENTED`, and the response headers name
`grpc-accept-encoding: identity,deflate,gzip` (gRPC Compression); the
flag 1 with `grpc-encoding: identity` or none is `INTERNAL`, a stream that
does not inflate too. Every response HEADERS carries
`grpc-accept-encoding`; when the request named `gzip` or `deflate`, the
response messages are compressed with it and `grpc-encoding` says so.

Inside the Protobuf sample, the `cdr_payload` field contains the
**complete CDR bytes incl. the 4-byte encap header** as over RTPS,
according to `zerodds-xcdr2-bindings-conformance-1.0` §3.

### §4.5 Status mapping

| gRPC status | DDS-side condition |
|-------------|--------------------|
| `OK (0)` | Sample written/streamed |
| `INVALID_ARGUMENT (3)` | CDR decode error |
| `PERMISSION_DENIED (7)` | ACL error |
| `UNAUTHENTICATED (16)` | JWT/mTLS error |
| `RESOURCE_EXHAUSTED (8)` | Send queue full / DDS backpressure |
| `DEADLINE_EXCEEDED (4)` | gRPC timeout header |
| `UNAVAILABLE (14)` | DDS liveliness lost / daemon drain |
| `INTERNAL (13)` | unexpected error |

What the rows stand for in the daemon:

- `INVALID_ARGUMENT`: a request message that is not proto3 wire format, a
  `cdr_payload` shorter than the encapsulation header or with an
  identifier that names no CDR representation, the `Sample` checks of
  §4.3. The CDR bytes after the header are passed on as they are (§4.4):
  the bridge registers no type for them.
- `RESOURCE_EXHAUSTED`: a DDS write that finds no room — RESOURCE_LIMITS
  reached, or a reliable writer blocked beyond `max_blocking_time` — and
  the limits of §3.
- `DEADLINE_EXCEEDED`: the `grpc-timeout` of the call passed.
- `UNAVAILABLE`: the calls still open at the end of the drain (§9.2); a
  `Subscribe` stream whose DataReader has matched DataWriters and none of them alive
  (`LIVELINESS_CHANGED`: `alive_count` 0, `not_alive_count` above 0), after
  the samples it holds went out; a call without a DDS participant.
- `INTERNAL`: another DDS error of a write, a reader the runtime refused,
  the request errors named in §4.3 and §4.4.
- Besides the table: `NOT_FOUND` for a service that is not mapped,
  `UNIMPLEMENTED` for a method the service does not have and for the
  disabled reflection service, `FAILED_PRECONDITION` for the direction a
  topic does not carry, `CANCELLED` (in the metrics) for a call the client
  reset.

### §4.6 Reflection service

If `grpc.reflection.enabled: true`, the daemon exports the standard
`grpc.reflection.v1alpha.ServerReflection` service with the auto-
generated topic services + sample message types.

The service is served under both names, `grpc.reflection.v1` and
`grpc.reflection.v1alpha` (grpc/grpc-proto `reflection.proto`; a client
asks for `v1` first). `ServerReflectionInfo` is a bidirectional stream:
each `ServerReflectionRequest` gets one `ServerReflectionResponse` with the
request's `host` as `valid_host` and the request as `original_request`;
the client's half-close ends the stream with `OK`. A stream whose client
does not read answers no further request once 256 KiB of responses wait
for the client's window (RFC 9113 §6.9), as a `Subscribe` stream does
(§4.1), so it holds at most 256 KiB and the last response (up to
`max_send_message_size`, §3; a response repeats its request as
`original_request`). The requests after them wait unanswered and the
daemon returns none of the stream's receive window meanwhile, so the
client sends at most what is left of `initial_window_size` (§3). Once the
client reads they are answered in order; a half-close behind them ends the
stream after their responses.

- `list_services`: the services of the connection's participant
  (`<package>.<service>`, §5.1) and the reflection service in both names.
- `file_by_filename`, `file_containing_symbol`: a
  `FileDescriptorProto`. Per package one file,
  `<package as path>/bridge.proto` (`zerodds/chat/v1/bridge.proto`), with
  the six messages of §4.2 and the services of the package with their four
  methods; the reflection protocol is the file
  `grpc/reflection/<v1|v1alpha>/reflection.proto`. A symbol is a service,
  a method, a message or a field. No file imports another.
- `all_extension_numbers_of_type`: a message of these files has no
  extensions — its name and no number.
- `file_containing_extension`: no file declares an extension.

An unknown file, symbol or type is an `error_response` with `NOT_FOUND`
(5) and the stream stays open (grpc-go). A request that is not protobuf or
names no request ends the stream with `INVALID_ARGUMENT`. With
`grpc.reflection.enabled: false` both names are `UNIMPLEMENTED`; another
method of the reflection service than `ServerReflectionInfo` is
`UNIMPLEMENTED` too.

## §5 Topic mapping

### §5.1 Default service name

Topic name `Chat::Message` → default gRPC service:
1. Split on `::` → ` `: `["Chat", "Message"]`
2. CamelCase concat: `ChatMessage`
3. Suffix: `ChatMessageStream`
4. Package: from config or default `zerodds.<lowercase-first-segment>.v1`

Override via the `grpc_service`/`grpc_package` fields.

Every character of the topic name that is not an ASCII letter or digit
separates words in step 1 (`rt/chatter` → `RtChatterStream`); a name that
begins with a digit gets the prefix `Topic`. In step 4 the first segment is
the text before the first `::`; a character a protobuf identifier does not
take becomes `_`, a leading digit gets a `_` (`dds.demo.Trade` →
`zerodds.dds_demo_trade.v1`), an empty segment gives `zerodds.topic.v1`.
A dotted `grpc_service` is `<package>.<Service>` (and may not stand beside
`grpc_package`); `grpc_service` is a protobuf identifier and
`grpc_package` a dotted one, otherwise the start stops. A call whose
`:path` names a package addresses the service only in that package; a
service name without a package addresses it too, while only one mapping
of the participant has that service name; when mappings in several
packages have it, a call without the package is `NOT_FOUND` and its
`grpc-message` names the packages. Two mappings of one participant that
name the same fully-qualified service stop the start.

### §5.2 Type discovery

Three paths:
- `grpc.reflection` service (standard).
- `Catalog` RPC per topic service returns the TopicEntry list.
- HTTP GET (outside gRPC) `https://<bind>/idl/<dds_name>` returns the
  IDL source as `text/x-omg-idl` (sidekick endpoint).

The IDL endpoint is on the gRPC listener: an HTTP/2 GET on any connection
(over TLS the connection negotiated `h2`, §4.1), and HTTP/1.1 — on the
cleartext listener a connection whose first octets are a request line,
not the HTTP/2 preface; on the TLS listener a connection that negotiated
ALPN `http/1.1` (offered by default, §3 `tls.alpn`), or none and sends a
request line. One HTTP/1.1 request, then the connection closes.
`<dds_name>` is one path segment, percent-encoded where RFC 3986 asks.
The answer is the text of the topic's `idl_file` (§3) with
`content-type: text/x-omg-idl`; a topic the participant does not map or
one without `idl_file` is 404, a malformed escape 400, another HTTP/1.1
path 404, another HTTP/1.1 method 405. The request is authenticated as a
call (§7.2; HTTP/1.1 with its `Authorization` header, over TLS with the
client certificate for `auth.mode: mtls`), else 401.

`CatalogResp.topics[].idl_url` is
`<scheme>://<authority>/idl/<dds_name>` for a topic with `idl_file`
(`https` over TLS, `http` on the cleartext listener, the `:authority` of
the `Catalog` call, else `grpc.bind`) and empty without one; `grpc_service`
is fully qualified. The admin `/catalog` (§8.2 listener) names
`dds_name`, the fully-qualified `grpc_service` and `direction`.

## §6 QoS translation

gRPC runs over TCP/HTTP/2 — TCP guarantees are given. Therefore:

| DDS QoS | gRPC behaviour |
|---------|----------------|
| Reliability `RELIABLE` | unmodified; backpressure via HTTP/2 flow control |
| Reliability `BEST_EFFORT` | daemon drops samples when the `WINDOW_UPDATE` queue is full |
| Durability `VOLATILE` | live stream only |
| Durability `TRANSIENT_LOCAL` | daemon keeps the last N samples + sends them as `initial_burst` on subscribe |
| History `KEEP_LAST(N)` | likewise |
| Lifespan | as `grpc-timeout` header or metadata `zerodds-lifespan-ms` |
| Deadline | daemon emits metadata `zerodds-deadline-missed` trailer |
| Liveliness | HTTP/2 `PING` frames per `keepalive_*` config |
| Partition | `SubscribeReq.partition_filter` |

What the rows do in the daemon:

- Reliability: `qos.reliability` of the topic's DataWriter and of each
  stream's DataReader. `RELIABLE`: a `Subscribe` stream takes no further
  sample once 256 KiB wait for the client's window (§4.1); the samples
  after it wait in the DataReader. `BEST_EFFORT`: while 256 KiB wait for
  the window, the samples the stream's DataReader holds are dropped, counted in
  `zerodds_grpc_bridge_samples_dropped_total{dds_topic}` and logged at
  `warn` (`samples_dropped` with `count`): at the first drop of a stream,
  then at most once a second with the samples dropped since the last
  line, and at the end of the stream with the rest.
- History of the writers: `qos.history` is the HISTORY of the topic's
  DataWriter that carries `Publish` calls into DDS and of its DataWriters of
  other lifespans: `keep_last` with `depth` N is KEEP_LAST(N), `keep_last`
  without a depth the runtime's default (KEEP_LAST 1), `keep_all` KEEP_ALL.
  A history the runtime refuses for the topic's DataWriter stops the start.
- Durability, History: a topic carried out of DDS with a durability other
  than `volatile` gets one DataReader of that durability at the start, and
  the daemon retains its samples: per instance the last
  `qos.history.depth` (`keep_all` or no depth: all), at most 10 000 of all
  such topics of a participant together, the oldest going first; a dispose
  or unregister removes the instance. A `Subscribe` with `initial_burst`
  N > 0 gets the last N retained samples first, oldest first, then the
  live samples from a VOLATILE DataReader. Until that DataReader has
  matched a DDS writer, the samples the writer writes come from the
  retained samples, so that the stream misses none between the burst and
  the live samples; each change goes out once (by its writer GUID and
  sequence number, DDSI-RTPS 2.5 §8.3.5.4). The burst and these samples
  wait for the client's window as the live samples do (§4.1: no further
  sample once 256 KiB wait), and go out before the live samples. With
  `initial_burst` 0 the stream's DataReader has the topic's durability (a
  TRANSIENT_LOCAL reader gets the history of the DDS writers). A topic the
  daemon retains nothing of (`volatile`) has an empty burst. The retained
  samples are those of the default partition: `initial_burst` with a
  `partition_filter` is `INVALID_ARGUMENT`.
- Lifespan: `qos.lifespan_ms` is the LIFESPAN of the topic's DataWriter. A
  `Publish` or `PublishOne` with the metadata `zerodds-lifespan-ms` (1 to
  2^31-1; another value is `INVALID_ARGUMENT`), else with `grpc-timeout`
  (rounded up to whole seconds, at least 1 s), writes with a DataWriter of
  that LIFESPAN, made at its first use; it waits up to 1 s to match the
  DataReaders the topic's DataWriter matched. At most 8 such DataWriters
  per topic: for a ninth lifespan the one that wrote least recently is
  deleted once its reliable readers acknowledged its samples (it waits up
  to 1 s) — on a topic with a durability other than `volatile` only one
  whose last sample expired. When none can go, the call is
  `RESOURCE_EXHAUSTED`. A DDS reader sees the deleted DataWriter end
  (instances only it wrote become NOT_ALIVE_NO_WRITERS). These waits hold
  only the calls of the lifespan being made or replaced: a call of that
  lifespan waits for the end (at most 3 s, then `RESOURCE_EXHAUSTED`) and
  takes the DataWriter made, or the old one when it could not go; a call
  of a DataWriter that exists does not wait.
- Deadline: `qos.deadline_ms` is the DEADLINE the topic's DataWriter
  offers and each stream's DataReader requests. A `Subscribe` stream of
  such a topic ends with the trailer `zerodds-deadline-missed: <n>`, the
  `REQUESTED_DEADLINE_MISSED.total_count` of its DataReader; each new miss
  is logged (`deadline_missed`).
- Liveliness: the keepalive PINGs of §3 (`grpc.keepalive.*`); a stream
  whose DDS writers lost their liveliness ends `UNAVAILABLE` (§4.5).
- Partition: §4.3.

## §7 Security

### §7.1 TLS

ALPN `h2` is the gRPC standard; `http/1.1` serves the IDL endpoint of
§5.2 (§3 `tls.alpn`). At least TLS 1.2; TLS 1.3 by default.
Cipher suites per `grpc.tls` config.

`grpc.tls.min_version` (`1.2` or `1.3`, default `1.2`) is the lowest
version the listener accepts; TLS 1.3 is negotiated when the client
offers it. `grpc.tls.cipher_suites` is the list of suites by their IANA
names (`TLS_AES_128_GCM_SHA256`, `TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384`;
for TLS 1.3 the rustls spelling `TLS13_…` is taken too), the first named
preferred (the server's order decides); without the key every suite of
the crypto provider of the build (ring, or aws-lc-rs in a FIPS build). A
name the build does not offer, a name given twice, an empty list, or a
list without a suite of a version from `min_version` up is a
configuration error (exit 1, also under `--check-config`).

Cert rotation via SIGHUP.

### §7.2 Auth modes

- `none`: dev mode only (bind 127.0.0.1). On a `grpc.bind` that is not a
  loopback address (127.0.0.0/8, `::1`, `localhost`) the start stops with
  exit 1, with TLS on or off, and `--check-config` refuses it. A SIGHUP
  reload (§9.2) checks `none` against the `grpc.bind` the listener serves,
  since a changed bind needs a restart: a file that moves the bind to
  127.0.0.1 and sets `none` on a daemon bound to another address is
  refused (`reload_refused`), and the live auth settings stay.
- `jwt`: `Authorization: Bearer <token>` metadata, the daemon validates
  RS256/ES256 against `jwt.public_key`, checks `audience` + `required_claim`.
  The key decides the algorithm (RFC 7518 §3.1): an RSA key (PEM PKCS#1 or
  SubjectPublicKeyInfo with rsaEncryption) verifies RS256, a P-256 key
  (SubjectPublicKeyInfo with id-ecPublicKey on prime256v1, RFC 5480)
  verifies ES256 with the 64-octet `R || S` signature of RFC 7518 §3.4. A
  token whose header names another `alg` than the key's is
  UNAUTHENTICATED (RFC 8725 §3.1); a key of another curve stops the start.
- `bearer`: a fixed token (`auth.bearer_token`) and the subject it stands
  for (§3).
- `mtls`: client certificate auth, subject DN as identity.

### §7.3 ACL

Per topic in the config:
```yaml
acl:
  publish: ["alice"]
  subscribe: ["*group:engineers*"]
```

Subject resolution: from the JWT `sub` claim or the TLS cert DN.

## §8 Operations + observability

### §8.1 Logging

Structured JSON log on stdout. Fields: `timestamp`, `level`,
`event`, `peer`, `service`, `method`, `dds_topic`, `bytes`,
`grpc_status`, `latency_us`, `stream_id`.

Each line is one JSON object (RFC 8259; text escaped, numbers as numbers)
that begins with `timestamp` (RFC 3339 in UTC with milliseconds), `level`
(`trace`, `debug`, `info`, `warn`, `error`) and `event`. `log_level` (§3,
`--log-level`, §2) is the threshold: a line below it is not written. Every
call that ended is one line with `event` `rpc` and the fields above, at
`info` when it ended OK and at `warn` otherwise, then with `grpc_message`:

- `peer`: the client's address, `ip:port`;
- `service`, `method`: from `:path`, as the client sent them;
- `dds_topic`: the DDS topic of the call (`<tenant>/<name>` for a
  tenant's, §1 L6), empty for a call without one (reflection, a call
  refused before its route);
- `bytes`: the octets of the call's request and response messages, each
  with its 5-octet prefix (§4.4);
- `grpc_status`: the status the call ended with (§4.5); a call the client
  reset, or whose connection closed before its status was sent, is
  `CANCELLED` (1), and so counted in `zerodds_grpc_bridge_rpcs_total`;
- `latency_us`: microseconds from the request HEADERS to the end of the
  call;
- `stream_id`: the HTTP/2 stream of the call.

A request to the IDL endpoint (§5.2) is a line with `event` `http` and
`peer`, `protocol` (`http/1.1` or `h2`), `method`, `path`, `http_status`,
`bytes` (the body) and `latency_us`, at `info` for a 2xx answer and at
`warn` otherwise. The other events (`startup`, `listening`, `accept`,
`auth_failed`, `goaway`, `drain_limit`, `dds_writes_drained`,
`otlp_export_failed` (§8.3), …) carry their own fields; `listening` names
in `addr` the address the listener bound, with port 0 in `grpc.bind` the
port the system picked. A line the daemon cannot write (stdout closed) is lost;
the daemon goes on. The usage text of `--help` and the error a start ends
with (§2) are on stderr.

### §8.2 Prometheus metrics

```
zerodds_grpc_bridge_rpcs_total              counter{service, method, status}
zerodds_grpc_bridge_streams_active          gauge{service}
zerodds_grpc_bridge_messages_in_total       counter{dds_topic}
zerodds_grpc_bridge_messages_out_total      counter{dds_topic}
zerodds_grpc_bridge_bytes_in_total          counter{dds_topic}
zerodds_grpc_bridge_bytes_out_total         counter{dds_topic}
zerodds_grpc_bridge_dds_samples_received    counter{dds_topic}
zerodds_grpc_bridge_dds_samples_published   counter{dds_topic}
zerodds_grpc_bridge_auth_failures_total     counter{reason}
zerodds_grpc_bridge_h2_resets_total         counter{error_code}
zerodds_grpc_bridge_samples_dropped_total   counter{dds_topic}
```

`samples_dropped_total`: the samples a best-effort `Subscribe` stream
dropped (§6).

### §8.3 OTLP spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → spans per RPC; trace propagation via
the W3C traceparent header (gRPC metadata).

With `OTEL_EXPORTER_OTLP_ENDPOINT` the daemon sends the spans of its calls
as OTLP/HTTP JSON to `/v1/traces` of that endpoint, every 5 s until the
signal of the shutdown, and once more at its end (§9.2), with what the
drained calls recorded; the periodic export has then ended and has no
request in flight. The connect to the collector, each write and each read
wait at most 5 s. The end of the shutdown waits at most 5 s for a periodic
export still in flight and the last export together, so the default drain
and this stay within the 40 s stop timeout of the service units; what has
not ended by then is left to the exit, its spans are lost, and an
`otlp_export_failed` line says so. Without the variable, or with it empty, nothing is recorded and
nothing is sent. The value is `http://host[:port]` with an optional
trailing `/`: `host` a name, an IPv4 address or an IPv6 address in
brackets, the port 80 when it is missing (RFC 9110 §4.2.1). The exporter
speaks HTTP/1.1 without TLS and posts to `/v1/traces` at the root of the
endpoint, so another scheme (`https` among them), user information, a
path, a query, a fragment or a port that is not a number from 1 to 65535
stops the start with exit 1 and the value named, and `--check-config`
refuses it (§2). A TLS collector is reached through a local collector or
proxy that speaks `http` to the daemon.

An export that fails (no connection, an answer other than 2xx) loses its
spans and is a line `otlp_export_failed` at `warn` with `err` and
`failures` (the failed exports since the line before): the first failure
at once, then at most one line per minute while exports keep failing; the
export at the end of the shutdown writes its line in any case.

A call that ended (as the `rpc` line of §8.1) is:

- a server span named `<service>/<method>` from the request HEADERS to the
  end of the call, with the attributes of the OpenTelemetry semantic
  conventions for gRPC: `rpc.system` `grpc`, `rpc.service`, `rpc.method`,
  `rpc.grpc.status_code` (an int), and `dds.topic`,
  `network.peer.address`, `network.peer.port` (an int). Its status is
  `Error` (with the `grpc-message`) for UNKNOWN, DEADLINE_EXCEEDED,
  UNIMPLEMENTED, INTERNAL, UNAVAILABLE and DATA_LOSS, and unset for the
  other codes, as the conventions set it for a server;
- for `Publish` and `PublishOne`, a child `dds.publish` from the first to
  the last DDS write of the call, with `dds.topic` and `dds.samples` (an
  int, the samples written);
- for `Subscribe`, a child `dds.subscribe` from the opening of the
  stream's DataReader to the end of the stream, with `dds.topic` and
  `dds.samples` (an int, the samples sent on the stream).

An int attribute is an OTLP `AnyValue` with `intValue`, the others are
`stringValue`.

A request with a `traceparent` header (W3C Trace Context §3.2; the name in
any case) continues the caller's trace: the server span has the trace-id
of the header and the header's parent-id as its parent. Version `00` is
read as the standard writes it (lower-case hex, all-zero ids invalid), a
higher version by the fields of `00` (§3.2.4), version `ff` is invalid. A
header that does not parse, or more than one, is ignored and the call
starts a trace of its own with a random trace-id. A caller that did not
set the sampled flag gets no span (the parent-based sampling of
OpenTelemetry, W3C Trace Context §3.2.2.5.1). `tracestate` is not read: the
daemon makes no HTTP request onwards that could carry it, and its DDS
writes carry no trace context.

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation.
2. TLS cert load.
3. DCPS DomainParticipant init on `domain`.
4. Per topic: register reader+writer (according to `direction`: `in` =
   gRPC client → DDS, one DataWriter for `Publish`; `out` = DDS →
   gRPC client, one DataReader for `Subscribe`; `bidir` = both); auto-
   generate the FileDescriptor for reflection. A `Publish` on a topic
   without a DataWriter and a `Subscribe` on one without a DataReader end with
   `grpc-status = 9` (FAILED_PRECONDITION); a failed DDS write
   with `grpc-status = 14` (UNAVAILABLE).
5. HTTP/2 server bind.
6. Install SIGHUP/SIGTERM/SIGINT handlers.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Send `GOAWAY` with last-stream-id on all HTTP/2 connections.
- Let running RPCs end naturally.
- Drain pending DDS writes.
- Clean up DDS entities.
- Exit 0.

The drain lasts `grpc.drain_secs` (§3, default 30 s), counted from the
signal. Each connection sends `GOAWAY(NO_ERROR)` with the last stream the
client opened at once and stays open (RFC 9113 §6.8): the calls up to that
stream are served as before — a `Subscribe` stream delivers until the
client ends it — and a stream the client opens after the GOAWAY is refused
with `RST_STREAM(REFUSED_STREAM)`. A connection ends when its last call
ended and the answers of its calls went out within the client's
flow-control windows (RFC 9113 §6.9; an answer that waits for the client's
`WINDOW_UPDATE` keeps the connection, as does the body of a GET `/idl/`,
§5.2); calls still open when the drain time is over end with
`grpc-status = 14` (UNAVAILABLE, §4.5), and what still waits for the
client's window then is not sent. The listening socket closes at the
signal: a new connection is refused. Then the daemon waits, within what is left
of the drain time, until the reliable DDS readers acknowledged what its
DataWriters wrote (DDS 1.4 §2.2.2.4.2.15), deletes its DataWriters and
DataReaders (SEDP dispose) and stops its participants (SPDP), sends the
last spans (§8.3, within 5 s), and exits with 0. With `drain_secs: 0` the open calls end with UNAVAILABLE at the
signal.

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart, since the service registry is regenerated). The auth
settings of the file are checked against the settings that wait for a
restart, as they are served (§7.2); a file that does not load, or whose
settings cannot be served so, changes nothing.

## §10 Cross-vendor

The daemon is a normal RTPS peer. gRPC side tested against grpc-go,
grpc-java, grpcurl, ghz (load tester), Bloom-RPC (GUI), tonic (Rust).

Verified in `crates/grpc-bridge/tests/cross_vendor.rs`, in the weekly job
`integration-bridges-vendor-grpc`: grpcurl with the descriptors of the
reflection service; grpc-go (Debian's `golang-google-grpc-dev`), grpc-java
(Debian's `libgrpc-java`, Netty) and tonic with stubs generated from the
shipped `.proto` (§4.2) by their own generators
(`crates/grpc-bridge/tests/interop/build_peers.sh`), each with a
client-streaming `Publish`, `PublishOne`, a status other than OK,
`Catalog` and a `Subscribe` stream against a DDS participant; ghz with 2000
`PublishOne` calls; BloomRPC driven under Xvfb over the DevTools protocol
(`crates/grpc-bridge/tests/interop/bloomrpc_drive.py`); on the DDS side
Cyclone DDS (`crates/grpc-bridge/tests/interop/cyclone_chat.c`).

On the DDS side the daemon's participants take part in SPDP and SEDP like
any other; a peer of another vendor matches the daemon's DataWriters and
DataReaders by topic name, type name and QoS. The daemon carries no type
object for `dds_type` (§4.4: the payload passes through), so a peer
matches by name; whether the type declares a key the daemon takes from
`keyed` (§3), as the entity kind of its endpoints says it. The payload a
peer writes reaches a `Subscribe` stream as it came, with its
encapsulation header; what a `Publish` writes reaches the peer as the
client serialized it.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-grpc-bridged`
- Config default: `/etc/zerodds/grpc-bridged.yaml`
- Systemd unit: `zerodds-grpc-bridged.service`
- launchd plist: `org.zerodds.grpc-bridged.plist`
- Windows service: `ZeroDDSGrpcBridge` (the name
  `packaging/windows/services/Install-Services.ps1` and the MSI register).
  Started by the service control manager, the binary runs as this service:
  `SERVICE_START_PENDING` until it serves (the moment of the `listening`
  event), then `SERVICE_RUNNING`; the controls stop and shutdown are the
  SIGTERM of §9.2 (`SERVICE_STOP_PENDING` with a wait hint of 60 s, the
  drain), `paramchange` (`sc.exe control ZeroDDSGrpcBridge paramchange`) is
  the SIGHUP reload; at the end `SERVICE_STOPPED` with the exit code of §2
  (`ERROR_SERVICE_SPECIFIC_ERROR` and the code for any code but 0). Started
  from a console, Ctrl+C, Ctrl+Break and the closing of the console are the
  SIGTERM.
- Docker: `zerodds/grpc-bridged:1.0`. The image carries the example
  configuration, which serves cleartext h2c on `127.0.0.1:50051` inside
  the container (§4.1); a published port (`-p 50051:50051`) does not reach
  it. Outside the container the daemon is reached with host networking
  (`--network host`), or with TLS on a non-loopback bind: a mounted
  configuration, or the command `--config /etc/zerodds/grpc-bridged.yaml
  --set grpc.bind=0.0.0.0:50051 --set grpc.tls.enabled=true` with the
  certificate and key mounted under `/etc/zerodds/certs`, and an auth mode
  other than `none` (§7.2), for example `--set auth.mode=mtls --set
  grpc.tls.client_ca_file=/etc/zerodds/certs/client-ca.pem`.

Manual: `man 1 zerodds-grpc-bridged` + `man 5 zerodds-grpc-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `http2_codec`, `hpack`, `grpc_status`,
`reflection`, `dds_pump`, `auth`) ≥ 5 tests in `crates/grpc-bridge/`,
`crates/http2/`, `crates/hpack/`. The modules are these files: `config`
`src/bridge_daemon/config.rs`; `http2_codec` `crates/http2/src/` with
`src/frame.rs` and `src/server.rs`; `hpack` `crates/hpack/src/`;
`grpc_status` `src/status.rs`; `reflection` `src/reflection.rs` and
`src/descriptor.rs`; `dds_pump` `src/bridge_daemon/dds_pump.rs`; `auth`
`src/bridge_daemon/auth.rs` and `src/bridge_security.rs` (paths under
`crates/grpc-bridge/` unless named otherwise).

### §12.2 Integration tests

`crates/grpc-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-grpc-bridged` as a subprocess.
- grpcurl as the CLI client for `Publish`/`Subscribe`.
- DDS subscriber in the test process.
- Verify byte-exact round trip.
- Reflection service round trip.

### §12.3 Multi-vendor

tonic client + Cyclone DDS participant + ZeroDDS gRPC bridge in Docker
Compose, run by `tests/compose_e2e.rs`.

The Compose file is `crates/grpc-bridge/tests/compose/docker-compose.yml`:
the bridge from `packaging/docker/grpc-bridged/Dockerfile` (h2c on
127.0.0.1:50051 of its network namespace), a Cyclone DDS participant that
answers each `Chat::Message` with `{ seq, "echo: " text }`, and the tonic
client in the bridge's network namespace, which publishes with
`PublishOne` until the answer comes back on its `Subscribe` stream; the
client's exit code is the result. `tests/compose_e2e.rs` runs it on a host
with Docker (a manual test, the CI runners start no containers);
`tests/cross_vendor.rs` runs the same three as processes in CI.

## §13 Cross-references

- Library: `crates/grpc-bridge/`, `crates/http2/`, `crates/hpack/`.
- gRPC standards: gRPC-PROTOCOL.md (via grpc/grpc), HTTP/2 RFC 7540, HPACK RFC 7541.
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-ws-bridge-1.0`, `zerodds-amqp-bridge-daemon-1.0`,
  `zerodds-corba-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch for bug fixes, minor for additive config/.proto
fields, major for wire protocol changes (e.g. migration to HTTP/3 +
gRPC-over-QUIC).
