// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-ast-layout`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-ast-layout` (`src/model.rs`): the
//! answers it copies into the serializer's plans — `xcdr::plan` under
//! `LayoutKey::data(encoding, Endianness::Little)` with a `DepCollector`
//! (`TypeLayout::needs_dheader`, `needs_sentinel`, `members` in declaration
//! order with `member`, `member_id` and `header`, every `MemberHeader`
//! variant); `collection_is_delimited`, `array_is_delimited`,
//! `sequence_element_carries_length`; `enumeration_bit_bound` under the
//! key's `enum_holder` and `enumerator_wire_value` under its
//! `enumerator_value`; `keyhash::key_members_with` under the key's
//! `key_holder_nesting` and `key_holder_order`, `keyhash::key_hash_form_for`
//! and `KeyHolderAlignment::max_alignment` for the key holder.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.2 (PL_CDR1 parameter list and its
//! sentinel), §7.4.2 and §7.4.3.4.2 (DHEADER, EMHEADER, the M flag of a
//! key member), §7.4.3.5.3 rules (9), (12), (18), (20); §7.6.8 (key holder,
//! PLAIN_CDR2 alignment, the sixteen-octet branch); IDL 4.2 enumerator
//! values with `@value` and `@bit_bound`.

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::xcdr::DepCollector;
use zerodds_ast_layout::{Encoding, Endianness, KeyHashForm, LayoutKey, MemberHeader, Purpose};

const IDL: &str = "\
module c {
    @appendable struct Part { long x; };
    @mutable struct M { @key long k; @optional string s; sequence<c::Part> parts;
        sequence<long> ls; c::Part arr[2]; };
    @final struct O { @optional long a; long b; };
    @bit_bound(8) enum Level { LOW, @value(5) MID, HIGH };
    @final struct F { @key unsigned long id; long v; };
    @final struct S { @key string name; };
};";

fn compose() -> Ast {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        IDL,
        &zerodds_idl_compose::PathLoader::bare(),
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
    composed.ast
}

fn type_id(ast: &Ast, name: &str) -> Id<TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type `{name}`"))
}

