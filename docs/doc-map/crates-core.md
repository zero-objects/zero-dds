# Kern-Crates

Diese Seite katalogisiert die Kern-Crates unter `crates/`: Typsystem und Wire, RTPS und Discovery, Transporte, DCPS, Beobachtung, DDS-Profile, Dienste sowie Test- und Prüf-Crates. Je Crate stehen Zweck, Einstiegspunkte mit Dateipfad, Abhängigkeiten aus der `Cargo.toml`, Nutzer im Workspace, Feature-Flags und Spezifikationsbezüge. „Nutzer“ sind Pakete unter `crates/`, `tools/` und `endpoints/`, die das Crate als `[dependencies]` nennen; `(dev)` und `(build)` kennzeichnen Dev- und Build-Abhängigkeiten. Aussagen, die nur aus einem Doku-Kommentar stammen, sind mit „(laut Doku-Kommentar)“ markiert.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## Übersicht

| Gruppe | Crates |
|---|---|
| Typsystem und Wire | foundation, float128, wire, wire-codec, types, qos, flatdata, flatdata-derive, sys, cdr (entfernt) |
| RTPS und Discovery | rtps, discovery |
| Transporte | transport, transport-udp, transport-tcp, transport-shm, transport-uds, transport-tsn, iceoryx-cyclone, rt-linux |
| DCPS und Konfiguration | dcps, dcps-async, config, sql-filter, xml, xml-compose, zerodds-xml-wire, time-service |
| Beobachtung | monitor, observability-otlp, inspect-endpoint |
| DDS-Profile und Aufsätze | rpc, dlrl, dlrl-codegen, xrce, xrce-agent, xrce-client, web, spatial-dds, spatial-ros2, zenoh-bridge |
| Dienste | recorder, recorder-decode, durability-store, durability-store-file, -sqlite, -postgres, -lakehouse, durability-service, durability-service-bin, routing-service |
| Test, Prüfung, Build | wire-conformance, wire-probe, conformance, endpoint-e2e, test-domains, chaos-clock-skew, safe-crates-only, lint, zerodds-build |

Workspace-Version aller Crates: `1.0.0-rc.7`, Edition 2024, `rust-version = "1.88"` (`Cargo.toml` im Wurzelverzeichnis). Crates ohne eigenen Eintrag in `[workspace] members` (transport*, sql-filter, inspect-endpoint, iceoryx-cyclone, recorder) sind über Pfadabhängigkeiten von `dcps` bzw. `discovery` Mitglieder.

## Typsystem und Wire

### `zerodds-foundation` — `crates/foundation`
- Zweck: Basisbausteine ohne externe Crates: Pufferpools, Prüfsummen, Beobachtungsereignisse, Tracing-Spans, RCU-Zelle. `no_std`-fähig, `forbid(unsafe_code)`.
- Einstieg (`src/lib.rs`): `PoolBuffer` (`buffer.rs`), `crc32c`, `crc64_xz`, `md5` (`crc.rs`), `Event`, `Sink`, `StderrJsonSink` (`observability.rs`), `Span`, `Histogram`, `TraceId` (`tracing.rs`), `RcuCell` (`rcu.rs`).
- Abhängig von: keinem ZeroDDS-Crate.
- Genutzt von: `rtps`, `dcps`, `monitor`, `observability-otlp`, `rs`, `tools/dashboard`, `safe-crates-only`.
- Features: `std` (Standard, zieht `alloc`), `alloc`, `safety` (leerer Marker).

### `zerodds-float128` — `crates/float128`
- Zweck: IEEE 754 binary128 in Software für IDL `long double` (CORBA 3.3 Teil 2 §9.3.1.3, XTypes 1.3 Table 31).
- Einstieg: `Float128`, `Class` (`float.rs`), `ParseFloat128Error` (`decimal.rs`), `Inexact` (`convert.rs`), x87-Konvertierung (`x87.rs`).
- Abhängig von: keinem ZeroDDS-Crate. Genutzt von: `wire`, `wire-codec`, `types`, `wire-probe`, `tools/wireprobe/wireprobe-zerodds-c-gen`.
- Features: keine.

### `zerodds-wire` — `crates/wire`
- Zweck: Laufzeit, die generierte Encoder aufrufen: `Writer`, `Reader`, Alignment-Regeln, Framing-Formen. Entscheidet nichts über einen Typ; Reihenfolge, Header und Längencodes kommen aus dem Layout-Plan (`crates/ast-layout`). `#![no_std]`, `forbid(unsafe_code)`.
- Einstieg: `Writer` und Framing-Typen (`writer.rs`, u. a. `PlCdr1Framing`, `WstringLengthPolicy`, `ParameterRole`), `Reader` (`reader.rs`), `Endianness`, `WireError`, `TryConstruct`/`fit_sequence`/`fit_string` (`try_construct.rs`), `md5` (`digest.rs`).
- Abhängig von: `float128`. Genutzt von 34 Paketen, u. a. `types`, `qos`, `rtps`, `discovery`, `dcps`, `wire-codec`, alle `corba-*`, `ts-wasm`, `py`, `zerodds-c-api`, `spatial-dds`.
- Features: keine (`no_std` ohne Bedingung).
- Spec: `docs/specs/zerodds-ast-1.0.md` §7, XTypes 1.3 §7.4.
- Hinweis: Der Doku-Kopf sagt „Layer 0. Depends on nothing“. Die `Cargo.toml` nennt `zerodds-float128`.

### `zerodds-wire-codec` — `crates/wire-codec`
- Zweck: Führt einen Layout-Plan gegen einen Wert aus, in beide Richtungen und über beliebige Wertdarstellungen. DHEADER, EMHEADER und Längencodes bestimmt `zerodds_ast_layout`; der Codec übernimmt sie.
- Einstieg: `encode` (`encode.rs`), `decode` (`decode.rs`), Traits `Sink`/`Source` (`sink.rs`, `source.rs`), `canonicalise`/`Difference` (`canonical.rs`), `default_of_type` (`default.rs`), JSON-Anbindung (`json.rs`).
- Abhängig von: `wire`, `ast`, `ast-core`, `ast-layout`, `float128`.
- Genutzt von: `types` (Feature `dynamic`), `dcps`, `wire-probe`, `recorder-decode`, `tools/record`, `tools/spy`, `tools/omg-xtypes-test-main`, `proto` (dev).
- Features: `serde-json` (Standard) — `serde_json::Value` als Wertdarstellung.

