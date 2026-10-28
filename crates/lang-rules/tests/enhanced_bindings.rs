// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The topic types the generators write for the Enhanced Service Mapping,
//! held against the clause (DDS-RPC 1.0 §7.5.1.2) — its non-normative
//! example of §7.5.1.2.6 and the rules around it — for the model, for
//! C++, for Java and for Rust.
//!
//! The structures are read off the generators' declarations (the
//! `zerodds_lang_cst` trees), not off rendered text, so what is compared
//! is the declaration: which structure, which members, in which order, of
//! which type.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_cst::{cpp, java, rust};
use zerodds_lang_rules::adapter::interface::{
    self, BindingError, EmptyOperationStyle, EnhancedMemberType, ServiceMapping,
};
use zerodds_lang_rules::emit::{interface_cpp, interface_java, interface_rust};

/// The hierarchy of §7.5.1.2.6, with the exception of §7.5.1.2.3's
/// example added to `add`.
const CALCULATOR: &str = "
    exception TooFast { long limit; };
    interface Adder { long add(in long a, in long b) raises (TooFast); };
    interface Subtractor { long sub(in long a, in long b); };
    interface Calculator : Adder, Subtractor {
        void on();
        void off();
    };
";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// A structure of the clause: name, annotations, and `(member, type)`
/// in order, types as the clause names them.
struct Expected {
    name: &'static str,
    marks: &'static str,
    members: &'static [(&'static str, &'static str)],
}

/// §7.5.1.2.2 to §7.5.1.2.6 for `Calculator`, `Adder` and `Subtractor`.
/// The `dummy` member of `Calculator_on_In` is `UnusedMember` (§7.5.1.1.4
/// rule 3c, which §7.5.1.2.2 takes over).
const CLAUSE: &[Expected] = &[
    Expected {
        name: "Adder_add_In",
        marks: "@Autoid",
        members: &[("a", "long"), ("b", "long")],
    },
    Expected {
        name: "Adder_add_Out",
        marks: "@Autoid",
        members: &[("return_", "long")],
    },
    Expected {
        name: "Adder_add_Result",
        marks: "@Choice @Autoid",
        members: &[("result", "Adder_add_Out"), ("toofast_ex", "TooFast")],
    },
    Expected {
        name: "Adder_Request",
        marks: "@Choice @Autoid",
        members: &[("add", "Adder_add_In")],
    },
    Expected {
        name: "Adder_Reply",
        marks: "@Choice @Autoid",
        members: &[
            ("add", "Adder_add_Result"),
            ("remoteEx", "RemoteExceptionCode_t"),
        ],
    },
    Expected {
        name: "Subtractor_sub_In",
        marks: "@Autoid",
        members: &[("a", "long"), ("b", "long")],
    },
    Expected {
        name: "Subtractor_sub_Out",
        marks: "@Autoid",
        members: &[("return_", "long")],
    },
    Expected {
        name: "Subtractor_sub_Result",
        marks: "@Choice @Autoid",
        members: &[("result", "Subtractor_sub_Out")],
    },
    Expected {
        name: "Calculator_on_In",
        marks: "@Autoid",
        members: &[("dummy", "UnusedMember")],
    },
    Expected {
        name: "Calculator_on_Out",
        marks: "@Autoid",
        members: &[("dummy", "UnusedMember")],
    },
    Expected {
        name: "Calculator_on_Result",
        marks: "@Choice @Autoid",
        members: &[("result", "Calculator_on_Out")],
    },
    Expected {
        name: "Calculator_off_Result",
        marks: "@Choice @Autoid",
        members: &[("result", "Calculator_off_Out")],
    },
    // The clause lists the inherited operations first.
    Expected {
        name: "Calculator_Request",
        marks: "@Choice @Autoid",
        members: &[
            ("add", "Adder_add_In"),
            ("sub", "Subtractor_sub_In"),
            ("on", "Calculator_on_In"),
            ("off", "Calculator_off_In"),
        ],
    },
    Expected {
        name: "Calculator_Reply",
        marks: "@Choice @Autoid",
        members: &[
            ("add", "Adder_add_Result"),
            ("sub", "Subtractor_sub_Result"),
            ("on", "Calculator_on_Result"),
            ("off", "Calculator_off_Result"),
            ("remoteEx", "RemoteExceptionCode_t"),
        ],
    },
];

