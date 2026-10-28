// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-AMQP 1.0 §7's mapping, read off a model the real front end built.
//!
//! The unit tests in `src/codegen/encode.rs` construct a [`TypePlan`] by
//! hand and prove the encodings. These start from IDL text and check
//! that the plan a type gets is the one §7 gives it: which AMQP code
//! each row of §7.1's primitive table takes, which body form §7.2.2 puts
//! a `@final`, an `@appendable` and a `@mutable` composite in, and what
//! §7.1 refuses.
//!
//! [`TypePlan`]: zerodds_amqp_bridge::codegen::TypePlan

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_amqp_bridge::AmqpExtValue;
use zerodds_amqp_bridge::codegen::{
    BodyForm, Descriptor, EncodeError, EncodeOptions, FieldEncoding, PlanError, PlanSet,
    SampleValue, described_composite, json_document, plan_of,
};
use zerodds_ast::Ast;
use zerodds_ast_core::Id;

const IDL: &str = r#"
module probe {
    struct Primitives {
        boolean a_boolean;
        octet an_octet;
        int8 an_int8;
        uint8 a_uint8;
        short a_short;
        unsigned short a_ushort;
        long a_long;
        unsigned long a_ulong;
        long long a_long_long;
        unsigned long long a_ulong_long;
        float a_float;
        double a_double;
        long double a_long_double;
        char a_char;
        wchar a_wchar;
        string a_string;
        wstring a_wstring;
    };

    enum Colour { RED, GREEN };

    struct Compounds {
        sequence<octet> blob;
        sequence<long> numbers;
        long grid[4];
        map<string, long> lookup;
        Colour tint;
    };

    union Choice switch (long) {
        case 1: long n;
        case 2: string s;
    };

    @mutable
    struct Evolving {
        @id(10) long a;
        @id(20) string b;
    };

    @appendable
    struct Growing {
        long a;
    };

    bitmask Standalone { ONE };

    struct WithBitmask {
        Standalone flags;
    };
};
"#;

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the fixture IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "type-mapping.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the fixture IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name must resolve: {:?}",
        report.broken
    );
    ast
}

fn types_by_name(ast: &Ast) -> BTreeMap<String, Id<zerodds_ast::TypeNode>> {
    ast.types()
        .filter(|(_, node)| !node.name.is_empty())
        .filter_map(|(id, node)| {
            let scoped = ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))?;
            Some((scoped.segments().join("::"), id))
        })
        .collect()
}

fn plan(ast: &Ast, name: &str) -> zerodds_amqp_bridge::codegen::TypePlan {
    let id = *types_by_name(ast)
        .get(name)
        .unwrap_or_else(|| panic!("no type `{name}`"));
    plan_of(ast, id, &[0xAB; 14], false).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn every_row_of_the_primitive_table_takes_the_code_it_names() {
    let ast = model();
    let plan = plan(&ast, "probe::Primitives");
    let seen: Vec<(&str, FieldEncoding)> = plan
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field.encoding.clone()))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("a_boolean", FieldEncoding::Boolean),
            ("an_octet", FieldEncoding::Ubyte),
            ("an_int8", FieldEncoding::Byte),
            ("a_uint8", FieldEncoding::Ubyte),
            ("a_short", FieldEncoding::Short),
            ("a_ushort", FieldEncoding::Ushort),
            ("a_long", FieldEncoding::Int),
            ("a_ulong", FieldEncoding::Uint),
            ("a_long_long", FieldEncoding::Long),
            ("a_ulong_long", FieldEncoding::Ulong),
            ("a_float", FieldEncoding::Float),
            ("a_double", FieldEncoding::Double),
            ("a_long_double", FieldEncoding::LongDouble),
            ("a_char", FieldEncoding::Char),
            ("a_wchar", FieldEncoding::WChar),
            ("a_string", FieldEncoding::Str),
            ("a_wstring", FieldEncoding::WStr),
        ]
    );
}

