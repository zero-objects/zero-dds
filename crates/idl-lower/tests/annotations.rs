// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotations reaching their typed fields. Spec §4, §5.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{
    AnyId, Ast, AutoId, Extensibility, Facets, MemberIdOrigin, TryConstruct, TypeKind,
    VerbatimPlacement, XTypesFacet,
};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_idl_lower::{KeyOnUnionDiscriminator, Options, lower, lower_with};
use zerodds_idl_parse::parse;

fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower(&parsed.syntax(), "test.idl").ast
}

fn diagnostics(source: &str) -> Vec<zerodds_idl_parse::Diagnostic> {
    let parsed = parse(source);
    lower(&parsed.syntax(), "test.idl").diagnostics
}

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

fn type_of(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&scoped(&[name]))
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{name} is not in the model"))
}

fn facets_of_type<'a>(ast: &'a Ast, name: &str) -> &'a Facets {
    let id = type_of(ast, name);
    let any = ast.any_of_type(id).expect("any");
    ast.facets(any).expect("facets")
}

fn xtypes_of_type<'a>(ast: &'a Ast, name: &str) -> &'a XTypesFacet {
    facets_of_type(ast, name)
        .xtypes
        .as_ref()
        .unwrap_or_else(|| panic!("{name} carries no xtypes facet"))
}

/// The XTypes facet of the first member of a struct.
fn xtypes_of_member<'a>(ast: &'a Ast, type_name: &str, index: usize) -> &'a XTypesFacet {
    let id = type_of(ast, type_name);
    let member = ast
        .members_of(id)
        .get(index)
        .copied()
        .unwrap_or_else(|| panic!("{type_name} has no member {index}"));
    let node = ast.member(member).expect("member");
    ast.facets(AnyId::Member(member, node.node_id))
        .expect("facets")
        .xtypes
        .as_ref()
        .unwrap_or_else(|| panic!("member {index} of {type_name} carries no xtypes facet"))
}

// -- extensibility -------------------------------------------------------

/// `@key` on a union discriminator is carried by default.
///
/// Table 21 lists `@key` against "Structure Members, Union
/// discriminator" and §7.2.2.4.4.4.8's own example is
/// `union MyCommand switch (@key CommandKind)`. The default front end
/// therefore accepts the declaration and records the key.
#[test]
fn a_key_on_a_union_discriminator_is_carried_by_default() {
    let idl = "\
enum CommandKind { START, STOP };
@final union MyCommand switch (@key CommandKind) {
  case START: long a;
  default: short b;
};";
    assert!(
        diagnostics(idl).is_empty(),
        "the clause permits it: {:?}",
        diagnostics(idl)
    );
    let ast = model(idl);
    assert!(ast.discriminator_is_key(type_of(&ast, "MyCommand")));
}

/// `KeyOnUnionDiscriminator::Refused` reports the declaration, with the
/// clause that permits it, and does not say the annotation has no
/// meaning there.
///
/// It is the setting for a tool that has to stay within what Cyclone's
/// `idlc` and `rtiddsgen` accept (both refuse the file) and `fastddsgen`
/// (no topic type for a union at all).
#[test]
fn a_key_on_a_union_discriminator_is_refused_with_the_clause_that_permits_it() {
    let idl = "\
enum CommandKind { START, STOP };
@final union MyCommand switch (@key CommandKind) {
  case START: long a;
  default: short b;
};";
    let refused = Options {
        key_on_union_discriminator: KeyOnUnionDiscriminator::Refused,
        ..Options::default()
    };
    let parsed = parse(idl);
    let diagnostics = lower_with(&parsed.syntax(), "annotations.idl", refused).diagnostics;
    let found = diagnostics
        .iter()
        .find(|d| d.spec_reference.contains("7.2.2.4.4.4.8"))
        .unwrap_or_else(|| panic!("no diagnostic naming the clause: {diagnostics:?}"));
    assert!(
        found.message.contains("union discriminator"),
        "{}",
        found.message
    );
    assert!(
        !found.message.contains("has no meaning"),
        "the clause permits it; the diagnostic must not say otherwise: {}",
        found.message
    );
}

/// Under `KeyOnUnionDiscriminator::Carried`, `@key` on a union
/// discriminator reaches the facet.
///
/// Table 19's `UNION_TYPE` rule — "Either the discriminators of both T1
/// and T2 are keys or neither are keys" — is the downstream reader, and
/// with the annotation dropped that rule could not tell a keyed
/// discriminator from an unkeyed one and matched the two. From the
/// facet it reaches `UnionDiscriminatorFlag::IS_KEY` in the TypeObject
/// and the KeyHash holder that `zerodds_ast_layout::keyhash` builds
/// from the discriminator alone.
#[test]
fn a_key_on_a_union_discriminator_reaches_the_facet_when_carried() {
    let carried = Options {
        key_on_union_discriminator: KeyOnUnionDiscriminator::Carried,
        ..Options::default()
    };
    let idl = "\
enum CommandKind { START, STOP };
@final union MyCommand switch (@key CommandKind) {
  case START: long a;
  default: short b;
};";
    let parsed = parse(idl);
    let lowered = lower_with(&parsed.syntax(), "annotations.idl", carried);
    assert!(
        !lowered
            .diagnostics
            .iter()
            .any(zerodds_idl_parse::Diagnostic::is_error),
        "the axis was asked to carry the annotation: {:?}",
        lowered.diagnostics
    );
    let ast = lowered.ast;
    let union = ast
        .types()
        .find(|(_, node)| node.name == "MyCommand")
        .map(|(id, _)| id)
        .expect("the union");
    assert!(
        ast.discriminator_is_key(union),
        "`@key` on the discriminator has to reach the facet"
    );

    // And a discriminator without it stays unkeyed, so the facet is
    // reporting the annotation and not a constant.
    let plain = "\
enum CommandKind { START, STOP };
@final union Other switch (CommandKind) {
  case START: long a;
  default: short b;
};";
    let parsed = parse(plain);
    let ast = lower_with(&parsed.syntax(), "annotations.idl", carried).ast;
    let union = ast
        .types()
        .find(|(_, node)| node.name == "Other")
        .map(|(id, _)| id)
        .expect("the union");
    assert!(!ast.discriminator_is_key(union));
}

/// `@default_nested` on a module reaches the types in it.
///
/// DDS-XTypes 1.3 §7.3.1.2.1.7: "@annotation default_nested { boolean
/// value default TRUE; };" applied to a module; a type inside it that
/// carries no `@nested` of its own takes the module's answer, and a
/// nested module that says nothing takes its parent's.
///
/// The annotation was in no source of this tree at all before this test
/// — the negative list counted zero of 10.722 files naming it
/// (`docs/spec-coverage/dds-xtypes-1.3.gaps.md` §D.1) — and it reached
/// nothing: it was not in `BUILTIN_ANNOTATIONS`, so it lowered as an
/// unknown annotation and was dropped.
#[test]
fn default_nested_on_a_module_reaches_the_types_in_it() {
    let idl = "\
@default_nested(TRUE) module M {
  @final struct Inner { long a; };
  @final @nested(FALSE) struct Topic { long a; };
  module Deeper {
    @final struct Deep { long a; };
  };
};
module N {
  @final struct Plain { long a; };
};";
    let parsed = parse(idl);
    let lowered = lower(&parsed.syntax(), "annotations.idl");
    assert!(
        !lowered
            .diagnostics
            .iter()
            .any(zerodds_idl_parse::Diagnostic::is_error),
        "{:?}",
        lowered.diagnostics
    );
    let ast = lowered.ast;
    let by_name = |name: &str| {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no type named `{name}`"))
    };

    // The module said TRUE and the type said nothing.
    assert!(ast.effective_nested(by_name("Inner")));
    // A nested module that says nothing takes its parent's answer.
    assert!(ast.effective_nested(by_name("Deep")));
    // A module that was never annotated is FALSE.
    assert!(!ast.effective_nested(by_name("Plain")));
}

#[test]
fn each_extensibility_keyword_reaches_the_facet() {
    for (source, expected) in [
        ("@final struct S { long x; };", Extensibility::Final),
        (
            "@appendable struct S { long x; };",
            Extensibility::Appendable,
        ),
        ("@mutable struct S { long x; };", Extensibility::Mutable),
        (
            "@extensibility(MUTABLE) struct S { long x; };",
            Extensibility::Mutable,
        ),
        (
            "@extensibility(FINAL) struct S { long x; };",
            Extensibility::Final,
        ),
    ] {
        let ast = model(source);
        assert_eq!(
            xtypes_of_type(&ast, "S").extensibility,
            Some(expected),
            "{source}"
        );
    }
}

#[test]
fn a_type_without_an_extensibility_annotation_claims_none() {
    // An absent annotation must not be read as `@final`; the default is
    // the completion pass's business, not the facet's.
    let ast = model("struct S { long x; };");
    let facets = facets_of_type(&ast, "S");
    assert!(
        facets
            .xtypes
            .as_ref()
            .is_none_or(|x| x.extensibility.is_none()),
        "an unannotated struct claimed an extensibility"
    );
}

#[test]
fn an_extensibility_argument_that_is_not_one_of_the_three_is_reported() {
    let found = diagnostics("@extensibility(SOMETHING) struct S { long x; };");
    assert!(
        found.iter().any(|d| d.code.as_str() == "IDL0007"),
        "{found:?}"
    );
}

// -- member annotations --------------------------------------------------

#[test]
fn the_member_flags_each_reach_their_own_field() {
    let ast = model(
        "struct S {
           @key long a;
           @optional long b;
           @external long c;
           @must_understand long d;
           @non_serialized long e;
         };",
    );
    assert!(xtypes_of_member(&ast, "S", 0).is_key);
    assert!(xtypes_of_member(&ast, "S", 1).is_optional);
    assert!(xtypes_of_member(&ast, "S", 2).is_external);
    assert!(xtypes_of_member(&ast, "S", 3).is_must_understand);
    assert!(xtypes_of_member(&ast, "S", 4).is_non_serialized);
}

