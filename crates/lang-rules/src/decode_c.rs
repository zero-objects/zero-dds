// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C column's decoder: [`crate::emit::decode`] bound to the C
//! backend.
//!
//! The walk moved to `emit/decode.rs` when it stopped being C's alone,
//! the same as the encoder's did. What is left here is the binding, and
//! the one step the generic walk cannot take: prepending the Table 9
//! defaulters.
//!
//! ## Why the defaulters are here and not in the walk
//!
//! A member the stream did not carry takes Table 9's value, and in C
//! that means a generated function per type: a struct-valued default is
//! not an expression C can assign. Which types need one is read off the
//! decoders that were generated — `c_defaulters_called_by` looks at
//! them — and a target that writes its defaults as literals needs no
//! such pass at all. So the pass belongs to the column, not to the walk.

use crate::emit::c::C;
use crate::emit::decode;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::LayoutKey;
use zerodds_lang_cst::c;

pub use crate::emit::encode::CodecOptions as CCodecOptions;

/// The name of the C decoder function for a type.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `decode::decoder_name::<C>`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
#[must_use]
pub fn decoder_name(ast: &Ast, id: Id<TypeNode>) -> String {
    decode::decoder_name::<C>(ast, id)
}

/// Build the C decoder for every type in the model, under one plan.
///
/// # Errors
/// As [`decode::decoder`], and when a Table 9 defaulter cannot be built.
pub fn c_decoder(
    ast: &Ast,
    key: LayoutKey,
    options: CCodecOptions,
) -> Result<Vec<c::Decl>, EncodeError> {
    with_defaulters(ast, decode::decoder::<C>(ast, key, options)?)
}

/// The C decoder for the types one root reaches, and no others.
///
/// # Errors
/// As [`c_decoder`].
pub fn c_decoder_from(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: CCodecOptions,
) -> Result<Vec<c::Decl>, EncodeError> {
    with_defaulters(ast, decode::decoder_from::<C>(ast, root, key, options)?)
}

/// The Table 9 defaulters, ahead of the decoders that call them.
///
/// `c_defaulters_called_by` emits one only for a type some member can
/// actually go missing at, so nothing it returns is a `static` function
/// with no caller — which the probe compiles as an error.
fn with_defaulters(ast: &Ast, decoders: Vec<c::Decl>) -> Result<Vec<c::Decl>, EncodeError> {
    let mut out = crate::default_c::c_defaulters_called_by(ast, &decoders)?;
    out.extend(decoders);
    Ok(out)
}
