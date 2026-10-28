# `zerodds-listener-callbacks` v1.1 — Spec Coverage

Audit of the vendor spec `docs/specs/zerodds-listener-callbacks-1.1.md`
against the code in `crates/zerodds-c-api/src/listener_ffi.rs` (with the
status counters and inconsistent-topic detection in `crates/dcps/` and
`crates/discovery/`).

**Source:** `docs/specs/zerodds-listener-callbacks-1.1.md` (ZeroDDS vendor
spec).

Implementation:

- `crates/zerodds-c-api/src/listener_ffi.rs` — listener vtables, set/get
  API, `zerodds_poll_listeners()` active firing + aggregators.
- `crates/dcps/src/runtime.rs` — status counters, delivered-sample
  counter, inconsistent-topic counter.
- `crates/discovery/src/sedp/cache.rs` — SEDP type-mismatch detection.

---

## §1 Architecture

### §1.1 Function-pointer table

**Spec:** §1.1 — listeners as `#[repr(C)]` structs of function pointers,
all optional (NULL = ignored).

**Repo:** `listener_ffi.rs` — 6 structs:
`ZeroDdsDomainParticipantListener`, `ZeroDdsPublisherListener`,
`ZeroDdsSubscriberListener`, `ZeroDdsTopicListener`,
`ZeroDdsDataWriterListener`, `ZeroDdsDataReaderListener`.

**Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`,
`dw_set_listener_clear_via_null`.

**Status:** done

### §1.2 `user_data` slot

**Spec:** §1.2 — one `void* user_data` per struct, passed unchanged to
every callback.

**Repo:** all 6 structs carry `user_data: *mut c_void` as the first
field; `fire_writer_vtable` / `fire_reader_vtable` pass it to each
callback.

**Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`.

**Status:** done

### §1.3 Set/Get API

**Spec:** §1.3 — one `*_set_listener` / `*_get_listener` pair per entity
type; NULL clears.

**Repo:** `zerodds_{dp,pub,sub,topic,dw,dr}_set_listener` +
`zerodds_{dp,dw,dr}_get_listener`. `set_listener(NULL)` removes the
registry entry and the cached counters.

**Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`,
`dw_set_listener_clear_via_null`.

**Status:** done

---

## §2 Listener inventory

### §2.1 DomainParticipantListener

**Spec:** §2.1 — `on_inconsistent_topic`, `on_data_on_readers`.

**Repo:** `ZeroDdsDomainParticipantListener`; both callbacks fired by the
DP aggregator in `zerodds_poll_listeners` (§6.3, §6.4).

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`,
`poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

### §2.2 PublisherListener

**Spec:** §2.2 — 4 writer-status callbacks, aggregating contained
DataWriters.

**Repo:** `ZeroDdsPublisherListener`; the Publisher aggregator walks
`publisher.datawriters` and fires via `fire_writer_vtable`.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`
(poll path), `dp_set_get_listener_roundtrip` (registry).

**Status:** done

### §2.3 SubscriberListener

**Spec:** §2.3 — 7 reader-status callbacks + `on_data_on_readers`.

**Repo:** `ZeroDdsSubscriberListener`; the Subscriber aggregator walks
`subscriber.datareaders`, fires the 7 reader callbacks via
`fire_reader_vtable` and `on_data_on_readers` with set semantics.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

### §2.4 TopicListener

**Spec:** §2.4 — `on_inconsistent_topic`.

**Repo:** `ZeroDdsTopicListener`; fired by the Topic-level pass on an
inconsistent-topic counter delta.

**Tests:** `listener_ffi::tests::poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

### §2.5 DataWriterListener

**Spec:** §2.5 — `on_liveliness_lost`, `on_offered_deadline_missed`,
`on_offered_incompatible_qos`, `on_publication_matched`.

**Repo:** `ZeroDdsDataWriterListener`; fired by the DataWriter level via
`fire_writer_vtable`.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.

**Status:** done

### §2.6 DataReaderListener

**Spec:** §2.6 — all 7 reader-status callbacks.

**Repo:** `ZeroDdsDataReaderListener`; fired by the DataReader level via
`fire_reader_vtable`.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

---

## §3 Status-mask semantics

**Spec:** §3 — `status_mask` filters which callbacks fire (bit set in mask
AND pointer non-NULL).

**Repo:** `set_listener(p, l, status_mask)` stores the mask; every fire
site checks `mask & STATUS_*` before invoking. Bit constants
`STATUS_INCONSISTENT_TOPIC` … `STATUS_SUBSCRIPTION_MATCHED` match the DDS
PSM values.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`
(mask `0xFFFFFFFF`).

