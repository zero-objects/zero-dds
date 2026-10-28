// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a value file means, in one place — at `serde_json::Value`.
//!
//! `zerodds_wire_codec::contract` states the rules over any
//! representation; this pins every one of them to the value file's own,
//! so that `fill`, `fill_cpp`, `fill_ros`, the three CORBA emitters and
//! the dumps go on calling `contract::text(value, path)` and get the
//! same answer `encode` gets. A backend that reached for the trait
//! method instead of the rule would be re-deriving the rule.
//!
//! Each wrapper has the signature its predecessor had, and `object`
//! answers with [`Fields`] where it used to answer with
//! `&serde_json::Map<String, Value>` — the same "this was checked to be
//! an aggregate" borrow, spelled without naming the map type.
//!
//! ## Why every function here carries a recursion marker
//!
//! None of them recurses. `dds_bounded_recursion` builds its call graph
//! from the **last segment** of a path call — its own note says so — so
//! `zerodds_wire_codec::contract::object(value, path)` inside `object`
//! reads as a call to `object`. Nineteen wrappers, nineteen findings,
//! and the same answer for each: depth 1, the callee is in another
//! crate, and the recursion the lint is looking for is over there. The
//! markers are per function because the marker is per function; the
//! reason is here so it is written once.

use serde_json::Value;
use zerodds_ast::{Ast, Bound, PrimitiveKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_float128::Float128;
use zerodds_wire_codec::encode::ProbeError;

pub use zerodds_wire_codec::contract::{
    Case, Consumed, MAX_ALIAS_HOPS, Member, UnevaluatedLabel, array_dimensions, bitfield_names,
    discriminator_c_width, join, members, reject_alias_cycle, resolve_alias, root_path,
    union_cases, union_members, union_selection,
};
pub use zerodds_wire_codec::source::Fields;

/// The named fields of a value file's object. See
/// [`zerodds_wire_codec::contract::object`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::object`, and the lint reads
/// a path call by its last segment. See the module note.
#[inline]
pub fn object<'a>(value: &'a Value, path: &str) -> Result<Fields<'a, Value>, ProbeError> {
    zerodds_wire_codec::contract::object(value, path)
}

/// See [`zerodds_wire_codec::contract::array`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::array`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], ProbeError> {
    zerodds_wire_codec::contract::array(value, path)
}

/// See [`zerodds_wire_codec::contract::text`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::text`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, ProbeError> {
    zerodds_wire_codec::contract::text(value, path)
}

/// See [`zerodds_wire_codec::contract::boolean`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::boolean`, and the lint reads
/// a path call by its last segment. See the module note.
#[inline]
pub fn boolean(value: &Value, path: &str) -> Result<bool, ProbeError> {
    zerodds_wire_codec::contract::boolean(value, path)
}

/// See [`zerodds_wire_codec::contract::float`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::float`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn float(value: &Value, path: &str) -> Result<f64, ProbeError> {
    zerodds_wire_codec::contract::float(value, path)
}

/// See [`zerodds_wire_codec::contract::float128`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::float128`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn float128(value: &Value, path: &str) -> Result<Float128, ProbeError> {
    zerodds_wire_codec::contract::float128(value, path)
}

/// See [`zerodds_wire_codec::contract::character`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::character`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn character(value: &Value, path: &str) -> Result<char, ProbeError> {
    zerodds_wire_codec::contract::character(value, path)
}

/// See [`zerodds_wire_codec::contract::character_for`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::character_for`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn character_for(kind: PrimitiveKind, value: &Value, path: &str) -> Result<char, ProbeError> {
    zerodds_wire_codec::contract::character_for(kind, value, path)
}

/// See [`zerodds_wire_codec::contract::integer`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::integer`, and the lint reads
/// a path call by its last segment. See the module note.
#[inline]
pub fn integer(value: &Value, path: &str) -> Result<i128, ProbeError> {
    zerodds_wire_codec::contract::integer(value, path)
}

/// See [`zerodds_wire_codec::contract::integer_for`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::integer_for`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn integer_for(kind: PrimitiveKind, value: &Value, path: &str) -> Result<i128, ProbeError> {
    zerodds_wire_codec::contract::integer_for(kind, value, path)
}

/// See [`zerodds_wire_codec::contract::enumerator`].
///
/// # Errors
/// [`ProbeError::Mismatch`] or [`ProbeError::Unknown`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::enumerator`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn enumerator(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<(String, i128), ProbeError> {
    zerodds_wire_codec::contract::enumerator(ast, type_, value, path)
}

/// See [`zerodds_wire_codec::contract::bit_names`].
///
/// # Errors
/// [`ProbeError::Mismatch`] or [`ProbeError::Unknown`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::bit_names`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn bit_names(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<Vec<(String, u16)>, ProbeError> {
    zerodds_wire_codec::contract::bit_names(ast, type_, value, path)
}

/// See [`zerodds_wire_codec::contract::bounded_items`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::bounded_items`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn bounded_items<'a>(
    ast: &Ast,
    value: &'a Value,
    bound: Bound,
    noun: &str,
    path: &str,
) -> Result<&'a [Value], ProbeError> {
    zerodds_wire_codec::contract::bounded_items(ast, value, bound, noun, path)
}

/// See [`zerodds_wire_codec::contract::map_entry`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::map_entry`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn map_entry<'a>(entry: &'a Value, path: &str) -> Result<(&'a Value, &'a Value), ProbeError> {
    zerodds_wire_codec::contract::map_entry(entry, path)
}

/// See [`zerodds_wire_codec::contract::member_value`].
///
/// # Errors
/// [`ProbeError::Missing`] or [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::member_value`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn member_value<'a>(
    object: Fields<'a, Value>,
    member: &Member,
    path: &str,
) -> Result<Option<&'a Value>, ProbeError> {
    zerodds_wire_codec::contract::member_value(object, member, path)
}

/// See [`zerodds_wire_codec::contract::reject_null`].
///
/// # Errors
/// [`ProbeError::Mismatch`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::reject_null`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn reject_null(value: &Value, path: &str) -> Result<(), ProbeError> {
    zerodds_wire_codec::contract::reject_null(value, path)
}

/// See [`zerodds_wire_codec::contract::reject_unknown_keys`].
///
/// # Errors
/// [`ProbeError::Unknown`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::reject_unknown_keys`, and
/// the lint reads a path call by its last segment. See the module note.
#[inline]
pub fn reject_unknown_keys(
    object: Fields<'_, Value>,
    declared: &[String],
    path: &str,
) -> Result<(), ProbeError> {
    zerodds_wire_codec::contract::reject_unknown_keys(object, declared, path)
}

/// See [`zerodds_wire_codec::contract::discriminator`].
///
/// # Errors
/// [`ProbeError::Missing`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::discriminator`, and the lint
/// reads a path call by its last segment. See the module note.
#[inline]
pub fn discriminator<'a>(object: Fields<'a, Value>, path: &str) -> Result<&'a Value, ProbeError> {
    zerodds_wire_codec::contract::discriminator(object, path)
}

/// See [`zerodds_wire_codec::contract::kind_of`].
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::contract::kind_of`, and the lint reads
/// a path call by its last segment. See the module note.
#[must_use]
#[inline]
pub fn kind_of(value: &Value) -> &'static str {
    zerodds_wire_codec::contract::kind_of(value)
}
