// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Strong assignability (DDS-XTypes 1.3 §7.2.4.3, delimited types
//! §7.2.4.2) and the aggregated-type rows of Table 19 (§7.2.4.4.8) that
//! read the `optional`, `must_understand` and key attributes.
//!
//! Every expected value is the one the clause gives:
//!
//! - Table 17: the element type of an array, sequence and map (and the key
//!   type of a map) is *strongly* assignable.
//! - Table 19, `@appendable` and `@final`: members of the same index have
//!   "the same setting for the 'optional' attribute" and the member type
//!   is strongly assignable.
//! - Table 19: members "for which both optional is false and
//!   must_understand is true" appear in both types.
//! - Table 19: member types are compared as `KeyErased` types.
//! - Table 19: key members of structure and union type are compared as
//!   `KeyHolder` types.
//! - Table 18: `T1.extensibility == T2.extensibility` for enumerations.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_types::assignability::{AssignabilityConfig, StrongAssignability, is_assignable};
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::PlainCollectionHeader;
use zerodds_types::type_identifier::kinds::EK_MINIMAL;
use zerodds_types::type_object::flags::{EnumTypeFlag, StructTypeFlag};
use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier, compute_minimal_hash};

fn register(reg: &mut TypeRegistry, object: MinimalTypeObject) -> TypeIdentifier {
    let hash = compute_minimal_hash(&object).unwrap();
    reg.insert_minimal(hash, object);
    TypeIdentifier::EquivalenceHashMinimal(hash)
}

fn i32t() -> TypeIdentifier {
    TypeIdentifier::Primitive(PrimitiveKind::Int32)
}

fn string8(bound: u8) -> TypeIdentifier {
    TypeIdentifier::String8Small { bound }
}

fn sequence_of(element: TypeIdentifier) -> TypeIdentifier {
    TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader::for_element(EK_MINIMAL),
        bound: 0,
        element: Box::new(element),
    }
}

fn yes(w: &TypeIdentifier, r: &TypeIdentifier, reg: &TypeRegistry, cfg: &AssignabilityConfig) {
    let a = is_assignable(w, r, reg, cfg);
    assert!(a.is_yes(), "expected assignable, got {a:?}");
}

fn no(w: &TypeIdentifier, r: &TypeIdentifier, reg: &TypeRegistry, cfg: &AssignabilityConfig) {
    let a = is_assignable(w, r, reg, cfg);
    assert!(!a.is_yes(), "expected not assignable, got {a:?}");
}

/// Two structures of the given extensibility that are assignable to each
/// other and not equivalent: the string member differs in its bound.
fn bound_pair(reg: &mut TypeRegistry, ext: Extensibility) -> (TypeIdentifier, TypeIdentifier) {
    let build = |bound: u8| {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::Elem")
                .extensibility(ext)
                .member("s", string8(bound), |m| m)
                .build_minimal(),
        )
    };
    (register(reg, build(8)), register(reg, build(4)))
}

/// Table 17, SEQUENCE_TYPE: the element is strongly assignable. A `@final`
/// structure is not delimited, so two such elements that are assignable
/// without being equivalent do not make their sequences assignable.
#[test]
fn a_sequence_of_final_structs_needs_equivalent_elements() {
    let mut reg = TypeRegistry::new();
    let (w, r) = bound_pair(&mut reg, Extensibility::Final);
    let cfg = AssignabilityConfig::default();
    yes(&w, &r, &reg, &cfg); // the elements alone are assignable
    no(&sequence_of(w.clone()), &sequence_of(r.clone()), &reg, &cfg);
    // The same element on both sides is equivalent.
    yes(&sequence_of(w.clone()), &sequence_of(w), &reg, &cfg);
}

/// §7.2.4.2: an `@appendable` type is delimited with encoding version 2
/// and not with version 1; a `@mutable` one in both.
#[test]
fn appendable_elements_are_delimited_under_xcdr2_only() {
    let mut reg = TypeRegistry::new();
    let (w, r) = bound_pair(&mut reg, Extensibility::Appendable);
    let xcdr2 = AssignabilityConfig::default();
    let xcdr1 = AssignabilityConfig {
        appendable_is_delimited: false,
        ..AssignabilityConfig::default()
    };
    yes(
        &sequence_of(w.clone()),
        &sequence_of(r.clone()),
        &reg,
        &xcdr2,
    );
    no(&sequence_of(w), &sequence_of(r), &reg, &xcdr1);

    let mut reg = TypeRegistry::new();
    let (w, r) = bound_pair(&mut reg, Extensibility::Mutable);
    yes(
        &sequence_of(w.clone()),
        &sequence_of(r.clone()),
        &reg,
        &xcdr1,
    );
    yes(&sequence_of(w), &sequence_of(r), &reg, &xcdr2);
}

