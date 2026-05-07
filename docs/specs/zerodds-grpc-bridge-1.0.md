# `zerodds-grpc-bridge` v1.0 — DDS↔gRPC-Bridge-Daemon

ZeroDDS Vendor-Spec. Spezifiziert das Verhalten eines konfigurierbaren
Daemons, der DDS-Topics als gRPC-Services (HTTP/2 Streaming) exponiert.

## Motivation

gRPC ist der De-facto-Standard für moderne Microservice-RPCs:
Kubernetes-Service-Mesh (Istio, Linkerd), Cloud-APIs (Google, AWS),
und Polyglot-Backend-Stacks. ZeroDDS bringt Real-Time-Industriedaten;
der `zerodds-grpc-bridged`-Daemon stellt jedes DDS-Topic als gRPC-
Service `<TopicSlug>Stream` mit `Publish(stream Sample)` /
`Subscribe(SubscribeReq) returns (stream Sample)` zur Verfügung.

Komplementär zu den Libraries `crates/grpc-bridge/`, `crates/http2/`
und `crates/hpack/` — die Libraries sind Building-Blocks (HTTP/2,
HPACK, Protobuf-Codec, gRPC-Status), dieser Daemon ist das
ausführbare Produkt.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Daemon spricht gRPC über HTTP/2 (RFC 7540) + HPACK (RFC 7541) + gRPC-PROTOCOL.md (gRPC-Status, gRPC-Encoding, gRPC-Timeout). |
| **L2 — DDS** | Daemon ist gültiger DDS-DomainParticipant (SPDP/SEDP, Discovery, Liveliness). |
| **L3 — Bridging** | Bidirektional: gRPC-Client→DDS-Publish (Client-Stream) + DDS→gRPC-Push (Server-Stream). |
| **L4 — Config** | Topic-Map per YAML-Config; Hot-Reload via SIGHUP optional. |
| **L5 — Auth** | TLS + ALPN(`h2`) + JWT-Bearer (`Authorization` Metadata) + mTLS. |
| **L6 — Multi-Tenant** | Mehrere DomainParticipants pro Daemon; pro gRPC-Connection eine Tenant-Bindung. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

```
zerodds-grpc-bridged [OPTIONS]

Options:
  --config <FILE>           Path zur Config-File (YAML/JSON/TOML)
  --bind <ADDR>             HTTP/2-Bind-Address (Default 0.0.0.0:50051)
  --domain <ID>             DDS-Domain-ID (Default 0)
  --tls-cert <FILE>         TLS-Cert (PEM) — Pflicht für gRPC-Standard (h2-ALPN)
  --tls-key <FILE>          TLS-Key (PEM)
  --tls-client-ca <FILE>    Client-CA für mTLS (optional)
  --reflection              gRPC-Reflection-Service aktivieren
  --topic <DDS:SVC>         Single-Topic-Override (mehrfach)
  --log-level <LEVEL>       trace/debug/info/warn/error (Default info)
  --metrics <ADDR>          Prometheus-Scrape-Endpoint (Default off)
  --version                 Versions-Info
  --help                    Hilfe

Exit-Codes:
  0   normaler Shutdown (SIGTERM/SIGINT)
  1   Config-Fehler
  2   Bind-Fehler (Port belegt)
  3   DDS-Discovery-Fehler
  4   TLS-Fehler
```

## §3 Config-File-Format

YAML-Schema:

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
    enabled: true                          # gRPC-Server-Reflection (grpc.reflection.v1alpha)

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
  listen: "127.0.0.1:9094"
  path: "/metrics"
```

ENV-Substitution: `${VAR}` und `${VAR:-default}`.

## §4 gRPC-Wire-Protocol

### §4.1 HTTP/2-Setup

Daemon ist HTTP/2-Server mit ALPN `h2`. Cleartext-h2c **nur** im Dev-
Mode (`tls.enabled=false` und `bind=127.0.0.1`).

`SETTINGS`-Frame mit den Werten aus `grpc.*`-Config; `GOAWAY` bei
Shutdown.

### §4.2 Service-Definition (auto-generiert)

Pro Topic generiert Daemon implizit eine .proto-Definition:
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

### §4.3 RPC-Flows

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
  grpc-status = 0  (OK) on success, mit grpc-message bei Fehler
```

