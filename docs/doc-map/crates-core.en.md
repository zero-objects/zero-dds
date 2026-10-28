# Core crates

German original: [crates-core.md](crates-core.md)

This page catalogs the core crates under `crates/`: type system and wire, RTPS and discovery, transports, DCPS, observability, DDS profiles, services, and test and verification crates. For each crate it lists purpose, entry points with file path, dependencies from the `Cargo.toml`, users in the workspace, feature flags and specification references. "Users" are packages under `crates/`, `tools/` and `endpoints/` that list the crate under `[dependencies]`; `(dev)` and `(build)` mark dev and build dependencies. Statements that come only from a doc comment are marked "(per doc comment)".

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## Overview

| Group | Crates |
|---|---|
| Type system and wire | foundation, float128, wire, wire-codec, types, qos, flatdata, flatdata-derive, sys, cdr (removed) |
| RTPS and discovery | rtps, discovery |
| Transports | transport, transport-udp, transport-tcp, transport-shm, transport-uds, transport-tsn, iceoryx-cyclone, rt-linux |
| DCPS and configuration | dcps, dcps-async, config, sql-filter, xml, xml-compose, zerodds-xml-wire, time-service |
| Observability | monitor, observability-otlp, inspect-endpoint |
| DDS profiles and add-ons | rpc, dlrl, dlrl-codegen, xrce, xrce-agent, xrce-client, web, spatial-dds, spatial-ros2, zenoh-bridge |
| Services | recorder, recorder-decode, durability-store, durability-store-file, -sqlite, -postgres, -lakehouse, durability-service, durability-service-bin, routing-service |
| Test, verification, build | wire-conformance, wire-probe, conformance, endpoint-e2e, test-domains, chaos-clock-skew, safe-crates-only, lint, zerodds-build |

Workspace version of all crates: `1.0.0-rc.7`, edition 2024, `rust-version = "1.88"` (`Cargo.toml` in the root directory). Crates without their own entry in `[workspace] members` (transport*, sql-filter, inspect-endpoint, iceoryx-cyclone, recorder) are members through path dependencies of `dcps` or `discovery`.

## Type system and wire

### `zerodds-foundation` — `crates/foundation`
- Purpose: Base building blocks without external crates: buffer pools, checksums, observability events, tracing spans, RCU cell. `no_std`-capable, `forbid(unsafe_code)`.
- Entry points (`src/lib.rs`): `PoolBuffer` (`buffer.rs`), `crc32c`, `crc64_xz`, `md5` (`crc.rs`), `Event`, `Sink`, `StderrJsonSink` (`observability.rs`), `Span`, `Histogram`, `TraceId` (`tracing.rs`), `RcuCell` (`rcu.rs`).
- Depends on: no ZeroDDS crate.
- Used by: `rtps`, `dcps`, `monitor`, `observability-otlp`, `rs`, `tools/dashboard`, `safe-crates-only`.
- Features: `std` (default, pulls in `alloc`), `alloc`, `safety` (empty marker).

### `zerodds-float128` — `crates/float128`
- Purpose: IEEE 754 binary128 in software for IDL `long double` (CORBA 3.3 Part 2 §9.3.1.3, XTypes 1.3 Table 31).
- Entry points: `Float128`, `Class` (`float.rs`), `ParseFloat128Error` (`decimal.rs`), `Inexact` (`convert.rs`), x87 conversion (`x87.rs`).
- Depends on: no ZeroDDS crate. Used by: `wire`, `wire-codec`, `types`, `wire-probe`, `tools/wireprobe/wireprobe-zerodds-c-gen`.
- Features: none.

### `zerodds-wire` — `crates/wire`
- Purpose: Runtime that generated encoders call: `Writer`, `Reader`, alignment rules, framing forms. Decides nothing about a type; order, headers and length codes come from the layout plan (`crates/ast-layout`). `#![no_std]`, `forbid(unsafe_code)`.
- Entry points: `Writer` and framing types (`writer.rs`, including `PlCdr1Framing`, `WstringLengthPolicy`, `ParameterRole`), `Reader` (`reader.rs`), `Endianness`, `WireError`, `TryConstruct`/`fit_sequence`/`fit_string` (`try_construct.rs`), `md5` (`digest.rs`).
- Depends on: `float128`. Used by 34 packages, including `types`, `qos`, `rtps`, `discovery`, `dcps`, `wire-codec`, all `corba-*`, `ts-wasm`, `py`, `zerodds-c-api`, `spatial-dds`.
- Features: none (`no_std` unconditionally).
- Spec: `docs/specs/zerodds-ast-1.0.md` §7, XTypes 1.3 §7.4.
- Note: The doc header says "Layer 0. Depends on nothing". The `Cargo.toml` lists `zerodds-float128`.

### `zerodds-wire-codec` — `crates/wire-codec`
- Purpose: Executes a layout plan against a value, in both directions and over any value representation. DHEADER, EMHEADER and length codes are determined by `zerodds_ast_layout`; the codec takes them over.
- Entry points: `encode` (`encode.rs`), `decode` (`decode.rs`), traits `Sink`/`Source` (`sink.rs`, `source.rs`), `canonicalise`/`Difference` (`canonical.rs`), `default_of_type` (`default.rs`), JSON binding (`json.rs`).
- Depends on: `wire`, `ast`, `ast-core`, `ast-layout`, `float128`.
- Used by: `types` (feature `dynamic`), `dcps`, `wire-probe`, `recorder-decode`, `tools/record`, `tools/spy`, `tools/omg-xtypes-test-main`, `proto` (dev).
- Features: `serde-json` (default) — `serde_json::Value` as value representation.

