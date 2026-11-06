// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-opcua-server`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-opcua-server`: `address_space::{
//! AddressSpace, NodeClass, NodeMeta, reference_types}` and
//! `attributes::{VariableAttributes, ClassAttributes, access_level}` to
//! build the topic tree (`src/address_space_build.rs`: `ORGANIZES` from the
//! Objects folder, `HAS_COMPONENT` per member, `CURRENT_READ` on a
//! `dds_to_ua` leaf, `CURRENT_READ | CURRENT_WRITE` on a `ua_to_dds` leaf);
//! `server::Server`, `client::serve_connection` and the client of
//! `src/ua_client.rs`; `identity::UserStore::anonymous_only` and
//! `identity::ClientUserId` for the per-session write ACL, which answers a
//! refused write with `status::BAD_USER_ACCESS_DENIED`
//! (`src/daemon/server.rs`).
//!
//! So the crate relies on: the reference types and the access level bits
//! being the ones of the address space model; the status code of a refused
//! write being the one of the code table; the anonymous-only store offering
//! exactly the anonymous token.
//!
//! Oracles: `NodeIds.csv` of OPCFoundation/UA-Nodeset (Organizes 35,
//! HasComponent 47, HasProperty 46, HasTypeDefinition 40);
//! OPC 10000-3 v1.05.06 §8.57 Table 40 (AccessLevelType: CurrentRead bit 0,
//! CurrentWrite bit 1, HistoryRead bit 2); `StatusCode.csv` of the same
//! repository (BadUserAccessDenied 0x801F0000, Good 0); OPC 10000-4
//! v1.05.07 §7.42 Table 193 (ANONYMOUS 0).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_server::address_space::reference_types;
use zerodds_opcua_server::attributes::access_level;
use zerodds_opcua_server::identity::{UserStore, UserTokenType};
use zerodds_opcua_server::status;

#[test]
fn the_reference_types_are_the_ones_of_the_nodeset() {
    assert_eq!(reference_types::ORGANIZES, NodeId::numeric(0, 35));
    assert_eq!(reference_types::HAS_COMPONENT, NodeId::numeric(0, 47));
    assert_eq!(reference_types::HAS_PROPERTY, NodeId::numeric(0, 46));
    assert_eq!(reference_types::HAS_TYPE_DEFINITION, NodeId::numeric(0, 40));
}

#[test]
fn the_access_level_bits_are_the_ones_of_table_40() {
    assert_eq!(access_level::CURRENT_READ, 0b001);
    assert_eq!(access_level::CURRENT_WRITE, 0b010);
    assert_eq!(access_level::HISTORY_READ, 0b100);
}

#[test]
fn a_refused_write_is_bad_user_access_denied_of_the_code_table() {
    assert_eq!(status::BAD_USER_ACCESS_DENIED, 0x801F_0000);
    assert_eq!(status::GOOD, 0);
}

#[test]
fn uacs_7_42_the_anonymous_only_store_offers_the_anonymous_token_alone() {
    let store = UserStore::anonymous_only();
    let types: Vec<UserTokenType> = store.policies().iter().map(|p| p.token_type).collect();
    assert_eq!(types, [UserTokenType::Anonymous]);
}
