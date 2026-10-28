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

#[cfg(test)]
extern crate std;

pub mod digest;
pub mod endianness;
pub mod error;
pub mod map_entries;
pub mod reader;
pub mod try_construct;
pub mod writer;

pub use digest::md5;
pub use endianness::Endianness;
pub use error::WireError;
pub use map_entries::{AscendingKey, MapEntry, WireMap};
pub use reader::Reader;
pub use try_construct::{
    Narrowed, TryConstruct, fit_map, fit_map_entries, fit_sequence, fit_string, fit_wstring,
};
pub use writer::{
    FixedDigits, FixedParts, ParameterRole, PidFlagPolicy, PlCdr1Framing, PlCdr1Reading,
    WcharFraming, WcharPosition, WcharWidth, WirePosition, Writer, WstringLengthCounts,
    WstringLengthPolicy, WstringTerminator, WstringUnitOrder, WstringUnitWidth, parse_fixed,
};
