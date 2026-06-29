// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeObject discriminator constants (XTypes 1.3 §7.3.4.1 / §7.3.4.4).
//!
//! These constants discriminate the `MinimalTypeObject` and
//! `CompleteTypeObject` union. The structured-type kinds occupy the
//! `0x50..=0x53` slots per DDS-XTypes 1.3 §7.2.2.2 — byte-verified against
//! CycloneDDS 11 / Fast DDS 3.6 / RTI Connext 7.7 (TypeObject vendor
//! oracle; see `proofs/typeobject`). An earlier version had these shifted
//! down by one slot (ANNOTATION=0x42, STRUCTURE=0x50, …), which wire-tagged
//! a struct as an *annotation* and broke the EquivalenceHash against every
//! vendor.

/// `alias<T>`.
pub const TK_ALIAS: u8 = 0x30;

/// `enum`.
pub const TK_ENUM: u8 = 0x40;
/// `bitmask`.
pub const TK_BITMASK: u8 = 0x41;

/// `annotation` (§7.2.2.2 = 0x50).
pub const TK_ANNOTATION: u8 = 0x50;

/// `struct` (§7.2.2.2 = 0x51).
pub const TK_STRUCTURE: u8 = 0x51;
/// `union` (§7.2.2.2 = 0x52).
pub const TK_UNION: u8 = 0x52;
/// `bitset` (§7.2.2.2 = 0x53).
pub const TK_BITSET: u8 = 0x53;

/// `sequence<T>` (collection).
pub const TK_SEQUENCE: u8 = 0x60;
/// `T[N]` (collection).
pub const TK_ARRAY: u8 = 0x61;
/// `map<K,V>` (collection).
pub const TK_MAP: u8 = 0x62;
