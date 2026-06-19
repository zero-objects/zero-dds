// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim`-Codegen-Hook (XTypes 1.3 §7.2.2.4.8 + IDL 4.2 §8.3.5.1).
//!
//! `@verbatim(language="c++", text="...", placement=BEFORE_DECLARATION)`
//! lets users embed literal text into the C++ output. This
//! bridge filters per `Annotation` list for C++-relevant language tags
//! and emits text for a given [`PlacementKind`].
//!
//! Akzeptierte Sprach-Tags: `c++`, `cpp`, `cxx`, plus `*` (Wildcard).

use std::fmt::Write;

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{PlacementKind, lower_annotations};

use crate::error::CppGenError;

/// C++ codegen language aliases for `(language="...")`.
pub(crate) const CXX_LANG_ALIASES: &[&str] = &["c++", "cpp", "cxx"];

fn fmt_err(_e: std::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

/// Emits all `(language="c++"|"*", placement=<placement>)`
/// blocks from `anns` with the `indent` prefix.
///
/// The order follows the source order of the annotations.
/// Multi-line texts are output line by line (each line indented).
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
