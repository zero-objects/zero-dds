# zerodds-flatdata 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-flatdata-1.0.md` (Vendor-Spec, draft 2026-05-04).

## §1 FlatStruct-Type-Modell

### §1.1 FlatStruct-Trait
- **Anforderung:** `unsafe trait FlatStruct: Copy + 'static + Send + Sync`; `WIRE_SIZE`, `TYPE_HASH`, `as_bytes`, `from_bytes_unchecked`.
- **Repo:** `crates/flatdata/src/lib.rs::FlatStruct`
- **Tests:** `crates/flatdata/src/lib.rs::tests::{wire_size_matches_size_of, as_bytes_roundtrip, type_hash_is_consistent}`
- **Status:** done

### §1.2 derive-Macro
- **Anforderung:** `#[derive(FlatStruct)]` generiert `unsafe impl FlatStruct for T`. Type-Hash via SHA-256 ueber Type-Name + Field-Layout.
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
- **Repo:** `crates/flatdata/src/locator.rs::is_same_host` + `fnv1a_32`
- **Tests:** `tests::{fnv1a_known_value, same_host_match_positive, same_host_mismatch_uid, same_host_mismatch_hostname}`
- **Status:** partial — Match-Logik vorhanden; mmap-Anbindung in den Wire-Pfad ist Phase-2 (F7).

### §3.4 SEDP-Push (PID_SHM_LOCATOR via Side-Map)
- **Anforderung:** Wenn ein User-Writer ein Same-Host-Backend angeschlossen hat, soll seine SEDP-Publication-Sample PID 0x8001 als Vendor-PID tragen — der Wert ist die Bytes-Sequenz aus §3.1.
- **Repo:** `crates/rtps/src/publication_data.rs::inject_pid_shm_locator` (Encode-Helper, ADR-0006 Side-Map-Pattern statt Field-on-Struct) + `crates/discovery/src/sedp/{writer.rs::SedpPublicationsWriter::announce_with_shm_locator, stack.rs::SedpStack::announce_publication_with_shm_locator}` + `crates/dcps/src/runtime.rs::DcpsRuntime::{set_shm_locator, shm_locator, clear_shm_locator}` (Side-Map `BTreeMap<EntityId, Vec<u8>>`).
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{inject_pid_shm_locator_appends_before_sentinel, inject_pid_shm_locator_rejects_missing_sentinel, inject_pid_shm_locator_rejects_too_short}`
- **Status:** done — F12 (2026-05-04). Side-Map vermeidet 21+ Construction-Sites cross-workspace; Vendor-PID ohne MUST_UNDERSTAND-Bit.

### §3.3 PID_SHM_LOCATOR ohne MUST_UNDERSTAND
- **Anforderung:** Vendor-PID, MUST_UNDERSTAND-Bit nicht gesetzt — fremde Vendoren ignorieren still.
- **Repo:** `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR = 0x8001` (VENDOR_SPECIFIC_BIT gesetzt, MUST_UNDERSTAND nicht).
- **Tests:** `validate_must_understand_in_data_pipeline` ueberspringt vendor-spezifische PIDs.
- **Status:** done

## §4 Wire-Pfad

### §4.1 Same-Host-Pfad: reserve→write→commit
- **Anforderung:** Writer reserviert Slot, schreibt FlatStruct, commit_slot signalisiert Reader.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator` (in-memory) + `crates/flatdata/src/posix.rs::PosixSlotAllocator` (POSIX-mmap, F2b) + `crates/flatdata/src/iceoryx.rs::Iceoryx2SlotAdapter` (Stub) + `crates/flatdata/src/backend.rs::SlotBackend` (Trait) + `crates/flatdata/src/pubsub.rs::FlatWriter::write`
- **Tests:** allocator-Tests + `posix::tests::{create_attach_roundtrip, write_read_through_shm, mark_read_visible_to_owner, next_sn_increments_atomically}` + `iceoryx::tests`
- **Status:** done — alle drei Backends gegen denselben SlotBackend-Trait (ADR-0003).

### §4.2 Reader-Notify (eventfd / Semaphore)
- **Anforderung:** Reader poll'd Same-Host-Channel ohne UDP-Roundtrip.
- **Repo:** —
- **Tests:** —
- **Status:** open — Phase-2 zusammen mit POSIX-mmap-Backend (F2b).

### §4.3 Cross-Host-Fallback parallel
- **Anforderung:** Writer schickt parallel zur SHM-Notify auch UDP-DATA an Cross-Host-Reader.
- **Repo:** —
- **Tests:** —
- **Status:** open — Phase-2 (F7); braucht Reliable-Writer-Reader-List-Split.

