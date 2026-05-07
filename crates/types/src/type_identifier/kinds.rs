// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 TypeIdentifier-Discriminator-Konstanten.
//!
//! Spec: OMG DDS-XTypes 1.3, §7.3.4.2 Table "TypeIdentifier discriminator".
//!
//! **Abgrenzung**: Dies sind die Konstanten fuer den TypeIdentifier-
//! Union-Discriminator (`_d`-Feld, 1 byte). Die TypeObject-Discriminator-
//! Konstanten (TK_STRUCTURE = 0xA0 etc.) leben in einem eigenen Modul
//! (T2), weil die Namensraeume fuer Primitives zwar ueberlappen, fuer
//! composite Types aber getrennt sind.

// ============================================================================
// Primitive TypeKind (0x00..0x11) — im TypeIdentifier als reine
// Discriminators OHNE Body verwendbar.
// ============================================================================

/// Kein Typ / Platzhalter.
pub const TK_NONE: u8 = 0x00;
/// `bool`.
pub const TK_BOOLEAN: u8 = 0x01;
/// `octet` / `byte` (8-bit unsigned).
pub const TK_BYTE: u8 = 0x02;
/// `int16`.
pub const TK_INT16: u8 = 0x03;
/// `int32`.
pub const TK_INT32: u8 = 0x04;
/// `int64`.
pub const TK_INT64: u8 = 0x05;
/// `uint16`.
pub const TK_UINT16: u8 = 0x06;
/// `uint32`.
pub const TK_UINT32: u8 = 0x07;
/// `uint64`.
pub const TK_UINT64: u8 = 0x08;
/// `float32`.
pub const TK_FLOAT32: u8 = 0x09;
/// `float64`.
pub const TK_FLOAT64: u8 = 0x0A;
/// `float128` (long double).
pub const TK_FLOAT128: u8 = 0x0B;
/// `int8`.
pub const TK_INT8: u8 = 0x0C;
/// `uint8`.
pub const TK_UINT8: u8 = 0x0D;
/// `char` (8-bit character).
pub const TK_CHAR8: u8 = 0x10;
/// `wchar` (16-bit character).
pub const TK_CHAR16: u8 = 0x11;

// ============================================================================
// TypeIdentifier-spezifische Discriminators (TI_*, EK_*)
// ============================================================================

/// `string<Bound>` mit Bound <= 255 — body: `octet bound`.
pub const TI_STRING8_SMALL: u8 = 0x70;
/// `string<Bound>` mit Bound > 255 — body: `uint32 bound`.
pub const TI_STRING8_LARGE: u8 = 0x71;
/// `wstring<Bound>` mit Bound <= 255.
pub const TI_STRING16_SMALL: u8 = 0x72;
/// `wstring<Bound>` mit Bound > 255.
pub const TI_STRING16_LARGE: u8 = 0x73;

/// `sequence<T, N>` mit N <= 255 — plain (T ist primitive oder auch plain).
pub const TI_PLAIN_SEQUENCE_SMALL: u8 = 0x80;
/// `sequence<T, N>` mit N > 255.
pub const TI_PLAIN_SEQUENCE_LARGE: u8 = 0x81;

/// `T[N]` mit Dimensionen-Max <= 255.
pub const TI_PLAIN_ARRAY_SMALL: u8 = 0x90;
/// `T[N]` mit Dimensionen-Max > 255.
pub const TI_PLAIN_ARRAY_LARGE: u8 = 0x91;

/// `map<K, V, N>` mit N <= 255.
pub const TI_PLAIN_MAP_SMALL: u8 = 0xA0;
/// `map<K, V, N>` mit N > 255.
pub const TI_PLAIN_MAP_LARGE: u8 = 0xA1;

/// Stark-zusammenhaengende Komponente (rekursive Typen) — XTypes §7.3.4.9.
pub const TI_STRONGLY_CONNECTED_COMPONENT: u8 = 0xB0;

/// 14-byte SHA256-Hash des Minimal-TypeObject.
pub const EK_MINIMAL: u8 = 0xF1;
/// 14-byte SHA256-Hash des Complete-TypeObject.
pub const EK_COMPLETE: u8 = 0xF2;

/// 14-byte Hash (im Wire-Format bei EK_MINIMAL/EK_COMPLETE).
pub const EQUIVALENCE_HASH_LEN: usize = 14;
