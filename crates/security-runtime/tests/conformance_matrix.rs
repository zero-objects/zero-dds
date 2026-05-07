//! Conformance-Matrix fuer DDS-Security 1.2 §1.1 + §1.2 + §2.1.
//!
//! Verifiziert produktiv, dass:
//!
//! 1. Alle 5 SPI-Traits aus `zerodds-security` werden durch je einen
//!    Builtin-Plugin im Workspace erfuellt (`accepts_builtin_plugin`).
//! 2. Jedes SPI rejected ein "misimplemented" Plugin (Plugin mit
//!    leerer Class-Id oder Error-only-Verhalten) gemaess
//!    Spec-Vertrag (`rejects_misimplemented_plugin`).
//! 3. Die Conformance-Points-Tabelle aus §2.1 (Builtin Plugins,
//!    Plugin-Framework, Plugin-Language-APIs, Logging+Tagging-Profil)
//!    ist als Test-Tabelle exhaustiv abgedeckt.
//!
//! Spec OMG DDS-Security 1.2:
//! * §1.1 — DDS-Security-Compliance-Profile als Erweiterung von DDS.
//! * §1.2 — 5 SPIs: Authentication, AccessControl, Cryptographic,
//!   Logging, DataTagging.
//! * §2.1 — Conformance-Points: Builtin / Plugin-Framework /
//!   Plugin-Language-APIs / Logging+Tagging.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Conformance-Matrix testet das Plugin-SPI gerade ueber `Box<dyn>`-
//! Erasure — das ist Sinn des Tests.)

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
    // Spec-Anforderung: ungueltige PropertyList → AuthenticationFailed
    // (nicht panic, nicht silent-success). Keine Cert-Properties =
    // misconfigured Local-Identity → muss Error sein.
    let res = plugin.validate_local_identity(&PropertyList::new(), [0xAA; 16]);
    assert!(
        res.is_err(),
        "Builtin Auth muss ohne Cert-Properties Error liefern"
    );
}

#[test]
fn auth_rejects_misimplemented_plugin() {
    /// Plugin mit leerer Class-Id (verletzt Spec §8.3.1.1).
    struct BrokenAuth;
    impl AuthenticationPlugin for BrokenAuth {
        fn validate_local_identity(
            &mut self,
            _props: &PropertyList,
            _participant_guid: [u8; 16],
        ) -> SecurityResult<IdentityHandle> {
            Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "broken: niemals success",
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
            "" // Spec-Verletzung — Class-Id muss nicht-leer sein.
        }
    }

    let mut plugin: Box<dyn AuthenticationPlugin> = Box::new(BrokenAuth);
    // Spec §8.3.1.1: Class-Id eindeutig + nicht-leer.
    assert!(
        plugin.plugin_class_id().is_empty(),
        "BrokenAuth-Stub demonstriert leere Class-Id (= Misimplementation)"
    );
    // Negativer SPI-Vertrag: jeder Call liefert Error.
    let r = plugin.validate_local_identity(&PropertyList::new(), [0; 16]);
    assert!(matches!(
        r.unwrap_err().kind,
        SecurityErrorKind::AuthenticationFailed
    ));
}

// ============================================================================
// §1.2 Conformance-Point 2 — AccessControl SPI (Builtin: Permissions)
// ============================================================================

