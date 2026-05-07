// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AES-GCM-128 / AES-GCM-256 CryptographicPlugin-Implementation.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`Arc<dyn SharedSecretProvider>` wird gehalten, damit der
//! Plugin mit beliebigen Authentication-Plugins arbeitet. Der
//! Provider ist reines Lookup-Interface ohne Safety-Implikation.)

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle, SharedSecretProvider};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin, ReceiverMac};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use ring::aead::{LessSafeKey, Nonce, UnboundKey};
use ring::hkdf;
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};

use crate::suite::Suite;

/// Session-ID: 4-byte-Präfix einer 12-byte GCM-Nonce. Pro
/// `CryptoHandle` zufaellig gezogen; schuetzt vor Nonce-Collision
/// zwischen verschiedenen Keys.
type SessionId = [u8; 4];

/// Ein Crypto-Slot — DDS-Security 1.2 §10.5.2 KeyMaterial-AES-GCM-GMAC
/// Tab.73 Layout (C3.7-b):
///
/// ```text
/// transformation_kind     (1 byte, Suite::transform_kind_id)
/// master_salt             (32 byte, fuer Spec-konforme session_key-
///                          Derivation; Spec §10.5.2 Tab.74)
/// sender_key_id           (4 byte, transformation_key_id)
/// master_sender_key       (16 oder 32 byte, suite.key_len())
/// session_id              (4 byte, rotiert pro Session)
/// ```
///
/// Encrypt/Decrypt nutzen `derive_session_key(master_key, master_salt,
/// session_id)` als Per-Submessage-AES-Key + `compute_aad(transform_kind,
/// key_id, session_id, &[])` als AES-GCM AAD. Damit ist der Hot-Path
/// spec-byte-kompatibel zu Cyclone DDS / FastDDS.
struct KeyMaterial {
    /// AEAD-Suite (128 vs. 256).
    suite: Suite,
    /// `transformation_key_id` (Spec §10.5.2 Tab.73, 4 byte). Eindeutig
    /// pro Slot.
    transformation_key_id: [u8; 4],
    /// Master-Sender-Key. Laenge gemaess `suite.key_len()` (16 oder 32).
    master_key: Vec<u8>,
    /// `master_salt` (Spec §10.5.2 Tab.73 + Tab.74) — 32 byte; fliesst
    /// in `derive_session_key` ein.
    master_salt: [u8; 32],
    /// Session-ID — konstant pro Slot, rotiert beim Key-Refresh.
    session_id: SessionId,
    /// Nonce-Counter — monoton pro Encrypt. Bei Ueberlauf (u64::MAX)
    /// wird Encrypt abgelehnt .
    counter: AtomicU64,
}

