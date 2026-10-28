// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The flags of Annex B (S. 280-281) in the objects `from_ast` builds: which
//! flag a member of each kind may carry, what the minimal masks keep, and
//! what the type flags of each kind are.
//!
//! ```text
//! typedef MemberFlag CollectionElementFlag;   // T1, T2, X
//! typedef MemberFlag StructMemberFlag;        // T1, T2, O, M, K, X
//! typedef MemberFlag UnionMemberFlag;         // T1, T2, D, X
//! typedef MemberFlag UnionDiscriminatorFlag;  // T1, T2, K
//! typedef MemberFlag EnumeratedLiteralFlag;   // D
//! typedef MemberFlag AnnotationParameterFlag; // Unused. No flags apply
//! typedef MemberFlag AliasMemberFlag;         // Unused. No flags apply
//! typedef MemberFlag BitflagFlag;             // Unused. No flags apply
//! typedef MemberFlag BitsetMemberFlag;        // Unused. No flags apply
//! ```
//!
//! Two places differ from the list on purpose, each measured against the
//! vendors or required by another clause (`docs/omg-xtypes-issues.md` §B2,
//! §B8): the union discriminator carries `M`, because §7.2.2.4.4.4.6
//! requires its must-understand attribute to be always true and Cyclone DDS
//! and Fast DDS write `0x11` (`M` and `T1`); an enumeration writes its
//! extensibility in `enum_flags`, which RTI Connext, OpenDDS and Cyclone DDS
//! do and Table 18 needs.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_types::from_ast::{complete_type_object, minimal_type_object};
use zerodds_types::type_identifier::TypeIdentifier;
use zerodds_types::type_object::flags::{
    EnumTypeFlag, StructMemberFlag, UnionDiscriminatorFlag, UnionMemberFlag,
};
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject};

const T1: u16 = 1 << 0;
const T2: u16 = 1 << 1;
const X: u16 = 1 << 2;
const O: u16 = 1 << 3;
const M: u16 = 1 << 4;
const K: u16 = 1 << 5;
const D: u16 = 1 << 6;

const IDL: &str = r#"
@mutable
struct S {
    @key long id;
    @optional long o;
    @must_understand long mu;
    @external long x;
    @try_construct(TRIM) long tc;
    long plain;
    sequence<long, 4> seq;
    long arr[3];
    map<long, long, 5> dict;
};

@mutable
union U switch (long) {
    case 1: long a;
    case 2: @external long ex;
    default: octet b;
};

union KeyedU switch (@key long) {
    case 1: long a;
};

enum E { A, B, C };
enum Marked { P, @default_literal Q, R };
@final enum Fin { F1, F2 };
@appendable enum App { A1, A2 };
bitmask Bits { F0, F1 };
bitset Bs { bitfield<3> x; };
typedef long Alias;
"#;

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "flags.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_id(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
        name.to_string(),
    ]))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

fn complete(ast: &Ast, name: &str) -> CompleteTypeObject {
    complete_type_object(ast, type_id(ast, name)).expect("the object builds")
}

fn element_flags(identifier: &TypeIdentifier) -> u16 {
    match identifier {
        TypeIdentifier::PlainSequenceSmall { header, .. }
        | TypeIdentifier::PlainSequenceLarge { header, .. }
        | TypeIdentifier::PlainArraySmall { header, .. }
        | TypeIdentifier::PlainArrayLarge { header, .. } => header.element_flags.0,
        other => panic!("not a plain collection: {other:?}"),
    }
}

