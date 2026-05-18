# zerodds-flatdata 1.0 — Open Items

Phase-1 + Phase-2-A geliefert. Verbleibend:

## Phase-2 (offen)

### §1 (1 open)
- **§1.2 derive-Macro** — `#[derive(FlatStruct)]` mit SHA-256-Type-Hash. v1.0: Caller schreibt unsafe impl per Hand.

### §3 (1 partial)
- **§3.2 Same-Host-Match** — Match-Logik vorhanden; mmap-Anbindung in den Wire-Pfad ist Phase-2 (F7).

### §4 (3 open, 1 partial)
- **§4.2 Reader-Notify (eventfd)** — Phase-2 zusammen mit POSIX-mmap-Backend.
- **§4.3 Cross-Host-Fallback parallel** — Phase-2 (F7); braucht Reliable-Writer-Reader-List-Split.
- **§4.4 Mixed-Vendor-Compat** — partial: PID-Filter da, E2E-Live-Test mit Cyclone offen.

### §5 (1 partial)
- **§5.1 reader_mask-Bitmap** — partial: Bitmap voll, 60-s-Timeout-Eviction Phase-2.

### §6 (1 partial)
- **§6.1 Type-Hash-Check** — partial: size-Check live, TYPE_HASH-Cross-Validation gegen Discovery-Hash Phase-2.

### §7 (1 partial)
- **§7.1 POSIX-Permissions 0600** — partial: transport-shm impl da, Slot-Backend-Anbindung Phase-2.

### §9 (1 open, 1 partial)
- **§9.1 read_flat liefert FlatSampleRef** — partial: liefert `Option<T>` statt Lifetime-bound Ref; Drop-Hook-Variante kommt mit POSIX-mmap-Backend.
- **§9.3 FlatSampleRef::Drop setzt Bit** — Phase-1 setzt Bit direkt im read; Drop-basierte Variante Phase-2.

### §10 (2 open, 1 partial)
- **§10.3 Cross-Host-Fallback-Test** — Phase-2 (F7-Wire-Pfad-Split).
- **§10.4 Cyclone-Compat-Test** — Phase-2 Live-Test.
- **§10.5 Backpressure** — partial: Allocator-Side gibt NoFreeSlot, Reliable/BestEffort-Distinction Phase-2 (DataWriter-Integration).

### §11 (2 open, 1 partial)
- **§11.1 P99 < 5 µs** — partial: Bench-Skelett, numerische Bestaetigung in CI.
- **§11.2 Throughput ~1 GB/s** — Bench-Erweiterung folgt.
- **§11.3 0 Heap-Allokation** — dhat-rs-Bench offen.

## Sprint-Plan Phase-2

| Sprint | Items | Aufwand |
|--------|-------|---------|
| **F2b** | POSIX-mmap-Backend (replaces InMemorySlotAllocator) — §4.1, §4.2, §7.1 done | 1-2 PT |
| **F7**  | Wire-Pfad-Split + Reader-Notify — §4.2, §4.3 | 1-2 PT |
| **F11** | derive(FlatStruct) Macro — §1.2 | 0.5 PT |
| **F12** | Discovery-Push PID_SHM_LOCATOR im SEDP — §3.2 mmap-Anbindung | 1 PT |
| **F13** | TYPE_HASH-Cross-Validation — §6.1 | 0.5 PT |
| **F14** | Cyclone-Compat-Test — §4.4, §10.4 | 0.5 PT |
| **F15** | Bench-Erweiterung + dhat-rs — §11.1-§11.3 | 1 PT |
| **F16** | DataWriter-Integration `T: DdsType + FlatStruct` — Reliable/BestEffort-Distinction §10.5 | 1 PT |

Gesamt-Phase-2: ~5-7 PT.

## Phase-1 done (zur Referenz)

- §1.1 FlatStruct-Trait ✓
- §2.1, §2.2 SlotHeader + Alignment ✓
- §3.1, §3.3 PID_SHM_LOCATOR Wire-Format + Vendor-Bit ✓
- §4.1 reserve→write→commit (in-memory) ✓
- §5.2 Reader-Disconnect retroaktiv ✓
- §7.2 Bounded-Slot-Allocation ✓
- §8.1, §8.2 Writer-API write + loan_slot ✓
- §9.2 FlatSampleRef::Deref ✓
- §10.1, §10.2 Slot-Allocator + Same-Host-E2E (in-memory) ✓