/// Collections of delimited elements are delimited (§7.2.4.2), so a
/// sequence of sequences of strings needs nothing beyond assignability.
#[test]
fn a_collection_of_delimited_elements_is_delimited() {
    let reg = TypeRegistry::new();
    let cfg = AssignabilityConfig::default();
    yes(
        &sequence_of(sequence_of(string8(8))),
        &sequence_of(sequence_of(string8(4))),
        &reg,
        &cfg,
    );
}

/// Table 17, ARRAY_TYPE and MAP_TYPE.
#[test]
fn array_elements_and_map_keys_and_values_are_strongly_assignable() {
    let mut reg = TypeRegistry::new();
    let (w, r) = bound_pair(&mut reg, Extensibility::Final);
    let cfg = AssignabilityConfig::default();
    let array_of = |element: TypeIdentifier| TypeIdentifier::PlainArraySmall {
        header: PlainCollectionHeader::for_element(EK_MINIMAL),
        array_bounds: vec![3],
        element: Box::new(element),
    };
    no(&array_of(w.clone()), &array_of(r.clone()), &reg, &cfg);
    let map_of = |key: TypeIdentifier, element: TypeIdentifier| TypeIdentifier::PlainMapSmall {
        header: PlainCollectionHeader::for_element(EK_MINIMAL),
        bound: 0,
        element: Box::new(element),
        key_flags: zerodds_types::type_identifier::CollectionElementFlag::discard(),
        key: Box::new(key),
    };
    no(
        &map_of(i32t(), w.clone()),
        &map_of(i32t(), r.clone()),
        &reg,
        &cfg,
    );
    yes(&map_of(i32t(), w.clone()), &map_of(i32t(), w), &reg, &cfg);
}

/// Table 19, `@appendable` and `@final`: the member type of the same index
/// is strongly assignable. A `@mutable` outer type asks no more than
/// assignability.
#[test]
fn members_of_appendable_and_final_types_are_strongly_assignable() {
    let mut reg = TypeRegistry::new();
    let (w_inner, r_inner) = bound_pair(&mut reg, Extensibility::Final);
    let cfg = AssignabilityConfig::default();
    for (ext, expected) in [
        (Extensibility::Appendable, false),
        (Extensibility::Final, false),
        (Extensibility::Mutable, true),
    ] {
        let outer = |inner: &TypeIdentifier| {
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::Outer")
                    .extensibility(ext)
                    .member("m", inner.clone(), |m| m)
                    .build_minimal(),
            )
        };
        let w = register(&mut reg, outer(&w_inner));
        let r = register(&mut reg, outer(&r_inner));
        if expected {
            yes(&w, &r, &reg, &cfg);
        } else {
            no(&w, &r, &reg, &cfg);
        }
    }
}

/// Table 19, `@appendable` and `@final`: members of the same index have
/// "the same setting for the 'optional' attribute".
#[test]
fn members_of_the_same_index_agree_on_optional() {
    let cfg = AssignabilityConfig::default();
    for ext in [Extensibility::Appendable, Extensibility::Final] {
        let mut reg = TypeRegistry::new();
        let build = |optional: bool| {
            MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::S")
                    .extensibility(ext)
                    .member("a", i32t(), |m| if optional { m.optional() } else { m })
                    .build_minimal(),
            )
        };
        let plain = register(&mut reg, build(false));
        let optional = register(&mut reg, build(true));
        no(&plain, &optional, &reg, &cfg);
        no(&optional, &plain, &reg, &cfg);
        yes(&optional, &optional, &reg, &cfg);
    }
}