### `zerodds-types` — `crates/types`
- Purpose: XTypes 1.3 type system: `TypeIdentifier`, Minimal/Complete `TypeObject`, `TypeInformation`, assignability, TypeLookup data types, dynamic types.
- Entry points: `TypeIdentifier` (`type_identifier/`), `TypeObject`, `MinimalTypeObject`, `CompleteTypeObject` (`type_object/`), `TypeInformation` (`type_information.rs`), `compute_minimal_hash`/`compute_complete_hash` (`hash.rs`), `TypeMatcher` (`type_matcher.rs`), assignability rules (`assignability.rs`), `TypeRegistry` (`resolve.rs`), `GetTypesRequest` and others (`type_lookup.rs`), `TypeConsistencyEnforcement` (`qos.rs`), `DynamicType`/`DynamicData` (`dynamic/`), bridge from the AST (`from_ast.rs`).
- Depends on: `wire`, `ast`, `ast-core`, optionally `wire-codec`, `float128`.
- Used by 34 packages, including `rtps`, `discovery`, `dcps`, `xml`, `xml-compose`, `rpc`, `routing-service`, `recorder-decode`, `zerodds-build`, `zerodds-c-api`, `idlc`.
- Features: `std`, `alloc`, `dynamic` (default; pulls in `std`, `wire-codec`, `float128`), `safety`.
- Spec: XTypes 1.3 §7.3.1, §7.3.4, §7.3.5, §7.3.6, §7.2.4.

### `zerodds-qos` — `crates/qos`
- Purpose: DDS 1.4 QoS policies (§2.2.3), request/offered compatibility, PL_CDR PID codec (DDSI-RTPS §9.6.3.2).
- Entry points: policies (`policies/`), `check_compatibility`, `CompatibilityResult`, `IncompatibleReason` (`compatibility.rs`), exclusive-ownership selection (`exclusive_ownership.rs`), `Pid` (`pid.rs`), `Duration` (`duration.rs`), default values (`defaults.rs`).
- Depends on: `wire`. Used by 34 packages, including `rtps`, `discovery`, `dcps`, all `durability-*`, `routing-service`, `xml`, `rpc`, bridges.
- Features: `std` (default), `alloc` (mandatory according to a comment in the `Cargo.toml`), `safety`.

### `zerodds-flatdata` — `crates/flatdata`
- Purpose: `unsafe trait FlatStruct` and slot header for same-host pub/sub over shared memory without copying.
- Entry points: `FlatWriter`, `FlatReader`, `FlatSlot`, `FlatSampleRef` (`pubsub.rs`), `SlotBackend` (`backend.rs`), `InMemorySlotAllocator` (`allocator.rs`), `PosixSlotAllocator` (`posix.rs`), `ShmLocator`, `is_same_host` (`locator.rs`), `SlotHeader`, `ReaderMask` (`slot.rs`), iceoryx2 bridge (`iceoryx.rs`).
- Depends on: `flatdata-derive` (dev). Used by: `dcps` (feature `flatdata-integration`), `zerodds-c-api`.
- Features: `std`, `alloc`, `posix-mmap` (default; `shm_open` + `mmap`), `iceoryx2-bridge` (`Iceoryx2Publisher<T>`/`Iceoryx2Subscriber<T>`, ADR 0004).
- Spec: `docs/specs/zerodds-flatdata-1.0.md`, ADR 0003–0005.

### `zerodds-flatdata-derive` — `crates/flatdata-derive`
- Purpose: Proc macro `#[derive(FlatStruct)]`; checks the layout conditions in the macro and generates `unsafe impl FlatStruct`.
- Entry points: `src/lib.rs`. Depends on: no ZeroDDS crate. Used by: `flatdata` (dev).
- Spec: `docs/specs/zerodds-flatdata-1.0.md` §1.2.

### `zerodds-sys` — `crates/sys`
- Purpose: Placeholder. Exports only the constant `REFERENCE_C_API_CRATE = "zerodds-c-api"` (`src/lib.rs:23`). The C interface is in `crates/zerodds-c-api`.
- Depends on: nothing. Used by: `safe-crates-only`.
- Features: `std` (default), `alloc`, `safety`.

### `crates/cdr` — removed
- No file remains under it in the repository (`git ls-files crates/cdr` empty); untracked fuzz leftovers may exist locally. The comments of the root `Cargo.toml` describe its replacement by `crates/wire` ("Its `cdr` half …" at `rtps`, `routing-service`, `zerodds-c-api`). The last user, `crates/iceoryx-cyclone/proofs/dcps-interop`, is ported to `zerodds-wire` (measured: `run.sh` on the Linux test host, READ PASS, WRITE PASS).

## RTPS and discovery

### `zerodds-rtps` — `crates/rtps`
- Purpose: DDSI-RTPS 2.5 messages and state machines: header, submessages, `HistoryCache`, reliable writer/reader, fragmentation, SPDP/SEDP data types, inline QoS.
- Entry points: `decode_datagram` (`datagram.rs:379`), submessages (`submessages.rs`, `submessage_header.rs`), `ReliableWriter` (`reliable_writer.rs:162`), `ReliableReader` (`reliable_reader.rs:261`), `HistoryCache` (`history_cache.rs:505`), `ReaderProxy`/`WriterProxy`, `FragmentAssembler` (`fragment_assembler.rs`), `ParticipantBuiltinTopicData` (`participant_data.rs`), `PublicationBuiltinTopicData` (`publication_data.rs`), `SubscriptionBuiltinTopicData` (`subscription_data.rs`), PID table (`parameter_list.rs`), `Locator`/`Guid` (`wire_types.rs`), vendor behavior `RtpsBehaviour` (`dialect.rs`).
- Depends on: `wire`, `types`, `qos`, `foundation`, optionally `monitor`, `inspect-endpoint`.
- Used by 26 packages, including `transport*`, `discovery`, `dcps`, `security-runtime`, `routing-service`, `rpc`, bridges, `tools/pcap`.
- Features: `std`, `alloc`, `metrics` (default, counters via `monitor`), `inspect` (tap in the `HistoryCache`, off by default), `safety`.
- Note: The doc header in `src/lib.rs` describes an early plan ("Submessages … follow in W2; transport trait + UDP impl in W3"). Submessages exist; the transport trait is in `crates/transport`.

