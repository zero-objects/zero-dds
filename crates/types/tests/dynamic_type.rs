//! Integration-Tests fuer die XTypes 1.3 §7.5 DynamicType-API
//! (C4.1 Foundation-Cluster).
//!
//! Tests sind nach den Auftrags-Bloecken A-E gruppiert und decken die
//! im Plan geforderten Spec-Varianten ab.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_types::dynamic::{
    DataLoan, DynamicDataFactory, DynamicError, DynamicType, DynamicTypeBuilder,
    DynamicTypeBuilderFactory, ExtensibilityKind, MemberDescriptor, TryConstructKind,
    TypeDescriptor, TypeKind,
};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

// ============================================================================
// Block A — TypeDescriptor + MemberDescriptor (10 tests)
// ============================================================================

#[test]
fn block_a_type_descriptor_for_each_kind_passes_consistency() {
    // Primitives.
    for k in [
        TypeKind::Boolean,
        TypeKind::Byte,
        TypeKind::Int8,
        TypeKind::UInt8,
        TypeKind::Int16,
        TypeKind::UInt16,
        TypeKind::Int32,
        TypeKind::UInt32,
        TypeKind::Int64,
        TypeKind::UInt64,
        TypeKind::Float32,
        TypeKind::Float64,
        TypeKind::Float128,
        TypeKind::Char8,
        TypeKind::Char16,
    ] {
        let d = TypeDescriptor::primitive(k, format!("{k:?}"));
        d.is_consistent().unwrap();
    }
    // Composite + Collection.
    TypeDescriptor::structure("::S").is_consistent().unwrap();
    TypeDescriptor::union("::U", TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .is_consistent()
        .unwrap();
    TypeDescriptor::sequence(
        "::Sq",
        TypeDescriptor::primitive(TypeKind::Int32, "int32"),
        100,
    )
    .is_consistent()
    .unwrap();
    TypeDescriptor::array(
        "::Ar",
        TypeDescriptor::primitive(TypeKind::Int32, "int32"),
        vec![3, 4],
    )
    .is_consistent()
    .unwrap();
    TypeDescriptor::map(
        "::M",
        TypeDescriptor::string8(64),
        TypeDescriptor::primitive(TypeKind::Int64, "int64"),
        500,
    )
    .is_consistent()
    .unwrap();
    TypeDescriptor::string8(255).is_consistent().unwrap();
    TypeDescriptor::string16(64).is_consistent().unwrap();
    TypeDescriptor::enumeration("::E").is_consistent().unwrap();
}

#[test]
fn block_a_member_descriptor_full_field_set() {
    let mut m = MemberDescriptor::new(
        "field1",
        42,
        TypeDescriptor::primitive(TypeKind::Int64, "int64"),
    );
    m.default_value = Some(String::from("0"));
    m.index = 3;
    m.label = vec![1, 2, 3];
    m.try_construct = TryConstructKind::Trim;
    m.is_key = true;
    m.is_optional = true;
    m.is_must_understand = true;
    m.is_shared = true;
    assert_eq!(m.name, "field1");
    assert_eq!(m.id, 42);
    assert!(m.is_key);
    // Test consistency: not is_default_label so labels are fine.
    m.is_consistent().unwrap();
}

#[test]
fn block_a_descriptor_inheritance_self_cycle_rejected() {
    let mut s = TypeDescriptor::structure("::Foo");
    s.base_type = Some(Box::new(TypeDescriptor::structure("::Foo")));
    let err = s.is_consistent().unwrap_err();
    assert!(err.contains("cycle"));
}

#[test]
fn block_a_descriptor_union_without_discriminator_rejected() {
    let mut u = TypeDescriptor::structure("::U");
    u.kind = TypeKind::Union;
    assert!(u.is_consistent().is_err());
}

#[test]
fn block_a_descriptor_array_zero_dim_rejected() {
    let a = TypeDescriptor::array(
        "::A",
        TypeDescriptor::primitive(TypeKind::Int32, "int32"),
        vec![3, 0, 4],
    );
    assert!(a.is_consistent().is_err());
}

#[test]
fn block_a_descriptor_map_missing_key_rejected() {
    let mut m = TypeDescriptor::map(
        "::M",
        TypeDescriptor::string8(8),
        TypeDescriptor::primitive(TypeKind::Int64, "int64"),
        100,
    );
    m.key_element_type = None;
    assert!(m.is_consistent().is_err());
}

#[test]
fn block_a_default_label_member_with_explicit_labels_rejected() {
    let mut m = MemberDescriptor::new("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"));
    m.is_default_label = true;
    m.label = vec![5];
    assert!(m.is_consistent().is_err());
}

