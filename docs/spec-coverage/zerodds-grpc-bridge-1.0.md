# `zerodds-grpc-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-grpc-bridge-1.0.md`

Implementation:

- `crates/grpc-bridge/` — DDS↔gRPC-Bridge.

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-grpc-bridged CLI

**Spec:** §2 — Optionen `--config`/`--bind`/`--domain`/`--tls-*`/
`--reflection`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
Exit-Codes 0/1/2/3/4.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`grpc`/`auth`/`topics`/`metrics`;
ENV-Substitution.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` (Config-
Parser), `crates/grpc-bridge/src/server.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `server.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs` (Daemon-Spawn mit Config).

**Status:** done

## §4 gRPC-Wire-Protocol

### §4.1 HTTP/2-Setup mit ALPN h2

**Spec:** §4.1 — HTTP/2-Server, ALPN `h2`, Cleartext-h2c nur Dev-Mode
(`bind=127.0.0.1`); SETTINGS-Frame, GOAWAY bei Shutdown.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/frame.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.2 Service-Definition Auto-Generation pro Topic

**Spec:** §4.2 — `<TopicSlug>Stream` Service mit `Publish`/`Subscribe`/
`PublishOne`/`Catalog`-RPCs; `Sample`/`PublishAck`/`SubscribeReq`/
`CatalogReq`/`CatalogResp`/`TopicEntry`-Messages.

**Repo:** `crates/grpc-bridge/src/path.rs` (Slug→Service-Name),
`crates/grpc-bridge/src/service_gen.rs` (Cluster-C
FileDescriptor-Auto-Generator),
`crates/grpc-bridge/src/server.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `path.rs` und
`service_gen.rs`; `crates/grpc-bridge/tests/bridge_e2e.rs`
(FileDescriptor-Roundtrip via Cluster-C).

**Status:** done

### §4.3 RPC-Flows Publish/Subscribe

**Spec:** §4.3 — HEADERS/DATA/HEADERS-END_STREAM-Flow mit `:method=POST`,
`:path=/<pkg>.<svc>/<method>`, content-type/grpc-encoding/grpc-timeout/
authorization; Streaming pro Sample.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/timeout.rs`. **L2-DDS-Anbindung** (Feature
`dds-runtime`): `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`
(`mod bridge_dds`) — `Publish` schreibt die `Sample.payload` via
`DcpsRuntime::write_user_sample_borrowed` auf einen echten DataWriter,
`Subscribe` drained den DataReader-Channel (`UserSample::Alive`). Ersetzt
den vormaligen `FUTURE (L2)`-Echo-Stub.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::dds_publish_then_subscribe_roundtrip`
(gRPC Publish → DataWriter → Loopback-DataReader → gRPC Subscribe,
byte-gleiche Payload durch echtes DDS).

**Status:** done

> **F2b — Java-Multi-Process-Bridge:** Der pure-Java gRPC-Client
> `org.zerodds.bridge.GrpcBridgeClient` (`crates/java-omgdds/java`)
> spricht dieselbe gRPC-Wire (minimaler raw-HTTP/2-Framer über Socket,
> da der Daemon prior-knowledge-h2c ist). Java↔Rust-e2e
> `GrpcBridgeClientTest` + `crates/java-omgdds/run_grpc_bridge_e2e.sh`
> (3/3 grün): Java publisht über gRPC → DDS → Java subscribt zurück.
> Realisiert den listener-callbacks §7.3 „gRPC-Bridge-Pfad (pure-Java
> per Vendor-Extension)".

### §4.4 Length-Prefix-Wrapper

**Spec:** §4.4 — 5-Byte-Header (Compr 1B + Length 4B BE) + Protobuf-
Sample-Payload; Compr 0=identity / 1=compressed (gzip/deflate via
`grpc-encoding`).

**Repo:** `crates/grpc-bridge/src/frame.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `frame.rs`;
`bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.5 Status-Mapping OK/INVALID_ARGUMENT/PERMISSION_DENIED/...

**Spec:** §4.5 — gRPC-Status-Codes 0/3/4/7/8/13/14/16 → DDS-Bedingungen.

**Repo:** `crates/grpc-bridge/src/status.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `status.rs`;
`bridge_e2e.rs::http2_unknown_service_yields_status_5`.

**Status:** done

### §4.6 Reflection-Service grpc.reflection.v1alpha

**Spec:** §4.6 — Standard-Reflection-Service mit auto-generierten
Topic-Services + Sample-Message-Types.

**Repo:** `crates/grpc-bridge/src/server.rs` (Reflection-Hook),
`crates/grpc-bridge/src/reflection.rs` (Cluster-C Reflection-RPC
voll wired).

**Tests:** Inline `#[cfg(test)] mod tests` in `reflection.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs` (grpcurl-via-Reflection-Test
via Cluster-C).

**Status:** done

## §5 Topic-Mapping