### `zerodds-discovery` — `crates/discovery`
- Purpose: SPDP, SEDP, TypeLookup service and DDS-Security builtin endpoints.
- Entry points: `SpdpBeacon` (`spdp.rs:121`), `SpdpReader` (`spdp.rs:367`), `DiscoveredParticipantsCache` (`spdp.rs:571`), `SedpStack`, `SedpEvents` (`sedp/stack.rs`), SEDP cache/reader/writer (`sedp/`), `TypeLookupServer`, `TypeLookupClient` (`type_lookup/`), `SecurityBuiltinStack`, `StatelessMessageWriter/Reader`, Volatile-Secure (`security/`), `PeerCapabilities` (`capabilities.rs`).
- Depends on: `rtps`, `types`, `qos`, `wire`, `security`, optionally `monitor`; dev: `transport`, `transport-udp`, `security-pki`.
- Used by: `dcps`, `tools/admin`, `tools/snitch`, `safe-crates-only`, `security-runtime` (dev).
- Features: `std`, `alloc`, `metrics` (default), `safety`. Examples: `spdp_demo`, `dump_spdp_beacon`.
- Spec: DDSI-RTPS 2.5 §8.5, §8.5.3, §8.5.4; XTypes 1.3 §7.6.3.3.4; DDS-Security 1.2 §7.4.2.

## Transports

### `zerodds-transport` — `crates/transport`
- Purpose: Trait `Transport` between the RTPS state machines and the concrete protocol; `no_std` + `alloc`.
- Entry points: `Transport` (`src/lib.rs:161`) with `send`, `recv`, `local_locator`, `local_locators`, `recv_batch`; `ReceivedDatagram`, `SendError`, `RecvError`; re-export of `Locator` from `rtps`.
- Depends on: `rtps`. Used by: all `transport-*`, `dcps`, `tools/bench-suite`, `tools/isolation-smoke`, `safe-crates-only`, `discovery` (dev).
- Features: `std` (default), `alloc`, `safety`.

### `zerodds-transport-udp` — `crates/transport-udp`
- Purpose: UDP/IP PSM (DDSI-RTPS 2.5 §9.6.1) for IPv4/IPv6, unicast and multicast.
- Entry points: `UdpTransport` (`udp_transport.rs:266`) with `bind_v4`, `bind_v4_reuse` (`SO_REUSEPORT`), `bind_v6`, `bind_multicast_v4`, `send_iovec`; `UdpTransportConfig` (`udp_transport.rs:112`); interface enumeration (`interfaces.rs`); `recvmmsg` path (`recv_batch.rs`).
- Depends on: `rtps`, `transport`, optionally `monitor`, `inspect-endpoint`. Used by: `dcps`, `tools/bench-suite`, `tools/isolation-smoke`, `safe-crates-only`, `discovery` (dev).
- Features: `std`, `alloc`, `recvmmsg-batch` (default; module only on Linux), `inspect`, `safety`.

### `zerodds-transport-tcp` — `crates/transport-tcp`
- Purpose: RTPS over TCP with length-prefix framing and an optional 16-byte handshake (vendor-specific).
- Entry points: `TcpTransport`, `TcpTransportError` (`tcp_transport.rs:272`), `accept_one`, framing (`framing.rs`), `BindConnectionRequest/Response` (`handshake.rs`).
- Depends on: `rtps`, `transport`. Used by: `dcps` (`UserTransportKind::TcpV4/TcpV6`), `tools/bench-suite`.
- Features: `std` (default), `alloc`.
- Spec: DDSI-RTPS 2.5 §9.4 (locator kinds `TCPv4 = 4`, `TCPv6 = 8`), `docs/spec-coverage/zerodds-tcp-transport-1.0.md`.

### `zerodds-transport-shm` — `crates/transport-shm`
- Purpose: Cross-process shared memory via `shm_open`/`mmap` with a lock-free single-producer single-consumer ring; 1:1 and role-asymmetric (owner/consumer).
- Entry points: `PosixShmTransport`, `ShmConfig`, `ShmRole`, `PosixShmError` (`posix.rs:1487`, `1237`, `1459`, `1266`).
- Depends on: `rtps`, `transport`. Used by: `dcps` (feature `same-host-shm`, `shm_user.rs`, `same_host_shm.rs`), `tools/bench-suite`, `tools/isolation-smoke`.
- Features: `std` (default), `alloc`.
- Spec: `docs/spec-coverage/zerodds-shm-transport-1.0.md`; locator kind `LOCATOR_KIND_SHM` in `crates/rtps/src/wire_types.rs`.

### `zerodds-transport-uds` — `crates/transport-uds`
- Purpose: Unix domain socket transport (`SOCK_DGRAM`) for container IPC, with a file system path or an abstract namespace.
- Entry points: `UdsTransport`, `UdsConfig`, `socket_path` (`src/lib.rs`), `UdsAbstractDgramTransport`, `UdsAddress` (`abstract_dgram.rs`).
- Depends on: `rtps`, `transport`. Used by: `dcps` (feature `same-host-uds`), `tools/bench-suite`, `tools/isolation-smoke`.
- Features: `std` (default), `alloc`.
- Spec: locator kind `LOCATOR_KIND_UDS = 0x01F00002`, `docs/spec-coverage/zerodds-uds-transport-1.0.md`.

### `zerodds-transport-tsn` — `crates/transport-tsn`
- Purpose: DDS-TSN 1.0: configuration model (PIM, §7.2) and DDSI-RTPS Ethernet PSM (Annex A, EtherType `0x88B5`).
- Entry points: PIM types (`pim/`, `stream.rs`, `traffic.rs`, `time_aware.rs`, `network_requirements.rs`), `IEEE802VlanTag` (`vlan_tag.rs`), `DataFrameSpecification` (`data_frame.rs`), Ethernet PSM (`ethernet_psm.rs`), `TsnTransport` (`socket.rs:42`, AF_PACKET).
- Depends on: `rtps`, `transport`. Used by: `dcps` (feature `tsn-live`, `UserTransportKind::Tsn`).
- Features: `std` (default), `alloc`, `live` (Linux, `libc`; empty module on other systems). Example: `tsn_latency`.
- Spec: OMG DDS-TSN 1.0 (`formal/2024-05-16`), `docs/spec-coverage/dds-tsn-1.0.md`.

