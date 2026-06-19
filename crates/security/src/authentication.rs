// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Authentication plugin SPI (OMG DDS-Security 1.1 §8.3).
//!
//! Responsible for:
//! 1. **Identity validation** at participant start — validate the local
//!    identity (e.g. X.509 cert + private key) against the trust anchor.
//! 2. **Identity handshake** between two participants — challenge/
//!    response with signed nonces, spec §8.3.2.
//! 3. **SharedSecret creation** at the end of the handshake — input
//!    for the CryptographicPlugin for key derivation.
//!
//! The SPI is **state-machine-light** — the plugin holds the
//! handshake state itself. The caller (DCPS layer) only triggers
//! `begin_handshake_request/reply`, `process_handshake_reply`,
//! `process_handshake_final`.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn AuthenticationPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::error::SecurityResult;
use crate::properties::PropertyList;

/// Opaque handle for a validated identity. The plugin-internal
/// state (X.509 cert, keys) is not exposed outward through this
/// handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityHandle(pub u64);

/// Opaque handle for a running handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandshakeHandle(pub u64);

/// Opaque handle for a shared secret (output of a completed
/// handshake). Passed on to the `CryptographicPlugin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedSecretHandle(pub u64);

/// Lookup bridge between [`AuthenticationPlugin`] and
/// [`crate::crypto::CryptographicPlugin`].
///
/// After the handshake the authentication plugin produces a
/// [`SharedSecretHandle`] and knows the associated 32 bytes from
/// `x25519 + HKDF-SHA256`. The crypto plugin needs exactly these
/// bytes to derive its per-peer master key — instead of generating a
/// random key and routing it as an opaque token through the
/// governance.
///
/// Implementors must be **Send + Sync** so that the crypto
/// plugin can hold the provider via `Arc<dyn SharedSecretProvider>`.
pub trait SharedSecretProvider: Send + Sync {
    /// Returns the raw bytes of the shared key.
    /// `None` if the handle is unknown (handshake not yet
    /// completed or already discarded).
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<alloc::vec::Vec<u8>>;

    /// Returns `(challenge1, challenge2)` of the associated handshake
    /// (challenge1 = initiator, challenge2 = replier). These feed into the
    /// VolatileSecure key derivation (DDS-Security §9.5.3.5, `calculate_kx_keys`)
    /// — both peers derive the same Kx key from `(SharedSecret, challenge1,
    /// challenge2)`. Default `None` for providers without challenge tracking
    /// (e.g. PSK/mock); their Kx path then uses the fallback.
    fn get_shared_secret_challenges(
        &self,
        _handle: SharedSecretHandle,
    ) -> Option<([u8; 32], [u8; 32])> {
        None
    }
}

/// Result of a handshake step.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HandshakeStepOutcome {
    /// The plugin wants to send another message to the peer.
    SendMessage {
        /// Opaque handshake token as a byte blob. Goes 1:1 into the
        /// `AuthHandshake` built-in submessage (spec §9.3.2.3).
        token: Vec<u8>,
    },
    /// Handshake completed successfully — `SharedSecretHandle`
    /// usable.
    Complete {
        /// Shared key handle for the CryptographicPlugin.
        secret: SharedSecretHandle,
    },
    /// Handshake still needs a message from the peer — nothing to do.
    WaitingForPeer,
}

/// Authentication plugin trait. Spec §8.3.2.7.
pub trait AuthenticationPlugin: Send + Sync {
    /// Called once at participant start: validate the local identity
    /// (certificate, key, trust anchor) and return a handle.
    ///
    /// # Spec
    /// §8.3.2.7.1 `validate_local_identity`.
    fn validate_local_identity(
        &mut self,
        props: &PropertyList,
        participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle>;

    /// Called as soon as a remote participant has been discovered via
    /// SPDP. The plugin validates the remote cert (from `remote_auth_token`)
    /// against its trust store.
    ///
    /// # Spec
    /// §8.3.2.7.2 `validate_remote_identity`.
    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        remote_auth_token: &[u8],
    ) -> SecurityResult<IdentityHandle>;

    /// Cross-vendor quirk: determines whether the next handshake algorithm
    /// strings (`c.dsign_algo`/`c.kagree_algo`) are emitted + hashed
    /// NUL-terminated. OpenDDS compares them with `sizeof` (incl. `\0`) and
    /// needs the NUL form; FastDDS (#3803) needs them WITHOUT; cyclone is
    /// tolerant. Since the handshake runs per-peer, the discovery layer calls
    /// this based on the peer's `VendorId` BEFORE `begin_handshake_request` or
    /// `begin_handshake_reply`. Default no-op (NUL-free = spec/FastDDS/
    /// Cyclone-conformant).
    fn set_algo_nul_terminate(&mut self, _nul: bool) {}

