// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The arguments of an annotation, and which of their names resolve.
//!
//! IDL 4.2 §7.4.15.4.2: "in case an unknown annotation is encountered,
//! it should be ignored by the IDL-processing tools."
//!
//! Ignoring the annotation and then resolving its argument is not
//! ignoring it. An identifier written as an argument value names an
//! enumerator of the annotation's own type, which is in no scope of the
//! file, so the resolve pass broke the reference and refused the whole
//! translation unit. `@RTI_topic_kind(NO_KEY)` and
//! `@allowed_data_representation(XCDR2)` are the form vendor
//! annotations take, and every file carrying one was refused over it —
//! including `crates/idl-parse/tests/fixtures/spec_features/rti_connext_pragmas.idl`
//! in this tree.
//!
//! The same mechanism serves a second case that is not about unknown
//! annotations at all: a builtin whose parameter is an enumerated type
//! — `@autoid(HASH)`, `@extensibility(FINAL)`, and `@verbatim`'s
//! `placement`, which IDL 4.2 §8.3.5.1 declares as `PlacementKind
//! placement`. `verbatim` was missing from that list, so every
//! `@verbatim(placement = BEFORE_DECLARATION)` in the tree was refused
//! over the enumerator — including the RTI fixture above, which carries
//! both defects in one file.
//!
//! These tests sit here rather than in `zerodds-idl-lower` because the
//! diagnostic came from the resolve pass, which runs in
//! `zerodds-idl-compose`, one layer above that crate. Asserting them
//! against lowering alone passes whether or not the fix is there — the
//! first draft of these tests did exactly that, and a mutation proved
//! it.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

/// The error codes `check` reports for `source`.
fn errors(source: &str) -> Vec<String> {
    let composed = zerodds_idl_compose::compose_source(
        "test.idl",
        source,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    composed
        .diagnostics
        .iter()
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

#[test]
fn an_unknown_annotation_is_ignored_whatever_it_is_given() {
    for source in [
        "@nosuchthing struct Sa { long m; };",
        "@nosuchthing(v = 1) struct Sa { long m; };",
        "@nosuchthing(v = SOME_NAME) struct Sa { long m; };",
        "@nosuchthing(SOME_NAME) struct Sa { long m; };",
        "@RTI_topic_kind(NO_KEY) struct Sa { long m; };",
        "@allowed_data_representation(XCDR2) struct Sa { long m; };",
        "@RTPS_data_representation(XCDR2) struct Sa { long m; };",
        "@RTI_resource_limits(initial_count=10, max_count=100) struct Sa { long m; };",
        // The Fast DDS spelling of a key, which this front end does not
        // act on and so must ignore.
        "struct Sa { @Key long m; };",
        // On a member and on an enumerator, not only on a type.
        "struct Sa { @RTI_something(SOME_NAME) long m; };",
        "enum Ea { @RTI_something(SOME_NAME) EA1, EA2 };",
    ] {
        let found = errors(source);
        assert!(
            found.is_empty(),
            "{source:?}: §7.4.15.4.2 says ignore it, got {found:?}"
        );
    }
}

#[test]
fn a_name_outside_an_unknown_annotation_still_has_to_resolve() {
    // The half that keeps the rule above from becoming "identifiers are
    // never resolved". Ignoring an unknown annotation is not licence to
    // stop reading the file, and a *declared* annotation's arguments are
    // not covered by the ignore rule at all — it applies to unknown ones.
    for source in [
        "struct Sa { NoSuchType m; };",
        "const long K = NOPE;",
        "@annotation Aa { long v; };\n@Aa(NOPE) struct Sa { long m; };",
        "typedef NoSuchType Ali;",
        "struct Sa { sequence<NoSuchType> m; };",
    ] {
        let found = errors(source);
        assert!(
            found.iter().any(|d| d.starts_with("IDL0010")),
            "{source:?} should not resolve, got {found:?}"
        );
    }
}

#[test]
fn a_literal_argument_of_an_unknown_annotation_still_reaches_the_model() {
    // Only an *identifier* has nothing in the file to resolve to. A
    // literal is lowered either way, so an unknown annotation that
    // carries one keeps it — dropping every argument would have been a
    // second silence in place of the first.
    let composed = zerodds_idl_compose::compose_source(
        "test.idl",
        "@nosuchthing(v = 7) struct Sa { long m; };",
        &zerodds_idl_compose::PathLoader::bare(),
    );
    let any = composed
        .ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![String::from(
            "Sa",
        )]))
        .expect("Sa");
    let applied = &composed.ast.facets(any).expect("facets").annotations;
    let kept = applied
        .iter()
        .find(|a| a.name == "nosuchthing")
        .expect("the annotation is kept whole");
    assert_eq!(kept.arguments.len(), 1, "{kept:?}");
    assert_eq!(
        composed.ast.eval_integral(kept.arguments[0].value).ok(),
        Some(7_i128),
        "the literal argument is still readable"
    );
}

