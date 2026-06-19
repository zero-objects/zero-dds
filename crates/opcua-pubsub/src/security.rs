// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OPC-UA PubSub message security (Part 14 §8 + the SecurityHeader of
//! §7.2.2.2.7) — signing and encryption of UADP NetworkMessages plus the
//! key model and Security Key Service (SKS).
//!
//! A secured NetworkMessage is laid out as
//!
//! ```text
//! [NetworkMessage header (Security flag set)]
//! [SecurityHeader: flags, token id, nonce]
//! [payload — AES-CTR encrypted when the Encrypted flag is set]
//! [SecurityFooter (empty for the CTR policies)]
//! [Signature — HMAC-SHA256 over everything above]
//! ```
//!
//! The supported [`SecurityPolicy`]s (Part 14 §8.2.1) are `PubSub-Aes128-CTR`
//! and `PubSub-Aes256-CTR` (AES-CTR for confidentiality + a trailing
//! HMAC-SHA256 signature for integrity) and `PubSub-Aes256-GCM` (AES-256-GCM
//! AEAD — confidentiality and integrity in one pass, the 16-byte tag standing
//! in for the signature, with the message header as additional authenticated
//! data). Keys come from a [`SecurityKeyService`] keyed by `SecurityTokenId`,
//! exactly as the OPC-UA `GetSecurityKeys` method distributes them.
//!
//! This module is behind the `security` feature.

use alloc::vec::Vec;

use aes::{Aes128, Aes256};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use ctr::Ctr128BE;
use ctr::cipher::{KeyIvInit, StreamCipher};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::binary::{UaReader, UaWriter};
use crate::error::{DecodeError, EncodeError};
use crate::uadp::network_message::{self, NetworkMessage};

type HmacSha256 = Hmac<Sha256>;
type Aes128Ctr = Ctr128BE<Aes128>;
type Aes256Ctr = Ctr128BE<Aes256>;

// SecurityFlags (Part 14 §7.2.2.2.7.1).
const SF_SIGNED: u8 = 0x01;
const SF_ENCRYPTED: u8 = 0x02;
const SF_FOOTER: u8 = 0x04;

const KEY_NONCE_LEN: usize = 4;
const MESSAGE_NONCE_LEN: usize = 8; // 4 random + 4 sequence (Part 14 §8.2.1.3)
const SIGNATURE_LEN: usize = 32; // HMAC-SHA256
const AES_IV_LEN: usize = 16;
const GCM_NONCE_LEN: usize = KEY_NONCE_LEN + MESSAGE_NONCE_LEN; // 12 (96-bit)
const GCM_TAG_LEN: usize = 16;

/// A UADP SecurityPolicy (Part 14 §8.2.1) supported for message security.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityPolicy {
    /// `PubSub-Aes128-CTR`: AES-128-CTR + HMAC-SHA256.
    Aes128Ctr,
    /// `PubSub-Aes256-CTR`: AES-256-CTR + HMAC-SHA256.
    Aes256Ctr,
    /// `PubSub-Aes256-GCM`: AES-256-GCM (AEAD — encryption + integrity in one).
    Aes256Gcm,
}

impl SecurityPolicy {
    /// The SecurityPolicy URI.
    #[must_use]
    pub const fn uri(self) -> &'static str {
        match self {
            Self::Aes128Ctr => "http://opcfoundation.org/UA/SecurityPolicy#PubSub-Aes128-CTR",
            Self::Aes256Ctr => "http://opcfoundation.org/UA/SecurityPolicy#PubSub-Aes256-CTR",
            Self::Aes256Gcm => "http://opcfoundation.org/UA/SecurityPolicy#PubSub-Aes256-GCM",
        }
    }

    /// `true` for the AEAD (GCM) policy, whose tag provides integrity without
    /// a separate HMAC signature.
    #[must_use]
    pub const fn is_aead(self) -> bool {
        matches!(self, Self::Aes256Gcm)
    }

    /// Length of the HMAC-SHA256 signing key (`0` for the AEAD policy, whose
    /// integrity comes from the GCM tag).
    #[must_use]
    pub const fn signing_key_len(self) -> usize {
        match self {
            Self::Aes128Ctr | Self::Aes256Ctr => 32,
            Self::Aes256Gcm => 0,
        }
    }

    /// Length of the AES encrypting key.
    #[must_use]
    pub const fn encrypting_key_len(self) -> usize {
        match self {
            Self::Aes128Ctr => 16,
            Self::Aes256Ctr | Self::Aes256Gcm => 32,
        }
    }

    /// Length of the per-key nonce.
    #[must_use]
    pub const fn key_nonce_len(self) -> usize {
        KEY_NONCE_LEN
    }

    /// Total length of a flat key blob (signing ‖ encrypting ‖ key nonce).
    #[must_use]
    pub const fn key_material_len(self) -> usize {
        self.signing_key_len() + self.encrypting_key_len() + self.key_nonce_len()
    }
}

