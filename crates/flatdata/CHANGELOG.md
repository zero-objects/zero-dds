# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-flatdata`-Crate.

### Spec-Referenzen

- **`docs/specs/zerodds-flatdata-1.0.md`** §1 (FlatStruct + derive), §2 (Slot-Layout), §3 (Discovery PID_SHM_LOCATOR), §4 (Wire-Pfad Same-Host + Cross-Host-Fallback + Mixed-Vendor-Compat), §5 (Lifetime + Refcount), §6 (Schema-Versioning + Type-Hash-Cross-Validation), §7 (Sicherheit POSIX-Permissions + Bounded-Slot-Allocation), §8 (Writer-API), §9 (Reader-API).
- **ADR-0003** — Drei-Backend-Architektur (in-memory + POSIX shm + Iceoryx2).

### Public-API

**FlatStruct + Slot:**
- `unsafe trait FlatStruct: Copy + 'static + Send + Sync` mit `WIRE_SIZE` + `TYPE_HASH` + `as_bytes` + `from_bytes_unchecked`.
- `SlotHeader` (16 byte: `sequence_number`, `sample_size`, `reader_mask`, `_reserved`).
- `SLOT_HEADER_SIZE`, `ReaderMask`, `align_up`.

**Backend:**
- `SlotBackend`-Trait + Methods (`reserve_slot`, `commit_slot`, `discard_slot`, `read_slot`, `mark_read`, `mark_reader_disconnected`, `slot_count`, `slot_total_size`, `slot_capacity`, `type_hash`).
- `SlotHandle`, `SlotError`.

**Allocators:**
- `InMemorySlotAllocator` mit `with_type_hash`-Builder.
- `PosixSlotAllocator` + `PosixSlotError` (Feature `posix-mmap`).
- `Iceoryx2Publisher<T>` / `Iceoryx2Subscriber<T>` / `Iceoryx2Error` (Feature `iceoryx2-bridge`).

**Pub/Sub:**
- `FlatWriter<T>` mit `write`, `loan_slot`. `FlatSlot<T>` mit `commit`/Drop-discard.
- `FlatReader<T>` mit `read` (Spec §9.1 + §6.1 Type-Hash-Validation), `type_hash`. `FlatSampleRef<T>` (Wrapper mit Deref).

**Locator-Helper:**
- `ShmLocator`, `LocatorError`, `is_same_host`, `fnv1a_32`.

### Implementierung

`InMemorySlotAllocator` ist die Referenz-Implementation: `Mutex<Vec<Slot>>` mit `loaned`-Flag pro Slot, `AtomicU32` als Sequence-Counter, optionalem `type_hash`. `reserve_slot` sucht den ersten freien (alle aktiven Reader haben `reader_mask`-Bit gesetzt) oder unbenutzten Slot. `commit_slot` schreibt Bytes + setzt SlotHeader; `mark_read` setzt das Reader-Bit per CAS-equivalenter Logik (Mutex-protected Header).

`PosixSlotAllocator` (Feature `posix-mmap`) baut ein POSIX-SHM-Segment mit Layout: `[Magic+slot_count+slot_total_size+next_sn]` + `slot_count * SLOT_HEADER_SIZE`-Slots aufgereiht. Owner-Process erzeugt + entfernt; Consumer-Processes attachen. Atomare `next_sn`-Vergabe + atomic `reader_mask`-CAS. Slot-`loaned`-Status liegt im Owner-Process-RAM (Mutex), nicht im SHM — die Loan-API ist dadurch Owner-zentrisch (Reader-Processes lesen nur committet Samples).

`FlatReader::read` validiert Spec §6.1 Type-Hash gegen `T::TYPE_HASH`: bei Backend-Hash-Mismatch wird kein Slot dereferenziert (Schema-Drift-Schutz). Linear-Scan ueber alle Slots; das neueste Sample mit hoechster Sequence-Number, das noch nicht in `last_sn` ist, wird zurueckgegeben. Reader-Bit wird auch bei "skipped"-Slots gesetzt (Slot-Recycling-Eligibility).

Die `iceoryx2-bridge` ist eine **separate Pub/Sub-API** gegen
[Eclipse iceoryx2 v0.8](https://github.com/eclipse-iceoryx/iceoryx2),
nicht eine `SlotBackend`-Implementation: iceoryx2's FIFO-Pub/Sub-
Modell mit interner Refcount-Verwaltung passt nicht zur
Random-Access-Slot-Pool-Form von `SlotBackend`. Stattdessen
exponiert die Bridge `Iceoryx2Publisher::send(&sample)` (mappt auf
`publisher.loan_slice_uninit` + `write_from_slice` + `send`) und
`Iceoryx2Subscriber::receive() -> Option<T>` (mappt auf
`subscriber.receive` + Slice-Length-Check). Die Spec-§6.1
Type-Hash-Cross-Validation greift via Service-Name-Komposition
(`<base>#<hex(TYPE_HASH)>`): Pub und Sub mit unterschiedlichem `T`
landen in verschiedenen iceoryx2-Services und matchen nicht.

`forbid(unsafe_code)` ist NICHT gesetzt: `unsafe trait FlatStruct` mit per-Block-`SAFETY`-Kommentaren um die `as_bytes`/`from_bytes_unchecked`/Pointer-Casts.

### Architektur

- **Layer:** 4 (Core Services), aber Layer-0-aehnlich (keine ZeroDDS-Crate-Deps).
- **Dependencies (in):** keine ZeroDDS-Crates. Externe: `shared_memory` (optional, Feature `posix-mmap`).
- **Dependents (out):** `zerodds-dcps` (Feature `flatdata-integration`); end-user-Builds direkt.
- **Feature-Flags:** `std` (default), `alloc` (via std), `posix-mmap` (default), `iceoryx2-bridge`.

### Stabilitaet

Alle `pub`-Items sind RC1-stabil. `unsafe trait FlatStruct` ist API-stabil; Implementer muessen die Layout-Garantien per `unsafe impl` zusichern. Breaking-Changes erfordern Major-Bump auf `2.0.0`.
