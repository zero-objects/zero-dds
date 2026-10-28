# 0003 — Flatdata backend trait (in-memory + POSIX mmap)

- **Status:** accepted
- **Date:** 2026-05-04
- **Authors:** @sandra
- **Context:** crates/flatdata, docs/specs/zerodds-flatdata-1.0.md

## Context

Zero-copy same-host pub/sub needs a shared-memory backend.
Options:

- **Own PosixShmTransport** (`crates/transport-shm` exists with
  1678 LOC, a complete POSIX mmap variant).
- **Iceoryx2 crate** (Eclipse, pure Rust from v0.5).
- **Combined**: own default + Iceoryx as opt-in.

The phase-1 implementation (`InMemorySlotAllocator`) is an in-process
heap stub for tests. It shares the **slot allocator interface**
with the later mmap backend. So that the code path can switch transparently
between the backends, we need a trait.

## Decision

**Flatdata exposes a `SlotBackend` trait over which
FlatWriter/FlatReader are generic.**

Implementations:
1. `InMemorySlotAllocator` — phase-1 default, test-friendly, no
   mmap.
2. `PosixSlotAllocator` (Phase-2-B) — POSIX `shm_open` + `mmap`,
   real cross-process zero-copy. Becomes the default from v1.0-Final.
3. `Iceoryx2SlotAdapter` (Phase-2-C, optional feature) — bridges to
   Iceoryx subscriber/publisher when the caller is in the Iceoryx ecosystem.

Trait methods: `reserve_slot`, `commit_slot`, `discard_slot`,
`read_slot`, `mark_read`, `mark_reader_disconnected`, `slot_count`,
`slot_total_size`.

## Alternatives

1. **Fixed POSIX mmap as the only backend** — excludes Iceoryx
   callers. Rejected.
2. **Iceoryx2 as the only backend** — lock-in to an external crate
   (API still unstable). Rejected.
3. **Backend trait** (chosen) — the caller chooses at build time or runtime;
   FlatWriter/Reader code stays unchanged.

## Consequences

**Positive**:
- `FlatWriter::write` and `FlatReader::read` are backend-agnostic;
  tests run against InMemory, production against POSIX/Iceoryx.
- The Iceoryx2 adapter can be added later — no API break.
- A caller can have several backends in parallel (e.g. POSIX in
  embedded, Iceoryx in the industrial cloud).

**Negative**:
- Small indirection (vtable call per reserve/commit/read) — measurable on a
  sub-µs path. Can later be reduced through monomorphization with
  static dispatch, if needed.
- The feature matrix becomes complex: 3 backends × 2 caller patterns
  (FlatWriter, DataWriter::write_flat).

**Follow-up tasks**:
- F2b: PosixSlotAllocator implementation.
- F-Iox (ADR-0004): Iceoryx2SlotAdapter.

## References

- `docs/specs/zerodds-flatdata-1.0.md` D-8
- `crates/flatdata/src/allocator.rs::InMemorySlotAllocator`
- `crates/transport-shm/src/posix.rs` (existing POSIX mmap code,
  is wrapped into the SlotBackend trait in F2b)
- ADR-0004 (Iceoryx2 optional)