#[test]
fn several_annotations_on_one_member_all_apply() {
    let ast = model("struct S { @key @optional @id(7) long a; };");
    let x = xtypes_of_member(&ast, "S", 0);
    assert!(x.is_key);
    assert!(x.is_optional);
    assert_eq!(x.effective_member_id(), Some(7));
}

/// The XTypes facet of one union branch's *member* node.
///
/// The member node and not the case node, deliberately: the case node is
/// where the source's annotations are applied, and the member node is
/// what the layout planner, the codec walks and every language adapter
/// read. The lowering mirrors the one onto the other.
fn xtypes_of_branch_member<'a>(ast: &'a Ast, type_name: &str, index: usize) -> &'a XTypesFacet {
    let id = type_of(ast, type_name);
    let case = ast
        .cases_of(id)
        .get(index)
        .copied()
        .unwrap_or_else(|| panic!("{type_name} has no case {index}"));
    let member = ast.case(case).expect("case").member;
    let node = ast.member(member).expect("member");
    ast.facets(AnyId::Member(member, node.node_id))
        .expect("facets")
        .xtypes
        .as_ref()
        .unwrap_or_else(|| panic!("branch {index} of {type_name} carries no xtypes facet"))
}

/// A union branch's annotations reach the member node behind the case.
///
/// The source writes them on the case and `apply_annotations` puts them
/// there; but `zerodds-ast-layout` reads `effective_member_id` and
/// `must_understand` off the member, the codec walks read `is_external`
/// and `try_construct` off the member, and the language adapters read
/// the member's facets to shape the branch's field. Before the mirror,
/// `case 1: @external Inner i;` lowered without a diagnostic and no
/// consumer ever saw the annotation — a silent drop.
#[test]
fn a_union_branchs_annotations_reach_the_member_behind_the_case() {
    let ast = model(
        "struct Inner { long a; };
         union U switch (long) {
           case 1: @external Inner i;
           case 2: @id(9) @external long b;
           default: @try_construct(DISCARD) long c;
         };",
    );
    assert!(xtypes_of_branch_member(&ast, "U", 0).is_external);
    let second = xtypes_of_branch_member(&ast, "U", 1);
    assert_eq!(second.effective_member_id(), Some(9));
    assert!(second.is_external);
    assert_eq!(
        xtypes_of_branch_member(&ast, "U", 2).try_construct,
        Some(TryConstruct::Discard)
    );
}

#[test]
fn an_annotation_on_a_multi_declarator_member_applies_to_each_name() {
    // `@key long a, b;` keys both, which is what the declaration says.
    let ast = model("struct S { @key long a, b; };");
    assert!(xtypes_of_member(&ast, "S", 0).is_key);
    assert!(xtypes_of_member(&ast, "S", 1).is_key);
}

#[test]
fn a_declared_id_records_that_it_was_declared() {
    // The number alone is not enough: a collision diagnostic has to say
    // which member declared its id and which one derived it. Spec §5.
    let ast = model("struct S { @id(3) long a; };");
    let x = xtypes_of_member(&ast, "S", 0);
    assert_eq!(x.effective_member_id(), Some(3));
    assert_eq!(
        x.member_id.as_ref().map(|a| a.origin.clone()),
        Some(MemberIdOrigin::Declared)
    );
}

#[test]
fn a_hashid_records_the_name_it_hashes() {
    let ast = model("struct S { @hashid long a; @hashid(\"other\") long b; };");
    assert_eq!(
        xtypes_of_member(&ast, "S", 0)
            .member_id
            .as_ref()
            .map(|a| a.origin.clone()),
        Some(MemberIdOrigin::Hashed {
            source_name: "a".to_string()
        })
    );
    assert_eq!(
        xtypes_of_member(&ast, "S", 1)
            .member_id
            .as_ref()
            .map(|a| a.origin.clone()),
        Some(MemberIdOrigin::Hashed {
            source_name: "other".to_string()
        })
    );
}

/// "If the annotation is used without any parameter or with the empty
/// string as a parameter, then the Member ID shall be computed from the
/// member name." DDS-XTypes 1.3 §7.3.1.2.1.1.
#[test]
fn an_empty_hashid_argument_is_the_member_name() {
    let ast = model("struct S { @hashid(\"\") long a; };");
    assert_eq!(
        xtypes_of_member(&ast, "S", 0)
            .member_id
            .as_ref()
            .map(|a| a.origin.clone()),
        Some(MemberIdOrigin::Hashed {
            source_name: "a".to_string()
        })
    );
    assert_eq!(
        xtypes_of_member(&ast, "S", 0).effective_member_id(),
        Some(zerodds_idl_lower::hashed_member_id("a"))
    );
}

/// "At most one of these two annotations may appear on a member.
/// Otherwise it shall be considered an error." DDS-XTypes 1.3
/// §7.3.1.2.1.1.
#[test]
fn an_id_and_a_hashid_on_one_member_are_rejected() {
    for source in [
        "struct S { @id(7) @hashid(\"alpha\") long a; };",
        "struct S { @hashid(\"alpha\") @id(7) long a; };",
    ] {
        let found = diagnostics(source);
        assert!(
            found.iter().any(|d| d.code.as_str() == "IDL0007"),
            "{source}: {found:?}"
        );
    }
}

#[test]
fn an_id_that_is_not_an_integer_is_reported_rather_than_ignored() {
    // Ignoring it would leave the member with the id it would have had
    // anyway, which is the one thing the source did not ask for.
    let found = diagnostics("struct S { @id(\"x\") long a; };");
    assert!(
        found.iter().any(|d| d.code.as_str() == "IDL0007"),
        "{found:?}"
    );
}

#[test]
fn both_autoid_policies_are_distinguished() {
    for (source, expected) in [
        ("@autoid(HASH) struct S { long a; };", AutoId::Hash),
        (
            "@autoid(SEQUENTIAL) struct S { long a; };",
            AutoId::Sequential,
        ),
        // Written bare, `@autoid` means HASH. XTypes 1.3 §7.3.1.2.1.1.
        ("@autoid struct S { long a; };", AutoId::Hash),
    ] {
        let ast = model(source);
        assert_eq!(xtypes_of_type(&ast, "S").autoid, Some(expected), "{source}");
    }
}

#[test]
fn each_try_construct_behaviour_is_distinguished() {
    for (written, expected) in [
        ("DISCARD", TryConstruct::Discard),
        ("USE_DEFAULT", TryConstruct::UseDefault),
        ("TRIM", TryConstruct::Trim),
    ] {
        // A `string<5>`, because `TRIM` on anything that cannot be
        // trimmed is a declaration the validation pass refuses —
        // Table 11 — and a fixture that carried one would be testing
        // the parse of an illegal type.
        let source = format!("struct S {{ @try_construct({written}) string<5> a; }};");
        let ast = model(&source);
        assert_eq!(
            xtypes_of_member(&ast, "S", 0).try_construct,
            Some(expected),
            "{written}"
        );
    }
}

/// DDS-XTypes 1.3 §7.3.1.2.1.12 declares the annotation as
/// `TryConstructFailAction value default USE_DEFAULT` and spells it out
/// twice more: "If the annotation is specified without a value ... the
/// behavior is set to DEFAULT", and Example 1 writes
/// `@try_construct string<5> member;` as the equivalent of the
/// USE_DEFAULT form.
///
/// This is not the same as no annotation at all, which §7.2.2.7 makes
/// DISCARD. The two defaults belong to different things — the
/// annotation's parameter and the absence of the annotation — and
/// reading the bare form as DISCARD would silently drop samples the
/// author asked to keep. OpenDDS constructs the empty string for it;
/// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md`.
#[test]
fn a_bare_try_construct_is_use_default_and_an_absent_one_is_discard() {
    let ast = model("struct S { @try_construct string<5> a; string<5> b; };");
    assert_eq!(
        xtypes_of_member(&ast, "S", 0).try_construct,
        Some(TryConstruct::UseDefault)
    );
    assert_eq!(xtypes_of_member(&ast, "S", 1).try_construct, None);
}

/// §7.3.1.2.1.12: "the TryConstruct annotation may be used in structure
/// and union members, the union discriminator, the elements of arrays
/// and sequences, and the key and/or values of map types."
///
/// Each position is a different field, because each means a different
/// thing: §7.3.1.2.1.12.3 trims the member's own collection and
/// §7.3.1.2.1.12.4 trims one element of it.
#[test]
fn try_construct_reaches_every_position_the_clause_names() {
    let ast = model(
        "typedef string<5> String5;
         @mutable struct S { sequence<@try_construct(TRIM) String5, 10> items; };",
    );
    let sequence = ast
        .types()
        .find(|(_, node)| matches!(node.kind, zerodds_ast::TypeKind::Sequence { .. }))
        .map(|(id, _)| id)
        .expect("the sequence type");
    assert_eq!(
        ast.element_try_construct(sequence, None),
        TryConstruct::Trim
    );

    let ast = model(
        "@mutable struct S { \
         map<@try_construct(DISCARD) string<5>, @try_construct(TRIM) string<5>, 10> m; };",
    );
    let map = ast
        .types()
        .find(|(_, node)| matches!(node.kind, zerodds_ast::TypeKind::Map { .. }))
        .map(|(id, _)| id)
        .expect("the map type");
    assert_eq!(ast.key_try_construct(map), TryConstruct::Discard);
    assert_eq!(ast.element_try_construct(map, None), TryConstruct::Trim);

    let ast = model(
        "@appendable enum K { K1, K2 };
         @mutable union U switch (@try_construct K) { case K1: long v; };",
    );
    let union = ast
        .types()
        .find(|(_, node)| node.name == "U")
        .map(|(id, _)| id)
        .expect("the union");
    assert_eq!(
        ast.discriminator_try_construct(union),
        TryConstruct::UseDefault
    );
}

