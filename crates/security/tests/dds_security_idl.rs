// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §11.2: `dds_security.idl`, the IDL representation of the
//! plugin interfaces. The file parses as OMG IDL 4.2, and each interface has
//! the operations of its table in clause 9 and no `out` parameter (the PIM to
//! PSM rule of §11.2: "'Out' parameters in the PIM are conventionally mapped
//! to 'inout' parameters in the PSM").

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl_parse::{Dialect, parse_with};

const IDL: &str = include_str!("../idl/dds_security.idl");

/// The body of `local interface <name> { ... };`.
fn interface_body(name: &str) -> &'static str {
    let head = format!("local interface {name} {{");
    let start = IDL
        .find(&head)
        .unwrap_or_else(|| panic!("interface {name}"))
        + head.len();
    let end = IDL[start..]
        .find("\n        };")
        .map_or(IDL.len(), |e| start + e);
    &IDL[start..end]
}

/// Names of the operations of an interface body: the identifier before each
/// `(`.
fn operations(body: &str) -> Vec<String> {
    let parts: Vec<&str> = body.split('(').collect();
    parts[..parts.len() - 1]
        .iter()
        .map(|before| {
            let reversed: String = before
                .trim_end()
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            reversed.chars().rev().collect()
        })
        .collect()
}

#[test]
fn the_file_parses_as_omg_idl() {
    let parsed = parse_with(IDL, Dialect::STRICT);
    assert!(
        !parsed.has_errors(),
        "dds_security.idl does not parse: {:?}",
        parsed.diagnostics()
    );
}

#[test]
fn no_operation_has_an_out_parameter() {
    for line in IDL.lines() {
        let code = line.split("//").next().unwrap_or("");
        assert!(
            !code.split_whitespace().any(|w| w == "out"),
            "an out parameter in `{line}`: the PSM maps out to inout (§11.2)"
        );
    }
}

fn expect(interface: &str, names: &[&str]) {
    let got = operations(interface_body(interface));
    assert_eq!(got, names, "operations of {interface}");
}

/// Table 29.
#[test]
fn authentication_has_the_operations_of_table_29() {
    expect(
        "Authentication",
        &[
            "validate_local_identity",
            "get_identity_token",
            "get_identity_status_token",
            "set_participant_security_config",
            "set_permissions_credential_and_token",
            "validate_remote_identity",
            "begin_handshake_request",
            "begin_handshake_reply",
            "process_handshake",
            "get_shared_secret",
            "get_authenticated_peer_credential_token",
            "set_listener",
            "return_identity_token",
            "return_identity_status_token",
            "return_authenticated_peer_credential_token",
            "return_handshake_handle",
            "return_identity_handle",
            "return_sharedsecret_handle",
        ],
    );
}

/// Table 31.
#[test]
fn the_authentication_listener_has_the_operations_of_table_31() {
    expect(
        "AuthenticationListener",
        &["on_revoke_identity", "on_status_changed"],
    );
}

/// Table 39 and Table 40.
#[test]
fn access_control_has_the_operations_of_table_39() {
    expect(
        "AccessControl",
        &[
            "validate_local_permissions",
            "validate_remote_permissions",
            "check_create_participant",
            "check_create_datawriter",
            "check_create_datareader",
            "check_create_topic",
            "check_local_datawriter_register_instance",
            "check_local_datawriter_dispose_instance",
            "check_remote_participant",
            "check_remote_datawriter",
            "check_remote_datareader",
            "check_remote_topic",
            "check_local_datawriter_match",
            "check_local_datareader_match",
            "check_remote_datawriter_register_instance",
            "check_remote_datawriter_dispose_instance",
            "get_permissions_token",
            "get_permissions_credential_token",
            "set_listener",
            "return_permissions_token",
            "return_permissions_credential_token",
            "get_participant_security_config",
            "get_topic_security_config",
            "get_datawriter_security_config",
            "get_datareader_security_config",
            "return_participant_security_config",
            "return_topic_security_config",
            "return_datawriter_security_config",
            "return_datareader_security_config",
        ],
    );
    expect("AccessControlListener", &["on_revoke_permissions"]);
}

/// Table 42.
#[test]
fn crypto_key_factory_has_the_operations_of_table_42() {
    expect(
        "CryptoKeyFactory",
        &[
            "register_local_participant",
            "register_matched_remote_participant",
            "register_local_datawriter",
            "register_matched_remote_datareader",
            "register_local_datareader",
            "register_matched_remote_datawriter",
            "revise_local_entity_keys",
            "activate_key_revision",
            "unregister_participant",
            "unregister_datawriter",
            "unregister_datareader",
        ],
    );
}

/// Table 43.
#[test]
fn crypto_key_exchange_has_the_operations_of_table_43() {
    expect(
        "CryptoKeyExchange",
        &[
            "create_local_participant_crypto_tokens",
            "set_remote_participant_crypto_tokens",
            "create_local_datawriter_crypto_tokens",
            "set_remote_datawriter_crypto_tokens",
            "create_local_datareader_crypto_tokens",
            "set_remote_datareader_crypto_tokens",
            "return_crypto_tokens",
        ],
    );
}

/// Table 44.
#[test]
fn crypto_transform_has_the_operations_of_table_44() {
    expect(
        "CryptoTransform",
        &[
            "encode_serialized_payload",
            "encode_datawriter_submessage",
            "encode_datareader_submessage",
            "encode_rtps_message",
            "decode_rtps_message",
            "preprocess_secure_submsg",
            "decode_datawriter_submessage",
            "decode_datareader_submessage",
            "decode_serialized_payload",
        ],
    );
}

/// Table 46 and Figure 21.
#[test]
fn logging_has_the_operations_of_table_46() {
    expect(
        "Logging",
        &["set_log_options", "log", "enable_logging", "set_listener"],
    );
    expect("LoggerListener", &["log_message"]);
}

/// Table 28.
#[test]
fn the_security_exception_has_message_code_and_minor_code() {
    let start = IDL.find("struct SecurityException {").unwrap();
    let end = start + IDL[start..].find("};").unwrap();
    let body = &IDL[start..end];
    for member in ["string message;", "long code;", "long minor_code;"] {
        assert!(body.contains(member), "{member}");
    }
}

/// Table 30.
#[test]
fn the_validation_results_are_the_six_of_table_30() {
    for name in [
        "VALIDATION_OK",
        "VALIDATION_FAILED",
        "VALIDATION_PENDING_RETRY",
        "VALIDATION_PENDING_HANDSHAKE_REQUEST",
        "VALIDATION_PENDING_HANDSHAKE_MESSAGE",
        "VALIDATION_OK_FINAL_MESSAGE",
    ] {
        assert!(IDL.contains(name), "{name}");
    }
}