### `zerodds-types` — `crates/types`
- Zweck: XTypes-1.3-Typsystem: `TypeIdentifier`, Minimal/Complete `TypeObject`, `TypeInformation`, Assignability, TypeLookup-Datentypen, dynamische Typen.
- Einstieg: `TypeIdentifier` (`type_identifier/`), `TypeObject`, `MinimalTypeObject`, `CompleteTypeObject` (`type_object/`), `TypeInformation` (`type_information.rs`), `compute_minimal_hash`/`compute_complete_hash` (`hash.rs`), `TypeMatcher` (`type_matcher.rs`), Assignability-Regeln (`assignability.rs`), `TypeRegistry` (`resolve.rs`), `GetTypesRequest` u. a. (`type_lookup.rs`), `TypeConsistencyEnforcement` (`qos.rs`), `DynamicType`/`DynamicData` (`dynamic/`), Brücke vom AST (`from_ast.rs`).
- Abhängig von: `wire`, `ast`, `ast-core`, optional `wire-codec`, `float128`.
- Genutzt von 34 Paketen, u. a. `rtps`, `discovery`, `dcps`, `xml`, `xml-compose`, `rpc`, `routing-service`, `recorder-decode`, `zerodds-build`, `zerodds-c-api`, `idlc`.
- Features: `std`, `alloc`, `dynamic` (Standard; zieht `std`, `wire-codec`, `float128`), `safety`.
- Spec: XTypes 1.3 §7.3.1, §7.3.4, §7.3.5, §7.3.6, §7.2.4.

### `zerodds-qos` — `crates/qos`
- Zweck: DDS-1.4-QoS-Policies (§2.2.3), Request/Offered-Kompatibilität, PL_CDR-PID-Codec (DDSI-RTPS §9.6.3.2).
- Einstieg: Policies (`policies/`), `check_compatibility`, `CompatibilityResult`, `IncompatibleReason` (`compatibility.rs`), Exclusive-Ownership-Auswahl (`exclusive_ownership.rs`), `Pid` (`pid.rs`), `Duration` (`duration.rs`), Standardwerte (`defaults.rs`).
- Abhängig von: `wire`. Genutzt von 34 Paketen, u. a. `rtps`, `discovery`, `dcps`, alle `durability-*`, `routing-service`, `xml`, `rpc`, Bridges.
- Features: `std` (Standard), `alloc` (Pflicht laut Kommentar in der `Cargo.toml`), `safety`.

### `zerodds-flatdata` — `crates/flatdata`
- Zweck: `unsafe trait FlatStruct` und Slot-Header für Same-Host-Pub/Sub ohne Kopie über Shared Memory.
- Einstieg: `FlatWriter`, `FlatReader`, `FlatSlot`, `FlatSampleRef` (`pubsub.rs`), `SlotBackend` (`backend.rs`), `InMemorySlotAllocator` (`allocator.rs`), `PosixSlotAllocator` (`posix.rs`), `ShmLocator`, `is_same_host` (`locator.rs`), `SlotHeader`, `ReaderMask` (`slot.rs`), iceoryx2-Brücke (`iceoryx.rs`).
- Abhängig von: `flatdata-derive` (dev). Genutzt von: `dcps` (Feature `flatdata-integration`), `zerodds-c-api`.
- Features: `std`, `alloc`, `posix-mmap` (Standard; `shm_open` + `mmap`), `iceoryx2-bridge` (`Iceoryx2Publisher<T>`/`Iceoryx2Subscriber<T>`, ADR 0004).
- Spec: `docs/specs/zerodds-flatdata-1.0.md`, ADR 0003–0005.

### `zerodds-flatdata-derive` — `crates/flatdata-derive`
- Zweck: Proc-Macro `#[derive(FlatStruct)]`; prüft die Layout-Bedingungen im Makro und erzeugt `unsafe impl FlatStruct`.
- Einstieg: `src/lib.rs`. Abhängig von: keinem ZeroDDS-Crate. Genutzt von: `flatdata` (dev).
- Spec: `docs/specs/zerodds-flatdata-1.0.md` §1.2.

### `zerodds-sys` — `crates/sys`
- Zweck: Platzhalter. Exportiert nur die Konstante `REFERENCE_C_API_CRATE = "zerodds-c-api"` (`src/lib.rs:23`). Die C-Schnittstelle liegt in `crates/zerodds-c-api`.
- Abhängig von: nichts. Genutzt von: `safe-crates-only`.
- Features: `std` (Standard), `alloc`, `safety`.

### `crates/cdr` — entfernt
- Im Repository liegt keine Datei mehr darunter (`git ls-files crates/cdr` leer); lokal können ungetrackte Fuzz-Reste liegen. Die Kommentare der Wurzel-`Cargo.toml` beschreiben die Ablösung durch `crates/wire` („Its `cdr` half …“ bei `rtps`, `routing-service`, `zerodds-c-api`). Der letzte Nutzer, `crates/iceoryx-cyclone/proofs/dcps-interop`, ist auf `zerodds-wire` portiert (gemessen: `run.sh` auf dem Linux-Testhost, READ PASS, WRITE PASS).

## RTPS und Discovery

### `zerodds-rtps` — `crates/rtps`
- Zweck: DDSI-RTPS-2.5-Nachrichten und Zustandsautomaten: Header, Submessages, `HistoryCache`, Reliable Writer/Reader, Fragmentierung, SPDP/SEDP-Datentypen, Inline-QoS.
- Einstieg: `decode_datagram` (`datagram.rs:379`), Submessages (`submessages.rs`, `submessage_header.rs`), `ReliableWriter` (`reliable_writer.rs:162`), `ReliableReader` (`reliable_reader.rs:261`), `HistoryCache` (`history_cache.rs:505`), `ReaderProxy`/`WriterProxy`, `FragmentAssembler` (`fragment_assembler.rs`), `ParticipantBuiltinTopicData` (`participant_data.rs`), `PublicationBuiltinTopicData` (`publication_data.rs`), `SubscriptionBuiltinTopicData` (`subscription_data.rs`), PID-Tabelle (`parameter_list.rs`), `Locator`/`Guid` (`wire_types.rs`), Herstellerverhalten `RtpsBehaviour` (`dialect.rs`).
- Abhängig von: `wire`, `types`, `qos`, `foundation`, optional `monitor`, `inspect-endpoint`.
- Genutzt von 26 Paketen, u. a. `transport*`, `discovery`, `dcps`, `security-runtime`, `routing-service`, `rpc`, Bridges, `tools/pcap`.
- Features: `std`, `alloc`, `metrics` (Standard, Zähler über `monitor`), `inspect` (Tap im `HistoryCache`, Standard aus), `safety`.
- Hinweis: Der Doku-Kopf in `src/lib.rs` beschreibt einen frühen Plan („Submessages … follow in W2; transport trait + UDP impl in W3“). Submessages sind vorhanden; der Transport-Trait liegt in `crates/transport`.

