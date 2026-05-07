//! Integration-Tests fuer DDS-XML 1.0 §7.3.3 (Types) + §7.3.7 (Samples).
//!
//! C7.D — Type-AST + Sample-Codec.

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

use zerodds_xml::{
    Extensibility, PrimitiveType, PrimitiveValue, SampleValue, StructMember, StructType, TypeDef,
    TypeRef, UnionDiscriminator, parse_dds_xml, parse_sample, parse_type_libraries,
    serialize_sample,
};

// ---------- §7.3.3 Types ---------------------------------------------------

#[test]
fn t01_struct_with_long_member() {
    let xml = r#"<types><struct name="X"><member name="x" type="long"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!("expected struct");
    };
    assert_eq!(s.name, "X");
    assert_eq!(s.members[0].name, "x");
    assert!(matches!(
        s.members[0].type_ref,
        TypeRef::Primitive(PrimitiveType::Long)
    ));
}

#[test]
fn t02_all_15_primitive_types() {
    let names = [
        "boolean",
        "octet",
        "char",
        "wchar",
        "short",
        "ushort",
        "long",
        "ulong",
        "longlong",
        "ulonglong",
        "float",
        "double",
        "longdouble",
        "string",
        "wstring",
    ];
    let mut body = String::new();
    body.push_str("<types><struct name=\"All\">");
    for (i, n) in names.iter().enumerate() {
        body.push_str(&format!("<member name=\"f{i}\" type=\"{n}\"/>"));
    }
    body.push_str("</struct></types>");
    let libs = parse_type_libraries(&body).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(s.members.len(), names.len());
    for (member, expected) in s.members.iter().zip(names.iter()) {
        let TypeRef::Primitive(p) = member.type_ref else {
            panic!("expected primitive for {expected}");
        };
        assert_eq!(p.as_xml(), *expected);
    }
}

#[test]
fn t03_extensibility_final_appendable_mutable() {
    for (kw, expected) in [
        ("final", Extensibility::Final),
        ("appendable", Extensibility::Appendable),
        ("mutable", Extensibility::Mutable),
    ] {
        let xml = format!(
            r#"<types><struct name="S" extensibility="{kw}"><member name="a" type="long"/></struct></types>"#
        );
        let libs = parse_type_libraries(&xml).expect("parse");
        let TypeDef::Struct(s) = &libs[0].types[0] else {
            panic!()
        };
        assert_eq!(s.extensibility, Some(expected));
    }
}

#[test]
fn t04_struct_inheritance_basetype() {
    let xml = r#"<types>
        <struct name="Parent"><member name="a" type="long"/></struct>
        <struct name="Child" baseType="Parent"><member name="b" type="long"/></struct>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(child) = &libs[0].types[1] else {
        panic!()
    };
    assert_eq!(child.base_type.as_deref(), Some("Parent"));
}

#[test]
fn t05_struct_member_key_true() {
    let xml =
        r#"<types><struct name="K"><member name="id" type="long" key="true"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert!(s.members[0].key);
}

#[test]
fn t06_struct_member_optional_true() {
    let xml = r#"<types><struct name="O"><member name="x" type="long" optional="true"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert!(s.members[0].optional);
}

#[test]
fn t07_struct_member_id_override() {
    let xml = r#"<types><struct name="I"><member name="x" type="long" id="42"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(s.members[0].id, Some(42));
}

#[test]
fn t08_struct_member_array_dimensions_multi() {
    let xml = r#"<types><struct name="A"><member name="m" type="long" arrayDimensions="3,4"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(s.members[0].array_dimensions, vec![3, 4]);
}

#[test]
fn t09_struct_member_max_lengths() {
    let xml = r#"<types><struct name="L"><member name="s" type="string" stringMaxLength="64" sequenceMaxLength="100"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(s.members[0].string_max_length, Some(64));
    assert_eq!(s.members[0].sequence_max_length, Some(100));
}

#[test]
fn t10_enum_with_explicit_values() {
    let xml = r#"<types><enum name="C">
        <enumerator name="R" value="0"/>
        <enumerator name="G" value="1"/>
        <enumerator name="B" value="2"/>
    </enum></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Enum(e) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(e.enumerators.len(), 3);
    assert_eq!(e.enumerators[0].value, Some(0));
    assert_eq!(e.enumerators[2].name, "B");
}

#[test]
fn t11_enum_without_values_implicit_numbering() {
    let xml = r#"<types><enum name="E">
        <enumerator name="A"/>
        <enumerator name="B"/>
    </enum></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Enum(e) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(e.enumerators[0].value, None);
    assert_eq!(e.enumerators[1].value, None);
}

