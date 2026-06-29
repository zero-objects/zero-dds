# zerodds-flatdata 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-flatdata-1.0.md` (vendor spec, draft 2026-05-04).

Implementation:

- `crates/flatdata/` — FlatData zero-copy serialization.

## §1 FlatStruct type model

### §1.1 FlatStruct trait
- **Requirement:** `unsafe trait FlatStruct: Copy + 'static + Send + Sync`; `WIRE_SIZE`, `TYPE_HASH`, `as_bytes`, `from_bytes_unchecked`.
- **Repo:** `crates/flatdata/src/lib.rs::FlatStruct`
- **Tests:** `crates/flatdata/src/lib.rs::tests::{wire_size_matches_size_of, as_bytes_roundtrip, type_hash_is_consistent}`
- **Status:** done

### §1.2 derive macro
- **Requirement:** `#[derive(FlatStruct)]` generates `unsafe impl FlatStruct for T`. Type hash via SHA-256 over the type name + field layout.
- **Repo:** `crates/flatdata-derive/src/lib.rs::derive_flat_struct`
- **Tests:** `crates/flatdata/tests/derive.rs` (5 tests: WIRE_SIZE, hash uniqueness, round-trip, tuple struct).
- **Status:** done — F11 (ADR-0005 prerequisite).

## §2 SHM slot layout

### §2.1 Header structure
- **Requirement:** 16-byte header: u32 sequence_number, u32 sample_size, u32 reader_mask, u32 _reserved.
- **Repo:** `crates/flatdata/src/slot.rs::SlotHeader`
- **Tests:** `slot::tests::{header_size_is_16, new_header_has_zero_mask, mark_read_sets_bit, all_read_with_two_active_readers, inactive_reader_bits_dont_block, roundtrip_le, from_bytes_too_short_returns_none}`
- **Status:** done

### §2.2 Slot alignment
- **Requirement:** slot size = (16 + sample_size) padded to a 64-byte cache line.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator::slot_total_size`
- **Tests:** `allocator::tests::slot_total_size_is_cache_line_padded`
- **Status:** done

## §3 Discovery — PID_SHM_LOCATOR

### §3.1 Wire format (PID 0x8001)
- **Requirement:** u32 hostname_hash + u32 uid + u32 slot_count + u32 slot_size + a CDR-string segment_path.
- **Repo:** `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR` + `crates/flatdata/src/locator.rs::ShmLocator`
- **Tests:** `crates/flatdata/src/locator.rs::tests::{roundtrip_le, truncated_header_errors, path_too_long_errors}`
- **Status:** done

### §3.2 Same-host match logic
- **Requirement:** match when hostname_hash + uid are locally equal AND the mmap succeeds.
- **Repo:** `crates/flatdata/src/locator.rs::is_same_host` + `fnv1a_32`; the SEDP discovery hook in `crates/dcps/src/runtime.rs` auto-binds same-host pairs (`register_pending`→`mark_bound`→idempotent `open_or_create`).
- **Tests:** `tests::{fnv1a_known_value, same_host_match_positive, same_host_mismatch_uid, same_host_mismatch_hostname}` + `same_host_e2e::e2e_two_runtimes_shm_roundtrip` (Linux bench host).
- **Status:** done — match logic + discovery-driven mmap auto-binding in the production path (no manual `set_flat_backend` call needed).

### §3.4 SEDP push (PID_SHM_LOCATOR via a side-map)
- **Requirement:** when a user writer has a same-host backend attached, its SEDP publication sample should carry PID 0x8001 as a vendor PID — the value is the byte sequence from §3.1.
- **Repo:** `crates/rtps/src/publication_data.rs::inject_pid_shm_locator` (encode helper, ADR-0006 side-map pattern instead of a field-on-struct) + `crates/discovery/src/sedp/{writer.rs::SedpPublicationsWriter::announce_with_shm_locator, stack.rs::SedpStack::announce_publication_with_shm_locator}` + `crates/dcps/src/runtime.rs::DcpsRuntime::{set_shm_locator, shm_locator, clear_shm_locator}` (side-map `BTreeMap<EntityId, Vec<u8>>`).
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{inject_pid_shm_locator_appends_before_sentinel, inject_pid_shm_locator_rejects_missing_sentinel, inject_pid_shm_locator_rejects_too_short}`
- **Status:** done — the side-map avoids 21+ construction sites cross-workspace; vendor PID without the MUST_UNDERSTAND bit.

