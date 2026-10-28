# zerodds-flatdata 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-flatdata-1.0.md` (ZeroDDS vendor spec,
zero-copy same-host path).

Implementation:

- `crates/flatdata/` — FlatStruct trait, slot layout, SlotBackend,
  in-memory allocator, POSIX shm/mmap backend, Iceoryx2 bridge (opt-in).
- `crates/flatdata-derive/` — `#[derive(FlatStruct)]`.
- `crates/rtps/` + `crates/discovery/` — vendor PID 0x8001 and its SEDP
  embedding.
- `crates/dcps/` — same-host binding, wire-path routing, DCPS integration.

Open items and decision records: `zerodds-flatdata-1.0.open.en.md`.

## Goals and non-goals

### Goal — pure-Rust default build without an iceoryx dependency

**Spec:** `docs/specs/zerodds-flatdata-1.0.md`, section "Goals" —
"**Pure Rust** — no C plugin, no Iceoryx cargo dep in the default build."

**Repo:** `crates/flatdata/Cargo.toml` (`default = ["std", "posix-mmap"]`;
`iceoryx2` declared `optional = true` and reachable only through the
`iceoryx2-bridge` feature).

**Tests:** `crates/flatdata/tests/default_build.rs::default_features_do_not_pull_in_iceoryx2`,
`crates/flatdata/tests/default_build.rs::posix_mmap_backend_is_on_by_default`

**Status:** done

### Non-goal — iceoryx wire compatibility only as an opt-in feature

**Spec:** `docs/specs/zerodds-flatdata-1.0.md`, section "Non-goals" —
"Iceoryx wire compat — only via the optional feature `--features
iceoryx2-bridge` (not part of v1.0)."

**Repo:** `crates/flatdata/Cargo.toml` (`iceoryx2-bridge = ["std",
"dep:iceoryx2"]`), `crates/flatdata/src/iceoryx.rs`
(`Iceoryx2Publisher`, `Iceoryx2Subscriber`, behind `#[cfg(feature =
"iceoryx2-bridge")]`).

**Tests:** `crates/flatdata/tests/default_build.rs::iceoryx_bridge_is_an_opt_in_feature`

**Status:** done

### Non-goal — cross-host zero-copy (RDMA, DPDK)

**Spec:** `docs/specs/zerodds-flatdata-1.0.md`, section "Non-goals" —
"Cross-host zero-copy (RDMA, DPDK) — separate spec."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — the spec itself puts the point outside its
scope; no requirement on the tree follows. The corresponding design
decision is D-7 below.

## §1 FlatStruct type model

### §1.1 FlatStruct trait

**Spec:** §1.1 — "`pub unsafe trait FlatStruct: Copy + 'static + Send +
Sync`" with `WIRE_SIZE = core::mem::size_of::<Self>()`, `TYPE_HASH: [u8;
16]`, `as_bytes(&self) -> &[u8]` and `unsafe fn
from_bytes_unchecked(bytes: &[u8]) -> Self`.

**Repo:** `crates/flatdata/src/lib.rs` (`FlatStruct` with the four
associated items; the default body of `as_bytes` is a byte cast over
`WIRE_SIZE`).

**Tests:** `crates/flatdata/src/lib.rs::tests::wire_size_matches_size_of`,
`crates/flatdata/src/lib.rs::tests::as_bytes_roundtrip`,
`crates/flatdata/src/lib.rs::tests::type_hash_is_consistent`

**Status:** done

### §1.2 The derive macro emits the trait implementation

**Spec:** §1.2 — "`#[derive(FlatStruct)]` generates `unsafe impl
FlatStruct for T`"; "The `TYPE_HASH` is
`sha256(<TypeName>{<field-name>:<field-ty>,...})[..16]`."

**Repo:** `crates/flatdata-derive/src/lib.rs` (`derive_flat_struct`,
`expand`, `layout_signature`, `type_signature`; SHA-256 over the layout
signature, first 16 bytes as `TYPE_HASH`).

**Tests:** `crates/flatdata/tests/derive.rs::derive_generates_wire_size`,
`crates/flatdata/tests/derive.rs::derive_generates_type_hash_nonzero`,
`crates/flatdata/tests/derive.rs::derive_as_bytes_roundtrip`,
`crates/flatdata/tests/derive.rs::derive_works_for_tuple_struct`,
`crates/flatdata/tests/derive.rs::derive_works_for_unit_struct`,
`crates/flatdata/tests/derive.rs::derive_hash_is_deterministic_for_identical_layout`,
`crates/flatdata-derive/src/lib.rs::tests::layout_signature_covers_name_order_and_types`

**Status:** done

### §1.2 The derive macro refuses layout-unsafe types

**Spec:** §1.2 — "The macro rejects with `compile_error!` if `T` carries neither
`#[repr(C)]` nor `#[repr(transparent)]`, and on `enum`/`union`."

**Repo:** `crates/flatdata-derive/src/lib.rs` (`expand` returns a
`syn::Error` for `Data::Enum` and `Data::Union`;
`has_repr_c_or_transparent` inspects the `repr` attribute).

**Tests:** `crates/flatdata-derive/src/lib.rs::tests::derive_rejects_enum`,
`crates/flatdata-derive/src/lib.rs::tests::derive_rejects_union`,
`crates/flatdata-derive/src/lib.rs::tests::derive_rejects_struct_without_repr_c`,
`crates/flatdata-derive/src/lib.rs::tests::repr_attribute_gate_accepts_c_and_transparent_only`,
`crates/flatdata/tests/derive.rs::derive_accepts_repr_transparent`