### `zerodds-discovery` — `crates/discovery`
- Zweck: SPDP, SEDP, TypeLookup-Dienst und DDS-Security-Builtin-Endpunkte.
- Einstieg: `SpdpBeacon` (`spdp.rs:121`), `SpdpReader` (`spdp.rs:367`), `DiscoveredParticipantsCache` (`spdp.rs:571`), `SedpStack`, `SedpEvents` (`sedp/stack.rs`), SEDP-Cache/Reader/Writer (`sedp/`), `TypeLookupServer`, `TypeLookupClient` (`type_lookup/`), `SecurityBuiltinStack`, `StatelessMessageWriter/Reader`, Volatile-Secure (`security/`), `PeerCapabilities` (`capabilities.rs`).
- Abhängig von: `rtps`, `types`, `qos`, `wire`, `security`, optional `monitor`; dev: `transport`, `transport-udp`, `security-pki`.
- Genutzt von: `dcps`, `tools/admin`, `tools/snitch`, `safe-crates-only`, `security-runtime` (dev).
- Features: `std`, `alloc`, `metrics` (Standard), `safety`. Beispiele: `spdp_demo`, `dump_spdp_beacon`.
- Spec: DDSI-RTPS 2.5 §8.5, §8.5.3, §8.5.4; XTypes 1.3 §7.6.3.3.4; DDS-Security 1.2 §7.4.2.

## Transporte

### `zerodds-transport` — `crates/transport`
- Zweck: Trait `Transport` zwischen RTPS-Automaten und konkretem Protokoll; `no_std` + `alloc`.
- Einstieg: `Transport` (`src/lib.rs:161`) mit `send`, `recv`, `local_locator`, `local_locators`, `recv_batch`; `ReceivedDatagram`, `SendError`, `RecvError`; Re-Export von `Locator` aus `rtps`.
- Abhängig von: `rtps`. Genutzt von: allen `transport-*`, `dcps`, `tools/bench-suite`, `tools/isolation-smoke`, `safe-crates-only`, `discovery` (dev).
- Features: `std` (Standard), `alloc`, `safety`.

### `zerodds-transport-udp` — `crates/transport-udp`
- Zweck: UDP/IP-PSM (DDSI-RTPS 2.5 §9.6.1) für IPv4/IPv6, Unicast und Multicast.
- Einstieg: `UdpTransport` (`udp_transport.rs:266`) mit `bind_v4`, `bind_v4_reuse` (`SO_REUSEPORT`), `bind_v6`, `bind_multicast_v4`, `send_iovec`; `UdpTransportConfig` (`udp_transport.rs:112`); Interface-Aufzählung (`interfaces.rs`); `recvmmsg`-Pfad (`recv_batch.rs`).
- Abhängig von: `rtps`, `transport`, optional `monitor`, `inspect-endpoint`. Genutzt von: `dcps`, `tools/bench-suite`, `tools/isolation-smoke`, `safe-crates-only`, `discovery` (dev).
- Features: `std`, `alloc`, `recvmmsg-batch` (Standard; Modul nur unter Linux), `inspect`, `safety`.

### `zerodds-transport-tcp` — `crates/transport-tcp`
- Zweck: RTPS über TCP mit Längenpräfix-Framing und optionalem 16-Byte-Handshake (herstellerspezifisch).
- Einstieg: `TcpTransport`, `TcpTransportError` (`tcp_transport.rs:272`), `accept_one`, Framing (`framing.rs`), `BindConnectionRequest/Response` (`handshake.rs`).
- Abhängig von: `rtps`, `transport`. Genutzt von: `dcps` (`UserTransportKind::TcpV4/TcpV6`), `tools/bench-suite`.
- Features: `std` (Standard), `alloc`.
- Spec: DDSI-RTPS 2.5 §9.4 (Locator-Arten `TCPv4 = 4`, `TCPv6 = 8`), `docs/spec-coverage/zerodds-tcp-transport-1.0.md`.

### `zerodds-transport-shm` — `crates/transport-shm`
- Zweck: Prozessübergreifendes Shared Memory über `shm_open`/`mmap` mit lock-freiem Single-Producer-Single-Consumer-Ring; 1:1 und rollenasymmetrisch (Owner/Consumer).
- Einstieg: `PosixShmTransport`, `ShmConfig`, `ShmRole`, `PosixShmError` (`posix.rs:1487`, `1237`, `1459`, `1266`).
- Abhängig von: `rtps`, `transport`. Genutzt von: `dcps` (Feature `same-host-shm`, `shm_user.rs`, `same_host_shm.rs`), `tools/bench-suite`, `tools/isolation-smoke`.
- Features: `std` (Standard), `alloc`.
- Spec: `docs/spec-coverage/zerodds-shm-transport-1.0.md`; Locator-Art `LOCATOR_KIND_SHM` in `crates/rtps/src/wire_types.rs`.

### `zerodds-transport-uds` — `crates/transport-uds`
- Zweck: Unix-Domain-Socket-Transport (`SOCK_DGRAM`) für Container-IPC, mit Dateisystempfad oder abstraktem Namensraum.
- Einstieg: `UdsTransport`, `UdsConfig`, `socket_path` (`src/lib.rs`), `UdsAbstractDgramTransport`, `UdsAddress` (`abstract_dgram.rs`).
- Abhängig von: `rtps`, `transport`. Genutzt von: `dcps` (Feature `same-host-uds`), `tools/bench-suite`, `tools/isolation-smoke`.
- Features: `std` (Standard), `alloc`.
- Spec: Locator-Art `LOCATOR_KIND_UDS = 0x01F00002`, `docs/spec-coverage/zerodds-uds-transport-1.0.md`.

### `zerodds-transport-tsn` — `crates/transport-tsn`
- Zweck: DDS-TSN 1.0: Konfigurationsmodell (PIM, §7.2) und DDSI-RTPS-Ethernet-PSM (Annex A, EtherType `0x88B5`).
- Einstieg: PIM-Typen (`pim/`, `stream.rs`, `traffic.rs`, `time_aware.rs`, `network_requirements.rs`), `IEEE802VlanTag` (`vlan_tag.rs`), `DataFrameSpecification` (`data_frame.rs`), Ethernet-PSM (`ethernet_psm.rs`), `TsnTransport` (`socket.rs:42`, AF_PACKET).
- Abhängig von: `rtps`, `transport`. Genutzt von: `dcps` (Feature `tsn-live`, `UserTransportKind::Tsn`).
- Features: `std` (Standard), `alloc`, `live` (Linux, `libc`; auf anderen Systemen leeres Modul). Beispiel: `tsn_latency`.
- Spec: OMG DDS-TSN 1.0 (`formal/2024-05-16`), `docs/spec-coverage/dds-tsn-1.0.md`.