/// Annex B: `StructMemberFlag` is T1, T2, O, M, K, X: none of the other flags
/// (`D`) is set, and the flags a member declares are set.
#[test]
fn a_struct_member_carries_t1_t2_o_m_k_x_and_nothing_else() {
    let ast = model();
    let CompleteTypeObject::Struct(s) = complete(&ast, "S") else {
        panic!()
    };
    let flags = |name: &str| {
        s.member_seq
            .iter()
            .find(|m| m.detail.name == name)
            .unwrap_or_else(|| panic!("member {name}"))
            .common
            .member_flags
            .0
    };
    for member in &s.member_seq {
        assert_eq!(
            member.common.member_flags.0 & !(T1 | T2 | O | M | K | X),
            0,
            "{}: a flag Annex B does not list for a struct member",
            member.detail.name
        );
    }
    assert_ne!(flags("id") & K, 0);
    assert_ne!(
        flags("id") & M,
        0,
        "§7.2.2.4.4.4.8: a key member is always must-understand"
    );
    assert_ne!(flags("o") & O, 0);
    assert_ne!(flags("mu") & M, 0);
    assert_ne!(flags("x") & X, 0);
    // TRIM is T1 and T2: 11.
    assert_eq!(flags("tc") & (T1 | T2), T1 | T2);
    assert_eq!(flags("plain") & (O | M | K | X), 0);
    assert_eq!(StructMemberFlag::IS_KEY, K);
}

/// Annex B: `UnionMemberFlag` is T1, T2, D, X; `UnionDiscriminatorFlag` is
/// T1, T2, K, and carries `M` as well (see the header of this file).
#[test]
fn union_members_and_the_discriminator_carry_what_annex_b_lists() {
    let ast = model();
    let CompleteTypeObject::Union(u) = complete(&ast, "U") else {
        panic!()
    };
    for member in &u.member_seq {
        assert_eq!(
            member.common.member_flags.0 & !(T1 | T2 | D | X),
            0,
            "{}: a flag Annex B does not list for a union member",
            member.detail.name
        );
    }
    let by_name = |n: &str| {
        u.member_seq
            .iter()
            .find(|m| m.detail.name == n)
            .unwrap_or_else(|| panic!("{n}"))
            .common
            .member_flags
            .0
    };
    assert_ne!(by_name("b") & D, 0, "the default branch carries D");
    assert_eq!(by_name("a") & D, 0);
    assert_ne!(by_name("ex") & X, 0);

    let discriminator = u.discriminator.common.member_flags.0;
    assert_eq!(
        discriminator & !(T1 | T2 | K | M),
        0,
        "the discriminator carries nothing beyond T1, T2, K and M"
    );
    assert_eq!(discriminator & K, 0, "an unkeyed discriminator has no K");
    assert_ne!(
        discriminator & M,
        0,
        "§7.2.2.4.4.4.6: always must-understand"
    );
    assert_eq!(
        discriminator,
        UnionDiscriminatorFlag::discriminator_default().0,
        "the vendors' 0x11"
    );
    let CompleteTypeObject::Union(keyed) = complete(&ast, "KeyedU") else {
        panic!()
    };
    assert_ne!(
        keyed.discriminator.common.member_flags.0 & K,
        0,
        "@key on the discriminator"
    );
}

/// Annex B: `EnumeratedLiteralFlag` is D and nothing else; the flags of an
/// alias member, a bitflag and a bitfield are "unused. No flags apply".
#[test]
fn an_enumerated_literal_carries_d_and_the_unused_flags_are_zero() {
    let ast = model();
    let CompleteTypeObject::Enumerated(marked) = complete(&ast, "Marked") else {
        panic!()
    };
    for literal in &marked.literal_seq {
        assert_eq!(literal.common.flags.0 & !D, 0, "{}", literal.detail.name);
    }
    let default: Vec<&str> = marked
        .literal_seq
        .iter()
        .filter(|l| l.common.flags.0 & D != 0)
        .map(|l| l.detail.name.as_str())
        .collect();
    assert_eq!(default, ["Q"], "the literal marked @default_literal");

    let CompleteTypeObject::Bitmask(bits) = complete(&ast, "Bits") else {
        panic!()
    };
    assert!(bits.flag_seq.iter().all(|f| f.common.flags.0 == 0));
    let CompleteTypeObject::Bitset(bs) = complete(&ast, "Bs") else {
        panic!()
    };
    assert!(bs.field_seq.iter().all(|f| f.common.flags.0 == 0));
    let CompleteTypeObject::Alias(alias) = complete(&ast, "Alias") else {
        panic!()
    };
    assert_eq!(alias.body.related_flags.0, 0);
    assert_eq!(alias.alias_flags.0, 0);
}