fn type_name(ast: &Ast, type_: &EnhancedMemberType) -> String {
    match type_ {
        EnhancedMemberType::Struct(name) => name.clone(),
        EnhancedMemberType::Exception(raise) => raise.name.clone(),
        EnhancedMemberType::RemoteException => "RemoteExceptionCode_t".to_string(),
        EnhancedMemberType::Dummy => "UnusedMember".to_string(),
        EnhancedMemberType::Idl(id) => {
            let node = ast.type_(*id).expect("a type");
            match &node.kind {
                zerodds_ast::TypeKind::Primitive(zerodds_ast::PrimitiveKind::Int32) => {
                    "long".to_string()
                }
                other => format!("{other:?}"),
            }
        }
    }
}

fn marks(structure: &interface::EnhancedStruct) -> String {
    let mut out = Vec::new();
    if structure.choice {
        out.push("@Choice");
    }
    if structure.autoid {
        out.push("@Autoid");
    }
    if structure.empty {
        out.push("@Empty");
    }
    if structure.mutable {
        out.push("@Mutable");
    }
    out.join(" ")
}

/// Every structure of the model of `Calculator`, `Adder` and `Subtractor`.
fn structures(ast: &Ast) -> Vec<interface::EnhancedStruct> {
    let mut out = Vec::new();
    for name in ["Adder", "Subtractor", "Calculator"] {
        let (id, _) = ast
            .interfaces()
            .find(|(_, node)| node.name == name)
            .expect("the interface");
        let service = interface::service(ast, id).expect("the service");
        let model = interface::enhanced_service(ast, &service, EmptyOperationStyle::DummyMember)
            .expect("the model");
        for operation in &model.operations {
            if operation.interface != name {
                continue;
            }
            out.push(operation.in_struct.clone());
            out.push(operation.out_struct.clone());
            out.push(operation.result_struct.clone());
        }
        out.push(model.request);
        out.push(model.reply);
    }
    out
}

/// The model the generators read: §7.5.1.2's structures, as the clause
/// lists them.
#[test]
fn the_model_is_the_clause() {
    let ast = model(CALCULATOR);
    let structures = structures(&ast);
    for expected in CLAUSE {
        let found = structures
            .iter()
            .find(|structure| structure.name == expected.name)
            .unwrap_or_else(|| panic!("no structure {}", expected.name));
        assert_eq!(marks(found), expected.marks, "{}", expected.name);
        let members: Vec<(String, String)> = found
            .members
            .iter()
            .map(|member| (member.name.clone(), type_name(&ast, &member.type_)))
            .collect();
        let wanted: Vec<(String, String)> = expected
            .members
            .iter()
            .map(|(name, type_)| ((*name).to_string(), (*type_).to_string()))
            .collect();
        assert_eq!(members, wanted, "{}", expected.name);
    }
}

/// §7.5.1.2.1.2: the member id is the HASH of the member name.
#[test]
fn the_member_ids_are_the_hash_of_the_names() {
    let ast = model(CALCULATOR);
    for structure in structures(&ast) {
        for member in &structure.members {
            assert_eq!(
                member.member_id,
                (interface::operation_hash(&member.name) as u32) & 0x0FFF_FFFF,
                "{}.{}",
                structure.name,
                member.name
            );
        }
    }
}

fn cpp_classes(decls: &[cpp::Decl], out: &mut Vec<cpp::Class>) {
    for decl in decls {
        match decl {
            cpp::Decl::Namespace(namespace) => cpp_classes(&namespace.decls, out),
            cpp::Decl::Class(class) => out.push(class.clone()),
            _ => {}
        }
    }
}

fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The same comparison on the C++ declarations: each member of a choice
/// structure is a `std::optional<T>` (an `@optional` member, DDS-Cxx-PSM),
/// the others are the Basic mapping's fields.
#[test]
fn cpp_declares_the_structures_of_the_clause() {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = model(CALCULATOR);
    let decls = interface_cpp::cpp_interface_items_with(&ast, ServiceMapping::Enhanced)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut classes = Vec::new();
    cpp_classes(&decls, &mut classes);
    let fields = |name: &str| -> Vec<(String, String)> {
        let class = classes
            .iter()
            .find(|class| class.name.as_str() == name)
            .unwrap_or_else(|| panic!("no class {name}"));
        class
            .sections
            .iter()
            .flat_map(|section| &section.members)
            .filter_map(|member| match member {
                cpp::ClassMember::Field(field) => Some((
                    field.name.as_str().to_string(),
                    squash(&zerodds_lang_render::cpp::type_(&field.type_)),
                )),
                _ => None,
            })
            .collect()
    };
    for expected in CLAUSE {
        let wanted: Vec<(String, String)> = expected
            .members
            .iter()
            .map(|(name, type_)| {
                let rendered = if expected.marks.contains("@Choice") {
                    let spelled = match *type_ {
                        "RemoteExceptionCode_t" => "dds::rpc::RemoteExceptionCode_t",
                        other => other,
                    };
                    format!("std::optional<{spelled}>")
                } else {
                    match *type_ {
                        "long" => "std::int32_t".to_string(),
                        "UnusedMember" => "dds::rpc::UnusedMember".to_string(),
                        other => other.to_string(),
                    }
                };
                ((*name).to_string(), squash(&rendered))
            })
            .collect();
        assert_eq!(fields(expected.name), wanted, "{}", expected.name);
    }
    // The Basic mapping's `Call` union and the headers are not here.
    assert!(
        !classes
            .iter()
            .any(|class| class.name.as_str().ends_with("_Call")),
        "no Call union under the Enhanced mapping"
    );
    let request = fields("Calculator_Request");
    assert!(request.iter().all(|(name, _)| name != "header"));
}

fn java_classes(members: &[java::Member], out: &mut Vec<java::Class>) {
    for member in members {
        if let java::Member::Nested(decl) = member {
            if let java::TypeDecl::Class(class) = decl.as_ref() {
                out.push(class.clone());
                java_classes(&class.members, out);
            }
        }
    }
}

/// And on the Java declarations: a member of a choice structure is a
/// reference to its holder, `null` when absent.
#[test]
fn java_declares_the_structures_of_the_clause() {
    let ast = model(CALCULATOR);
    let decls = interface_java::java_interface_items_with(&ast, ServiceMapping::Enhanced)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut classes = Vec::new();
    for decl in &decls {
        if let java::TypeDecl::Class(class) = decl {
            java_classes(&class.members, &mut classes);
        }
    }
    let fields = |name: &str| -> Vec<(String, String)> {
        let class = classes
            .iter()
            .find(|class| class.name.as_str() == name)
            .unwrap_or_else(|| panic!("no class {name}"));
        class
            .members
            .iter()
            .filter_map(|member| match member {
                java::Member::Field(field) => Some((
                    field.name.as_str().to_string(),
                    squash(&zerodds_lang_render::java::type_(&field.type_)),
                )),
                _ => None,
            })
            .collect()
    };
    for expected in CLAUSE {
        let got = fields(expected.name);
        let names: Vec<&str> = got.iter().map(|(name, _)| name.as_str()).collect();
        let wanted: Vec<&str> = expected.members.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, wanted, "{}", expected.name);
        if expected.marks.contains("@Choice") {
            for ((_, rendered), (_, clause_type)) in got.iter().zip(expected.members) {
                let leaf = rendered.rsplit('.').next().unwrap();
                let wanted = match *clause_type {
                    "RemoteExceptionCode_t" => "RemoteExceptionCode",
                    other => other,
                };
                assert_eq!(leaf, wanted, "{}", expected.name);
            }
        }
    }
    assert!(
        !classes
            .iter()
            .any(|class| class.name.as_str().ends_with("_Call"))
    );
}

