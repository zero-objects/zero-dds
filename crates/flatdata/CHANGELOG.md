# Changelog

This format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), and versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-flatdata` crate.

### Spec references

- **`docs/specs/zerodds-flatdata-1.0.md`** §1 (FlatStruct + derive), §2 (slot layout), §3 (discovery PID_SHM_LOCATOR), §4 (wire path same-host + cross-host fallback + mixed-vendor compat), §5 (lifetime + refcount), §6 (schema versioning + type-hash cross-validation), §7 (security: POSIX permissions + bounded slot allocation), §8 (writer API), §9 (reader API).
- **ADR-0003** — three-backend architecture (in-memory + POSIX shm + Iceoryx2).

### Public API

**FlatStruct + Slot:**
- `unsafe trait FlatStruct: Copy + 'static + Send + Sync` with `WIRE_SIZE` + `TYPE_HASH` + `as_bytes` + `from_bytes_unchecked`.
- `SlotHeader` (16 byte: `sequence_number`, `sample_size`, `reader_mask`, `_reserved`).
- `SLOT_HEADER_SIZE`, `ReaderMask`, `align_up`.

**Backend:**
- `SlotBackend` trait + methods (`reserve_slot`, `commit_slot`, `discard_slot`, `read_slot`, `mark_read`, `mark_reader_disconnected`, `slot_count`, `slot_total_size`, `slot_capacity`, `type_hash`).
- `SlotHandle`, `SlotError`.

**Allocators:**
- `InMemorySlotAllocator` with `with_type_hash` builder.
- `PosixSlotAllocator` + `PosixSlotError` (feature `posix-mmap`).
- `Iceoryx2Publisher<T>` / `Iceoryx2Subscriber<T>` / `Iceoryx2Error` (feature `iceoryx2-bridge`).

**Pub/Sub:**
- `FlatWriter<T>` with `write`, `loan_slot`. `FlatSlot<T>` with `commit` / Drop-discard.
- `FlatReader<T>` with `read` (spec §9.1 + §6.1 type-hash validation), `type_hash`. `FlatSampleRef<T>` (wrapper with Deref).

**Locator helpers:**
- `ShmLocator`, `LocatorError`, `is_same_host`, `fnv1a_32`.

### Implementation

`InMemorySlotAllocator` is the reference implementation: `Mutex<Vec<Slot>>` with a `loaned` flag per slot, `AtomicU32` as the sequence counter, and an optional `type_hash`. `reserve_slot` finds the first free slot (all active readers have their `reader_mask` bit set) or an unused one. `commit_slot` writes the bytes and sets the SlotHeader; `mark_read` sets the reader bit via CAS-equivalent logic (Mutex-protected header).

`PosixSlotAllocator` (feature `posix-mmap`) builds a POSIX SHM segment with the layout: `[magic + slot_count + slot_total_size + next_sn]` + `slot_count * SLOT_HEADER_SIZE` slots laid out in sequence. The owner process creates and removes it; consumer processes attach. Atomic `next_sn` assignment + atomic `reader_mask` CAS. The slot `loaned` status lives in the owner process's RAM (Mutex), not in the SHM — which makes the loan API owner-centric (reader processes only read committed samples).

`FlatReader::read` validates the spec §6.1 type-hash against `T::TYPE_HASH`: on a backend-hash mismatch no slot is dereferenced (schema-drift protection). A linear scan over all slots returns the newest sample with the highest sequence number that is not yet in `last_sn`. The reader bit is also set on "skipped" slots (slot-recycling eligibility).

The `iceoryx2-bridge` is a **separate pub/sub API** against
[Eclipse iceoryx2 v0.8](https://github.com/eclipse-iceoryx/iceoryx2),
not a `SlotBackend` implementation: iceoryx2's FIFO pub/sub
model with internal refcount management does not fit the
random-access slot-pool form of `SlotBackend`. Instead, the bridge
exposes `Iceoryx2Publisher::send(&sample)` (maps to
`publisher.loan_slice_uninit` + `write_from_slice` + `send`) and
`Iceoryx2Subscriber::receive() -> Option<T>` (maps to
`subscriber.receive` + slice-length check). The spec §6.1
type-hash cross-validation takes effect via service-name composition
(`<base>#<hex(TYPE_HASH)>`): a pub and sub with different `T`
end up in different iceoryx2 services and do not match.

`forbid(unsafe_code)` is NOT set: `unsafe trait FlatStruct` with per-block `SAFETY` comments around the `as_bytes` / `from_bytes_unchecked` / pointer casts.

### Architecture

- **Layer:** 4 (Core Services), but Layer-0-like (no ZeroDDS crate deps).
- **Dependencies (in):** no ZeroDDS crates. External: `shared_memory` (optional, feature `posix-mmap`).
- **Dependents (out):** `zerodds-dcps` (feature `flatdata-integration`); direct in end-user builds.
- **Feature flags:** `std` (default), `alloc` (via std), `posix-mmap` (default), `iceoryx2-bridge`.

### Stability

All `pub` items are RC1-stable. `unsafe trait FlatStruct` is API-stable; implementers must guarantee the layout properties via `unsafe impl`. Breaking changes require a major bump to `2.0.0`.
