# `zerodds-flatdata` v1.0 — zero-copy same-host spec

ZeroDDS vendor spec. Status: **Draft 2026-05-04**, implemented in
`crates/transport-shm` (present) + `crates/dcps` (extension
planned).

## Motivation

Same-host pub/sub over UDP/RTPS today costs ~30 µs latency per 1 kB
sample (loopback roundtrip + encap header + 2x memcpy). The industry
standard for robotics/avionics is zero-copy via POSIX SHM. This
spec defines the path so that same-host readers see sub-µs latency
without breaking the backwards compatibility of the UDP path.

## Goals

- **Same-host latency P99 < 5 µs** for a 1 kB sample.
- **Cross-host path unchanged** — UDP/RTPS remains the default.
- **Backwards compatibility** — readers without SHM support get
  conventional UDP DATA with the full payload (the writer sends
  in parallel).
- **Pure Rust** — no C plugin, no Iceoryx cargo dep in the default
  build.

## Non-goals

- Cross-host zero-copy (RDMA, DPDK) — separate spec.
- Iceoryx wire compat — only via the optional feature
  `--features iceoryx2-bridge` (not part of v1.0).

## §1 FlatStruct type model

### §1.1 FlatStruct trait

```rust
/// Marker trait for FlatData-capable types. Guarantees:
/// - `Self: Copy` (no drop glue, plain bytes)
/// - `Self: 'static` (no lifetime reference)
/// - `#[repr(C)]` with a fixed, defined alignment
/// - `as_bytes()` and `from_bytes_unchecked()` are safe by layout
pub unsafe trait FlatStruct: Copy + 'static + Send + Sync {
    /// Wire size = `core::mem::size_of::<Self>()`.
    const WIRE_SIZE: usize = core::mem::size_of::<Self>();
    /// Type hash (XTypes-1.3 §7.6.3.2.2 + Iceoryx2-FixedSizeName).
    const TYPE_HASH: [u8; 16];
    /// Returns the slot layout as a slice.
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: Self is Copy + repr(C) → byte cast is defined.
        unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, Self::WIRE_SIZE) }
    }
    /// Reconstructs from a raw slice. The caller must have validated hash + size.
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Self;
}
```

### §1.2 Derive macro

`#[derive(FlatStruct)]` generates `unsafe impl FlatStruct for T`:

```rust
#[derive(FlatStruct, Copy, Clone)]
#[repr(C)]
struct Pose {
    x: f64,
    y: f64,
    z: f64,
    qx: f32, qy: f32, qz: f32, qw: f32,
    ts_nanos: u64,
}
// expands to:
// unsafe impl FlatStruct for Pose {
//     const TYPE_HASH: [u8; 16] = [/* sha256("zerodds::Pose:f64,f64,...")[..16] */];
//     unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Self {
//         core::ptr::read(bytes.as_ptr() as *const Pose)
//     }
// }
```

Macro crate: `crates/flatdata-derive` (`zerodds-flatdata-derive`). The macro rejects with `compile_error!` if `T` carries neither `#[repr(C)]` nor `#[repr(transparent)]`, and on `enum`/`union`. The `TYPE_HASH` is `sha256(<TypeName>{<field-name>:<field-ty>,...})[..16]`.

## §2 SHM slot layout

```text
+--------- SHM slot (slot_size = sample_size + header_size) ---------+
| 0x00 | u32  | sequence_number (writer-local)                       |
| 0x04 | u32  | sample_size (= FlatStruct::WIRE_SIZE)                |
| 0x08 | u32  | reader_mask  (bitmap: which readers have read)       |
| 0x0c | u32  | _reserved (padding)                                  |
| 0x10 | [u8] | FlatStruct data                                      |
+--------------------------------------------------------------------+
```

Header size: 16 byte. Slot size = 16 + sample_size, padded to
the next 64-byte boundary (cache line alignment).

## §3 Discovery — PID_SHM_LOCATOR

### §3.1 Wire format

Vendor PID `0x8001 PID_SHM_LOCATOR`:

```text
+----- PID value -----+
| 0x00 | u32 | hostname-hash (FNV-1a of the hostname bytes)      |
| 0x04 | u32 | uid (POSIX uid_t) — for same-user match           |
| 0x08 | u32 | slot_count                                        |
| 0x0c | u32 | slot_size                                         |
| 0x10 |     | segment_path: CDR-String (`/dev/shm/zddspub_<eid>`) |
+---------------------+
```

### §3.2 Match logic

Same-host match if:
1. `hostname-hash` from PID_SHM_LOCATOR == local hostname-hash, AND
2. `uid` from PID_SHM_LOCATOR == local uid, AND
3. the local process can `mmap()` the segment.

Cross-host = match fails → the reader uses UDP DATA as always.

## §4 Wire path

### §4.1 Same-host path

1. The writer reserves a slot via `PosixShmTransport::reserve_slot()`.
   Returns `SlotHandle = (segment_id, slot_index, &mut [u8])`.
2. The writer writes the FlatStruct directly into `slot.bytes_mut()`.
3. The writer calls `commit_slot(slot)`. The backend sets `sample_size` and
   `reader_mask = 0`. The backend signals same-host readers via
   eventfd or a POSIX semaphore.
