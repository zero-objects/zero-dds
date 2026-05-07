# `zerodds-flatdata`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-flatdata/badge.svg)](https://docs.rs/zerodds-flatdata)

Zero-Copy Same-Host-Pub/Sub fuer den [ZeroDDS](https://zerodds.org)-Stack: `FlatStruct`-Trait, Slot-Layout per Spec, drei produktive `SlotBackend`-Implementationen (in-memory + POSIX shm/mmap + Iceoryx2-Bridge). Safety classification: **STANDARD** (mit per-Block-`SAFETY`-Kommentaren um die `unsafe trait FlatStruct`-Garantien herum lokalisiert).

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| ZeroDDS-flatdata 1.0 | §1 (FlatStruct + derive), §2 (Slot-Layout), §4 (Wire-Pfad), §5 (Lifetime + Refcount), §6 (Schema-Versioning), §7 (Sicherheit), §8/§9 (Writer-/Reader-API) |
| ADR-0003 | Drei-Backend-Architektur (in-memory / POSIX shm / Iceoryx2) |

## Was ist drin

- **`FlatStruct`-Trait** — `unsafe trait` mit `Copy + 'static + Send + Sync` und `TYPE_HASH`-Konstante. Garantiert Layout-Stabilitaet, sodass `as_bytes()` und `from_bytes_unchecked()` per Layout safe sind.
- **`SlotHeader` + Slot-Layout** — 16-Byte-Header (`sequence_number`, `sample_size`, `reader_mask`, reserved) + Payload (Spec §2).
- **`SlotBackend`-Trait** — abstrahiert das Backend; Reader/Writer sind generisch.
- **`InMemorySlotAllocator`** — Default-Backend fuer Tests + Single-Process-Pub/Sub.
- **`PosixSlotAllocator`** (Feature `posix-mmap`, default an) — `shm_open`+`mmap`, Cross-Process Same-Host. Owner/Consumer-Modell mit flink-Datei.
- **`Iceoryx2Publisher<T>` / `Iceoryx2Subscriber<T>`** (Feature `iceoryx2-bridge`) — separate Pub/Sub-API gegen [Eclipse iceoryx2 v0.8](https://github.com/eclipse-iceoryx/iceoryx2). Mappt auf `loan_slice_uninit` + `send` / `receive`. Type-Hash-Cross-Validation per Service-Name-Komposition.
- **`FlatWriter<T>`** + **`FlatSlot<T>`** — Spec §8.1/§8.2: `write` + `loan_slot` + `commit`/Drop-discard.
- **`FlatReader<T>`** + **`FlatSampleRef<T>`** — Spec §9.1: `read` mit Type-Hash-Cross-Validation (Spec §6.1) + `last_sn`-Re-Read-Suppression.
- **`ShmLocator`** + `is_same_host` + FNV-1a-Helper — Discovery-Vendor-PID-Hilfen.

## Schichten-Position

Layer 4 — Core Services. Keine ZeroDDS-Crate-Deps; einzige externe Dep ist `shared_memory` (optional, hinter `posix-mmap`-Feature).

## Quickstart

```rust,ignore
use std::sync::Arc;
use zerodds_flatdata::{FlatStruct, FlatWriter, FlatReader, InMemorySlotAllocator};

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
struct Pose { x: f64, y: f64, z: f64 }

unsafe impl FlatStruct for Pose {
    const TYPE_HASH: [u8; 16] = [0xAA; 16];
}

let alloc = Arc::new(InMemorySlotAllocator::new(0, 4, Pose::WIRE_SIZE));
let writer = FlatWriter::<Pose>::new(Arc::clone(&alloc), 0b1);
let reader = FlatReader::<Pose>::new(Arc::clone(&alloc), 0);

writer.write(&Pose { x: 1.0, y: 2.0, z: 3.0 }).unwrap();
let got = reader.read().unwrap().unwrap();
assert_eq!(got, Pose { x: 1.0, y: 2.0, z: 3.0 });
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library + Mutex + Threads. |
| `alloc` | ✅ (via std) | `Vec`/`Arc`. |
| `posix-mmap` | ✅ | `PosixSlotAllocator` (POSIX `shm_open` + `mmap`). Hangt an `shared_memory`. |
| `iceoryx2-bridge` | ❌ | Iceoryx2-Adapter-Skelett (Bridge zu iceoryx2-Service). |

## Stabilitaet

Alle `pub`-Items sind ab `1.0.0` stabil. Die `unsafe trait FlatStruct` ist API-stabil; Implementer müssen die Layout-Garantien per `unsafe impl` zusichern.

## Tests

```bash
cargo test -p zerodds-flatdata
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/specs/zerodds-flatdata-1.0.md`](../../docs/specs/zerodds-flatdata-1.0.md)
- [`zerodds-dcps`](../dcps) — DCPS-Integration (Feature `flatdata-integration`)
- [`zerodds-flatdata-derive`](../flatdata-derive) — `#[derive(FlatStruct)]` Proc-Macro
