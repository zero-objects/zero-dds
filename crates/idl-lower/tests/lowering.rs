// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lowering the CST into the common AST. Spec §4.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Completeness, InterfaceKind, PrimitiveKind, SourceOrigin, TypeKind};
use zerodds_ast_core::ScopedName;
use zerodds_idl_lower::{lower, lower_into};
use zerodds_idl_parse::{DiagnosticCode, parse};

/// Parse and lower one file into a fresh model.
fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower(&parsed.syntax(), "test.idl").ast
}

/// Assert that lowering `source` trips a rule, printing what it did
/// report when it does not.
fn assert_trips_lowering(source: &str, code: DiagnosticCode) {
    let parsed = parse(source);
    let found = lower(&parsed.syntax(), "test.idl").diagnostics;
    assert!(
        found.iter().any(|d| d.code == code),
        "{source:?} did not trip {}: {found:?}",
        code.as_str()
    );
}

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

/// A named type in the model, by scoped name.
fn find(ast: &Ast, segments: &[&str]) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&scoped(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{segments:?} is not in the model"))
}

// -- modules -------------------------------------------------------------

#[test]
fn a_module_becomes_a_module_node() {
    let ast = model("module Robot { struct Pose { long x; }; };");
    assert!(ast.module_by_path(&scoped(&["Robot"])).is_some());
    let _ = find(&ast, &["Robot", "Pose"]);
}

#[test]
fn nested_modules_nest_in_the_model_too() {
    let ast = model("module A { module B { struct S { long x; }; }; };");
    assert!(ast.module_by_path(&scoped(&["A", "B"])).is_some());
    let _ = find(&ast, &["A", "B", "S"]);
}

#[test]
fn a_module_reopened_across_two_files_converges_on_one_node() {
    // The reason the middle was built first: the frontend needs no dedup
    // pass, because ensure_module makes reopening converge. Spec §4.
    let mut ast = Ast::new();
    for (path, source) in [
        ("a.idl", "module Robot { struct Pose { long x; }; };"),
        ("b.idl", "module Robot { struct Twist { long y; }; };"),
    ] {
        let parsed = parse(source);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
        let _ = lower_into(&mut ast, &parsed.syntax(), path);
    }

    let robot = ast
        .module_by_path(&scoped(&["Robot"]))
        .expect("one Robot module");
    // Both types landed in the same module.
    let _ = find(&ast, &["Robot", "Pose"]);
    let _ = find(&ast, &["Robot", "Twist"]);

    // And there is exactly one Robot: a second would make the scoped
    // name ambiguous.
    assert!(ast.module_by_path(&scoped(&["Robot"])) == Some(robot));
}

/// OMG IDL 4.2 §7.4.2: a module may be reopened, and the reopening
/// continues the same scope. §7.5.1 then resolves an unqualified name in
/// it against everything that scope holds, whichever opening declared it.
///
/// The scope stack used to drop a module's bindings when the opening
/// closed, so `First` was looked up as `::First` from the root and
/// reported `NotFound` — while the identical reference inside a single
/// opening resolved.
#[test]
fn a_reopened_module_still_sees_what_the_first_opening_declared() {
    let ast = model(
        "module common { struct First { long a; }; };
         module common { struct S { First f; }; };",
    );
    let s = find(&ast, &["common", "S"]);
    let member = ast.members_of(s).first().copied().expect("member");
    let type_ = &ast.member(member).expect("member").type_;
    assert!(type_.is_resolved(), "`First` did not resolve: {type_:?}");
    assert_eq!(type_.resolved(), Some(find(&ast, &["common", "First"])));
}

/// The same scope also means the same collision space: a name declared
/// in both openings is a duplicate, not two declarations.
#[test]
fn a_name_declared_in_two_openings_of_one_module_collides() {
    let parsed = parse(
        "module common { struct Twice { long a; }; };
         module common { struct Twice { long b; }; };",
    );
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::DuplicateName),
        "{:?}",
        lowered.diagnostics
    );
}

/// And the same forward-declaration space: the definition completes the
/// node the earlier opening created rather than adding a second.
#[test]
fn a_definition_completes_a_forward_declared_in_an_earlier_opening() {
    let ast = model(
        "module common { struct Pose; };
         module common { struct Pose { long x; }; };",
    );
    let pose = find(&ast, &["common", "Pose"]);
    assert_eq!(
        ast.type_(pose).expect("Pose").completeness,
        Completeness::Complete
    );
    assert_eq!(ast.members_of(pose).len(), 1);
}

// -- enumerators as values -----------------------------------------------

/// OMG IDL 4.2 §7.4.1.4.2: "An enum constant can only be defined using a
/// scoped name for the enumerator. The scoped name is resolved using the
/// normal scope resolution rules." Its example is `module M { enum Size
/// { small, medium, large }; }; const M::Size MYSIZE = M::medium;` —
/// `M::medium`, so the enumerator sits in the scope enclosing the
/// enumeration and not inside it.
#[test]
fn an_enumerator_is_named_from_the_scope_around_its_enumeration() {
    let ast = model("module M { enum Size { small, medium, large }; };");
    let any = ast
        .find_by_scoped_name(&scoped(&["M", "medium"]))
        .expect("M::medium");
    assert_eq!(any.kind(), zerodds_ast::NodeKind::Enumerator);
    // And not through the enumeration.
    assert!(
        ast.find_by_scoped_name(&scoped(&["M", "Size", "medium"]))
            .is_err()
    );
}

/// §7.4.1.4.4.4.2 rule (54) makes a union's `<case_label>` a
/// `<const_expr>`, and the same clause admits an enumeration as the
/// `<switch_type_spec>`. Until a name in an expression could reach an
/// enumerator, the label was taken for a constant, looked up from the
/// root and reported `NotFound`, so no union over an enumeration lowered
/// at all.
#[test]
fn a_union_over_an_enumeration_lowers_with_its_labels_evaluated() {
    let ast = model(
        "enum Colour { RED, GREEN, BLUE };
         union Choice switch (Colour) {
             case RED: long whole;
             case GREEN:
             case BLUE: double fraction;
         };",
    );
    let choice = find(&ast, &["Choice"]);
    let labels: Vec<i128> = ast
        .cases_of(choice)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .flat_map(|case| case.labels.clone())
        .filter_map(|label| match label {
            zerodds_ast::CaseLabel::Value(expr) => ast.eval_integral(expr).ok(),
            zerodds_ast::CaseLabel::Default => None,
        })
        .collect();
    assert_eq!(labels, [0, 1, 2]);
}