/// Table 19: "Members for which both optional is false and
/// must_understand is true in either T1 or T2 appear […] in both". A
/// member that is `@optional` is not one of them, however it is marked.
#[test]
fn an_optional_must_understand_member_may_be_missing() {
    let cfg = AssignabilityConfig::default();
    let mut reg = TypeRegistry::new();
    let with_extra = |optional: bool| {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::S")
                .extensibility(Extensibility::Mutable)
                .member("a", i32t(), |m| m.id(1))
                .member("b", i32t(), |m| {
                    let m = m.id(2).must_understand();
                    if optional { m.optional() } else { m }
                })
                .build_minimal(),
        )
    };
    let without = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::S")
            .extensibility(Extensibility::Mutable)
            .member("a", i32t(), |m| m.id(1))
            .build_minimal(),
    );
    let bare = register(&mut reg, without);
    let must = register(&mut reg, with_extra(false));
    let optional = register(&mut reg, with_extra(true));
    // Not optional: missing on either side is refused.
    no(&must, &bare, &reg, &cfg);
    no(&bare, &must, &reg, &cfg);
    // Optional: missing on either side is fine.
    yes(&optional, &bare, &reg, &cfg);
    yes(&bare, &optional, &reg, &cfg);
}

fn keyed_inner(key_bound: u8, ext: Extensibility) -> MinimalTypeObject {
    MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::Inner")
            .extensibility(ext)
            .member("k", string8(key_bound), |m| m.key())
            .member("v", i32t(), |m| m)
            .build_minimal(),
    )
}

/// Table 19: the member types are compared as `KeyErased` types. A
/// writer whose nested type has a longer key member than the reader's is
/// not refused when that nested type is the type of an ordinary member:
/// the key designation of the member's type is erased.
#[test]
fn a_nested_types_key_bound_does_not_count_for_a_non_key_member() {
    let cfg = AssignabilityConfig::default();
    let mut reg = TypeRegistry::new();
    let w_inner = register(&mut reg, keyed_inner(16, Extensibility::Appendable));
    let r_inner = register(&mut reg, keyed_inner(8, Extensibility::Appendable));
    // The control: as top-level types the key bound is enforced.
    no(&w_inner, &r_inner, &reg, &cfg);

    let outer = |inner: &TypeIdentifier, key: bool| {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::Outer")
                .extensibility(Extensibility::Mutable)
                .member("m", inner.clone(), |m| if key { m.key() } else { m })
                .build_minimal(),
        )
    };
    let w = register(&mut reg, outer(&w_inner, false));
    let r = register(&mut reg, outer(&r_inner, false));
    yes(&w, &r, &reg, &cfg);

    // As a key member the bullet for a structure key member applies:
    // KeyHolder(m1.type) must be assignable from KeyHolder(m2.type), and
    // the holder of `Inner` is its key member `k`.
    let wk = register(&mut reg, outer(&w_inner, true));
    let rk = register(&mut reg, outer(&r_inner, true));
    no(&wk, &rk, &reg, &cfg);
}

/// Table 19, last key bullet: "For every discriminator value of m2.type
/// that selects a member m22 in m2.type, the discriminator value selects a
/// member m11 in m1.type". A writer's union key member with a label the
/// reader's union has no member for is refused, where the ordinary union
/// rule (an `@appendable` union may grow) accepts the pair.
#[test]
fn every_value_of_a_union_key_member_must_select_a_member_in_the_reader() {
    let cfg = AssignabilityConfig::default();
    let mut reg = TypeRegistry::new();
    let u_w = register(
        &mut reg,
        MinimalTypeObject::Union(
            TypeObjectBuilder::union_type("::U", i32t())
                .extensibility(Extensibility::Appendable)
                .case("a", i32t(), vec![1])
                .case("b", i32t(), vec![2])
                .build_minimal(),
        ),
    );
    let u_r = register(
        &mut reg,
        MinimalTypeObject::Union(
            TypeObjectBuilder::union_type("::U", i32t())
                .extensibility(Extensibility::Appendable)
                .case("a", i32t(), vec![1])
                .build_minimal(),
        ),
    );
    let outer = |u: &TypeIdentifier, key: bool| {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::Outer")
                .extensibility(Extensibility::Mutable)
                .member("u", u.clone(), |m| if key { m.key() } else { m })
                .build_minimal(),
        )
    };
    // As an ordinary member the pair is fine …
    let w = register(&mut reg, outer(&u_w, false));
    let r = register(&mut reg, outer(&u_r, false));
    yes(&w, &r, &reg, &cfg);
    // … as a key member the value 2 selects nothing in the reader's union.
    let wk = register(&mut reg, outer(&u_w, true));
    let rk = register(&mut reg, outer(&u_r, true));
    no(&wk, &rk, &reg, &cfg);
    // The other direction has every value of the writer's union covered.
    yes(&rk, &wk, &reg, &cfg);
}