### `zerodds-iceoryx-cyclone` — `crates/iceoryx-cyclone`
- Zweck: Same-Host-Brücke zu Cyclone DDS über iceoryx C++ (POSH) und `libiceoryx_binding_c`, im PSMX-Chunk-Format von Cyclone.
- Einstieg: `DdsPsmxMetadata`, `iox_event_name` (`src/lib.rs`), `init_runtime`, `CycloneIoxReader`, `CycloneIoxWriter` (`posh.rs`). Binaries `zerodds-cyclone-iox-reader`, `zerodds-cyclone-iox-writer` (`src/bin/`).
- Abhängig von: keinem ZeroDDS-Crate. Genutzt von: `dcps` (Feature `cyclone-iox`, dort mit `posh`).
- Features: `posh` (FFI-Anbindung; Standard leer).

### `zerodds-rt-linux` — `crates/rt-linux`
- Zweck: Adapter für Echtzeit-Scheduling unter Linux: `sched_setattr` (FIFO/RR/DEADLINE), `sched_setaffinity`, `current_scheduler()`. Auf anderen Systemen `io::Error::Unsupported` (laut Doku-Kommentar).
- Einstieg: `SchedulerKind`, `SchedulerProfile`, `RunningSchedulerInfo` (`scheduler.rs`), `pin_current_thread_to_cpus` (`affinity.rs`), Syscalls (`syscalls.rs`).
- Abhängig von: nichts. Genutzt von: keinem Paket im Workspace. `dcps` setzt Thread-Priorität und CPU-Bindung eigenständig über `libc` (`apply_thread_tuning`, `crates/dcps/src/runtime.rs:21890`).
- Features: `std` (Standard).

## DCPS und Konfiguration

### `zerodds-dcps` — `crates/dcps`
- Zweck: Öffentliche DCPS-API (DDS 1.4 §2.2.2) und Laufzeit: Entitäten, Threads, Matching, Durability, WLP, TypeLookup-Anbindung, Same-Host-Pfade.
- Einstieg: `DomainParticipantFactory` (`factory.rs`), `DomainParticipant` (`participant.rs`), `Publisher`, `DataWriter` (`publisher.rs`; `write` bei Zeile 2186), `Subscriber`, `DataReader` (`subscriber.rs`; `take` bei 3539), `Topic`, `ContentFilteredTopic` (`topic.rs`), `DdsType` (`dds_type.rs:55`), `WaitSet`, `ReadCondition`, `QueryCondition`, `GuardCondition` (`condition.rs`), `DcpsRuntime`, `RuntimeConfig`, `UserTransportKind`, `DeliveryMode` (`runtime.rs`), Builtin-Topics (`builtin_topics.rs`, `builtin_subscriber.rs`), WLP (`wlp.rs`), Scheduler (`scheduler.rs`), `LayeredUserTransport` (`layered_transport.rs`).
- Abhängig von (normal): `wire`, `wire-codec`, `ast`, `ast-core`, `ast-layout`, `foundation`, `qos`, `config`, `rtps`, `types`, `transport`, `transport-udp`, `transport-tcp`, `discovery`, `sql-filter`; optional `transport-shm`, `transport-uds`, `transport-tsn`, `flatdata`, `iceoryx-cyclone`, `inspect-endpoint`, `monitor`, `security`, `security-runtime`, `security-logging`.
- Genutzt von 29 Paketen, u. a. `dcps-async`, `rpc`, `routing-service`, `durability-service`, `zenoh-bridge`, `zerodds-c-api`, `py`, `rs`, `rmw-zerodds-shim`, alle DDS-Bridges, `tools/spy`, `tools/record`.
- Features: `std`, `alloc`, `metrics` (Standard), `security`, `fips`, `wolfcrypt`, `flatdata-integration`, `same-host-shm`, `same-host-uds`, `same-host-iceoryx2`, `tsn-live`, `cyclone-iox`, `recvmmsg-batch`, `inspect`, `live-interop`, `safety`.
- Spec: DDS 1.4 §2.2, DDSI-RTPS 2.5 §8.5, XTypes 1.3 §7.6.3.
- Hinweis: Der Doku-Kopf nennt als Unterbau `cdr` und `idl-rust` (Layer 1 und 3). Beide Crates sind nicht mehr im Baum; die `Cargo.toml` nennt `wire`, `wire-codec` und die `ast`-Crates.

### `zerodds-dcps-async` — `crates/dcps-async`
- Zweck: Laufzeitneutrale async-Wrapper um die synchrone DCPS-API. Die Newtypes teilen den `Arc` der synchronen Gegenstücke (laut Doku-Kommentar).
- Einstieg: `AsyncDomainParticipantFactory` (`factory.rs`), Participant, Publisher, Subscriber, Writer, Reader (`participant.rs`, `publisher.rs`, `subscriber.rs`, `writer.rs`, `reader.rs`).
- Abhängig von: `dcps`, `qos`. Genutzt von: `rs`.
- Features: `std` (Standard), `tokio-glue` (optional `tokio`). Benchmark `write_async_vs_sync`.
- Spec: `docs/specs/zerodds-async-1.0.md`, ADR 0002.

### `zerodds-config` — `crates/config`
- Zweck: Geschichtete Konfiguration aus Basisdatei, sortierten Drop-in-Verzeichnissen und verschachtelten `include:`; jeder Wert kennt Datei und Zeile seiner Herkunft. Ersetzt drei YAML-Parser-Kopien der Bridges und die direkte Env-Auswertung in `RuntimeConfig::default()` (laut Doku-Kommentar).
- Einstieg: `Config`, `Config::origin_of`, `Node`, `Origin`, `ConfigError` (`src/lib.rs`), `Loader`, `MAX_INCLUDE_DEPTH`, `origins` (`load.rs`), `parse_str` (`parse.rs`), Schichten (`layers.rs`).
- Abhängig von: nichts. Genutzt von: `dcps`, `zerodds-c-api`, `rmw-zerodds-shim`, `amqp-endpoint`, `coap-bridge`, `corba-dds-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`.
- Features: keine. Einstellungen im Detail: [config.md](config.md).

### `zerodds-sql-filter` — `crates/sql-filter`
- Zweck: Parser und Auswerter für DDS-Filterausdrücke (SQL-92-Teilmenge mit `%N`-Parametern).
- Einstieg: `parse` (`parser.rs`), `Expr`, `Value` (`ast.rs`), `RowAccess`, `EvalError` (`evaluator.rs`), Lexer (`lexer.rs`).
- Abhängig von: nichts. Genutzt von: `dcps` (`ContentFilteredTopic`, `QueryCondition`), `zerodds-c-api`, `routing-service`, `tools/omg-shape-main`.
- Features: `std` (Standard), `alloc`, `safety`.
- Spec: DDS 1.4 Annex B.2.1.

