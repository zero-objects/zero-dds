// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `InstanceHandle` — opaque, local identifier for entities and sample
//! instances (DDS DCPS 1.4 §2.3.3 IDL-PSM, §2.2.2.5.1
//! SampleInfo.instance_handle).
//!
//! In the spec, `InstanceHandle_t` is a builtin type with no fixed wire
//! form — it is **never** placed on the wire, but serves solely for
//! local identification (e.g. to address a sample stream in
//! `DataReader::read_instance()`). The spec only guarantees that
//! `HANDLE_NIL` has a reserved "no handle" value.
//!
//! We encode it as an **opaque `u64`**:
//! * Unique per entity / sample instance within a runtime.
//! * `HANDLE_NIL` = 0 (spec convention).
//! * Created via [`InstanceHandleAllocator`] with a monotonically
//!   increasing counter — no reuse of dropped handles.

extern crate alloc;

use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_rtps::wire_types::Guid;

/// Opaque `InstanceHandle_t` (DDS-DCPS 1.4 §2.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct InstanceHandle(u64);

impl InstanceHandle {
    /// Reserved "no handle" value (spec convention).
    pub const NIL: Self = Self(0);

    /// Constructor from a raw u64 value. Low-level API — normally the
    /// [`InstanceHandleAllocator`] produces the values.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Raw value.
    #[must_use]
    pub const fn as_raw(&self) -> u64 {
        self.0
    }

    /// `true` if the handle is [`Self::NIL`].
    #[must_use]
    pub const fn is_nil(&self) -> bool {
        self.0 == 0
    }

    /// Deterministic derivation of an [`InstanceHandle`] from a `Guid`
    /// (16-byte BuiltinTopicKey). Used for the `ignore_*` /
    /// `get_discovered_*` APIs (DDS DCPS 1.4 §2.2.2.2.1.14-17,
    /// §2.2.2.2.1.27-30), because there the spec requires
    /// `InstanceHandle_t` as the argument, while the builtin subscriber
    /// keeps `BuiltinTopicKey_t` (= Guid) as the sample key.
    ///
    /// Implementation: 64-bit FNV-1a over all 16 bytes — fast, no_std,
    /// low collision for a few thousand discovered entities.
    /// `HANDLE_NIL` is avoided by bumping it up to 1.
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

/// Spec alias for the NIL handle (DDS-DCPS 1.4 §2.3.3).
pub const HANDLE_NIL: InstanceHandle = InstanceHandle::NIL;

/// Atomic counter for handing out unique [`InstanceHandle`] values.
///
/// One instance per runtime/process — typically on the `DcpsRuntime`
/// as the shared allocation source for entities and sample instances.
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
    /// New allocator. The first allocation returns `1` (HANDLE_NIL=0 is
    /// reserved).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// Returns the next free handle (monotonically increasing).
    /// Thread-safe.
    #[must_use]
    pub fn allocate(&self) -> InstanceHandle {
        let v = self.next.fetch_add(1, Ordering::Relaxed);
        // Wraparound defense: u64 lasts > 580 years at 1M handles/s.
        // If it still hits 0 → NIL → we skip it and take the next one.
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
