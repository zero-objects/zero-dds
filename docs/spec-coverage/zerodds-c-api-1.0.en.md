# `zerodds-c-api` v1.0 — Spec Coverage

Audit of the vendor spec `docs/specs/zerodds-c-api-1.0.md` against the code in
`crates/zerodds-c-api/`. The generated C header `include/zerodds.h` is the
normative contract document; every requirement is checked item by item with
spec reference, repo path, test path and status.

**Source:** `docs/specs/zerodds-c-api-1.0.md` (ZeroDDS vendor spec).

---

## §1 Architecture

### §1.1 Opaque handle hierarchy + lifetime

**Spec:** §1 — "Every entity is an opaque handle (`zerodds_*`). Lifetime via a
`*_create`/`*_destroy` pair." Hierarchy DPF → DP → {Topic, Publisher,
Subscriber} → {DataWriter, DataReader}.

**Repo:** `crates/zerodds-c-api/src/entities.rs` (opaque structs
`ZeroDdsDomainParticipant`/`ZeroDdsPublisher`/`ZeroDdsSubscriber`/
`ZeroDdsTopic`/`ZeroDdsDataWriter`/`ZeroDdsDataReader`); factory root in
`factory_ffi.rs`. Each type has a `*_create`/`*_delete` pair.

**Tests:** `factory_ffi::tests::create_and_delete_participant_clean_lifecycle`,
`participant_ffi::tests::delete_contained_entities_drops_all`.

**Status:** done

### §1.2 Stable wire form (`#[repr(C)]`)

**Spec:** §1 / §3 — "Stable wire form: ABI-stable structs for QoS, SampleInfo,
Status; opaque handles for entity lifecycle."

**Repo:** all QoS, SampleInfo and status structs are `#[repr(C)]` in
`qos_ffi.rs`, `subscriber_ffi.rs`, `publisher_ffi.rs`, `topic_ffi.rs`.

**Tests:** `qos_ffi::tests::duration_roundtrip`,
`qos_ffi::tests::dw_qos_default_from_null`.

**Status:** done

---

## §2 Entity hierarchy

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
`extra_ffi.rs` (QoS round-trip + `contains_entity`) + `builtin_ffi.rs`
(`get_discovered_*`). `zerodds_dp_get_current_time` writes the wall-clock time
via `zerodds_dcps::get_current_time()` into a `zerodds_ZeroDdsTime` out buffer.

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
(`pub_copy_from_topic_qos`, QoS conversion).

**Tests:** `publisher_ffi::tests::create_delete_datawriter_roundtrip`,
`lookup_datawriter_finds_existing`, `pub_suspend_resume_clean`,
`dw_get_topic_publisher_roundtrip`.

**Status:** done

### §2.4 Subscriber

**Spec:** §2.4 — `create`/`delete_datareader`, `lookup_datareader`,
`begin`/`end_access`, `get_datareaders`, `notify_datareaders`, `get`/`set_qos`,
`get`/`set_default_datareader_qos`, `copy_from_topic_qos`,
`delete_contained_entities`, `get_participant`.

**Repo:** `crates/zerodds-c-api/src/subscriber_ffi.rs` + `extra_ffi.rs`
(`sub_copy_from_topic_qos`, QoS conversion).

**Tests:** `subscriber_ffi::tests::create_delete_datareader`,
`lookup_datareader_finds_existing`.

**Status:** done

### §2.5 Topic

**Spec:** §2.5 — `get`/`set_qos`, `get_inconsistent_topic_status`, `get_name`,
`get_type_name`, `get_participant`.

**Repo:** `crates/zerodds-c-api/src/topic_ffi.rs`. `get`/`set_qos` via the
`topic_qos_to_c`/`from_c` round-trip; `get_inconsistent_topic_status` returns
the counter from the DCPS topic (0 unless a type mismatch is active,
spec-conformant).

**Tests:** `topic_ffi::tests::topic_get_name_and_type_roundtrip`,
`inconsistent_topic_status_default`.

**Status:** done

### §2.6 DataWriter

**Spec:** §2.6 — `register`/`unregister_instance` (+`_w_timestamp`),
`get_key_value`, `lookup_instance`, `write` (+`_w_timestamp`), `dispose`
(+`_w_timestamp`), `wait_for_acknowledgments`, `assert_liveliness`,
`get`/`set_qos`, 4 status getters, `get_matched_subscriptions(_data)`,
`get_topic`, `get_publisher`, `wait_for_matched`, loan API
(`loan_message`/`commit_loan`/`discard_loan`).

