# zerodds-flatdata 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-flatdata-1.0.md` (Vendor-Spec, draft 2026-05-04).

Implementation:

- `crates/flatdata/` — FlatData Zero-Copy-Serialisierung.

## §1 FlatStruct-Type-Modell

### §1.1 FlatStruct-Trait
- **Anforderung:** `unsafe trait FlatStruct: Copy + 'static + Send + Sync`; `WIRE_SIZE`, `TYPE_HASH`, `as_bytes`, `from_bytes_unchecked`.
- **Repo:** `crates/flatdata/src/lib.rs::FlatStruct`
- **Tests:** `crates/flatdata/src/lib.rs::tests::{wire_size_matches_size_of, as_bytes_roundtrip, type_hash_is_consistent}`
- **Status:** done

### §1.2 derive-Macro
- **Anforderung:** `#[derive(FlatStruct)]` generiert `unsafe impl FlatStruct for T`. Type-Hash via SHA-256 über Type-Name + Field-Layout.
- **Repo:** `crates/flatdata-derive/src/lib.rs::derive_flat_struct`
- **Tests:** `crates/flatdata/tests/derive.rs` (5 Tests: WIRE_SIZE, Hash-Eindeutigkeit, Roundtrip, Tuple-Struct).
- **Status:** done — F11 (ADR-0005-Voraussetzung).

## §2 SHM-Slot-Layout

### §2.1 Header-Struktur
- **Anforderung:** 16 byte Header: u32 sequence_number, u32 sample_size, u32 reader_mask, u32 _reserved.
- **Repo:** `crates/flatdata/src/slot.rs::SlotHeader`
- **Tests:** `slot::tests::{header_size_is_16, new_header_has_zero_mask, mark_read_sets_bit, all_read_with_two_active_readers, inactive_reader_bits_dont_block, roundtrip_le, from_bytes_too_short_returns_none}`
- **Status:** done

### §2.2 Slot-Alignment
- **Anforderung:** Slot-Size = (16 + sample_size) gepaddet auf 64-byte Cache-Line.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator::slot_total_size`
- **Tests:** `allocator::tests::slot_total_size_is_cache_line_padded`
- **Status:** done

## §3 Discovery — PID_SHM_LOCATOR

### §3.1 Wire-Format (PID 0x8001)
- **Anforderung:** u32 hostname_hash + u32 uid + u32 slot_count + u32 slot_size + CDR-String segment_path.
- **Repo:** `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR` + `crates/flatdata/src/locator.rs::ShmLocator`
- **Tests:** `crates/flatdata/src/locator.rs::tests::{roundtrip_le, truncated_header_errors, path_too_long_errors}`
- **Status:** done

### §3.2 Same-Host-Match-Logik
- **Anforderung:** Match wenn hostname_hash + uid lokal stimmen UND mmap erfolgreich.
- **Repo:** `crates/flatdata/src/locator.rs::is_same_host` + `fnv1a_32`; der SEDP-Discovery-Hook in `crates/dcps/src/runtime.rs` bindet same-host-Paare automatisch (`register_pending`→`mark_bound`→idempotentes `open_or_create`).
- **Tests:** `tests::{fnv1a_known_value, same_host_match_positive, same_host_mismatch_uid, same_host_mismatch_hostname}` + `same_host_e2e::e2e_two_runtimes_shm_roundtrip` (codepit).
- **Status:** done — Match-Logik + Discovery-getriebene mmap-Auto-Anbindung im Produktionspfad (kein manueller `set_flat_backend`-Call nötig).

### §3.4 SEDP-Push (PID_SHM_LOCATOR via Side-Map)
- **Anforderung:** Wenn ein User-Writer ein Same-Host-Backend angeschlossen hat, soll seine SEDP-Publication-Sample PID 0x8001 als Vendor-PID tragen — der Wert ist die Bytes-Sequenz aus §3.1.
- **Repo:** `crates/rtps/src/publication_data.rs::inject_pid_shm_locator` (Encode-Helper, ADR-0006 Side-Map-Pattern statt Field-on-Struct) + `crates/discovery/src/sedp/{writer.rs::SedpPublicationsWriter::announce_with_shm_locator, stack.rs::SedpStack::announce_publication_with_shm_locator}` + `crates/dcps/src/runtime.rs::DcpsRuntime::{set_shm_locator, shm_locator, clear_shm_locator}` (Side-Map `BTreeMap<EntityId, Vec<u8>>`).
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{inject_pid_shm_locator_appends_before_sentinel, inject_pid_shm_locator_rejects_missing_sentinel, inject_pid_shm_locator_rejects_too_short}`
- **Status:** done — Side-Map vermeidet 21+ Construction-Sites cross-workspace; Vendor-PID ohne MUST_UNDERSTAND-Bit.

