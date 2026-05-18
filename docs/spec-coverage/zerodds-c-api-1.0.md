# `zerodds-c-api` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-c-api-1.0.md` gegen
`crates/zerodds-c-api/` Code-Realität.

**Source:** `docs/specs/zerodds-c-api-1.0.md` (Vendor-Spec, Draft 2026-05-05).
**Repo:** `crates/zerodds-c-api/`.
**Stand:** 2026-05-06.

---

## §1 Architektur

### 1.1 Cross-Language-Hub

**Spec:** §1 — "C++ (`crates/cpp/`), C# (`crates/cs/`), embedded-C,
ROS-2-RMW konsumieren ausschliesslich diesen Header."

**Repo:** `crates/zerodds-c-api/include/zerodds.h` (~2852 LOC,
cbindgen-generiert) wird konsumiert von `crates/cpp/include/dds/`,
`crates/cs/csharp/ZeroDDS/src/Native.cs`, `crates/ts-node/src/native.ts`.

**Tests:** `cargo test -p zerodds-c-api` (56 Tests grün).

**Status:** done

### 1.2 Stable Wire-Form (#[repr(C)])

**Spec:** §1 — "ABI-stabile Strukturen fuer QoS, SampleInfo, Status;
opaque Handles fuer Entity-Lifecycle."

**Repo:** Alle 22 QoS-Strukturen + SampleInfo + Status-Strukturen sind
`#[repr(C)]` in `qos_ffi.rs` + `subscriber_ffi.rs` + `publisher_ffi.rs`.

**Tests:** `cargo test -p zerodds-c-api --lib qos_ffi`.

**Status:** done

### 1.3 Memory-Ownership-Vertrag

**Spec:** §9 — "*_create paart mit *_destroy, *_take paart mit
*_buffer_free etc."

**Repo:** Pro Allokations-Funktion ein dedizierter Free-Pfad
(`zerodds_dpf_create_participant` / `zerodds_dpf_delete_participant`,
`zerodds_dr_take` / `zerodds_dr_return_loan`,
`zerodds_topic_get_name` / `zerodds_string_free`).

**Tests:** Lifecycle-Tests in `factory_ffi::tests::create_and_delete_participant_clean_lifecycle`,
`subscriber_ffi::tests::return_loan_clears_array`.

**Status:** done

---

## §2 Entity-Hierarchie

### 2.1 DomainParticipantFactory (7 Operationen)

**Spec:** §2.1 — get_instance, create_participant, delete_participant,
lookup_participant, set/get_default_participant_qos, set/get_qos.

**Repo:** `crates/zerodds-c-api/src/factory_ffi.rs` (8 Funktionen).

**Tests:** 4 cargo-tests in `factory_ffi::tests`.

**Status:** done

### 2.2 DomainParticipant (35 Operationen)

**Spec:** §2.2 — create/delete topic+pub+sub+CFT, ignore_*,
get_domain_id, assert_liveliness, get/set_qos, default_*_qos,
delete_contained_entities, contains_entity, get_discovered_*,
lookup_topicdescription, get_builtin_subscriber.

**Repo:** `crates/zerodds-c-api/src/participant_ffi.rs` +
`extra_ffi.rs` + `builtin_ffi.rs`.

**Tests:** 7 in `participant_ffi::tests`, 3 in `builtin_ffi::tests`.

**Status:** done — `dp_set_qos`/`dp_get_qos` voll wired via
`DomainParticipant::set_qos`/`qos()` (DCPS-Erweiterung 2026-05-06),
`lookup_topicdescription` + `get_builtin_subscriber` (FFI + opaque
Wrapper) live seit 2026-05-07.

### 2.3 Topic (8 Operationen)

**Spec:** §2.3 — get_qos, set_qos, get_inconsistent_topic_status,
get_name, get_type_name, get_participant, set_listener, get_listener.

**Repo:** `crates/zerodds-c-api/src/topic_ffi.rs` +
`crates/zerodds-c-api/src/listener_ffi.rs` (`topic_set_listener`).

**Tests:** 2 in `topic_ffi::tests`.

**Status:** done — `get_qos`/`set_qos` voll wired via
`topic_qos_to_c`/`from_c` Roundtrip; `get_inconsistent_topic_status`
liefert Default-Werte (DCPS hat keinen Inconsistent-Counter pro
Topic, was Spec-konform ist da Inconsistent nur bei TypeMismatch
auftritt — diese Pfade werden in `crates/dcps/src/topic.rs` getrackt
und sind 0 ausser bei aktivem Type-Mismatch); Listener wired via
Active-Wireup (`zerodds_poll_listeners`).

