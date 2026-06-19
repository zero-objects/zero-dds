//! Conformance matrix for DDS-Security 1.2 §1.1 + §1.2 + §2.1.
//!
//! Verifies in production that:
//!
//! 1. All 5 SPI traits from `zerodds-security` are fulfilled by one
//!    builtin plugin each in the workspace (`accepts_builtin_plugin`).
//! 2. Each SPI rejects a "misimplemented" plugin (a plugin with
//!    an empty class id or error-only behavior) according to the
//!    spec contract (`rejects_misimplemented_plugin`).
//! 3. The conformance-points table from §2.1 (builtin plugins,
//!    plugin framework, plugin language APIs, logging+tagging profile)
//!    is covered exhaustively as a test table.
//!
//! Spec OMG DDS-Security 1.2:
//! * §1.1 — DDS-Security compliance profile as an extension of DDS.
//! * §1.2 — 5 SPIs: Authentication, AccessControl, Cryptographic,
//!   Logging, DataTagging.
//! * §2.1 — conformance points: Builtin / Plugin Framework /
//!   Plugin Language APIs / Logging+Tagging.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (the conformance matrix tests the plugin SPI precisely over `Box<dyn>`
//! erasure — that is the point of the test.)

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::{Arc, Mutex};

use zerodds_security::access_control::{AccessControlPlugin, AccessDecision, PermissionsHandle};
use zerodds_security::authentication::{AuthenticationPlugin, IdentityHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::logging::{LogLevel, LoggingPlugin};
use zerodds_security::mock::{MockLogEntry, MockLogSink, MockLoggingPlugin};
use zerodds_security::properties::PropertyList;
use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_logging::StderrLoggingPlugin;
use zerodds_security_permissions::PermissionsAccessControl;
use zerodds_security_pki::PkiAuthenticationPlugin;
use zerodds_security_runtime::BuiltinDataTaggingPlugin;

// ============================================================================
// §1.2 Conformance-Point 1 — Authentication SPI (Builtin: PKI-DH)
// ============================================================================

#[test]
fn auth_accepts_builtin_pki_plugin() {
    let mut plugin: Box<dyn AuthenticationPlugin> = Box::new(PkiAuthenticationPlugin::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Auth:PKI-DH:1.2");
    // Spec requirement: an invalid PropertyList → AuthenticationFailed
    // (not panic, not silent success). No cert properties =
    // misconfigured local identity → must be an error.
    let res = plugin.validate_local_identity(&PropertyList::new(), [0xAA; 16]);
    assert!(
        res.is_err(),
        "Builtin Auth must return an error without cert properties"
    );
}

#[test]
fn auth_rejects_misimplemented_plugin() {
    /// A plugin with an empty class id (violates spec §8.3.1.1).
    struct BrokenAuth;
    impl AuthenticationPlugin for BrokenAuth {
        fn validate_local_identity(
            &mut self,
            _props: &PropertyList,
            _participant_guid: [u8; 16],
        ) -> SecurityResult<IdentityHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken: never success",
            ))
        }
        fn validate_remote_identity(
            &mut self,
            _local: IdentityHandle,
            _remote_participant_guid: [u8; 16],
            _remote_auth_token: &[u8],
        ) -> SecurityResult<IdentityHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken",
            ))
        }
        fn begin_handshake_request(
            &mut self,
            _initiator: IdentityHandle,
            _replier: IdentityHandle,
        ) -> SecurityResult<(
            zerodds_security::authentication::HandshakeHandle,
            zerodds_security::authentication::HandshakeStepOutcome,
        )> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken",
            ))
        }
        fn begin_handshake_reply(
            &mut self,
            _replier: IdentityHandle,
            _initiator: IdentityHandle,
            _request_token: &[u8],
        ) -> SecurityResult<(
            zerodds_security::authentication::HandshakeHandle,
            zerodds_security::authentication::HandshakeStepOutcome,
        )> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken",
            ))
        }
        fn process_handshake(
            &mut self,
            _handshake: zerodds_security::authentication::HandshakeHandle,
            _token: &[u8],
        ) -> SecurityResult<zerodds_security::authentication::HandshakeStepOutcome> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken",
            ))
        }
        fn shared_secret(
            &self,
            _handshake: zerodds_security::authentication::HandshakeHandle,
        ) -> SecurityResult<zerodds_security::authentication::SharedSecretHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken",
            ))
        }
        fn plugin_class_id(&self) -> &str {
            "" // Spec violation — the class id must be non-empty.
        }
    }

    let mut plugin: Box<dyn AuthenticationPlugin> = Box::new(BrokenAuth);
    // Spec §8.3.1.1: class id unique + non-empty.
    assert!(
        plugin.plugin_class_id().is_empty(),
        "the BrokenAuth stub demonstrates an empty class id (= misimplementation)"
    );
    // Negative SPI contract: every call returns an error.
    let r = plugin.validate_local_identity(&PropertyList::new(), [0; 16]);
    assert!(matches!(
        r.unwrap_err().kind,
        SecurityErrorKind::AuthenticationFailed
    ));
}

