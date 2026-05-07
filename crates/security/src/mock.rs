// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mock-Plugins fuer Tests.
//!
//! Die Mocks akzeptieren **jeden** Peer und simulieren einen
//! Handshake in genau zwei Schritten. Niemals fuer Produktion — sie
//! liefern keine echte Crypto.
//!
//! Zweck:
//! 1. Das SPI-Interface gegen einen tatsaechlich funktionierenden Flow
//!    validieren (Signature-Checks, Handshake-State-Machine).
//! 2. DCPS-Layer kann ab v1.4 gegen den Mock sub-testen, bevor der
//!    Produktions-Plugin fertig ist.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Tests instanziieren `Box<dyn AuthenticationPlugin>`.)

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use alloc::borrow::ToOwned;
#[cfg(feature = "std")]
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::access_control::{AccessControlPlugin, AccessDecision, PermissionsHandle};
use crate::authentication::{
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle, SharedSecretHandle,
};
use crate::data_tagging::{DataTag, DataTaggingPlugin};
use crate::error::{SecurityError, SecurityErrorKind, SecurityResult};
use crate::logging::{LogLevel, LoggingPlugin};
use crate::properties::PropertyList;

// ============================================================================
// MockAuthenticationPlugin
// ============================================================================

/// Mock-Implementation — akzeptiert alles, Handshake-Step-Count
/// hard-coded.
#[derive(Debug, Default)]
pub struct MockAuthenticationPlugin {
    next_handle: AtomicU64,
    handshakes: BTreeMap<HandshakeHandle, SharedSecretHandle>,
}

impl MockAuthenticationPlugin {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&self) -> u64 {
        // `fetch_add` auf AtomicU64 — kein `&mut self` benoetigt,
        // daher koennen `validate_*` read-only-Semantik implizieren.
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl AuthenticationPlugin for MockAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        _props: &PropertyList,
        _participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle> {
        Ok(IdentityHandle(self.next_id()))
    }

    fn validate_remote_identity(
        &mut self,
        _local: IdentityHandle,
        _remote_participant_guid: [u8; 16],
        _remote_auth_token: &[u8],
    ) -> SecurityResult<IdentityHandle> {
        Ok(IdentityHandle(self.next_id()))
    }

    fn begin_handshake_request(
        &mut self,
        _initiator: IdentityHandle,
        _replier: IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let h = HandshakeHandle(self.next_id());
        // Mock-Handshake: Request-Token ist ein Fixtext.
        Ok((
            h,
            HandshakeStepOutcome::SendMessage {
                token: b"MOCK-REQUEST".to_vec(),
            },
        ))
    }

    fn begin_handshake_reply(
        &mut self,
        _replier: IdentityHandle,
        _initiator: IdentityHandle,
        request_token: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        if request_token != b"MOCK-REQUEST" {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "mock: unerwartetes Request-Token",
            ));
        }
        let h = HandshakeHandle(self.next_id());
        // Reply-Token, nach dessen Empfang der Initiator den Handshake
        // abschliessen kann.
        Ok((
            h,
            HandshakeStepOutcome::SendMessage {
                token: b"MOCK-REPLY".to_vec(),
            },
        ))
    }

    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        if token == b"MOCK-REPLY" {
            let secret = SharedSecretHandle(self.next_id());
            self.handshakes.insert(handshake, secret);
            return Ok(HandshakeStepOutcome::Complete { secret });
        }
        if token == b"MOCK-FINAL-ACK" {
            // Replier-Seite abgeschlossen.
            let secret = self
                .handshakes
                .get(&handshake)
                .copied()
                .unwrap_or(SharedSecretHandle(self.next_id()));
            return Ok(HandshakeStepOutcome::Complete { secret });
        }
        Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "mock: unbekanntes handshake-token",
        ))
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        self.handshakes.get(&handshake).copied().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: handshake-handle unbekannt",
            )
        })
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Auth:Mock"
    }
}

// ============================================================================
// MockAccessControlPlugin — jedes Topic erlaubt (Permit-Everything).
// ============================================================================

/// Mock-Access-Control: erlaubt alles. Nur fuer Tests.
#[derive(Debug, Default)]
pub struct MockAccessControlPlugin {
    next_handle: AtomicU64,
}

impl MockAccessControlPlugin {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl AccessControlPlugin for MockAccessControlPlugin {
    fn validate_local_permissions(
        &mut self,
        _local: IdentityHandle,
        _participant_guid: [u8; 16],
        _props: &PropertyList,
    ) -> SecurityResult<PermissionsHandle> {
        Ok(PermissionsHandle(self.next_id()))
    }

    fn validate_remote_permissions(
        &mut self,
        _local: IdentityHandle,
        _remote: IdentityHandle,
        _remote_permissions_token: &[u8],
        _remote_credential: &[u8],
    ) -> SecurityResult<PermissionsHandle> {
        Ok(PermissionsHandle(self.next_id()))
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
        "DDS:Access:Mock"
    }
}

// ============================================================================
// MockLoggingPlugin — sammelt Events in einem Vec fuer Test-Assertions
// ============================================================================

/// Ein Log-Eintrag — fuer Test-Assertions gesammelt.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockLogEntry {
    /// Severity.
    pub level: LogLevel,
    /// Participant-GUID (16 octets).
    pub participant: [u8; 16],
    /// Category-String.
    pub category: String,
    /// Message.
    pub message: String,
}

/// Shared Sink-Typ — `Arc<Mutex<Vec<MockLogEntry>>>`.
#[cfg(feature = "std")]
pub type MockLogSink = std::sync::Arc<std::sync::Mutex<Vec<MockLogEntry>>>;