### `zerodds-xml` — `crates/xml`
- Zweck: DDS-XML-1.0-Lader: Wohlgeformtheit, IDL-PSM-Abbildung, QoS-Bibliotheken mit Vererbung, Typen, Domains, Participants, Applications, Samples.
- Einstieg: `parse_domain_libraries` (`domain.rs`), `parse_application_libraries` (`application.rs`), `QosLibrary`, `QosProfile` (`qos.rs`), `resolve_profile` (`qos_inheritance.rs`), `QosProfileRegistry` (`registry.rs`), `parse_type_libraries` (`xtypes_parser.rs`), `load_type_libraries_from_uri` (`xsd_loader.rs`), TypeObject-Brücke (`typeobject_bridge.rs`), `XmlError`.
- Abhängig von: `qos`, `types`. Genutzt von: `xml-compose`, `rpc`, `xrce` (Feature `xml-config`), `tools/xmlc`, `tools/admin`, `tools/omg-xtypes-test-main`.
- Features: `std` (Standard), `alloc`. Benchmark `parse_hotpaths`.
- Spec: OMG DDS-XML 1.0 (`formal/18-10-01`) §7.1, §7.2.

### `zerodds-xml-compose` — `crates/xml-compose`
- Zweck: DDS-XML-Typdefinitionen in denselben AST, den `idl-compose` aus IDL erzeugt, samt registrierter TypeObjects. Nur zur Build-Zeit.
- Einstieg: `compose`, `compose_with`, `compose_source`, `compose_all`, `Composed`, `ComposeOptions` (`src/lib.rs`), Absenkung (`lower.rs`).
- Abhängig von: `xml`, `ast`, `ast-core`, `types`, `idl-lower`. Genutzt von: `tools/omg-xtypes-test-main`.
- Features: keine.

### `zerodds-xml-wire` — `crates/zerodds-xml-wire`
- Zweck: DDS-XML als Wire-Format für Topic-Daten (`formal/2019-09-01` §6), parallel zu CDR.
- Einstieg: `encode_to_xml`, `decode_xml` (`codec.rs`), `XmlParser` (`parser.rs`), `XmlEmitter` (`emitter.rs`), `validate` (`validator.rs`), `XsdGenerator` (`xsd.rs`).
- Abhängig von: keinem ZeroDDS-Crate. Genutzt von: `zerodds-soap`, `conformance`.
- Features: `std` (Standard), `alloc`.

### `zerodds-time-service` — `crates/time-service`
- Zweck: OMG Time Service 1.1 (`formal/2002-05-07`): Datentypen, UTO, TIO, `TimeService`. `TimerEventService` ist ausgenommen (benötigt einen CORBA-Event-Channel, laut Doku-Kommentar).
- Einstieg: `TimeT`, `UtcT`, `IntervalT`, `TdfT` (`time_base.rs`), `Uto` (`uto.rs`), `Tio` (`tio.rs`), `TimeService`, `TimeUnavailable` (`service.rs`).
- Abhängig von: nichts. Genutzt von: keinem Paket im Workspace.
- Features: `std` (Standard), `alloc`.

## Beobachtung

### `zerodds-monitor` — `crates/monitor`
- Zweck: Metrik-Register, Prometheus-Textexport und HTTP-Server, W3C-Trace-Context-PID-Codec, Span-Schema.
- Einstieg: `Registry`, `default_registry` (`registry.rs`), `Counter`, `Gauge`, `LabeledHistogram`, `Labels` (`counter.rs`, `gauge.rs`, `histogram.rs`, `labels.rs`), `render_prometheus` (`prometheus.rs`), `serve_prometheus` (`server.rs`), Trace-Context (`trace_context.rs`), Namen (`metric_names.rs`, `span_names.rs`).
- Abhängig von: `foundation`. Genutzt von 16 Paketen, u. a. `rtps`, `discovery`, `dcps`, `transport-udp`, `security-crypto`, `routing-service`, Bridges, `tools/perf`, `tools/dashboard`.
- Features: `std`, `alloc`, `prometheus-server` (Standard).
- Spec: `docs/specs/zerodds-monitor-1.1.md`.

### `zerodds-observability-otlp` — `crates/observability-otlp`
- Zweck: OTLP/HTTP/JSON-Export von `foundation::tracing::Span`, `Histogram` und `observability::Event` an `/v1/traces`, `/v1/metrics` und `/v1/logs` (laut Doku-Kommentar; `post("/v1/traces", ..)` in `src/lib.rs:168` gesehen).
- Einstieg: `OtlpConfig`, `OtlpExporter`, `ExportError` (`src/lib.rs`).
- Abhängig von: `foundation`. Genutzt von: `rmw-zerodds-shim`, `amqp-endpoint`, `coap-bridge`, `corba-dds-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`.
- Features: keine.
- Spec: `docs/specs/zerodds-observability-otlp-1.0.md`.

### `zerodds-inspect-endpoint` — `crates/inspect-endpoint`
- Zweck: Debug-Pfad für den externen Reality Inspector (PDE). Ohne Feature `inspect` exportiert das Crate nichts, was den Datenpfad berührt.
- Einstieg: `InspectServer`, `ServerConfig`, `BroadcastHook` (`server.rs`), `register_dcps_tap`, `register_rtps_tap`, `register_transport_tap`, `TapHook` (`tap.rs`), `Frame`, `FrameKind` (`frame.rs`), Authentifizierung (`auth.rs`).
- Abhängig von: keinem ZeroDDS-Crate. Genutzt von: `rtps`, `transport-udp`, `dcps` (jeweils Feature `inspect`).
- Features: `inspect` (Standard aus; zieht `serde_json`).

## DDS-Profile und Aufsätze

### `zerodds-rpc` — `crates/rpc`
- Zweck: DDS-RPC 1.0 (`formal/16-12-04`) Request/Reply auf DCPS.
- Einstieg: `RequesterEndpoint`, `ReplierEndpoint`, `RpcEndpointBuilder` (`endpoint.rs`), Requester/Replier (`requester.rs`, `replier.rs`), `RequestHeader`, `ReplyHeader`, `SampleIdentity` (`common_types.rs`), `lower_rpc_annotations` (`annotations.rs`), Codegen-Modell (`codegen.rs`), `FunctionStub`/`FunctionSkeleton` (`function_call.rs`), Topic-Benennung (`topic_naming.rs`), Discovery-Erweiterung (`discovery_ext.rs`).
- Abhängig von: `dcps`, `ast`, `ast-core`, `qos`, `rtps`, `types`, `xml`. Genutzt von: keinem Paket im Workspace (normal).
- Features: `std` (Standard), `alloc`, `safety`.

