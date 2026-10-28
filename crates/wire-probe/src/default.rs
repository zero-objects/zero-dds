// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a member takes when the stream does not carry it — at
//! `serde_json::Value`.
//!
//! Table 9 and §7.2.2.4.4.4.9 live in
//! `zerodds_wire_codec::default`; this pins both entry points to the
//! value file's own representation.
//!
//! Both carry a recursion marker for the reason `contract.rs`'s module
//! note gives: `dds_bounded_recursion` reads a path call by its last
//! segment, so forwarding to the function of the same name in the other
//! crate looks like recursion. Depth 1 each.

use serde_json::Value;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_wire_codec::encode::ProbeError;

/// The default value of one member: its `@default`, or its type's. See
/// [`zerodds_wire_codec::default::member`].
///
/// # Errors
/// [`ProbeError`] when the type has no default this project can name, or
/// when a `@default` and the member's type disagree.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::default::member`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn member(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    path: &str,
) -> Result<Value, ProbeError> {
    zerodds_wire_codec::default::member(ast, node, type_, path)
}

/// Table 9's default for a type. See
/// [`zerodds_wire_codec::default::of_type`].
///
/// # Errors
/// [`ProbeError`] for a type kind with no default.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::default::of_type`, and the lint reads
/// a path call by its last segment. See the module note.
#[inline]
pub fn of_type(ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<Value, ProbeError> {
    zerodds_wire_codec::default::of_type(ast, type_, path)
}
