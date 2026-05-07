// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Builtin Pre-Shared-Key Cryptographic-Plugin (Spec §10.9).
//!
//! Spec-Class-Id `"DDS:Crypto:PSK:AES-GCM-GMAC:1.2"`. Wire-Layout
//! ist **identisch** zum X.509-Pfad ([`AesGcmCryptoPlugin`]) — Spec
//! §10.9 garantiert das ausdruecklich. Unterschied: die Master-Keys
//! werden via HKDF-SHA256 direkt aus dem Pre-Shared-Key abgeleitet
//! statt aus einem DH-Shared-Secret.
//!
//! Im SRTPS_PREFIX-Submessage-Header setzt der PSK-Pfad den
//! `PreSharedKeyFlag` (Spec §10.9.1) — siehe
//! `zerodds_security_rtps::PRE_SHARED_KEY_FLAG`.
//!
//! # Architektur
//!
//! Composition statt Inheritance: `PskCryptoPlugin` haelt einen
//! [`AesGcmCryptoPlugin`] und delegiert AEAD-Hot-Path-Calls (encrypt,
//! decrypt, multi-MAC, …) 1:1 an ihn. Die einzige Erweiterung ist die
//! `register_psk_local`/`register_psk_remote`-Konfigurations-API: ein Pre-Shared-Key wird
//! pro `(local, remote)`-Paar via HKDF expandiert, und der Plugin
//! schreibt das daraus entstehende KeyMaterial direkt in den
//! AesGcm-Slot — ohne RNG-Random-Phase.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use ring::hkdf;
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin, ReceiverMac};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use crate::plugin::AesGcmCryptoPlugin;
use crate::suite::Suite;

/// Plugin-Class-Id (Spec §10.9).
pub const CLASS_ID_PSK_CRYPTO: &str = "DDS:Crypto:PSK:AES-GCM-GMAC:1.2";

/// HKDF-Info-String fuer Master-Key-Derivation aus dem Pre-Shared-
/// Key. Spec-konformer Domain-Separator (§10.9.2).
pub const HKDF_INFO_PSK_MASTER_KEY: &[u8] = b"DDS-Security-1.2-PSK-MasterKey";

/// PSK-Crypto-Plugin. Class-Id `"DDS:Crypto:PSK:AES-GCM-GMAC:1.2"`,
/// Wire-Layout = AES-GCM-Plugin.
pub struct PskCryptoPlugin {
    inner: AesGcmCryptoPlugin,
    suite: Suite,
    /// Pre-Shared-Keys pro Identity-Handle-Pair (lokale Konfiguration).
    /// Im PSK-Modus wird der Master-Key deterministisch aus
    /// (PSK || session_salt) abgeleitet — beide Seiten landen ohne
    /// Token-Exchange beim gleichen Material.
    psks: BTreeMap<u64, Vec<u8>>,
}