### `zerodds-iceoryx-cyclone` — `crates/iceoryx-cyclone`
- Purpose: Same-host bridge to Cyclone DDS via iceoryx C++ (POSH) and `libiceoryx_binding_c`, in Cyclone's PSMX chunk format.
- Entry points: `DdsPsmxMetadata`, `iox_event_name` (`src/lib.rs`), `init_runtime`, `CycloneIoxReader`, `CycloneIoxWriter` (`posh.rs`). Binaries `zerodds-cyclone-iox-reader`, `zerodds-cyclone-iox-writer` (`src/bin/`).
- Depends on: no ZeroDDS crate. Used by: `dcps` (feature `cyclone-iox`, there with `posh`).
- Features: `posh` (FFI binding; default empty).

### `zerodds-rt-linux` — `crates/rt-linux`
- Purpose: Adapter for real-time scheduling on Linux: `sched_setattr` (FIFO/RR/DEADLINE), `sched_setaffinity`, `current_scheduler()`. On other systems `io::Error::Unsupported` (per doc comment).
- Entry points: `SchedulerKind`, `SchedulerProfile`, `RunningSchedulerInfo` (`scheduler.rs`), `pin_current_thread_to_cpus` (`affinity.rs`), syscalls (`syscalls.rs`).
- Depends on: nothing. Used by: no package in the workspace. `dcps` sets thread priority and CPU affinity on its own via `libc` (`apply_thread_tuning`, `crates/dcps/src/runtime.rs:21890`).
- Features: `std` (default).

## DCPS and configuration

### `zerodds-dcps` — `crates/dcps`
- Purpose: Public DCPS API (DDS 1.4 §2.2.2) and runtime: entities, threads, matching, durability, WLP, TypeLookup binding, same-host paths.
- Entry points: `DomainParticipantFactory` (`factory.rs`), `DomainParticipant` (`participant.rs`), `Publisher`, `DataWriter` (`publisher.rs`; `write` at line 2186), `Subscriber`, `DataReader` (`subscriber.rs`; `take` at 3539), `Topic`, `ContentFilteredTopic` (`topic.rs`), `DdsType` (`dds_type.rs:55`), `WaitSet`, `ReadCondition`, `QueryCondition`, `GuardCondition` (`condition.rs`), `DcpsRuntime`, `RuntimeConfig`, `UserTransportKind`, `DeliveryMode` (`runtime.rs`), builtin topics (`builtin_topics.rs`, `builtin_subscriber.rs`), WLP (`wlp.rs`), scheduler (`scheduler.rs`), `LayeredUserTransport` (`layered_transport.rs`).
- Depends on (normal): `wire`, `wire-codec`, `ast`, `ast-core`, `ast-layout`, `foundation`, `qos`, `config`, `rtps`, `types`, `transport`, `transport-udp`, `transport-tcp`, `discovery`, `sql-filter`; optionally `transport-shm`, `transport-uds`, `transport-tsn`, `flatdata`, `iceoryx-cyclone`, `inspect-endpoint`, `monitor`, `security`, `security-runtime`, `security-logging`.
- Used by 29 packages, including `dcps-async`, `rpc`, `routing-service`, `durability-service`, `zenoh-bridge`, `zerodds-c-api`, `py`, `rs`, `rmw-zerodds-shim`, all DDS bridges, `tools/spy`, `tools/record`.
- Features: `std`, `alloc`, `metrics` (default), `security`, `fips`, `wolfcrypt`, `flatdata-integration`, `same-host-shm`, `same-host-uds`, `same-host-iceoryx2`, `tsn-live`, `cyclone-iox`, `recvmmsg-batch`, `inspect`, `live-interop`, `safety`.
- Spec: DDS 1.4 §2.2, DDSI-RTPS 2.5 §8.5, XTypes 1.3 §7.6.3.
- Note: The doc header names `cdr` and `idl-rust` (layers 1 and 3) as the underlying layers. Neither crate is in the tree anymore; the `Cargo.toml` lists `wire`, `wire-codec` and the `ast` crates.

### `zerodds-dcps-async` — `crates/dcps-async`
- Purpose: Runtime-neutral async wrappers around the synchronous DCPS API. The newtypes share the `Arc` of their synchronous counterparts (per doc comment).
- Entry points: `AsyncDomainParticipantFactory` (`factory.rs`), participant, publisher, subscriber, writer, reader (`participant.rs`, `publisher.rs`, `subscriber.rs`, `writer.rs`, `reader.rs`).
- Depends on: `dcps`, `qos`. Used by: `rs`.
- Features: `std` (default), `tokio-glue` (optional `tokio`). Benchmark `write_async_vs_sync`.
- Spec: `docs/specs/zerodds-async-1.0.md`, ADR 0002.

### `zerodds-config` — `crates/config`
- Purpose: Layered configuration from a base file, sorted drop-in directories and nested `include:`; each value knows the file and line it came from. Replaces three YAML parser copies in the bridges and the direct env evaluation in `RuntimeConfig::default()` (per doc comment).
- Entry points: `Config`, `Config::origin_of`, `Node`, `Origin`, `ConfigError` (`src/lib.rs`), `Loader`, `MAX_INCLUDE_DEPTH`, `origins` (`load.rs`), `parse_str` (`parse.rs`), layers (`layers.rs`).
- Depends on: nothing. Used by: `dcps`, `zerodds-c-api`, `rmw-zerodds-shim`, `amqp-endpoint`, `coap-bridge`, `corba-dds-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`.
- Features: none. Settings in detail: [config.en.md](config.en.md).

### `zerodds-sql-filter` — `crates/sql-filter`
- Purpose: Parser and evaluator for DDS filter expressions (SQL-92 subset with `%N` parameters).
- Entry points: `parse` (`parser.rs`), `Expr`, `Value` (`ast.rs`), `RowAccess`, `EvalError` (`evaluator.rs`), lexer (`lexer.rs`).
- Depends on: nothing. Used by: `dcps` (`ContentFilteredTopic`, `QueryCondition`), `zerodds-c-api`, `routing-service`, `tools/omg-shape-main`.
- Features: `std` (default), `alloc`, `safety`.
- Spec: DDS 1.4 Annex B.2.1.