/// One set of PubSub keys identified by a `SecurityTokenId` (Part 14 §8.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityKey {
    /// Identifies this key within its SecurityGroup.
    pub token_id: u32,
    /// HMAC-SHA256 signing key.
    pub signing_key: Vec<u8>,
    /// AES encrypting key.
    pub encrypting_key: Vec<u8>,
    /// Per-key nonce (high bytes of the AES-CTR IV).
    pub key_nonce: Vec<u8>,
}

impl SecurityKey {
    /// Splits a flat key blob (as distributed by `GetSecurityKeys`:
    /// signing ‖ encrypting ‖ key nonce) into a [`SecurityKey`].
    ///
    /// # Errors
    /// [`SecurityError::BadKeyLength`] if `blob` is not
    /// [`SecurityPolicy::key_material_len`] bytes.
    pub fn from_blob(
        policy: SecurityPolicy,
        token_id: u32,
        blob: &[u8],
    ) -> Result<Self, SecurityError> {
        if blob.len() != policy.key_material_len() {
            return Err(SecurityError::BadKeyLength);
        }
        let s = policy.signing_key_len();
        let e = policy.encrypting_key_len();
        Ok(Self {
            token_id,
            signing_key: blob[..s].to_vec(),
            encrypting_key: blob[s..s + e].to_vec(),
            key_nonce: blob[s + e..].to_vec(),
        })
    }

    fn validate(&self, policy: SecurityPolicy) -> Result<(), SecurityError> {
        if self.signing_key.len() != policy.signing_key_len()
            || self.encrypting_key.len() != policy.encrypting_key_len()
            || self.key_nonce.len() != policy.key_nonce_len()
        {
            return Err(SecurityError::BadKeyLength);
        }
        Ok(())
    }
}

/// The SecurityHeader of a secured NetworkMessage (Part 14 §7.2.2.2.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityHeader {
    /// SecurityFlags bitfield.
    pub flags: u8,
    /// Key identifier the message was protected with.
    pub security_token_id: u32,
    /// Per-message nonce (the variable part of the AES-CTR IV).
    pub message_nonce: Vec<u8>,
    /// Size of the SecurityFooter (0 = none / no footer flag).
    pub security_footer_size: u16,
}

impl SecurityHeader {
    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_u8(self.flags);
        w.write_u32(self.security_token_id);
        let nonce_len =
            u8::try_from(self.message_nonce.len()).map_err(|_| EncodeError::ValueOutOfRange {
                message: "MessageNonce longer than 255 bytes",
            })?;
        w.write_u8(nonce_len);
        w.write_bytes(&self.message_nonce);
        if self.flags & SF_FOOTER != 0 {
            w.write_u16(self.security_footer_size);
        }
        Ok(())
    }

    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let flags = r.read_u8()?;
        let security_token_id = r.read_u32()?;
        let nonce_len = r.read_u8()? as usize;
        let message_nonce = r.read_bytes(nonce_len)?.to_vec();
        let security_footer_size = if flags & SF_FOOTER != 0 {
            r.read_u16()?
        } else {
            0
        };
        Ok(Self {
            flags,
            security_token_id,
            message_nonce,
            security_footer_size,
        })
    }
}

/// The result of a `GetSecurityKeys` call (Part 14 §8.4.2) — the current key
/// plus a queue of future keys for a SecurityGroup.
#[derive(Debug, Clone, PartialEq)]
pub struct SecurityKeys {
    /// SecurityPolicy the keys are for.
    pub policy: SecurityPolicy,
    /// SecurityGroup the keys belong to.
    pub security_group_id: alloc::string::String,
    /// The currently active key.
    pub current_key: SecurityKey,
    /// Upcoming keys, in activation order.
    pub future_keys: Vec<SecurityKey>,
    /// Key lifetime in milliseconds.
    pub key_lifetime_ms: f64,
}