#[test]
fn an_octet_sequence_is_binary_and_a_long_sequence_is_a_list() {
    let ast = model();
    let plan = plan(&ast, "probe::Compounds");
    let by_name: BTreeMap<&str, &FieldEncoding> = plan
        .fields
        .iter()
        .map(|field| (field.name.as_str(), &field.encoding))
        .collect();
    // §8.1.2 gives an octet sequence its own row: Base64, not an array
    // of numbers.
    assert_eq!(by_name["blob"], &FieldEncoding::Binary);
    assert_eq!(
        by_name["numbers"],
        &FieldEncoding::Sequence(Box::new(FieldEncoding::Int))
    );
    assert_eq!(
        by_name["grid"],
        &FieldEncoding::Array(Box::new(FieldEncoding::Int))
    );
    assert_eq!(
        by_name["lookup"],
        &FieldEncoding::Map(Box::new(FieldEncoding::Str), Box::new(FieldEncoding::Int))
    );
    assert!(matches!(by_name["tint"], FieldEncoding::Enumeration { .. }));
}

#[test]
fn the_body_form_follows_the_types_extensibility() {
    let ast = model();
    // §7.2.2 — a positional list for the forms whose member set does
    // not move, and §7.2.2.5's map for the one that does.
    assert_eq!(plan(&ast, "probe::Primitives").body, BodyForm::List);
    assert_eq!(plan(&ast, "probe::Growing").body, BodyForm::List);
    assert_eq!(plan(&ast, "probe::Evolving").body, BodyForm::Map);
    assert_eq!(plan(&ast, "probe::Choice").body, BodyForm::Union);
}

#[test]
fn a_mutable_body_is_keyed_by_the_declared_member_ids() {
    let ast = model();
    let plan = plan(&ast, "probe::Evolving");
    let ids: Vec<u32> = plan.fields.iter().map(|field| field.id).collect();
    assert_eq!(ids, vec![10, 20]);
    let sample = vec![
        Some(SampleValue::Int(1)),
        Some(SampleValue::Str("two".to_string())),
    ];
    let value = described_composite(
        &PlanSet::default(),
        &plan,
        &sample,
        EncodeOptions::default(),
    )
    .expect("the encoding");
    let AmqpExtValue::Described { value, .. } = value else {
        panic!("a described composite");
    };
    assert_eq!(
        *value,
        AmqpExtValue::Map(vec![
            (AmqpExtValue::Uint(10), AmqpExtValue::Int(1)),
            (AmqpExtValue::Uint(20), AmqpExtValue::Str("two".to_string())),
        ])
    );
}

#[test]
fn a_union_plans_its_discriminator_and_then_its_branches() {
    let ast = model();
    let plan = plan(&ast, "probe::Choice");
    let names: Vec<&str> = plan.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["discriminator", "n", "s"]);
    // §7.2.3 — the two-element list, and §8.1.2's two-key object.
    let sample = vec![
        Some(SampleValue::Int(2)),
        None,
        Some(SampleValue::Str("live".to_string())),
    ];
    let value = described_composite(
        &PlanSet::default(),
        &plan,
        &sample,
        EncodeOptions::default(),
    )
    .expect("the encoding");
    let AmqpExtValue::Described { value, .. } = value else {
        panic!("a described composite");
    };
    assert_eq!(
        *value,
        AmqpExtValue::List(vec![
            AmqpExtValue::Int(2),
            AmqpExtValue::Str("live".to_string()),
        ])
    );
    assert_eq!(
        json_document(&PlanSet::default(), &plan, &sample).expect("json"),
        "{\"discriminator\":2,\"value\":\"live\"}"
    );
}

#[test]
fn a_type_outside_the_mapping_is_named_rather_than_encoded_as_something_else() {
    let ast = model();
    // §7.1: "unmapped IDL types SHALL cause the encoder to raise an
    // UnsupportedType error." A bitmask has no AMQP code.
    let id = *types_by_name(&ast)
        .get("probe::WithBitmask")
        .expect("probe::WithBitmask");
    match plan_of(&ast, id, &[0xAB; 14], false) {
        Err(PlanError::UnsupportedType(name)) => {
            assert!(name.contains("Standalone"), "{name}");
        }
        other => panic!("expected an UnsupportedType refusal, got {other:?}"),
    }
}

