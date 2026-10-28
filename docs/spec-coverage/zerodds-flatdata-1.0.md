# zerodds-flatdata 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-flatdata-1.0.md` (ZeroDDS-Vendor-Spec,
Zero-Copy-Same-Host-Pfad).

Implementation:

- `crates/flatdata/` — FlatStruct-Trait, Slot-Layout, SlotBackend, In-Memory-
  Allocator, POSIX-shm/mmap-Backend, Iceoryx2-Bridge (Opt-in).
- `crates/flatdata-derive/` — `#[derive(FlatStruct)]`.
- `crates/rtps/` + `crates/discovery/` — Vendor-PID 0x8001 und dessen
  SEDP-Einbettung.
- `crates/dcps/` — Same-Host-Bindung, Wire-Pfad-Routing, DCPS-Integration.

Offene Punkte und Decision-Records: `zerodds-flatdata-1.0.open.md`.

## Ziele und Nicht-Ziele

### Ziel — Pure-Rust-Default-Build ohne Iceoryx-Dependency

**Spec:** `docs/specs/zerodds-flatdata-1.0.md`, Abschnitt „Ziele" —
„**Pure-Rust** — kein C-Plugin, kein Iceoryx-Cargo-Dep im Default-Build."

**Repo:** `crates/flatdata/Cargo.toml` (`default = ["std", "posix-mmap"]`;
`iceoryx2` als `optional = true` deklariert und nur über das Feature
`iceoryx2-bridge` erreichbar).

**Tests:** `crates/flatdata/tests/default_build.rs::default_features_do_not_pull_in_iceoryx2`,
`crates/flatdata/tests/default_build.rs::posix_mmap_backend_is_on_by_default`

**Status:** done

### Nicht-Ziel — Iceoryx-Wire-Compat nur als Opt-in-Feature

**Spec:** `docs/specs/zerodds-flatdata-1.0.md`, Abschnitt „Nicht-Ziele" —
„Iceoryx-Wire-Compat — nur via Optional-Feature `--features
iceoryx2-bridge` (nicht Teil v1.0)."

**Repo:** `crates/flatdata/Cargo.toml` (`iceoryx2-bridge = ["std",
"dep:iceoryx2"]`), `crates/flatdata/src/iceoryx.rs`
(`Iceoryx2Publisher`, `Iceoryx2Subscriber`, hinter `#[cfg(feature =
"iceoryx2-bridge")]`).

**Tests:** `crates/flatdata/tests/default_build.rs::iceoryx_bridge_is_an_opt_in_feature`

**Status:** done

### Nicht-Ziel — Cross-Host-Zero-Copy (RDMA, DPDK)

**Spec:** `docs/specs/zerodds-flatdata-1.0.md`, Abschnitt „Nicht-Ziele" —
„Cross-Host-Zero-Copy (RDMA, DPDK) — separate Spec."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — die Spec schließt den Punkt selbst aus
ihrem Geltungsbereich aus; es entsteht keine Anforderung an den Baum.
Die zugehörige Entwurfsentscheidung steht unten als D-7.

## §1 FlatStruct-Type-Modell

### §1.1 FlatStruct-Trait

**Spec:** §1.1 — „`pub unsafe trait FlatStruct: Copy + 'static + Send +
Sync`" mit `WIRE_SIZE = core::mem::size_of::<Self>()`, `TYPE_HASH: [u8;
16]`, `as_bytes(&self) -> &[u8]` und `unsafe fn
from_bytes_unchecked(bytes: &[u8]) -> Self`.

**Repo:** `crates/flatdata/src/lib.rs` (`FlatStruct` mit den vier
Assoziierten, Default-Body von `as_bytes` als Byte-Cast über
`WIRE_SIZE`).

**Tests:** `crates/flatdata/src/lib.rs::tests::wire_size_matches_size_of`,
`crates/flatdata/src/lib.rs::tests::as_bytes_roundtrip`,
`crates/flatdata/src/lib.rs::tests::type_hash_is_consistent`

**Status:** done

### §1.2 Derive-Macro erzeugt die Trait-Implementierung

**Spec:** §1.2 — „`#[derive(FlatStruct)]` generiert `unsafe impl
FlatStruct for T`"; „Der `TYPE_HASH` ist
`sha256(<TypeName>{<field-name>:<field-ty>,...})[..16]`."

**Repo:** `crates/flatdata-derive/src/lib.rs` (`derive_flat_struct`,
`expand`, `layout_signature`, `type_signature`; SHA-256 über die
Layout-Signatur, erste 16 Byte als `TYPE_HASH`).

**Tests:** `crates/flatdata/tests/derive.rs::derive_generates_wire_size`,
`crates/flatdata/tests/derive.rs::derive_generates_type_hash_nonzero`,
`crates/flatdata/tests/derive.rs::derive_as_bytes_roundtrip`,
`crates/flatdata/tests/derive.rs::derive_works_for_tuple_struct`,
`crates/flatdata/tests/derive.rs::derive_works_for_unit_struct`,
`crates/flatdata/tests/derive.rs::derive_hash_is_deterministic_for_identical_layout`,
`crates/flatdata-derive/src/lib.rs::tests::layout_signature_covers_name_order_and_types`

**Status:** done

### §1.2 Derive-Macro lehnt Layout-unsichere Typen ab

**Spec:** §1.2 — „Der Macro lehnt mit `compile_error!` ab, wenn `T` weder
`#[repr(C)]` noch `#[repr(transparent)]` traegt, und auf `enum`/`union`."

**Repo:** `crates/flatdata-derive/src/lib.rs` (`expand` gibt für
`Data::Enum` und `Data::Union` einen `syn::Error` zurück;
`has_repr_c_or_transparent` prüft das `repr`-Attribut).

**Tests:** `crates/flatdata-derive/src/lib.rs::tests::derive_rejects_enum`,
`crates/flatdata-derive/src/lib.rs::tests::derive_rejects_union`,
`crates/flatdata-derive/src/lib.rs::tests::derive_rejects_struct_without_repr_c`,
`crates/flatdata-derive/src/lib.rs::tests::repr_attribute_gate_accepts_c_and_transparent_only`,
`crates/flatdata/tests/derive.rs::derive_accepts_repr_transparent`