**Repo:** `crates/zerodds-c-api/src/publisher_ffi.rs` (writer statuses,
`get_matched_subscriptions` from `reader_proxies`) + `extra_ffi.rs`
(instance ops, loan hooks) + `shm_loan_ffi.rs` (zero-copy backend). The loan
API has two transparently selected paths: heap box without activation; real
same-host zero-copy with the `flatdata-loan` feature + `zerodds_dw_enable_shm_loan`.
Both paths follow the byte-oriented FFI contract (spec non-goal "the C FFI is
byte-oriented"): the caller writes the **serialized CDR payload** into the loan
buffer — identical to `zerodds_dw_write`. `loan_message` returns a pointer into
a POSIX SHM slot for that, `commit_loan` finalizes the slot in place with no
staging copy (`commit_in_place`) and publishes it over RTPS
(`write_user_sample_borrowed` frames it with the encapsulation header → CDR
wire, interop-safe for cross-host/cross-vendor). A same-host reader reads the
same CDR body zero-copy from the segment. The loan state is keyed by
`(runtime, EntityId)`, so it is also usable via the runtime FFI
(`zerodds_writer_*`) the ROS-2 RMW bridge uses.

**Tests:** `extra_ffi::tests::dw_register_unregister_lookup_instance`,
`dw_get_key_value_buffer_too_small`, `dw_loan_message_then_commit_or_discard`;
`shm_loan_e2e::shm_loan_writer_to_reader_zero_copy`,
`shm_loan_runtime_path_writer_to_reader_zero_copy`,
`loan_without_shm_falls_back_to_heap`.

**Status:** done

### §2.7 DataReader

**Spec:** §2.7 — `read`/`take` (+`_w_condition`, `_next_sample`, `_instance`,
`_next_instance`), `return_loan`, `get_key_value`, `lookup_instance`,
`create`/`delete_readcondition`, `create_querycondition`, 6 status getters,
`wait_for_historical_data`, `get_matched_publications(_data)`,
`get_topicdescription`, `get_subscriber`, `get`/`set_qos`, `wait_for_matched`.

**Repo:** `crates/zerodds-c-api/src/subscriber_ffi.rs` (reader statuses,
`read` non-destructive via a local `read_cache`,
`read`/`take_instance`/`_next_instance` via `sample_array_filter_*`,
`_w_condition` via condition state mask) + `extra_ffi.rs`
(`get_matched_publications`). `wait_for_historical_data`: Volatile returns Ok
immediately, TransientLocal via `crates/dcps/src/durability_service.rs`.

**Tests:** `subscriber_ffi::tests::take_on_empty_returns_no_data`,
`return_loan_clears_array`, `statuses_default_ok`,
`cft_filter_active_passes_untyped_samples`;
`extra_ffi::tests::dr_get_matched_publications_empty`.

**Status:** done

---

## §3 QoS structures

### §3.1 22 policy structs + composite sets (`#[repr(C)]`)

**Spec:** §3 — all 22 normative DDS QoS policies plus the 6 composite QoS sets
(DP/DPF/Topic/Publisher/Subscriber/DataWriter/DataReader) as `#[repr(C)]` with
exact field layout.

**Repo:** `crates/zerodds-c-api/src/qos_ffi.rs` defines all 22 policy structs +
the composite sets.

**Tests:** `qos_ffi::tests::duration_roundtrip`, `duration_infinite_roundtrip`,
`dw_qos_default_from_null`.

**Status:** done

### §3.2 Conversion C → Rust

**Spec:** §3 — `*_qos_from_c` convert the caller buffer into DCPS QoS.

**Repo:** `qos_ffi::{dpf_qos_from_c, dp_qos_from_c, topic_qos_from_c,
pub_qos_from_c, sub_qos_from_c, dw_qos_from_c, dr_qos_from_c}`.

**Tests:** `qos_ffi::tests::dp_qos_userdata_passthrough`,
`partition_cstring_array_passthrough`.

**Status:** done

### §3.3 Conversion Rust → C

**Spec:** §3 — implicit in the `*_get_qos` path: Rust QoS snapshot into the
caller buffer.

**Repo:** `qos_ffi::{dp/topic/pub/sub/dw/dr_qos_to_c}` — all 6 with a
variable-length buffer path for UserData/TopicData/GroupData (caller buffer +
`OutOfResources` if too small).

**Tests:** `extra_ffi::tests::dp_set_get_qos_userdata_roundtrip`.

**Status:** done

---

## §4 Status structures

### §4.1 13 status structures (`#[repr(C)]`)

**Spec:** §4 — all normative status structures (Inconsistent, SampleLost,
SampleRejected, LivelinessLost/Changed, Requested/Offered Deadline,
Requested/Offered IncompatibleQos, Subscription/Publication matched).

**Repo:** `topic_ffi.rs` (Inconsistent), `publisher_ffi.rs` (writer statuses),
`subscriber_ffi.rs` (reader statuses) — all `#[repr(C)]`.

**Tests:** `subscriber_ffi::tests::statuses_default_ok`,
`topic_ffi::tests::inconsistent_topic_status_default`.

**Status:** done

---

## §5 Sample API

### §5.1 SampleInfo + SampleArray

**Spec:** §5 — `SampleInfo` as `#[repr(C)]` with all 13 fields; `SampleArray`
with `buffers + lengths + infos + count + loan_token`.

**Repo:** `subscriber_ffi.rs::ZeroDdsSampleInfo` (13 fields) +
`ZeroDdsSampleArray`.

**Tests:** `subscriber_ffi::tests::take_on_empty_returns_no_data`,
`return_loan_clears_array`.

**Status:** done

### §5.2 Loan-token contract

**Spec:** §5 — `loan_token` refers to an internal allocation block, freed via
`dr_return_loan`.

**Repo:** `subscriber_ffi.rs` — internal `LoanMemory` struct
(`payloads + buffers + lengths + infos`), `zerodds_dr_return_loan` frees it.

**Tests:** `subscriber_ffi::tests::return_loan_clears_array`.

**Status:** done

---

## §6 Conditions + WaitSet

### §6.1 GuardCondition + StatusCondition + WaitSet

**Spec:** §6 — Guard/Status conditions + WaitSet with
`attach`/`detach`/`wait`/`get_conditions`.

**Repo:** `crates/zerodds-c-api/src/condition_ffi.rs` (GuardCondition,
StatusCondition, ReadCondition, QueryCondition, WaitSet + `waitset_wait`).

**Tests:** `condition_ffi::tests::guardcondition_lifecycle_and_trigger`,
`statuscondition_lifecycle_and_mask`, `condition_header_at_offset_zero`.

**Status:** done

### §6.2 ReadCondition + QueryCondition

**Spec:** §6/§2.7 — `dr_create_readcondition` + `dr_create_querycondition` with
state mask and (Query) filter expression + parameters.

**Repo:** `condition_ffi.rs` (structs + create) + `subscriber_ffi.rs` (trigger
evaluates the state mask against the reader cache; QueryCondition additionally
the filter expression via `zerodds-sql-filter`).

**Tests:** `subscriber_ffi::tests::cft_filter_active_passes_untyped_samples`.

**Status:** done

---

## §7 Built-in topic data

### §7.1 4 BuiltinTopicData structures

**Spec:** §7 — Participant, Topic, Publication, Subscription as `#[repr(C)]`;
the discovery/matched getters fill them.

**Repo:** `builtin_ffi.rs` (4 structs + `get_discovered_*`);
`dw_get_matched_subscription_data` / `dr_get_matched_publication_data` via the
BuiltinSubscriber cache (`subscription_reader().read()` /
`publication_reader().read()`).

**Tests:** `builtin_ffi::tests::discovered_topics_empty_initially`,
`discovered_publications_count_starts_zero`,
`discovered_participant_data_unknown_handle_returns_error`.

**Status:** done

---

## §8 Status codes

### §8.1 15 status codes

**Spec:** §8 — 15 status codes (`0`=OK, `-1`..`-14`).

**Repo:** `lib.rs::ZeroDdsStatus` enum with all 15 codes.

**Tests:** used across the whole suite, e.g.
`factory_ffi::tests::delete_with_null_handles_returns_bad_handle`
(checks `BadHandle`).

**Status:** done

---

## §9 Memory-ownership contract

### §9.1 Paired allocation/free paths

**Spec:** §9 — ownership table: `*_create` pairs with `*_destroy`,
`dr_take` payloads with `return_loan`, `*_get_qos` strings static (no free),
`topic_get_name`/`version` static.

**Repo:** one dedicated free path per allocation function
(`zerodds_dpf_create_participant`/`_delete_participant`,
`zerodds_dr_take`/`_return_loan`, `zerodds_topic_get_name`/`zerodds_string_free`).

**Tests:** `factory_ffi::tests::create_and_delete_participant_clean_lifecycle`,
`subscriber_ffi::tests::return_loan_clears_array`.

**Status:** done

---

## §10 Stability

### §10.1 ABI stability from 1.0.0-rc.1

**Spec:** §10 — `include/zerodds.h` is ABI-stable from `1.0.0-rc.1`;
`#[repr(C)]` layouts; function/status-code additions are additive, layout
breaks only on a major bump.

**Repo:** `crates/zerodds-c-api/include/zerodds.h` (cbindgen-generated); symbol
snapshot `tests/abi.snapshot.json` (additive enforced, removal = audit fail).

**Tests:** `abi_compat::abi_snapshot_matches`.

**Status:** done

---

## §11 Test obligation

### §11.1 Round-trip + header-compile tests

**Spec:** §11 — at least one round-trip test per function against a
two-participant setup; header-compile test.

**Repo:** unit tests in every `*_ffi` module; integration `smoke_ffi.rs`
(two-participant pub-sub round-trip); header compile in the C++/C# smoke
(`crates/cpp/`).

**Tests:** `smoke_ffi::ffi_pub_sub_roundtrip`,
`abi_compat::abi_snapshot_matches`, `cargo test -p zerodds-c-api`.

**Status:** done

---

## §12 Cross-reference to related vendor specs

### §12.1 Cross-reference list

**Spec:** §12 — references to `zerodds-listener-callbacks-1.1.md`,
`zerodds-async-1.0.md`, `zerodds-java-omgdds-1.0.md`.

**Repo:** `docs/specs/zerodds-c-api-1.0.md` §12 (documentation cross-reference,
no code requirement).

**Tests:** —

**Status:** n/a (informative) — a pure documentation cross-reference with no
implementable normative requirement.

---

## §13 FFI surface status

### §13.1 Full surface delivered

**Spec:** §13 — the exposed FFI surface (DPF/DP/Topic/Pub+DW/Sub+DR/QoS/
Conditions/Built-in/Listener/instance ops/loan/matched listings); the
authoritative item status lives in this coverage audit.

**Repo:** `crates/zerodds-c-api/` — all surface groups are exposed, incl. the
loan API (§2.6, real SHM zero-copy), matched pubs/subs listings (§2.6/§2.7),
listener wire-up (`zerodds_poll_listeners`, see `zerodds-listener-callbacks-1.1.md`)
and split `read`/`take` via the reader read cache (§2.7).

**Tests:** `abi_compat::abi_snapshot_matches` (surface snapshot);
`smoke_ffi::ffi_pub_sub_roundtrip`.

**Status:** done

### §13.2 Runtime FFI: delivery modes + zero-copy + event-driven raw readiness

**Spec:** a ZeroDDS extension beyond the OMG DCPS-PSM-C, normative in
`docs/specs/zerodds-delivery-modes-1.0.md`; consumed by the ROS-2 RMW bridge
(see the `ros2-rmw` coverage). The runtime path (`zerodds_writer_*`/
`zerodds_reader_*`) picks a per-endpoint delivery mode and offers real same-host
zero-copy on both sides plus a blocking raw-readiness source.

**Repo:** `crates/zerodds-c-api/src/lib.rs` + `shm_loan_ffi.rs`:

- `zerodds_writer_set_delivery_mode(w, mode)` — `0`=Portable (default,
  interop-safe, CDR on the wire), `1`=RawSameHost (zero-copy/zero-serialize,
  same-host, no RTPS), `2`=Iceoryx returns `Unsupported` (Iceoryx runs through the
  dedicated `zerodds_writer_enable_iceoryx`/`zerodds_reader_enable_iceoryx`,
  feature `delivery-iceoryx`).
- `zerodds_writer_enable_shm_loan` — enables the writer's POSIX-SHM loan path.
- `zerodds_reader_enable_shm` / `zerodds_reader_take_shm` / `zerodds_reader_release_shm`
  — reader-side zero-copy take: `take_shm` returns a pointer into the SHM slot
  (valid until `release_shm`), `NoData` when nothing is pending.
- `zerodds_reader_raw_wait(reader, timeout_ms)` — blocks event-driven on the
  reader's raw source (RawSameHost SHM change-generation futex or iceoryx2
  listener), `Ok` on a real wake, `NoData` on timeout; lock-free (clones the
  backend/subscriber handle before parking). The RMW bridge folds this into
  `rmw_wait` via a doorbell thread.

**Tests:** `shm_loan_e2e::delivery_mode_setter_validation`,
`raw_same_host_mode_writer_to_reader`, `shm_loan_writer_to_reader_zero_copy`,
`shm_loan_runtime_path_writer_to_reader_zero_copy`,
`loan_without_shm_falls_back_to_heap`.

**Status:** done

---

## Audit status

24 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-c-api` — 136 tests green, 0 failed.

Open items: none. Decision records: none.