impl KeyMaterial {
    fn new_random(suite: Suite, rng: &SystemRandom) -> SecurityResult<Self> {
        let mut mk = alloc::vec![0u8; suite.key_len()];
        rng.fill(&mut mk).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill master_key failed",
            )
        })?;
        let mut salt = [0u8; 32];
        rng.fill(&mut salt).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill master_salt failed",
            )
        })?;
        let mut sid = [0u8; 4];
        rng.fill(&mut sid).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill session_id failed",
            )
        })?;
        let mut key_id = [0u8; 4];
        rng.fill(&mut key_id).map_err(|_| {
            SecurityError::new(SecurityErrorKind::CryptoFailed, "rng fill key_id failed")
        })?;
        Ok(Self {
            suite,
            transformation_key_id: key_id,
            master_key: mk,
            master_salt: salt,
            session_id: sid,
            counter: AtomicU64::new(0),
        })
    }

    /// Spec-konformes Token-Layout (DDS-Security 1.2 §10.5.2 Tab.73,
    /// C3.7-b):
    ///
    /// ```text
    /// [transformation_kind(1) | session_id(4) | sender_key_id(4) |
    ///  master_salt(32) | master_sender_key(N)]
    /// ```
    ///
    /// Total: `1 + 4 + 4 + 32 + suite.key_len()` byte.
    fn from_serialized(serialized: &[u8]) -> SecurityResult<Self> {
        if serialized.len() < 1 + 4 + 4 + 32 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: token zu kurz (mindestens 41 byte vor master_key)",
            ));
        }
        // Spec-konform: AES128_GCM=0x02, AES256_GCM=0x04, HmacSha256
        // im AES128_GMAC-Slot=0x01 (siehe Suite::transform_kind_id).
        let suite = Suite::from_transform_kind_id(serialized[0]).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("crypto: unbekannte suite-id 0x{:02x}", serialized[0]),
            )
        })?;
        let expected = 1 + 4 + 4 + 32 + suite.key_len();
        if serialized.len() != expected {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("crypto: token fuer {suite:?} muss {expected} byte sein"),
            ));
        }
        let mut sid = [0u8; 4];
        sid.copy_from_slice(&serialized[1..5]);
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&serialized[5..9]);
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&serialized[9..41]);
        let mk = serialized[41..].to_vec();
        Ok(Self {
            suite,
            transformation_key_id: key_id,
            master_key: mk,
            master_salt: salt,
            session_id: sid,
            counter: AtomicU64::new(0),
        })
    }

    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + 4 + 4 + 32 + self.master_key.len());
        out.push(self.suite.transform_kind_id());
        out.extend_from_slice(&self.session_id);
        out.extend_from_slice(&self.transformation_key_id);
        out.extend_from_slice(&self.master_salt);
        out.extend_from_slice(&self.master_key);
        out
    }

    /// Liefert das Spec-konforme `transformation_kind` als 4-byte BE-
    /// Array (Spec §10.5 Tab.79). Wird in der AAD verwendet.
    fn transformation_kind_bytes(&self) -> [u8; 4] {
        self.suite.transform_kind()
    }

    /// Spec §10.5.2 Tab.74 + §8.1 Tab.78 — leitet den Per-Submessage
    /// `session_key` und die AAD ab. Verwendet die C3.7-Helper aus
    /// `zerodds_security_crypto::session_key`.
    fn derive_session_key_bytes(&self) -> Vec<u8> {
        let full = crate::session_key::derive_session_key(
            &self.master_key,
            &self.master_salt,
            &self.session_id,
        );
        // Truncate auf Suite-Key-Laenge (16 byte AES-128, 32 byte AES-256
        // oder HMAC-SHA256-Auth).
        full[..self.suite.key_len()].to_vec()
    }

    /// Baut die AAD fuer diese Submessage. `extension` ist der RTPS-
    /// Header beim `rtps_protection_kind != NONE` — fuer Submessage-
    /// Protection ist `extension` leer.
    fn aad(&self, extension: &[u8]) -> Vec<u8> {
        crate::session_key::compute_aad(
            self.transformation_kind_bytes(),
            self.transformation_key_id,
            self.session_id,
            extension,
        )
    }

    /// Leitet Master-Key + Session-ID deterministisch aus einem
    /// `SharedSecret` (32 Byte aus X25519-DH-Handshake) via HKDF-SHA256
    /// ab. beide Kommunikations-Partner berechnen denselben
    /// Master-Key, ohne Token-Exchange.
    ///
    /// Die `session_id` wird ebenfalls deterministisch aus dem Secret
    /// abgeleitet (als zweite HKDF-Info), damit beide Seiten
    /// kompatible Nonces produzieren. Falls dasselbe Secret fuer
    /// mehrere Slots genutzt wird, muss der Caller die Slots manuell
    /// separat rotieren.
    fn from_shared_secret(suite: Suite, shared_secret: &[u8]) -> SecurityResult<Self> {
        if shared_secret.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: empty shared_secret",
            ));
        }
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"zerodds.crypto.shared-secret.v1");
        let prk = salt.extract(shared_secret);

        let expand = |info: &[u8], out_len: usize| -> SecurityResult<Vec<u8>> {
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
                    SecurityError::new(SecurityErrorKind::CryptoFailed, "hkdf expand failed")
                })?;
            let mut buf = alloc::vec![0u8; out_len];
            okm.fill(&mut buf).map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "hkdf fill failed")
            })?;
            Ok(buf)
        };

        // Master-Key (Spec §10.5.2 Tab.73).
        let master_key = expand(b"dds.sec.crypto.master_key", suite.key_len())?;

        // Master-Salt (Spec §10.5.2 Tab.73 + Tab.74) — 32 byte fuer
        // HMAC-SHA256-basierte session_key-Derivation.
        let master_salt_vec = expand(b"dds.sec.crypto.master_salt", 32)?;
        let mut master_salt = [0u8; 32];
        master_salt.copy_from_slice(&master_salt_vec);

        // Sender-Key-Id (4 byte) — eindeutig pro Slot, fliesst in AAD.
        let key_id_vec = expand(b"dds.sec.crypto.sender_key_id", 4)?;
        let mut transformation_key_id = [0u8; 4];
        transformation_key_id.copy_from_slice(&key_id_vec);

        // Session-Id (4 byte) — Nonce-Praefix + AAD-Bestandteil.
        let sid_vec = expand(b"dds.sec.crypto.session_id", 4)?;
        let mut session_id = [0u8; 4];
        session_id.copy_from_slice(&sid_vec);

        Ok(Self {
            suite,
            transformation_key_id,
            master_key,
            master_salt,
            session_id,
            counter: AtomicU64::new(0),
        })
    }

    fn next_nonce(&self) -> SecurityResult<[u8; 12]> {
        let c = self.counter.fetch_add(1, Ordering::Relaxed);
        if c == u64::MAX {
            return Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: nonce-counter exhausted — key-refresh required ",
            ));
        }
        let mut n = [0u8; 12];
        n[..4].copy_from_slice(&self.session_id);
        n[4..].copy_from_slice(&c.to_be_bytes());
        Ok(n)
    }
}

/// Manuelle Laenge-Struktur fuer `ring::hkdf::Prk::expand`. Das
/// `KeyType`-Trait der ring-Crate akzeptiert nur Types die
/// `len()` + `HKDF-Algo` liefern.
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