/// Table 18: "T1.extensibility == T2.extensibility". An enumeration whose
/// flag word names no extensibility is APPENDABLE (§7.3.1.2.1.8).
#[test]
fn an_enum_without_an_extensibility_flag_is_appendable() {
    let cfg = AssignabilityConfig::default();
    let mut reg = TypeRegistry::new();
    let enum_with = |flags: u16| {
        let mut e = TypeObjectBuilder::enum_type("::E")
            .bit_bound(32)
            .literal("A", 0)
            .literal("B", 1)
            .build_minimal();
        e.enum_flags = EnumTypeFlag(flags);
        MinimalTypeObject::Enumerated(e)
    };
    let zero = register(&mut reg, enum_with(0));
    let appendable = register(&mut reg, enum_with(StructTypeFlag::IS_APPENDABLE));
    let final_ = register(&mut reg, enum_with(StructTypeFlag::IS_FINAL));
    yes(&zero, &appendable, &reg, &cfg);
    yes(&appendable, &zero, &reg, &cfg);
    no(&zero, &final_, &reg, &cfg);
    no(&final_, &zero, &reg, &cfg);
    yes(&final_, &final_, &reg, &cfg);
}

/// `StrongAssignability::NamesIgnored`, the axis for the OMG dds-xtypes
/// cases `SFinal[10]_S[20]_SFinalAlt[10]_S[20]` and
/// `seq(SFinal,10)_seq(SFinalAlt,10)`: an array and a sequence of `@final`
/// structures that differ only in the name of their member are assignable
/// under the named setting and not under the default (§7.2.4.3: not
/// delimited, and the MINIMAL TypeObjects differ in the member's
/// `name_hash`). Both configurations ignore member names in plain
/// assignability, as the readers of the suite's client do
/// (`TypeConsistencyEnforcement::ignore_member_names` TRUE, the default of
/// the suite's `test_main.cxx`). A different member type or array bound keeps
/// them apart under both.
#[test]
fn final_elements_that_differ_only_in_member_names_count_under_the_named_setting() {
    let mut reg = TypeRegistry::new();
    let array = |bound: u8, element: TypeIdentifier| TypeIdentifier::PlainArraySmall {
        header: PlainCollectionHeader::for_element(EK_MINIMAL),
        array_bounds: vec![bound],
        element: Box::new(element),
    };
    let u32t = TypeIdentifier::Primitive(PrimitiveKind::UInt32);
    let build = |name: &str, member: TypeIdentifier| {
        MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::F_S__array20_uint32")
                .extensibility(Extensibility::Final)
                .member(name, member, |m| m)
                .build_minimal(),
        )
    };
    let x1 = register(&mut reg, build("x1", array(20, u32t.clone())));
    let altx1 = register(&mut reg, build("altx1", array(20, u32t.clone())));
    let other_type = register(&mut reg, build("altx1", array(20, i32t())));
    let other_bound = register(&mut reg, build("altx1", array(21, u32t)));
    let spec = AssignabilityConfig {
        ignore_member_names: true,
        ..AssignabilityConfig::default()
    };
    let names_ignored = AssignabilityConfig {
        strong_assignability: StrongAssignability::NamesIgnored,
        ..spec
    };

    no(
        &array(10, x1.clone()),
        &array(10, altx1.clone()),
        &reg,
        &spec,
    );
    yes(
        &array(10, x1.clone()),
        &array(10, altx1.clone()),
        &reg,
        &names_ignored,
    );
    no(
        &sequence_of(x1.clone()),
        &sequence_of(altx1.clone()),
        &reg,
        &spec,
    );
    yes(
        &sequence_of(x1.clone()),
        &sequence_of(altx1),
        &reg,
        &names_ignored,
    );

    no(
        &array(10, x1.clone()),
        &array(10, other_type),
        &reg,
        &names_ignored,
    );
    no(
        &array(10, x1),
        &array(10, other_bound),
        &reg,
        &names_ignored,
    );
}

/// The setting's spelling.
#[test]
fn the_strong_assignability_axis_reads_its_two_values() {
    assert_eq!(
        StrongAssignability::default(),
        StrongAssignability::Equivalence
    );
    for v in [
        StrongAssignability::Equivalence,
        StrongAssignability::NamesIgnored,
    ] {
        assert_eq!(StrongAssignability::parse(v.as_str()), Some(v));
    }
    assert_eq!(StrongAssignability::parse("loose"), None);
}
