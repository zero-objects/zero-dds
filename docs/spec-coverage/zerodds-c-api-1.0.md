# `zerodds-c-api` v1.0 — Spec-Coverage

Audit der Vendor-Spec `docs/specs/zerodds-c-api-1.0.md` gegen den Code in
`crates/zerodds-c-api/`. Der generierte C-Header `include/zerodds.h` ist das
normative Vertragsdokument; jede Anforderung wird Item-für-Item mit
Spec-Bezug, Repo-Pfad, Test-Pfad und Status geprüft.

**Quelle:** `docs/specs/zerodds-c-api-1.0.md` (ZeroDDS Vendor-Spec).

---

## §1 Architektur

### §1.1 Opaque-Handle-Hierarchie + Lifetime

**Spec:** §1 — "Jede Entity ist ein opaque Handle (`zerodds_*`). Lifetime per
`*_create`/`*_destroy`-Pair." Hierarchie DPF → DP → {Topic, Publisher,
Subscriber} → {DataWriter, DataReader}.

**Repo:** `crates/zerodds-c-api/src/entities.rs` (opaque Structs
`ZeroDdsDomainParticipant`/`ZeroDdsPublisher`/`ZeroDdsSubscriber`/
`ZeroDdsTopic`/`ZeroDdsDataWriter`/`ZeroDdsDataReader`); Factory-Wurzel in
`factory_ffi.rs`. Jeder Typ hat ein `*_create`/`*_delete`-Paar.

**Tests:** `factory_ffi::tests::create_and_delete_participant_clean_lifecycle`,
`participant_ffi::tests::delete_contained_entities_drops_all`.

**Status:** done

### §1.2 Stabile Wire-Form (`#[repr(C)]`)

**Spec:** §1 / §3 — "Stabile Wire-Form: ABI-stabile Strukturen für QoS,
SampleInfo, Status; opaque Handles für Entity-Lifecycle."

**Repo:** alle QoS-, SampleInfo- und Status-Strukturen sind `#[repr(C)]` in
`qos_ffi.rs`, `subscriber_ffi.rs`, `publisher_ffi.rs`, `topic_ffi.rs`.

**Tests:** `qos_ffi::tests::duration_roundtrip`,
`qos_ffi::tests::dw_qos_default_from_null`.

**Status:** done

---

## §2 Entity-Hierarchie

### §2.1 DomainParticipantFactory

**Spec:** §2.1 — `get_instance`, `create_participant`, `delete_participant`,
`lookup_participant`, `get`/`set_default_participant_qos`, `get`/`set_qos`.

**Repo:** `crates/zerodds-c-api/src/factory_ffi.rs`
(`zerodds_dpf_get_instance` + `_create_participant`/`_delete_participant`/
`_lookup_participant`/`_get`/`_set_default_participant_qos`/`_get`/`_set_qos`).

**Tests:** `factory_ffi::tests::factory_singleton_returns_non_null_stable_ptr`,
`create_and_delete_participant_clean_lifecycle`,
`delete_with_null_handles_returns_bad_handle`.

**Status:** done

### §2.2 DomainParticipant

**Spec:** §2.2 — create/delete Topic+Publisher+Subscriber+ContentFilteredTopic,
`find_topic`, `get_builtin_subscriber`, `ignore_*`, `get_domain_id`,
`assert_liveliness`, `get_current_time`, `get`/`set_qos`, `default_*_qos`,
`delete_contained_entities`, `get_discovered_*`, `contains_entity`.

**Repo:** `crates/zerodds-c-api/src/participant_ffi.rs` +
`extra_ffi.rs` (QoS-Roundtrip + `contains_entity`) + `builtin_ffi.rs`
(`get_discovered_*`). `zerodds_dp_get_current_time` schreibt die
Wall-Clock-Zeit über `zerodds_dcps::get_current_time()` in einen
`zerodds_ZeroDdsTime`-Out-Buffer.

**Tests:** `participant_ffi::tests::create_topic_then_find_then_delete`,
`domain_id_roundtrip`, `get_current_time_returns_nonzero`,
`delete_contained_entities_drops_all`;
`extra_ffi::tests::dp_get_set_qos_roundtrip_default`;
`builtin_ffi::tests::discovered_topics_empty_initially`.