**Status:** done

### §1.2 The type hash detects schema drift

**Spec:** §1.2 — the hash input is `<TypeName>{<field-name>:<field-ty>,...}`,
so a rename, a field reorder and a field type change all reach the hash.

**Repo:** `crates/flatdata-derive/src/lib.rs` (`layout_signature` writes
type name, field names and field types into the hash input).

**Tests:** `crates/flatdata/tests/derive.rs::derive_detects_field_reorder`,
`crates/flatdata/tests/derive.rs::derive_detects_field_type_change`,
`crates/flatdata/tests/derive.rs::derive_detects_type_rename`,
`crates/flatdata/tests/derive.rs::derive_generates_distinct_hash_per_layout`

**Status:** done

## §2 SHM slot layout

### §2 Slot header — 16 bytes, four u32 fields

**Spec:** §2 — slot header with `0x00 u32 sequence_number`, `0x04 u32
sample_size`, `0x08 u32 reader_mask`, `0x0c u32 _reserved`;
"Header size: 16 byte."

**Repo:** `crates/flatdata/src/slot.rs` (`SlotHeader`,
`SLOT_HEADER_SIZE`, `to_le_bytes`/`from_bytes` in exactly that field
order).

**Tests:** `crates/flatdata/src/slot.rs::tests::header_size_is_16`,
`crates/flatdata/src/slot.rs::tests::roundtrip_le`,
`crates/flatdata/src/slot.rs::tests::from_bytes_too_short_returns_none`,
`crates/flatdata/src/slot.rs::tests::new_header_has_zero_mask`

**Status:** done

### §2 Slot size — 16 + sample_size, padded to 64 bytes

**Spec:** §2 — "Slot size = 16 + sample_size, padded to
the next 64-byte boundary (cache line alignment)."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::slot_total_size` rounds `SLOT_HEADER_SIZE +
slot_capacity` up to the next multiple of 64),
`crates/flatdata/src/posix.rs` (`PosixSlotAllocator` lays the segment out
with the same `slot_total_size`).

**Tests:** `crates/flatdata/src/allocator.rs::tests::slot_total_size_is_cache_line_padded`,
`crates/flatdata/src/posix.rs::tests::create_attach_roundtrip`

**Status:** done

## §3 Discovery — PID_SHM_LOCATOR

### §3.1 Wire format of vendor PID 0x8001

**Spec:** §3.1 — vendor PID `0x8001 PID_SHM_LOCATOR` with `u32
hostname-hash (FNV-1a of the hostname bytes)`, `u32 uid`, `u32 slot_count`,
`u32 slot_size`, `segment_path: CDR-String`.

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SHM_LOCATOR =
0x8001`), `crates/flatdata/src/locator.rs` (`ShmLocator` with
`to_bytes`/`from_bytes` in exactly that field order, `fnv1a_32`).

**Tests:** `crates/flatdata/src/locator.rs::tests::roundtrip_le`,
`crates/flatdata/src/locator.rs::tests::truncated_header_errors`,
`crates/flatdata/src/locator.rs::tests::path_too_long_errors`,
`crates/flatdata/src/locator.rs::tests::fnv1a_known_value`

**Status:** done

### §3.1 PID_SHM_LOCATOR in the SEDP publication sample

**Spec:** §3.1 together with §3.2 — the reader takes the locator "from
PID_SHM_LOCATOR", so the writer has to carry it in its SEDP publication
announcement.

**Repo:** `crates/rtps/src/publication_data.rs`
(`inject_pid_shm_locator` appends the PID before PID_SENTINEL),
`crates/discovery/src/sedp/writer.rs`
(`SedpPublicationsWriter::announce_with_shm_locator`),
`crates/discovery/src/sedp/stack.rs`
(`SedpStack::announce_publication_with_shm_locator`),
`crates/dcps/src/runtime.rs` (`DcpsRuntime::set_shm_locator`,
`shm_locator`, `clear_shm_locator` as a side map keyed on the user
writer's `EntityId`).

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::announce_with_shm_locator_puts_vendor_pid_on_the_wire`,
`crates/rtps/src/publication_data.rs::tests::inject_pid_shm_locator_appends_before_sentinel`,
`crates/rtps/src/publication_data.rs::tests::inject_pid_shm_locator_rejects_missing_sentinel`,
`crates/rtps/src/publication_data.rs::tests::inject_pid_shm_locator_rejects_too_short`

**Status:** done

### §3.2 Same-host match on hostname hash and uid

**Spec:** §3.2 — "Same-host match if: 1. `hostname-hash` from
PID_SHM_LOCATOR == local hostname-hash, AND 2. `uid` from
PID_SHM_LOCATOR == local uid".

**Repo:** `crates/flatdata/src/locator.rs` (`is_same_host` compares both
fields against the local host; `fnv1a_32` as the hash),
`crates/dcps/src/runtime.rs` (`note_same_host_peer`: a ZeroDDS
participant SPDP reports with the same `host_id` gets an SHM channel),
`crates/dcps/src/same_host_channel.rs` (`SameHostChannels::add_peer`).

**Tests:** `crates/flatdata/src/locator.rs::tests::same_host_match_positive`,
`crates/flatdata/src/locator.rs::tests::same_host_mismatch_uid`,
`crates/flatdata/src/locator.rs::tests::same_host_mismatch_hostname`,
`crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not`

**Status:** done

### §3.2 Third match condition — mmap on the segment

**Spec:** §3.2 — "3. the local process can `mmap()` the segment." If
that fails, the match fails.

**Repo:** `crates/flatdata/src/posix.rs` (`PosixSlotAllocator::attach`
maps the segment named by the locator; failures surface as
`PosixSlotError`), `crates/dcps/src/same_host_channel.rs`
(`SameHostChannels::open`: when mapping fails the participant gets no
channel and stays on UDP; event `shm.channel_open_failed`).

**Tests:** `crates/flatdata/src/posix.rs::tests::create_attach_roundtrip`,
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`,
`crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not`
(before the peer opened its ring nothing goes through the channel)

**Status:** done

## §4 Wire path

### §4.1 The writer reserves a slot

**Spec:** §4.1 (1) — "The writer reserves a slot via
`PosixShmTransport::reserve_slot()`. Returns `SlotHandle =
(segment_id, slot_index, &mut [u8])`."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::reserve_slot`),
`crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::reserve_slot` returns a `SlotHandle` carrying
`segment_id` and `slot_index`), `crates/flatdata/src/posix.rs`
(`PosixSlotAllocator` implements the same trait over mmap).