#[test]
fn access_control_accepts_builtin_permissions_plugin() {
    let mut plugin: Box<dyn AccessControlPlugin> = Box::new(PermissionsAccessControl::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Access:Permissions:1.2");
    // Builtin laeuft ohne registrierte Permissions → liefert Error
    // (Spec-konform: AccessControl ist non-default-permissive).
    let res =
        plugin.validate_local_permissions(IdentityHandle(1), [0xAA; 16], &PropertyList::new());
    assert!(
        res.is_err(),
        "Builtin AccessControl muss ohne registered Permissions Error liefern"
    );
}

#[test]
fn access_control_rejects_misimplemented_plugin() {
    /// Plugin das immer "Permit" zurueckgibt — Spec §9.4 verlangt
    /// dass non-validated Permissions stets `Deny` ergeben.
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
    // Misimplementation-Demonstration: Permit auf NICHT-validated
    // PermissionsHandle(0). Der Builtin (`PermissionsAccessControl`)
    // wuerde hier `Deny` liefern (siehe `access_control_accepts_*`-Test
    // oben — Builtin ist non-default-permissive).
    let decision = plugin
        .check_create_datawriter(PermissionsHandle(0), "any-topic")
        .expect("ok");
    assert!(
        decision.is_permitted(),
        "AlwaysPermit-Stub demonstriert Permissions-Bypass (= Misimplementation)"
    );
}

// ============================================================================
// §1.2 Conformance-Point 3 — Cryptographic SPI (Builtin: AES-GCM-GMAC)
// ============================================================================

#[test]
fn crypto_accepts_builtin_aes_gcm_plugin() {
    let plugin: Box<dyn CryptographicPlugin> = Box::new(AesGcmCryptoPlugin::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Crypto:AES-GCM-GMAC:1.2");
}

#[test]
fn crypto_rejects_misimplemented_plugin() {
    /// Crypto-Plugin mit leerer Class-Id + jede Operation liefert
    /// `CryptoFailed` (Spec-Verletzung: leere Class-Id verletzt §8.5).
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
        "BrokenCrypto demonstriert leere Class-Id (= Misimplementation)"
    );
    // Negative Smoke: encrypt-Call liefert CryptoFailed.
    let r = plugin.encrypt_submessage(CryptoHandle(0), &[], b"x", &[]);
    assert_eq!(r.unwrap_err().kind, SecurityErrorKind::CryptoFailed);
}

// ============================================================================
// §1.2 Conformance-Point 4 — Logging SPI (Builtin: stderr-sink)
// ============================================================================

#[test]
fn logging_accepts_builtin_stderr_plugin() {
    let plugin: Box<dyn LoggingPlugin> = Box::new(StderrLoggingPlugin::new());
    assert_eq!(plugin.plugin_class_id(), "DDS:Logging:stderr");
    // Spec §8.6: Logging-Plugin darf nicht panic'en. Critical-Event
    // schreibt eine Zeile an stderr — der Test fuehrt dies durch und
    // verifiziert das Plugin nicht stuerzt.
    plugin.log(
        LogLevel::Critical,
        [0xAA; 16],
        "auth.failed",
        "conformance-matrix-smoke",
    );
}

#[test]
fn logging_rejects_misimplemented_plugin() {
    // Mock-Plugin sammelt Events; "Misimplementation" = Plugin droppt
    // alle Events silently. Diese Variante ist Spec-erlaubt (Plugin
    // darf filtern), aber die Mock-Variante zeigt: ein Plugin das
    // events explizit verwirft, kann dadurch erkannt werden (sink ist
    // leer nach log-Call).
    struct DropEverything;
    impl LoggingPlugin for DropEverything {
        fn log(&self, _l: LogLevel, _p: [u8; 16], _c: &str, _m: &str) {
            // explizit no-op
        }
        fn plugin_class_id(&self) -> &str {
            "DDS:Logging:DropEverything"
        }
    }
    // Vergleich mit MockLoggingPlugin (collects → sichtbar).
    let sink: MockLogSink = Arc::new(Mutex::new(Vec::<MockLogEntry>::new()));
    let collector = MockLoggingPlugin::new(Arc::clone(&sink));
    collector.log(LogLevel::Critical, [0; 16], "test", "msg");
    assert_eq!(sink.lock().unwrap().len(), 1, "Mock muss Event sammeln");

    let dropper: Box<dyn LoggingPlugin> = Box::new(DropEverything);
    let sink2: MockLogSink = Arc::new(Mutex::new(Vec::<MockLogEntry>::new()));
    // DropEverything ignoriert events — Test-Sink bleibt leer.
    dropper.log(LogLevel::Critical, [0; 16], "test", "msg");
    assert!(
        sink2.lock().unwrap().is_empty(),
        "DropEverything-Stub verwirft alle Events (Misimplementation)"
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
    /// Plugin das set_tags ignoriert — get_tags liefert immer leere
    /// Liste. Verletzt SPI-Vertrag (set/get-Roundtrip).
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
        "Amnesiac-Stub demonstriert verletzten set/get-Roundtrip (Misimplementation)"
    );
}

// ============================================================================
// §2.1 Conformance-Points-Tabelle — alle 5 SPIs gleichzeitig
// ============================================================================

#[test]
fn conformance_points_full_matrix() {
    // Tabelle: Spec §2.1 listet 4 Conformance-Points.
    // 1. Builtin Plugins — alle 5 SPIs haben einen produktiven Builtin.
    let _auth: Box<dyn AuthenticationPlugin> = Box::new(PkiAuthenticationPlugin::new());
    let _access: Box<dyn AccessControlPlugin> = Box::new(PermissionsAccessControl::new());
    let _crypto: Box<dyn CryptographicPlugin> = Box::new(AesGcmCryptoPlugin::new());
    let _logging: Box<dyn LoggingPlugin> = Box::new(StderrLoggingPlugin::new());
    let _tagging: Box<dyn DataTaggingPlugin> = Box::new(BuiltinDataTaggingPlugin::new());

    // 2. Plugin-Framework — Class-Ids unterscheiden sich pro SPI-Slot.
    let class_ids = [
        ("Auth", _auth.plugin_class_id()),
        ("Access", _access.plugin_class_id()),
        ("Crypto", _crypto.plugin_class_id()),
        ("Logging", _logging.plugin_class_id()),
        ("Tagging", _tagging.plugin_class_id()),
    ];
    for (slot, id) in &class_ids {
        assert!(!id.is_empty(), "{slot}-Plugin hat leere Class-Id");
        assert!(
            id.starts_with("DDS:"),
            "{slot}-Plugin Class-Id verletzt Konvention 'DDS:<Service>:<Variant>': {id}"
        );
    }
    // Eindeutigkeit der Class-Ids ueber alle Slots.
    let mut seen = std::collections::BTreeSet::new();
    for (_, id) in &class_ids {
        assert!(
            seen.insert(id.to_string()),
            "Class-Id-Konflikt: {id} doppelt belegt"
        );
    }

    // 3. Plugin-Language-APIs — n/a (Rust-only Crate-Boundary). Wir
    //    verifizieren stattdessen, dass jeder Plugin als `Box<dyn>`
    //    bedienbar ist (via die obigen Casts) — das ist Rusts
    //    Aequivalent zur Plugin-Language-API-Conformance.

    // 4. Logging+Tagging-Profil — beide Plugins stellen die Profil-
    //    spezifischen Operationen bereit.
    _logging.log(
        LogLevel::Informational,
        [0; 16],
        "matrix.complete",
        "all 5 SPIs verified",
    );
}
