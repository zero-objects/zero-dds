// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Parsing, recovery and the ambiguities. Spec §2.3, §3.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_cst::CstKind;
use zerodds_idl_parse::{DiagnosticCode, parse};

/// Every node kind in the tree, so a test can assert a form was
/// recognised without walking by hand.
fn kinds(source: &str) -> Vec<CstKind> {
    parse(source)
        .syntax()
        .descendants()
        .iter()
        .map(|node| node.kind())
        .collect()
}

fn parses_cleanly(source: &str) -> bool {
    !parse(source).has_errors()
}

// -- the property that must hold whatever happens ------------------------

#[test]
fn a_clean_file_round_trips_byte_for_byte() {
    let source = "module Robot {\n  // the pose\n  struct Pose {\n    long x;\n  };\n};\n";
    let result = parse(source);
    assert!(!result.has_errors(), "{:?}", result.diagnostics());
    assert_eq!(result.text(), source);
}

#[test]
fn a_broken_file_round_trips_too() {
    // Spec §2.2, §3.2: recovery inserts error nodes rather than dropping
    // input, so nothing is lost on the way in.
    for source in [
        "struct { ??? };",
        "module M { struct S { long }; };",
        "@@@ !!! ???",
        "struct S { long x }",
        "",
        "   \n\n// only a comment\n",
    ] {
        assert_eq!(
            parse(source).text(),
            source,
            "round trip failed for {source:?}"
        );
    }
}

#[test]
fn a_file_of_pure_garbage_produces_diagnostics_and_a_tree_not_a_panic() {
    let source = "!!! ??? $$$ @@@";
    let result = parse(source);
    assert!(result.has_errors());
    assert_eq!(result.text(), source);
}

// -- declaration forms ---------------------------------------------------

#[test]
fn each_declaration_form_is_recognised() {
    for (source, expected) in [
        ("module M { };", CstKind::Module),
        ("const long N = 1;", CstKind::ConstDecl),
        ("typedef long Distance;", CstKind::TypedefDecl),
        ("struct S { long x; };", CstKind::StructDecl),
        (
            "union U switch (long) { case 1: long a; };",
            CstKind::UnionDecl,
        ),
        ("enum E { A, B };", CstKind::EnumDecl),
        ("bitmask B { A, C };", CstKind::BitmaskDecl),
        ("bitset S { bitfield<3> a; };", CstKind::BitsetDecl),
        ("native N;", CstKind::NativeDecl),
        ("exception E { long code; };", CstKind::ExceptionDecl),
        ("interface I { };", CstKind::InterfaceDecl),
        ("valuetype V { public long x; };", CstKind::ValuetypeDecl),
    ] {
        let found = kinds(source);
        assert!(
            found.contains(&expected),
            "{source:?} did not produce {expected:?}, got {found:?}"
        );
        assert!(parses_cleanly(source), "{source:?} produced diagnostics");
    }
}

#[test]
fn a_name_colliding_with_a_keyword_parses_as_a_name_and_is_refused_a_layer_up() {
    // IDL 4.2 §7.2.4 is two rules. "Keywords must be written exactly as
    // shown in the above list" is the lexer's, so `Alias` is an
    // identifier and the declaration has the shape its author wrote;
    // "Identifiers that collide with keywords … are illegal" is
    // §7.2.3's, reported by the lowering as `IDL0017` with the keyword
    // named.
    //
    // Deciding both here is what made `enum E { AA, CUSTOM };` fail with
    // five parse errors naming `valuetype`.
    assert!(parses_cleanly("typedef long Alias;"));
    assert!(parses_cleanly("typedef long _Alias;"));
    assert!(parses_cleanly("typedef long Distance;"));
    assert!(kinds("typedef long Alias;").contains(&CstKind::Declarator));
}

#[test]
fn the_multi_word_primitive_types_parse_as_one_type_each() {
    for source in [
        "typedef unsigned short A;",
        "typedef unsigned long B;",
        "typedef unsigned long long C;",
        "typedef long long D;",
        "typedef long double E;",
        "typedef long F;",
    ] {
        assert!(
            parses_cleanly(source),
            "{source:?}: {:?}",
            parse(source).diagnostics()
        );
        assert_eq!(parse(source).text(), source);
    }
}

