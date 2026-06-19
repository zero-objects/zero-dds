# ZeroDDS-SHM-Transport 1.0 — Spec-Coverage

ZeroDDS-vendor-spezifischer Shared-Memory-Transport. Analog zu Cyclone's
iceoryx-Integration, FastDDS-SHM und RTI-DDS-SHM. **Nicht OMG-normativ.**
Implementiert in:

- `crates/transport-shm/` — Segment-Layout + SpSc-Ringbuffer + Crash-Recovery (`posix.rs`)

Der vendor-reservierte Locator-Kind-Wert (§9.4) ist eine Konstante in
`crates/rtps/src/wire_types.rs`.

| Spec-Family | Status |
|---|---|
| **OMG-normativ** | DDSI-RTPS 2.5 §9.4 LocatorKind (vendor-reserved range) — Locator-Wert in `crates/rtps/src/wire_types.rs` |
| **ZeroDDS-eigene Spec** | Segment-Layout + SpSc-Ringbuffer + Crash-Recovery — [`zerodds-shm-transport-1.0.md`](https://github.com/zero-objects/zero-dds/blob/main/docs/spec-coverage/zerodds-shm-transport-1.0.md) |

## §1 Scope und Spec-Status

### §1.1 Was OMG normiert

DDSI-RTPS 2.5 normiert für SHM nur:

- **§9.4 LocatorKind**: vendor-reservierte Werte für nicht-IP-
  Transports. ZeroDDS-Wert in `crates/rtps/src/wire_types.rs`.

Kein normatives Wire-Format, kein normatives Segment-Layout, kein
normatives Cleanup-Protokoll.

### §1.2 Was OMG nicht normiert

- **Segment-Layout**: vendor-spezifisch.
- **Synchronisations-Modell** (SpSc / SpmC / Mutex): vendor-spezifisch.
- **Crash-Recovery**: vendor-spezifisch.
- **Cleanup-Mechanik**: vendor-spezifisch.
- **Multi-Reader-Distribution**: vendor-spezifisch.

### §1.3 ZeroDDS-Wahl

ZeroDDS-SHM-Transport definiert eine **eigene Spec** (diese Datei) für
Segment-Layout + Synchronisation. Die Spec ist:

- **OMG-konform** auf Locator-Ebene (§9.4 vendor-reservierter Wert).
- **Vendor-spezifisch** auf Wire-/Synchronisations-Ebene (analog
  iceoryx/FastDDS-SHM/RTI).

## §2 Segment-Layout

```text
  offset 0:   magic: u32 BE   "ZSHM"  (0x5A53484D)
  offset 4:   version: u32 LE
  offset 8:   capacity: u64 LE         (Daten-Region, ohne Header)
  offset 16:  head: AtomicU64          (nächster Schreib-Offset, Writer-owned)
  offset 24:  tail: AtomicU64          (nächster Lese-Offset, Reader-owned)
  offset 32:  shutdown: AtomicU32      (0=active, 1=owner-gone)
  offset 36:  reserved (padding zu 64-Byte cache-line)
  offset 64:  data-region [capacity bytes]
```

- **Magic** `"ZSHM"` — Versions-Discriminator, lehnt fremde Segmente ab.
- **Version**: aktuell `1`. Bump bei Layout-Änderung; Opener lehnen
  unbekannte Versionen ab.
- **Head/Tail**: `AcqRel`-Atomics für Lock-free Single-Producer-Single-
  Consumer-Synchronisation.
- **Shutdown**: Owner→Consumer-Termination-Signal (Owner setzt auf `1`
  in `Drop`).

Repo-Anker: `crates/transport-shm/src/posix.rs::HEADER_BYTES`,
`SHM_MAGIC`, `SHM_VERSION`.

## §3 Frame-Format

Length-Prefix-Format innerhalb der Daten-Region:

```text
+---------+---------+---------+---------+----- ...
| len: u32 LE                            | bytes [len]
+---------+---------+---------+---------+----- ...
```

- `len = 0xFFFF_FFFE` markiert ein **Padding-Frame** (Ring-End-Marker).
  Writer setzt es ein, wenn nicht genug zusammenhängender Space am
  Ring-Ende ist; springt danach an den Anfang.
- `len < capacity` ist ein Daten-Frame.

Repo-Anker: `posix.rs::PADDING_FRAME_LEN`,
`posix.rs::SegmentLayout::push_frame`,
`posix.rs::SegmentLayout::pop_frame`.

## §4 Synchronisations-Modell

### §4.1 Single-Producer-Single-Consumer

Ein Segment pro `(owner, consumer)`-Paar — nicht ein Segment pro
Owner mit Multi-Reader-Fan-out.

**Rationale**:
- Lock-free SpSc skaliert linear mit Reader-Count, keine globale
  Contention.
- `pthread_mutex` mit `PTHREAD_PROCESS_SHARED` wäre der
  Alternativweg, ist aber crash-recovery-fragile.
- SpmC (wie iceoryx) blockt den Writer am slowest-Reader — bei
  heterogenen Readern schlecht.

**Preis**: N Segmente bei N Readern. Bei 100 Readern × 1 MiB Default
= 100 MiB. Akzeptabel; Segment-Größe pro Paar konfigurierbar.

### §4.2 Memory-Ordering

- Writer: `Release`-Store auf `head` nach Frame-Write.
- Reader: `Acquire`-Load von `head` vor Frame-Read.
- Garantiert: Writer-side Frame-Bytes sind sichtbar, sobald Reader
  den neuen `head`-Wert sieht.

Repo-Anker: `posix.rs::SegmentLayout::head` /
`SegmentLayout::tail`.

## §5 Cleanup-Semantik

### §5.1 Predictable os_id

`segment_os_id(owner, consumer)` liefert einen deterministischen
Segment-Namen (`/zd-<owner>-<consumer>` bzw. `/zd-<owner-tail15>-<consumer-tail15>` auf macOS-PSHMNAMLEN).

Repo-Anker: `posix.rs::segment_os_id`.

### §5.2 Crash-Recovery

Vor jedem Owner-`create()` wird `shm_unlink(os_id)` gerufen. Dies:
- Räumt Zombie-Segmente eines crashed Owners auf.
- Ist idempotent (`ENOENT` wird ignoriert).
- Verhindert systemweiten `/dev/shm`-Leak.

Repo-Anker: `posix.rs::shm_unlink_by_os_id`.

### §5.3 Shutdown-Flag

Owner setzt `shutdown = 1` in `Drop` (Release-Store). Consumer prüft
das Flag in `wait_for_frame` nach jedem leeren Poll und kehrt mit
einem gezielten Error (`Io{message:"shm owner terminated"}`) zurück,
statt blind in den `recv_timeout` zu fallen.

Repo-Anker: `posix.rs::SegmentLayout::set_shutdown`,
`posix.rs::SegmentLayout::is_shutdown`.

### §5.4 Race-Protection beim Owner-Create

Eine exklusive Whole-File-Lock auf einer Sentinel-Datei serialisiert
parallele Owner-Creates auf dieselbe `os_id` — über Threads UND Prozesse.
Linux/macOS via `flock(LOCK_EX)`, Windows via `LockFileEx`
(`LOCKFILE_EXCLUSIVE_LOCK`, blockierend). Beide werden beim Schließen des
Handles bzw. Prozess-Tod automatisch freigegeben (gleiche Crash-Resilienz).

Repo-Anker: `posix.rs::acquire_flock_excl`,
`posix.rs::FlockGuard`.

## §6 Plattform-Support

| Plattform | Status | Anmerkungen |
|---|---|---|
| Linux | ✅ primary | Volle Test-Coverage |
| macOS | ✅ supported | PSHMNAMLEN-Limit beachtet |
| Windows | ✅ supported | Zero-Copy-SHM über `shared_memory` (`CreateFileMapping`); Owner-Create-Race via `LockFileEx`; Cleanup über OS-Handle-Reference-Counting. Test-Suite grün auf Windows (19/19, inkl. `open_concurrent_two_threads_both_bound`) |
| no_std | nicht supported | std-only (mmap braucht OS-Calls) |

## §7 Test-Coverage

| Spec-Sektion | Tests |
|---|---|
| §2 Segment-Layout | `posix.rs::tests::magic_and_layout_*` |
| §3 Frame-Format | `posix.rs::tests::push_pop_*`, `padding_*` |
| §4 SpSc-Synchronisation | `posix.rs::tests::concurrent_*` |
| §5.2 Crash-Recovery | `posix.rs::tests::recovers_zombie_segment` |
| §5.3 Shutdown-Flag | `posix.rs::tests::owner_drop_signals_consumer` |
| §5.4 Race-Protection | `posix.rs::tests::flock_*` |
| §6 Cross-Process | `tests/l1_cross_process.rs` |

Total: 19 lib + 1 integration = 20 Tests. Alle grün auf Linux, macOS und
Windows (`cargo test -p zerodds-transport-shm`).

## §8 Status

**Voll abgedeckt**. ZeroDDS-SHM-Transport ist eine vollständige,
in-sich-kohärente Spec; alle §-Sektionen sind implementiert und
getestet. Plattform-Support: Linux (primary), macOS und Windows — alle
drei mit grüner Test-Suite.