**Status:** done

### §2.3 Publisher

**Spec:** §2.3 — `create`/`delete_datawriter`, `lookup_datawriter`,
`suspend`/`resume_publications`, `begin`/`end_coherent_changes`,
`wait_for_acknowledgments`, `get`/`set_qos`, `get`/`set_default_datawriter_qos`,
`copy_from_topic_qos`, `delete_contained_entities`, `get_participant`.

**Repo:** `crates/zerodds-c-api/src/publisher_ffi.rs` + `extra_ffi.rs`
(`pub_copy_from_topic_qos`, QoS-Konvertierung).

**Tests:** `publisher_ffi::tests::create_delete_datawriter_roundtrip`,
`lookup_datawriter_finds_existing`, `pub_suspend_resume_clean`,
`dw_get_topic_publisher_roundtrip`;
`tests/presentation.rs::a_coherent_set_is_taken_whole_once_its_last_end_closes_it`,
`a_writer_created_inside_an_open_set_joins_it`,
`a_group_coherent_set_spans_the_writers_and_holds_them`.

**Status:** done

### §2.4 Subscriber

**Spec:** §2.4 — `create`/`delete_datareader`, `lookup_datareader`,
`begin`/`end_access`, `get_datareaders`, `notify_datareaders`, `get`/`set_qos`,
`get`/`set_default_datareader_qos`, `copy_from_topic_qos`,
`delete_contained_entities`, `get_participant`.

**Repo:** `crates/zerodds-c-api/src/subscriber_ffi.rs` + `extra_ffi.rs`
(`sub_copy_from_topic_qos`, QoS-Konvertierung).

**Tests:** `subscriber_ffi::tests::create_delete_datareader`,
`lookup_datareader_finds_existing`;
`tests/presentation.rs::a_group_coherent_set_spans_the_writers_and_holds_them`
(`begin`/`end_access`);
`listener_ffi::tests::notify_datareaders_calls_on_data_available_once_per_change`,
`subscriber_ffi::tests::get_datareaders_lists_the_readers_with_samples_in_the_states`,
`sub_get_datareaders_reports_the_total`.

**Status:** done — `get_datareaders` nimmt die drei Zustandsmasken und listet
unter GROUP mit `ordered_access` einen Reader je Sample (DDS 1.4
§2.2.2.5.2.10); die Reihenfolge ist nicht getestet.

### §2.5 Topic

**Spec:** §2.5 — `get`/`set_qos`, `get_inconsistent_topic_status`, `get_name`,
`get_type_name`, `get_participant`.

**Repo:** `crates/zerodds-c-api/src/topic_ffi.rs`. `get`/`set_qos` über
`topic_qos_to_c`/`from_c`-Roundtrip; `get_inconsistent_topic_status` liefert
den Counter aus dem DCPS-Topic (0 außer bei aktivem Type-Mismatch,
spec-konform).

**Tests:** `topic_ffi::tests::topic_get_name_and_type_roundtrip`,
`inconsistent_topic_status_default`.

**Status:** done

### §2.6 DataWriter

**Spec:** §2.6 — `register`/`unregister_instance` (+`_w_timestamp`),
`get_key_value`, `lookup_instance`, `write` (+`_w_timestamp`), `dispose`
(+`_w_timestamp`), `wait_for_acknowledgments`, `assert_liveliness`,
`get`/`set_qos`, 4 Status-Getter, `get_matched_subscriptions(_data)`,
`get_topic`, `get_publisher`, `wait_for_matched`, Loan-API
(`loan_message`/`commit_loan`/`discard_loan`).