**Status:** done

---

## §4 Threading contract

### §4.1 Caller-driven poll delivery

**Spec:** §4.1 — callbacks delivered from `zerodds_poll_listeners()`, never
synchronously inside a `set_listener`/`write`/`take` call.

**Repo:** `zerodds_poll_listeners` is the only fire site; the registry
holds raw pointers and `set_listener` never invokes callbacks.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.

**Status:** done

### §4.2 Re-entrancy

**Spec:** §4.2 — inside a callback the caller may read; must not free the
listener/entity.

**Repo:** registry snapshots are taken and all locks released before any
callback runs, so a callback may re-enter read APIs without deadlock.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`
(callbacks run outside the registry locks).

**Status:** done

### §4.3 Lifetime

**Spec:** §4.3 — listener pointer is caller-owned; registry holds it weak.

**Repo:** `static OnceLock<ListenerRegistry>` stores raw pointers;
`set_listener(NULL)` clears.

**Tests:** `listener_ffi::tests::dw_set_listener_clear_via_null`.

**Status:** done

---

## §5 Aggregator model — caller-driven multi-bind

**Spec:** §5 — independent multi-level firing (no first-match suppression);
the same listener pointer bindable to several entities; aggregators walk
their children.

**Repo:** `zerodds_poll_listeners` computes each entity's delta once, then
fires the DataWriter/DataReader level and each bound aggregator level
independently via `collect_publisher_writers` /
`collect_subscriber_readers` / `collect_participant_subscriber_readers`.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`
(DataReader + Subscriber both fire for one sample).

**Status:** done

---

## §6 Active firing

### §6.1 Writer statuses

**Spec:** §6.1 — 4 writer callbacks fire on a writer-counter delta at the
DataWriter and Publisher levels.

**Repo:** `read_writer_counters` + `writer_delta` + `fire_writer_vtable`;
counters from `DcpsRuntime::user_writer_*`.

**Tests:** `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.

**Status:** done

### §6.2 Reader statuses

**Spec:** §6.2 — 6 reader status callbacks (matched, sample_lost,
deadline, incompatible_qos, liveliness_changed, sample_rejected) fire on a
reader-counter delta at the DataReader and Subscriber levels.

**Repo:** `read_reader_counters` (incl. `user_reader_liveliness_status`,
`user_reader_sample_rejected`) + `reader_delta` + `fire_reader_vtable`.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

### §6.3 Data availability (set semantics)

**Spec:** §6.3 — `on_data_available` on a delivered-sample delta;
`on_data_on_readers` once per poll per subscriber with fresh data.

**Repo:** `UserReaderSlot::samples_delivered_count` (bumped only at the
user-delivery sites) exposed via `DcpsRuntime::user_reader_samples_delivered`;
the Subscriber/DP aggregators fire `on_data_on_readers` once per
subscriber with a data delta.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.

**Status:** done

### §6.4 Inconsistent topic

**Spec:** §6.4 — local clash prevented at `create_topic`; remote
type-mismatch detected during SEDP matching, fires `on_inconsistent_topic`
on Topic and DomainParticipant listeners.

**Repo:** `zerodds_dp_create_topic` rejects local clashes;
`DiscoveredEndpointsCache::topic_name_conflicts` +
`DcpsRuntime::inconsistent_topic_seq` bumped in
`match_local_{writer,reader}_against_cache`, read via
`inconsistent_topic_count`; poll fires on the Topic/DP levels.

**Tests:** `sedp::cache::tests::topic_name_conflicts_detects_type_mismatch`,
`listener_ffi::tests::poll_fires_inconsistent_topic_for_topic_and_participant`.

**Status:** done

---

## §7 Cross-language mapping

### §7.1 C++ Bridge

**Spec:** §7.1 — C++ wraps the C vtable.

**Repo:** `crates/cpp/include/dds/pub/DataWriterListener.hpp`,
`crates/cpp/include/dds/sub/DataReaderListener.hpp` — one
`_DataWriterListenerBridge`/`_DataReaderListenerBridge` each, with static
`extern "C"` shims; `user_data` points at the C++ listener subclass.

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_listener_bridge_dispatches_to_virtual`
— reads back the vtable the bridge installed, via `zerodds_dw_get_listener`/
`zerodds_dr_get_listener`, and invokes its entries exactly as
`zerodds_poll_listeners()` does (`fn(vt->user_data, handle)`): `user_data` is
the C++ subclass itself, `on_publication_matched` and `on_liveliness_lost`
each reach their own virtual method on the writer, `on_data_available`
reaches the reader's, a NULL `user_data` fires nothing, and
`attach(..., nullptr, 0)` unregisters again.
`crates/cpp/tests/smoke_dds_psm.cpp::test_listener_attach_detach` covers the
no-poll case beside it. Driven through
`crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke`; locally (macOS,
this run) 1/1 green, `OK — all DDS-PSM-Cxx smoke tests passed`.