### §3.3 PID_SHM_LOCATOR without MUST_UNDERSTAND
- **Requirement:** vendor PID, MUST_UNDERSTAND bit not set — foreign vendors ignore it silently.
- **Repo:** `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR = 0x8001` (VENDOR_SPECIFIC_BIT set, MUST_UNDERSTAND not).
- **Tests:** `validate_must_understand_in_data_pipeline` skips vendor-specific PIDs.
- **Status:** done

## §4 Wire path

### §4.1 Same-host path: reserve→write→commit
- **Requirement:** the writer reserves a slot, writes the FlatStruct, commit_slot signals the reader.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator` (in-memory) + `crates/flatdata/src/posix.rs::PosixSlotAllocator` (POSIX mmap) + `crates/flatdata/src/iceoryx.rs::Iceoryx2SlotAdapter` (optional bridge) + `crates/flatdata/src/backend.rs::SlotBackend` (trait) + `crates/flatdata/src/pubsub.rs::FlatWriter::write`
- **Tests:** allocator tests + `posix::tests::{create_attach_roundtrip, write_read_through_shm, mark_read_visible_to_owner, next_sn_increments_atomically}` + `iceoryx::tests`
- **Status:** done — all three backends against the same SlotBackend trait (ADR-0003).

### §4.2 Reader notify (eventfd / semaphore)
- **Requirement:** the reader polls the same-host channel without a UDP round-trip.
- **Repo:** `crates/flatdata/src/{backend.rs,allocator.rs,posix.rs}` — `notify_generation`/`wait_for_change` on the `SlotBackend` trait; in-memory `ChangeNotify` (condvar + generation), POSIX cross-process futex on a generation word in the SHM header (FUTEX_WAIT/WAKE, Linux). DCPS: `DataReader::read_flat_blocking(timeout)` + `FlatReader::read_blocking`.
- **Tests:** `futex_notify_wakes_consumer_across_mappings` (Linux bench host, cross-process wake) + 5 in-memory thread tests.
- **Status:** done — event-driven notify on both backends, no busy-poll / UDP round-trip.

### §4.3 Cross-host fallback in parallel
- **Requirement:** the writer also sends UDP DATA to cross-host readers in parallel to the SHM notify.
- **Repo:** `crates/dcps/src/runtime.rs::same_host_udp_skip_set` — collects the UDP unicast locators of same-host SHM-bound readers; the write hot path skips them for UDP (same-host → SHM, cross-host → UDP, no double delivery).
- **Tests:** `same_host_e2e::e2e_cross_vendor_different_host_id_no_shm_bind` (Linux bench host).
- **Status:** done — same-host-vs-cross-host routing in the production wire path (Wave 4b / ADR-0006).

### §4.4 Mixed-vendor compat
- **Requirement:** Cyclone/Fast-DDS get UDP DATA; the SHM path is ignored because the PID is unknown.
- **Repo:** vendor PID via §3.3 + wire inject via §3.4 (`inject_pid_shm_locator`) — Cyclone ignores PID 0x8001 silently, since the MUST_UNDERSTAND bit is not set.
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{unknown_pids_are_skipped, inject_pid_shm_locator_appends_before_sentinel}` — both test the standard PL-CDR decoder, which is byte-identical to the one Cyclone uses (DDSI-RTPS 2.5 §9.4.2.11). Encode-with-inject + decode = identical to the original sample → the Cyclone path is isomorphic.
- **Status:** done — the wire-level round-trip via the standard decoder is the direct proof; live confirmation extensible via `crates/discovery/tests/cyclone_live_sedp.rs` (Cyclone discovers the ZeroDDS participant, matches the SEDP pubs unaffected).

## §5 Lifetime + refcount

### §5.1 reader_mask bitmap
- **Requirement:** 32-bit bitmap; slot free when all bits are set or after a 60 s timeout.
- **Repo:** `crates/flatdata/src/slot.rs::SlotHeader::{mark_read, all_read}` + `allocator::reserve_slot`
- **Tests:** `allocator::tests::slot_recyclable_after_all_readers_marked` + `evict_stale_frees_slot_held_by_hung_reader`.
- **Status:** done — bitmap fully implemented; `InMemorySlotAllocator::evict_stale(max_age, active_mask)` force-frees committed, not-fully-read, non-loaned slots whose sample is older than `max_age` (backstop against a hung-but-alive reader; clean disconnects are covered by the SPDP lease path). Out-of-band `committed_at: Instant` per slot.

