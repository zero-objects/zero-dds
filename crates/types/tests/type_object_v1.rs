// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `TypeObject` of XTypes 1.1 Annex B (`TypeObjectV1`) and its `_TypeId`
//! (`TypeIdV1`), the optional members `type` (0x0072) and `type_id` (0x0069)
//! of the built-in topic data of XTypes 1.3 Annex D.
//!
//! The byte vectors in `a_minimal_object_has_the_octets_the_rules_give` and
//! `a_type_id_has_the_octets_of_its_union` are derived by hand from XTypes
//! 1.1 §7.4.1 and Annex B; the id of a type is checked against the MD5 of its
//! big-endian serialization, as §7.3.4.1.2 describes.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::from_ast::{Form, register_all, type_identifier};
use zerodds_types::hash::compute_complete_hash;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::{PrimitiveKind, TypeIdentifier};
use zerodds_types::type_object::common::{
    AnnotationParameterValue, AppliedAnnotation, AppliedAnnotationParameter, NameHash,
    OptionalAppliedAnnotationSeq,
};
use zerodds_types::type_object::complete::{
    CompleteAnnotationHeader, CompleteAnnotationParameter, CompleteAnnotationType,
    CompleteTypeObject,
};
use zerodds_types::type_object::flags::AnnotationParameterFlag;
use zerodds_types::type_object::flags::AnnotationTypeFlag;
use zerodds_types::type_object::minimal::CommonAnnotationParameter;
use zerodds_types::type_object_v1::convert::{from_complete, to_complete};
use zerodds_types::type_object_v1::{
    LibraryElement, TypeId, TypeLibrary, TypeObjectV1, V1Error, kind,
};