**Status:** done

### §1.2 Type-Hash erkennt Schema-Drift

**Spec:** §1.2 — die Hash-Eingabe ist `<TypeName>{<field-name>:<field-ty>,...}`,
womit Umbenennung, Feld-Reihenfolge und Feld-Typ in den Hash eingehen.

**Repo:** `crates/flatdata-derive/src/lib.rs` (`layout_signature` schreibt
Typname, Feldnamen und Feldtypen in die Hash-Eingabe).

**Tests:** `crates/flatdata/tests/derive.rs::derive_detects_field_reorder`,
`crates/flatdata/tests/derive.rs::derive_detects_field_type_change`,
`crates/flatdata/tests/derive.rs::derive_detects_type_rename`,
`crates/flatdata/tests/derive.rs::derive_generates_distinct_hash_per_layout`

**Status:** done

## §2 SHM-Slot-Layout

### §2 Slot-Header — 16 Byte, vier u32-Felder

**Spec:** §2 — Slot-Header mit `0x00 u32 sequence_number`, `0x04 u32
sample_size`, `0x08 u32 reader_mask`, `0x0c u32 _reserved`;
„Header-Size: 16 byte."

**Repo:** `crates/flatdata/src/slot.rs` (`SlotHeader`,
`SLOT_HEADER_SIZE`, `to_le_bytes`/`from_bytes` in genau dieser
Feldreihenfolge).

**Tests:** `crates/flatdata/src/slot.rs::tests::header_size_is_16`,
`crates/flatdata/src/slot.rs::tests::roundtrip_le`,
`crates/flatdata/src/slot.rs::tests::from_bytes_too_short_returns_none`,
`crates/flatdata/src/slot.rs::tests::new_header_has_zero_mask`

**Status:** done

### §2 Slot-Size — 16 + sample_size, auf 64 Byte gepaddet

**Spec:** §2 — „Slot-Size = 16 + sample_size, gepaddet auf naechste
64-byte-Boundary (Cache-Line-Alignment)."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::slot_total_size` rundet `SLOT_HEADER_SIZE +
slot_capacity` auf das nächste Vielfache von 64 auf),
`crates/flatdata/src/posix.rs` (`PosixSlotAllocator` legt das Segment mit
demselben `slot_total_size` an).

**Tests:** `crates/flatdata/src/allocator.rs::tests::slot_total_size_is_cache_line_padded`,
`crates/flatdata/src/posix.rs::tests::create_attach_roundtrip`

**Status:** done

## §3 Discovery — PID_SHM_LOCATOR

### §3.1 Wire-Format des Vendor-PID 0x8001

**Spec:** §3.1 — Vendor-PID `0x8001 PID_SHM_LOCATOR` mit `u32
hostname-hash (FNV-1a der hostname-Bytes)`, `u32 uid`, `u32 slot_count`,
`u32 slot_size`, `segment_path: CDR-String`.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SHM_LOCATOR =
0x8001`), `crates/flatdata/src/locator.rs` (`ShmLocator` mit
`to_bytes`/`from_bytes` in genau dieser Feldfolge, `fnv1a_32`).

**Tests:** `crates/flatdata/src/locator.rs::tests::roundtrip_le`,
`crates/flatdata/src/locator.rs::tests::truncated_header_errors`,
`crates/flatdata/src/locator.rs::tests::path_too_long_errors`,
`crates/flatdata/src/locator.rs::tests::fnv1a_known_value`

**Status:** done

### §3.1 PID_SHM_LOCATOR im SEDP-Publication-Sample

**Spec:** §3.1 zusammen mit §3.2 — der Reader liest den Locator „aus
PID_SHM_LOCATOR", der Writer muss ihn also in seiner
SEDP-Publication-Ankündigung mitschicken.

**Repo:** `crates/rtps/src/publication_data.rs`
(`inject_pid_shm_locator` hängt den PID vor das PID_SENTINEL),
`crates/discovery/src/sedp/writer.rs`
(`SedpPublicationsWriter::announce_with_shm_locator`),
`crates/discovery/src/sedp/stack.rs`
(`SedpStack::announce_publication_with_shm_locator`),
`crates/dcps/src/runtime.rs` (`DcpsRuntime::set_shm_locator`,
`shm_locator`, `clear_shm_locator` als Side-Map über die
`EntityId` des User-Writers).

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::announce_with_shm_locator_puts_vendor_pid_on_the_wire`,
`crates/rtps/src/publication_data.rs::tests::inject_pid_shm_locator_appends_before_sentinel`,
`crates/rtps/src/publication_data.rs::tests::inject_pid_shm_locator_rejects_missing_sentinel`,
`crates/rtps/src/publication_data.rs::tests::inject_pid_shm_locator_rejects_too_short`

**Status:** done

### §3.2 Same-Host-Match über hostname-hash und uid

**Spec:** §3.2 — „Same-Host-Match wenn: 1. `hostname-hash` aus
PID_SHM_LOCATOR == lokaler hostname-hash, UND 2. `uid` aus
PID_SHM_LOCATOR == lokaler uid".

**Repo:** `crates/flatdata/src/locator.rs` (`is_same_host` vergleicht
beide Felder gegen den lokalen Host; `fnv1a_32` als Hash),
`crates/dcps/src/runtime.rs` (`note_same_host_peer`: ein von SPDP
gemeldeter ZeroDDS-Teilnehmer mit derselben `host_id` bekommt einen
SHM-Kanal), `crates/dcps/src/same_host_channel.rs`
(`SameHostChannels::add_peer`).

**Tests:** `crates/flatdata/src/locator.rs::tests::same_host_match_positive`,
`crates/flatdata/src/locator.rs::tests::same_host_mismatch_uid`,
`crates/flatdata/src/locator.rs::tests::same_host_mismatch_hostname`,
`crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not`

**Status:** done

### §3.2 Dritte Match-Bedingung — mmap auf das Segment

**Spec:** §3.2 — „3. lokaler Process kann `mmap()` auf das Segment."
Scheitert das, gilt der Match als gescheitert.

**Repo:** `crates/flatdata/src/posix.rs` (`PosixSlotAllocator::attach`
mappt das über den Locator benannte Segment; Fehler kommen als
`PosixSlotError` zurück), `crates/dcps/src/same_host_channel.rs`
(`SameHostChannels::open`: scheitert das Mappen, bleibt der Teilnehmer
ohne Kanal und damit auf UDP; Ereignis `shm.channel_open_failed`).

**Tests:** `crates/flatdata/src/posix.rs::tests::create_attach_roundtrip`,
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`,
`crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not`
(vor dem Öffnen durch den Peer geht nichts durch den Kanal)