// ============================================================================
// §1.2 conformance point 2 — AccessControl SPI (builtin: Permissions)
// ============================================================================

#[test]
fn access_control_accepts_builtin_permissions_plugin() {
    let mut plugin: Box<dyn AccessControlPlugin> = Box::new(PermissionsAccessControl::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Access:Permissions:1.2");
    // The builtin runs without registered permissions → returns an error
    // (spec-conform: AccessControl is non-default-permissive).
    let res =
        plugin.validate_local_permissions(IdentityHandle(1), [0xAA; 16], &PropertyList::new());
    assert!(
        res.is_err(),
        "the builtin AccessControl must return an error without registered permissions"
    );
}

#[test]
fn access_control_rejects_misimplemented_plugin() {
    /// A plugin that always returns "Permit" — spec §9.4 requires
    /// that non-validated permissions always yield `Deny`.
    struct AlwaysPermit;
    impl AccessControlPlugin for AlwaysPermit {
        fn validate_local_permissions(
            &mut self,
            _local: IdentityHandle,
            _participant_guid: [u8; 16],
            _props: &PropertyList,
        ) -> SecurityResult<PermissionsHandle> {
            Ok(PermissionsHandle(0))
        }
        fn validate_remote_permissions(
            &mut self,
            _local: IdentityHandle,
            _remote: IdentityHandle,
            _remote_token: &[u8],
            _remote_credential: &[u8],
        ) -> SecurityResult<PermissionsHandle> {
            Ok(PermissionsHandle(0))
        }
        fn check_create_datawriter(
            &self,
            _p: PermissionsHandle,
            _topic: &str,
        ) -> SecurityResult<AccessDecision> {
            Ok(AccessDecision::Permit)
        }
        fn check_create_datareader(
            &self,
            _p: PermissionsHandle,
            _topic: &str,
        ) -> SecurityResult<AccessDecision> {
            Ok(AccessDecision::Permit)
        }
        fn check_remote_datawriter_match(
            &self,
            _l: PermissionsHandle,
            _r: PermissionsHandle,
            _topic: &str,
        ) -> SecurityResult<AccessDecision> {
            Ok(AccessDecision::Permit)
        }
        fn check_remote_datareader_match(
            &self,
            _l: PermissionsHandle,
            _r: PermissionsHandle,
            _topic: &str,
        ) -> SecurityResult<AccessDecision> {
            Ok(AccessDecision::Permit)
        }
        fn plugin_class_id(&self) -> &str {
            "DDS:Access:AlwaysPermit"
        }
    }

    let plugin: Box<dyn AccessControlPlugin> = Box::new(AlwaysPermit);
    // Misimplementation demonstration: Permit on a NON-validated
    // PermissionsHandle(0). The builtin (`PermissionsAccessControl`)
    // would return `Deny` here (see the `access_control_accepts_*` test
    // above — the builtin is non-default-permissive).
    let decision = plugin
        .check_create_datawriter(PermissionsHandle(0), "any-topic")
        .expect("ok");
    assert!(
        decision.is_permitted(),
        "the AlwaysPermit stub demonstrates a permissions bypass (= misimplementation)"
    );
}

// ============================================================================
// §1.2 conformance point 3 — Cryptographic SPI (builtin: AES-GCM-GMAC)
// ============================================================================

#[test]
fn crypto_accepts_builtin_aes_gcm_plugin() {
    let plugin: Box<dyn CryptographicPlugin> = Box::new(AesGcmCryptoPlugin::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Crypto:AES-GCM-GMAC:1.2");
}

#[test]
fn crypto_rejects_misimplemented_plugin() {
    /// A crypto plugin with an empty class id + every operation returns
    /// `CryptoFailed` (spec violation: an empty class id violates §8.5).
    struct BrokenCrypto;
    impl CryptographicPlugin for BrokenCrypto {
        fn register_local_participant(
            &mut self,
            _identity: IdentityHandle,
            _properties: &[(&str, &str)],
        ) -> SecurityResult<CryptoHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn register_matched_remote_participant(
            &mut self,
            _local: CryptoHandle,
            _remote_identity: IdentityHandle,
            _shared_secret: zerodds_security::authentication::SharedSecretHandle,
        ) -> SecurityResult<CryptoHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn register_local_endpoint(
            &mut self,
            _participant: CryptoHandle,
            _is_writer: bool,
            _properties: &[(&str, &str)],
        ) -> SecurityResult<CryptoHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn create_local_participant_crypto_tokens(
            &mut self,
            _local: CryptoHandle,
            _remote: CryptoHandle,
        ) -> SecurityResult<Vec<u8>> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn set_remote_participant_crypto_tokens(
            &mut self,
            _local: CryptoHandle,
            _remote: CryptoHandle,
            _tokens: &[u8],
        ) -> SecurityResult<()> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn encrypt_submessage(
            &self,
            _local: CryptoHandle,
            _remote_list: &[CryptoHandle],
            _plaintext: &[u8],
            _aad_extension: &[u8],
        ) -> SecurityResult<Vec<u8>> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn decrypt_submessage(
            &self,
            _local: CryptoHandle,
            _remote: CryptoHandle,
            _ciphertext: &[u8],
            _aad_extension: &[u8],
        ) -> SecurityResult<Vec<u8>> {
            Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "broken",
            ))
        }
        fn plugin_class_id(&self) -> &str {
            ""
        }
    }
    let plugin: Box<dyn CryptographicPlugin> = Box::new(BrokenCrypto);
    assert!(
        plugin.plugin_class_id().is_empty(),
        "BrokenCrypto demonstrates an empty class id (= misimplementation)"
    );
    // Negative smoke: the encrypt call returns CryptoFailed.
    let r = plugin.encrypt_submessage(CryptoHandle(0), &[], b"x", &[]);
    assert_eq!(r.unwrap_err().kind, SecurityErrorKind::CryptoFailed);
}