/// An alias states the behaviour its uses take by default, and a use
/// site that writes its own overrides it.
///
/// The clause does not list aliases among the positions, and Cyclone and
/// RTI both refuse `@try_construct` on a `typedef` by name. Neither is a
/// statement about what an alias-borne annotation would *mean*, and with
/// the specification silent the broader reading is the one taken here:
/// nothing an author wrote can be lost by it, because the use site wins.
/// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md` records
/// the two refusals.
#[test]
fn an_alias_supplies_a_default_that_the_use_site_overrides() {
    let ast = model(
        "@try_construct(TRIM) typedef string<5> String5;
         @mutable struct S { String5 inherited; @try_construct(USE_DEFAULT) String5 written; };",
    );
    let member = |index: usize| {
        let members = ast.all_members_of(
            ast.types()
                .find(|(_, node)| node.name == "S")
                .map(|(id, _)| id)
                .expect("S"),
        );
        ast.member(members[index]).expect("a member").clone()
    };
    for (index, expected) in [(0, TryConstruct::Trim), (1, TryConstruct::UseDefault)] {
        let node = member(index);
        assert_eq!(
            ast.effective_try_construct(
                node.facets.xtypes.as_ref().and_then(|x| x.try_construct),
                node.type_.resolved(),
            ),
            expected,
            "member {index}"
        );
    }
}

/// Table 17 gives a map's key no escape — "If a key element of O2 cannot
/// construct the corresponding key type of T1 the entire map O2 cannot
/// construct any object of type T1" — which contradicts §7.2.2.7's
/// sentence that a map "may explicitly declare that their key ... has
/// one of three kinds". `opendds_idl` resolves it the same way and
/// aborts: "map keys may only use DISCARD as their @try_construct mode".
#[test]
fn a_map_key_may_only_be_discard() {
    let refused =
        diagnostics("@mutable struct S { map<@try_construct(TRIM) string<5>, string<5>, 10> m; };");
    assert!(
        refused
            .iter()
            .any(|d| d.message.contains("a map key can only be DISCARD")),
        "{refused:?}"
    );
    let accepted = diagnostics(
        "@mutable struct S { map<@try_construct(DISCARD) string<5>, string<5>, 10> m; };",
    );
    assert!(accepted.is_empty(), "{accepted:?}");
}

/// Table 11: TRIM "only applies to elements or members of type string,
/// wide string, sequence, or map. The behavior when applied to other
/// element/member types is unspecified and may be treated as an error."
///
/// Treated as an error here, which is Cyclone's reading — its front end
/// says "`@try_construct(TRIM)` can not be applied to types which cannot
/// have bounds". RTI accepts the annotation and drops it, and OpenDDS
/// accepts it and generates its DISCARD branch: both give the author the
/// opposite of what was written without saying so.
#[test]
fn trim_on_a_type_that_cannot_be_trimmed_is_refused() {
    for source in [
        "@mutable struct S { @try_construct(TRIM) long m; };",
        "@mutable struct S { @try_construct(TRIM) long m[3]; };",
        "@mutable struct S { sequence<@try_construct(TRIM) long, 10> m; };",
        "@try_construct(TRIM) typedef long L; @mutable struct S { L m; };",
        "@appendable enum K { K1, K2 };
         @mutable union U switch (@try_construct(TRIM) K) { case K1: long v; };",
    ] {
        let refused = diagnostics(source);
        assert!(
            refused
                .iter()
                .any(|d| d.message.contains("TRIM") || d.message.contains("cannot be TRIM")),
            "{source}: {refused:?}"
        );
    }
    // The four positions where it *is* defined stay accepted.
    for source in [
        "@mutable struct S { @try_construct(TRIM) string<5> m; };",
        "@mutable struct S { @try_construct(TRIM) wstring<5> m; };",
        "@mutable struct S { @try_construct(TRIM) sequence<long, 4> m; };",
        "@mutable struct S { @try_construct(TRIM) map<long, long, 4> m; };",
        "typedef string<5> String5;
         @mutable struct S { sequence<@try_construct(TRIM) String5, 10> m; };",
    ] {
        let accepted = diagnostics(source);
        assert!(accepted.is_empty(), "{source}: {accepted:?}");
    }
}

/// An annotation with no place to go is reported, never dropped: the
/// model has no slot for `@key` on a collection element, and an
/// annotation that vanished between the source and the model would make
/// a regenerated declaration differ from the one that was written.
#[test]
fn only_try_construct_is_accepted_on_a_collection_element() {
    let refused = diagnostics("@mutable struct S { sequence<@key long, 10> m; };");
    assert!(
        refused
            .iter()
            .any(|d| d.message.contains("has no meaning on a collection element")),
        "{refused:?}"
    );
}

/// `@external` on the elements of a collection is the collection's
/// `element_external` property (DDS-XTypes 1.3 §7.3.1.2.1.4):
/// `sequence<@external Foo, 42>` and `map<string, @external Foo, 42>`.
/// It was refused because the model had no place for it; it has one now,
/// on the collection's own node beside the try-construct behaviour.
#[test]
fn external_on_an_element_a_map_key_or_a_map_value_is_carried() {
    let ast = model(
        "struct Inner { long a; };
         @mutable struct S {
             sequence<@external Inner, 4> seq;
             map<@external long, long, 4> keyed;
             map<long, @external Inner, 4> valued;
             sequence<Inner, 4> plain;
         };",
    );
    let collection = |index: usize| {
        let member = ast.members_of(type_of(&ast, "S"))[index];
        ast.member(member)
            .expect("member")
            .type_
            .resolved()
            .expect("a resolved collection")
    };
    assert!(ast.element_is_external(collection(0)));
    assert!(ast.key_is_external(collection(1)));
    assert!(!ast.element_is_external(collection(1)));
    assert!(ast.element_is_external(collection(2)));
    assert!(!ast.key_is_external(collection(2)));
    assert!(!ast.element_is_external(collection(3)));
    for source in [
        "struct Inner { long a; }; @mutable struct S { sequence<@external Inner, 4> m; };",
        "struct Inner { long a; }; @mutable struct S { map<@external long, long, 4> m; };",
        "struct Inner { long a; }; @mutable struct S { map<long, @external Inner, 4> m; };",
    ] {
        assert!(
            diagnostics(source).is_empty(),
            "{source}: {:?}",
            diagnostics(source)
        );
    }
    // Table 21 keeps the discriminator out of `@external`'s places.
    assert!(
        diagnostics("union U switch (@external long) { case 1: long a; };")
            .iter()
            .any(|d| d.message.contains("`@external` has no meaning on"))
    );
}

/// DDS-XTypes 1.3 §7.3.1.2.1.4: "To declare the elements of a collection
/// type external, apply the annotation to the collection declaration
/// like this: [...] Arrays: `Foo array_of_foo @external [42];`". The
/// annotation stands between the declarator's name and its dimensions
/// and is the array's elements', not the member's.
#[test]
fn external_after_an_array_declarators_name_is_the_elements() {
    let ast = model(
        "struct Foo { long a; };
         @mutable struct S {
             Foo array_of_foo @external [42];
             Foo plain[42];
             Foo inner @external [2][3];
         };
         typedef Foo Foos @external [4];",
    );
    let array = |index: usize| {
        let member = ast.members_of(type_of(&ast, "S"))[index];
        ast.member(member)
            .expect("member")
            .type_
            .resolved()
            .expect("a resolved array")
    };
    assert!(ast.element_is_external(array(0)));
    assert!(!ast.element_is_external(array(1)));
    assert!(
        ast.element_is_external(array(2)),
        "both dimensions, one array"
    );
    assert!(
        !xtypes_of_member(&ast, "S", 0).is_external,
        "the member is not external: its elements are"
    );
    let TypeKind::Alias { aliased } = &ast.type_(type_of(&ast, "Foos")).expect("alias").kind else {
        panic!("Foos is not an alias");
    };
    assert!(ast.element_is_external(aliased.resolved().expect("the array")));
    assert!(
        diagnostics("struct Foo { long a; }; struct S { Foo array_of_foo @external [42]; };")
            .is_empty()
    );
}

/// An annotation between a name and nothing is the elements' of an array
/// that is not there: it is reported, not dropped.
#[test]
fn an_annotation_after_the_name_of_a_declarator_with_no_dimensions_is_reported() {
    let found = diagnostics("struct S { long a @external; };");
    assert!(
        found
            .iter()
            .any(|d| d.message.contains("this declarator is not an array")),
        "{found:?}"
    );
}

/// DDS-XTypes 1.3 §7.2.2.6 attaches an annotation to "an element/member/
/// literal of an aggregated/collection/enumerated type": a declared
/// annotation on the element of a sequence, the key and the value of a
/// map and an array's element is kept where `@external` is, on the
/// collection's node. It was reported as having "no meaning" there.
#[test]
fn a_declared_annotation_on_an_element_a_key_or_an_array_is_kept() {
    let ast = model(
        "@annotation Mark { long n default 0; };
         @mutable struct S {
             sequence<@Mark(1) long, 4> seq;
             map<@Mark(2) long, @Mark(3) long, 4> pairs;
             long arr @Mark(4) [2];
         };",
    );
    let collection = |index: usize| {
        let member = ast.members_of(type_of(&ast, "S"))[index];
        ast.member(member)
            .expect("member")
            .type_
            .resolved()
            .expect("a resolved collection")
    };
    assert_eq!(ast.element_annotations(collection(0)).len(), 1);
    assert_eq!(ast.element_annotations(collection(0))[0].name, "Mark");
    assert_eq!(ast.key_annotations(collection(1)).len(), 1);
    assert_eq!(ast.element_annotations(collection(1)).len(), 1);
    assert_eq!(ast.element_annotations(collection(2)).len(), 1);
    assert!(ast.key_annotations(collection(0)).is_empty());
    assert!(
        diagnostics(
            "@annotation Mark { long n default 0; };
             struct S { sequence<@Mark(1) long, 4> seq; };"
        )
        .is_empty()
    );
}