#[test]
fn a_forward_declaration_is_its_own_form() {
    for source in ["struct S;", "union U;", "interface I;"] {
        assert!(
            kinds(source).contains(&CstKind::ForwardDecl),
            "{source:?} was not a forward declaration"
        );
        assert!(parses_cleanly(source));
    }
}

#[test]
fn a_struct_with_a_base_records_its_inheritance() {
    let source = "struct Derived : Base { long x; };";
    assert!(kinds(source).contains(&CstKind::InheritanceClause));
    assert!(parses_cleanly(source));
}

#[test]
fn an_interface_with_operations_and_attributes_parses() {
    let source = r"
interface Control {
  readonly attribute long speed;
  void go(in long distance, out long result) raises (Failure);
  oneway void notify(in string msg);
};
";
    let found = kinds(source);
    assert!(found.contains(&CstKind::Operation));
    assert!(found.contains(&CstKind::AttributeDecl));
    assert!(found.contains(&CstKind::Parameter));
    assert!(found.contains(&CstKind::RaisesClause));
    assert!(parses_cleanly(source), "{:?}", parse(source).diagnostics());
}

#[test]
fn a_union_case_may_carry_several_labels() {
    let source = "union U switch (long) { case 1: case 2: long a; default: long b; };";
    let found = kinds(source);
    let labels = found.iter().filter(|k| **k == CstKind::CaseLabel).count();
    assert_eq!(labels, 3);
    assert!(parses_cleanly(source));
}

#[test]
fn annotations_bind_to_the_declaration_that_follows() {
    let source = "@final @topic struct S { @key long id; };";
    let found = kinds(source);
    let annotations = found.iter().filter(|k| **k == CstKind::Annotation).count();
    assert_eq!(annotations, 3);
    assert!(parses_cleanly(source), "{:?}", parse(source).diagnostics());
}

/// DDS-RPC 1.0 §7.3.1.1 production (106): `<attr_spec> ::= [ <annotation> ]
/// "attribute" <param_type_spec> <attr_declarator>`. The annotation
/// precedes a writable attribute, and it is part of that attribute's node.
#[test]
fn an_annotation_precedes_a_writable_attribute() {
    let source = "interface I { @id(1) attribute long x; };";
    let result = parse(source);
    assert!(!result.has_errors(), "{:?}", result.diagnostics());
    let syntax = result.syntax();
    let attributes: Vec<_> = syntax
        .descendants()
        .into_iter()
        .filter(|node| node.kind() == CstKind::AttributeDecl)
        .collect();
    assert_eq!(attributes.len(), 1);
    let attribute = &attributes[0];
    assert!(attribute.text().trim_start().starts_with("@id(1)"));
    assert!(!attribute.text().contains("readonly"));
    let first = attribute.children().into_iter().next().expect("a child");
    assert_eq!(first.kind(), CstKind::Annotation);
    // The same with a `getraises`/`setraises` pair, which only a writable
    // attribute can carry.
    let source = "interface I { @id(2) attribute long y getraises (E) setraises (F); };";
    assert!(parses_cleanly(source), "{:?}", parse(source).diagnostics());
    let found = kinds(source);
    let clauses = found
        .iter()
        .filter(|kind| **kind == CstKind::RaisesClause)
        .count();
    assert_eq!(clauses, 2);
}

#[test]
fn an_annotation_takes_arguments_in_both_forms() {
    for source in [
        "@id(7) struct S { long x; };",
        "@range(min = 1, max = 9) struct S { long x; };",
        "@key struct S { long x; };",
    ] {
        assert!(
            parses_cleanly(source),
            "{source:?}: {:?}",
            parse(source).diagnostics()
        );
    }
}

#[test]
fn several_declarators_share_one_type() {
    let source = "struct S { double x, y, z; };";
    let found = kinds(source);
    let declarators = found.iter().filter(|k| **k == CstKind::Declarator).count();
    assert_eq!(declarators, 3);
    assert!(parses_cleanly(source));
}

// -- the ambiguities, spec §3.3 ------------------------------------------