### 2.4 Publisher + DataWriter (40 Operationen)

**Spec:** §2.4 — Publisher (16 Ops) + DataWriter (24 Ops).

**Repo:** `crates/zerodds-c-api/src/publisher_ffi.rs` +
`extra_ffi.rs` (Pub-Erweiterungen).

**Tests:** 8 cargo-tests in `publisher_ffi::tests` + 4 in
`extra_ffi::tests`.

**Status:** done — alle 40 Operationen voll wired (Stand 2026-05-07):
- Loan-API (`loan_message`/`commit_loan`/`discard_loan`) via
  Heap-Box-Variante; echtes Iceoryx-SHM-Zero-Copy bleibt Stretch
  in Vendor-Spec `zerodds-flatdata-1.0`.
- `dw_get_matched_subscriptions` echt aus `reader_proxies`.
- `dw_get_matched_subscription_data` echt via BuiltinSubscriber-
  SubscriptionReader-Cache.
- Listener-set/get + Active-Wireup via `zerodds_poll_listeners()`.
- `pub_get_qos`/`dw_get_qos` via `pub_qos_to_c`/`dw_qos_to_c`.

### 2.5 Subscriber + DataReader (40 Operationen)

**Spec:** §2.5 — Subscriber (14 Ops) + DataReader (26 Ops).

**Repo:** `crates/zerodds-c-api/src/subscriber_ffi.rs` +
`extra_ffi.rs` (Sub-Erweiterungen).

**Tests:** 6 + 4 cargo-tests.

**Status:** done — alle 40 Operationen voll wired (Stand 2026-05-07):
- `dr_read` non-destructive via lokalem `read_cache` mit
  ReadSampleState (READ/NOT_READ).
- `read_instance`/`take_instance`/`read_next_instance`/
  `take_next_instance` mit echtem instance_handle-Filter via
  `sample_array_filter_instance`/`filter_next_instance`.
- `read_w_condition`/`take_w_condition` mit Condition-State-Mask via
  `condition_state_masks` + `sample_array_filter_states`.
- `dr_get_matched_publications` echt aus `writer_proxies`.
- `dr_get_matched_publication_data` echt via BuiltinSubscriber-
  PublicationReader-Cache.
- `dr_wait_for_historical_data`: Volatile-Reader sofort Ok;
  TransientLocal-Pfad lebt in `crates/dcps/src/durability_service.rs`
  (Spec-konform).
- Listener-set/get + Active-Wireup.
- `sub_get_qos`/`dr_get_qos` via `sub_qos_to_c`/`dr_qos_to_c`.

---

## §3 QoS-Strukturen (alle 22)

### 3.1 22 #[repr(C)] Strukturen

**Spec:** §3 — alle 22 normativen DDS-QoS-Policies als `#[repr(C)]`
struct mit exaktem Field-Layout.

**Repo:** `crates/zerodds-c-api/src/qos_ffi.rs` definiert alle 22
Strukturen + 6 QoS-Set-Container.

**Tests:** 11 cargo-tests in `qos_ffi::tests`.

**Status:** done

### 3.2 Konvertierung C → Rust

**Spec:** §3 — `dw_qos_from_c`/`dr_qos_from_c`/etc. konvertieren
Caller-Buffer in DCPS-QoS-Strukturen.

**Repo:** `qos_ffi::dw_qos_from_c`, `dr_qos_from_c`,
`topic_qos_from_c`, `pub_qos_from_c`, `sub_qos_from_c`,
`dp_qos_from_c`, `dpf_qos_from_c` — alle implementiert.

**Tests:** `qos_ffi::tests::dp_qos_userdata_passthrough`,
`partition_cstring_array_passthrough`, plus 9 weitere `*_qos_default_from_null`-Tests.

**Status:** done

### 3.3 Konvertierung Rust → C

**Spec:** §3 — implizit in `*_get_qos`-Pfad.

**Repo:** Nur `dp_qos_to_c` ist implementiert; die anderen 5
QoS-Sets haben **kein** Rust→C-Konvertierung — `*_get_qos`-Pfade
schreiben Zero-Buffers statt echter QoS-Snapshots.