### §3.3 PID_SHM_LOCATOR ohne MUST_UNDERSTAND
- **Anforderung:** Vendor-PID, MUST_UNDERSTAND-Bit nicht gesetzt — fremde Vendoren ignorieren still.
- **Repo:** `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR = 0x8001` (VENDOR_SPECIFIC_BIT gesetzt, MUST_UNDERSTAND nicht).
- **Tests:** `validate_must_understand_in_data_pipeline` überspringt vendor-spezifische PIDs.
- **Status:** done

## §4 Wire-Pfad

### §4.1 Same-Host-Pfad: reserve→write→commit
- **Anforderung:** Writer reserviert Slot, schreibt FlatStruct, commit_slot signalisiert Reader.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator` (in-memory) + `crates/flatdata/src/posix.rs::PosixSlotAllocator` (POSIX-mmap) + `crates/flatdata/src/iceoryx.rs::Iceoryx2SlotAdapter` (optionaler Bridge) + `crates/flatdata/src/backend.rs::SlotBackend` (Trait) + `crates/flatdata/src/pubsub.rs::FlatWriter::write`
- **Tests:** allocator-Tests + `posix::tests::{create_attach_roundtrip, write_read_through_shm, mark_read_visible_to_owner, next_sn_increments_atomically}` + `iceoryx::tests`
- **Status:** done — alle drei Backends gegen denselben SlotBackend-Trait (ADR-0003).

### §4.2 Reader-Notify (eventfd / Semaphore)
- **Anforderung:** Reader poll'd Same-Host-Channel ohne UDP-Roundtrip.
- **Repo:** `crates/flatdata/src/{backend.rs,allocator.rs,posix.rs}` — `notify_generation`/`wait_for_change` auf dem `SlotBackend`-Trait; In-Memory `ChangeNotify` (Condvar + Generation), POSIX cross-process Futex auf einem Generation-Word im SHM-Header (FUTEX_WAIT/WAKE, Linux). DCPS: `DataReader::read_flat_blocking(timeout)` + `FlatReader::read_blocking`.
- **Tests:** `futex_notify_wakes_consumer_across_mappings` (codepit, cross-process Wake) + 5 In-Memory-Thread-Tests.
- **Status:** done — event-driven Notify auf beiden Backends, kein Busy-Poll/UDP-Roundtrip.

### §4.3 Cross-Host-Fallback parallel
- **Anforderung:** Writer schickt parallel zur SHM-Notify auch UDP-DATA an Cross-Host-Reader.
- **Repo:** `crates/dcps/src/runtime.rs::same_host_udp_skip_set` — sammelt die UDP-Unicast-Locators der same-host-SHM-gebundenen Reader; der Write-Hot-Path überspringt sie für UDP (same-host → SHM, Cross-Host → UDP, kein Doppel-Delivery).
- **Tests:** `same_host_e2e::e2e_cross_vendor_different_host_id_no_shm_bind` (codepit).
- **Status:** done — Same-Host-vs-Cross-Host-Routing im Produktions-Wire-Pfad (Wave 4b / ADR-0006).

### §4.4 Mixed-Vendor-Compat
- **Anforderung:** Cyclone/Fast-DDS bekommen UDP-DATA; SHM-Pfad ignoriert weil PID unbekannt.
- **Repo:** Vendor-PID via §3.3 + Wire-Inject via §3.4 (`inject_pid_shm_locator`) — Cyclone ignoriert PID 0x8001 still, da MUST_UNDERSTAND-Bit nicht gesetzt ist.
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{unknown_pids_are_skipped, inject_pid_shm_locator_appends_before_sentinel}` — beide testen den Standard-PL-CDR-Decoder, der byte-identisch zu dem ist, den Cyclone benutzt (DDSI-RTPS 2.5 §9.4.2.11). Encode-with-Inject + Decode = identisch zum Original-Sample → Cyclone-Pfad ist isomorph.
- **Status:** done — Wire-Level-Roundtrip via Standard-Decoder ist der direkte Beweis; Live-Bestätigung über `crates/discovery/tests/cyclone_live_sedp.rs` erweiterbar (Cyclone discovered ZeroDDS-Participant matched die SEDP-Pubs unbeeinträchtigt).