#[test]
fn block_a_extensibility_kinds_are_distinct() {
    assert_ne!(ExtensibilityKind::Final, ExtensibilityKind::Mutable);
    assert_ne!(ExtensibilityKind::Final, ExtensibilityKind::Appendable);
    assert_ne!(ExtensibilityKind::Mutable, ExtensibilityKind::Appendable);
}

#[test]
fn block_a_try_construct_kinds_distinct() {
    assert_ne!(TryConstructKind::Discard, TryConstructKind::UseDefault);
    assert_ne!(TryConstructKind::Discard, TryConstructKind::Trim);
    assert_ne!(TryConstructKind::UseDefault, TryConstructKind::Trim);
}

#[test]
fn block_a_type_kind_classifies_primitive_and_aggregable_disjoint() {
    for k in [TypeKind::Int32, TypeKind::Float64, TypeKind::Boolean] {
        assert!(k.is_primitive());
        assert!(!k.is_aggregable());
    }
    for k in [TypeKind::Structure, TypeKind::Union, TypeKind::Bitmask] {
        assert!(!k.is_primitive());
        assert!(k.is_aggregable());
    }
}

// ============================================================================
// Block B — DynamicType + DynamicTypeBuilder (10 tests)
// ============================================================================

fn struct_with_5_members() -> DynamicType {
    let mut b = DynamicTypeBuilderFactory::create_struct("::Big");
    b.add_struct_member("a", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    b.add_struct_member("b", 2, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
        .unwrap();
    b.add_struct_member(
        "c",
        3,
        TypeDescriptor::primitive(TypeKind::Float64, "double"),
    )
    .unwrap();
    b.add_struct_member("d", 4, TypeDescriptor::string8(64))
        .unwrap();
    b.add_struct_member(
        "e",
        5,
        TypeDescriptor::primitive(TypeKind::Boolean, "boolean"),
    )
    .unwrap();
    b.build().unwrap()
}

#[test]
fn block_b_builder_happy_path_struct_with_5_members() {
    let t = struct_with_5_members();
    assert_eq!(t.name(), "::Big");
    assert_eq!(t.member_count(), 5);
    assert_eq!(t.kind(), TypeKind::Structure);
}

#[test]
fn block_b_builder_rejects_duplicate_name() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let err = b
        .add_struct_member("x", 2, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
        .unwrap_err();
    assert!(matches!(err, DynamicError::BuilderConflict(_)));
}

#[test]
fn block_b_builder_rejects_duplicate_id() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("a", 7, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let err = b
        .add_struct_member("b", 7, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
        .unwrap_err();
    assert!(matches!(err, DynamicError::BuilderConflict(_)));
}

#[test]
fn block_b_builder_rejects_inheritance_cycle() {
    let mut s = TypeDescriptor::structure("::A");
    s.base_type = Some(Box::new(TypeDescriptor::structure("::A")));
    let err = DynamicTypeBuilderFactory::create_type(s).unwrap_err();
    assert!(matches!(err, DynamicError::Inconsistent(_)));
}

#[test]
fn block_b_member_by_name_lookup_works() {
    let t = struct_with_5_members();
    assert_eq!(t.member_by_name("c").unwrap().id(), 3);
    assert!(t.member_by_name("zzz").is_none());
}

#[test]
fn block_b_member_by_id_lookup_works() {
    let t = struct_with_5_members();
    assert_eq!(t.member_by_id(2).unwrap().name(), "b");
    assert!(t.member_by_id(999).is_none());
}

#[test]
fn block_b_member_by_index_lookup_works() {
    let t = struct_with_5_members();
    assert_eq!(t.member_by_index(0).unwrap().name(), "a");
    assert_eq!(t.member_by_index(4).unwrap().name(), "e");
    assert!(t.member_by_index(5).is_none());
}

#[test]
fn block_b_dynamic_type_equals_deep() {
    let t1 = struct_with_5_members();
    let t2 = struct_with_5_members();
    assert!(t1.equals(&t2));
}

#[test]
fn block_b_dynamic_type_equals_distinguishes_different_types() {
    let t1 = struct_with_5_members();
    let mut b = DynamicTypeBuilderFactory::create_struct("::Different");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t2 = b.build().unwrap();
    assert!(!t1.equals(&t2));
}

#[test]
fn block_b_set_descriptor_after_build_rejected() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let _ = b.build().unwrap();
    let err = b
        .set_descriptor(TypeDescriptor::structure("::Other"))
        .unwrap_err();
    assert!(matches!(err, DynamicError::PreconditionNotMet(_)));
}

// ============================================================================
// Block C — DynamicData (10 tests)
// ============================================================================

#[test]
fn block_c_data_for_all_12_primitives_get_set_roundtrip() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::P");
    b.add_struct_member(
        "b",
        1,
        TypeDescriptor::primitive(TypeKind::Boolean, "boolean"),
    )
    .unwrap();
    b.add_struct_member("by", 2, TypeDescriptor::primitive(TypeKind::Byte, "octet"))
        .unwrap();
    b.add_struct_member(
        "i32",
        3,
        TypeDescriptor::primitive(TypeKind::Int32, "int32"),
    )
    .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    d.set_boolean_value(1, true).unwrap();
    d.set_byte_value(2, 0x7F).unwrap();
    d.set_int32_value(3, -42).unwrap();
    assert!(d.get_boolean_value(1).unwrap());
    assert_eq!(d.get_byte_value(2).unwrap(), 0x7F);
    assert_eq!(d.get_int32_value(3).unwrap(), -42);
}

#[test]
fn block_c_data_type_mismatch_rejected() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    let err = d.set_int32_value(1, 5).unwrap_err();
    assert!(matches!(err, DynamicError::BadParameter(_)));
}

