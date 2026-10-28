//! DDS-XTypes 1.3 §7.5.2 and Annex C: the TypeKind constants, the typed
//! sequences of `DynamicData`, the members of an annotation type, the bound
//! of an unbounded collection and the equality of members.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_float128::Float128;
use zerodds_types::dynamic::{
    AnnotationType, DynamicError, DynamicType, DynamicTypeBuilder, DynamicTypeBuilderFactory,
    ExtensibilityKind, LENGTH_UNLIMITED, NewMember, TK_ALIAS, TK_ANNOTATION, TK_ARRAY, TK_BITMASK,
    TK_BITSET, TK_BOOLEAN, TK_BYTE, TK_CHAR8, TK_CHAR16, TK_ENUM, TK_FLOAT32, TK_FLOAT64,
    TK_FLOAT128, TK_INT8, TK_INT16, TK_INT32, TK_INT64, TK_MAP, TK_NONE, TK_SEQUENCE, TK_STRING8,
    TK_STRING16, TK_STRUCTURE, TK_UINT8, TK_UINT16, TK_UINT32, TK_UINT64, TK_UNION, TypeKind,
};

fn model(idl: &str) -> Arc<Ast> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "annex_c.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    Arc::new(ast)
}

/// Annex C: "const TypeKind TK_NONE = 0x00; [...] const TypeKind TK_MAP =
/// 0x62;" — the constants have the numbers of the annex, and every kind
/// has exactly one of them.
#[test]
fn the_type_kind_constants_have_the_numbers_of_annex_c() {
    let annex: [(&str, u8, u8, TypeKind); 28] = [
        ("TK_NONE", TK_NONE, 0x00, TypeKind::NoType),
        ("TK_BOOLEAN", TK_BOOLEAN, 0x01, TypeKind::Boolean),
        ("TK_BYTE", TK_BYTE, 0x02, TypeKind::Byte),
        ("TK_INT16", TK_INT16, 0x03, TypeKind::Int16),
        ("TK_INT32", TK_INT32, 0x04, TypeKind::Int32),
        ("TK_INT64", TK_INT64, 0x05, TypeKind::Int64),
        ("TK_UINT16", TK_UINT16, 0x06, TypeKind::UInt16),
        ("TK_UINT32", TK_UINT32, 0x07, TypeKind::UInt32),
        ("TK_UINT64", TK_UINT64, 0x08, TypeKind::UInt64),
        ("TK_FLOAT32", TK_FLOAT32, 0x09, TypeKind::Float32),
        ("TK_FLOAT64", TK_FLOAT64, 0x0A, TypeKind::Float64),
        ("TK_FLOAT128", TK_FLOAT128, 0x0B, TypeKind::Float128),
        ("TK_INT8", TK_INT8, 0x0C, TypeKind::Int8),
        ("TK_UINT8", TK_UINT8, 0x0D, TypeKind::UInt8),
        ("TK_CHAR8", TK_CHAR8, 0x10, TypeKind::Char8),
        ("TK_CHAR16", TK_CHAR16, 0x11, TypeKind::Char16),
        ("TK_STRING8", TK_STRING8, 0x20, TypeKind::String8),
        ("TK_STRING16", TK_STRING16, 0x21, TypeKind::String16),
        ("TK_ALIAS", TK_ALIAS, 0x30, TypeKind::Alias),
        ("TK_ENUM", TK_ENUM, 0x40, TypeKind::Enumeration),
        ("TK_BITMASK", TK_BITMASK, 0x41, TypeKind::Bitmask),
        ("TK_ANNOTATION", TK_ANNOTATION, 0x50, TypeKind::Annotation),
        ("TK_STRUCTURE", TK_STRUCTURE, 0x51, TypeKind::Structure),
        ("TK_UNION", TK_UNION, 0x52, TypeKind::Union),
        ("TK_BITSET", TK_BITSET, 0x53, TypeKind::Bitset),
        ("TK_SEQUENCE", TK_SEQUENCE, 0x60, TypeKind::Sequence),
        ("TK_ARRAY", TK_ARRAY, 0x61, TypeKind::Array),
        ("TK_MAP", TK_MAP, 0x62, TypeKind::Map),
    ];
    for (name, constant, number, kind) in annex {
        assert_eq!(constant, number, "{name}");
        assert_eq!(kind.value(), number, "{name}");
        assert_eq!(TypeKind::from_value(number), Some(kind), "{name}");
    }
    assert_eq!(TypeKind::ALL.len(), annex.len());
    assert_eq!(TypeKind::from_value(0x0E), None);
}