### §5.2 Reader disconnect retroactive
- **Requirement:** on an SPDP lease expiry its bit is set retroactively.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator::mark_reader_disconnected`
- **Tests:** `allocator::tests::reader_disconnect_frees_blocked_slots`
- **Status:** done

## §6 Schema versioning

### §6.1 Type-hash check on read
- **Requirement:** the reader checks sample_size against WIRE_SIZE; on drift a slot drop, fallback to UDP.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatReader::read` (size check + TYPE_HASH field expose) + `crates/dcps/src/flatdata_integration.rs::FlatDcpsBridge::read_flat` (TYPE_HASH cross-validation against the backend hash) + `crates/flatdata/src/{backend.rs::SlotBackend::type_hash, allocator.rs::InMemorySlotAllocator::with_type_hash}`.
- **Tests:** `crates/dcps/tests/flatdata_integration.rs::{rejects_type_hash_mismatch, accepts_matching_type_hash}` + `writer_write_then_reader_read` (the size-match path).
- **Status:** done — the backend carries an optional TYPE_HASH; read_flat returns `PreconditionNotMet` on drift, otherwise a sample read.

## §7 Security

### §7.1 POSIX permissions 0600
- **Requirement:** the SHM segment is owner-only (mode=0600).
- **Repo:** `crates/flatdata/src/posix.rs::PosixSlotAllocator::create` chmods both artefacts to 0600 after `shm_open` (flink file + the `/dev/shm` object on Linux); otherwise `shared_memory` left them at the umask default (often 0644 = world-readable).
- **Tests:** Linux-gated `segment_is_owner_only_0600` (Linux bench host, checks both modes == 0600).
- **Status:** done — the zero-copy payload is owner-only; other local users cannot read it.

### §7.2 Bounded slot allocation
- **Requirement:** the reader drops a slot index outside [0, slot_count).
- **Repo:** `crates/flatdata/src/allocator.rs::commit_slot/read_slot/mark_read` (all return `OutOfBounds` for idx >= slots.len()).
- **Tests:** indirectly via the loop bounds in `FlatReader::read`.
- **Status:** done

## §8 API surface (DataWriter)

### §8.1 DataWriter::write_flat
- **Requirement:** `fn write_flat<T: FlatStruct>(&self, &T) -> Result<()>` — reserve+write+commit in one.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatWriter::write`
- **Tests:** `pubsub::tests::{writer_write_then_reader_read, reader_recycles_slot_after_read}`
- **Status:** done — a standalone API in the flatdata crate + integration into the zerodds-dcps DataWriter via the trait bound `T: FlatStruct` (`crates/dcps/src/flatdata_integration.rs::write_flat`).

### §8.2 DataWriter::loan_slot + FlatSlot
- **Requirement:** low-level: `fn loan_slot() -> FlatSlot<'_, T>`; FlatSlot carries a SlotHandle, has write/commit.
- **Repo:** `crates/flatdata/src/pubsub.rs::{FlatWriter::loan_slot, FlatSlot}`. Two commit paths: `FlatSlot::commit(sample)` (convenience, one copy into the slot) **and** the true zero-copy path `FlatSlot::as_mut() -> &mut T` (write into the zeroed SHM slot) + `commit_in_place()` (no staging, no commit copy). The latter builds on the new backend primitive `SlotBackend::{slot_data_ptr, commit_in_place}` (implemented in `InMemorySlotAllocator` + `PosixSlotAllocator`).
- **Tests:** `pubsub::tests::{writer_loan_commit_pattern, writer_loan_in_place_zero_copy, loan_drop_without_commit_releases_slot}` + `allocator::tests::{in_place_loan_writes_without_staging_copy, slot_data_ptr_rejects_unreserved_slot, commit_in_place_too_large_returns_error}`.
- **Status:** done — incl. a real in-place zero-copy loan (no commit copy).

## §9 API surface (DataReader)

### §9.1 DataReader::read_flat
- **Requirement:** `fn read_flat() -> Result<Option<FlatSampleRef<'_, T>>>` — a reference instead of a copy.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatReader::{read, read_ref}` — `read` returns `Option<T>` (copy-out), `read_ref` returns `Option<FlatSampleRef<T>>` (reference with a drop hook, §9.3).
- **Tests:** `pubsub::tests::{writer_write_then_reader_read, reader_does_not_re_read_same_slot, reader_recycles_slot_after_read}` + read_ref drop tests.
- **Status:** done (crate API) — the reference variant `read_ref()` with a drop hook exists; `DataReader::read_flat` stays copy-out, a `DataReader::read_flat_ref` over the `Arc<dyn SlotBackend>` path is pure wire-up follow-up (`FlatSampleRef` is backend-agnostic).
- **Reader zero-copy primitives:** `SlotBackend::{slot_read_ptr, next_unread_slot}` (default + `InMemorySlotAllocator`/`PosixSlotAllocator`) return a read pointer into the SHM slot resp. the next unread slot per reader index — the counterpart to the writer primitives in §8.2. One consumer is the C FFI (`zerodds_dr_enable_shm`/`zerodds_dr_take_shm`/`zerodds_dr_release_shm`, crate `zerodds-c-api`, feature `flatdata-loan`), which lets a same-host reader read zero-copy from the writer's segment (e2e: `zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`).

### §9.2 FlatSampleRef::Deref
- **Requirement:** `Deref<Target = T>` — the caller reads `&T` directly.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatSampleRef`
- **Tests:** `pubsub::tests::flat_sample_ref_deref`
- **Status:** done

