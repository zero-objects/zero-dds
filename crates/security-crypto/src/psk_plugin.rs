// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Built-in pre-shared-key cryptographic plugin (spec §10.9).
//!
//! Spec class ID `"DDS:Crypto:PSK:AES-GCM-GMAC:1.2"`. The wire layout
//! is **identical** to the X.509 path ([`AesGcmCryptoPlugin`]) — spec
//! §10.9 guarantees that explicitly. Difference: the master keys
//! are derived directly from the pre-shared key via HKDF-SHA256
//! instead of from a DH shared secret.
//!
//! In the SRTPS_PREFIX submessage header the PSK path sets the
//! `PreSharedKeyFlag` (spec §10.9.1) — see
//! `zerodds_security_rtps::PRE_SHARED_KEY_FLAG`.
//!
//! # Architecture
//!
//! Composition instead of inheritance: `PskCryptoPlugin` holds an
//! [`AesGcmCryptoPlugin`] and delegates AEAD hot-path calls (encrypt,
//! decrypt, multi-MAC, …) 1:1 to it. The only extension is the
//! `register_psk_local`/`register_psk_remote` configuration API: a pre-shared key is
//! expanded per `(local, remote)` pair via HKDF, and the plugin
//! writes the resulting KeyMaterial directly into the
//! AesGcm slot — without an RNG random phase.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::backend::hkdf;
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin, ReceiverMac};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use crate::plugin::AesGcmCryptoPlugin;
use crate::suite::Suite;

/// Plugin-Class-Id (Spec §10.9).
pub const CLASS_ID_PSK_CRYPTO: &str = "DDS:Crypto:PSK:AES-GCM-GMAC:1.2";

/// HKDF info string for the master-key derivation from the pre-shared
/// key. Spec-conformant domain separator (§10.9.2).
pub const HKDF_INFO_PSK_MASTER_KEY: &[u8] = b"DDS-Security-1.2-PSK-MasterKey";

/// PSK crypto plugin. Class ID `"DDS:Crypto:PSK:AES-GCM-GMAC:1.2"`,
/// wire layout = AES-GCM plugin.
pub struct PskCryptoPlugin {
    inner: AesGcmCryptoPlugin,
    suite: Suite,
    /// Pre-shared keys per identity-handle pair (local configuration).
    /// In PSK mode the master key is derived deterministically from
    /// (PSK || session_salt) — both sides land on the same material
    /// without a token exchange.
    psks: BTreeMap<u64, Vec<u8>>,
}

