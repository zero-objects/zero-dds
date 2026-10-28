// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the read direction builds a value into.
//!
//! The counterpart of [`crate::source`], and the asymmetry between them
//! is real: reading a value needs accessors, building one needs
//! constructors, and a constructor has to be told *what kind of thing*
//! it is building or the representation cannot store it.
//!
//! ## Every constructor carries its type
//!
//! [`Sink::integer`], [`Sink::float`] and [`Sink::character`] all take
//! the [`PrimitiveKind`] the value came from, and none of them has to.
//! `serde_json::Value` uses it for exactly one decision — a 64-bit width
//! becomes a decimal string, because a JSON number is a double and loses
//! the low bits past 2^53 — and could have made that decision from the
//! magnitude instead. It does not, because a form that changed shape at
//! 2^53 would compare unequal across the boundary, and because a
//! representation that stores an `int16` differently from an `int32`
//! must be *told* which it has rather than left to infer it from a value
//! that fits both.
//!
//! This is the hazard the module note in [`crate`] names, in its
//! constructor form: a coordinate the current implementation happens not
//! to need is still a coordinate, and dropping it from the signature is
//! what makes the next implementation guess.

use crate::encode::ProbeError;
use std::string::String;
use std::vec::Vec;
use zerodds_ast::PrimitiveKind;
use zerodds_float128::Float128;

/// A value the read direction can be built into.
///
/// `Clone` because Table 9's default for an array is "the same
/// dimensions and same element type whose elements take the default
/// value for their corresponding type": the default of a type does not
/// depend on the index, so one element is built and cloned rather than
/// the model walked once per element — a 4096-element array of a nested
/// structure would otherwise be 4096 identical walks.
pub trait Sink: Sized + Clone {
    /// `true` or `false`.
    #[must_use]
    fn boolean(value: bool) -> Self;

    /// An integer of `kind`.
    ///
    /// `value` is already inside `kind`'s range: the reader took it out
    /// of a field of that width, and [`crate::canonical`] put it through
    /// [`crate::contract::integer_for`] first.
    ///
    /// # Errors
    /// [`ProbeError::Mismatch`] where the representation cannot carry
    /// the value, naming `path`.
    fn integer(kind: PrimitiveKind, value: i128, path: &str) -> Result<Self, ProbeError>;

    /// A floating value of `kind`.
    ///
    /// # Errors
    /// [`ProbeError::Mismatch`] where the representation cannot write
    /// the value — JSON has no NaN and no infinity — naming `path`.
    fn float(kind: PrimitiveKind, value: f64, path: &str) -> Result<Self, ProbeError>;

    /// A `long double`, exactly.
    #[must_use]
    fn float128(value: Float128) -> Self;

    /// One character of `kind`.
    #[must_use]
    fn character(kind: PrimitiveKind, value: char) -> Self;

    /// A `string` or a `wstring`.
    ///
    /// Also the loose form of a `@default` literal that has not met its
    /// type yet — an enumerator's name, a character, a decimal integer —
    /// which [`crate::canonical::canonicalise`] then reads back through
    /// [`crate::source::Source`] and rebuilds at the member's own type.
    /// That round trip is why the loose form has to be readable and not
    /// only storable.
    #[must_use]
    fn text(value: String) -> Self;

    /// A `fixed<P, S>`, in the canonical decimal
    /// `crate::canonical::decimal` gives it.
    #[must_use]
    fn fixed(decimal: String) -> Self;

    /// The enumerator this value names.
    ///
    /// A name and never the number behind it: the number depends on
    /// `@value` and on the dialect's
    /// [`zerodds_ast_layout::EnumeratorValue`], and two peers that
    /// number their enumerators differently still mean the same
    /// enumerator.
    #[must_use]
    fn enumerator(name: String) -> Self;

    /// A bitmask, as the names of the bits it sets, in declaration
    /// order.
    #[must_use]
    fn bit_names(names: Vec<String>) -> Self;

    /// A bitset, as its named bitfields in declaration order.
    ///
    /// Anonymous bitfields are padding and are not among them.
    ///
    /// # Errors
    /// [`ProbeError::Mismatch`] where the representation cannot carry a
    /// field's value, naming `path`.
    fn bitfields(fields: Vec<(String, i128)>, path: &str) -> Result<Self, ProbeError>;

    /// A sequence, an array or a map.
    #[must_use]
    fn collection(items: Vec<Self>) -> Self;

    /// One entry of a map, as a key and a value.
    ///
    /// A pair and not a named field: a `map<long, float>` keys on
    /// something that is not a name, which §7.4.1.1.4 admits and a
    /// field-keyed form does not. The pair has to read back out through
    /// [`crate::source::Source::as_items`] in that order, because that
    /// is how a map's entries are put into one order for comparison.
    #[must_use]
    fn map_entry(key: Self, value: Self) -> Self;

    /// A structure, a valuetype or a union.
    #[must_use]
    fn aggregate(members: Aggregate<Self>) -> Self;

    /// A number no type governs — the value of a `@default` float
    /// literal, before the member's own width narrows it.
    ///
    /// # Errors
    /// [`ProbeError::Mismatch`] where the representation cannot write
    /// the value, naming `path`.
    fn number(value: f64, path: &str) -> Result<Self, ProbeError>;
}

/// An aggregate under construction.
///
/// The read direction does not build a structure in one go: a
/// `@mutable` type is driven by the headers the writer wrote, in
/// whatever order it wrote them, and the members the stream left out
/// take their defaults afterwards. So the value is assembled by name,
/// and the two things the assembly has to answer are whether a member
/// is already in — §7.4.1.2.3's defaulting asks it — and whether one
/// arrived twice, which is a stream this type does not describe.
///
/// Insertion order is preserved and a repeat replaces in place, which
/// is `serde_json::Map::insert`'s contract and the one the two
/// `@mutable` readers were written against.
pub struct Aggregate<V> {
    entries: Vec<(String, V)>,
}

impl<V> Default for Aggregate<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Aggregate<V> {
    /// A fresh, empty aggregate.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Whether a member of this name is already in.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|(known, _)| known == name)
    }

    /// Put a member in, answering with the one it replaced.
    ///
    /// `Some` is what says a stream carried the same member twice.
    pub fn insert(&mut self, name: String, value: V) -> Option<V> {
        for (known, held) in &mut self.entries {
            if *known == name {
                return Some(core::mem::replace(held, value));
            }
        }
        self.entries.push((name, value));
        None
    }

    /// The members, in the order they went in.
    #[must_use]
    pub fn into_entries(self) -> Vec<(String, V)> {
        self.entries
    }
}