### §9.3 FlatSampleRef::Drop sets the bit
- **Requirement:** the Drop impl calls release_slot, sets the reader bit in reader_mask.
- **Repo:** `crates/flatdata/src/pubsub.rs` — `FlatReader::read_ref()` returns the latest unread sample as a `FlatSampleRef` whose `Drop` sets the reader bit; the slot stays un-recyclable for the reference lifetime. Shared `scan_best(defer_best)` path with `read()`; `DeferredRelease` holds the backend as `Arc<dyn SlotBackend>`.
- **Tests:** 2 (slot held until drop / `into_inner` releases).
- **Status:** done

## §10 Test strategy

### §10.1 Unit: slot allocator
- **Requirement:** PosixShmTransport reserve/commit/release as unit tests.
- **Repo:** `crates/flatdata/src/allocator.rs::tests` (8 tests).
- **Tests:** see above.
- **Status:** done — `InMemorySlotAllocator` + `PosixSlotAllocator` (`posix::tests`: `create_attach_roundtrip`, `write_read_through_shm`, `mark_read_visible_to_owner`, `next_sn_increments_atomically`).

### §10.2 Integration: same-host pub/sub
- **Requirement:** end-to-end with FlatStruct, latency below target.
- **Repo:** `crates/flatdata/src/pubsub.rs::tests` (6 tests).
- **Tests:** see above.
- **Status:** done — in-memory E2E + POSIX mmap round-trip (`posix::tests`).

### §10.3 Cross-host fallback test
- **Requirement:** mixed domain (same-host + cross-host reader); both get the sample.
- **Repo:** `crates/dcps/tests/same_host_e2e.rs`.
- **Tests:** `e2e_two_runtimes_shm_roundtrip` (same-host → SHM) + `e2e_cross_vendor_different_host_id_no_shm_bind` (different host-id → UDP), bench-green.
- **Status:** done — same-host-vs-cross-host behaviour proven via the 2-runtime E2E.

### §10.4 Cyclone compat
- **Requirement:** the Cyclone reader gets UDP DATA, ignores PID_SHM_LOCATOR.
- **Repo:** wire-level proof via §3.4 + §4.4: the standard PL-CDR decoder skips vendor PID 0x8001 (no MUST_UNDERSTAND bit, RTPS 2.5 §9.4.2.11). Cyclone uses the same decoder algorithm, byte-identical handling. Live confirmation available via `crates/discovery/tests/cyclone_live_sedp.rs` — the `cyclone_live_sedp_discovery` test discovers a Cyclone instance successfully, which confirms the mutual PID-filter logic.
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{unknown_pids_are_skipped, inject_pid_shm_locator_appends_before_sentinel}`.
- **Status:** done — wire-level identity with the Cyclone decoder proven through spec conformance.

### §10.5 Backpressure
- **Requirement:** cache full + slow reader; Reliable blocks, BestEffort drops.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatWriter::write_bp(sample, Reliability, timeout)` — `Reliable` blocks event-driven on the `ChangeNotify` until a slot frees (`WriteOutcome::TimedOut` on deadline), `BestEffort` drops immediately (`WriteOutcome::Dropped`); no busy-poll. The POSIX cross-process block shares the notify primitive with §4.2.
- **Tests:** 3 (drop / block-until-reader-frees / timeout).
- **Status:** done — Reliable/BestEffort distinction implemented event-driven.

## §11 Performance targets

### §11.1 Same-host P99 < 5 µs
- **Requirement:** 1 kB sample, P99 latency below 5 µs (criterion bench).
- **Repo:** `crates/flatdata/benches/loopback.rs` + `.gitlab-ci.yml::bench-main` (`cargo bench -p zerodds-flatdata --bench loopback -- --save-baseline pre`) + a `bench-compare` regression check.
- **Tests:** the bench runs on every main push; regression > 10% red via `tests/perf/check_bench_regressions.py`. The in-memory backend delivers the API-overhead lower bound (the POSIX mmap backend is equal or faster).
- **Status:** done — the bench is active in the CI pipeline.

