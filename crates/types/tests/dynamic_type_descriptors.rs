//! DDS-XTypes 1.3 §7.5.2: the TypeDescriptor and MemberDescriptor of the
//! Dynamic Language Binding (Tables 50 to 55).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_types::dynamic::{
    DynamicType, DynamicTypeBuilder, ExtensibilityKind, LENGTH_UNLIMITED, MEMBER_ID_INVALID,
    NewMember, TypeKind,
};

/// §7.5.2.4.1: an alias descriptor names the aliased type in `base_type`,
/// and has no element type.
#[test]
fn an_alias_names_its_target_as_the_base_type() {
    let mut b = DynamicTypeBuilder::new();
    let long = b.primitive(TypeKind::Int32).unwrap();
    let alias = b.alias("Meters", long).unwrap();
    let ty = DynamicType::new(b.finish(), alias);
    let d = ty.descriptor().unwrap();
    assert_eq!(d.kind, TypeKind::Alias);
    assert_eq!(d.base_type.unwrap().kind(), TypeKind::Int32);
    assert!(d.element_type.is_none());
    assert!(ty.descriptor().unwrap().is_consistent().is_ok());
}

/// §7.5.2.4.5 and §7.5.2.2.5: the element type of a string is Char8, of a
/// wstring Char16.
#[test]
fn a_string_has_a_character_element_type() {
    let mut b = DynamicTypeBuilder::new();
    let narrow = b.string(false, Some(8)).unwrap();
    let wide = b.string(true, None).unwrap();
    let ast = b.finish();
    let narrow = DynamicType::new(ast.clone(), narrow).descriptor().unwrap();
    assert_eq!(narrow.element_type.unwrap().kind(), TypeKind::Char8);
    assert_eq!(narrow.bound, vec![8]);
    let wide = DynamicType::new(ast, wide).descriptor().unwrap();
    assert_eq!(wide.element_type.unwrap().kind(), TypeKind::Char16);
    assert_eq!(wide.bound, vec![LENGTH_UNLIMITED]);
}

/// §7.5.2.4.2 and §7.5.2.4.5: a bitmask's bound is `bound`, its element
/// type is Boolean; Table 51: its flags are members of Boolean type; and
/// §7.5.2.7.5: a member of a type that is not aggregated has
/// MEMBER_ID_INVALID.
#[test]
fn a_bitmask_has_a_bound_a_boolean_element_and_boolean_flags() {
    let mut b = DynamicTypeBuilder::new();
    let mask = b.bitmask("Flags", Some(8)).unwrap();
    b.add_bit(mask, "READ", None).unwrap();
    b.add_bit(mask, "WRITE", Some(3)).unwrap();
    let ty = DynamicType::new(b.finish(), mask);

    let d = ty.descriptor().unwrap();
    assert_eq!(d.bound, vec![8]);
    assert_eq!(d.element_type.unwrap().kind(), TypeKind::Boolean);
    assert!(d.bit_bound.is_none());

    let write = ty.member_by_name("WRITE").unwrap();
    assert_eq!(write.dynamic_type().unwrap().kind(), TypeKind::Boolean);
    assert_eq!(write.id(), MEMBER_ID_INVALID);
    assert_eq!(write.descriptor().unwrap().id, MEMBER_ID_INVALID);
    assert_eq!(write.literal_value(), Some(3));
    // Not found by an id: a bit does not belong to an aggregated type.
    assert!(ty.member_by_id(3).is_none());
    assert!(ty.member_by_id(MEMBER_ID_INVALID).is_none());
}

#[test]
fn an_enumerator_has_no_member_id_and_the_enumeration_as_its_type() {
    let mut b = DynamicTypeBuilder::new();
    let e = b.enumeration("Color", None).unwrap();
    b.add_enumerator(e, "RED", None, false).unwrap();
    b.add_enumerator(e, "GREEN", Some(5), false).unwrap();
    let ty = DynamicType::new(b.finish(), e);
    let green = ty.member_by_name("GREEN").unwrap();
    assert_eq!(green.id(), MEMBER_ID_INVALID);
    assert_eq!(green.literal_value(), Some(5));
    assert_eq!(green.dynamic_type().unwrap().kind(), TypeKind::Enumeration);
    assert!(ty.member_by_id(5).is_none());
}

/// Table 51: an alias has the members of its base type; a bitset, like
/// every kind the table does not list, has none.
#[test]
fn an_alias_has_the_members_of_its_base_and_a_bitset_has_none() {
    let mut b = DynamicTypeBuilder::new();
    let e = b.enumeration("Color", None).unwrap();
    b.add_enumerator(e, "RED", None, false).unwrap();
    let alias = b.alias("Shade", e).unwrap();
    let set = b.bitset("Bits").unwrap();
    b.add_bitfield(set, Some("a"), 3, None).unwrap();
    let ast = b.finish();

    let alias = DynamicType::new(ast.clone(), alias);
    assert_eq!(alias.member_count(), 1);
    let red = alias.member_by_name("RED").unwrap();
    assert_eq!(red.dynamic_type().unwrap().kind(), TypeKind::Enumeration);
    assert_eq!(DynamicType::new(ast, set).member_count(), 0);
}

/// Table 53: `label` is `Int64 [*]`; a label past 32 bits is kept.
#[test]
fn a_union_label_past_32_bits_is_kept() {
    let mut b = DynamicTypeBuilder::new();
    let wide = b.primitive(TypeKind::Int64).unwrap();
    let text = b.string(false, None).unwrap();
    let union = b.union("Wide", wide, ExtensibilityKind::Final).unwrap();
    let label = i64::from(i32::MAX) + 7;
    b.add_case(union, &[label], false, &NewMember::new("a", text))
        .unwrap();
    let ty = DynamicType::new(b.finish(), union);
    assert_eq!(
        ty.member_by_name("a").unwrap().descriptor().unwrap().label,
        vec![label]
    );
}