/// A Security Key Service (Part 14 §8.4) — stores and rotates the keys of one
/// SecurityGroup and looks them up by token id for `unprotect`.
#[derive(Debug, Clone)]
pub struct SecurityKeyService {
    policy: SecurityPolicy,
    security_group_id: alloc::string::String,
    current: SecurityKey,
    future: Vec<SecurityKey>,
    key_lifetime_ms: f64,
}

impl SecurityKeyService {
    /// Creates an SKS holding `current` as the active key.
    #[must_use]
    pub fn new(
        policy: SecurityPolicy,
        security_group_id: impl Into<alloc::string::String>,
        current: SecurityKey,
    ) -> Self {
        Self {
            policy,
            security_group_id: security_group_id.into(),
            current,
            future: Vec::new(),
            key_lifetime_ms: 3_600_000.0,
        }
    }

    /// The SecurityPolicy.
    #[must_use]
    pub const fn policy(&self) -> SecurityPolicy {
        self.policy
    }

    /// The currently active key.
    #[must_use]
    pub const fn current_key(&self) -> &SecurityKey {
        &self.current
    }

    /// Queues a future key (becomes current on [`rotate`](Self::rotate)).
    pub fn push_future_key(&mut self, key: SecurityKey) -> &mut Self {
        self.future.push(key);
        self
    }

    /// Sets the key lifetime advertised by `GetSecurityKeys`.
    pub fn set_key_lifetime_ms(&mut self, ms: f64) -> &mut Self {
        self.key_lifetime_ms = ms;
        self
    }

    /// Advances to the next future key, returning `false` if none is queued.
    pub fn rotate(&mut self) -> bool {
        if self.future.is_empty() {
            return false;
        }
        self.current = self.future.remove(0);
        true
    }

    /// Looks up a key by token id across the current and future keys.
    #[must_use]
    pub fn key_for_token(&self, token_id: u32) -> Option<&SecurityKey> {
        if self.current.token_id == token_id {
            return Some(&self.current);
        }
        self.future.iter().find(|k| k.token_id == token_id)
    }

    /// Builds the `GetSecurityKeys` response snapshot.
    #[must_use]
    pub fn get_security_keys(&self) -> SecurityKeys {
        SecurityKeys {
            policy: self.policy,
            security_group_id: self.security_group_id.clone(),
            current_key: self.current.clone(),
            future_keys: self.future.clone(),
            key_lifetime_ms: self.key_lifetime_ms,
        }
    }

    /// Computes the exact `GetSecurityKeys` method result (Part 14 §8.3.2): the
    /// flat key blobs (`signing ‖ encrypting ‖ key nonce`) starting at
    /// `starting_token_id` (`0` = current), at most `requested_key_count`
    /// (`0` = all available), for `security_group_id`.
    ///
    /// This is the server-method binding point a UA Server's `Call` service
    /// delegates to; the SecureChannel/Session transport that carries the call
    /// is caller-layer (ZeroDDS ships no OPC-UA server wire stack). Returns
    /// `None` if `security_group_id` is unknown.
    #[must_use]
    pub fn get_security_keys_for(
        &self,
        security_group_id: &str,
        starting_token_id: u32,
        requested_key_count: u32,
    ) -> Option<GetSecurityKeysResult> {
        if security_group_id != self.security_group_id {
            return None;
        }
        // Ordered key sequence: current first, then the queued future keys.
        let ordered = core::iter::once(&self.current).chain(self.future.iter());
        let start = if starting_token_id == 0 {
            self.current.token_id
        } else {
            starting_token_id
        };
        let selected: Vec<&SecurityKey> = ordered
            .skip_while(|k| k.token_id != start)
            .take(if requested_key_count == 0 {
                usize::MAX
            } else {
                requested_key_count as usize
            })
            .collect();
        let first_token_id = selected.first().map_or(start, |k| k.token_id);
        let keys = selected
            .iter()
            .map(|k| {
                let mut blob = Vec::with_capacity(
                    k.signing_key.len() + k.encrypting_key.len() + k.key_nonce.len(),
                );
                blob.extend_from_slice(&k.signing_key);
                blob.extend_from_slice(&k.encrypting_key);
                blob.extend_from_slice(&k.key_nonce);
                blob
            })
            .collect();
        Some(GetSecurityKeysResult {
            security_policy_uri: alloc::string::String::from(self.policy.uri()),
            first_token_id,
            keys,
            time_to_next_key_ms: self.key_lifetime_ms,
            key_lifetime_ms: self.key_lifetime_ms,
        })
    }
}

