// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Independent threshold arithmetic, including the variable-extent distinction.
#![allow(clippy::expect_used, clippy::unwrap_used)]
use zerodds_ast::{
    Ast, Bound, CaseLabel, CharWidth, MemberSpec, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::keyhash::key_hash_form_for;
use zerodds_ast_layout::{Encoding, Extent, KeyHashForm, LayoutKey};

fn add(ast: &mut Ast, name: &str, kind: TypeKind) -> Id<TypeNode> {
    ast.add_type(ast.root_module(), name, kind).expect("type")
}
fn structure(ast: &mut Ast, name: &str) -> Id<TypeNode> {
    add(
        ast,
        name,
        TypeKind::Structure {
            base: None,
            members: vec![],
        },
    )
}
fn member(ast: &mut Ast, root: Id<TypeNode>, name: &str, target: Id<TypeNode>, keyed: bool) {
    let member = ast
        .add_member(root, MemberSpec::new(name, Ref::Resolved(target)))
        .expect("member");
    let any = ast.any_of_member(member).expect("member id");
    ast.facets_mut(any).expect("facets").xtypes_mut().is_key = keyed;
}
fn bounded(ast: &mut Ast, n: i128) -> Bound {
    Bound::Const(ast.add_int_expr(n).expect("bound"))
}
fn string(ast: &mut Ast, n: i128) -> Id<TypeNode> {
    let bound = bounded(ast, n);
    add(
        ast,
        &format!("String{n}"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        },
    )
}
fn form(ast: &Ast, root: Id<TypeNode>) -> KeyHashForm {
    key_hash_form_for(ast, root, LayoutKey::key(Encoding::PlainCdr2)).expect("maximum")
}

#[test]
fn bounded_string_threshold_is_maximum_not_fixed_extent() {
    for (n, expected) in [
        (8, KeyHashForm::ZeroPadded),
        (11, KeyHashForm::ZeroPadded),
        (12, KeyHashForm::Md5),
        (32, KeyHashForm::Md5),
    ] {
        let mut ast = Ast::new();
        let text = string(&mut ast, n);
        let root = structure(&mut ast, "S");
        member(&mut ast, root, "name", text, true);
        let layout = zerodds_ast_layout::plan(
            &ast,
            text,
            LayoutKey::key(Encoding::PlainCdr2),
            &mut Default::default(),
        )
        .expect("layout");
        assert_eq!(
            layout.extent,
            Extent::Unbounded,
            "actual length remains variable"
        );
        assert_eq!(form(&ast, root), expected, "4 + bound + NUL");
    }
}

#[test]
fn alignment_between_variable_key_members_counts() {
    for (second_bound, expected) in [(3, KeyHashForm::ZeroPadded), (4, KeyHashForm::Md5)] {
        let mut ast = Ast::new();
        let first = string(&mut ast, 1);
        let second = string(&mut ast, second_bound);
        let root = structure(&mut ast, "S");
        member(&mut ast, root, "a", first, true);
        member(&mut ast, root, "b", second, true);
        assert_eq!(form(&ast, root), expected, "6 bytes, align to8, then4+n+1");
    }
}

#[test]
fn sequence_and_map_bounds_include_prefix_and_each_entry() {
    for (count, expected) in [(12, KeyHashForm::ZeroPadded), (13, KeyHashForm::Md5)] {
        let mut ast = Ast::new();
        let octet = add(&mut ast, "Octet", TypeKind::Primitive(PrimitiveKind::Octet));
        let bound = bounded(&mut ast, count);
        let seq = add(
            &mut ast,
            "Seq",
            TypeKind::Sequence {
                element: Ref::Resolved(octet),
                bound,
            },
        );
        let root = structure(&mut ast, "S");
        member(&mut ast, root, "seq", seq, true);
        assert_eq!(form(&ast, root), expected, "4-byte count plus octets");
    }
    for (count, expected) in [(1, KeyHashForm::ZeroPadded), (2, KeyHashForm::Md5)] {
        let mut ast = Ast::new();
        let long = add(&mut ast, "Long", TypeKind::Primitive(PrimitiveKind::Int32));
        let bound = bounded(&mut ast, count);
        let map = add(
            &mut ast,
            "Map",
            TypeKind::Map {
                key: Ref::Resolved(long),
                value: Ref::Resolved(long),
                bound,
            },
        );
        let root = structure(&mut ast, "S");
        member(&mut ast, root, "map", map, true);
        assert_eq!(
            form(&ast, root),
            expected,
            "4-byte count plus8 per primitive pair"
        );
    }
}

#[test]
fn nested_keys_and_arrays_do_not_double_count_or_add_trailing_padding() {
    let mut ast = Ast::new();
    let short = add(&mut ast, "Short", TypeKind::Primitive(PrimitiveKind::Int16));
    let octet = add(&mut ast, "Octet", TypeKind::Primitive(PrimitiveKind::Octet));
    let odd = structure(&mut ast, "Odd");
    member(&mut ast, odd, "a", short, true);
    member(&mut ast, odd, "b", octet, true);
    let dimension = ast.add_int_expr(3).expect("three");
    let array = add(
        &mut ast,
        "Pair",
        TypeKind::Array {
            element: Ref::Resolved(odd),
            dims: vec![dimension],
        },
    );
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "pair", array, true);
    member(&mut ast, root, "tail", octet, true);
    assert_eq!(
        form(&ast, root),
        KeyHashForm::ZeroPadded,
        "4 DHEADER +3+1pad+3+1pad+3 +1 =16"
    );
    let nested = structure(&mut ast, "Nested");
    member(&mut ast, nested, "odd", odd, true);
    assert_eq!(form(&ast, nested), KeyHashForm::ZeroPadded);
}

