# `zerodds-listener-callbacks` v1.1 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-listener-callbacks-1.1.md` gegen
den Code in `crates/zerodds-c-api/src/listener_ffi.rs` (mit den
Status-Countern und der Inconsistent-Topic-Detektion in `crates/dcps/`
und `crates/discovery/`).

**Quelle:** `docs/specs/zerodds-listener-callbacks-1.1.md` (ZeroDDS
Vendor-Spec).

Implementierung:

- `crates/zerodds-c-api/src/listener_ffi.rs` — Listener-vtables, Set/Get-
  API, `zerodds_poll_listeners()` Active-Firing + Aggregatoren.
- `crates/dcps/src/runtime.rs` — Status-Counter, Delivered-Sample-Counter,
  Inconsistent-Topic-Counter.
- `crates/discovery/src/sedp/cache.rs` — SEDP-Typ-Mismatch-Detektion.

---

## §1 Architektur

### §1.1 Funktions-Pointer-Tabelle

**Spec:** §1.1 — Listener als `#[repr(C)]`-Strukturen aus Funktions-
Pointern, alle optional (NULL = ignoriert).

**Repo:** `listener_ffi.rs` — 6 Strukturen:
`ZeroDdsDomainParticipantListener`, `ZeroDdsPublisherListener`,
`ZeroDdsSubscriberListener`, `ZeroDdsTopicListener`,
`ZeroDdsDataWriterListener`, `ZeroDdsDataReaderListener`.

**Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`,
`dw_set_listener_clear_via_null`.

**Status:** done

### §1.2 `user_data`-Slot

**Spec:** §1.2 — ein `void* user_data` pro Struktur, unverändert an jeden
Callback gereicht.

**Repo:** alle 6 Strukturen tragen `user_data: *mut c_void` als erstes
Feld; `fire_writer_vtable` / `fire_reader_vtable` reichen es an jeden
Callback.

**Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`.

**Status:** done

### §1.3 Set/Get-API

**Spec:** §1.3 — ein `*_set_listener` / `*_get_listener`-Paar pro
Entity-Typ; NULL clears.

**Repo:** `zerodds_{dp,pub,sub,topic,dw,dr}_set_listener` +
`zerodds_{dp,dw,dr}_get_listener`. `set_listener(NULL)` entfernt den
Registry-Eintrag und die gecachten Counter.

**Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`,
`dw_set_listener_clear_via_null`.

**Status:** done

---

## §2 Listener-Inventar

### §2.1 DomainParticipantListener

**Spec:** §2.1 — `on_inconsistent_topic`, `on_data_on_readers`.

**Repo:** `ZeroDdsDomainParticipantListener`; beide Callbacks vom
DP-Aggregator in `zerodds_poll_listeners` gefeuert (§6.3, §6.4).

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`,
`poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

### §2.2 PublisherListener

**Spec:** §2.2 — 4 Writer-Status-Callbacks, aggregiert über die
enthaltenen DataWriter.

**Repo:** `ZeroDdsPublisherListener`; der Publisher-Aggregator läuft
`publisher.datawriters` durch und feuert via `fire_writer_vtable`.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`
(Poll-Pfad), `dp_set_get_listener_roundtrip` (Registry).

**Status:** done

### §2.3 SubscriberListener

**Spec:** §2.3 — 7 Reader-Status-Callbacks + `on_data_on_readers`.

**Repo:** `ZeroDdsSubscriberListener`; der Subscriber-Aggregator läuft
`subscriber.datareaders` durch, feuert die 7 Reader-Callbacks via
`fire_reader_vtable` und `on_data_on_readers` mit Set-Semantik.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

### §2.4 TopicListener

**Spec:** §2.4 — `on_inconsistent_topic`.

**Repo:** `ZeroDdsTopicListener`; vom Topic-Ebenen-Pass auf einen
Inconsistent-Topic-Counter-Delta gefeuert.