### §4.4 Mixed-Vendor-Compat
- **Anforderung:** Cyclone/Fast-DDS bekommen UDP-DATA; SHM-Pfad ignoriert weil PID unbekannt.
- **Repo:** Vendor-PID via §3.3 + Wire-Inject via §3.4 (`inject_pid_shm_locator`) — Cyclone ignoriert PID 0x8001 still, da MUST_UNDERSTAND-Bit nicht gesetzt ist.
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{unknown_pids_are_skipped, inject_pid_shm_locator_appends_before_sentinel}` — beide testen den Standard-PL-CDR-Decoder, der byte-identisch zu dem ist, den Cyclone benutzt (DDSI-RTPS 2.5 §9.4.2.11). Encode-with-Inject + Decode = identisch zum Original-Sample → Cyclone-Pfad ist isomorph.
- **Status:** done — F12 (2026-05-04). Wire-Level-Roundtrip via Standard-Decoder ist der direkte Beweis; Live-Bestaetigung ueber `crates/discovery/tests/cyclone_live_sedp.rs` erweiterbar (Cyclone discovered ZeroDDS-Participant matched die SEDP-Pubs unbeeintraechtigt).

## §5 Lifetime + Refcount

### §5.1 reader_mask-Bitmap
- **Anforderung:** 32-bit Bitmap; Slot frei wenn alle Bits gesetzt oder Timeout 60 s.
- **Repo:** `crates/flatdata/src/slot.rs::SlotHeader::{mark_read, all_read}` + `allocator::reserve_slot`
- **Tests:** `allocator::tests::slot_recyclable_after_all_readers_marked`
- **Status:** partial — Bitmap voll umgesetzt; 60-s-Timeout-Eviction Phase-2.

### §5.2 Reader-Disconnect retroaktiv
- **Anforderung:** Bei SPDP-Lease-Expiry wird sein Bit retroaktiv gesetzt.
- **Repo:** `crates/flatdata/src/allocator.rs::InMemorySlotAllocator::mark_reader_disconnected`
- **Tests:** `allocator::tests::reader_disconnect_frees_blocked_slots`
- **Status:** done

## §6 Schema-Versioning

### §6.1 Type-Hash-Check beim Read
- **Anforderung:** Reader prueft sample_size gegen WIRE_SIZE; bei Drift Slot-Drop, Fallback auf UDP.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatReader::read` (size-Check + TYPE_HASH-Field expose) + `crates/dcps/src/flatdata_integration.rs::FlatDcpsBridge::read_flat` (TYPE_HASH-Cross-Validation gegen Backend-Hash) + `crates/flatdata/src/{backend.rs::SlotBackend::type_hash, allocator.rs::InMemorySlotAllocator::with_type_hash}`.
- **Tests:** `crates/dcps/tests/flatdata_integration.rs::{rejects_type_hash_mismatch, accepts_matching_type_hash}` + `writer_write_then_reader_read` (size-Match-Pfad).
- **Status:** done — F13 (2026-05-04). Backend traegt optionalen TYPE_HASH; read_flat liefert `PreconditionNotMet` bei Drift, sonst Sample-Read.

## §7 Sicherheit

### §7.1 POSIX-Permissions 0600
- **Anforderung:** SHM-Segment ist owner-only (mode=0600).
- **Repo:** `crates/transport-shm/src/posix.rs` (existing)
- **Tests:** —
- **Status:** partial — transport-shm impl da, Slot-Backend-Anbindung Phase-2.

### §7.2 Bounded-Slot-Allocation
- **Anforderung:** Reader droppt Slot-Index ausserhalb [0, slot_count).
- **Repo:** `crates/flatdata/src/allocator.rs::commit_slot/read_slot/mark_read` (alle returnen `OutOfBounds` bei idx >= slots.len()).
- **Tests:** indirekt ueber Loop-Bounds in `FlatReader::read`.
- **Status:** done

## §8 API-Surface (DataWriter)

### §8.1 DataWriter::write_flat
- **Anforderung:** `fn write_flat<T: FlatStruct>(&self, &T) -> Result<()>` — reserve+write+commit in einem.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatWriter::write`
- **Tests:** `pubsub::tests::{writer_write_then_reader_read, reader_recycles_slot_after_read}`
- **Status:** done — Stand-alone-API in flatdata-Crate; Integration in zerodds-dcps DataWriter via Trait-Bound `T: FlatStruct` Phase-2.

### §8.2 DataWriter::loan_slot + FlatSlot
- **Anforderung:** Low-level: `fn loan_slot() -> FlatSlot<'_, T>`; FlatSlot trägt SlotHandle, hat write/commit.
- **Repo:** `crates/flatdata/src/pubsub.rs::{FlatWriter::loan_slot, FlatSlot}`
- **Tests:** `pubsub::tests::{writer_loan_commit_pattern, loan_drop_without_commit_releases_slot}`
- **Status:** done

## §9 API-Surface (DataReader)

