// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C column's encoder: [`crate::emit::encode`] bound to the C
//! backend.
//!
//! The walk moved to `emit/encode.rs` when it stopped being C's alone.
//! What is left here is the binding, and it is a module rather than four
//! turbofishes at each call site because the C generator asks for these
//! by name in twenty places and a `::<C>` at each is twenty chances to
//! reach for the wrong backend.
//!
//! What C forces that the walk does not decide:
//!
//! - **A pointer parameter.** A codec takes `const T *v` and reaches its
//!   members with `->`. Every target spells that its own way, and
//!   [`crate::emit::CodecTarget::member`] is where.
//! - **An array parameter decays.** An array codec takes the array
//!   itself, not its address: C adjusts an array parameter to a pointer
//!   to its first element, and `(*v)[i][j]` would otherwise need
//!   parentheses the tree has no node for. So a call site passes
//!   `v->grid` for an array and `&v->x` for everything else.
//! - **Prototypes ahead of bodies**, so a recursive or mutually
//!   recursive type does not depend on the order the walk visited its
//!   types in.

use crate::emit::c::C;
use crate::emit::encode;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{LayoutKey, Purpose};
use zerodds_lang_cst::c;

pub use crate::emit::encode::CodecOptions as CCodecOptions;

/// Build the C encoder for every type in the model, under one plan.
///
/// # Errors
/// As [`encode::codec`].
pub fn c_codec(
    ast: &Ast,
    key: LayoutKey,
    options: CCodecOptions,
) -> Result<Vec<c::Decl>, EncodeError> {
    encode::codec::<C>(ast, key, options)
}

/// The C encoder for the types one root reaches, and no others.
///
/// # Errors
/// As [`encode::codec_from`].
pub fn c_codec_from(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: CCodecOptions,
) -> Result<Vec<c::Decl>, EncodeError> {
    encode::codec_from::<C>(ast, root, key, options)
}

/// The C key holder for a keyed type, and nothing for one with no `@key`.
///
/// # Errors
/// As [`encode::key_holder`].
pub fn c_key_holder(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Option<c::Decl>, EncodeError> {
    encode::key_holder::<C>(ast, root, key)
}

/// The name of the C encoder function for a type.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `encode::codec_name::<C>`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
#[must_use]
pub fn codec_name(ast: &Ast, id: Id<TypeNode>, purpose: Purpose) -> String {
    encode::codec_name::<C>(ast, id, purpose)
}

/// Whether the C codec generated for `id` may legitimately have no
/// caller. See `encode::maybe_unused`.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `encode::maybe_unused`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
#[must_use]
pub fn maybe_unused(ast: &Ast, id: Id<TypeNode>, key: LayoutKey) -> bool {
    encode::maybe_unused(ast, id, key)
}
