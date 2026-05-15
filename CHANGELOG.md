# Changelog

All notable changes to ZeroDDS at the workspace level are documented
here. Per-crate CHANGELOGs in each `crates/<name>/CHANGELOG.md`.

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning 2.0](https://semver.org/).

## [Unreleased]

Reserved for changes after the `1.0.0-rc.2` workspace tag.

## [1.0.0-rc.2] — 2026-05-15

Hotfix release. Fixes two reported bugs in `zerodds-websocket-bridge`
that prevented the `zerodds-ws-bridged` daemon from being usable in
production-style deployments (zeroCollab Wave 2b ran into both).
No other crates changed materially — workspace-shared version bump
keeps versions consistent across all 90 crates.

### Fixed

- `zerodds-websocket-bridge`: `--topic`, `--auth-token`, `--tls-cert`,
  `--tls-key` and `--metrics` CLI flags were parsed but never applied
  to `DaemonConfig`. The daemon booted with `topics=0 auth-mode=none
  tls=off metrics=off` regardless of CLI overrides. Extracted
  `apply_cli_overrides()` with replace-semantics for scalars and
  additive semantics for `--topic` per spec §2. 8 new unit tests.
  (Resolves GitHub issue #1, PR #5.)
- `zerodds-websocket-bridge`: shipped `ws-bridged.yaml.example` used
  the old nested `participant:/websocket:/routes:/observability:`
  schema; the real parser expects flat top-level keys per spec §3.
  Bridge silently ignored the unknown keys and booted with defaults.
  Example rewritten + spec ref corrected (§4 → §3) + `include_str!`-
  based loadback integration test added to fail CI on future drift.
  (Resolves GitHub issue #3, PR #5.)

### Added

- `zerodds-websocket-bridge`: `load_from_str` now emits a stderr
  WARN-line for unknown top-level config keys instead of silently
  ignoring them (forward-compat preserved, drift visible). Was an
  announced follow-up to issue #3.

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate. 90/91 crates RC1-ready under
`docs/release/RC1_GUARDRAILS.md` Definition of Done.

### Added — Layer 0 Foundation

- `dds-foundation` — error types, `PoolBuffer<CAP>`, `BufferPool`,
  fixed-capacity stack-allocated buffers for the hot path.
- `zerodds-monitor` — Prometheus metric registry with
  `serve_prometheus()` HTTP exposition.
- `zerodds-observability-otlp` — OTLP/HTTP/JSON v1.4 exporter for
  traces, metrics (explicit-buckets histograms), and log records.
- `zerodds-time` — OMG Time 1.1 implementation.
- `zerodds-flatdata` — zero-copy memory-mapped storage backend.
- `zerodds-flatdata-derive` — proc-macro derive for flatdata schemas.

### Added — Layer 1 Transport

- `zerodds-transport-udp` — UDP unicast + multicast with PVE-aware
  multicast setup.
- `zerodds-transport-tcp` — TCP transport with connection pooling.
- `zerodds-transport-shm` — shared-memory transport (iceoryx2-compatible).
- `zerodds-transport-tsn` — Time-Sensitive Networking profile (IEEE 802.1).

### Added — Layer 2 Wire Protocol

- `zerodds-rtps` — OMG DDSI-RTPS 2.5 (121/121 normative sections + 3 n/a),
  reliable + best-effort writers, fragmentation, NACK_FRAG, all 8
  submessage types.
- `zerodds-discovery` — SPDP + SEDP, ParameterList encoding,
  cross-vendor live discovery against Cyclone DDS.
- `zerodds-builtin-topics` — DCPSParticipant, DCPSPublication,
  DCPSSubscription, DCPSTopic readers.

### Added — Layer 3 Core Services

- `zerodds-dcps` — OMG DDS DCPS 1.4 (100/100 + 2 n/a), Factory →
  Participant → Pub/Sub → Writer/Reader.
- `zerodds-qos` — all standard QoS policies, validation, XML-Loader.
- `zerodds-types` — OMG DDS-XTypes 1.3 (82/82 + 1 n/a), TypeObject,
  TypeLookup, Assignability.
- `zerodds-security` — OMG DDS-Security 1.2 (50/50 + 3 n/a),
  Auth/AccessControl/Crypto/Logging/DataTagging plugins, CRL, RTPS-AAD.
- `zerodds-sql-filter` — content-filtered topic SQL parser + evaluator.
- `zerodds-listener-callbacks` — listener dispatch with safety guarantees.

### Added — Layer 4 Schema

- `zerodds-cdr` + `zerodds-cdr-derive` — XCDR1 + XCDR2 codec with
  derive-macro for `DdsType`.
- `zerodds-idl` — OMG IDL 4.2 (649/649 + 24 n/a) parser + AST + validator.
- `zerodds-idl-{cpp,csharp,java,rust,ts}` — code generators per language.

### Added — Layer 5 Protocol Bridges

- `zerodds-websocket-bridge` — RFC 6455 + 7692, daemon
  `zerodds-ws-bridged`.
- `zerodds-mqtt-bridge` — OASIS MQTT 5.0, daemon `zerodds-mqtt-bridged`.
- `zerodds-coap-bridge` — RFC 7252 + 7641 + 7959 + 6690, daemon
  `zerodds-coap-bridged`.
- `zerodds-amqp-endpoint` — OASIS AMQP 1.0, daemon
  `zerodds-amqp-bridged`.
- `zerodds-grpc-bridge` — gRPC HTTP/2 + gRPC-Web, service auto-generation
  per topic, gRPC reflection v1alpha, daemon `zerodds-grpc-bridged`.
- `zerodds-corba-dds-bridge` — bidirectional CORBA ↔ DDS bridge with
  GIOP/IIOP, Notify channel, LocateRequest, daemon `zerodds-corba-bridged`.
- `bridge-security` — shared TLS (rustls 0.23) + auth (bearer / JWT-RS256
  / mTLS / SASL-PLAIN) + ACL (wildcard + group matching) + SIGHUP cert
  rotation across all six bridge daemons.
- `zerodds-amqp-bridge` — separate AMQP-bridge crate.
- `zerodds-hpack` + `zerodds-http2` — HTTP/2 + HPACK primitives for
  the gRPC bridge.
- `zerodds-zenoh-bridge` — Zenoh interop bridge.

### Added — Layer 6 Language Bindings

- `zerodds-c-api` — extern "C" API, 185 exported symbols, ABI-snapshot
  test (`abi.snapshot.json`).
- `zerodds-cpp` — C++17 header + ABI bridge.
- `zerodds-cs` — C# / .NET 8 P/Invoke bindings.
- `zerodds-java-jni` — JNI bridge.
- `zerodds-py` — PyO3 Python bindings.
- `zerodds-rs` — pure-Rust convenience layer.
- `zerodds-ts-node` — Node.js NAPI bindings.
- `zerodds-ts-wasm` — browser WASM bindings.
- `sys` — marker crate for the C-FFI foundation.

### Added — Layer 7 Bridging Services

- `zerodds-conformance` — workspace-wide conformance test harness.
- `zerodds-soap` — DDS-SOAP-PSM bridge.
- `zerodds-dlrl` + `zerodds-dlrl-codegen` — OMG DLRL 1.2.
- `zerodds-opcua-gateway` — OPC-UA Pub/Sub gateway.
- `zerodds-rmw-zerodds-shim` + `zerodds-ros2-rmw` — ROS-2 RMW
  implementation for ZeroDDS, REP-2007/2008/2009 service + action
  patterns.
- `zerodds-web` — DDS-Web 1.0.
- `zerodds-xrce` + `zerodds-xrce-agent` + `zerodds-xrce-client` — OMG
  DDS-XRCE 1.0 (82/82 + 13 n/a).

### Added — Layer 8 CORBA + CCM

- `zerodds-corba-codegen` — OMG CORBA 3.3 Annex-A.1 IDL-Mapping.
- `zerodds-corba-cos-event` — OMG CosEventService 1.2.
- `zerodds-corba-csiv2` — OMG CORBA 3.3 Part 2 §10 CSIv2 with SAS
  protocol and TLS mechanism OID.
- `zerodds-corba-giop` — GIOP wire codec for 1.0/1.1/1.2 plus
  Bidirectional GIOP.
- `zerodds-corba-iiop` — IIOP TCP transport.
- `zerodds-corba-ior` — IOR struct, all standard tagged components,
  corbaloc / corbaname URLs.
- `zerodds-corba-cosnaming` — OMG CosNaming 1.3.
- `zerodds-corba-poa` — POA implementation with all 7 policies.
- `zerodds-corba-ir` — Interface Repository.
- `zerodds-ami4ccm` — OMG AMI4CCM 1.1.
- `zerodds-ccm` + `zerodds-corba-ccm` + `zerodds-corba-ccm-lib` +
  `zerodds-corba-ccm-ejb` + `zerodds-corba-dnc` — full OMG CCM 4.0 +
  DDS4CCM 1.1 stack with EJB bridge and Deployment & Configuration.
- `zerodds-rtc` — OMG RTC 1.0.

### Added — Vendor Specifications

Seven Vendor Specifications published in OMG-DDS-stylistic format:

- **DDS-AMQP 1.0** — DDS over OASIS AMQP 1.0.
- **DDS-TS 1.0** — TypeScript PSM for OMG IDL 4.2.
- **DDS-WebSocket-Bridge 1.0**, **DDS-MQTT-Bridge 1.0**,
  **DDS-CoAP-Bridge 1.0**, **DDS-gRPC-Bridge 1.0**,
  **DDS-CORBA-Bridge 1.0**, **DDS-ROS2-Bridge 1.0** — daemon-mode
  bridge specifications with conformance levels L1–L6.
- **ZeroDDS-FFI-Loader 1.0** — cross-language ABI loading specification
  with 185-symbol ABI snapshot baseline.
- **ZeroDDS-Deployment 1.0** — Linux/macOS/Windows deployment with
  systemd / launchd / Windows Service registration.

LaTeX sources under `documentation/specs/`, PDFs auto-built via
`tectonic` on tag push.

### Added — Examples + Demos

- `examples/tutorials/dds-chat/` — 15-chapter curriculum covering
  pub/sub, QoS, security, type evolution, recording, and performance,
  with 9 language ports (Rust, C++, C++/Qt6, C#, Java, Python, TS-Node,
  TS-Browser, Flutter), 7 bridges, and 4 apps (Mobile, Web SPA,
  Qt Desktop, MCU).
- `examples/demos/dds-warehouse/` — 10-station industrial-IoT
  automated high-rack-warehouse demo covering DLRL cache, TimeService,
  RTC robotics, TSN-RT, XRCE sensors, OPC-UA PLC, AMI4CCM,
  CCM/DDS4CCM, CORBA mainframe, COS-Event-Service, DDS-Web, DDS-XML.
- `examples/demos/perf-camera-dds/` — Flutter mobile camera publisher
  → WebSocket bridge → DDS → Qt6 desktop tile-view performance demo.
- `examples/demos/otel/` — OpenTelemetry observability sample with
  Jaeger compose file.

### Added — Packaging

- Linux: `.deb` and `.rpm` packages with systemd units; Arch `PKGBUILD`;
  AppImage with musl-static builds.
- macOS: Homebrew formulae plus `launchd` plists.
- Windows: WiX 4 MSI registering each bridge as a Windows service;
  Scoop and Chocolatey manifests for developer workstations.
- Multi-arch Docker images (`linux/amd64`, `linux/arm64`) with
  `docker-compose.yml` covering all seven bridges plus auxiliary
  brokers.

### Stability

- All `pub` items in RC-tagged crates are stable for `1.0`. Breaking
  changes require a new major version.
- The wire format (XCDR1, XCDR2, RTPS 2.5, GIOP 1.2) is fixed by the
  underlying OMG specifications and will not change in `1.x`.
- Internal APIs (anything `pub(crate)`) and "unstable-" prefixed
  modules are not subject to SemVer.

### Acknowledgements

ZeroDDS exists thanks to the OMG specification authors, the broader
DDS community at Eclipse Cyclone DDS, OpenDDS, eProsima Fast DDS, and
RTI Connext, the Rust language and ecosystem maintainers, and the
mosquitto, RabbitMQ, libcoap, and omniORB teams whose
implementations served as reference points for cross-vendor
verification.