**Tests:** `listener_ffi::tests::poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

### §2.5 DataWriterListener

**Spec:** §2.5 — `on_liveliness_lost`, `on_offered_deadline_missed`,
`on_offered_incompatible_qos`, `on_publication_matched`.

**Repo:** `ZeroDdsDataWriterListener`; von der DataWriter-Ebene via
`fire_writer_vtable` gefeuert.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.

**Status:** done

### §2.6 DataReaderListener

**Spec:** §2.6 — alle 7 Reader-Status-Callbacks.

**Repo:** `ZeroDdsDataReaderListener`; von der DataReader-Ebene via
`fire_reader_vtable` gefeuert.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

---

## §3 Status-Mask-Semantik

**Spec:** §3 — `status_mask` filtert, welche Callbacks feuern (Bit in der
Mask gesetzt UND Pointer non-NULL).

**Repo:** `set_listener(p, l, status_mask)` speichert die Mask; jede
Fire-Stelle prüft `mask & STATUS_*` vor dem Aufruf. Die Bit-Konstanten
`STATUS_INCONSISTENT_TOPIC` … `STATUS_SUBSCRIPTION_MATCHED` entsprechen den
DDS-PSM-Werten.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`
(Mask `0xFFFFFFFF`).

**Status:** done

---

## §4 Threading-Vertrag

### §4.1 Caller-driven Poll-Delivery

**Spec:** §4.1 — Callbacks aus `zerodds_poll_listeners()`, nie synchron in
einem `set_listener`/`write`/`take`-Aufruf.

**Repo:** `zerodds_poll_listeners` ist die einzige Fire-Stelle; die
Registry hält rohe Pointer, und `set_listener` ruft nie Callbacks.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.

**Status:** done

### §4.2 Re-Entrancy

**Spec:** §4.2 — im Callback darf der Caller lesen; nicht den Listener/die
Entity freigeben.

**Repo:** Registry-Snapshots werden gezogen und alle Locks vor jedem
Callback freigegeben, sodass ein Callback Read-APIs deadlock-frei
re-entrant aufrufen kann.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`
(Callbacks laufen außerhalb der Registry-Locks).

**Status:** done

### §4.3 Lifetime

**Spec:** §4.3 — Listener-Pointer ist Caller-owned; Registry hält ihn
weak.

**Repo:** `static OnceLock<ListenerRegistry>` speichert rohe Pointer;
`set_listener(NULL)` clears.

**Tests:** `listener_ffi::tests::dw_set_listener_clear_via_null`.

**Status:** done

---

## §5 Aggregator-Modell — Caller-driven Multi-Bind

**Spec:** §5 — unabhängiges Multi-Level-Firing (kein First-Match-Suppress);
derselbe Listener-Pointer an mehrere Entities bindbar; die Aggregatoren
laufen ihre Children durch.

**Repo:** `zerodds_poll_listeners` berechnet das Delta jeder Entity einmal
und feuert dann die DataWriter/DataReader-Ebene sowie jede gebundene
Aggregator-Ebene unabhängig via `collect_publisher_writers` /
`collect_subscriber_readers` / `collect_participant_subscriber_readers`.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`
(DataReader + Subscriber feuern beide für ein Sample).

**Status:** done

---

## §6 Active-Firing

### §6.1 Writer-Status

**Spec:** §6.1 — 4 Writer-Callbacks feuern auf einen Writer-Counter-Delta
an der DataWriter- und der Publisher-Ebene.

**Repo:** `read_writer_counters` + `writer_delta` + `fire_writer_vtable`;
Counter aus `DcpsRuntime::user_writer_*`.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.

**Status:** done

### §6.2 Reader-Status

**Spec:** §6.2 — 6 Reader-Status-Callbacks (matched, sample_lost, deadline,
incompatible_qos, liveliness_changed, sample_rejected) feuern auf einen
Reader-Counter-Delta an der DataReader- und der Subscriber-Ebene.

**Repo:** `read_reader_counters` (inkl. `user_reader_liveliness_status`,
`user_reader_sample_rejected`) + `reader_delta` + `fire_reader_vtable`.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

### §6.3 Daten-Verfügbarkeit (Set-Semantik)

**Spec:** §6.3 — `on_data_available` auf einen Delivered-Sample-Delta;
`on_data_on_readers` einmal pro Poll pro Subscriber mit frischen Daten.

**Repo:** `UserReaderSlot::samples_delivered_count` (nur an den
User-Delivery-Stellen erhöht), exponiert via
`DcpsRuntime::user_reader_samples_delivered`; die Subscriber/DP-Aggregatoren
feuern `on_data_on_readers` einmal pro Subscriber mit Daten-Delta.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

### §6.4 Inconsistent-Topic