### `zerodds-xml` — `crates/xml`
- Purpose: DDS-XML 1.0 loader: well-formedness, IDL PSM mapping, QoS libraries with inheritance, types, domains, participants, applications, samples.
- Entry points: `parse_domain_libraries` (`domain.rs`), `parse_application_libraries` (`application.rs`), `QosLibrary`, `QosProfile` (`qos.rs`), `resolve_profile` (`qos_inheritance.rs`), `QosProfileRegistry` (`registry.rs`), `parse_type_libraries` (`xtypes_parser.rs`), `load_type_libraries_from_uri` (`xsd_loader.rs`), TypeObject bridge (`typeobject_bridge.rs`), `XmlError`.
- Depends on: `qos`, `types`. Used by: `xml-compose`, `rpc`, `xrce` (feature `xml-config`), `tools/xmlc`, `tools/admin`, `tools/omg-xtypes-test-main`.
- Features: `std` (default), `alloc`. Benchmark `parse_hotpaths`.
- Spec: OMG DDS-XML 1.0 (`formal/18-10-01`) §7.1, §7.2.

### `zerodds-xml-compose` — `crates/xml-compose`
- Purpose: DDS-XML type definitions into the same AST that `idl-compose` produces from IDL, including registered TypeObjects. Build time only.
- Entry points: `compose`, `compose_with`, `compose_source`, `compose_all`, `Composed`, `ComposeOptions` (`src/lib.rs`), lowering (`lower.rs`).
- Depends on: `xml`, `ast`, `ast-core`, `types`, `idl-lower`. Used by: `tools/omg-xtypes-test-main`.
- Features: none.

### `zerodds-xml-wire` — `crates/zerodds-xml-wire`
- Purpose: DDS-XML as wire format for topic data (`formal/2019-09-01` §6), parallel to CDR.
- Entry points: `encode_to_xml`, `decode_xml` (`codec.rs`), `XmlParser` (`parser.rs`), `XmlEmitter` (`emitter.rs`), `validate` (`validator.rs`), `XsdGenerator` (`xsd.rs`).
- Depends on: no ZeroDDS crate. Used by: `zerodds-soap`, `conformance`.
- Features: `std` (default), `alloc`.

### `zerodds-time-service` — `crates/time-service`
- Purpose: OMG Time Service 1.1 (`formal/2002-05-07`): data types, UTO, TIO, `TimeService`. `TimerEventService` is excluded (it requires a CORBA event channel, per doc comment).
- Entry points: `TimeT`, `UtcT`, `IntervalT`, `TdfT` (`time_base.rs`), `Uto` (`uto.rs`), `Tio` (`tio.rs`), `TimeService`, `TimeUnavailable` (`service.rs`).
- Depends on: nothing. Used by: no package in the workspace.
- Features: `std` (default), `alloc`.

## Observability

### `zerodds-monitor` — `crates/monitor`
- Purpose: Metrics registry, Prometheus text export and HTTP server, W3C Trace Context PID codec, span schema.
- Entry points: `Registry`, `default_registry` (`registry.rs`), `Counter`, `Gauge`, `LabeledHistogram`, `Labels` (`counter.rs`, `gauge.rs`, `histogram.rs`, `labels.rs`), `render_prometheus` (`prometheus.rs`), `serve_prometheus` (`server.rs`), trace context (`trace_context.rs`), names (`metric_names.rs`, `span_names.rs`).
- Depends on: `foundation`. Used by 16 packages, including `rtps`, `discovery`, `dcps`, `transport-udp`, `security-crypto`, `routing-service`, bridges, `tools/perf`, `tools/dashboard`.
- Features: `std`, `alloc`, `prometheus-server` (default).
- Spec: `docs/specs/zerodds-monitor-1.1.md`.

### `zerodds-observability-otlp` — `crates/observability-otlp`
- Purpose: OTLP/HTTP/JSON export of `foundation::tracing::Span`, `Histogram` and `observability::Event` to `/v1/traces`, `/v1/metrics` and `/v1/logs` (per doc comment; `post("/v1/traces", ..)` seen in `src/lib.rs:168`).
- Entry points: `OtlpConfig`, `OtlpExporter`, `ExportError` (`src/lib.rs`).
- Depends on: `foundation`. Used by: `rmw-zerodds-shim`, `amqp-endpoint`, `coap-bridge`, `corba-dds-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`.
- Features: none.
- Spec: `docs/specs/zerodds-observability-otlp-1.0.md`.

### `zerodds-inspect-endpoint` — `crates/inspect-endpoint`
- Purpose: Debug path for the external Reality Inspector (PDE). Without feature `inspect` the crate exports nothing that touches the data path.
- Entry points: `InspectServer`, `ServerConfig`, `BroadcastHook` (`server.rs`), `register_dcps_tap`, `register_rtps_tap`, `register_transport_tap`, `TapHook` (`tap.rs`), `Frame`, `FrameKind` (`frame.rs`), authentication (`auth.rs`).
- Depends on: no ZeroDDS crate. Used by: `rtps`, `transport-udp`, `dcps` (each with feature `inspect`).
- Features: `inspect` (off by default; pulls in `serde_json`).

## DDS profiles and add-ons

### `zerodds-rpc` — `crates/rpc`
- Purpose: DDS-RPC 1.0 (`formal/16-12-04`) request/reply on DCPS.
- Entry points: `RequesterEndpoint`, `ReplierEndpoint`, `RpcEndpointBuilder` (`endpoint.rs`), requester/replier (`requester.rs`, `replier.rs`), `RequestHeader`, `ReplyHeader`, `SampleIdentity` (`common_types.rs`), `lower_rpc_annotations` (`annotations.rs`), codegen model (`codegen.rs`), `FunctionStub`/`FunctionSkeleton` (`function_call.rs`), topic naming (`topic_naming.rs`), discovery extension (`discovery_ext.rs`).
- Depends on: `dcps`, `ast`, `ast-core`, `qos`, `rtps`, `types`, `xml`. Used by: no package in the workspace (normal).
- Features: `std` (default), `alloc`, `safety`.