    /// Starts the handshake. Returns the first token that must be
    /// sent to the peer.
    ///
    /// # Spec
    /// §8.3.2.7.3 `begin_handshake_request`.
    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        replier: IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>;

    /// Peer side of the handshake start. `request_token` is what the
    /// initiator sent via `begin_handshake_request`.
    ///
    /// # Spec
    /// §8.3.2.7.4 `begin_handshake_reply`.
    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        initiator: IdentityHandle,
        request_token: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)>;

    /// Pass through follow-up handshake messages.
    ///
    /// # Spec
    /// §8.3.2.7.5 `process_handshake`.
    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome>;

    /// Ends the handshake and returns the final SharedSecret.
    /// Failure aborts. Called by the caller after a `Complete`
    /// outcome to pull the secret out of the plugin.
    ///
    /// Alternatively: the `Complete` outcome already contains the
    /// handle — this method is only for polling integrations.
    ///
    /// # Spec
    /// §8.3.2.7.8 `get_shared_secret`.
    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle>;

    /// Identity plugin name (e.g. "DDS:Auth:PKI-DH:1.2"). Announced in SPDP as
    /// `dds.sec.auth.plugin_class`.
    fn plugin_class_id(&self) -> &str;

    /// Returns the `IdentityToken` for a local identity (spec
    /// §9.3.2.4). Published in the SPDP announce as `PID_IDENTITY_TOKEN` (0x1001).
    /// Default: empty token (= the plugin does not support the
    /// feature).
    ///
    /// # Errors
    /// Implementation-specific.
    fn get_identity_token(&self, _local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    /// Returns the `IdentityStatusToken` for a local identity
    /// (spec §9.3.2.5.1.2). Default: empty.
    ///
    /// # Errors
    /// Implementation-specific.
    fn get_identity_status_token(&self, _local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }

    /// Returns the `PermissionsToken` (spec §7.2.4, `PID_PERMISSIONS_TOKEN`
    /// 0x1002) for the SPDP announce. Strictly per spec the
    /// AccessControlPlugin produces it; since ZeroDDS holds the permissions in
    /// the auth plugin (`set_local_permissions`, for the `c.perm` handshake),
    /// the getter lives here. Default: empty (no permissions configured ⇒
    /// AccessControl inactive ⇒ token omitted). Cross-vendor requirement:
    /// secure vendors (cyclone/FastDDS) only validate a remote
    /// if SPDP carries **both** tokens (identity + permissions).
    fn get_permissions_token(&self) -> Vec<u8> {
        Vec::new()
    }

    /// Sets the local `ParticipantBuiltinTopicData` as PL_CDR bytes that
    /// are sent along in the handshake as `c.pdata` (spec §9.3.2.5.2). The
    /// replier deserializes c.pdata as a ParameterList and binds the
    /// participant_guid to the authenticated identity. Default: no-op.
    fn set_local_participant_data(&mut self, _pdata: alloc::vec::Vec<u8>) {}

    /// Sets the permissions credential and the permissions token on
    /// a local identity (spec §9.3.2.4 + §9.3.2.5.4). Fed by the
    /// caller layer with the output of the AccessControlPlugin.
    ///
    /// # Errors
    /// Default: `Unsupported` (the plugin ignores the permissions bind).
    fn set_permissions_credential_and_token(
        &mut self,
        _local: IdentityHandle,
        _permissions_credential: &[u8],
        _permissions_token: &[u8],
    ) -> SecurityResult<()> {
        Ok(())
    }

    /// Returns the `AuthenticatedPeerCredentialToken` (spec §9.3.2.5.6).
    /// Fetched by the AccessControl layer after a successful handshake
    /// to perform the caller subject match.
    /// Default: empty.
    ///
    /// # Errors
    /// Implementation-specific.
    fn get_authenticated_peer_credential_token(
        &self,
        _handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        Ok(Vec::new())
    }
}

/// Factory alias — avoids `Box<dyn ...>` boilerplate at call sites.
pub type AuthPluginBox = Box<dyn AuthenticationPlugin>;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // Compile check: the stub impl demonstrates that the trait is object-safe
    // + Send+Sync satisfiable.
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
