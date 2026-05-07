// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! FlatStruct-Trait + Slot-Header fuer Zero-Copy Same-Host-Pub/Sub.
//!
//! Crate `zerodds-flatdata`. Safety classification: **STANDARD**.
//!
//! Spec: `docs/specs/zerodds-flatdata-1.0.md`.
//!
//! # Sicherheits-Begruendung
//!
//! Dieser Crate implementiert eine `unsafe trait FlatStruct`, dessen
//! Garantien (Copy + repr(C) + 'static + keine Pointer/Vec/String) der
//! Implementer per `unsafe impl` zusichert. Die `as_bytes` /
//! `from_bytes_unchecked`-Helpers sind dann sicher per Layout — wir
//! lokalisieren das `unsafe`-Island hier statt es in den DataWriter-
//! Pfad zu streuen.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
// Begruendet: FlatStruct-Trait ist by-design unsafe (Layout-Garantien
// liegen beim Caller). Per-Block-SAFETY-Kommentare sind unten.
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
pub use iceoryx::{Iceoryx2Error, Iceoryx2Publisher, Iceoryx2Subscriber};
#[cfg(feature = "alloc")]
pub use locator::{LocatorError, ShmLocator, fnv1a_32, is_same_host};
#[cfg(feature = "posix-mmap")]
pub use posix::{PosixSlotAllocator, PosixSlotError};
#[cfg(feature = "std")]
pub use pubsub::{FlatReader, FlatSampleRef, FlatSlot, FlatWriter};
pub use slot::{ReaderMask, SLOT_HEADER_SIZE, SlotHeader};

/// Marker-Trait fuer FlatData-faehige Types.
///
/// Spec-Zitat (zerodds-flatdata-1.0 §1.1):
///
/// > Garantiert:
/// > - `Self: Copy` (kein Drop-Glue, plain bytes)
/// > - `Self: 'static` (kein Lifetime-Reference)
/// > - `#[repr(C)]` mit fest definiertem Alignment
/// > - `as_bytes()` und `from_bytes_unchecked()` sind safe-by-Layout
///
/// # Safety
///
/// Implementer MUSS sicherstellen:
/// - `Self` ist `#[repr(C)]` (oder `#[repr(transparent)]` ueber einen
///   einzigen `repr(C)`-Type).
/// - Alle Felder sind `FlatStruct` oder Primitiv-Types ohne padding-
///   sensitive UB (kein `#[repr(packed)]` mit Pointer-aligned Fields).
/// - `Self: Copy` (Trait-Bound erzwingt das).
/// - `TYPE_HASH` ist eindeutig fuer die exakte Wire-Layout-Variante;
///   bei jeder Schema-Aenderung MUSS der Hash regeneriert werden.
pub unsafe trait FlatStruct: Copy + 'static + Send + Sync {
    /// Wire-Size der `repr(C)`-Struktur (= `core::mem::size_of::<Self>()`).
    const WIRE_SIZE: usize = core::mem::size_of::<Self>();

    /// Eindeutiger Type-Hash (16 byte). Caller-Code generiert via
    /// SHA-256(`type_name + field_layout_string`) und nimmt die
    /// ersten 16 byte. Reader prueft diesen Hash gegen den
    /// Discovery-Hash; Mismatch → Slot-Drop.
    const TYPE_HASH: [u8; 16];

    /// Liefert das Slot-Layout als Slice. Safe-by-Layout — `Self: Copy`
    /// + `repr(C)` garantieren dass der byte-cast defined ist.
    #[must_use]
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: FlatStruct-Trait verlangt repr(C) + Copy. Damit ist
        // `*const Self` ↔ `*const u8` ein wohldefinierter Reinterpret-
        // Cast (keine Tail-Padding-Lecks weil Caller sich verpflichtet
        // hat).
        unsafe {
            core::slice::from_raw_parts(core::ptr::from_ref(self).cast::<u8>(), Self::WIRE_SIZE)
        }
    }

    /// Rekonstruiert aus rohem Slice. Caller MUSS:
    /// - bytes.len() >= WIRE_SIZE
    /// - bytes-Provenance valide fuer WIRE_SIZE
    /// - Type-Hash zuvor verifiziert (sonst UB bei alignement-Mismatch)
    ///
    /// # Safety
    /// Siehe oben.
    #[must_use]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::WIRE_SIZE);
        // SAFETY: Caller-Kontrakt + WIRE_SIZE-Check.
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

    // SAFETY: Pose ist repr(C), Copy, 'static. Felder sind alle
    // Primitiv-i64 ohne padding-Issues.
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
        // SAFETY: bytes stammt aus genau dieser Pose-Instanz, daher
        // Type-Hash trivial konsistent.
        let p2: Pose = unsafe { Pose::from_bytes_unchecked(bytes) };
        assert_eq!(p, p2);
    }

    #[test]
    fn type_hash_is_consistent() {
        assert_eq!(Pose::TYPE_HASH, [0x42; 16]);
    }
}