### `zerodds-dlrl` — `crates/dlrl`
- Purpose: DLRL (DDS 1.4 §2.1.3, Annex B): object-oriented cache over DCPS with identity, relationships, transactions.
- Entry points: `ObjectCache`, `ObjectRef`, `WeakObjectRef` (`object_cache.rs`), `parse_pragma`, `DlrlPragma` (`pragma.rs`), `Query` (`query.rs`), relationships (`relationship.rs`), subscriptions (`subscription.rs`), transactions (`transaction.rs`), metamodel (`metamodel.rs`).
- Depends on: no ZeroDDS crate. Used by: `dlrl-codegen`.
- Features: `std` (default), `alloc`.

### `zerodds-dlrl-codegen` — `crates/dlrl-codegen`
- Purpose: Code generation per DDS 1.4 Annex B.4 from the pragmas `DCPS_DATA_TYPE`, `DCPS_DATA_KEY`, `DCPS_DLRL_RELATION`. Emits text, without `lang-cst`.
- Entry points: generators `cpp.rs`, `csharp.rs`, `java.rs`, `ts.rs`.
- Depends on: `dlrl`. Used by: no package in the workspace.
- Features: `std` (default, forwards to `dlrl/std`).

### `zerodds-xrce` — `crates/xrce`
- Purpose: DDS-XRCE wire codec (`formal/2020-11-01`) with transport mappings.
- Entry points: header (`header.rs`), submessages (`submessages/`), `ObjectId` (`object_id.rs`), `ObjectStore` (`object_store.rs`), reliable streams (`reliable.rs`), fragmentation (`fragment.rs`), `MulticastDiscovery` (`discovery.rs`), transports UDP, TCP (`XrceTcpClient/Server`), TLS (`XrceTlsClient/Server`), DTLS (`DtlsLayer`), serial (`transport_serial.rs`), `load_xrce_config_from_file` (`xml_config.rs`).
- Depends on: optionally `xml`. Used by: `xrce-agent`, `xrce-client`, `endpoint-e2e`, `endpoints/rust`, `endpoints/golden-gen`, `endpoints/xrce-agent-demo`.
- Features: `std` (default; pulls in `xml-config`), `alloc`, `xml-config`, `dtls` (`webrtc-dtls`, `tokio`), `tls` (`rustls`, `rcgen`), `safety`.

### `zerodds-xrce-agent` — `crates/xrce-agent`
- Purpose: State machine of the XRCE agent (DDS-XRCE 1.0 §7.3) in-process.
- Entry points: `XrceAgent`, `AgentError`, `TraceEvent`, `TraceSink` (`src/lib.rs`).
- Depends on: `xrce`. Used by: no package in the workspace.
- Features: `std` (default), `alloc`, `safety`.
- Note: The doc header calls the crate "DDS participant wrapper". The `Cargo.toml` does not list `dcps`; a binding to a DCPS participant is not in this crate.

### `zerodds-xrce-client` — `crates/xrce-client`
- Purpose: Synchronous XRCE client without callbacks (DDS-XRCE 1.0 §7.2).
- Entry points: `XrceClient<T: ClientTransport>`, `ClientTransport`, `ClientState`, `ClientError` (`src/lib.rs`).
- Depends on: `xrce`. Used by: `endpoints/rust`, `safe-crates-only`.
- Features: `std` (default), `alloc`, `safety`.

### `zerodds-web` — `crates/web`
- Purpose: OMG DDS-WEB 1.0 (`formal/2014-12-01`): WebDDS object model (§7) and REST PSM (§8.3) as a `no_std` + `alloc` library.
- Entry points: `WebDdsRoot`, `Application`, `Client`, `DomainParticipant` (`model.rs`), `DdsBackend`, `enforce` (`bridge.rs`), `Permissions`, `Rule`, `Decision` (`access_control.rs`), REST (`rest.rs`), headers (`headers.rs`), sessions (`session.rs`).
- Depends on: no ZeroDDS crate. Used by: no package in the workspace. The DCPS binding goes through the trait `DdsBackend`.
- Features: `std` (default), `alloc`.

### `zerodds-rtc` — `crates/rtc`

OMG RTC 1.0 (formal/2008-04-04), Lightweight Robotic Technology Component,
local PSM (§5.2, §5.3, §6.3) as a `no_std` library.

- Modules: `return_code` (`ReturnCode_t`), `lifecycle` (`LifeCycleState`,
  `ComponentAction`, state machine), `object` (`LightweightRTObject`),
  `execution` (`ExecutionContext`), `semantics` (periodic, stimulus,
  modes), `resource`.
- Not included: CORBA PSM (§6.5), LwCCM PSM (§6.4); the introspection data
  model (§5.4) is included, the discovery part is not (per
  doc comment, `crates/rtc/src/lib.rs`).
- Features: `std` (default), `alloc`. No other crate depends on it.
- Tests: `crates/rtc/tests/annex_a_operation_surface.rs`.

### `zerodds-spatial-dds` — `crates/spatial-dds`
- Purpose: SpatialDDS 1.6 profile. The types are generated at build time from `idl/spatial_core.idl` (via `idl-parse` → `idl-lower` → `lang-rules`). The IDL is modified relative to upstream; per doc comment, compatibility with peers generated from the unmodified IDL is not assured.
- Entry points: `topic(operator, kind)` (`src/lib.rs`) and the generated types.
- Depends on: `wire`; build: `ast`, `ast-core`, `ast-layout`, `idl-lower`, `idl-parse`, `lang-cst`, `lang-render`, `lang-rules`. Used by: `spatial-ros2`.
- Features: none.
- Contradiction: `crates/spatial-dds` is listed in `[workspace] members`; the closing comment of the same list says both spatial crates are "ported and not here", because `cargo build --workspace` would fail otherwise.

### `zerodds-spatial-ros2` — `crates/spatial-ros2`
- Purpose: Semantic mapping SpatialDDS ↔ ROS 2 messages (`geometry_msgs`, `sensor_msgs`, `vision_msgs`, `tf2_msgs`, `geographic_msgs`).
- Entry points: `framed_pose_to_ros`, `framed_pose_from_ros`, `frame_transform_to_ros`, `tf_message_from` and others (`convert.rs`), naming rules (`naming.rs`).
- Depends on: `spatial-dds`, `wire` (+ build crates as above). Used by: no package in the workspace.
- Features: none. Same membership contradiction as `spatial-dds`.

