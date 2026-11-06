// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc-rust-e2e` with `zerodds-rpc`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rpc`: the code `build.rs` generates and `src/lib.rs` includes: `requester::Requester` (`with_options`, `send_request_blocking`), `replier::{Replier, ReplierHandler}`, `qos_profile::RpcQos`, `topic_naming::{AnnotatedTopics, TopicNameOverrides, BindingStyle::RequestReply}`, `endpoint_options::{RpcEndpointOptions, RpcMapping::Enhanced}` and `error::{RpcResult, RpcError::Codec}`.
//!
//! What it relies on: the topic names of §7.4.1 (the qualified interface,
//! the service name, the suffix; the request/reply style without the
//! interface segment), the QoS of §7.10.2 for both directions, and the
//! exception codes of §7.5.1.1.1 as the spec's ordinals.
//!
//! Oracle: DDS-RPC 1.0 §7.4.1, §7.5.1.1.1, §7.10.2.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::qos::{DurabilityKind, HistoryKind, ReliabilityKind};
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::topic_naming::{
    BindingStyle, DEFAULT_SERVICE_NAME, ServiceTopicNames, TopicNameOverrides,
    qualified_interface_name, reply_topic_name, request_topic_name,
};

/// §7.4.1: `<qualified interface>_<service name>_Request` and `_Reply`,
/// the modules joined by underscores, the service name `Service` by
/// default; the request/reply style drops the interface segment.
#[test]
fn rpc_7_4_1_the_topic_names_of_a_service() {
    let interface = qualified_interface_name(&["robot"], "Calculator");
    assert_eq!(interface, "robot_Calculator");
    assert_eq!(DEFAULT_SERVICE_NAME, "Service");
    assert_eq!(
        request_topic_name(&interface).unwrap(),
        "robot_Calculator_Service_Request"
    );
    assert_eq!(
        reply_topic_name(&interface).unwrap(),
        "robot_Calculator_Service_Reply"
    );
    let names = ServiceTopicNames::synthesize(
        BindingStyle::RequestReply,
        &interface,
        &[],
        DEFAULT_SERVICE_NAME,
        None,
        None,
        &TopicNameOverrides::none(),
    )
    .unwrap();
    assert_eq!(names.request, "Service_Request");
    assert_eq!(names.reply, "Service_Reply");
}

/// §7.10.2: both directions RELIABLE, KEEP_ALL, VOLATILE.
#[test]
fn rpc_7_10_2_the_endpoints_are_reliable_keep_all_and_volatile() {
    let qos = RpcQos::default_basic();
    for writer in [qos.request_writer_qos(), qos.reply_writer_qos()] {
        assert_eq!(writer.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(writer.history.kind, HistoryKind::KeepAll);
        assert_eq!(writer.durability.kind, DurabilityKind::Volatile);
    }
    for reader in [qos.request_reader_qos(), qos.reply_reader_qos()] {
        assert_eq!(reader.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(reader.history.kind, HistoryKind::KeepAll);
        assert_eq!(reader.durability.kind, DurabilityKind::Volatile);
    }
}

/// §7.5.1.1.1: `RemoteExceptionCode_t` in the order the spec lists it.
#[test]
fn rpc_7_5_1_1_1_the_exception_codes_are_the_spec_ordinals() {
    for (v, code) in [
        (0u32, RemoteExceptionCode::Ok),
        (1, RemoteExceptionCode::Unsupported),
        (2, RemoteExceptionCode::InvalidArgument),
        (3, RemoteExceptionCode::OutOfResources),
        (4, RemoteExceptionCode::UnknownOperation),
        (5, RemoteExceptionCode::UnknownException),
    ] {
        assert_eq!(RemoteExceptionCode::from_u32(v).unwrap(), code);
    }
    assert!(RemoteExceptionCode::from_u32(6).is_err());
}
