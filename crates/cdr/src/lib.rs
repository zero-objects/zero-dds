// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-cdr`. Safety classification: **SAFE**.
//!
//! XCDR1/XCDR2 encoder/decoder + KeyHash + PL_CDR1 Member-Codec für
//! `@mutable` strukturierte Daten — implementiert OMG XTypes 1.3 §7.4
//! Wire-Format vollstaendig.
//!
//! ## Spec
//!
//! - **OMG XTypes 1.3** §7.4 (Wire-Encoding) inkl. §7.4.1.2 (PL_CDR1),
//!   §7.4.2 (Plain CDR2 / Delimited CDR2 / PL_CDR2), §7.4.4 (Composite
//!   Types: String/Sequence/Array/Optional), §7.4.5 (Struct-Extensibility:
//!   final/appendable/mutable).
//! - **OMG XTypes 1.3** §7.6.8 KeyHash (CDR_BE Key-Holder + MD5-Fallback).
//! - **DDSI-RTPS 2.5** §10 Wire-Encapsulation (RepresentationIdentifier).
//!
//! ## Schichten-Position
//!
//! Layer 1 — Primitives. Direkte Abhaengige: `zerodds-types`, `zerodds-qos`,
//! `zerodds-rtps`, `zerodds-discovery`, `zerodds-dcps`, `zerodds-idl-rust`,
//! `dds-corba-{ior,iiop,giop,csiv2,cosnaming,rust}`, `zerodds-ts-wasm`.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`BufferReader`] / [`BufferWriter`] — alignment-tracking byte-level
//!   I/O Primitives.
//! - [`Endianness`] — Little/Big-Endian-Marker.
//! - [`CdrEncode`] / [`CdrDecode`] — Trait-Familie für serialisierbare
//!   Typen; Implementationen für alle Spec-Primitives plus Composite-
//!   Types (String, `Vec<T>`, `[T; N]`, `Option<T>`) im `composite`-Modul.
//! - [`EncodeError`] / [`DecodeError`] — typisierte Fehler mit Offset-
//!   Information.
//! - [`fixed::Fixed`] — IDL-`fixed<P,S>` Decimal-Type mit BCD-Encoding.
//! - [`struct_enc`] — XCDR2-Extensibility-Encoder (`encode_final`,
//!   `encode_appendable`, [`struct_enc::MutableStructEncoder`] +
//!   `read_mutable_member`).
//! - [`xcdr1`] — XTypes-1.3-konformer PL_CDR1-Member-Codec
//!   (`encode_pl_cdr1_member`, `read_pl_cdr1_member`,
//!   `read_all_pl_cdr1_members`, `write_pl_cdr1_sentinel`).
//! - [`key_hash::compute_key_hash`] / [`PlainCdr2BeKeyHolder`] /
//!   [`KEY_HASH_LEN`] — XTypes 1.3 §7.6.8 KeyHash mit MD5-Fallback.
//!
//! ## Beispiel
//!
//! ```rust
//! use zerodds_cdr::{BufferWriter, BufferReader, Endianness, CdrEncode, CdrDecode};
//!
//! let mut w = BufferWriter::new(Endianness::Little);
//! 42u32.encode(&mut w).unwrap();
//! let bytes = w.into_bytes();
//!
//! let mut r = BufferReader::new(&bytes, Endianness::Little);
//! assert_eq!(u32::decode(&mut r).unwrap(), 42);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod buffer;
#[cfg(feature = "alloc")]
pub mod composite;
pub mod encode;
pub mod endianness;
pub mod error;
#[cfg(feature = "alloc")]
pub mod fixed;
#[cfg(feature = "alloc")]
pub mod key_hash;
#[cfg(feature = "alloc")]
pub mod struct_enc;
#[cfg(feature = "alloc")]
pub mod xcdr1;

#[cfg(feature = "serde-bridge")]
pub mod serde_bridge;

pub use endianness::Endianness;
pub use error::{DecodeError, EncodeError};

#[cfg(feature = "alloc")]
pub use key_hash::{KEY_HASH_LEN, PlainCdr2BeKeyHolder, compute_key_hash};

pub use buffer::BufferReader;
#[cfg(feature = "alloc")]
pub use buffer::BufferWriter;

pub use encode::{CdrDecode, CdrEncode};