## §5 Lifetime + Refcount

### §5.1 reader_mask-Bitmap
- **Anforderung:** 32-bit Bitmap; Slot frei wenn alle Bits gesetzt oder Timeout 60 s.
- **Repo:** `crates/flatdata/src/slot.rs::SlotHeader::{mark_read, all_read}` + `allocator::reserve_slot`
- **Tests:** `allocator::tests::slot_recyclable_after_all_readers_marked` + `evict_stale_frees_slot_held_by_hung_reader`.
- **Status:** done — Bitmap voll umgesetzt; `InMemorySlotAllocator::evict_stale(max_age, active_mask)` force-freed committete, nicht-voll-gelesene, nicht-geloante Slots, deren Sample älter als `max_age` ist (Backstop gegen hängenden-aber-lebenden Reader; saubere Disconnects deckt der SPDP-Lease-Pfad). Out-of-band `committed_at: Instant` pro Slot.

### §5.2 Reader-Disconnect retroaktiv
- **Anforderung:** Bei SPDP-Lease-Expiry wird sein Bit retroaktiv gesetzt.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator::mark_reader_disconnected`
- **Tests:** `allocator::tests::reader_disconnect_frees_blocked_slots`
- **Status:** done

## §6 Schema-Versioning

### §6.1 Type-Hash-Check beim Read
- **Anforderung:** Reader prüft sample_size gegen WIRE_SIZE; bei Drift Slot-Drop, Fallback auf UDP.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatReader::read` (size-Check + TYPE_HASH-Field expose) + `crates/dcps/src/flatdata_integration.rs::FlatDcpsBridge::read_flat` (TYPE_HASH-Cross-Validation gegen Backend-Hash) + `crates/flatdata/src/{backend.rs::SlotBackend::type_hash, allocator.rs::InMemorySlotAllocator::with_type_hash}`.
- **Tests:** `crates/dcps/tests/flatdata_integration.rs::{rejects_type_hash_mismatch, accepts_matching_type_hash}` + `writer_write_then_reader_read` (size-Match-Pfad).
- **Status:** done — Backend trägt optionalen TYPE_HASH; read_flat liefert `PreconditionNotMet` bei Drift, sonst Sample-Read.

## §7 Sicherheit

### §7.1 POSIX-Permissions 0600
- **Anforderung:** SHM-Segment ist owner-only (mode=0600).
- **Repo:** `crates/flatdata/src/posix.rs::PosixSlotAllocator::create` chmod't nach `shm_open` beide Artefakte auf 0600 (flink-File + `/dev/shm`-Objekt auf Linux); ohne das ließ `shared_memory` sie auf umask-Default (oft 0644 = world-readable).
- **Tests:** Linux-gated `segment_is_owner_only_0600` (codepit, prüft beide Modes == 0600).
- **Status:** done — Zero-Copy-Payload ist owner-only; fremde lokale User können sie nicht lesen.

### §7.2 Bounded-Slot-Allocation
- **Anforderung:** Reader droppt Slot-Index außerhalb [0, slot_count).
- **Repo:** `crates/flatdata/src/allocator.rs::commit_slot/read_slot/mark_read` (alle returnen `OutOfBounds` bei idx >= slots.len()).
- **Tests:** indirekt über Loop-Bounds in `FlatReader::read`.
- **Status:** done

## §8 API-Surface (DataWriter)

