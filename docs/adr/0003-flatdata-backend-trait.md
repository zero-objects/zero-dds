# 0003 — Flatdata Backend-Trait (in-memory + POSIX-mmap)

- **Status:** accepted
- **Datum:** 2026-05-04
- **Autoren:** @sandra
- **Kontext:** crates/flatdata, docs/specs/zerodds-flatdata-1.0.md

## Kontext

Zero-Copy Same-Host-Pub/Sub braucht ein Shared-Memory-Backend.
Optionen:

- **Eigener PosixShmTransport** (`crates/transport-shm` existiert mit
  1678 LOC, vollwertige POSIX-mmap-Variante).
- **Iceoryx2-Crate** (Eclipse, pure-Rust ab v0.5).
- **Combined**: eigener Default + Iceoryx als opt-in.

Die Phase-1-Implementation (`InMemorySlotAllocator`) ist ein in-
Process-Heap-Stub für Tests. Er teilt das **Slot-Allocator-Interface**
mit dem späteren mmap-Backend. Damit der Code-Pfad transparent
zwischen den Backends wechseln kann, brauchen wir einen Trait.

## Entscheidung

**Flatdata exponiert einen `SlotBackend`-Trait, gegen den der
FlatWriter/FlatReader generisch ist.**

Implementations:
1. `InMemorySlotAllocator` — Phase-1-Default, Test-friendly, kein
   mmap.
2. `PosixSlotAllocator` (Phase-2-B) — POSIX-`shm_open` + `mmap`,
   echte Cross-Process-Zero-Copy. Wird zum Default ab v1.0-Final.
3. `Iceoryx2SlotAdapter` (Phase-2-C, optional Feature) — bridged auf
   Iceoryx-Subscriber/Publisher, wenn Caller im Iceoryx-Ecosystem
   ist.

Trait-Methoden: `reserve_slot`, `commit_slot`, `discard_slot`,
`read_slot`, `mark_read`, `mark_reader_disconnected`, `slot_count`,
`slot_total_size`.

## Alternativen

1. **Hartes POSIX-mmap als einziger Backend** — schließt Iceoryx-
   Caller aus. Verworfen.
2. **Iceoryx2 als einziger Backend** — Lock-in zu externer Crate
   (API noch unstable). Verworfen.
3. **Backend-Trait** (gewählt) — Caller wählt zur Build-/Runtime;
   FlatWriter/Reader-Code bleibt unverändert.

## Konsequenzen

**Positiv**:
- `FlatWriter::write` und `FlatReader::read` sind backend-agnostic;
  Tests laufen gegen InMemory, Production gegen POSIX/Iceoryx.
- Iceoryx2-Adapter ist später additive — keine API-Bruch.
- Caller kann mehrere Backends parallel haben (z.B. POSIX im
  Embedded, Iceoryx in der Industrie-Cloud).

**Negativ**:
- Kleine Indirection (vtable-call pro reserve/commit/read) — bei
  Sub-µs-Pfad messbar. Kann später durch monomorphization mit
  static-dispatch reduziert werden, wenn nötig.
- Feature-Matrix wird komplex: 3 Backends × 2 Caller-Patterns
  (FlatWriter, DataWriter::write_flat).

**Folge-Aufgaben**:
- F2b: PosixSlotAllocator-Implementation.
- F-Iox (ADR-0004): Iceoryx2SlotAdapter.

## Referenzen

- `docs/specs/zerodds-flatdata-1.0.md` D-8
- `crates/flatdata/src/allocator.rs::InMemorySlotAllocator`
- `crates/transport-shm/src/posix.rs` (existing POSIX-Mmap-Code,
  wird in F2b zum SlotBackend-Trait gewrapped)
- ADR-0004 (Iceoryx2 optional)