**Status:** done

### §7.2 C# Bridge

**Spec:** §7.2 — `IDataWriterListener<T>` + `GCHandle` `user_data`;
`ListenerPoll.PollAll()`.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Listener.cs` —
`IDataWriterListener<T>`/`IDataReaderListener<T>`, the
`[UnmanagedCallersOnly(CallConvs = [CallConvCdecl])]` trampolines in
`ListenerTrampolines` (no delegate marshalling, no reflection),
`WriterListenerRegistration`/`ReaderListenerRegistration` holding the vtable in
`Marshal.AllocHGlobal` memory and the sink in a `GCHandle` whose
`GCHandle.ToIntPtr` is the `user_data`; `ListenerPoll.PollAll()` calls
`zerodds_poll_listeners`. Bound to the C FFI through
`crates/cs/csharp/ZeroDDS/src/Native.cs`
(`DwSetListenerRaw`/`DrSetListenerRaw`, `PollListeners`).

**Tests:** `crates/cs/csharp/ZeroDDS.Tests/Program.cs`, the "Listener attach +
active firing" block: no callback before the first poll (§4.1), one
`on_publication_matched` and one `on_subscription_matched` on the first poll
with the status read back (§6.1/§6.2), no second callback without a delta (§6),
`on_data_available` after a `Write` (§6.3), and no callback at all once the
registration is disposed (§1.3/§9). Run through
`crates/cs/csharp/build_cs_smoke.sh`:
`OK — all C# DDS-PSM-Cxx smoke tests passed`.

**Status:** done

### §7.3 Java Bridge

**Spec:** §7.3 — in-process Java-heap listeners on the `InProcessBus` +
multi-process gRPC bridge.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/sub/DataReaderListener.java`
and `.../pub/DataWriterListener.java` (Java-heap objects, with no vtable and no
`user_data` in between), bound through `DataReader.setListener`/`getListener`
and `DataWriter.setListener`/`getListener`; the match events come from
`org/zerodds/internal/InProcessBus.java`
(`Endpoint.onWriterPresenceChanged`, `WriterPresence.onReaderPresenceChanged`)
and `on_data_available` straight out of the delivery path `DataReader.onWrite`.
Status types
`org/omg/dds/core/status/{SubscriptionMatchedStatus,PublicationMatchedStatus}.java`.
Multi-process through `org/zerodds/bridge/GrpcBridgeClient.java`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/ListenerTest.java`
(5 cases: the match plus one callback per sample, the writer-side
`on_publication_matched` including the un-match, `setListener(null)` delivering
nothing further while the samples still arrive, a non-overlapping PARTITION not
being a match, and the status staying readable with no listener bound),
`crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java`
with `crates/java-omgdds/run_grpc_bridge_e2e.sh` (3/3 against a running
`zerodds-grpc-bridged --features dds-runtime`). `mvn test` overall: 95 tests,
0 failures.

**Status:** done

### §7.4 Python Bridge

**Spec:** §7.4 — caller-driven polling API as the idiomatic
listener-equivalent under the GIL.

**Repo:** `crates/py/src/ffi.rs` (`wait_for_data`,
`wait_for_matched_subscription`, `wait_for_matched_publication` — the waiting
operations release the GIL via `py.detach`), `crates/py/src/listener.rs`
(`DataWriterListener`/`DataReaderListener` as callback holders).

**Tests:** `crates/py/python/tests/test_listener.py` (6 cases over listener
construction and `set_listener` on the reader and the writer),
`crates/py/python/tests/test_smoke.py`.