**Status:** done

## §4 Wire-Pfad

### §4.1 Writer reserviert einen Slot

**Spec:** §4.1 (1) — „Writer reserviert via
`PosixShmTransport::reserve_slot()` einen Slot. Liefert `SlotHandle =
(segment_id, slot_index, &mut [u8])`."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::reserve_slot`),
`crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::reserve_slot` liefert `SlotHandle` mit
`segment_id` und `slot_index`), `crates/flatdata/src/posix.rs`
(`PosixSlotAllocator` implementiert denselben Trait auf mmap).

**Tests:** `crates/flatdata/src/allocator.rs::tests::reserve_returns_first_free_slot`,
`crates/flatdata/src/allocator.rs::tests::reserve_returns_no_free_slot_when_all_loaned`,
`crates/flatdata/src/posix.rs::tests::create_attach_roundtrip`

**Status:** done

### §4.1 Writer schreibt direkt in den Slot

**Spec:** §4.1 (2) — „Writer schreibt FlatStruct direkt in
`slot.bytes_mut()`."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::slot_data_ptr` gibt den Zeiger in den
reservierten Slot heraus, `commit_in_place` schließt ohne
Staging-Kopie ab), `crates/flatdata/src/pubsub.rs`
(`FlatSlot::as_mut`, `FlatSlot::commit_in_place`).

**Tests:** `crates/flatdata/src/allocator.rs::tests::in_place_loan_writes_without_staging_copy`,
`crates/flatdata/src/allocator.rs::tests::slot_data_ptr_rejects_unreserved_slot`,
`crates/flatdata/src/pubsub.rs::tests::writer_loan_in_place_zero_copy`

**Status:** done

### §4.1 commit_slot setzt sample_size und reader_mask

**Spec:** §4.1 (3) — „Writer ruft `commit_slot(slot)`. Backend setzt
`sample_size` und `reader_mask = 0`."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::commit_slot` schreibt `sample_size`, setzt
`reader_mask` auf 0 und vergibt die nächste `sequence_number`),
`crates/flatdata/src/posix.rs` (`PosixSlotAllocator::commit_slot`,
`next_sn` als atomarer Zähler im Segment-Header).

**Tests:** `crates/flatdata/src/allocator.rs::tests::commit_writes_bytes_and_increments_sn`,
`crates/flatdata/src/allocator.rs::tests::commit_too_large_returns_error`,
`crates/flatdata/src/slot.rs::tests::new_header_has_zero_mask`,
`crates/flatdata/src/posix.rs::tests::next_sn_increments_atomically`

**Status:** done

### §4.1 Backend signalisiert den Same-Host-Reader

**Spec:** §4.1 (3)/(4) — „Backend signalisiert Same-Host-Reader via
eventfd oder POSIX-Semaphore. Reader poll'd Same-Host-Channel (eventfd)
oder UDP-Notify."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::notify_generation`,
`SlotBackend::wait_for_change`), `crates/flatdata/src/allocator.rs`
(`ChangeNotify` — Condvar plus Generationszähler, prozessintern; Lock und
`notify_all` nur, wenn ein Thread wartet),
`crates/flatdata/src/posix.rs` (`wait_for_change` parkt unter Linux per
Futex, unter macOS ab 14.4 per `os_sync_wait_on_address` auf dem
Generations-Wort im Segment-Header; anderswo ist die Funktion ein No-op.
Ein Wartezähler an 0x14 im Segment-Header zählt die geparkten Threads
aller Prozesse; geweckt wird nur, wenn er nicht 0 ist),
`crates/flatdata/src/pubsub.rs` (`FlatReader::read_blocking`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::read_blocking_wakes_on_commit`,
`crates/flatdata/src/pubsub.rs::tests::read_blocking_times_out_without_writer`,
`crates/flatdata/src/posix.rs::tests::futex_notify_wakes_consumer_across_mappings`
(Linux auf dem Linux-Testhost, macOS 26),
`crates/flatdata/src/posix.rs::tests::every_bump_reaches_a_parked_peer_and_the_count_returns_to_zero`
(2 000 Übergaben über 4 Slots, kein Park bis zum Timeout; Linux-Testhost),
`crates/flatdata/src/posix.rs::tests::a_segment_without_the_waiter_count_is_refused`

**Status:** partial — ereignisgesteuert für das In-Memory-Backend
(Condvar) und das POSIX-Backend unter Linux und macOS. Auf den BSDs und
Windows pollt der Reader weiter; siehe „§4.1 Cross-Process-Notify auf
nicht-Linux-POSIX".

### §4.1 Reader liest ohne Kopie und gibt den Slot frei

**Spec:** §4.1 (5) — „Reader liest Slot via `mmap`-`&[u8]` ohne Copy.
Setzt sein Bit in `reader_mask`. Wenn alle Reader gelesen → Slot wird
wieder reservierbar."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::slot_read_ptr`,
`SlotBackend::next_unread_slot`), `crates/flatdata/src/allocator.rs`
(`mark_read` setzt das Reader-Bit; `reserve_slot` gibt einen Slot erst
wieder aus, wenn `SlotHeader::all_read` für die aktive Reader-Maske
gilt), `crates/zerodds-c-api/src/shm_loan_ffi.rs`
(`zerodds_dr_take_shm` / `zerodds_dr_release_shm` als Zero-Copy-Konsument
derselben Primitive).

