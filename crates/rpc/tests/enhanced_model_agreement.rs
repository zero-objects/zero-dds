// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced Service Mapping of §7.5.1.2, derived twice and held
//! against itself.
//!
//! `crates/rpc/src/codegen.rs::build_enhanced_service` derives it for the
//! run time, `zerodds_lang_rules::adapter::interface::enhanced_service`
//! for the generated source (the generators cannot depend on this crate).
//! A generated `Calc_Request` the run time does not call that, or with a
//! member the run time hashes differently, compiles on both sides and
//! meets only on the wire.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::interface::{self, EnhancedMemberType};
use zerodds_rpc::codegen::{
    EmptyOperationStyle, EnhancedOptions, EnhancedStruct, MemberType, build_enhanced_service_with,
};
use zerodds_rpc::service_mapping::lower_service;

/// An exception, an `@Mutable` operation, an operation with nothing to
/// carry, an attribute, an `inout` parameter, a return value that
/// collides with an `out` parameter, and a hierarchy.
const HIERARCHY: &str = "
    module robot {
      exception Overflow { long code; };
      interface Adder { long add(in long a, in long b) raises (Overflow); };
      interface Subtractor { long sub(in long a, in long b); };
      interface Calculator : Adder, Subtractor {
        void on();
        void off();
        @Mutable long scale(in long k) raises (Overflow);
        attribute long total;
        void swap(inout long v);
        long clash(out long return_);
      };
    };
";

/// An operation named like the member the reply carries the remote
/// exception code in (§7.5.1.2.5 rule 5).
const REMOTE_EX: &str = "
    interface S { void remoteEx(); void remoteEx_1(); };
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

fn agree(wire: &EnhancedStruct, seam: &interface::EnhancedStruct) {
    assert_eq!(wire.name, seam.name);
    assert_eq!(wire.choice, seam.choice, "{}: @Choice", wire.name);
    assert_eq!(wire.autoid, seam.autoid, "{}: @Autoid", wire.name);
    assert_eq!(wire.empty, seam.empty, "{}: @Empty", wire.name);
    assert_eq!(wire.mutable, seam.mutable, "{}: @Mutable", wire.name);
    assert_eq!(wire.members.len(), seam.members.len(), "{}", wire.name);
    for (wire_member, seam_member) in wire.members.iter().zip(&seam.members) {
        assert_eq!(wire_member.name, seam_member.name, "{}", wire.name);
        assert_eq!(
            wire_member.member_id,
            Some(seam_member.member_id),
            "{}.{}: the @Autoid member id",
            wire.name,
            wire_member.name
        );
        match (&wire_member.type_ref, &seam_member.type_) {
            (MemberType::Struct(a), EnhancedMemberType::Struct(b)) => assert_eq!(a, b),
            (MemberType::RemoteException, EnhancedMemberType::RemoteException)
            | (MemberType::UnusedMember, EnhancedMemberType::Dummy) => {}
            (MemberType::Exception(a), EnhancedMemberType::Exception(b)) => {
                assert_eq!(a.name, b.name);
                assert_eq!(a.scoped_name, b.scoped_name);
            }
            (MemberType::Idl(a), EnhancedMemberType::Idl(b)) => {
                assert_eq!(a.resolved(), Some(*b), "{}.{}", wire.name, wire_member.name);
            }
            (a, b) => panic!("{}.{}: {a:?} against {b:?}", wire.name, wire_member.name),
        }
    }
}

fn check(idl: &str, interface_name: &str, style: EmptyOperationStyle) {
    let ast = model(idl);
    let (id, _) = ast
        .interfaces()
        .find(|(_, node)| node.name == interface_name)
        .expect("the interface");
    let wire_service = lower_service(&ast, id).expect("the wire model");
    let seam_service = interface::service(&ast, id).expect("the seam");
    let wire = build_enhanced_service_with(
        &wire_service,
        EnhancedOptions {
            empty_operations: style,
        },
    )
    .expect("the wire model");
    let seam_style = match style {
        EmptyOperationStyle::DummyMember => interface::EmptyOperationStyle::DummyMember,
        EmptyOperationStyle::EmptyAnnotation => interface::EmptyOperationStyle::EmptyAnnotation,
    };
    let seam = interface::enhanced_service(&ast, &seam_service, seam_style).expect("the seam");

    assert_eq!(wire.operations.len(), seam.operations.len());
    for (wire_op, seam_op) in wire.operations.iter().zip(&seam.operations) {
        assert_eq!(wire_op.interface, seam_op.interface);
        assert_eq!(wire_op.name, seam_op.operation.name);
        agree(&wire_op.in_struct, &seam_op.in_struct);
        agree(&wire_op.out_struct, &seam_op.out_struct);
        agree(&wire_op.result_struct, &seam_op.result_struct);
    }
    agree(&wire.request, &seam.request);
    agree(&wire.reply, &seam.reply);
}

#[test]
fn the_two_derivations_of_a_hierarchy_agree() {
    check(HIERARCHY, "Calculator", EmptyOperationStyle::DummyMember);
}

#[test]
fn the_two_derivations_agree_on_the_empty_annotation_reading() {
    check(
        HIERARCHY,
        "Calculator",
        EmptyOperationStyle::EmptyAnnotation,
    );
}

#[test]
fn the_two_derivations_agree_on_the_remote_ex_collision() {
    check(REMOTE_EX, "S", EmptyOperationStyle::DummyMember);
    let ast = model(REMOTE_EX);
    let (id, _) = ast.interfaces().next().expect("an interface");
    let seam = interface::enhanced_service(
        &ast,
        &interface::service(&ast, id).unwrap(),
        interface::EmptyOperationStyle::DummyMember,
    )
    .unwrap();
    // `remoteEx` and `remoteEx_1` are taken: the code goes to `remoteEx_2`.
    assert_eq!(seam.reply.members.last().unwrap().name, "remoteEx_2");
}
