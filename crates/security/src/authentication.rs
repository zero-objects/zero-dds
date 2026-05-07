// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Authentication-Plugin SPI (OMG DDS-Security 1.1 §8.3).
//!
//! Verantwortlich fuer:
//! 1. **Identity-Validation** beim Participant-Start — lokale Identity
//!    (z.B. X.509-Cert + Private-Key) gegen Trust-Anchor pruefen.
//! 2. **Identity-Handshake** zwischen zwei Participants — Challenge/
//!    Response mit signierten Noncen, Spec §8.3.2.
//! 3. **SharedSecret-Erzeugung** am Ende des Handshakes — Eingabe
//!    fuers CryptographicPlugin fuer Key-Derivation.
//!
//! Der SPI ist **State-Machine-Light** — das Plugin haelt den
//! Handshake-State selbst. Der Caller (DCPS-Layer) triggert nur
//! `begin_handshake_request/reply`, `process_handshake_reply`,
//! `process_handshake_final`.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin-SPI benötigt `Box<dyn AuthenticationPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::error::SecurityResult;
use crate::properties::PropertyList;

/// Opaker Handle fuer eine validierte Identity. Der Plugin-Interne
/// Zustand (X.509-Cert, Keys) wird nicht ueber diesen Handle nach
/// aussen exponiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityHandle(pub u64);

/// Opaker Handle fuer einen laufenden Handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandshakeHandle(pub u64);

/// Opaker Handle fuer ein geteiltes Geheimnis (Ausgabe eines
/// abgeschlossenen Handshakes). Wird an `CryptographicPlugin`
/// weitergereicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedSecretHandle(pub u64);

/// Lookup-Bruecke zwischen [`AuthenticationPlugin`] und
/// [`crate::crypto::CryptographicPlugin`].
///
/// Der Authentication-Plugin produziert nach Handshake einen
/// [`SharedSecretHandle`] und kennt die zugehoerigen 32 Byte aus
/// `x25519 + HKDF-SHA256`. Der Crypto-Plugin braucht genau diese
/// Bytes um seinen per-peer Master-Key abzuleiten — statt einen
/// random Key zu generieren und als opaken Token durch die
/// Governance zu routen.
///
/// Implementierer muessen **Send + Sync** sein, damit der Crypto-
/// Plugin den Provider via `Arc<dyn SharedSecretProvider>` halten
/// kann.
pub trait SharedSecretProvider: Send + Sync {
    /// Liefert die rohen Bytes des geteilten Schluessels.
    /// `None` wenn der Handle unbekannt ist (Handshake noch nicht
    /// abgeschlossen oder bereits verworfen).
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<alloc::vec::Vec<u8>>;
}

/// Ergebnis eines Handshake-Steps.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HandshakeStepOutcome {
    /// Plugin will eine weitere Nachricht an den Peer senden.
    SendMessage {
        /// Opaker Handshake-Token als Byte-Blob. Geht 1:1 in die
        /// `AuthHandshake`-Builtin-Submessage (Spec §9.3.2.3).
        token: Vec<u8>,
    },
    /// Handshake erfolgreich abgeschlossen — `SharedSecretHandle`
    /// verwendbar.
    Complete {
        /// Geteilter Schluessel-Handle fuer den CryptographicPlugin.
        secret: SharedSecretHandle,
    },
    /// Handshake brauch noch Nachricht vom Peer — nichts zu tun.
    WaitingForPeer,
}

/// Authentication-Plugin-Trait. Spec §8.3.2.7.
pub trait AuthenticationPlugin: Send + Sync {
    /// Wird einmal beim Participant-Start aufgerufen: lokale Identity
    /// validieren (Zertifikat, Key, Trust-Anchor) und einen Handle
    /// zurueckgeben.
    ///
    /// # Spec
    /// §8.3.2.7.1 `validate_local_identity`.
    fn validate_local_identity(
        &mut self,
        props: &PropertyList,
        participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle>;

    /// Wird aufgerufen, sobald via SPDP ein Remote-Participant entdeckt
    /// wurde. Plugin validiert das Remote-Cert (aus `remote_auth_token`)
    /// gegen seinen Trust-Store.
    ///
    /// # Spec
    /// §8.3.2.7.2 `validate_remote_identity`.
    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        remote_auth_token: &[u8],
    ) -> SecurityResult<IdentityHandle>;

    /// Startet den Handshake. Liefert das erste Token, das an den
    /// Peer gesendet werden muss.
    ///
    /// # Spec
    /// §8.3.2.7.3 `begin_handshake_request`.
    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        replier: IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>;