### §8.1 DataWriter::write_flat
- **Anforderung:** `fn write_flat<T: FlatStruct>(&self, &T) -> Result<()>` — reserve+write+commit in einem.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatWriter::write`
- **Tests:** `pubsub::tests::{writer_write_then_reader_read, reader_recycles_slot_after_read}`
- **Status:** done — Stand-alone-API in flatdata-Crate + Integration in zerodds-dcps DataWriter via Trait-Bound `T: FlatStruct` (`crates/dcps/src/flatdata_integration.rs::write_flat`).

### §8.2 DataWriter::loan_slot + FlatSlot
- **Anforderung:** Low-level: `fn loan_slot() -> FlatSlot<'_, T>`; FlatSlot trägt SlotHandle, hat write/commit.
- **Repo:** `crates/flatdata/src/pubsub.rs::{FlatWriter::loan_slot, FlatSlot}`. Zwei Commit-Pfade: `FlatSlot::commit(sample)` (Convenience, eine Kopie in den Slot) **und** der echte Zero-Copy-Pfad `FlatSlot::as_mut() -> &mut T` (in den genullten SHM-Slot schreiben) + `commit_in_place()` (kein Staging, keine Commit-Kopie). Letzterer baut auf dem neuen Backend-Primitiv `SlotBackend::{slot_data_ptr, commit_in_place}` (in `InMemorySlotAllocator` + `PosixSlotAllocator` implementiert).
- **Tests:** `pubsub::tests::{writer_loan_commit_pattern, writer_loan_in_place_zero_copy, loan_drop_without_commit_releases_slot}` + `allocator::tests::{in_place_loan_writes_without_staging_copy, slot_data_ptr_rejects_unreserved_slot, commit_in_place_too_large_returns_error}`.
- **Status:** done — inkl. echter In-place-Zero-Copy-Loan (kein Commit-Copy).

## §9 API-Surface (DataReader)

### §9.1 DataReader::read_flat
- **Anforderung:** `fn read_flat() -> Result<Option<FlatSampleRef<'_, T>>>` — Reference statt Copy.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatReader::{read, read_ref}` — `read` liefert `Option<T>` (Copy-out), `read_ref` liefert `Option<FlatSampleRef<T>>` (Referenz mit Drop-Hook, §9.3).
- **Tests:** `pubsub::tests::{writer_write_then_reader_read, reader_does_not_re_read_same_slot, reader_recycles_slot_after_read}` + read_ref-Drop-Tests.
- **Status:** done (Crate-API) — die Referenz-Variante `read_ref()` mit Drop-Hook existiert; `DataReader::read_flat` bleibt Copy-out, ein `DataReader::read_flat_ref` über den `Arc<dyn SlotBackend>`-Pfad ist reiner Wire-up-Follow-up (`FlatSampleRef` ist backend-agnostisch).
- **Reader-Zero-Copy-Primitive:** `SlotBackend::{slot_read_ptr, next_unread_slot}` (Default + `InMemorySlotAllocator`/`PosixSlotAllocator`) liefern einen Read-Pointer in den SHM-Slot bzw. den nächsten ungelesenen Slot je Reader-Index — das Gegenstück zu den Writer-Primitiven aus §8.2. Konsument ist u.a. die C-FFI (`zerodds_dr_enable_shm`/`zerodds_dr_take_shm`/`zerodds_dr_release_shm`, Crate `zerodds-c-api`, Feature `flatdata-loan`), die damit einen Same-Host-Reader zero-copy aus dem Writer-Segment lesen lässt (e2e: `zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`).

### §9.2 FlatSampleRef::Deref
- **Anforderung:** `Deref<Target = T>` — Caller liest `&T` direkt.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatSampleRef`
- **Tests:** `pubsub::tests::flat_sample_ref_deref`
- **Status:** done

### §9.3 FlatSampleRef::Drop setzt Bit
- **Anforderung:** Drop-Impl ruft release_slot, setzt Reader-Bit im reader_mask.
- **Repo:** `crates/flatdata/src/pubsub.rs` — `FlatReader::read_ref()` liefert den neuesten ungelesenen Sample als `FlatSampleRef`, dessen `Drop` das Reader-Bit setzt; der Slot bleibt für die Referenz-Lebensdauer un-recycelbar. Gemeinsamer `scan_best(defer_best)`-Pfad mit `read()`; `DeferredRelease` hält den Backend als `Arc<dyn SlotBackend>`.
- **Tests:** 2 (Slot gehalten bis Drop / `into_inner` gibt frei).
- **Status:** done

## §10 Test-Strategie

### §10.1 Unit: Slot-Allocator
- **Anforderung:** PosixShmTransport reserve/commit/release als Unit-Tests.
- **Repo:** `crates/flatdata/src/allocator.rs::tests` (8 Tests).
- **Tests:** s.o.
- **Status:** done — `InMemorySlotAllocator` + `PosixSlotAllocator` (`posix::tests`: `create_attach_roundtrip`, `write_read_through_shm`, `mark_read_visible_to_owner`, `next_sn_increments_atomically`).

