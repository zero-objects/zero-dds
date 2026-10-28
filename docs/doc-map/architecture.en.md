# Architecture

German original: [architecture.md](architecture.md)

This page describes how the parts of ZeroDDS work together, from the type level up to the services: the layer model, the path of a sample from `DataWriter::write` to a remote `DataReader`, the thread model of the `DcpsRuntime`, discovery, reliability, durability, type discovery, content filters, the same-host and zero-copy path, and the hook points of security. All line numbers refer to the state of `main` from which this page was created. Statements that come only from a doc comment are marked “(per doc comment)”.

Related pages: [README.en.md](README.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## 1 Layer model

```
 +--------------------------------------------------------------------+
 |  Services and bridges                                              |
 |  routing-service, durability-service, recorder, rpc, dlrl,         |
 |  xrce-agent, zenoh-bridge, mqtt/coap/ws/grpc/amqp-bridge, c-api    |
 +--------------------------------------------------------------------+
 |  DCPS (crates/dcps)  DomainParticipant/Publisher/Subscriber/       |
 |  Topic/DataWriter/DataReader  +  DcpsRuntime (runtime.rs)          |
 |  dcps-async (wrapper)            sql-filter (content filter)       |
 +---------------------------+----------------------------------------+
 |  Discovery                |  Security (optional, feature           |
 |  (crates/discovery)       |  `security` in dcps)                   |
 |  SPDP, SEDP, TypeLookup,  |  security (SPI), security-runtime      |
 |  security builtins        |  (SharedSecurityGate), pki, crypto,    |
 |                           |  permissions, logging, rtps            |
 +---------------------------+----------------------------------------+
 |  RTPS (crates/rtps)  header, submessages, ReliableWriter/Reader,   |
 |  HistoryCache, fragmentation, SPDP/SEDP data types, QoS PIDs       |
 +--------------------------------------------------------------------+
 |  Transports: trait `Transport` (crates/transport)                  |
 |  transport-udp | transport-tcp | transport-shm | transport-uds |   |
 |  transport-tsn   (+ flatdata / iceoryx-cyclone for same host)      |
 +--------------------------------------------------------------------+
 |  Type system and wire                                              |
 |  wire (writer/reader, alignment, framing) · wire-codec (layout     |
 |  plan <-> bytes) · types (XTypes 1.3 TypeIdentifier/TypeObject,    |
 |  assignability, DynamicType) · qos · float128 · foundation         |
 |  Layout plan: crates/ast-layout (see crates-languages-codegen.md)  |
 +--------------------------------------------------------------------+
```

| Layer | Crates | Purpose |
|---|---|---|
| Type system and wire | `wire`, `wire-codec`, `types`, `qos`, `float128`, `foundation` | XCDR1/XCDR2 encoding, XTypes type model, QoS policies and PID codec |
| RTPS | `rtps` | DDSI-RTPS 2.5 messages, writer/reader state machines, `HistoryCache` |
| Transports | `transport`, `transport-udp`, `-tcp`, `-shm`, `-uds`, `-tsn` | `Transport` trait (`send`, `recv`, `recv_batch`, `local_locator`) and implementations |
| Discovery | `discovery` | `SpdpBeacon`, `SpdpReader`, `SedpStack`, `TypeLookupServer/Client`, `SecurityBuiltinStack` |
| DCPS | `dcps`, `dcps-async`, `sql-filter`, `config` | public DDS API, runtime, threads, matching |
| Security | `security*` | DDS-Security plugins, hooked in via `RuntimeConfig::security` |
| Services | `routing-service`, `durability-service`, `recorder`, `rpc`, … | use the DCPS API like an application |

Dependency direction (checked in the `Cargo.toml` files): `wire` depends only on `float128`. `types` depends on `wire`, `wire-codec`, `ast`, `ast-core`. `rtps` depends on `wire`, `types`, `qos`, `foundation` and knows no transports. `transport` depends on `rtps` (re-export of `Locator`). `discovery` depends on `rtps`, `types`, `qos`, `wire`, `security`. `dcps` pulls in all transports, `discovery`, `sql-filter`, `config` and, optionally, the security crates.

## 2 Runtime model

A `DomainParticipant` starts a `DcpsRuntime` when it is created (`crates/dcps/src/participant.rs:996` calls `DcpsRuntime::start`). The struct is in `crates/dcps/src/runtime.rs:8233`, the configuration in `RuntimeConfig` (`runtime.rs:1756`).

Threads that `DcpsRuntime::start` creates (`runtime.rs:9683–9940`):

| Thread name | Function | Purpose |
|---|---|---|
| `zdds-recv-spdp-mc` | `recv_spdp_multicast_loop` (`runtime.rs:22334`) | receive SPDP multicast |
| `zdds-recv-meta` | `recv_metatraffic_loop` (`runtime.rs:22448`) | metatraffic unicast: SPDP, SEDP, TypeLookup, WLP, security builtins |
| `zdds-recv-user` | `recv_user_data_loop` (`runtime.rs:22701`) | user data over `user_unicast` |
| `zdds-recv-user-N` | `recv_user_data_loop` | additional receivers when `extra_recv_threads > 0`, same port via `SO_REUSEPORT` (`UdpTransport::bind_v4_reuse`) |
| `zdds-tcp-accept` | closure around `TcpTransport::accept_one` | only with TCP as user-data transport |
| `zdds-recv-shm` | `recv_same_host_loop` (starts one thread `zdds-recv-shm-p` per same-host peer) | only with feature `same-host-shm` |
| `zdds-tick` | `tick_loop` (`runtime.rs:22864`) | periodic outbound work, default variant |
| `zdds-tick-sched` | `scheduler_tick_loop` | with `scheduler_tick`: deadline heap from `crates/dcps/src/scheduler.rs` instead of a fixed tick interval |
| — | — | with `external_tick` no tick thread; `DcpsRuntime::tick_driver` (`runtime.rs:23924`) returns a `DcpsTickDriver`, driven for example by a tokio executor (`zerodds-async-1.0` §4) |

One tick iteration (`run_tick_iteration`, `runtime.rs:23119`) does, in this order: periodic SPDP announce via `spdp_beacon`, `SedpStack::tick` (HEARTBEAT, resend, ACKNACK of the builtin endpoints), `SecurityBuiltinStack::poll`, WLP tick, `ReliableWriter::tick` of every user writer (HEARTBEAT and resends), `ReliableReader::tick_outbound` of every user reader (ACKNACK, NACK_FRAG), non-blocking polling of the interface sockets. After that, every driver calls `tick_housekeep` (`runtime.rs:23659`): deadline, lifespan, liveliness (reader and writer side) and the timeout for pending TypeLookup replies.

The receive path and the write path wake the scheduler via `raise_tick_wake`, so that an ACKNACK does not wait for the next tick. The comment in `runtime.rs:9689` sets the lock order: hold `sedp` and `wlp` briefly, take no further lock while holding them.

`apply_thread_tuning` (`runtime.rs:21890`) sets thread priority and CPU affinity directly via `libc::pthread_setschedparam` and `sched_setaffinity`. `crates/rt-linux` is not used for this.

## 3 Path of a sample

```
 Application
   | DataWriter<T>::write(&sample)            dcps/src/publisher.rs:2186
   |   topic.codec().encode(..)  -> body (XCDR1/XCDR2, LE/BE)
   |   keyhash_and_holder(..)    -> 16-byte key hash
   |   [durability_backend.store(..)]  TRANSIENT/PERSISTENT
   v
 DcpsRuntime::write_user_sample_keyed / write_user_sample
   | write_user_sample_with_key                 dcps/src/runtime.rs:18057
   |   user_payload_encap(..)    -> encapsulation header
   |   ReliableWriter::write_sample_with_inline_qos   rtps/src/reliable_writer.rs:1149
   |      -> CacheChange in HistoryCache, Vec<OutboundDatagram>
   |   stage_outbound(..) / PendingMessage  (LATENCY_BUDGET batching)
   v
 send_pending_batch                             runtime.rs:17790
   |   secure_outbound_for_target  (security, if active)
   |   user_unicast.send(locator, bytes)        Transport trait
   |   [RoutedUserTransport: a same-host peer's locator -> SHM channel]   same-host-shm
   |   [inproc peers: handle_user_datagram directly]
   |   intra_runtime_dispatch_alive  (readers in the same participant)
   v
 ==================== Network / SHM / UDS / TCP =====================
   v
 recv_user_data_loop -> Transport::recv_batch   runtime.rs:22701
 dispatch_user_datagram                         runtime.rs:22822
   |   secure_inbound_bytes         (security, if active)
   |   dispatch_type_lookup_datagram  (branch off TL_SVC_* frames)
   v
 handle_user_datagram                           runtime.rs:26668
   |   decode_datagram                          rtps/src/datagram.rs:379
   |   DATA     -> ReliableReader::handle_data  rtps/src/reliable_reader.rs:1199
   |   DATA_FRAG-> handle_data_frag;  HEARTBEAT -> handle_heartbeat
   |   GAP      -> handle_gap;        ACKNACK   -> ReliableWriter::handle_acknack
   |   TIME_BASED_FILTER (tbf_should_deliver), listener call
   |   sample_tx (mpsc) .send(UserSample) + wake_async_waker
   v
 DataReader<T>::take / read                     dcps/src/subscriber.rs:3539 / 4048
   |   autopurge, lifespan, decode by encapsulation (decode_for_encap)
   |   sample_passes_filter (closure), time-based filter, exclusive ownership
   v
 Application
```

Details, checked in the code:

- `DataWriter::write` determines the representation from the same source as the encapsulation header (`DcpsRuntime::user_writer_offer_first`). The typed writer locks the representation when it is created (`lock_user_writer_representation`), so that body and header match.
- Without a runtime (offline mode) the sample goes into an internal queue with `RESOURCE_LIMITS` blocking (`publisher.rs`, end of `write`).
- Small samples (up to `SMALL_FRAME_CAP`) go via `write_user_sample_pooled` (`runtime.rs:967`) into a `PoolBuffer` from `crates/foundation`.
- With `LATENCY_BUDGET.duration = 0` and `batch_at_zero_budget = false`, every message is sent immediately. With a budget, `stage_outbound` collects messages per target set, and the tick sends them when the budget has expired (`drain_due_batches`, `runtime.rs:6085`, called in the tick at `runtime.rs:23505`). `flush_writer_batches(eid, true)` flushes immediately, for example before lifecycle markers and when the writer is removed (per doc comment).
- With `DATA.reader_id == ENTITYID_UNKNOWN` the sample is offered to all reader slots, otherwise only to the addressed one.
- Within one process, `crates/dcps/src/inproc.rs` delivers datagrams directly to other runtimes of the same domain; the normal wire paths run in addition.

## 4 Discovery

SPDP (DDSI-RTPS 2.5 §8.5.3):

1. `SpdpBeacon` (`crates/discovery/src/spdp.rs:121`) is sent periodically in the tick (`spdp_period`; at the start `initial_announce_count` times at an interval of `initial_announce_period`).
2. Reception via `zdds-recv-spdp-mc` or `zdds-recv-meta` → `handle_spdp_datagram` (`runtime.rs:27561`) → `SpdpReader::parse_datagram` → entry in `DiscoveredParticipantsCache` (`spdp.rs:571`).
3. For new participants: `SedpStack::on_participant_discovered` creates the SEDP proxies. The runtime immediately sends a unicast SPDP to the new peer and repeats it at `initial_announce_period` until `initial_announce_count` responses are out (`send_spdp_follow_ups`). With security active, it calls `handle_remote_endpoints` of the security builtin stack.
4. Lease expiry: `expire_discovered_participants`, `remove_discovered_participant`.

SEDP (§8.5.4):

1. `recv_metatraffic_loop` → `handle_sedp_datagram` (`runtime.rs:24530`) → `SedpStack::handle_datagram` (`crates/discovery/src/sedp/stack.rs`) → `SedpEvents`.
2. `apply_sedp_removals`, `push_sedp_events_to_builtin_readers` (builtin topics `DCPSPublication`/`DCPSSubscription`).
3. `run_matching_pass` (`runtime.rs:25148`) checks every local writer and reader against the SEDP cache: `match_local_writer_against_cache`, `match_local_reader_against_cache`.
4. `wire_writer_to_remote_reader` (`runtime.rs:16375`) checks in this order: ignore filter (§2.2.2.2.1.16), security `check_remote_datareader`, locator present, durability rank, partition overlap, type consistency (`decide_type_consistency`). On success `ReliableWriter::add_reader_proxy` (`runtime.rs:16951`). The reverse direction is `wire_reader_to_remote_writer` (`runtime.rs:17001`) with `add_writer_proxy`.
5. `record_match_refusal` logs rejected pairs with the reason.

Further discovery paths: `initial_peers` and `discovery_server` in `RuntimeConfig`, `inproc.rs` for participants in the same process, WLP (DDSI-RTPS §8.4.13) in `crates/dcps/src/wlp.rs`.

## 5 Reliability

- `ReliableWriter` (`crates/rtps/src/reliable_writer.rs:162`) corresponds to the stateful writer from DDSI-RTPS 2.5 §8.4.9 with 1..N `ReaderProxy`. `tick(now)` (`reliable_writer.rs:1571`) sends HEARTBEAT when `heartbeat_period` has elapsed and the cache is not empty, and serves requested fragments (NACK_FRAG) and resends.
- `handle_acknack` (`reliable_writer.rs:1831`) and `handle_nackfrag` (`reliable_writer.rs:1968`) are called from `handle_user_datagram`.
- `ReliableReader` (`crates/rtps/src/reliable_reader.rs:261`) keeps one `WriterProxyState` per writer, delivers in SN order (`handle_data`, `handle_heartbeat`, `handle_gap`) and generates ACKNACK/NACK_FRAG in `tick_outbound` (`reliable_reader.rs:1797`). `heartbeat_response_delay` and `heartbeat_suppression_duration` are configurable.
- Fragmentation: `ReliableWriter::set_fragmentation(fragment_size, mtu)`; reassembly in `crates/rtps/src/fragment_assembler.rs`, upper limit `max_reassembly_sample_bytes` in `RuntimeConfig`.
- Best-effort reader: `ReliableReader::set_best_effort(true)`; there is no separate best-effort writer in the user-data path.
- `writer.rs`/`reader.rs` in `crates/rtps` (stateless endpoints) are used by `crates/discovery/src/security/stateless.rs`. `reliable_stateless_writer.rs` is not used outside `crates/rtps` by any crate in `crates/discovery` or `crates/dcps` (grep).
- `crates/rtps/src/dialect.rs` (`RtpsBehaviour`, `set_peer_behaviour`) holds vendor-dependent protocol behavior per peer (see [interop-and-dialects.en.md](interop-and-dialects.en.md)).

## 6 Durability and history

| Level | Implementation |
|---|---|
| History | `HistoryCache` (`crates/rtps/src/history_cache.rs:505`) with `HistoryKind::KeepAll` (fixed bound, error on overflow) or `KeepLast(depth)` (ring buffer). On the writer side via `set_cache_history`. |
| `VOLATILE` | After every write `ReliableWriter::remove_acknowledged` (`runtime.rs`, in `write_user_sample_with_key`). |
| `TRANSIENT_LOCAL` | In addition `slot.retained` with `RetainedSample` per instance, depth via `enforce_retained_depth` (`runtime.rs:457`). When a late-joining reader is matched, `wire_writer_to_remote_reader` replays the history. |
| `TRANSIENT` / `PERSISTENT` in the writer | `DataWriter` with `durability_backend` stores the raw body (`crates/dcps/src/durability_service.rs`); on match, `replay_for_topic` delivers the samples afterwards with new SNs. |
| Standalone service | `crates/durability-service` (ADR 0009): reader `RELIABLE/TRANSIENT_LOCAL/KEEP_ALL`, storage in a `DurabilityStore` (`crates/durability-store`, adapters file/sqlite/postgres/lakehouse), replay via a `TRANSIENT_LOCAL` writer (per doc comment in `crates/durability-service/src/lib.rs`). |

Lifespan: `slot.sample_insert_times` on the writer side, `expire_by_lifespan` in housekeeping, `live_sample_expired` in `DataReader::take`.

## 7 Type discovery

- Type model: `TypeIdentifier`, `TypeObject` (Minimal/Complete), `TypeInformation` in `crates/types` (XTypes 1.3 §7.3). Hashes via `compute_minimal_hash`/`compute_complete_hash` (`crates/types/src/hash.rs`).
- `TypeInformation` is transmitted in SEDP as PID `0x0075` (`crates/rtps/src/parameter_list.rs:209`).
- `Subscriber::create_datareader` registers the local TypeObject with the `TypeLookupServer`, so that a peer gets an answer to `getTypes` (`crates/dcps/src/subscriber.rs:1172`, comment “Gap 2 (#24)”).
- Decision at match time: `decide_type_consistency` (`runtime.rs:24407`). Identical `TypeIdentifier` values count as consistent. If the TypeObject is missing from the registry, the result is `TypeConsistency::Unresolved`; the runtime sends `send_type_lookup_request` (`runtime.rs:10332`) and records the pair in `type_resolution_pending`. After `TYPE_RESOLUTION_GRACE` (3 s, `runtime.rs:182`), `expire_type_resolution_grace` forces the decision (XTypes §7.6.3.4.2 step 2). Otherwise `TypeMatcher::match_types` (`crates/types/src/type_matcher.rs`) checks assignability according to the `TypeConsistencyEnforcement` flags.
- TypeLookup service: `crates/discovery/src/type_lookup/{client,server,endpoints}.rs`, transport via `crates/dcps/src/type_lookup_transport.rs`, input via `dispatch_type_lookup_datagram` (`runtime.rs:30540`) on the user-data path and on the metatraffic path.
- Without generated code: `crates/types/src/dynamic` (`DynamicType`, `DynamicData`) and `crates/wire-codec` (`encode`/`decode` along the layout plan). `crates/recorder-decode` uses this path.

## 8 Content filters

- Parser and evaluation: `crates/sql-filter` (DDS 1.4 Annex B.2.1). `parse(&str) -> Expr`, `Expr::evaluate(row, params)` against `RowAccess`.
- `DomainParticipant::create_contentfilteredtopic` (`crates/dcps/src/participant.rs:2488`) creates `ContentFilteredTopic<T>` (`crates/dcps/src/topic.rs:1517`). The constructor parses the expression and checks the `%N` indices. `evaluate` evaluates against a `RowAccess`; `DdsTypeRow` (`dds_type.rs:687`) implements `RowAccess` via `DdsType::field_value`.
- `DataReader::with_filter` (`subscriber.rs`) sets a Rust closure filter, which `take`/`read` apply via `sample_passes_filter`.
- `Subscriber::create_datareader_with_cft` creates the reader on the related topic, filters with the expression and the parameters current at the time, and puts the filter into the SEDP subscription as `PID_CONTENT_FILTER_PROPERTY` (`UserReaderConfig::content_filter`). `ContentFilteredTopic::set_filter_parameters` re-announces the subscription (`DcpsRuntime::update_user_reader_content_filter`). A sample against which the expression cannot be evaluated (a field that `field_value` does not answer) is not delivered.
- ZeroDDS writers do not filter on the writer side; every reader filters itself, also downstream of a foreign writer that filters on the writer side.
- Parameters (`%N`) are read as literals according to DDS 1.4 Annex B (`Value::from_parameter`): `42` is an integer, `'42'` is text.
- `QueryCondition` (`crates/dcps/src/condition.rs`) also uses `sql-filter`.
- C API: `zerodds_sub_create_datareader_with_cft` announces the filter in the same way. The filter reads the fields via the `CompleteTypeObject` (`zerodds_cft_set_type_object`, set in C++ by `ContentFilteredTopic<T>` from `topic_type_support<T>::type_object()`) or via a positional schema (`zerodds_cft_set_schema`); an expression that reads a field, with neither of the two set, prevents the reader from being created. For a type that contains other types, `zerodds_cft_set_type_objects` takes the dependencies along; `idlc cpp` emits them as `topic_type_support<T>::type_object_dependency(i)`. `zerodds_cft_set_filter_parameters` replaces the parameters; existing readers filter with them from the next take on and re-announce them (C++: `ContentFilteredTopic::filter_parameters`).
- Other users of `sql-filter`: `crates/zerodds-c-api` (`entities.rs`, `subscriber_ffi.rs`, `condition_ffi.rs`), `crates/routing-service/src/transform/sqlbridge.rs`.

## 9 Same host, SHM and zero-copy

| Path | Feature / setting | Code | Serialized? |
|---|---|---|---|
| Same-host SHM alongside UDP | `same-host-shm` | `same_host_channel.rs` (one channel per same-host participant, opened when SPDP reports it), `same_host_shm.rs` (segment configuration); send through `RoutedUserTransport` and `send_metatraffic`, receive `recv_same_host_loop` | yes, every unicast datagram to the peer over `PosixShmTransport` |
| SHM as user-data transport | `transport.user_transport = SHM` (`UserTransportKind::Shm`) | `shm_user.rs` (`ShmUserTransport`), `layered_transport.rs` (`LayeredUserTransport`: SHM preferred, UDP as fallback) | yes |
| UDS | `same-host-uds`, `UserTransportKind::Uds` | `crates/transport-uds` | yes; the kernel copies |
| FlatData | `flatdata-integration` | `flatdata_integration.rs` (`DataWriter::write_flat`, `DataReader::read_flat`), `crates/flatdata` (`FlatStruct`, `PosixSlotAllocator`) | no, `repr(C)` slots in shared memory |
| iceoryx2 | `same-host-iceoryx2` → `zerodds-flatdata/iceoryx2-bridge` | `crates/flatdata/src/iceoryx.rs` | no |
| Cyclone iceoryx (POSH) | `cyclone-iox`, Linux only | `cyclone_iox_integration.rs`, `crates/iceoryx-cyclone` | XCDR1 body in PSMX chunk |
| UDP batch receive | `recvmmsg-batch` (default in `transport-udp`) | `crates/transport-udp/src/recv_batch.rs`, `Transport::recv_batch` | — |

`DeliveryMode` (`runtime.rs:1493`: `Portable`, `RawSameHost`, `Iceoryx`) is read from `delivery.mode`. It applies to loaning writers: the loan writers of the C API (`crates/zerodds-c-api/src/shm_loan_ffi.rs`, `default_mode`) and the RMW publishers and subscriptions with a loanable type. `DataWriter::write` in Rust and the copying `write` of the C API are always `Portable`. `Iceoryx` needs one iceoryx2 service per writer (`zerodds_dw_enable_iceoryx`; the RMW derives it from the topic); a writer without a service sends `Portable` and reports this once on stderr; the RMW reports every fallback with its reason via `RCUTILS_LOG_WARN` (`zerodds-delivery-modes-1.0` §4).

Specifications: `docs/specs/zerodds-zero-copy-1.0.md`, `docs/specs/zerodds-flatdata-1.0.md`, `docs/specs/zerodds-delivery-modes-1.0.md`, ADR 0003–0006 in `docs/adr/`.

## 10 Security

- SPI: traits `AuthenticationPlugin`, `AccessControlPlugin`, `CryptographicPlugin`, `LoggingPlugin`, `DataTaggingPlugin` in `crates/security/src/`.
- Implementations: `security-pki` (authentication, X.509), `security-keyexchange` (X25519/P-256), `security-crypto` (AES-GCM/HMAC), `security-permissions` (governance/permissions XML), `security-logging`, `security-rtps` (secure submessage wrapper, §7.3.6).
- Runtime: `zerodds_security_runtime::SharedSecurityGate` sits in `RuntimeConfig::security` (`runtime.rs:1910`). Without the feature `security`, the hooks pass data through unchanged.
- Hook points in `crates/dcps/src/runtime.rs`:
  - outbound: `secure_outbound_for_target` (`5621`) per target locator in UDP sending; via `RoutedUserTransport` the same sending also carries the same-host SHM channel (`9227`), the inproc path protects a datagram the same way for the peer's locator (`deliver_in_process`, `5897`); protection level per topic via `gate.data_protection(topic)`.
  - inbound: `secure_inbound_bytes` (`5330`) in all receive loops; SPDP and stateless messages are exempt from message encryption (comment referring to DDS-Security §8.4.2.4).
  - Handshake and key exchange: `SecurityBuiltinStack` (`crates/discovery/src/security/stack.rs`, stateless and volatile-secure endpoints), input `dispatch_security_builtin_datagram` (`30105`), output in the tick.
  - Access control during matching: `check_remote_datareader` in `wire_writer_to_remote_reader`.
- Feature chain: `dcps/security`, `dcps/fips` → `security-runtime/fips`, `dcps/wolfcrypt` → `security-runtime/wolfcrypt`.

Details on configuration and governance: [security.en.md](security.en.md).

## 11 Services and bridges

Services and bridges use the public API of `crates/dcps` like an application (checked against the dependencies in the `Cargo.toml` files): `routing-service`, `durability-service`, `rpc`, `zenoh-bridge`, the protocol bridges and `zerodds-c-api`. `recorder` has no ZeroDDS dependency; the tools `tools/record`, `tools/replay`, `tools/spy` connect it to DCPS. Description of the individual crates: [crates-core.en.md](crates-core.en.md) and [crates-bridges-services.en.md](crates-bridges-services.en.md).
