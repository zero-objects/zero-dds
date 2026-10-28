// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A node's `SourceFacet` names the file it was written in and the range
//! in that file, and the macros that wrote it.
//!
//! Lowering sees the expanded text: includes inlined, macros replaced,
//! comments and directives gone. It gave every node a range of that text
//! under the unit's path and an empty macro chain, so a reader of a node's
//! source — a lint, a generator naming a declaration — had to map it, and
//! one that did not pointed into the wrong file or the wrong line.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, SourceFacet, SourceOrigin};

const MAIN: &str = "// the unit\n#include \"inc.idl\"\n\
                    #define DECL(n) struct n { long x; };\n\
                    DECL(Made)\n\
                    struct Here { long y; };\n";
const INCLUDED: &str = "// an included file\n\nstruct Included { long z; };\n";

fn composed() -> Ast {
    let loader = zerodds_idl_parse::MemoryLoader::new().with("inc.idl", INCLUDED);
    let composed = zerodds_idl_compose::compose_source_with(
        "main.idl",
        MAIN,
        &loader,
        zerodds_idl_compose::ComposeOptions::default(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    composed.ast
}

fn source_of<'a>(ast: &'a Ast, name: &str) -> &'a SourceFacet {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(_, node)| &node.facets.source)
        .unwrap_or_else(|| panic!("{name} is not in the model"))
}

fn text<'a>(source: &'a str, facet: &SourceFacet) -> &'a str {
    let span = facet.span.expect("a range");
    &source[span.start as usize..span.end as usize]
}

#[test]
fn a_node_written_in_the_unit_is_in_the_units_offsets() {
    let ast = composed();
    let here = source_of(&ast, "Here");
    assert_eq!(
        here.origin,
        SourceOrigin::File {
            path: "main.idl".to_string()
        }
    );
    assert!(text(MAIN, here).contains("Here"), "{:?}", text(MAIN, here));
    assert!(here.macro_chain.is_empty());
}

#[test]
fn a_node_from_an_included_file_names_that_file() {
    let ast = composed();
    let included = source_of(&ast, "Included");
    assert_eq!(
        included.origin,
        SourceOrigin::File {
            path: "inc.idl".to_string()
        }
    );
    assert!(
        text(INCLUDED, included).contains("Included"),
        "{:?}",
        text(INCLUDED, included)
    );
}

#[test]
fn a_node_a_macro_wrote_points_at_the_invocation_and_names_the_macro() {
    let ast = composed();
    let made = source_of(&ast, "Made");
    assert_eq!(
        made.origin,
        SourceOrigin::File {
            path: "main.idl".to_string()
        }
    );
    assert!(
        text(MAIN, made).starts_with("DECL"),
        "{:?}",
        text(MAIN, made)
    );
    let outer = made.macro_chain.first().expect("the macro that wrote it");
    assert_eq!(outer.name, "DECL");
    assert_eq!(outer.path.as_deref(), Some("main.idl"));
    let at = outer.invoked_at.expect("where it was invoked");
    assert!(MAIN[at.start as usize..at.end as usize].starts_with("DECL"));
}

/// In a bundle every unit's nodes are placed with that unit's own map;
/// the next unit indexes another expanded text.
#[test]
fn a_bundle_places_each_units_nodes_in_its_own_file() {
    let a = "struct A { long x; };\n";
    let b = "// b\n\n#define WIDE long long\nstruct B { WIDE w; A inner; };\n";
    let composed = zerodds_idl_compose::compose_all_sources(
        &[
            ("a.idl".to_string(), a.to_string()),
            ("b.idl".to_string(), b.to_string()),
        ],
        &zerodds_idl_parse::MemoryLoader::new(),
        zerodds_idl_compose::ComposeOptions::default(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let in_a = source_of(&composed.ast, "A");
    let in_b = source_of(&composed.ast, "B");
    assert_eq!(
        in_a.origin,
        SourceOrigin::File {
            path: "a.idl".to_string()
        }
    );
    assert_eq!(
        in_b.origin,
        SourceOrigin::File {
            path: "b.idl".to_string()
        }
    );
    assert!(text(a, in_a).contains('A'), "{:?}", text(a, in_a));
    assert!(text(b, in_b).contains("struct B"), "{:?}", text(b, in_b));
}

/// A name that does not resolve is reported at the member that names it,
/// in its file, in a unit and in a bundle; it was reported at offset 0
/// of no file. A reference holds no range of its own, so the member's
/// declarator is the place, and the message names the missing type.
#[test]
fn a_name_that_does_not_resolve_is_reported_where_it_is_written() {
    let c = "// c\n\nstruct C { Missing m; };\n";
    let unresolved = |diagnostics: &[zerodds_idl_parse::Diagnostic]| {
        diagnostics
            .iter()
            .find(|d| d.code == zerodds_idl_parse::DiagnosticCode::UnresolvedName)
            .cloned()
            .expect("an unresolved-name diagnostic")
    };

    let unit = zerodds_idl_compose::compose_source_with(
        "c.idl",
        c,
        &zerodds_idl_parse::MemoryLoader::new(),
        zerodds_idl_compose::ComposeOptions::default(),
    );
    let found = unresolved(&unit.diagnostics);
    let span = found.primary;
    assert_eq!(&c[span.start as usize..span.end as usize], "m", "{found:?}");
    assert!(found.message.contains("Missing"), "{found:?}");

    let bundle = zerodds_idl_compose::compose_all_sources(
        &[
            ("a.idl".to_string(), "struct A { long x; };\n".to_string()),
            ("c.idl".to_string(), c.to_string()),
        ],
        &zerodds_idl_parse::MemoryLoader::new(),
        zerodds_idl_compose::ComposeOptions::default(),
    );
    let found = unresolved(&bundle.diagnostics);
    assert_eq!(found.file.as_deref(), Some("c.idl"), "{found:?}");
    let span = found.primary;
    assert_eq!(&c[span.start as usize..span.end as usize], "m", "{found:?}");
    assert!(found.message.contains("Missing"), "{found:?}");
}