4. The reader polls the same-host channel (eventfd) or the UDP notify.
5. The reader reads the slot via `mmap` `&[u8]` without a copy. Sets its bit
   in `reader_mask`. When all readers have read → the slot becomes
   reservable again.

### §4.2 Cross-host fallback

Readers on another host get an unchanged UDP DATA submessage
with the full payload. So the writer sends **in parallel**:
- same-host readers: SHM slot notify
- cross-host readers: UDP DATA with encap header + payload

The discovery logic splits the `matched_readers` into two lists.

### §4.3 Mixed-vendor compat

Cyclone/Fast-DDS readers (regardless of same host) **always** get UDP DATA.
They do not know `PID_SHM_LOCATOR` (vendor PID, MUST_UNDERSTAND bit
not set → silently ignored). Cross-vendor interop is preserved.

## §5 Lifetime + refcount

### §5.1 Slot refcount

`reader_mask` is a 32-bit bitmap. A slot is "free" if:
- all bits are set = all readers have read, OR
- `commit_time + 60s` has been exceeded (timeout — straggler readers).

The writer cache holds N slots, one bit index per slot. If the cache is full
and no slot is free: the writer `write` blocks (Reliable) or drops
(BestEffort) — analogous to RESOURCE_LIMITS.

### §5.2 Reader disconnect

A reader that gets disconnected (SPDP lease expiry) is removed from the
bitmap allocator; the writer sets its bit retroactively → the slot
is free.

## §6 Schema versioning

### §6.1 Type hash check

The reader reads the `type_hash` of the topic type from discovery. On the
first SHM slot read, the reader checks that the first 4 bytes of the
slot header (sample_size) match `WIRE_SIZE`, otherwise slot drop.

If the type hash differs: the reader does not match — fallback
to UDP DATA. For Cyclone/Fast-DDS: the same mechanism, they
simply see the normal UDP DATA.

## §7 Security

### §7.1 POSIX permissions

The SHM segment is `mode=0600` (owner-read-write only). Cross-user
access is refused by the OS.

### §7.2 Bounded slot allocation

`slot_count` is in the discovery sample. A reader that sees a slot index
outside of `[0, slot_count)` drops it.

## §8 API surface (DataWriter)

```rust
impl<T: FlatStruct + DdsType> DataWriter<T> {
    /// Spec §4.1: reserve + write + commit in one call.
    pub fn write_flat(&self, sample: &T) -> Result<()>;

    /// Low-level: explicit slot reservation.
    pub fn loan_slot(&self) -> Result<FlatSlot<'_, T>>;
}

pub struct FlatSlot<'a, T: FlatStruct> {
    slot: &'a mut T,
    handle: SlotHandle,
    writer: &'a DataWriter<T>,
}

impl<T: FlatStruct> FlatSlot<'_, T> {
    pub fn write(&mut self, sample: T) {
        *self.slot = sample;
    }
    pub fn commit(self) -> Result<()> {
        self.writer.commit_slot(self.handle)
    }
}
```

## §9 API surface (DataReader)

```rust
impl<T: FlatStruct + DdsType> DataReader<T> {
    /// Returns a slot reference that can be read without a copy.
    /// Lifetime bound to the reader; on `drop` the
    /// bit in reader_mask is set.
    pub fn read_flat(&self) -> Result<Option<FlatSampleRef<'_, T>>>;
}

pub struct FlatSampleRef<'a, T: FlatStruct> {
    slot: &'a T,
    handle: SlotHandle,
    reader: &'a DataReader<T>,
}

impl<T: FlatStruct> Deref for FlatSampleRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { self.slot }
}

impl<T: FlatStruct> Drop for FlatSampleRef<'_, T> {
    fn drop(&mut self) {
        // Set the reader bit in reader_mask.
        let _ = self.reader.release_slot(self.handle);
    }
}
```

## §10 Test strategy

- **Unit:** PosixShmTransport slot allocator, reserve_slot/commit/release.
- **Integration:** same-host pub/sub with FlatStruct, latency bench.
- **Cross-host fallback:** mixed domain (same-host + cross-host
  readers); both get the sample.
- **Cyclone compat:** the Cyclone reader ignores PID_SHM_LOCATOR and
  gets UDP DATA as always.
- **Backpressure:** cache full + matched reader slow; the writer
  blocks correctly (Reliable) or drops (BestEffort).

## §11 Performance targets

- **Same-host P99 < 5 µs** for a 1 kB sample (against UDP loopback ~30 µs).
- **Throughput:** ~1 GB/s per writer (memcpy-bound).
- **0 heap allocations per write** (slot reuse).
- **Same-host pub latency < UDP pub latency / 5**.

## §12 Scope of delivery

- FlatStruct trait + derive macro
- PosixShmTransport slot API (reserve_slot / commit_slot / release_slot)
- DataWriter::write_flat + loan_slot/FlatSlot
- DataReader::read_flat + FlatSampleRef
- PID_SHM_LOCATOR encode/decode + SEDP push
- Same-host match logic (hostname+uid)
- Same-host SHM vs. cross-host UDP wire path
- Test suite (unit + integration + Cyclone compat) + latency bench + documentation

The current implementation status per item is in the coverage audit
`docs/spec-coverage/zerodds-flatdata-1.0.md`.