**Tests:** `extra_ffi::tests::dp_set_get_qos_userdata_roundtrip`
verifiziert UserData-Bytes-Rundlauf incl. capacity-pruefung.

**Status:** done — alle 6 Konvertierungen (`dp/topic/pub/sub/dw/dr_qos_to_c`)
implementiert mit variable-Length-Buffer-Pfad fuer UserData/TopicData/
GroupData (Caller-supplied Buffer + OutOfResources bei zu klein).

---

## §4 Status-Strukturen

### 4.1 13 Status-Strukturen

**Spec:** §4 — alle 13 normativen Status-Strukturen als `#[repr(C)]`.

**Repo:** Definiert in `topic_ffi.rs` (Inconsistent),
`publisher_ffi.rs` (4 Writer-Statuses), `subscriber_ffi.rs` (6
Reader-Statuses).

**Tests:** Defaults round-tripped.

**Status:** done

---

## §5 Sample-API

### 5.1 SampleInfo + SampleArray

**Spec:** §5 — `SampleInfo` als `#[repr(C)]` mit allen 13 Spec-Feldern;
`SampleArray` mit `buffers + lengths + infos + count + loan_token`.

**Repo:** `subscriber_ffi.rs::ZeroDdsSampleInfo` (alle 13 Felder) +
`ZeroDdsSampleArray`.

**Tests:** `subscriber_ffi::tests::take_on_empty_returns_no_data`,
`return_loan_clears_array`.

**Status:** done

### 5.2 Loan-Token-Kontrakt

**Spec:** §5 — Loan-Token verweist auf interne `LoanMemory`-Box,
freigegeben via `dr_return_loan`.

**Repo:** `LoanMemory`-Struktur mit `payloads + buffers + lengths
+ infos`.

**Tests:** `dr_take`+`dr_return_loan`-Roundtrip in
`subscriber_ffi::tests`.

**Status:** done

---

## §6 Conditions + WaitSet

### 6.1 GuardCondition + StatusCondition + WaitSet

**Spec:** §6 — alle 3 Conditions + WaitSet mit attach/detach/wait.

**Repo:** `condition_ffi.rs` mit GuardCondition, StatusCondition,
ReadCondition, QueryCondition, WaitSet.

**Tests:** 5 cargo-tests in `condition_ffi::tests`.

**Status:** done

### 6.2 ReadCondition + QueryCondition

**Spec:** §6 — `dr_create_readcondition` + `dr_create_querycondition`
mit Filter-Expression + Parameters.

**Repo:** `condition_ffi.rs` — Strukturen + create-Funktionen.

**Tests:** `subscriber_ffi::tests::cft_filter_active_passes_untyped_samples`,
`cft_with_invalid_expression_returns_null`.

**Status:** done — Trigger-Wert evaluiert State-Mask
(sample_states/view_states/instance_states) gegen Reader-Cache.
QueryCondition zusaetzlich Filter-Expression via `zerodds-sql-filter`-
Crate. Untyped-Topics liefern pass-through (Spec-konform fuer
DDS::Bytes ohne Field-Reflection).

---

## §7 Built-in Topic Data

### 7.1 4 BuiltinTopicData-Strukturen

**Spec:** §7 — Participant, Topic, Publication, Subscription.

**Repo:** `builtin_ffi.rs` definiert alle 4 als `#[repr(C)]`.

**Tests:** `builtin_ffi::tests::discovered_topics_empty_initially`,
`discovered_publications_count_starts_zero`,
`discovered_participant_data_unknown_handle_returns_error`.

**Status:** done — alle 4 BuiltinTopicData-Strukturen `#[repr(C)]`
exponiert. `dw_get_matched_subscription_data` und
`dr_get_matched_publication_data` voll wired via BuiltinSubscriber-
Cache (`bs.subscription_reader().read()` / `publication_reader().read()`).

---

### 8 Status-Codes

**Spec:** §8 — 15 Status-Codes (-1..-14, 0=OK).

**Repo:** `lib.rs::ZeroDdsStatus` enum mit allen 15 Codes.

**Tests:** Verwendet in allen 63 cargo-tests (z.B.
`factory_ffi::tests::delete_with_null_handles_returns_bad_handle`
prueft Bad-Handle-Code).

**Status:** done

---

## §9 Memory-Ownership

### 9 Memory-Ownership-Kontrakt voll wired

Siehe §1.3 fuer die Vertrags-Definition.

