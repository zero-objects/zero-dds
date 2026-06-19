// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! FlatStruct trait + slot header for zero-copy same-host pub/sub.
//!
//! Crate `zerodds-flatdata`. Safety classification: **STANDARD**.
//!
//! Spec: `docs/specs/zerodds-flatdata-1.0.md`.
//!
//! # Safety rationale
//!
//! This crate implements an `unsafe trait FlatStruct` whose
//! guarantees (Copy + repr(C) + 'static + no pointers/Vec/String) the
//! implementer assures via `unsafe impl`. The `as_bytes` /
//! `from_bytes_unchecked` helpers are then safe by layout — we
//! localize the `unsafe` island here instead of scattering it into
//! the DataWriter path.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
// Rationale: the FlatStruct trait is unsafe by design (layout
// guarantees are the caller's responsibility). Per-block SAFETY
// comments are below.
#![allow(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
mod allocator;
#[cfg(feature = "std")]
mod backend;
#[cfg(feature = "iceoryx2-bridge")]
mod iceoryx;
#[cfg(feature = "alloc")]
mod locator;
#[cfg(feature = "posix-mmap")]
mod posix;
#[cfg(feature = "std")]
mod pubsub;
mod slot;

#[cfg(feature = "std")]
pub use allocator::{InMemorySlotAllocator, SlotError, SlotHandle};
#[cfg(feature = "std")]
pub use backend::SlotBackend;
#[cfg(feature = "iceoryx2-bridge")]
pub use iceoryx::{
    Iceoryx2Error, Iceoryx2Publisher, Iceoryx2Subscriber, RawIceoryx2Publisher,
    RawIceoryx2Subscriber,
};
#[cfg(feature = "alloc")]
pub use locator::{LocatorError, ShmLocator, fnv1a_32, is_same_host};
#[cfg(feature = "posix-mmap")]
pub use posix::{PosixSlotAllocator, PosixSlotError};
#[cfg(feature = "std")]
pub use pubsub::{FlatReader, FlatSampleRef, FlatSlot, FlatWriter};
pub use slot::{ReaderMask, SLOT_HEADER_SIZE, SlotHeader};

/// Marker trait for FlatData-capable types.
///
/// Spec quote (zerodds-flatdata-1.0 §1.1):
///
/// > Guarantees:
/// > - `Self: Copy` (no Drop glue, plain bytes)
/// > - `Self: 'static` (no lifetime reference)
/// > - `#[repr(C)]` with a fixed, defined alignment
/// > - `as_bytes()` and `from_bytes_unchecked()` are safe by layout
///
/// # Safety
///
/// The implementer MUST ensure:
/// - `Self` is `#[repr(C)]` (or `#[repr(transparent)]` over a
///   single `repr(C)` type).
/// - All fields are `FlatStruct` or primitive types without
///   padding-sensitive UB (no `#[repr(packed)]` with pointer-aligned fields).
/// - `Self: Copy` (the trait bound enforces this).
/// - `TYPE_HASH` is unique for the exact wire-layout variant;
///   on every schema change the hash MUST be regenerated.
pub unsafe trait FlatStruct: Copy + 'static + Send + Sync {
    /// Wire size of the `repr(C)` struct (= `core::mem::size_of::<Self>()`).
    const WIRE_SIZE: usize = core::mem::size_of::<Self>();

    /// Unique type-hash (16 byte). Caller code generates it via
    /// SHA-256(`type_name + field_layout_string`) and takes the
    /// first 16 byte. The reader checks this hash against the
    /// discovery hash; on mismatch → slot drop.
    const TYPE_HASH: [u8; 16];

    /// Returns the slot layout as a slice. Safe by layout — `Self: Copy`
    /// + `repr(C)` guarantee that the byte cast is defined.
    #[must_use]
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: the FlatStruct trait requires repr(C) + Copy. This makes
        // `*const Self` ↔ `*const u8` a well-defined reinterpret
        // cast (no tail-padding leaks because the caller has
        // committed to that).
        unsafe {
            core::slice::from_raw_parts(core::ptr::from_ref(self).cast::<u8>(), Self::WIRE_SIZE)
        }
    }

    /// Reconstructs from a raw slice. The caller MUST:
    /// - bytes.len() >= WIRE_SIZE
    /// - bytes provenance valid for WIRE_SIZE
    /// - type-hash verified beforehand (otherwise UB on alignment mismatch)
    ///
    /// # Safety
    /// See above.
    #[must_use]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::WIRE_SIZE);
        // SAFETY: caller contract + WIRE_SIZE check.
        unsafe { core::ptr::read_unaligned(bytes.as_ptr().cast::<Self>()) }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    #[repr(C)]
    struct Pose {
        x: i64,
        y: i64,
        z: i64,
    }

    // SAFETY: Pose is repr(C), Copy, 'static. All fields are
    // primitive i64 without padding issues.
    unsafe impl FlatStruct for Pose {
        const TYPE_HASH: [u8; 16] = [0x42; 16];
    }

    #[test]
    fn wire_size_matches_size_of() {
        assert_eq!(Pose::WIRE_SIZE, core::mem::size_of::<Pose>());
        assert_eq!(Pose::WIRE_SIZE, 24);
    }

    #[test]
    fn as_bytes_roundtrip() {
        let p = Pose { x: 1, y: 2, z: 3 };
        let bytes = p.as_bytes();
        assert_eq!(bytes.len(), 24);
        // SAFETY: bytes come from exactly this Pose instance, so the
        // type-hash is trivially consistent.
        let p2: Pose = unsafe { Pose::from_bytes_unchecked(bytes) };
        assert_eq!(p, p2);
    }

    #[test]
    fn type_hash_is_consistent() {
        assert_eq!(Pose::TYPE_HASH, [0x42; 16]);
    }
}
