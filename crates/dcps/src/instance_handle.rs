// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `InstanceHandle` — opaker, lokaler Identifier fuer Entities und
//! Sample-Instanzen (DDS DCPS 1.4 §2.3.3 IDL-PSM, §2.2.2.5.1
//! SampleInfo.instance_handle).
//!
//! In der Spec ist `InstanceHandle_t` ein Builtin-Type ohne fixe
//! Wire-Form — er wird **nie** auf den Wire gestellt, sondern dient
//! ausschliesslich der lokalen Identifikation (z.B. um in
//! `DataReader::read_instance()` einen Sample-Stream zu adressieren).
//! Die Spec stellt nur sicher, dass `HANDLE_NIL` einen reservierten
//! "kein Handle"-Wert hat.
//!
//! Wir kodieren ihn als **opake `u64`**:
//! * Eindeutig pro Entity / Sample-Instanz innerhalb einer Runtime.
//! * `HANDLE_NIL` = 0 (Spec-Konvention).
//! * Erzeugung via [`InstanceHandleAllocator`] mit monoton steigendem
//!   Counter — keine Wiederverwendung gedroppter Handles.

extern crate alloc;

use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_rtps::wire_types::Guid;

/// Opaker `InstanceHandle_t` (DDS-DCPS 1.4 §2.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct InstanceHandle(u64);

impl InstanceHandle {
    /// Reservierter "kein Handle"-Wert (Spec-Konvention).
    pub const NIL: Self = Self(0);

    /// Konstruktor aus einem rohen u64-Wert. Niedrige API — ueblicherweise
    /// erzeugt der [`InstanceHandleAllocator`] die Werte.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Roher Wert.
    #[must_use]
    pub const fn as_raw(&self) -> u64 {
        self.0
    }

    /// `true` wenn der Handle [`Self::NIL`] ist.
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        self.0 == 0
    }

    /// Deterministische Ableitung eines [`InstanceHandle`] aus einem
    /// `Guid` (16 Byte BuiltinTopicKey). Wird fuer
    /// `ignore_*`-/`get_discovered_*`-APIs (DDS DCPS 1.4
    /// §2.2.2.2.1.14-17, §2.2.2.2.1.27-30) verwendet, weil die Spec
    /// dort `InstanceHandle_t` als Argument verlangt, der Builtin-
    /// Subscriber aber `BuiltinTopicKey_t` (=Guid) als Sample-Key
    /// fuehrt.
    ///
    /// Implementation: 64-bit FNV-1a ueber alle 16 Bytes — schnell,
    /// no_std, kollisionsarm fuer einige tausend Discovered Entities.
    /// `HANDLE_NIL` wird durch Anheben auf 1 vermieden.
    #[must_use]
    pub fn from_guid(guid: Guid) -> Self {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut h = FNV_OFFSET;
        for b in guid.to_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV_PRIME);
        }
        if h == 0 { Self(1) } else { Self(h) }
    }
}

impl core::fmt::Display for InstanceHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_nil() {
            f.write_str("HANDLE_NIL")
        } else {
            write!(f, "InstanceHandle({:#x})", self.0)
        }
    }
}

/// Spec-Alias fuer den NIL-Handle (DDS-DCPS 1.4 §2.3.3).
pub const HANDLE_NIL: InstanceHandle = InstanceHandle::NIL;

/// Atomarer Counter zur Vergabe eindeutiger [`InstanceHandle`]-Werte.
///
/// Eine Instanz pro Runtime/Process — ueblicherweise auf der
/// `DcpsRuntime` als gemeinsame Allokationsquelle fuer Entities und
/// Sample-Instanzen.
#[derive(Debug)]
pub struct InstanceHandleAllocator {
    next: AtomicU64,
}

impl Default for InstanceHandleAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceHandleAllocator {
    /// Neuer Allocator. Erste Vergabe liefert `1` (HANDLE_NIL=0 ist
    /// reserviert).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// Liefert den naechsten freien Handle (monoton steigend).
    /// Thread-safe.
    #[must_use]
    pub fn allocate(&self) -> InstanceHandle {
        let v = self.next.fetch_add(1, Ordering::Relaxed);
        // Wraparound-Defense: u64 reicht > 580 Jahre bei 1 Mio Handles/s.
        // Falls trotzdem 0 → NIL → wir ueberspringen und nehmen den
        // naechsten.
        if v == 0 {
            self.allocate()
        } else {
            InstanceHandle(v)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn handle_nil_is_zero() {
        assert_eq!(HANDLE_NIL.as_raw(), 0);
        assert!(HANDLE_NIL.is_nil());
    }

    #[test]
    fn allocator_starts_at_one() {
        let alloc = InstanceHandleAllocator::new();
        let h = alloc.allocate();
        assert_eq!(h.as_raw(), 1);
        assert!(!h.is_nil());
    }

    #[test]
    fn allocator_is_monotonic() {
        let alloc = InstanceHandleAllocator::new();
        let h1 = alloc.allocate();
        let h2 = alloc.allocate();
        let h3 = alloc.allocate();
        assert!(h1.as_raw() < h2.as_raw());
        assert!(h2.as_raw() < h3.as_raw());
    }

    #[test]
    fn handles_are_distinct() {
        let alloc = InstanceHandleAllocator::new();
        let mut seen = alloc::collections::BTreeSet::new();
        for _ in 0..1000 {
            let h = alloc.allocate();
            assert!(seen.insert(h.as_raw()));
        }
    }

    #[test]
    fn display_shows_nil_or_hex() {
        assert_eq!(alloc::format!("{HANDLE_NIL}"), "HANDLE_NIL");
        let h = InstanceHandle::from_raw(0xABCD);
        assert_eq!(alloc::format!("{h}"), "InstanceHandle(0xabcd)");
    }

    #[test]
    fn from_raw_and_as_raw_roundtrip() {
        let h = InstanceHandle::from_raw(42);
        assert_eq!(h.as_raw(), 42);
    }

    #[test]
    fn from_guid_is_deterministic() {
        use zerodds_rtps::wire_types::Guid;
        let g = Guid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 1, 0xC1]);
        let h1 = InstanceHandle::from_guid(g);
        let h2 = InstanceHandle::from_guid(g);
        assert_eq!(h1, h2);
        assert!(!h1.is_nil());
    }

    #[test]
    fn from_guid_distinguishes_keys() {
        use zerodds_rtps::wire_types::Guid;
        let g1 = Guid::from_bytes([1; 16]);
        let g2 = Guid::from_bytes([2; 16]);
        assert_ne!(InstanceHandle::from_guid(g1), InstanceHandle::from_guid(g2));
    }
}