**Tests:** `crates/flatdata/src/allocator.rs::tests::reserve_returns_first_free_slot`,
`crates/flatdata/src/allocator.rs::tests::reserve_returns_no_free_slot_when_all_loaned`,
`crates/flatdata/src/posix.rs::tests::create_attach_roundtrip`

**Status:** done

### §4.1 The writer writes straight into the slot

**Spec:** §4.1 (2) — "The writer writes the FlatStruct directly into
`slot.bytes_mut()`."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::slot_data_ptr` hands out the pointer into the
reserved slot, `commit_in_place` finishes without a staging copy),
`crates/flatdata/src/pubsub.rs` (`FlatSlot::as_mut`,
`FlatSlot::commit_in_place`).

**Tests:** `crates/flatdata/src/allocator.rs::tests::in_place_loan_writes_without_staging_copy`,
`crates/flatdata/src/allocator.rs::tests::slot_data_ptr_rejects_unreserved_slot`,
`crates/flatdata/src/pubsub.rs::tests::writer_loan_in_place_zero_copy`

**Status:** done

### §4.1 commit_slot sets sample_size and reader_mask

**Spec:** §4.1 (3) — "The writer calls `commit_slot(slot)`. The backend sets
`sample_size` and `reader_mask = 0`."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::commit_slot` writes `sample_size`, sets
`reader_mask` to 0 and issues the next `sequence_number`),
`crates/flatdata/src/posix.rs` (`PosixSlotAllocator::commit_slot`,
`next_sn` as an atomic counter in the segment header).

**Tests:** `crates/flatdata/src/allocator.rs::tests::commit_writes_bytes_and_increments_sn`,
`crates/flatdata/src/allocator.rs::tests::commit_too_large_returns_error`,
`crates/flatdata/src/slot.rs::tests::new_header_has_zero_mask`,
`crates/flatdata/src/posix.rs::tests::next_sn_increments_atomically`

**Status:** done

### §4.1 The backend signals the same-host reader

**Spec:** §4.1 (3)/(4) — "The backend signals same-host readers via
eventfd or a POSIX semaphore. The reader polls the same-host channel (eventfd)
or the UDP notify."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::notify_generation`,
`SlotBackend::wait_for_change`), `crates/flatdata/src/allocator.rs`
(`ChangeNotify` — condvar plus generation counter, in-process; the lock
and `notify_all` only when a thread waits),
`crates/flatdata/src/posix.rs` (`wait_for_change` parks on the
generation word in the segment header: a futex on Linux,
`os_sync_wait_on_address` on macOS from 14.4; elsewhere the function is a
no-op. A waiter count at 0x14 in the segment header counts the parked
threads of all processes; a bump wakes only when it is not 0),
`crates/flatdata/src/pubsub.rs` (`FlatReader::read_blocking`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::read_blocking_wakes_on_commit`,
`crates/flatdata/src/pubsub.rs::tests::read_blocking_times_out_without_writer`,
`crates/flatdata/src/posix.rs::tests::futex_notify_wakes_consumer_across_mappings`
(Linux on the Linux test host, macOS 26),
`crates/flatdata/src/posix.rs::tests::every_bump_reaches_a_parked_peer_and_the_count_returns_to_zero`
(2 000 hand-overs through 4 slots, no park runs to its timeout; the Linux test host),
`crates/flatdata/src/posix.rs::tests::a_segment_without_the_waiter_count_is_refused`

**Status:** partial — event-driven for the in-memory backend (condvar)
and the POSIX backend on Linux and macOS. On the BSDs and Windows the
reader still polls; see "§4.1 cross-process notify on non-Linux POSIX".

### §4.1 The reader reads without a copy and frees the slot

**Spec:** §4.1 (5) — "The reader reads the slot via `mmap` `&[u8]` without a copy.
Sets its bit in `reader_mask`. When all readers have read → the slot becomes
reservable again."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::slot_read_ptr`,
`SlotBackend::next_unread_slot`), `crates/flatdata/src/allocator.rs`
(`mark_read` sets the reader bit; `reserve_slot` hands a slot out again
only once `SlotHeader::all_read` holds for the active reader mask),
`crates/zerodds-c-api/src/shm_loan_ffi.rs` (`zerodds_dr_take_shm` /
`zerodds_dr_release_shm` as a zero-copy consumer of the same primitives).