/// Annex C: the typed sequences are the types of the `*_values`
/// operations of `DynamicData`.
#[test]
fn the_typed_sequences_are_the_types_of_the_values_operations() {
    use zerodds_types::dynamic::{
        BooleanSeq, ByteSeq, CharSeq, DynamicSample, Float32Seq, Float64Seq, Float128Seq, Int8Seq,
        Int16Seq, Int32Seq, Int64Seq, StringSeq, UInt8Seq, UInt16Seq, UInt32Seq, UInt64Seq,
        WcharSeq, WstringSeq,
    };
    let _: fn(&DynamicSample, u32) -> Result<Int32Seq, DynamicError> =
        DynamicSample::get_int32_values;
    let _: fn(&DynamicSample, u32) -> Result<UInt32Seq, DynamicError> =
        DynamicSample::get_uint32_values;
    let _: fn(&DynamicSample, u32) -> Result<Int8Seq, DynamicError> =
        DynamicSample::get_int8_values;
    let _: fn(&DynamicSample, u32) -> Result<UInt8Seq, DynamicError> =
        DynamicSample::get_uint8_values;
    let _: fn(&DynamicSample, u32) -> Result<Int16Seq, DynamicError> =
        DynamicSample::get_int16_values;
    let _: fn(&DynamicSample, u32) -> Result<UInt16Seq, DynamicError> =
        DynamicSample::get_uint16_values;
    let _: fn(&DynamicSample, u32) -> Result<Int64Seq, DynamicError> =
        DynamicSample::get_int64_values;
    let _: fn(&DynamicSample, u32) -> Result<UInt64Seq, DynamicError> =
        DynamicSample::get_uint64_values;
    let _: fn(&DynamicSample, u32) -> Result<Float32Seq, DynamicError> =
        DynamicSample::get_float32_values;
    let _: fn(&DynamicSample, u32) -> Result<Float64Seq, DynamicError> =
        DynamicSample::get_float64_values;
    let _: fn(&DynamicSample, u32) -> Result<Float128Seq, DynamicError> =
        DynamicSample::get_float128_values;
    let _: fn(&DynamicSample, u32) -> Result<CharSeq, DynamicError> =
        DynamicSample::get_char8_values;
    let _: fn(&DynamicSample, u32) -> Result<WcharSeq, DynamicError> =
        DynamicSample::get_char16_values;
    let _: fn(&DynamicSample, u32) -> Result<BooleanSeq, DynamicError> =
        DynamicSample::get_boolean_values;
    let _: fn(&DynamicSample, u32) -> Result<ByteSeq, DynamicError> =
        DynamicSample::get_byte_values;
    let _: fn(&DynamicSample, u32) -> Result<StringSeq, DynamicError> =
        DynamicSample::get_string_values;
    let _: fn(&DynamicSample, u32) -> Result<WstringSeq, DynamicError> =
        DynamicSample::get_wstring_values;

    // And they work: a sequence member is set and read as a typed
    // sequence.
    let ast =
        model("struct S { sequence<long> a; sequence<wstring> w; sequence<long double> f; };");
    let s = DynamicType::find(&ast, "S").unwrap();
    let mut sample = DynamicSample::new(&s).unwrap();
    let a = sample.get_member_id_by_name("a").unwrap();
    let w = sample.get_member_id_by_name("w").unwrap();
    let f = sample.get_member_id_by_name("f").unwrap();
    let ints: Int32Seq = vec![3, -1, 7];
    sample.set_int32_values(a, &ints).unwrap();
    assert_eq!(sample.get_int32_values(a).unwrap(), ints);
    let words: WstringSeq = vec!["alpha".into(), "beta".into()];
    sample.set_wstring_values(w, &words).unwrap();
    assert_eq!(sample.get_wstring_values(w).unwrap(), words);
    let wide: Float128Seq = vec![Float128::from_f64(1.5)];
    sample.set_float128_values(f, &wide).unwrap();
    assert_eq!(sample.get_float128_values(f).unwrap().len(), 1);
}

/// Table 50, S. 203: "If this descriptor represents a sequence, map,
/// bitmask, or string type, the length of the property value is one", also
/// for a collection with no bound. The value that stands for no bound is
/// `LENGTH_UNLIMITED` (§7.5.2.2.3 to §7.5.2.2.5).
#[test]
fn the_bound_of_an_unbounded_collection_is_one_value() {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let long = factory.get_primitive_type(TypeKind::Int32).unwrap();
    for builder in [
        factory.create_string_type(LENGTH_UNLIMITED).unwrap(),
        factory.create_wstring_type(LENGTH_UNLIMITED).unwrap(),
        factory
            .create_sequence_type(&long, LENGTH_UNLIMITED)
            .unwrap(),
        factory
            .create_map_type(&long, &long, LENGTH_UNLIMITED)
            .unwrap(),
    ] {
        let kind = builder.get_kind().unwrap();
        let descriptor = builder.get_descriptor().unwrap();
        assert_eq!(descriptor.bound, vec![LENGTH_UNLIMITED], "{kind:?}");
        assert!(descriptor.is_consistent().is_ok(), "{kind:?}");
        // A type created from the descriptor is unbounded again.
        let again = factory.create_type(&descriptor).unwrap().build().unwrap();
        assert_eq!(again.descriptor().unwrap().bound, vec![LENGTH_UNLIMITED]);
    }
    // A bound that was declared stays that number.
    let bounded = factory.create_sequence_type(&long, 5).unwrap();
    assert_eq!(bounded.get_descriptor().unwrap().bound, vec![5]);
    // A descriptor with no bound at all is read as unbounded, as before.
    let mut empty = factory
        .create_sequence_type(&long, 5)
        .unwrap()
        .get_descriptor()
        .unwrap();
    empty.bound.clear();
    let built = factory.create_type(&empty).unwrap().build().unwrap();
    assert_eq!(built.descriptor().unwrap().bound, vec![LENGTH_UNLIMITED]);
}