/// The output of the `GetSecurityKeys` method (Part 14 §8.3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct GetSecurityKeysResult {
    /// SecurityPolicy URI the keys are for.
    pub security_policy_uri: alloc::string::String,
    /// Token id of the first returned key.
    pub first_token_id: u32,
    /// Flat key blobs (`signing ‖ encrypting ‖ key nonce`), in token order.
    pub keys: Vec<Vec<u8>>,
    /// Milliseconds until the next key becomes current.
    pub time_to_next_key_ms: f64,
    /// Key lifetime in milliseconds.
    pub key_lifetime_ms: f64,
}

/// An error from message protection / key handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// The cleartext NetworkMessage could not be encoded.
    Encode(EncodeError),
    /// The secured bytes could not be parsed / the cleartext failed to decode.
    Decode(DecodeError),
    /// No key in the SKS matches the message's `SecurityTokenId`.
    UnknownToken(u32),
    /// The HMAC signature did not verify (tampered or wrong key).
    SignatureMismatch,
    /// The bytes are not a secured NetworkMessage (Security flag clear).
    NotSecured,
    /// A key field had the wrong length for its policy.
    BadKeyLength,
    /// The MessageNonce had the wrong length for the policy.
    BadNonceLength,
    /// The secured message was truncated (missing footer/signature bytes).
    Truncated,
}

impl From<EncodeError> for SecurityError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