#[test]
fn union_takes_largest_branch_not_sum() {
    for (n, expected) in [(7, KeyHashForm::ZeroPadded), (8, KeyHashForm::Md5)] {
        let mut ast = Ast::new();
        let long = add(&mut ast, "Long", TypeKind::Primitive(PrimitiveKind::Int32));
        let text = string(&mut ast, n);
        let union = add(
            &mut ast,
            "Choice",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: vec![],
                default_case: None,
            },
        );
        let zero = ast.add_int_expr(0).expect("zero");
        ast.add_case(
            union,
            vec![CaseLabel::Value(zero)],
            MemberSpec::new("number", Ref::Resolved(long)),
        )
        .expect("case");
        ast.add_case(
            union,
            vec![CaseLabel::Default],
            MemberSpec::new("text", Ref::Resolved(text)),
        )
        .expect("default");
        let root = structure(&mut ast, "S");
        member(&mut ast, root, "choice", union, true);
        assert_eq!(
            form(&ast, root),
            expected,
            "4-byte discriminator plus largest branch"
        );
    }
}

#[test]
fn huge_bounds_never_wrap_and_invalid_bounds_refuse() {
    let mut ast = Ast::new();
    let text = string(&mut ast, u32::MAX as i128);
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "name", text, true);
    assert_eq!(form(&ast, root), KeyHashForm::Md5);
    let invalid = string(&mut ast, -1);
    let bad = structure(&mut ast, "Bad");
    member(&mut ast, bad, "name", invalid, true);
    assert!(key_hash_form_for(&ast, bad, LayoutKey::key(Encoding::PlainCdr2)).is_err());
}

#[test]
fn bounded_recursion_is_not_a_finite_depth_bound() {
    let mut ast = Ast::new();
    let recursive = structure(&mut ast, "Node");
    let bound = bounded(&mut ast, 1);
    let seq = add(
        &mut ast,
        "Children",
        TypeKind::Sequence {
            element: Ref::Resolved(recursive),
            bound,
        },
    );
    member(&mut ast, recursive, "children", seq, true);
    assert_eq!(
        form(&ast, recursive),
        KeyHashForm::Md5,
        "each node may have one further child"
    );
}

#[test]
fn unbounded_count_of_zero_byte_elements_has_a_finite_wire_maximum() {
    let mut ast = Ast::new();
    let empty = structure(&mut ast, "Empty");
    let seq = add(
        &mut ast,
        "EmptyItems",
        TypeKind::Sequence {
            element: Ref::Resolved(empty),
            bound: Bound::Unbounded,
        },
    );
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "items", seq, true);
    assert_eq!(
        form(&ast, root),
        KeyHashForm::ZeroPadded,
        "DHEADER + count, every element writes zero bytes"
    );
}

