// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-foundation`. Safety classification: **SAFE**.
//!
//! Foundation-Layer-Primitives fuer den ZeroDDS-Stack: Hot-Path-
//! Buffer-Pools, Wire-Integritaets-Hashes (CRC-32C / CRC-64-XZ / MD5),
//! Observability-Event-Sprache + Sinks, Tracing-Spans + Histogramme,
//! und ein Lock-Free-Read RCU-Cell-Container.
//!
//! `no_std`-tauglich, `forbid(unsafe_code)`. Pure-Rust ohne externe
//! Crates — die Foundation-Pillar-Idee „Frame bleibt nicht hohl".
//!
//! ## Schichten-Position
//!
//! Layer 0 (Foundation). Hat **keine** Dependencies auf andere
//! ZeroDDS-Crates. Wird von Layer 1 (Primitives: cdr, qos, types)
//! und allen hoeheren Schichten verwendet.
//!
//! Architektur-Referenz: `docs/architecture/02_architecture.md §3`
//! und `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! ## Public API
//!
//! - **Stack-Buffer** ([`PoolBuffer`], [`PoolBufferError`]): fixed-capacity
//!   Buffer fuer Hot-Path-Allokationen, on-stack mit `CAP`-Generic.
//!   Append-Operationen sind O(1) ohne Heap-Touch; Overflow wird als
//!   Result signalisiert statt zu panicen.
//! - **CRC + MD5** ([`crc32c`], [`crc64_xz`], [`md5`]): Wire-Integritaets-
//!   Hashes; pure-Rust mit Standard-Lookup-Tables.
//! - **Observability** ([`Event`], [`Sink`], [`Level`], [`Component`],
//!   [`NullSink`], [`StderrJsonSink`], [`VecSink`]): strukturierte
//!   DDS-Events; Sink-Trait fuer beliebige Konsumenten.
//! - **Tracing** ([`Span`], [`SpanContext`], [`TraceId`], [`SpanId`],
//!   [`SpanKind`], [`SpanStatus`], [`Histogram`]): Spans + Histogramme
//!   fuer grobgranulares Tracing; OTLP-Export im
//!   `zerodds-observability-otlp`-Crate.
//! - **RCU** ([`RcuCell`]): Copy-on-Write-Container fuer wenig-Schreib/
//!   viel-Lese-Patterns ohne `unsafe`.
//!
//! ## Feature-Flags
//!
//! | Feature | Default | Zweck |
//! |---------|---------|-------|
//! | `std` | ✅ | Aktiviert `BufferPool`, `RcuCell`, `StderrJsonSink`, `VecSink`. Implies `alloc`. |
//! | `alloc` | ✅ (via `std`) | Aktiviert `observability` + `tracing` + MD5 mit Vec-Padding. |
//! | `safety` | ❌ | Reserviert fuer zukuenftige Safety-Build-Constraints. |
//!
//! Ohne Features (`default-features = false`): nur `PoolBuffer`,
//! `crc32c`, `crc64_xz`, `md5` (no_std-MD5-Pfad ist auf 56 Byte
//! Eingabe limitiert).
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_foundation::{crc32c, PoolBuffer, PoolBufferError};
//!
//! // CRC-32C ueber ein RTPS-Datagramm.
//! let payload = b"\x52\x54\x50\x53\x02\x05\x01\x0F";
//! let checksum = crc32c(payload);
//! assert_eq!(checksum & 0xFFFF_FFFF, checksum);
//!
//! // Hot-Path-Buffer mit fester Kapazitaet.
//! let mut buf: PoolBuffer<256> = PoolBuffer::new();
//! buf.extend_from_slice(payload).unwrap();
//! assert_eq!(buf.as_slice(), payload);
//!
//! // Overflow ist explizit, kein Panic.
//! let mut tiny: PoolBuffer<4> = PoolBuffer::new();
//! assert_eq!(
//!     tiny.extend_from_slice(payload),
//!     Err(PoolBufferError::Overflow)
//! );
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod buffer;
pub mod crc;
#[cfg(feature = "alloc")]
pub mod observability;
#[cfg(feature = "std")]
pub mod rcu;
#[cfg(feature = "alloc")]
pub mod tracing;

pub use buffer::{PoolBuffer, PoolBufferError};
pub use crc::{crc32c, crc64_xz, md5};
#[cfg(feature = "alloc")]
pub use observability::{Component, Event, Level, NullSink, SharedSink, Sink, null_sink};
#[cfg(feature = "std")]
pub use observability::{StderrJsonSink, VecSink};
#[cfg(feature = "std")]
pub use rcu::RcuCell;
#[cfg(feature = "alloc")]
pub use tracing::{Histogram, Span, SpanContext, SpanId, SpanKind, SpanStatus, TraceId};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke-Test: Crate kompiliert und Testharness laeuft.
    }
}
