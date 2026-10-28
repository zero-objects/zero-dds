// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-float128`. Safety classification: **SAFE**.
//!
//! IEEE 754 binary128, in software.
//!
//! CDR gives `long double` sixteen bytes and CORBA 3.3 part 2 §9.3.1.3
//! says what is in them: one sign bit, a fifteen-bit exponent biased by
//! 16383, and a 112-bit stored fraction. That is binary128, and
//! DDS-XTypes 1.3 Table 31 names it — "IEEE standard for normalized
//! quadruple-precision floating-point numbers", encoded size sixteen,
//! alignment eight.
//!
//! Rust has no stable `f128`, and every serialiser in this project used
//! to answer that by writing a `double` and eight zero bytes: the right
//! length and the wrong value, in silence. This crate is the answer
//! instead. A wire runtime does not need to *compute* with a quadruple —
//! it needs to carry one exactly, compare it, and read and write it as
//! text. Those are bit manipulation and long division, not research.
//!
//! ## What is here
//!
//! - [`Float128`] — the value, one `u128` of IEEE 754 binary128 pattern.
//! - Lossless sixteen-byte round trip in both byte orders.
//! - Exact conversion from `f32`, `f64` and the integer types, and back
//!   with the inexact cases *reported* rather than rounded in silence.
//! - IEEE comparison, and [`Float128::total_cmp`] for the places that
//!   need a total order — a key holder, a sorted expectation.
//! - Decimal parsing and formatting that round-trips, so a JSON value
//!   file can carry one.
//! - [`x87`] — the 80-bit extended format, because two of the four DDS
//!   vendors put *that* on the wire on x86 and a type that knows only
//!   the conformant form cannot read what the field actually sends.
//!
//! ## What is not here
//!
//! Arithmetic. `Add`, `Sub`, `Mul`, `Div` and `sqrt` are not
//! implemented; `docs/float128-column.md` §6 says what they would take.
//! Nothing in the wire path needs them, and a half-correct `Add` is
//! worse than none.
//!
//! ## Spec
//!
//! - CORBA 3.3 part 2 §9.3.1.1 (alignment) and §9.3.1.3 with Figure 9.2
//!   (the bit layout, and the whole-sixteen-byte order reversal).
//! - DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (size sixteen, alignment eight)
//!   and §7.4.3.2.2 `MAXALIGN` (four under XCDR2).
//! - IEEE 754-2008 clause 3.6 for binary128 itself.
//!
//! ## Layer position
//!
//! Layer 0. Depends on nothing.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod bigint;
mod cmp;
mod convert;
mod decimal;
mod float;
pub mod x87;

pub use convert::Inexact;
pub use decimal::ParseFloat128Error;
pub use float::{Class, Float128};
