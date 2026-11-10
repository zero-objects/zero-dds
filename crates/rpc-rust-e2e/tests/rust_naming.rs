// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust naming scheme of the RPC binding (GitHub #61): one IDL source
//! generated with the IDL names and with the names after RFC 430
//! (`zerodds_lang_rules::rust_names`), for the Basic and for the Enhanced
//! Service Mapping. The Rust names differ; the octets on the wire and the
//! registered type names, which DDS-RPC 1.0 §7.5.1.1.4-7 and §7.5.1.2
//! compose from the IDL names, do not.
//!
//! The two modules under the Rust scheme are compiled under
//! `deny(nonstandard_style)` (`src/lib.rs`), so the trait, its methods and
//! parameters, the client, the dispatcher and the locals of the codec pass
//! the lints too.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_dcps::dds_type::DdsType;
use zerodds_rpc_rust_e2e::naming_enhanced_idl::Arm_Ctl as enhanced_idl;
use zerodds_rpc_rust_e2e::naming_enhanced_rust::arm_ctl as enhanced_rust;
use zerodds_rpc_rust_e2e::naming_idl::Arm_Ctl as by_idl;
use zerodds_rpc_rust_e2e::naming_rust::arm_ctl as by_rust;
use zerodds_wire::{Endianness, WireError, Writer};

fn octets(write: impl FnOnce(&mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut writer = Writer::with_max_align(Endianness::Little, 4);
    write(&mut writer).expect("encode");
    writer.finish()
}

/// The names the generator writes under the Rust scheme: the trait of the
/// interface and of its base.
#[allow(dead_code)]
fn names_a_service<T: by_rust::RobotArm + by_rust::BaseArm>() {}

#[test]
fn rpc_7_5_1_1_the_basic_topic_types_are_the_same_under_both_naming_schemes() {
    // The client and the error enum, by their names under the Rust scheme.
    assert!(core::any::type_name::<by_rust::RobotArmClient>().ends_with("RobotArmClient"));
    assert!(core::any::type_name::<by_rust::RobotArmError>().ends_with("RobotArmError"));
    assert_eq!(
        <by_idl::robot_arm_Call as DdsType>::TYPE_NAME,
        <by_rust::RobotArmCall as DdsType>::TYPE_NAME
    );
    assert_eq!(
        <by_idl::robot_arm_Return as DdsType>::TYPE_NAME,
        <by_rust::RobotArmReturn as DdsType>::TYPE_NAME
    );
    assert!(
        <by_rust::RobotArmCall as DdsType>::TYPE_NAME.contains("robot_arm_Call"),
        "{}",
        <by_rust::RobotArmCall as DdsType>::TYPE_NAME
    );

    let call_idl = by_idl::robot_arm_Call::MoveTo(by_idl::robot_arm_moveTo_In {
        targetX: 3,
        stepCount: 4,
    });
    let call_rust = by_rust::RobotArmCall::MoveTo(by_rust::RobotArmMoveToIn {
        target_x: 3,
        step_count: 4,
    });
    assert_eq!(
        octets(|w| call_idl.encode(w)),
        octets(|w| call_rust.encode(w))
    );

    let ok_idl = by_idl::robot_arm_Return::MoveTo(by_idl::robot_arm_moveTo_Result::Ok(
        by_idl::robot_arm_moveTo_Out {
            stepCount: 5,
            reachedX: 6,
            return_: 7,
        },
    ));
    let ok_rust = by_rust::RobotArmReturn::MoveTo(by_rust::RobotArmMoveToResult::Ok(
        by_rust::RobotArmMoveToOut {
            step_count: 5,
            reached_x: 6,
            return_: 7,
        },
    ));
    assert_eq!(octets(|w| ok_idl.encode(w)), octets(|w| ok_rust.encode(w)));

    let raised_idl = by_idl::robot_arm_Return::MoveTo(
        by_idl::robot_arm_moveTo_Result::out_of_range(by_idl::out_of_range { maxValue: 9 }),
    );
    let raised_rust = by_rust::RobotArmReturn::MoveTo(by_rust::RobotArmMoveToResult::OutOfRange(
        by_rust::OutOfRange { max_value: 9 },
    ));
    assert_eq!(
        octets(|w| raised_idl.encode(w)),
        octets(|w| raised_rust.encode(w))
    );

    // The inherited operation, declared by the base interface.
    let base_idl = by_idl::base_arm_Call::PingCount(by_idl::base_arm_pingCount_In { seqNo: 2 });
    let base_rust = by_rust::BaseArmCall::PingCount(by_rust::BaseArmPingCountIn { seq_no: 2 });
    assert_eq!(
        octets(|w| base_idl.encode(w)),
        octets(|w| base_rust.encode(w))
    );
}

#[test]
fn rpc_7_5_1_2_the_enhanced_topic_types_are_the_same_under_both_naming_schemes() {
    assert_eq!(
        <enhanced_idl::robot_arm_Request as DdsType>::TYPE_NAME,
        <enhanced_rust::RobotArmRequest as DdsType>::TYPE_NAME
    );
    assert_eq!(
        <enhanced_idl::robot_arm_Reply as DdsType>::TYPE_NAME,
        <enhanced_rust::RobotArmReply as DdsType>::TYPE_NAME
    );
    assert_eq!(
        <enhanced_idl::robot_arm_Request as DdsType>::type_object(),
        <enhanced_rust::RobotArmRequest as DdsType>::type_object()
    );
    assert_eq!(
        <enhanced_idl::robot_arm_Reply as DdsType>::type_object(),
        <enhanced_rust::RobotArmReply as DdsType>::type_object()
    );

    let request_idl = enhanced_idl::robot_arm_Request::MoveTo(enhanced_idl::robot_arm_moveTo_In {
        targetX: 3,
        stepCount: 4,
    });
    let request_rust = enhanced_rust::RobotArmRequest::MoveTo(enhanced_rust::RobotArmMoveToIn {
        target_x: 3,
        step_count: 4,
    });
    assert_eq!(
        octets(|w| request_idl.encode(w)),
        octets(|w| request_rust.encode(w))
    );

    // §7.5.1.2.6: the request choice of the derived interface holds the
    // inherited operation, whose structure the base declares.
    let inherited_idl =
        enhanced_idl::robot_arm_Request::PingCount(enhanced_idl::base_arm_pingCount_In {
            seqNo: 2,
        });
    let inherited_rust =
        enhanced_rust::RobotArmRequest::PingCount(enhanced_rust::BaseArmPingCountIn { seq_no: 2 });
    assert_eq!(
        octets(|w| inherited_idl.encode(w)),
        octets(|w| inherited_rust.encode(w))
    );

    let raised_idl =
        enhanced_idl::robot_arm_Reply::MoveTo(enhanced_idl::robot_arm_moveTo_Result::out_of_range(
            enhanced_idl::out_of_range { maxValue: 9 },
        ));
    let raised_rust = enhanced_rust::RobotArmReply::MoveTo(
        enhanced_rust::RobotArmMoveToResult::OutOfRange(enhanced_rust::OutOfRange { max_value: 9 }),
    );
    assert_eq!(
        octets(|w| raised_idl.encode(w)),
        octets(|w| raised_rust.encode(w))
    );
}