impl PskCryptoPlugin {
    /// Konstruktor mit Default-Suite `AES-GCM-128`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_suite(Suite::Aes128Gcm)
    }

    /// Konstruktor mit expliziter Suite.
    #[must_use]
    pub fn with_suite(suite: Suite) -> Self {
        Self {
            inner: AesGcmCryptoPlugin::with_suite(suite),
            suite,
            psks: BTreeMap::new(),
        }
    }

    /// Aktive Suite (fuer Tests / Metrics).
    #[must_use]
    pub fn suite(&self) -> Suite {
        self.suite
    }

    /// Registriert einen Pre-Shared-Key. Der Caller adressiert ihn
    /// spaeter via `register_psk_remote` ueber den gleichen
    /// `psk_id`-Namespace. Im PSK-Pfad gibt es keinen Random-Phase,
    /// d.h. Encrypt-Tokens werden via `register_psk_remote` direkt
    /// ohne RNG generiert.
    ///
    /// # Errors
    /// `BadArgument` wenn der Key leer ist.
    pub fn register_psk(&mut self, psk_id: u64, key: Vec<u8>) -> SecurityResult<()> {
        if key.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-crypto: pre-shared-key leer",
            ));
        }
        self.psks.insert(psk_id, key);
        Ok(())
    }

    /// Registriert einen Remote-Slot fuer einen bekannten PSK. Der
    /// Plugin leitet via HKDF einen Per-Peer-Master-Key ab und
    /// schreibt ihn als Wire-Token in den AES-GCM-Slot. Beide Seiten
    /// muessen denselben PSK + dieselbe `session_id` verwenden, damit
    /// die Decrypt-Seite passt.
    ///
    /// # Errors
    /// `BadArgument` wenn `psk_id` unbekannt; sonstige Crypto-Fehler.
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
                    "psk-crypto: psk_id nicht registriert",
                )
            })?
            .clone();
        let master_key = derive_psk_master_key(self.suite, &psk, &session_id)?;
        let master_salt = derive_psk_master_salt(&psk, &session_id)?;
        let key_id = derive_psk_key_id(&psk, &session_id)?;

        // Build serialized token (Spec §10.5.2 Tab.73, C3.7-b):
        // [kind_id(1) | session_id(4) | sender_key_id(4) |
        //  master_salt(32) | master_key(N)]
        let mut token = Vec::with_capacity(1 + 4 + 4 + 32 + master_key.len());
        token.push(self.suite.transform_kind_id());
        token.extend_from_slice(&session_id);
        token.extend_from_slice(&key_id);
        token.extend_from_slice(&master_salt);
        token.extend_from_slice(&master_key);

        // Erst einen Slot via inner-Plugin allocieren — der Random-
        // Inhalt wird gleich danach durch unseren PSK-abgeleiteten
        // Token ueberschrieben.
        let slot = self.inner.register_matched_remote_participant(
            local,
            remote_identity,
            SharedSecretHandle(0),
        )?;
        self.inner
            .set_remote_participant_crypto_tokens(local, slot, &token)?;
        Ok(slot)
    }

    /// Registriert auch den **lokalen** Slot deterministisch aus
    /// PSK + session_id — ueblicherweise aufgerufen statt
    /// `register_local_participant` wenn man pure-PSK-Sym-Keys haben
    /// will (beide Seiten rechnen den Key offline aus).
    ///
    /// # Errors
    /// Wie [`Self::register_psk_remote`].
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
                    "psk-crypto: psk_id nicht registriert",
                )
            })?
            .clone();
        let master_key = derive_psk_master_key(self.suite, &psk, &session_id)?;
        let master_salt = derive_psk_master_salt(&psk, &session_id)?;
        let key_id = derive_psk_key_id(&psk, &session_id)?;
        let mut token = Vec::with_capacity(1 + 4 + 4 + 32 + master_key.len());
        token.push(self.suite.transform_kind_id());
        token.extend_from_slice(&session_id);
        token.extend_from_slice(&key_id);
        token.extend_from_slice(&master_salt);
        token.extend_from_slice(&master_key);

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

/// Spec §10.9.2 — Master-Key-Ableitung aus PSK + Session-Salt.
/// `master_sender_key = HKDF-SHA256(psk, salt=session_id, info=
/// "DDS-Security-1.2-PSK-MasterKey")`.
fn derive_psk_master_key(
    suite: Suite,
    psk: &[u8],
    session_id: &[u8; 4],
) -> SecurityResult<Vec<u8>> {
    derive_psk_field(psk, session_id, HKDF_INFO_PSK_MASTER_KEY, suite.key_len())
}

/// Spec §10.9.2: master_salt + sender_key_id deterministisch aus
/// (PSK, session_id) — beide Seiten rechnen offline. Verwendet eigene
/// HKDF-Info-Strings, damit keine Kollision mit master_key.
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
        // remote-slots = die gleichen Keys (PSK ist symmetrisch).
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