impl PskCryptoPlugin {
    /// Constructor with the default suite `AES-GCM-128`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_suite(Suite::Aes128Gcm)
    }

    /// Constructor with an explicit suite.
    #[must_use]
    pub fn with_suite(suite: Suite) -> Self {
        Self {
            inner: AesGcmCryptoPlugin::with_suite(suite),
            suite,
            psks: BTreeMap::new(),
        }
    }

    /// Active suite (for tests / metrics).
    #[must_use]
    pub fn suite(&self) -> Suite {
        self.suite
    }

    /// Registers a pre-shared key. The caller addresses it
    /// later via `register_psk_remote` over the same
    /// `psk_id` namespace. In the PSK path there is no random phase,
    /// i.e. encrypt tokens are generated via `register_psk_remote` directly
    /// without an RNG.
    ///
    /// # Errors
    /// `BadArgument` if the key is empty.
    pub fn register_psk(&mut self, psk_id: u64, key: Vec<u8>) -> SecurityResult<()> {
        if key.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-crypto: pre-shared-key empty",
            ));
        }
        self.psks.insert(psk_id, key);
        Ok(())
    }

    /// Registers a remote slot for a known PSK. The
    /// plugin derives a per-peer master key via HKDF and
    /// writes it as a wire token into the AES-GCM slot. Both sides
    /// must use the same PSK + the same `session_id`, so that
    /// the decrypt side matches.
    ///
    /// # Errors
    /// `BadArgument` if `psk_id` is unknown; other crypto errors.
    pub fn register_psk_remote(
        &mut self,
        local: CryptoHandle,
        remote_identity: IdentityHandle,
        psk_id: u64,
        session_id: [u8; 4],
    ) -> SecurityResult<CryptoHandle> {
        let psk = self
            .psks
            .get(&psk_id)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk-crypto: psk_id not registered",
                )
            })?
            .clone();
        let master_key = derive_psk_master_key(self.suite, &psk, &session_id)?;
        let master_salt = derive_psk_master_salt(&psk, &session_id)?;
        let key_id = derive_psk_key_id(&psk, &session_id)?;

        // Build serialized token (Spec §10.5.2 Tab.73, C3.7-b):
        // [kind_id(1) | session_id(4) | sender_key_id(4) |
        //  master_salt(32) | master_key(N)]
        // CryptoToken keymat in cyclone/spec CDR-BE format (§9.5.2.1.1):
        // transform_kind[4] || master_salt seq || sender_key_id[4] ||
        // master_sender_key seq || receiver_specific_key_id[4] || rcv seq(0).
        // (session_id does not travel in the keymat — the decode path reads it from the
        // wire nonce; master_key/salt already depend on session_id.)
        let mut token =
            Vec::with_capacity(4 + 4 + master_salt.len() + 4 + 4 + master_key.len() + 8);
        token.extend_from_slice(&self.suite.transform_kind());
        token.extend_from_slice(&(master_salt.len() as u32).to_be_bytes());
        token.extend_from_slice(&master_salt);
        token.extend_from_slice(&key_id);
        token.extend_from_slice(&(master_key.len() as u32).to_be_bytes());
        token.extend_from_slice(&master_key);
        token.extend_from_slice(&[0u8; 4]);
        token.extend_from_slice(&0u32.to_be_bytes());

        // First allocate a slot via the inner plugin — the random
        // content is overwritten right after by our PSK-derived
        // token.
        let slot = self.inner.register_matched_remote_participant(
            local,
            remote_identity,
            SharedSecretHandle(0),
        )?;
        self.inner
            .set_remote_participant_crypto_tokens(local, slot, &token)?;
        Ok(slot)
    }

    /// Registers the **local** slot too, deterministically from
    /// PSK + session_id — usually called instead of
    /// `register_local_participant` when you want pure PSK symmetric keys
    /// (both sides compute the key offline).
    ///
    /// # Errors
    /// Like [`Self::register_psk_remote`].
    pub fn register_psk_local(
        &mut self,
        psk_id: u64,
        session_id: [u8; 4],
    ) -> SecurityResult<CryptoHandle> {
        let psk = self
            .psks
            .get(&psk_id)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk-crypto: psk_id not registered",
                )
            })?
            .clone();
        let master_key = derive_psk_master_key(self.suite, &psk, &session_id)?;
        let master_salt = derive_psk_master_salt(&psk, &session_id)?;
        let key_id = derive_psk_key_id(&psk, &session_id)?;
        // CryptoToken keymat in cyclone/spec CDR-BE format (§9.5.2.1.1):
        // transform_kind[4] || master_salt seq || sender_key_id[4] ||
        // master_sender_key seq || receiver_specific_key_id[4] || rcv seq(0).
        // (session_id does not travel in the keymat — the decode path reads it from the
        // wire nonce; master_key/salt already depend on session_id.)
        let mut token =
            Vec::with_capacity(4 + 4 + master_salt.len() + 4 + 4 + master_key.len() + 8);
        token.extend_from_slice(&self.suite.transform_kind());
        token.extend_from_slice(&(master_salt.len() as u32).to_be_bytes());
        token.extend_from_slice(&master_salt);
        token.extend_from_slice(&key_id);
        token.extend_from_slice(&(master_key.len() as u32).to_be_bytes());
        token.extend_from_slice(&master_key);
        token.extend_from_slice(&[0u8; 4]);
        token.extend_from_slice(&0u32.to_be_bytes());

        let slot = self
            .inner
            .register_local_participant(IdentityHandle(0), &[])?;
        self.inner
            .set_remote_participant_crypto_tokens(slot, slot, &token)?;
        Ok(slot)
    }
}