**Tests:** `crates/flatdata/src/allocator.rs::tests::slot_recyclable_after_all_readers_marked`,
`crates/flatdata/src/pubsub.rs::tests::reader_recycles_slot_after_read`,
`crates/flatdata/src/posix.rs::tests::mark_read_visible_to_owner`,
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`

**Status:** done

### §4.2 Cross-Host-Reader bekommen unveränderte UDP-DATA

**Spec:** §4.2 — „Reader auf anderem Host bekommen unveraenderte
UDP-DATA-Submessage mit voller Payload. Writer schickt also **parallel**
… Die Discovery-Logik teilt die `matched_readers` in zwei Listen."

**Repo:** `crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send`
routet nach Ziel-Locator: nur ein Locator, den ein Same-Host-Teilnehmer mit
Kanal angekündigt hat, geht durch dessen Ring; `RoutedUserTransport` vor
`user_unicast`), `crates/dcps/src/runtime.rs` (`send_metatraffic`). Die
Aufteilung je Reader ergibt sich aus seinen Locators: ein Reader auf
einem anderen Host hat keinen Kanal, sein Datagramm geht unverändert über
UDP.

**Tests:** `crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not` (auf jedem Host),
`crates/dcps/tests/same_host_e2e.rs::e2e_cross_vendor_different_host_id_no_shm_bind`
(Linux, Linux-Testhost)

**Status:** done

### §4.3 Fremde Vendoren bekommen immer UDP-DATA

**Spec:** §4.3 — „Cyclone/Fast-DDS-Reader (egal Same-Host) bekommen
**immer** UDP-DATA. Sie kennen `PID_SHM_LOCATOR` nicht (Vendor-PID,
MUST_UNDERSTAND-Bit nicht gesetzt → silently ignoriert)."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SHM_LOCATOR = 0x8001`
liegt im Vendor-Bereich; `validate_must_understand_in_data_pipeline`
lässt unbekannte Vendor-PIDs ohne MUST_UNDERSTAND-Bit passieren),
`crates/rtps/src/publication_data.rs` (`from_pl_cdr_le` überspringt
unbekannte PIDs).

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::announce_with_shm_locator_puts_vendor_pid_on_the_wire`,
`crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`,
`crates/rtps/src/parameter_list.rs::tests::validate_must_understand_in_data_pipeline_unknown_pid_rejects`

**Status:** done

## §5 Lifetime und Refcount

### §5.1 reader_mask als 32-Bit-Bitmap

**Spec:** §5.1 — „`reader_mask` ist 32-bit Bitmap. Slot ist ‚frei' wenn:
alle Bits gesetzt = alle Reader haben gelesen".

**Repo:** `crates/flatdata/src/slot.rs` (`ReaderMask` als `u32`,
`SlotHeader::mark_read`, `SlotHeader::all_read` gegen die aktive
Reader-Maske).

**Tests:** `crates/flatdata/src/slot.rs::tests::mark_read_sets_bit`,
`crates/flatdata/src/slot.rs::tests::all_read_with_two_active_readers`,
`crates/flatdata/src/slot.rs::tests::inactive_reader_bits_dont_block`,
`crates/flatdata/src/allocator.rs::tests::slot_recyclable_after_all_readers_marked`

**Status:** done

### §5.1 Straggler-Timeout — commit_time + 60 s

**Spec:** §5.1 — „Slot ist ‚frei' wenn … `commit_time + 60s`
ueberschritten (Timeout — straggler-Reader)."

**Repo:** `crates/flatdata/src/pubsub.rs` (`STRAGGLER_TIMEOUT` = 60 s,
`FlatWriter::with_straggler_timeout`, `FlatWriter::straggler_timeout`;
`FlatWriter::reserve` ruft bei erschöpftem Pool zuerst `evict_stale`),
`crates/flatdata/src/allocator.rs` (`InMemorySlotAllocator::evict_stale`
gibt committete, nicht vollständig gelesene, nicht geliehene Slots frei,
deren `committed_at` älter als `max_age` ist).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::straggler_timeout_defaults_to_sixty_seconds`,
`crates/flatdata/src/pubsub.rs::tests::write_reclaims_a_slot_held_past_the_straggler_timeout`,
`crates/flatdata/src/pubsub.rs::tests::loan_slot_reclaims_a_slot_held_past_the_straggler_timeout`,
`crates/flatdata/src/allocator.rs::tests::evict_stale_frees_slot_held_by_hung_reader`,
`crates/flatdata/src/allocator.rs::tests::evict_stale_leaves_read_slots_untouched`

**Status:** done

### §5.1 Voller Cache — Reliable blockt, BestEffort dropped

**Spec:** §5.1 — „Wenn Cache voll und kein Slot frei: Writer-`write`
blockt (Reliable) oder dropped (BestEffort) — analog zu
RESOURCE_LIMITS."

**Repo:** `crates/flatdata/src/pubsub.rs` (`Reliability`, `WriteOutcome`,
`FlatWriter::write_bp`: `Reliable` wartet ereignisgesteuert auf dem
`ChangeNotify` bis zur Deadline, `BestEffort` liefert sofort
`WriteOutcome::Dropped`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::write_bp_best_effort_drops_when_full`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_blocks_until_reader_frees`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_times_out`

**Status:** done

### §5.2 Reader-Disconnect setzt sein Bit retroaktiv

**Spec:** §5.2 — „Reader, der disconnected wird (SPDP-lease-expiry), wird
aus dem Bitmap-Allocator entfernt; Writer setzt sein Bit retroaktiv →
Slot ist frei."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::mark_reader_disconnected` setzt das Bit des
Readers auf allen committeten Slots und weckt blockierte Writer).

**Tests:** `crates/flatdata/src/allocator.rs::tests::reader_disconnect_frees_blocked_slots`

**Status:** done

## §6 Schema-Versioning

### §6.1 Größenprüfung beim Slot-Read

