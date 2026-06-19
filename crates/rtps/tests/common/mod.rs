//! Shared helpers for integration tests in `crates/rtps/tests/`.
//!
//! Included via `mod common;`. Test files should not contain
//! duplicates of generators/fixtures.

#![allow(dead_code)] // each test file uses only a subset

use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

/// Canonical test GUIDs. The writer and reader side must use the same
/// values in E2E tests, so that `handle_acknack(src_guid, ...)`
/// dispatches correctly.
pub const TEST_WRITER_KEY: [u8; 3] = [0x10, 0x20, 0x30];
pub const TEST_READER_KEY: [u8; 3] = [0xA0, 0xB0, 0xC0];

#[must_use]
pub fn test_writer_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([1; 12]),
        EntityId::user_writer_with_key(TEST_WRITER_KEY),
    )
}

#[must_use]
pub fn test_reader_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([2; 12]),
        EntityId::user_reader_with_key(TEST_READER_KEY),
    )
}

/// Deterministic but non-trivial sample content for
/// byte-exact reassembly checking.
///
/// Formula: `byte[i] = (sn * K + i) & 0xFF`, where
/// `K = 0x9E3779B1` (Knuth multiplicative hash, golden-ratio fraction).
/// This choice:
/// - spreads the bytes evenly (no constant or linear
///   value that would mask reassembly bugs),
/// - is deterministic (no RNG state; the same sn yields an identical
///   sequence) — important for test reproducibility,
/// - is computable in o(len) without tables.
///
/// If this function changes, all fragment E2E tests must be
/// adapted accordingly (they compare `pattern_for(i, n)`
/// 1:1 with the reassembled reader sample).
#[must_use]
pub fn pattern_for(sn: usize, len: usize) -> Vec<u8> {
    let seed = (sn as u32).wrapping_mul(0x9E37_79B1);
    (0..len)
        .map(|i| (seed.wrapping_add(i as u32) & 0xFF) as u8)
        .collect()
}

/// Einfacher xorshift32-RNG — deterministisch, reproducible.
#[derive(Debug, Clone)]
pub struct XorShift32(pub u32);

impl XorShift32 {
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