/// Table 51, S. 205: "ANNOTATION_TYPE [...] a 'member' in this sense has
/// the same meaning as it does in the definition of aggregated types
/// generally."
#[test]
fn an_annotation_type_has_members() {
    let ast = model(
        r#"
        @annotation Scaled { long scale default 10; string label; boolean flag; };
        "#,
    );
    let scaled = AnnotationType::find(&ast, "Scaled").unwrap();
    assert_eq!(scaled.member_count(), 3);
    let by_index = scaled.get_member_by_index(0).unwrap();
    assert_eq!(by_index.name(), "scale");
    assert_eq!(by_index.id(), 0);
    let label = scaled.get_member_by_name("label").unwrap();
    assert_eq!(label.index(), 1);
    assert_eq!(scaled.get_member(label.id()).unwrap().name(), "label");

    // The descriptor of a member has its name, id, type and default.
    let scale = scaled
        .get_member_by_name("scale")
        .unwrap()
        .descriptor()
        .unwrap();
    assert_eq!(scale.name, "scale");
    assert_eq!(scale.id, 0);
    assert_eq!(scale.type_.kind(), TypeKind::Int32);
    assert_eq!(scale.default_value, "10");
    assert!(scale.has_default_value);
    assert!(scale.is_consistent().is_ok());
    let label = label.descriptor().unwrap();
    assert_eq!(label.type_.kind(), TypeKind::String8);
    assert_eq!(label.default_value, "");
    assert_eq!(
        scaled
            .get_member_by_name("flag")
            .unwrap()
            .descriptor()
            .unwrap()
            .type_
            .kind(),
        TypeKind::Boolean
    );

    // The maps of the type.
    assert_eq!(
        scaled
            .get_all_members_by_name()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["flag", "label", "scale"]
    );
    assert_eq!(
        scaled.get_all_members().keys().copied().collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // Out of range or unknown: RETCODE_BAD_PARAMETER.
    for error in [
        scaled.get_member_by_index(3).unwrap_err(),
        scaled.get_member_by_name("nope").unwrap_err(),
        scaled.get_member(9).unwrap_err(),
    ] {
        assert_eq!(error.return_code(), "RETCODE_BAD_PARAMETER", "{error}");
    }

    // Two members are equal when they belong to the same annotation type
    // and agree in their descriptors.
    let again = scaled.get_member_by_name("scale").unwrap();
    assert!(again.equals(&scaled.get_member_by_name("scale").unwrap()));
    assert!(!again.equals(&scaled.get_member_by_name("label").unwrap()));

    // A built-in annotation reports its parameters, with the type the
    // value is written as.
    let range = AnnotationType::find(&ast, "range").unwrap();
    let names: Vec<String> = range
        .members()
        .iter()
        .map(|m| m.name().to_string())
        .collect();
    assert_eq!(names, vec!["min", "max"]);
    let key = AnnotationType::find(&ast, "key").unwrap();
    assert_eq!(
        key.get_member_by_name("value")
            .unwrap()
            .descriptor()
            .unwrap()
            .type_
            .kind(),
        TypeKind::Boolean
    );
}

fn struct_of(name: &str, members: &[&str]) -> DynamicType {
    let mut b = DynamicTypeBuilder::new();
    let long = b.primitive(TypeKind::Int32).unwrap();
    let s = b.structure(name, ExtensibilityKind::Final).unwrap();
    for member in members {
        b.add_member(s, &NewMember::new(member, long)).unwrap();
    }
    DynamicType::new(b.finish(), s)
}

/// Table 52, S. 206: "Two members shall be considered equal if and only if
/// they belong to the same type and all of their respective properties
/// [...] are equal."
#[test]
fn members_of_different_types_are_not_equal() {
    let one = struct_of("A", &["x", "y"]);
    let same = struct_of("A", &["x", "y"]);
    let other_name = struct_of("B", &["x", "y"]);
    let other_shape = struct_of("A", &["x", "y", "z"]);

    let x = one.member_by_name("x").unwrap();
    // The same member of an equal type.
    assert!(x.equals(&same.member_by_name("x").unwrap()));
    // A member of the same name, id, index and type, in a type of another
    // name.
    assert!(!x.equals(&other_name.member_by_name("x").unwrap()));
    // And in a type with another member list.
    assert!(!x.equals(&other_shape.member_by_name("x").unwrap()));
    // The types themselves still compare as they did.
    assert!(one.equals(&same));
    assert!(!one.equals(&other_shape));
}
