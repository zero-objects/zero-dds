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
`crates/cpp/include/dds/sub/DataReaderListener.hpp`.

**Status:** done

### §7.2 C# Bridge

**Spec:** §7.2 — `IDataWriterListener<T>` + `GCHandle` `user_data`;
`ListenerPoll.PollAll()`.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Listener.cs`.

**Status:** done

### §7.3 Java Bridge

**Spec:** §7.3 — in-process Java-heap listeners on the `InProcessBus` +
multi-process gRPC bridge.

**Repo:** `crates/java-omgdds/java/` (in-process listeners) +
`org.zerodds.bridge.GrpcBridgeClient`
(`crates/java-omgdds/java/src/main/java/org/zerodds/bridge/GrpcBridgeClient.java`).

**Tests:** `crates/java-omgdds/java/src/test/java/org/zerodds/bridge/GrpcBridgeClientTest.java`,
`crates/java-omgdds/run_grpc_bridge_e2e.sh`.

**Status:** done

### §7.4 Python Bridge

**Spec:** §7.4 — caller-driven polling API as the idiomatic
listener-equivalent under the GIL.

**Repo:** `crates/py/src/ffi.rs` (`wait_for_data`,
`wait_for_matched_subscription`, `wait_for_matched_publication`).

**Status:** done

### §7.5 TypeScript Bridge

**Spec:** §7.5 — caller-driven polling API matching the single-threaded
Node event loop.

**Repo:** `crates/ts-node/src/dds.ts` (`DataReader.waitForMatched`,
`DataWriter.waitForMatched`).

**Status:** done

---

## §8 Test obligations

**Spec:** §8 — identity round-trip, NULL clear, poll fires on delta /
nothing without delta, aggregator firing, inconsistent-topic firing.

**Repo / Tests:** `listener_ffi::tests::dp_set_get_listener_roundtrip`,
`dw_set_listener_clear_via_null`,
`poll_listeners_returns_count_and_clears_state`,
`poll_fires_data_available_and_data_on_readers`,
`poll_fires_inconsistent_topic_for_topic_and_participant`.

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
— 664 tests green, 0 failed.

Open items: none. Decision records: none.