/// The qualified form of the same thing, which resolves through the
/// model's index rather than the scope stack.
#[test]
fn a_case_label_may_name_its_enumerator_by_a_qualified_name() {
    let ast = model(
        "module m { enum Colour { RED, GREEN, BLUE }; };
         union Choice switch (m::Colour) {
             case m::GREEN: long whole;
             default: double fraction;
         };",
    );
    let choice = find(&ast, &["Choice"]);
    let first = ast
        .cases_of(choice)
        .first()
        .copied()
        .and_then(|id| ast.case(id).ok())
        .expect("a case");
    let zerodds_ast::CaseLabel::Value(expr) = first.labels[0] else {
        panic!("expected a value label");
    };
    assert_eq!(ast.eval_integral(expr), Ok(1));
}

/// §7.4.1.4.4.3's `element_spec ::= type_spec declarator` — one
/// declarator, where a struct member takes a list.
///
/// `parse_union_case` shares `parse_declarator_list` with the struct
/// member, so `case 1: long a, b;` parsed without a word, and lowering
/// took the first declarator and stopped: `b` reached the model as
/// nothing at all — no case, no member, no diagnostic — and every
/// consumer downstream saw a union the source did not write.
#[test]
fn a_union_case_with_a_second_declarator_is_reported_and_not_dropped() {
    assert_trips_lowering(
        "union U switch (long) { case 1: long a, b; default: short c; };",
        DiagnosticCode::UnexpectedToken,
    );
}

/// The single-declarator form the clause does allow is untouched.
#[test]
fn a_union_case_with_one_declarator_lowers_without_a_diagnostic() {
    let parsed = parse("union U switch (long) { case 1: long a; default: short c; };");
    let found = lower(&parsed.syntax(), "test.idl").diagnostics;
    assert!(found.is_empty(), "{found:?}");
}

/// An enumerator is also usable wherever any other constant expression
/// is — the same production, per §7.4.1.4.2.
#[test]
fn an_enumerator_evaluates_inside_a_larger_expression() {
    let ast = model(
        "enum Colour { RED, GREEN, BLUE };
         struct S { long a[BLUE + 1]; };",
    );
    let s = find(&ast, &["S"]);
    let member = ast.members_of(s).first().copied().expect("member");
    let array = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("the array");
    let TypeKind::Array { dims, .. } = &ast.type_(array).expect("array").kind else {
        panic!("expected an array");
    };
    assert_eq!(ast.eval_integral(dims[0]), Ok(3));
}

/// Sharing the enclosing scope means sharing its collision space:
/// §7.2.3 makes two enumerations declaring one literal name a duplicate.
#[test]
fn one_literal_name_in_two_enumerations_collides() {
    let parsed = parse("enum A { X, Y }; enum B { X, Z };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::DuplicateName),
        "{:?}",
        lowered.diagnostics
    );
}

// -- structs and members -------------------------------------------------

#[test]
fn a_struct_carries_its_members_in_order() {
    let ast = model("struct Pose { long x; double y; };");
    let pose = find(&ast, &["Pose"]);
    let members = ast.members_of(pose);
    assert_eq!(members.len(), 2);
    assert_eq!(ast.member(members[0]).expect("x").name, "x");
    assert_eq!(ast.member(members[1]).expect("y").name, "y");
}

