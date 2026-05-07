# `zerodds-flatdata` v1.0 — Zero-Copy Same-Host-Spec

ZeroDDS Vendor-Spec. Status: **Draft 2026-05-04**, in
`crates/transport-shm` (vorhanden) + `crates/dcps` (Erweiterung
vorgesehen) implementiert.

## Motivation

Same-Host Pub/Sub kostet ueber UDP/RTPS heute ~30 µs Latenz pro 1 kB
Sample (Loopback-Roundtrip + Encap-Header + 2x Memcpy). Industry-
Standard fuer Robotics/Avionics ist Zero-Copy via POSIX-SHM. Diese
Spec definiert den Pfad, sodass Same-Host-Reader Sub-µs-Latenz
sehen, ohne die UDP-Path-Backwards-Compat zu brechen.

## Ziele

- **Same-Host-Latenz P99 < 5 µs** fuer 1 kB Sample.
- **Cross-Host-Pfad unveraendert** — UDP/RTPS bleibt Default.
- **Backwards-Compatibility** — Reader ohne SHM-Support bekommen
  konventionelle UDP-DATA mit voller Payload (Writer schickt
  parallel).
- **Pure-Rust** — kein C-Plugin, kein Iceoryx-Cargo-Dep im Default-
  Build.

## Nicht-Ziele

- Cross-Host-Zero-Copy (RDMA, DPDK) — separate Spec.
- Iceoryx-Wire-Compat — nur via Optional-Feature
  `--features iceoryx2-bridge` (nicht Teil v1.0).

## §1 FlatStruct-Type-Modell

### §1.1 FlatStruct-Trait

```rust
/// Marker-Trait fuer FlatData-faehige Types. Garantiert:
/// - `Self: Copy` (kein Drop-Glue, plain bytes)
/// - `Self: 'static` (kein Lifetime-Reference)
/// - `#[repr(C)]` mit fest definiertem Alignment
/// - `as_bytes()` und `from_bytes_unchecked()` sind safe-by-Layout
pub unsafe trait FlatStruct: Copy + 'static + Send + Sync {
    /// Wire-Size = `core::mem::size_of::<Self>()`.
    const WIRE_SIZE: usize = core::mem::size_of::<Self>();
    /// Type-Hash (XTypes-1.3 §7.6.3.2.2 + Iceoryx2-FixedSizeName).
    const TYPE_HASH: [u8; 16];
    /// Liefert das Slot-Layout als Slice.
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: Self ist Copy + repr(C) → byte-cast ist defined.
        unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, Self::WIRE_SIZE) }
    }
    /// Rekonstruiert aus rohem Slice. Caller muss Hash + Size validiert haben.
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Self;
}
```

### §1.2 Derive-Macro

`#[derive(FlatStruct)]` generiert `unsafe impl FlatStruct for T`:

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

Macro-Crate: `crates/flatdata-derive` (`zerodds-flatdata-derive`). Der Macro lehnt mit `compile_error!` ab, wenn `T` weder `#[repr(C)]` noch `#[repr(transparent)]` traegt, und auf `enum`/`union`. Der `TYPE_HASH` ist `sha256(<TypeName>{<field-name>:<field-ty>,...})[..16]`.

## §2 SHM-Slot-Layout

```text
+--------- SHM-Slot (slot_size = sample_size + header_size) ---------+
| 0x00 | u32  | sequence_number (writer-lokal)                       |
| 0x04 | u32  | sample_size (= FlatStruct::WIRE_SIZE)                |
| 0x08 | u32  | reader_mask  (Bitmap: welche Reader haben gelesen)   |
| 0x0c | u32  | _reserved (padding)                                  |
| 0x10 | [u8] | FlatStruct-Daten                                     |
+--------------------------------------------------------------------+
```

Header-Size: 16 byte. Slot-Size = 16 + sample_size, gepaddet auf
naechste 64-byte-Boundary (Cache-Line-Alignment).

## §3 Discovery — PID_SHM_LOCATOR

### §3.1 Wire-Format

Vendor-PID `0x8001 PID_SHM_LOCATOR`:

```text
+----- PID-Value -----+
| 0x00 | u32 | hostname-hash (FNV-1a der hostname-Bytes)         |
| 0x04 | u32 | uid (POSIX uid_t) — fuer Same-User-Match          |
| 0x08 | u32 | slot_count                                        |
| 0x0c | u32 | slot_size                                         |
| 0x10 |     | segment_path: CDR-String (`/dev/shm/zddspub_<eid>`) |
+---------------------+
```

### §3.2 Match-Logik

Same-Host-Match wenn:
1. `hostname-hash` aus PID_SHM_LOCATOR == lokaler hostname-hash, UND
2. `uid` aus PID_SHM_LOCATOR == lokaler uid, UND
3. lokaler Process kann `mmap()` auf das Segment.

Cross-Host = Match scheitert → Reader nutzt UDP-DATA wie immer.

## §4 Wire-Pfad

### §4.1 Same-Host-Pfad

1. Writer reserviert via `PosixShmTransport::reserve_slot()` einen
   Slot. Liefert `SlotHandle = (segment_id, slot_index, &mut [u8])`.
2. Writer schreibt FlatStruct direkt in `slot.bytes_mut()`.
3. Writer ruft `commit_slot(slot)`. Backend setzt `sample_size` und
   `reader_mask = 0`. Backend signalisiert Same-Host-Reader via
   eventfd oder POSIX-Semaphore.