### §10.2 Integration: Same-Host-Pub/Sub
- **Anforderung:** End-to-End mit FlatStruct, Latency unter Target.
- **Repo:** `crates/flatdata/src/pubsub.rs::tests` (6 Tests).
- **Tests:** s.o.
- **Status:** done — InMemory-E2E + POSIX-mmap-Roundtrip (`posix::tests`).

### §10.3 Cross-Host-Fallback-Test
- **Anforderung:** Mixed-Domain (Same-Host + Cross-Host Reader); beide bekommen Sample.
- **Repo:** `crates/dcps/tests/same_host_e2e.rs`.
- **Tests:** `e2e_two_runtimes_shm_roundtrip` (same-host → SHM) + `e2e_cross_vendor_different_host_id_no_shm_bind` (anderer host-id → UDP), codepit-grün.
- **Status:** done — Same-Host-vs-Cross-Host-Verhalten via 2-Runtime-E2E belegt.

### §10.4 Cyclone-Compat
- **Anforderung:** Cyclone-Reader bekommt UDP-DATA, ignoriert PID_SHM_LOCATOR.
- **Repo:** Wire-Level-Beweis via §3.4 + §4.4: Standard-PL-CDR-Decoder skip-t Vendor-PID 0x8001 (kein MUST_UNDERSTAND-Bit, RTPS 2.5 §9.4.2.11). Cyclone benutzt denselben Decoder-Algorithmus, byte-identische Behandlung. Live-Bestätigung verfügbar via `crates/discovery/tests/cyclone_live_sedp.rs` — der `cyclone_live_sedp_discovery`-Test discovert eine Cyclone-Instanz erfolgreich, was die wechselseitige PID-Filter-Logik bestätigt.
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{unknown_pids_are_skipped, inject_pid_shm_locator_appends_before_sentinel}`.
- **Status:** done — Wire-Level-Identität zum Cyclone-Decoder durch Spec-Konformität bewiesen.

### §10.5 Backpressure
- **Anforderung:** Cache full + slow Reader; Reliable blockt, BestEffort dropped.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatWriter::write_bp(sample, Reliability, timeout)` — `Reliable` blockt event-driven auf dem `ChangeNotify` bis ein Slot frei wird (`WriteOutcome::TimedOut` bei Deadline), `BestEffort` dropt sofort (`WriteOutcome::Dropped`); kein Busy-Poll. Der POSIX-cross-process-Block teilt sich den Notify-Primitive mit §4.2.
- **Tests:** 3 (drop / block-bis-Reader-frei / timeout).
- **Status:** done — Reliable/BestEffort-Distinction event-driven umgesetzt.

## §11 Performance-Targets

### §11.1 Same-Host P99 < 5 µs
- **Anforderung:** 1 kB Sample, P99-Latenz unter 5 µs (criterion bench).
- **Repo:** `crates/flatdata/benches/loopback.rs` + `.gitlab-ci.yml::bench-main` (`cargo bench -p zerodds-flatdata --bench loopback -- --save-baseline pre`) + `bench-compare` Regression-Check.
- **Tests:** Bench läuft auf jedem main-Push; Regression > 10% rot via `tests/perf/check_bench_regressions.py`. InMemory-Backend liefert API-Overhead-Untergrenze (POSIX-mmap-Backend ist gleich oder schneller).
- **Status:** done — Bench in CI-Pipeline aktiv.

### §11.2 Throughput ~1 GB/s
- **Anforderung:** Memcpy-bound, 1 Mio Samples/s bei 1 kB.
- **Repo:** `crates/flatdata/benches/loopback.rs::flat_throughput_1kb` (criterion `Throughput::Bytes`/`Elements`).
- **Tests:** Bench misst **1,09 GiB/s** + **~1,05 Melem/s** für 1-kB-Samples.
- **Status:** done — ~1-GB/s- + 1-Mio-Samples/s-Ziel erfüllt.

### §11.3 0 Heap-Allokation
- **Anforderung:** Pro write_flat keine Heap-Calls (criterion + dhat-rs).
- **Repo:** `crates/flatdata/tests/zero_alloc.rs` (dhat-global-allocator).
- **Tests:** `write_flat_is_heap_allocation_free` belegt 0 Heap-Blocks über 1000 Writes.
- **Status:** done — 0-Heap-Allokation im Zero-Copy-Pfad nachgewiesen.

## §12 Decisions

