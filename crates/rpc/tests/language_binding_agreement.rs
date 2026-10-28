// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The wire model of §7.5.1 and the language bindings of §7.11, held
//! against each other.
//!
//! Two crates derive the same names from the same clause. `codegen.rs`
//! derives them for the runtime — what a `Requester` writes and a
//! `Replier` reads — and `zerodds_lang_rules::adapter::interface`
//! derives them for the generated source. Nothing but this file makes
//! them agree.
//!
//! The failure without it is silent in the worst way: a generated header
//! declaring `Calculator_Request` and a runtime writing a topic type it
//! calls something else both compile, both pass their own tests, and
//! meet only on the wire, where the reader finds a type name it has no
//! registration for and drops the sample.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::interface;
use zerodds_rpc::codegen::{MemberType, build_basic_pair, build_enhanced_service};
use zerodds_rpc::service_mapping::lower_service;

const CALCULATOR: &str = "
    module robot {
      interface Calculator {
        long add(in long a, in long b);
        long divmod(in long a, in long b, out long remainder);
      };
    };
";

/// An operation that raises, so §7.5.1.1.5's exception cases have
/// something to carry, and one that carries nothing at all, so rule 3c's
/// `dummy` member does.
const RAISING: &str = "
    module robot {
      exception Overflow { long code; };
      interface Calculator {
        long add(in long a, in long b) raises (Overflow);
        void reset();
      };
    };
";

/// A hierarchy, an explicit `<service_name>` and both topic
/// annotations — everything §7.4 lets an IDL author say.
const ANNOTATED: &str = r#"
    module robot {
      interface Adder { long add(in long a); };
      @service(name = "Arithmetic")
      interface Calculator : Adder { long mul(in long a); };
    };
"#;

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

/// The one interface of the model, through both readings.
fn both(ast: &Ast) -> (zerodds_rpc::ServiceDef, interface::Service) {
    let (id, _) = ast.interfaces().next().expect("an interface");
    let wire = lower_service(ast, id).expect("the wire model");
    let seam = interface::service(ast, id).expect("the seam");
    (wire, seam)
}

/// The two readings agree on the operation set and on each operation's
/// parameters and directions.
#[test]
fn the_seam_and_the_wire_model_read_one_interface_the_same_way() {
    let ast = model(CALCULATOR);
    let (wire, seam) = both(&ast);

    assert_eq!(
        wire.methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        seam.operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect::<Vec<_>>()
    );

    for (method, operation) in wire.methods.iter().zip(seam.operations.iter()) {
        assert!(!method.oneway, "{}", method.name);
        assert_eq!(
            method.in_params().count(),
            operation.inputs().count(),
            "{}",
            method.name
        );
        assert_eq!(
            method.out_params().count(),
            operation.outputs().count(),
            "{}",
            method.name
        );
    }
}

/// §7.5.1.1.4-7: the basic-layout struct and topic names the runtime
/// derives are the ones the seam states and the generated header writes.
#[test]
fn the_basic_topic_type_names_agree() {
    let ast = model(CALCULATOR);
    let (wire, seam) = both(&ast);
    let (request, reply) = build_basic_pair(&wire).expect("the basic pair");

    // §7.5.1.1.6-7: the *type* names are `${interfaceName}_Request` and
    // `${interfaceName}_Reply`, built from the simple name.
    assert_eq!(request.name, interface::request_type_name(&seam.name));
    assert_eq!(reply.name, interface::reply_type_name(&seam.name));
    // §7.4.1: the *topic* names are a different string, built from the
    // fully-qualified interface name and `<service_name>`.
    assert_eq!(
        request.topic_name,
        seam.request_topic_name(interface::BindingStyle::FunctionCall)
    );
    assert_eq!(
        reply.topic_name,
        seam.reply_topic_name(interface::BindingStyle::FunctionCall)
    );
    assert_ne!(request.name, request.topic_name);

    let MemberType::CallUnion(call) = &request.members[1].type_ref else {
        panic!("the request's second member is the call union");
    };
    assert_eq!(call.name, interface::call_union_name(&seam.name));
    for case in &call.cases {
        assert_eq!(
            case.case_type_name,
            interface::in_struct_name(&seam.name, &case.method)
        );
    }

    let MemberType::CallUnion(returns) = &reply.members[1].type_ref else {
        panic!("the reply's second member is the Return union");
    };
    assert_eq!(returns.name, interface::return_union_name(&seam.name));
    for case in &returns.cases {
        assert_eq!(
            case.case_type_name,
            interface::result_union_name(&seam.name, &case.method)
        );
    }
}