/// DDS-XTypes 1.3 §7.3.1.2.5: "Annotations can be applied to the implicit
/// discriminator member of a union type by applying them to the
/// discriminator type declaration in the header of the union type's
/// definition: `union MyUnion switch (@MyAnnotation long) {`".
#[test]
fn a_declared_annotation_on_a_discriminator_is_kept() {
    let ast = model(
        "@annotation MyAnnotation { long n default 0; };
         union MyUnion switch (@MyAnnotation(7) long) {
             case 0: string member_0;
             default: long default_member;
         };",
    );
    let union_ = type_of(&ast, "MyUnion");
    assert_eq!(ast.discriminator_annotations(union_).len(), 1);
    assert_eq!(
        ast.discriminator_annotations(union_)[0].name,
        "MyAnnotation"
    );
    assert!(
        facets_of_type(&ast, "MyUnion").annotations.is_empty(),
        "the union's own list is not the discriminator's"
    );
}

/// A built-in annotation that has no meaning on a use is still reported:
/// only a declared annotation is kept there.
#[test]
fn a_builtin_annotation_with_no_meaning_on_a_use_is_still_reported() {
    let found = diagnostics("struct S { sequence<@key long, 4> seq; };");
    assert!(
        found
            .iter()
            .any(|d| d.message.contains("`@key` has no meaning on")),
        "{found:?}"
    );
}

#[test]
fn a_unit_and_a_range_reach_their_fields() {
    let ast = model("struct S { @unit(\"m/s\") @range(min = 0, max = 100) long a; };");
    let x = xtypes_of_member(&ast, "S", 0);
    assert_eq!(x.unit.as_deref(), Some("m/s"));
    let range = x.range.expect("a range");
    assert!(range.min.is_some() && range.max.is_some());
}

#[test]
fn min_and_max_written_separately_combine_into_one_range() {
    let ast = model("struct S { @min(1) @max(9) long a; };");
    let range = xtypes_of_member(&ast, "S", 0).range.expect("a range");
    assert!(range.min.is_some(), "the min was lost");
    assert!(range.max.is_some(), "the max was lost");
}

// -- enumerators and bits ------------------------------------------------

#[test]
fn an_explicit_enumerator_value_keeps_the_expression_it_was_written_as() {
    // `@value(1 << 3)` is 8 and is not written `8`; a formatter has to
    // reproduce what was there.
    let ast = model("enum E { @value(1 << 3) A, B };");
    let e = type_of(&ast, "E");
    let TypeKind::Enumeration { enumerators } = &ast.type_(e).expect("E").kind else {
        panic!("expected an enumeration");
    };
    let first = ast.enumerator(enumerators[0]).expect("A");
    let expr = first.value.expect("an explicit value");
    assert_eq!(ast.eval_integral(expr).expect("evaluates"), 8);
    assert!(
        ast.enumerator(enumerators[1]).expect("B").value.is_none(),
        "a positional enumerator gained an expression it never had"
    );
    // The rule still answers for it: 8 was written, so B is 9.
    assert_eq!(ast.effective_enumerator_value(enumerators[1]), Some(9));
}

#[test]
fn an_explicit_bit_position_reaches_the_bit() {
    let ast = model("bitmask B { @position(5) A, C };");
    let b = type_of(&ast, "B");
    let TypeKind::Bitmask { bits } = &ast.type_(b).expect("B").kind else {
        panic!("expected a bitmask");
    };
    assert_eq!(ast.bit_value(bits[0]).expect("A").position, Some(5));
    assert_eq!(ast.bit_value(bits[1]).expect("C").position, None);
    assert_eq!(ast.effective_bit_position(bits[1]), Some(6));
}

#[test]
fn a_bit_bound_reaches_the_type() {
    let ast = model("@bit_bound(16) enum E { A };");
    assert_eq!(xtypes_of_type(&ast, "E").bit_bound, Some(16));
}

#[test]
fn a_default_literal_marks_the_enumerator_and_not_the_enum() {
    let ast = model("enum E { A, @default_literal B };");
    let e = type_of(&ast, "E");
    let TypeKind::Enumeration { enumerators } = &ast.type_(e).expect("E").kind else {
        panic!("expected an enumeration");
    };
    let marked = |index: usize| {
        let id = enumerators[index];
        let node = ast.enumerator(id).expect("enumerator");
        ast.facets(AnyId::Enumerator(id, node.node_id))
            .expect("facets")
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_default_literal)
    };
    assert!(!marked(0));
    assert!(marked(1));
}

// -- type-level annotations ----------------------------------------------

#[test]
fn a_topic_annotation_keeps_both_of_its_parameters() {
    let ast = model("@topic(name = \"Square\", platform = \"*\") struct S { long a; };");
    let topic = xtypes_of_type(&ast, "S").topic.clone().expect("a topic");
    assert_eq!(topic.name.as_deref(), Some("Square"));
    assert_eq!(topic.platform.as_deref(), Some("*"));
}

#[test]
fn a_data_representation_annotation_is_a_mask_not_one_value() {
    let ast = model("@data_representation(XCDR1 | XCDR2) struct S { long a; };");
    let mask = xtypes_of_type(&ast, "S")
        .data_representation
        .expect("a mask");
    assert!(mask.xcdr1 && mask.xcdr2);
    assert!(!mask.xml);
}

#[test]
fn an_unknown_data_representation_is_reported() {
    let found = diagnostics("@data_representation(XCDR9) struct S { long a; };");
    assert!(
        found.iter().any(|d| d.code.as_str() == "IDL0007"),
        "{found:?}"
    );
}

#[test]
fn a_verbatim_directive_keeps_its_language_placement_and_text() {
    let ast = model(
        "@verbatim(language = \"c++\", placement = \"END_FILE\", text = \"// tail\")
         struct S { long a; };",
    );
    let directives = &xtypes_of_type(&ast, "S").verbatim;
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].language, "c++");
    assert_eq!(directives[0].placement, VerbatimPlacement::EndFile);
    assert_eq!(directives[0].text, "// tail");
}

#[test]
fn a_verbatim_directive_defaults_language_and_placement() {
    // OMG IDL 4.2 §8.3.5.1: `string language default "*"` and
    // `PlacementKind placement default BEFORE_DECLARATION`.
    let ast = model("@verbatim(text = \"// body\") struct S { long a; };");
    let directives = &xtypes_of_type(&ast, "S").verbatim;
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].language, "*");
    assert_eq!(
        directives[0].placement,
        VerbatimPlacement::BeforeDeclaration
    );
    assert_eq!(directives[0].text, "// body");
}

#[test]
fn repeated_verbatim_directives_accumulate_in_declaration_order() {
    let ast = model(
        "@verbatim(language = \"c\", placement = BEGIN_FILE, text = \"// one\")
         @verbatim(language = \"java\", text = \"// two\")
         @verbatim(text = \"// three\")
         struct S { long a; };",
    );
    let directives = &xtypes_of_type(&ast, "S").verbatim;
    assert_eq!(directives.len(), 3);
    assert_eq!(directives[0].text, "// one");
    assert_eq!(directives[0].placement, VerbatimPlacement::BeginFile);
    assert_eq!(directives[1].language, "java");
    assert_eq!(directives[2].language, "*");
}

#[test]
fn a_verbatim_directive_for_an_unknown_language_is_kept_as_written() {
    // §8.3.5.1 does not close the language set: a value no backend
    // answers to is stored unchanged and simply reaches no output.
    let ast = model("@verbatim(language = \"cobol\", text = \"* hi\") struct S { long a; };");
    let directives = &xtypes_of_type(&ast, "S").verbatim;
    assert_eq!(directives[0].language, "cobol");
}

#[test]
fn an_unknown_verbatim_placement_is_reported() {
    let found =
        diagnostics("@verbatim(placement = MIDDLE_FILE, text = \"x\") struct S { long a; };");
    assert!(
        found.iter().any(|d| d.message.contains("@verbatim")),
        "{found:?}"
    );
}

#[test]
fn an_unknown_verbatim_parameter_is_reported() {
    let found = diagnostics("@verbatim(position = \"top\", text = \"x\") struct S { long a; };");
    assert!(
        found
            .iter()
            .any(|d| d.message.contains("no parameter named `position`")),
        "{found:?}"
    );
}

#[test]
fn a_verbatim_directive_on_a_member_reaches_the_member() {
    let ast = model("struct S { @verbatim(language = \"c++\", text = \"// m\") long a; };");
    let directives = &xtypes_of_member(&ast, "S", 0).verbatim;
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].text, "// m");
}

#[test]
fn a_nested_annotation_reaches_the_type() {
    let ast = model("@nested struct S { long a; };");
    assert!(xtypes_of_type(&ast, "S").is_nested);
}

// -- CORBA annotations ---------------------------------------------------

#[test]
fn service_and_ami_reach_the_corba_facet() {
    let ast = model("@service @ami interface I { void go(); };");
    let any = ast.find_by_scoped_name(&scoped(&["I"])).expect("I");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert!(corba.is_service);
    assert!(corba.is_ami);
}

// -- user-declared annotations -------------------------------------------