#[test]
fn t12_union_with_cases() {
    let xml = r#"<types><union name="V" discriminator="long">
        <case>
            <caseDiscriminator value="0"/>
            <member name="i" type="long"/>
        </case>
        <case>
            <caseDiscriminator value="1"/>
            <member name="f" type="float"/>
        </case>
    </union></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Union(u) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(u.cases.len(), 2);
    assert!(matches!(
        u.cases[0].discriminators[0],
        UnionDiscriminator::Value(ref v) if v == "0"
    ));
}

#[test]
fn t13_union_with_default_case() {
    let xml = r#"<types><union name="V" discriminator="long">
        <case>
            <caseDiscriminator value="default"/>
            <member name="s" type="string"/>
        </case>
    </union></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Union(u) = &libs[0].types[0] else {
        panic!()
    };
    assert!(matches!(
        u.cases[0].discriminators[0],
        UnionDiscriminator::Default
    ));
}

#[test]
fn t14_bitmask_and_bitset() {
    let xml = r#"<types>
        <bitmask name="F" bitBound="32">
            <bit_value name="A" position="0"/>
            <bit_value name="B" position="1"/>
        </bitmask>
        <bitset name="S">
            <bitfield name="ready" type="boolean" mask="0x01"/>
            <bitfield name="error" type="long" mask="0x06"/>
        </bitset>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Bitmask(bm) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(bm.bit_bound, Some(32));
    assert_eq!(bm.bit_values.len(), 2);
    let TypeDef::Bitset(bs) = &libs[0].types[1] else {
        panic!()
    };
    assert_eq!(bs.bit_fields.len(), 2);
    assert_eq!(bs.bit_fields[1].mask, "0x06");
}

#[test]
fn t15_typedef_simple_and_array() {
    let xml = r#"<types>
        <typedef name="Velocity" type="float" arrayDimensions="3"/>
        <typedef name="UserId" type="long"/>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Typedef(td0) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(td0.array_dimensions, vec![3]);
    let TypeDef::Typedef(td1) = &libs[0].types[1] else {
        panic!()
    };
    assert!(td1.array_dimensions.is_empty());
}

#[test]
fn t16_module_three_levels_deep() {
    let xml = r#"<types>
        <module name="A">
            <module name="B">
                <module name="C">
                    <struct name="Deep"><member name="x" type="long"/></struct>
                </module>
            </module>
        </module>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Module(a) = &libs[0].types[0] else {
        panic!()
    };
    let TypeDef::Module(b) = &a.types[0] else {
        panic!()
    };
    let TypeDef::Module(c) = &b.types[0] else {
        panic!()
    };
    let TypeDef::Struct(s) = &c.types[0] else {
        panic!()
    };
    assert_eq!(s.name, "Deep");
}

#[test]
fn t17_dds_xml_resolve_type() {
    let xml = r#"<dds>
        <types>
            <module name="MyModule">
                <struct name="State"><member name="id" type="long"/></struct>
            </module>
        </types>
    </dds>"#;
    let dds = parse_dds_xml(xml).expect("parse");
    let td = dds.resolve_type("MyModule::State").expect("resolve");
    assert_eq!(td.name(), "State");
    // Bare name fallback
    let td2 = dds.resolve_type("State").expect("bare");
    assert_eq!(td2.name(), "State");
    // Missing
    assert!(dds.resolve_type("NoSuchType").is_none());
}

#[test]
fn t18_struct_with_inheritance_resolves() {
    let xml = r#"<dds><types>
        <struct name="P"><member name="a" type="long"/></struct>
        <struct name="C" baseType="P"><member name="b" type="long"/></struct>
    </types></dds>"#;
    let dds = parse_dds_xml(xml).expect("parse");
    let TypeDef::Struct(c) = dds.resolve_type("C").expect("c") else {
        panic!()
    };
    assert_eq!(c.base_type.as_deref(), Some("P"));
    assert!(dds.resolve_type("P").is_some());
}

// ---------- §7.3.7 Samples -------------------------------------------------

#[test]
fn t19_sample_parse_struct() {
    let td = TypeDef::Struct(StructType {
        name: "State".into(),
        members: vec![StructMember {
            name: "id".into(),
            type_ref: TypeRef::Primitive(PrimitiveType::Long),
            ..Default::default()
        }],
        ..Default::default()
    });
    let xml = r#"<sample type_ref="State"><id>42</id></sample>"#;
    let v = parse_sample(xml, &td, &[]).expect("parse");
    let SampleValue::Struct(m) = v else { panic!() };
    assert!(matches!(
        m.get("id"),
        Some(SampleValue::Primitive(PrimitiveValue::I32(42)))
    ));
}