/// §7.5.1.2.4-5 and §7.4.3: the Enhanced Service Mapping's two topic
/// types are `${I}_Request` and `${I}_Reply`, on the interface's one
/// topic pair, and the per-operation structures keep the names of the
/// Basic mapping.
#[test]
fn the_enhanced_topic_type_names_agree() {
    let ast = model(CALCULATOR);
    let (wire, seam) = both(&ast);
    let service = build_enhanced_service(&wire).expect("the enhanced service");

    assert_eq!(
        service.request.name,
        interface::request_type_name(&seam.name)
    );
    assert_eq!(service.reply.name, interface::reply_type_name(&seam.name));
    assert_eq!(
        service.request_topic,
        seam.request_topic_name(interface::BindingStyle::FunctionCall)
    );
    assert_eq!(
        service.reply_topic,
        seam.reply_topic_name(interface::BindingStyle::FunctionCall)
    );
    assert_eq!(service.operations.len(), seam.operations.len());
    for (built, operation) in service.operations.iter().zip(seam.operations.iter()) {
        assert_eq!(
            built.in_struct.name,
            interface::in_struct_name(&seam.name, &operation.name)
        );
        assert_eq!(
            built.out_struct.name,
            interface::out_struct_name(&seam.name, &operation.name)
        );
        assert_eq!(
            built.result_struct.name,
            interface::result_union_name(&seam.name, &operation.name)
        );
    }
}

/// §7.4.1, §7.4.2.2 and §7.4.3, read twice: the run time reads them in
/// `crates/rpc/src/topic_naming.rs` and the generator in
/// `crates/lang-rules/src/adapter/interface.rs`, and the two have to
/// produce the same strings or a generated client subscribes to a topic
/// the runtime never publishes.
#[test]
fn both_readings_of_seven_four_produce_the_same_topic_names() {
    for source in [CALCULATOR, ANNOTATED] {
        let ast = model(source);
        let (id, _) = ast.interfaces().last().expect("an interface");
        let wire = lower_service(&ast, id).expect("the wire model");
        let seam = interface::service(&ast, id).expect("the seam");

        assert_eq!(wire.qualified_name, seam.qualified_name);
        assert_eq!(wire.service_name, seam.service_name);
        assert_eq!(wire.request_topic, seam.request_topic);
        assert_eq!(wire.reply_topic, seam.reply_topic);

        for (runtime_style, seam_style) in [
            (
                zerodds_rpc::topic_naming::BindingStyle::FunctionCall,
                interface::BindingStyle::FunctionCall,
            ),
            (
                zerodds_rpc::topic_naming::BindingStyle::RequestReply,
                interface::BindingStyle::RequestReply,
            ),
        ] {
            let names = wire.topic_names_for(runtime_style).expect("topic names");
            assert_eq!(
                names.request,
                seam.request_topic_name(seam_style),
                "{runtime_style:?}"
            );
            assert_eq!(
                names.reply,
                seam.reply_topic_name(seam_style),
                "{runtime_style:?}"
            );
            assert_eq!(
                names.request_aliases,
                seam.topic_aliases(seam_style, false),
                "{runtime_style:?}"
            );
            assert_eq!(
                names.reply_aliases,
                seam.topic_aliases(seam_style, true),
                "{runtime_style:?}"
            );
        }
    }
}

/// The names the clause's own non-normative table prints, end to end
/// from IDL. §7.4.3, "Everything Default" and "User-defined service
/// name".
#[test]
fn the_published_table_comes_out_of_the_idl() {
    let ast = model(ANNOTATED);
    let (id, _) = ast.interfaces().last().expect("Calculator");
    let wire = lower_service(&ast, id).expect("the wire model");
    let names = wire.topic_names().expect("topic names");
    assert_eq!(names.request, "robot_Calculator_Arithmetic_Request");
    assert_eq!(names.reply, "robot_Calculator_Arithmetic_Reply");
    assert_eq!(names.request_aliases, ["robot_Adder_Arithmetic_Request"]);
    assert_eq!(names.reply_aliases, ["robot_Adder_Arithmetic_Reply"]);
}

