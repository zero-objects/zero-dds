//! DDS-XTypes 1.3 Annex C: the IDL of the Dynamic Language Binding and the
//! Rust API that stands for it.
//!
//! The IDL is `crates/types/tests/fixtures/annex-c-dynamic-language-binding.idl`,
//! the text of the annex. The test parses it and lists every constant, every
//! operation of every interface and valuetype, and every state member of a
//! valuetype. Each of them is then bound to its counterpart in
//! `zerodds_types::dynamic`, `zerodds_dcps::serialized_type` and
//! `zerodds_xml_compose`: a binding is a path that has to resolve when this
//! file compiles, and it is recorded under the name the IDL gives it. The
//! recorded set and the IDL's must be equal, so that an operation added to
//! the IDL (or left out of the binding) fails the test by name.
//!
//! The behaviour of the operations is tested where they are defined
//! (`crates/types/tests/dynamic_*.rs`, `crates/xml-compose/tests`,
//! `crates/dcps/src/serialized_type.rs`); this file is about the surface.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::collections::BTreeSet;

use zerodds_ast::{Ast, TypeKind as AstKind};
use zerodds_dcps::serialized_type::SerializedTypeSupport as TypeSupport;
use zerodds_types::dynamic::{
    AnnotationDescriptor, DynamicDataFactory, DynamicSample, DynamicType,
    DynamicTypeBuilderFactory, DynamicTypeMember, MemberDescriptor, TypeBuilder, TypeDescriptor,
    TypeKind, VerbatimTextDescriptor,
};

const IDL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../types/tests/fixtures/annex-c-dynamic-language-binding.idl"
));

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "annex-c.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

/// `Interface::operation` for each operation the IDL declares.
fn idl_operations(ast: &Ast) -> BTreeSet<String> {
    ast.operations()
        .map(|(_, op)| {
            let owner = ast
                .scoped_name_of(op.parent)
                .expect("an operation has an owner")
                .to_string();
            let owner = owner.rsplit("::").next().unwrap().to_string();
            format!("{owner}::{}", op.name)
        })
        .collect()
}

/// `Valuetype.state` for each state member of a valuetype.
fn idl_state(ast: &Ast) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, node) in ast.types() {
        if let AstKind::Valuetype { state, .. } = &node.kind {
            for member in state {
                out.insert(format!(
                    "{}.{}",
                    node.name,
                    ast.member(*member).unwrap().name
                ));
            }
        }
    }
    out
}

macro_rules! bind {
    ($seen:ident, $iface:literal, $($op:ident => $path:expr),* $(,)?) => {
        $(
            let _ = $path;
            assert!($seen.insert(concat!($iface, "::", stringify!($op)).to_string()));
        )*
    };
}

macro_rules! typed {
    ($seen:ident; $(($get:ident, $set:ident, $gets:ident, $sets:ident)),* $(,)?) => {
        $(
            let _ = DynamicSample::$get;
            let _ = DynamicSample::$set;
            let _ = DynamicSample::$gets;
            let _ = DynamicSample::$sets;
            for op in [stringify!($get), stringify!($set), stringify!($gets), stringify!($sets)] {
                assert!($seen.insert(format!("DynamicData::{op}")));
            }
        )*
    };
}