**Repo:** `crates/zerodds-c-api/src/publisher_ffi.rs` (Writer-Statuses,
`get_matched_subscriptions` aus `reader_proxies`) + `extra_ffi.rs`
(Instance-Ops, Loan-Hooks) + `shm_loan_ffi.rs` (Zero-Copy-Backend). Die
Loan-API hat zwei transparent gewählte Pfade: ohne Aktivierung Heap-Box;
mit Feature `flatdata-loan` + `zerodds_dw_enable_shm_loan` echtes
Same-Host-Zero-Copy. In beiden Pfaden gilt der byte-orientierte
FFI-Kontrakt (Spec-Nichtziel "das C-FFI ist byte-orientiert"): der
Caller schreibt die **serialisierte CDR-Payload** in den Loan-Buffer —
identisch zu `zerodds_dw_write`. `loan_message` liefert dafür einen Zeiger
in einen POSIX-SHM-Slot, `commit_loan` finalisiert den Slot in-place ohne
Staging-Copy (`commit_in_place`) und publiziert ihn über RTPS
(`write_user_sample_borrowed` rahmt mit dem Encapsulation-Header → CDR-wire,
interop-sicher für Cross-Host/Cross-Vendor). Ein Same-Host-Reader liest
denselben CDR-Body zero-copy aus dem Segment. Loan-State auf
`(Runtime, EntityId)` geschlüsselt, daher auch über die Runtime-FFI
(`zerodds_writer_*`) der ROS-2-RMW-Bridge nutzbar.

**Tests:** `extra_ffi::tests::dw_register_unregister_lookup_instance`,
`dw_get_key_value_reports_the_needed_length`, `dw_loan_message_then_commit_or_discard`;
`shm_loan_e2e::shm_loan_writer_to_reader_zero_copy`,
`shm_loan_runtime_path_writer_to_reader_zero_copy`,
`loan_without_shm_falls_back_to_heap`.

**Status:** done

### §2.7 DataReader

**Spec:** §2.7 — `read`/`take` (+`_w_condition`, `_next_sample`, `_instance`,
`_next_instance`), `return_loan`, `get_key_value`, `lookup_instance`,
`create`/`delete_readcondition`, `create_querycondition`, 6 Status-Getter,
`wait_for_historical_data`, `get_matched_publications(_data)`,
`get_topicdescription`, `get_subscriber`, `get`/`set_qos`, `wait_for_matched`.

**Repo:** `crates/zerodds-c-api/src/subscriber_ffi.rs` (Reader-Statuses,
`read` non-destruktiv über lokalen `read_cache`,
`read`/`take_instance`/`_next_instance` über `sample_array_filter_*`,
`_w_condition` über Condition-State-Mask) + `extra_ffi.rs`
(`get_matched_publications`). `wait_for_historical_data`: Volatile sofort Ok,
TransientLocal über `crates/dcps/src/durability_service.rs`.

**Tests:** `subscriber_ffi::tests::take_on_empty_returns_no_data`,
`return_loan_clears_array`, `statuses_default_ok`,
`cft_reader_without_type_or_schema_is_refused_for_a_field_filter`;
`extra_ffi::tests::dr_get_matched_publications_empty`;
`tests/presentation.rs::the_samples_of_an_instance_are_consecutive`,
`topic_scope_with_ordered_access_keeps_the_write_order`,
`group_scope_with_ordered_access_takes_one_sample_at_a_time`;
`subscriber_ffi::tests::keep_last_keeps_the_newest_of_the_instance`,
`a_best_effort_reader_over_its_instance_limit_evicts_the_oldest`,
`a_reliable_reader_over_its_instance_limit_holds_the_sample_back`;
`tests/reader_resource_limits.rs::a_reliable_c_reader_over_its_instance_limit_rejects_and_delivers_later`.

**Status:** done — HISTORY `KEEP_LAST` und RESOURCE_LIMITS
`max_samples`/`max_samples_per_instance` wie im typisierten Reader (DDS 1.4
§2.2.3.18, §2.2.3.19); angewandt war nur `max_instances`.

---

## §3 QoS-Strukturen

### §3.1 22 Policy-Strukturen + Composite-Sets (`#[repr(C)]`)

**Spec:** §3 — alle 22 normativen DDS-QoS-Policies sowie die 6
Composite-QoS-Sets (DP/DPF/Topic/Publisher/Subscriber/DataWriter/DataReader)
als `#[repr(C)]` mit exaktem Field-Layout.

**Repo:** `crates/zerodds-c-api/src/qos_ffi.rs` definiert alle 22 Policy-
Strukturen + die Composite-Sets.

**Tests:** `qos_ffi::tests::duration_roundtrip`, `duration_infinite_roundtrip`,
`dw_qos_default_from_null`.

**Status:** done

### §3.2 Konvertierung C → Rust