**Tests:** `crates/flatdata/src/allocator.rs::tests::slot_recyclable_after_all_readers_marked`,
`crates/flatdata/src/pubsub.rs::tests::reader_recycles_slot_after_read`,
`crates/flatdata/src/posix.rs::tests::mark_read_visible_to_owner`,
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`

**Status:** done

### §4.2 Cross-host readers get unchanged UDP DATA

**Spec:** §4.2 — "Readers on another host get an unchanged UDP DATA submessage
with the full payload. So the writer sends **in parallel**
… The discovery logic splits the `matched_readers` into two lists."

**Repo:** `crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send`
routes by destination locator: only a locator a same-host participant with
a channel announced goes through its ring; `RoutedUserTransport` in front
of `user_unicast`), `crates/dcps/src/runtime.rs` (`send_metatraffic`). The
split per reader follows from its locators: a reader on another host has
no channel, and its datagram goes over UDP unchanged.

**Tests:** `crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not` (on any host),
`crates/dcps/tests/same_host_e2e.rs::e2e_cross_vendor_different_host_id_no_shm_bind`
(Linux, Linux test host)

**Status:** done

### §4.3 Foreign vendors always get UDP DATA

**Spec:** §4.3 — "Cyclone/Fast-DDS readers (regardless of same host) **always** get
UDP DATA. They do not know `PID_SHM_LOCATOR` (vendor PID,
MUST_UNDERSTAND bit not set → silently ignored)."

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SHM_LOCATOR = 0x8001`
sits in the vendor range; `validate_must_understand_in_data_pipeline`
lets unknown vendor PIDs without the MUST_UNDERSTAND bit through),
`crates/rtps/src/publication_data.rs` (`from_pl_cdr_le` skips unknown
PIDs).

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::announce_with_shm_locator_puts_vendor_pid_on_the_wire`,
`crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/rtps/src/parameter_list.rs::tests::a_vendor_specific_must_understand_pid_of_another_vendor_is_an_error`,
`crates/rtps/src/parameter_list.rs::tests::validate_must_understand_in_data_pipeline_unknown_pid_rejects`

**Status:** done

## §5 Lifetime and refcount

### §5.1 reader_mask as a 32-bit bitmap

**Spec:** §5.1 — "`reader_mask` is a 32-bit bitmap. A slot is "free" if:
all bits are set = all readers have read".

**Repo:** `crates/flatdata/src/slot.rs` (`ReaderMask` as `u32`,
`SlotHeader::mark_read`, `SlotHeader::all_read` against the active reader
mask).

**Tests:** `crates/flatdata/src/slot.rs::tests::mark_read_sets_bit`,
`crates/flatdata/src/slot.rs::tests::all_read_with_two_active_readers`,
`crates/flatdata/src/slot.rs::tests::inactive_reader_bits_dont_block`,
`crates/flatdata/src/allocator.rs::tests::slot_recyclable_after_all_readers_marked`

**Status:** done

### §5.1 Straggler timeout — commit_time + 60 s

**Spec:** §5.1 — "A slot is "free" if: … `commit_time + 60s`
has been exceeded (timeout — straggler readers)."

**Repo:** `crates/flatdata/src/pubsub.rs` (`STRAGGLER_TIMEOUT` = 60 s,
`FlatWriter::with_straggler_timeout`, `FlatWriter::straggler_timeout`;
`FlatWriter::reserve` calls `evict_stale` first once the pool is
exhausted), `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::evict_stale` frees committed, not fully read,
not loaned slots whose `committed_at` is older than `max_age`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::straggler_timeout_defaults_to_sixty_seconds`,
`crates/flatdata/src/pubsub.rs::tests::write_reclaims_a_slot_held_past_the_straggler_timeout`,
`crates/flatdata/src/pubsub.rs::tests::loan_slot_reclaims_a_slot_held_past_the_straggler_timeout`,
`crates/flatdata/src/allocator.rs::tests::evict_stale_frees_slot_held_by_hung_reader`,
`crates/flatdata/src/allocator.rs::tests::evict_stale_leaves_read_slots_untouched`

**Status:** done

### §5.1 Full cache — reliable blocks, best-effort drops

**Spec:** §5.1 — "If the cache is full and no slot is free: the writer `write`
blocks (Reliable) or drops (BestEffort) — analogous to
RESOURCE_LIMITS."

**Repo:** `crates/flatdata/src/pubsub.rs` (`Reliability`, `WriteOutcome`,
`FlatWriter::write_bp`: `Reliable` waits event-driven on the
`ChangeNotify` until the deadline, `BestEffort` returns
`WriteOutcome::Dropped` immediately).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::write_bp_best_effort_drops_when_full`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_blocks_until_reader_frees`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_times_out`

**Status:** done

### §5.2 A disconnected reader has its bit set retroactively

**Spec:** §5.2 — "A reader that gets disconnected (SPDP lease expiry) is removed from
the bitmap allocator; the writer sets its bit retroactively → the slot
is free."

**Repo:** `crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::mark_reader_disconnected` sets that reader's bit
on every committed slot and wakes blocked writers).

**Tests:** `crates/flatdata/src/allocator.rs::tests::reader_disconnect_frees_blocked_slots`

**Status:** done

## §6 Schema versioning

### §6.1 Size check on the slot read

**Spec:** §6.1 — "On the first SHM slot read, the reader checks that the first
4 bytes of the slot header (sample_size) match `WIRE_SIZE`, otherwise
slot drop."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatReader::scan_best` skips
every slot whose payload is shorter than `T::WIRE_SIZE` and never
dereferences it).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_write_then_reader_read`,
`crates/flatdata/src/pubsub.rs::tests::reader_does_not_re_read_same_slot`,
`crates/flatdata/src/allocator.rs::tests::commit_too_large_returns_error`

**Status:** done

### §6.1 Type-hash drift — no match, fall back to UDP DATA

**Spec:** §6.1 — "If the type hash differs: the reader does not match — fallback
to UDP DATA."

**Repo:** `crates/flatdata/src/backend.rs` (`SlotBackend::type_hash`),
`crates/flatdata/src/allocator.rs`
(`InMemorySlotAllocator::with_type_hash`),
`crates/flatdata/src/pubsub.rs` (`FlatReader::scan_best` bails out before
touching any slot when the backend hash is not `T::TYPE_HASH`),
`crates/dcps/src/flatdata_integration.rs` (`read_flat` returns an error
rather than a sample on drift; `write_flat` without a backend takes the
UDP path).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::reader_rejects_type_hash_mismatch`,
`crates/flatdata/src/pubsub.rs::tests::reader_accepts_matching_type_hash`,
`crates/dcps/tests/flatdata_integration.rs::datareader_read_flat_rejects_type_hash_mismatch`,
`crates/dcps/tests/flatdata_integration.rs::datawriter_write_flat_falls_back_to_udp_without_backend`