/// And on the Rust declarations: a choice structure is an `enum` with a
/// variant per member and one for a member the binding does not know, and
/// the `In`, `Out` structures are structures.
#[test]
fn rust_declares_the_structures_of_the_clause() {
    let ast = model(CALCULATOR);
    let items = interface_rust::rust_interface_pairs_with(&ast, ServiceMapping::Enhanced)
        .unwrap_or_else(|error| panic!("{error}"));
    let find_enum = |name: &str| -> Vec<String> {
        for (_, item) in &items {
            if let rust::Item::Enum(declared) = item {
                if declared.name.as_str() == name {
                    return declared
                        .variants
                        .iter()
                        .map(|variant| variant.name.as_str().to_string())
                        .collect();
                }
            }
        }
        panic!("no enum {name}");
    };
    let find_struct = |name: &str| -> Vec<String> {
        for (_, item) in &items {
            if let rust::Item::Struct(declared) = item {
                if declared.name.as_str() == name {
                    if let rust::StructBody::Named(fields) = &declared.body {
                        return fields
                            .iter()
                            .map(|field| field.name.as_str().to_string())
                            .collect();
                    }
                }
            }
        }
        panic!("no struct {name}");
    };
    // The variants are the members, in the clause's order, then the member
    // the binding does not know.
    assert_eq!(
        find_enum("Calculator_Request"),
        ["Add", "Sub", "On", "Off", "UnknownOp"]
    );
    assert_eq!(
        find_enum("Calculator_Reply"),
        ["Add", "Sub", "On", "Off", "RemoteEx", "UnknownOp"]
    );
    // and the variant for an exception member the binding does not know
    // (§7.5.2: REMOTE_EX_UNKNOWN_EXCEPTION at the client).
    assert_eq!(
        find_enum("Adder_add_Result"),
        ["Ok", "TooFast", "UnknownEx"]
    );
    assert_eq!(find_enum("Subtractor_sub_Result"), ["Ok", "UnknownEx"]);
    assert_eq!(find_struct("Adder_add_In"), ["a", "b"]);
    assert_eq!(find_struct("Adder_add_Out"), ["return_"]);
    assert_eq!(find_struct("Calculator_on_In"), ["dummy"]);
    // The Basic mapping's structures with a header are not here.
    for (_, item) in &items {
        if let rust::Item::Struct(declared) = item {
            assert_ne!(declared.name.as_str(), "Calculator_Request");
        }
    }
}

/// The Enhanced model of the interface called `name` in `idl`.
fn enhanced_of(idl: &str, name: &str) -> (Ast, Result<interface::EnhancedService, BindingError>) {
    let ast = model(idl);
    let (id, _) = ast
        .interfaces()
        .find(|(_, node)| node.name == name)
        .expect("the interface");
    let service = interface::service(&ast, id).expect("the service");
    let result = interface::enhanced_service(&ast, &service, EmptyOperationStyle::DummyMember);
    (ast, result)
}

/// §7.5.1.2.1.2: "The collisions of member-ids produced as a result of
/// @Autoid annotation must be detected." `m9852` and `m12254` hash to the
/// same 28-bit id, so the `In` structure of `op` cannot be built, and the
/// error names both members.
#[test]
fn two_parameters_with_one_member_id_are_an_error() {
    let (_, result) = enhanced_of(
        "interface Clash { void op(in long m9852, in long m12254); };",
        "Clash",
    );
    match result {
        Err(BindingError::Codec(reason)) => {
            assert!(reason.contains("m9852"), "{reason}");
            assert!(reason.contains("m12254"), "{reason}");
            assert!(reason.contains("Clash_op_In"), "{reason}");
        }
        other => panic!("{other:?}"),
    }
}

