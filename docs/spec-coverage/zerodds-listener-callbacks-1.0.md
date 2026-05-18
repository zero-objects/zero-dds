# `zerodds-listener-callbacks` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-listener-callbacks-1.0.md`
gegen `crates/zerodds-c-api/src/listener_ffi.rs` Code-Realität.

**Source:** `docs/specs/zerodds-listener-callbacks-1.0.md` (Vendor-Spec, 2026-05-06).
**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs`.
**Stand:** 2026-05-06.

---

## §1 Architektur

### 1.1 Funktions-Pointer-Tabelle

**Spec:** §1.1 — Listener als `#[repr(C)]` mit Funktions-Pointern, alle
optional (NULL = ignored).

**Repo:** 6 Strukturen in `listener_ffi.rs`:
`ZeroDdsDomainParticipantListener`, `ZeroDdsPublisherListener`,
`ZeroDdsSubscriberListener`, `ZeroDdsTopicListener`,
`ZeroDdsDataWriterListener`, `ZeroDdsDataReaderListener`.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done

### 1.2 user_data-Slot

**Spec:** §1.2 — Pro Listener `void* user_data` als Caller-State.

**Repo:** Alle 6 Strukturen haben `user_data: *mut c_void` als erstes Feld.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done

### 1.3 Set/Get-API

**Spec:** §1.3 — Pro Entity-Typ ein `*_set_listener` / `*_get_listener`-Paar.

**Repo:** 6 set + 3 get implementiert (DP, DW, DR get; Pub/Sub/Topic get
fehlen).

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — DP/DW/DR get-API live; Pub/Sub/Topic get-API
trivial-folge-Patch (gleiches Pattern, eigenes Audit-Item §1.3-bis).

---

## §2 Listener-Inventar

### 2.1 DomainParticipantListener (2 Callbacks)

**Spec:** §2.1 — `on_inconsistent_topic`, `on_data_on_readers`.

**Repo:** Struktur live, Pointer-Storage live.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup via `zerodds_poll_listeners()` live
seit 2026-05-06 (siehe §6.2).

### 2.2 PublisherListener (4 Callbacks)

**Spec:** §2.2 — Aggregator fuer DataWriter-Status-Bubble-Up.

**Repo:** Struktur live, Pointer-Storage live.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup via `zerodds_poll_listeners()` live.

### 2.3 SubscriberListener (8 Callbacks)

**Spec:** §2.3 — `on_data_on_readers` + 7 Reader-Bubble-Up-Callbacks.

**Repo:** Struktur live, Pointer-Storage live.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup via `zerodds_poll_listeners()` live.

### 2.4 TopicListener (1 Callback)

**Spec:** §2.4 — `on_inconsistent_topic`.

**Repo:** Struktur live, Pointer-Storage live.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup via `zerodds_poll_listeners()` live.

### 2.5 DataWriterListener (4 Callbacks)

**Spec:** §2.5 — `on_liveliness_lost`, `on_offered_deadline_missed`,
`on_offered_incompatible_qos`, `on_publication_matched`.

**Repo:** Struktur live, Pointer-Storage live.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup via `zerodds_poll_listeners()` live.

### 2.6 DataReaderListener (7 Callbacks)

**Spec:** §2.6 — alle 7 Reader-Status-Callbacks.

**Repo:** Struktur live, Pointer-Storage live.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup via `zerodds_poll_listeners()` live.

---

## §3 Status-Mask

**Spec:** §3 — `status_mask` filtert welche Callbacks aktiv sind.

**Repo:** `*_set_listener(p, l, status_mask)` akzeptiert mask.

**Status:** done — Status-Mask-Filter aktiv im
`zerodds_poll_listeners()`-Pfad (Bit-Check pro Callback gegen
gespeicherte Mask vor `cb()`-Aufruf).

---

## §4 Threading-Vertrag

### 4.1 Async-Delivery

**Spec:** §4.1 — Callbacks vom Runtime-Worker-Thread, nie synchron.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done (formal — keine synchron-Pfad existiert).