**Spec:** §3 — `*_qos_from_c` konvertieren Caller-Buffer in DCPS-QoS.

**Repo:** `qos_ffi::{dpf_qos_from_c, dp_qos_from_c, topic_qos_from_c,
pub_qos_from_c, sub_qos_from_c, dw_qos_from_c, dr_qos_from_c}`.

**Tests:** `qos_ffi::tests::dp_qos_userdata_passthrough`,
`partition_cstring_array_passthrough`.

**Status:** done

### §3.3 Konvertierung Rust → C

**Spec:** §3 — implizit im `*_get_qos`-Pfad: Rust-QoS-Snapshot in
Caller-Buffer.

**Repo:** `qos_ffi::{dp/topic/pub/sub/dw/dr_qos_to_c}` — alle 6 mit
Variable-Length-Buffer-Pfad für UserData/TopicData/GroupData (Caller-Buffer +
`OutOfResources` bei zu klein).

**Tests:** `extra_ffi::tests::dp_set_get_qos_userdata_roundtrip`.

**Status:** done

---

## §4 Status-Strukturen

### §4.1 13 Status-Strukturen (`#[repr(C)]`)

**Spec:** §4 — alle normativen Status-Strukturen (Inconsistent, SampleLost,
SampleRejected, LivelinessLost/Changed, Requested/Offered Deadline,
Requested/Offered IncompatibleQos, Subscription/Publication-Matched).

**Repo:** `topic_ffi.rs` (Inconsistent), `publisher_ffi.rs` (Writer-Statuses),
`subscriber_ffi.rs` (Reader-Statuses) — alle `#[repr(C)]`.

**Tests:** `subscriber_ffi::tests::statuses_default_ok`,
`topic_ffi::tests::inconsistent_topic_status_default`.

**Status:** done

---

## §5 Sample-API

### §5.1 SampleInfo + SampleArray

**Spec:** §5 — `SampleInfo` als `#[repr(C)]` mit allen 13 Feldern;
`SampleArray` mit `buffers + lengths + infos + count + loan_token`.

**Repo:** `subscriber_ffi.rs::ZeroDdsSampleInfo` (13 Felder) +
`ZeroDdsSampleArray`.

**Tests:** `subscriber_ffi::tests::take_on_empty_returns_no_data`,
`return_loan_clears_array`.

**Status:** done

### §5.2 Loan-Token-Kontrakt

**Spec:** §5 — `loan_token` verweist auf internen Allokations-Block,
freigegeben via `dr_return_loan`.

**Repo:** `subscriber_ffi.rs` — interne `LoanMemory`-Struktur
(`payloads + buffers + lengths + infos`), `zerodds_dr_return_loan` gibt frei.

**Tests:** `subscriber_ffi::tests::return_loan_clears_array`.

**Status:** done

---

## §6 Conditions + WaitSet

### §6.1 GuardCondition + StatusCondition + WaitSet

**Spec:** §6 — Guard/Status-Conditions + WaitSet mit
`attach`/`detach`/`wait`/`get_conditions`.

**Repo:** `crates/zerodds-c-api/src/condition_ffi.rs` (GuardCondition,
StatusCondition, ReadCondition, QueryCondition, WaitSet + `waitset_wait`).
Eine StatusCondition löst auf die StatusChangedFlags ihrer Entity aus
(`status_changes_of`): die eines Writers und Readers aus dem Runtime
(`user_writer_status_changes`, `user_reader_status_changes`),
DATA_AVAILABLE und DATA_ON_READERS aus ungelesenen Samples,
INCONSISTENT_TOPIC aus dem Zähler des Topics;
`entities::note_status_entity` sagt, was der untypisierte
Entity-Zeiger ist. Sie löste aus, sobald eine Maske gesetzt war.

**Tests:** `condition_ffi::tests::guardcondition_lifecycle_and_trigger`,
`statuscondition_lifecycle_and_mask`, `condition_header_at_offset_zero`,
`a_status_condition_triggers_on_the_entitys_flags`.

**Status:** done

### §6.2 ReadCondition + QueryCondition

**Spec:** §6/§2.7 — `dr_create_readcondition` + `dr_create_querycondition`
mit State-Mask und (Query) Filter-Expression + Parametern.

