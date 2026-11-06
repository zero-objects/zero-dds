// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-idl-compose` (`src/model.rs`):
//! `compose_source(path, text, &PathLoader::bare())` on the IDL a
//! description stands for, `Composed::failed`, `Composed::diagnostics`
//! (each one's `message`) for the refusal, and `Composed::ast`.
//!
//! Oracle: IDL 4.2 §7.2.3 (identifiers that differ only in case collide),
//! §7.2.3.1 (an escaped identifier `_in` is the identifier `in`), §7.4.15
//! (an annotation applied with its parameter).

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

fn compose(text: &str) -> zerodds_idl_compose::Composed {
    zerodds_idl_compose::compose_source(
        "derived.idl",
        text,
        &zerodds_idl_compose::PathLoader::bare(),
    )
}

#[test]
fn idl_4_2_7_2_3_1_an_escaped_keyword_is_the_identifier() {
    let composed = compose(
        "module t { struct Inner { long x; }; }; \
         module t { @final struct S { @key(TRUE) ::t::Inner who; @optional(TRUE) ::t::Inner _in; }; };",
    );
    assert!(
        !composed.failed(),
        "{:?}",
        composed
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
    let ast = &composed.ast;
    let (s, _) = ast.types().find(|(_, node)| node.name == "S").unwrap();
    let names: Vec<String> = ast
        .members_of(s)
        .iter()
        .map(|id| ast.member(*id).unwrap().name.clone())
        .collect();
    assert_eq!(names, ["who", "in"]);
}

#[test]
fn idl_4_2_7_2_3_a_collision_fails_with_a_diagnostic() {
    let composed = compose("module t { struct S { long a; long A; }; };");
    assert!(composed.failed());
    assert!(
        composed.diagnostics.iter().any(|d| !d.message.is_empty()),
        "the refusal has a message"
    );
}