/// Every operation of the IDL has a binding, and every binding is an
/// operation of the IDL.
#[test]
fn every_operation_of_annex_c_has_its_counterpart() {
    let ast = model();
    let wanted = idl_operations(&ast);
    assert!(wanted.len() > 120, "{} operations parsed", wanted.len());

    let mut seen = BTreeSet::new();

    bind!(seen, "DynamicTypeBuilderFactory",
        get_instance => DynamicTypeBuilderFactory::get_instance,
        delete_instance => DynamicTypeBuilderFactory::delete_instance,
        get_primitive_type => DynamicTypeBuilderFactory::get_primitive_type,
        create_type => DynamicTypeBuilderFactory::create_type,
        create_type_copy => DynamicTypeBuilderFactory::create_type_copy,
        create_type_w_type_object => DynamicTypeBuilderFactory::create_type_w_type_object,
        create_string_type => DynamicTypeBuilderFactory::create_string_type,
        create_wstring_type => DynamicTypeBuilderFactory::create_wstring_type,
        create_sequence_type => DynamicTypeBuilderFactory::create_sequence_type,
        create_array_type => DynamicTypeBuilderFactory::create_array_type,
        create_map_type => DynamicTypeBuilderFactory::create_map_type,
        create_bitmask_type => DynamicTypeBuilderFactory::create_bitmask_type,
        create_type_w_uri => zerodds_xml_compose::create_type_w_uri,
        create_type_w_document => zerodds_xml_compose::create_type_w_document,
        delete_type => DynamicTypeBuilderFactory::delete_type,
    );

    bind!(seen, "DynamicTypeSupport",
        create_type_support => TypeSupport::create_type_support,
        delete_type_support => TypeSupport::delete_type_support,
        register_type => TypeSupport::register_type,
        get_type_name => TypeSupport::get_type_name,
    );

    bind!(seen, "AnnotationDescriptor",
        get_value => AnnotationDescriptor::get_value,
        get_all_value => AnnotationDescriptor::get_all_value,
        set_value => AnnotationDescriptor::set_value,
        copy_from => AnnotationDescriptor::copy_from,
        equals => AnnotationDescriptor::equals,
        is_consistent => AnnotationDescriptor::is_consistent,
    );

    bind!(seen, "VerbatimTextDescriptor",
        copy_from => VerbatimTextDescriptor::copy_from,
        equals => VerbatimTextDescriptor::equals,
        is_consistent => VerbatimTextDescriptor::is_consistent,
    );

    bind!(seen, "TypeDescriptor",
        copy_from => TypeDescriptor::copy_from,
        equals => TypeDescriptor::equals,
        is_consistent => TypeDescriptor::is_consistent,
    );

    bind!(seen, "MemberDescriptor",
        copy_from => MemberDescriptor::copy_from,
        equals => MemberDescriptor::equals,
        is_consistent => MemberDescriptor::is_consistent,
    );

    bind!(seen, "DynamicTypeMember",
        get_descriptor => DynamicTypeMember::get_descriptor,
        get_annotation_count => DynamicTypeMember::get_annotation_count,
        get_annotation => DynamicTypeMember::get_annotation,
        get_verbatim_text_count => DynamicTypeMember::get_verbatim_text_count,
        get_verbatim_text => DynamicTypeMember::get_verbatim_text,
        equals => DynamicTypeMember::equals,
        get_id => DynamicTypeMember::get_id,
        get_name => DynamicTypeMember::get_name,
    );

    bind!(seen, "DynamicTypeBuilder",
        get_descriptor => TypeBuilder::get_descriptor,
        get_name => TypeBuilder::get_name,
        get_kind => TypeBuilder::get_kind,
        get_member_by_name => TypeBuilder::get_member_by_name,
        get_all_members_by_name => TypeBuilder::get_all_members_by_name,
        get_member => TypeBuilder::get_member,
        get_all_members => TypeBuilder::get_all_members,
        get_member_count => TypeBuilder::get_member_count,
        get_member_by_index => TypeBuilder::get_member_by_index,
        get_annotation_count => TypeBuilder::get_annotation_count,
        get_annotation => TypeBuilder::get_annotation,
        equals => TypeBuilder::equals,
        add_member => TypeBuilder::add_member,
        apply_annotation => TypeBuilder::apply_annotation,
        build => TypeBuilder::build,
    );

    bind!(seen, "DynamicType",
        get_descriptor => DynamicType::get_descriptor,
        get_name => DynamicType::get_name,
        get_kind => DynamicType::get_kind,
        get_member_by_name => DynamicType::get_member_by_name,
        get_all_members_by_name => DynamicType::get_all_members_by_name,
        get_member => DynamicType::get_member,
        get_all_members => DynamicType::get_all_members,
        get_member_count => DynamicType::get_member_count,
        get_member_by_index => DynamicType::get_member_by_index,
        get_annotation_count => DynamicType::get_annotation_count,
        get_annotation => DynamicType::get_annotation,
        get_verbatim_text_count => DynamicType::get_verbatim_text_count,
        get_verbatim_text => DynamicType::get_verbatim_text,
        equals => DynamicType::equals,
    );

    bind!(seen, "DynamicDataFactory",
        get_instance => DynamicDataFactory::get_instance,
        delete_instance => DynamicDataFactory::delete_instance,
        create_data => DynamicDataFactory::create_data,
        delete_data => DynamicDataFactory::delete_data,
    );

    bind!(seen, "DynamicData",
        get_descriptor => DynamicSample::get_descriptor,
        set_descriptor => DynamicSample::set_descriptor,
        equals => DynamicSample::equals,
        get_member_id_by_name => DynamicSample::get_member_id_by_name,
        get_member_id_at_index => DynamicSample::get_member_id_at_index,
        get_item_count => DynamicSample::get_item_count,
        clear_all_values => DynamicSample::clear_all_values,
        clear_nonkey_values => DynamicSample::clear_nonkey_values,
        clear_value => DynamicSample::clear_value,
        loan_value => DynamicSample::loan_value,
        return_loaned_value => DynamicSample::return_loaned_value,
        clone => <DynamicSample as Clone>::clone,
        get_complex_value => DynamicSample::get_complex_value,
        set_complex_value => DynamicSample::set_complex_value,
    );
    typed!(seen;
        (get_int32_value, set_int32_value, get_int32_values, set_int32_values),
        (get_uint32_value, set_uint32_value, get_uint32_values, set_uint32_values),
        (get_int8_value, set_int8_value, get_int8_values, set_int8_values),
        (get_uint8_value, set_uint8_value, get_uint8_values, set_uint8_values),
        (get_int16_value, set_int16_value, get_int16_values, set_int16_values),
        (get_uint16_value, set_uint16_value, get_uint16_values, set_uint16_values),
        (get_int64_value, set_int64_value, get_int64_values, set_int64_values),
        (get_uint64_value, set_uint64_value, get_uint64_values, set_uint64_values),
        (get_float32_value, set_float32_value, get_float32_values, set_float32_values),
        (get_float64_value, set_float64_value, get_float64_values, set_float64_values),
        (get_float128_value, set_float128_value, get_float128_values, set_float128_values),
        (get_char8_value, set_char8_value, get_char8_values, set_char8_values),
        (get_char16_value, set_char16_value, get_char16_values, set_char16_values),
        (get_byte_value, set_byte_value, get_byte_values, set_byte_values),
        (get_boolean_value, set_boolean_value, get_boolean_values, set_boolean_values),
        (get_string_value, set_string_value, get_string_values, set_string_values),
        (get_wstring_value, set_wstring_value, get_wstring_values, set_wstring_values),
    );

    let missing: Vec<_> = wanted.difference(&seen).collect();
    let extra: Vec<_> = seen.difference(&wanted).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "operations of the IDL with no binding: {missing:?}; bindings with no operation in the IDL: {extra:?}"
    );
}