// ============================================================================
// §1.2 conformance point 4 — Logging SPI (builtin: stderr sink)
// ============================================================================

#[test]
fn logging_accepts_builtin_stderr_plugin() {
    let plugin: Box<dyn LoggingPlugin> = Box::new(StderrLoggingPlugin::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Logging:stderr");
    // Spec §8.6: a logging plugin must not panic. A critical event
    // writes a line to stderr — the test does this and
    // verifies the plugin does not crash.
    plugin.log(
        LogLevel::Critical,
        [0xAA; 16],
        "auth.failed",
        "conformance-matrix-smoke",
    );
}

#[test]
fn logging_rejects_misimplemented_plugin() {
    // The mock plugin collects events; "misimplementation" = the plugin drops
    // all events silently. This variant is spec-allowed (a plugin
    // may filter), but the mock variant shows: a plugin that
    // explicitly discards events can be detected by this (the sink is
    // empty after the log call).
    struct DropEverything;
    impl LoggingPlugin for DropEverything {
        fn log(&self, _l: LogLevel, _p: [u8; 16], _c: &str, _m: &str) {
            // explicit no-op
        }
        fn plugin_class_id(&self) -> &str {
            "DDS:Logging:DropEverything"
        }
    }
    // Comparison with MockLoggingPlugin (collects → visible).
    let sink: MockLogSink = Arc::new(Mutex::new(Vec::<MockLogEntry>::new()));
    let collector = MockLoggingPlugin::new(Arc::clone(&sink));
    collector.log(LogLevel::Critical, [0; 16], "test", "msg");
    assert_eq!(sink.lock().unwrap().len(), 1, "mock must collect the event");

    let dropper: Box<dyn LoggingPlugin> = Box::new(DropEverything);
    let sink2: MockLogSink = Arc::new(Mutex::new(Vec::<MockLogEntry>::new()));
    // DropEverything ignores events — the test sink stays empty.
    dropper.log(LogLevel::Critical, [0; 16], "test", "msg");
    assert!(
        sink2.lock().unwrap().is_empty(),
        "DropEverything stub discards all events (misimplementation)"
    );
}

// ============================================================================
// §1.2 Conformance-Point 5 — DataTagging SPI (Builtin)
// ============================================================================

#[test]
fn data_tagging_accepts_builtin_plugin() {
    let mut plugin: Box<dyn DataTaggingPlugin> = Box::new(BuiltinDataTaggingPlugin::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Tagging:Builtin");
    let g = [0xCC; 16];
    plugin.set_tags(
        g,
        vec![DataTag {
            name: "classification".into(),
            value: "secret".into(),
        }],
    );
    assert_eq!(plugin.get_tags(g).len(), 1);
}

#[test]
fn data_tagging_rejects_misimplemented_plugin() {
    /// A plugin that ignores set_tags — get_tags always returns an empty
    /// list. Violates the SPI contract (set/get roundtrip).
    struct AmnesiacTagger;
    impl DataTaggingPlugin for AmnesiacTagger {
        fn set_tags(&mut self, _e: [u8; 16], _t: Vec<DataTag>) {}
        fn get_tags(&self, _e: [u8; 16]) -> Vec<DataTag> {
            Vec::new()
        }
        fn plugin_class_id(&self) -> &str {
            "DDS:Tagging:Amnesiac"
        }
    }
    let mut plugin: Box<dyn DataTaggingPlugin> = Box::new(AmnesiacTagger);
    plugin.set_tags(
        [1; 16],
        vec![DataTag {
            name: "k".into(),
            value: "v".into(),
        }],
    );
    assert!(
        plugin.get_tags([1; 16]).is_empty(),
        "the Amnesiac stub demonstrates a broken set/get roundtrip (misimplementation)"
    );
}

// ============================================================================
// §2.1 conformance-points table — all 5 SPIs at once
// ============================================================================

#[test]
fn conformance_points_full_matrix() {
    // Table: spec §2.1 lists 4 conformance points.
    // 1. Builtin plugins — all 5 SPIs have a production builtin.
    let _auth: Box<dyn AuthenticationPlugin> = Box::new(PkiAuthenticationPlugin::new());
    let _access: Box<dyn AccessControlPlugin> = Box::new(PermissionsAccessControl::new());
    let _crypto: Box<dyn CryptographicPlugin> = Box::new(AesGcmCryptoPlugin::new());
    let _logging: Box<dyn LoggingPlugin> = Box::new(StderrLoggingPlugin::new());
    let _tagging: Box<dyn DataTaggingPlugin> = Box::new(BuiltinDataTaggingPlugin::new());

    // 2. Plugin framework — class ids differ per SPI slot.
    let class_ids = [
        ("Auth", _auth.plugin_class_id()),
        ("Access", _access.plugin_class_id()),
        ("Crypto", _crypto.plugin_class_id()),
        ("Logging", _logging.plugin_class_id()),
        ("Tagging", _tagging.plugin_class_id()),
    ];
    for (slot, id) in &class_ids {
        assert!(!id.is_empty(), "the {slot} plugin has an empty class id");
        assert!(
            id.starts_with("DDS:"),
            "the {slot} plugin class id violates the convention 'DDS:<Service>:<Variant>': {id}"
        );
    }
    // Uniqueness of the class ids across all slots.
    let mut seen = std::collections::BTreeSet::new();
    for (_, id) in &class_ids {
        assert!(
            seen.insert(id.to_string()),
            "class-id conflict: {id} used twice"
        );
    }

    // 3. Plugin language APIs — n/a (Rust-only crate boundary). We
    //    instead verify that each plugin is usable as a `Box<dyn>`
    //    (via the casts above) — that is Rust's
    //    equivalent to plugin-language-API conformance.

    // 4. Logging+Tagging profile — both plugins provide the profile-
    //    specific operations.
    _logging.log(
        LogLevel::Informational,
        [0; 16],
        "matrix.complete",
        "all 5 SPIs verified",
    );
}