impl From<DecodeError> for SecurityError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl core::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::UnknownToken(t) => write!(f, "no key for SecurityTokenId {t}"),
            Self::SignatureMismatch => write!(f, "NetworkMessage signature verification failed"),
            Self::NotSecured => write!(f, "NetworkMessage is not secured"),
            Self::BadKeyLength => write!(f, "key field length does not match the policy"),
            Self::BadNonceLength => write!(f, "MessageNonce length does not match the policy"),
            Self::Truncated => write!(f, "secured NetworkMessage is truncated"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SecurityError {}

/// Builds the 16-byte AES-CTR initial counter block: key nonce ‖ message
/// nonce ‖ 32-bit block counter (0).
fn build_iv(key_nonce: &[u8], message_nonce: &[u8]) -> [u8; AES_IV_LEN] {
    let mut iv = [0u8; AES_IV_LEN];
    iv[..KEY_NONCE_LEN].copy_from_slice(key_nonce);
    iv[KEY_NONCE_LEN..KEY_NONCE_LEN + MESSAGE_NONCE_LEN].copy_from_slice(message_nonce);
    iv
}

/// Applies the AES-CTR keystream in place (encryption and decryption are the
/// same operation for CTR).
fn aes_ctr_xor(policy: SecurityPolicy, key: &SecurityKey, message_nonce: &[u8], buf: &mut [u8]) {
    let iv = build_iv(&key.key_nonce, message_nonce);
    match policy {
        SecurityPolicy::Aes128Ctr => {
            let mut c = Aes128Ctr::new(key.encrypting_key.as_slice().into(), (&iv).into());
            c.apply_keystream(buf);
        }
        SecurityPolicy::Aes256Ctr => {
            let mut c = Aes256Ctr::new(key.encrypting_key.as_slice().into(), (&iv).into());
            c.apply_keystream(buf);
        }
        // The AEAD policy never takes the CTR path (callers guard on
        // `policy.is_aead()` and use AES-GCM instead).
        SecurityPolicy::Aes256Gcm => {}
    }
}

/// Builds the 12-byte AES-GCM nonce: key nonce ‖ message nonce (96 bits).
fn build_gcm_nonce(key_nonce: &[u8], message_nonce: &[u8]) -> [u8; GCM_NONCE_LEN] {
    let mut nonce = [0u8; GCM_NONCE_LEN];
    nonce[..KEY_NONCE_LEN].copy_from_slice(key_nonce);
    nonce[KEY_NONCE_LEN..].copy_from_slice(message_nonce);
    nonce
}

/// Signs (and optionally encrypts) `nm` into a secured byte stream
/// (Part 14 §8.2.1).
///
/// For the CTR policies this is AES-CTR (when `encrypt`) plus a trailing
/// HMAC-SHA256 signature. For the GCM (AEAD) policy the payload is always
/// encrypted and authenticated in one pass with the message header as
/// additional authenticated data; `encrypt` is ignored.
///
/// `message_nonce` must be unique per message under a given key (the caller
/// supplies it — e.g. 4 random bytes plus the 4-byte NetworkMessage sequence
/// number) and must be 8 bytes (`MESSAGE_NONCE_LEN`).
///
/// # Errors
/// Returns [`SecurityError`] on bad key/nonce lengths or an encoding failure.
pub fn protect(
    nm: &NetworkMessage,
    policy: SecurityPolicy,
    key: &SecurityKey,
    message_nonce: &[u8],
    encrypt: bool,
) -> Result<Vec<u8>, SecurityError> {
    key.validate(policy)?;
    if message_nonce.len() != MESSAGE_NONCE_LEN {
        return Err(SecurityError::BadNonceLength);
    }

    let do_encrypt = encrypt || policy.is_aead();
    let mut out = UaWriter::new();
    nm.encode_header(&mut out, true)?;

    let mut flags = SF_SIGNED;
    if do_encrypt {
        flags |= SF_ENCRYPTED;
    }
    SecurityHeader {
        flags,
        security_token_id: key.token_id,
        message_nonce: message_nonce.to_vec(),
        security_footer_size: 0,
    }
    .encode(&mut out)?;

    let mut payload = UaWriter::new();
    nm.encode_payload(&mut payload)?;
    let payload = payload.into_vec();

    if policy.is_aead() {
        // AES-GCM: header ‖ secheader is the AAD; the AEAD blob (ciphertext ‖
        // 16-byte tag) carries both confidentiality and integrity.
        let nonce = build_gcm_nonce(&key.key_nonce, message_nonce);
        let cipher = <Aes256Gcm as KeyInit>::new_from_slice(&key.encrypting_key)
            .map_err(|_| SecurityError::BadKeyLength)?;
        let blob = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &payload,
                    aad: out.as_slice(),
                },
            )
            .map_err(|_| SecurityError::SignatureMismatch)?;
        let mut buf = out.into_vec();
        buf.extend_from_slice(&blob);
        return Ok(buf);
    }

    // CTR policies: optional AES-CTR encryption then a trailing HMAC.
    let mut payload = payload;
    if do_encrypt {
        aes_ctr_xor(policy, key, message_nonce, &mut payload);
    }
    out.write_bytes(&payload);
    let mut buf = out.into_vec();
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&key.signing_key)
        .map_err(|_| SecurityError::BadKeyLength)?;
    mac.update(&buf);
    let sig = mac.finalize().into_bytes();
    buf.extend_from_slice(&sig);
    Ok(buf)
}