**Spec:** §6.1 — „Beim ersten SHM-Slot-Read prüft Reader, dass die ersten
4 byte des Slot-Header (sample_size) zu `WIRE_SIZE` passen, sonst
Slot-Drop."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatReader::scan_best`
überspringt jeden Slot, dessen Nutzbytes kürzer als `T::WIRE_SIZE`
sind, und dereferenziert ihn nicht).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_write_then_reader_read`,
`crates/flatdata/src/pubsub.rs::tests::reader_does_not_re_read_same_slot`,
`crates/flatdata/src/allocator.rs::tests::commit_too_large_returns_error`

**Status:** done

### §6.1 Type-Hash-Drift — kein Match, Fallback auf UDP-DATA

**Spec:** §6.1 — „Falls Type-Hash unterschiedlich: Reader matcht nicht —
fallback auf UDP-DATA."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::type_hash`),
`crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::with_type_hash`),
`crates/flatdata/src/pubsub.rs` (`FlatReader::scan_best` bricht vor jedem
Slot-Zugriff ab, wenn der Backend-Hash nicht `T::TYPE_HASH` ist),
`crates/dcps/src/flatdata_integration.rs` (`read_flat` liefert bei Drift
einen Fehler statt eines Samples; `write_flat` ohne Backend geht auf den
UDP-Pfad).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::reader_rejects_type_hash_mismatch`,
`crates/flatdata/src/pubsub.rs::tests::reader_accepts_matching_type_hash`,
`crates/dcps/tests/flatdata_integration.rs::datareader_read_flat_rejects_type_hash_mismatch`,
`crates/dcps/tests/flatdata_integration.rs::datawriter_write_flat_falls_back_to_udp_without_backend`

**Status:** done

## §7 Sicherheit

### §7.1 SHM-Segment ist owner-only (mode 0600)

**Spec:** §7.1 — „SHM-Segment ist `mode=0600` (owner-read-write only).
Cross-User-Access wird OS-seitig verweigert."

**Repo:** `crates/flatdata/src/posix.rs` (`PosixSlotAllocator::create`:
`shared_memory` legt das shm-Objekt mit 0600 an (`shm_open` mit
`S_IRUSR | S_IWUSR`); die flink-Datei setzt `create` unter `#[cfg(unix)]`
auf 0600, unter Linux zusätzlich das `/dev/shm`-Objekt).

**Tests:** `crates/flatdata/src/posix.rs::tests::flink_file_is_owner_only_0600`,
`crates/flatdata/src/posix.rs::tests::segment_is_owner_only_0600`
(Linux, Linux-Testhost), `crates/flatdata/src/posix.rs::tests::shm_object_is_owner_only_0600_by_descriptor` (liest den Modus über einen Deskriptor
des Objekts; macOS 26)

**Status:** done

### §7.2 Slot-Index außerhalb von [0, slot_count) wird verworfen

**Spec:** §7.2 — „`slot_count` ist im Discovery-Sample. Reader, der einen
Slot-Index ausserhalb von `[0, slot_count)` sieht, droppt."

**Repo:** `crates/flatdata/src/allocator.rs` — `read_slot`, `mark_read`,
`commit_slot`, `commit_in_place`, `discard_slot`, `slot_data_ptr` und
`slot_read_ptr` geben für einen Index außerhalb des Pools
`SlotError::OutOfBounds` zurück, bevor irgendein Slot dereferenziert
wird.

**Tests:** `crates/flatdata/src/allocator.rs::tests::slot_index_out_of_bounds_is_rejected`

**Status:** done

## §8 API-Surface (DataWriter)

### §8.1 write_flat — reserve, write und commit in einem Aufruf

**Spec:** §8 — „`pub fn write_flat(&self, sample: &T) -> Result<()>`;
Spec §4.1: reserve + write + commit in einem Call."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write`),
`crates/dcps/src/flatdata_integration.rs` (`write_flat` auf dem
DCPS-`DataWriter` mit dem Bound `T: DdsType + FlatStruct`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_write_then_reader_read`,
`crates/dcps/tests/flatdata_integration.rs::datawriter_write_flat_uses_configured_backend`,
`crates/dcps/tests/flatdata_integration.rs::flat_writer_ext_writes_to_backend`,
`crates/dcps/tests/flatdata_integration.rs::datawriter_set_flat_backend_none_disables_shm_path`

**Status:** done

### §8.2 loan_slot und FlatSlot

**Spec:** §8 — „`pub fn loan_slot(&self) -> Result<FlatSlot<'_, T>>`";
`FlatSlot` trägt `slot`, `handle` und `writer` und bietet `write(&mut
self, sample: T)` sowie `commit(self) -> Result<()>`.

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::loan_slot`,
`FlatSlot` mit `commit`, `as_mut`, `commit_in_place`; `Drop` verwirft
eine nicht committete Leihgabe).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_loan_commit_pattern`,
`crates/flatdata/src/pubsub.rs::tests::writer_loan_in_place_zero_copy`,
`crates/flatdata/src/pubsub.rs::tests::loan_drop_without_commit_releases_slot`,
`crates/flatdata/src/allocator.rs::tests::commit_in_place_too_large_returns_error`,
`crates/flatdata/src/allocator.rs::tests::discard_frees_slot_for_reuse`

**Status:** done

## §9 API-Surface (DataReader)

### §9.1 read_flat liefert eine Slot-Referenz ohne Kopie

**Spec:** §9 — „`pub fn read_flat(&self) -> Result<Option<FlatSampleRef<'_,
T>>>`. Liefert eine Slot-Reference die ohne Copy lesbar ist."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatReader::read_ref` liefert
`FlatSampleRef` mit gehaltenem Slot; `FlatReader::read` ist die
Copy-out-Variante), `crates/flatdata/src/backend.rs`
(`SlotBackend::slot_read_ptr`, `SlotBackend::next_unread_slot` als
Zero-Copy-Primitive), `crates/dcps/src/flatdata_integration.rs`
(`read_flat` auf dem DCPS-`DataReader`),
`crates/zerodds-c-api/src/shm_loan_ffi.rs` (`zerodds_dr_enable_shm`,
`zerodds_dr_take_shm`, `zerodds_dr_release_shm`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::read_ref_holds_slot_until_drop`,
`crates/dcps/tests/flatdata_integration.rs::datareader_read_flat_uses_configured_backend`,
`crates/dcps/tests/flatdata_integration.rs::datareader_read_flat_returns_none_without_backend`,
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`,
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_runtime_path_writer_to_reader_zero_copy`

**Status:** done

### §9.2 FlatSampleRef implementiert Deref

**Spec:** §9 — „`impl<T: FlatStruct> Deref for FlatSampleRef<'_, T> { type
Target = T; fn deref(&self) -> &T { self.slot } }`".

**Repo:** `crates/flatdata/src/pubsub.rs` (`impl Deref for FlatSampleRef`
mit `Target = T`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::flat_sample_ref_deref`