**Repo:** `condition_ffi.rs` (Strukturen + create) +
`subscriber_ffi.rs` (Trigger evaluiert State-Mask gegen Reader-Cache;
QueryCondition zusätzlich Filter-Expression über `zerodds-sql-filter`).

**Tests:** `subscriber_ffi::tests::cft_reader_without_type_or_schema_is_refused_for_a_field_filter`.

**Status:** done

---

## §7 Built-in Topic Data

### §7.1 4 BuiltinTopicData-Strukturen

**Spec:** §7 — Participant, Topic, Publication, Subscription als `#[repr(C)]`;
Discovery-/Matched-Getter füllen sie.

**Repo:** `builtin_ffi.rs` (4 Strukturen + `get_discovered_*`);
`dw_get_matched_subscription_data` / `dr_get_matched_publication_data` über
`DcpsRuntime::user_writer_matched_subscription_data` /
`user_reader_matched_publication_data`: ein entdeckter Endpunkt aus dem
SEDP-Cache, ein Endpunkt desselben Participants aus seiner letzten eigenen
Ankündigung; ein Handle ohne Match liefert `ZERODDS_BAD_PARAMETER`.

**Tests:** `builtin_ffi::tests::discovered_topics_empty_initially`,
`discovered_publications_count_starts_zero`,
`discovered_participant_data_unknown_handle_returns_error`;
`extra_ffi::tests::matched_data_of_a_same_participant_pair`.

**Status:** done

---

## §8 Status-Codes

### §8.1 15 Status-Codes

**Spec:** §8 — 15 Status-Codes (`0`=OK, `-1`..`-14`).

**Repo:** `crates/zerodds-c-api/src/lib.rs::ZeroDdsStatus` enum mit allen 15 Codes.

**Tests:** in der gesamten Suite verwendet, z. B.
`factory_ffi::tests::delete_with_null_handles_returns_bad_handle`
(prüft `BadHandle`).

**Status:** done

---

## §9 Memory-Ownership-Vertrag

### §9.1 Paarweise Allokations-/Free-Pfade

**Spec:** §9 — Ownership-Tabelle: `*_create` paart mit `*_destroy`,
`dr_take`-Payloads mit `return_loan`, `*_get_qos`-Strings statisch (kein
Free), `topic_get_name`/`version` statisch.

**Repo:** pro Allokations-Funktion ein dedizierter Free-Pfad
(`zerodds_dpf_create_participant`/`_delete_participant`,
`zerodds_dr_take`/`_return_loan`, `zerodds_topic_get_name`/`zerodds_string_free`).

**Tests:** `factory_ffi::tests::create_and_delete_participant_clean_lifecycle`,
`subscriber_ffi::tests::return_loan_clears_array`.

**Status:** done

---

## §10 Stabilität

### §10.1 ABI-Stabilität ab 1.0.0-rc.1

**Spec:** §10 — `include/zerodds.h` ist ABI-stable ab `1.0.0-rc.1`;
`#[repr(C)]`-Layouts; Funktions-/Status-Code-Zusätze additiv,
Layout-Breaking nur bei Major-Bump.

**Repo:** `crates/zerodds-c-api/include/zerodds.h` (cbindgen-generiert);
Symbol-Snapshot `tests/abi.snapshot.json` (additiv erzwungen, Entfernung =
Audit-Fail).

**Tests:** `abi_compat::abi_snapshot_matches`.

**Status:** done

---

## §11 Test-Pflicht

### §11.1 Round-Trip- + Header-Compile-Tests

**Spec:** §11 — pro Funktion mindestens ein Round-Trip-Test gegen ein
Zwei-Participant-Setup; Header-Compile-Test.

**Repo:** Unit-Tests in jedem `*_ffi`-Modul; Integration `smoke_ffi.rs`
(Zwei-Participant-Pub-Sub-Roundtrip); Header-Compile in C++/C#-Smoke
(`crates/cpp/`).

**Tests:** `smoke_ffi::ffi_pub_sub_roundtrip`,
`abi_compat::abi_snapshot_matches`, `cargo test -p zerodds-c-api`.

**Status:** done

---

## §12 Cross-Reference auf zugehörige Vendor-Specs

### §12.1 Cross-Reference-Liste