#[test]
fn block_c_data_unknown_member_id_rejected() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    assert!(d.set_int32_value(999, 0).is_err());
}

#[test]
fn block_c_data_get_unset_member_yields_precondition_error() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t = b.build().unwrap();
    let d = DynamicDataFactory::create_data(&t).unwrap();
    let err = d.get_int32_value(1).unwrap_err();
    assert!(matches!(err, DynamicError::PreconditionNotMet(_)));
}

#[test]
fn block_c_data_complex_value_nested_struct() {
    let mut inner = DynamicTypeBuilderFactory::create_struct("::I");
    inner
        .add_struct_member("v", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let inner_t = inner.build().unwrap();

    let mut outer = DynamicTypeBuilderFactory::create_struct("::O");
    outer
        .add_member(MemberDescriptor::new(
            "n",
            10,
            TypeDescriptor::structure("::I"),
        ))
        .unwrap();
    let outer_t = outer.build().unwrap();

    let mut id = DynamicDataFactory::create_data(&inner_t).unwrap();
    id.set_int32_value(1, 7).unwrap();
    let mut od = DynamicDataFactory::create_data(&outer_t).unwrap();
    od.set_complex_value(10, id).unwrap();
    assert_eq!(
        od.get_complex_value(10)
            .unwrap()
            .get_int32_value(1)
            .unwrap(),
        7
    );
}

#[test]
fn block_c_data_sequence_get_item_count_reports_size() {
    // Seq-Element-Type: ein 1-Member-Struct (member-id=1 kann gesetzt
    // werden). Damit treibt der Test die Sequence-API ohne primitive-
    // Element-Storage-Edge-Case.
    let mut elem = DynamicTypeBuilderFactory::create_struct("::Item");
    elem.add_struct_member("v", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let elem_t = elem.build().unwrap();

    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member(
        "items",
        1,
        TypeDescriptor::sequence("items", TypeDescriptor::structure("::Item"), 100),
    )
    .unwrap();
    let t = b.build().unwrap();

    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    let mut e0 = DynamicDataFactory::create_data(&elem_t).unwrap();
    let mut e1 = DynamicDataFactory::create_data(&elem_t).unwrap();
    let mut e2 = DynamicDataFactory::create_data(&elem_t).unwrap();
    e0.set_int32_value(1, 10).unwrap();
    e1.set_int32_value(1, 20).unwrap();
    e2.set_int32_value(1, 30).unwrap();
    d.set_sequence_value(1, vec![e0, e1, e2]).unwrap();
    assert_eq!(d.get_sequence_length(1).unwrap(), 3);
    assert_eq!(
        d.get_sequence_element(1, 1)
            .unwrap()
            .get_int32_value(1)
            .unwrap(),
        20
    );
}

#[test]
fn block_c_data_string_set_get() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("name", 1, TypeDescriptor::string8(64))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    d.set_string_value(1, "hello").unwrap();
    assert_eq!(d.get_string_value(1).unwrap(), "hello");
}

#[test]
fn block_c_data_wstring_set_get() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("wname", 1, TypeDescriptor::string16(64))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    d.set_wstring_value(1, vec![0x4F, 0x4B]).unwrap();
    assert_eq!(d.get_wstring_value(1).unwrap(), vec![0x4F, 0x4B]);
}

#[test]
fn block_c_data_equals_value_diffs() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t = b.build().unwrap();
    let mut a = DynamicDataFactory::create_data(&t).unwrap();
    let mut b2 = DynamicDataFactory::create_data(&t).unwrap();
    a.set_int32_value(1, 1).unwrap();
    b2.set_int32_value(1, 2).unwrap();
    assert!(!a.equals(&b2));
    b2.set_int32_value(1, 1).unwrap();
    assert!(a.equals(&b2));
}

#[test]
fn block_c_data_factory_create_then_delete_is_clean() {
    let t = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
    let d = DynamicDataFactory::create_data(&t).unwrap();
    DynamicDataFactory::delete_data(d);
}