const IDL: &str = r#"
module Sensors {
    enum Mode { OFF, ON, AUTO };
    bitmask Flags { F0, F1, F2 };
    typedef long Counter;
    @mutable struct Base { @key long id; };
    @final struct Inner { double v; string<8> tag; };
    @mutable struct Reading : Base {
        Mode state;
        @optional Inner part;
        sequence<Inner, 5> parts;
        long grid[2][3];
        Counter count;
        map<long, string> labels;
        Flags bits;
        wstring<4> w;
    };
    union Choice switch (long) {
        case 1: long a;
        case 2:
        case 3: Inner b;
        default: octet c;
    };
    struct Holder { Choice pick; Reading data; };
};
"#;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "v1.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_node(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    let segments: Vec<String> = name.split("::").map(str::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

fn registry_of(ast: &Ast) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    let skipped = register_all(ast, &mut registry).expect("the registry fills");
    assert!(skipped.is_empty(), "{skipped:?}");
    registry
}

fn root_of(ast: &Ast, name: &str) -> TypeIdentifier {
    type_identifier(ast, type_node(ast, name), Form::Complete).expect("the identifier")
}

#[test]
fn a_minimal_object_has_the_octets_the_rules_give() {
    // TypeObject is MUTABLE: `library` is member 0, `the_type` member 1, a
    // sentinel ends the list. An empty library is a count of zero; the one
    // `_TypeId` of `long` is its kind and its primitive id, two shorts.
    let object = TypeObjectV1 {
        library: TypeLibrary::default(),
        the_type: vec![TypeId::Primitive(kind::INT_32_TYPE)],
    };
    let bytes = object.to_bytes_le().expect("serializes");
    let expected_head: [u8; 20] = [
        0x00, 0x00, 0x04, 0x00, // member 0, four octets
        0x00, 0x00, 0x00, 0x00, //   an empty sequence
        0x01, 0x00, 0x08, 0x00, // member 1, eight octets
        0x01, 0x00, 0x00, 0x00, //   one element
        0x05, 0x00, 0x05, 0x00, //   INT_32_TYPE, primitive id 5
    ];
    assert_eq!(&bytes[..20], &expected_head);
    // The sentinel: the id PID_LIST_END (0x3F02) under whichever flag bits the
    // framing sets, and a length of zero.
    assert_eq!(bytes.len(), 24);
    assert_eq!(
        u16::from_le_bytes([bytes[20], bytes[21]]) & 0x3FFF,
        0x3F02,
        "the list ends with the sentinel"
    );
    assert_eq!(&bytes[22..24], &[0, 0]);
    assert_eq!(
        TypeObjectV1::from_bytes(&bytes, false).expect("reads back"),
        object
    );
}

#[test]
fn a_type_id_has_the_octets_of_its_union() {
    // `_TypeId` is an extensible union over a short: a primitive kind selects
    // the primitive id, every other value the 64-bit id, aligned to eight.
    let primitive = TypeId::Primitive(kind::UINT_16_TYPE);
    assert_eq!(primitive.to_bytes_le().unwrap(), [0x04, 0x00, 0x04, 0x00]);
    assert_eq!(primitive.to_bytes_be().unwrap(), [0x00, 0x04, 0x00, 0x04]);
    let constructed = TypeId::Constructed {
        kind: kind::STRUCTURE_TYPE,
        id: 0x0102_0304_0506_0708,
    };
    assert_eq!(
        constructed.to_bytes_le().unwrap(),
        [
            0x16, 0x00, 0, 0, 0, 0, 0, 0, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01
        ]
    );
    assert_eq!(
        constructed.to_bytes_be().unwrap(),
        [
            0x00, 0x16, 0, 0, 0, 0, 0, 0, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08
        ]
    );
    for id in [primitive, constructed, TypeId::NONE] {
        for big in [false, true] {
            let bytes = if big {
                id.to_bytes_be().unwrap()
            } else {
                id.to_bytes_le().unwrap()
            };
            assert_eq!(TypeId::from_bytes(&bytes, big).unwrap(), id);
        }
    }
}

#[test]
fn a_library_made_of_every_kind_survives_both_byte_orders() {
    let ast = model(IDL);
    let registry = registry_of(&ast);
    let object = from_complete(&root_of(&ast, "Sensors::Holder"), &registry).expect("converts");
    // Every kind of the library is in it.
    let mut kinds = std::collections::BTreeSet::new();
    fn walk(library: &TypeLibrary, kinds: &mut std::collections::BTreeSet<i16>) {
        for element in &library.element {
            match element {
                LibraryElement::Module(m) => walk(&m.library, kinds),
                other => {
                    kinds.insert(other.type_kind().expect("a type"));
                }
            }
        }
    }
    walk(&object.library, &mut kinds);
    for expected in [
        kind::ENUMERATION_TYPE,
        kind::BITSET_TYPE,
        kind::ALIAS_TYPE,
        kind::ARRAY_TYPE,
        kind::SEQUENCE_TYPE,
        kind::STRING_TYPE,
        kind::MAP_TYPE,
        kind::UNION_TYPE,
        kind::STRUCTURE_TYPE,
    ] {
        assert!(
            kinds.contains(&expected),
            "kind {expected} is missing: {kinds:?}"
        );
    }
    for big in [false, true] {
        let bytes = if big {
            object.to_bytes_be().expect("serializes")
        } else {
            object.to_bytes_le().expect("serializes")
        };
        let back = TypeObjectV1::from_bytes(&bytes, big).expect("reads back");
        assert_eq!(back, object, "big endian: {big}");
    }
}

#[test]
fn the_id_of_a_type_is_the_low_64_bits_of_the_md5_of_its_serialization() {
    let ast = model(IDL);
    let registry = registry_of(&ast);
    let object = from_complete(&root_of(&ast, "Sensors::Holder"), &registry).expect("converts");
    let mut checked = 0;
    fn each(library: &TypeLibrary, f: &mut impl FnMut(&LibraryElement)) {
        for element in &library.element {
            match element {
                LibraryElement::Module(m) => each(&m.library, f),
                other => f(other),
            }
        }
    }
    each(&object.library, &mut |element| {
        let mut copy = element.clone();
        copy.base_mut().expect("a type").property.type_id = TypeId::NONE;
        let bytes = TypeLibrary::element_bytes(&copy, true).expect("serializes");
        let digest = zerodds_wire::md5(&bytes);
        let mut low = [0u8; 8];
        low.copy_from_slice(&digest[8..16]);
        let expected = TypeId::Constructed {
            kind: element.type_kind().expect("a type"),
            id: u64::from_be_bytes(low),
        };
        // The type that refers to itself none; the ids here are the hashes.
        assert_eq!(
            element.base().expect("a type").property.type_id,
            expected,
            "{}",
            element.base().unwrap().property.name
        );
        checked += 1;
    });
    assert!(checked >= 9, "{checked} types checked");
    // The root is in the library under the id `the_type` names.
    assert!(object.library.find(object.the_type[0]).is_some());
}

#[test]
fn two_types_that_differ_have_two_ids_and_a_type_keeps_its_own() {
    let ast = model(IDL);
    let registry = registry_of(&ast);
    let a = from_complete(&root_of(&ast, "Sensors::Inner"), &registry).expect("converts");
    let b = from_complete(&root_of(&ast, "Sensors::Reading"), &registry).expect("converts");
    let again = from_complete(&root_of(&ast, "Sensors::Inner"), &registry).expect("converts");
    assert_ne!(a.the_type, b.the_type);
    assert_eq!(a, again);
}

#[test]
fn a_topic_type_comes_back_as_the_complete_objects_it_was() {
    let ast = model(IDL);
    let registry = registry_of(&ast);
    let mut differences = Vec::new();
    for name in [
        "Sensors::Mode",
        "Sensors::Flags",
        "Sensors::Counter",
        "Sensors::Inner",
        "Sensors::Choice",
        "Sensors::Base",
        "Sensors::Reading",
        "Sensors::Holder",
    ] {
        let root = root_of(&ast, name);
        let v1 = from_complete(&root, &registry).expect("converts to V1");
        // Over the wire in between.
        let bytes = v1.to_bytes_le().expect("serializes");
        let v1 = TypeObjectV1::from_bytes(&bytes, false).expect("reads back");
        let converted = to_complete(&v1).expect("converts back");
        if converted.roots != vec![root.clone()] {
            differences.push(format!(
                "{name}: the root is {:?}, not {root:?}",
                converted.roots
            ));
        }
        // Every object the conversion made is the object the registry has
        // under that hash, or says where it differs from the one it replaces.
        for (hash, object) in &converted.types {
            if registry.get_complete(hash) != Some(object) {
                let same_name = registry.iter_completes().find(|(_, o)| {
                    format!("{o:?}")
                        .split("type_name: \"")
                        .nth(1)
                        .and_then(|r| r.split('"').next())
                        == format!("{object:?}")
                            .split("type_name: \"")
                            .nth(1)
                            .and_then(|r| r.split('"').next())
                });
                differences.push(format!(
                    "{name}: the object of {hash:02x?} is not in the registry:\n  made:     {object:?}\n  original: {:?}",
                    same_name.map(|(_, o)| o)
                ));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn an_annotation_and_its_application_cross_both_ways() {
    let annotation = CompleteTypeObject::Annotation(CompleteAnnotationType {
        annotation_flag: AnnotationTypeFlag(0),
        header: CompleteAnnotationHeader {
            annotation_name: String::from("Meta::Tag"),
        },
        member_seq: vec![
            CompleteAnnotationParameter {
                common: CommonAnnotationParameter {
                    member_flags: AnnotationParameterFlag(0),
                    member_type_id: TypeIdentifier::String8Small { bound: 0 },
                },
                name: String::from("label"),
                default_value: AnnotationParameterValue::String8(String::new()),
            },
            CompleteAnnotationParameter {
                common: CommonAnnotationParameter {
                    member_flags: AnnotationParameterFlag(0),
                    member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                },
                name: String::from("weight"),
                default_value: AnnotationParameterValue::Int32(1),
            },
        ],
    });
    let annotation_hash = compute_complete_hash(&annotation).expect("hash");
    let mut tagged = TypeObjectBuilder::struct_type("Meta::Tagged")
        .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .build_complete();
    let mut params = vec![
        AppliedAnnotationParameter {
            paramname_hash: NameHash::from_name("label"),
            value: AnnotationParameterValue::String8(String::from("hot")),
        },
        AppliedAnnotationParameter {
            paramname_hash: NameHash::from_name("weight"),
            value: AnnotationParameterValue::Int32(7),
        },
    ];
    params.sort_by_key(|p| p.paramname_hash);
    tagged.header.detail.ann_custom = OptionalAppliedAnnotationSeq(Some(vec![AppliedAnnotation {
        annotation_typeid: TypeIdentifier::EquivalenceHashComplete(annotation_hash),
        param_seq: params,
    }]));
    let tagged = CompleteTypeObject::Struct(tagged);
    let tagged_hash = compute_complete_hash(&tagged).expect("hash");
    let mut registry = TypeRegistry::new();
    registry.insert_complete(annotation_hash, annotation);
    registry.insert_complete(tagged_hash, tagged.clone());
    let root = TypeIdentifier::EquivalenceHashComplete(tagged_hash);

    let v1 = from_complete(&root, &registry).expect("converts to V1");
    let found = v1.library.find(v1.the_type[0]).expect("the root").1;
    let LibraryElement::Structure(s) = found else {
        panic!("a structure expected: {found:?}");
    };
    assert_eq!(
        s.base.annotation.len(),
        1,
        "the applied annotation is carried"
    );
    assert_eq!(s.base.annotation[0].member.len(), 2);

    let converted = to_complete(&v1).expect("converts back");
    assert_eq!(converted.roots, vec![root]);
    let back = converted
        .types
        .iter()
        .find(|(h, _)| *h == tagged_hash)
        .map(|(_, o)| o)
        .expect("the struct is among the objects");
    assert_eq!(back, &tagged);
}

#[test]
fn what_annex_b_has_no_form_for_is_refused_by_name() {
    // int8: XTypes 1.1 has no 8-bit signed integer.
    let ast = model("module M { struct S { int8 small; }; };");
    let registry = registry_of(&ast);
    let error = from_complete(&root_of(&ast, "M::S"), &registry).expect_err("int8");
    assert!(matches!(error, V1Error::Unrepresentable(_)), "{error}");
    assert!(error.to_string().contains("int8"), "{error}");

    // A bit set with bit fields.
    let ast =
        model("module M { bitset B { bitfield<3> a; bitfield<5> b; }; struct S { B bits; }; };");
    let registry = registry_of(&ast);
    let error = from_complete(&root_of(&ast, "M::S"), &registry).expect_err("bit set");
    assert!(matches!(error, V1Error::Unrepresentable(_)), "{error}");

    // A minimal identifier names no complete object.
    let registry = TypeRegistry::new();
    let error = from_complete(
        &TypeIdentifier::EquivalenceHashMinimal(zerodds_types::EquivalenceHash([1; 14])),
        &registry,
    )
    .expect_err("minimal");
    assert!(matches!(error, V1Error::Unrepresentable(_)), "{error}");

    // A hash the registry does not hold.
    let error = from_complete(
        &TypeIdentifier::EquivalenceHashComplete(zerodds_types::EquivalenceHash([2; 14])),
        &registry,
    )
    .expect_err("unknown");
    assert!(matches!(error, V1Error::Unresolved(_)), "{error}");
}

#[test]
fn a_type_that_refers_to_itself_has_no_form() {
    // The id of a type is the hash of a serialization that contains the ids
    // of the types it uses: a cycle has none.
    let ast = model("module M { struct Node { long v; sequence<Node> next; }; };");
    let registry = registry_of(&ast);
    let error = from_complete(&root_of(&ast, "M::Node"), &registry).expect_err("a cycle");
    assert!(matches!(error, V1Error::Unrepresentable(_)), "{error}");
    assert!(error.to_string().contains("refers to itself"), "{error}");
}

#[test]
fn a_member_this_reader_does_not_know_is_skipped_unless_it_must_be_understood() {
    let object = TypeObjectV1 {
        library: TypeLibrary::default(),
        the_type: vec![TypeId::Primitive(kind::BOOLEAN_TYPE)],
    };
    let bytes = object.to_bytes_le().unwrap();
    let (head, sentinel) = bytes.split_at(bytes.len() - 4);
    let with = |pid: u16| {
        let mut out = head.to_vec();
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&[9, 9, 9, 9]);
        out.extend_from_slice(sentinel);
        out
    };
    // Member 77, not defined: a reader skips it (§7.4.1.2.3: members may
    // come in any order and unknown ones carry their length).
    assert_eq!(
        TypeObjectV1::from_bytes(&with(77), false).expect("skipped"),
        object
    );
    // The same with FLAG_MUST_UNDERSTAND (bit 14): the sample is refused.
    let error = TypeObjectV1::from_bytes(&with(77 | 0x4000), false).expect_err("must understand");
    assert!(matches!(error, V1Error::Invalid(_)), "{error}");
}

#[test]
fn a_truncated_or_ill_formed_object_is_an_error_not_a_guess() {
    let ast = model(IDL);
    let registry = registry_of(&ast);
    let object = from_complete(&root_of(&ast, "Sensors::Holder"), &registry).expect("converts");
    let bytes = object.to_bytes_le().unwrap();
    for cut in [0, 3, bytes.len() / 2, bytes.len() - 1] {
        assert!(
            TypeObjectV1::from_bytes(&bytes[..cut], false).is_err(),
            "{cut} octets are not an object"
        );
    }
    // A sequence count past what the stream holds.
    let mut bad = bytes.clone();
    bad[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(TypeObjectV1::from_bytes(&bad, false).is_err());
}

#[test]
fn a_name_past_its_bound_is_refused_on_writing() {
    let mut object = TypeObjectV1::default();
    let ast = model("module M { struct S { long a; }; };");
    let registry = registry_of(&ast);
    let v1 = from_complete(&root_of(&ast, "M::S"), &registry).unwrap();
    object.library = v1.library;
    if let Some(LibraryElement::Module(m)) = object.library.element.first_mut() {
        if let Some(LibraryElement::Structure(s)) = m.library.element.first_mut() {
            s.base.property.name = "x".repeat(300);
        }
    }
    let error = object.to_bytes_le().expect_err("ObjectName holds 256");
    assert!(matches!(error, V1Error::Invalid(_)), "{error}");
}