/// AES-GCM Crypto-Plugin. Keys werden in einem internen Slab
/// gehalten; Lookup per `CryptoHandle`. Welche Suite lokal erzeugte
/// Keys haben, bestimmt `local_suite` — Remote-Keys kommen mit ihrer
/// eigenen Suite-ID via Token.
pub struct AesGcmCryptoPlugin {
    rng: SystemRandom,
    next_handle: AtomicU64,
    /// Suite fuer lokal erzeugte Keys.
    local_suite: Suite,
    // RwLock fuer `slots` — read-heavy (jedes Encrypt/Decrypt liest);
    // Register passiert selten beim Setup.
    slots: RwLock<BTreeMap<CryptoHandle, KeyMaterial>>,
    /// Optionaler SharedSecretProvider . Wenn gesetzt,
    /// zieht `register_matched_remote_participant` den Master-Key
    /// deterministisch aus dem DH-Shared-Secret statt einen Random-
    /// Key zu generieren. Backward-Compat: `None` = v1.4-Pfad mit
    /// Random-Key und Token-Exchange.
    secret_provider: Option<Arc<dyn SharedSecretProvider>>,
    // Verknuepft ein lokales/Remote-Participant-Paar mit dem
    // **fuer diesen Link** benutzten Remote-Slot — nach
    // `set_remote_participant_crypto_tokens`.
    // (local, remote_identity) → remote_slot_handle
    #[allow(clippy::type_complexity)]
    remote_map: Mutex<BTreeMap<(CryptoHandle, IdentityHandle), CryptoHandle>>,
}

impl Default for AesGcmCryptoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AesGcmCryptoPlugin {
    /// Konstruktor mit Default-Suite `AES-GCM-128`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_suite(Suite::Aes128Gcm)
    }

    /// Konstruktor mit expliziter Suite (`Aes128Gcm` oder `Aes256Gcm`).
    #[must_use]
    pub fn with_suite(suite: Suite) -> Self {
        Self {
            rng: SystemRandom::new(),
            next_handle: AtomicU64::new(0),
            local_suite: suite,
            slots: RwLock::new(BTreeMap::new()),
            remote_map: Mutex::new(BTreeMap::new()),
            secret_provider: None,
        }
    }

    /// Baut einen Plugin-Slot mit echtem DH-abgeleiteten Per-Peer-Key
    /// . Der `SharedSecretProvider` liefert die rohen Bytes
    /// aus einem abgeschlossenen Authentication-Handshake; wir leiten
    /// via HKDF-SHA256 einen deterministischen Master-Key ab, den beide
    /// Seiten ohne Token-Exchange berechnen koennen.
    ///
    /// Mit Provider: `register_matched_remote_participant` nutzt den
    /// uebergebenen `SharedSecretHandle`, um den Per-Peer-Key aus dem
    /// PKI-Handshake zu ziehen. Ohne Provider bleibt das v1.4-Verhalten
    /// (Random-Key + Token-Exchange).
    #[must_use]
    pub fn with_secret_provider(suite: Suite, provider: Arc<dyn SharedSecretProvider>) -> Self {
        Self {
            rng: SystemRandom::new(),
            next_handle: AtomicU64::new(0),
            local_suite: suite,
            slots: RwLock::new(BTreeMap::new()),
            remote_map: Mutex::new(BTreeMap::new()),
            secret_provider: Some(provider),
        }
    }

    /// Lokale Suite (fuer Tests / Metrics).
    #[must_use]
    pub fn local_suite(&self) -> Suite {
        self.local_suite
    }

    /// Zaehl der verbleibenden sicheren Encrypts auf einem Slot.
    /// Wrapping-Point: `Suite::max_encrypts()` (2^48 per Default).
    /// Caller sollte bei `0` einen Key-Refresh anstossen .
    ///
    /// # Errors
    /// `BadArgument` wenn der Handle nicht existiert.
    pub fn encrypts_remaining(&self, handle: CryptoHandle) -> SecurityResult<u64> {
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "crypto: unknown handle")
        })?;
        let used = mat.counter.load(Ordering::Relaxed);
        let max = mat.suite.max_encrypts();
        Ok(max.saturating_sub(used))
    }

    /// Erzwingt einen neuen Master-Key im Slot. Nach `rotate_key` ist
    /// der alte Key futsch — Gegenseite muss per
    /// [`CryptographicPlugin::create_local_participant_crypto_tokens`]
    /// einen neuen Token ziehen und via
    /// [`CryptographicPlugin::set_remote_participant_crypto_tokens`]
    /// synchronisieren.
    ///
    /// # Errors
    /// `BadArgument` wenn der Handle nicht existiert; `CryptoFailed`
    /// wenn die RNG versagt.
    pub fn rotate_key(&mut self, handle: CryptoHandle) -> SecurityResult<()> {
        let fresh = KeyMaterial::new_random(self.local_suite, &self.rng)?;
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        let slot = slots.get_mut(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: rotate_key unknown handle",
            )
        })?;
        slot.master_key = fresh.master_key;
        slot.session_id = fresh.session_id;
        slot.counter.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn insert(&self, mat: KeyMaterial) -> SecurityResult<CryptoHandle> {
        let handle = CryptoHandle(self.next_id());
        self.slots
            .write()
            .map_err(|_| poisoned())?
            .insert(handle, mat);
        Ok(handle)
    }
}