#[test]
fn a_user_declared_annotation_is_kept_whole() {
    // Nothing else would hold it: it has no typed field, and dropping it
    // would make a regenerated declaration differ from the source.
    let ast = model(
        "@annotation Sensitive { string reason; };
         @Sensitive(reason = \"personal\") struct S { long a; };",
    );
    let facets = facets_of_type(&ast, "S");
    assert_eq!(facets.annotations.len(), 1);
    let applied = &facets.annotations[0];
    assert_eq!(applied.name, "Sensitive");
    assert!(
        applied.definition.is_resolved(),
        "the declaration was in scope and should have resolved"
    );
    assert_eq!(applied.arguments.len(), 1);
    assert_eq!(applied.arguments[0].name.as_deref(), Some("reason"));
}

#[test]
fn a_user_annotation_used_before_its_declaration_waits_rather_than_failing() {
    let ast = model(
        "@Later struct S { long a; };
         @annotation Later { boolean value; };",
    );
    let applied = &facets_of_type(&ast, "S").annotations[0];
    assert!(applied.definition.is_pending(), "{:?}", applied.definition);
}

#[test]
fn a_specified_annotation_does_not_also_appear_in_the_applied_list() {
    // Two answers to one question is worse than one: `@key` is
    // is_key and nothing else.
    let ast = model("struct S { @key long a; };");
    let id = type_of(&ast, "S");
    let member = ast.members_of(id)[0];
    let node = ast.member(member).expect("member");
    let facets = ast
        .facets(AnyId::Member(member, node.node_id))
        .expect("facets");
    assert!(facets.annotations.is_empty(), "{:?}", facets.annotations);
}

// -- provenance ----------------------------------------------------------

#[test]
fn an_annotated_declaration_still_records_where_it_came_from() {
    let ast = model("@mutable struct S { long a; };");
    let facets = facets_of_type(&ast, "S");
    assert!(facets.source.span.is_some());
    assert!(matches!(
        facets.source.origin,
        zerodds_ast::SourceOrigin::File { .. }
    ));
}

// -- §8.2 rule (218): an annotation's name is a scoped name --------------

#[test]
fn a_root_anchored_annotation_name_reaches_the_builtin_it_names() {
    // (218) `<annotation_appl> ::= "@" <scoped_name> [ "("
    // <annotation_appl_params> ")" ]`. A `<scoped_name>` is rule (4)'s:
    // an identifier, a root-anchored `::` identifier, or a chain. This
    // took one token, so `@::key` diagnosed on the `::` with "expected
    // an annotation name after `@`".
    let ast = model("struct S { @::key long a; };");
    assert!(xtypes_of_member(&ast, "S", 0).is_key);
}

#[test]
fn a_qualified_annotation_name_reaches_its_declaration() {
    // `@ann::mark(7)` took `ann` for the whole name and resumed parsing
    // at the `::` inside what it then believed was a member
    // declaration — "expected a declarator name", and eleven cascading
    // diagnostics after it, none of which named an annotation.
    let source = "module ann { @annotation mark { long value; }; };\n\
                  struct S { @ann::mark(7) short b; };";
    let parsed = parse(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    assert_eq!(parsed.text(), source, "the round trip changed");
}

#[test]
fn every_spelling_of_one_builtin_says_the_same_thing() {
    // The last segment is the name that resolves: every builtin is
    // matched by its simple name, so all three of these are `@key`.
    for source in ["struct S { @key long a; };", "struct S { @::key long a; };"] {
        let ast = model(source);
        assert!(xtypes_of_member(&ast, "S", 0).is_key, "{source:?}");
    }
}

#[test]
fn an_annotation_name_that_is_a_keyword_still_works_qualified() {
    // GitHub issue #6's half: `@default` collides with a keyword by
    // design, and §8.2 gives no exception from rule (4) for being
    // written after an `@`, so every segment admits a keyword too.
    for source in [
        "struct S { @default(3) long a; };",
        "struct S { @::default(3) long a; };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
    }
}

#[test]
fn an_annotation_with_no_name_at_all_is_still_refused() {
    // The rule the fix must not lose. A `@` with nothing after it that
    // can be a name is one diagnostic naming §8.2, not silence.
    //
    // "Nothing that can be a name" is narrow, and deliberately: every
    // segment admits a keyword, because `@default` and `@public` are
    // annotations whose names collide with keywords by design and §8.2
    // gives the name no exception from rule (4) for sitting after an
    // `@`. So `@:: long a;` names an annotation `long` rather than
    // failing here, which is what this front end already did for
    // `@long` and is not what this change is about.
    for source in ["struct S { @", "struct S { @:: ("] {
        let parsed = parse(source);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|d| d.spec_reference == "OMG IDL 4.2 §8.2"),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
    }
}

// -- the suffix form, DDS-XTypes 1.3 §7.3.1.2.5 Table 24 ------------------

/// The `ShapeType` the OMG `dds-security` interoperability suite ships.
///
/// Written exactly as that suite writes it, `//@key` and all. Before the
/// suffix form was read, this file produced no diagnostic at all and
/// left `color` with `is_key() == false`: the file said "this is the
/// key", the model said "no key", and nothing said anything.
#[test]
fn the_suffix_form_marks_the_key_the_interop_suite_declares() {
    let idl = "\
const long COLOR_MAX_LENGTH=64;
struct ShapeType {
    string<COLOR_MAX_LENGTH> color; //@key
    long x;
    long y;
    long shapesize;
};";
    let parsed = parse(idl);
    assert!(
        !parsed.has_errors(),
        "the suite's own file must parse: {:?}",
        parsed.diagnostics()
    );
    let ast = model(idl);
    let id = type_of(&ast, "ShapeType");
    let member = ast.members_of(id).first().copied().expect("color");
    let node = ast.member(member).expect("member");
    let facets = ast
        .facets(AnyId::Member(member, node.node_id))
        .expect("facets");
    assert!(
        facets.is_key(),
        "`//@key` must mark the member the file marks"
    );
}

/// "Alternately and **equivalently**" — Table 24's own word.
///
/// The claim is testable, so it is tested: the same declaration written
/// once each way has to reach the same facets. Anything that made the
/// suffix form a second, nearly-equal path would show up here.
#[test]
fn the_prefix_and_the_suffix_form_produce_the_same_model() {
    for (prefix, suffix) in [
        ("@key long x;", "long x; //@key"),
        ("@optional long x;", "long x; //@optional"),
        ("@external long x;", "long x; //@external"),
        ("@must_understand long x;", "long x; //@must_understand"),
        ("@non_serialized long x;", "long x; //@non_serialized"),
        ("@id(7) long x;", "long x; //@id(7)"),
        ("@hashid(\"old\") long x;", "long x; //@hashid(\"old\")"),
        ("@unit(\"m/s\") long x;", "long x; //@unit(\"m/s\")"),
        ("@min(1) @max(9) long x;", "long x; //@min(1)\n//@max(9)"),
        (
            "@try_construct(TRIM) long x;",
            "long x; //@try_construct(TRIM)",
        ),
    ] {
        let with_prefix = format!("struct S {{ {prefix} }};");
        // The comment runs to the end of its line, so the closing brace
        // of the suffix spelling has to start a new one.
        let with_suffix = format!("struct S {{ {suffix}\n}};");
        let left = model(&with_prefix);
        let right = model(&with_suffix);
        assert_eq!(
            format!("{:?}", xtypes_of_member(&left, "S", 0)),
            format!("{:?}", xtypes_of_member(&right, "S", 0)),
            "{prefix} and {suffix} must reach the same facet"
        );
    }
}

/// `//@id(7)` assigns the id `@id(7)` assigns.
#[test]
fn a_suffix_annotation_carries_its_arguments() {
    let ast = model("struct S { long x; //@id(7)\nlong y; };");
    assert_eq!(
        xtypes_of_member(&ast, "S", 0)
            .member_id
            .as_ref()
            .map(|assignment| assignment.effective),
        Some(7),
        "`//@id(7)` must assign the id the file assigns"
    );
}

/// `}; //@my_type_annotation` — §7.3.1.2.3's own example, on the type.
#[test]
fn the_suffix_form_applies_to_the_type_whose_brace_precedes_it() {
    let ast = model("struct Gadget { long my_integer; }; //@nested");
    assert!(
        xtypes_of_type(&ast, "Gadget").is_nested,
        "`}}; //@nested` must annotate the type it follows"
    );
    let ast = model("struct Gadget { long my_integer; }; //@my_type_annotation");
    assert_eq!(
        facets_of_type(&ast, "Gadget")
            .annotations
            .iter()
            .map(|applied| applied.name.as_str())
            .collect::<Vec<_>>(),
        ["my_type_annotation"],
        "an annotation this specification does not define reaches the facet unchanged"
    );
    // The two forms on one declaration are one set, not two that
    // overwrite each other: the prefix one sits in the wrapper node and
    // the suffix one inside the declaration, and both have to arrive.
    let ast = model("@final struct Gadget { long my_integer; }; //@nested");
    let xtypes = xtypes_of_type(&ast, "Gadget");
    assert_eq!(xtypes.extensibility, Some(Extensibility::Final));
    assert!(xtypes.is_nested);
}

/// §7.3.1.2.3's second example: two annotations on one member, the
/// second on the line below, and one on the type.
#[test]
fn several_suffix_annotations_apply_to_the_same_element() {
    let idl = "\
struct Gadget {
    long my_integer; //@key
//@id(4)
}; //@appendable";
    let ast = model(idl);
    let member = xtypes_of_member(&ast, "Gadget", 0);
    assert!(member.is_key, "the first of the run must apply");
    assert_eq!(
        member.member_id.as_ref().map(|a| a.effective),
        Some(4),
        "the second of the run must apply to the same member"
    );
    assert_eq!(
        xtypes_of_type(&ast, "Gadget").extensibility,
        Some(Extensibility::Appendable)
    );
}

/// Two on one line, which §7.3.1.2.3 admits: the only rule it gives a
/// second annotation is that its at-sign be preceded by a double slash
/// with no white space.
#[test]
fn two_suffix_annotations_on_one_line_both_apply() {
    let ast = model("struct S { long x; //@key //@must_understand\n};");
    let member = xtypes_of_member(&ast, "S", 0);
    assert!(member.is_key && member.is_must_understand);
}

