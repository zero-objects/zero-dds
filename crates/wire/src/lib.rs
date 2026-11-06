// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wire`. Safety classification: **SAFE**.
//!
//! The runtime a generated encoder calls: `Writer` and `Reader`, the
//! alignment rules, and the framing forms.
//!
//! This crate makes no decision about a type. Which members are written,
//! in what order, with which header and which length code is the layout
//! plan's answer and arrives here as arguments. A runtime that decided
//! any of it would be a second place where the wire format lives.
//!
//! ## A Rust type for an IDL primitive or `string`
//!
//! [`WireRepr`] lets a Rust type stand for an IDL primitive or `string`
//! in generated code — a checked date-time newtype for a `string`,
//! `NonZeroU32` for an `unsigned long` — while the octets, the
//! `TypeObject` and the key hash stay the IDL's.
//! `zerodds_build::Config::rust_type` names the mapping; [`repr`] has the
//! trait, the impls this crate carries (`core::num::NonZero*` over the
//! integers, `String`, `Cow<'_, str>`, and each primitive's own Rust type)
//! and the [`DisplayFromStr`] adapter for any `Display + FromStr` type. The writer hands an impl an [`IdlOut`] for
//! exactly its IDL type and formats a `string` straight into the sample's
//! buffer; the reader lends a `string` out of the received sample
//! ([`Reader::read_str`]) instead of allocating one.
//!
//! ## A type defined in Rust: the codec API
//!
//! [`codec`] has the traits a type defined in Rust implements —
//! `DdsTypeInfo` (its IDL shape), `DdsSerialize`, `DdsDeserialize<'de>` —
//! and the serializer and deserializer that execute the type's layout
//! plan. `derive(DdsType)` of `zerodds-codec` writes the three; a
//! hand-written impl is checked at compile time against the description.
//! A union is typed the same way, its branch a type-level index; a member
//! read through `codec::DdsMemberRead<'de>` may borrow from the received
//! bytes (`&'de str`, `&'de [u8]`, `Cow<'de, …>`).
//! `core::time::Duration` is the DDS `Duration_t` and, with the `std`
//! feature, `std::time::SystemTime` is `Time_t`.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-ast-1.0.md` §7 (the layout plan this serves).
//! - DDS-XTypes 1.3 §7.4 for the encodings themselves.
//!
//! ## Layer position
//!
//! Layer 0. Its one dependency is `zerodds-float128`, the value type of
//! `long double`; it does not depend on the model — a runtime that reached
//! back into the model would be a runtime that could disagree with it.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod codec;
pub mod digest;
pub mod endianness;
pub mod error;
pub mod map_entries;
pub mod reader;
pub mod repr;
pub mod try_construct;
pub mod writer;

pub use digest::md5;
pub use endianness::Endianness;
pub use error::WireError;
pub use map_entries::{AscendingKey, MapEntry, WireMap};
pub use reader::Reader;
pub use repr::{DisplayFromStr, IdlOut, IdlPrimitive, WireRepr, WireReprWith, Written};
pub use try_construct::{
    Narrowed, TryConstruct, fit_map, fit_map_entries, fit_sequence, fit_str, fit_string,
    fit_wstring,
};
pub use writer::{
    FixedDigits, FixedParts, ParameterRole, PidFlagPolicy, PlCdr1Framing, PlCdr1Reading,
    WcharFraming, WcharPosition, WcharWidth, WirePosition, Writer, WstringLengthCounts,
    WstringLengthPolicy, WstringTerminator, WstringUnitOrder, WstringUnitWidth, parse_fixed,
};
