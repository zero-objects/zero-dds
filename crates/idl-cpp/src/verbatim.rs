// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim`-Codegen-Hook (XTypes 1.3 §7.2.2.4.8 + IDL 4.2 §8.3.5.1).
//!
//! `@verbatim(language="c++", text="...", placement=BEFORE_DECLARATION)`
//! laesst Anwender literal-Text in den C++-Output einbetten. Diese
//! Bridge filtert pro `Annotation`-Liste auf C++-relevante Sprach-Tags
//! und emittiert Text fuer ein gegebenes [`PlacementKind`].
//!
//! Akzeptierte Sprach-Tags: `c++`, `cpp`, `cxx`, plus `*` (Wildcard).

use std::fmt::Write;

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{PlacementKind, lower_annotations};

use crate::error::CppGenError;

/// C++-Codegen-Sprach-Aliase fuer `@verbatim(language="...")`.
pub(crate) const CXX_LANG_ALIASES: &[&str] = &["c++", "cpp", "cxx"];

fn fmt_err(_e: std::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

/// Emittiert alle `@verbatim(language="c++"|"*", placement=<placement>)`-
/// Bloecke aus `anns` mit der `indent`-Praefix.
///
/// Reihenfolge folgt der Source-Reihenfolge der Annotations.
/// Mehrzeilige Texte werden zeilenweise ausgegeben (Indent jeder Zeile).
pub(crate) fn emit_verbatim_at(
    out: &mut String,
    indent: &str,
    anns: &[Annotation],
    placement: PlacementKind,
) -> Result<(), CppGenError> {
    let Ok(lowered) = lower_annotations(anns) else {
        return Ok(());
    };
    for v in lowered.verbatims_for_language(CXX_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            writeln!(out, "{indent}{line}").map_err(fmt_err)?;
        }
    }
    Ok(())
}