/// A `//@` where no annotation may be applied is a diagnostic with a
/// range, not a comment.
///
/// The alternative is the worst answer this frontend can give: a file
/// that says something about a declaration, and a model that does not
/// say it, with nothing in between.
#[test]
fn a_suffix_annotation_that_suffixes_nothing_is_refused() {
    for source in [
        "//@key\nstruct S { long x; };",
        "struct S { //@key\nlong x; };",
        "struct S { long x[ //@key\n2]; };",
    ] {
        let parsed = parse(source);
        let found = parsed
            .diagnostics()
            .iter()
            .find(|d| d.spec_reference == "DDS-XTypes 1.3 §7.3.1.2.5")
            .unwrap_or_else(|| panic!("{source:?} was accepted in silence"));
        assert!(found.is_error(), "{}", found.message);
        assert!(
            found.primary.end > found.primary.start,
            "the diagnostic must name a range: {found:?}"
        );
    }
}

/// An unknown name in the suffix form gets the prefix form's answer.
///
/// `@ann` on a name nothing declares reaches `Facets::annotations` as a
/// pending reference; `//@ann` must do the same and not a second thing.
#[test]
fn an_unknown_suffix_annotation_gets_the_prefix_forms_answer() {
    let with_prefix = model("struct S { @ann(3) long x; };");
    let with_suffix = model("struct S { long x; //@ann(3)\n};");
    let names = |ast: &Ast| {
        let id = type_of(ast, "S");
        let member = ast.members_of(id).first().copied().expect("x");
        let node = ast.member(member).expect("member");
        ast.facets(AnyId::Member(member, node.node_id))
            .expect("facets")
            .annotations
            .iter()
            .map(|applied| format!("{}/{}", applied.name, applied.arguments.len()))
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&with_prefix), names(&with_suffix));
    assert_eq!(names(&with_suffix), ["ann/1"]);
}

/// Prose after the annotation is not decoration.
///
/// §7.3.1.2.3 makes the whole comment the annotation string, so
/// `//@key and here is why` is a file this frontend must not guess at.
#[test]
fn text_after_a_suffix_annotation_is_reported() {
    let parsed = parse("struct S { long x; //@key because it identifies the shape\n};");
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.spec_reference == "DDS-XTypes 1.3 §7.3.1.2.3"),
        "{:?}",
        parsed.diagnostics()
    );
}

/// A comment that is not the suffix form stays a comment.
///
/// §7.3.1.2.3 admits nothing between the double slash and the at-sign,
/// which is what keeps `// @key` and `/// @key` out of the annotation
/// grammar — and `///@key`, whose at-sign follows three slashes.
#[test]
fn only_the_exact_marker_opens_a_suffix_annotation() {
    for source in [
        "struct S { long x; // @key\n};",
        "struct S { long x; /// @key\n};",
        "struct S { long x; ///@key\n};",
        "struct S { long x; /*@key*/ };",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.has_errors(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let ast = lower(&parsed.syntax(), "test.idl").ast;
        assert!(
            !xtypes_of_member(&ast, "S", 0).is_key,
            "{source:?} is a comment, not an annotation"
        );
    }
}

/// The tree still reproduces the file byte for byte.
///
/// The suffix form's tokens come out of a comment's own bytes, so the
/// round trip is the check that none of them were invented or dropped.
/// Spec §2.2.
#[test]
fn a_file_with_suffix_annotations_round_trips() {
    for source in [
        "struct S { long x; //@key\n};",
        "struct S {\n  long x; //@id(7)\n//@key\n}; //@appendable\n",
        "enum E { A, //@value(3)\nB //@default_literal\n};\n",
        "struct S { long x; //@unit(\"m//@s\")\n};",
    ] {
        assert_eq!(parse(source).text(), source, "{source:?}");
    }
}

/// The form reaches the elements Table 21 names beside members and
/// types: an enumerated literal and a union case.
#[test]
fn the_suffix_form_reaches_literals_and_union_cases() {
    let ast = model("enum E { A, B //@default_literal\n};");
    let id = type_of(&ast, "E");
    let TypeKind::Enumeration { enumerators } = &ast.type_(id).expect("E").kind else {
        panic!("expected an enumeration");
    };
    let second = enumerators[1];
    let node = ast.enumerator(second).expect("B");
    assert!(
        ast.facets(AnyId::Enumerator(second, node.node_id))
            .and_then(|f| f.xtypes.as_ref())
            .is_some_and(|x| x.is_default_literal),
        "`B //@default_literal` must mark B"
    );

    let ast = model(
        "\
enum Kind { ONE, TWO };
union U switch (Kind) {
  case ONE: long a; //@external
  case TWO: short b;
};",
    );
    let id = type_of(&ast, "U");
    let TypeKind::Union { cases, .. } = &ast.type_(id).expect("U").kind else {
        panic!("expected a union");
    };
    let member = ast.case(cases[0]).expect("case ONE").member;
    let node = ast.member(member).expect("member");
    assert!(
        ast.facets(AnyId::Member(member, node.node_id))
            .and_then(|f| f.xtypes.as_ref())
            .is_some_and(|x| x.is_external),
        "`long a; //@external` must mark the case member"
    );
}

/// DDS-XTypes 1.3 §7.3.1.2.4, Table 22: `struct <ann_identifier> {
/// <ann_members> }; //@annotation` is the declaration of an annotation
/// type "containing the members <ann_members>", for compilers that do not
/// know the `@annotation` token.
#[test]
fn table_22s_declaration_form_declares_an_annotation_type() {
    let ast = model(
        "struct Level {
            long value;
            string note; //@default(\"none\")
        }; //@annotation
        @Level(value = 3) struct Used { long m; };",
    );
    assert!(
        ast.find_by_scoped_name(&scoped(&["Level"]))
            .ok()
            .and_then(AnyId::as_type)
            .is_none(),
        "it is no structure"
    );
    let (id, definition) = ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Level")
        .expect("the annotation type Level");
    let _ = id;
    let members: Vec<_> = definition
        .members
        .iter()
        .map(|member| ast.annotation_member(*member).expect("member"))
        .map(|member| (member.name.as_str(), member.default.is_some()))
        .collect();
    assert_eq!(members, [("value", false), ("note", true)]);

    // And an application of it names the declaration.
    let used = facets_of_type(&ast, "Used");
    assert_eq!(used.annotations.len(), 1);
    assert!(used.annotations[0].definition.resolved().is_some());
}

// -- DDS-XTypes 1.3 §7.3.1.2.1: the boolean parameter of the built-ins ---

/// The annotations of §7.3.1.2.1.2 to §7.3.1.2.1.14 declare `boolean
/// value default TRUE`. Written with `FALSE` they say the opposite of
/// what the bare form says.
#[test]
fn the_boolean_parameter_of_a_builtin_annotation_is_read() {
    let ast = model(
        "struct S {
            @key(FALSE) long a;
            @key(TRUE) long b;
            @key long c;
            @optional(FALSE) long d;
            @external(FALSE) long e;
            @must_understand(FALSE) long f;
            @non_serialized(FALSE) long g;
            @optional long h;
        };
        @ignore_literal_names(FALSE) enum E { X, Y };
        @ignore_literal_names enum F { X2, Y2 };",
    );
    let flag =
        |index: usize, read: fn(&XTypesFacet) -> bool| read(xtypes_of_member(&ast, "S", index));
    assert!(!flag(0, |x| x.is_key), "@key(FALSE)");
    assert!(flag(1, |x| x.is_key), "@key(TRUE)");
    assert!(flag(2, |x| x.is_key), "@key");
    assert!(!flag(3, |x| x.is_optional), "@optional(FALSE)");
    assert!(!flag(4, |x| x.is_external), "@external(FALSE)");
    assert!(
        !flag(5, |x| x.is_must_understand),
        "@must_understand(FALSE)"
    );
    assert!(!flag(6, |x| x.is_non_serialized), "@non_serialized(FALSE)");
    assert!(flag(7, |x| x.is_optional), "@optional");
    assert!(!xtypes_of_type(&ast, "E").ignore_literal_names);
    assert!(xtypes_of_type(&ast, "F").ignore_literal_names);
}

/// §7.3.1.2.5: annotation names are "not case-sensitive". A name the file
/// declares itself keeps its own spelling.
#[test]
fn annotation_names_are_matched_without_regard_to_case() {
    let ast = model("struct S { @Key long a; @KEY @Optional long b; long c; };");
    assert!(xtypes_of_member(&ast, "S", 0).is_key);
    let b = xtypes_of_member(&ast, "S", 1);
    assert!(b.is_key && b.is_optional);

    let declared = model("@annotation Key { long n; };\nstruct S { @Key(1) long a; };");
    let member = declared
        .members_of(type_of(&declared, "S"))
        .first()
        .copied()
        .expect("a");
    let node = declared.member(member).expect("member");
    let facets = declared
        .facets(AnyId::Member(member, node.node_id))
        .expect("facets");
    assert!(
        facets.xtypes.as_ref().is_none_or(|x| !x.is_key),
        "the file's own `Key` is not the builtin `key`"
    );
    assert_eq!(
        facets.annotations.len(),
        1,
        "it is kept as a user annotation"
    );
}

// -- DDS-XTypes 1.3 §7.3.1.2.2, Table 21: where an annotation applies ----

fn refused_as_misplaced(source: &str) -> bool {
    diagnostics(source).iter().any(|d| {
        d.code == zerodds_idl_parse::DiagnosticCode::InvalidAnnotation
            && d.spec_reference.contains("Table 21")
            && d.message.contains("applies to")
    })
}

/// "The application of the annotations listed above is restricted to the
/// type elements specified in Table 21."
#[test]
fn a_builtin_annotation_outside_its_row_of_table_21_is_refused() {
    for source in [
        // @bit_bound: enumerated types, bit mask types.
        "@bit_bound(8) struct S { long a; };",
        // @position: bitmask values.
        "struct S { @position(2) long a; };",
        // @autoid: module, structure and union declarations.
        "@autoid(HASH) enum E { A, B };",
        // @optional, @must_understand, @non_serialized: structure members.
        "@optional struct S { long a; };",
        "enum E { @must_understand A, B };",
        // @id, @hashid: structure and union members.
        "@id(3) struct S { long a; };",
        // @key: structure members and union discriminators.
        "@key struct S { long a; };",
        // @extensibility and its shortcuts, @nested: type declarations.
        "struct S { @final long a; };",
        "struct S { @nested long a; };",
        // @default_literal, @value: enumerated literals.
        "struct S { @value(1) long a; };",
        "struct S { @default_literal long a; };",
    ] {
        assert!(
            refused_as_misplaced(source),
            "{source:?} was not refused: {:?}",
            diagnostics(source)
        );
    }
}