#[test]
fn several_declarators_share_the_type_and_become_separate_members() {
    let ast = model("struct P { double x, y, z; };");
    let p = find(&ast, &["P"]);
    let members = ast.members_of(p);
    assert_eq!(members.len(), 3);
    let names: Vec<String> = members
        .iter()
        .map(|m| ast.member(*m).expect("member").name.clone())
        .collect();
    assert_eq!(names, vec!["x", "y", "z"]);
    // All three point at one primitive node, not three copies.
    let types: Vec<_> = members
        .iter()
        .filter_map(|m| ast.member(*m).ok())
        .filter_map(|m| m.type_.resolved())
        .collect();
    assert_eq!(types.len(), 3);
    assert!(types.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn each_primitive_keyword_reaches_its_model_kind() {
    for (idl, expected) in [
        ("boolean", PrimitiveKind::Boolean),
        ("octet", PrimitiveKind::Octet),
        ("char", PrimitiveKind::Char),
        ("wchar", PrimitiveKind::WChar),
        ("short", PrimitiveKind::Int16),
        ("long", PrimitiveKind::Int32),
        ("long long", PrimitiveKind::Int64),
        ("unsigned short", PrimitiveKind::UInt16),
        ("unsigned long", PrimitiveKind::UInt32),
        ("unsigned long long", PrimitiveKind::UInt64),
        ("float", PrimitiveKind::Float32),
        ("double", PrimitiveKind::Float64),
        ("long double", PrimitiveKind::Float128),
        ("int8", PrimitiveKind::Int8),
        ("uint8", PrimitiveKind::UInt8),
        ("int64", PrimitiveKind::Int64),
    ] {
        let source = alloc_format(idl);
        let ast = model(&source);
        let s = find(&ast, &["S"]);
        let member = ast.members_of(s).first().copied().expect("one member");
        let type_id = ast
            .member(member)
            .expect("member")
            .type_
            .resolved()
            .unwrap_or_else(|| panic!("{idl} did not resolve"));
        assert_eq!(
            ast.type_(type_id).expect("type").kind,
            TypeKind::Primitive(expected),
            "{idl}"
        );
    }
}

fn alloc_format(idl: &str) -> String {
    format!("struct S {{ {idl} x; }};")
}

// -- forward references, spec §2 of the AST ------------------------------

#[test]
fn a_reference_to_a_type_declared_later_is_pending_not_an_error() {
    // The model is built for partial knowledge; the frontend uses it
    // rather than working around it.
    let ast = model("struct Holder { Later value; }; struct Later { long x; };");
    let holder = find(&ast, &["Holder"]);
    let member = ast.members_of(holder).first().copied().expect("member");
    assert!(ast.member(member).expect("member").type_.is_pending());
}

#[test]
fn a_reference_to_a_type_declared_earlier_resolves_immediately() {
    let ast = model("struct Earlier { long x; }; struct Holder { Earlier value; };");
    let holder = find(&ast, &["Holder"]);
    let member = ast.members_of(holder).first().copied().expect("member");
    assert!(ast.member(member).expect("member").type_.is_resolved());
}

#[test]
fn a_pending_reference_resolves_once_the_name_arrives() {
    let mut ast = model("struct Holder { Later value; }; struct Later { long x; };");
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.resolved >= 1, "nothing resolved: {report:?}");

    let holder = find(&ast, &["Holder"]);
    let member = ast.members_of(holder).first().copied().expect("member");
    assert!(ast.member(member).expect("member").type_.is_resolved());
}

#[test]
fn a_forward_declaration_on_its_own_is_forward() {
    let ast = model("struct Pose;");
    let pose = find(&ast, &["Pose"]);
    assert_eq!(
        ast.type_(pose).expect("Pose").completeness,
        Completeness::Forward
    );
    assert!(ast.members_of(pose).is_empty());
}

#[test]
fn the_definition_completes_the_forward_node_rather_than_adding_a_second() {
    // Forwardness is a state, not a kind: references taken while only
    // the forward declaration existed have to stay valid. Spec §3.2.
    let parsed = parse("struct Pose; struct Pose { long x; };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "completing a forward declaration was reported as a duplicate: {:?}",
        lowered.diagnostics
    );

    let ast = lowered.ast;
    let pose = find(&ast, &["Pose"]);
    assert_eq!(
        ast.type_(pose).expect("Pose").completeness,
        Completeness::Complete
    );
    assert_eq!(ast.members_of(pose).len(), 1);
    // One Pose in the model, not two.
    assert_eq!(
        ast.types().filter(|(_, node)| node.name == "Pose").count(),
        1
    );
}

#[test]
fn a_reference_taken_against_a_forward_declaration_survives_the_definition() {
    let mut ast = model("struct Pose; struct Holder { Pose value; }; struct Pose { long x; };");
    let holder = find(&ast, &["Holder"]);
    let member = ast.members_of(holder).first().copied().expect("member");
    let before = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("resolved against the forward declaration");

    let _ = ast.resolve_all_by_name().expect("a resolve pass");

    let after = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("still resolved");
    assert_eq!(before, after, "the handle changed under the reference");
    assert_eq!(
        ast.type_(after).expect("Pose").completeness,
        Completeness::Complete
    );
}

#[test]
fn a_second_full_definition_is_a_duplicate_not_a_completion() {
    let parsed = parse("struct Pose { long x; }; struct Pose { long y; };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.message.contains("already declared")),
        "{:?}",
        lowered.diagnostics
    );
}

#[test]
fn a_name_that_never_arrives_ends_up_broken_rather_than_pending() {
    let mut ast = model("struct Holder { Absent value; };");
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(!report.is_clean(), "an absent name reported nothing");

    let holder = find(&ast, &["Holder"]);
    let member = ast.members_of(holder).first().copied().expect("member");
    let reference = &ast.member(member).expect("member").type_;
    assert!(reference.is_broken());
    assert!(!reference.is_pending());
}

// -- the other declaration forms -----------------------------------------

#[test]
fn an_enum_carries_its_enumerators_in_order() {
    let ast = model("enum Mode { IDLE, ACTIVE, FAULT };");
    let mode = find(&ast, &["Mode"]);
    let TypeKind::Enumeration { enumerators } = &ast.type_(mode).expect("Mode").kind else {
        panic!("expected an enumeration");
    };
    let names: Vec<String> = enumerators
        .iter()
        .map(|e| ast.enumerator(*e).expect("enumerator").name.clone())
        .collect();
    assert_eq!(names, vec!["IDLE", "ACTIVE", "FAULT"]);
}

#[test]
fn a_bitmask_carries_its_bits() {
    let ast = model("bitmask Flags { READY, MOVING };");
    let flags = find(&ast, &["Flags"]);
    let TypeKind::Bitmask { bits } = &ast.type_(flags).expect("Flags").kind else {
        panic!("expected a bitmask");
    };
    assert_eq!(bits.len(), 2);
}

#[test]
fn a_typedef_becomes_an_alias_to_its_target() {
    let ast = model("typedef long Distance;");
    let distance = find(&ast, &["Distance"]);
    let TypeKind::Alias { aliased } = &ast.type_(distance).expect("Distance").kind else {
        panic!("expected an alias");
    };
    assert!(aliased.is_resolved());
}

#[test]
fn one_typedef_may_declare_several_names() {
    let ast = model("typedef long A, B;");
    let _ = find(&ast, &["A"]);
    let _ = find(&ast, &["B"]);
}

/// IDL 4.2 §7.4.1.4.4: a `<declarator>` under a `typedef` is the same
/// production as one on a struct member, dimensions included.
///
/// The failure this pins was silent: the dimensions were discarded and
/// `Matrix` became a plain alias of `long`, with no diagnostic.
#[test]
fn a_typedef_declarator_keeps_its_array_dimensions() {
    let ast = model("typedef long Matrix[2][3];");
    let matrix = find(&ast, &["Matrix"]);
    let TypeKind::Alias { aliased } = &ast.type_(matrix).expect("Matrix").kind else {
        panic!("expected an alias");
    };
    let target = aliased.resolved().expect("the aliased type");
    let TypeKind::Array { dims, .. } = &ast.type_(target).expect("the array").kind else {
        panic!("expected an alias of an array, not of the element type");
    };
    let extents: Vec<i128> = dims
        .iter()
        .map(|dim| ast.eval_integral(*dim).expect("a dimension"))
        .collect();
    assert_eq!(extents, [2, 3]);
}

/// One declarator with dimensions and one without, from one type
/// specification: `A` is an array and `B` is not.
#[test]
fn dimensions_belong_to_the_declarator_and_not_to_the_typedef() {
    let ast = model("typedef long A[4], B;");
    let a = find(&ast, &["A"]);
    let b = find(&ast, &["B"]);
    for (id, name, expect_array) in [(a, "A", true), (b, "B", false)] {
        let TypeKind::Alias { aliased } = &ast.type_(id).expect(name).kind else {
            panic!("expected an alias for {name}");
        };
        let target = aliased.resolved().expect("the aliased type");
        let is_array = matches!(
            ast.type_(target).expect("the target").kind,
            TypeKind::Array { .. }
        );
        assert_eq!(is_array, expect_array, "{name}");
    }
}

#[test]
fn an_exception_carries_its_members() {
    let ast = model("exception Failure { long code; };");
    let any = ast
        .find_by_scoped_name(&scoped(&["Failure"]))
        .expect("Failure");
    assert_eq!(any.kind(), zerodds_ast::NodeKind::Exception);
}

#[test]
fn a_native_declaration_becomes_a_native_type() {
    let ast = model("native Handle;");
    let handle = find(&ast, &["Handle"]);
    assert_eq!(ast.type_(handle).expect("Handle").kind, TypeKind::Native);
}

#[test]
fn a_sequence_member_carries_its_element_type() {
    let ast = model("struct Bag { sequence<long> ids; };");
    let bag = find(&ast, &["Bag"]);
    let member = ast.members_of(bag).first().copied().expect("member");
    let seq = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("a sequence type");
    let TypeKind::Sequence { element, .. } = &ast.type_(seq).expect("sequence").kind else {
        panic!("expected a sequence");
    };
    assert!(element.is_resolved());
}

#[test]
fn a_struct_with_a_base_records_it() {
    let ast = model("struct Base { long a; }; struct Derived : Base { long b; };");
    let derived = find(&ast, &["Derived"]);
    let TypeKind::Structure { base, .. } = &ast.type_(derived).expect("Derived").kind else {
        panic!("expected a struct");
    };
    assert!(base.as_ref().is_some_and(|b| b.is_resolved()));
}

// -- provenance ----------------------------------------------------------

#[test]
fn every_lowered_type_names_the_file_it_came_from() {
    // A diagnostic that cannot point at a file is not much of a
    // diagnostic. Spec §4.
    let parsed = parse("module Robot { struct Pose { long x; }; };");
    let lowered = lower(&parsed.syntax(), "robot.idl");
    let pose = find(&lowered.ast, &["Robot", "Pose"]);
    let any = lowered.ast.any_of_type(pose).expect("any");
    let facets = lowered.ast.facets(any).expect("facets");
    match &facets.source.origin {
        SourceOrigin::File { path } => assert_eq!(path, "robot.idl"),
        other => panic!("expected a file origin, got {other:?}"),
    }
    assert!(facets.source.span.is_some(), "no byte range recorded");
}

// -- collisions, spec §6 -------------------------------------------------

#[test]
fn two_names_differing_only_by_case_collide() {
    // IDL is case-insensitive for collisions and case-preserving
    // otherwise. OMG IDL 4.2 §7.2.3.
    let parsed = parse("struct Pose { long x; }; struct pose { long y; };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.message.contains("already declared")),
        "{:?}",
        lowered.diagnostics
    );
}

#[test]
fn distinct_names_do_not_collide() {
    let parsed = parse("struct Pose { long x; }; struct Twist { long y; };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
}

// -- the model stays coherent --------------------------------------------

#[test]
fn a_lowered_model_satisfies_its_own_invariants() {
    // The model checks these on every commit, so reaching here at all
    // means the lowering never built something structurally wrong.
    let ast = model(
        "module Robot {
           enum Mode { IDLE, ACTIVE };
           struct Header { long id; };
           struct Pose : Header { double x, y; sequence<long> path; };
         };",
    );
    assert!(ast.node_count() > 5);
    // And nothing was left half-built.
    assert!(ast.commit_sequence() > 0);
}

#[test]
fn a_file_that_parses_to_nothing_lowers_to_an_empty_model() {
    let ast = model("");
    // Only the root and the built-ins.
    assert!(ast.types().next().is_none());
}

// -- §7.4.2 Any, rules (69) and (70) -------------------------------------

#[test]
fn an_any_member_reaches_the_model_as_a_type_rather_than_a_broken_reference() {
    // `is_primitive_keyword` admitted `any`, `primitive_of` had no arm
    // for it, and the member's type became `Ref::Broken` with neither
    // the parser nor the lowering saying anything — the first complaint
    // came from the layout planner three layers later, about a node id.
    //
    // Building Block Any is one of the fifteen and this project takes
    // all fifteen, so the construct is in the model. What it has no wire
    // form for is refused at serialisation, by name.
    let parsed = parse("struct S { any a; };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let s = find(&lowered.ast, &["S"]);
    let member = lowered.ast.members_of(s).first().copied().expect("member");
    let type_ = lowered
        .ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("`any` resolved to a type");
    assert_eq!(lowered.ast.type_(type_).expect("any").kind, TypeKind::Any);
}

#[test]
fn every_any_in_a_file_shares_one_built_in_node() {
    // The same rule the fifteen primitives follow: a file with a hundred
    // `long` members holds one `long`.
    let ast = model("struct S { any a; any b; };");
    let count = ast
        .types()
        .filter(|(_, node)| node.kind == TypeKind::Any)
        .count();
    assert_eq!(count, 1);
}

#[test]
fn any_is_reachable_in_every_position_the_grammar_gives_it() {
    // (70) `<base_type_spec> ::+ <any_type>` — a member, a typedef, a
    // sequence element, an operation parameter and its return.
    for source in [
        "struct S { any a; };",
        "typedef any Whatever;",
        "struct S { sequence<any> bag; };",
        "interface I { any f(in any a); };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered
                .ast
                .types()
                .any(|(_, node)| node.kind == TypeKind::Any),
            "{source:?} produced no `any`"
        );
    }
}

// -- §7.4.6 CORBA-Specific – Interfaces, rules (117) and (118) -----------

#[test]
fn an_object_parameter_reaches_the_model_as_an_object_reference() {
    // `Object` was the second of the three keywords `is_primitive_keyword`
    // admitted and `primitive_of` had no arm for: the parameter was
    // created, its type became `Ref::Broken`, and nothing said so.
    //
    // (117) `<object_type> ::= "Object"` names no interface, which is
    // what the absent reference in the model says. A reference to an
    // invented `CORBA::Object` declaration would say the file had one.
    let parsed = parse("interface I { void f(in Object o); };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let object = find(&lowered.ast, &["Object"]);
    assert_eq!(
        lowered.ast.type_(object).expect("Object").kind,
        TypeKind::ObjectRef { interface: None }
    );

    let interface = match lowered.ast.find_by_scoped_name(&scoped(&["I"])).expect("I") {
        zerodds_ast::AnyId::Interface(id, _) => id,
        other => panic!("I is not an interface: {other:?}"),
    };
    let operation = lowered
        .ast
        .operations_of(interface)
        .first()
        .copied()
        .expect("f");
    let param = lowered
        .ast
        .operation(operation)
        .expect("f")
        .params
        .first()
        .copied()
        .expect("o");
    assert_eq!(
        lowered
            .ast
            .param(param)
            .expect("o")
            .type_
            .resolved()
            .expect("`Object` resolved to a type"),
        object
    );
}

#[test]
fn object_is_reachable_in_every_position_rule_118_gives_it() {
    // (118) `<base_type_spec> ::+ <object_type>`, so wherever a base
    // type may be written.
    for source in [
        "struct S { Object o; };",
        "typedef Object Handle;",
        "struct S { sequence<Object> refs; };",
        "interface I { Object f(); };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered
                .ast
                .types()
                .any(|(_, node)| matches!(node.kind, TypeKind::ObjectRef { .. })),
            "{source:?} produced no object reference"
        );
    }
}

// -- §7.4.7 CORBA-Specific – Value Types, rules (131) and (132) ----------

#[test]
fn a_valuebase_parameter_reaches_the_model_as_valuebase() {
    // The third of the three keywords `is_primitive_keyword` admitted
    // and `primitive_of` had no arm for. (131) is `<value_base_type> ::=
    // "ValueBase"`: the abstract base every valuetype has, written as a
    // type, naming nothing and declaring no state.
    let parsed = parse("interface I { void f(in ValueBase v); };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let base = find(&lowered.ast, &["ValueBase"]);
    assert_eq!(
        lowered.ast.type_(base).expect("ValueBase").kind,
        TypeKind::ValueBase
    );
}

#[test]
fn valuebase_is_reachable_in_every_position_rule_132_gives_it() {
    // (132) `<base_type_spec> ::+ <value_base_type>`.
    for source in [
        "struct S { ValueBase v; };",
        "typedef ValueBase AnyValue;",
        "struct S { sequence<ValueBase> values; };",
        "interface I { ValueBase f(); };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered
                .ast
                .types()
                .any(|(_, node)| node.kind == TypeKind::ValueBase),
            "{source:?} produced no `ValueBase`"
        );
    }
}

#[test]
fn none_of_the_three_built_in_type_names_leaves_a_broken_reference() {
    // The three that vanished, together: `any`, `Object` and
    // `ValueBase` were the keywords the parser admitted and the lowering
    // had no arm for, and each produced a member whose type was
    // `Ref::Broken` with no diagnostic from either layer.
    let parsed = parse("struct S { any a; Object o; ValueBase v; };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let s = find(&lowered.ast, &["S"]);
    for member in lowered.ast.members_of(s) {
        let node = lowered.ast.member(*member).expect("member");
        assert!(
            node.type_.resolved().is_some(),
            "`{}` still has no type behind it",
            node.name
        );
    }
}

// -- §7.4.1.3 rule (43): `const fixed` -----------------------------------

/// The `fixed<d,s>` a constant ended up with.
fn fixed_of(ast: &Ast, name: &str) -> (u8, u8) {
    let id = match ast
        .find_by_scoped_name(&scoped(&[name]))
        .unwrap_or_else(|_| panic!("{name} is not in the model"))
    {
        zerodds_ast::AnyId::Const(id, _) => id,
        other => panic!("{name} is not a constant: {other:?}"),
    };
    let type_ = ast
        .constant(id)
        .expect(name)
        .type_
        .resolved()
        .unwrap_or_else(|| panic!("{name} has no type"));
    match ast.type_(type_).expect("type").kind {
        TypeKind::Fixed { digits, scale } => (digits, scale),
        ref other => panic!("{name} is not a fixed: {other:?}"),
    }
}

#[test]
fn a_fixed_constant_takes_its_type_from_its_literal() {
    // (43) `<fixed_pt_const_type> ::= "fixed"` — the constant's type is
    // written without parameters and derived from the value.
    // §7.4.1.4.3, p. 31: "A fixed-point literal has the apparent number
    // of total and fractional digits. For example, 0123.450d is
    // considered to be fixed<7,3> and 3000.00d is fixed<6,2>."
    let ast =
        model("const fixed C = 3.14d;\nconst fixed D = 0123.450d;\nconst fixed E = 3000.00d;");
    assert_eq!(fixed_of(&ast, "C"), (3, 2));
    assert_eq!(fixed_of(&ast, "D"), (7, 3));
    assert_eq!(fixed_of(&ast, "E"), (6, 2));
}

#[test]
fn a_prefix_operator_does_not_change_a_fixed_constants_precision() {
    // §7.4.1.4.3: "Prefix operators do not affect the precision; a
    // prefix + is optional, and does not change the result."
    let ast = model("const fixed N = -3.14d;\nconst fixed P = +3.14d;");
    assert_eq!(fixed_of(&ast, "N"), (3, 2));
    assert_eq!(fixed_of(&ast, "P"), (3, 2));
}

#[test]
fn table_7_11_decides_the_type_of_a_fixed_expression() {
    // `+`/`-`: fixed<max(d1-s1,d2-s2)+max(s1,s2)+1, max(s1,s2)>
    // `*`:     fixed<d1+d2, s1+s2>
    let ast = model(
        "const fixed A = 3.14d + 1.0d;\n\
         const fixed S = 3.14d - 1.0d;\n\
         const fixed M = 3.14d * 2.0d;\n",
    );
    // 3.14d is fixed<3,2>, 1.0d is fixed<2,1>:
    // d = max(3-2, 2-1) + max(2,1) + 1 = 1 + 2 + 1 = 4, s = 2.
    assert_eq!(fixed_of(&ast, "A"), (4, 2));
    assert_eq!(fixed_of(&ast, "S"), (4, 2));
    // d = 3 + 2 = 5, s = 2 + 1 = 3.
    assert_eq!(fixed_of(&ast, "M"), (5, 3));
}

#[test]
fn a_fixed_constant_may_name_another_one() {
    let ast = model("const fixed A = 1.5d;\nconst fixed B = A + 1.0d;");
    assert_eq!(fixed_of(&ast, "A"), (2, 1));
    // fixed<2,1> + fixed<2,1>: d = max(1,1) + 1 + 1 = 3, s = 1.
    assert_eq!(fixed_of(&ast, "B"), (3, 1));
}

#[test]
fn a_fixed_expression_is_capped_at_thirty_one_digits() {
    // §7.4.1.4.3: "If an individual computation between a pair of
    // fixed-point literals actually generates more than 31 significant
    // digits, then a 31-digit result is retained as follows:
    // fixed<d,s> => fixed<31, 31-d+s>."
    let ast = model("const fixed W = 12345678901234567.0d * 12345678901234567.0d;");
    // fixed<18,1> * fixed<18,1> = fixed<36,2>, capped to
    // fixed<31, 31-36+2> = fixed<31,-3> -> the scale cannot go below 0.
    let (digits, scale) = fixed_of(&ast, "W");
    assert_eq!(digits, 31);
    assert_eq!(scale, 0);
}

#[test]
fn an_integer_operand_in_a_fixed_expression_is_refused() {
    // Measured on the Linux test host: all four ORBs that take
    // `const fixed` at all refuse `const fixed C = 3.14d + 1;` —
    // omniORB "Cannot interpret integer literal as fixed point", JacORB
    // "Expecting fixed point constant", OpenDDS and TAO "incompatible
    // types in constant assignment".
    let parsed = parse("const fixed C = 3.14d + 1;");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnexpectedToken),
        "{:?}",
        lowered.diagnostics
    );
}