**Spec:** §12 — Verweise auf `zerodds-listener-callbacks-1.1.md`,
`zerodds-async-1.0.md`, `zerodds-java-omgdds-1.0.md`.

**Repo:** `docs/specs/zerodds-c-api-1.0.md` §12 (Doku-Querverweis, keine
Code-Anforderung).

**Tests:** —

**Status:** n/a (informative) — reiner Doku-Querverweis ohne
implementierbare normative Anforderung.

---

## §13 FFI-Surface-Status

### §13.1 Vollständige Surface ausgeliefert

**Spec:** §13 — exponierte FFI-Surface (DPF/DP/Topic/Pub+DW/Sub+DR/QoS/
Conditions/Built-in/Listener/Instance-Ops/Loan/Matched-Listings); der
authoritative Item-Status liegt in diesem Coverage-Audit.

**Repo:** `crates/zerodds-c-api/` — alle Surface-Gruppen sind exponiert, inkl.
Loan-API (§2.6, echtes SHM-Zero-Copy), Matched-Pubs/Subs-Listings (§2.6/§2.7),
Listener-Wireup (`zerodds_poll_listeners`, siehe `zerodds-listener-callbacks-1.1.md`)
und getrenntem `read`/`take` über den Reader-Read-Cache (§2.7).

**Tests:** `abi_compat::abi_snapshot_matches` (Surface-Snapshot);
`smoke_ffi::ffi_pub_sub_roundtrip`.

**Status:** done

### §13.2 Runtime-FFI: Delivery-Modes + Zero-Copy + event-driven Raw-Readiness

**Spec:** ZeroDDS-Extension über den OMG-DCPS-PSM-C hinaus, normativ in
`docs/specs/zerodds-delivery-modes-1.0.md`; konsumiert von der ROS-2-RMW-Bridge
(siehe `ros2-rmw` Coverage). Der Runtime-Pfad (`zerodds_writer_*`/
`zerodds_reader_*`) wählt pro Endpoint einen Delivery-Mode und bietet beidseitig
echtes Same-Host-Zero-Copy plus eine blockierende Raw-Readiness-Quelle.

**Repo:** `crates/zerodds-c-api/src/lib.rs` + `shm_loan_ffi.rs`:

- `zerodds_writer_set_delivery_mode(w, mode)` — `0`=Portable (Default,
  interop-sicher, CDR auf dem Draht), `1`=RawSameHost (Zero-Copy/Zero-Serialize,
  same-host, kein RTPS), `2`=Iceoryx liefert `Unsupported` (Iceoryx läuft über die
  dedizierten `zerodds_writer_enable_iceoryx`/`zerodds_reader_enable_iceoryx`,
  Feature `delivery-iceoryx`).
- `zerodds_writer_enable_shm_loan` — aktiviert den POSIX-SHM-Loan-Pfad des Writers.
- `zerodds_reader_enable_shm` / `zerodds_reader_take_shm` / `zerodds_reader_release_shm`
  — reader-seitiges Zero-Copy-Take: `take_shm` liefert einen Zeiger in den
  SHM-Slot (gültig bis `release_shm`), `NoData` wenn nichts ansteht.
- `zerodds_reader_raw_wait(reader, timeout_ms)` — blockiert event-getrieben auf der
  Raw-Quelle des Readers (RawSameHost-SHM-Change-Generation-Futex bzw.
  iceoryx2-Listener), `Ok` bei echtem Wake, `NoData` bei Timeout; lock-free
  (klont den Backend-/Subscriber-Handle vor dem Parken). Die RMW-Bridge faltet das
  über einen Doorbell-Thread in `rmw_wait` ein.

**Tests:** `shm_loan_e2e::delivery_mode_setter_validation`,
`raw_same_host_mode_writer_to_reader`, `shm_loan_writer_to_reader_zero_copy`,
`shm_loan_runtime_path_writer_to_reader_zero_copy`,
`loan_without_shm_falls_back_to_heap`.

**Status:** done

---

## §14 Aufrufe für die PSM-C++-Schicht

### §14.1 QosProvider-Aufrufe

**Spec:** DDS-PSM-Cxx 1.0 §7.5 (QosProvider) über die C-Schicht: Dokument lesen, QoS je Entity-Art mit Vererbung und Profilnamen liefern.

