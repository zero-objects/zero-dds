# Architektur

Diese Seite beschreibt, wie ZeroDDS von der Typebene bis zu den Diensten zusammenarbeitet: das Schichtenmodell, den Weg eines Samples von `DataWriter::write` bis zu einem entfernten `DataReader`, das Thread-Modell der `DcpsRuntime`, Discovery, Zuverlässigkeit, Durability, Typ-Discovery, Content-Filter, den Same-Host- und Zero-Copy-Pfad und die Einhängepunkte der Security. Alle Zeilenangaben beziehen sich auf den Stand von `main`, aus dem diese Seite erstellt wurde. Aussagen, die nur aus einem Doku-Kommentar stammen, sind mit „(laut Doku-Kommentar)“ markiert.

Verwandte Seiten: [README.md](README.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## 1 Schichtenmodell

```
 +--------------------------------------------------------------------+
 |  Dienste und Bridges                                               |
 |  routing-service, durability-service, recorder, rpc, dlrl,         |
 |  xrce-agent, zenoh-bridge, mqtt/coap/ws/grpc/amqp-bridge, c-api    |
 +--------------------------------------------------------------------+
 |  DCPS (crates/dcps)  DomainParticipant/Publisher/Subscriber/       |
 |  Topic/DataWriter/DataReader  +  DcpsRuntime (runtime.rs)          |
 |  dcps-async (Wrapper)            sql-filter (Content-Filter)       |
 +---------------------------+----------------------------------------+
 |  Discovery                |  Security (optional, Feature           |
 |  (crates/discovery)       |  `security` in dcps)                   |
 |  SPDP, SEDP, TypeLookup,  |  security (SPI), security-runtime      |
 |  Security-Builtins        |  (SharedSecurityGate), pki, crypto,    |
 |                           |  permissions, logging, rtps            |
 +---------------------------+----------------------------------------+
 |  RTPS (crates/rtps)  Header, Submessages, ReliableWriter/Reader,   |
 |  HistoryCache, Fragmentierung, SPDP/SEDP-Datentypen, QoS-PIDs      |
 +--------------------------------------------------------------------+
 |  Transporte: Trait `Transport` (crates/transport)                  |
 |  transport-udp | transport-tcp | transport-shm | transport-uds |   |
 |  transport-tsn   (+ flatdata / iceoryx-cyclone für Same-Host)      |
 +--------------------------------------------------------------------+
 |  Typsystem und Wire                                                |
 |  wire (Writer/Reader, Alignment, Framing) · wire-codec (Layout-    |
 |  Plan <-> Bytes) · types (XTypes 1.3 TypeIdentifier/TypeObject,    |
 |  Assignability, DynamicType) · qos · float128 · foundation         |
 |  Layout-Plan: crates/ast-layout (siehe crates-languages-codegen.md)|
 +--------------------------------------------------------------------+
```

| Schicht | Crates | Aufgabe |
|---|---|---|
| Typsystem und Wire | `wire`, `wire-codec`, `types`, `qos`, `float128`, `foundation` | XCDR1/XCDR2-Kodierung, XTypes-Typmodell, QoS-Policies und PID-Codec |
| RTPS | `rtps` | DDSI-RTPS 2.5 Nachrichten, Writer-/Reader-Zustandsautomaten, `HistoryCache` |
| Transporte | `transport`, `transport-udp`, `-tcp`, `-shm`, `-uds`, `-tsn` | `Transport`-Trait (`send`, `recv`, `recv_batch`, `local_locator`) und Implementierungen |
| Discovery | `discovery` | `SpdpBeacon`, `SpdpReader`, `SedpStack`, `TypeLookupServer/Client`, `SecurityBuiltinStack` |
| DCPS | `dcps`, `dcps-async`, `sql-filter`, `config` | öffentliche DDS-API, Laufzeit, Threads, Matching |
| Security | `security*` | DDS-Security Plugins, eingehängt über `RuntimeConfig::security` |
| Dienste | `routing-service`, `durability-service`, `recorder`, `rpc`, … | nutzen die DCPS-API wie eine Anwendung |

Abhängigkeitsrichtung (aus den `Cargo.toml` geprüft): `wire` hängt nur von `float128` ab. `types` hängt von `wire`, `wire-codec`, `ast`, `ast-core` ab. `rtps` hängt von `wire`, `types`, `qos`, `foundation` ab und kennt keine Transporte. `transport` hängt von `rtps` ab (Re-Export von `Locator`). `discovery` hängt von `rtps`, `types`, `qos`, `wire`, `security` ab. `dcps` bindet alle Transporte, `discovery`, `sql-filter`, `config` und optional die Security-Crates ein.

## 2 Laufzeitmodell

Ein `DomainParticipant` startet beim Erzeugen eine `DcpsRuntime` (`crates/dcps/src/participant.rs:996` ruft `DcpsRuntime::start`). Die Struktur liegt in `crates/dcps/src/runtime.rs:8233`, die Konfiguration in `RuntimeConfig` (`runtime.rs:1756`).

Threads, die `DcpsRuntime::start` anlegt (`runtime.rs:9683–9940`):

| Thread-Name | Funktion | Aufgabe |
|---|---|---|
| `zdds-recv-spdp-mc` | `recv_spdp_multicast_loop` (`runtime.rs:22334`) | SPDP-Multicast empfangen |
| `zdds-recv-meta` | `recv_metatraffic_loop` (`runtime.rs:22448`) | Metatraffic-Unicast: SPDP, SEDP, TypeLookup, WLP, Security-Builtins |
| `zdds-recv-user` | `recv_user_data_loop` (`runtime.rs:22701`) | Nutzdaten über `user_unicast` |
| `zdds-recv-user-N` | `recv_user_data_loop` | zusätzliche Empfänger bei `extra_recv_threads > 0`, gleicher Port über `SO_REUSEPORT` (`UdpTransport::bind_v4_reuse`) |
| `zdds-tcp-accept` | Closure um `TcpTransport::accept_one` | nur bei TCP-Nutztransport |
| `zdds-recv-shm` | `recv_same_host_loop` (startet je Same-Host-Peer einen Thread `zdds-recv-shm-p`) | nur mit Feature `same-host-shm` |
| `zdds-tick` | `tick_loop` (`runtime.rs:22864`) | periodische Ausgangsarbeit, Standardvariante |
| `zdds-tick-sched` | `scheduler_tick_loop` | bei `scheduler_tick`: Deadline-Heap aus `crates/dcps/src/scheduler.rs` statt festem Takt |
| — | — | bei `external_tick` kein Tick-Thread; `DcpsRuntime::tick_driver` (`runtime.rs:23924`) liefert einen `DcpsTickDriver`, den z. B. ein tokio-Executor treibt (`zerodds-async-1.0` §4) |

Eine Tick-Iteration (`run_tick_iteration`, `runtime.rs:23119`) erledigt der Reihe nach: periodisches SPDP-Announce über `spdp_beacon`, `SedpStack::tick` (HEARTBEAT, Resend, ACKNACK der Builtin-Endpunkte), `SecurityBuiltinStack::poll`, WLP-Tick, `ReliableWriter::tick` jedes Nutz-Writers (HEARTBEAT und Resends), `ReliableReader::tick_outbound` jedes Nutz-Readers (ACKNACK, NACK_FRAG), nicht blockierendes Pollen der Interface-Sockets. Danach ruft jeder Treiber `tick_housekeep` (`runtime.rs:23659`): Deadline, Lifespan, Liveliness (Reader- und Writer-Seite) und die Frist für ausstehende TypeLookup-Antworten.

Empfangspfad und Schreibpfad wecken den Scheduler über `raise_tick_wake`, damit ein ACKNACK nicht bis zum nächsten Takt wartet. Der Kommentar in `runtime.rs:9689` legt die Lock-Reihenfolge fest: `sedp` und `wlp` kurz halten, unter diesen Locks keinen weiteren Lock nehmen.

Thread-Priorität und CPU-Bindung setzt `apply_thread_tuning` (`runtime.rs:21890`) direkt über `libc::pthread_setschedparam` und `sched_setaffinity`. `crates/rt-linux` wird dafür nicht verwendet.

## 3 Weg eines Samples

```
 Anwendung
   | DataWriter<T>::write(&sample)            dcps/src/publisher.rs:2186
   |   topic.codec().encode(..)  -> Body (XCDR1/XCDR2, LE/BE)
   |   keyhash_and_holder(..)    -> 16-Byte-Key-Hash
   |   [durability_backend.store(..)]  TRANSIENT/PERSISTENT
   v
 DcpsRuntime::write_user_sample_keyed / write_user_sample
   | write_user_sample_with_key                 dcps/src/runtime.rs:18057
   |   user_payload_encap(..)    -> Encapsulation-Header
   |   ReliableWriter::write_sample_with_inline_qos   rtps/src/reliable_writer.rs:1149
   |      -> CacheChange in HistoryCache, Vec<OutboundDatagram>
   |   stage_outbound(..) / PendingMessage  (LATENCY_BUDGET-Batching)
   v
 send_pending_batch                             runtime.rs:17790
   |   secure_outbound_for_target  (Security, falls aktiv)
   |   user_unicast.send(locator, bytes)        Transport-Trait
   |   [RoutedUserTransport: Locator eines Same-Host-Peers -> SHM-Kanal]   same-host-shm
   |   [inproc-Peers: handle_user_datagram direkt]
   |   intra_runtime_dispatch_alive  (Reader im selben Participant)
   v
 ====================== Netz / SHM / UDS / TCP ======================
   v
 recv_user_data_loop -> Transport::recv_batch   runtime.rs:22701
 dispatch_user_datagram                         runtime.rs:22822
   |   secure_inbound_bytes         (Security, falls aktiv)
   |   dispatch_type_lookup_datagram  (TL_SVC_*-Frames abzweigen)
   v
 handle_user_datagram                           runtime.rs:26668
   |   decode_datagram                          rtps/src/datagram.rs:379
   |   DATA     -> ReliableReader::handle_data  rtps/src/reliable_reader.rs:1199
   |   DATA_FRAG-> handle_data_frag;  HEARTBEAT -> handle_heartbeat
   |   GAP      -> handle_gap;        ACKNACK   -> ReliableWriter::handle_acknack
   |   TIME_BASED_FILTER (tbf_should_deliver), Listener-Aufruf
   |   sample_tx (mpsc) .send(UserSample) + wake_async_waker
   v
 DataReader<T>::take / read                     dcps/src/subscriber.rs:3539 / 4048
   |   autopurge, Lifespan, Decode nach Encapsulation (decode_for_encap)
   |   sample_passes_filter (Closure), Time-Based-Filter, Exclusive Ownership
   v
 Anwendung
```

Einzelheiten, im Code geprüft:

- `DataWriter::write` bestimmt die Repräsentation aus derselben Quelle wie der Encapsulation-Header (`DcpsRuntime::user_writer_offer_first`). Der typisierte Writer sperrt die Repräsentation beim Anlegen (`lock_user_writer_representation`), damit Body und Header übereinstimmen.
- Ohne Runtime (Offline-Modus) landet das Sample in einer internen Queue mit `RESOURCE_LIMITS`-Blockierung (`publisher.rs`, Ende von `write`).
- Kleine Samples (bis `SMALL_FRAME_CAP`) gehen über `write_user_sample_pooled` (`runtime.rs:967`) in einen `PoolBuffer` aus `crates/foundation`.
- Bei `LATENCY_BUDGET.duration = 0` und `batch_at_zero_budget = false` wird jede Nachricht sofort gesendet. Mit Budget sammelt `stage_outbound` Nachrichten je Zielmenge, der Tick sendet sie nach Ablauf (`drain_due_batches`, `runtime.rs:6085`, Aufruf im Tick bei `runtime.rs:23505`). `flush_writer_batches(eid, true)` leert sofort, etwa vor Lifecycle-Markern und beim Entfernen des Writers (laut Doku-Kommentar).
- Mit `DATA.reader_id == ENTITYID_UNKNOWN` wird das Sample allen Reader-Slots angeboten, sonst nur dem adressierten.
- Innerhalb eines Prozesses liefert `crates/dcps/src/inproc.rs` Datagramme direkt an andere Runtimes derselben Domain; zusätzlich laufen die normalen Wire-Pfade.

## 4 Discovery

SPDP (DDSI-RTPS 2.5 §8.5.3):

1. `SpdpBeacon` (`crates/discovery/src/spdp.rs:121`) wird im Tick periodisch gesendet (`spdp_period`, zu Beginn `initial_announce_count` mal im Abstand `initial_announce_period`).
2. Empfang über `zdds-recv-spdp-mc` bzw. `zdds-recv-meta` → `handle_spdp_datagram` (`runtime.rs:27561`) → `SpdpReader::parse_datagram` → Eintrag in `DiscoveredParticipantsCache` (`spdp.rs:571`).
3. Für neue Teilnehmer: `SedpStack::on_participant_discovered` legt die SEDP-Proxies an. Die Runtime schickt dem neuen Peer sofort ein Unicast-SPDP und wiederholt es im Abstand `initial_announce_period`, bis `initial_announce_count` Antworten gesendet sind (`send_spdp_follow_ups`). Bei aktiver Security ruft sie `handle_remote_endpoints` des Security-Builtin-Stacks.
4. Ablauf der Lease: `expire_discovered_participants`, `remove_discovered_participant`.

SEDP (§8.5.4):

1. `recv_metatraffic_loop` → `handle_sedp_datagram` (`runtime.rs:24530`) → `SedpStack::handle_datagram` (`crates/discovery/src/sedp/stack.rs`) → `SedpEvents`.
2. `apply_sedp_removals`, `push_sedp_events_to_builtin_readers` (Builtin-Topics `DCPSPublication`/`DCPSSubscription`).
3. `run_matching_pass` (`runtime.rs:25148`) prüft jeden lokalen Writer und Reader gegen den SEDP-Cache: `match_local_writer_against_cache`, `match_local_reader_against_cache`.
4. `wire_writer_to_remote_reader` (`runtime.rs:16375`) prüft in dieser Reihenfolge: Ignore-Filter (§2.2.2.2.1.16), Security `check_remote_datareader`, Locator vorhanden, Durability-Rang, Partition-Überlappung, Typkonsistenz (`decide_type_consistency`). Bei Erfolg `ReliableWriter::add_reader_proxy` (`runtime.rs:16951`). Die Gegenrichtung ist `wire_reader_to_remote_writer` (`runtime.rs:17001`) mit `add_writer_proxy`.
5. Abgelehnte Paare protokolliert `record_match_refusal` mit Grund.

Weitere Discovery-Wege: `initial_peers` und `discovery_server` in `RuntimeConfig`, `inproc.rs` für Teilnehmer im selben Prozess, WLP (DDSI-RTPS §8.4.13) in `crates/dcps/src/wlp.rs`.

## 5 Zuverlässigkeit

- `ReliableWriter` (`crates/rtps/src/reliable_writer.rs:162`) entspricht dem Stateful Writer aus DDSI-RTPS 2.5 §8.4.9 mit 1..N `ReaderProxy`. `tick(now)` (`reliable_writer.rs:1571`) sendet HEARTBEAT, wenn `heartbeat_period` abgelaufen ist und der Cache nicht leer ist, und bedient angeforderte Fragmente (NACK_FRAG) und Resends.
- `handle_acknack` (`reliable_writer.rs:1831`) und `handle_nackfrag` (`reliable_writer.rs:1968`) werden aus `handle_user_datagram` aufgerufen.
- `ReliableReader` (`crates/rtps/src/reliable_reader.rs:261`) hält pro Writer einen `WriterProxyState`, liefert in SN-Reihenfolge (`handle_data`, `handle_heartbeat`, `handle_gap`) und erzeugt ACKNACK/NACK_FRAG in `tick_outbound` (`reliable_reader.rs:1797`). `heartbeat_response_delay` und `heartbeat_suppression_duration` sind einstellbar.
- Fragmentierung: `ReliableWriter::set_fragmentation(fragment_size, mtu)`; Zusammensetzen in `crates/rtps/src/fragment_assembler.rs`, Obergrenze `max_reassembly_sample_bytes` in `RuntimeConfig`.
- Best-Effort-Reader: `ReliableReader::set_best_effort(true)`; es gibt keinen separaten Best-Effort-Writer im Nutzpfad.
- `writer.rs`/`reader.rs` in `crates/rtps` (zustandslose Endpunkte) nutzt `crates/discovery/src/security/stateless.rs`. `reliable_stateless_writer.rs` wird außerhalb von `crates/rtps` von keinem Crate in `crates/discovery` oder `crates/dcps` verwendet (grep).
- `crates/rtps/src/dialect.rs` (`RtpsBehaviour`, `set_peer_behaviour`) hält herstellerabhängiges Protokollverhalten pro Peer (siehe [interop-and-dialects.md](interop-and-dialects.md)).

## 6 Durability und History

| Stufe | Umsetzung |
|---|---|
| History | `HistoryCache` (`crates/rtps/src/history_cache.rs:505`) mit `HistoryKind::KeepAll` (fest begrenzt, Fehler bei Überlauf) oder `KeepLast(depth)` (Ringpuffer). Writer-seitig über `set_cache_history`. |
| `VOLATILE` | Nach jedem Schreiben `ReliableWriter::remove_acknowledged` (`runtime.rs`, in `write_user_sample_with_key`). |
| `TRANSIENT_LOCAL` | Zusätzlich `slot.retained` mit `RetainedSample` pro Instanz, Tiefe über `enforce_retained_depth` (`runtime.rs:457`). Beim Match eines spät hinzukommenden Readers wiederholt `wire_writer_to_remote_reader` die Historie. |
| `TRANSIENT` / `PERSISTENT` im Writer | `DataWriter` mit `durability_backend` speichert den rohen Body (`crates/dcps/src/durability_service.rs`); beim Match liefert `replay_for_topic` die Samples mit neuen SNs nach. |
| Eigenständiger Dienst | `crates/durability-service` (ADR 0009): Reader `RELIABLE/TRANSIENT_LOCAL/KEEP_ALL`, Ablage in einem `DurabilityStore` (`crates/durability-store`, Adapter file/sqlite/postgres/lakehouse), Replay über einen `TRANSIENT_LOCAL`-Writer (laut Doku-Kommentar in `crates/durability-service/src/lib.rs`). |

Lifespan: `slot.sample_insert_times` auf Writer-Seite, `expire_by_lifespan` im Housekeeping, `live_sample_expired` in `DataReader::take`.

## 7 Typ-Discovery

- Typmodell: `TypeIdentifier`, `TypeObject` (Minimal/Complete), `TypeInformation` in `crates/types` (XTypes 1.3 §7.3). Hashes über `compute_minimal_hash`/`compute_complete_hash` (`crates/types/src/hash.rs`).
- `TypeInformation` wird als PID `0x0075` (`crates/rtps/src/parameter_list.rs:209`) in SEDP übertragen.
- `Subscriber::create_datareader` registriert den lokalen TypeObject beim `TypeLookupServer`, damit ein Peer `getTypes` beantwortet bekommt (`crates/dcps/src/subscriber.rs:1172`, Kommentar „Gap 2 (#24)“).
- Entscheidung beim Match: `decide_type_consistency` (`runtime.rs:24407`). Gleiche `TypeIdentifier` gelten als konsistent. Fehlt der TypeObject im Register, ergibt sich `TypeConsistency::Unresolved`; die Runtime sendet `send_type_lookup_request` (`runtime.rs:10332`) und merkt das Paar in `type_resolution_pending`. Nach `TYPE_RESOLUTION_GRACE` (3 s, `runtime.rs:182`) erzwingt `expire_type_resolution_grace` die Entscheidung (XTypes §7.6.3.4.2 Schritt 2). Sonst prüft `TypeMatcher::match_types` (`crates/types/src/type_matcher.rs`) Assignability nach den `TypeConsistencyEnforcement`-Flags.
- TypeLookup-Dienst: `crates/discovery/src/type_lookup/{client,server,endpoints}.rs`, Transport über `crates/dcps/src/type_lookup_transport.rs`, Eingang über `dispatch_type_lookup_datagram` (`runtime.rs:30540`) auf dem Nutz- und dem Metatraffic-Pfad.
- Ohne generierten Code: `crates/types/src/dynamic` (`DynamicType`, `DynamicData`) und `crates/wire-codec` (`encode`/`decode` entlang des Layout-Plans). `crates/recorder-decode` nutzt diesen Weg.

## 8 Content-Filter

- Parser und Auswertung: `crates/sql-filter` (DDS 1.4 Annex B.2.1). `parse(&str) -> Expr`, `Expr::evaluate(row, params)` gegen `RowAccess`.
- `DomainParticipant::create_contentfilteredtopic` (`crates/dcps/src/participant.rs:2488`) erzeugt `ContentFilteredTopic<T>` (`crates/dcps/src/topic.rs:1517`). Der Konstruktor parst den Ausdruck und prüft die `%N`-Indizes. `evaluate` wertet gegen ein `RowAccess` aus; `DdsTypeRow` (`dds_type.rs:687`) implementiert `RowAccess` über `DdsType::field_value`.
- `DataReader::with_filter` (`subscriber.rs`) setzt einen Rust-Closure-Filter, den `take`/`read` über `sample_passes_filter` anwenden.
- `Subscriber::create_datareader_with_cft` legt den Reader auf dem verwandten Topic an, filtert mit dem Ausdruck und den jeweils aktuellen Parametern und trägt den Filter als `PID_CONTENT_FILTER_PROPERTY` in die SEDP-Subscription (`UserReaderConfig::content_filter`). `ContentFilteredTopic::set_filter_parameters` kündigt die Subscription neu an (`DcpsRuntime::update_user_reader_content_filter`). Ein Sample, gegen das der Ausdruck nicht ausgewertet werden kann (Feld, das `field_value` nicht beantwortet), wird nicht zugestellt.
- ZeroDDS-Writer filtern nicht schreiberseitig; jeder Reader filtert selbst, auch nach einem fremden Writer, der schreiberseitig filtert.
- Parameter (`%N`) werden nach DDS 1.4 Annex B als Literal gelesen (`Value::from_parameter`): `42` ist eine Ganzzahl, `'42'` Text.
- `QueryCondition` (`crates/dcps/src/condition.rs`) nutzt ebenfalls `sql-filter`.
- C-API: `zerodds_sub_create_datareader_with_cft` kündigt den Filter ebenso an. Die Felder liest der Filter über das `CompleteTypeObject` (`zerodds_cft_set_type_object`, von `ContentFilteredTopic<T>` in C++ aus `topic_type_support<T>::type_object()` gesetzt) oder über ein positionales Schema (`zerodds_cft_set_schema`); ein Ausdruck, der ein Feld liest, ohne beides, lässt den Reader nicht entstehen. Für einen Typ, der andere Typen enthält, nimmt `zerodds_cft_set_type_objects` die Abhängigkeiten mit; `idlc cpp` gibt sie als `topic_type_support<T>::type_object_dependency(i)` aus. `zerodds_cft_set_filter_parameters` ersetzt die Parameter; bestehende Reader filtern ab dem nächsten Take damit und kündigen sie neu an (C++: `ContentFilteredTopic::filter_parameters`).
- Weitere Nutzer von `sql-filter`: `crates/zerodds-c-api` (`entities.rs`, `subscriber_ffi.rs`, `condition_ffi.rs`), `crates/routing-service/src/transform/sqlbridge.rs`.

## 9 Same-Host, SHM und Zero-Copy

| Weg | Feature / Einstellung | Code | Serialisiert? |
|---|---|---|---|
| Same-Host-SHM neben UDP | `same-host-shm` | `same_host_channel.rs` (ein Kanal je Same-Host-Teilnehmer, angelegt bei der SPDP-Meldung), `same_host_shm.rs` (Segment-Konfiguration); Senden über `RoutedUserTransport` und `send_metatraffic`, Empfang `recv_same_host_loop` | ja, jedes Unicast-Datagramm an den Peer über `PosixShmTransport` |
| SHM als Nutztransport | `transport.user_transport = SHM` (`UserTransportKind::Shm`) | `shm_user.rs` (`ShmUserTransport`), `layered_transport.rs` (`LayeredUserTransport`: SHM bevorzugt, UDP als Rückfall) | ja |
| UDS | `same-host-uds`, `UserTransportKind::Uds` | `crates/transport-uds` | ja; der Kernel kopiert |
| FlatData | `flatdata-integration` | `flatdata_integration.rs` (`DataWriter::write_flat`, `DataReader::read_flat`), `crates/flatdata` (`FlatStruct`, `PosixSlotAllocator`) | nein, `repr(C)`-Slots im Shared Memory |
| iceoryx2 | `same-host-iceoryx2` → `zerodds-flatdata/iceoryx2-bridge` | `crates/flatdata/src/iceoryx.rs` | nein |
| Cyclone iceoryx (POSH) | `cyclone-iox`, nur Linux | `cyclone_iox_integration.rs`, `crates/iceoryx-cyclone` | XCDR1-Body in PSMX-Chunk |
| UDP-Batch-Empfang | `recvmmsg-batch` (Standard in `transport-udp`) | `crates/transport-udp/src/recv_batch.rs`, `Transport::recv_batch` | — |

`DeliveryMode` (`runtime.rs:1493`: `Portable`, `RawSameHost`, `Iceoryx`) wird aus `delivery.mode` gelesen. Es gilt für loanende Writer: die Loan-Writer der C-API (`crates/zerodds-c-api/src/shm_loan_ffi.rs`, `default_mode`) und die RMW-Publisher und -Subscriptions mit loanbarem Typ. `DataWriter::write` in Rust und das kopierende `write` der C-API sind immer `Portable`. `Iceoryx` braucht je Writer einen iceoryx2-Service (`zerodds_dw_enable_iceoryx`; das RMW leitet ihn aus dem Topic ab); ein Writer ohne Service sendet `Portable` und meldet das einmal auf stderr, das RMW meldet jeden Rückfall mit Grund über `RCUTILS_LOG_WARN` (`zerodds-delivery-modes-1.0` §4).

Spezifikationen: `docs/specs/zerodds-zero-copy-1.0.md`, `docs/specs/zerodds-flatdata-1.0.md`, `docs/specs/zerodds-delivery-modes-1.0.md`, ADR 0003–0006 in `docs/adr/`.

## 10 Security

- SPI: Traits `AuthenticationPlugin`, `AccessControlPlugin`, `CryptographicPlugin`, `LoggingPlugin`, `DataTaggingPlugin` in `crates/security/src/`.
- Implementierungen: `security-pki` (Authentifizierung, X.509), `security-keyexchange` (X25519/P-256), `security-crypto` (AES-GCM/HMAC), `security-permissions` (Governance/Permissions-XML), `security-logging`, `security-rtps` (Secure-Submessage-Wrapper, §7.3.6).
- Laufzeit: `zerodds_security_runtime::SharedSecurityGate` steht in `RuntimeConfig::security` (`runtime.rs:1910`). Ohne Feature `security` sind die Hooks Durchreicher.
- Einhängepunkte in `crates/dcps/src/runtime.rs`:
  - ausgehend: `secure_outbound_for_target` (`5621`) je Ziel-Locator im UDP-Versand; derselbe Versand trägt über `RoutedUserTransport` auch den Same-Host-SHM-Kanal (`9227`), der inproc-Pfad schützt ein Datagramm ebenso für den Locator des Peers (`deliver_in_process`, `5897`); Schutzstufe je Topic über `gate.data_protection(topic)`.
  - eingehend: `secure_inbound_bytes` (`5330`) in allen Empfangsschleifen; SPDP und Stateless-Nachrichten sind von der Nachrichtenverschlüsselung ausgenommen (Kommentar mit Verweis auf DDS-Security §8.4.2.4).
  - Handshake und Schlüsseltausch: `SecurityBuiltinStack` (`crates/discovery/src/security/stack.rs`, Stateless- und Volatile-Secure-Endpunkte), Eingang `dispatch_security_builtin_datagram` (`30105`), Ausgang im Tick.
  - Zugriffskontrolle beim Matching: `check_remote_datareader` in `wire_writer_to_remote_reader`.
- Feature-Kette: `dcps/security`, `dcps/fips` → `security-runtime/fips`, `dcps/wolfcrypt` → `security-runtime/wolfcrypt`.

Details zu Konfiguration und Governance: [security.md](security.md).

## 11 Dienste und Bridges

Dienste und Bridges verwenden die öffentliche API von `crates/dcps` wie eine Anwendung (geprüft an den Abhängigkeiten in den `Cargo.toml`): `routing-service`, `durability-service`, `rpc`, `zenoh-bridge`, die Protokoll-Bridges und `zerodds-c-api`. `recorder` hat keine ZeroDDS-Abhängigkeit; die Werkzeuge `tools/record`, `tools/replay`, `tools/spy` verbinden ihn mit DCPS. Beschreibung der einzelnen Crates: [crates-core.md](crates-core.md) und [crates-bridges-services.md](crates-bridges-services.md).