### `zerodds-dlrl` — `crates/dlrl`
- Zweck: DLRL (DDS 1.4 §2.1.3, Annex B): objektorientierter Cache über DCPS mit Identität, Beziehungen, Transaktionen.
- Einstieg: `ObjectCache`, `ObjectRef`, `WeakObjectRef` (`object_cache.rs`), `parse_pragma`, `DlrlPragma` (`pragma.rs`), `Query` (`query.rs`), Beziehungen (`relationship.rs`), Abonnements (`subscription.rs`), Transaktionen (`transaction.rs`), Metamodell (`metamodel.rs`).
- Abhängig von: keinem ZeroDDS-Crate. Genutzt von: `dlrl-codegen`.
- Features: `std` (Standard), `alloc`.

### `zerodds-dlrl-codegen` — `crates/dlrl-codegen`
- Zweck: Codegenerierung nach DDS 1.4 Annex B.4 aus den Pragmas `DCPS_DATA_TYPE`, `DCPS_DATA_KEY`, `DCPS_DLRL_RELATION`. Gibt Text aus, ohne `lang-cst`.
- Einstieg: Generatoren `cpp.rs`, `csharp.rs`, `java.rs`, `ts.rs`.
- Abhängig von: `dlrl`. Genutzt von: keinem Paket im Workspace.
- Features: `std` (Standard, reicht an `dlrl/std` weiter).

### `zerodds-xrce` — `crates/xrce`
- Zweck: DDS-XRCE-Wire-Codec (`formal/2020-11-01`) mit Transportabbildungen.
- Einstieg: Header (`header.rs`), Submessages (`submessages/`), `ObjectId` (`object_id.rs`), `ObjectStore` (`object_store.rs`), Reliable Streams (`reliable.rs`), Fragmentierung (`fragment.rs`), `MulticastDiscovery` (`discovery.rs`), Transporte UDP, TCP (`XrceTcpClient/Server`), TLS (`XrceTlsClient/Server`), DTLS (`DtlsLayer`), seriell (`transport_serial.rs`), `load_xrce_config_from_file` (`xml_config.rs`).
- Abhängig von: optional `xml`. Genutzt von: `xrce-agent`, `xrce-client`, `endpoint-e2e`, `endpoints/rust`, `endpoints/golden-gen`, `endpoints/xrce-agent-demo`.
- Features: `std` (Standard; zieht `xml-config`), `alloc`, `xml-config`, `dtls` (`webrtc-dtls`, `tokio`), `tls` (`rustls`, `rcgen`), `safety`.

### `zerodds-xrce-agent` — `crates/xrce-agent`
- Zweck: Zustandsautomat des XRCE-Agenten (DDS-XRCE 1.0 §7.3) im Prozess.
- Einstieg: `XrceAgent`, `AgentError`, `TraceEvent`, `TraceSink` (`src/lib.rs`).
- Abhängig von: `xrce`. Genutzt von: keinem Paket im Workspace.
- Features: `std` (Standard), `alloc`, `safety`.
- Hinweis: Der Doku-Kopf nennt das Crate „DDS participant wrapper“. Die `Cargo.toml` nennt `dcps` nicht; eine Anbindung an einen DCPS-Participant liegt nicht in diesem Crate.

### `zerodds-xrce-client` — `crates/xrce-client`
- Zweck: Synchroner XRCE-Client ohne Callbacks (DDS-XRCE 1.0 §7.2).
- Einstieg: `XrceClient<T: ClientTransport>`, `ClientTransport`, `ClientState`, `ClientError` (`src/lib.rs`).
- Abhängig von: `xrce`. Genutzt von: `endpoints/rust`, `safe-crates-only`.
- Features: `std` (Standard), `alloc`, `safety`.

### `zerodds-web` — `crates/web`
- Zweck: OMG DDS-WEB 1.0 (`formal/2014-12-01`): WebDDS-Objektmodell (§7) und REST-PSM (§8.3) als `no_std` + `alloc`-Bibliothek.
- Einstieg: `WebDdsRoot`, `Application`, `Client`, `DomainParticipant` (`model.rs`), `DdsBackend`, `enforce` (`bridge.rs`), `Permissions`, `Rule`, `Decision` (`access_control.rs`), REST (`rest.rs`), Header (`headers.rs`), Sessions (`session.rs`).
- Abhängig von: keinem ZeroDDS-Crate. Genutzt von: keinem Paket im Workspace. Die DCPS-Anbindung erfolgt über das Trait `DdsBackend`.
- Features: `std` (Standard), `alloc`.

### `zerodds-rtc` — `crates/rtc`

OMG RTC 1.0 (formal/2008-04-04), Lightweight Robotic Technology Component,
lokales PSM (§5.2, §5.3, §6.3) als `no_std`-Bibliothek.

- Module: `return_code` (`ReturnCode_t`), `lifecycle` (`LifeCycleState`,
  `ComponentAction`, Zustandsautomat), `object` (`LightweightRTObject`),
  `execution` (`ExecutionContext`), `semantics` (periodisch, Stimulus,
  Modi), `resource`.
- Nicht enthalten: CORBA-PSM (§6.5), LwCCM-PSM (§6.4); das Datenmodell der
  Introspektion (§5.4) ist enthalten, der Discovery-Teil nicht (laut
  Doku-Kommentar, `crates/rtc/src/lib.rs`).
- Features: `std` (Default), `alloc`. Kein anderes Crate hängt davon ab.
- Tests: `crates/rtc/tests/annex_a_operation_surface.rs`.

### `zerodds-spatial-dds` — `crates/spatial-dds`
- Zweck: SpatialDDS-1.6-Profil. Die Typen werden zur Build-Zeit aus `idl/spatial_core.idl` erzeugt (über `idl-parse` → `idl-lower` → `lang-rules`). Die IDL ist gegenüber Upstream angepasst; Kompatibilität mit unverändert generierten Peers ist laut Doku-Kommentar nicht gesichert.
- Einstieg: `topic(operator, kind)` (`src/lib.rs`) und die generierten Typen.
- Abhängig von: `wire`; build: `ast`, `ast-core`, `ast-layout`, `idl-lower`, `idl-parse`, `lang-cst`, `lang-render`, `lang-rules`. Genutzt von: `spatial-ros2`.
- Features: keine.
- Widerspruch: `crates/spatial-dds` steht in `[workspace] members`; der Schlusskommentar derselben Liste sagt, beide Spatial-Crates seien „ported and not here“, weil `cargo build --workspace` sonst fehlschlage.

### `zerodds-spatial-ros2` — `crates/spatial-ros2`
- Zweck: Semantische Abbildung SpatialDDS ↔ ROS-2-Nachrichten (`geometry_msgs`, `sensor_msgs`, `vision_msgs`, `tf2_msgs`, `geographic_msgs`).
- Einstieg: `framed_pose_to_ros`, `framed_pose_from_ros`, `frame_transform_to_ros`, `tf_message_from` u. a. (`convert.rs`), Namensregeln (`naming.rs`).
- Abhängig von: `spatial-dds`, `wire` (+ Build-Crates wie oben). Genutzt von: keinem Paket im Workspace.
- Features: keine. Gleicher Mitgliedschaftswiderspruch wie `spatial-dds`.