**Subscribe (DDS → gRPC)**:
- Client öffnet Server-Streaming-RPC.
- Daemon registriert DataReader, jeder Sample geht als DATA-Frame raus
  (Length-Prefix-Wrapper).
- `grpc-status=0` beim Reader-Cleanup.

### §4.4 Length-Prefix-Wrapper

gRPC-Standard:
```
+--------+----------------+----------------------+
| Compr  | Message Length | Protobuf-Sample      |
| 1 byte | 4 bytes (BE)   | (variable)           |
+--------+----------------+----------------------+
```
Compr-Flag: 0=identity, 1=compressed (gzip/deflate gemäß `grpc-encoding`).

Inside the Protobuf-Sample, das `cdr_payload`-Field enthält die
**vollständige CDR-Bytes inkl. 4-Byte-Encap-Header** wie über RTPS,
gemäß `zerodds-xcdr2-bindings-conformance-1.0` §3.

### §4.5 Status-Mapping

| gRPC-Status | DDS-Side-Bedingung |
|-------------|--------------------|
| `OK (0)` | Sample geschrieben/gestreamt |
| `INVALID_ARGUMENT (3)` | CDR-Decode-Fehler |
| `PERMISSION_DENIED (7)` | ACL-Fehler |
| `UNAUTHENTICATED (16)` | JWT/mTLS-Fehler |
| `RESOURCE_EXHAUSTED (8)` | Send-Queue-Full / DDS-Backpressure |
| `DEADLINE_EXCEEDED (4)` | gRPC-Timeout-Header |
| `UNAVAILABLE (14)` | DDS-Liveliness-Lost / Daemon-Drain |
| `INTERNAL (13)` | unerwarteter Fehler |

### §4.6 Reflection-Service

Wenn `grpc.reflection.enabled: true`, exportiert Daemon das Standard-
`grpc.reflection.v1alpha.ServerReflection`-Service mit den auto-
generierten Topic-Services + Sample-Message-Types.

## §5 Topic-Mapping

### §5.1 Service-Namen-Default

Topic-Name `Chat::Message` → gRPC-Service-Default:
1. `::` → ` ` Split: `["Chat", "Message"]`
2. CamelCase-Concat: `ChatMessage`
3. Suffix: `ChatMessageStream`
4. Package: aus Config oder Default `zerodds.<lowercase-first-segment>.v1`

Override per `grpc_service`/`grpc_package`-Felder.

### §5.2 Type-Discovery

Drei Pfade:
- `grpc.reflection` Service (Standard).
- `Catalog` RPC pro Topic-Service liefert TopicEntry-Liste.
- HTTP-GET (außerhalb gRPC) `https://<bind>/idl/<dds_name>` liefert
  IDL-Quelltext als `text/x-omg-idl` (Sidekick-Endpoint).

## §6 QoS-Translation

gRPC läuft über TCP/HTTP/2 — TCP-Garantien sind gegeben. Daher:

| DDS-QoS | gRPC-Verhalten |
|---------|----------------|
| Reliability `RELIABLE` | unmodifiziert; Backpressure via HTTP/2-Flow-Control |
| Reliability `BEST_EFFORT` | Daemon dropped Samples bei voller `WINDOW_UPDATE`-Queue |
| Durability `VOLATILE` | nur Live-Stream |
| Durability `TRANSIENT_LOCAL` | Daemon hält letzte N Samples + sendet als `initial_burst` beim Subscribe |
| History `KEEP_LAST(N)` | analog |
| Lifespan | als `grpc-timeout` Header bzw. metadata `zerodds-lifespan-ms` |
| Deadline | Daemon emittiert metadata `zerodds-deadline-missed` Trailer |
| Liveliness | HTTP/2 `PING`-Frames per `keepalive_*`-Config |
| Partition | `SubscribeReq.partition_filter` |

## §7 Security

### §7.1 TLS

ALPN `h2` ist gRPC-Standard. Mindestens TLS 1.2; TLS 1.3 default.
Cipher-Suites pro `grpc.tls`-Config.