**Status:** done

### §9.3 FlatSampleRef::drop setzt das Reader-Bit

**Spec:** §9 — „`impl<T: FlatStruct> Drop for FlatSampleRef<'_, T>`: Drop
ruft `release_slot`, setzt das Reader-Bit im reader_mask."

**Repo:** `crates/flatdata/src/pubsub.rs` (`DeferredRelease` hält Backend,
`SlotHandle` und Reader-Index; `impl Drop for FlatSampleRef` ruft
`mark_read`; `into_inner` gibt den Slot ebenfalls frei).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::read_ref_holds_slot_until_drop`,
`crates/flatdata/src/pubsub.rs::tests::read_ref_into_inner_releases_slot`

**Status:** done

## §10 Test-Strategie

### §10.1 Unit — Slot-Allocator reserve/commit/release

**Spec:** §10 — „**Unit:** PosixShmTransport-Slot-Allocator,
reserve_slot/commit/release."

**Repo:** `crates/flatdata/src/allocator.rs` (`InMemorySlotAllocator`),
`crates/flatdata/src/posix.rs` (`PosixSlotAllocator`).

**Tests:** `crates/flatdata/src/allocator.rs::tests::reserve_returns_first_free_slot`,
`crates/flatdata/src/allocator.rs::tests::commit_writes_bytes_and_increments_sn`,
`crates/flatdata/src/allocator.rs::tests::discard_frees_slot_for_reuse`,
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`,
`crates/flatdata/src/posix.rs::tests::mark_read_visible_to_owner`

**Status:** done

### §10.2 Integration — Same-Host-Pub/Sub mit FlatStruct

**Spec:** §10 — „**Integration:** Same-Host-Pub/Sub mit FlatStruct,
Latency-Bench."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter`/`FlatReader` über
demselben Backend), `crates/flatdata/benches/loopback.rs`
(criterion-Bench `flat_write_read_1kb`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_write_then_reader_read`,
`crates/flatdata/src/pubsub.rs::tests::reader_recycles_slot_after_read`,
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`,
`crates/flatdata/tests/perf_targets.rs::same_host_p99_latency_meets_target`

**Status:** done

### §10.3 Cross-Host-Fallback — beide Reader bekommen das Sample

**Spec:** §10 — „**Cross-Host-Fallback:** Mixed-Domain (Same-Host +
Cross-Host Reader); beide bekommen Sample."

**Repo:** `crates/dcps/tests/same_host_e2e.rs` (zwei `DcpsRuntime` in
einem Prozess, einmal mit gleichem und einmal mit fremdem Host-Anteil im
GUID-Prefix), `crates/dcps/src/same_host_channel.rs`
(`SameHostChannels::send`).

**Tests:** `crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not` (auf jedem Host),
`crates/dcps/tests/same_host_e2e.rs::e2e_two_runtimes_shm_roundtrip`,
`crates/dcps/tests/same_host_e2e.rs::e2e_cross_vendor_different_host_id_no_shm_bind`
(beide Linux, Linux-Testhost)

**Status:** done

### §10.4 Cyclone-Compat — fremder Reader ignoriert den Vendor-PID

**Spec:** §10 — „**Cyclone-Compat:** Cyclone-Reader ignoriert
PID_SHM_LOCATOR und bekommt UDP-DATA wie immer."

**Repo:** `crates/rtps/src/publication_data.rs` (`from_pl_cdr_le`
überspringt unbekannte PIDs — derselbe Algorithmus, den ein fremder
Vendor auf denselben Bytes fährt), `crates/dcps/src/runtime.rs`
(`set_shm_locator` kündigt die Publikation neu an, damit der PID das
Wire erreicht), `crates/discovery/tests/cyclone_live_sedp.rs` und
`crates/dcps/tests/cyclone_live_shm_locator.rs` (Live-Läufe gegen eine
echte Cyclone-Instanz).

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::announce_with_shm_locator_puts_vendor_pid_on_the_wire`,
`crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/dcps/src/runtime.rs::tests::a_shm_locator_set_after_registration_is_announced`,
`crates/discovery/tests/cyclone_live_sedp.rs::cyclone_live_sedp_discovery`
(gegen ein lokales Cyclone `ddsperf`, im Job `live-interop`; Linux-Testhost 3/3),
`crates/dcps/tests/cyclone_live_shm_locator.rs::a_cyclone_reader_takes_the_samples_of_a_writer_announcing_the_shm_locator`
(Cyclone-11.0.1-Reader über die Python-Bindung, im Job `live-interop`;
Linux-Testhost 8/8 mit 50 von 50 Samples; im Mitschnitt trägt die zweite
DATA(w) der Publikation den PID 0x8001)

**Status:** done

### §10.5 Backpressure — Reliable blockt, BestEffort dropped

**Spec:** §10 — „**Backpressure:** Cache full + matched Reader langsam;
Writer blockt korrekt (Reliable) bzw. dropped (BestEffort)."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write_bp` mit
`Reliability` und `WriteOutcome`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::write_bp_best_effort_drops_when_full`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_blocks_until_reader_frees`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_times_out`

**Status:** done

## §11 Performance-Targets

### §11.1 Same-Host P99 unter 5 µs für ein 1-kB-Sample

**Spec:** §11 — „**Same-Host P99 < 5 µs** fuer 1 kB Sample (gegen
UDP-Loopback ~30 µs)."

**Repo:** `crates/flatdata/src/pubsub.rs` (write/read-Pfad),
`crates/flatdata/benches/loopback.rs` (criterion-Bench
`flat_write_read_1kb`), `crates/flatdata/tests/perf_targets.rs`
(Messung mit Assertion statt reinem Bench-Report).

**Tests:** `crates/flatdata/tests/perf_targets.rs::same_host_p99_latency_meets_target`
(`InMemorySlotAllocator`) und `::posix_same_host_p99_latency_meets_target`
(`PosixSlotAllocator`: Writer auf dem Mapping des Besitzers, Reader auf
einem zweiten Mapping desselben Segments, wie im SHM-Loan-Pfad der C-API).
Release-Profil auf dem Linux-Testhost (16 Kerne, Lastmittel 60–73), Budget 5 µs:
In-Memory P99 = 0,32–0,45 µs, POSIX P99 = 0,23–0,25 µs.

**Status:** done

### §11.2 Durchsatz um 1 GB/s pro Writer

**Spec:** §11 — „**Throughput:** ~1 GB/s pro Writer (Memcpy-bound)."

**Repo:** `crates/flatdata/benches/loopback.rs`
(`Throughput::Bytes`/`Throughput::Elements` für 1-kB-Samples),
`crates/flatdata/tests/perf_targets.rs`.

**Tests:** `crates/flatdata/tests/perf_targets.rs::write_throughput_meets_target`
— gemessen in 200 Blöcken zu je 100 Runden, gegated wird der schnellste
Block (Last verlängert einen Block nur). Release-Profil auf dem Linux-Testhost
(16 Kerne, Lastmittel 60–73), Untergrenze 1000 MB/s: In-Memory
3 181–4 783 MB/s, POSIX (`::posix_write_throughput_meets_target`)
10 001–10 282 MB/s, Median-Block 9 455–9 762 MB/s.

**Status:** done

### §11.3 Keine Heap-Allokation pro write

**Spec:** §11 — „**0 Heap-Allokation pro write** (Slot-Reuse)."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write` arbeitet
ausschliesslich auf dem vorab angelegten Slot-Pool).