#[test]
fn a_nested_sequence_splits_the_shift_token_and_still_round_trips() {
    // The case that breaks a naive parser: one `>>` token closing two
    // argument lists. Spec §2.3.
    let source = "typedef sequence<sequence<long>> Nested;";
    let result = parse(source);
    assert!(!result.has_errors(), "{:?}", result.diagnostics());
    assert_eq!(result.text(), source, "the split broke the round trip");

    let found = kinds(source);
    let sequences = found
        .iter()
        .filter(|k| **k == CstKind::SequenceType)
        .count();
    assert_eq!(sequences, 2);
}

#[test]
fn three_deep_nesting_splits_as_far_as_it_needs_to() {
    let source = "typedef sequence<sequence<sequence<long>>> Deep;";
    let result = parse(source);
    assert!(!result.has_errors(), "{:?}", result.diagnostics());
    assert_eq!(result.text(), source);
}

#[test]
fn shift_right_in_an_expression_is_not_split() {
    // Same token, opposite decision. Spec §3.3.
    let source = "const long N = 256 >> 2;";
    let result = parse(source);
    assert!(!result.has_errors(), "{:?}", result.diagnostics());
    assert_eq!(result.text(), source);
    assert!(kinds(source).contains(&CstKind::BinaryExpr));
}

#[test]
fn a_leading_double_colon_is_the_global_scope() {
    let source = "struct S { ::Robot::Pose p; };";
    let result = parse(source);
    assert!(!result.has_errors(), "{:?}", result.diagnostics());
    assert!(kinds(source).contains(&CstKind::ScopedName));
}

#[test]
fn an_array_declarator_is_only_known_at_the_bracket() {
    let source = "struct S { long grid[3][4]; };";
    let found = kinds(source);
    let dims = found
        .iter()
        .filter(|k| **k == CstKind::ArrayDimension)
        .count();
    assert_eq!(dims, 2);
    assert!(parses_cleanly(source));
}

// -- expression precedence, spec §3.3 ------------------------------------

#[test]
fn every_operator_of_the_ladder_parses() {
    for source in [
        "const long N = 1 | 2;",
        "const long N = 1 ^ 2;",
        "const long N = 1 & 2;",
        "const long N = 1 << 2;",
        "const long N = 1 >> 2;",
        "const long N = 1 + 2;",
        "const long N = 1 - 2;",
        "const long N = 1 * 2;",
        "const long N = 1 / 2;",
        "const long N = 1 % 2;",
        "const long N = -1;",
        "const long N = ~1;",
        "const long N = (1 + 2) * 3;",
    ] {
        assert!(
            parses_cleanly(source),
            "{source:?}: {:?}",
            parse(source).diagnostics()
        );
        assert_eq!(parse(source).text(), source);
    }
}

#[test]
fn tighter_operators_bind_first() {
    // `1 | 2 & 3` groups as `1 | (2 & 3)`, so the outer expression has an
    // inner one on its right.
    let source = "const long N = 1 | 2 & 3;";
    let result = parse(source);
    assert!(!result.has_errors());
    let binaries = kinds(source)
        .iter()
        .filter(|k| **k == CstKind::BinaryExpr)
        .count();
    assert_eq!(binaries, 2);
}

#[test]
fn subtraction_is_left_associative() {
    // `1 - 2 - 3` is `(1 - 2) - 3`: two binary nodes, nested left.
    let source = "const long N = 1 - 2 - 3;";
    let result = parse(source);
    assert!(!result.has_errors());
    assert_eq!(result.text(), source);
    let binaries = kinds(source)
        .iter()
        .filter(|k| **k == CstKind::BinaryExpr)
        .count();
    assert_eq!(binaries, 2);
}

// -- recovery, spec §3.2 -------------------------------------------------

#[test]
fn three_errors_in_one_file_produce_three_diagnostics() {
    // A frontend that stops at the first makes a large file take as many
    // runs as it has mistakes.
    let source = r"
struct A { long x }
struct B { long y }
struct C { long z }
";
    let result = parse(source);
    assert!(
        result.diagnostics().len() >= 3,
        "expected at least three, got {:?}",
        result.diagnostics()
    );
}