/// §7.5.2.7.5: a member added with no id gets one more than the maximum.
#[test]
fn a_member_with_no_id_gets_the_maximum_plus_one() {
    let mut b = DynamicTypeBuilder::new();
    let long = b.primitive(TypeKind::Int32).unwrap();
    let s = b.structure("S", ExtensibilityKind::Mutable).unwrap();
    b.add_member(s, &NewMember::new("high", long).with_id(10))
        .unwrap();
    b.add_member(s, &NewMember::new("low", long).with_id(2))
        .unwrap();
    b.add_member(s, &NewMember::new("next", long)).unwrap();
    let ty = DynamicType::new(b.finish(), s);
    assert_eq!(ty.member_by_name("next").unwrap().id(), 11);
}

/// §7.5.2.7.6: a member added at an index existing members occupy shifts
/// them up; an index past the end appends.
#[test]
fn a_member_added_at_an_index_shifts_the_others() {
    let mut b = DynamicTypeBuilder::new();
    let long = b.primitive(TypeKind::Int32).unwrap();
    let s = b.structure("S", ExtensibilityKind::Final).unwrap();
    b.add_member(s, &NewMember::new("a", long)).unwrap();
    b.add_member(s, &NewMember::new("c", long)).unwrap();
    b.add_member_at(s, 1, &NewMember::new("b", long)).unwrap();
    b.add_member_at(s, 99, &NewMember::new("d", long)).unwrap();
    let ty = DynamicType::new(b.finish(), s);
    let names: Vec<String> = ty.members().iter().map(|m| m.name().to_string()).collect();
    assert_eq!(names, ["a", "b", "c", "d"]);
    assert_eq!(ty.member_by_name("b").unwrap().index(), 1);
    assert_eq!(ty.member_by_name("d").unwrap().index(), 3);
}

/// Table 55: adding a member to an alias adds it to the alias's base type.
#[test]
fn a_member_added_to_an_alias_goes_to_the_base_type() {
    let mut b = DynamicTypeBuilder::new();
    let long = b.primitive(TypeKind::Int32).unwrap();
    let s = b.structure("S", ExtensibilityKind::Final).unwrap();
    let alias = b.alias("T", s).unwrap();
    b.add_member(alias, &NewMember::new("a", long)).unwrap();
    let ast = b.finish();
    assert_eq!(DynamicType::new(ast.clone(), s).member_count(), 1);
    assert_eq!(DynamicType::new(ast, alias).member_count(), 1);
}

/// §7.5.2.2.2: a bitmask bound out of range fails. §7.5.2.2.5:
/// LENGTH_UNLIMITED is no bound.
#[test]
fn bounds_are_checked_and_unlimited_is_unbounded() {
    let mut b = DynamicTypeBuilder::new();
    assert!(b.bitmask("Zero", Some(0)).is_err());
    assert!(b.bitmask("TooWide", Some(65)).is_err());
    assert!(b.bitmask("Max", Some(64)).is_ok());
    let unlimited = b.string(false, Some(LENGTH_UNLIMITED)).unwrap();
    let ty = DynamicType::new(b.finish(), unlimited);
    assert_eq!(ty.descriptor().unwrap().bound, vec![LENGTH_UNLIMITED]);
}

fn int(value: i128) -> zerodds_types::dynamic::DynamicData {
    zerodds_types::dynamic::DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, value)
}

fn aggregate(
    members: Vec<(&str, zerodds_types::dynamic::DynamicData)>,
) -> zerodds_types::dynamic::DynamicData {
    zerodds_types::dynamic::DynamicData::Aggregate(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn pair() -> DynamicType {
    let mut b = DynamicTypeBuilder::new();
    let long = b.primitive(TypeKind::Int32).unwrap();
    let s = b.structure("Pair", ExtensibilityKind::Mutable).unwrap();
    b.add_member(s, &NewMember::new("a", long)).unwrap();
    b.add_member(s, &NewMember::new("b", long)).unwrap();
    b.add_member(s, &NewMember::new("c", long).optional())
        .unwrap();
    DynamicType::new(b.finish(), s)
}

/// §7.5.2.11.5: "Members shall be compared without regard to their order."
#[test]
fn the_members_of_an_aggregate_are_compared_without_their_order() {
    let ty = pair();
    let one = aggregate(vec![("a", int(1)), ("b", int(2))]);
    let other = aggregate(vec![("b", int(2)), ("a", int(1))]);
    assert!(ty.values_equal(&one, &other).unwrap());
    assert!(
        !ty.values_equal(&one, &aggregate(vec![("a", int(1)), ("b", int(3))]))
            .unwrap()
    );
}

/// "One of the samples may omit a non-optional member that is present in
/// the other if that member takes its default value in the latter sample."
#[test]
fn a_non_optional_member_left_out_equals_its_default() {
    let ty = pair();
    let left_out = aggregate(vec![("a", int(1))]);
    assert!(
        ty.values_equal(&left_out, &aggregate(vec![("a", int(1)), ("b", int(0))]))
            .unwrap()
    );
    assert!(
        !ty.values_equal(&left_out, &aggregate(vec![("a", int(1)), ("b", int(5))]))
            .unwrap()
    );
}

/// An optional member that is present in one sample and absent in the
/// other is a difference, even where its value is the default.
#[test]
fn an_optional_member_present_and_absent_is_a_difference() {
    let ty = pair();
    let absent = aggregate(vec![("a", int(1)), ("b", int(2))]);
    let present = aggregate(vec![("a", int(1)), ("b", int(2)), ("c", int(0))]);
    assert!(!ty.values_equal(&absent, &present).unwrap());
}