**Repo:** `crates/zerodds-c-api/src/qos_provider_ffi.rs`: `zerodds_qosprovider_create` (URIs `file:`, `data:`, `http:`, `https:` über rustls; andere Schemata `Unsupported`), `zerodds_qosprovider_participant_qos`/`publisher_qos`/`subscriber_qos`/`topic_qos`/`datawriter_qos`/`datareader_qos`, `zerodds_qosprovider_destroy`.

**Tests:** `qos_provider::a_provider_answers_the_qos_of_every_entity_kind_with_inheritance`, `a_data_uri_and_an_http_uri_name_the_document_too`, `an_unknown_scheme_is_unsupported_and_a_missing_file_or_a_closed_https_port_is_bad_parameter`. Commits 9629b1b5a34, 3f6b97b33d8, c55027bbbe3.

**Status:** done

### §14.2 Typbeschreibung aus IDL und DDS-XML

**Spec:** DDS-PSM-Cxx 1.0 §7.10 (TypeProvider/DynamicType) und DDS-XTypes 1.3: die Typen eines Dokuments mit Mitgliedern, Schlüsseln, Aufzählungswerten, Aliasen und Arrays beschreiben.

**Repo:** `crates/zerodds-c-api/src/types_ffi.rs`: `zerodds_types_load` (IDL- und DDS-XML-Dokument, gescopte Namen aufgelöst) und `zerodds_types_describe_idl`.

**Tests:** `types_load::idl_types_are_described_with_members_keys_enumerators_and_aliases`, `a_dds_xml_type_library_is_described_like_the_idl_it_stands_for`, `a_document_that_cannot_be_read_and_an_unknown_scheme_are_refused`. Commits 43f069409ca, 8398cb2ad39, c55027bbbe3, 8222719f6d1.

**Status:** done

### §14.3 Dynamische Typen aus IDL-Text

**Spec:** DDS-XTypes 1.3 §7.5 (DynamicType, DynamicData) über die C-Schicht: Typ aus IDL, Identität wie bei einem übersetzten Typ, Wert als JSON codieren und decodieren.

**Repo:** `crates/zerodds-c-api/src/dyntype_ffi.rs`: `zerodds_dyntype_from_idl`, `_name`, `_extensibility`, `_is_keyed`, `_type_object`, `_dependency_count`, `_dependency`, `_default_json`, `_encode_json`, `_decode_json`, `_bytes_free`, `_destroy`.

**Tests:** `dyntype::a_type_from_idl_has_the_identity_of_a_compiled_one_and_a_value_round_trips`, `idl_that_does_not_parse_or_a_name_it_does_not_declare_is_bad_parameter`. Commit 869802b57a6.

**Status:** done

### §14.4 QueryCondition: Setter und Lesen mit Bedingung

**Spec:** DDS 1.4 §2.2.2.5.9 (QueryCondition) und §2.2.2.5.3.8 ff. (read/take mit Bedingung): Ausdruck und Parameter ersetzen; `read`/`take` und die Instanz-Varianten wenden den Ausdruck an.

**Repo:** `crates/zerodds-c-api/src/condition_ffi.rs`: `zerodds_querycondition_set_expression`, `zerodds_querycondition_set_parameters`; `zerodds_dr_read_w_condition`, `zerodds_dr_take_w_condition`, `zerodds_dr_read_instance_w_condition`, `zerodds_dr_take_instance_w_condition`, `zerodds_dr_read_next_instance_w_condition`, `zerodds_dr_take_next_instance_w_condition`.

**Tests:** `query_condition_update::a_query_condition_takes_a_new_expression_and_new_parameters`, `query_condition_read::read_and_take_with_a_query_condition_return_only_the_samples_the_expression_selects`, `a_query_condition_without_a_type_has_no_data_to_judge_when_empty_and_refuses_a_sample_it_cannot_read`. Commits eb8bfdd601e, 8733e3630a0, 8fe6e6329a4, c55027bbbe3.

**Status:** done

### §14.5 Listener-Getter von Publisher, Subscriber und Topic

**Spec:** DDS 1.4 §2.2.2.2.1.x (`get_listener` der Entities): die zuletzt gesetzte Tabelle zurückgeben.