/// The rows' own places are accepted.
#[test]
fn a_builtin_annotation_on_its_row_of_table_21_is_accepted() {
    for source in [
        "@bit_bound(8) enum E { A, B };",
        "@bit_bound(8) bitmask M { A, B };",
        "bitmask M { @position(2) A, B };",
        "@autoid(HASH) struct S { long a; };",
        "@autoid(SEQUENTIAL) module Mo { struct S { long a; }; };",
        "@autoid(HASH) union U switch (long) { case 1: long a; };",
        "struct S { @optional @must_understand @non_serialized long a; };",
        "@mutable struct S { @id(3) long a; @hashid(\"x\") long b; };",
        "struct S { @key long a; };",
        "@final struct S { long a; };\n@appendable enum E { A, B };\n@nested struct T { long a; };",
        "enum E { @default_literal @value(3) A, B };",
    ] {
        assert!(
            !refused_as_misplaced(source),
            "{source:?} was refused: {:?}",
            diagnostics(source)
        );
    }
}

// -- modules, @nested, @topic, @autoid, @data_representation -------------

fn nested_type(ast: &Ast, module: &str, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&scoped(&[module, name]))
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{module}::{name} is not in the model"))
}

/// §7.3.1.2.1.1 and Table 21: `@autoid` applies to a module, and the
/// types in it that carry none of their own follow it.
///
/// The hash of a member name is the first four octets of its MD5 read as
/// a little-endian integer and masked to 28 bits (the clause's
/// Example 2): `a` is `{0x0c, 0xc1, 0x75, 0xb9}`, 0x0975C10C, and `b` is
/// `{0x92, 0xeb, 0x5f, 0xfe}`, 0x0E5FEB92.
#[test]
fn autoid_on_a_module_numbers_the_members_of_its_types() {
    let ast = model(
        "@autoid(HASH) module M {
            struct S { long a; long b; };
            @autoid(SEQUENTIAL) struct T { long a; long b; };
        };",
    );
    let assignment_of = |type_name: &str, index: usize| {
        let id = nested_type(&ast, "M", type_name);
        let member = ast.members_of(id)[index];
        let node = ast.member(member).expect("member");
        ast.facets(AnyId::Member(member, node.node_id))
            .and_then(|f| f.xtypes.as_ref())
            .and_then(|x| x.member_id.clone())
            .expect("a member id")
    };
    assert_eq!(assignment_of("S", 0).effective, 0x0975_C10C);
    assert_eq!(assignment_of("S", 1).effective, 0x0E5F_EB92);
    assert_eq!(
        assignment_of("S", 0).origin,
        zerodds_ast::MemberIdOrigin::AutoHash
    );
    assert_eq!(assignment_of("T", 0).effective, 0);
    assert_eq!(assignment_of("T", 1).effective, 1);
}

/// §7.3.1.2.1.7: a type's `@nested(FALSE)` wins over the module's
/// `@default_nested(TRUE)`; §7.3.1.2.1.16: "The @topic annotation
/// overrides the @nested and @default_nested annotations."
#[test]
fn nested_false_and_topic_override_the_default_nested_of_the_module() {
    let ast = model(
        "@default_nested(TRUE) module M {
            struct Plain { long a; };
            @nested(FALSE) struct Explicit { long a; };
            @topic struct Topic { long a; };
            @nested(TRUE) struct Nested { long a; };
        };",
    );
    assert!(ast.effective_nested(nested_type(&ast, "M", "Plain")));
    assert!(!ast.effective_nested(nested_type(&ast, "M", "Explicit")));
    assert!(!ast.effective_nested(nested_type(&ast, "M", "Topic")));
    assert!(ast.effective_nested(nested_type(&ast, "M", "Nested")));
}

/// `@topic` is legal on the aggregated types DDS allows as topic types
/// (§7.3.1.2.1.16, §7.6.2).
#[test]
fn topic_on_a_type_that_is_no_topic_type_is_refused() {
    let found = diagnostics("@topic enum E { A, B };");
    assert!(
        found.iter().any(
            |d| d.code == zerodds_idl_parse::DiagnosticCode::InvalidAnnotation
                && d.spec_reference.contains("7.6.2")
        ),
        "{found:?}"
    );
    assert!(diagnostics("@topic struct S { long a; };").is_empty());
    assert!(diagnostics("@topic union U switch (long) { case 1: long a; };").is_empty());
}

/// §7.3.1.2.1.7: "IDL compilers shall provide the means to change the
/// default value for non-annotated top-level modules."
#[test]
fn the_default_for_unannotated_top_level_modules_is_an_option() {
    let source = "
        struct Global { long a; };
        module Plain { struct S { long a; }; };
        @default_nested(FALSE) module Explicit { struct S { long a; }; };";
    let parsed = parse(source);
    let nested = |default_nested: Option<bool>| {
        let lowered = lower_with(
            &parsed.syntax(),
            "nested.idl",
            Options {
                default_nested,
                ..Options::default()
            },
        );
        let ast = lowered.ast;
        let global = type_of(&ast, "Global");
        (
            ast.effective_nested(global),
            ast.effective_nested(nested_type(&ast, "Plain", "S")),
            ast.effective_nested(nested_type(&ast, "Explicit", "S")),
        )
    };
    assert_eq!(
        nested(None),
        (false, false, false),
        "the specification's FALSE"
    );
    assert_eq!(
        nested(Some(true)),
        (true, true, false),
        "the option fills the silence and a module's own word still wins"
    );
}

/// §7.3.1.2.1.15: `@data_representation` applies to modules and types.
#[test]
fn data_representation_on_a_module_is_the_default_of_its_types() {
    let ast = model(
        "@data_representation(XCDR2) module M {
            struct S { long a; };
            @data_representation(XCDR1) struct T { long a; };
        };
        struct Free { long a; };",
    );
    let mask = |id| ast.effective_data_representation(id);
    let s = mask(nested_type(&ast, "M", "S")).expect("the module's mask");
    assert!(s.xcdr2 && !s.xcdr1);
    let t = mask(nested_type(&ast, "M", "T")).expect("the type's own mask");
    assert!(t.xcdr1 && !t.xcdr2);
    assert_eq!(mask(type_of(&ast, "Free")), None);
}

/// §7.3.1.2.1.15: "If the @data_representation annotation is not present
/// it is interpreted as if the DataRepresentationMask value was set to
/// 0xffffffff." A declared mask stays what it says.
#[test]
fn a_type_without_data_representation_has_the_mask_0xffffffff() {
    let ast = model(
        "struct Free { long a; };
        @data_representation(XCDR2) struct Two { long a; };
        @data_representation(XCDR1 | XML) struct Mixed { long a; };",
    );
    let free = ast.data_representation_mask(type_of(&ast, "Free"));
    assert_eq!(free, zerodds_ast::DataRepresentationMask::ALL);
    assert_eq!(
        zerodds_ast::DataRepresentationMask::ALL_BITS,
        0xffff_ffff,
        "the value the clause names"
    );
    assert!(free.xcdr1 && free.xcdr2 && free.xml);
    let two = ast.data_representation_mask(type_of(&ast, "Two"));
    assert_eq!(two.bits(), 0b100, "XCDR2 is position 2");
    let mixed = ast.data_representation_mask(type_of(&ast, "Mixed"));
    assert_eq!(mixed.bits(), 0b011, "XCDR1 is position 0, XML position 1");
}

/// §7.3.1.2.1.8: "It shall be considered an error for it to be applied
/// to the same type multiple times."
#[test]
fn the_extensibility_is_given_once() {
    for source in [
        "@final @mutable struct S { long a; };",
        "@final @final struct S { long a; };",
        "@mutable @extensibility(FINAL) struct S { long a; };",
        "@appendable @appendable enum E { A, B };",
    ] {
        assert!(
            diagnostics(source).iter().any(|d| d.code
                == zerodds_idl_parse::DiagnosticCode::InvalidAnnotation
                && d.spec_reference.contains("7.3.1.2.1.8")),
            "{source:?}: {:?}",
            diagnostics(source)
        );
    }
    assert!(diagnostics("@final struct S { long a; };").is_empty());
}

/// §7.3.1.2.1.5: "the value member of the annotation may take any value
/// from 1 to 32, inclusive, when this annotation is applied to an
/// enumerated type."
#[test]
fn an_enumerations_bit_bound_is_from_1_to_32() {
    for source in [
        "@bit_bound(33) enum E { A };",
        "@bit_bound(0) enum E { A };",
    ] {
        assert!(
            diagnostics(source)
                .iter()
                .any(|d| d.code == zerodds_idl_parse::DiagnosticCode::InvalidBound),
            "{source:?}: {:?}",
            diagnostics(source)
        );
    }
    assert!(diagnostics("@bit_bound(32) enum E { A };").is_empty());
    assert!(diagnostics("@bit_bound(1) enum E { A };").is_empty());
    // A bitmask keeps the bound of IDL 4.2 §7.4.13.4.3.3.
    assert!(diagnostics("@bit_bound(64) bitmask M { A };").is_empty());
}