#[test]
fn a_plan_of_something_that_is_not_a_composite_is_refused() {
    let ast = model();
    let id = *types_by_name(&ast)
        .get("probe::Colour")
        .expect("probe::Colour");
    assert!(matches!(
        plan_of(&ast, id, &[0xAB; 14], false),
        Err(PlanError::NotAComposite(_))
    ));
}

#[test]
fn the_two_descriptor_forms_come_from_the_same_identifier() {
    let ast = model();
    let id = *types_by_name(&ast)
        .get("probe::Growing")
        .expect("probe::Growing");
    let identifier = [
        0x01_u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
    ];
    let truncated = plan_of(&ast, id, &identifier, false).expect("truncated");
    let full = plan_of(&ast, id, &identifier, true).expect("full");
    // §7.2.1.1 — the first eight octets as a big-endian ulong.
    assert_eq!(
        truncated.descriptor,
        Descriptor::Ulong(0x0102_0304_0506_0708)
    );
    // §7.2.1.2 — `dds:type:` and the lower-case hexadecimal.
    assert_eq!(
        full.descriptor,
        Descriptor::Symbol("dds:type:0102030405060708090a0b0c0d0e".to_string())
    );
}

#[test]
fn a_sample_nested_past_the_bound_is_refused_by_name() {
    // A recursive IDL type gives the plan graph a cycle: `Recursive`
    // has a member of its own type, so `value_of` and `json_value`
    // follow the composite chain for as long as the sample nests. Only
    // `MAX_SAMPLE_DEPTH` stops that walk, and it must stop it with an
    // error that names where it stood rather than with a stack
    // overflow.
    let ast = model();
    let mut recursive = plan(&ast, "probe::Growing");
    recursive.scoped_name = "probe::Recursive".to_string();
    recursive.fields[0].encoding = FieldEncoding::Composite("probe::Recursive".to_string());
    let plans = PlanSet::new(vec![recursive.clone()]);

    // Deeper than the bound, and bottoming out on a value the plan does
    // not admit: reaching the bottom would be `Mismatch`, so `TooDeep`
    // is proof the walk stopped where it says it does.
    let mut sample = vec![Some(SampleValue::Int(5))];
    for _ in 0..200 {
        sample = vec![Some(SampleValue::Composite(sample))];
    }

    assert!(
        matches!(
            described_composite(&plans, &recursive, &sample, EncodeOptions::default()),
            Err(EncodeError::TooDeep(_))
        ),
        "the described-composite walk must refuse a sample past the bound"
    );
    assert!(
        matches!(
            json_document(&plans, &recursive, &sample),
            Err(EncodeError::TooDeep(_))
        ),
        "the JSON walk must refuse a sample past the bound"
    );
}

#[test]
fn a_nested_composite_carries_its_own_descriptor() {
    // §7.2.2.4 — "Nested composite types SHALL themselves be encoded as
    // described composites recursively." The nested value is a
    // `Described` of its own and not a bare list.
    let ast = model();
    let inner = plan(&ast, "probe::Growing");
    let mut outer = plan(&ast, "probe::Growing");
    outer.scoped_name = "probe::Outer".to_string();
    outer.descriptor = Descriptor::Ulong(0x1111_1111_1111_1111);
    outer.fields[0].encoding = FieldEncoding::Composite("probe::Growing".to_string());
    let plans = PlanSet::new(vec![inner, outer.clone()]);
    let sample = vec![Some(SampleValue::Composite(vec![Some(SampleValue::Int(
        5,
    ))]))];
    let value = described_composite(&plans, &outer, &sample, EncodeOptions::default())
        .expect("the encoding");
    let AmqpExtValue::Described { descriptor, value } = value else {
        panic!("a described composite");
    };
    assert_eq!(descriptor, 0x1111_1111_1111_1111);
    let AmqpExtValue::List(items) = *value else {
        panic!("a list body");
    };
    assert!(
        matches!(items.first(), Some(AmqpExtValue::Described { .. })),
        "the nested composite must carry its own descriptor: {items:?}"
    );
}