/// §7.5.1.1.5 rule 3a.i and 3b: the `Out` structure lists the `inout` and
/// `out` parameters in the order of the parameters, the return value last;
/// an `out` parameter named `return_` moves the return value to `return_1`.
#[test]
fn the_out_structure_orders_the_parameters_and_numbers_a_colliding_return() {
    let (_, result) = enhanced_of(
        "interface Mixer {
             void mix(in long a, inout long b, out double c);
             long odd(out long return_);
         };",
        "Mixer",
    );
    let model = result.expect("the model");
    let members = |operation: &str| -> Vec<String> {
        model
            .operations
            .iter()
            .find(|candidate| candidate.operation.name == operation)
            .expect("the operation")
            .out_struct
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect()
    };
    assert_eq!(members("mix"), ["b", "c"]);
    assert_eq!(members("odd"), ["return_", "return_1"]);
}

/// §7.5.1.2.5 rule 5 with N = 2: operations `remoteEx` and `remoteEx_1`
/// leave `remoteEx_2` for the `RemoteException` member.
#[test]
fn remote_ex_takes_the_first_free_number_past_one() {
    let (_, result) = enhanced_of("interface S { void remoteEx(); void remoteEx_1(); };", "S");
    let model = result.expect("the model");
    let names: Vec<(&str, bool)> = model
        .reply
        .members
        .iter()
        .map(|member| {
            (
                member.name.as_str(),
                matches!(member.type_, EnhancedMemberType::RemoteException),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("remoteEx", false),
            ("remoteEx_1", false),
            ("remoteEx_2", true)
        ]
    );
}

/// The `CompleteTypeObject` structure of the type called `name` in `idl`.
fn complete_struct(
    idl: &str,
    name: &str,
) -> zerodds_types::type_object::complete::CompleteStructType {
    let ast = model(idl);
    let id = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            name.to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type");
    match zerodds_types::from_ast::complete_type_object(&ast, id).expect("a TypeObject") {
        zerodds_types::type_object::CompleteTypeObject::Struct(found) => found,
        other => panic!("not a structure: {other:?}"),
    }
}

/// DDS-RPC 1.0 §7.5.1.2.1.1: a `@Choice` structure written in IDL has
/// the TypeObject of a mutable structure whose members are all optional,
/// like the structures the generators synthesize.
#[test]
fn a_choice_structure_written_in_idl_has_a_mutable_type_object_with_optional_members() {
    use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
    let found = complete_struct("@Choice struct Request { long add; long sub; };", "Request");
    assert!(found.struct_flags.has(StructTypeFlag::IS_MUTABLE));
    assert!(!found.struct_flags.has(StructTypeFlag::IS_FINAL));
    assert_eq!(found.member_seq.len(), 2);
    for member in &found.member_seq {
        assert!(
            member
                .common
                .member_flags
                .has(StructMemberFlag::IS_OPTIONAL),
            "{}",
            member.detail.name
        );
    }
    // The control: without the annotation the structure is final and its
    // members are required.
    let plain = complete_struct("struct Request { long add; long sub; };", "Request");
    assert!(!plain.struct_flags.has(StructTypeFlag::IS_MUTABLE));
    assert!(
        !plain.member_seq[0]
            .common
            .member_flags
            .has(StructMemberFlag::IS_OPTIONAL)
    );
}

/// §7.5.1.2.1.2: `@Autoid` on a structure written in IDL gives the
/// members of its TypeObject the HASH of their names as member ids, the
/// ids the synthesized structures carry.
#[test]
fn an_autoid_structure_written_in_idl_has_hashed_member_ids_in_its_type_object() {
    let found = complete_struct("@Autoid struct Hashed { long add; long sub; };", "Hashed");
    for member in &found.member_seq {
        assert_eq!(
            member.common.member_id,
            interface::member_id(member.detail.name.as_str()),
            "{}",
            member.detail.name
        );
    }
}
