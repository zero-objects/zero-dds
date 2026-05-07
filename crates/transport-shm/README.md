# zerodds-transport-shm

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-shm)](https://docs.rs/zerodds-transport-shm)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-shm)](https://crates.io/crates/zerodds-transport-shm)

ZeroDDS-SHM-Transport: Cross-Process Shared-Memory-Transport.
Layer 2 (Wire-Implementation).

`std`-only, Safety-Klasse **STANDARD** (Unsafe-Island im `posix`-Modul
für mmap-Zugriff + libc-flock-FFI; Rest der Crate ist Atomics-only).

## Spec-Status

OMG normiert keinen SHM-Transport für DDS. Vendoren haben jeweils
eigene Implementationen (Cyclone+iceoryx, FastDDS-SHM, RTI-DDS-SHM).
ZeroDDS definiert seine eigene Variante explizit als
**ZeroDDS-SHM-Transport 1.0**, dokumentiert in
[`docs/spec-coverage/zerodds-shm-transport-1.0.md`](../../docs/spec-coverage/zerodds-shm-transport-1.0.md).

DDSI-RTPS-Konformität: Locator-Kind ist DDSI-RTPS 2.5 §9.4-vendor-
reservierter Wert (in `crates/rtps/src/wire_types.rs`).

## Was liefert dieses Crate

- `PosixShmTransport` — `Transport`-Trait-Impl via POSIX `shm_open` + `mmap`
- `ShmConfig` — Segment-Konfiguration (capacity, flink_dir, …)
- `ShmRole` — Owner / Consumer
- `PosixShmError` — typisierte Fehler

## Architektur-Überblick

| Aspekt | Wahl | Rationale |
|---|---|---|
| Sync-Modell | SpSc pro (Owner, Consumer)-Paar | Lock-free, lineare Skalierung mit Reader-Count |
| Atomics | `AcqRel` auf `head`/`tail` | Cross-Process-wohldefiniert |
| Crash-Recovery | predictable `os_id` + `shm_unlink` vor Owner-Create | Idempotent, verhindert Zombie-Segments |
| Race-Protection | advisory `flock(LOCK_EX)` (Linux/macOS) | Serialisiert parallele Owner-Creates |
| Owner-Termination | `shutdown`-Flag im Header (Release-Store in Drop) | Klares Owner-Gone-Signal an Consumer |

Volle Details: [Spec §2-§5](../../docs/spec-coverage/zerodds-shm-transport-1.0.md).

## Plattform-Support

| Plattform | Status |
|---|---|
| Linux | ✅ primary (Test-Coverage) |
| macOS | ✅ supported (PSHMNAMLEN-Limit) |
| Windows | ⚠️ best-effort (kompiliert via `shared_memory`-Crate; `flock`/`shm_unlink` no-op auf Non-Unix) |
| no_std | nicht supported (mmap braucht OS) |

## Tests

```bash
cargo test -p zerodds-transport-shm
```

18 Tests grün (17 lib + 1 cross-process integration).

## Lizenz

Apache-2.0 OR MIT — siehe Workspace-Root.