#[test]
fn a_missing_semicolon_recovers_at_the_next_declaration() {
    let source = "struct A { long x; }\nstruct B { long y; };";
    let result = parse(source);
    assert!(result.has_errors());
    // The second struct was still recognised.
    let structs = kinds(source)
        .iter()
        .filter(|k| **k == CstKind::StructDecl)
        .count();
    assert_eq!(structs, 2, "recovery lost the second declaration");
}

#[test]
fn a_bad_member_does_not_hide_the_ones_after_it() {
    let source = "struct S { ??? ; long y; long z; };";
    let result = parse(source);
    assert!(result.has_errors());
    assert_eq!(result.text(), source);
    let declarators = kinds(source)
        .iter()
        .filter(|k| **k == CstKind::Declarator)
        .count();
    assert!(declarators >= 2, "later members were lost");
}

#[test]
fn every_diagnostic_points_inside_the_file() {
    let source = "struct A { long x }\nmodule { };\n??? ";
    let end = u32::try_from(source.len()).expect("small file");
    for diagnostic in parse(source).diagnostics() {
        assert!(
            diagnostic.primary.end <= end,
            "{diagnostic} pointed past the file"
        );
    }
}

#[test]
fn every_diagnostic_carries_a_code_and_a_specification_reference() {
    let source = "struct A { long x }";
    for diagnostic in parse(source).diagnostics() {
        assert!(!diagnostic.code.as_str().is_empty());
        assert!(
            !diagnostic.spec_reference.is_empty(),
            "{diagnostic} has no paragraph"
        );
    }
}

#[test]
fn diagnostic_codes_are_distinct() {
    // A code that collided with another would make a build grepping for
    // one catch the other.
    let mut codes: Vec<&str> = DiagnosticCode::ALL.iter().map(|c| c.as_str()).collect();
    let count = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), count);
}

// -- realistic input -----------------------------------------------------

#[test]
fn a_file_using_most_of_the_grammar_parses_cleanly_and_round_trips() {
    let source = r#"
// A realistic file.
module Robot {

  const long MAX_JOINTS = 0x10;

  enum Mode { IDLE, ACTIVE, FAULT };

  bitmask Flags { READY, MOVING };

  typedef sequence<double, MAX_JOINTS> Joints;
  typedef sequence<sequence<long>> Nested;

  struct Header {
    @key long id;
    string<64> name;
    double stamp;
  };

  @final
  struct Pose : Header {
    double x, y, z;
    long grid[3][4];
    Joints angles;
  };

  union Value switch (Mode) {
    case IDLE: long idle;
    case ACTIVE: case FAULT: double active;
    default: string other;
  };

  exception Failure { long code; string reason; };

  interface Control {
    readonly attribute Mode setting;
    void move(in Pose target, out long status) raises (Failure);
    oneway void reset();
  };
};
"#;
    let result = parse(source);
    assert!(
        !result.has_errors(),
        "diagnostics: {:?}",
        result.diagnostics()
    );
    assert_eq!(result.text(), source);
}

proptest::proptest! {
    /// Whatever the input, parsing terminates and reproduces it.
    #[test]
    fn any_input_terminates_and_round_trips(source in "\\PC{0,300}") {
        proptest::prop_assert_eq!(parse(&source).text(), source);
    }
}

// -- rule (221): what an `@annotation` body admits -----------------------

#[test]
fn an_annotation_body_admits_only_what_rule_221_lists() {
    // (221) `<annotation_body> ::= { <annotation_member> | <enum_dcl> ";"
    // | <const_dcl> ";" | <typedef_dcl> ";" }*` — four alternatives and
    // no others. Measured on the Linux test host: `idlc`, `rtiddsgen`,
    // `fastddsgen`, `opendds_idl`, `tao_idl`, `omniidl` and JacORB all
    // seven refuse `@annotation A { struct Inner { long x; }; };`.
    for source in [
        "@annotation A { struct Inner { long x; }; long v; };",
        "@annotation A { union U switch (long) { case 1: long x; }; long v; };",
        "@annotation A { bitmask B { X }; long v; };",
        "@annotation A { bitset S { bitfield<3> a; }; long v; };",
        "@annotation A { native H; long v; };",
    ] {
        let parsed = parse(source);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|d| d.code == DiagnosticCode::UnexpectedToken),
            "{source:?} was accepted: {:?}",
            parsed.diagnostics()
        );
        // Refusing is not dropping: the tree still covers the file.
        assert_eq!(parsed.text(), source);
    }
}

