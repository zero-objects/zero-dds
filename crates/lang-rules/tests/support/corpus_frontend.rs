// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Corpus gates use the same admissibility checks as the production frontend.
//!
//! Low-level adapter fixtures intentionally construct partial models. Corpus
//! compiler checks do not: sending an inadmissible by-value cycle to a language
//! compiler measures a path that the production composer refuses first.

/// Check corpus admission, failing on either direction of expectation drift.
pub fn accepts(source: &str, origin: &str) -> bool {
    let path = std::path::Path::new(origin);
    let case = if path.file_name().is_some_and(|name| name == "type.idl") {
        path.parent().and_then(std::path::Path::file_name)
    } else {
        path.file_stem()
    }
    .and_then(std::ffi::OsStr::to_str);
    let expected_refusal = matches!(
        case,
        Some("recursive-array" | "resolve-module-named-as-type")
    );
    let composed = zerodds_idl_compose::compose_source(
        origin,
        source,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert_eq!(
        composed.failed(),
        expected_refusal,
        "frontend admission changed for {origin}: {:?}",
        composed.diagnostics,
    );
    if composed.failed() {
        eprintln!("FRONTEND_REFUSED {origin}: {:?}", composed.diagnostics);
        false
    } else {
        true
    }
}