fn key(encoding: Encoding) -> LayoutKey {
    LayoutKey::data(encoding, Endianness::Little)
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_type_under_xcdr2_has_a_dheader_and_emheaders() {
    let ast = compose();
    let m = type_id(&ast, "M");
    let layout = zerodds_ast_layout::xcdr::plan(
        &ast,
        m,
        key(Encoding::PlCdr2),
        &mut DepCollector::default(),
    )
    .unwrap();
    assert!(layout.needs_dheader);
    assert!(!layout.needs_sentinel);
    let declared: Vec<_> = ast
        .members_of(m)
        .iter()
        .map(|member| ast.member(*member).unwrap().node_id)
        .collect();
    let planned: Vec<_> = layout.members.iter().map(|plan| plan.member).collect();
    assert_eq!(planned, declared, "the plan keeps the declaration order");
    let ids: Vec<u32> = layout.members.iter().map(|plan| plan.member_id).collect();
    assert_eq!(ids, [0, 1, 2, 3, 4]);
    for (index, plan) in layout.members.iter().enumerate() {
        match plan.header {
            MemberHeader::EmHeader {
                must_understand, ..
            } => {
                assert_eq!(
                    must_understand,
                    index == 0,
                    "the M flag is the key member's"
                );
            }
            other => panic!("an EMHEADER, got {other:?}"),
        }
    }
}

#[test]
fn xtypes_7_4_1_2_a_mutable_type_under_xcdr1_is_a_parameter_list() {
    let ast = compose();
    let layout = zerodds_ast_layout::xcdr::plan(
        &ast,
        type_id(&ast, "M"),
        key(Encoding::PlCdr1),
        &mut DepCollector::default(),
    )
    .unwrap();
    assert!(layout.needs_sentinel);
    assert!(!layout.needs_dheader);
    assert!(
        layout
            .members
            .iter()
            .all(|plan| matches!(plan.header, MemberHeader::PidCdr1 { .. })),
        "{:?}",
        layout.members
    );
}

#[test]
fn xtypes_7_4_3_5_3_rules_18_and_20_an_optional_member_of_a_final_type() {
    let ast = compose();
    let o = type_id(&ast, "O");
    let v2 = zerodds_ast_layout::xcdr::plan(
        &ast,
        o,
        key(Encoding::PlainCdr2),
        &mut DepCollector::default(),
    )
    .unwrap();
    assert!(!v2.needs_dheader);
    assert_eq!(v2.members[0].header, MemberHeader::OptionalFlag);
    assert_eq!(v2.members[1].header, MemberHeader::NoHeader);
    let v1 = zerodds_ast_layout::xcdr::plan(
        &ast,
        o,
        key(Encoding::PlainCdr1),
        &mut DepCollector::default(),
    )
    .unwrap();
    assert!(matches!(
        v1.members[0].header,
        MemberHeader::OptionalCdr1 { .. }
    ));
    assert_eq!(v1.members[1].header, MemberHeader::NoHeader);
}

#[test]
fn xtypes_7_4_3_5_3_rules_9_and_12_collections_of_non_primitives_are_delimited() {
    let ast = compose();
    let inner = type_id(&ast, "Part");
    let long = ast
        .members_of(type_id(&ast, "F"))
        .get(1)
        .and_then(|member| ast.member(*member).unwrap().type_.resolved())
        .unwrap();
    assert!(zerodds_ast_layout::collection_is_delimited(
        &ast,
        inner,
        key(Encoding::PlainCdr2)
    ));
    assert!(!zerodds_ast_layout::collection_is_delimited(
        &ast,
        long,
        key(Encoding::PlainCdr2)
    ));
    assert!(!zerodds_ast_layout::collection_is_delimited(
        &ast,
        inner,
        key(Encoding::PlainCdr1)
    ));
    assert!(zerodds_ast_layout::array_is_delimited(
        &ast,
        inner,
        key(Encoding::PlainCdr2)
    ));
    assert_eq!(
        zerodds_ast_layout::sequence_element_carries_length(&ast, long, key(Encoding::PlainCdr2)),
        Some(false)
    );
}

#[test]
fn idl_4_2_an_enumeration_holder_and_its_values() {
    let ast = compose();
    let e = type_id(&ast, "Level");
    let key = key(Encoding::PlainCdr2);
    assert_eq!(
        zerodds_ast_layout::enumeration_bit_bound(&ast, e, key.enum_holder),
        8
    );
    let TypeKind::Enumeration { enumerators } = &ast.type_(e).unwrap().kind else {
        panic!("an enumeration");
    };
    let values: Vec<_> = enumerators
        .iter()
        .map(|id| zerodds_ast_layout::enumerator_wire_value(&ast, *id, key.enumerator_value))
        .collect();
    assert_eq!(values, [Some(0), Some(5), Some(6)]);
}

#[test]
fn xtypes_7_6_8_the_key_holder_members_alignment_and_hash_form() {
    let ast = compose();
    let f = type_id(&ast, "F");
    let data = key(Encoding::PlainCdr2);
    let holder = LayoutKey {
        purpose: Purpose::Key,
        endianness: Endianness::Big,
        ..data
    };
    let keys = zerodds_ast_layout::keyhash::key_members_with(
        &ast,
        f,
        holder.key_holder_nesting,
        holder.key_holder_order,
    )
    .unwrap();
    let id_member = ast.member(ast.members_of(f)[0]).unwrap().node_id;
    assert_eq!(keys, [id_member]);
    assert_eq!(
        holder.key_holder_alignment.max_alignment(holder.encoding),
        4
    );
    assert_eq!(
        zerodds_ast_layout::keyhash::key_hash_form_for(&ast, f, holder).unwrap(),
        KeyHashForm::ZeroPadded
    );
    assert_eq!(
        zerodds_ast_layout::keyhash::key_hash_form_for(&ast, type_id(&ast, "S"), holder).unwrap(),
        KeyHashForm::Md5
    );
}
