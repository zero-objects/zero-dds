// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeObjects the Enhanced Service Mapping announces for its topic types
//! (DDS-RPC 1.0 §7.5.1.2; XTypes 1.3 §7.3): `${I}_Request` and `${I}_Reply`
//! are mutable structures whose members are all optional and carry the HASH of
//! the operation name as member id, and the objects they name by hash are
//! delivered with them.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_lang_rules::adapter::interface::{self, ServiceMapping, member_id};
use zerodds_lang_rules::emit::interface_rust::type_information::service_type_objects;
use zerodds_types::{CompleteTypeObject, TypeIdentifier, TypeObject, compute_hash};

const IDL: &str = "
    module robot {
      exception TooFast { long limit; };
      interface Base { long ident(); };
      interface Calc : Base {
        long add(in long a, in long b) raises (TooFast);
        @Mutable void mix(in long x, in long y);
      };
    };
";

fn objects() -> zerodds_lang_rules::emit::interface_rust::type_information::ServiceTypeObjects {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calc.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let services = interface::services(&ast).expect("services");
    let calc = services
        .into_iter()
        .find(|s| s.name == "Calc")
        .expect("Calc");
    let calc = interface::Service {
        mapping: ServiceMapping::Enhanced,
        ..calc
    };
    let model =
        interface::enhanced_service(&ast, &calc, calc.mapping.empty_style()).expect("the model");
    service_type_objects(&ast, &calc, &model).expect("the objects")
}

fn struct_of(bytes: &[u8]) -> zerodds_types::type_object::complete::CompleteStructType {
    match TypeObject::from_bytes_le(bytes).expect("decodes") {
        TypeObject::Complete(CompleteTypeObject::Struct(s)) => s,
        other => panic!("not a complete structure: {other:?}"),
    }
}

#[test]
fn the_request_is_a_mutable_choice_with_the_hash_of_the_name_as_member_id() {
    let request = struct_of(&objects().request.object);
    assert_eq!(request.header.detail.type_name, "robot::Calc_Request");
    assert!(
        request.struct_flags.0 & zerodds_types::type_object::flags::StructTypeFlag::IS_MUTABLE != 0,
        "mutable"
    );
    let names: Vec<_> = request
        .member_seq
        .iter()
        .map(|m| m.detail.name.clone())
        .collect();
    assert_eq!(names, ["ident", "add", "mix"]);
    for member in &request.member_seq {
        assert_eq!(
            member.common.member_id,
            member_id(&member.detail.name),
            "{}",
            member.detail.name
        );
        assert!(
            member.common.member_flags.0
                & zerodds_types::type_object::flags::StructMemberFlag::IS_OPTIONAL
                != 0,
            "{} is optional",
            member.detail.name
        );
    }
}

#[test]
fn what_the_members_name_by_hash_is_delivered_with_the_object() {
    let objects = objects();
    let request = struct_of(&objects.request.object);
    let hashes: Vec<[u8; 14]> = objects
        .request
        .dependencies
        .iter()
        .map(|bytes| {
            compute_hash(&TypeObject::from_bytes_le(bytes).unwrap())
                .unwrap()
                .0
        })
        .collect();
    for member in &request.member_seq {
        let TypeIdentifier::EquivalenceHashComplete(hash) = &member.common.member_type_id else {
            panic!("{}: {:?}", member.detail.name, member.common.member_type_id);
        };
        assert!(
            hashes.contains(&hash.0),
            "{} is not delivered",
            member.detail.name
        );
    }
    // The reply has `remoteEx`, an enumeration, and a `Result` per operation.
    let reply = struct_of(&objects.reply.object);
    assert_eq!(reply.member_seq.len(), 4);
    assert_eq!(reply.member_seq[3].detail.name, "remoteEx");
}

#[test]
fn a_mutable_operation_has_mutable_parameter_structures() {
    let objects = objects();
    let mix_in = objects
        .request
        .dependencies
        .iter()
        .filter_map(|bytes| struct_of_or_none(bytes))
        .find(|s| s.header.detail.type_name == "robot::Calc_mix_In")
        .expect("Calc_mix_In is delivered");
    assert!(
        mix_in.struct_flags.0 & zerodds_types::type_object::flags::StructTypeFlag::IS_MUTABLE != 0
    );
}

fn struct_of_or_none(
    bytes: &[u8],
) -> Option<zerodds_types::type_object::complete::CompleteStructType> {
    match TypeObject::from_bytes_le(bytes).ok()? {
        TypeObject::Complete(CompleteTypeObject::Struct(s)) => Some(s),
        _ => None,
    }
}

#[test]
fn the_java_binding_names_the_descriptions_of_its_topic_types() {
    use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
    use zerodds_lang_cst::Language;
    let parsed = zerodds_idl_parse::parse(IDL);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calc.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let text = zerodds_lang_rules::generate::source_with_mapping(
        &ast,
        Language::Java,
        key,
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    )
    .expect("the Java binding");
    assert_eq!(
        text.matches("TypeDescription.of(").count(),
        8,
        "a request and a reply description in the requester and in the replier params of each of the two interfaces"
    );
    assert!(text.contains(".typeDescriptions("));
}

/// XTypes 1.3 §7.6.3.3.4: a participant answers a TypeLookup request for any
/// identifier it announced in a TypeInformation. The information of a topic
/// type names MINIMAL identifiers too, and the Java runtime serves only the
/// objects its description holds, so the minimal object of the root and of
/// every minimal dependency is delivered, with references among them.
#[test]
fn every_minimal_identifier_the_information_announces_has_its_object() {
    use zerodds_types::type_information::TypeInformation;
    let objects = objects();
    for topic in [&objects.request, &objects.reply] {
        let information =
            TypeInformation::from_bytes_le(topic.information.as_ref().expect("information"))
                .expect("the information decodes");
        let delivered: Vec<[u8; 14]> = topic
            .minimal
            .iter()
            .map(|bytes| {
                let object = TypeObject::from_bytes_le(bytes).unwrap();
                assert!(matches!(object, TypeObject::Minimal(_)), "{object:?}");
                compute_hash(&object).unwrap().0
            })
            .collect();
        let announced = std::iter::once(&information.minimal.typeid_with_size)
            .chain(information.minimal.dependent_typeids.iter());
        for with_size in announced {
            let TypeIdentifier::EquivalenceHashMinimal(hash) = &with_size.type_id else {
                panic!("{:?}", with_size.type_id);
            };
            assert!(delivered.contains(&hash.0), "{hash:?} is not delivered");
        }
        let first = 1 + topic.dependencies.len();
        assert_eq!(topic.minimal_references.len(), topic.minimal.len());
        for indexes in &topic.minimal_references {
            for at in indexes {
                assert!(
                    (first..first + topic.minimal.len()).contains(at),
                    "a minimal object names {at}, outside the minimal objects"
                );
            }
        }
    }
}
