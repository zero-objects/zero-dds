# `zerodds-flatdata`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-flatdata/badge.svg)](https://docs.rs/zerodds-flatdata)

Zero-copy same-host pub/sub for the [ZeroDDS](https://zerodds.org) stack: `FlatStruct` trait, slot layout per spec, three production `SlotBackend` implementations (in-memory + POSIX shm/mmap + Iceoryx2 bridge). Safety classification: **STANDARD** (localized with per-block `SAFETY` comments around the `unsafe trait FlatStruct` guarantees).

## Spec mapping

| Spec | Section |
|------|-----------|
| ZeroDDS-flatdata 1.0 | §1 (FlatStruct + derive), §2 (slot layout), §4 (wire path), §5 (lifetime + refcount), §6 (schema versioning), §7 (security), §8/§9 (writer/reader API) |
| ADR-0003 | Three-backend architecture (in-memory / POSIX shm / Iceoryx2) |

## What's inside

- **`FlatStruct` trait** — `unsafe trait` with `Copy + 'static + Send + Sync` and a `TYPE_HASH` constant. Guarantees layout stability so that `as_bytes()` and `from_bytes_unchecked()` are safe by layout.
- **`SlotHeader` + slot layout** — 16-byte header (`sequence_number`, `sample_size`, `reader_mask`, reserved) + payload (spec §2).
- **`SlotBackend` trait** — abstracts the backend; readers/writers are generic.
- **`InMemorySlotAllocator`** — default backend for tests + single-process pub/sub.
- **`PosixSlotAllocator`** (feature `posix-mmap`, on by default) — `shm_open` + `mmap`, cross-process same-host. Owner/consumer model with a flink file.
- **`Iceoryx2Publisher<T>` / `Iceoryx2Subscriber<T>`** (feature `iceoryx2-bridge`) — separate pub/sub API against [Eclipse iceoryx2 v0.8](https://github.com/eclipse-iceoryx/iceoryx2). Maps to `loan_slice_uninit` + `send` / `receive`. Type-hash cross-validation via service-name composition.
- **`FlatWriter<T>`** + **`FlatSlot<T>`** — spec §8.1/§8.2: `write` + `loan_slot` + `commit` / Drop-discard.
- **`FlatReader<T>`** + **`FlatSampleRef<T>`** — spec §9.1: `read` with type-hash cross-validation (spec §6.1) + `last_sn` re-read suppression.
- **`ShmLocator`** + `is_same_host` + FNV-1a helper — discovery vendor-PID helpers.

## Layer position

Layer 4 — Core Services. No ZeroDDS crate deps; the only external dep is `shared_memory` (optional, behind the `posix-mmap` feature).

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

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Standard library + Mutex + threads. |
| `alloc` | ✅ (via std) | `Vec`/`Arc`. |
| `posix-mmap` | ✅ | `PosixSlotAllocator` (POSIX `shm_open` + `mmap`). Depends on `shared_memory`. |
| `iceoryx2-bridge` | ❌ | Iceoryx2 adapter skeleton (bridge to iceoryx2 service). |

## Stability

All `pub` items are stable as of `1.0.0`. The `unsafe trait FlatStruct` is API-stable; implementers must guarantee the layout properties via `unsafe impl`.

## Tests

```bash
cargo test -p zerodds-flatdata
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/specs/zerodds-flatdata-1.0.md`](../../docs/specs/zerodds-flatdata-1.0.md)
- [`zerodds-dcps`](../dcps) — DCPS integration (feature `flatdata-integration`)
- [`zerodds-flatdata-derive`](../flatdata-derive) — `#[derive(FlatStruct)]` proc-macro