**Status:** done

### §7.5 TypeScript Bridge

**Spec:** §7.5 — caller-driven polling API matching the single-threaded
Node event loop.

**Repo:** `crates/ts-node/src/dds.ts` (`DataReader.waitForMatched`,
`DataWriter.waitForMatched`, plus the promise forms
`waitForMatchedSubscription`/`waitForMatchedPublication`).

**Tests:** `crates/ts-node/test/smoke.test.ts` — `"ts pub-sub roundtrip"` waits for the
match with `DataWriter.waitForMatched(1, 5000n)` before writing;
`"typed facade: createTypedTopic + createTypedWriter"`,
`"async layer: writeAsync / takeAsync / streamSample"` and
`"fluent facade: bytes pub/sub roundtrip"` use
`waitForMatchedSubscription`/`waitForMatchedPublication` as the
caller-driven listener equivalent;
`"dds-psm-cxx api: GuardCondition + WaitSet"` covers the wait path.
`crates/ts-node/test/qos.test.ts` exercises the same API over nine QoS
cases. Run `npm test` in `crates/ts-node`.

**Status:** done

---

## §8 Test obligations

**Spec:** §8 — identity round-trip, NULL clear, poll fires on delta /
nothing without delta, aggregator firing, inconsistent-topic firing.

**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs` — the set/get API and
`zerodds_poll_listeners()` together with its aggregators.

**Tests:** `crates/zerodds-c-api/src/listener_ffi.rs` — five tests, one per
obligation in the clause:
- Obligation 1 (identity round-trip): `listener_ffi::tests::dp_set_get_listener_roundtrip`.
- Obligation 2 (NULL clear): `listener_ffi::tests::dw_set_listener_clear_via_null`.
- Obligation 3 (poll fires on a delta, nothing without one):
  `listener_ffi::tests::poll_listeners_returns_count_and_clears_state`.
- Obligation 4 (aggregator firing incl. `on_data_on_readers` set semantics):
  `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`.
- Obligation 5 (inconsistent topic on the Topic and the DomainParticipant):
  `listener_ffi::tests::poll_fires_inconsistent_topic_for_topic_and_participant`.

Run with `cargo test -p zerodds-c-api --lib listener_ffi`; locally (macOS,
this run) 5/5 green, 0 failures.

**Status:** done

---

## §9 Memory ownership

**Spec:** §9 — caller owns the listener struct; registry holds it weak;
caller must clear before freeing.

**Repo:** raw-pointer registry; `set_listener(NULL)` clears; tests clear
listeners before entity teardown so no dangling pointer survives a later
poll.

**Tests:** `listener_ffi::tests::dw_set_listener_clear_via_null`.

**Status:** done

---

## §10 Stability

**Spec:** §10 — semver policy: append-only struct evolution, major bump
for breaking changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — a forward-looking versioning policy, not
an implementable requirement.

---

## §11 Spec-conformance notes

**Spec:** §11 — all DDS 1.4 §2.2.4 listener methods exposed 1:1 and
actively fired; `user_data` is a vendor detail.

**Repo:** all 17 callbacks across the 6 structs are exposed and fired
(§6); `user_data` threaded through every fire site.

**Tests:** `listener_ffi::tests::poll_fires_data_available_and_data_on_readers`,
`poll_fires_inconsistent_topic_for_topic_and_participant`,
`poll_listeners_returns_count_and_clears_state`.

**Status:** done

---

## Audit-Status

26 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-c-api -p zerodds-discovery -p zerodds-dcps --lib`
— 791 tests green, 0 failed, 1 ignored.
Language sides: `crates/cs/csharp/build_cs_smoke.sh` with no failure;
`mvn test` in `crates/java-omgdds/java/` 95 tests, 0 failures, `ListenerTest`
5/5 among them; `run_grpc_bridge_e2e.sh` 3/3;
`pytest crates/py/python/tests/test_listener.py`;
`cargo test -p zerodds-cpp --test cpp_smoke_via_cargo` 1/1 (§7.1, locally
macOS, this run); `npm test` in `crates/ts-node` — `smoke.test.ts` 8/8
(§7.5, locally macOS, this run);
`cargo test -p zerodds-c-api --lib listener_ffi` 5/5 (§8, locally macOS,
this run).

Open items: none. Decision records: none.
