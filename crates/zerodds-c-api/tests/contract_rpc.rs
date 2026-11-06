// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-rpc`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rpc` (feature `rpc`):
//! `src/rpc_discovery_ffi.rs` hands `discovery_ext::{
//! service_matching_complete, client_matching_complete}` to C as
//! `zerodds_rpc_service_matching_complete` and
//! `zerodds_rpc_client_matching_complete`.
//!
//! What it relies on: a client has discovered a service instance once one
//! of its request writer's readers and one of its reply reader's writers
//! name each other and the reader announced the client's instance name; a
//! service has discovered a client once the reply reader the client's
//! request writer names is matched and names that writer back; a peer
//! that announced no related endpoint completes only when unannounced
//! peers are accepted.
//!
//! Oracle: DDS-RPC 1.0 §7.6.2.2.1 (Figure 7.3) and §7.6.2.2.2 (Figure 7.4).

#![cfg(feature = "rpc")]
#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::runtime::{RpcDiscoveryExt, RpcPeer};
use zerodds_rpc::discovery_ext::{client_matching_complete, service_matching_complete};

const REQUEST_READER: [u8; 16] = [1; 16];
const REPLY_WRITER: [u8; 16] = [2; 16];
const REQUEST_WRITER: [u8; 16] = [3; 16];
const REPLY_READER: [u8; 16] = [4; 16];

fn peer(key: [u8; 16], related: Option<[u8; 16]>, instance: Option<&str>) -> RpcPeer {
    RpcPeer {
        key,
        ext: RpcDiscoveryExt {
            service_instance_name: instance.map(str::to_string),
            related_entity_guid: related,
            topic_aliases: None,
        },
    }
}

/// §7.6.2.2.2: the service instance is complete when its request reader
/// and its reply writer name each other and the reader announced the
/// instance the client addresses.
#[test]
fn rpc_7_6_2_2_2_a_service_is_complete_when_its_two_endpoints_name_each_other() {
    let reader = peer(REQUEST_READER, Some(REPLY_WRITER), Some("calc-a"));
    let writer = peer(REPLY_WRITER, Some(REQUEST_READER), Some("calc-a"));
    assert!(service_matching_complete(
        core::slice::from_ref(&reader),
        core::slice::from_ref(&writer),
        "calc-a",
        false
    ));
    assert!(
        !service_matching_complete(core::slice::from_ref(&reader), &[writer], "calc-b", false),
        "another instance"
    );
    let stranger = peer(REPLY_WRITER, Some([9; 16]), None);
    assert!(
        !service_matching_complete(&[reader], &[stranger], "calc-a", false),
        "a writer that names another reader"
    );
    let unannounced_reader = peer(REQUEST_READER, None, None);
    let unannounced_writer = peer(REPLY_WRITER, None, None);
    assert!(!service_matching_complete(
        core::slice::from_ref(&unannounced_reader),
        core::slice::from_ref(&unannounced_writer),
        "",
        false
    ));
    assert!(service_matching_complete(
        &[unannounced_reader],
        &[unannounced_writer],
        "",
        true
    ));
}

/// §7.6.2.2.1: the service has discovered the client once the reply reader
/// the client's request writer names is matched and names it back.
#[test]
fn rpc_7_6_2_2_1_a_client_is_discovered_once_its_reply_reader_is_matched() {
    let client_writer = peer(REQUEST_WRITER, Some(REPLY_READER), None);
    let reply_reader = peer(REPLY_READER, Some(REQUEST_WRITER), None);
    assert!(client_matching_complete(
        &client_writer,
        &[reply_reader],
        false
    ));
    assert!(
        !client_matching_complete(&client_writer, &[], false),
        "its reply reader not matched yet"
    );
    let unannounced = peer(REQUEST_WRITER, None, None);
    assert!(!client_matching_complete(&unannounced, &[], false));
    assert!(client_matching_complete(&unannounced, &[], true));
}