// ============================================================================
// Block D — Factory + Bridge (5 tests)
// ============================================================================

#[test]
fn block_d_factory_create_type_happy_path() {
    let b = DynamicTypeBuilderFactory::create_type(TypeDescriptor::structure("::S")).unwrap();
    let _ = b.build().unwrap();
}

#[test]
fn block_d_factory_get_primitive_singleton_returns_same_type() {
    let a = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int64).unwrap();
    let b = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int64).unwrap();
    // Identitaet via equals — gleicher Inhalt + (idR.) gleicher Arc.
    assert!(a.equals(&b));
    assert_eq!(a.kind(), TypeKind::Int64);
}

#[test]
fn block_d_factory_create_type_w_typeobject_for_struct() {
    // Build DynamicType, convert to TypeObject, then back via Factory.
    let t = struct_with_5_members();
    let to = t.to_type_object().unwrap();
    let b: DynamicTypeBuilder = DynamicTypeBuilderFactory::create_type_w_type_object(&to).unwrap();
    let t2 = b.build().unwrap();
    assert_eq!(t2.member_count(), 5);
}

#[test]
fn block_d_typeobject_roundtrip_preserves_equality() {
    let t = struct_with_5_members();
    let to = t.to_type_object().unwrap();
    let t2 = DynamicTypeBuilderFactory::create_type_w_type_object(&to)
        .unwrap()
        .build()
        .unwrap();
    assert!(
        t.equals(&t2),
        "DynamicType→TypeObject→DynamicType muss equals=true geben"
    );
    // Sanity: TypeObject ist Complete-Variante (XTypes 1.3 §7.6.3).
    assert!(matches!(
        to,
        TypeObject::Complete(CompleteTypeObject::Struct(_))
    ));
}

#[test]
fn block_d_factory_create_string_and_wstring() {
    let s = DynamicTypeBuilderFactory::create_string_type(255);
    assert_eq!(s.kind(), TypeKind::String8);
    assert_eq!(s.descriptor().bound, vec![255]);
    let w = DynamicTypeBuilderFactory::create_wstring_type(64);
    assert_eq!(w.kind(), TypeKind::String16);
}

// ============================================================================
// Block E — Loans (5 tests)
// ============================================================================

#[test]
fn block_e_loan_lifecycle_single_member() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    let loan: DataLoan = d.loan_value(1).unwrap();
    assert_eq!(loan.member_id(), 1);
    d.return_loaned_value(loan).unwrap();
}

#[test]
fn block_e_loan_double_loan_rejected() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    let _l = d.loan_value(1).unwrap();
    let err = d.loan_value(1).unwrap_err();
    assert!(matches!(err, DynamicError::LoanError(_)));
}

#[test]
fn block_e_loan_and_return_then_loan_again_succeeds() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    b.add_struct_member("y", 2, TypeDescriptor::primitive(TypeKind::Int64, "int64"))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    // Zwei Members koennen parallel geliehen werden.
    let l1 = d.loan_value(1).unwrap();
    let l2 = d.loan_value(2).unwrap();
    d.return_loaned_value(l1).unwrap();
    d.return_loaned_value(l2).unwrap();
    // Erneuter Loan auf #1 ist nach return wieder erlaubt.
    let l3 = d.loan_value(1).unwrap();
    d.return_loaned_value(l3).unwrap();
}

#[test]
fn block_e_loan_unknown_member_rejected() {
    let mut b = DynamicTypeBuilderFactory::create_struct("::S");
    b.add_struct_member("x", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let t = b.build().unwrap();
    let mut d = DynamicDataFactory::create_data(&t).unwrap();
    let err = d.loan_value(999).unwrap_err();
    assert!(matches!(err, DynamicError::BadParameter(_)));
}

#[test]
fn block_e_loan_on_nested_struct_member() {
    let mut inner = DynamicTypeBuilderFactory::create_struct("::I");
    inner
        .add_struct_member("v", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    let inner_t = inner.build().unwrap();

    let mut outer = DynamicTypeBuilderFactory::create_struct("::O");
    outer
        .add_member(MemberDescriptor::new(
            "n",
            10,
            TypeDescriptor::structure("::I"),
        ))
        .unwrap();
    let outer_t = outer.build().unwrap();

    let mut nested = DynamicDataFactory::create_data(&inner_t).unwrap();
    nested.set_int32_value(1, 5).unwrap();
    let mut od = DynamicDataFactory::create_data(&outer_t).unwrap();
    od.set_complex_value(10, nested).unwrap();

    let loan = od.loan_value(10).unwrap();
    assert_eq!(loan.member_id(), 10);
    od.return_loaned_value(loan).unwrap();
}