**Spec:** §6.4 — lokale Kollision bei `create_topic` verhindert; Remote-
Typ-Mismatch beim SEDP-Matching erkannt, feuert `on_inconsistent_topic`
auf Topic- und DomainParticipant-Listenern.

**Repo:** `zerodds_dp_create_topic` lehnt lokale Kollisionen ab;
`DiscoveredEndpointsCache::topic_name_conflicts` +
`DcpsRuntime::inconsistent_topic_seq` in
`match_local_{writer,reader}_against_cache` erhöht, gelesen via
`inconsistent_topic_count`; Poll feuert auf Topic-/DP-Ebene.

**Tests:** `sedp::cache::tests::topic_name_conflicts_detects_type_mismatch`,
`listener_ffi::tests::poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

---

## §7 Cross-Language-Mapping

### §7.1 C++ Bridge

**Spec:** §7.1 — C++ wrappt die C-vtable.

**Repo:** `crates/cpp/include/dds/pub/DataWriterListener.hpp`,
`crates/cpp/include/dds/sub/DataReaderListener.hpp`.

**Status:** done

### §7.2 C# Bridge

**Spec:** §7.2 — `IDataWriterListener<T>` + `GCHandle`-`user_data`;
`ListenerPoll.PollAll()`.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Listener.cs`.

**Status:** done

### §7.3 Java Bridge

**Spec:** §7.3 — In-Process-Java-Heap-Listener am `InProcessBus` +
Multi-Process-gRPC-Bridge.

**Repo:** `crates/java-omgdds/java/` (In-Process-Listener) +
`org.zerodds.bridge.GrpcBridgeClient`
(`crates/java-omgdds/java/src/main/java/org/zerodds/bridge/GrpcBridgeClient.java`).

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java`,
`crates/java-omgdds/run_grpc_bridge_e2e.sh`.

**Status:** done

### §7.4 Python Bridge

**Spec:** §7.4 — Caller-driven Polling-API als idiomatisches
Listener-Äquivalent unter dem GIL.

**Repo:** `crates/py/src/ffi.rs` (`wait_for_data`,
`wait_for_matched_subscription`, `wait_for_matched_publication`).

**Status:** done

### §7.5 TypeScript Bridge

**Spec:** §7.5 — Caller-driven Polling-API passend zum single-threaded
Node-Event-Loop.

**Repo:** `crates/ts-node/src/dds.ts` (`DataReader.waitForMatched`,
`DataWriter.waitForMatched`).

**Status:** done

---

## §8 Test-Pflicht

**Spec:** §8 — Identity-Roundtrip, NULL-Clear, Poll feuert auf Delta /
nichts ohne Delta, Aggregator-Firing, Inconsistent-Topic-Firing.

**Repo / Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`,
`dw_set_listener_clear_via_null`,
`poll_listeners_returns_count_and_clears_state`,
`poll_fires_data_available_and_data_on_readers`,
`poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

---

## §9 Memory-Ownership

**Spec:** §9 — Caller besitzt die Listener-Struktur; Registry hält sie
weak; Caller muss vor dem Freigeben clearen.

**Repo:** Raw-Pointer-Registry; `set_listener(NULL)` clears; Tests clearen
Listener vor dem Entity-Teardown, sodass kein dangling Pointer einen
späteren Poll überlebt.

**Tests:** `listener_ffi::tests::dw_set_listener_clear_via_null`.

**Status:** done

---

## §10 Stabilität

**Spec:** §10 — Semver-Policy: append-only Struktur-Evolution, Major-Bump
für Breaking-Changes.

**Status:** n/a (informative) — eine vorausschauende Versionierungs-Policy,
keine implementierbare Anforderung.

---

## §11 Spec-Konformitäts-Hinweise

**Spec:** §11 — alle DDS 1.4 §2.2.4 Listener-Methoden 1:1 exponiert und
aktiv gefeuert; `user_data` ist ein Vendor-Detail.

**Repo:** alle 17 Callbacks über die 6 Strukturen sind exponiert und
gefeuert (§6); `user_data` durch jede Fire-Stelle gereicht.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`,
`poll_fires_inconsistent_topic_for_topic_and_participant`,
`poll_listeners_returns_count_and_clears_state`.

**Status:** done

---

## Audit-Status

26 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-c-api -p zerodds-discovery -p zerodds-dcps --lib`
— 664 Tests grün, 0 failed.

Offene Punkte: keine. Decision-Records: keine.