/// Annex B: `CollectionElementFlag` is T1, T2, X.
#[test]
fn a_collection_element_carries_t1_t2_x() {
    let ast = model();
    let CompleteTypeObject::Struct(s) = complete(&ast, "S") else {
        panic!()
    };
    for name in ["seq", "arr"] {
        let member = s
            .member_seq
            .iter()
            .find(|m| m.detail.name == name)
            .expect(name);
        let flags = element_flags(&member.common.member_type_id);
        assert_eq!(flags & !(T1 | T2 | X), 0, "{name}");
    }
}

/// The minimal masks: `TypeFlagMinimalMask` 0x0007 keeps F, A, M and drops
/// N and H; `MemberFlagMinimalMask` keeps T1, T2, X, O, M, K; a union member
/// and an enumerated literal keep `D` (the comment of Annex B selects it, the
/// value 0x003f does not, and the minimal union has no label for the default
/// branch other than this flag).
#[test]
fn the_minimal_object_keeps_the_flags_that_decide_assignability() {
    let ast = model();
    let MinimalTypeObject::Struct(s) = minimal_type_object(&ast, type_id(&ast, "S")).expect("m")
    else {
        panic!()
    };
    assert_eq!(
        s.struct_flags.0 & !0x0007,
        0,
        "no N and no H in a minimal struct"
    );
    for member in &s.member_seq {
        assert_eq!(member.common.member_flags.0 & !0x003f, 0);
    }
    let MinimalTypeObject::Union(u) = minimal_type_object(&ast, type_id(&ast, "U")).expect("m")
    else {
        panic!()
    };
    assert_eq!(u.union_flags.0 & !0x0007, 0);
    let defaults = u
        .member_seq
        .iter()
        .filter(|m| m.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0)
        .count();
    assert_eq!(defaults, 1, "the default branch is still marked");
    let MinimalTypeObject::Enumerated(e) =
        minimal_type_object(&ast, type_id(&ast, "Marked")).expect("m")
    else {
        panic!()
    };
    let default: Vec<usize> = e
        .literal_seq
        .iter()
        .enumerate()
        .filter(|(_, l)| l.common.flags.0 & D != 0)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(default, [1]);
}

/// Annex B declares `EnumTypeFlag` unused; Table 18 compares the
/// extensibility of two enumerations and RTI Connext, OpenDDS and Cyclone DDS
/// write it into `enum_flags` (`docs/omg-xtypes-issues.md` §B8), as this does.
/// The other type kinds of the list that declare "Unused. No flags apply"
/// have zero flags.
#[test]
fn an_enumeration_writes_its_extensibility_and_the_other_unused_type_flags_are_zero() {
    let ast = model();
    let flags_of = |name: &str| match complete(&ast, name) {
        CompleteTypeObject::Enumerated(e) => e.enum_flags.0,
        other => panic!("{other:?}"),
    };
    assert_ne!(flags_of("Fin") & EnumTypeFlag::IS_FINAL, 0);
    assert_ne!(flags_of("App") & EnumTypeFlag::IS_APPENDABLE, 0);
    assert_ne!(
        flags_of("E") & EnumTypeFlag::IS_APPENDABLE,
        0,
        "the default of an enumeration"
    );
    let CompleteTypeObject::Bitmask(bits) = complete(&ast, "Bits") else {
        panic!()
    };
    assert_eq!(bits.bitmask_flags.0, 0);
    let CompleteTypeObject::Bitset(bs) = complete(&ast, "Bs") else {
        panic!()
    };
    assert_eq!(bs.bitset_flags.0, 0);
}
