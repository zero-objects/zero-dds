// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java column's decoder: [`crate::emit::decode`] bound to the Java
//! backend.
//!
//! The counterpart of [`crate::decode_c`], and the same shape: the walk
//! is shared and what is left here is the binding plus the one step the
//! generic walk cannot take — prepending the Table 9 defaulters.

use crate::emit::decode;
use crate::emit::java::Java;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::LayoutKey;
use zerodds_lang_cst::java;

pub use crate::emit::encode::CodecOptions as JavaCodecOptions;

/// The name of the Java decoder function for a type.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `decode::decoder_name::<Java>`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call.
#[must_use]
pub fn decoder_name(ast: &Ast, id: Id<TypeNode>) -> String {
    decode::decoder_name::<Java>(ast, id)
}

/// Build the Java decoder for every type in the model, under one plan.
///
/// # Errors
/// As [`decode::decoder`], and when a Table 9 defaulter cannot be built.
pub fn java_decoder(
    ast: &Ast,
    key: LayoutKey,
    options: JavaCodecOptions,
) -> Result<Vec<java::Member>, EncodeError> {
    with_defaulters(ast, decode::decoder::<Java>(ast, key, options)?)
}

/// The Java decoder for the types one root reaches, and no others.
///
/// # Errors
/// As [`java_decoder`].
pub fn java_decoder_from(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
    options: JavaCodecOptions,
) -> Result<Vec<java::Member>, EncodeError> {
    with_defaulters(ast, decode::decoder_from::<Java>(ast, root, key, options)?)
}

/// The Table 9 defaulters, ahead of the decoders that call them.
///
/// `java_defaulters_called_by` emits one only for a type a value can
/// actually go missing at, so nothing it returns is a `static` function
/// with no caller — which the probe compiles as an error under
/// `-Xlint:all -Werror`.
fn with_defaulters(
    ast: &Ast,
    decoders: Vec<java::Member>,
) -> Result<Vec<java::Member>, EncodeError> {
    let mut out = crate::default_java::java_defaulters_called_by(ast, &decoders)?;
    out.extend(decoders);
    Ok(out)
}