**Status:** done

## §7 Security

### §7.1 The SHM segment is owner-only (mode 0600)

**Spec:** §7.1 — "The SHM segment is `mode=0600` (owner-read-write only).
Cross-user access is refused by the OS."

**Repo:** `crates/flatdata/src/posix.rs` (`PosixSlotAllocator::create`:
`shared_memory` creates the shm object with 0600 (`shm_open` with
`S_IRUSR | S_IWUSR`); `create` sets the flink file to 0600 under
`#[cfg(unix)]`, and on Linux the `/dev/shm` object as well).

**Tests:** `crates/flatdata/src/posix.rs::tests::flink_file_is_owner_only_0600`,
`crates/flatdata/src/posix.rs::tests::segment_is_owner_only_0600`
(Linux, Linux test host), `crates/flatdata/src/posix.rs::tests::shm_object_is_owner_only_0600_by_descriptor` (reads the mode off a descriptor of the
object; macOS 26)

**Status:** done

### §7.2 A slot index outside [0, slot_count) is dropped

**Spec:** §7.2 — "`slot_count` is in the discovery sample. A reader that sees a slot index
outside of `[0, slot_count)` drops it."

**Repo:** `crates/flatdata/src/allocator.rs` — `read_slot`, `mark_read`,
`commit_slot`, `commit_in_place`, `discard_slot`, `slot_data_ptr` and
`slot_read_ptr` return `SlotError::OutOfBounds` for an index outside the
pool before dereferencing any slot.

**Tests:** `crates/flatdata/src/allocator.rs::tests::slot_index_out_of_bounds_is_rejected`

**Status:** done

## §8 API surface (DataWriter)

### §8.1 write_flat — reserve, write and commit in one call

**Spec:** §8 — "`pub fn write_flat(&self, sample: &T) -> Result<()>`;
Spec §4.1: reserve + write + commit in one call."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write`),
`crates/dcps/src/flatdata_integration.rs` (`write_flat` on the DCPS
`DataWriter` under the bound `T: DdsType + FlatStruct`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_write_then_reader_read`,
`crates/dcps/tests/flatdata_integration.rs::datawriter_write_flat_uses_configured_backend`,
`crates/dcps/tests/flatdata_integration.rs::flat_writer_ext_writes_to_backend`,
`crates/dcps/tests/flatdata_integration.rs::datawriter_set_flat_backend_none_disables_shm_path`

**Status:** done

### §8.2 loan_slot and FlatSlot

**Spec:** §8 — "`pub fn loan_slot(&self) -> Result<FlatSlot<'_, T>>`";
`FlatSlot` carries `slot`, `handle` and `writer` and offers `write(&mut
self, sample: T)` plus `commit(self) -> Result<()>`.

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::loan_slot`,
`FlatSlot` with `commit`, `as_mut`, `commit_in_place`; `Drop` discards an
uncommitted loan).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_loan_commit_pattern`,
`crates/flatdata/src/pubsub.rs::tests::writer_loan_in_place_zero_copy`,
`crates/flatdata/src/pubsub.rs::tests::loan_drop_without_commit_releases_slot`,
`crates/flatdata/src/allocator.rs::tests::commit_in_place_too_large_returns_error`,
`crates/flatdata/src/allocator.rs::tests::discard_frees_slot_for_reuse`

**Status:** done

## §9 API surface (DataReader)

### §9.1 read_flat returns a slot reference without a copy

**Spec:** §9 — "`pub fn read_flat(&self) -> Result<Option<FlatSampleRef<'_,
T>>>`. Returns a slot reference that can be read without a copy."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatReader::read_ref` returns
a `FlatSampleRef` holding its slot; `FlatReader::read` is the copy-out
variant), `crates/flatdata/src/backend.rs` (`SlotBackend::slot_read_ptr`,
`SlotBackend::next_unread_slot` as the zero-copy primitives),
`crates/dcps/src/flatdata_integration.rs` (`read_flat` on the DCPS
`DataReader`), `crates/zerodds-c-api/src/shm_loan_ffi.rs`
(`zerodds_dr_enable_shm`, `zerodds_dr_take_shm`,
`zerodds_dr_release_shm`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::read_ref_holds_slot_until_drop`,
`crates/dcps/tests/flatdata_integration.rs::datareader_read_flat_uses_configured_backend`,
`crates/dcps/tests/flatdata_integration.rs::datareader_read_flat_returns_none_without_backend`,
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`,
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_runtime_path_writer_to_reader_zero_copy`

**Status:** done

### §9.2 FlatSampleRef implements Deref

**Spec:** §9 — "`impl<T: FlatStruct> Deref for FlatSampleRef<'_, T> { type
Target = T; fn deref(&self) -> &T { self.slot } }`".

**Repo:** `crates/flatdata/src/pubsub.rs` (`impl Deref for FlatSampleRef`
with `Target = T`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::flat_sample_ref_deref`