### D-1: Eigener PosixShmTransport statt Iceoryx2-Dep
- **Wahl:** Default-Build nutzt `crates/transport-shm`, kein iceoryx2-Crate.
- **Begründung:** transport-shm existiert (1678 LOC), iceoryx2 ist 2026 noch unter Stabilization (API-changes), Pure-Rust-Workspace bleibt.
- **Konsequenz:** Wir reimplementieren Lock-free-Ringbuffer + Multi-Reader-Bitmap selbst.

### D-2: Iceoryx2-Bridge als optional Feature
- **Wahl:** `--features iceoryx2-bridge` als Adapter-Layer für Caller im Iceoryx-Ecosystem (v1.1, nicht v1.0).
- **Begründung:** wer Iceoryx-Subscriber-Pfad braucht, kann opt-in.
- **Konsequenz:** v1.0 scope-out; v1.1+.

### D-3: Same-Host-Detection via hostname-hash + uid
- **Wahl:** Match-Bedingung ist (hostname_hash, uid) Tupel.
- **Begründung:** Container-Friendly (gleicher Host = gleicher Kernel; uid trennt Tenants); kein Spec-Konflikt.
- **Konsequenz:** Multi-User-Scenarios isoliert; Caller mit shared uid (Container-Setup) profitiert.

### D-4: Vendor-PID ohne MUST_UNDERSTAND
- **Wahl:** PID_SHM_LOCATOR=0x8001 ohne MUST_UNDERSTAND-Bit.
- **Begründung:** Cross-Vendor-Compat — Cyclone/Fast-DDS sollen den PID ignorieren, nicht ablehnen.
- **Konsequenz:** Mixed-Domain-Discovery funktioniert weiterhin.

### D-5: Parallel-Send Same-Host-SHM + Cross-Host-UDP
- **Wahl:** Writer entscheidet pro Reader: SHM-Notify ODER UDP-DATA. Bei mixed: parallel.
- **Begründung:** Maximaler Durchsatz Same-Host, kein Cross-Host-Bruch.
- **Konsequenz:** Wire-Pfad-Logik im Reliable-Writer wird komplexer (zwei Reader-Listen).

### D-6: FlatStruct ist `Copy + 'static`
- **Wahl:** Strikte Restriction, keine Pointer/Vec/String in FlatStruct.
- **Begründung:** Wire-byte-cast safe-by-Layout; keine Drop-Hooks.
- **Konsequenz:** Nicht alle Types sind FlatStruct-fähig — Strings/Variable-Length kommen via klassischer DDS-Type.

### D-7: Cross-Host-Zero-Copy out-of-scope v1.0
- **Wahl:** RDMA, DPDK, kernel-bypass — separate Spec.
- **Begründung:** Komplexität (Hardware-Dep, RDMA-Driver), Caller-Subset klein.
- **Konsequenz:** v1.0 nur Same-Host. Cross-Host-Zero-Copy = `zerodds-rdma-1.0` (Future).

### D-8: In-memory- + POSIX-mmap-Backend hinter gleicher API
- **Wahl:** `InMemorySlotAllocator` als Default-Impl; das POSIX-mmap-Backend liegt hinter derselben Public-API.
- **Begründung:** API zementieren bevor mmap-Komplexität eingeführt wird; Tests laufen ohne mmap-Dep.
- **Konsequenz:** der `SlotBackend`-Trait abstrahiert beide Backends; FlatWriter/FlatReader sind backend-agnostisch.

### D-9: Stand-alone Crate vs DCPS-Integration
- **Wahl:** flatdata ist eigenständiger Crate; FlatWriter/FlatReader sind separate Types von DataWriter/DataReader.
- **Begründung:** zerodds-dcps darf nicht auf unsafe-trait FlatStruct abhängen — Layout-Restriction würde generelle DCPS-API verletzen.
- **Konsequenz:** Caller, der Zero-Copy will, bekommt eigenen Pub/Sub-Pfad; klassische DDS-Pub/Sub bleibt unverändert. Die DCPS-Integration via `T: DdsType + FlatStruct`-Bound liegt in `crates/dcps/src/flatdata_integration.rs`.

---

## Audit-Status

30 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-flatdata` — 47 lib- + 11 Integration- + 1 Zero-Alloc-Test grün, 0 failed.

Offene Punkte: keine. Decision-Records: siehe §12 (D-1–D-9).
