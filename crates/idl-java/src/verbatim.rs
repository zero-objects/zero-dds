// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim`-Codegen-Hook (XTypes 1.3 §7.2.2.4.8 + IDL 4.2 §8.3.5.1).
//!
//! `@verbatim(language="java", text="...", placement=BEFORE_DECLARATION)`
//! laesst Anwender literal-Text in den Java-Output einbetten.
//!
//! Akzeptierte Sprach-Tags: `java`, plus `*` (Wildcard).

use std::fmt::Write;

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{PlacementKind, lower_annotations};

use crate::error::JavaGenError;

/// Java-Codegen-Sprach-Aliase fuer `@verbatim(language="...")`.
pub(crate) const JAVA_LANG_ALIASES: &[&str] = &["java"];

fn fmt_err(_e: std::fmt::Error) -> JavaGenError {
    JavaGenError::Internal("string formatting failed".into())
}

/// Emittiert alle `@verbatim(language="java"|"*", placement=<placement>)`-
/// Bloecke aus `anns` mit dem `indent`-Praefix.
pub(crate) fn emit_verbatim_at(
    out: &mut String,
    indent: &str,
    anns: &[Annotation],
    placement: PlacementKind,
) -> Result<(), JavaGenError> {
    let Ok(lowered) = lower_annotations(anns) else {
        return Ok(());
    };
    for v in lowered.verbatims_for_language(JAVA_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            writeln!(out, "{indent}{line}").map_err(fmt_err)?;
        }
    }
    Ok(())
}