### `zerodds-zenoh-bridge` — `crates/zenoh-bridge`
- Zweck: Brücke ZeroDDS ↔ Eclipse Zenoh: Topics auf Key-Expressions, Pumpen in beide Richtungen. Abbildungsschicht ohne `zenoh`, Live-Pfad hinter Feature.
- Einstieg: Abbildung (`mapping.rs`), `ZenohBridge`, `ZenohBridgeBuilder`, `BridgeError` (`runtime.rs`).
- Abhängig von: `qos`, optional `dcps`. Genutzt von: keinem Paket im Workspace.
- Features: `std` (Standard), `zenoh-runtime` (`zenoh`, `tokio`, `dcps`).

## Dienste

### `zerodds-recorder` — `crates/recorder`
- Zweck: Aufzeichnungs- und Wiedergabeformat `.zddsrec`, ohne ZeroDDS-Abhängigkeit.
- Einstieg: `Header`, `Frame`, `FrameView`, `SampleKind` (`format.rs`), `RecordWriter` (`writer.rs`), `RecordReader` (`reader.rs`), `RecordingSession`, `SessionOptions`, `TopicKey` (`session.rs`).
- Genutzt von: `tools/record`, `tools/replay`, `tools/recorder-bridge`, `rs`, `dcps` (dev).
- Features: `std` (Standard), `alloc`, `safety`.
- Spec: `docs/specs/zddsrec-1.0.md`.

### `zerodds-recorder-decode` — `crates/recorder-decode`
- Zweck: Dekodiert aufgezeichnete oder empfangene Samples in typisierte Ausgaben für `zerodds-spy` und `zerodds-record`.
- Einstieg: `type_source` (IDL laden, Typ zu `DynamicType` auflösen, `--type-file`), `wire` (Kodierung bestimmen), `json_sink.rs`, `sqlite_sink.rs`, `replay_source.rs`.
- Abhängig von: `types`, `wire-codec`, `ast`, `ast-layout`, `idl-parse`, `idl-lower`. Genutzt von: `tools/record`, `tools/replay`, `tools/spy`.
- Features: keine.

### `zerodds-durability-store` — `crates/durability-store`
- Zweck: Speicherabstraktion des Durability-Service (DDS 1.4 §2.2.3.4/§2.2.3.5, ADR 0009), ohne DDS- oder RTPS-Abhängigkeit.
- Einstieg: Trait `DurabilityStore` (`store.rs`), `Contract` (`contract.rs`), `DurabilitySample`, `Selector`, `Cursor`, `Page` (`model.rs`), `InMemoryStore` (`memory.rs`), `TieredStore` (`tiered.rs`), `StoreError`.
- Abhängig von: `qos`. Genutzt von: `durability-service`, `durability-service-bin` und den vier Adaptern.
- Features: keine.

### `zerodds-durability-store-file` — `crates/durability-store-file`
- Zweck: `PERSISTENT`-Adapter mit einer Datei pro Sample: `<root>/<hex(topic)>/<hex(instance_key)>/<sequence>.bin`, Marker `.unregistered`.
- Abhängig von: `durability-store`, `qos`. Genutzt von: `durability-service-bin`, `durability-service` (dev). Features: keine.

### `zerodds-durability-store-sqlite` — `crates/durability-store-sqlite`
- Zweck: Standard-Adapter für `PERSISTENT` auf sqlite mit `journal_mode=WAL`, `synchronous=NORMAL`; Primärschlüssel `(topic, instance, sequence)`.
- Abhängig von: `durability-store`, `qos`. Genutzt von: `durability-service-bin`, `durability-service` (dev). Features: keine.

### `zerodds-durability-store-postgres` — `crates/durability-store-postgres`
- Zweck: PostgreSQL-Adapter als gemeinsame, netzwerkerreichbare Ablage für viele Produzenten; gleiche Semantik wie sqlite.
- Abhängig von: `durability-store`, `qos`. Genutzt von: `durability-service-bin` (Feature `postgres`).
- Features: `timescaledb`.

### `zerodds-durability-store-lakehouse` — `crates/durability-store-lakehouse`
- Zweck: DuckDB-Adapter: typisierte Tabelle für SQL-Auswertung und `LakehouseStore::export_parquet` für Parquet-Partitionen.
- Abhängig von: `durability-store`, `qos`. Genutzt von: `durability-service-bin` (Feature `lakehouse`).
- Features: `duckdb` (optional `duckdb`; Standard leer).

### `zerodds-durability-service` — `crates/durability-service`
- Zweck: Durability-Service als eigener Participant (ADR 0009): Aufnahme über einen `RELIABLE/TRANSIENT_LOCAL/KEEP_ALL`-Reader, Ablage in einem `DurabilityStore`, Wiedergabe über einen `TRANSIENT_LOCAL/KEEP_ALL`-Writer (laut Doku-Kommentar).
- Einstieg: `DurabilityService`, `ServiceError` (`src/lib.rs`).
- Abhängig von: `dcps`, `durability-store`, `qos`, `types`; dev: `durability-store-file`, `durability-store-sqlite`, `test-domains`. Genutzt von: `durability-service-bin`.
- Features: keine.

### `zerodds-durability-service-bin` — `crates/durability-service-bin`
- Zweck: Daemon `zerodds-durability-svc` (`src/main.rs`): ein Prozess pro Domain, Store wählbar (`--store sqlite|file|lakehouse|postgres`), `--auto` für TRANSIENT/PERSISTENT-Topics oder `--topic`.
- Abhängig von: `dcps`, `durability-service`, `durability-store`, `-file`, `-sqlite`, optional `-lakehouse`, `-postgres`.
- Features: `lakehouse`, `postgres`, `postgres-timescaledb`. Optionen: [cli-options.md](cli-options.md).

### `zerodds-routing-service` — `crates/routing-service`
- Zweck: Weiterleitung von Samples zwischen Domains, Topics, QoS-Profilen und Partitionen innerhalb von DDS. Eine `Route` koppelt einen Eingangs- mit einem Ausgangs-Endpunkt, optional mit Transformation und SQL-Filter.
- Einstieg: `Router::start`, `start_with_types`, `start_with_runtime` (`engine.rs:23–68`), `RouterConfig`, `Route`, `Endpoint` (`config.rs`), XML-Konfiguration (`xml.rs`), Transformation und Body-Codec (`transform/`, `transform/codec.rs`, `transform/sqlbridge.rs`), `RouteMetrics` (`metrics.rs`). Binary `zerodds-router` (`src/bin/zerodds-router.rs`).
- Abhängig von: `dcps`, `qos`, `rtps`, `wire`, `types`, `sql-filter`, `monitor`; dev: `test-domains`. Genutzt von: keinem Paket im Workspace.
- Features: keine.