### `zerodds-zenoh-bridge` — `crates/zenoh-bridge`
- Purpose: Bridge ZeroDDS ↔ Eclipse Zenoh: topics to key expressions, pumps in both directions. Mapping layer without `zenoh`, live path behind a feature.
- Entry points: mapping (`mapping.rs`), `ZenohBridge`, `ZenohBridgeBuilder`, `BridgeError` (`runtime.rs`).
- Depends on: `qos`, optionally `dcps`. Used by: no package in the workspace.
- Features: `std` (default), `zenoh-runtime` (`zenoh`, `tokio`, `dcps`).

## Services

### `zerodds-recorder` — `crates/recorder`
- Purpose: Recording and playback format `.zddsrec`, without ZeroDDS dependency.
- Entry points: `Header`, `Frame`, `FrameView`, `SampleKind` (`format.rs`), `RecordWriter` (`writer.rs`), `RecordReader` (`reader.rs`), `RecordingSession`, `SessionOptions`, `TopicKey` (`session.rs`).
- Used by: `tools/record`, `tools/replay`, `tools/recorder-bridge`, `rs`, `dcps` (dev).
- Features: `std` (default), `alloc`, `safety`.
- Spec: `docs/specs/zddsrec-1.0.md`.

### `zerodds-recorder-decode` — `crates/recorder-decode`
- Purpose: Decodes recorded or received samples into typed output for `zerodds-spy` and `zerodds-record`.
- Entry points: `type_source` (load IDL, resolve type to `DynamicType`, `--type-file`), `wire` (determine encoding), `json_sink.rs`, `sqlite_sink.rs`, `replay_source.rs`.
- Depends on: `types`, `wire-codec`, `ast`, `ast-layout`, `idl-parse`, `idl-lower`. Used by: `tools/record`, `tools/replay`, `tools/spy`.
- Features: none.

### `zerodds-durability-store` — `crates/durability-store`
- Purpose: Storage abstraction of the durability service (DDS 1.4 §2.2.3.4/§2.2.3.5, ADR 0009), without DDS or RTPS dependency.
- Entry points: trait `DurabilityStore` (`store.rs`), `Contract` (`contract.rs`), `DurabilitySample`, `Selector`, `Cursor`, `Page` (`model.rs`), `InMemoryStore` (`memory.rs`), `TieredStore` (`tiered.rs`), `StoreError`.
- Depends on: `qos`. Used by: `durability-service`, `durability-service-bin` and the four adapters.
- Features: none.

### `zerodds-durability-store-file` — `crates/durability-store-file`
- Purpose: `PERSISTENT` adapter with one file per sample: `<root>/<hex(topic)>/<hex(instance_key)>/<sequence>.bin`, marker `.unregistered`.
- Depends on: `durability-store`, `qos`. Used by: `durability-service-bin`, `durability-service` (dev). Features: none.

### `zerodds-durability-store-sqlite` — `crates/durability-store-sqlite`
- Purpose: Default adapter for `PERSISTENT` on sqlite with `journal_mode=WAL`, `synchronous=NORMAL`; primary key `(topic, instance, sequence)`.
- Depends on: `durability-store`, `qos`. Used by: `durability-service-bin`, `durability-service` (dev). Features: none.

### `zerodds-durability-store-postgres` — `crates/durability-store-postgres`
- Purpose: PostgreSQL adapter as a shared, network-reachable store for many producers; same semantics as sqlite.
- Depends on: `durability-store`, `qos`. Used by: `durability-service-bin` (feature `postgres`).
- Features: `timescaledb`.

### `zerodds-durability-store-lakehouse` — `crates/durability-store-lakehouse`
- Purpose: DuckDB adapter: typed table for SQL analysis and `LakehouseStore::export_parquet` for Parquet partitions.
- Depends on: `durability-store`, `qos`. Used by: `durability-service-bin` (feature `lakehouse`).
- Features: `duckdb` (optional `duckdb`; default empty).

### `zerodds-durability-service` — `crates/durability-service`
- Purpose: Durability service as its own participant (ADR 0009): intake via a `RELIABLE/TRANSIENT_LOCAL/KEEP_ALL` reader, storage in a `DurabilityStore`, replay via a `TRANSIENT_LOCAL/KEEP_ALL` writer (per doc comment).
- Entry points: `DurabilityService`, `ServiceError` (`src/lib.rs`).
- Depends on: `dcps`, `durability-store`, `qos`, `types`; dev: `durability-store-file`, `durability-store-sqlite`, `test-domains`. Used by: `durability-service-bin`.
- Features: none.

### `zerodds-durability-service-bin` — `crates/durability-service-bin`
- Purpose: Daemon `zerodds-durability-svc` (`src/main.rs`): one process per domain, store selectable (`--store sqlite|file|lakehouse|postgres`), `--auto` for TRANSIENT/PERSISTENT topics or `--topic`.
- Depends on: `dcps`, `durability-service`, `durability-store`, `-file`, `-sqlite`, optionally `-lakehouse`, `-postgres`.
- Features: `lakehouse`, `postgres`, `postgres-timescaledb`. Options: [cli-options.en.md](cli-options.en.md).

### `zerodds-routing-service` — `crates/routing-service`
- Purpose: Forwarding of samples between domains, topics, QoS profiles and partitions within DDS. A `Route` couples an input endpoint with an output endpoint, optionally with transformation and SQL filter.
- Entry points: `Router::start`, `start_with_types`, `start_with_runtime` (`engine.rs:23–68`), `RouterConfig`, `Route`, `Endpoint` (`config.rs`), XML configuration (`xml.rs`), transformation and body codec (`transform/`, `transform/codec.rs`, `transform/sqlbridge.rs`), `RouteMetrics` (`metrics.rs`). Binary `zerodds-router` (`src/bin/zerodds-router.rs`).
- Depends on: `dcps`, `qos`, `rtps`, `wire`, `types`, `sql-filter`, `monitor`; dev: `test-domains`. Used by: no package in the workspace.
- Features: none.