#[test]
fn a_builtin_with_an_enumerated_parameter_reads_it_without_resolving_it() {
    // An enumerator of the annotation's own type is in no scope of the
    // file, so it must not be lowered as an expression — and the
    // annotation must still be read.
    for source in [
        "@autoid(HASH) struct Sa { long m; };",
        "@autoid(SEQUENTIAL) struct Sa { long m; };",
        "@extensibility(FINAL) struct Sa { long m; };",
        "@extensibility(MUTABLE) struct Sa { long m; };",
        "struct Sa { @try_construct(TRIM) string<4> m; };",
        "@verbatim(language=\"cpp\", placement=BEFORE_DECLARATION, text=\"// x\")\n\
         struct Sa { long m; };",
        "@verbatim(language=\"c\", placement=AFTER_DECLARATION, text=\"/* x */\")\n\
         struct Sa { long m; };",
    ] {
        let found = errors(source);
        assert!(found.is_empty(), "{source:?} was refused: {found:?}");
    }
}

#[test]
fn a_verbatim_directive_still_reaches_the_model_whole() {
    // Not resolving the enumerator must not mean losing it. All three
    // parameters are read out of the argument's text and identifier, so
    // the directive arrives complete.
    let composed = zerodds_idl_compose::compose_source(
        "test.idl",
        "@verbatim(language=\"cpp\", placement=BEFORE_DECLARATION, text=\"// x\")\n\
         struct Sa { long m; };",
        &zerodds_idl_compose::PathLoader::bare(),
    );
    let any = composed
        .ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![String::from(
            "Sa",
        )]))
        .expect("Sa");
    let verbatim = composed
        .ast
        .facets(any)
        .expect("facets")
        .xtypes
        .as_ref()
        .expect("an xtypes facet")
        .verbatim
        .clone();
    assert_eq!(verbatim.len(), 1, "{verbatim:?}");
    assert_eq!(verbatim[0].language, "cpp");
    assert_eq!(verbatim[0].text, "// x");
    assert_eq!(
        verbatim[0].placement,
        zerodds_ast::VerbatimPlacement::BeforeDeclaration
    );
}

#[test]
fn the_rti_fixture_that_carried_both_defects_now_passes() {
    // `crates/idl-parse/tests/fixtures/spec_features/rti_connext_pragmas.idl`
    // is real vendor IDL: `@verbatim(placement=BEFORE_DECLARATION)` and
    // `@RTI_topic_kind(NO_KEY)` in one file. It was refused over both
    // names, and it is the shape an actual user hands this tool.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = repo.join("crates/idl-parse/tests/fixtures/spec_features/rti_connext_pragmas.idl");
    let source = std::fs::read_to_string(&path).expect("the fixture");
    let found = errors(&source);
    assert!(found.is_empty(), "the RTI fixture was refused: {found:?}");
}

// -- an annotation body is its own scope (rule 221) ----------------------

#[test]
fn an_annotation_body_may_declare_the_types_its_members_use() {
    // Rule (221): `<annotation_body> ::= { <annotation_member> |
    // <enum_dcl> ";" | <const_dcl> ";" | <typedef_dcl> ";" }*`. The
    // enum, const and typedef were parsed and never lowered, which
    // nothing noticed while the members themselves were being dropped —
    // with the members read, a member's type had nowhere to resolve.
    for source in [
        // The enum form, declared and applied with nothing named from
        // it. Whether an enumerator declared *inside* an annotation body
        // may be written unqualified at an application site —
        // `@Mode(WRITE)` — is a question neither §7.5 nor rule (221)
        // settles and no fixture in this tree exercises, so there is no
        // assertion about it here.
        "@annotation Mode { enum Kind { READ, WRITE }; Kind kind default READ; };\n\
         @Mode struct Sa { long m; };",
        "@annotation Limited { const long MAX_LEN = 64; long len default 0; };\n\
         @Limited(8) struct Sa { long m; };",
        "@annotation Aliased { typedef long Id; Id ref default 0; };\n\
         @Aliased(3) struct Sa { long m; };",
        // An empty body is legal and declares nothing.
        "@annotation MyKey {};\n@MyKey struct Sa { long m; };",
    ] {
        let found = errors(source);
        assert!(found.is_empty(), "{source:?} was refused: {found:?}");
    }
}

#[test]
fn the_user_annotation_fixture_passes() {
    // `crates/idl-parse/tests/fixtures/spec_features/user_annotations.idl`
    // is the file that exercises rules 218-221 whole. It carries all
    // three body declaration forms and applications of each.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = repo.join("crates/idl-parse/tests/fixtures/spec_features/user_annotations.idl");
    let source = std::fs::read_to_string(&path).expect("the fixture");
    let found = errors(&source);
    assert!(found.is_empty(), "the fixture was refused: {found:?}");
}