#[test]
fn a_bare_fixed_is_still_refused_as_a_member_type() {
    // (42) `<fixed_pt_type> ::= "fixed" "<" <positive_int_const> ","
    // <positive_int_const> ">"` — a member's `fixed` carries its
    // parameters. Only (43)'s constant form goes without.
    let parsed = parse("struct S { fixed f; };");
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        !lowered.diagnostics.is_empty(),
        "a bare `fixed` member was accepted"
    );
}

#[test]
fn a_parameterised_fixed_member_is_unaffected() {
    let ast = model("struct S { fixed<9,2> amount; };");
    let s = find(&ast, &["S"]);
    let member = ast.members_of(s).first().copied().expect("member");
    let type_ = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("resolved");
    assert_eq!(
        ast.type_(type_).expect("fixed").kind,
        TypeKind::Fixed {
            digits: 9,
            scale: 2
        }
    );
}

// -- §7.4.3 rule (75): a forward-declared interface ----------------------

#[test]
fn a_forward_declared_interface_is_completed_by_its_definition() {
    // (75) `<interface_forward_dcl> ::= <interface_header>`. Every
    // forward used to bind a `Structure` placeholder whatever keyword it
    // wrote, so this file collided with itself: `DuplicateName` for the
    // definition, which could not complete a type it was not, and
    // `UndefinedForward` for the placeholder nothing ever finished.
    // `struct S;` and `union U;` were right all along.
    let parsed = parse("interface I; interface I { void f(); };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let interfaces: Vec<_> = lowered
        .ast
        .interfaces()
        .filter(|(_, node)| node.name == "I")
        .collect();
    assert_eq!(
        interfaces.len(),
        1,
        "the forward and its definition are one node"
    );
    let (id, node) = interfaces[0];
    assert_eq!(node.completeness, Completeness::Complete);
    assert_eq!(lowered.ast.operations_of(id).len(), 1);
}

#[test]
fn a_forward_interface_keeps_the_form_its_header_wrote() {
    for (source, expected) in [
        ("interface I; interface I { };", InterfaceKind::Interface),
        (
            "local interface I; local interface I { };",
            InterfaceKind::Local,
        ),
        (
            "abstract interface I; abstract interface I { };",
            InterfaceKind::Abstract,
        ),
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        let (_, node) = lowered
            .ast
            .interfaces()
            .find(|(_, node)| node.name == "I")
            .expect("I");
        assert_eq!(node.kind, expected, "{source:?}");
    }
}

#[test]
fn an_interface_forward_that_is_never_defined_is_still_reported() {
    // The rule the fix must not lose: §7.4.1.4.5 wants the definition in
    // the same translation unit, and the check walks both arenas now
    // that a forward interface is an interface node.
    assert_trips_lowering("interface I;", DiagnosticCode::UndefinedForward);
}

#[test]
fn a_reference_taken_against_a_forward_interface_survives_the_definition() {
    // Why forwardness is a state and not a kind.
    let parsed = parse(
        "interface Peer;\n\
         interface Session { void join(in Peer p); };\n\
         interface Peer { void ping(); };",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.ast.resolve_all_by_name().expect("resolve");
    let (peer, node) = lowered
        .ast
        .interfaces()
        .find(|(_, node)| node.name == "Peer")
        .expect("Peer");
    assert_eq!(node.completeness, Completeness::Complete);
    assert_eq!(lowered.ast.operations_of(peer).len(), 1);
}

#[test]
fn a_second_full_interface_definition_is_a_duplicate_not_a_completion() {
    assert_trips_lowering(
        "interface I { }; interface I { };",
        DiagnosticCode::DuplicateName,
    );
}

// -- §7.4.8 rule (135): a forward-declared component ---------------------

#[test]
fn a_forward_declared_component_is_completed_by_its_definition() {
    // (135) `<component_forward_dcl> ::= "component" <identifier>` —
    // the same rule as (75) one building block on, and it had the same
    // bug: a `Structure` placeholder the definition could not complete.
    let parsed = parse("component C; component C { };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let found: Vec<_> = lowered
        .ast
        .interfaces()
        .filter(|(_, node)| node.name == "C")
        .collect();
    assert_eq!(
        found.len(),
        1,
        "the forward and its definition are one node"
    );
    assert_eq!(found[0].1.completeness, Completeness::Complete);
    assert_eq!(found[0].1.kind, InterfaceKind::Component);
}

#[test]
fn every_component_model_forward_keeps_its_own_form() {
    for (source, expected) in [
        ("component C; component C { };", InterfaceKind::Component),
        ("home H; home H { };", InterfaceKind::Home),
        ("connector K; connector K { };", InterfaceKind::Connector),
        ("porttype P; porttype P { };", InterfaceKind::PortType),
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        let (_, node) = lowered
            .ast
            .interfaces()
            .find(|(_, node)| node.completeness == Completeness::Complete)
            .expect("the definition");
        assert_eq!(node.kind, expected, "{source:?}");
    }
}

#[test]
fn a_component_forward_that_is_never_defined_is_still_reported() {
    assert_trips_lowering("component C;", DiagnosticCode::UndefinedForward);
}

// -- §7.4.7 rule (126): a boxed value ------------------------------------

#[test]
fn a_value_box_reaches_the_model_as_one() {
    // (126) `<value_box_def> ::= "valuetype" <identifier> <type_spec>`.
    // The parser committed to a body at the name and answered "expected
    // `{` opening the valuetype body", so `TypeKind::ValueBox` sat in
    // `crates/ast` unreachable from IDL.
    let parsed = parse("valuetype VB long;");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let vb = find(&lowered.ast, &["VB"]);
    let TypeKind::ValueBox { boxed } = lowered.ast.type_(vb).expect("VB").kind.clone() else {
        panic!("VB is not a value box");
    };
    let target = boxed.resolved().expect("the boxed type");
    assert_eq!(
        lowered.ast.type_(target).expect("boxed").kind,
        TypeKind::Primitive(PrimitiveKind::Int32)
    );
}

#[test]
fn a_value_box_may_box_any_type_spec() {
    for source in [
        "valuetype VB long;",
        "valuetype VB string;",
        "valuetype VB sequence<long>;",
        "struct S { long x; }; valuetype VB S;",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered
                .ast
                .types()
                .any(|(_, node)| matches!(node.kind, TypeKind::ValueBox { .. })),
            "{source:?} produced no value box"
        );
    }
}

#[test]
fn the_other_valuetype_forms_are_unaffected_by_the_box() {
    // A value box is told from a valuetype by what follows the name, so
    // every other form has to keep parsing as itself.
    for source in [
        "valuetype V { public long x; };",
        "custom valuetype V { public long x; };",
        "eventtype E { public long x; };",
        "valuetype Base { }; valuetype V : Base { public long x; };",
        "interface I { }; valuetype V supports I { public long x; };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        assert!(
            !lowered
                .ast
                .types()
                .any(|(_, node)| matches!(node.kind, TypeKind::ValueBox { .. })),
            "{source:?} was read as a value box"
        );
    }
}

#[test]
fn a_value_box_round_trips_byte_for_byte() {
    let source = "valuetype VB sequence<long>;\n";
    assert_eq!(parse(source).text(), source);
}

// -- §7.4.7 rule (127): an abstract valuetype -----------------------------

#[test]
fn an_abstract_valuetype_parses_as_a_valuetype() {
    // (127) `<value_abs_def> ::= "abstract" ("valuetype"|"eventtype")
    // <identifier> …`. `abstract` belongs to two productions and the
    // parser took it for (79)'s interface header outright, so this
    // failed with "expected `interface`". One token of lookahead tells
    // them apart. `ValuetypeKind::Abstract` was already in the model and
    // `valuetype_kind` already read the token.
    let parsed = parse("abstract valuetype AV { void f(); };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let av = find(&lowered.ast, &["AV"]);
    let TypeKind::Valuetype { kind, .. } = lowered.ast.type_(av).expect("AV").kind.clone() else {
        panic!("AV is not a valuetype");
    };
    assert_eq!(kind, zerodds_ast::ValuetypeKind::Abstract);
}

#[test]
fn abstract_still_reaches_the_interface_header_it_also_belongs_to() {
    // The lookahead must not take `abstract` away from (79).
    let parsed = parse("abstract interface AI { void f(); };");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let (_, node) = lowered
        .ast
        .interfaces()
        .find(|(_, node)| node.name == "AI")
        .expect("AI");
    assert_eq!(node.kind, InterfaceKind::Abstract);
}

#[test]
fn a_forward_declared_valuetype_is_completed_by_its_definition() {
    // Rule (105). The third instance of the forward bug: a bodiless
    // valuetype was declared complete, so its definition was a
    // `DuplicateName` rather than the body it is.
    for source in [
        "valuetype V; valuetype V { public long x; };",
        "abstract valuetype V; abstract valuetype V { void f(); };",
        "eventtype E; eventtype E { public long x; };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let lowered = lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source:?}: {:?}",
            lowered.diagnostics
        );
        let declared: Vec<_> = lowered
            .ast
            .types()
            .filter(|(_, node)| matches!(node.kind, TypeKind::Valuetype { .. }))
            .collect();
        assert_eq!(declared.len(), 1, "{source:?}: two nodes for one name");
        assert_eq!(
            declared[0].1.completeness,
            Completeness::Complete,
            "{source:?}"
        );
    }
}

#[test]
fn a_valuetype_forward_that_is_never_defined_is_still_reported() {
    assert_trips_lowering("valuetype V;", DiagnosticCode::UndefinedForward);
    assert_trips_lowering("abstract eventtype E;", DiagnosticCode::UndefinedForward);
}

#[test]
fn a_component_keeps_its_supports_clause_apart_from_its_base() {
    // `component C : B supports I1, I2` writes two inheritance clauses.
    // Only the first was read, so every `supports` list a component, home
    // or connector carried was dropped with no diagnostic. CCM 4.0 §6.3.2
    // needs both lists and needs them apart: `Components::CCMObject` is
    // added only when there is no base, and the supported interfaces are
    // added on top of whichever base applies.
    let ast = model(
        "interface I1 {}; interface I2 {}; component B {};
         component C : B supports I1, I2 {};",
    );
    let (_, component) = ast
        .interfaces()
        .find(|(_, node)| node.name == "C")
        .expect("C");
    assert_eq!(component.bases.len(), 1, "the base list took the supports");
    let supports = component
        .facets
        .corba
        .as_ref()
        .map_or(0, |corba| corba.supports.len());
    assert_eq!(supports, 2, "the supports clause was dropped");
}

#[test]
fn a_state_member_keeps_the_keyword_it_was_written_with() {
    // Rule (106). `public` and `private` were consumed and thrown away,
    // so CCM 4.0 §6.7.2 — no private state member in a primary-key
    // valuetype — could not be checked at all.
    let ast = model("valuetype V { public long a; private long b; };");
    let visibility = |name: &str| {
        ast.members()
            .find(|(_, node)| node.name == name)
            .and_then(|(_, node)| node.facets.corba.as_ref())
            .and_then(|corba| corba.state_visibility)
    };
    assert_eq!(
        visibility("a"),
        Some(zerodds_ast::StateVisibility::Public),
        "`public` was dropped"
    );
    assert_eq!(
        visibility("b"),
        Some(zerodds_ast::StateVisibility::Private),
        "`private` was dropped"
    );
}

#[test]
fn a_struct_member_has_no_state_visibility() {
    let ast = model("struct S { long a; };");
    let corba = ast
        .members()
        .find(|(_, node)| node.name == "a")
        .and_then(|(_, node)| node.facets.corba.as_ref())
        .and_then(|corba| corba.state_visibility);
    assert_eq!(corba, None, "a struct member gained a visibility keyword");
}

#[test]
fn an_interface_named_where_a_type_is_expected_is_an_object_reference() {
    // IDL 4.2 §7.4.6. An interface is not a type, and nothing offered the
    // type that stands for a reference to one, so every use of an
    // interface as a data type broke with `KindMismatch` — a struct
    // member, an operation's return, a parameter, and every `provides`
    // port of every component.
    let mut ast = model("interface I {}; struct S { I x; }; interface J { I ping(in I a); };");
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);

    let (_, member) = ast
        .members()
        .find(|(_, node)| node.name == "x")
        .expect("S::x");
    let target = member.type_.resolved().expect("S::x resolved");
    assert!(
        matches!(
            ast.type_(target).expect("target").kind,
            TypeKind::ObjectRef { interface: Some(_) }
        ),
        "S::x is not an object reference"
    );
}

