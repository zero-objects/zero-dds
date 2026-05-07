# `zerodds-foundation`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-foundation/badge.svg)](https://docs.rs/zerodds-foundation)

Foundation-Layer-Primitive für den [ZeroDDS](https://zerodds.org)-Stack. Pure-Rust, `no_std`-tauglich, `forbid(unsafe_code)`. Safety classification: **SAFE**.

## Was ist drin

- **Stack-Buffer** — `PoolBuffer<CAP>` mit fester Kapazität für Hot-Path-Allokationen ohne Heap-Touch; explizite `PoolBufferError::Overflow` statt Panic.
- **Wire-Integrity-Hashes** — `crc32c` (RFC 4960 App. B), `crc64_xz` (ECMA-182 / XZ utils), `md5` (RFC 1321). Pure-Rust ohne externe Crypto-Crate-Abhängigkeit. Genutzt in DDSI-RTPS HEADER_EXTENSION-Checksum, XTypes EquivalenceHash, KeyHash und Group-Digest.
- **Observability** — strukturierte `Event`/`Component`/`Level` plus `Sink`-Trait für beliebige Konsumenten. `NullSink`, `StderrJsonSink`, `VecSink` als Reference-Implementations.
- **Tracing** — `Span`/`SpanContext`/`TraceId`/`SpanId` plus `Histogram` für grobgranulares Tracing; OTLP-Export im `zerodds-observability-otlp`-Crate.
- **RCU** — `RcuCell<T>` als Copy-on-Write-Container für wenig-Schreib/viel-Lese-Patterns ohne `unsafe` (Mutex-protected Reference-Cell, `Arc<T>`-Snapshots).

## Schichten-Position

Layer 0 (Foundation). Hat **keine** Dependencies auf andere ZeroDDS-Crates. Wird von Layer 1 (Primitives: cdr, qos, types) und allen höheren Schichten verwendet.

## Quickstart

```rust
use zerodds_foundation::{crc32c, PoolBuffer, PoolBufferError};

// CRC-32C über ein RTPS-Datagramm.
let payload = b"\x52\x54\x50\x53\x02\x05\x01\x0F";
let checksum = crc32c(payload);

// Hot-Path-Buffer mit fester Kapazität.
let mut buf: PoolBuffer<256> = PoolBuffer::new();
buf.extend_from_slice(payload).unwrap();

// Overflow ist explizit, kein Panic.
let mut tiny: PoolBuffer<4> = PoolBuffer::new();
assert_eq!(
    tiny.extend_from_slice(payload),
    Err(PoolBufferError::Overflow)
);
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Aktiviert `RcuCell`, `StderrJsonSink`, `VecSink`. Implies `alloc`. |
| `alloc` | ✅ (via `std`) | Aktiviert `observability` + `tracing` + MD5 mit beliebiger Eingabelänge. |
| `safety` | ❌ | Reserviert für zukünftige Safety-Build-Constraints. |

Ohne Features (`default-features = false`): nur `PoolBuffer`, `crc32c`, `crc64_xz`, `md5` (no_std-MD5-Pfad ist auf 56 Byte Eingabe limitiert).

## Stabilität

Alle `pub`-Items sind ab `1.0.0` stabil; Breaking-Changes erfordern Major-Version-Bump.

## Tests

```bash
cargo test -p zerodds-foundation
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — Schichten-Architektur des Workspaces
- [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) — Safety-Klassifikation