**Status:** done

### §9.3 FlatSampleRef::drop sets the reader bit

**Spec:** §9 — "`impl<T: FlatStruct> Drop for FlatSampleRef<'_, T>`"; its
`drop` carries the comment "Set the reader bit in reader_mask." above
`self.reader.release_slot(self.handle)`.

**Repo:** `crates/flatdata/src/pubsub.rs` (`DeferredRelease` holds the
backend, the `SlotHandle` and the reader index; `impl Drop for
FlatSampleRef` calls `mark_read`; `into_inner` frees the slot as well).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::read_ref_holds_slot_until_drop`,
`crates/flatdata/src/pubsub.rs::tests::read_ref_into_inner_releases_slot`

**Status:** done

## §10 Test strategy

### §10.1 Unit — slot allocator reserve/commit/release

**Spec:** §10 — "**Unit:** PosixShmTransport slot allocator,
reserve_slot/commit/release."

**Repo:** `crates/flatdata/src/allocator.rs` (`InMemorySlotAllocator`),
`crates/flatdata/src/posix.rs` (`PosixSlotAllocator`).

**Tests:** `crates/flatdata/src/allocator.rs::tests::reserve_returns_first_free_slot`,
`crates/flatdata/src/allocator.rs::tests::commit_writes_bytes_and_increments_sn`,
`crates/flatdata/src/allocator.rs::tests::discard_frees_slot_for_reuse`,
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`,
`crates/flatdata/src/posix.rs::tests::mark_read_visible_to_owner`

**Status:** done

### §10.2 Integration — same-host pub/sub with a FlatStruct

**Spec:** §10 — "**Integration:** same-host pub/sub with FlatStruct,
latency bench."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter`/`FlatReader` over
one backend), `crates/flatdata/benches/loopback.rs` (criterion bench
`flat_write_read_1kb`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::writer_write_then_reader_read`,
`crates/flatdata/src/pubsub.rs::tests::reader_recycles_slot_after_read`,
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`,
`crates/flatdata/tests/perf_targets.rs::same_host_p99_latency_meets_target`

**Status:** done

### §10.3 Cross-host fallback — both readers get the sample

**Spec:** §10 — "**Cross-host fallback:** mixed domain (same-host + cross-host
readers); both get the sample."

**Repo:** `crates/dcps/tests/same_host_e2e.rs` (two `DcpsRuntime` in one
process, once with the same and once with a foreign host part in the GUID
prefix), `crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send`).

**Tests:** `crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not` (on any host),
`crates/dcps/tests/same_host_e2e.rs::e2e_two_runtimes_shm_roundtrip`,
`crates/dcps/tests/same_host_e2e.rs::e2e_cross_vendor_different_host_id_no_shm_bind`
(both Linux, Linux test host)

**Status:** done

### §10.4 Cyclone compatibility — a foreign reader ignores the vendor PID

**Spec:** §10 — "**Cyclone compat:** the Cyclone reader ignores PID_SHM_LOCATOR and
gets UDP DATA as always."

**Repo:** `crates/rtps/src/publication_data.rs` (`from_pl_cdr_le` skips
unknown PIDs — the same algorithm a foreign vendor runs over the same
bytes), `crates/dcps/src/runtime.rs` (`set_shm_locator` re-announces the
publication so the PID reaches the wire),
`crates/discovery/tests/cyclone_live_sedp.rs` and
`crates/dcps/tests/cyclone_live_shm_locator.rs` (live runs against a real
Cyclone instance).

**Tests:** `crates/discovery/src/sedp/writer.rs::tests::announce_with_shm_locator_puts_vendor_pid_on_the_wire`,
`crates/rtps/src/publication_data.rs::tests::unknown_pids_are_skipped`,
`crates/dcps/src/runtime.rs::tests::a_shm_locator_set_after_registration_is_announced`,
`crates/discovery/tests/cyclone_live_sedp.rs::cyclone_live_sedp_discovery`
(against a local Cyclone `ddsperf`, in the `live-interop` job; the Linux test host 3/3),
`crates/dcps/tests/cyclone_live_shm_locator.rs::a_cyclone_reader_takes_the_samples_of_a_writer_announcing_the_shm_locator`
(a Cyclone 11.0.1 reader through the Python binding, in the `live-interop`
job; the Linux test host 8/8 with 50 of 50 samples; in the capture the second DATA(w)
of the publication carries PID 0x8001)

**Status:** done

### §10.5 Backpressure — reliable blocks, best-effort drops

**Spec:** §10 — "**Backpressure:** cache full + matched reader slow; the writer
blocks correctly (Reliable) or drops (BestEffort)."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write_bp` with
`Reliability` and `WriteOutcome`).