#[test]
fn a_home_resolves_what_it_manages_and_the_key_it_manages_by() {
    // The CORBA facet's references were unreachable from `pending_slots`,
    // so they were never indexed: `manages C primarykey K` stayed
    // `Pending` however many resolution passes ran over the model.
    let mut ast = model(
        "component C;
         home H manages C primarykey CKey {};
         component C {};
         valuetype CKey { public long v; };",
    );
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);

    let (_, home) = ast
        .interfaces()
        .find(|(_, node)| node.name == "H")
        .expect("H");
    let manages = home
        .facets
        .corba
        .as_ref()
        .and_then(|corba| corba.manages.as_ref())
        .expect("the manages clause");
    assert!(
        manages.component.is_resolved(),
        "`manages C` never resolved"
    );
    assert!(
        manages
            .primary_key
            .as_ref()
            .is_some_and(zerodds_ast_core::Ref::is_resolved),
        "`primarykey CKey` never resolved"
    );
}

#[test]
fn a_ports_role_resolves_the_interface_or_event_behind_it() {
    let mut ast = model(
        "component C { provides I1 p; consumes Tick s; };
         interface I1 {}; eventtype Tick {};",
    );
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);

    let (id, _) = ast
        .interfaces()
        .find(|(_, node)| node.name == "C")
        .expect("C");
    let attributes = ast.interface(id).expect("C").attributes.clone();
    let mut seen = 0;
    for attribute in attributes {
        let role = ast
            .attribute(attribute)
            .expect("port")
            .facets
            .corba
            .as_ref()
            .and_then(|corba| corba.component_role.clone())
            .expect("a component role");
        match role {
            zerodds_ast::ComponentRole::Facet { interface, .. } => {
                assert!(interface.is_resolved(), "`provides I1 p` never resolved");
                seen += 1;
            }
            zerodds_ast::ComponentRole::EventSink { event, .. } => {
                assert!(event.is_resolved(), "`consumes Tick s` never resolved");
                seen += 1;
            }
            other => panic!("unexpected role {other:?}"),
        }
    }
    assert_eq!(seen, 2, "not every port carried a role");
}

