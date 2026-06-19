// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-foundation`. Safety classification: **SAFE**.
//!
//! Foundation-layer primitives for the ZeroDDS stack: hot-path
//! buffer pools, wire-integrity hashes (CRC-32C / CRC-64-XZ / MD5),
//! observability event language + sinks, tracing spans + histograms,
//! and a lock-free-read RCU cell container.
//!
//! `no_std`-capable, `forbid(unsafe_code)`. Pure Rust without external
//! crates — the foundation-pillar idea "the frame does not stay hollow".
//!
//! ## Layer position
//!
//! Layer 0 (Foundation). Has **no** dependencies on other
//! ZeroDDS crates. Used by layer 1 (primitives: cdr, qos, types)
//! and all higher layers.
//!
//! Architecture reference: `docs/architecture/02_architecture.md §3`
//! and `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! ## Public API
//!
//! - **Stack buffer** ([`PoolBuffer`], [`PoolBufferError`]): fixed-capacity
//!   buffer for hot-path allocations, on-stack with a `CAP` generic.
//!   Append operations are O(1) without touching the heap; overflow is
//!   signalled as a Result instead of panicking.
//! - **CRC + MD5** ([`crc32c`], [`crc64_xz`], [`md5`]): wire-integrity
//!   hashes; pure Rust with standard lookup tables.
//! - **Observability** ([`Event`], [`Sink`], [`Level`], [`Component`],
//!   [`NullSink`], [`StderrJsonSink`], [`VecSink`]): structured
//!   DDS events; a Sink trait for arbitrary consumers.
//! - **Tracing** ([`Span`], [`SpanContext`], [`TraceId`], [`SpanId`],
//!   [`SpanKind`], [`SpanStatus`], [`Histogram`]): spans + histograms
//!   for coarse-grained tracing; OTLP export in the
//!   `zerodds-observability-otlp` crate.
//! - **RCU** ([`RcuCell`]): copy-on-write container for low-write/
//!   high-read patterns without `unsafe`.
//!
//! ## Feature flags
//!
//! | Feature | Default | Purpose |
//! |---------|---------|-------|
//! | `std` | ✅ | Enables `BufferPool`, `RcuCell`, `StderrJsonSink`, `VecSink`. Implies `alloc`. |
//! | `alloc` | ✅ (via `std`) | Enables `observability` + `tracing` + MD5 with Vec padding. |
//! | `safety` | ❌ | Reserved for future safety build constraints. |
//!
//! Without features (`default-features = false`): only `PoolBuffer`,
//! `crc32c`, `crc64_xz`, `md5` (the no_std MD5 path is limited to 56
//! bytes of input).
//!
//! ## Example
//!
//! ```rust
//! use zerodds_foundation::{crc32c, PoolBuffer, PoolBufferError};
//!
//! // CRC-32C over an RTPS datagram.
//! let payload = b"\x52\x54\x50\x53\x02\x05\x01\x0F";
//! let checksum = crc32c(payload);
//! assert_eq!(checksum & 0xFFFF_FFFF, checksum);
//!
//! // Hot-path buffer with fixed capacity.
//! let mut buf: PoolBuffer<256> = PoolBuffer::new();
//! buf.extend_from_slice(payload).unwrap();
//! assert_eq!(buf.as_slice(), payload);
//!
//! // Overflow is explicit, no panic.
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
        // Smoke test: the crate compiles and the test harness runs.
    }
}