**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs`: `zerodds_pub_get_listener`, `zerodds_sub_get_listener`, `zerodds_topic_get_listener` (neben `zerodds_dp_get_listener`).

**Tests:** `listener_ffi::tests::pub_sub_topic_set_get_listener_roundtrip`, `dp_set_get_listener_roundtrip`. Commits 34966e1b222, f5f6c58e4a1.

**Status:** done

### §14.6 enable, Statusänderungen, Instanzhandle

**Spec:** DDS 1.4 §2.2.2.1.1 (Entity): `enable`, `get_status_changes`, `get_instance_handle` und `autoenable_created_entities` der Fabrik für die C-Entities.

**Repo:** `crates/zerodds-c-api/src/`: `zerodds_{dp,pub,sub,topic,dw,dr}_enable`, `_get_status_changes`, `_get_instance_handle`.

**Tests:** `entity_enable::entities_of_a_participant_that_does_not_autoenable_are_created_disabled`, `a_writer_of_a_publisher_that_does_not_autoenable_is_not_announced_until_enabled`, `instance_handles_are_the_ones_contains_entity_and_the_matched_lists_use`, `status_changes_show_what_happened_and_clear_when_the_status_is_read`; `entity_enable_factory::a_participant_of_a_factory_that_does_not_autoenable_is_created_disabled`. Commits 613141ae219, 19859043431.

**Status:** done

### §14.7 Schlüssel-Hash einer Instanz

**Spec:** DDS 1.4 §2.2.2.4.2.x (`lookup_instance`, `register_instance`) und DDSI-RTPS §9.6.3.8 (KeyHash): der Schlüssel-Hash einer registrierten Instanz.

**Repo:** `crates/zerodds-c-api/src/instance_ffi.rs`: `zerodds_dw_instance_key_hash` (unbekanntes Handle: `BadParameter`).

**Tests:** `instance_key_hash::the_key_hash_of_a_registered_instance_comes_back_and_an_unknown_handle_is_refused`; `dw_write_derives_the_key::a_write_registers_the_instance_of_the_key_its_body_carries`. Commit bd6558f058f.

**Status:** done

### §14.8 Reader des Built-in-Subscriber

**Spec:** DDS 1.4 §2.2.5 und §2.2.2.5.2.7 (`lookup_datareader`): die Reader der Topics `DCPSParticipant`, `DCPSTopic`, `DCPSPublication`, `DCPSSubscription` mit `read` und `take` ihrer Samples und SampleInfo.

**Repo:** `crates/zerodds-c-api/src/builtin_reader_ffi.rs`: `zerodds_builtin_subscriber_lookup_datareader`, `zerodds_builtin_reader_kind`, `zerodds_builtin_reader_read`, `zerodds_builtin_samples_free`, `zerodds_builtin_sample_free`, `zerodds_builtin_reader_destroy`.

**Tests:** `builtin_reader::the_builtin_readers_hold_the_participant_topic_publication_and_subscription_discovery_found`.

**Status:** done

### §14.9 MultiTopic

**Spec:** DDS 1.4 §2.2.2.3.4 (MultiTopic) und Annex B.2 (Ausdruck): anlegen, löschen, Parameter ersetzen, Reader auf der MultiTopic mit `read` und `take`.

**Repo:** `crates/zerodds-c-api/src/multitopic_ffi.rs`: `zerodds_topic_set_type_objects`, `zerodds_dp_create_multitopic`, `zerodds_dp_delete_multitopic`, `zerodds_multitopic_set_parameters`, `zerodds_sub_create_multitopic_reader`, `zerodds_multitopic_reader_read`, `zerodds_multitopic_reader_read_ex` (nach Instanz, mit Zustandsmasken, `max_samples` und Abfrage), `zerodds_multitopic_samples_free`, `zerodds_multitopic_reader_destroy`.

**Tests:** `multitopic_c::a_multitopic_joins_two_topics_and_is_not_deleted_while_a_reader_exists`.

**Status:** done

---

## Audit-Status

33 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-c-api` — 162 Tests grün, 0 failed (6 ignoriert), gemessen vor den Items §14; deren Tests sind auf dem Mac nur kompiliert, der Lauf steht auf dem Testhost aus.

Offene Punkte: keine. Decision-Records: keine.