Cert-Rotation via SIGHUP.

### §7.2 Auth-Modes

- `none`: nur Dev-Mode (bind 127.0.0.1).
- `jwt`: `Authorization: Bearer <token>` Metadata, Daemon validiert
  RS256/ES256 gegen `jwt.public_key`, prüft `audience` + `required_claim`.
- `mtls`: Client-Zertifikat-Auth, Subject-DN als Identity.

### §7.3 ACL

Per Topic im Config:
```yaml
acl:
  publish: ["alice"]
  subscribe: ["*group:engineers*"]
```

Subject-Resolution: aus JWT `sub`-Claim oder TLS-Cert-DN.

## §8 Operations + Observability

### §8.1 Logging

Strukturiertes JSON-Log auf stdout. Felder: `timestamp`, `level`,
`event`, `peer`, `service`, `method`, `dds_topic`, `bytes`,
`grpc_status`, `latency_us`, `stream_id`.

### §8.2 Prometheus-Metrics

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

### §8.3 OTLP-Spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → Spans pro RPC; Trace-Propagation via
W3C-Traceparent-Header (gRPC-Metadata).

## §9 Lifecycle

### §9.1 Startup

1. Config-Parse + Validation.
2. TLS-Cert-Load.
3. DCPS-DomainParticipant init auf `domain`.
4. Pro Topic: Reader+Writer registrieren (gemäß `direction`); auto-
   generiere FileDescriptor für Reflection.
5. HTTP/2-Server-Bind.
6. SIGHUP/SIGTERM/SIGINT-Handler installieren.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Send `GOAWAY` mit last-stream-id auf alle HTTP/2-Connections.
- Lass laufende RPCs natürlich beenden.
- Drain pending DDS-Writes.
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → Config-Reload (TLS-Cert + ACL hot-update; topic-map-Änderungen
brauchen Restart, da Service-Registry neu generiert).

## §10 Cross-Vendor

Daemon ist normaler RTPS-Peer. gRPC-Seite getestet gegen grpc-go,
grpc-java, grpcurl, ghz (Load-Tester), Bloom-RPC (GUI), tonic (Rust).

Verifiziert in `crates/grpc-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Binary: `zerodds-grpc-bridged`
- Config-Default: `/etc/zerodds/grpc-bridged.yaml`
- Systemd-Unit: `zerodds-grpc-bridged.service`
- launchd-Plist: `org.zerodds.grpc-bridged.plist`
- Win-Service: `ZeroDDSGRPCBridge`
- Docker: `zerodds/grpc-bridged:1.0`

Manual: `man 1 zerodds-grpc-bridged` + `man 5 zerodds-grpc-bridged.yaml`.

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`config`, `http2_codec`, `hpack`, `grpc_status`,
`reflection`, `dds_pump`, `auth`) ≥ 5 Tests in `crates/grpc-bridge/`,
`crates/http2/`, `crates/hpack/`.

### §12.2 Integration-Tests

`crates/grpc-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-grpc-bridged` als Subprocess.
- grpcurl als CLI-Client für `Publish`/`Subscribe`.
- DDS-Subscriber im Test-Process.
- Verify byte-genauer Roundtrip.
- Reflection-Service-Roundtrip.

### §12.3 Multi-Vendor

`tests/cross_vendor.rs`: tonic-Client + Cyclone-DDS-Subscriber +
ZeroDDS-gRPC-Bridge im Docker-Compose.

## §13 Cross-References

- Library: `crates/grpc-bridge/`, `crates/http2/`, `crates/hpack/`.
- gRPC-Standards: gRPC-PROTOCOL.md (über grpc/grpc), HTTP/2 RFC 7540, HPACK RFC 7541.
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Verwandte Daemons: `zerodds-ws-bridge-1.0`, `zerodds-amqp-bridge-daemon-1.0`,
  `zerodds-corba-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch für Bugfixes, Minor für additive Config/.proto-
Felder, Major für Wire-Protocol-Changes (z.B. Migration auf HTTP/3 +
gRPC-over-QUIC).