/// A constant named by its simple name from inside the scope that
/// declares it. §7.5: a simple name walks the enclosing scopes.
///
/// `Lowerer::scoped_name_of` marks every name it reads absolute, whether
/// or not the source wrote a leading `::`, and a pending reference carries
/// no scope — so a simple name left pending was later looked up from the
/// root and reported `NotFound`. This is the shape it was found in, where
/// the bound and the constant sit in one module:
#[test]
fn a_constant_resolves_by_its_simple_name_in_its_own_scope() {
    let ast = model(
        r"
        module sensing { module common {
            const unsigned long SZ_SMALL = 256;
            struct BlobRef { string uri; };
            struct FrameHeader { sequence<BlobRef, SZ_SMALL> blobs; };
        }; };
    ",
    );
    let header = find(&ast, &["sensing", "common", "FrameHeader"]);
    let member = ast.members_of(header).first().copied().expect("member");
    let sequence = ast
        .member(member)
        .expect("member node")
        .type_
        .resolved()
        .expect("the sequence type");
    let TypeKind::Sequence { bound, .. } = ast.type_(sequence).expect("type").kind else {
        panic!("the member is a sequence")
    };
    let bound = bound.expr().expect("the sequence is bounded");
    assert_eq!(
        ast.eval_integral(bound).expect("the bound evaluates"),
        256,
        "the bound is the constant declared beside it"
    );
}