/// The implied IDL of `FooTypeSupport` (a comment in the annex) and the
/// attributes of `DynamicData` and `AnnotationDescriptor`.
#[test]
fn the_implied_type_support_and_the_attributes_have_theirs() {
    let _ = TypeSupport::register_type;
    let _ = TypeSupport::get_type_name;
    let _ = TypeSupport::get_type;
    let _ = TypeSupport::create_sample;
    let _ = TypeSupport::create_dynamic_sample;
    // `readonly attribute DynamicType type` of DynamicData.
    let _ = DynamicSample::type_;
    // `public DynamicType type` of AnnotationDescriptor.
    let _ = AnnotationDescriptor::annotation_type;
    let _ = AnnotationDescriptor::set_annotation_type;
}

/// The constants of the IDL are the `TK_*` of `zerodds_types::dynamic` with
/// the same numbers.
#[test]
fn the_type_kind_constants_of_the_idl_are_the_rust_constants() {
    use zerodds_types::dynamic as d;
    let ast = model();
    let rust: &[(&str, u8)] = &[
        ("TK_NONE", d::TK_NONE),
        ("TK_BOOLEAN", d::TK_BOOLEAN),
        ("TK_BYTE", d::TK_BYTE),
        ("TK_INT16", d::TK_INT16),
        ("TK_INT32", d::TK_INT32),
        ("TK_INT64", d::TK_INT64),
        ("TK_UINT16", d::TK_UINT16),
        ("TK_UINT32", d::TK_UINT32),
        ("TK_UINT64", d::TK_UINT64),
        ("TK_FLOAT32", d::TK_FLOAT32),
        ("TK_FLOAT64", d::TK_FLOAT64),
        ("TK_FLOAT128", d::TK_FLOAT128),
        ("TK_INT8", d::TK_INT8),
        ("TK_UINT8", d::TK_UINT8),
        ("TK_CHAR8", d::TK_CHAR8),
        ("TK_CHAR16", d::TK_CHAR16),
        ("TK_STRING8", d::TK_STRING8),
        ("TK_STRING16", d::TK_STRING16),
        ("TK_ALIAS", d::TK_ALIAS),
        ("TK_ENUM", d::TK_ENUM),
        ("TK_BITMASK", d::TK_BITMASK),
        ("TK_ANNOTATION", d::TK_ANNOTATION),
        ("TK_STRUCTURE", d::TK_STRUCTURE),
        ("TK_UNION", d::TK_UNION),
        ("TK_BITSET", d::TK_BITSET),
        ("TK_SEQUENCE", d::TK_SEQUENCE),
        ("TK_ARRAY", d::TK_ARRAY),
        ("TK_MAP", d::TK_MAP),
    ];
    let mut idl = BTreeSet::new();
    for (_, constant) in ast.constants() {
        if !constant.name.starts_with("TK_") {
            continue;
        }
        let value = match ast
            .eval_const(constant.expr)
            .expect("a constant expression")
        {
            zerodds_ast::ConstValue::Integer(v) => v,
            other => panic!("{}: {other:?}", constant.name),
        };
        let (_, number) = rust
            .iter()
            .find(|(name, _)| *name == constant.name)
            .unwrap_or_else(|| panic!("no Rust constant for {}", constant.name));
        assert_eq!(i128::from(*number), value, "{}", constant.name);
        assert!(TypeKind::from_value(*number).is_some(), "{}", constant.name);
        idl.insert(constant.name.clone());
    }
    assert_eq!(idl.len(), rust.len(), "{idl:?}");
}

