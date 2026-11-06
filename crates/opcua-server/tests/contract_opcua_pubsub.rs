// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-server` with `zerodds-opcua-pubsub`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-opcua-pubsub`: the cursor pair
//! `UaWriter`/`UaReader` with `UaEncode`/`UaDecode` for every service body
//! (`services.rs`, `wire.rs`, `identity.rs`); the data types
//! `UserTokenPolicy`/`UserTokenType` (re-exported by `identity.rs`) and
//! `ApplicationDescription`/`EndpointDescription` of GetEndpoints and
//! FindServers; `binary::check_array_len` as the guard before an array is
//! allocated (`wire.rs`, `services.rs`).
//!
//! So the crate relies on: a UserTokenPolicy being its five fields in the
//! order of the service definition, a String being its Int32 length and its
//! UTF-8 octets; the token types being the numbers of the service
//! definition; an array length the remaining octets cannot hold being
//! refused before anything is allocated.
//!
//! Oracles: OPC 10000-6 v1.05.07 §5.2.2.4 Figure 4 (`水Boy` as
//! `06 00 00 00 E6 B0 B4 42 6F 79`), §5.2.2.2 (Int32 little-endian);
//! OPC 10000-4 v1.05.07 §7.41 Table 192 (UserTokenPolicy: policyId,
//! tokenType, issuedTokenType, issuerEndpointUrl, securityPolicyUri), §7.42
//! Table 193 (UserTokenType values).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_opcua_pubsub::binary::check_array_len;
use zerodds_opcua_pubsub::uadp::datatypes::{UserTokenPolicy, UserTokenType};
use zerodds_opcua_pubsub::{UaDecode, UaEncode, UaReader, UaWriter};

fn string(s: &str) -> Vec<u8> {
    let mut out = i32::try_from(s.len()).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out
}

#[test]
fn uacs_7_41_a_user_token_policy_is_its_five_fields_in_order() {
    let policy = UserTokenPolicy {
        policy_id: "水Boy".to_string(),
        token_type: UserTokenType::IssuedToken,
        issued_token_type: "http://opcfoundation.org/UA/UserToken#JWT".to_string(),
        issuer_endpoint_url: "https://issuer".to_string(),
        security_policy_uri: "http://opcfoundation.org/UA/SecurityPolicy#None".to_string(),
    };
    let mut want = vec![0x06, 0x00, 0x00, 0x00, 0xe6, 0xb0, 0xb4, 0x42, 0x6f, 0x79];
    want.extend_from_slice(&[0x03, 0x00, 0x00, 0x00]);
    want.extend(string("http://opcfoundation.org/UA/UserToken#JWT"));
    want.extend(string("https://issuer"));
    want.extend(string("http://opcfoundation.org/UA/SecurityPolicy#None"));
    let mut w = UaWriter::new();
    policy.encode(&mut w).unwrap();
    assert_eq!(w.as_slice(), want);
    let mut r = UaReader::new(&want);
    assert_eq!(UserTokenPolicy::decode(&mut r).unwrap(), policy);
    assert!(r.is_empty());
}

#[test]
fn uacs_7_42_the_user_token_types_are_the_values_of_table_193() {
    for (token_type, value) in [
        (UserTokenType::Anonymous, 0u8),
        (UserTokenType::UserName, 1),
        (UserTokenType::Certificate, 2),
        (UserTokenType::IssuedToken, 3),
    ] {
        let policy = UserTokenPolicy {
            policy_id: "p".to_string(),
            token_type,
            issued_token_type: "t".to_string(),
            issuer_endpoint_url: "u".to_string(),
            security_policy_uri: "s".to_string(),
        };
        let mut w = UaWriter::new();
        policy.encode(&mut w).unwrap();
        // After the policyId "p" (four length octets, one character).
        assert_eq!(w.as_slice()[5..9], [value, 0, 0, 0]);
    }
}

#[test]
fn uacs_5_2_5_an_array_length_the_message_cannot_hold_is_refused() {
    let bytes = [0u8; 8];
    let r = UaReader::new(&bytes);
    // Two Int32 elements fit into eight octets, three do not.
    assert!(check_array_len(&r, 2, 4, "test array").is_ok());
    assert!(check_array_len(&r, 3, 4, "test array").is_err());
}