fn poisoned() -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::Internal,
        "crypto: internal rwlock poisoned",
    )
}

impl CryptographicPlugin for AesGcmCryptoPlugin {
    fn register_local_participant(
        &mut self,
        _identity: IdentityHandle,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        let mat = KeyMaterial::new_random(self.local_suite, &self.rng)?;
        self.insert(mat)
    }

    fn register_matched_remote_participant(
        &mut self,
        _local: CryptoHandle,
        _remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle> {
        // wenn ein SharedSecretProvider konfiguriert ist,
        // ziehen wir den Per-Peer-Key deterministisch aus dem DH-
        // abgeleiteten SharedSecret. Das spart den Token-Exchange
        // (beide Seiten rechnen den gleichen Master-Key lokal aus)
        // und macht jeden Per-Peer-Link kryptographisch eigenstaendig.
        // Wenn Provider konfiguriert UND er das Secret kennt → HKDF-
        // Ableitung. Liefert der Provider `None` (Handle unbekannt),
        // fallen wir auf den v1.4-Random-Pfad zurueck — so koennen
        // Mixed-Setups (DH-MAC-Keys parallel zu Token-Exchange-Cipher-
        // Keys) im gleichen Plugin-Objekt existieren.
        if let Some(provider) = &self.secret_provider {
            if let Some(secret) = provider.get_shared_secret(shared_secret) {
                let mat = KeyMaterial::from_shared_secret(self.local_suite, &secret)?;
                return self.insert(mat);
            }
        }
        // v1.4-Pfad: Placeholder-Slot; wird mit
        // `set_remote_participant_crypto_tokens` auf den echten
        // Remote-Key gesetzt.
        let mat = KeyMaterial::new_random(self.local_suite, &self.rng)?;
        self.insert(mat)
    }

    fn register_local_endpoint(
        &mut self,
        _participant: CryptoHandle,
        _is_writer: bool,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        let mat = KeyMaterial::new_random(self.local_suite, &self.rng)?;
        self.insert(mat)
    }

    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        _remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        // Serialisiert den lokalen Master-Key. Der Token wird ueber die
        // built-in DCPSParticipantVolatileMessageSecure-Topic ausgetauscht,
        // die selber bereits mit Session-Keys aus dem SharedSecret-
        // Handshake verschluesselt ist (Spec §9.5.3.5) — eine zusaetzliche
        // Wrap-Schicht ueber dem Token waere also Doppel-Encrypt.
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown local handle",
            )
        })?;
        Ok(mat.serialize())
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        let mat = KeyMaterial::from_serialized(tokens)?;
        self.slots
            .write()
            .map_err(|_| poisoned())?
            .insert(remote, mat);
        Ok(())
    }

    fn encrypt_submessage(
        &self,
        local: CryptoHandle,
        _remote_list: &[CryptoHandle],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        #[cfg(feature = "metrics")]
        let _op = crate::metrics::CryptoOp::start("encrypt");
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown local handle",
            )
        })?;
        let nonce = mat.next_nonce()?;
        // Spec §10.5.2 Tab.74: Per-Submessage session_key aus master_key
        // + master_salt + session_id; Spec §8.1 Tab.78: AAD aus
        // transformation_kind + key_id + session_id + extension. Caller
        // liefert die Spec-konforme Extension (RTPS-Header bei §8.5.1.9.7,
        // Submessage-Header bei §8.5.1.9.2, leer bei PSK).
        let session_key = mat.derive_session_key_bytes();
        let aad_bytes = mat.aad(aad_extension);

        if !mat.suite.is_aead() {
            // HMAC-Auth-only-Pfad (ZeroDDS-Erweiterung). HMAC-Input ist
            // `aad || nonce || plaintext` — domain-separated.
            let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, &session_key);
            let mut ctx = hmac::Context::with_key(&hmac_key);
            ctx.update(&aad_bytes);
            ctx.update(&nonce);
            ctx.update(plaintext);
            let tag = ctx.sign();
            let mut out = Vec::with_capacity(12 + plaintext.len() + 32);
            out.extend_from_slice(&nonce);
            out.extend_from_slice(plaintext);
            out.extend_from_slice(tag.as_ref());
            return Ok(out);
        }

        let key = key_from_bytes(mat.suite, &session_key)?;

        // Erst plaintext encrypten (AES-GCM haengt den 16-byte Tag
        // direkt an den Vec an). Danach nonce davor setzen.
        let mut payload: Vec<u8> = plaintext.to_vec();
        let nonce_obj = Nonce::assume_unique_for_key(nonce);
        key.seal_in_place_append_tag(nonce_obj, ring::aead::Aad::from(&aad_bytes), &mut payload)
            .map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: seal failed")
            })?;
        // Wire-Format: [nonce(12) | ciphertext + tag(16)].
        let mut out = Vec::with_capacity(12 + payload.len());
        out.extend_from_slice(&nonce);
        out.extend(payload);
        Ok(out)
    }

    fn decrypt_submessage(
        &self,
        local: CryptoHandle,
        _remote: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        #[cfg(feature = "metrics")]
        let _op = crate::metrics::CryptoOp::start("decrypt");
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&local).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "crypto: unknown handle")
        })?;

        // Spec §10.5.2 Tab.74 + §8.1 Tab.78 — symmetrisch zu encrypt:
        // session_key + AAD aus master_salt + key_id + session_id +
        // extension (Caller-supplied, muss byte-identisch zur Sender-
        // AAD sein, sonst Tag-Mismatch).
        let session_key = mat.derive_session_key_bytes();
        let aad_bytes = mat.aad(aad_extension);

        if !mat.suite.is_aead() {
            // HMAC-Auth-only-Pfad: `[nonce(12) | plaintext | hmac(32)]`.
            if ciphertext.len() < 12 + 32 {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: hmac-buffer zu kurz fuer nonce+tag",
                ));
            }
            let (nonce_bytes, rest) = ciphertext.split_at(12);
            let (plain, tag) = rest.split_at(rest.len() - 32);
            let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, &session_key);
            let mut signed_input =
                Vec::with_capacity(aad_bytes.len() + nonce_bytes.len() + plain.len());
            signed_input.extend_from_slice(&aad_bytes);
            signed_input.extend_from_slice(nonce_bytes);
            signed_input.extend_from_slice(plain);
            hmac::verify(&hmac_key, &signed_input, tag).map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: hmac verify failed (tag mismatch)",
                )
            })?;
            return Ok(plain.to_vec());
        }

        if ciphertext.len() < 12 + 16 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: ciphertext zu kurz fuer nonce+tag",
            ));
        }
        let (nonce_bytes, ct) = ciphertext.split_at(12);
        let key = key_from_bytes(mat.suite, &session_key)?;

        let mut n = [0u8; 12];
        n.copy_from_slice(nonce_bytes);
        let mut buf = ct.to_vec();
        let nonce_obj = Nonce::assume_unique_for_key(n);
        let plain = key
            .open_in_place(nonce_obj, ring::aead::Aad::from(&aad_bytes), &mut buf)
            .map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: open/verify failed (tag mismatch?)",
                )
            })?;
        Ok(plain.to_vec())
    }

    fn encrypt_submessage_multi(
        &self,
        local: CryptoHandle,
        receivers: &[(CryptoHandle, u32)],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<(Vec<u8>, Vec<ReceiverMac>)> {
        let handles: Vec<CryptoHandle> = receivers.iter().map(|(h, _)| *h).collect();
        let ciphertext = self.encrypt_submessage(local, &handles, plaintext, aad_extension)?;

        // Pro Remote einen truncated-HMAC-SHA256 ueber den Ciphertext
        // bilden. Der HMAC-Key ist der pro-Receiver-Slot-master_key.
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mut macs = Vec::with_capacity(receivers.len());
        for (remote, key_id) in receivers {
            let mat = slots.get(remote).ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: unknown remote handle for receiver-specific mac",
                )
            })?;
            // Spec §10.5.2 Tab.74: Receiver-Specific-Key via HMAC-SHA256
            // (master_recv_key, master_salt || "SessionReceiverKey" ||
            // session_id) — wir mappen master_key auf den Receiver-Slot.
            let receiver_session_key = crate::session_key::derive_session_hmac_key(
                &mat.master_key,
                &mat.master_salt,
                &mat.session_id,
            );
            let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, &receiver_session_key);
            let tag = hmac::sign(&hmac_key, &ciphertext);
            // 16-byte Truncation (Spec §7.3.6.3 `receiver_mac octet[16]`).
            let mut mac16 = [0u8; 16];
            mac16.copy_from_slice(&tag.as_ref()[..16]);
            macs.push(ReceiverMac {
                key_id: *key_id,
                mac: mac16,
            });
        }
        Ok((ciphertext, macs))
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
        if macs.is_empty() {
            // Single-MAC-Path: kein Multi-MAC im Datagram → normaler
            // Decrypt-Pfad mit derselben AAD-Extension.
            return self.decrypt_submessage(local, remote, ciphertext, aad_extension);
        }

        let our_mac = macs
            .iter()
            .find(|m| m.key_id == own_key_id)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: no receiver-specific MAC matches own key_id",
                )
            })?;

        // Verify: HMAC(own_receiver_key, ciphertext) gegen our_mac.
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&own_mac_key_handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown own_mac_key_handle",
            )
        })?;
        // Spec §10.5.2 Tab.74 — Receiver-Side: gleiche Derivation wie
        // im Sender (oben in encrypt_submessage_multi).
        let receiver_session_key = crate::session_key::derive_session_hmac_key(
            &mat.master_key,
            &mat.master_salt,
            &mat.session_id,
        );
        let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, &receiver_session_key);
        let full_tag = hmac::sign(&hmac_key, ciphertext);
        if full_tag.as_ref()[..16] != our_mac.mac {
            return Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: receiver-specific mac mismatch",
            ));
        }
        drop(slots);

        // MAC stimmt → normaler Decrypt-Pfad mit Sender-Key (gleiche
        // AAD-Extension wie der Sender verwendet hat).
        self.decrypt_submessage(local, remote, ciphertext, aad_extension)
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Crypto:AES-GCM-GMAC:1.2"
    }
}