    /// Peer-Seite des Handshake-Starts. `request_token` ist was der
    /// Initiator per `begin_handshake_request` geschickt hat.
    ///
    /// # Spec
    /// §8.3.2.7.4 `begin_handshake_reply`.
    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        initiator: IdentityHandle,
        request_token: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>;

    /// Folge-Nachrichten des Handshakes durchreichen.
    ///
    /// # Spec
    /// §8.3.2.7.5 `process_handshake`.
    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome>;

    /// Beendet den Handshake und liefert den finalen SharedSecret.
    /// Fehlschlag bricht ab. Wird nach `Complete`-Outcome durch den
    /// Caller aufgerufen, um den Secret aus dem Plugin zu ziehen.
    ///
    /// Alternative: der `Complete`-Outcome enthaelt den Handle
    /// bereits — diese Methode ist nur fuer Polling-Integrations.
    ///
    /// # Spec
    /// §8.3.2.7.8 `get_shared_secret`.
    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle>;

    /// Identity-Plugin-Name (z.B. "DDS:Auth:PKI-DH:1.2"). Wird in SPDP als
    /// `dds.sec.auth.plugin_class` annonciert.
    fn plugin_class_id(&self) -> &str;

    /// Liefert das `IdentityToken` fuer eine Local-Identity (Spec
    /// §9.3.2.4). Wird im SPDP-Announce als `PID_IDENTITY_TOKEN` (0x1001)
    /// publiziert. Default: leerer Token (= Plugin unterstuetzt das
    /// Feature nicht).
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn get_identity_token(&self, _local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    /// Liefert das `IdentityStatusToken` fuer eine Local-Identity
    /// (Spec §9.3.2.5.1.2). Default: leer.
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn get_identity_status_token(&self, _local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    /// Setzt das Permissions-Credential und das Permissions-Token an
    /// einer Local-Identity (Spec §9.3.2.4 + §9.3.2.5.4). Wird vom
    /// Caller-Layer mit dem Output des AccessControlPlugin gespeist.
    ///
    /// # Errors
    /// Default: `Unsupported` (Plugin ignoriert Permissions-Bind).
    fn set_permissions_credential_and_token(
        &mut self,
        _local: IdentityHandle,
        _permissions_credential: &[u8],
        _permissions_token: &[u8],
    ) -> SecurityResult<()> {
        Ok(())
    }

    /// Liefert das `AuthenticatedPeerCredentialToken` (Spec §9.3.2.5.6).
    /// Wird nach erfolgreichem Handshake vom AccessControl-Layer
    /// abgeholt, um den Caller-Subject-Match durchzufuehren.
    /// Default: leer.
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn get_authenticated_peer_credential_token(
        &self,
        _handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }
}

/// Factory-Alias — vermeidet `Box<dyn ...>`-Boilerplate an Call-Sites.
pub type AuthPluginBox = Box<dyn AuthenticationPlugin>;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // Compile-Check: Stub-Impl demonstriert dass der Trait object-safe
    // + Send+Sync erfuellbar ist.
    struct StubAuth;
    impl AuthenticationPlugin for StubAuth {
        fn validate_local_identity(
            &mut self,
            _props: &PropertyList,
            _guid: [u8; 16],
        ) -> SecurityResult<IdentityHandle> {
            Ok(IdentityHandle(1))
        }
        fn validate_remote_identity(
            &mut self,
            _l: IdentityHandle,
            _r: [u8; 16],
            _t: &[u8],
        ) -> SecurityResult<IdentityHandle> {
            Ok(IdentityHandle(2))
        }
        fn begin_handshake_request(
            &mut self,
            _i: IdentityHandle,
            _r: IdentityHandle,
        ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
            Ok((HandshakeHandle(1), HandshakeStepOutcome::WaitingForPeer))
        }
        fn begin_handshake_reply(
            &mut self,
            _r: IdentityHandle,
            _i: IdentityHandle,
            _t: &[u8],
        ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
            Ok((HandshakeHandle(1), HandshakeStepOutcome::WaitingForPeer))
        }
        fn process_handshake(
            &mut self,
            _h: HandshakeHandle,
            _t: &[u8],
        ) -> SecurityResult<HandshakeStepOutcome> {
            Ok(HandshakeStepOutcome::Complete {
                secret: SharedSecretHandle(1),
            })
        }
        fn shared_secret(&self, _h: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
            Ok(SharedSecretHandle(1))
        }
        fn plugin_class_id(&self) -> &str {
            "DDS:Auth:Stub"
        }
    }

    #[test]
    fn stub_can_be_boxed() {
        let _: AuthPluginBox = Box::new(StubAuth);
    }
}
