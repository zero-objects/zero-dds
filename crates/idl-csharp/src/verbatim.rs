// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` codegen hook (XTypes 1.3 §7.2.2.4.8 + IDL 4.2 §8.3.5.1).
//!
//! `@verbatim(language="csharp", text="...", placement=BEFORE_DECLARATION)`
//! lets the user embed literal text into the C# output.
//!
//! Akzeptierte Sprach-Tags: `c#`, `csharp`, `cs`, plus `*` (Wildcard).

use std::fmt::Write;

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{PlacementKind, lower_annotations};

use crate::error::CsGenError;

/// C# codegen language aliases for `@verbatim(language="...")`.
pub(crate) const CSHARP_LANG_ALIASES: &[&str] = &["c#", "csharp", "cs"];

fn fmt_err(_e: std::fmt::Error) -> CsGenError {
    CsGenError::Internal("string formatting failed".into())
}

/// Emits all `@verbatim(language="c#"|"*", placement=<placement>)`
/// blocks from `anns` with the `indent` prefix.
pub(crate) fn emit_verbatim_at(
    out: &mut String,
    indent: &str,
    anns: &[Annotation],
    placement: PlacementKind,
) -> Result<(), CsGenError> {
    let Ok(lowered) = lower_annotations(anns) else {
        return Ok(());
    };
    for v in lowered.verbatims_for_language(CSHARP_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            writeln!(out, "{indent}{line}").map_err(fmt_err)?;
        }
    }
    Ok(())
}