/// Verifies and decrypts a secured NetworkMessage produced by [`protect`],
/// returning the cleartext message (Part 14 §8.2.1).
///
/// # Errors
/// Returns [`SecurityError`] if the message is not secured, references an
/// unknown token, fails signature verification, or is truncated/malformed.
pub fn unprotect(
    bytes: &[u8],
    policy: SecurityPolicy,
    sks: &SecurityKeyService,
) -> Result<NetworkMessage, SecurityError> {
    let mut r = UaReader::new(bytes);
    let header = network_message::decode_header(&mut r)?;
    if !header.security {
        return Err(SecurityError::NotSecured);
    }
    let sec = SecurityHeader::decode(&mut r)?;
    let payload_start = r.position();

    let key = sks
        .key_for_token(sec.security_token_id)
        .ok_or(SecurityError::UnknownToken(sec.security_token_id))?;
    key.validate(policy)?;
    if sec.message_nonce.len() != MESSAGE_NONCE_LEN {
        return Err(SecurityError::BadNonceLength);
    }

    if policy.is_aead() {
        // AES-GCM: header ‖ secheader is the AAD; the rest is ciphertext ‖ tag.
        if bytes.len() < payload_start + GCM_TAG_LEN {
            return Err(SecurityError::Truncated);
        }
        let nonce = build_gcm_nonce(&key.key_nonce, &sec.message_nonce);
        let cipher = <Aes256Gcm as KeyInit>::new_from_slice(&key.encrypting_key)
            .map_err(|_| SecurityError::BadKeyLength)?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &bytes[payload_start..],
                    aad: &bytes[..payload_start],
                },
            )
            .map_err(|_| SecurityError::SignatureMismatch)?;
        return Ok(network_message::finish_decode(header, &plaintext)?);
    }

    if bytes.len() < payload_start + sec.security_footer_size as usize + SIGNATURE_LEN {
        return Err(SecurityError::Truncated);
    }
    let sig_start = bytes.len() - SIGNATURE_LEN;
    let payload_end = sig_start - sec.security_footer_size as usize;

    // Verify the HMAC over everything but the trailing signature.
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&key.signing_key)
        .map_err(|_| SecurityError::BadKeyLength)?;
    mac.update(&bytes[..sig_start]);
    mac.verify_slice(&bytes[sig_start..])
        .map_err(|_| SecurityError::SignatureMismatch)?;

    let mut payload = bytes[payload_start..payload_end].to_vec();
    if sec.flags & SF_ENCRYPTED != 0 {
        aes_ctr_xor(policy, key, &sec.message_nonce, &mut payload);
    }

    Ok(network_message::finish_decode(header, &payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uadp::dataset_message::DataSetMessage;
    use crate::uadp::network_message::PublisherId;
    use zerodds_opcua_gateway::data_value::{Variant, VariantValue};

    fn sks(policy: SecurityPolicy) -> SecurityKeyService {
        let blob = alloc::vec![0xABu8; policy.key_material_len()];
        let key = SecurityKey::from_blob(policy, 7, &blob).expect("key");
        SecurityKeyService::new(policy, "group-1", key)
    }

    fn sample_nm() -> NetworkMessage {
        let mut nm = NetworkMessage::with_messages(alloc::vec![DataSetMessage::key_frame_variant(
            5,
            alloc::vec![Variant::scalar(VariantValue::Int32(1234))]
        ),]);
        nm.publisher_id = Some(PublisherId::UInt16(9));
        nm
    }

    const NONCE: [u8; MESSAGE_NONCE_LEN] = [1, 2, 3, 4, 5, 6, 7, 8];

    #[test]
    fn sign_only_round_trip() {
        let policy = SecurityPolicy::Aes256Ctr;
        let svc = sks(policy);
        let nm = sample_nm();
        let bytes = protect(&nm, policy, svc.current_key(), &NONCE, false).expect("protect");
        let back = unprotect(&bytes, policy, &svc).expect("unprotect");
        assert_eq!(back, nm);
    }

    #[test]
    fn encrypt_and_sign_round_trip_aes256() {
        let policy = SecurityPolicy::Aes256Ctr;
        let svc = sks(policy);
        let nm = sample_nm();
        let bytes = protect(&nm, policy, svc.current_key(), &NONCE, true).expect("protect");
        // The Int32 payload must not appear in the clear.
        assert!(!bytes.windows(4).any(|w| w == 1234i32.to_le_bytes()));
        let back = unprotect(&bytes, policy, &svc).expect("unprotect");
        assert_eq!(back, nm);
    }

    #[test]
    fn aead_gcm_round_trip() {
        let policy = SecurityPolicy::Aes256Gcm;
        let svc = sks(policy);
        let nm = sample_nm();
        let bytes = protect(&nm, policy, svc.current_key(), &NONCE, true).expect("protect");
        // AEAD: the Int32 payload must not appear in the clear.
        assert!(!bytes.windows(4).any(|w| w == 1234i32.to_le_bytes()));
        let back = unprotect(&bytes, policy, &svc).expect("unprotect");
        assert_eq!(back, nm);
    }

    #[test]
    fn aead_gcm_tag_detects_tampering() {
        let policy = SecurityPolicy::Aes256Gcm;
        let svc = sks(policy);
        let mut bytes = protect(&sample_nm(), policy, svc.current_key(), &NONCE, true).expect("p");
        let last = bytes.len() - 1; // flip a byte inside the AEAD blob (tag region)
        bytes[last] ^= 0xFF;
        assert_eq!(
            unprotect(&bytes, policy, &svc),
            Err(SecurityError::SignatureMismatch)
        );
    }

    #[test]
    fn gcm_key_has_no_signing_key() {
        assert_eq!(SecurityPolicy::Aes256Gcm.signing_key_len(), 0);
        assert!(SecurityPolicy::Aes256Gcm.is_aead());
        let svc = sks(SecurityPolicy::Aes256Gcm);
        assert!(svc.current_key().signing_key.is_empty());
        assert_eq!(svc.current_key().encrypting_key.len(), 32);
    }

    #[test]
    fn encrypt_and_sign_round_trip_aes128() {
        let policy = SecurityPolicy::Aes128Ctr;
        let svc = sks(policy);
        let nm = sample_nm();
        let bytes = protect(&nm, policy, svc.current_key(), &NONCE, true).expect("protect");
        let back = unprotect(&bytes, policy, &svc).expect("unprotect");
        assert_eq!(back, nm);
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let policy = SecurityPolicy::Aes256Ctr;
        let svc = sks(policy);
        let mut bytes = protect(&sample_nm(), policy, svc.current_key(), &NONCE, true).expect("p");
        let last = bytes.len() - SIGNATURE_LEN - 1;
        bytes[last] ^= 0xFF; // flip a payload byte
        assert_eq!(
            unprotect(&bytes, policy, &svc),
            Err(SecurityError::SignatureMismatch)
        );
    }

    #[test]
    fn unknown_token_is_rejected() {
        let policy = SecurityPolicy::Aes256Ctr;
        let svc = sks(policy);
        let bytes = protect(&sample_nm(), policy, svc.current_key(), &NONCE, false).expect("p");
        // An SKS with a different token cannot resolve the key.
        let other = sks(policy);
        let mut other = other;
        // Force a different current token id.
        other.current.token_id = 999;
        assert_eq!(
            unprotect(&bytes, policy, &other),
            Err(SecurityError::UnknownToken(7))
        );
    }

    #[test]
    fn plain_message_is_not_secured() {
        let policy = SecurityPolicy::Aes256Ctr;
        let svc = sks(policy);
        let bytes = crate::binary::to_binary(&sample_nm()).expect("enc");
        assert_eq!(
            unprotect(&bytes, policy, &svc),
            Err(SecurityError::NotSecured)
        );
    }

    #[test]
    fn sks_rotation_and_lookup() {
        let policy = SecurityPolicy::Aes128Ctr;
        let mut svc = sks(policy);
        let future =
            SecurityKey::from_blob(policy, 8, &alloc::vec![0xCD; policy.key_material_len()])
                .expect("k");
        svc.push_future_key(future);
        assert_eq!(svc.current_key().token_id, 7);
        assert!(svc.key_for_token(8).is_some());
        assert!(svc.rotate());
        assert_eq!(svc.current_key().token_id, 8);
        assert!(!svc.rotate());
    }

    #[test]
    fn get_security_keys_snapshot() {
        let policy = SecurityPolicy::Aes256Ctr;
        let svc = sks(policy);
        let keys = svc.get_security_keys();
        assert_eq!(keys.policy, policy);
        assert_eq!(keys.current_key.token_id, 7);
        assert_eq!(keys.security_group_id, "group-1");
    }

    #[test]
    fn get_security_keys_for_method_result() {
        let policy = SecurityPolicy::Aes128Ctr;
        let mut svc = sks(policy); // current token 7
        svc.push_future_key(
            SecurityKey::from_blob(policy, 8, &alloc::vec![0xCD; policy.key_material_len()])
                .expect("k8"),
        );

        // Unknown group → None.
        assert!(svc.get_security_keys_for("other", 0, 0).is_none());

        // starting = 0 (current), all keys → tokens 7 then 8.
        let r = svc.get_security_keys_for("group-1", 0, 0).expect("result");
        assert_eq!(r.security_policy_uri, policy.uri());
        assert_eq!(r.first_token_id, 7);
        assert_eq!(r.keys.len(), 2);
        assert_eq!(r.keys[0].len(), policy.key_material_len());

        // starting = 8, count = 1 → only the future key.
        let r2 = svc.get_security_keys_for("group-1", 8, 1).expect("result");
        assert_eq!(r2.first_token_id, 8);
        assert_eq!(r2.keys.len(), 1);
    }
}