#[test]
fn wstring_maximum_respects_unit_width_and_termination_policy() {
    let mut ast = Ast::new();
    let bound = bounded(&mut ast, 6);
    let text = add(
        &mut ast,
        "Wide",
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        },
    );
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "name", text, true);
    assert_eq!(form(&ast, root), KeyHashForm::ZeroPadded, "4 + 6*2 =16");
    let mut key = LayoutKey::key(Encoding::PlainCdr2);
    key.wstring_length = zerodds_ast_layout::WstringLengthPolicy::CHARACTER_COUNT_TERMINATED;
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("policy"),
        KeyHashForm::Md5,
        "4 + (6+1)*4 is not the default two-byte unterminated holder"
    );
}

#[test]
fn holder_alignment_cap_is_four_without_replacing_xcdr1_policies() {
    let mut ast = Ast::new();
    let long = add(&mut ast, "Long", TypeKind::Primitive(PrimitiveKind::Int32));
    let double = add(
        &mut ast,
        "Double",
        TypeKind::Primitive(PrimitiveKind::Float64),
    );
    let octet = add(&mut ast, "Octet", TypeKind::Primitive(PrimitiveKind::Octet));
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "a", long, true);
    member(&mut ast, root, "b", double, true);
    member(&mut ast, root, "c", long, true);
    let key = LayoutKey::key(Encoding::PlainCdr1);
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("holder"),
        KeyHashForm::ZeroPadded,
        "actual Writer cap4: 4 + 8 + 4 =16, not cap8's20"
    );
    member(&mut ast, root, "d", octet, true);
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("holder"),
        KeyHashForm::Md5,
        "independent negative control: an additional octet requires17"
    );
}

#[test]
fn collection_elements_apply_keyless_nested_whole_policy() {
    let mut ast = Ast::new();
    let long = add(&mut ast, "Long", TypeKind::Primitive(PrimitiveKind::Int32));
    let leaf = structure(&mut ast, "Leaf");
    member(&mut ast, leaf, "a", long, true);
    for name in ["b", "c", "d"] {
        member(&mut ast, leaf, name, long, false);
    }
    let envelope = structure(&mut ast, "Envelope");
    member(&mut ast, envelope, "leaf", leaf, false);
    let bound = bounded(&mut ast, 1);
    let sequence = add(
        &mut ast,
        "Items",
        TypeKind::Sequence {
            element: Ref::Resolved(envelope),
            bound,
        },
    );
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "items", sequence, true);
    let mut key = LayoutKey::key(Encoding::PlainCdr1);
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("recursive"),
        KeyHashForm::ZeroPadded,
        "recursive filtering: count4 + leaf key4 =8"
    );
    key.key_holder_nesting = zerodds_ast_layout::KeyHolderNesting::KeylessNestedWhole;
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("whole"),
        KeyHashForm::Md5,
        "keyless element switches its subtree to data: count4 + leaf16 =20"
    );
}

#[test]
fn optional_parameter_end_padding_follows_header_policy() {
    let mut ast = Ast::new();
    let octet = add(&mut ast, "Octet", TypeKind::Primitive(PrimitiveKind::Octet));
    let nested = structure(&mut ast, "OptionalByte");
    let optional = ast
        .add_member(nested, MemberSpec::new("value", Ref::Resolved(octet)))
        .expect("member");
    let any = ast.any_of_member(optional).expect("member id");
    ast.facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .is_optional = true;
    let eleven = ast.add_int_expr(11).expect("eleven");
    let tail = add(
        &mut ast,
        "Tail",
        TypeKind::Array {
            element: Ref::Resolved(octet),
            dims: vec![eleven],
        },
    );
    let root = structure(&mut ast, "S");
    member(&mut ast, root, "optional", nested, true);
    member(&mut ast, root, "tail", tail, true);
    let mut key = LayoutKey::key(Encoding::PlainCdr1);
    key.pl_cdr1_headers = zerodds_ast_layout::PlCdr1HeaderPolicy::ShortTrueLength;
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("true length"),
        KeyHashForm::ZeroPadded,
        "header4 + present octet1 + tail11 =16; no invented trailing pad"
    );
    key.pl_cdr1_headers = zerodds_ast_layout::PlCdr1HeaderPolicy::Specification;
    assert_eq!(
        key_hash_form_for(&ast, root, key).expect("padded"),
        KeyHashForm::Md5,
        "header4 + octet1 + policy pad3 + tail11 =19"
    );
}