4. Reader poll'd Same-Host-Channel (eventfd) oder UDP-Notify.
5. Reader liest Slot via `mmap`-`&[u8]` ohne Copy. Setzt sein Bit
   in `reader_mask`. Wenn alle Reader gelesen → Slot wird wieder
   reservierbar.

### §4.2 Cross-Host-Fallback

Reader auf anderem Host bekommen unveraenderte UDP-DATA-Submessage
mit voller Payload. Writer schickt also **parallel**:
- Same-Host-Reader: SHM-Slot-Notify
- Cross-Host-Reader: UDP-DATA mit Encap-Header + Payload

Die Discovery-Logik teilt die `matched_readers` in zwei Listen.

### §4.3 Mixed-Vendor-Compat

Cyclone/Fast-DDS-Reader (egal Same-Host) bekommen **immer** UDP-DATA.
Sie kennen `PID_SHM_LOCATOR` nicht (Vendor-PID, MUST_UNDERSTAND-Bit
nicht gesetzt → silently ignoriert). Cross-Vendor-Interop bleibt.

## §5 Lifetime + Refcount

### §5.1 Slot-Refcount

`reader_mask` ist 32-bit Bitmap. Slot ist "frei" wenn:
- alle Bits gesetzt = alle Reader haben gelesen, ODER
- `commit_time + 60s` ueberschritten (Timeout — straggler-Reader).

Writer-Cache haelt N Slots, pro Slot ein Bit-Index. Wenn Cache voll
und kein Slot frei: Writer-`write` blockt (Reliable) oder dropped
(BestEffort) — analog zu RESOURCE_LIMITS.

### §5.2 Reader-Disconnect

Reader, der disconnected wird (SPDP-lease-expiry), wird aus dem
Bitmap-Allocator entfernt; Writer setzt sein Bit retroaktiv → Slot
ist frei.

## §6 Schema-Versioning

### §6.1 Type-Hash-Check

Reader liest aus Discovery den `type_hash` des Topic-Type. Beim
ersten SHM-Slot-Read prüft Reader, dass die ersten 4 byte des
Slot-Header (sample_size) zu `WIRE_SIZE` passen, sonst Slot-Drop.

Falls Type-Hash unterschiedlich: Reader matcht nicht — fallback
auf UDP-DATA. Bei Cyclone/Fast-DDS: gleicher Mechanismus, sie
sehen einfach die normale UDP-DATA.

## §7 Sicherheit

### §7.1 POSIX-Permissions

SHM-Segment ist `mode=0600` (owner-read-write only). Cross-User-
Access wird OS-seitig verweigert.

### §7.2 Bounded-Slot-Allocation

`slot_count` ist im Discovery-Sample. Reader, der einen Slot-Index
ausserhalb von `[0, slot_count)` sieht, droppt.

## §8 API-Surface (DataWriter)

```rust
impl<T: FlatStruct + DdsType> DataWriter<T> {
    /// Spec §4.1: reserve + write + commit in einem Call.
    pub fn write_flat(&self, sample: &T) -> Result<()>;

    /// Low-level: explizite Slot-Reservierung.
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

## §9 API-Surface (DataReader)

```rust
impl<T: FlatStruct + DdsType> DataReader<T> {
    /// Liefert eine Slot-Reference die ohne Copy lesbar ist.
    /// Lifetime gebunden an den Reader; bei `drop` wird das
    /// Bit im reader_mask gesetzt.
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
        // Reader-Bit im reader_mask setzen.
        let _ = self.reader.release_slot(self.handle);
    }
}
```

## §10 Test-Strategie

- **Unit:** PosixShmTransport-Slot-Allocator, reserve_slot/commit/release.
- **Integration:** Same-Host-Pub/Sub mit FlatStruct, Latency-Bench.
- **Cross-Host-Fallback:** Mixed-Domain (Same-Host + Cross-Host
  Reader); beide bekommen Sample.
- **Cyclone-Compat:** Cyclone-Reader ignoriert PID_SHM_LOCATOR und
  bekommt UDP-DATA wie immer.
- **Backpressure:** Cache full + matched Reader langsam; Writer
  blockt korrekt (Reliable) bzw. dropped (BestEffort).

## §11 Performance-Targets

- **Same-Host P99 < 5 µs** fuer 1 kB Sample (gegen UDP-Loopback ~30 µs).
- **Throughput:** ~1 GB/s pro Writer (Memcpy-bound).
- **0 Heap-Allokation pro write** (Slot-Reuse).
- **Same-Host-Pub-Latenz < UDP-Pub-Latenz / 5**.

## §12 Roadmap

| Sprint | Inhalt |
|--------|--------|
| **F1** | FlatStruct-Trait + derive-Macro |
| **F2** | PosixShmTransport-Slot-API (reserve_slot / commit_slot / release_slot) |
| **F3** | DataWriter::write_flat + loan_slot/FlatSlot |
| **F4** | DataReader::read_flat + FlatSampleRef + Drop-Hook |
| **F5** | PID_SHM_LOCATOR encode/decode + SEDP-Push |
| **F6** | Same-Host-Match-Logik (hostname+uid) |
| **F7** | Wire-Pfad-Split: Same-Host SHM vs Cross-Host UDP parallel |
| **F8** | Test-Suite: Unit + Integration + Cyclone-Compat |
| **F9** | Latenz-Bench (criterion) |
| **F10** | Doku + Examples |

Aufwand: ~3-5 PT.