impl Default for PskCryptoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Spec §10.9.2 — master-key derivation from PSK + session salt.
/// `master_sender_key = HKDF-SHA256(psk, salt=session_id, info=
/// "DDS-Security-1.2-PSK-MasterKey")`.
fn derive_psk_master_key(
    suite: Suite,
    psk: &[u8],
    session_id: &[u8; 4],
) -> SecurityResult<Vec<u8>> {
    derive_psk_field(psk, session_id, HKDF_INFO_PSK_MASTER_KEY, suite.key_len())
}

/// Spec §10.9.2: master_salt + sender_key_id deterministically from
/// (PSK, session_id) — both sides compute offline. Uses its own
/// HKDF info strings so there is no collision with master_key.
const HKDF_INFO_PSK_MASTER_SALT: &[u8] = b"DDS-Security-1.2-PSK-MasterSalt";
const HKDF_INFO_PSK_KEY_ID: &[u8] = b"DDS-Security-1.2-PSK-SenderKeyId";

fn derive_psk_master_salt(psk: &[u8], session_id: &[u8; 4]) -> SecurityResult<[u8; 32]> {
    let v = derive_psk_field(psk, session_id, HKDF_INFO_PSK_MASTER_SALT, 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Ok(out)
}

fn derive_psk_key_id(psk: &[u8], session_id: &[u8; 4]) -> SecurityResult<[u8; 4]> {
    let v = derive_psk_field(psk, session_id, HKDF_INFO_PSK_KEY_ID, 4)?;
    let mut out = [0u8; 4];
    out.copy_from_slice(&v);
    Ok(out)
}

fn derive_psk_field(
    psk: &[u8],
    session_id: &[u8; 4],
    info: &[u8],
    out_len: usize,
) -> SecurityResult<Vec<u8>> {
    if psk.is_empty() {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk-crypto: empty psk",
        ));
    }
    let salt_obj = hkdf::Salt::new(hkdf::HKDF_SHA256, session_id);
    let prk = salt_obj.extract(psk);
    let info_arr = [info];
    let okm = prk
        .expand(
            &info_arr,
            HkdfLen {
                len: out_len,
                hmac: hkdf::HKDF_SHA256,
            },
        )
        .map_err(|_| {
            SecurityError::new(SecurityErrorKind::CryptoFailed, "psk-crypto: HKDF expand")
        })?;
    let mut out = alloc::vec![0u8; out_len];
    okm.fill(&mut out).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "psk-crypto: HKDF fill")
    })?;
    Ok(out)
}

struct HkdfLen {
    len: usize,
    hmac: hkdf::Algorithm,
}

impl hkdf::KeyType for HkdfLen {
    fn len(&self) -> usize {
        self.len
    }
}

impl From<HkdfLen> for hkdf::Algorithm {
    fn from(v: HkdfLen) -> Self {
        v.hmac
    }
}

impl CryptographicPlugin for PskCryptoPlugin {
    fn register_local_participant(
        &mut self,
        identity: IdentityHandle,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        self.inner.register_local_participant(identity, properties)
    }

    fn register_matched_remote_participant(
        &mut self,
        local: CryptoHandle,
        remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle> {
        self.inner
            .register_matched_remote_participant(local, remote_identity, shared_secret)
    }

    fn register_local_endpoint(
        &mut self,
        participant: CryptoHandle,
        is_writer: bool,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        self.inner
            .register_local_endpoint(participant, is_writer, properties)
    }

    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        self.inner
            .create_local_participant_crypto_tokens(local, remote)
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        self.inner
            .set_remote_participant_crypto_tokens(local, remote, tokens)
    }

    fn encrypt_submessage(
        &self,
        local: CryptoHandle,
        remote_list: &[CryptoHandle],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.inner
            .encrypt_submessage(local, remote_list, plaintext, aad_extension)
    }

    fn decrypt_submessage(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.inner
            .decrypt_submessage(local, remote, ciphertext, aad_extension)
    }

