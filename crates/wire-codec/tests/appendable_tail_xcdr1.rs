// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! XTypes 1.3 §7.2.4.2 under encoding version 1: an `@appendable` type
//! "may have additional members appended at the end", and a reader with
//! fewer members ignores them. XCDR1 has no DHEADER, so for the
//! top-level type the sample's own extent delimits the writer's members
//! — and only there: a nested `@appendable` type has the enclosing
//! type's bytes behind it, and a `@final` type has no evolution at all.
//!
//! Measured on the OMG `dds-xtypes` suite before this existed:
//! `ext_appendable_struct_3` under `-x 1` (a two-member `@appendable`
//! writer, a one-member reader of the same name) answered
//! "it did not decode" on every sample.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast::{Ast, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{decode, encode};

fn aggregate(
    ast: &mut Ast,
    name: &str,
    extensibility: Extensibility,
    members: &[(&str, Ref<TypeNode>)],
) -> Id<TypeNode> {
    let root = ast.root_module();
    let id = ast
        .add_type(
            root,
            name,
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect(name);
    let any = ast.any_of_type(id).expect("any");
    ast.facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .extensibility = Some(extensibility);
    for (member, type_) in members {
        ast.add_member(id, MemberSpec::new(member, type_.clone()))
            .expect(member);
    }
    id
}

/// `Short{x1}`, `Long{x1,x2}` in both extensibilities, and a `@final`
/// `Outer{inner: Short}` / `OuterLong{inner: Long}` pair around the
/// appendable ones.
fn model() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let l = Ref::Resolved(long);
    let short = aggregate(
        &mut ast,
        "Short",
        Extensibility::Appendable,
        &[("x1", l.clone())],
    );
    let long_ = aggregate(
        &mut ast,
        "Long",
        Extensibility::Appendable,
        &[("x1", l.clone()), ("x2", l.clone())],
    );
    aggregate(
        &mut ast,
        "ShortFinal",
        Extensibility::Final,
        &[("x1", l.clone())],
    );
    aggregate(
        &mut ast,
        "LongFinal",
        Extensibility::Final,
        &[("x1", l.clone()), ("x2", l.clone())],
    );
    aggregate(
        &mut ast,
        "Outer",
        Extensibility::Final,
        &[("inner", Ref::Resolved(short)), ("tail", l.clone())],
    );
    aggregate(
        &mut ast,
        "OuterLong",
        Extensibility::Final,
        &[("inner", Ref::Resolved(long_)), ("tail", l)],
    );
    ast
}

fn request(type_name: &str, encoding: Encoding) -> ProbeRequest {
    ProbeRequest::new(type_name, encoding, Endianness::Little)
}

fn read_as(
    ast: &Ast,
    written_as: &str,
    read_as: &str,
    encoding: Encoding,
    sample: &Value,
) -> Result<Value, String> {
    let bytes = encode(ast, &request(written_as, encoding), sample).map_err(|e| e.to_string())?;
    decode(ast, &request(read_as, encoding), &bytes).map_err(|e| e.to_string())
}

/// The evolved writer's appended member is ignored by the top-level
/// reader under XCDR1, as it is under XCDR2 through the DHEADER.
#[test]
fn a_top_level_appendable_reader_ignores_the_writers_appended_members() {
    let ast = model();
    let sample = json!({"x1": 1, "x2": 2});
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let got = read_as(&ast, "Long", "Short", encoding, &sample)
            .unwrap_or_else(|e| panic!("{encoding:?}: {e}"));
        assert_eq!(got, json!({"x1": 1}), "{encoding:?}");
    }
}

/// `@final` has no evolution: the same two octets more are a mismatch.
#[test]
fn a_final_reader_still_refuses_octets_it_did_not_read() {
    let ast = model();
    let sample = json!({"x1": 1, "x2": 2});
    let err = read_as(
        &ast,
        "LongFinal",
        "ShortFinal",
        Encoding::PlainCdr1,
        &sample,
    )
    .expect_err("a @final type with a tail must not decode");
    assert!(err.contains("left at offset"), "{err}");
}

/// A nested `@appendable` type under XCDR1 has no extent of its own: the
/// enclosing type's members follow, and the read fails on them rather
/// than guessing where the nested type ended.
#[test]
fn a_nested_appendable_type_keeps_the_refusal() {
    let ast = model();
    let sample = json!({"inner": {"x1": 1, "x2": 2}, "tail": 42});
    let result = read_as(&ast, "OuterLong", "Outer", Encoding::PlainCdr1, &sample);
    assert!(
        result.as_ref().is_err()
            || result.as_ref().ok() != Some(&json!({"inner": {"x1": 1}, "tail": 42})),
        "the nested case must not come back looking right: {result:?}"
    );
}
