# `zerodds-foundation`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-foundation/badge.svg)](https://docs.rs/zerodds-foundation)

Foundation-layer primitives for the [ZeroDDS](https://zerodds.org) stack. Pure Rust, `no_std`-capable, `forbid(unsafe_code)`. Safety classification: **SAFE**.

## What's inside

- **Stack buffer** — `PoolBuffer<CAP>` with fixed capacity for hot-path allocations without touching the heap; explicit `PoolBufferError::Overflow` instead of a panic.
- **Wire-integrity hashes** — `crc32c` (RFC 4960 App. B), `crc64_xz` (ECMA-182 / XZ utils), `md5` (RFC 1321). Pure Rust with no external crypto-crate dependency. Used in the DDSI-RTPS HEADER_EXTENSION checksum, XTypes EquivalenceHash, KeyHash and group digest.
- **Observability** — structured `Event`/`Component`/`Level` plus a `Sink` trait for arbitrary consumers. `NullSink`, `StderrJsonSink`, `VecSink` as reference implementations.
- **Tracing** — `Span`/`SpanContext`/`TraceId`/`SpanId` plus `Histogram` for coarse-grained tracing; OTLP export in the `zerodds-observability-otlp` crate.
- **RCU** — `RcuCell<T>` as a copy-on-write container for low-write/high-read patterns without `unsafe` (mutex-protected reference cell, `Arc<T>` snapshots).

## Layer position

Layer 0 (Foundation). Has **no** dependencies on other ZeroDDS crates. Used by layer 1 (primitives: cdr, qos, types) and all higher layers.

## Quickstart

```rust
use zerodds_foundation::{crc32c, PoolBuffer, PoolBufferError};

// CRC-32C over an RTPS datagram.
let payload = b"\x52\x54\x50\x53\x02\x05\x01\x0F";
let checksum = crc32c(payload);

// Hot-path buffer with fixed capacity.
let mut buf: PoolBuffer<256> = PoolBuffer::new();
buf.extend_from_slice(payload).unwrap();

// Overflow is explicit, no panic.
let mut tiny: PoolBuffer<4> = PoolBuffer::new();
assert_eq!(
    tiny.extend_from_slice(payload),
    Err(PoolBufferError::Overflow)
);
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Enables `RcuCell`, `StderrJsonSink`, `VecSink`. Implies `alloc`. |
| `alloc` | ✅ (via `std`) | Enables `observability` + `tracing` + MD5 with arbitrary input length. |
| `safety` | ❌ | Reserved for future safety build constraints. |

Without features (`default-features = false`): only `PoolBuffer`, `crc32c`, `crc64_xz`, `md5` (the no_std MD5 path is limited to 56 bytes of input).

## Stability

All `pub` items are stable from `1.0.0`; breaking changes require a major version bump.

## Tests

```bash
cargo test -p zerodds-foundation
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — layered architecture of the workspace
- [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) — safety classification
