// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the write direction reads a value out of.
//!
//! [`Source`] is the accessors a representation has to offer and
//! nothing more. Every rule *about* a value — that a key has to name a
//! member, that `null` is neither a value nor an absence, that a bound
//! is enforced, that an `octet` of 300 is refused — lives in
//! [`crate::contract`] and is asked once for every representation.
//!
//! ## Where the line is
//!
//! Two questions are the representation's and not the contract's, and
//! both are on this trait rather than in `contract`:
//!
//! - **How an integer is spelled.** A JSON number is a double, so a
//!   64-bit integer written as one says a different number than its
//!   author typed and every reader of the file agrees on the wrong one.
//!   That is a fact about JSON, not about IDL, so [`Source::as_integer`]
//!   is where a representation states its own rule.
//! - **How a value renders.** [`Source::render`] is the one form two
//!   implementations' values can be compared in, and what "one form"
//!   means depends on what the values are written in.
//!
//! Everything else — the range of an `octet`, the width of a `char`,
//! the bound of a `sequence` — is the type's and is asked in one place.

use crate::encode::ProbeError;
use std::string::String;
use std::vec::Vec;
use zerodds_float128::Float128;

/// A value the write direction can be executed against.
///
/// The accessors answer `None` where the value is not of that shape;
/// the refusal, with its path, is [`crate::contract`]'s to build.
pub trait Source: Sized {
    /// A name for what this value is, for a refusal that says what was
    /// there rather than only what was wanted.
    fn kind(&self) -> &'static str;

    /// Whether this value is the representation's explicit nothing.
    ///
    /// Never a value and never an absence — see
    /// [`crate::contract::reject_null`]. A representation with no such
    /// thing answers `false` and the rule costs it nothing.
    fn is_null(&self) -> bool;

    /// Whether this value is an aggregate with named fields.
    ///
    /// Asked before [`Self::entries`] so that a walk over a
    /// hundred-member sample does not build a list to find out.
    fn is_object(&self) -> bool;

    /// `true` or `false`.
    fn as_bool(&self) -> Option<bool>;

    /// The text this value carries.
    fn as_text(&self) -> Option<&str>;

    /// The floating value this value carries.
    fn as_float(&self) -> Option<f64>;

    /// The elements of a sequence, an array or a map.
    fn as_items(&self) -> Option<&[Self]>;

    /// One named field, or `None` where the value has no such field —
    /// including where it is not an aggregate at all.
    fn field(&self, name: &str) -> Option<&Self>;

    /// Every named field, in the order the representation holds them.
    ///
    /// Empty for a value that is not an aggregate, which
    /// [`Self::is_object`] is what tells apart.
    fn entries(&self) -> Vec<(&str, &Self)>;

    /// The integer this value carries, exactly.
    ///
    /// A representation that cannot carry every integer exactly refuses
    /// the spellings it cannot, here, naming what it wants instead. It
    /// is not [`crate::contract`]'s rule to state: the range an IDL
    /// primitive admits is the type's business and is checked in
    /// [`crate::contract::integer_for`], which asks this first.
    ///
    /// # Errors
    /// [`ProbeError::Mismatch`], naming `path`.
    fn as_integer(&self, path: &str) -> Result<i128, ProbeError>;

    /// The `long double` this value carries, exactly.
    ///
    /// Separate from [`Self::as_float`] for the reason
    /// [`Self::as_integer`] is separate from a number: sixty of a binary128
    /// significand's 113 bits do not survive a double, so a
    /// representation whose only number is one has to accept the value
    /// in some other form and say so here.
    ///
    /// # Errors
    /// [`ProbeError::Mismatch`], naming `path`.
    fn as_float128(&self, path: &str) -> Result<Float128, ProbeError>;

    /// The one form two implementations' values can be compared in.
    ///
    /// A pairwise run compares what came out of B against what went
    /// into A, and a representation usually has more than one way to
    /// write the same value. This is the one it writes for comparison —
    /// keys ordered, no incidental whitespace, numbers in one spelling.
    /// [`crate::canonical::canonicalise`] is what puts a value into the
    /// form this then renders; rendering an uncanonicalised value is
    /// still defined and still comparable, which is what
    /// [`crate::decode()`] needs to order a map's entries.
    fn render(&self) -> String;
}

/// The named fields of a value that has been checked to have them.
///
/// A borrow that carries the check with it: [`crate::contract::object`]
/// is the only thing that builds one, so a walk cannot ask an aggregate
/// question of a value nobody established was an aggregate. That is what
/// the concrete `&serde_json::Map<String, Value>` used to do, kept
/// through the generalisation rather than traded for a bare `&V`.
pub struct Fields<'a, V>(&'a V);

impl<V> Clone for Fields<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for Fields<'_, V> {}

impl<'a, V: Source> Fields<'a, V> {
    /// Wrap a value [`crate::contract::object`] has established is one.
    pub(crate) const fn of(value: &'a V) -> Self {
        Self(value)
    }

    /// The value itself, for a walk that needs it back.
    #[must_use]
    pub const fn value(self) -> &'a V {
        self.0
    }

    /// One field by name.
    #[must_use]
    pub fn get(self, name: &str) -> Option<&'a V> {
        self.0.field(name)
    }

    /// Every field's name, in the order the representation holds them.
    #[must_use]
    pub fn names(self) -> Vec<&'a str> {
        self.0.entries().into_iter().map(|(name, _)| name).collect()
    }
}

impl<'a, V: Source> IntoIterator for Fields<'a, V> {
    type Item = (&'a str, &'a V);
    type IntoIter = std::vec::IntoIter<(&'a str, &'a V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.entries().into_iter()
    }
}