**Tests:** `crates/flatdata/src/pubsub.rs::tests::write_bp_best_effort_drops_when_full`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_blocks_until_reader_frees`,
`crates/flatdata/src/pubsub.rs::tests::write_bp_reliable_times_out`

**Status:** done

## §11 Performance targets

### §11.1 Same-host P99 below 5 µs for a 1 kB sample

**Spec:** §11 — "**Same-host P99 < 5 µs** for a 1 kB sample (against
UDP loopback ~30 µs)."

**Repo:** `crates/flatdata/src/pubsub.rs` (write/read path),
`crates/flatdata/benches/loopback.rs` (criterion bench
`flat_write_read_1kb`), `crates/flatdata/tests/perf_targets.rs`
(measurement with an assertion rather than a bench report only).

**Tests:** `crates/flatdata/tests/perf_targets.rs::same_host_p99_latency_meets_target`
(`InMemorySlotAllocator`) and `::posix_same_host_p99_latency_meets_target`
(`PosixSlotAllocator`: the writer on the owner's mapping, the reader on a
second mapping of the same segment, as in the C API's SHM loan path).
Release profile on the Linux test host (16 cores, load average 60–73), budget 5 µs:
in-memory P99 = 0.32–0.45 µs, POSIX P99 = 0.23–0.25 µs.

**Status:** done

### §11.2 Throughput around 1 GB/s per writer

**Spec:** §11 — "**Throughput:** ~1 GB/s per writer (memcpy-bound)."

**Repo:** `crates/flatdata/benches/loopback.rs`
(`Throughput::Bytes`/`Throughput::Elements` for 1 kB samples),
`crates/flatdata/tests/perf_targets.rs`.

**Tests:** `crates/flatdata/tests/perf_targets.rs::write_throughput_meets_target`
— measured in 200 batches of 100 rounds, gated on the fastest batch (load
only lengthens a batch). Release profile on the Linux test host (16 cores, load
average 60–73), floor 1000 MB/s: in-memory 3 181–4 783 MB/s, POSIX
(`::posix_write_throughput_meets_target`) 10 001–10 282 MB/s, median batch
9 455–9 762 MB/s.

**Status:** done

### §11.3 No heap allocation per write

**Spec:** §11 — "**0 heap allocations per write** (slot reuse)."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write` works
purely on the pre-allocated slot pool).

**Tests:** `crates/flatdata/tests/zero_alloc.rs::write_flat_is_heap_allocation_free`
(counting global allocator, the measuring thread only, 1000 writes),
`crates/flatdata/tests/zero_alloc.rs::the_counter_counts_the_measuring_thread`

**Status:** done

### §11.4 Same-host publish below a fifth of the UDP publish latency

**Spec:** §11 — "**Same-host pub latency < UDP pub latency / 5**."

**Repo:** `crates/flatdata/src/pubsub.rs` (`FlatWriter::write`),
`crates/flatdata/tests/perf_targets.rs` (absolute bound),
`crates/flatdata/benches/loopback.rs` (group `flat_vs_udp_publish_1kb` —
the live side-by-side of slot path against UDP `send_to`).

**Bound:** Both numbers come from the spec: the Motivation states
"~30 µs latency per 1 kB sample" for same-host pub/sub over UDP/RTPS, §11
repeats it as the reference of the section ("against UDP loopback ~30 µs"),
and §11.4 states the factor of five. 30 µs / 5 = 6 µs.

The bound is absolute on purpose. The earlier form divided by a UDP
socket measured in the same process, which makes the gate measure its own
denominator. Five runs of the same binary on one dev machine (aarch64
macOS, release, 2 000 rounds) produced UDP medians of 3.375 / 3.541 /
4.458 / 5.125 / 5.791 µs at a constant 83 ns publish median; the CI
container measured 1.704 µs for the same `send_to`, and the ratio gate
went red without any change to the publish path.

**Tests:** `crates/flatdata/tests/perf_targets.rs::same_host_publish_beats_a_fifth_of_udp_publish`
— 2 000 untimed warm-up rounds, then 20 000 measured; the median is
gated. Measured on a dev machine (aarch64 macOS), five runs each:
release p50 = 83 ns (5/5 identical), p99 = 84–208 ns; debug p50 = 292 ns
(5/5 identical), p99 = 416 ns–1.042 µs. Budget 6 µs.

**Bench:** `cargo bench -p zerodds-flatdata --bench loopback` measures
`flat_publish` against `udp_publish`; the CI jobs `bench-main` and
`bench-compare` invoke exactly this bench file and compare against the
baseline of the same host. On the dev machine (criterion, 1 s warm-up,
3 s measurement time): 52.076 ns against 4.8372 µs, ratio 93.

Release profile on the Linux test host (16 cores, load average 60–73): in-memory
p50 = 90–110 ns, POSIX (`::posix_same_host_publish_beats_a_fifth_of_udp_publish`)
p50 = 100 ns.

**Status:** done

## §12 Deliverables

### §12 Deliverables

**Spec:** §12 — the list of eight deliverables (FlatStruct trait plus
derive, slot API, writer API, reader API, PID_SHM_LOCATOR, same-host
match, wire-path split, test suite with bench and docs).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — the section summarises what §1 to §11
require and states no requirement of its own. The state per deliverable
is in the items of those sections.

## Open points as their own items

### §4.1 Cross-process notify on non-Linux POSIX

**Spec:** §4.1 (3) — "The backend signals same-host readers via eventfd
or a POSIX semaphore." The requirement names no operating system.

**Repo:** `crates/flatdata/src/posix.rs` (`os_sync`: macOS from 14.4,
`os_sync_wait_on_address` with `OS_SYNC_WAIT_ON_ADDRESS_SHARED`, resolved
at run time through `dlsym`).

**Tests:** `crates/flatdata/src/posix.rs::tests::futex_notify_wakes_consumer_across_mappings` (macOS 26)

