// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Where a diagnostic points. Spec §4 (lowering), §6 (validation).
//!
//! A span is the only part of a diagnostic a reader acts on: the message
//! says what is wrong and the span says where. Every assertion here
//! names both the line and column a person reads off an editor's ruler
//! and the exact text the span covers, because a span that merely
//! stopped being zero is not yet a span that points at the mistake.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{AnyId, Ast};
use zerodds_ast_core::ScopedName;
use zerodds_idl_cst::{LineColumn, Span};
use zerodds_idl_lower::lower;
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, parse};

/// Lower one file, keeping the model and every diagnostic it produced.
fn lowered(source: &str) -> (Ast, Vec<Diagnostic>) {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    let out = lower(&parsed.syntax(), "test.idl");
    (out.ast, out.diagnostics)
}

/// The one diagnostic with `code`, or a failure naming what did come.
fn only(diagnostics: &[Diagnostic], code: DiagnosticCode) -> &Diagnostic {
    let mut found = diagnostics.iter().filter(|d| d.code == code);
    let first = found
        .next()
        .unwrap_or_else(|| panic!("no {} among {diagnostics:?}", code.as_str()));
    assert!(
        found.next().is_none(),
        "more than one {}: {diagnostics:?}",
        code.as_str()
    );
    first
}

/// Where a span starts, and what it covers, in the terms a reader uses.
fn points_at(source: &str, span: Span) -> (u32, u32, &str) {
    let at = LineColumn::of(source, span.start);
    (
        at.line,
        at.column,
        span.slice(source).expect("the span is in bounds"),
    )
}

/// The source range recorded for a named type.
fn type_span(ast: &Ast, segments: &[&str]) -> Span {
    let name = ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect());
    let id = ast
        .find_by_scoped_name(&name)
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{segments:?} is not in the model"));
    let node_id = ast.type_(id).expect("the node is there").node_id;
    facet_span(ast, AnyId::Type(id, node_id))
}

/// The source range recorded on any node.
fn facet_span(ast: &Ast, any: AnyId) -> Span {
    let span = ast
        .facets(any)
        .unwrap_or_else(|| panic!("{any:?} has no facets"))
        .source
        .span
        .unwrap_or_else(|| panic!("{any:?} carries no source range"));
    Span::new(span.start, span.end)
}

/// The source range recorded for a member of an aggregate, by name.
fn member_span(ast: &Ast, segments: &[&str], member: &str) -> Span {
    let name = ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect());
    let id = ast
        .find_by_scoped_name(&name)
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{segments:?} is not in the model"));
    let direct = ast.members_of(id).iter().copied().find(|m| {
        ast.member(*m)
            .is_ok_and(|node| node.name.as_str() == member)
    });
    let from_case = ast.cases_of(id).iter().copied().find_map(|case| {
        let node = ast.case(case).ok()?;
        let member_node = ast.member(node.member).ok()?;
        (member_node.name.as_str() == member).then_some(node.member)
    });
    let found = direct
        .or(from_case)
        .unwrap_or_else(|| panic!("{segments:?} has no member `{member}`"));
    let any = ast
        .any_of_member(found)
        .expect("the member is in the model");
    facet_span(ast, any)
}

// -- what a member's span names ------------------------------------------

/// The lexer attaches trivia leading, so a declarator's CST node begins
/// at the whitespace and comments before it. A member's recorded range
/// must begin at the declarator.
#[test]
fn a_struct_member_names_its_declarator_and_not_the_space_before_it() {
    let source = "struct S {\n    long a;\n};\n";
    let (ast, _) = lowered(source);
    assert_eq!(
        points_at(source, member_span(&ast, &["S"], "a")),
        (2, 10, "a")
    );
}

/// `long a,\n         b;` is one declaration and two members. The second
/// declarator's leading trivia crosses a line break, so a span that
/// keeps it reports the line above the member it is about.
#[test]
fn the_second_declarator_of_a_list_names_its_own_line() {
    let source = "struct S {\n    long a,\n         b;\n};\n";
    let (ast, _) = lowered(source);
    assert_eq!(
        points_at(source, member_span(&ast, &["S"], "b")),
        (3, 10, "b")
    );
}