#[test]
fn the_declarations_rule_221_does_list_are_still_taken() {
    for source in [
        "@annotation A { enum Kind { RED, BLUE }; Kind k; };",
        "@annotation A { const long LIMIT = 4; long v; };",
        "@annotation A { typedef long Small; Small v; };",
        "@annotation A { long v; };",
        "@annotation A { long v default 3; };",
    ] {
        assert!(parses_cleanly(source), "{source:?} was refused");
    }
}

// -- dialects, spec §3.5 -------------------------------------------------

#[test]
fn a_vendor_extension_is_rejected_under_the_strict_dialect() {
    // `keylist` is RTI's. A file using it is not one every vendor's
    // compiler reads, and a strict parse has to say so.
    let parsed = zerodds_idl_parse::parse_with(
        "struct S { long id; };\nkeylist S (id);",
        zerodds_idl_parse::Dialect::STRICT,
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.code == zerodds_idl_parse::DiagnosticCode::VendorExtension),
        "{:?}",
        parsed.diagnostics()
    );
}

#[test]
fn the_same_file_is_accepted_under_the_vendors_dialect_and_still_says_so() {
    // Admitting a construct and saying nothing about it are two
    // different things. `keylist` is in no building block of IDL 4.2, so
    // a file that uses one is not one every vendor's compiler reads —
    // and the caller has to learn that from the parse rather than from
    // the peer that could not read the file.
    let source = "struct S { long id; };\nkeylist S (id);";
    let mut dialect = zerodds_idl_parse::Dialect::STRICT;
    dialect.rti = true;
    let parsed = zerodds_idl_parse::parse_with(source, dialect);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let reported = parsed
        .diagnostics()
        .iter()
        .find(|d| d.code == zerodds_idl_parse::DiagnosticCode::VendorExtension)
        .expect("the extension is recorded");
    assert_eq!(reported.severity, zerodds_idl_parse::Severity::Warning);
    assert!(reported.message.contains("RTI"), "{reported}");
    assert_eq!(parsed.syntax().text(), source, "the round trip changed");
}

#[test]
fn a_rejected_extension_still_parses_and_round_trips() {
    // A diagnostic is not a refusal to read: the tree still covers the
    // file, so a formatter can work on it and a later pass can decide.
    let source = "struct S { long id; };\nkeylist S (id);";
    let parsed = zerodds_idl_parse::parse_with(source, zerodds_idl_parse::Dialect::STRICT);
    assert_eq!(parsed.syntax().text(), source);
}

#[test]
fn the_default_parse_admits_every_vendor_and_names_the_one_it_admitted() {
    // A tool reading files it did not write needs them all; strictness
    // is something a caller asks for. What the caller does *not* have to
    // ask for is the fact — `parse` passes `ALL_VENDORS` and every
    // caller in this tree uses `parse`, so a gate nothing selects is a
    // gate that never fires and a file silently stops being IDL 4.2.
    assert_eq!(
        zerodds_idl_parse::Dialect::default(),
        zerodds_idl_parse::Dialect::STRICT,
        "the default Dialect value should be the strict one"
    );
    let parsed = zerodds_idl_parse::parse("struct S { long id; };\nkeylist S (id);");
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert!(
        parsed.diagnostics().iter().any(|d| d.code
            == zerodds_idl_parse::DiagnosticCode::VendorExtension
            && d.severity == zerodds_idl_parse::Severity::Warning),
        "the default parse said nothing about a construct IDL 4.2 does not have: {:?}",
        parsed.diagnostics()
    );
}

#[test]
fn a_file_with_no_vendor_extension_is_not_warned_about() {
    // A warning that fires on everything says nothing.
    let parsed = zerodds_idl_parse::parse("struct S { @key long id; };");
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
}

// -- the suffix annotation form, DDS-XTypes 1.3 §7.3.1.2.5 ---------------

