// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two readings of an operation with nothing to carry in the Enhanced
//! Service Mapping, selected by the generator's `ServiceMapping`:
//! the `dummy` member of §7.5.1.1.4 rule 3c, which §7.5.1.2.2 takes over and
//! §7.5.1.2.6's example shows (the default), or the empty structure marked
//! `@Empty` (§7.5.1.2.1.3).
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::emit::encode::CodecOptions;
use zerodds_lang_rules::generate::{ServiceMapping, source_with_mapping};

fn rust(mapping: ServiceMapping) -> String {
    let parsed = zerodds_idl_parse::parse("interface Calc { void ping(); long one(in long a); };");
    assert!(parsed.diagnostics().is_empty());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "empty.idl");
    assert!(lowered.diagnostics.is_empty());
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    source_with_mapping(
        &ast,
        Language::Rust,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
        CodecOptions::default(),
        mapping,
    )
    .unwrap_or_else(|error| panic!("{error:?}"))
}

fn body_of<'a>(text: &'a str, declaration: &str) -> &'a str {
    let at = text
        .find(declaration)
        .unwrap_or_else(|| panic!("no `{declaration}` in {text}"));
    let rest = &text[at..];
    let end = rest.find('}').expect("the end of the structure");
    &rest[..end]
}

#[test]
fn the_default_reading_gives_the_operation_a_dummy_member() {
    let text = rust(ServiceMapping::Enhanced);
    assert!(
        body_of(&text, "pub struct Calc_ping_In").contains("pub dummy: u8"),
        "{text}"
    );
    assert!(
        body_of(&text, "pub struct Calc_ping_Out").contains("pub dummy: u8"),
        "{text}"
    );
}

#[test]
fn the_empty_structure_reading_gives_it_no_member() {
    let text = rust(ServiceMapping::EnhancedEmptyStructures);
    assert!(
        !body_of(&text, "pub struct Calc_ping_In").contains("dummy"),
        "{text}"
    );
    assert!(
        !body_of(&text, "pub struct Calc_ping_Out").contains("dummy"),
        "{text}"
    );
    // The operation that has something to carry is unchanged.
    assert!(
        body_of(&text, "pub struct Calc_one_In").contains("pub a: i32"),
        "{text}"
    );
    // The client builds the empty structure without the member.
    assert!(!text.contains("dummy: 0"), "{text}");
}
