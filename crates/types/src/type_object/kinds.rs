// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeObject discriminator constants (XTypes 1.3 §7.3.4.1 / §7.3.4.4).
//!
//! These constants discriminate the `MinimalTypeObject` and
//! `CompleteTypeObject` union. They differ from the
//! TypeIdentifier constants for composite types (TypeObject has
//! its own kind codes for STRUCTURE, UNION, etc., from 0x50).

/// `alias<T>`.
pub const TK_ALIAS: u8 = 0x30;

/// `enum`.
pub const TK_ENUM: u8 = 0x40;
/// `bitmask`.
pub const TK_BITMASK: u8 = 0x41;

/// `annotation`.
pub const TK_ANNOTATION: u8 = 0x42;

/// `struct`.
pub const TK_STRUCTURE: u8 = 0x50;
/// `union`.
pub const TK_UNION: u8 = 0x51;
/// `bitset`.
pub const TK_BITSET: u8 = 0x52;

/// `sequence<T>` (collection).
pub const TK_SEQUENCE: u8 = 0x60;
/// `T[N]` (collection).
pub const TK_ARRAY: u8 = 0x61;
/// `map<K,V>` (collection).
pub const TK_MAP: u8 = 0x62;