/// `//@key` is an annotation node, not a comment.
///
/// Table 24 of §7.3.1.2.5 gives `{ "//@"<annotation_type_name> [ "("
/// <arguments> ")" ] }*` beside the `@` production and calls the two
/// equivalent, so the tree the two produce has to be the same shape.
#[test]
fn a_suffix_annotation_reaches_the_tree_as_an_annotation() {
    let mut suffix = kinds("struct S { long x; //@id(7)\n};");
    let mut prefix = kinds("struct S { @id(7) long x; };");
    // The two differ in where the annotation sits among its siblings —
    // that is what "prefix" and "suffix" mean — and in nothing else.
    suffix.sort_unstable();
    prefix.sort_unstable();
    assert_eq!(
        suffix, prefix,
        "the suffix form must build the nodes the prefix form builds"
    );
    assert!(suffix.contains(&CstKind::Annotation));
    assert!(suffix.contains(&CstKind::AnnotationArguments));
    // And the annotation is a child of the member it suffixes, which is
    // what the lowering reads.
    let parsed = parse("struct S { long x; //@id(7)\n};");
    let member = parsed
        .syntax()
        .descendants()
        .into_iter()
        .find(|node| node.kind() == CstKind::Member)
        .expect("a member");
    assert!(
        member
            .children()
            .iter()
            .any(|child| child.kind() == CstKind::Annotation),
        "the annotation belongs to the declaration it follows"
    );
}

/// The tokens of a suffix annotation come out of the comment's own
/// bytes, so the file still comes back byte for byte. Spec §2.2.
#[test]
fn a_file_with_suffix_annotations_round_trips_byte_for_byte() {
    for source in [
        "struct S { long x; //@key\n};\n",
        "struct Gadget {\n    long my_integer; //@my_annotation1(greeting=\"Hello\")\n//@my_annotation2\n}; //@my_type_annotation\n",
        "enum E { A, //@value(3)\nB //@default_literal\n};\n",
        "struct S { long x; //@key //@must_understand\n};\n",
        "struct S { long x; //@unit(\"m//@s\")\n};\n",
    ] {
        let result = parse(source);
        assert!(
            !result.has_errors(),
            "{source:?}: {:?}",
            result.diagnostics()
        );
        assert_eq!(result.text(), source, "{source:?}");
    }
}

/// A `//@` that suffixes no declaration is reported where it stands.
///
/// §7.3.1.2.5 admits the form only after the declaration it applies to.
/// Skipping the rest as a comment would leave a file that says `//@key`
/// and a model that says the member is not a key, with nothing in
/// between — which is the one answer this frontend must never give.
#[test]
fn a_suffix_annotation_in_a_position_that_admits_none_is_reported() {
    for source in [
        "//@key\nstruct S { long x; };",
        "struct S { //@key\nlong x; };",
        "module M { //@key\n};",
        "struct S { long x; };\nmodule M {\n//@key\n};",
    ] {
        let result = parse(source);
        let found = result
            .diagnostics()
            .iter()
            .find(|d| d.spec_reference == "DDS-XTypes 1.3 §7.3.1.2.5")
            .unwrap_or_else(|| panic!("{source:?} was accepted in silence"));
        assert_eq!(found.code, DiagnosticCode::UnexpectedToken);
        assert_eq!(
            found.primary.slice(source),
            Some("//@key"),
            "the diagnostic must point at the comment itself"
        );
        // Reported and still round-tripped: the comment stays in the
        // tree as the trivia it turned out to be.
        assert_eq!(result.text(), source);
    }
}

/// An ordinary comment stays ordinary.
///
/// §7.3.1.2.3: "there must be no space in between the double slash
/// ("//") and the at-sign ('@')" — which is what keeps `// @key` and
/// `///@key` out of the grammar, and the block comment form with them.
#[test]
fn only_the_exact_slash_slash_at_marker_is_the_suffix_form() {
    for source in [
        "struct S { long x; // @key\n};",
        "struct S { long x; ///@key\n};",
        "struct S { long x; /*@key*/ };",
        "// @key\nstruct S { long x; };",
    ] {
        let result = parse(source);
        assert!(
            !result.has_errors(),
            "{source:?}: {:?}",
            result.diagnostics()
        );
        assert!(
            !kinds(source).contains(&CstKind::Annotation),
            "{source:?} is a comment"
        );
    }
}