### §5.1 Service-Namen-Default Topic→ChatMessageStream

**Spec:** §5.1 — `::`-Split + CamelCase + Suffix `Stream`; Package
`zerodds.<segment>.v1`.

**Repo:** `crates/grpc-bridge/src/path.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `path.rs`.

**Status:** done

### §5.2 Type-Discovery Reflection + Catalog + IDL-Endpoint

**Spec:** §5.2 — drei Pfade (Reflection-Service, Catalog-RPC,
HTTP-GET-Sidekick).

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/reflection.rs`,
`crates/grpc-bridge/src/service_gen.rs` (Cluster-C Type-Discovery via
Reflection + Catalog-RPC + HTTP-GET-Sidekick).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (Type-Discovery
Roundtrip via Cluster-C).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → gRPC-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition Map; `partition_filter` als Subscribe-Field.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/timeout.rs`,
`crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`
(reliable Roundtrip); QoS-Matrix in
`crates/grpc-bridge/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS h2-ALPN + Cert-Rotation

**Spec:** §7.1 — TLS 1.2+ (1.3 default), ALPN `h2`, SIGHUP-Cert-Rotation.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/grpc-bridge/tests/security_e2e.rs` (TLS h2-ALPN +
SIGHUP-Cert-Rotation via Cluster-B-Foundation).

**Status:** done

### §7.2 Auth-Modes none/jwt/mtls

**Spec:** §7.2 — JWT-Bearer-Authorization-Metadata, mTLS-Cert-DN.

**Repo:** `crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** `crates/grpc-bridge/tests/security_e2e.rs` (JWT + mTLS
Roundtrip via Cluster-B).

**Status:** done

### §7.3 Per-Topic-ACL

**Spec:** §7.3 — `acl.publish`/`acl.subscribe` Listen mit Subject-
Resolution.

**Repo:** `crates/grpc-bridge/src/server.rs` (ACL-Hook),
`crates/grpc-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/grpc-bridge/tests/security_e2e.rs` (ACL-
Enforcement gegen Subject-Matrix via Cluster-B).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (log-level Args).

**Status:** done

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/daemon_runtime.rs` (Counter/Gauge-Familien
Cluster-A-Wireup).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (`/metrics`-
Endpoint via Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP-Spans + W3C-Traceparent

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` + Trace-Propagation per
Traceparent-Header.

**Repo:** `crates/grpc-bridge/src/metadata.rs` (Traceparent-Parsing-
Hook), `crates/grpc-bridge/src/daemon_runtime.rs` (OTLP-Init via
`zerodds-observability-otlp`),
`crates/grpc-bridge/src/server.rs` (Span-Emit pro RPC).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (Daemon-Spawn mit
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer + FileDescriptor →
HTTP/2-Bind → Signal-Handler.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`,
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, GOAWAY, RPCs natürlich
beenden; SIGHUP TLS+ACL-Reload.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/daemon_runtime.rs` (SIGTERM/SIGINT/SIGHUP via
Cluster-A-Signal-Handler),
`crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (Daemon-Stop),
`crates/grpc-bridge/tests/security_e2e.rs` (SIGHUP-Reload TLS+ACL).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Spec:** §10 — Daemon ist normaler RTPS-Peer; gRPC-Seite gegen
grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic.

**Repo:** `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor RTPS-Peer; grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic-
Matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-grpc-bridged`; Configs/Services/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-grpc-bridged.service`,
`packaging/macos/launchd/org.zerodds.grpc-bridged.plist`,
`packaging/macos/homebrew/zerodds-grpc-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/grpc-bridged/`,
`packaging/linux/configs/grpc-bridged.yaml.example`,
`man/man1/zerodds-grpc-bridged.1`,
`man/man5/zerodds-grpc-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/http2_codec/hpack/grpc_status/reflection/
dds_pump/auth je ≥ 5 Tests.

**Repo:** `crates/grpc-bridge/src/{frame.rs,metadata.rs,path.rs,server.rs,status.rs,timeout.rs}` plus `crates/http2/`, `crates/hpack/`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, grpcurl als Client, byte-genauer
Roundtrip + Reflection.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — tonic-Client + Cyclone-DDS-Subscriber + ZeroDDS-
gRPC-Bridge im Compose.

**Repo:** `crates/grpc-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs` (tonic-Client +
Cyclone-DDS-Subscriber).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + Standards + Daemons

**Spec:** §13 — Library `crates/grpc-bridge/`/`crates/http2/`/
`crates/hpack/`, gRPC-PROTOCOL.md, RFC 7540/7541, Wire-Format,
Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config/.proto, Major=
Wire-Protocol-Change (HTTP/3 + gRPC-over-QUIC).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

24 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-grpc-bridge` — Tests grün, 0 failed.

Keine offenen Punkte oder Decision-Records — alle Items `done` / `n/a (informative)`.
