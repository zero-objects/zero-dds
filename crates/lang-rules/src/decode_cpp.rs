// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ column's decoder: [`crate::emit::decode`] bound to the C++
//! backend.
//!
//! The counterpart of [`crate::decode_c`], and the same shape: the walk
//! is shared and what is left here is the binding plus the one step the
//! generic walk cannot take — prepending the Table 9 defaulters.
//!
//! ## Why the defaulters are here and not in the walk
//!
//! Which types need one is read off the decoders that were generated,
//! and a target that wrote its defaults as literals would need no such
//! pass at all. So the pass belongs to the column. C++ needs it for the
//! same reason C does, and `crate::default_cpp` says why it is not
//! `T{}`.

use crate::emit::cpp::Cpp;
use crate::emit::decode;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::LayoutKey;
use zerodds_lang_cst::cpp;

pub use crate::emit::encode::CodecOptions as CppCodecOptions;

/// The name of the C++ decoder function for a type.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `decode::decoder_name::<Cpp>`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
#[must_use]
pub fn decoder_name(ast: &Ast, id: Id<TypeNode>) -> String {
    decode::decoder_name::<Cpp>(ast, id)
}

/// Build the C++ decoder for every type in the model, under one plan.
///
/// # Errors
/// As [`decode::decoder`], and when a Table 9 defaulter cannot be built.
pub fn cpp_decoder(
    ast: &Ast,
    key: LayoutKey,
    options: CppCodecOptions,
) -> Result<Vec<cpp::Decl>, EncodeError> {
    with_defaulters(ast, decode::decoder::<Cpp>(ast, key, options)?)
}

/// The C++ decoder for the types one root reaches, and no others.
///
/// # Errors
/// As [`cpp_decoder`].
pub fn cpp_decoder_from(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: CppCodecOptions,
) -> Result<Vec<cpp::Decl>, EncodeError> {
    with_defaulters(ast, decode::decoder_from::<Cpp>(ast, root, key, options)?)
}

/// The C++ decoder for the types several roots reach, each function once.
///
/// # Errors
/// As [`cpp_decoder`].
pub fn cpp_decoder_from_many(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    key: LayoutKey,
    options: CppCodecOptions,
) -> Result<Vec<cpp::Decl>, EncodeError> {
    with_defaulters(
        ast,
        decode::decoder_from_many::<Cpp>(ast, roots, key, options)?,
    )
}

/// [`cpp_decoder_from_many`] without the types in `excluded`.
///
/// # Errors
/// As [`cpp_decoder`].
pub fn cpp_decoder_from_many_excluding(
    ast: &Ast,
    roots: &[Id<TypeNode>],
    key: LayoutKey,
    options: CppCodecOptions,
    excluded: &[Id<TypeNode>],
) -> Result<Vec<cpp::Decl>, EncodeError> {
    let decoders = decode::decoder_from_many_excluding::<Cpp>(ast, roots, key, options, excluded)?;
    let mut out = crate::default_cpp::cpp_defaulters_called_by_excluding(ast, &decoders, excluded)?;
    out.extend(decoders);
    Ok(out)
}

/// The Table 9 defaulters, ahead of the decoders that call them.
///
/// `cpp_defaulters_called_by` emits one only for a type a value can
/// actually go missing at, so nothing it returns is a `static` function
/// with no caller — which the probe compiles as an error under
/// `-Wunused-function -Werror`.
fn with_defaulters(ast: &Ast, decoders: Vec<cpp::Decl>) -> Result<Vec<cpp::Decl>, EncodeError> {
    let mut out = crate::default_cpp::cpp_defaulters_called_by(ast, &decoders)?;
    out.extend(decoders);
    Ok(out)
}