/// The same file with the second declarator repeating the first: the
/// duplicate is on line 3, and that is where the message has to point.
#[test]
fn a_duplicate_declarator_is_reported_on_the_line_it_is_written_on() {
    let source = "struct S {\n    long a,\n         a;\n};\n";
    let (_, diagnostics) = lowered(source);
    let at = only(&diagnostics, DiagnosticCode::DuplicateName).primary;
    assert_eq!(points_at(source, at), (3, 10, "a"));
}

/// A member id collision names the second member, which is the one a
/// reader has to change.
#[test]
fn a_member_id_collision_points_at_the_member_that_collides() {
    let source = "@mutable\nstruct S {\n    @id(1) long a;\n    @id(1) long b;\n};\n";
    let (_, diagnostics) = lowered(source);
    let at = only(&diagnostics, DiagnosticCode::DuplicateMemberId).primary;
    assert_eq!(points_at(source, at), (4, 17, "b"));
}

/// A union branch is a case and a member, and they are two ranges: the
/// case is the whole branch, the member behind it is its declarator —
/// the same range a struct member of the same declaration would get.
#[test]
fn a_union_case_member_names_its_declarator_and_not_the_whole_branch() {
    let source = "union U switch (long) {\n    case 1: long a;\n    case 2: long b;\n};\n";
    let (ast, _) = lowered(source);
    assert_eq!(
        points_at(source, member_span(&ast, &["U"], "b")),
        (3, 18, "b")
    );
}

// -- what a case's span names --------------------------------------------

/// A case node carried no source range at all: `Ast::add_case` marks its
/// facets synthetic, and lowering never overwrote them. Every rule that
/// reports a case therefore pointed at byte zero — line 1, column 1 — of
/// whichever file was being compiled.
#[test]
fn a_union_case_carries_the_range_of_its_branch() {
    let source = "union U switch (long) {\n    case 1: long a;\n    case 2: long b;\n};\n";
    let (ast, _) = lowered(source);
    let union = ast
        .find_by_scoped_name(&ScopedName::absolute(vec!["U".to_string()]))
        .ok()
        .and_then(AnyId::as_type)
        .expect("U is in the model");
    let cases = ast.cases_of(union).to_vec();
    assert_eq!(cases.len(), 2, "two branches");
    let second = ast.case(cases[1]).expect("the branch is in the model");
    let at = facet_span(&ast, AnyId::Case(cases[1], second.node_id));
    assert_eq!(points_at(source, at), (3, 5, "case 2: long b;"));
}

#[test]
fn a_repeated_case_label_is_reported_on_the_branch_that_repeats_it() {
    let source = "union U switch (long) {\n    case 1: long a;\n    case 1: long b;\n};\n";
    let (_, diagnostics) = lowered(source);
    let at = only(&diagnostics, DiagnosticCode::InvalidCaseLabel).primary;
    assert_eq!(points_at(source, at), (3, 5, "case 1: long b;"));
}

// -- what a type's span names --------------------------------------------

/// A type inside a module has the module's indentation before it, and a
/// blank line before that when one is written. The range must begin at
/// the keyword.
#[test]
fn a_nested_type_names_its_declaration_and_not_the_blank_line_above_it() {
    let source = "module m {\n\n    struct S {\n        long a;\n    };\n};\n";
    let (ast, _) = lowered(source);
    let at = type_span(&ast, &["m", "S"]);
    assert_eq!(
        points_at(source, at),
        (3, 5, "struct S {\n        long a;\n    };")
    );
}

/// `struct S;` never completed is reported by name, and the report has
/// to name the declaration: lowering recorded an origin for a forward
/// *interface* and none for a forward *type*, so this one message landed
/// on byte zero.
#[test]
fn an_unfinished_forward_declaration_points_at_the_declaration() {
    let source = "module m {\n    struct Missing;\n};\n";
    let (_, diagnostics) = lowered(source);
    let at = only(&diagnostics, DiagnosticCode::UndefinedForward).primary;
    assert_eq!(points_at(source, at), (2, 5, "struct Missing;"));
}

#[test]
fn an_unfinished_forward_interface_points_at_the_declaration_too() {
    let source = "module m {\n    interface Missing;\n};\n";
    let (_, diagnostics) = lowered(source);
    let at = only(&diagnostics, DiagnosticCode::UndefinedForward).primary;
    assert_eq!(points_at(source, at), (2, 5, "interface Missing;"));
}