/// DDS-XTypes 1.3 §7.2.2.4.8.2: the placement "shall be interpreted in a
/// case-insensitive manner" and the clause names `begin-declaration-file`
/// and `end-declaration-file`.
#[test]
fn a_verbatim_placement_is_read_without_regard_to_case_and_in_the_xtypes_spelling() {
    for (written, expected) in [
        ("begin-declaration-file", VerbatimPlacement::BeginFile),
        ("BEGIN-DECLARATION-FILE", VerbatimPlacement::BeginFile),
        ("End-Declaration-File", VerbatimPlacement::EndFile),
        ("Before-Declaration", VerbatimPlacement::BeforeDeclaration),
        ("BEGIN-declaration", VerbatimPlacement::BeginDeclaration),
        ("end-DECLARATION", VerbatimPlacement::EndDeclaration),
        ("After-declaration", VerbatimPlacement::AfterDeclaration),
        ("end_file", VerbatimPlacement::EndFile),
    ] {
        let ast = model(&format!(
            "@verbatim(language = \"c\", placement = \"{written}\", text = \"x\")
             struct S {{ long a; }};"
        ));
        let directives = &xtypes_of_type(&ast, "S").verbatim;
        assert_eq!(directives.len(), 1, "{written}");
        assert_eq!(directives[0].placement, expected, "{written}");
    }
    let found = diagnostics(
        "@verbatim(language = \"c\", placement = \"middle-of-nowhere\", text = \"x\")
         struct S { long a; };",
    );
    assert!(
        found
            .iter()
            .any(|d| d.code == zerodds_idl_parse::DiagnosticCode::InvalidAnnotation),
        "{found:?}"
    );
}

/// Table 21 lists `@must_understand` for "Structure Members" only, so a
/// union branch that carries it is refused rather than given a flag no
/// consumer reads.
#[test]
fn must_understand_on_a_union_branch_is_refused() {
    let found = diagnostics("union U switch (long) { case 1: @must_understand long b; };");
    assert!(
        found
            .iter()
            .any(|d| d.message.contains("must_understand") && d.message.contains("applies to")),
        "{found:?}"
    );
}

/// DDS-RPC 1.0 §7.5.1.2.1: `@Mutable`, `@Empty`, `@Choice` and `@Autoid` are
/// capitalised annotations the specification defines. Scope lookup is
/// case-insensitive (IDL 4.2 §7.5), and `@Mutable` differs from DDS-XTypes'
/// `@mutable` in case only: the lookup finds neither, and the reference to the
/// built-in declaration stayed pending and broke in the resolve pass.
#[test]
fn the_dds_rpc_annotations_resolve_to_their_builtin_declarations() {
    let mut ast = model(
        "exception E { long x; };
         interface I {
           @Mutable long a(in long v) raises (E);
           @Empty long b(in long v) raises (E);
           @Mutable long c(in long v);
         };",
    );
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.broken.is_empty(), "{:?}", report.broken);
}

/// DDS-RPC 1.0 writes `@DDSService` where IDL 4.2 has `@service` (§7.3.1.3,
/// §7.4.2.2, §7.3.2): the same marker, with the same `name`, and no reference
/// left unresolved for an annotation nobody declares.
#[test]
fn dds_service_marks_the_same_thing_as_service() {
    let mut ast = model(
        r#"@DDSService(name="Calc") interface A { void f(); };
           @service(name="Calc") interface B { void f(); };"#,
    );
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.broken.is_empty(), "{:?}", report.broken);
    let service = |name: &str| {
        let (_, node) = ast
            .interfaces()
            .find(|(_, node)| node.name == name)
            .expect("interface");
        let corba = node.facets.corba.as_ref().expect("corba facet");
        (corba.is_service, corba.service_arguments.len())
    };
    assert_eq!(service("A"), (true, 1));
    assert_eq!(service("A"), service("B"));
}

/// The diagnostics of `source` that name `@Choice` as misplaced.
fn choice_refusals(source: &str) -> Vec<String> {
    diagnostics(source)
        .into_iter()
        .filter(|d| d.message.contains("Choice") && d.message.contains("applies to structures"))
        .map(|d| d.message)
        .collect()
}

/// DDS-RPC 1.0 §7.5.1.2.1.1: "The @Choice annotation is allowed on
/// structures only."
#[test]
fn choice_is_refused_on_anything_but_a_structure() {
    assert!(
        choice_refusals("struct S { long a; }; @Choice struct T { long a; };").is_empty(),
        "a structure takes it"
    );
    assert_eq!(
        choice_refusals("@Choice union U switch (long) { case 1: long a; };").len(),
        1,
        "a union"
    );
    assert_eq!(
        choice_refusals("struct S { @Choice long a; };").len(),
        1,
        "a member"
    );
    assert_eq!(
        choice_refusals("@Choice interface I { void f(); };").len(),
        1,
        "an interface"
    );
    assert_eq!(
        choice_refusals("@Choice enum E { A, B };").len(),
        1,
        "an enumeration"
    );
}

/// §7.5.1.2.1.1: "When present, the structure shall be interpreted as if
/// the structure had the @Extensibility(MUTABLE_EXTENSIBILITY) annotation
/// and all the members of the structure had the @Optional annotation."
#[test]
fn a_choice_structure_is_mutable_with_every_member_optional() {
    let ast = model("@Choice struct Request { long add; string sub; long long mul; };");
    let facet = xtypes_of_type(&ast, "Request");
    assert_eq!(facet.extensibility, Some(Extensibility::Mutable));
    for index in 0..3 {
        assert!(
            xtypes_of_member(&ast, "Request", index).is_optional,
            "member {index}"
        );
    }
    // The application itself stays in the model.
    assert!(
        facets_of_type(&ast, "Request")
            .annotations
            .iter()
            .any(|applied| applied.name == "Choice")
    );
}

/// `@Choice @mutable` says the same twice and is accepted; another
/// extensibility contradicts the clause.
#[test]
fn a_choice_structure_cannot_be_final_or_appendable() {
    for ok in [
        "@Choice @mutable struct S { long a; };",
        "@mutable @Choice struct S { long a; };",
    ] {
        assert!(diagnostics(ok).is_empty(), "{ok}: {:?}", diagnostics(ok));
    }
    for bad in [
        "@Choice @final struct S { long a; };",
        "@appendable @Choice struct S { long a; };",
    ] {
        assert!(
            diagnostics(bad)
                .iter()
                .any(|d| d.message.contains("@Choice") && d.message.contains("@mutable")),
            "{bad}: {:?}",
            diagnostics(bad)
        );
    }
}

/// A structure without `@Choice` is not touched.
#[test]
fn a_plain_structure_keeps_its_members_required() {
    let ast = model("struct S { long a; };");
    assert!(!xtypes_of_member(&ast, "S", 0).is_optional);
}

/// §7.5.1.2.1: "All annotations below are defined in the dds::rpc
/// module." The qualified spelling names the same annotation as the
/// simple one, and resolves.
#[test]
fn the_qualified_dds_rpc_spelling_is_the_same_annotation() {
    let mut ast = model(
        "@dds::rpc::Choice struct Request { long add; long sub; };
         @dds::rpc::Autoid struct Other { long x; };",
    );
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.broken.is_empty(), "{:?}", report.broken);
    let facet = xtypes_of_type(&ast, "Request");
    assert_eq!(facet.extensibility, Some(Extensibility::Mutable));
    assert!(xtypes_of_member(&ast, "Request", 0).is_optional);
    let applied = &facets_of_type(&ast, "Request").annotations;
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].name, "Choice");
    assert!(
        matches!(applied[0].definition, zerodds_ast_core::Ref::Resolved(_)),
        "the reference reaches the built-in declaration"
    );
    assert_eq!(
        xtypes_of_member(&ast, "Other", 0)
            .member_id
            .as_ref()
            .map(|id| id.effective),
        Some(zerodds_idl_lower::hashed_member_id("x"))
    );
}

/// §7.5.1.2.1.2: "The @Autoid annotation indicates that the member-id of
/// each member shall be computed using the HASH algorithm applied to the
/// name of the member." On a structure, for every member.
#[test]
fn autoid_on_a_structure_assigns_hashed_member_ids() {
    let ast = model("@Autoid struct S { long add; long sub; long mul; };");
    for (index, name) in ["add", "sub", "mul"].into_iter().enumerate() {
        let assignment = xtypes_of_member(&ast, "S", index)
            .member_id
            .clone()
            .expect("an id");
        assert_eq!(
            assignment.effective,
            zerodds_idl_lower::hashed_member_id(name)
        );
        assert_eq!(assignment.origin, MemberIdOrigin::AutoHash, "{name}");
    }
    // Without it the ids count up from zero.
    let plain = model("struct S { long add; long sub; };");
    assert_eq!(
        xtypes_of_member(&plain, "S", 1)
            .member_id
            .as_ref()
            .map(|id| id.effective),
        Some(1)
    );
}

/// `@Autoid(HASH)` names the policy; on one data member it hashes that
/// member's name and leaves the rest of the structure counting.
#[test]
fn autoid_on_a_member_hashes_that_members_name() {
    let ast = model("struct S { long first; @Autoid long second; };");
    let first = xtypes_of_member(&ast, "S", 0)
        .member_id
        .clone()
        .expect("id");
    assert_eq!(first.effective, 0);
    let second = xtypes_of_member(&ast, "S", 1)
        .member_id
        .clone()
        .expect("id");
    assert_eq!(
        second.effective,
        zerodds_idl_lower::hashed_member_id("second")
    );
    let explicit = model("@Autoid(HASH) struct T { long a; };");
    assert_eq!(
        xtypes_of_member(&explicit, "T", 0)
            .member_id
            .as_ref()
            .map(|id| id.effective),
        Some(zerodds_idl_lower::hashed_member_id("a"))
    );
}

/// `@Autoid` is allowed on structures and data members (§7.5.1.2.1.2),
/// and is refused on an enumeration, where it can mean nothing.
#[test]
fn autoid_is_refused_where_there_are_no_member_ids() {
    let found = diagnostics("@Autoid enum E { A, B };");
    assert!(
        found
            .iter()
            .any(|d| d.message.contains("Autoid") && d.message.contains("applies to")),
        "{found:?}"
    );
}