**Status:** partial — macOS is built and tested; the BSDs have no build
target in CI, and their reader still polls.

### §4.2 Wire-level proof of the reader split without a Linux gate

**Spec:** §4.2 — "The discovery logic splits the `matched_readers` into two
lists"; the cross-host reader receives the unchanged UDP DATA.

**Repo:** `crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send`).

**Tests:** `crates/dcps/src/same_host_channel.rs::tests::a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not` (two `SameHostChannels` over real rings, no
network: a frame to a peer locator arrives in the ring, one to another
locator does not go into the channel),
`crates/dcps/src/same_host_channel.rs::tests::a_frame_the_ring_refuses_is_counted_and_left_to_udp`

**Status:** done

### §7.1 Owner-only mode of the shm object on non-Linux unix

**Spec:** §7.1 — "The SHM segment is `mode=0600` (owner-read-write only)."

**Repo:** `shared_memory` (`shm_open` with `S_IRUSR | S_IWUSR`).

**Tests:** `crates/flatdata/src/posix.rs::tests::shm_object_is_owner_only_0600_by_descriptor` (macOS 26: `fstat` on a descriptor of the
object reads 0600)

**Status:** done

### §10.4 Cyclone live run without a lab gate

**Spec:** §10 — "**Cyclone compat:** the Cyclone reader ignores PID_SHM_LOCATOR and
gets UDP DATA as always."

**Repo:** `crates/discovery/tests/cyclone_live_sedp.rs` (starts a local
`ddsperf` instead of SSH, address through a UDP connect probe, shares the
SPDP port with `SO_REUSEADDR`), `ci/full.yml` (job `live-interop`).

**Tests:** `crates/discovery/tests/cyclone_live_sedp.rs::cyclone_live_sedp_discovery` (the Linux test host 3/3: Cyclone discovered, 28–34 unicast
replies, 3 SEDP publications)

**Status:** done

## Decisions

Design decisions on the spec. They are not coverage items; each names the
choice, the reason and the consequence.

**D-1 — own POSIX SHM path instead of an iceoryx2 dependency.** The
default build uses `crates/flatdata/src/posix.rs`, no iceoryx2 crate.
Reason: iceoryx2 is still stabilising, and the workspace stays pure Rust.
Consequence: the lock-free ring buffer and the multi-reader bitmap are
implemented here.

**D-2 — iceoryx2 bridge as an optional feature.** `--features
iceoryx2-bridge` provides `Iceoryx2Publisher`/`Iceoryx2Subscriber` for
callers in the iceoryx ecosystem. Reason: opt-in rather than mandate.
Consequence: the bridge does not sit behind `SlotBackend`, because
iceoryx2 has a FIFO model rather than a random-access slot pool.

**D-3 — same-host detection on hostname hash and uid.** Reason:
container-friendly, separates tenants. Consequence: callers sharing a uid
benefit, a foreign uid falls back to UDP.

**D-4 — vendor PID without the MUST_UNDERSTAND bit.** Reason: foreign
vendors should ignore the PID rather than reject the sample.
Consequence: mixed-domain discovery is unaffected.

**D-5 — per reader either SHM or UDP, in parallel for mixed peers.**
Reason: maximum same-host throughput without breaking cross-host.
Consequence: the send path carries two target lists.

**D-6 — `FlatStruct` is `Copy + 'static`.** Reason: the byte cast is
defined only with a fixed layout and no drop glue. Consequence: strings
and variable-length types run over the classic DDS path.

**D-7 — cross-host zero-copy outside v1.0.** RDMA and DPDK are a spec of
their own. Reason: hardware dependency, small caller base. Consequence:
v1.0 covers the same-host case only.

**D-8 — in-memory and POSIX mmap backend behind one API.** Reason: pin
the API down before mmap complexity arrives; tests run without mmap.
Consequence: `SlotBackend` abstracts both, `FlatWriter`/`FlatReader` are
backend-agnostic.

**D-9 — flatdata as a crate of its own.** `zerodds-dcps` does not depend
on the unsafe trait `FlatStruct`. Consequence: the zero-copy path has its
own pub/sub types; the DCPS binding lives in
`crates/dcps/src/flatdata_integration.rs` behind the bound `T: DdsType +
FlatStruct`.

---

## Audit status

43 done / 2 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-flatdata -p zerodds-flatdata-derive --release` — 87 tests green, 0 failed.

Test run: `cargo test -p zerodds-dcps --features flatdata-integration --test flatdata_integration` — 11 tests green, 0 failed.

Test run: `cargo test -p zerodds-dcps --features same-host-shm --lib same_host` — 8 tests green, 0 failed.

Test run: `cargo test -p zerodds-rtps --lib publication_data::` — 25 tests green, 0 failed.

Test run: `cargo test -p zerodds-rtps --lib parameter_list::tests::validate_must_understand` — 4 tests green, 0 failed.

Test run: `cargo test -p zerodds-discovery --lib sedp::writer` — 5 tests green, 0 failed.

Test run: `cargo test -p zerodds-c-api --features flatdata-loan --test shm_loan_e2e` — 5 tests green, 0 failed.

Not executed: the `#[cfg(target_os = "linux")]` tests
(`futex_notify_wakes_consumer_across_mappings`,
`segment_is_owner_only_0600`, `crates/dcps/tests/same_host_e2e.rs`) and
the `#[ignore]` test `cyclone_live_sedp_discovery`. Audit host: macOS.

Open points and decision records: `zerodds-flatdata-1.0.open.en.md`.
