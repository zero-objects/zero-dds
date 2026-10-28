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
  tls:
    enabled: true
    cert_file: "/etc/zerodds/grpc-cert.pem"
    key_file:  "/etc/zerodds/grpc-key.pem"
    client_ca_file: "/etc/zerodds/grpc-client-ca.pem"   # mTLS optional
    require_client_cert: false
    alpn: ["h2"]
  reflection:
    enabled: true                          # gRPC server reflection (grpc.reflection.v1alpha)

auth:
  mode: "none"                             # none | jwt | mtls
  jwt:
    public_key: "/etc/zerodds/jwt-pub.pem"
    audience: "zerodds-grpc"
    required_claim: "scope=dds.read"

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    grpc_service: "ChatMessageStream"
    grpc_package: "zerodds.chat.v1"
    direction: "bidir"
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
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
```

ENV substitution: `${VAR}` and `${VAR:-default}`.

## §4 gRPC wire protocol

### §4.1 HTTP/2 setup

The daemon is an HTTP/2 server with ALPN `h2`. Cleartext h2c **only** in dev
mode (`tls.enabled=false` and `bind=127.0.0.1`).

`SETTINGS` frame with the values from the `grpc.*` config; `GOAWAY` on
shutdown.

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

**Subscribe (DDS → gRPC)**:
- The client opens a server-streaming RPC.
- The daemon registers a DataReader; every sample goes out as a DATA frame
  (length-prefix wrapper).
- `grpc-status=0` on reader cleanup.

### §4.4 Length-prefix wrapper

gRPC standard:
```
+--------+----------------+----------------------+
| Compr  | Message Length | Protobuf-Sample      |
| 1 byte | 4 bytes (BE)   | (variable)           |
+--------+----------------+----------------------+
```
Compr flag: 0=identity, 1=compressed (gzip/deflate according to `grpc-encoding`).

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

### §4.6 Reflection service

If `grpc.reflection.enabled: true`, the daemon exports the standard
`grpc.reflection.v1alpha.ServerReflection` service with the auto-
generated topic services + sample message types.

## §5 Topic mapping

### §5.1 Default service name

Topic name `Chat::Message` → default gRPC service:
1. Split on `::` → ` `: `["Chat", "Message"]`
2. CamelCase concat: `ChatMessage`
3. Suffix: `ChatMessageStream`
4. Package: from config or default `zerodds.<lowercase-first-segment>.v1`

Override via the `grpc_service`/`grpc_package` fields.

### §5.2 Type discovery

Three paths:
- `grpc.reflection` service (standard).
- `Catalog` RPC per topic service returns the TopicEntry list.
- HTTP GET (outside gRPC) `https://<bind>/idl/<dds_name>` returns the
  IDL source as `text/x-omg-idl` (sidekick endpoint).

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

## §7 Security

### §7.1 TLS

ALPN `h2` is the gRPC standard. At least TLS 1.2; TLS 1.3 by default.
Cipher suites per `grpc.tls` config.

Cert rotation via SIGHUP.

### §7.2 Auth modes

- `none`: dev mode only (bind 127.0.0.1).
- `jwt`: `Authorization: Bearer <token>` metadata, the daemon validates
  RS256/ES256 against `jwt.public_key`, checks `audience` + `required_claim`.
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
```

### §8.3 OTLP spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → spans per RPC; trace propagation via
the W3C traceparent header (gRPC metadata).

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

SIGHUP → config reload (TLS cert + ACL hot update; topic map changes
need a restart, since the service registry is regenerated).

## §10 Cross-vendor

The daemon is a normal RTPS peer. gRPC side tested against grpc-go,
grpc-java, grpcurl, ghz (load tester), Bloom-RPC (GUI), tonic (Rust).

Verified in `crates/grpc-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-grpc-bridged`
- Config default: `/etc/zerodds/grpc-bridged.yaml`
- Systemd unit: `zerodds-grpc-bridged.service`
- launchd plist: `org.zerodds.grpc-bridged.plist`
- Windows service: `ZeroDDSGRPCBridge`
- Docker: `zerodds/grpc-bridged:1.0`

Manual: `man 1 zerodds-grpc-bridged` + `man 5 zerodds-grpc-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `http2_codec`, `hpack`, `grpc_status`,
`reflection`, `dds_pump`, `auth`) ≥ 5 tests in `crates/grpc-bridge/`,
`crates/http2/`, `crates/hpack/`.

### §12.2 Integration tests

`crates/grpc-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-grpc-bridged` as a subprocess.
- grpcurl as the CLI client for `Publish`/`Subscribe`.
- DDS subscriber in the test process.
- Verify byte-exact round trip.
- Reflection service round trip.

### §12.3 Multi-vendor

`tests/cross_vendor.rs`: tonic client + Cyclone DDS subscriber +
ZeroDDS gRPC bridge in Docker Compose.

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