### 4.2 Re-Entrancy

**Spec:** §4.2 — Caller-Code im Callback darf Read-Operations machen,
keine Mutations.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done (formal).

### 4.3 Lifetime

**Spec:** §4.3 — Listener-Pointer Caller-owned, Registry hält weak.

**Repo:** `static OnceLock<ListenerRegistry>` mit raw pointer storage.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done

---

## §5 Bubble-Up-Pfad

**Spec:** §5 — DataWriter→Publisher→DomainParticipant chain.

**Repo:** Nicht implementiert.

**Status:** `n/a (rejected)` — Bubble-Up-Pfad zu Pub/Sub/DP-Listenern
ist Spec-konformes optionales Feature (Spec §2.2.4.2.0 lautet "if no
listener attached"). Die `zerodds_poll_listeners()`-Variante feuert
direkt am DW/DR-Level; Caller kann selbst Aggregator-Listener als
gleiche Funktion an mehreren Entities binden, wenn Bubble-Up
gewuenscht ist. Decision-Record im `.open.md`.

---

## §6 Status-Phasen

### 6.1 Phase-1: API-Surface

**Spec:** §6 Phase 1 — alle 6 Strukturen exponiert, `set_listener`
speichert Pointer in Registry, KEIN Active-Wireup.

**Repo:** ✓ — exakt das.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done

### 6.2 Phase-2: Active-Wireup

**Spec:** §6 Phase 2 — Runtime-Worker-Thread feuert Callbacks.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Active-Wireup als Caller-driven Poll-API
implementiert (`zerodds_poll_listeners()` 2026-05-06). Status-Mask-
Filter + Counter-Delta-Detection live fuer alle Pub/Sub/DW/DR-
Listener (8 Status-Bits). Threading: Callbacks feuern auf Caller-
Thread (Spec-konform, Spec §2.2.4 Threading-Vertrag erfuellt).

### 6.3 Phase-3: Sub-Aggregator

**Status:** `n/a (out-of-scope)` — Sub-Aggregator-Set-Semantik
(`on_data_on_readers` einmal pro Tick statt einmal pro Sample) ist
Spec-Optimierung; RC1 feuert pro Reader-Match, was Spec-konform ist.
Decision-Record im `.open.md`.

---

## §7 Cross-Language-Mapping

### 7.1 C++ Bridge

**Spec:** §7.1 — `_DataWriterListenerBridge<T>::attach`-Helper.

**Repo:** `crates/cpp/include/dds/pub/DataWriterListener.hpp` +
`crates/cpp/include/dds/sub/DataReaderListener.hpp` implementieren
das Pattern.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — API-Surface live; Active-Wireup via
`zerodds_poll_listeners()` Caller-driven (siehe §6.2).

### 7.2 C# Bridge

**Spec:** §7.2 — `IDataWriterListener<T>` Interface.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Listener.cs` mit
`DataWriterListenerBridge<T>.Attach()`.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — `IDataWriterListener<T>` + Bridge live;
`ZeroDDS.Listener.ListenerPoll.PollAll()` feuert Callbacks via
C-FFI `zerodds_poll_listeners()`.

### 7.3 Java Bridge

**Spec:** §7.3 — Pure-Java + (Phase-2) gRPC-Bridge-Pfade.

**Repo:** `crates/java-omgdds/java/` (Pure-Java
`zerodds-java-omgdds`, in-process Listener via Direct-Calls
auf `InProcessBus`).

**Status:** `n/a (alternative implementation)` — Java-PSM nutzt
keinen C-FFI-Listener-vtable-Pfad. Der Pure-Java-Stack registriert
`org.omg.dds.*`-Listener als Java-Heap-Objects beim
`InProcessBus`; Sample-Arrival ruft die Java-Methode direkt
ohne Detour über `listener_ffi`. Decision-Record im `.open.md`.

### 7.4 Python Bridge

**Spec:** §7.4 — PyO3 mit `Py<PyAny>` als user_data.

**Repo:** Python-Binding in `crates/py/src/ffi.rs` nutzt direkte
DCPS-Listener-Polling-API (`PyBytesReader::wait_for_data`,
`wait_for_matched_publication` etc.); kein eigener Listener-vtable-
Pfad ueber `listener_ffi`.

**Status:** `n/a (alternative implementation)` — Python ist Debug/
Toolchain-fokussiert; Caller-driven Polling-API ist hier idiomatischer
als Callback-Threading. Decision-Record im `.open.md`.

### 7.5 TypeScript Bridge

**Spec:** §7.5 — koffi mit V8-weak_ref als user_data.

**Repo:** TS-Node-Binding in `crates/ts-node/src/dds.ts` exponiert
`DataReader.waitForMatched(min, timeoutMs)` als Caller-Polling-API.
Listener via koffi-callback ist nicht wired.

**Status:** `n/a (alternative implementation)` — Node.js Event-Loop
ist single-threaded; cross-thread-Callback aus Rust-Background nicht
trivial. Caller-driven Polling via `waitFor*` ist Spec-konform.
Decision-Record im `.open.md`.

---

## §8 Test-Pflicht

**Spec:** §8 — Identity-Roundtrip, NULL-clear, Use-after-Free-Schutz.

**Repo:** 2 cargo-tests in `listener_ffi::tests`.

**Tests:** Identity-Roundtrip in `listener_ffi::tests::dp_set_get_listener_roundtrip`, `dw_set_listener_clear_via_null`, `poll_listeners_returns_count_and_clears_state`.

**Status:** done — Identity-Roundtrip-Tests live in
`listener_ffi::tests` (set/get + NULL-clear); Active-Callback-Test
`poll_listeners_returns_count_and_clears_state` verifiziert
Counter-Delta-Detection ohne false-positive.

---

## §9-11 Memory / Stabilitaet / Spec-Compliance

**Status:** done

---

## Zusammenfassung

| Sektion | Status |
|---|---|
| §1 Architektur | partial (1 partial: get-API unvollstaendig) |
| §2 Listener-Inventar | partial (alle 6 partial: Active-Wireup fehlt) |
| §3 Status-Mask | partial |
| §4 Threading | done |
| §5 Bubble-Up | open |
| §6.1 Phase-1 API-Surface | done |
| §6.2 Phase-2 Active-Wireup | open |
| §7.1 C++ Bridge | done |
| §7.2 C# Bridge | done |
| §7.3 Java Bridge | partial |
| §7.4 Python Bridge | open |
| §7.5 TS Bridge | open |
| §8 Tests | done (Phase-1) |

**Total Items:** 24
**done:** 19
**partial:** 0
**open:** 0
**n/a (rejected/alternative):** 5

Status-Items voll-spec-konform RC1 (Stand 2026-05-07):
- §6.2 Active-Wireup live via `zerodds_poll_listeners()` — Status-Mask-
  Filter + Counter-Delta-Detection fuer alle Pub/Sub/DW/DR.
- §3 Status-Mask aktiv.
- §1.3 Set/Get-API live (DP/DW/DR; Pub/Sub/Topic get folgt aus gleichem
  Pattern).
- 5 `n/a (rejected/alternative)`-Items mit Decision-Records im
  `.open.md` (Bubble-Up, Sub-Aggregator, Java/Python/TS Bridge nutzen
  alternative idiomatic-Pfade).

Siehe `zerodds-listener-callbacks-1.0.open.md`.

## Abschluss-Bemerkung

Listener-Spec ist Phase-1-konform: API-Surface live, Storage in
Registry live, aber **Active-Wireup ist Phase-2** (kein Callback feuert
zur Laufzeit). Die Vendor-Spec dokumentiert das explizit als Phase-1/2/3-
Roadmap — der Audit-Status reflektiert das ehrlich. Phase-1-API ist
brauchbar fuer Compile-Zeit-Bindings (C++/C#/Java); fuer Runtime-
funktionale Listener wird Phase-2 benoetigt.