/// The same name from outside its module still needs its path, and a name
/// that reaches nothing at all is still pending rather than silently zero.
#[test]
fn a_qualified_constant_still_resolves_through_its_module() {
    let ast = model(
        r"
        module m { const unsigned long CAP = 8; };
        module other { struct S { sequence<long, m::CAP> xs; }; };
    ",
    );
    let s = find(&ast, &["other", "S"]);
    let member = ast.members_of(s).first().copied().expect("member");
    let sequence = ast
        .member(member)
        .expect("member node")
        .type_
        .resolved()
        .expect("the sequence type");
    let TypeKind::Sequence { bound, .. } = ast.type_(sequence).expect("type").kind else {
        panic!("the member is a sequence")
    };
    let bound = bound.expr().expect("the sequence is bounded");
    assert_eq!(ast.eval_integral(bound).expect("evaluates"), 8);
}

/// A constant whose value no integer represents. `eval_integral` answers
/// `NotIntegral`, which is correct where a number is required and useless
/// where a backend only needs to know what the constant says — a string
/// constant used to be unemittable for exactly that reason.
#[test]
fn a_string_constant_evaluates_even_though_no_integer_holds_it() {
    use zerodds_ast::ConstValue;
    let ast = model(r#" module m { const string MODULE_ID = "spatial.sensing.lidar/1.5"; }; "#);
    let (_, node) = ast
        .constants()
        .find(|(_, node)| node.name == "MODULE_ID")
        .expect("the constant");
    assert!(
        ast.eval_integral(node.expr).is_err(),
        "no integer represents it"
    );
    assert_eq!(
        ast.eval_const(node.expr).expect("it still has a value"),
        ConstValue::String(String::from("spatial.sensing.lidar/1.5"))
    );
}

/// Arithmetic stays integral — IDL has no operator over a string — and a
/// reference to another constant is followed.
#[test]
fn eval_const_follows_a_reference_and_keeps_arithmetic_integral() {
    use zerodds_ast::ConstValue;
    let ast = model(
        r"
        module m {
            const unsigned long BASE = 64;
            const unsigned long DOUBLE = BASE * 2;
        };
    ",
    );
    let (_, node) = ast
        .constants()
        .find(|(_, node)| node.name == "DOUBLE")
        .expect("the constant");
    assert_eq!(
        ast.eval_const(node.expr).expect("evaluates"),
        ConstValue::Integer(128)
    );
}

/// `#pragma keylist` and `keylist T (a);` are RTI's and Cyclone's
/// spelling of `@key`, and they have to reach the member.
///
/// The directive used to be stored on the vendor facet and read by
/// nobody: `Facets::is_key` answers from `xtypes.is_key` alone, and the
/// only other reader of `vendor.rti.keylist` in the tree is the snapshot
/// serialiser. A Cyclone `.idl` therefore produced a type with **no key
/// at all** — every sample in one instance, a constant KeyHash, and
/// nothing reporting it.
#[test]
fn a_keylist_pragma_makes_the_named_member_a_key() {
    let ast = model("struct Sample { long id; long payload; };\n#pragma keylist Sample id\n");
    let sample = find(&ast, &["Sample"]);
    let members = ast.members_of(sample);
    let key_of = |name: &str| {
        members
            .iter()
            .find(|id| ast.member(**id).expect("member").name == name)
            .map(|id| ast.member(*id).expect("member").facets.is_key())
            .expect("the member")
    };
    assert!(key_of("id"), "the directive names it");
    assert!(!key_of("payload"), "and does not name this one");
}

/// The vendor facet is kept as well: a round trip back to that dialect
/// needs to write the directive out again.
#[test]
fn a_keylist_is_also_kept_on_the_vendor_facet() {
    let ast = model("struct Sample { long id; };\n#pragma keylist Sample id\n");
    let sample = find(&ast, &["Sample"]);
    let facets = &ast.type_(sample).expect("type").facets;
    let rti = facets.vendor.rti.as_ref().expect("the vendor facet");
    assert_eq!(rti.keylist, vec![String::from("id")]);
}

/// A field the type does not declare is reported. Keying nothing in
/// silence is the same failure the fix above removed.
#[test]
fn a_keylist_naming_a_field_that_is_not_there_is_reported() {
    let parsed = parse("struct Sample { long id; };\n#pragma keylist Sample nope\n");
    let found = lower(&parsed.syntax(), "test.idl").diagnostics;
    assert!(
        found
            .iter()
            .any(|d| d.code == DiagnosticCode::UnresolvedName && d.message.contains("nope")),
        "{found:?}"
    );
}
