// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim`-Codegen-Hook (XTypes 1.3 §7.2.2.4.8 + IDL 4.2 §8.3.5.1).
//!
//! `@verbatim(language="rust", text="...", placement=BEFORE_DECLARATION)`
//! lets users embed literal text into the Rust output. This bridge filters
//! per `Annotation` list for Rust-relevant language tags and emits the text
//! for a given [`PlacementKind`]. Mirrors `idl-cpp/src/verbatim.rs`.
//!
//! Accepted language tags: `rust`, `rs`, plus `*` (wildcard).

use zerodds_idl::ast::Annotation;
use zerodds_idl::semantics::annotations::{PlacementKind, lower_annotations};

/// Rust codegen language aliases for `(language="...")`.
pub(crate) const RUST_LANG_ALIASES: &[&str] = &["rust", "rs"];

/// Emits all `(language="rust"|"rs"|"*", placement=<placement>)` blocks
/// from `anns` with the `indent` prefix.
///
/// Order follows the source order of the annotations. Multi-line texts are
/// output line by line (each line indented). Verbatim text is the user's
/// responsibility — it is spliced in unmodified.
pub(crate) fn emit_verbatim_at(
    out: &mut String,
    indent: &str,
    anns: &[Annotation],
    placement: PlacementKind,
) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(RUST_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            out.push_str(indent);
            out.push_str(line);
            out.push('\n');
        }
    }
}