**Tests:** `crates/flatdata/tests/zero_alloc.rs::write_flat_is_heap_allocation_free`
(zählender globaler Allocator, nur der messende Thread, 1000 Writes),
`crates/flatdata/tests/zero_alloc.rs::the_counter_counts_the_measuring_thread`

**Status:** done

### §11.4 Same-Host-Publish unter einem Fünftel der UDP-Publish-Latenz

**Spec:** §11 — „**Same-Host-Pub-Latenz < UDP-Pub-Latenz / 5**."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write`),
`crates/flatdata/tests/perf_targets.rs` (absolute Schranke),
`crates/flatdata/benches/loopback.rs` (Gruppe `flat_vs_udp_publish_1kb`
— die laufende Gegenüberstellung Slot-Pfad gegen UDP-`send_to`).

**Schranke:** Beide Zahlen stehen in der Spec: die Motivation nennt
„~30 µs Latenz pro 1 kB Sample" für Same-Host-Pub/Sub über UDP/RTPS, §11
wiederholt sie als Referenz der Sektion („gegen UDP-Loopback ~30 µs"),
und §11.4 nennt den Faktor 5. 30 µs / 5 = 6 µs.

Die Schranke ist bewusst absolut. Die frühere Form teilte durch einen im
selben Prozess gemessenen UDP-Socket; damit misst das Gate seinen eigenen
Nenner. Fünf Läufe desselben Binaries auf einer Dev-Maschine (aarch64
macOS, Release, 2 000 Runden) lieferten UDP-Mediane von 3,375 / 3,541 /
4,458 / 5,125 / 5,791 µs bei konstant 83 ns Publish-Median; der
CI-Container maß 1,704 µs für dasselbe `send_to`, und das Verhältnis-Gate
wurde rot, ohne dass sich der Publish-Pfad geändert hatte.

**Tests:** `crates/flatdata/tests/perf_targets.rs::same_host_publish_beats_a_fifth_of_udp_publish`
— 2 000 ungemessene Warmlauf-Runden, dann 20 000 gemessene; gegated wird
der Median. Gemessen auf einer Dev-Maschine (aarch64 macOS), je fünf
Läufe: Release p50 = 83 ns (5/5 identisch), p99 = 84–208 ns; Debug
p50 = 292 ns (5/5 identisch), p99 = 416 ns–1,042 µs. Budget 6 µs.

**Bench:** `cargo bench -p zerodds-flatdata --bench loopback` misst
`flat_publish` gegen `udp_publish`; die CI-Jobs `bench-main` und
`bench-compare` rufen genau diese Bench-Datei auf und vergleichen gegen
die Baseline desselben Hosts. Auf der Dev-Maschine (criterion, 1 s
Warmlauf, 3 s Messzeit): 52,076 ns gegen 4,8372 µs, Verhältnis 93.

Release-Profil auf dem Linux-Testhost (16 Kerne, Lastmittel 60–73): In-Memory
p50 = 90–110 ns, POSIX (`::posix_same_host_publish_beats_a_fifth_of_udp_publish`)
p50 = 100 ns.

**Status:** done

## §12 Lieferumfang

### §12 Lieferumfang

**Spec:** §12 — Aufzählung der acht Liefergegenstände (FlatStruct-Trait
plus derive, Slot-API, Writer-API, Reader-API, PID_SHM_LOCATOR,
Same-Host-Match, Wire-Pfad-Trennung, Test-Suite mit Bench und Doku).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — die Sektion fasst zusammen, was §1 bis §11
normieren, und stellt keine eigene Anforderung. Der Stand pro
Liefergegenstand steht in den Items dieser Sektionen.

## Offene Punkte als eigene Items

### §4.1 Cross-Process-Notify auf nicht-Linux-POSIX

**Spec:** §4.1 (3) — „Backend signalisiert Same-Host-Reader via eventfd
oder POSIX-Semaphore." Die Anforderung nennt kein Betriebssystem.

**Repo:** `crates/flatdata/src/posix.rs` (`os_sync`: macOS ab 14.4,
`os_sync_wait_on_address` mit `OS_SYNC_WAIT_ON_ADDRESS_SHARED`, zur
Laufzeit über `dlsym` aufgelöst).

**Tests:** `crates/flatdata/src/posix.rs::tests::futex_notify_wakes_consumer_across_mappings` (macOS 26)

**Status:** partial — macOS ist gebaut und getestet; die BSDs haben kein
Build-Ziel in der CI, ihr Reader pollt weiter.

### §4.2 Wire-Level-Nachweis des Reader-Splits ohne Linux-Gate

**Spec:** §4.2 — „Die Discovery-Logik teilt die `matched_readers` in zwei
Listen"; der Cross-Host-Reader erhält die unveränderte UDP-DATA.

**Repo:** `crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send`).