## Test, verification and build

### `zerodds-wire-conformance` — `crates/wire-conformance`
- Purpose: Compiles the code produced by the generator (`build.rs`, `mod generated`), so that signature mismatches against `wire` show up as compiler errors.
- Depends on: `wire`; build: `ast*`, `idl-*`, `lang-*`. Used by: nobody (test crate). Features: none.

### `zerodds-wire-probe` — `crates/wire-probe`
- Purpose: ZeroDDS side of the interop suite: IDL type and JSON value in, wire bytes out. Works reflectively over the model and the layout plan, without a code generator.
- Entry points: `encode`, `decode`, `dump`, `dump_fastdds`, `fill`, `Vendor`, `ProbeRequest` (`src/lib.rs:107–116`), binary `wireprobe-zerodds` (`src/main.rs`), further binaries in `src/bin/` (`wireprobe-compare`, `wireprobe-dump`, `wireprobe-fill`, `wireprobe-corba-idl`, `wireprobe-ros-package`).
- Depends on: `wire`, `wire-codec`, `float128`, `ast*`, `idl-compose`, `idl-cst`, `idl-parse`, `idl-lower`.
- Used by: `tools/interop-results`, `wireprobe-zerodds-*-gen` (c, cpp, csharp, java, python, typescript); as a dev dependency by `dcps`, `routing-service`, `xrce-agent`, `zenoh-bridge`, `opcua-bridge` and `tools/idlc`.
- Spec: `docs/specs/zerodds-interop-suite-1.0.md`.

### `zerodds-conformance` — `crates/conformance`
- Purpose: Test vectors of external conformance suites as constants, executed against the implementations.
- Entry points: `run_suite`, `TestCase`, `CaseResult` (`src/lib.rs`); modules `autobahn_ws` (RFC 6455/7692), `coap_plugtest`, `h2spec_grpc`, `oasis_mqtt`, `dds_xml_xvendor`.
- Depends on: `coap-bridge`, `grpc-bridge`, `hpack`, `http2`, `mqtt-bridge`, `websocket-bridge`, `zerodds-xml-wire`, `dotnet-build-lock`. Features: `std` (default), `alloc`.

### `zerodds-endpoint-e2e` — `crates/endpoint-e2e`
- Purpose: Rust counterpart for ping-pong tests of the native endpoint SDKs over UDP and XRCE `WRITE_DATA`; decodes with `wire`.
- Entry points: `bind_peer`, `ping_pong`, `xrce_frame`, `xrce_unframe`, `xcdr2_writer`, `xcdr2_reader` (`src/lib.rs`), codegen helpers (`codegen.rs`), binary `src/bin/peer.rs`.
- Depends on: `wire`, `xrce`, `ast-layout`, `idl-parse`, `idl-lower`, `lang-cst`, `lang-rules`; dev: `endpoint-rust`, `dotnet-build-lock`. Features: none.

### `zerodds-test-domains` — `crates/test-domains`
- Purpose: Assigns a DDS domain exclusively to one test process, because the domain number determines the UDP port (RTPS §9.6.1.4.1: `7400 + 250 * domain`).
- Entry points: `lease_domain() -> DomainLease`, `unique_domain()`, `domain_port_window(domain)` (`src/lib.rs`).
- Used by (dev): `dcps`, `zerodds-c-api`, `durability-service`, `durability-service-bin`, `routing-service`, `tools/record`. Features: none.

### `chaos-clock-skew` — `crates/chaos-clock-skew`
- Purpose: `LD_PRELOAD` library (`crate-type = ["cdylib", "rlib"]`) that shifts `clock_gettime()` and `gettimeofday()` by `CHAOS_CLOCK_SKEW_NS` (fixed offset) and `CHAOS_CLOCK_DRIFT_PPM` (drift).
- Depends on: nothing. Used by: no package in the workspace (loaded via `LD_PRELOAD`). Features: none.

### `safe-crates-only` — `crates/safe-crates-only`
- Purpose: Meta crate without logic. Forces the SAFE-classified crates under the `safety` profile into a `no_std` build: `cargo build -p safe-crates-only --no-default-features --features safety`.
- Depends on: `foundation`, `wire`, `types`, `qos`, `rtps`, `discovery`, `transport`, `transport-udp`, `security`, `xrce-client`, `sys`.
- Features: `safety` (forwards `safety` + `alloc` to the listed crates; `wire` has no features).
- Reference: `docs/architecture/04_safety_by_architecture.md`.

### `zerodds-lint` — `crates/lint`
- Purpose: Project lints based on the AST, invoked with `cargo run -p zerodds-lint -- check`. Per doc comment, it runs in CI (`.gitlab-ci.yml`, job `zerodds-lint`) and in the pre-commit hook (`scripts/pre-commit.sh`); not intended for crates.io.
- Entry points: binary `zerodds-lint` (`src/bin/zerodds-lint.rs`), `runner.rs`, `scanner.rs`, `classification.rs`, lints in `src/lints/`: `bounded_recursion`, `no_alloc_in_hot_path`, `no_dyn_in_safe`, `no_panic_in_safe`, `no_realloc_in_hot_path`, `require_safety_comment`, `safety_classification_present`.
- Depends on: no ZeroDDS crate. Features: none (`default = []`).
- Reference: `docs/architecture/04_safety_by_architecture.md` §3.4.

### `zerodds-build` — `crates/zerodds-build`
- Purpose: Helper for `build.rs`: compiles IDL at build time into Rust types with a `DdsType` implementation.
- Entry points: `Config::new().include_dir(..).compile(&[..])`, `compile`, `Error`, `DefaultExt` (`src/lib.rs`), `dds_type.rs`.
- Depends on: `ast`, `ast-core`, `ast-layout`, `idl-compose`, `idl-parse`, `lang-cst`, `lang-render`, `lang-rules`, `types`; dev: `wire`.
- Used by (build): `tools/bench-suite`, `tools/omg-security-main`, `tools/omg-shape-main`, `tools/omg-xtypes-main`. Features: none.