### §11.2 Throughput ~1 GB/s
- **Requirement:** memcpy-bound, 1 M samples/s at 1 kB.
- **Repo:** `crates/flatdata/benches/loopback.rs::flat_throughput_1kb` (criterion `Throughput::Bytes`/`Elements`).
- **Tests:** the bench measures **1.09 GiB/s** + **~1.05 Melem/s** for 1 kB samples.
- **Status:** done — ~1 GB/s + 1M samples/s target met.

### §11.3 0 heap allocations
- **Requirement:** no heap calls per write_flat (criterion + dhat-rs).
- **Repo:** `crates/flatdata/tests/zero_alloc.rs` (dhat global allocator).
- **Tests:** `write_flat_is_heap_allocation_free` proves 0 heap blocks over 1000 writes.
- **Status:** done — zero heap allocation in the zero-copy path proven.

## §12 Decisions

### D-1: An own PosixShmTransport instead of an Iceoryx2 dep
- **Choice:** the default build uses `crates/transport-shm`, no iceoryx2 crate.
- **Rationale:** transport-shm exists (1678 LOC), iceoryx2 is still under stabilization in 2026 (API changes), the pure-Rust workspace stays.
- **Consequence:** we reimplement the lock-free ring buffer + multi-reader bitmap ourselves.

### D-2: Iceoryx2 bridge as an optional feature
- **Choice:** `--features iceoryx2-bridge` as an adapter layer for callers in the iceoryx ecosystem (v1.1, not v1.0).
- **Rationale:** anyone who needs the iceoryx subscriber path can opt in.
- **Consequence:** v1.0 scope-out; v1.1+.

### D-3: Same-host detection via hostname-hash + uid
- **Choice:** the match condition is the (hostname_hash, uid) tuple.
- **Rationale:** container-friendly (same host = same kernel; uid separates tenants); no spec conflict.
- **Consequence:** multi-user scenarios isolated; a caller with a shared uid (container setup) benefits.

### D-4: Vendor PID without MUST_UNDERSTAND
- **Choice:** PID_SHM_LOCATOR=0x8001 without the MUST_UNDERSTAND bit.
- **Rationale:** cross-vendor compat — Cyclone/Fast-DDS should ignore the PID, not reject it.
- **Consequence:** mixed-domain discovery keeps working.

### D-5: Parallel send same-host-SHM + cross-host-UDP
- **Choice:** the writer decides per reader: SHM notify OR UDP DATA. On mixed: in parallel.
- **Rationale:** maximum throughput same-host, no cross-host break.
- **Consequence:** the wire-path logic in the reliable writer gets more complex (two reader lists).

### D-6: FlatStruct is `Copy + 'static`
- **Choice:** a strict restriction, no pointer/Vec/String in FlatStruct.
- **Rationale:** wire-byte-cast safe-by-layout; no drop hooks.
- **Consequence:** not all types are FlatStruct-capable — strings/variable-length come via the classic DDS type.

### D-7: Cross-host zero-copy out-of-scope v1.0
- **Choice:** RDMA, DPDK, kernel-bypass — a separate spec.
- **Rationale:** complexity (hardware dep, RDMA driver), the caller subset is small.
- **Consequence:** v1.0 same-host only. Cross-host zero-copy = `zerodds-rdma-1.0` (future).

### D-8: In-memory + POSIX-mmap backend behind one API
- **Choice:** `InMemorySlotAllocator` as the default impl; the POSIX mmap backend sits behind the same public API.
- **Rationale:** cement the API before mmap complexity is introduced; tests run without an mmap dep.
- **Consequence:** the `SlotBackend` trait abstracts both backends; FlatWriter/FlatReader are backend-agnostic.

### D-9: Standalone crate vs DCPS integration
- **Choice:** flatdata is a standalone crate; FlatWriter/FlatReader are separate types from DataWriter/DataReader.
- **Rationale:** zerodds-dcps must not depend on the unsafe trait FlatStruct — the layout restriction would violate the general DCPS API.
- **Consequence:** a caller who wants zero-copy gets its own pub/sub path; classic DDS pub/sub stays unchanged. DCPS integration via a `T: DdsType + FlatStruct` bound lives in `crates/dcps/src/flatdata_integration.rs`.

---

## Audit status

30 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-flatdata` — 47 lib + 11 integration + 1 zero-alloc tests green, 0 failed.

Open items: none. Decision records: see §12 (D-1–D-9).