/// The state members of the valuetypes are the fields of the Rust records.
#[test]
fn the_state_of_the_valuetypes_is_in_the_records() {
    let ast = model();
    let state = idl_state(&ast);

    // Every field is named; a field added to a record fails to compile
    // here until it is listed.
    let TypeDescriptor {
        kind: _,
        name: _,
        base_type: _,
        discriminator_type: _,
        bound: _,
        element_type: _,
        key_element_type: _,
        extensibility_kind: _,
        is_nested: _,
        bit_bound: _,
    } = TypeDescriptor::default_for_test();
    let MemberDescriptor {
        name: _,
        id: _,
        type_: _,
        index: _,
        label: _,
        is_default_label: _,
        try_construct_kind: _,
        is_key: _,
        is_optional: _,
        is_must_understand: _,
        is_shared: _,
        has_default_value: _,
        default_value: _,
    } = MemberDescriptor::default_for_test();
    let VerbatimTextDescriptor {
        placement: _,
        text: _,
        language: _,
    } = VerbatimTextDescriptor::default();

    let rust: BTreeSet<String> = [
        "TypeDescriptor.kind",
        "TypeDescriptor.name",
        "TypeDescriptor.base_type",
        "TypeDescriptor.discriminator_type",
        "TypeDescriptor.bound",
        "TypeDescriptor.element_type",
        "TypeDescriptor.key_element_type",
        "TypeDescriptor.extensibility_kind",
        "TypeDescriptor.is_nested",
        "MemberDescriptor.name",
        "MemberDescriptor.id",
        // `type` is a keyword in Rust.
        "MemberDescriptor.type",
        "MemberDescriptor.default_value",
        "MemberDescriptor.index",
        "MemberDescriptor.label",
        "MemberDescriptor.try_construct_kind",
        "MemberDescriptor.is_key",
        "MemberDescriptor.is_optional",
        "MemberDescriptor.is_must_understand",
        "MemberDescriptor.is_shared",
        "MemberDescriptor.is_default_label",
        "VerbatimTextDescriptor.placement",
        "VerbatimTextDescriptor.text",
        // `AnnotationDescriptor.type` is `annotation_type()`.
        "AnnotationDescriptor.type",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(state, rust);
}

/// Test-only constructors for the exhaustive pattern above.
trait DefaultForTest {
    fn default_for_test() -> Self;
}

impl DefaultForTest for TypeDescriptor {
    fn default_for_test() -> Self {
        let long = DynamicTypeBuilderFactory::get_instance()
            .get_primitive_type(TypeKind::Int32)
            .unwrap();
        long.descriptor().unwrap()
    }
}

impl DefaultForTest for MemberDescriptor {
    fn default_for_test() -> Self {
        let ast = {
            let parsed = zerodds_idl_parse::parse("struct S { long a; };");
            let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "s.idl");
            let mut ast = lowered.ast;
            ast.resolve_all_by_name().expect("resolve");
            std::sync::Arc::new(ast)
        };
        DynamicType::find(&ast, "S")
            .unwrap()
            .member_by_name("a")
            .unwrap()
            .descriptor()
            .unwrap()
    }
}