fn key_from_bytes(suite: Suite, k: &[u8]) -> SecurityResult<LessSafeKey> {
    if k.len() != suite.key_len() {
        return Err(SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            alloc::format!(
                "crypto: key_from_bytes expected {} bytes, got {}",
                suite.key_len(),
                k.len()
            ),
        ));
    }
    let algo = suite.algorithm().ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            "crypto: key_from_bytes fuer non-AEAD-Suite aufgerufen",
        )
    })?;
    let unbound = UnboundKey::new(algo, k).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            "crypto: UnboundKey creation",
        )
    })?;
    Ok(LessSafeKey::new(unbound))
}

#[allow(dead_code)]
fn suppress_unused_remote_map_warning(
    p: &AesGcmCryptoPlugin,
) -> &Mutex<BTreeMap<(CryptoHandle, IdentityHandle), CryptoHandle>> {
    &p.remote_map
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn plugin_class_id_matches_spec() {
        let p = AesGcmCryptoPlugin::new();
        assert_eq!(p.plugin_class_id(), "DDS:Crypto:AES-GCM-GMAC:1.2");
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();

        let plain = b"hello zerodds secure world";
        let ct = p.encrypt_submessage(local, &[remote], plain, &[]).unwrap();
        // Wire-Format laenge = plain + 12 nonce + 16 tag
        assert_eq!(ct.len(), plain.len() + 12 + 16);

        let back = p.decrypt_submessage(local, remote, &ct, &[]).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();

        let plain = b"AAAAAAAAAAAAAAAA";
        let mut ct = p.encrypt_submessage(local, &[remote], plain, &[]).unwrap();
        // Flip ein byte im ciphertext-Bereich.
        ct[14] ^= 0x01;
        let err = p.decrypt_submessage(local, remote, &ct, &[]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn two_encrypts_produce_different_ciphertexts() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();

        let plain = b"same plaintext";
        let ct1 = p.encrypt_submessage(local, &[], plain, &[]).unwrap();
        let ct2 = p.encrypt_submessage(local, &[], plain, &[]).unwrap();
        // Nonce-Counter ist unterschiedlich → wire-bytes unterschiedlich.
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn cross_plugin_interop_via_tokens() {
        // Alice verschluesselt, schickt Token an Bob, Bob dekodiert.
        let mut alice = AesGcmCryptoPlugin::new();
        let mut bob = AesGcmCryptoPlugin::new();

        let alice_local = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bob_local = bob
            .register_local_participant(IdentityHandle(2), &[])
            .unwrap();

        // Alice serialisiert ihren Master-Key als Token.
        let token = alice
            .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
            .unwrap();

        // Bob akzeptiert den Token und speichert unter einem neuen Handle.
        let alice_seen_by_bob = bob
            .register_matched_remote_participant(
                bob_local,
                IdentityHandle(1),
                SharedSecretHandle(1),
            )
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bob_local, alice_seen_by_bob, &token)
            .unwrap();

        // Alice verschluesselt ein Payload.
        let plain = b"cross-plugin-test";
        let ct = alice
            .encrypt_submessage(alice_local, &[], plain, &[])
            .unwrap();

        // Bob entschluesselt mit dem uebertragenen Key.
        let back = bob
            .decrypt_submessage(alice_seen_by_bob, CryptoHandle(0), &ct, &[])
            .unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn decrypt_rejects_too_short_input() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let err = p
            .decrypt_submessage(local, CryptoHandle(0), b"short", &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    // -------------------------------------------------------------
    // AES-GCM-256 Suite
    // -------------------------------------------------------------

    #[test]
    fn default_plugin_uses_aes128() {
        let p = AesGcmCryptoPlugin::new();
        assert_eq!(p.local_suite(), Suite::Aes128Gcm);
    }

    #[test]
    fn aes256_plugin_reports_aes256_suite() {
        let p = AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm);
        assert_eq!(p.local_suite(), Suite::Aes256Gcm);
    }

    #[test]
    fn aes256_encrypt_decrypt_roundtrip() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();

        let plain = b"aes-256 payload with forward secrecy";
        let ct = p.encrypt_submessage(local, &[remote], plain, &[]).unwrap();
        assert_eq!(ct.len(), plain.len() + 12 + 16);
        let back = p.decrypt_submessage(local, remote, &ct, &[]).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn aes256_tampered_ciphertext_fails_verify() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();

        let mut ct = p
            .encrypt_submessage(local, &[remote], b"0123456789abcdef0123", &[])
            .unwrap();
        ct[14] ^= 0x01;
        let err = p.decrypt_submessage(local, remote, &ct, &[]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn tokens_carry_suite_tag_so_cross_suite_interop_works() {
        // Alice = 256, Bob = 128. Alice serialisiert ihren 256er Key
        // via Token. Bob nimmt den Token entgegen — das Token enthaelt
        // den Spec-konformen Suite-Tag 0x04 (AES256_GCM, §10.5 Tab.79),
        // also nutzt Bob fuer DIESEN Slot AES-256, obwohl sein
        // local_suite auf 128 steht.
        let mut alice = AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm);
        let mut bob = AesGcmCryptoPlugin::with_suite(Suite::Aes128Gcm);

        let a_local = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let b_local = bob
            .register_local_participant(IdentityHandle(2), &[])
            .unwrap();

        let token = alice
            .create_local_participant_crypto_tokens(a_local, CryptoHandle(0))
            .unwrap();
        assert_eq!(
            token[0],
            crate::suite::transform_kind::AES256_GCM,
            "suite-tag must be Spec AES256_GCM (0x04)"
        );

        let alice_slot_in_bob = bob
            .register_matched_remote_participant(b_local, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(b_local, alice_slot_in_bob, &token)
            .unwrap();

        let plain = b"cross-suite interop ok";
        let ct = alice.encrypt_submessage(a_local, &[], plain, &[]).unwrap();
        let back = bob
            .decrypt_submessage(alice_slot_in_bob, CryptoHandle(0), &ct, &[])
            .unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn rejects_token_with_unknown_suite_id() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        // Token mit suite-id 0xFF (ungueltig).
        let bogus = [
            0xFFu8, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let err = p
            .set_remote_participant_crypto_tokens(local, remote, &bogus)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    // -------------------------------------------------------------
    //  — HMAC-SHA256 Auth-only + Key-Refresh
    // -------------------------------------------------------------

    #[test]
    fn hmac_only_suite_roundtrip_without_encryption() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::HmacSha256);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let plain = b"payload kept in plaintext, only signed";
        let signed = p.encrypt_submessage(local, &[], plain, &[]).unwrap();
        // Wire-Format: nonce(12) + plaintext + hmac(32). Plaintext
        // muss **unverschluesselt** vorhanden sein.
        assert!(
            signed.windows(plain.len()).any(|w| w == plain),
            "HMAC-Suite sollte plaintext NICHT verschluesseln"
        );
        let back = p
            .decrypt_submessage(local, CryptoHandle(0), &signed, &[])
            .unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn hmac_tampered_payload_fails_verify() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::HmacSha256);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let mut signed = p
            .encrypt_submessage(local, &[], b"original message", &[])
            .unwrap();
        // Flip byte in plaintext (nach nonce, vor tag).
        signed[15] ^= 0x01;
        let err = p
            .decrypt_submessage(local, CryptoHandle(0), &signed, &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn hmac_tampered_tag_fails_verify() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::HmacSha256);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let mut signed = p.encrypt_submessage(local, &[], b"x", &[]).unwrap();
        let last = signed.len() - 1;
        signed[last] ^= 0x01;
        let err = p
            .decrypt_submessage(local, CryptoHandle(0), &signed, &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn encrypts_remaining_decrements_per_call() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let before = p.encrypts_remaining(local).unwrap();
        let _ = p.encrypt_submessage(local, &[], b"x", &[]).unwrap();
        let after = p.encrypts_remaining(local).unwrap();
        assert_eq!(before - after, 1);
    }

    #[test]
    fn rotate_key_resets_counter_and_changes_key() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        // Encrypt + Token ziehen; dann rotieren; dann neuen Token
        // ziehen — beide Tokens muessen verschieden sein.
        let _ = p.encrypt_submessage(local, &[], b"x", &[]).unwrap();
        let token_before = p
            .create_local_participant_crypto_tokens(local, CryptoHandle(0))
            .unwrap();

        p.rotate_key(local).unwrap();
        assert_eq!(
            p.encrypts_remaining(local).unwrap(),
            Suite::Aes128Gcm.max_encrypts(),
            "counter muss nach rotate bei 0 starten"
        );

        let token_after = p
            .create_local_participant_crypto_tokens(local, CryptoHandle(0))
            .unwrap();
        assert_ne!(token_before, token_after, "master-key muss neu sein");
    }

    #[test]
    fn rotate_key_rejects_unknown_handle() {
        let mut p = AesGcmCryptoPlugin::new();
        let err = p.rotate_key(CryptoHandle(9999)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    // =======================================================================
    // SharedSecretProvider-Integration (PKI ↔ Crypto)
    // =======================================================================

    use alloc::collections::BTreeMap as BTreeMap2;
    use alloc::sync::Arc as ArcA;
    use std::sync::RwLock as StdRwLock;

    /// Minimaler In-Memory-Provider fuer Tests; in der Produktion
    /// kommt `PkiAuthenticationPlugin` zum Einsatz (implementiert
    /// `SharedSecretProvider` in `zerodds-security-pki`).
    struct MemProvider {
        inner: StdRwLock<BTreeMap2<SharedSecretHandle, Vec<u8>>>,
    }

    impl MemProvider {
        fn new() -> Self {
            Self {
                inner: StdRwLock::new(BTreeMap2::new()),
            }
        }
        fn insert(&self, handle: SharedSecretHandle, bytes: Vec<u8>) {
            self.inner.write().unwrap().insert(handle, bytes);
        }
    }

    impl SharedSecretProvider for MemProvider {
        fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
            self.inner.read().ok()?.get(&handle).cloned()
        }
    }

    #[test]
    fn with_secret_provider_derives_same_master_key_for_both_sides() {
        // Simulates: Alice & Bob kennen den gleichen 32-byte-DH-
        // Shared-Secret (aus X25519). Beide Seiten registrieren den
        // Slot via register_matched_remote_participant mit demselben
        // SharedSecretHandle — beide HKDF-leiten exakt denselben
        // Master-Key ab und koennen deshalb direkt kommunizieren ohne
        // Token-Exchange.
        let shared = alloc::vec![0xA5u8; 32];
        let provider_a = ArcA::new(MemProvider::new());
        let provider_b = ArcA::new(MemProvider::new());
        provider_a.insert(SharedSecretHandle(1), shared.clone());
        provider_b.insert(SharedSecretHandle(1), shared.clone());

        let mut alice = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            ArcA::clone(&provider_a) as ArcA<dyn SharedSecretProvider>,
        );
        let mut bob = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            ArcA::clone(&provider_b) as ArcA<dyn SharedSecretProvider>,
        );

        let alice_local = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bob_local = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();

        let alice_to_bob = alice
            .register_matched_remote_participant(
                alice_local,
                IdentityHandle(2),
                SharedSecretHandle(1),
            )
            .unwrap();
        let bob_to_alice = bob
            .register_matched_remote_participant(
                bob_local,
                IdentityHandle(1),
                SharedSecretHandle(1),
            )
            .unwrap();

        // Encrypt/Decrypt kreuzen: Alice encryptet, Bob entschluesselt.
        let plain = b"x25519-handshake-derived-key";
        let wire = alice
            .encrypt_submessage(alice_to_bob, &[], plain, &[])
            .unwrap();
        let back = bob
            .decrypt_submessage(bob_to_alice, bob_to_alice, &wire, &[])
            .unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn with_secret_provider_different_secrets_yield_distinct_keys() {
        // Zwei unterschiedliche DH-Shared-Secrets (Alice↔Bob vs
        // Alice↔Charlie) muessen zu zwei unterschiedlichen Master-Keys
        // fuehren — sonst waere das ein fataler Key-Reuse.
        let provider = ArcA::new(MemProvider::new());
        provider.insert(SharedSecretHandle(1), alloc::vec![0x11u8; 32]);
        provider.insert(SharedSecretHandle(2), alloc::vec![0x22u8; 32]);
        let mut p = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            ArcA::clone(&provider) as ArcA<dyn SharedSecretProvider>,
        );
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bob = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let charlie = p
            .register_matched_remote_participant(local, IdentityHandle(3), SharedSecretHandle(2))
            .unwrap();
        let tok_bob = p
            .create_local_participant_crypto_tokens(bob, CryptoHandle(0))
            .unwrap();
        let tok_charlie = p
            .create_local_participant_crypto_tokens(charlie, CryptoHandle(0))
            .unwrap();
        assert_ne!(
            tok_bob, tok_charlie,
            "DH-Shared-Secrets muessen zu verschiedenen Per-Peer-Keys fuehren"
        );
    }

    #[test]
    fn with_secret_provider_unknown_handle_falls_back_to_random() {
        // Wenn der Provider das Handle nicht kennt, nimmt der Plugin
        // einen Random-Key (v1.4-Pfad). Damit koexistieren DH-MAC-
        // Keys und Token-Exchange-Cipher-Keys im gleichen Plugin.
        let provider = ArcA::new(MemProvider::new()); // leer
        let mut p = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            ArcA::clone(&provider) as ArcA<dyn SharedSecretProvider>,
        );
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let h = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(42))
            .expect("unknown handle → random slot, kein Error");
        // Slot existiert und ist nutzbar (Random-Key).
        let _tok = p
            .create_local_participant_crypto_tokens(h, CryptoHandle(0))
            .unwrap();
    }

    #[test]
    fn without_provider_backward_compat_random_key_preserved() {
        // Kein Provider → v1.4-Pfad: Random-Key bei jedem Register.
        let mut p = AesGcmCryptoPlugin::new(); // ohne Provider
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let a = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b = p
            .register_matched_remote_participant(local, IdentityHandle(3), SharedSecretHandle(2))
            .unwrap();
        let tok_a = p
            .create_local_participant_crypto_tokens(a, CryptoHandle(0))
            .unwrap();
        let tok_b = p
            .create_local_participant_crypto_tokens(b, CryptoHandle(0))
            .unwrap();
        assert_ne!(tok_a, tok_b, "Random-Keys → zwei verschiedene Tokens");
    }

    #[test]
    fn hkdf_derivation_is_deterministic() {
        // Gleicher Secret → gleicher Master-Key bit-identisch.
        let secret = alloc::vec![0xCDu8; 32];
        let m1 = KeyMaterial::from_shared_secret(Suite::Aes128Gcm, &secret).unwrap();
        let m2 = KeyMaterial::from_shared_secret(Suite::Aes128Gcm, &secret).unwrap();
        assert_eq!(m1.master_key, m2.master_key);
        assert_eq!(m1.session_id, m2.session_id);
    }

    #[test]
    fn hkdf_rejects_empty_secret() {
        let res = KeyMaterial::from_shared_secret(Suite::Aes128Gcm, &[]);
        match res {
            Err(e) => assert_eq!(e.kind, SecurityErrorKind::BadArgument),
            Ok(_) => panic!("expected BadArgument, got Ok"),
        }
    }
}