## Test, Prüfung und Build

### `zerodds-wire-conformance` — `crates/wire-conformance`
- Zweck: Kompiliert den vom Generator erzeugten Code (`build.rs`, `mod generated`), damit Signaturfehler gegenüber `wire` als Compilerfehler auffallen.
- Abhängig von: `wire`; build: `ast*`, `idl-*`, `lang-*`. Genutzt von: niemandem (Test-Crate). Features: keine.

### `zerodds-wire-probe` — `crates/wire-probe`
- Zweck: ZeroDDS-Seite der Interop-Suite: IDL-Typ und JSON-Wert rein, Wire-Bytes raus. Arbeitet reflektiv über Modell und Layout-Plan, ohne Codegenerator.
- Einstieg: `encode`, `decode`, `dump`, `dump_fastdds`, `fill`, `Vendor`, `ProbeRequest` (`src/lib.rs:107–116`), Binary `wireprobe-zerodds` (`src/main.rs`), weitere Binaries in `src/bin/` (`wireprobe-compare`, `wireprobe-dump`, `wireprobe-fill`, `wireprobe-corba-idl`, `wireprobe-ros-package`).
- Abhängig von: `wire`, `wire-codec`, `float128`, `ast*`, `idl-compose`, `idl-cst`, `idl-parse`, `idl-lower`.
- Genutzt von: `tools/interop-results`, `wireprobe-zerodds-*-gen` (c, cpp, csharp, java, python, typescript); als Dev-Abhängigkeit von `dcps`, `routing-service`, `xrce-agent`, `zenoh-bridge`, `opcua-bridge` und `tools/idlc`.
- Spec: `docs/specs/zerodds-interop-suite-1.0.md`.

### `zerodds-conformance` — `crates/conformance`
- Zweck: Testvektoren externer Konformitätssuiten als Konstanten, ausgeführt gegen die Implementierungen.
- Einstieg: `run_suite`, `TestCase`, `CaseResult` (`src/lib.rs`); Module `autobahn_ws` (RFC 6455/7692), `coap_plugtest`, `h2spec_grpc`, `oasis_mqtt`, `dds_xml_xvendor`.
- Abhängig von: `coap-bridge`, `grpc-bridge`, `hpack`, `http2`, `mqtt-bridge`, `websocket-bridge`, `zerodds-xml-wire`, `dotnet-build-lock`. Features: `std` (Standard), `alloc`.

### `zerodds-endpoint-e2e` — `crates/endpoint-e2e`
- Zweck: Rust-Gegenstelle für Ping-Pong-Tests der nativen Endpoint-SDKs über UDP und XRCE `WRITE_DATA`; dekodiert mit `wire`.
- Einstieg: `bind_peer`, `ping_pong`, `xrce_frame`, `xrce_unframe`, `xcdr2_writer`, `xcdr2_reader` (`src/lib.rs`), Codegen-Helfer (`codegen.rs`), Binary `src/bin/peer.rs`.
- Abhängig von: `wire`, `xrce`, `ast-layout`, `idl-parse`, `idl-lower`, `lang-cst`, `lang-rules`; dev: `endpoint-rust`, `dotnet-build-lock`. Features: keine.

### `zerodds-test-domains` — `crates/test-domains`
- Zweck: Vergibt eine DDS-Domain exklusiv an einen Testprozess, weil die Domain-Nummer den UDP-Port bestimmt (RTPS §9.6.1.4.1: `7400 + 250 * domain`).
- Einstieg: `lease_domain() -> DomainLease`, `unique_domain()`, `domain_port_window(domain)` (`src/lib.rs`).
- Genutzt von (dev): `dcps`, `zerodds-c-api`, `durability-service`, `durability-service-bin`, `routing-service`, `tools/record`. Features: keine.

### `chaos-clock-skew` — `crates/chaos-clock-skew`
- Zweck: `LD_PRELOAD`-Bibliothek (`crate-type = ["cdylib", "rlib"]`), die `clock_gettime()` und `gettimeofday()` um `CHAOS_CLOCK_SKEW_NS` (fester Versatz) und `CHAOS_CLOCK_DRIFT_PPM` (Drift) verschiebt.
- Abhängig von: nichts. Genutzt von: keinem Paket im Workspace (wird per `LD_PRELOAD` geladen). Features: keine.

### `safe-crates-only` — `crates/safe-crates-only`
- Zweck: Meta-Crate ohne Logik. Zwingt die SAFE-klassifizierten Crates unter dem Profil `safety` in einen `no_std`-Build: `cargo build -p safe-crates-only --no-default-features --features safety`.
- Abhängig von: `foundation`, `wire`, `types`, `qos`, `rtps`, `discovery`, `transport`, `transport-udp`, `security`, `xrce-client`, `sys`.
- Features: `safety` (reicht `safety` + `alloc` an die genannten Crates weiter; `wire` hat keine Features).
- Bezug: `docs/architecture/04_safety_by_architecture.md`.

### `zerodds-lint` — `crates/lint`
- Zweck: Projekt-Lints auf AST-Basis, Aufruf `cargo run -p zerodds-lint -- check`. Laut Doku-Kommentar in CI (`.gitlab-ci.yml`, Job `zerodds-lint`) und im Pre-Commit-Hook (`scripts/pre-commit.sh`); nicht für crates.io vorgesehen.
- Einstieg: Binary `zerodds-lint` (`src/bin/zerodds-lint.rs`), `runner.rs`, `scanner.rs`, `classification.rs`, Lints in `src/lints/`: `bounded_recursion`, `no_alloc_in_hot_path`, `no_dyn_in_safe`, `no_panic_in_safe`, `no_realloc_in_hot_path`, `require_safety_comment`, `safety_classification_present`.
- Abhängig von: keinem ZeroDDS-Crate. Features: keine (`default = []`).
- Bezug: `docs/architecture/04_safety_by_architecture.md` §3.4.

### `zerodds-build` — `crates/zerodds-build`
- Zweck: Helfer für `build.rs`: kompiliert IDL zur Build-Zeit zu Rust-Typen mit `DdsType`-Implementierung.
- Einstieg: `Config::new().include_dir(..).compile(&[..])`, `compile`, `Error`, `DefaultExt` (`src/lib.rs`), `dds_type.rs`.
- Abhängig von: `ast`, `ast-core`, `ast-layout`, `idl-compose`, `idl-parse`, `lang-cst`, `lang-render`, `lang-rules`, `types`; dev: `wire`.
- Genutzt von (build): `tools/bench-suite`, `tools/omg-security-main`, `tools/omg-shape-main`, `tools/omg-xtypes-main`. Features: keine.