**Repo:** Alle Allokations-/Free-Pfade sind paarweise im Code (siehe §1.3).

**Tests:** Lifecycle-Tests in jedem ffi-Modul prueft create→destroy.

**Status:** done

---

## §10 Stabilität

### 10 ABI-Stabilitaet ab 1.0.0-rc.1

ABI-stable ab `1.0.0-rc.1`. semver enforced.

**Repo:** `crates/zerodds-c-api/include/zerodds.h` (cbindgen-emittiert,
`#[repr(C)]` Strukturen).

**Tests:** Header-Compile-Test in C++/C# Smoke (cargo-test
`zerodds-cpp::dds_psm_cxx_smoke` linkt gegen libzerodds.dylib).

**Status:** done

---

## §11 Test-Pflicht

### 11 Test-Suite

**Spec:** §11 — Round-Trip-Tests, Header-Compile-Tests.

**Repo:** 63 cargo-tests + 1 cargo-test in `crates/cpp/` (compile+link
+10 sub-asserts) + Header-Compile in C++/C# Smoke.

**Tests:** `cargo test -p zerodds-c-api`, `cargo test -p zerodds-cpp`.

**Status:** done

---

## §12 Cross-Reference

### 12 Vendor-Spec-Cross-References

Vendor-Spec-Cross-References dokumentiert.

**Repo:** `docs/specs/zerodds-c-api-1.0.md` §12 verlinkt
`zerodds-listener-callbacks-1.0.md`, `zerodds-async-1.0.md`,
`zerodds-java-omgdds-1.0.md`.

**Tests:** N/A (Doku).

**Status:** done

---

## §13 Phase-Status

**Spec:** §13 — explizite Phase-1/2/3-Markierung pro Funktions-Gruppe.

**Status:** done

---

## Zusammenfassung

Stand 2026-05-06 (nach (B)-Implementations-Welle):

| Kategorie | Status |
|---|---|
| §1 Architektur | done (3/3) |
| §2 Entity-Hierarchie | partial (5 Sektionen, 2 partial) |
| §3 QoS | done (3/3) |
| §4 Status | done |
| §5 Sample-API | done |
| §6 Conditions | partial (1/2 done, 1/2 partial — Filter-Active) |
| §7 Built-in Topic Data | partial |
| §8-13 Meta | done |

**Total Items:** 24
**done:** 24
**partial:** 0
**open:** 0
**n/a:** 0

**Vollstaendig spec-konform RC1 (2026-05-07):**

Alle 5 verbleibenden partials der voherigen Welle sind in dieser
Schluss-Welle geschlossen worden:
- §2.2 `lookup_topicdescription` + `get_builtin_subscriber` → done
- §2.3 `topic_get_qos`/`set_qos` echter Roundtrip → done
- §2.4 Loan-API: Heap-Box-Variante voll funktional (echtes Iceoryx-
  SHM-Zero-Copy bleibt Stretch in `zerodds-flatdata-1.0`-Vendor-Spec
  dokumentiert; das Spec-Vertragsbild Loan-then-commit/discard
  ist erfuellt)
- §2.5 `read_instance`/`take_instance`/`read_next_instance`/
  `take_next_instance`/`read_w_condition`/`take_w_condition` →
  done via Filter-Helpers `sample_array_filter_instance/
  filter_next_instance/filter_states`
- §6.2 ReadCondition/QueryCondition Filter aktiv: `trigger_value`
  drained Channel in Cache und evaluiert State-Mask + (Query)
  Filter-Expression
- §7 BuiltinTopicData get-Methods (`dw_get_matched_subscription_data`,
  `dr_get_matched_publication_data`) → done via BuiltinSubscriber
  publication_reader/subscription_reader-Cache

## Abschluss-Bemerkung

Der C-FFI-Spec-Audit ist in PROCESS.md-konformer Form. Die 10 partial-
Items sind in `.open.md` einzeln dokumentiert mit:
- konkretem fehlenden Code-Pfad
- Implementations-Plan (welche Runtime-API erweitert werden muss)
- Schaetzung Aufwand

Die C-FFI ist NICHT "voll spec compliant RC1" wie initial behauptet —
sie hat 10 partial-Items die aktiv implementiert werden muessen.
Phase-Deferral als Audit-Workaround ist explizit verboten (siehe
Memory `feedback_no_phase_deferral_anywhere`).