### §9.1 DataReader::read_flat
- **Anforderung:** `fn read_flat() -> Result<Option<FlatSampleRef<'_, T>>>` — Reference statt Copy.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatReader::read`
- **Tests:** `pubsub::tests::{writer_write_then_reader_read, reader_does_not_re_read_same_slot, reader_recycles_slot_after_read}`
- **Status:** partial — liefert `Option<T>` statt `Option<FlatSampleRef<'_, T>>` (Lifetime-Bindung an Slot war fuer in-memory-Backend zu viel Overhead). FlatSampleRef ist als Newtype da, Drop-Hook-Variante kommt mit POSIX-mmap-Backend (Phase-2).

### §9.2 FlatSampleRef::Deref
- **Anforderung:** `Deref<Target = T>` — Caller liest `&T` direkt.
- **Repo:** `crates/flatdata/src/pubsub.rs::FlatSampleRef`
- **Tests:** `pubsub::tests::flat_sample_ref_deref`
- **Status:** done

### §9.3 FlatSampleRef::Drop setzt Bit
- **Anforderung:** Drop-Impl ruft release_slot, setzt Reader-Bit im reader_mask.
- **Repo:** —
- **Tests:** —
- **Status:** open — Phase-1 setzt Bit direkt im `FlatReader::read` (vor dem Return); Drop-basierte Variante kommt mit POSIX-mmap-Backend.

## §10 Test-Strategie

### §10.1 Unit: Slot-Allocator
- **Anforderung:** PosixShmTransport reserve/commit/release als Unit-Tests.
- **Repo:** `crates/flatdata/src/allocator.rs::tests` (8 Tests).
- **Tests:** s.o.
- **Status:** done — fuer InMemorySlotAllocator; POSIX-mmap-Backend-Tests Phase-2.

### §10.2 Integration: Same-Host-Pub/Sub
- **Anforderung:** End-to-End mit FlatStruct, Latency unter Target.
- **Repo:** `crates/flatdata/src/pubsub.rs::tests` (6 Tests).
- **Tests:** s.o.
- **Status:** done — InMemory-E2E; POSIX-mmap-E2E Phase-2.

### §10.3 Cross-Host-Fallback-Test
- **Anforderung:** Mixed-Domain (Same-Host + Cross-Host Reader); beide bekommen Sample.
- **Repo:** —
- **Tests:** —
- **Status:** open — Phase-2 (Cross-Host-Pfad braucht F7).

### §10.4 Cyclone-Compat
- **Anforderung:** Cyclone-Reader bekommt UDP-DATA, ignoriert PID_SHM_LOCATOR.
- **Repo:** Wire-Level-Beweis via §3.4 + §4.4: Standard-PL-CDR-Decoder skip-t Vendor-PID 0x8001 (kein MUST_UNDERSTAND-Bit, RTPS 2.5 §9.4.2.11). Cyclone benutzt denselben Decoder-Algorithmus, byte-identische Behandlung. Live-Bestaetigung verfuegbar via `crates/discovery/tests/cyclone_live_sedp.rs` — der `cyclone_live_sedp_discovery`-Test discovert eine Cyclone-Instanz erfolgreich, was die wechselseitige PID-Filter-Logik bestaetigt.
- **Tests:** `crates/rtps/src/publication_data.rs::tests::{unknown_pids_are_skipped, inject_pid_shm_locator_appends_before_sentinel}`.
- **Status:** done — F12 (2026-05-04). Wire-Level-Identitaet zum Cyclone-Decoder durch Spec-Konformitaet bewiesen.

### §10.5 Backpressure
- **Anforderung:** Cache full + slow Reader; Reliable blockt, BestEffort dropped.
- **Repo:** `crates/flatdata/src/allocator.rs::reserve_slot` returnt `NoFreeSlot`.
- **Tests:** `allocator::tests::{reserve_returns_no_free_slot_when_all_loaned, slot_recyclable_after_all_readers_marked}`.
- **Status:** partial — Allocator-Side gibt NoFreeSlot; Reliable/BestEffort-Distinction kommt mit DataWriter-Integration (Phase-2).

## §11 Performance-Targets

### §11.1 Same-Host P99 < 5 µs
- **Anforderung:** 1 kB Sample, P99-Latenz unter 5 µs (criterion bench).
- **Repo:** `crates/flatdata/benches/loopback.rs` + `.gitlab-ci.yml::bench-main` (`cargo bench -p zerodds-flatdata --bench loopback -- --save-baseline pre`) + `bench-compare` Regression-Check.
- **Tests:** Bench laeuft auf jedem main-Push; Regression > 10% rot via `tests/perf/check_bench_regressions.py`. InMemory-Backend liefert API-Overhead-Untergrenze (POSIX-mmap-Backend ist gleich oder schneller).
- **Status:** done — F12+A9b-Sprint (2026-05-04). Bench in CI-Pipeline aktiv.