/// Mock-Logger: sammelt alle Events in einem `MockLogSink`, damit Tests
/// die Events nachtraeglich inspizieren koennen.
#[cfg(feature = "std")]
pub struct MockLoggingPlugin {
    sink: MockLogSink,
}

#[cfg(feature = "std")]
impl MockLoggingPlugin {
    /// Konstruktor.
    #[must_use]
    pub fn new(sink: MockLogSink) -> Self {
        Self { sink }
    }
}

#[cfg(feature = "std")]
impl LoggingPlugin for MockLoggingPlugin {
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        if let Ok(mut v) = self.sink.lock() {
            v.push(MockLogEntry {
                level,
                participant,
                category: category.to_owned(),
                message: message.to_owned(),
            });
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:Mock"
    }
}

// ============================================================================
// MockDataTaggingPlugin — minimaler Tag-Store fuer Tests
// ============================================================================

/// Mock-DataTagging-Plugin: speichert Tag-Listen pro Endpoint-GUID in
/// einer In-Memory-Map. Liefert auf Unknown-GUID einen leeren Vec
/// (Spec-konformer Default).
#[derive(Debug, Default)]
pub struct MockDataTaggingPlugin {
    tags: BTreeMap<[u8; 16], Vec<DataTag>>,
}

impl MockDataTaggingPlugin {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl DataTaggingPlugin for MockDataTaggingPlugin {
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>) {
        self.tags.insert(endpoint_guid, tags);
    }

    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag> {
        self.tags.get(&endpoint_guid).cloned().unwrap_or_default()
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Tagging:Mock"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn mock_authentication_end_to_end_handshake() {
        // Zwei Plugins simulieren zwei Participants.
        let mut alice = MockAuthenticationPlugin::new();
        let mut bob = MockAuthenticationPlugin::new();

        let alice_id = alice
            .validate_local_identity(&PropertyList::new(), [0xAA; 16])
            .expect("alice identity");
        let bob_id = bob
            .validate_local_identity(&PropertyList::new(), [0xBB; 16])
            .expect("bob identity");

        // Alice sieht Bob via SPDP.
        let bob_remote_at_alice = alice
            .validate_remote_identity(alice_id, [0xBB; 16], b"mock-bob-token")
            .expect("bob-remote");
        let alice_remote_at_bob = bob
            .validate_remote_identity(bob_id, [0xAA; 16], b"mock-alice-token")
            .expect("alice-remote");

        // Alice startet Handshake.
        let (alice_h, outcome1) = alice
            .begin_handshake_request(alice_id, bob_remote_at_alice)
            .expect("request");
        let request_token = match outcome1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            other => panic!("erwartet SendMessage, got {other:?}"),
        };

        // Bob antwortet.
        let (bob_h, outcome2) = bob
            .begin_handshake_reply(bob_id, alice_remote_at_bob, &request_token)
            .expect("reply");
        let reply_token = match outcome2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            other => panic!("erwartet SendMessage, got {other:?}"),
        };

        // Alice verarbeitet Reply → Complete.
        let outcome3 = alice
            .process_handshake(alice_h, &reply_token)
            .expect("proc");
        let alice_secret = match outcome3 {
            HandshakeStepOutcome::Complete { secret } => secret,
            other => panic!("erwartet Complete, got {other:?}"),
        };

        // Bob-Seite: final-ack abschliessen.
        let outcome4 = bob
            .process_handshake(bob_h, b"MOCK-FINAL-ACK")
            .expect("proc bob");
        assert!(matches!(outcome4, HandshakeStepOutcome::Complete { .. }));

        // Secret-Handle auf Alice-Seite ist queryable.
        let fetched = alice.shared_secret(alice_h).expect("fetch");
        assert_eq!(fetched, alice_secret);
    }

    #[test]
    fn mock_access_control_permits_everything() {
        let mut ac = MockAccessControlPlugin::new();
        let local = IdentityHandle(1);
        let perms = ac
            .validate_local_permissions(local, [0xAA; 16], &PropertyList::new())
            .expect("perms");
        assert!(
            ac.check_create_datawriter(perms, "Chatter")
                .unwrap()
                .is_permitted()
        );
        assert!(
            ac.check_create_datareader(perms, "Chatter")
                .unwrap()
                .is_permitted()
        );
    }

    #[test]
    fn auth_plugin_can_be_boxed_as_trait_object() {
        let plugin: Box<dyn AuthenticationPlugin> = Box::new(MockAuthenticationPlugin::new());
        assert_eq!(plugin.plugin_class_id(), "DDS:Auth:Mock");
    }

    #[test]
    fn mock_data_tagging_set_get_roundtrip() {
        let mut tagger = MockDataTaggingPlugin::new();
        let g = [0xAB; 16];
        let tags = alloc::vec![DataTag {
            name: "classification".into(),
            value: "secret".into(),
        }];
        tagger.set_tags(g, tags.clone());
        assert_eq!(tagger.get_tags(g), tags);
        assert!(tagger.get_tags([0xCD; 16]).is_empty());
        assert_eq!(tagger.plugin_class_id(), "DDS:Tagging:Mock");
    }

    #[cfg(feature = "std")]
    #[test]
    fn mock_logging_captures_events() {
        use std::sync::{Arc, Mutex};
        let sink = Arc::new(Mutex::new(Vec::new()));
        let logger = MockLoggingPlugin::new(Arc::clone(&sink));
        logger.log(LogLevel::Critical, [0u8; 16], "auth.failed", "bad cert");
        let captured = sink.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].level, LogLevel::Critical);
        assert_eq!(captured[0].category, "auth.failed");
        assert_eq!(captured[0].message, "bad cert");
    }
}