/// §7.5.1.1.4-7, read twice: `crates/rpc/src/codegen.rs` derives the
/// sample structure for the runtime and
/// `crates/lang-rules/src/adapter/interface.rs` for the generator. Every
/// name and every case label the two produce has to be the same string
/// and the same number, or the generated reader decodes a case the
/// writer never selected.
#[test]
fn both_readings_of_seven_five_produce_the_same_sample_structure() {
    let ast = model(RAISING);
    let (id, _) = ast.interfaces().last().expect("an interface");
    let wire = lower_service(&ast, id).expect("the wire model");
    let seam = interface::service(&ast, id).expect("the seam");
    let (request, reply) = build_basic_pair(&wire).expect("the basic pair");

    // §7.5.1.1.6 rule 2b, §7.5.1.1.7 rule 3b: the second member is
    // `data`.
    assert_eq!(request.members[1].name, interface::DATA_MEMBER);
    assert_eq!(reply.members[1].name, interface::DATA_MEMBER);

    let MemberType::CallUnion(call) = &request.members[1].type_ref else {
        panic!("the request's second member is the Call union");
    };
    let MemberType::CallUnion(returns) = &reply.members[1].type_ref else {
        panic!("the reply's second member is the Return union");
    };
    assert_eq!(call.name, interface::call_union_name(&seam.name));
    assert_eq!(returns.name, interface::return_union_name(&seam.name));

    // §7.5.1.1.6 rule 3: the `default` case's member.
    assert_eq!(call.default_member, interface::UNKNOWN_OP_MEMBER);
    assert_eq!(returns.default_member, interface::UNKNOWN_OP_MEMBER);

    for case in &call.cases {
        // Rules 4 and 5a: the constant and the label.
        assert_eq!(
            case.hash_constant,
            interface::operation_hash_constant(&seam.name, &case.method)
        );
        assert_eq!(case.discriminator, interface::operation_hash(&case.method));
        assert_eq!(
            case.case_type_name,
            interface::in_struct_name(&seam.name, &case.method)
        );
    }

    for case in &returns.cases {
        // §7.5.1.1.7 rule 5b: the reply case selects the per-operation
        // `Result` union, not the `Out` structure.
        assert_eq!(
            case.case_type_name,
            interface::result_union_name(&seam.name, &case.method)
        );
        let result = case.result_union.as_ref().expect("a Result union");
        assert_eq!(
            result.out_type_name,
            interface::out_struct_name(&seam.name, &case.method)
        );

        let operation = seam
            .operations
            .iter()
            .find(|candidate| candidate.name == case.method)
            .expect("the operation the case names");
        assert_eq!(result.exceptions.len(), operation.raises.len());
        for (raised, declared) in result.exceptions.iter().zip(operation.raises.iter()) {
            // §7.5.1.1.5 rule 4: the constant and the hash of the
            // *scoped* name.
            assert_eq!(
                raised.hash_constant,
                interface::exception_hash_constant(&declared.name)
            );
            assert_eq!(
                raised.discriminator,
                interface::exception_hash(&declared.scoped_name)
            );
            // Rule 5b: the member name.
            assert_eq!(
                raised.member,
                interface::exception_member_name(&declared.name)
            );
        }
    }
}

/// The two placeholder types §7.5.1.1.1 declares are one octet each, and
/// the member names the clause fixes are spelled once per column.
#[test]
fn the_common_types_and_member_names_are_spelled_once() {
    assert_eq!(zerodds_rpc::PLACEHOLDER_OCTETS, 1);
    assert_eq!(core::mem::size_of::<zerodds_rpc::UnusedMember>(), 1);
    assert_eq!(core::mem::size_of::<zerodds_rpc::UnknownOperation>(), 1);
    assert_eq!(core::mem::size_of::<zerodds_rpc::UnknownException>(), 1);

    assert_eq!(zerodds_rpc::DUMMY_MEMBER, interface::DUMMY_MEMBER);
    assert_eq!(zerodds_rpc::RETURN_MEMBER, interface::RETURN_MEMBER);
    assert_eq!(zerodds_rpc::RESULT_MEMBER, interface::RESULT_MEMBER);
    assert_eq!(zerodds_rpc::UNKNOWN_OP_MEMBER, interface::UNKNOWN_OP_MEMBER);
    assert_eq!(zerodds_rpc::RETCODE_OK, interface::RETCODE_OK);
}

/// The member a reply carries the return value in is spelled once.
#[test]
fn the_return_member_is_spelled_the_same_in_both() {
    let ast = model(CALCULATOR);
    let (wire, _) = both(&ast);
    let service = build_enhanced_service(&wire).expect("the enhanced service");
    let add = service
        .operations
        .iter()
        .find(|operation| operation.name == "add")
        .expect("add");
    assert!(
        add.out_struct
            .members
            .iter()
            .any(|member| member.name == interface::RETURN_MEMBER),
        "the Out structure carries `{}`: {:?}",
        interface::RETURN_MEMBER,
        add.out_struct
            .members
            .iter()
            .map(|m| &m.name)
            .collect::<Vec<_>>()
    );
}

/// Every struct name the runtime derives appears, spelled exactly, in
/// the C++ header the generator writes. Comparing the two derivations
/// alone would pass on two functions that agree with each other and
/// with no output.
#[test]
fn the_generated_cpp_header_declares_the_names_the_runtime_derives() {
    let ast = model(CALCULATOR);
    let (wire, _) = both(&ast);
    let (request, reply) = build_basic_pair(&wire).expect("the basic pair");

    let header = zerodds_lang_rules::emit::interface_cpp::cpp_interface_items(&ast)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::cpp::decl)
        .collect::<Vec<_>>()
        .join("\n");

    for name in [&request.name, &reply.name] {
        assert!(
            header.contains(&format!("struct {name} {{")),
            "`{name}` is not declared in:\n{header}"
        );
    }
    let MemberType::CallUnion(call) = &request.members[1].type_ref else {
        panic!("the request's second member is the call union");
    };
    assert!(header.contains(&format!("struct {} {{", call.name)));
    for case in &call.cases {
        assert!(
            header.contains(&format!("struct {} {{", case.case_type_name)),
            "`{}` is not declared in:\n{header}",
            case.case_type_name
        );
    }

    // The typed endpoints of §7.11.1.1.1 name the same two topic types.
    assert!(
        header.contains(&format!(
            "using CalculatorRequester = dds::rpc::core::Requester<{}, {}>;",
            request.name, reply.name
        )),
        "{header}"
    );
}
