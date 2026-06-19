# `zerodds-grpc-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-grpc-bridge-1.0.md`

Implementation:

- `crates/grpc-bridge/` — DDS↔gRPC bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-grpc-bridged CLI

**Spec:** §2 — options
`--config`/`--bind`/`--domain`/`--tls-*`/`--reflection`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`grpc`/`auth`/`topics`/`metrics`; ENV
substitution.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` (config
parser), `crates/grpc-bridge/src/server.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `server.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs` (daemon spawn with config).

**Status:** done

## §4 gRPC wire protocol

### §4.1 HTTP/2 setup with ALPN h2

**Spec:** §4.1 — an HTTP/2 server, ALPN `h2`, cleartext h2c only in dev
mode (`bind=127.0.0.1`); SETTINGS frame, GOAWAY on shutdown.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/frame.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.2 Service-definition auto-generation per topic

**Spec:** §4.2 — a `<TopicSlug>Stream` service with
`Publish`/`Subscribe`/`PublishOne`/`Catalog` RPCs;
`Sample`/`PublishAck`/`SubscribeReq`/`CatalogReq`/`CatalogResp`/`TopicEntry`
messages.

**Repo:** `crates/grpc-bridge/src/path.rs` (slug→service-name),
`crates/grpc-bridge/src/service_gen.rs` (cluster-C FileDescriptor
auto-generator), `crates/grpc-bridge/src/server.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `path.rs` and
`service_gen.rs`; `crates/grpc-bridge/tests/bridge_e2e.rs` (FileDescriptor
round-trip via cluster-C).

**Status:** done

### §4.3 RPC flows publish/subscribe

**Spec:** §4.3 — HEADERS/DATA/HEADERS-END_STREAM flow with `:method=POST`,
`:path=/<pkg>.<svc>/<method>`,
content-type/grpc-encoding/grpc-timeout/authorization; streaming per
sample.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/metadata.rs`, `crates/grpc-bridge/src/timeout.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.4 Length-prefix wrapper

**Spec:** §4.4 — a 5-byte header (compr 1B + length 4B BE) + protobuf
sample payload; compr 0=identity / 1=compressed (gzip/deflate via
`grpc-encoding`).

**Repo:** `crates/grpc-bridge/src/frame.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `frame.rs`;
`bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.5 Status mapping OK/INVALID_ARGUMENT/PERMISSION_DENIED/...

**Spec:** §4.5 — gRPC status codes 0/3/4/7/8/13/14/16 → DDS conditions.

**Repo:** `crates/grpc-bridge/src/status.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `status.rs`;
`bridge_e2e.rs::http2_unknown_service_yields_status_5`.

**Status:** done

### §4.6 Reflection service grpc.reflection.v1alpha

**Spec:** §4.6 — the standard reflection service with auto-generated topic
services + sample message types.

**Repo:** `crates/grpc-bridge/src/server.rs` (reflection hook),
`crates/grpc-bridge/src/reflection.rs` (cluster-C reflection RPC fully
wired).

**Tests:** inline `#[cfg(test)] mod tests` in `reflection.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs` (grpcurl-via-reflection test via
cluster-C).

**Status:** done

## §5 Topic mapping

### §5.1 Service-name default Topic→ChatMessageStream

**Spec:** §5.1 — `::` split + CamelCase + suffix `Stream`; package
`zerodds.<segment>.v1`.

**Repo:** `crates/grpc-bridge/src/path.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `path.rs`.

**Status:** done

### §5.2 Type discovery reflection + catalog + IDL endpoint

**Spec:** §5.2 — three paths (reflection service, catalog RPC, HTTP-GET
sidekick).

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/reflection.rs`,
`crates/grpc-bridge/src/service_gen.rs` (cluster-C type discovery via
reflection + catalog RPC + HTTP-GET sidekick).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (type discovery
round-trip via cluster-C).

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → gRPC-behavior map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition
map; `partition_filter` as a subscribe field.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/timeout.rs`, `crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`
(reliable round-trip); a QoS matrix in
`crates/grpc-bridge/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS h2-ALPN + cert rotation

**Spec:** §7.1 — TLS 1.2+ (1.3 default), ALPN `h2`, SIGHUP cert rotation.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/grpc-bridge/tests/security_e2e.rs` (TLS h2-ALPN + SIGHUP
cert rotation via the cluster-B foundation).

**Status:** done

### §7.2 Auth modes none/jwt/mtls

**Spec:** §7.2 — JWT-bearer authorization metadata, mTLS cert DN.

**Repo:** `crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** `crates/grpc-bridge/tests/security_e2e.rs` (JWT + mTLS round-trip
via cluster-B).

**Status:** done

### §7.3 Per-topic ACL

**Spec:** §7.3 — `acl.publish`/`acl.subscribe` lists with subject
resolution.

**Repo:** `crates/grpc-bridge/src/server.rs` (ACL hook),
`crates/grpc-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/grpc-bridge/tests/security_e2e.rs` (ACL enforcement
against a subject matrix via cluster-B).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (log-level args).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/daemon_runtime.rs` (counter/gauge families
cluster-A wire-up).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (`/metrics` endpoint via
cluster-A wire-up).

**Status:** done

### §8.3 OTLP spans + W3C traceparent

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` + trace propagation via a
traceparent header.

**Repo:** `crates/grpc-bridge/src/metadata.rs` (traceparent parsing hook),
`crates/grpc-bridge/src/daemon_runtime.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/grpc-bridge/src/server.rs` (span emit
per RPC).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (daemon spawn with
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer + FileDescriptor →
HTTP/2 bind → signal handler.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`,
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, GOAWAY, RPCs end naturally; SIGHUP
TLS+ACL reload.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/daemon_runtime.rs` (SIGTERM/SIGINT/SIGHUP via the
cluster-A signal handler),
`crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs` (daemon stop),
`crates/grpc-bridge/tests/security_e2e.rs` (SIGHUP reload TLS+ACL).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Spec:** §10 — the daemon is a normal RTPS peer; the gRPC side against
grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic.

**Repo:** `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
RTPS peer; grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-grpc-bridged`; configs/services/Docker;
manuals.

**Repo:** `packaging/linux/systemd/zerodds-grpc-bridged.service`,
`packaging/macos/launchd/org.zerodds.grpc-bridged.plist`,
`packaging/macos/homebrew/zerodds-grpc-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/grpc-bridged/`,
`packaging/linux/configs/grpc-bridged.yaml.example`,
`man/man1/zerodds-grpc-bridged.1`, `man/man5/zerodds-grpc-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/http2_codec/hpack/grpc_status/reflection/dds_pump/auth,
≥ 5 tests each.

**Repo:** `crates/grpc-bridge/src/{frame.rs,metadata.rs,path.rs,server.rs,status.rs,timeout.rs}`
plus `crates/http2/`, `crates/hpack/`.

**Tests:** inline `#[cfg(test)] mod tests` per module.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, grpcurl as the client, byte-exact
round-trip + reflection.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a tonic client + a Cyclone-DDS subscriber + the ZeroDDS
gRPC bridge in compose.

**Repo:** `crates/grpc-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs` (tonic client +
Cyclone-DDS subscriber).

**Status:** done

## §13 Cross-references

### §13 Related library + standards + daemons

**Spec:** §13 — library `crates/grpc-bridge/`/`crates/http2/`/`crates/hpack/`,
gRPC-PROTOCOL.md, RFC 7540/7541, wire format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config/.proto, major=wire-protocol
change (HTTP/3 + gRPC-over-QUIC).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

24 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-grpc-bridge` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