#[test]
fn t20_sample_roundtrip() {
    let td = TypeDef::Struct(StructType {
        name: "S".into(),
        members: vec![
            StructMember {
                name: "a".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                ..Default::default()
            },
            StructMember {
                name: "b".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::String),
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let xml = r#"<sample type_ref="S"><a>7</a><b>hi</b></sample>"#;
    let v = parse_sample(xml, &td, &[]).expect("parse");
    let serialized = serialize_sample(&v, &td, &[]);
    let v2 = parse_sample(&serialized, &td, &[]).expect("re-parse");
    assert_eq!(v, v2);
}

#[test]
fn t21_sample_with_sequence_items() {
    let td = TypeDef::Struct(StructType {
        name: "V".into(),
        members: vec![StructMember {
            name: "vector".into(),
            type_ref: TypeRef::Primitive(PrimitiveType::Float),
            sequence_max_length: Some(100),
            ..Default::default()
        }],
        ..Default::default()
    });
    let xml = r#"<sample type_ref="V">
        <vector>
            <item>1.0</item>
            <item>2.0</item>
            <item>3.5</item>
        </vector>
    </sample>"#;
    let v = parse_sample(xml, &td, &[]).expect("parse");
    let SampleValue::Struct(m) = v else { panic!() };
    let SampleValue::Sequence(items) = m.get("vector").expect("vector") else {
        panic!()
    };
    assert_eq!(items.len(), 3);
    assert!(matches!(
        items[2],
        SampleValue::Primitive(PrimitiveValue::F32(_))
    ));
}

#[test]
fn t22_sample_with_union_active_branch() {
    let xml = r#"<dds><types>
        <union name="V" discriminator="long">
            <case><caseDiscriminator value="0"/><member name="i" type="long"/></case>
            <case><caseDiscriminator value="default"/><member name="s" type="string"/></case>
        </union>
    </types></dds>"#;
    let dds = parse_dds_xml(xml).expect("parse");
    let td = dds.resolve_type("V").expect("V").clone();
    let sample = r#"<sample type_ref="V"><discriminator>0</discriminator><i>99</i></sample>"#;
    let v = parse_sample(sample, &td, &dds.type_libraries).expect("parse");
    let SampleValue::Union {
        discriminator,
        value,
    } = v
    else {
        panic!()
    };
    assert_eq!(discriminator, "0");
    assert!(matches!(
        *value,
        SampleValue::Primitive(PrimitiveValue::I32(99))
    ));
}

#[test]
fn t23_sample_with_nested_struct() {
    let xml = r#"<dds><types>
        <struct name="Inner"><member name="x" type="long"/></struct>
        <struct name="Outer"><member name="inner" type="Inner"/></struct>
    </types></dds>"#;
    let dds = parse_dds_xml(xml).expect("parse");
    let outer = dds.resolve_type("Outer").expect("Outer").clone();
    let sample = r#"<sample type_ref="Outer"><inner><x>5</x></inner></sample>"#;
    let v = parse_sample(sample, &outer, &dds.type_libraries).expect("parse");
    let SampleValue::Struct(m) = v else { panic!() };
    let SampleValue::Struct(inner) = m.get("inner").expect("inner") else {
        panic!()
    };
    assert!(matches!(
        inner.get("x"),
        Some(SampleValue::Primitive(PrimitiveValue::I32(5)))
    ));
}

#[test]
fn t24_sample_string_escaping_roundtrip() {
    let td = TypeDef::Struct(StructType {
        name: "S".into(),
        members: vec![StructMember {
            name: "msg".into(),
            type_ref: TypeRef::Primitive(PrimitiveType::String),
            ..Default::default()
        }],
        ..Default::default()
    });
    // Spec §7.3.7.4.5: non-ASCII via numeric entity. roxmltree decodes
    // the entity into the actual Unicode character, so post-parse the
    // value contains the literal codepoint.
    let xml = r#"<sample type_ref="S"><msg>caf&#xe9;</msg></sample>"#;
    let v = parse_sample(xml, &td, &[]).expect("parse");
    let SampleValue::Struct(m) = v.clone() else {
        panic!()
    };
    let SampleValue::Primitive(PrimitiveValue::Str(s)) = m.get("msg").expect("msg") else {
        panic!()
    };
    assert_eq!(s, "caf\u{e9}");
    // Roundtrip via serializer (escapes Non-ASCII as numeric entity).
    let out = serialize_sample(&v, &td, &[]);
    assert!(out.contains("&#xe9;"), "serialized: {out}");
}

#[test]
fn t25_sample_optional_member_missing_ok() {
    let td = TypeDef::Struct(StructType {
        name: "S".into(),
        members: vec![
            StructMember {
                name: "a".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                ..Default::default()
            },
            StructMember {
                name: "b".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                optional: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let xml = r#"<sample type_ref="S"><a>1</a></sample>"#;
    let v = parse_sample(xml, &td, &[]).expect("parse");
    let SampleValue::Struct(m) = v else { panic!() };
    assert!(m.contains_key("a"));
    assert!(!m.contains_key("b"));
}

#[test]
fn t26_sample_must_understand_attribute_parses() {
    let xml = r#"<types><struct name="X"><member name="m" type="long" mustUnderstand="true"/></struct></types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert!(s.members[0].must_understand);
}