**Tests:** `crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not` (zwei `SameHostChannels` über echte Ringe, ohne
Netz: ein Frame an einen Peer-Locator kommt im Ring an, einer an einen
anderen Locator geht nicht in den Kanal),
`crates/dcps/src/same_host_channel.rs::tests::a_frame_the_ring_refuses_is_counted_and_left_to_udp`

**Status:** done

### §7.1 Owner-only-Modus des shm-Objekts auf nicht-Linux-Unix

**Spec:** §7.1 — „SHM-Segment ist `mode=0600` (owner-read-write only)."

**Repo:** `shared_memory` (`shm_open` mit `S_IRUSR | S_IWUSR`).

**Tests:** `crates/flatdata/src/posix.rs::tests::shm_object_is_owner_only_0600_by_descriptor` (macOS 26: `fstat` auf einem Deskriptor des
Objekts liefert 0600)

**Status:** done

### §10.4 Cyclone-Live-Lauf ohne Lab-Gate

**Spec:** §10 — „**Cyclone-Compat:** Cyclone-Reader ignoriert
PID_SHM_LOCATOR und bekommt UDP-DATA wie immer."

**Repo:** `crates/discovery/tests/cyclone_live_sedp.rs` (startet ein
lokales `ddsperf` statt per SSH, Adresse per UDP-Connect-Probe, teilt den
SPDP-Port mit `SO_REUSEADDR`), `ci/full.yml` (Job `live-interop`).

**Tests:** `crates/discovery/tests/cyclone_live_sedp.rs::cyclone_live_sedp_discovery` (Linux-Testhost 3/3: Cyclone erkannt, 28–34 Unicast-Antworten,
3 SEDP-Publikationen)

**Status:** done

## Entscheidungen

Entwurfsentscheidungen zur Spec. Sie sind keine Coverage-Items; jede
nennt die Wahl, den Grund und die Folge.

**D-1 — Eigener POSIX-SHM-Pfad statt iceoryx2-Dependency.** Der
Default-Build nutzt `crates/flatdata/src/posix.rs`, kein iceoryx2-Crate.
Grund: iceoryx2 ist noch in Stabilisierung, der Workspace bleibt
pure-Rust. Folge: Lock-free-Ringpuffer und Multi-Reader-Bitmap sind
selbst implementiert.

**D-2 — Iceoryx2-Bridge als optionales Feature.** `--features
iceoryx2-bridge` liefert `Iceoryx2Publisher`/`Iceoryx2Subscriber` für
Aufrufer im Iceoryx-Ökosystem. Grund: Opt-in statt Zwang. Folge: die
Bridge sitzt nicht hinter `SlotBackend`, weil iceoryx2 ein FIFO-Modell
statt eines Random-Access-Slot-Pools hat.

**D-3 — Same-Host-Erkennung über hostname-hash und uid.** Grund:
container-freundlich, trennt Tenants. Folge: Aufrufer mit gemeinsamer uid
profitieren, fremde uid fällt auf UDP zurück.

**D-4 — Vendor-PID ohne MUST_UNDERSTAND-Bit.** Grund: fremde Vendoren
sollen den PID ignorieren, nicht das Sample ablehnen. Folge:
Mixed-Domain-Discovery bleibt unbeeinträchtigt.

**D-5 — Pro Reader entweder SHM oder UDP, bei gemischten Peers
parallel.** Grund: maximaler Durchsatz same-host ohne Cross-Host-Bruch.
Folge: der Sendepfad führt zwei Ziel-Listen.

**D-6 — `FlatStruct` ist `Copy + 'static`.** Grund: der Byte-Cast ist nur
mit festem Layout und ohne Drop-Glue definiert. Folge: Strings und
Variable-Length-Typen laufen über den klassischen DDS-Pfad.

**D-7 — Cross-Host-Zero-Copy außerhalb von v1.0.** RDMA und DPDK sind
eine eigene Spec. Grund: Hardware-Abhängigkeit, kleiner Aufrufer-Kreis.
Folge: v1.0 deckt nur den Same-Host-Fall.

**D-8 — In-Memory- und POSIX-mmap-Backend hinter derselben API.** Grund:
die API festschreiben, bevor mmap-Komplexität dazukommt; Tests laufen
ohne mmap. Folge: `SlotBackend` abstrahiert beide,
`FlatWriter`/`FlatReader` sind backend-agnostisch.

**D-9 — flatdata als eigener Crate.** `zerodds-dcps` hängt nicht am
unsafe-Trait `FlatStruct`. Folge: der Zero-Copy-Pfad hat eigene
Pub/Sub-Typen; die DCPS-Anbindung liegt in
`crates/dcps/src/flatdata_integration.rs` hinter dem Bound `T: DdsType +
FlatStruct`.

---

## Audit-Status

43 done / 2 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-flatdata -p zerodds-flatdata-derive --release` — 87 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-dcps --features flatdata-integration --test flatdata_integration` — 11 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-dcps --features same-host-shm --lib same_host` — 8 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-rtps --lib publication_data::` — 25 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-rtps --lib parameter_list::tests::validate_must_understand` — 4 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-discovery --lib sedp::writer` — 5 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-c-api --features flatdata-loan --test shm_loan_e2e` — 5 Tests grün, 0 failed.

Nicht ausgeführt: die `#[cfg(target_os = "linux")]`-Tests
(`futex_notify_wakes_consumer_across_mappings`,
`segment_is_owner_only_0600`, `crates/dcps/tests/same_host_e2e.rs`) und
der `#[ignore]`-Test `cyclone_live_sedp_discovery`. Audit-Host: macOS.

Offene Punkte und Decision-Records: `zerodds-flatdata-1.0.open.md`.