    fn encrypt_submessage_multi(
        &self,
        local: CryptoHandle,
        receivers: &[(CryptoHandle, u32)],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<(Vec<u8>, Vec<ReceiverMac>)> {
        self.inner
            .encrypt_submessage_multi(local, receivers, plaintext, aad_extension)
    }

    #[allow(clippy::too_many_arguments)]
    fn decrypt_submessage_with_receiver_mac(
        &self,
        local: CryptoHandle,
        remote: CryptoHandle,
        own_key_id: u32,
        own_mac_key_handle: CryptoHandle,
        ciphertext: &[u8],
        macs: &[ReceiverMac],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.inner.decrypt_submessage_with_receiver_mac(
            local,
            remote,
            own_key_id,
            own_mac_key_handle,
            ciphertext,
            macs,
            aad_extension,
        )
    }

    fn plugin_class_id(&self) -> &str {
        CLASS_ID_PSK_CRYPTO
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn class_id_matches_spec() {
        let p = PskCryptoPlugin::new();
        assert_eq!(p.plugin_class_id(), "DDS:Crypto:PSK:AES-GCM-GMAC:1.2");
    }

    #[test]
    fn transform_kind_id_aes128_matches_x509_path() {
        let p = PskCryptoPlugin::with_suite(Suite::Aes128Gcm);
        assert_eq!(p.suite().transform_kind_id(), 0x02);
    }

    #[test]
    fn transform_kind_id_aes256_matches_x509_path() {
        let p = PskCryptoPlugin::with_suite(Suite::Aes256Gcm);
        assert_eq!(p.suite().transform_kind_id(), 0x04);
    }

    #[test]
    fn psk_master_key_derivation_is_deterministic() {
        let psk = alloc::vec![0xAB; 32];
        let session = [0u8, 0, 0, 1];
        let k1 = derive_psk_master_key(Suite::Aes128Gcm, &psk, &session).unwrap();
        let k2 = derive_psk_master_key(Suite::Aes128Gcm, &psk, &session).unwrap();
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 16);
    }

    #[test]
    fn psk_master_key_changes_with_session_id() {
        let psk = alloc::vec![0xAB; 32];
        let k1 = derive_psk_master_key(Suite::Aes128Gcm, &psk, &[0, 0, 0, 1]).unwrap();
        let k2 = derive_psk_master_key(Suite::Aes128Gcm, &psk, &[0, 0, 0, 2]).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn psk_master_key_rejects_empty_psk() {
        let err = derive_psk_master_key(Suite::Aes128Gcm, &[], &[0u8; 4]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_rejects_empty_key() {
        let mut p = PskCryptoPlugin::new();
        let err = p.register_psk(1, Vec::new()).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_remote_unknown_id_rejected() {
        let mut p = PskCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let err = p
            .register_psk_remote(local, IdentityHandle(2), 99, [0u8; 4])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn psk_encrypt_decrypt_roundtrip_two_plugins_same_psk() {
        let psk = alloc::vec![0x77u8; 32];
        let mut alice = PskCryptoPlugin::new();
        let mut bob = PskCryptoPlugin::new();
        alice.register_psk(7, psk.clone()).unwrap();
        bob.register_psk(7, psk).unwrap();

        let session = [0u8, 0, 0, 42];
        let alice_local = alice.register_psk_local(7, session).unwrap();
        let bob_local = bob.register_psk_local(7, session).unwrap();
        // remote slots = the same keys (PSK is symmetric).
        let alice_to_bob = alice
            .register_psk_remote(alice_local, IdentityHandle(2), 7, session)
            .unwrap();
        let bob_to_alice = bob
            .register_psk_remote(bob_local, IdentityHandle(1), 7, session)
            .unwrap();

        let plain = b"top-secret-psk-payload";
        let wire = alice
            .encrypt_submessage(alice_to_bob, &[], plain, &[])
            .unwrap();
        let back = bob
            .decrypt_submessage(bob_to_alice, bob_to_alice, &wire, &[])
            .unwrap();
        assert_eq!(back, plain);
    }
}