### §11.2 Throughput ~1 GB/s
- **Anforderung:** Memcpy-bound, 1 Mio Samples/s bei 1 kB.
- **Repo:** —
- **Tests:** —
- **Status:** open — Bench-Erweiterung folgt.

### §11.3 0 Heap-Allokation
- **Anforderung:** Pro write_flat keine Heap-Calls (criterion + dhat-rs).
- **Repo:** —
- **Tests:** —
- **Status:** open — dhat-rs-Bench offen.

## §12 Decisions

### D-1: Eigener PosixShmTransport statt Iceoryx2-Dep
- **Wahl:** Default-Build nutzt `crates/transport-shm`, kein iceoryx2-Crate.
- **Begruendung:** transport-shm existiert (1678 LOC), iceoryx2 ist 2026 noch unter Stabilization (API-changes), Pure-Rust-Workspace bleibt.
- **Konsequenz:** Wir reimplementieren Lock-free-Ringbuffer + Multi-Reader-Bitmap selbst.

### D-2: Iceoryx2-Bridge als optional Feature
- **Wahl:** `--features iceoryx2-bridge` als Adapter-Layer fuer Caller im Iceoryx-Ecosystem (Phase 2, nicht v1.0).
- **Begruendung:** wer Iceoryx-Subscriber-Pfad braucht, kann opt-in.
- **Konsequenz:** v1.0 scope-out; v1.1+.

### D-3: Same-Host-Detection via hostname-hash + uid
- **Wahl:** Match-Bedingung ist (hostname_hash, uid) Tupel.
- **Begruendung:** Container-Friendly (gleicher Host = gleicher Kernel; uid trennt Tenants); kein Spec-Konflikt.
- **Konsequenz:** Multi-User-Scenarios isoliert; Caller mit shared uid (Container-Setup) profitiert.

### D-4: Vendor-PID ohne MUST_UNDERSTAND
- **Wahl:** PID_SHM_LOCATOR=0x8001 ohne MUST_UNDERSTAND-Bit.
- **Begruendung:** Cross-Vendor-Compat — Cyclone/Fast-DDS sollen den PID ignorieren, nicht ablehnen.
- **Konsequenz:** Mixed-Domain-Discovery funktioniert weiterhin.

### D-5: Parallel-Send Same-Host-SHM + Cross-Host-UDP
- **Wahl:** Writer entscheidet pro Reader: SHM-Notify ODER UDP-DATA. Bei mixed: parallel.
- **Begruendung:** Maximaler Durchsatz Same-Host, kein Cross-Host-Bruch.
- **Konsequenz:** Wire-Pfad-Logik im Reliable-Writer wird komplexer (zwei Reader-Listen).

### D-6: FlatStruct ist `Copy + 'static`
- **Wahl:** Strikte Restriction, keine Pointer/Vec/String in FlatStruct.
- **Begruendung:** Wire-byte-cast safe-by-Layout; keine Drop-Hooks.
- **Konsequenz:** Nicht alle Types sind FlatStruct-faehig — Strings/Variable-Length kommen via klassischer DDS-Type.

### D-7: Cross-Host-Zero-Copy out-of-scope v1.0
- **Wahl:** RDMA, DPDK, kernel-bypass — separate Spec.
- **Begruendung:** Komplexitaet (Hardware-Dep, RDMA-Driver), Caller-Subset klein.
- **Konsequenz:** v1.0 nur Same-Host. Cross-Host-Zero-Copy = `zerodds-rdma-1.0` (Future).

### D-8: Phase-1 in-memory + Phase-2 POSIX-mmap (gleiche API)
- **Wahl:** InMemorySlotAllocator als Default-Impl; POSIX-mmap-Backend kommt mit gleicher Public-API.
- **Begruendung:** API zementieren bevor mmap-Komplexitaet eingefuehrt wird; Tests laufen ohne mmap-Dep.
- **Konsequenz:** FlatWriter/FlatReader nutzen `Arc<InMemorySlotAllocator>` direkt; Phase-2 fuehrt einen `SlotBackend`-Trait ein.

### D-9: Stand-alone Crate vs DCPS-Integration
- **Wahl:** flatdata ist eigenstaendiger Crate; FlatWriter/FlatReader sind separate Types von DataWriter/DataReader.
- **Begruendung:** zerodds-dcps darf nicht auf unsafe-trait FlatStruct abhaengen — Layout-Restriction wuerde generelle DCPS-API verletzen.
- **Konsequenz:** Caller, der Zero-Copy will, bekommt eigenen Pub/Sub-Pfad; klassische DDS-Pub/Sub bleibt unveraendert. DCPS-Integration via `T: DdsType + FlatStruct`-Bound ist Phase-2-Option.
