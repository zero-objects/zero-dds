// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AES-GCM-128 / AES-GCM-256 CryptographicPlugin implementation.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`Arc<dyn SharedSecretProvider>` is held so the
//! plugin works with arbitrary authentication plugins. The
//! provider is a pure lookup interface with no safety implication.)

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle, SharedSecretProvider};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use crate::backend::aead::{LessSafeKey, Nonce, UnboundKey};
use crate::backend::digest;
use crate::backend::hkdf;
use crate::backend::hmac;
use crate::backend::rand::{SecureRandom, SystemRandom};

use crate::suite::Suite;

mod spec;
pub use spec::{
    PROP_DOMAIN_ID, PROP_DOMAIN_TAG, PROP_ORIGIN_AUTHENTICATION_FALLBACK, PROP_RTPS_AAD,
    RECEIVER_MACS_PER_MESSAGE,
};

/// Session ID: 4-byte prefix of a 12-byte GCM nonce. Drawn
/// randomly per `CryptoHandle`; protects against nonce collision
/// between different keys.
type SessionId = [u8; 4];

/// The session of a key material (DDS-Security 1.2 §10.5.3.3.2 Table 81).
#[derive(Debug, Clone, Copy)]
struct Session {
    /// The SessionId.
    id: SessionId,
    /// `session_block_counter`: the blocks ciphered with its session key.
    blocks: u64,
}

/// `max_blocks_per_session` without `org.zerodds.sec.crypto.max_blocks_per_session`:
/// 2^32 blocks of 16 octets, 64 GiB per session key.
pub const DEFAULT_MAX_BLOCKS_PER_SESSION: u64 = 1 << 32;

/// The property `dds.sec.crypto.symmetric_cipher_algorithm` (DDS-Security
/// 1.2 §10.5.1 Table 71): of a DataWriter or DataReader it overrides, in the
/// properties of [`CryptographicPlugin::register_local_endpoint`], the
/// default of the DomainParticipant.
pub const SYMMETRIC_CIPHER_ALGORITHM: &str = "dds.sec.crypto.symmetric_cipher_algorithm";

/// A crypto slot — DDS-Security 1.2 §10.5.2 KeyMaterial-AES-GCM-GMAC
/// Tab.73 layout (C3.7-b):
///
/// ```text
/// transformation_kind     (1 byte, Suite::transform_kind_id)
/// master_salt             (32 bytes, for spec-conformant session_key
///                          derivation; spec §10.5.2 Tab.74)
/// sender_key_id           (4 bytes, transformation_key_id)
/// master_sender_key       (16 or 32 bytes, suite.key_len())
/// session_id              (4 bytes, rotated per session)
/// ```
///
/// Encrypt/decrypt use `derive_session_key(master_key, master_salt,
/// session_id)` as the per-submessage AES key + `compute_aad(transform_kind,
/// key_id, session_id, &[])` as the AES-GCM AAD. This makes the hot path
/// spec-byte-compatible with Cyclone DDS / FastDDS.
struct KeyMaterial {
    /// AEAD suite (128 vs. 256).
    suite: Suite,
    /// `transformation_key_id` (spec §10.5.2 Tab.73, 4 bytes). Unique
    /// per slot.
    transformation_key_id: [u8; 4],
    /// `transformation_key_revision` (§7.3.17, three octets), the first
    /// three octets of the `transformation_kind` of the CryptoHeader. Zero
    /// except for the key material of a pre-shared key (§10.5.2.1.3, Table
    /// 76: the high octets of the passphrase id).
    key_revision: [u8; 3],
    /// Master sender key. Length per `suite.key_len()` (16 or 32).
    master_key: Vec<u8>,
    /// `master_salt` (spec §10.5.2 Tab.73 + Tab.74), feeds into
    /// `derive_session_key`. `sequence<octet, 32>` (§10.5.2.1.1): up to 32
    /// octets; one of the suite key length is made, as Cyclone does it, and
    /// a peer's 16 octets for AES-128 are taken.
    master_salt: Vec<u8>,
    /// The current session: its SessionId and the blocks ciphered under its
    /// session key (DDS-Security 1.2 §10.5.3.3.2 Table 81). The SessionId
    /// "is incremented each time a new SessionKey is needed"; it never
    /// changed.
    session: Mutex<Session>,
    /// `max_blocks_per_session` (Table 81): the blocks one session key
    /// ciphers at most.
    max_blocks_per_session: u64,
    /// Nonce counter — monotonic per encrypt. On overflow (u64::MAX)
    /// the encrypt is rejected.
    counter: AtomicU64,
    /// The initial `InitializationVectorSuffix` (DDS-Security 1.2 §10.5.3.3.2
    /// Table 81: "An initially random NONCE"): the suffix of the n-th
    /// operation is `iv_start + n`, modulo 2^64.
    iv_start: u64,
    /// `receiver_specific_key_id` / `master_receiver_specific_key`
    /// (§10.5.2.1.1) and the origin authentication of a local key.
    origin: Origin,
}

/// A receiver-specific key (DDS-Security 1.2 §10.5.2.1.1): its
/// `receiver_specific_key_id` (not 0) and the `master_receiver_specific_key`
/// of the length of the sender key (§10.5.2.1.1 Table 74).
#[derive(Clone)]
struct ReceiverKey {
    id: [u8; 4],
    master: Vec<u8>,
}

/// What a key material holds for origin authentication.
///
/// A key a peer sent us (`receiver_key`) is the key of this side as the one
/// receiver: a message under it must carry a receiver-specific MAC computed
/// with it (§10.5.3.3.6). A local key whose Metadata Protection Kind
/// specifies origin authentication (`enabled`) has one receiver-specific key
/// for each remote endpoint it was given a token for (`receivers`, §10.5.3.1
/// `register_matched_remote_datareader` / `_datawriter`), and every message
/// under it carries a MAC for each of them.
struct Origin {
    receiver_key: Option<ReceiverKey>,
    enabled: bool,
    receivers: Mutex<Vec<(ReceiverId, ReceiverKey)>>,
}

/// The remote endpoint or participant a receiver-specific key is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReceiverId {
    /// The GUID of a remote endpoint (the bundled `CryptographicPlugin`
    /// operations name remote endpoints by it).
    Guid([u8; 16]),
    /// The handle a `register_matched_remote_*` operation of the
    /// `CryptoKeyFactory` returned (§9.5.1.8).
    Handle(u64),
}

impl Origin {
    fn none() -> Self {
        Self {
            receiver_key: None,
            enabled: false,
            receivers: Mutex::new(Vec::new()),
        }
    }
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
        let mut salt = alloc::vec![0u8; suite.key_len()];
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
            session: Mutex::new(Session { id: sid, blocks: 0 }),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            key_revision: [0; 3],
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin::none(),
        })
    }

    /// Spec-conformant token layout (DDS-Security 1.2 §10.5.2.1 Table 72,
    /// C3.7-b):
    ///
    /// ```text
    /// [transformation_kind(1) | session_id(4) | sender_key_id(4) |
    ///  master_salt(32) | master_sender_key(N)]
    /// ```
    ///
    /// Total: `1 + 4 + 4 + 32 + suite.key_len()` byte.
    // Spec-conformant token (de)serializer; the active path is serialize_keymat_cyclone.
    #[allow(dead_code)]
    fn from_serialized(serialized: &[u8]) -> SecurityResult<Self> {
        if serialized.len() < 1 + 4 + 4 + 32 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: token too short (at least 41 bytes before master_key)",
            ));
        }
        // Spec-conformant: AES128_GMAC=0x01, AES128_GCM=0x02, AES256_GMAC=0x03,
        // AES256_GCM=0x04 (see Suite::transform_kind_id).
        let suite = Suite::from_transform_kind_id(serialized[0]).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("crypto: unknown suite-id 0x{:02x}", serialized[0]),
            )
        })?;
        let expected = 1 + 4 + 4 + 32 + suite.key_len();
        if serialized.len() != expected {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("crypto: token for {suite:?} must be {expected} bytes"),
            ));
        }
        let mut sid = [0u8; 4];
        sid.copy_from_slice(&serialized[1..5]);
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&serialized[5..9]);
        let salt = serialized[9..41].to_vec();
        let mk = serialized[41..].to_vec();
        Ok(Self {
            suite,
            transformation_key_id: key_id,
            master_key: mk,
            master_salt: salt,
            session: Mutex::new(Session { id: sid, blocks: 0 }),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            key_revision: [0; 3],
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin::none(),
        })
    }

    #[allow(dead_code)] // Reserved API, see from_serialized
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + 4 + 4 + 32 + self.master_key.len());
        out.push(self.suite.transform_kind_id());
        out.extend_from_slice(&self.current_session_id());
        out.extend_from_slice(&self.transformation_key_id);
        out.extend_from_slice(&self.master_salt);
        out.extend_from_slice(&self.master_key);
        out
    }

    /// `KeyMaterial_AES_GCM_GMAC` in the **cyclone/spec CDR big-endian format**
    /// (DDS-Security §9.5.2.1.1) for the crypto token exchange:
    ///
    /// ```text
    /// transformation_kind          octet[4]      (e.g. {0,0,0,4} = AES256_GCM)
    /// master_salt                  seq<octet>    len(4 BE) || bytes
    /// sender_key_id                octet[4]
    /// master_sender_key            seq<octet>    len(4 BE) || bytes
    /// receiver_specific_key_id     octet[4]      (0 = none)
    /// master_receiver_specific_key seq<octet>    len(4 BE)=0
    /// ```
    ///
    /// All lengths 4-byte-aligned (32-byte keys + 4-byte IDs → no padding).
    /// No encapsulation header — this is the raw `dds.cryp.keymat` property
    /// value (88 bytes for AES256). Order: kind, SALT, key_id, KEY, ...
    fn serialize_keymat_cyclone(&self) -> Vec<u8> {
        self.serialize_keymat_for(None)
    }

    /// As [`Self::serialize_keymat_cyclone`], with the receiver-specific key
    /// of one receiver: `receiver_specific_key_id` and
    /// `master_receiver_specific_key` (§10.5.2.1.1). Without one the id is 0
    /// and the key empty ("if and only if").
    fn serialize_keymat_for(&self, receiver: Option<&ReceiverKey>) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 4 + 32 + 4 + 4 + self.master_key.len() + 8 + 32);
        out.extend_from_slice(&self.transformation_kind_bytes());
        out.extend_from_slice(&(self.master_salt.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.master_salt);
        out.extend_from_slice(&self.transformation_key_id);
        out.extend_from_slice(&(self.master_key.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.master_key);
        match receiver {
            Some(key) => {
                out.extend_from_slice(&key.id);
                out.extend_from_slice(&(key.master.len() as u32).to_be_bytes());
                out.extend_from_slice(&key.master);
            }
            None => {
                out.extend_from_slice(&[0u8; 4]); // receiver_specific_key_id = none
                out.extend_from_slice(&0u32.to_be_bytes()); // master_receiver_specific_key = empty
            }
        }
        out
    }

    /// Parses the cyclone/spec CDR format from [`Self::serialize_keymat_cyclone`],
    /// the receiver-specific key included (§10.5.2.1.1).
    fn from_keymat_cyclone(data: &[u8]) -> SecurityResult<Self> {
        fn read_seq<'a>(data: &'a [u8], o: &mut usize) -> SecurityResult<&'a [u8]> {
            if *o + 4 > data.len() {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: keymat seq length truncated",
                ));
            }
            let len =
                u32::from_be_bytes([data[*o], data[*o + 1], data[*o + 2], data[*o + 3]]) as usize;
            *o += 4;
            if *o + len > data.len() {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: keymat seq body truncated",
                ));
            }
            let s = &data[*o..*o + len];
            *o += len;
            Ok(s)
        }
        if data.len() < 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: keymat too short",
            ));
        }
        // transformation_kind = {0,0,0,id} (4-byte BE) → the last byte is the suite ID.
        let suite = Suite::from_transform_kind_id(data[3]).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown transformation_kind in the keymat",
            )
        })?;
        let mut o = 4;
        let salt = read_seq(data, &mut o)?.to_vec();
        if o + 4 > data.len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: keymat sender_key_id truncated",
            ));
        }
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&data[o..o + 4]);
        o += 4;
        let master_key = read_seq(data, &mut o)?.to_vec();
        // §10.5.2.1.1: master_salt is a sequence<octet, 32>. Cyclone sends
        // the suite key length (16 for AES-128); it was refused unless 32.
        if salt.is_empty() || salt.len() > 32 || master_key.len() != suite.key_len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: keymat salt/key length does not match the suite",
            ));
        }
        let master_salt = salt;
        // §10.5.2.1.1: receiver_specific_key_id is 0 if and only if
        // master_receiver_specific_key is empty; Table 74: the key has the
        // length of the sender key. A keymat that ends after the sender key
        // (an older peer) has none.
        let receiver_key = if o + 4 <= data.len() {
            let mut id = [0u8; 4];
            id.copy_from_slice(&data[o..o + 4]);
            o += 4;
            let master = read_seq(data, &mut o)?.to_vec();
            match (id == [0u8; 4], master.is_empty()) {
                (true, true) => None,
                (false, false) if master.len() == suite.key_len() => {
                    Some(ReceiverKey { id, master })
                }
                _ => {
                    return Err(SecurityError::new(
                        SecurityErrorKind::BadArgument,
                        "crypto: keymat receiver_specific_key_id and master_receiver_specific_key do not match",
                    ));
                }
            }
        } else {
            None
        };
        Ok(Self {
            suite,
            transformation_key_id: key_id,
            master_key,
            master_salt,
            session: Mutex::new(Session {
                id: [0u8; 4],
                blocks: 0,
            }),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            key_revision: [data[0], data[1], data[2]],
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin {
                receiver_key,
                ..Origin::none()
            },
        })
    }

    /// Returns the spec-conformant `transformation_kind` as a 4-byte BE
    /// array (spec §10.5 Tab.79). Used in the AAD.
    fn transformation_kind_bytes(&self) -> [u8; 4] {
        let [id0, id1, id2] = self.key_revision;
        [id0, id1, id2, self.suite.transform_kind_id()]
    }

    /// **Cyclone-conformant** submessage crypto (DDS-Security §9.5.3.3, AES-GCM
    /// with an **empty AAD** — `crypto_cipher_encrypt_data` with `outpdata` set
    /// encrypts the whole input without a separate AAD buffer).
    /// Returns `(CryptoHeader[20], ciphertext, common_mac[16])`:
    ///
    /// ```text
    /// session_id  = session_for(len)           (4 byte)
    /// iv_suffix   = ++counter                  (8 byte, BE)
    /// session_key = HMAC(master_key, "SessionKey" || master_salt[..keylen] || session_id)
    /// nonce(12)   = session_id || iv_suffix
    /// CryptoHeader(20, BE) = transform_kind(4) || key_id(4) || session_id(4) || iv_suffix(8)
    /// ```
    fn seal_cyclone(&self, plaintext: &[u8]) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let session_id = self.session_for(plaintext.len());
        let suffix = self.next_iv_suffix();
        let iv_suffix = suffix.to_be_bytes();
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &session_id,
            key_len,
        );
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&session_id);
        nonce[4..].copy_from_slice(&iv_suffix);
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        // GMAC (SIGN, !is_aead): the payload stays cleartext, 16-byte auth tag over
        // the plaintext (as AAD), empty body. GCM (ENCRYPT): seal in place.
        let (buf, tag) = if self.suite.is_aead() {
            let mut b = plaintext.to_vec();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::empty(),
                &mut b,
            )
            .map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: cyclone seal failed",
                )
            })?;
            let t = b.split_off(b.len() - 16);
            (b, t)
        } else {
            // GMAC (SIGN): standard GMAC (cyclone-conformant, pcap-verified):
            // tag = GHASH(plaintext-as-AAD, empty body); the payload stays
            // cleartext. Wire framing WITHOUT content.length (see encode_/
            // decode_serialized_payload or assemble_srtps_message).
            let mut sink: Vec<u8> = Vec::new();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::from(plaintext),
                &mut sink,
            )
            .map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: cyclone gmac failed",
                )
            })?;
            (plaintext.to_vec(), sink)
        };
        let mut header = Vec::with_capacity(20);
        header.extend_from_slice(&self.transformation_kind_bytes());
        header.extend_from_slice(&self.transformation_key_id);
        header.extend_from_slice(&session_id);
        header.extend_from_slice(&iv_suffix);
        Ok((header, buf, tag))
    }

    /// Like [`Self::seal_cyclone`], but with AAD = `compute_aad(kind, key_id,
    /// session_id, aad_extension)` (DDS-Security §7.4.6.6 RTPS message protection;
    /// aad_extension = RTPS header). Returns `(CryptoHeader[20], ct, common_mac)`.
    fn seal_rtps_cyclone(
        &self,
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let session_id = self.session_for(plaintext.len());
        let suffix = self.next_iv_suffix();
        let iv_suffix = suffix.to_be_bytes();
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &session_id,
            key_len,
        );
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&session_id);
        nonce[4..].copy_from_slice(&iv_suffix);
        let _ = aad_extension; // cyclone-conformant: SRTPS-GCM uses an empty AAD (like the data path)
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        // GMAC (rtps_protection=SIGN): submessages stay cleartext, 16-byte
        // auth tag over the plaintext (as AAD). GCM: seal in place.
        let (buf, tag) = if self.suite.is_aead() {
            let mut b = plaintext.to_vec();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::empty(),
                &mut b,
            )
            .map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: srtps seal failed")
            })?;
            let t = b.split_off(b.len() - 16);
            (b, t)
        } else {
            // GMAC (SIGN): standard GMAC (cyclone-conformant, pcap-verified):
            // tag = GHASH(plaintext-as-AAD, empty body); the payload stays
            // cleartext. Wire framing WITHOUT content.length (see encode_/
            // decode_serialized_payload or assemble_srtps_message).
            let mut sink: Vec<u8> = Vec::new();
            // GMAC AAD = body-only (cyclone convention; cross-vendor pcap-verified,
            // FastDDS confirmed via in-Rust brute force: AAD_MATCH variant=ct).
            let _ = aad_extension;
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::from(plaintext),
                &mut sink,
            )
            .map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: srtps gmac failed")
            })?;
            (plaintext.to_vec(), sink)
        };
        let mut header = Vec::with_capacity(20);
        header.extend_from_slice(&self.transformation_kind_bytes());
        header.extend_from_slice(&self.transformation_key_id);
        header.extend_from_slice(&session_id);
        header.extend_from_slice(&iv_suffix);
        Ok((header, buf, tag))
    }

    /// Opens a protected submessage: the common MAC and the content as
    /// [`Self::open_cyclone`], then the origin authentication of
    /// [`Self::check_receiver_mac`]. `footer` is the whole body of the
    /// SEC_POSTFIX, the `common_mac` and the `receiver_specific_macs`.
    fn open_submessage(&self, header: &[u8], ct: &[u8], footer: &[u8]) -> SecurityResult<Vec<u8>> {
        let (common_mac, tail) = footer.split_at_checked(16).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: SEC_POSTFIX shorter than the common_mac",
            )
        })?;
        let plain = self.open_cyclone(header, ct, common_mac)?;
        self.check_receiver_mac(header, common_mac, tail)?;
        Ok(plain)
    }

    /// The receiver-specific MAC of `key` for a message with the CryptoHeader
    /// `header` and the common MAC `common_mac` (DDS-Security 1.2
    /// §10.5.3.3.4.5, §10.5.3.3.6): the AES-GMAC tag, under the
    /// `SessionReceiverSpecificKey` of the SessionId of the header and with
    /// the InitializationVector of the header, computed on the `common_mac`.
    fn receiver_mac(
        &self,
        key: &ReceiverKey,
        header: &[u8],
        common_mac: &[u8],
    ) -> SecurityResult<[u8; 16]> {
        if header.len() != 20 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: receiver-specific MAC needs a 20-octet CryptoHeader",
            ));
        }
        let mut session_id = [0u8; 4];
        session_id.copy_from_slice(&header[8..12]);
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&header[8..20]);
        let key_len = self.suite.key_len();
        let session_key = crate::session_key::derive_session_receiver_key_cyclone(
            &key.master,
            &self.master_salt,
            &session_id,
            key_len,
        );
        let aead = key_from_bytes(self.suite, &session_key[..key_len])?;
        let mut sink: Vec<u8> = Vec::new();
        aead.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            crate::backend::aead::Aad::from(common_mac),
            &mut sink,
        )
        .map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: receiver-specific MAC failed",
            )
        })?;
        let mut mac = [0u8; 16];
        mac.copy_from_slice(sink.get(..16).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: receiver-specific MAC has the wrong length",
            )
        })?);
        Ok(mac)
    }

    /// The `receiver_specific_macs` of the CryptoFooter after the common
    /// MAC (§10.5.3.3.4.3): the number of MACs, then for each receiver of a
    /// key with origin authentication its `receiver_mac_key_id` and
    /// `receiver_mac`; a length of 0 for every other key.
    fn footer_tail(&self, header: &[u8], common_mac: &[u8]) -> SecurityResult<Vec<u8>> {
        self.footer_tail_for(header, common_mac, None)
    }

    /// As [`Self::footer_tail`], with the MACs of the receivers `only` names
    /// (`None`: of every receiver): `encode_datawriter_submessage` and
    /// `encode_rtps_message` prepare a message for a set of receivers at a
    /// time (§9.5.1.10.2, `receiving_datareader_crypto_list_index`).
    fn footer_tail_for(
        &self,
        header: &[u8],
        common_mac: &[u8],
        only: Option<&[ReceiverId]>,
    ) -> SecurityResult<Vec<u8>> {
        let receivers = self.origin.receivers.lock().map_err(|_| poisoned())?;
        let chosen: Vec<&ReceiverKey> = receivers
            .iter()
            .filter(|(id, _)| only.is_none_or(|ids| ids.contains(id)))
            .map(|(_, key)| key)
            .collect();
        if !self.origin.enabled || chosen.is_empty() {
            return Ok(0u32.to_be_bytes().to_vec());
        }
        let mut out = Vec::with_capacity(4 + chosen.len() * 20);
        out.extend_from_slice(&(chosen.len() as u32).to_be_bytes());
        for key in chosen {
            out.extend_from_slice(&key.id);
            out.extend_from_slice(&self.receiver_mac(key, header, common_mac)?);
        }
        Ok(out)
    }

    /// Origin authentication on the receiving side (§10.5.3.3.6): when the
    /// peer gave this side a receiver-specific key with the key material, the
    /// footer must hold the receiver-specific MAC of that key and it must be
    /// the one computed here; a message without it, or with another, is not
    /// from the key's sender. A key without a receiver-specific key checks
    /// nothing, and the MACs of other receivers in the footer are not its
    /// business. `tail` is what follows the common MAC in the CryptoFooter.
    fn check_receiver_mac(
        &self,
        header: &[u8],
        common_mac: &[u8],
        tail: &[u8],
    ) -> SecurityResult<()> {
        let Some(key) = &self.origin.receiver_key else {
            return Ok(());
        };
        let bad =
            |detail: &'static str| SecurityError::new(SecurityErrorKind::CryptoFailed, detail);
        let count = tail
            .get(..4)
            .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]) as usize)
            .ok_or_else(|| {
                bad("crypto: origin authentication: the footer has no receiver_specific_macs")
            })?;
        if count.checked_mul(20).and_then(|n| n.checked_add(4)) != Some(tail.len()) {
            return Err(bad(
                "crypto: origin authentication: receiver_specific_macs does not fill the footer",
            ));
        }
        let entry = tail[4..]
            .chunks_exact(20)
            .find(|entry| entry[..4] == key.id)
            .ok_or_else(|| {
                bad("crypto: origin authentication: no receiver-specific MAC for this receiver")
            })?;
        let expected = self.receiver_mac(key, header, common_mac)?;
        let diff = entry[4..]
            .iter()
            .zip(expected.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        if diff != 0 {
            return Err(bad(
                "crypto: origin authentication: receiver-specific MAC mismatch",
            ));
        }
        Ok(())
    }

    /// The receiver-specific key of this local key for the remote endpoint
    /// `remote`, made on the first call (§10.5.3.1
    /// `register_matched_remote_datareader` / `_datawriter`: "a new key
    /// (MasterReceiverREndpointSpecificKey) ... shared only with that one
    /// specific remote DataReader"). Its id is not 0 and is no id of another
    /// key of this side (`taken`).
    fn receiver_key_for(
        &self,
        remote: ReceiverId,
        rng: &SystemRandom,
        taken: &mut BTreeSet<[u8; 4]>,
    ) -> SecurityResult<ReceiverKey> {
        let mut receivers = self.origin.receivers.lock().map_err(|_| poisoned())?;
        if let Some((_, key)) = receivers.iter().find(|(r, _)| *r == remote) {
            return Ok(key.clone());
        }
        let mut master = alloc::vec![0u8; self.suite.key_len()];
        rng.fill(&mut master).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill master_receiver_specific_key failed",
            )
        })?;
        let id = loop {
            let mut id = [0u8; 4];
            rng.fill(&mut id).map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "rng fill receiver_specific_key_id failed",
                )
            })?;
            if id != [0; 4] && taken.insert(id) {
                break id;
            }
        };
        let key = ReceiverKey { id, master };
        receivers.push((remote, key.clone()));
        Ok(key)
    }

    /// §7.3.20.1, §9.5.2.1.1: the `transformation_kind` of a CryptoHeader
    /// (key revision and algorithm id) is part of the identifier the receiver
    /// decodes with. A header that names another algorithm than the key it
    /// found (a GMAC header on a GCM key, or the reverse) or a key revision
    /// this key is not is refused; the kind is not under the tag.
    fn check_header_kind(&self, header: &[u8]) -> SecurityResult<()> {
        if header.get(..4) != Some(self.transformation_kind_bytes().as_slice()) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: transformation_kind of the CryptoHeader does not match the key",
            ));
        }
        Ok(())
    }

    /// Like [`Self::seal_rtps_cyclone`], with the RTPS Header and the adjusted
    /// HeaderExtension as "Additional Authenticated Data" (DDS-Security 1.2
    /// §7.4.6.6.4, §10.9.3.3.3 `encode_rtps_message`): AES-GCM takes them as
    /// the AAD of the AEAD; GMAC takes them in front of the message it
    /// authenticates, as [`Self::open_rtps_cyclone`] reads them back.
    fn seal_rtps_with_aad(
        &self,
        plaintext: &[u8],
        aad: &[u8],
    ) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let session_id = self.session_for(plaintext.len());
        let suffix = self.next_iv_suffix();
        let iv_suffix = suffix.to_be_bytes();
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &session_id,
            key_len,
        );
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&session_id);
        nonce[4..].copy_from_slice(&iv_suffix);
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        let (body, tag) = if self.suite.is_aead() {
            let mut b = plaintext.to_vec();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::from(aad),
                &mut b,
            )
            .map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: srtps seal failed")
            })?;
            let t = b.split_off(b.len() - 16);
            (b, t)
        } else {
            let mut authenticated = aad.to_vec();
            authenticated.extend_from_slice(plaintext);
            let mut sink: Vec<u8> = Vec::new();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::from(&authenticated),
                &mut sink,
            )
            .map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: srtps gmac failed")
            })?;
            (plaintext.to_vec(), sink)
        };
        let mut header = Vec::with_capacity(20);
        header.extend_from_slice(&self.transformation_kind_bytes());
        header.extend_from_slice(&self.transformation_key_id);
        header.extend_from_slice(&session_id);
        header.extend_from_slice(&iv_suffix);
        Ok((header, body, tag))
    }

    /// Counterpart to [`Self::seal_rtps_cyclone`]. `aad_extension` is the
    /// "Additional Authenticated Data" of a message whose SRTPS_PREFIX sets
    /// the flag A: its RTPS Header and adjusted HeaderExtension (DDS-Security
    /// 1.2 §7.4.6.6, §10.5.3.3.1 Table 80, 2); empty without A.
    fn open_rtps_cyclone(
        &self,
        header: &[u8],
        ct: &[u8],
        tag: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        if header.len() != 20 || tag.len() != 16 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: srtps header/tag length wrong",
            ));
        }
        self.check_header_kind(header)?;
        let mut session_id = [0u8; 4];
        session_id.copy_from_slice(&header[8..12]);
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&header[8..12]);
        nonce[4..].copy_from_slice(&header[12..20]);
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &session_id,
            key_len,
        );
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        if self.suite.is_aead() {
            let mut buf = Vec::with_capacity(ct.len() + tag.len());
            buf.extend_from_slice(ct);
            buf.extend_from_slice(tag);
            let plain = key
                .open_in_place(
                    Nonce::assume_unique_for_key(nonce),
                    crate::backend::aead::Aad::from(aad_extension),
                    &mut buf,
                )
                .map_err(|_| {
                    SecurityError::new(
                        SecurityErrorKind::CryptoFailed,
                        "crypto: srtps open/verify failed (tag mismatch?)",
                    )
                })?;
            Ok(plain.to_vec())
        } else {
            // GMAC (SIGN): AAD = body-only (cyclone convention), symmetric to the
            // seal; with the flag A the RTPS Header and HeaderExtension ahead
            // of it.
            let mut authenticated = aad_extension.to_vec();
            authenticated.extend_from_slice(ct);
            let mut buf = tag.to_vec();
            key.open_in_place(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::from(&authenticated),
                &mut buf,
            )
            .map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: srtps gmac verify failed (tag mismatch?)",
                )
            })?;
            Ok(ct.to_vec())
        }
    }

    /// Counterpart to [`Self::seal_cyclone`]. `header` is the 20-byte
    /// CryptoHeader (session_id/iv_suffix come from it, not from `self`),
    /// `ct` the ciphertext, `tag` the 16-byte common_mac. `self.master_key`/
    /// `master_salt` are symmetric (both peers derive the same Kx key).
    fn open_cyclone(&self, header: &[u8], ct: &[u8], tag: &[u8]) -> SecurityResult<Vec<u8>> {
        if header.len() != 20 || tag.len() != 16 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: cyclone header/tag length wrong",
            ));
        }
        self.check_header_kind(header)?;
        let mut session_id = [0u8; 4];
        session_id.copy_from_slice(&header[8..12]);
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&header[8..12]);
        nonce[4..].copy_from_slice(&header[12..20]);
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &session_id,
            key_len,
        );
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        if self.suite.is_aead() {
            let mut buf = Vec::with_capacity(ct.len() + tag.len());
            buf.extend_from_slice(ct);
            buf.extend_from_slice(tag);
            let plain = key
                .open_in_place(
                    Nonce::assume_unique_for_key(nonce),
                    crate::backend::aead::Aad::empty(),
                    &mut buf,
                )
                .map_err(|_| {
                    SecurityError::new(
                        SecurityErrorKind::CryptoFailed,
                        "crypto: cyclone open/verify failed (tag mismatch?)",
                    )
                })?;
            Ok(plain.to_vec())
        } else {
            // GMAC (SIGN): standard GMAC — the tag verifies over ct as AAD,
            // empty ciphertext body (cyclone-conformant, pcap-verified v2).
            let mut buf = tag.to_vec();
            key.open_in_place(
                Nonce::assume_unique_for_key(nonce),
                crate::backend::aead::Aad::from(ct),
                &mut buf,
            )
            .map_err(|_| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: cyclone gmac verify failed (tag mismatch?)",
                )
            })?;
            Ok(ct.to_vec())
        }
    }

    /// Spec §10.5.2 Tab.74 + §8.1 Tab.78 — derives the per-submessage
    /// `session_key` for `session_id` (helpers of
    /// `zerodds_security_crypto::session_key`). The decode path reads the
    /// `session_id` from the **wire**
    /// (nonce prefix) instead of from the KeyMaterial — so the per-token
    /// exchanged data key (which carries **no** session_id, §9.5.2.1.1)
    /// is still decryptable.
    fn derive_session_key_bytes_for(&self, session_id: &[u8; 4]) -> Vec<u8> {
        let full =
            crate::session_key::derive_session_key(&self.master_key, &self.master_salt, session_id);
        full[..self.suite.key_len()].to_vec()
    }

    /// Builds the AAD for a submessage under `session_id`. `extension` is the
    /// RTPS header for `rtps_protection_kind != NONE` — for submessage
    /// protection `extension` is empty.
    fn aad_for(&self, session_id: [u8; 4], extension: &[u8]) -> Vec<u8> {
        crate::session_key::compute_aad(
            self.transformation_kind_bytes(),
            self.transformation_key_id,
            session_id,
            extension,
        )
    }

    /// Derives master key + session ID deterministically from a
    /// `SharedSecret` (32 bytes from an X25519 DH handshake) via HKDF-SHA256.
    /// Both communication partners compute the same
    /// master key without a token exchange.
    ///
    /// The `session_id` is also derived deterministically from the secret
    /// (as a second HKDF info), so that both sides produce
    /// compatible nonces. If the same secret is used for
    /// multiple slots, the caller must rotate the slots manually
    /// and separately.
    /// Derives the **Kx key** (KeyExchange channel, VolatileSecure, spec
    /// §9.5.3.5) deterministically from the handshake `SharedSecret`.
    /// Its own HKDF info domain (`dds.sec.crypto.kx`), so the Kx key is
    /// **cryptographically separate** from any data key (no
    /// key reuse). Both partners compute the same Kx key without a
    /// token exchange; data keys come independently via token.
    fn from_shared_secret_kx(suite: Suite, shared_secret: &[u8]) -> SecurityResult<Self> {
        Self::from_shared_secret_domain(suite, shared_secret, "dds.sec.crypto.kx")
    }

    /// VolatileSecure Kx key from the handshake SharedSecret + challenges,
    /// **byte-identical** to DDS-Security §9.5.3.5 / cyclone `calculate_kx_keys`:
    ///
    /// ```text
    /// master_salt       = HMAC-SHA256( SHA256(ch1 || "keyexchange salt" || ch2), shared_secret )
    /// master_sender_key = HMAC-SHA256( SHA256(ch2 || "key exchange key" || ch1), shared_secret )
    /// ```
    ///
    /// (The challenge order swaps between salt and key; cookies are each
    /// 16 bytes without a NUL.) `transformation_kind = AES256_GCM`, `sender_key_id =
    /// 0`, `session_id = 0`. Both peers derive the same Kx key — so
    /// the VolatileSecure channel is cross-vendor-readable. Replaces the earlier
    /// proprietary HKDF (`from_shared_secret_kx`), which cyclone did not share.
    fn from_kx_shared_secret(
        shared_secret: &[u8],
        challenge1: &[u8; 32],
        challenge2: &[u8; 32],
    ) -> SecurityResult<Self> {
        fn kx_part(a: &[u8; 32], cookie: &[u8], b: &[u8; 32], secret: &[u8]) -> [u8; 32] {
            let mut concat = Vec::with_capacity(32 + cookie.len() + 32);
            concat.extend_from_slice(a);
            concat.extend_from_slice(cookie);
            concat.extend_from_slice(b);
            let hash = digest::digest(&digest::SHA256, &concat);
            let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, hash.as_ref());
            let tag = hmac::sign(&hmac_key, secret);
            let mut out = [0u8; 32];
            out.copy_from_slice(tag.as_ref());
            out
        }
        let master_salt =
            kx_part(challenge1, b"keyexchange salt", challenge2, shared_secret).to_vec();
        let master_key =
            kx_part(challenge2, b"key exchange key", challenge1, shared_secret).to_vec();
        Ok(Self {
            suite: Suite::Aes256Gcm,
            transformation_key_id: [0u8; 4],
            master_key,
            master_salt,
            session: Mutex::new(Session {
                id: [0u8; 4],
                blocks: 0,
            }),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            key_revision: [0; 3],
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin::none(),
        })
    }

    /// HKDF derivation with a configurable info domain.
    fn from_shared_secret_domain(
        suite: Suite,
        shared_secret: &[u8],
        domain: &str,
    ) -> SecurityResult<Self> {
        if shared_secret.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: empty shared_secret",
            ));
        }
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"org.zerodds.crypto.shared-secret.v1");
        let prk = salt.extract(shared_secret);

        let expand = |suffix: &str, out_len: usize| -> SecurityResult<Vec<u8>> {
            let info = alloc::format!("{domain}.{suffix}");
            let info_arr = [info.as_bytes()];
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

        // Master key (spec §10.5.2 Tab.73).
        let master_key = expand("master_key", suite.key_len())?;

        // Master salt (spec §10.5.2 Tab.73 + Tab.74) — 32 bytes for
        // HMAC-SHA256-based session_key derivation.
        let master_salt = expand("master_salt", 32)?;

        // Sender key id (4 bytes) — unique per slot, feeds into the AAD.
        let key_id_vec = expand("sender_key_id", 4)?;
        let mut transformation_key_id = [0u8; 4];
        transformation_key_id.copy_from_slice(&key_id_vec);

        // Session id (4 bytes) — nonce prefix + AAD component.
        let sid_vec = expand("session_id", 4)?;
        let mut session_id = [0u8; 4];
        session_id.copy_from_slice(&sid_vec);

        Ok(Self {
            suite,
            transformation_key_id,
            master_key,
            master_salt,
            session: Mutex::new(Session {
                id: session_id,
                blocks: 0,
            }),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            key_revision: [0; 3],
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin::none(),
        })
    }

    /// The SessionId to cipher `len` octets under (DDS-Security 1.2
    /// §10.5.3.3.4): "If the operation detects that the counter would
    /// exceed the maximum then it should modify the SessionId and derive
    /// new session keys prior to transforming any of the input plain text."
    /// The SessionId is incremented (Table 81) and the block counter starts
    /// over.
    fn session_for(&self, len: usize) -> SessionId {
        let blocks = (len as u64).div_ceil(16).max(1);
        let mut session = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if session.blocks.saturating_add(blocks) > self.max_blocks_per_session {
            session.id = u32::from_be_bytes(session.id).wrapping_add(1).to_be_bytes();
            session.blocks = 0;
        }
        session.blocks = session.blocks.saturating_add(blocks);
        session.id
    }

    /// The SessionId of the current session.
    fn current_session_id(&self) -> SessionId {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .id
    }

    fn next_nonce(&self, session_id: SessionId) -> SecurityResult<[u8; 12]> {
        let c = self.counter.fetch_add(1, Ordering::Relaxed);
        if c == u64::MAX {
            return Err(SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: nonce-counter exhausted — key-refresh required ",
            ));
        }
        let mut n = [0u8; 12];
        n[..4].copy_from_slice(&session_id);
        n[4..].copy_from_slice(&self.iv_start.wrapping_add(c).to_be_bytes());
        Ok(n)
    }

    /// The `InitializationVectorSuffix` of the next operation (Table 81): it
    /// "shall be changed each time an encryption or MAC operation is
    /// performed using the same key", from a random start.
    fn next_iv_suffix(&self) -> u64 {
        self.iv_start
            .wrapping_add(self.counter.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

/// Manual length struct for `crate::backend::hkdf::Prk::expand`. The
/// ring crate's `KeyType` trait accepts only types that
/// provide `len()` + an HKDF algo.
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

/// AES-GCM crypto plugin. Keys are held in an internal slab;
/// lookup by `CryptoHandle`. Which suite locally created
/// keys have is determined by `local_suite` — remote keys come with their
/// own suite ID via token.
pub struct AesGcmCryptoPlugin {
    rng: SystemRandom,
    next_handle: AtomicU64,
    /// Suite for locally created keys.
    local_suite: Suite,
    /// Per-scope suite override (None = local_suite). participant_suite:
    /// participant RTPS key (rtps_protection); endpoint_suite: endpoint key
    /// for payload/submessage (data/metadata). SIGN -> Aes256Gmac, ENCRYPT
    /// -> Gcm. The Kx key stays GCM independently of this.
    participant_suite: Option<Suite>,
    endpoint_suite: Option<Suite>,
    /// Submessage key suite (metadata_protection-kind). If it differs from
    /// `endpoint_suite` (data_protection-kind), the endpoint has two keys:
    /// `slots` = submessage key (metadata-kind), `payload_slots` = payload
    /// key (data-kind).
    metadata_suite: Option<Suite>,
    // RwLock for `slots` — read-heavy (every encrypt/decrypt reads);
    // register happens rarely at setup.
    slots: RwLock<BTreeMap<CryptoHandle, KeyMaterial>>,
    /// Second (payload) key of a local datawriter endpoint whose
    /// submessages and payload are both protected, or whose metadata and
    /// data suites differ.
    payload_slots: RwLock<BTreeMap<CryptoHandle, KeyMaterial>>,
    /// The local datawriters whose `slots` key protects their submessages
    /// alone, so that a relay-only reader may have it (§9.5.1.9.5).
    submessage_keys: RwLock<BTreeSet<CryptoHandle>>,
    /// The `transformation_key_id`s of the keys this plugin made: a new one
    /// is drawn until no other key of this side has it (DDS-Security 1.2
    /// §10.5.2.2: the id identifies the key material). They were drawn at
    /// random without a check.
    local_key_ids: Mutex<BTreeSet<[u8; 4]>>,
    /// `max_blocks_per_session` of the keys this side makes (DDS-Security
    /// 1.2 §10.5.3.3.2 Table 81).
    max_blocks_per_session: u64,
    /// Kx keys for the KeyExchange channel (VolatileSecure, spec
    /// §9.5.3.5) — the same `CryptoHandle` as the data slot, but a
    /// separate (domain-separated) key from the handshake secret. Only
    /// filled if a `secret_provider` knows the secret; the
    /// data key comes independently via a token exchange.
    kx_slots: RwLock<BTreeMap<CryptoHandle, KeyMaterial>>,
    /// Optional SharedSecretProvider. If set,
    /// `register_matched_remote_participant` derives the master key
    /// deterministically from the DH shared secret instead of generating a random
    /// key. Backward compat: `None` = the v1.4 path with a
    /// random key and a token exchange.
    secret_provider: Option<Arc<dyn SharedSecretProvider>>,
    // Links a local/remote participant pair with the
    // remote slot used **for this link** — after
    // `set_remote_participant_crypto_tokens`.
    // (local, remote_identity) → remote_slot_handle
    #[allow(clippy::type_complexity)]
    remote_map: Mutex<BTreeMap<(CryptoHandle, IdentityHandle), CryptoHandle>>,
    // Protected discovery / per-endpoint keys (DDS-Security §9.5.2.1.1): every
    // remote endpoint (user writer + secure built-in discovery endpoints) has its
    // own KeyMaterial with its own transformation_key_id. The receiver
    // maps an incoming SEC_* submessage to the key ONLY via the key_id in the
    // CryptoHeader — hence index (sender, key_id) → slot handle (multiple per
    // peer; the KeyMaterial lives in `slots`, not cloneable because of the
    // AtomicU64 counter). The sender is its remote participant handle: a
    // transformation_key_id is chosen by its sender (§9.5.2.1.1), two peers may
    // choose the same one, and a key of one peer must not decode what arrives
    // from another.
    remote_by_key_id: RwLock<RemoteKeyIndex>,
    /// The state of the operations of the interfaces `CryptoKeyFactory`,
    /// `CryptoKeyExchange` and `CryptoTransform` (DDS-Security 1.2 §9.5.1,
    /// [`spec`]).
    spec: RwLock<spec::SpecState>,
}

/// The remote key index: (sender, `transformation_key_id`, revision) to the
/// slot handle (DDS-Security 1.2 §7.3.20.2).
type RemoteKeyIndex = BTreeMap<(CryptoHandle, [u8; 4], [u8; 3]), CryptoHandle>;

impl Default for AesGcmCryptoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AesGcmCryptoPlugin {
    /// Constructor with the default suite `AES-GCM-256`: DDS-Security 1.2
    /// §10.5.1.1 reads a missing or `AUTO` `symmetric_cipher_algorithm` as
    /// `AES256-GCM`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_suite(Suite::Aes256Gcm)
    }

    /// Constructor with an explicit suite (`Aes128Gcm` or `Aes256Gcm`).
    #[must_use]
    pub fn with_suite(suite: Suite) -> Self {
        Self {
            rng: SystemRandom::new(),
            next_handle: AtomicU64::new(0),
            local_suite: suite,
            slots: RwLock::new(BTreeMap::new()),
            payload_slots: RwLock::new(BTreeMap::new()),
            submessage_keys: RwLock::new(BTreeSet::new()),
            local_key_ids: Mutex::new(BTreeSet::new()),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            kx_slots: RwLock::new(BTreeMap::new()),
            remote_map: Mutex::new(BTreeMap::new()),
            remote_by_key_id: RwLock::new(BTreeMap::new()),
            spec: RwLock::new(spec::SpecState::default()),
            secret_provider: None,
            participant_suite: None,
            endpoint_suite: None,
            metadata_suite: None,
        }
    }

    /// The slot of the key `(owner, key_id, revision)`: the three-tuple of
    /// DDS-Security 1.2 §7.3.20.2 "sender_participant_guid,
    /// transformation_key_id, transformation_key_revision" identifies key
    /// material. The slot of the key `key_id` of the remote participant `owner`; with
    /// `None`, of any remote participant (the key-id-only lookup the
    /// owner-less trait operations keep).
    fn key_id_slot(
        &self,
        owner: Option<CryptoHandle>,
        key_id: [u8; 4],
        revision: [u8; 3],
    ) -> SecurityResult<Option<CryptoHandle>> {
        let index = self.remote_by_key_id.read().map_err(|_| poisoned())?;
        Ok(match owner {
            Some(owner) => index.get(&(owner, key_id, revision)).copied(),
            None => index
                .iter()
                .find(|((_, id, rev), _)| *id == key_id && *rev == revision)
                .map(|(_, slot)| *slot),
        })
    }

    /// Indexes the key material `mat` under (`owner`, its key id, its key
    /// revision) in its own slot; re-installing the same revision of the same
    /// key id of the same owner overwrites it. A revision of a key id
    /// (DDS-Security 1.2 §9.5.1.6) is a key of its own: the receiver keeps the
    /// revision it replaces, for what is still in flight under it, and drops
    /// the ones before that, "when it is safe to do so".
    fn index_remote_key(&self, owner: CryptoHandle, mat: KeyMaterial) -> SecurityResult<()> {
        self.index_remote_key_slot(owner, mat).map(|_| ())
    }

    /// As [`Self::index_remote_key`], returning the slot of the key.
    fn index_remote_key_slot(
        &self,
        owner: CryptoHandle,
        mat: KeyMaterial,
    ) -> SecurityResult<CryptoHandle> {
        let key_id = mat.transformation_key_id;
        let revision = mat.key_revision;
        let slot_handle = self
            .remote_by_key_id
            .read()
            .map_err(|_| poisoned())?
            .get(&(owner, key_id, revision))
            .copied()
            // Use the same allocator as `insert()` (next_id = fetch_add+1),
            // otherwise the slot handle (fetch_add WITHOUT +1) collides with the
            // last-assigned local endpoint handle and overwrites
            // its KeyMaterial.
            .unwrap_or_else(|| CryptoHandle(self.next_id()));
        let mut index = self.remote_by_key_id.write().map_err(|_| poisoned())?;
        index.insert((owner, key_id, revision), slot_handle);
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        slots.insert(slot_handle, mat);
        // Keep this revision and the one below it; drop the older ones.
        let mut revisions: Vec<[u8; 3]> = index
            .keys()
            .filter(|(o, id, _)| *o == owner && *id == key_id)
            .map(|(_, _, rev)| *rev)
            .collect();
        revisions.sort_by_key(|rev| zerodds_security::algorithm::key_revision_to_int(*rev));
        while revisions.len() > 2 {
            let oldest = revisions.remove(0);
            if let Some(slot) = index.remove(&(owner, key_id, oldest)) {
                slots.remove(&slot);
            }
        }
        Ok(slot_handle)
    }

    fn decode_data_by_key_id_of(
        &self,
        owner: Option<CryptoHandle>,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let (header, ct, footer) = parse_secure_submessages_footer(wire)?;
        // CryptoHeader (§9.5.2.1.1, 20 bytes BE): transform_kind(4) | key_id(4) |
        // session_id(4) | iv_suffix(8). The key is mapped via key_id.
        if header.len() < 8 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: SEC_PREFIX header too short for key_id",
            ));
        }
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&header[4..8]);
        let revision = [header[0], header[1], header[2]];
        let handle = self.key_id_slot(owner, key_id, revision)?.ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no remote key for this transformation_key_id and revision",
            )
        })?;
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: key_id slot disappeared",
            )
        })?;
        mat.open_submessage(&header, &ct, &footer)
    }

    /// SRTPS decode with the key `(owner, key_id)`; with `None`, the
    /// key-id-only lookup and, without an index entry, any slot with the key
    /// id (self/same-handle test).
    fn decode_rtps_message_of(
        &self,
        owner: Option<CryptoHandle>,
        message: &[u8],
        expected: Option<&spec::Expected>,
    ) -> SecurityResult<Vec<u8>> {
        let SrtpsMessage {
            header: rtps_header,
            extension,
            prefix_flags,
            cheader: ch,
            ct,
            tag,
            tail,
        } = parse_srtps(message)?;
        if ch.len() < 8 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: SRTPS CryptoHeader too short",
            ));
        }
        // Table 80, decode_rtps_message: with the flag P the message is
        // protected with the pre-shared key material (§10.5.2.1.3), which
        // this plugin does not hold; its MAC would be checked against the
        // wrong key.
        if prefix_flags & SRTPS_FLAG_P != 0 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: SRTPS with a pre-shared key, and no pre-shared key material here",
            ));
        }
        // §7.4.6.6.4: "If the AdditionalAuthenticatedDataFlag is set the
        // authentication tag(s) ... include also the RTPS Header and RTPS
        // HeaderExtension as Additional Authenticated Data". It was checked
        // against no AAD.
        let aad = if prefix_flags & SRTPS_FLAG_A != 0 {
            let mut aad = rtps_header.clone();
            aad.extend_from_slice(&header_extension_for_aad(&extension));
            aad
        } else {
            Vec::new()
        };
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&ch[4..8]);
        let revision = [ch[0], ch[1], ch[2]];
        let handle = self.key_id_slot(owner, key_id, revision)?;
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = match (handle, owner) {
            (Some(h), _) => slots.get(&h),
            (None, None) => slots
                .values()
                .find(|m| m.transformation_key_id == key_id && m.key_revision == revision),
            (None, Some(_)) => None,
        }
        .ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no key for SRTPS transformation_key_id",
            )
        })?;
        // Table 80: "the transformation_kind matches the one the receiving
        // DomainParticipant is expecting".
        if let Some(expected) = expected {
            expected.check(mat)?;
        }
        let body = mat.open_rtps_cyclone(&ch, &ct, &tag, &aad)?;
        mat.check_receiver_mac(&ch, &tag, &tail)?;
        // Encode prepends a source-marker INFO_SRC (§8.3.7.9) into the
        // protected body (cross-vendor attribution). On the receiving side
        // it is redundant — the source is the RTPS header prefix. Strip it,
        // so the SRTPS layer is transparent (decode(encode(x)) == x).
        // INFO_SRC layout: id(1)|flags(1)|octetsToNextHeader(2)|unused(4)|
        // version(2)|vendor(2)|guidPrefix(12) = 24 bytes; guidPrefix @ [12..24].
        let stripped: &[u8] =
            if body.len() >= 24 && body[0] == 0x0c && body.get(12..24) == rtps_header.get(8..20) {
                &body[24..]
            } else {
                &body[..]
            };
        decoded_rtps_message(&rtps_header, &extension, stripped)
    }

    /// `decode_serialized_payload` with the key `(owner, key_id)`; with
    /// `None`, the key-id-only lookup.
    fn decode_serialized_payload_of(
        &self,
        owner: Option<CryptoHandle>,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        if encoded.len() < 20 + 4 + 16 + 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: encoded serialized_payload too short",
            ));
        }
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&encoded[4..8]);
        let revision = [encoded[0], encoded[1], encoded[2]];
        let handle = self.key_id_slot(owner, key_id, revision)?.ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no remote key for serialized_payload key_id and revision",
            )
        })?;
        CryptographicPlugin::decode_serialized_payload_with(self, handle, encoded)
    }

    /// Builds a plugin slot with a real DH-derived per-peer key.
    /// The `SharedSecretProvider` provides the raw bytes
    /// from a completed authentication handshake; we derive
    /// a deterministic master key via HKDF-SHA256 that both
    /// sides can compute without a token exchange.
    ///
    /// With a provider: `register_matched_remote_participant` uses the
    /// passed `SharedSecretHandle` to pull the per-peer key from the
    /// PKI handshake. Without a provider the v1.4 behavior remains
    /// (random key + token exchange).
    #[must_use]
    pub fn with_secret_provider(suite: Suite, provider: Arc<dyn SharedSecretProvider>) -> Self {
        Self {
            rng: SystemRandom::new(),
            next_handle: AtomicU64::new(0),
            local_suite: suite,
            slots: RwLock::new(BTreeMap::new()),
            payload_slots: RwLock::new(BTreeMap::new()),
            submessage_keys: RwLock::new(BTreeSet::new()),
            local_key_ids: Mutex::new(BTreeSet::new()),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            kx_slots: RwLock::new(BTreeMap::new()),
            remote_map: Mutex::new(BTreeMap::new()),
            remote_by_key_id: RwLock::new(BTreeMap::new()),
            spec: RwLock::new(spec::SpecState::default()),
            secret_provider: Some(provider),
            participant_suite: None,
            endpoint_suite: None,
            metadata_suite: None,
        }
    }

    /// Local suite (for tests / metrics).
    #[must_use]
    pub fn local_suite(&self) -> Suite {
        self.local_suite
    }

    /// Sets the per-scope suites for locally created keys from the governance
    /// (SIGN -> Aes256Gmac, ENCRYPT -> Aes256Gcm). Call before boxing.
    pub fn set_local_protection_suites(
        &mut self,
        participant: Option<Suite>,
        endpoint: Option<Suite>,
        metadata: Option<Suite>,
    ) {
        self.participant_suite = participant;
        self.endpoint_suite = endpoint;
        self.metadata_suite = metadata;
    }

    /// Count of remaining safe encrypts on a slot.
    /// Wrapping point: `Suite::max_encrypts()` (2^48 by default).
    /// The caller should trigger a key refresh at `0`.
    ///
    /// # Errors
    /// `BadArgument` if the handle does not exist.
    pub fn encrypts_remaining(&self, handle: CryptoHandle) -> SecurityResult<u64> {
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "crypto: unknown handle")
        })?;
        let used = mat.counter.load(Ordering::Relaxed);
        let max = mat.suite.max_encrypts();
        Ok(max.saturating_sub(used))
    }

    /// Forces new key material in the slot. After `rotate_key` the
    /// old key is gone — the peer must pull a new token via
    /// [`CryptographicPlugin::create_local_participant_crypto_tokens`]
    /// and synchronize via
    /// [`CryptographicPlugin::set_remote_participant_crypto_tokens`].
    ///
    /// Spec §9.5.2.1.2 Tab.73: `sender_key_id` identifies the key
    /// material, and the receiver indexes its remote slots by exactly
    /// that id (`decode_data_by_key_id`). A rotation therefore has to
    /// replace `master_sender_key`, `master_salt` **and**
    /// `transformation_key_id` together. Keeping the old key id while
    /// swapping the key would put two different keys behind one id: the
    /// receiver's index would overwrite the previous entry on install,
    /// and every message still in flight under the old key would fail
    /// its tag check with no way to tell why.
    ///
    /// # Errors
    /// `BadArgument` if the handle does not exist; `CryptoFailed`
    /// if the RNG fails.
    pub fn rotate_key(&mut self, handle: CryptoHandle) -> SecurityResult<()> {
        let fresh = self.new_local_key(self.local_suite)?;
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        let slot = slots.get_mut(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: rotate_key unknown handle",
            )
        })?;
        slot.master_key = fresh.master_key;
        slot.master_salt = fresh.master_salt;
        slot.transformation_key_id = fresh.transformation_key_id;
        if let (Ok(mut session), Ok(fresh_session)) = (slot.session.lock(), fresh.session.lock()) {
            *session = *fresh_session;
        }
        slot.counter.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// A new key of `suite` for this side, with a `transformation_key_id`
    /// no other key of this side has and not 0, the id of the Kx key.
    fn new_local_key(&self, suite: Suite) -> SecurityResult<KeyMaterial> {
        let mut ids = self.local_key_ids.lock().map_err(|_| poisoned())?;
        loop {
            let mut mat = KeyMaterial::new_random(suite, &self.rng)?;
            if mat.transformation_key_id != [0; 4] && ids.insert(mat.transformation_key_id) {
                mat.max_blocks_per_session = self.max_blocks_per_session;
                return Ok(mat);
            }
        }
    }

    /// Sets `max_blocks_per_session` (DDS-Security 1.2 §10.5.3.3.2 Table 81)
    /// for the keys made after it: the blocks of 16 octets one session key
    /// ciphers before the SessionId is incremented. Fast DDS reads the value
    /// from `org.zerodds.sec.crypto.max_blocks_per_session`.
    pub fn set_max_blocks_per_session(&mut self, blocks: u64) {
        self.max_blocks_per_session = blocks.max(1);
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

fn encode_payload_from_mat(mat: &KeyMaterial, payload: &[u8]) -> SecurityResult<Vec<u8>> {
    let (header, ct, tag) = mat.seal_cyclone(payload)?;
    let mut out = Vec::with_capacity(20 + 4 + ct.len() + 16 + 4);
    out.extend_from_slice(&header);
    // GCM: SecureDataBody = content.length(4 BE) + ciphertext. GMAC (SIGN):
    // cyclone OMITS content.length — the cleartext payload follows directly after
    // the CryptoHeader (length implicit from the submessage). pcap-attested.
    if mat.suite.is_aead() {
        out.extend_from_slice(&(ct.len() as u32).to_be_bytes());
    }
    out.extend_from_slice(&ct);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&0u32.to_be_bytes());
    Ok(out)
}

/// Table 80, `encode_serialized_payload`: the CryptoFooter of a protected
/// SerializedPayload has an empty `receiver_specific_macs`. The format has no
/// length (§10.5.3.3.4.4), so a payload is read on that assumption, and a
/// footer that announces MACs (`count`, the 4 octets after `common_mac`)
/// is refused: with MACs the last 20 octets are not the footer.
fn check_no_receiver_macs(count: &[u8]) -> SecurityResult<()> {
    if count != [0u8; 4] {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: serialized_payload footer carries receiver_specific_macs",
        ));
    }
    Ok(())
}

/// A random initial `InitializationVectorSuffix` (DDS-Security 1.2 §10.5.3.3.2
/// Table 81).
fn random_iv_start() -> SecurityResult<u64> {
    let mut bytes = [0u8; 8];
    SystemRandom::new().fill(&mut bytes).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            "rng fill initialization_vector_suffix failed",
        )
    })?;
    Ok(u64::from_be_bytes(bytes))
}

fn poisoned() -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::Internal,
        "crypto: internal rwlock poisoned",
    )
}

// Cyclone wire-format constants (DDS-Security §7.3.6 / cyclone add_submessage).
const SEC_PREFIX_ID: u8 = 0x31;
const SEC_BODY_ID: u8 = 0x30;
const SEC_POSTFIX_ID: u8 = 0x32;
const SUBMSG_FLAG_LE: u8 = 0x01;

/// Builds the cyclone-conformant `SEC_PREFIX` + `SEC_BODY` + `SEC_POSTFIX` sequence.
/// Submessage header: id(1) || flags(1=LE) || octetsToNextHeader(2 LE). Contents:
/// CryptoHeader(20, BE fields), SEC_BODY content.length(**4 BE**) || ciphertext,
/// CryptoFooter common_mac(16) || receiver_specific_macs._length(**4 BE**)=0.
const SRTPS_PREFIX_ID: u8 = 0x33;
const SRTPS_POSTFIX_ID: u8 = 0x34;
/// PAD and INFO_TS (DDSI-RTPS 2.5 §9.4.5.1.3): the two submessages whose
/// `octetsToNextHeader == 0` means an empty body, not the rest of the
/// message.
const PAD_ID: u8 = 0x01;
const INFO_TS_ID: u8 = 0x09;

/// The body length of a submessage with `octetsToNextHeader` `otn`,
/// `remaining` octets after its header (DDSI-RTPS 2.5 §9.4.5.1.3): 0 makes
/// it the last submessage, up to the end of the message, "and the kind of
/// Submessage is NOT PAD or INFO_TS"; for those two it is an empty body.
fn submessage_body_len(id: u8, otn: usize, remaining: usize) -> usize {
    if otn == 0 && id != PAD_ID && id != INFO_TS_ID {
        remaining
    } else {
        otn
    }
}

/// SRTPS message-protection wire (DDS-Security §7.3.7): RTPS header(20, plaintext)
/// + SRTPS_PREFIX(CryptoHeader[20]) + SEC_BODY(ct) + SRTPS_POSTFIX(common_mac).
#[cfg(test)]
fn assemble_srtps_message(rtps_header: &[u8], cheader: &[u8], ct: &[u8], tag: &[u8]) -> Vec<u8> {
    assemble_srtps_message_with(rtps_header, cheader, ct, tag, &0u32.to_be_bytes())
}

/// As [`assemble_srtps_message`], with the `receiver_specific_macs` of the
/// CryptoFooter (`tail`, §10.5.3.3.4.3) in place of an empty list.
fn assemble_srtps_message_with(
    rtps_header: &[u8],
    cheader: &[u8],
    ct: &[u8],
    tag: &[u8],
    tail: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(rtps_header.len() + 24 + 8 + ct.len() + 24);
    out.extend_from_slice(rtps_header);
    out.push(SRTPS_PREFIX_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&(cheader.len() as u16).to_le_bytes());
    out.extend_from_slice(cheader);
    // GCM: ciphertext in the SEC_BODY (content.length + ct). GMAC (SIGN): cyclone
    // OMITS the SEC_BODY wrapper — the cleartext submessages stand directly
    // between SRTPS_PREFIX and SRTPS_POSTFIX (pcap-verified). kind @ cheader[3].
    if matches!(cheader.get(3), Some(1) | Some(3)) {
        out.extend_from_slice(ct);
    } else {
        push_sec_body(&mut out, ct);
    }
    out.push(SRTPS_POSTFIX_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&((tag.len() + tail.len()) as u16).to_le_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(tail);
    out
}

impl KeyMaterial {
    /// `encode_rtps_message` without "Additional Authenticated Data"
    /// (DDS-Security 1.2 §10.5.3.3.1 Table 80, 1): the RTPS Header stays, an
    /// INFO_SRC with its content leads the protected submessages, the flag A
    /// is unset. `only` names the receivers whose receiver-specific MACs go
    /// into the SRTPS_POSTFIX (`None`: every receiver of this key).
    fn encode_rtps_info_src(
        &self,
        message: &[u8],
        only: Option<&[ReceiverId]>,
    ) -> SecurityResult<Vec<u8>> {
        if message.len() < 20 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: rtps message too short",
            ));
        }
        let (rtps_header, body) = message.split_at(20);
        // Cross-vendor: an INFO_SRC (§8.3.7.9) as the first protected submessage
        // carries the source GuidPrefix into the encrypted body. cyclone
        // attributes the decrypted submessages to this source; without
        // INFO_SRC it reconstructs the source from the cleartext header and
        // swaps each 4-byte word of the GuidPrefix (-> unmatched
        // proxy, reliable stall). version[4..6]/vendor[6..8]/guidPrefix[8..20]
        // come from the RTPS header.
        let mut inner = Vec::with_capacity(8 + 20 + body.len());
        inner.push(0x0c); // SMID INFO_SRC
        inner.push(0x01); // flags: LittleEndian
        inner.extend_from_slice(&20u16.to_le_bytes()); // octetsToNextHeader
        inner.extend_from_slice(&[0u8; 4]); // unused
        inner.extend_from_slice(&rtps_header[4..8]); // ProtocolVersion + VendorId
        inner.extend_from_slice(&rtps_header[8..20]); // GuidPrefix
        inner.extend_from_slice(body);
        let (ch, ct, tag) = self.seal_rtps_cyclone(&inner, rtps_header)?;
        let tail = self.footer_tail_for(&ch, &tag, only)?;
        Ok(assemble_srtps_message_with(
            rtps_header,
            &ch,
            &ct,
            &tag,
            &tail,
        ))
    }

    /// `encode_rtps_message` with "Additional Authenticated Data" (Table 80,
    /// 2 and 3): the RTPS Header and the HeaderExtension, if there is one,
    /// stay in front of the SRTPS_PREFIX, no INFO_SRC is inserted, the flag A
    /// is set (and the flag P with `psk`), the tag covers the header and the
    /// HeaderExtension with `messageLength` and `messageChecksum` zero, and
    /// afterwards both values describe the encoded message.
    ///
    /// # Errors
    /// `BadArgument` for a message under 20 octets or a truncated
    /// HeaderExtension; `CryptoFailed` from the cipher.
    fn encode_rtps_aad(
        &self,
        message: &[u8],
        psk: bool,
        only: Option<&[ReceiverId]>,
    ) -> SecurityResult<Vec<u8>> {
        if message.len() < 20 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: rtps message too short",
            ));
        }
        let (rtps_header, rest) = message.split_at(20);
        let mut extension = Vec::new();
        let mut body = rest;
        if rest.first() == Some(&HEADER_EXTENSION_ID) && rest.len() >= 4 {
            let le = rest[1] & SUBMSG_FLAG_LE != 0;
            let len = if le {
                usize::from(u16::from_le_bytes([rest[2], rest[3]]))
            } else {
                usize::from(u16::from_be_bytes([rest[2], rest[3]]))
            };
            if rest.len() < 4 + len {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: HeaderExtension truncated",
                ));
            }
            extension = rest[..4 + len].to_vec();
            body = &rest[4 + len..];
        }
        let mut aad = rtps_header.to_vec();
        aad.extend_from_slice(&header_extension_for_aad(&extension));
        let (cheader, ct, tag) = self.seal_rtps_with_aad(body, &aad)?;
        let tail = self.footer_tail_for(&cheader, &tag, only)?;
        let flags = SRTPS_FLAG_A | if psk { SRTPS_FLAG_P } else { 0 };
        build_srtps_wire(rtps_header, &extension, flags, &cheader, &ct, &tag, &tail)
    }

    /// The same message with the receiver-specific MACs of the receivers
    /// `only` names, for a later call of `encode_rtps_message` with a
    /// non-zero `receiving_participant_crypto_list_index` (§9.5.1.10.4: "the
    /// calls ... modify the encoded_rtps_message, replacing the
    /// receiver-specific parts"). The common MAC and the content stay.
    fn retail_srtps(&self, encoded: &[u8], only: &[ReceiverId]) -> SecurityResult<Vec<u8>> {
        let m = parse_srtps(encoded)?;
        let tail = self.footer_tail_for(&m.cheader, &m.tag, Some(only))?;
        build_srtps_wire(
            &m.header,
            &m.extension,
            m.prefix_flags & (SRTPS_FLAG_A | SRTPS_FLAG_P),
            &m.cheader,
            &m.ct,
            &m.tag,
            &tail,
        )
    }

    /// A revision of this key material for `revise_local_entity_keys`
    /// (DDS-Security 1.2 §10.5.3.1): "The transformation_kind shall only
    /// change the key_revision member. The master_salt shall be entirely new.
    /// The sender_key_id shall remain the same. The master_sender_key shall
    /// be entirely new. The receiver_specific_key shall remain the same."
    fn revised(
        &self,
        revision: [u8; 3],
        max_blocks_per_session: u64,
        rng: &SystemRandom,
    ) -> SecurityResult<Self> {
        let mut master_key = alloc::vec![0u8; self.master_key.len()];
        rng.fill(&mut master_key).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill master_key failed",
            )
        })?;
        let mut master_salt = alloc::vec![0u8; self.master_salt.len()];
        rng.fill(&mut master_salt).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill master_salt failed",
            )
        })?;
        let mut session_id = [0u8; 4];
        rng.fill(&mut session_id).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill session_id failed",
            )
        })?;
        let receivers = self
            .origin
            .receivers
            .lock()
            .map_err(|_| poisoned())?
            .clone();
        Ok(Self {
            suite: self.suite,
            transformation_key_id: self.transformation_key_id,
            key_revision: revision,
            master_key,
            master_salt,
            session: Mutex::new(Session {
                id: session_id,
                blocks: 0,
            }),
            max_blocks_per_session,
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin {
                receiver_key: None,
                enabled: self.origin.enabled,
                receivers: Mutex::new(receivers),
            },
        })
    }
}

/// An SRTPS message from its parts: the RTPS Header, the HeaderExtension
/// (empty without one) between it and the SRTPS_PREFIX, the flags A and P of
/// the SRTPS_PREFIX, the CryptoHeader, the content, the common MAC and the
/// `receiver_specific_macs` (`tail`). With a HeaderExtension, its
/// `messageLength` and `messageChecksum` are set to the encoded message
/// (Table 80, 3.3).
fn build_srtps_wire(
    rtps_header: &[u8],
    extension: &[u8],
    flags: u8,
    cheader: &[u8],
    ct: &[u8],
    tag: &[u8],
    tail: &[u8],
) -> SecurityResult<Vec<u8>> {
    let mut out = assemble_srtps_message_with(rtps_header, cheader, ct, tag, tail);
    // The flags of the SRTPS_PREFIX, right behind the RTPS Header.
    out[21] |= flags;
    if !extension.is_empty() {
        out.splice(20..20, extension.iter().copied());
        adjust_header_extension(&mut out)?;
    }
    Ok(out)
}

/// The checksum a HeaderExtension with the flags `flags` carries for
/// `message` (whose checksum field is zero): the big-endian CRC-32C,
/// CRC-64/XZ or the MD5 digest (DDSI-RTPS 2.5 §9.4.2.15, Table 9.8).
fn header_extension_checksum(flags: u8, message: &[u8]) -> Vec<u8> {
    match (flags >> 5) & 0x03 {
        1 => zerodds_foundation::crc32c(message).to_be_bytes().to_vec(),
        2 => zerodds_foundation::crc64_xz(message).to_be_bytes().to_vec(),
        3 => zerodds_foundation::md5(message).to_vec(),
        _ => Vec::new(),
    }
}

/// Sets `messageLength` and `messageChecksum` of the HeaderExtension that
/// follows the RTPS Header in `message` to the length and the checksum of
/// `message` (DDSI-RTPS 2.5 §8.3.7.4: the checksum "shall be computed over
/// the entire RTPS Message" with its own field zero).
fn adjust_header_extension(message: &mut [u8]) -> SecurityResult<()> {
    if message.len() < 24 || message[20] != HEADER_EXTENSION_ID {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: no HeaderExtension behind the RTPS Header",
        ));
    }
    let flags = message[21];
    let (length, checksum) = header_extension_fields(flags);
    if let Some((at, 4)) = length {
        let total = u32::try_from(message.len()).map_err(|_| {
            SecurityError::new(SecurityErrorKind::BadArgument, "crypto: message too long")
        })?;
        let bytes = if flags & SUBMSG_FLAG_LE != 0 {
            total.to_le_bytes()
        } else {
            total.to_be_bytes()
        };
        message[24 + at..24 + at + 4].copy_from_slice(&bytes);
    }
    if let Some((at, len)) = checksum {
        message[24 + at..24 + at + len].fill(0);
        let sum = header_extension_checksum(flags, message);
        message[24 + at..24 + at + len].copy_from_slice(&sum);
    }
    Ok(())
}

/// The decoded RTPS message: the RTPS Header, the HeaderExtension as the
/// sender had it with `messageLength` and `messageChecksum` set to the
/// decoded message, and the body (Table 80, 3.3 and 4: they "correspond to
/// the values passed as input to the encode_rtps_message operation").
fn decoded_rtps_message(
    rtps_header: &[u8],
    extension: &[u8],
    body: &[u8],
) -> SecurityResult<Vec<u8>> {
    let mut out = Vec::with_capacity(rtps_header.len() + extension.len() + body.len());
    out.extend_from_slice(rtps_header);
    out.extend_from_slice(extension);
    out.extend_from_slice(body);
    if !extension.is_empty() {
        adjust_header_extension(&mut out)?;
    }
    Ok(out)
}

/// The key material of the RTPS pre-shared-key protection (DDS-Security 1.2
/// §10.5.2.1.3) and the `encode_rtps_message` / `decode_rtps_message` of
/// §10.9.3.3.3 that use it. The derivation of the material is
/// [`crate::psk_rtps`].
pub(crate) struct PskRtpsKey(KeyMaterial);

/// A message protected with a pre-shared key, taken apart far enough to
/// choose the key: the RTPS Header, the CryptoHeader and the flags of the
/// SRTPS_PREFIX.
pub(crate) struct PskWire {
    /// The RTPS Header (20 octets).
    pub(crate) header: Vec<u8>,
    /// The CryptoHeader (20 octets).
    pub(crate) cheader: Vec<u8>,
    message: SrtpsMessage,
}

/// Takes a message protected with a pre-shared key apart.
///
/// # Errors
/// `BadArgument` for a message that is no SRTPS message, whose SRTPS_PREFIX
/// does not set the PreSharedKeyFlag, or does not set the
/// AdditionalAuthenticatedDataFlag ("If this flag is not set, the decode
/// operation shall fail", Table 89).
pub(crate) fn parse_psk_wire(wire: &[u8]) -> SecurityResult<PskWire> {
    let message = parse_srtps(wire)?;
    if message.cheader.len() < 20 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: SRTPS CryptoHeader too short",
        ));
    }
    if message.prefix_flags & SRTPS_FLAG_P == 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: the SRTPS message is not protected with a pre-shared key",
        ));
    }
    if message.prefix_flags & SRTPS_FLAG_A == 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: the SRTPS message does not authenticate its RTPS Header (flag A not set)",
        ));
    }
    Ok(PskWire {
        header: message.header.clone(),
        cheader: message.cheader.clone(),
        message,
    })
}

impl PskRtpsKey {
    /// The key material for `suite` made of the derived values.
    ///
    /// # Errors
    /// `CryptoFailed` if the system random generator fails.
    pub(crate) fn new(
        suite: Suite,
        sender_key_id: [u8; 4],
        revision: [u8; 3],
        master_salt: Vec<u8>,
        master_key: Vec<u8>,
    ) -> SecurityResult<Self> {
        // The key is the same on every start, so the nonce must not repeat
        // across starts: the SessionId (which also selects the session
        // key) starts at random.
        let mut sid = [0u8; 4];
        SystemRandom::new().fill(&mut sid).map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "rng fill session_id failed",
            )
        })?;
        Ok(Self(KeyMaterial {
            suite,
            transformation_key_id: sender_key_id,
            key_revision: revision,
            master_key,
            master_salt,
            session: Mutex::new(Session { id: sid, blocks: 0 }),
            max_blocks_per_session: DEFAULT_MAX_BLOCKS_PER_SESSION,
            counter: AtomicU64::new(0),
            iv_start: random_iv_start()?,
            origin: Origin::none(),
        }))
    }

    /// The `transformation_kind` of the CryptoHeader this key writes.
    pub(crate) fn kind(&self) -> [u8; 4] {
        self.0.transformation_kind_bytes()
    }

    /// `encode_rtps_message` with `transform_with_psk` TRUE (Table 89): the
    /// RTPS Header and the HeaderExtension, if any, stay; then the
    /// SRTPS_PREFIX with the flags A and P, the body (the encrypted
    /// submessages under GCM, as they are under GMAC) and the SRTPS_POSTFIX
    /// with the tag, computed with the header and the HeaderExtension (its
    /// `messageLength` and `messageChecksum` zero) as AAD. The
    /// `messageLength` and the `messageChecksum` of the HeaderExtension then
    /// describe the encoded message.
    ///
    /// # Errors
    /// `BadArgument` for a message under 20 octets or a truncated
    /// HeaderExtension; `CryptoFailed` from the cipher.
    pub(crate) fn encode_rtps_message(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        self.0.encode_rtps_aad(message, true, None)
    }

    /// `decode_rtps_message` with the key chosen (Table 89): the tag over the
    /// body is checked with the RTPS Header and the HeaderExtension, its
    /// `messageLength` and `messageChecksum` zero, as AAD; on success the
    /// RTPS Header, the HeaderExtension without those two values, and the
    /// body.
    ///
    /// # Errors
    /// `BadArgument` if the CryptoHeader names another kind than this key's;
    /// `CryptoFailed` if the tag does not verify.
    pub(crate) fn decode_rtps_message(&self, wire: &PskWire) -> SecurityResult<Vec<u8>> {
        let m = &wire.message;
        if m.cheader[..4] != self.kind() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: the transformation kind is not the one expected",
            ));
        }
        let mut aad = m.header.clone();
        aad.extend_from_slice(&header_extension_for_aad(&m.extension));
        let body = self.0.open_rtps_cyclone(&m.cheader, &m.ct, &m.tag, &aad)?;
        decoded_rtps_message(&m.header, &m.extension, &body)
    }
}

/// Decomposed SRTPS message: (rtps_header[20], CryptoHeader[20], ciphertext, common_mac[16]).
#[cfg(test)]
type SrtpsParts = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

/// The flag A of the SRTPS_PREFIX: the RTPS Header and HeaderExtension are
/// "Additional Authenticated Data" (DDS-Security 1.2 §7.4.6.6 Table 7).
const SRTPS_FLAG_A: u8 = 0x02;
/// The flag P of the SRTPS_PREFIX: the message is protected with a
/// pre-shared key (Table 7).
const SRTPS_FLAG_P: u8 = 0x04;
/// The submessage id of the HeaderExtension (DDSI-RTPS 2.5 §9.4.5.1.1).
const HEADER_EXTENSION_ID: u8 = 0x00;

/// An SRTPS message taken apart.
struct SrtpsMessage {
    /// The RTPS Header (20 octets).
    header: Vec<u8>,
    /// The HeaderExtension submessage after the header, empty without one
    /// (Table 80: "the original RTPS Header and, if present, the original
    /// HeaderExtension, followed by the SecureRTPSPrefixSubMsg").
    extension: Vec<u8>,
    /// The flags of the SRTPS_PREFIX.
    prefix_flags: u8,
    /// The CryptoHeader (20 octets).
    cheader: Vec<u8>,
    /// The protected submessages (GMAC) or the ciphertext (GCM).
    ct: Vec<u8>,
    /// The common MAC (16 octets).
    tag: Vec<u8>,
    /// What follows the common MAC in the SRTPS_POSTFIX: the
    /// `receiver_specific_macs` (§10.5.3.3.4.3).
    tail: Vec<u8>,
}

/// Parses the SRTPS wire -> (rtps_header[20], CryptoHeader[20], ct, common_mac[16]).
#[cfg(test)]
fn parse_srtps_message(wire: &[u8]) -> SecurityResult<SrtpsParts> {
    let m = parse_srtps(wire)?;
    Ok((m.header, m.cheader, m.ct, m.tag))
}

/// Takes an SRTPS message apart, a HeaderExtension between the RTPS Header
/// and the SRTPS_PREFIX included.
fn parse_srtps(wire: &[u8]) -> SecurityResult<SrtpsMessage> {
    if wire.len() < 20 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: srtps too short",
        ));
    }
    let header = wire[..20].to_vec();
    let mut rest = &wire[20..];
    let mut extension = Vec::new();
    if rest.first() == Some(&HEADER_EXTENSION_ID) && rest.len() >= 4 {
        let len = if rest[1] & SUBMSG_FLAG_LE != 0 {
            u16::from_le_bytes([rest[2], rest[3]]) as usize
        } else {
            u16::from_be_bytes([rest[2], rest[3]]) as usize
        };
        if rest.len() < 4 + len {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: srtps HeaderExtension truncated",
            ));
        }
        extension = rest[..4 + len].to_vec();
        rest = &rest[4 + len..];
    }
    let prefix_flags = rest.get(1).copied().unwrap_or(0);
    let (cheader, ct, mut tag) = parse_srtps_body(rest)?;
    let tail = tag.split_off(16);
    Ok(SrtpsMessage {
        header,
        extension,
        prefix_flags,
        cheader,
        ct,
        tag,
        tail,
    })
}

/// Offset and length of a field in the body of a HeaderExtension.
type FieldSpan = Option<(usize, usize)>;

/// The offsets and lengths of `messageLength` and `messageChecksum` in the
/// body of a HeaderExtension (DDSI-RTPS 2.5 §9.4.5.2: L, T, U, W, C, P in
/// this order).
fn header_extension_fields(flags: u8) -> (FieldSpan, FieldSpan) {
    let mut at = 0;
    let length = (flags & 0x02 != 0).then(|| {
        let field = (at, 4);
        at += 4;
        field
    });
    if flags & 0x04 != 0 {
        at += 8;
    }
    if flags & 0x08 != 0 {
        at += 4;
    }
    if flags & 0x10 != 0 {
        at += 8;
    }
    let checksum = match (flags >> 5) & 0x03 {
        1 => Some((at, 4)),
        2 => Some((at, 8)),
        3 => Some((at, 16)),
        _ => None,
    };
    (length, checksum)
}

/// The HeaderExtension as "Additional Authenticated Data": `messageLength`
/// and `messageChecksum`, if present, set to zero (Table 80, 3.1, 3.2).
fn header_extension_for_aad(extension: &[u8]) -> Vec<u8> {
    let mut out = extension.to_vec();
    if out.len() < 4 {
        return out;
    }
    let (length, checksum) = header_extension_fields(out[1]);
    for (at, len) in [length, checksum].into_iter().flatten() {
        if let Some(field) = out.get_mut(4 + at..4 + at + len) {
            field.fill(0);
        }
    }
    out
}

fn parse_srtps_body(w0: &[u8]) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    fn rd(w: &[u8]) -> SecurityResult<(u8, &[u8], &[u8])> {
        if w.len() < 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: srtps submsg trunc",
            ));
        }
        let id = w[0];
        let len = if w[1] & SUBMSG_FLAG_LE != 0 {
            u16::from_le_bytes([w[2], w[3]]) as usize
        } else {
            u16::from_be_bytes([w[2], w[3]]) as usize
        };
        // DDSI-RTPS 2.5 §9.4.5.1.3: octetsToNextHeader==0 makes the submessage
        // the LAST, up to the message end, unless it is a PAD or INFO_TS.
        // OpenDDS sets that for the SRTPS_POSTFIX (otn=0); cyclone/ZeroDDS
        // carry an explicit length. Without this handling ZeroDDS read the
        // POSTFIX body as 0 bytes -> post.len() < 16 -> "expected
        // SRTPS_POSTFIX(16)" -> OpenDDS' SRTPS (secure SEDP + user DATA) was
        // COMPLETELY discarded (cross-vendor rtps-enc reverse-decode stall,
        // SEC_TRACE-attested: 59x crypto_error for OpenDDS-0x33). An INFO_TS
        // with the invalidate flag has otn=0 and an empty body, and taking
        // it for the last submessage swallowed the SRTPS_POSTFIX.
        let len = submessage_body_len(id, len, w.len().saturating_sub(4));
        if w.len() < 4 + len {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: srtps submsg body trunc",
            ));
        }
        Ok((id, &w[4..4 + len], &w[4 + len..]))
    }
    let (id1, pfx, r1) = rd(w0)?;
    if id1 != SRTPS_PREFIX_ID || pfx.len() != 20 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: expected SRTPS_PREFIX(20)",
        ));
    }
    // GMAC (SIGN): no SEC_BODY — r1 = [cleartext submessages][SRTPS_POSTFIX][..].
    // ct = the cleartext submessages between SRTPS_PREFIX and SRTPS_POSTFIX; the
    // common_mac (16 B) is at the start of the POSTFIX body.
    if matches!(pfx.get(3), Some(1) | Some(3)) {
        if r1.len() < 24 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: srtps gmac too short",
            ));
        }
        // The SRTPS_POSTFIX is found by walking the submessages, never at a
        // fixed distance from the end: Fast DDS puts a vendor submessage
        // (e.g. 0x80) after it, or receiver_specific_macs into it. A fast path
        // took the octet 24 before the end for the POSTFIX id when it was
        // 0x34, which an octet of a MAC or of that vendor submessage is about
        // once in 256 messages; ct and the MAC were then cut in the wrong
        // place and the GMAC check failed. The walk stops at the first
        // submessage with id 0x34.
        let mut cur = r1;
        let mut consumed = 0usize;
        loop {
            let (id, body, rest) = rd(cur)?;
            if id == SRTPS_POSTFIX_ID {
                if body.len() < 16 {
                    return Err(SecurityError::new(
                        SecurityErrorKind::BadArgument,
                        "crypto: SRTPS_POSTFIX common_mac too short",
                    ));
                }
                return Ok((pfx.to_vec(), r1[..consumed].to_vec(), body.to_vec()));
            }
            consumed += 4 + body.len();
            cur = rest;
        }
    }
    let (id2, body, r2) = rd(r1)?;
    if id2 != SEC_BODY_ID || body.len() < 4 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: expected SEC_BODY",
        ));
    }
    let ct_len = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
    if body.len() < 4 + ct_len {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: SEC_BODY len > submsg",
        ));
    }
    let ct = body[4..4 + ct_len].to_vec();
    let (id3, post, _r3) = rd(r2)?;
    if id3 != SRTPS_POSTFIX_ID || post.len() < 16 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: expected SRTPS_POSTFIX(16)",
        ));
    }
    Ok((pfx.to_vec(), ct, post.to_vec()))
}

/// `SEC_BODY`: `content.length` (4, BE) and the ciphertext, padded with
/// zeros to a multiple of four. The submessage after it must start on a
/// 32-bit boundary (DDSI-RTPS 2.5 §9.4.1); a ciphertext of odd length
/// without the padding moved the `SEC_POSTFIX` off it, and Cyclone refused
/// the whole message ("decode_rtps_message: invalid message"). The readers
/// take the ciphertext length from `content.length` and skip the padding.
fn push_sec_body(out: &mut Vec<u8>, ct: &[u8]) {
    let pad = (4 - ct.len() % 4) % 4;
    out.push(SEC_BODY_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&((4 + ct.len() + pad) as u16).to_le_bytes());
    out.extend_from_slice(&(ct.len() as u32).to_be_bytes());
    out.extend_from_slice(ct);
    out.extend(core::iter::repeat_n(0u8, pad));
}

fn assemble_secure_submessages(header: &[u8], ct: &[u8], tag: &[u8]) -> Vec<u8> {
    assemble_secure_submessages_with(header, ct, tag, &0u32.to_be_bytes())
}

/// As [`assemble_secure_submessages`], with the `receiver_specific_macs` of
/// the CryptoFooter (`tail`, §10.5.3.3.4.3) in place of an empty list.
fn assemble_secure_submessages_with(header: &[u8], ct: &[u8], tag: &[u8], tail: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + header.len() + 4 + 4 + ct.len() + 4 + tag.len() + 4);
    // SEC_PREFIX
    out.push(SEC_PREFIX_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&(header.len() as u16).to_le_bytes());
    out.extend_from_slice(header);
    // GCM: ciphertext in the SEC_BODY (content.length + ct). GMAC (metadata=SIGN):
    // cyclone OMITS the SEC_BODY wrapper — the cleartext submessage stands
    // directly between SEC_PREFIX and SEC_POSTFIX (pcap-verified, like SRTPS).
    // kind @ header[3] (1/3=GMAC, 2/4=GCM).
    if matches!(header.get(3), Some(1) | Some(3)) {
        out.extend_from_slice(ct);
    } else {
        push_sec_body(&mut out, ct);
    }
    // SEC_POSTFIX: common_mac + receiver_specific_macs._length(BE)=0
    out.push(SEC_POSTFIX_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&((tag.len() + tail.len()) as u16).to_le_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(tail);
    out
}

/// Parses the `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX` sequence back to
/// `(CryptoHeader[20], ciphertext, common_mac[16])`. Counterpart to
/// [`assemble_secure_submessages`]; tolerant of the endianness flag.
#[cfg(test)]
fn parse_secure_submessages(wire: &[u8]) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let (header, ct, mut footer) = parse_secure_submessages_footer(wire)?;
    footer.truncate(16);
    Ok((header, ct, footer))
}

/// As [`parse_secure_submessages`], the third element the whole body of the
/// SEC_POSTFIX: the `common_mac` (16 octets) and the `receiver_specific_macs`
/// that follow it (§10.5.3.3.4.3).
fn parse_secure_submessages_footer(wire: &[u8]) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    fn read_submsg(w: &[u8]) -> SecurityResult<(u8, &[u8], &[u8])> {
        if w.len() < 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: secure submessage truncated (header)",
            ));
        }
        let id = w[0];
        // Length endianness follows flag bit 0 (1=LE), like cyclone/RTPS.
        let len = if w[1] & SUBMSG_FLAG_LE != 0 {
            u16::from_le_bytes([w[2], w[3]]) as usize
        } else {
            u16::from_be_bytes([w[2], w[3]]) as usize
        };
        // DDSI-RTPS 2.5 §9.4.5.1.3: octetsToNextHeader==0 => last submessage, to
        // the message end, unless it is a PAD or INFO_TS (empty body). OpenDDS sets
        // that for the SEC_POSTFIX (per-submessage protection,
        // discovery_/data_protection=ENCRYPT); without this handling ZeroDDS read the
        // POSTFIX body as 0 B → "expected SEC_POSTFIX(16)" → could not decode OpenDDS'
        // secure SEDP/DATA → the user-datawriter token exchange never happened →
        // OpenDDS "Crypto Key not found" (like the SRTPS otn=0 fix in parse_srtps_body).
        let len = submessage_body_len(id, len, w.len().saturating_sub(4));
        if w.len() < 4 + len {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: secure submessage truncated (body)",
            ));
        }
        Ok((id, &w[4..4 + len], &w[4 + len..]))
    }
    let (id1, prefix_body, r1) = read_submsg(wire)?;
    if id1 != SEC_PREFIX_ID || prefix_body.len() != 20 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: expected SEC_PREFIX with 20-byte CryptoHeader",
        ));
    }
    // GMAC (metadata=SIGN): no SEC_BODY — r1 = [cleartext submessage][SEC_POSTFIX].
    if matches!(prefix_body.get(3), Some(1) | Some(3)) {
        let (_cid, _cbody, r2g) = read_submsg(r1)?;
        let ct = r1[..r1.len() - r2g.len()].to_vec();
        let (pid, postfix_body, _r3) = read_submsg(r2g)?;
        if pid != SEC_POSTFIX_ID || postfix_body.len() < 16 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: expected SEC_POSTFIX with 16-byte common_mac",
            ));
        }
        return Ok((prefix_body.to_vec(), ct, postfix_body.to_vec()));
    }
    let (id2, body_body, r2) = read_submsg(r1)?;
    if id2 != SEC_BODY_ID || body_body.len() < 4 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: expected SEC_BODY",
        ));
    }
    let ct_len =
        u32::from_be_bytes([body_body[0], body_body[1], body_body[2], body_body[3]]) as usize;
    if body_body.len() < 4 + ct_len {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: SEC_BODY content.length > submessage",
        ));
    }
    let ct = body_body[4..4 + ct_len].to_vec();
    let (id3, postfix_body, _r3) = read_submsg(r2)?;
    if id3 != SEC_POSTFIX_ID || postfix_body.len() < 16 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: expected SEC_POSTFIX with 16-byte common_mac",
        ));
    }
    Ok((prefix_body.to_vec(), ct, postfix_body.to_vec()))
}

/// Seal (AES-GCM or AES-GMAC auth-only) with the given `KeyMaterial`.
/// Wire format: `[nonce(12) | ciphertext + tag(16)]` (GCM) or
/// `[nonce(12) | plaintext | tag(16)]` (GMAC: AES-GCM over an empty
/// plaintext, the AAD and the plaintext authenticated). Shared core of
/// `encrypt_submessage` (data slot) and `encode_kx_submessage` (Kx slot).
fn seal_with(mat: &KeyMaterial, plaintext: &[u8], aad_extension: &[u8]) -> SecurityResult<Vec<u8>> {
    let session_id = mat.session_for(plaintext.len());
    let nonce = mat.next_nonce(session_id)?;
    // Spec §10.5.2 Tab.74: per-submessage session_key; spec §8.1 Tab.78: AAD.
    let session_key = mat.derive_session_key_bytes_for(&session_id);
    let aad_bytes = mat.aad_for(session_id, aad_extension);
    let key = key_from_bytes(mat.suite, &session_key)?;

    if !mat.suite.is_aead() {
        let mut authenticated = aad_bytes;
        authenticated.extend_from_slice(plaintext);
        let mut tag = Vec::new();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            crate::backend::aead::Aad::from(&authenticated),
            &mut tag,
        )
        .map_err(|_| SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: gmac failed"))?;
        let mut out = Vec::with_capacity(12 + plaintext.len() + 16);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(plaintext);
        out.extend_from_slice(&tag);
        return Ok(out);
    }

    let mut payload: Vec<u8> = plaintext.to_vec();
    let nonce_obj = Nonce::assume_unique_for_key(nonce);
    key.seal_in_place_append_tag(
        nonce_obj,
        crate::backend::aead::Aad::from(&aad_bytes),
        &mut payload,
    )
    .map_err(|_| SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: seal failed"))?;
    let mut out = Vec::with_capacity(12 + payload.len());
    out.extend_from_slice(&nonce);
    out.extend(payload);
    Ok(out)
}

/// Open (AES-GCM or AES-GMAC auth-only) with the given `KeyMaterial`.
/// Counterpart to [`seal_with`]; shared core of
/// `decrypt_submessage` (data slot) and `decode_kx_submessage` (Kx slot).
fn open_with(
    mat: &KeyMaterial,
    ciphertext: &[u8],
    aad_extension: &[u8],
) -> SecurityResult<Vec<u8>> {
    // Read session_id from the nonce prefix (wire) — not from the KeyMaterial,
    // so a per-token exchanged data key (without session_id) reproduces the
    // session_key chosen by the sender.
    if ciphertext.len() < 12 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: ciphertext too short for nonce",
        ));
    }
    let mut wire_session_id = [0u8; 4];
    wire_session_id.copy_from_slice(&ciphertext[..4]);
    let session_key = mat.derive_session_key_bytes_for(&wire_session_id);
    let aad_bytes = mat.aad_for(wire_session_id, aad_extension);

    if !mat.suite.is_aead() {
        if ciphertext.len() < 12 + 16 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: gmac buffer too short for nonce+tag",
            ));
        }
        let (nonce_bytes, rest) = ciphertext.split_at(12);
        let (plain, tag) = rest.split_at(rest.len() - 16);
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(nonce_bytes);
        let mut authenticated = aad_bytes;
        authenticated.extend_from_slice(plain);
        let key = key_from_bytes(mat.suite, &session_key)?;
        let mut buf = tag.to_vec();
        key.open_in_place(
            Nonce::assume_unique_for_key(nonce),
            crate::backend::aead::Aad::from(&authenticated),
            &mut buf,
        )
        .map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: gmac verify failed (tag mismatch)",
            )
        })?;
        return Ok(plain.to_vec());
    }

    if ciphertext.len() < 12 + 16 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: ciphertext too short for nonce+tag",
        ));
    }
    let (nonce_bytes, ct) = ciphertext.split_at(12);
    let key = key_from_bytes(mat.suite, &session_key)?;
    let mut n = [0u8; 12];
    n.copy_from_slice(nonce_bytes);
    let mut buf = ct.to_vec();
    let nonce_obj = Nonce::assume_unique_for_key(n);
    let plain = key
        .open_in_place(
            nonce_obj,
            crate::backend::aead::Aad::from(&aad_bytes),
            &mut buf,
        )
        .map_err(|_| {
            SecurityError::new(
                SecurityErrorKind::CryptoFailed,
                "crypto: open/verify failed (tag mismatch?)",
            )
        })?;
    Ok(plain.to_vec())
}

impl CryptographicPlugin for AesGcmCryptoPlugin {
    fn register_local_participant(
        &mut self,
        _identity: IdentityHandle,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        let mut mat = self.new_local_key(self.participant_suite.unwrap_or(self.local_suite))?;
        // §10.5.3.1 register_matched_remote_participant: with an RTPS
        // Protection Kind that specifies origin authentication the key
        // material to a remote participant carries a receiver-specific key.
        mat.origin.enabled = properties.iter().any(|(name, value)| {
            *name == zerodds_security::crypto::PARTICIPANT_RTPS_ORIGIN_AUTHENTICATION
                && *value == "true"
        });
        self.insert(mat)
    }

    fn register_matched_remote_participant(
        &mut self,
        _local: CryptoHandle,
        _remote_identity: IdentityHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<CryptoHandle> {
        // Dual-key (FU2 approach A): the data slot is ALWAYS a
        // placeholder, only filled via `set_remote_participant_crypto_tokens`
        // with the real remote data key (token exchange,
        // spec §9.5.3.5). There is NO deterministic data
        // shortcut.
        //
        // ADDITIONALLY: if a SharedSecretProvider knows the secret,
        // we derive a separate **Kx key** from the handshake secret
        // (domain-separated) and store it under the same handle in
        // `kx_slots`. This protects the VolatileSecure channel (over which the
        // data tokens are exchanged confidentially) before
        // the data key is established.
        let handle = CryptoHandle(self.next_id());
        let data_placeholder = self.new_local_key(self.local_suite)?;
        self.slots
            .write()
            .map_err(|_| poisoned())?
            .insert(handle, data_placeholder);
        if let Some(provider) = &self.secret_provider {
            if let Some(secret) = provider.get_shared_secret(shared_secret) {
                // Cross-vendor: if the provider knows the handshake challenges,
                // derive the Kx key per DDS-Security §9.5.3.5 (cyclone-
                // identical). Otherwise (PSK/mock without challenges) fall back to the
                // proprietary HKDF — stays ZeroDDS<->ZeroDDS consistent.
                let kx = match provider.get_shared_secret_challenges(shared_secret) {
                    Some((c1, c2)) => KeyMaterial::from_kx_shared_secret(&secret, &c1, &c2)?,
                    None => KeyMaterial::from_shared_secret_kx(self.local_suite, &secret)?,
                };
                self.kx_slots
                    .write()
                    .map_err(|_| poisoned())?
                    .insert(handle, kx);
            }
        }
        Ok(handle)
    }

    fn unregister_local_endpoint(&mut self, handle: CryptoHandle) -> SecurityResult<()> {
        let mut released = Vec::new();
        if let Some(mat) = self.slots.write().map_err(|_| poisoned())?.remove(&handle) {
            released.push(mat.transformation_key_id);
        }
        if let Some(mat) = self
            .payload_slots
            .write()
            .map_err(|_| poisoned())?
            .remove(&handle)
        {
            released.push(mat.transformation_key_id);
        }
        self.submessage_keys
            .write()
            .map_err(|_| poisoned())?
            .remove(&handle);
        let mut ids = self.local_key_ids.lock().map_err(|_| poisoned())?;
        for id in released {
            ids.remove(&id);
        }
        Ok(())
    }

    fn unregister_matched_remote_participant(
        &mut self,
        handle: CryptoHandle,
    ) -> SecurityResult<()> {
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        slots.remove(&handle);
        self.kx_slots
            .write()
            .map_err(|_| poisoned())?
            .remove(&handle);
        let mut by_id = self.remote_by_key_id.write().map_err(|_| poisoned())?;
        let owned: Vec<_> = by_id
            .keys()
            .filter(|(owner, _, _)| *owner == handle)
            .copied()
            .collect();
        for key in owned {
            if let Some(slot) = by_id.remove(&key) {
                slots.remove(&slot);
            }
        }
        self.remote_map
            .lock()
            .map_err(|_| poisoned())?
            .retain(|_, slot| *slot != handle);
        Ok(())
    }

    fn register_local_endpoint(
        &mut self,
        _participant: CryptoHandle,
        is_writer: bool,
        properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        // Submessage key suite = metadata_protection-kind (reader ACKNACK +
        // writer DATA submessage); payload key suite = data_protection-kind.
        // A writer whose submessages and payload are both protected has two
        // keys, as Cyclone DDS has (`crypto_key_factory.c`,
        // `writer_key_material_message` / `_payload`): the submessage key in
        // `slots`, the payload key in `payload_slots`, the token carries two
        // keymats. A relay-only reader then gets the submessage key alone
        // (DDS-Security 1.2 §9.5.1.9.5: "only the cryptographic material
        // needed to verify and decode the RTPS SubMessage but not the
        // CryptoContent"); with one key it would read the payload. Only
        // writers have a payload layer; readers need only the submessage key.
        //
        // The kinds are those of the endpoint (§10.5.3.1 Table 78: "the
        // settings of the Metadata Protection Kind ... and Data Protection
        // Kind" of its topic), when the caller hands them in; without them
        // the participant-wide suites of the plugin.
        let kind = |name: &str| {
            properties
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        // §10.5.1, §10.5.1.1: `symmetric_cipher_algorithm` names the key
        // width of the endpoint; "AUTO", or none, leaves the default of the
        // DomainParticipant, whose width is that of the plugin (AES256 unless
        // configured otherwise). GMAC or GCM follows the Protection Kind.
        let bits = match kind(SYMMETRIC_CIPHER_ALGORITHM) {
            None | Some("AUTO") => (self.local_suite.key_len() * 8) as u16,
            value => crate::suite::symmetric_cipher_key_bits(SYMMETRIC_CIPHER_ALGORITHM, value)?,
        };
        let suite_of = |kind: &str| match kind {
            "SIGN" | "SIGN_WITH_ORIGIN_AUTHENTICATION" => {
                Some(Suite::Aes256Gmac.with_key_bits(bits))
            }
            "ENCRYPT" | "ENCRYPT_WITH_ORIGIN_AUTHENTICATION" => {
                Some(Suite::Aes256Gcm.with_key_bits(bits))
            }
            _ => None,
        };
        let (msg_suite, data_suite, both_protected, submessage_only) = match (
            kind(zerodds_security::crypto::ENDPOINT_METADATA_PROTECTION_KIND),
            kind(zerodds_security::crypto::ENDPOINT_DATA_PROTECTION_KIND),
        ) {
            (Some(metadata), Some(data)) => {
                let meta = suite_of(metadata);
                let payload = suite_of(data);
                let data_suite = payload.or(meta).unwrap_or(self.local_suite);
                (
                    meta.unwrap_or(data_suite),
                    data_suite,
                    meta.is_some() && payload.is_some(),
                    // The slot key protects the submessages and nothing
                    // else: the payload has its own key or none.
                    meta.is_some(),
                )
            }
            _ => {
                let data = self.endpoint_suite.unwrap_or(self.local_suite);
                (self.metadata_suite.unwrap_or(data), data, false, false)
            }
        };
        let mut msg = self.new_local_key(msg_suite)?;
        // §10.5.3.1 register_matched_remote_datareader / _datawriter: with
        // a Metadata Protection Kind that "specified the use of origin
        // authentication" the key material to a remote endpoint carries a
        // receiver-specific key, and the submessages a MAC for it.
        msg.origin.enabled = kind(zerodds_security::crypto::ENDPOINT_METADATA_PROTECTION_KIND)
            .is_some_and(|metadata| metadata.ends_with("_WITH_ORIGIN_AUTHENTICATION"));
        let handle = self.insert(msg)?;
        let separate_payload = is_writer && (both_protected || msg_suite != data_suite);
        if separate_payload {
            let pay = self.new_local_key(data_suite)?;
            self.payload_slots
                .write()
                .map_err(|_| poisoned())?
                .insert(handle, pay);
        }
        if is_writer && (submessage_only || separate_payload) {
            self.submessage_keys
                .write()
                .map_err(|_| poisoned())?
                .insert(handle);
        }
        Ok(handle)
    }

    fn create_endpoint_crypto_token_for(
        &mut self,
        endpoint: CryptoHandle,
        remote_endpoint: [u8; 16],
    ) -> SecurityResult<Vec<u8>> {
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&endpoint).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown local handle",
            )
        })?;
        if !mat.origin.enabled {
            return Ok(mat.serialize_keymat_cyclone());
        }
        let mut taken = self.local_key_ids.lock().map_err(|_| poisoned())?;
        let key = mat.receiver_key_for(ReceiverId::Guid(remote_endpoint), &self.rng, &mut taken)?;
        Ok(mat.serialize_keymat_for(Some(&key)))
    }

    fn endpoint_relay_token(&self, handle: CryptoHandle) -> Option<Vec<u8>> {
        if !self.submessage_keys.read().ok()?.contains(&handle) {
            return None;
        }
        self.slots
            .read()
            .ok()?
            .get(&handle)
            .map(KeyMaterial::serialize_keymat_cyclone)
    }

    fn endpoint_payload_token(&self, handle: CryptoHandle) -> Option<Vec<u8>> {
        self.payload_slots
            .read()
            .ok()?
            .get(&handle)
            .map(KeyMaterial::serialize_keymat_cyclone)
    }

    fn create_local_participant_crypto_tokens(
        &mut self,
        local: CryptoHandle,
        remote: CryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        // Serializes the local master key. The token is exchanged over the
        // built-in DCPSParticipantVolatileMessageSecure topic,
        // which is itself already encrypted with session keys from the SharedSecret
        // handshake (spec §9.5.3.5) — an additional
        // wrap layer over the token would be a double encrypt.
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown local handle",
            )
        })?;
        if mat.origin.enabled && remote != CryptoHandle(0) {
            // The participant key of an RTPS Protection Kind with origin
            // authentication: a receiver-specific key for this one remote
            // participant (`Participant2ParticipantKeyMaterial`).
            let mut taken = self.local_key_ids.lock().map_err(|_| poisoned())?;
            let key = mat.receiver_key_for(ReceiverId::Handle(remote.0), &self.rng, &mut taken)?;
            return Ok(mat.serialize_keymat_for(Some(&key)));
        }
        Ok(mat.serialize_keymat_cyclone())
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        // Per-key index ((remote, key_id) → own slot handle): multiple keys of
        // a peer (secure built-in endpoints + user endpoints) coexist, so that
        // decode_data_by_key_id finds the right one.
        self.index_remote_key(remote, KeyMaterial::from_keymat_cyclone(tokens)?)?;
        if remote == CryptoHandle(0) {
            // The owner-less endpoint form of earlier callers: no
            // participant slot to fill.
            return Ok(());
        }
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        // Per-participant handle (backward compat for the message-level SRTPS
        // decode that uses slots[remote]): FIRST-WINS. The participant crypto
        // token comes first (install order) and is the key that
        // decode_secured_rtps_message needs. Later-arriving datawriter/
        // datareader tokens use the key_id path (remote_by_key_id) and must NOT
        // overwrite slots[remote] — otherwise SRTPS decode tag mismatch
        // (cross-instance; same-handle tests never hit the collision).
        slots.insert(remote, KeyMaterial::from_keymat_cyclone(tokens)?);
        Ok(())
    }

    fn set_remote_endpoint_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        // Only the (remote, key_id) index: an endpoint key must not take the
        // place of the participant key in slots[remote].
        self.index_remote_key(remote, KeyMaterial::from_keymat_cyclone(tokens)?)
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
        seal_with(mat, plaintext, aad_extension)
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
        open_with(mat, ciphertext, aad_extension)
    }

    fn encode_kx_submessage(
        &self,
        handle: CryptoHandle,
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let kx = self.kx_slots.read().map_err(|_| poisoned())?;
        let mat = kx.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no kx-key for handle (provider unknown / no handshake secret)",
            )
        })?;
        seal_with(mat, plaintext, aad_extension)
    }

    fn decode_kx_submessage(
        &self,
        handle: CryptoHandle,
        ciphertext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let kx = self.kx_slots.read().map_err(|_| poisoned())?;
        let mat = kx.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no kx-key for handle (provider unknown / no handshake secret)",
            )
        })?;
        open_with(mat, ciphertext, aad_extension)
    }

    fn encode_kx_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        plaintext: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let kx = self.kx_slots.read().map_err(|_| poisoned())?;
        let mat = kx.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no kx-key for handle (provider unknown / no handshake secret)",
            )
        })?;
        let (header, ct, tag) = mat.seal_cyclone(plaintext)?;
        Ok(assemble_secure_submessages(&header, &ct, &tag))
    }

    fn decode_kx_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let kx = self.kx_slots.read().map_err(|_| poisoned())?;
        let mat = kx.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no kx-key for handle (provider unknown / no handshake secret)",
            )
        })?;
        let (header, ct, footer) = parse_secure_submessages_footer(wire)?;
        mat.open_submessage(&header, &ct, &footer)
    }

    fn encode_data_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        plaintext: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown data handle",
            )
        })?;
        let (header, ct, tag) = mat.seal_cyclone(plaintext)?;
        let tail = mat.footer_tail(&header, &tag)?;
        Ok(assemble_secure_submessages_with(&header, &ct, &tag, &tail))
    }

    fn decode_data_datawriter_submessage(
        &self,
        handle: CryptoHandle,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown data handle",
            )
        })?;
        let (header, ct, footer) = parse_secure_submessages_footer(wire)?;
        mat.open_submessage(&header, &ct, &footer)
    }

    fn decode_data_by_key_id(&self, wire: &[u8]) -> SecurityResult<Vec<u8>> {
        self.decode_data_by_key_id_of(None, wire)
    }

    fn decode_data_by_key_id_from(
        &self,
        remote: CryptoHandle,
        wire: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.decode_data_by_key_id_of(Some(remote), wire)
    }

    fn encode_rtps_message_cyclone(
        &self,
        local: CryptoHandle,
        message: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        if message.len() < 20 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: rtps message too short",
            ));
        }
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown local handle",
            )
        })?;
        mat.encode_rtps_info_src(message, None)
    }

    fn decode_rtps_message_cyclone(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        self.decode_rtps_message_of(None, message, None)
    }

    fn decode_rtps_message_cyclone_from(
        &self,
        remote: CryptoHandle,
        message: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.decode_rtps_message_of(Some(remote), message, None)
    }

    /// §8.5.1.9.4 / §9.5.3.3.1 `decode_serialized_payload`. Counterpart to
    /// [`Self::encode_serialized_payload`]; key via the `transformation_key_id`
    /// in the CryptoHeader (index `remote_by_key_id`).
    fn decode_serialized_payload(&self, encoded: &[u8]) -> SecurityResult<Vec<u8>> {
        self.decode_serialized_payload_of(None, encoded)
    }

    fn decode_serialized_payload_by_key_id_from(
        &self,
        remote: CryptoHandle,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        self.decode_serialized_payload_of(Some(remote), encoded)
    }

    /// §8.5.1.9.1 / §9.5.3.3.1 `encode_serialized_payload` (ENCRYPT). Protects
    /// the SerializedPayload of ONE endpoint (per-endpoint writer key, `handle`)
    /// as an INNER layer — without a SEC_PREFIX/POSTFIX submessage frame. Cyclone
    /// expects exactly this format for `data_protection_kind=ENCRYPT`:
    /// `CryptoHeader(20) | content.length(4 BE) | ciphertext | common_mac(16) |
    /// receiver_specific_macs.length(4 BE)=0`.
    fn encode_serialized_payload(
        &self,
        handle: CryptoHandle,
        payload: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        // Dual-key (metadata != data, meta-sign-data): the SerializedPayload uses
        // the separate payload key (data-kind) from `payload_slots`, NOT the
        // submessage key (metadata-kind) from `slots`. Otherwise (metadata == data,
        // all other profiles) there is only the `slots` key.
        {
            let pslots = self.payload_slots.read().map_err(|_| poisoned())?;
            if let Some(mat) = pslots.get(&handle) {
                return encode_payload_from_mat(mat, payload);
            }
        }
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown handle for encode_serialized_payload",
            )
        })?;
        encode_payload_from_mat(mat, payload)
    }

    fn decode_serialized_payload_with(
        &self,
        handle: CryptoHandle,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        if encoded.len() < 20 + 16 + 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: encoded serialized_payload too short",
            ));
        }
        let header = &encoded[..20];
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: serialized_payload handle slot disappeared",
            )
        })?;
        // GCM: header + content.length(4) + ciphertext + footer(20). GMAC: header
        // + cleartext + footer(20) (no content.length; cyclone-conformant). footer
        // = common_mac(16) + receiver_specific_macs.length(4)=0.
        let (ct, tag): (&[u8], &[u8]) = if mat.suite.is_aead() {
            if encoded.len() < 24 + 16 + 4 {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: encoded serialized_payload too short",
                ));
            }
            let ct_len =
                u32::from_be_bytes([encoded[20], encoded[21], encoded[22], encoded[23]]) as usize;
            if encoded.len() < 24 + ct_len + 16 + 4 {
                return Err(SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: serialized_payload content.length > buffer",
                ));
            }
            check_no_receiver_macs(&encoded[24 + ct_len + 16..24 + ct_len + 20])?;
            (
                &encoded[24..24 + ct_len],
                &encoded[24 + ct_len..24 + ct_len + 16],
            )
        } else {
            let n = encoded.len();
            check_no_receiver_macs(&encoded[n - 4..])?;
            (&encoded[20..n - 20], &encoded[n - 20..n - 4])
        };
        mat.open_cyclone(header, ct, tag)
    }

    fn decode_serialized_payload_kx(
        &self,
        handle: CryptoHandle,
        encoded: &[u8],
    ) -> SecurityResult<Vec<u8>> {
        if encoded.len() < 20 + 4 + 16 + 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: encoded serialized_payload too short",
            ));
        }
        let header = &encoded[..20];
        let ct_len =
            u32::from_be_bytes([encoded[20], encoded[21], encoded[22], encoded[23]]) as usize;
        if encoded.len() < 24 + ct_len + 16 + 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: serialized_payload content.length > buffer",
            ));
        }
        check_no_receiver_macs(&encoded[24 + ct_len + 16..24 + ct_len + 20])?;
        let ct = &encoded[24..24 + ct_len];
        let tag = &encoded[24 + ct_len..24 + ct_len + 16];
        let kx = self.kx_slots.read().map_err(|_| poisoned())?;
        let mat = kx.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: serialized_payload Kx handle slot disappeared",
            )
        })?;
        mat.open_cyclone(header, ct, tag)
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
            "crypto: key_from_bytes called for a non-AEAD suite",
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

    /// Offsets at which each submessage of `wire` starts, walking the
    /// octetsToNextHeader of each (all little-endian here).
    fn submessage_starts(wire: &[u8]) -> Vec<usize> {
        let mut starts = Vec::new();
        let mut at = 0;
        while at + 4 <= wire.len() {
            starts.push(at);
            at += 4 + usize::from(u16::from_le_bytes([wire[at + 2], wire[at + 3]]));
        }
        assert_eq!(at, wire.len(), "the lengths do not add up to the message");
        starts
    }

    /// A ciphertext of any length leaves every following submessage on a
    /// 32-bit boundary (DDSI-RTPS 2.5 §9.4.1). Before, a 91-byte ciphertext
    /// put SEC_POSTFIX at offset 219 of a user DATA message and Cyclone
    /// refused it as "invalid message"; the padding is not part of the
    /// ciphertext the reader takes back.
    #[test]
    fn sec_body_is_padded_to_four_for_any_ciphertext_length() {
        let header = [0, 0, 0, 4, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let tag = [0xAB; 16];
        for len in 1..=8 {
            let ct: Vec<u8> = (0..len).map(|i| i as u8 + 1).collect();
            let wire = assemble_secure_submessages(&header, &ct, &tag);
            for at in submessage_starts(&wire) {
                assert_eq!(at % 4, 0, "ct {len}: a submessage starts at {at}");
            }
            let (_, ct2, tag2) = parse_secure_submessages(&wire).expect("parse");
            assert_eq!(ct2, ct);
            assert_eq!(tag2, tag);

            let rtps_header = [
                0x52, 0x54, 0x50, 0x53, 2, 5, 1, 0xf0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
            ];
            let msg = assemble_srtps_message(&rtps_header, &header, &ct, &tag);
            for at in submessage_starts(&msg[20..]) {
                assert_eq!(at % 4, 0, "srtps ct {len}: a submessage starts at {at}");
            }
            let (_, _, ct3, tag3) = parse_srtps_message(&msg).expect("parse srtps");
            assert_eq!(ct3, ct);
            assert_eq!(tag3, tag);
        }
    }

    #[test]
    fn cyclone_volatile_submessage_roundtrips() {
        // VolatileSecure-Kx-Key (cyclone calculate_kx_keys) + cyclone-Wire-Format
        // seal -> assemble(SEC_PREFIX/BODY/POSTFIX) -> parse -> open.
        let mat = KeyMaterial::from_kx_shared_secret(
            b"a-32-byte-handshake-shared-secret",
            &[0x11; 32],
            &[0x22; 32],
        )
        .expect("kx key");
        let pt = b"ParticipantGenericMessage crypto-token payload bytes";
        let (header, ct, tag) = mat.seal_cyclone(pt).expect("seal");
        // CryptoHeader: 20 byte, transform_kind = AES256_GCM = {0,0,0,4},
        // key_id = 0, session_id = 0.
        assert_eq!(header.len(), 20);
        assert_eq!(&header[0..4], &[0, 0, 0, 4]);
        assert_eq!(&header[4..8], &[0, 0, 0, 0]);
        assert_eq!(&header[8..12], &[0, 0, 0, 0]);
        assert_eq!(tag.len(), 16);
        // ciphertext length == plaintext length (GCM without padding).
        assert_eq!(ct.len(), pt.len());

        let wire = assemble_secure_submessages(&header, &ct, &tag);
        // SEC_PREFIX(0x31) am Anfang.
        assert_eq!(wire[0], SEC_PREFIX_ID);
        let (h2, ct2, tag2) = parse_secure_submessages(&wire).expect("parse");
        assert_eq!(h2, header);
        assert_eq!(ct2, ct);
        assert_eq!(tag2, tag);

        let dec = mat.open_cyclone(&h2, &ct2, &tag2).expect("open");
        assert_eq!(dec, pt);

        // Tampering in the ciphertext -> tag mismatch.
        let mut bad = ct.clone();
        bad[0] ^= 0xFF;
        assert!(mat.open_cyclone(&header, &bad, &tag).is_err());
    }

    #[test]
    fn cyclone_kx_key_matches_both_directions() {
        // Both peers derive the same Kx key (symmetric) -> A seals, B opens.
        let secret = b"shared-secret-from-ecdh-handshake";
        let (c1, c2) = ([0xAAu8; 32], [0xBBu8; 32]);
        let a = KeyMaterial::from_kx_shared_secret(secret, &c1, &c2).expect("kx key");
        let b = KeyMaterial::from_kx_shared_secret(secret, &c1, &c2).expect("kx key");
        let (h, ct, tag) = a
            .seal_cyclone(b"cross-peer volatile payload")
            .expect("seal");
        let dec = b.open_cyclone(&h, &ct, &tag).expect("open on peer b");
        assert_eq!(dec, b"cross-peer volatile payload");
    }

    #[test]
    fn remote_token_install_does_not_clobber_local_endpoint_handle() {
        // Regression: `insert()` (local endpoints) assigns handles via
        // next_id()=fetch_add+1, `set_remote_participant_crypto_tokens` drew
        // its slot handle via fetch_add WITHOUT +1 — both on the same counter.
        // Off-by-one → a remote-token install collided with the last-assigned
        // local endpoint handle and overwrote its
        // KeyMaterial. Cross-vendor: cyclone could no longer decode ZeroDDS'
        // ff0004c7 ACKNACK (wrong transformation_key_id on the wire).
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let ep = p.register_local_endpoint(local, true, &[]).unwrap();
        let kid_before = {
            let s = p.encode_data_datawriter_submessage(ep, b"x").unwrap();
            [s[8], s[9], s[10], s[11]]
        };
        // Install a foreign token (different key_id) — forces a
        // slot allocation that must NOT fall on `ep`.
        let remote_token = {
            let mut other = AesGcmCryptoPlugin::new();
            let ol = other
                .register_local_participant(IdentityHandle(9), &[])
                .unwrap();
            let oep = other.register_local_endpoint(ol, true, &[]).unwrap();
            other
                .create_local_participant_crypto_tokens(oep, CryptoHandle(0))
                .unwrap()
        };
        p.set_remote_participant_crypto_tokens(local, CryptoHandle(0), &remote_token)
            .unwrap();
        let kid_after = {
            let s = p.encode_data_datawriter_submessage(ep, b"x").unwrap();
            [s[8], s[9], s[10], s[11]]
        };
        assert_eq!(
            kid_before, kid_after,
            "Remote-Install hat lokalen Endpoint-Slot ueberschrieben (Handle-Kollision)"
        );
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
        // Flip a byte in the ciphertext range.
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
        // The nonce counter differs → the wire bytes differ.
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn cross_plugin_interop_via_tokens() {
        // Alice encrypts, sends a token to Bob, Bob decodes.
        let mut alice = AesGcmCryptoPlugin::new();
        let mut bob = AesGcmCryptoPlugin::new();

        let alice_local = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bob_local = bob
            .register_local_participant(IdentityHandle(2), &[])
            .unwrap();

        // Alice serializes her master key as a token.
        let token = alice
            .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
            .unwrap();

        // Bob accepts the token and stores it under a new handle.
        let alice_seen_by_bob = bob
            .register_matched_remote_participant(
                bob_local,
                IdentityHandle(1),
                SharedSecretHandle(1),
            )
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bob_local, alice_seen_by_bob, &token)
            .unwrap();

        // Alice encrypts a payload.
        let plain = b"cross-plugin-test";
        let ct = alice
            .encrypt_submessage(alice_local, &[], plain, &[])
            .unwrap();

        // Bob decrypts with the transferred key.
        let back = bob
            .decrypt_submessage(alice_seen_by_bob, CryptoHandle(0), &ct, &[])
            .unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn serialized_payload_cross_plugin_via_tokens() {
        // §9.5.3.3.1 data_protection: Alice encode_serialized_payload (innere
        // payload layer), token to Bob, Bob decode_serialized_payload (key_id).
        let mut alice = AesGcmCryptoPlugin::new();
        let mut bob = AesGcmCryptoPlugin::new();
        let alice_local = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bob_local = bob
            .register_local_participant(IdentityHandle(2), &[])
            .unwrap();
        let token = alice
            .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
            .unwrap();
        let alice_seen_by_bob = bob
            .register_matched_remote_participant(
                bob_local,
                IdentityHandle(1),
                SharedSecretHandle(1),
            )
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bob_local, alice_seen_by_bob, &token)
            .unwrap();

        // SerializedPayload = Encap-Header (XCDR2 LE) + CDR-Daten.
        let plain = b"\x00\x07\x00\x00serialized-payload-data-protection";
        let enc = alice.encode_serialized_payload(alice_local, plain).unwrap();
        // §9.5.3.3.1: CryptoHeader(20) + content.length(4) + ct + common_mac(16)
        // + receiver_specific_macs.length(4).
        assert_eq!(enc.len(), 20 + 4 + plain.len() + 16 + 4);
        let back = bob.decode_serialized_payload(&enc).unwrap();
        assert_eq!(back, plain);
    }

    /// Table 80, `encode_serialized_payload`: `receiver_specific_macs` of the
    /// CryptoFooter is empty, and the format has no length (§10.5.3.3.4.4),
    /// so a payload that announces MACs cannot be placed. Its count (the last
    /// four octets) was not read.
    #[test]
    fn a_payload_footer_with_receiver_macs_is_refused() {
        for suite in [Suite::Aes128Gcm, Suite::Aes256Gmac] {
            let mut alice = AesGcmCryptoPlugin::with_suite(suite);
            let mut bob = AesGcmCryptoPlugin::with_suite(suite);
            let alice_local = alice
                .register_local_participant(IdentityHandle(1), &[])
                .unwrap();
            let bob_local = bob
                .register_local_participant(IdentityHandle(2), &[])
                .unwrap();
            let token = alice
                .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
                .unwrap();
            let seen = bob
                .register_matched_remote_participant(
                    bob_local,
                    IdentityHandle(1),
                    SharedSecretHandle(1),
                )
                .unwrap();
            bob.set_remote_participant_crypto_tokens(bob_local, seen, &token)
                .unwrap();
            let plain = b"\x00\x07\x00\x00a payload with a footer";
            let enc = alice.encode_serialized_payload(alice_local, plain).unwrap();
            assert_eq!(bob.decode_serialized_payload(&enc).unwrap(), plain);
            let n = enc.len();
            assert_eq!(&enc[n - 4..], &[0; 4], "an empty receiver_specific_macs");
            let mut with_macs = enc.clone();
            with_macs[n - 1] = 1;
            assert!(
                bob.decode_serialized_payload(&with_macs).is_err(),
                "{suite:?}"
            );
        }
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
    fn default_plugin_uses_aes256() {
        // §10.5.1: a missing symmetric_cipher_algorithm is AUTO, and AUTO is
        // AES256-GCM.
        let p = AesGcmCryptoPlugin::new();
        assert_eq!(p.local_suite(), Suite::Aes256Gcm);
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
        // Alice = 256, Bob = 128. Alice serializes her 256-bit key
        // via a token. Bob receives the token — it contains
        // the spec-conformant suite tag 0x04 (AES256_GCM, §10.5 Tab.79),
        // so Bob uses AES-256 for THIS slot, even though his
        // local_suite is 128.
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
        // CDR format §9.5.2.1.1: transformation_kind = octet[4] = {0,0,0,id};
        // the suite byte is at position 3 (BE).
        assert_eq!(
            token[3],
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
        // Token with suite-id 0xFF (invalid).
        let bogus = [
            0xFFu8, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let err = p
            .set_remote_participant_crypto_tokens(local, remote, &bogus)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    // -------------------------------------------------------------
    //  — AES-GMAC auth-only + Key-Refresh
    // -------------------------------------------------------------

    /// AES128-GMAC (DDS-Security 1.2 §10.5.2 Table 77, kind 0x01) keeps the
    /// payload plain and appends a 16-octet tag; the slot carried
    /// HMAC-SHA256 with a 32-octet one.
    #[test]
    fn gmac_only_suite_roundtrip_without_encryption() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::Aes128Gmac);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let plain = b"payload kept in plaintext, only signed";
        let signed = p.encrypt_submessage(local, &[], plain, &[]).unwrap();
        // Wire format: nonce(12) + plaintext + tag(16). The plaintext
        // must be present **unencrypted**.
        assert!(
            signed.windows(plain.len()).any(|w| w == plain),
            "a GMAC suite does not encrypt the plaintext"
        );
        assert_eq!(signed.len(), 12 + plain.len() + 16);
        let back = p
            .decrypt_submessage(local, CryptoHandle(0), &signed, &[])
            .unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn gmac_tampered_payload_fails_verify() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::Aes128Gmac);
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let mut signed = p
            .encrypt_submessage(local, &[], b"original message", &[])
            .unwrap();
        // Flip byte in plaintext (after nonce, before tag).
        signed[15] ^= 0x01;
        let err = p
            .decrypt_submessage(local, CryptoHandle(0), &signed, &[])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::CryptoFailed);
    }

    #[test]
    fn gmac_tampered_tag_fails_verify() {
        let mut p = AesGcmCryptoPlugin::with_suite(Suite::Aes128Gmac);
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
        // Encrypt + pull a token; then rotate; then pull a new token
        // — both tokens must differ.
        let _ = p.encrypt_submessage(local, &[], b"x", &[]).unwrap();
        let token_before = p
            .create_local_participant_crypto_tokens(local, CryptoHandle(0))
            .unwrap();

        p.rotate_key(local).unwrap();
        assert_eq!(
            p.encrypts_remaining(local).unwrap(),
            Suite::Aes128Gcm.max_encrypts(),
            "counter must start at 0 after rotate"
        );

        let token_after = p
            .create_local_participant_crypto_tokens(local, CryptoHandle(0))
            .unwrap();
        assert_ne!(token_before, token_after, "master key must be new");
    }

    /// §10.3 / §9.5.2.1.2 Tab.73 — the KeyMaterial is the triple
    /// (`master_sender_key`, `master_salt`, `sender_key_id`), and the
    /// receiver picks the key by the `sender_key_id` in the CryptoHeader
    /// (bytes 4..8, BE). A key change must therefore carry a new key id
    /// all the way onto the wire, and the receiver must keep the two
    /// generations apart instead of silently mixing them.
    #[test]
    fn key_rotation_changes_the_wire_key_id_and_the_derived_session_key() {
        let mut alice = AesGcmCryptoPlugin::new();
        let mut bob = AesGcmCryptoPlugin::new();
        let alice_local = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bob_local = bob
            .register_local_participant(IdentityHandle(2), &[])
            .unwrap();
        let alice_seen_by_bob = bob
            .register_matched_remote_participant(
                bob_local,
                IdentityHandle(1),
                SharedSecretHandle(1),
            )
            .unwrap();

        let plain = b"\x00\x07\x00\x00key-material-generation-one";

        // Generation 1: Bob installs the token, decodes by key id.
        let token_gen1 = alice
            .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bob_local, alice_seen_by_bob, &token_gen1)
            .unwrap();
        let wire_gen1 = alice.encode_serialized_payload(alice_local, plain).unwrap();
        let key_id_gen1 = &wire_gen1[4..8];
        assert_eq!(bob.decode_serialized_payload(&wire_gen1).unwrap(), plain);

        // Generation 2: new master key, new salt, new key id.
        alice.rotate_key(alice_local).unwrap();
        let wire_gen2 = alice.encode_serialized_payload(alice_local, plain).unwrap();
        let key_id_gen2 = &wire_gen2[4..8];
        assert_ne!(
            key_id_gen1, key_id_gen2,
            "rotation must publish a new sender_key_id, not reuse the old one"
        );
        assert_ne!(
            &wire_gen1[20..],
            &wire_gen2[20..],
            "a new master key must produce a different ciphertext for the \
             same plaintext"
        );

        // Bob has not been told about generation 2 — the lookup by key id
        // must fail loudly instead of trying the stale key.
        let err = bob
            .decode_serialized_payload(&wire_gen2)
            .expect_err("unknown key id must not decode");
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);

        // After the key exchange both generations resolve, each to its own
        // key material.
        let token_gen2 = alice
            .create_local_participant_crypto_tokens(alice_local, CryptoHandle(0))
            .unwrap();
        assert_ne!(token_gen1, token_gen2, "token carries the new key material");
        bob.set_remote_participant_crypto_tokens(bob_local, alice_seen_by_bob, &token_gen2)
            .unwrap();
        assert_eq!(bob.decode_serialized_payload(&wire_gen2).unwrap(), plain);
        assert_eq!(
            bob.decode_serialized_payload(&wire_gen1).unwrap(),
            plain,
            "the old key id still resolves to the old key material",
        );
    }

    /// The session key is derived from the master key and the master
    /// salt (§10.5.2 Tab.74); rotating either one has to move the
    /// derived key. Guards the derivation inputs the rotation touches.
    #[test]
    fn session_key_derivation_consumes_every_rotated_key_material_field() {
        let base_key = [0x11u8; 16];
        let base_salt = [0x22u8; 32];
        let sid = [0u8, 0, 0, 7];
        let base = crate::session_key::derive_session_key_cyclone(&base_key, &base_salt, &sid, 16);

        let other_key = [0x12u8; 16];
        let other_salt = [0x23u8; 32];
        assert_ne!(
            base,
            crate::session_key::derive_session_key_cyclone(&other_key, &base_salt, &sid, 16),
            "master_sender_key feeds the derivation",
        );
        assert_ne!(
            base,
            crate::session_key::derive_session_key_cyclone(&base_key, &other_salt, &sid, 16),
            "master_salt feeds the derivation",
        );
        assert_ne!(
            base,
            crate::session_key::derive_session_key_cyclone(
                &base_key,
                &base_salt,
                &[0u8, 0, 0, 8],
                16
            ),
            "session_id feeds the derivation",
        );
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

    /// Minimal in-memory provider for tests; in production
    /// `PkiAuthenticationPlugin` is used (implements
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
    fn with_secret_provider_derives_same_kx_key_for_both_sides() {
        // Alice & Bob know the same 32-byte DH shared secret. Both
        // register via register_matched_remote_participant — both
        // HKDF-derive the same **Kx key** (KeyExchange channel) and
        // can therefore exchange VolatileSecure payloads (crypto tokens) via
        // encode_kx/decode_kx, WITHOUT knowing data keys
        // beforehand. (Data keys come exclusively via token exchange
        // — there is no deterministic data shortcut.)
        let (mut alice, mut bob) = dual_key_pair(0xA5);
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

        // Kx channel: Alice encrypts, Bob decrypts (same Kx key).
        let plain = b"x25519-handshake-derived-kx-key";
        let wire = alice
            .encode_kx_submessage(alice_to_bob, plain, &[])
            .unwrap();
        let back = bob.decode_kx_submessage(bob_to_alice, &wire, &[]).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn with_secret_provider_different_secrets_yield_distinct_kx_keys() {
        // Two different DH shared secrets (Alice↔Bob vs
        // Alice↔Charlie) must lead to two different Kx keys
        // — otherwise it would be a fatal key reuse. Proof: a
        // Kx ciphertext created with secret-1 (bob slot) must NOT be
        // decryptable with secret-2 (charlie slot).
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
        let wire = p
            .encode_kx_submessage(bob, b"secret-1-payload", &[])
            .unwrap();
        assert!(
            p.decode_kx_submessage(charlie, &wire, &[]).is_err(),
            "different DH secrets must yield different Kx keys (no key reuse)"
        );
    }

    #[test]
    fn with_secret_provider_unknown_handle_falls_back_to_random() {
        // If the provider does not know the handle, the plugin takes
        // a random key (v1.4 path). This lets DH-MAC
        // keys and token-exchange cipher keys coexist in the same plugin.
        let provider = ArcA::new(MemProvider::new()); // empty
        let mut p = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            ArcA::clone(&provider) as ArcA<dyn SharedSecretProvider>,
        );
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let h = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(42))
            .expect("unknown handle → random slot, no error");
        // The slot exists and is usable (random key).
        let _tok = p
            .create_local_participant_crypto_tokens(h, CryptoHandle(0))
            .unwrap();
    }

    #[test]
    fn without_provider_backward_compat_random_key_preserved() {
        // No provider → v1.4 path: a random key on every register.
        let mut p = AesGcmCryptoPlugin::new(); // without provider
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
        assert_ne!(tok_a, tok_b, "random keys → two different tokens");
    }

    #[test]
    fn hkdf_derivation_is_deterministic() {
        // Same secret → same master key, bit-identical.
        let secret = alloc::vec![0xCDu8; 32];
        let m1 = KeyMaterial::from_shared_secret_kx(Suite::Aes128Gcm, &secret).unwrap();
        let m2 = KeyMaterial::from_shared_secret_kx(Suite::Aes128Gcm, &secret).unwrap();
        assert_eq!(m1.master_key, m2.master_key);
        assert_eq!(m1.current_session_id(), m2.current_session_id());
    }

    #[test]
    fn hkdf_rejects_empty_secret() {
        let res = KeyMaterial::from_shared_secret_kx(Suite::Aes128Gcm, &[]);
        match res {
            Err(e) => assert_eq!(e.kind, SecurityErrorKind::BadArgument),
            Ok(_) => panic!("expected BadArgument, got Ok"),
        }
    }

    // -------------------------------------------------------------
    // FU2 S1.1 — dual key: the Kx channel (VolatileSecure) is separate from
    // the data key (token). Both from the same SharedSecret, but
    // domain-separated.
    // -------------------------------------------------------------

    fn dual_key_pair(secret: u8) -> (AesGcmCryptoPlugin, AesGcmCryptoPlugin) {
        let shared = alloc::vec![secret; 32];
        let pa = ArcA::new(MemProvider::new());
        let pb = ArcA::new(MemProvider::new());
        pa.insert(SharedSecretHandle(1), shared.clone());
        pb.insert(SharedSecretHandle(1), shared);
        (
            AesGcmCryptoPlugin::with_secret_provider(
                Suite::Aes128Gcm,
                ArcA::clone(&pa) as ArcA<dyn SharedSecretProvider>,
            ),
            AesGcmCryptoPlugin::with_secret_provider(
                Suite::Aes128Gcm,
                ArcA::clone(&pb) as ArcA<dyn SharedSecretProvider>,
            ),
        )
    }

    #[test]
    fn data_token_exchange_does_not_clobber_kx_key() {
        let (mut alice, mut bob) = dual_key_pair(0x33);
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let a2b = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        // Daten-Token-Austausch: bob installiert alice's lokalen Daten-Key.
        let alice_data_tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bl, b2a, &alice_data_tok)
            .unwrap();
        // (a) The Kx channel STILL round-trips (not overwritten by the data token).
        let kx_plain = b"kx-after-token-exchange";
        let kx_wire = alice.encode_kx_submessage(a2b, kx_plain, &[]).unwrap();
        assert_eq!(
            bob.decode_kx_submessage(b2a, &kx_wire, &[]).unwrap(),
            kx_plain
        );
        // (b) Data path: alice encrypts with the local key, bob decrypts
        //     with b2a (= alice's data key via the token).
        let d_plain = b"user-data-secured";
        let d_wire = alice.encrypt_submessage(al, &[], d_plain, &[]).unwrap();
        assert_eq!(
            bob.decrypt_submessage(b2a, b2a, &d_wire, &[]).unwrap(),
            d_plain
        );
    }

    #[test]
    fn encode_kx_without_provider_has_no_kx_slot() {
        let mut p = AesGcmCryptoPlugin::new(); // without a provider → no Kx
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let h = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let err = p.encode_kx_submessage(h, b"x", &[]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// DDS-Security 1.2 §10.5.3.1 Table 78: the key material of an endpoint
    /// follows its Metadata and Data Protection Kind when the caller hands
    /// them in: SIGN gives AES-256-GMAC, ENCRYPT AES-256-GCM, and a writer
    /// with two different kinds gets a submessage and a payload key.
    #[test]
    fn the_key_material_of_an_endpoint_follows_its_protection_kinds() {
        use zerodds_security::crypto::{
            ENDPOINT_DATA_PROTECTION_KIND as DATA, ENDPOINT_METADATA_PROTECTION_KIND as META,
        };
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let kind_of = |p: &mut AesGcmCryptoPlugin, h: CryptoHandle| {
            p.create_local_participant_crypto_tokens(h, CryptoHandle(0))
                .unwrap()[3]
        };
        let sign = p
            .register_local_endpoint(local, true, &[(META, "NONE"), (DATA, "SIGN")])
            .unwrap();
        assert_eq!(kind_of(&mut p, sign), 3, "SIGN: AES256_GMAC");
        assert!(p.endpoint_payload_token(sign).is_none(), "one key");
        let enc = p
            .register_local_endpoint(local, true, &[(META, "ENCRYPT"), (DATA, "ENCRYPT")])
            .unwrap();
        assert_eq!(kind_of(&mut p, enc), 4, "ENCRYPT: AES256_GCM");
        let enc_payload = p
            .endpoint_payload_token(enc)
            .expect("both protected: a payload key of its own");
        assert_eq!(enc_payload[3], 4);
        assert_ne!(
            &enc_payload[..],
            &p.create_local_participant_crypto_tokens(enc, CryptoHandle(0))
                .unwrap()[..],
            "two keys, not one key twice"
        );
        let dual = p
            .register_local_endpoint(local, true, &[(META, "SIGN"), (DATA, "ENCRYPT")])
            .unwrap();
        assert_eq!(kind_of(&mut p, dual), 3, "the submessage key: SIGN");
        let payload = p.endpoint_payload_token(dual).expect("a payload key");
        assert_eq!(payload[3], 4, "the payload key: ENCRYPT");
        let reader = p
            .register_local_endpoint(local, false, &[(META, "SIGN"), (DATA, "ENCRYPT")])
            .unwrap();
        assert!(
            p.endpoint_payload_token(reader).is_none(),
            "a reader has no payload"
        );
    }

    /// DDS-Security 1.2 §10.5.1, §10.5.1.1 Table 71: the key width follows
    /// `symmetric_cipher_algorithm` (GMAC or GCM follows the Protection
    /// Kind). A DataWriter's property overrides the participant's; without
    /// it, or with `AUTO`, the default of the participant applies, and the
    /// default of that is `AES256-GCM`.
    #[test]
    fn the_key_width_follows_symmetric_cipher_algorithm() {
        use zerodds_security::crypto::{
            ENDPOINT_DATA_PROTECTION_KIND as DATA, ENDPOINT_METADATA_PROTECTION_KIND as META,
        };
        let kind_and_len = |p: &mut AesGcmCryptoPlugin, cipher: Option<&str>, meta: &str| {
            let local = p
                .register_local_participant(IdentityHandle(1), &[])
                .unwrap();
            let mut props = alloc::vec![(META, meta), (DATA, "NONE")];
            if let Some(cipher) = cipher {
                props.push((SYMMETRIC_CIPHER_ALGORITHM, cipher));
            }
            let h = p.register_local_endpoint(local, true, &props).unwrap();
            let token = p
                .create_local_participant_crypto_tokens(h, CryptoHandle(0))
                .unwrap();
            let key = KeyMaterial::from_keymat_cyclone(&token).unwrap();
            (token[3], key.master_key.len())
        };
        let mut p256 = AesGcmCryptoPlugin::new();
        assert_eq!(kind_and_len(&mut p256, None, "ENCRYPT"), (4, 32), "AUTO");
        assert_eq!(kind_and_len(&mut p256, Some("AUTO"), "SIGN"), (3, 32));
        assert_eq!(
            kind_and_len(&mut p256, Some("AES128+GCM"), "ENCRYPT"),
            (2, 16)
        );
        assert_eq!(
            kind_and_len(&mut p256, Some("AES128-GMAC"), "SIGN"),
            (1, 16)
        );
        assert_eq!(
            kind_and_len(&mut p256, Some("AES256-GCM"), "ENCRYPT"),
            (4, 32)
        );
        let mut p128 = AesGcmCryptoPlugin::with_suite(Suite::Aes128Gcm);
        assert_eq!(
            kind_and_len(&mut p128, None, "ENCRYPT"),
            (2, 16),
            "participant"
        );
        assert_eq!(kind_and_len(&mut p128, Some("AUTO"), "SIGN"), (1, 16));
        assert_eq!(
            kind_and_len(&mut p128, Some("AES256-GCM"), "ENCRYPT"),
            (4, 32)
        );
        let local = p128
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        assert!(
            p128.register_local_endpoint(
                local,
                true,
                &[(SYMMETRIC_CIPHER_ALGORITHM, "ChaCha20"), (META, "SIGN")]
            )
            .is_err()
        );
    }

    /// The keymat of §10.5.2.1.1 for the tests of origin authentication:
    /// kind, salt, sender key id and key, receiver key id and key.
    fn keymat(
        kind: u8,
        salt: &[u8],
        sender_id: [u8; 4],
        key: &[u8],
        receiver: Option<([u8; 4], &[u8])>,
    ) -> Vec<u8> {
        let mut out = alloc::vec![0, 0, 0, kind];
        out.extend_from_slice(&(salt.len() as u32).to_be_bytes());
        out.extend_from_slice(salt);
        out.extend_from_slice(&sender_id);
        out.extend_from_slice(&(key.len() as u32).to_be_bytes());
        out.extend_from_slice(key);
        let (id, master) = receiver.unwrap_or(([0; 4], &[]));
        out.extend_from_slice(&id);
        out.extend_from_slice(&(master.len() as u32).to_be_bytes());
        out.extend_from_slice(master);
        out
    }

    /// DDS-Security 1.2 §10.5.3.3.3 and §10.5.3.3.4.5: the receiver-specific
    /// MAC is the AES-GMAC tag, under
    /// `SessionReceiverSpecificKey := HMAC256(MasterReceiverSpecificKey,
    /// "SessionReceiverKey" | MasterSalt | SessionId)` (cut to 128 bits for
    /// AES128) and the InitializationVector `SessionId | suffix`, computed on
    /// the `common_mac`. The expected tags were computed with Python
    /// (`hmac`, `cryptography` AESGCM) from that formula, not with this code.
    #[test]
    fn the_receiver_specific_mac_is_the_one_of_the_spec() {
        let session_id = [0, 0, 0, 5];
        let iv_suffix = [0, 0, 0, 0, 0, 0, 0, 9];
        let common_mac = [0x33u8; 16];
        // (kind, key length, expected MAC)
        for (kind, len, expected) in [
            (2u8, 16usize, "8deed9fd382040d2ac43402ec59384a3"),
            (4u8, 32usize, "169bfd7e1c86be02b7b6f3c405b62071"),
        ] {
            let salt = alloc::vec![0x22u8; len];
            let master = alloc::vec![0x11u8; len];
            let mat = KeyMaterial::from_keymat_cyclone(&keymat(
                kind,
                &salt,
                [1, 2, 3, 4],
                &alloc::vec![0x44u8; len],
                Some(([9, 9, 9, 9], &master)),
            ))
            .unwrap();
            let mut header = mat.transformation_kind_bytes().to_vec();
            header.extend_from_slice(&[1, 2, 3, 4]);
            header.extend_from_slice(&session_id);
            header.extend_from_slice(&iv_suffix);
            let key = mat.origin.receiver_key.clone().expect("the receiver key");
            assert_eq!(key.id, [9, 9, 9, 9]);
            let mac = mat.receiver_mac(&key, &header, &common_mac).unwrap();
            let hex: String = mac.iter().map(|b| alloc::format!("{b:02x}")).collect();
            assert_eq!(hex, expected, "kind {kind}");
        }
    }

    /// §10.5.2.1.1: "A zero value for receiver_specific_key_id ... shall
    /// occur if and only if the length of the master_receiver_specific_key
    /// is also zero"; Table 74: the key has the length of the sender key.
    #[test]
    fn a_receiver_specific_key_id_and_key_go_together() {
        let salt = [0x22u8; 16];
        let key = [0x44u8; 16];
        let none = keymat(2, &salt, [1, 2, 3, 4], &key, None);
        assert!(
            KeyMaterial::from_keymat_cyclone(&none)
                .unwrap()
                .origin
                .receiver_key
                .is_none()
        );
        for bad in [
            keymat(2, &salt, [1, 2, 3, 4], &key, Some(([0; 4], &[7u8; 16]))),
            keymat(2, &salt, [1, 2, 3, 4], &key, Some(([5; 4], &[]))),
            keymat(2, &salt, [1, 2, 3, 4], &key, Some(([5; 4], &[7u8; 32]))),
        ] {
            assert!(KeyMaterial::from_keymat_cyclone(&bad).is_err());
        }
    }

    /// Origin authentication (§10.5.3.1, §10.5.3.3.6): the writer gives each
    /// reader its own receiver-specific key, its submessages carry a MAC for
    /// each, a reader checks the MAC of its key. A reader that shares the
    /// sender key cannot make a message another reader accepts, and a
    /// message without the MAC, or with a changed one, is refused.
    #[test]
    fn a_reader_checks_the_origin_of_a_submessage_by_its_own_key() {
        use zerodds_security::crypto::{
            ENDPOINT_DATA_PROTECTION_KIND as DATA, ENDPOINT_METADATA_PROTECTION_KIND as META,
        };
        let mut w = AesGcmCryptoPlugin::new();
        let w_local = w
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let writer = w
            .register_local_endpoint(
                w_local,
                true,
                &[(META, "ENCRYPT_WITH_ORIGIN_AUTHENTICATION"), (DATA, "NONE")],
            )
            .unwrap();
        let reader_a = [0xA1u8; 16];
        let reader_b = [0xB2u8; 16];
        let token_a = w
            .create_endpoint_crypto_token_for(writer, reader_a)
            .unwrap();
        let token_b = w
            .create_endpoint_crypto_token_for(writer, reader_b)
            .unwrap();
        assert_eq!(
            token_a,
            w.create_endpoint_crypto_token_for(writer, reader_a)
                .unwrap(),
            "one key for a reader"
        );
        let parsed_a = KeyMaterial::from_keymat_cyclone(&token_a).unwrap();
        let parsed_b = KeyMaterial::from_keymat_cyclone(&token_b).unwrap();
        let (key_a, key_b) = (
            parsed_a.origin.receiver_key.clone().expect("key of A"),
            parsed_b.origin.receiver_key.clone().expect("key of B"),
        );
        assert_ne!(key_a.id, key_b.id);
        assert_ne!(key_a.master, key_b.master);
        assert_eq!(parsed_a.master_key, parsed_b.master_key, "one sender key");

        let wire = w
            .encode_data_datawriter_submessage(writer, b"the DATA submessage")
            .unwrap();
        let reader = |token: &[u8]| {
            let mut p = AesGcmCryptoPlugin::new();
            let local = p
                .register_local_participant(IdentityHandle(1), &[])
                .unwrap();
            let remote = p
                .register_matched_remote_participant(
                    local,
                    IdentityHandle(2),
                    SharedSecretHandle(1),
                )
                .unwrap();
            p.set_remote_endpoint_crypto_tokens(local, remote, token)
                .unwrap();
            (p, remote)
        };
        let (a, from_w_a) = reader(&token_a);
        let (b, from_w_b) = reader(&token_b);
        assert_eq!(
            a.decode_data_by_key_id_from(from_w_a, &wire).unwrap(),
            b"the DATA submessage"
        );
        assert_eq!(
            b.decode_data_by_key_id_from(from_w_b, &wire).unwrap(),
            b"the DATA submessage"
        );

        // The footer lists both receivers: count, then id and MAC each.
        let (_, _, footer) = parse_secure_submessages_footer(&wire).unwrap();
        assert_eq!(footer.len(), 16 + 4 + 2 * 20);
        assert_eq!(&footer[16..20], &2u32.to_be_bytes());

        // A changed MAC for A: A refuses, B (whose MAC is intact) accepts.
        let mut forged = wire.clone();
        let at = forged
            .windows(footer.len())
            .position(|w| w == footer.as_slice())
            .unwrap();
        let entry_a = (0..2)
            .map(|i| at + 20 + i * 20)
            .find(|&o| forged[o..o + 4] == key_a.id)
            .unwrap();
        forged[entry_a + 4] ^= 1;
        assert!(a.decode_data_by_key_id_from(from_w_a, &forged).is_err());
        assert!(b.decode_data_by_key_id_from(from_w_b, &forged).is_ok());

        // B holds the sender key. A message it makes with it, without the
        // receiver-specific MAC of A, is refused by A.
        let mut b_copy = KeyMaterial::from_keymat_cyclone(&token_b).unwrap();
        b_copy.origin = Origin::none();
        let (header, ct, tag) = b_copy.seal_cyclone(b"from B, not from the writer").unwrap();
        let forged_by_b = assemble_secure_submessages(&header, &ct, &tag);
        assert!(
            a.decode_data_by_key_id_from(from_w_a, &forged_by_b)
                .is_err()
        );
        // It does open for a reader that has no receiver-specific key.
        let mut plain_reader = AesGcmCryptoPlugin::new();
        let local = plain_reader
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = plain_reader
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        plain_reader
            .set_remote_endpoint_crypto_tokens(
                local,
                remote,
                &w.create_local_participant_crypto_tokens(writer, CryptoHandle(0))
                    .unwrap(),
            )
            .unwrap();
        assert!(
            plain_reader
                .decode_data_by_key_id_from(remote, &forged_by_b)
                .is_ok(),
            "no origin authentication without a receiver-specific key"
        );
    }

    /// Without origin authentication the token has no receiver-specific key
    /// and the footer no MAC (§10.5.2.1.1).
    #[test]
    fn a_key_without_origin_authentication_has_no_receiver_specific_mac() {
        use zerodds_security::crypto::{
            ENDPOINT_DATA_PROTECTION_KIND as DATA, ENDPOINT_METADATA_PROTECTION_KIND as META,
        };
        let mut w = AesGcmCryptoPlugin::new();
        let local = w
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let writer = w
            .register_local_endpoint(local, true, &[(META, "ENCRYPT"), (DATA, "NONE")])
            .unwrap();
        let token = w.create_endpoint_crypto_token_for(writer, [7; 16]).unwrap();
        assert!(
            KeyMaterial::from_keymat_cyclone(&token)
                .unwrap()
                .origin
                .receiver_key
                .is_none()
        );
        let wire = w.encode_data_datawriter_submessage(writer, b"x").unwrap();
        let (_, _, footer) = parse_secure_submessages_footer(&wire).unwrap();
        assert_eq!(&footer[16..], &0u32.to_be_bytes());
    }

    /// DDS-Security 1.2 §9.5.1.9.5: the token of a relay-only reader opens
    /// the writer's submessages and not its payload; the full token opens
    /// both. A writer without a key for its submessages alone has no relay
    /// token, never the payload key.
    #[test]
    fn a_relay_token_opens_the_submessage_and_not_the_payload() {
        use zerodds_security::crypto::{
            ENDPOINT_DATA_PROTECTION_KIND as DATA, ENDPOINT_METADATA_PROTECTION_KIND as META,
        };
        let mut w = AesGcmCryptoPlugin::new();
        let w_local = w
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let writer = w
            .register_local_endpoint(w_local, true, &[(META, "ENCRYPT"), (DATA, "ENCRYPT")])
            .unwrap();
        let submessage = w
            .encode_data_datawriter_submessage(writer, b"the DATA submessage")
            .unwrap();
        let payload = w.encode_serialized_payload(writer, b"the payload").unwrap();

        let receiver = |tokens: &[Vec<u8>]| {
            let mut p = AesGcmCryptoPlugin::new();
            let local = p
                .register_local_participant(IdentityHandle(1), &[])
                .unwrap();
            let remote = p
                .register_matched_remote_participant(
                    local,
                    IdentityHandle(2),
                    SharedSecretHandle(1),
                )
                .unwrap();
            for token in tokens {
                p.set_remote_endpoint_crypto_tokens(local, remote, token)
                    .unwrap();
            }
            (p, remote)
        };
        let relay_token = w.endpoint_relay_token(writer).expect("a relay token");
        let (relay, from_w) = receiver(&[relay_token.clone()]);
        assert_eq!(
            relay
                .decode_data_by_key_id_from(from_w, &submessage)
                .unwrap(),
            b"the DATA submessage"
        );
        assert!(
            relay
                .decode_serialized_payload_by_key_id_from(from_w, &payload)
                .is_err(),
            "the relay opened the CryptoContent"
        );

        let full = [
            w.create_local_participant_crypto_tokens(writer, CryptoHandle(0))
                .unwrap(),
            w.endpoint_payload_token(writer).unwrap(),
        ];
        assert_eq!(
            full[0], relay_token,
            "the relay token is the submessage keymat"
        );
        let (subscriber, from_w) = receiver(&full);
        assert_eq!(
            subscriber
                .decode_serialized_payload_by_key_id_from(from_w, &payload)
                .unwrap(),
            b"the payload"
        );

        let payload_only = w
            .register_local_endpoint(w_local, true, &[(META, "NONE"), (DATA, "ENCRYPT")])
            .unwrap();
        assert!(
            w.endpoint_relay_token(payload_only).is_none(),
            "its one key is the payload key"
        );
        let shared = w.register_local_endpoint(w_local, true, &[]).unwrap();
        assert!(
            w.endpoint_relay_token(shared).is_none(),
            "one key for submessages and payload"
        );
        let plain_payload = w
            .register_local_endpoint(w_local, true, &[(META, "SIGN"), (DATA, "NONE")])
            .unwrap();
        assert!(w.endpoint_relay_token(plain_payload).is_some());
        let reader = w
            .register_local_endpoint(w_local, false, &[(META, "ENCRYPT"), (DATA, "ENCRYPT")])
            .unwrap();
        assert!(
            w.endpoint_relay_token(reader).is_none(),
            "a reader relays nothing"
        );
    }

    /// A GMAC SRTPS message with a vendor submessage after the
    /// SRTPS_POSTFIX, as Fast DDS sends it, whose octet 24 before the end is
    /// 0x34. The SRTPS_POSTFIX is the one the submessages lead to; the
    /// parser took that octet for its id, cut ct and the MAC in the wrong
    /// place, and the GMAC check failed (measured against Fast DDS 3.6.1
    /// under rtps_protection SIGN: 2 to 6 refused messages per 200 samples).
    #[test]
    fn the_srtps_postfix_is_found_by_its_submessages_not_by_its_place() {
        let mut cheader = alloc::vec![0x00, 0x00, 0x00, 0x01];
        cheader.extend_from_slice(&[0xAA; 16]);
        let ct = [0x15u8, 0x01, 0x04, 0x00, 1, 2, 3, 4];
        let tag = [0x77u8; 16];
        let mut wire = assemble_srtps_message(&[0u8; 20], &cheader, &ct, &tag);
        // A vendor submessage of 28 octets after the POSTFIX, with 0x34 at
        // 24 octets before the end of the message.
        let mut vendor = alloc::vec![0x80, 0x01, 24, 0x00];
        vendor.extend_from_slice(&[0u8; 24]);
        vendor[4] = SRTPS_POSTFIX_ID;
        wire.extend_from_slice(&vendor);
        assert_eq!(wire[wire.len() - 24], SRTPS_POSTFIX_ID);
        let (_, ch, got_ct, got_tag) = parse_srtps_message(&wire).unwrap();
        assert_eq!(ch, cheader);
        assert_eq!(got_ct, ct);
        assert_eq!(got_tag, tag);
    }

    /// A submessage: id, flags (little endian), the length of `body`, `body`.
    fn submessage(id: u8, body: &[u8]) -> Vec<u8> {
        let mut out = alloc::vec![id, SUBMSG_FLAG_LE];
        out.extend_from_slice(&(body.len() as u16).to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    /// A SEC_PREFIX + SEC_BODY + SEC_POSTFIX sequence of AES256_GCM.
    fn sec_sequence(prefix: &[u8], body: &[u8], postfix: &[u8]) -> Vec<u8> {
        let mut out = submessage(SEC_PREFIX_ID, prefix);
        out.extend(submessage(SEC_BODY_ID, body));
        out.extend(submessage(SEC_POSTFIX_ID, postfix));
        out
    }

    /// DDS-Security 1.2 §7.4.6.3.3: "The RTPS Submessage is invalid if the
    /// submessageLength in the Submessage header is too small": a SEC_BODY
    /// whose `content` is longer than the submessage is refused, as is one that
    /// cannot hold the length of the content.
    #[test]
    fn a_secure_body_with_too_small_a_length_is_invalid() {
        let mut prefix = alloc::vec![0, 0, 0, 0x04];
        prefix.extend_from_slice(&[0u8; 16]);
        let postfix = [0u8; 16];
        // content.length says 100 octets, the submessage holds 2.
        let mut body = 100u32.to_be_bytes().to_vec();
        body.extend_from_slice(&[1, 2]);
        let err = parse_secure_submessages(&sec_sequence(&prefix, &body, &postfix)).unwrap_err();
        assert!(err.detail.contains("content.length"), "{err}");
        // A body of three octets cannot hold the length at all.
        let err =
            parse_secure_submessages(&sec_sequence(&prefix, &[0, 0, 0], &postfix)).unwrap_err();
        assert!(err.detail.contains("SEC_BODY"), "{err}");
        // The same sequence with a fitting length is valid.
        let mut fitting = 2u32.to_be_bytes().to_vec();
        fitting.extend_from_slice(&[1, 2]);
        let (_, ct, _) =
            parse_secure_submessages(&sec_sequence(&prefix, &fitting, &postfix)).unwrap();
        assert_eq!(ct, [1, 2]);
    }

    /// §7.4.6.4.3: a SEC_PREFIX whose submessageLength is too small for the
    /// CryptoHeader (20 octets) is invalid.
    #[test]
    fn a_secure_prefix_with_too_small_a_length_is_invalid() {
        let short = [0u8, 0, 0, 0x04, 1, 2, 3];
        let mut body = 0u32.to_be_bytes().to_vec();
        body.extend_from_slice(&[]);
        let err = parse_secure_submessages(&sec_sequence(&short, &body, &[0u8; 16])).unwrap_err();
        assert!(err.detail.contains("SEC_PREFIX"), "{err}");
    }

    /// §7.4.6.5.3: a SEC_POSTFIX with no SEC_PREFIX before it is invalid, and
    /// so is one whose body cannot hold the common_mac (16 octets).
    #[test]
    fn a_secure_postfix_without_a_prefix_is_invalid() {
        let alone = submessage(SEC_POSTFIX_ID, &[0u8; 16]);
        let err = parse_secure_submessages(&alone).unwrap_err();
        assert!(err.detail.contains("SEC_PREFIX"), "{err}");
        let mut prefix = alloc::vec![0, 0, 0, 0x04];
        prefix.extend_from_slice(&[0u8; 16]);
        let err = parse_secure_submessages(&sec_sequence(&prefix, &0u32.to_be_bytes(), &[0u8; 15]))
            .unwrap_err();
        assert!(err.detail.contains("SEC_POSTFIX"), "{err}");
    }

    /// DDSI-RTPS 2.5 §9.4.5.1.3: an INFO_TS with octetsToNextHeader 0 has an
    /// empty body ("the next Submessage header starts immediately after");
    /// it was read as the last submessage and swallowed the SRTPS_POSTFIX.
    #[test]
    fn an_info_ts_with_otn_zero_inside_srtps_is_an_empty_submessage() {
        let mut cheader = alloc::vec![0x00, 0x00, 0x00, 0x01];
        cheader.extend_from_slice(&[0xAA; 16]);
        let ct = [0x09u8, 0x03, 0x00, 0x00, 0x15, 0x01, 0x04, 0x00, 1, 2, 3, 4];
        let tag = [0x66u8; 16];
        let wire = assemble_srtps_message(&[0u8; 20], &cheader, &ct, &tag);
        let (_, _, got_ct, got_tag) = parse_srtps_message(&wire).unwrap();
        assert_eq!(got_ct, ct);
        assert_eq!(got_tag, tag);
        assert_eq!(submessage_body_len(PAD_ID, 0, 40), 0);
        assert_eq!(submessage_body_len(0x15, 0, 40), 40);
    }

    /// An SRTPS message as a sender with the flag A makes it (DDS-Security
    /// 1.2 §10.5.3.3.1 Table 80, 2.1): the body without INFO_SRC encrypted
    /// with the RTPS Header as AAD, the flag A in the SRTPS_PREFIX.
    fn srtps_with_flag_a(mat: &KeyMaterial, header: &[u8], body: &[u8]) -> Vec<u8> {
        let key_len = mat.suite.key_len();
        let session_id = [0x11u8, 0x22, 0x33, 0x44];
        let iv_suffix = [0u8, 0, 0, 0, 0, 0, 0, 9];
        let sk = crate::session_key::derive_session_key_cyclone(
            &mat.master_key,
            &mat.master_salt,
            &session_id,
            key_len,
        );
        let key = key_from_bytes(mat.suite, &sk[..key_len]).unwrap();
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&session_id);
        nonce[4..].copy_from_slice(&iv_suffix);
        let mut ct = body.to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            crate::backend::aead::Aad::from(header),
            &mut ct,
        )
        .unwrap();
        let tag = ct.split_off(ct.len() - 16);
        let mut cheader = mat.suite.transform_kind().to_vec();
        cheader.extend_from_slice(&mat.transformation_key_id);
        cheader.extend_from_slice(&session_id);
        cheader.extend_from_slice(&iv_suffix);
        let mut wire = assemble_srtps_message(header, &cheader, &ct, &tag);
        wire[21] |= SRTPS_FLAG_A;
        wire
    }

    /// §7.4.6.6.4: with the flag A the tags include the RTPS Header as
    /// "Additional Authenticated Data". Such a message was checked against
    /// no AAD and refused; now it decodes, and one with a changed header
    /// fails. A message with the flag P (pre-shared key) is refused as
    /// such.
    #[test]
    fn a_message_with_the_flag_a_is_checked_with_its_header() {
        let (mut alice, mut bob) = dual_key_pair(0x66);
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let _ = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        let tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bl, b2a, &tok)
            .unwrap();
        let mut header = b"RTPS\x02\x05\x01\x0f".to_vec();
        header.extend_from_slice(&[0x5A; 12]);
        let body = b"\x15\x01\x04\x00\x01\x02\x03\x04";
        let wire = {
            let slots = alice.slots.read().unwrap();
            srtps_with_flag_a(&slots[&al], &header, body)
        };
        let mut expected = header.clone();
        expected.extend_from_slice(body);
        assert_eq!(bob.decode_rtps_message_cyclone(&wire).unwrap(), expected);

        let mut other_vendor = wire.clone();
        other_vendor[7] ^= 0x01;
        assert!(
            bob.decode_rtps_message_cyclone(&other_vendor).is_err(),
            "the header is authenticated"
        );
        let mut psk = wire.clone();
        psk[21] |= SRTPS_FLAG_P;
        let err = bob.decode_rtps_message_cyclone(&psk).unwrap_err();
        assert!(err.detail.contains("pre-shared"), "{err}");
    }

    /// Table 80, 3: the HeaderExtension as AAD has `messageLength` and
    /// `messageChecksum` set to zero; the decoded message keeps the extension
    /// with both values describing the decoded message (Table 80, 4: they
    /// "correspond to the values passed as input to the encode_rtps_message
    /// operation"), the checksum of the kind the flags name.
    #[test]
    fn the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message() {
        // E | L | T | C=01 (CRC-32C): messageLength(4), timestamp(8),
        // checksum(4).
        let mut he = alloc::vec![HEADER_EXTENSION_ID, 0x27, 16, 0];
        he.extend_from_slice(&[0xAA; 4]);
        he.extend_from_slice(&[0x77; 8]);
        he.extend_from_slice(&[0xCC; 4]);
        let aad = header_extension_for_aad(&he);
        assert_eq!(&aad[..4], &he[..4]);
        assert_eq!(&aad[4..8], &[0; 4]);
        assert_eq!(&aad[8..16], &[0x77; 8]);
        assert_eq!(&aad[16..20], &[0; 4]);
        let header = [
            0x52, 0x54, 0x50, 0x53, 2, 5, 1, 1, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
        ];
        let body = [0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8];
        let decoded = decoded_rtps_message(&header, &he, &body).unwrap();
        assert_eq!(decoded.len(), 20 + 20 + 12);
        assert_eq!(&decoded[..20], &header);
        assert_eq!(decoded[21], 0x27, "the flags stay: E | L | T | C=01");
        assert_eq!(&decoded[24..28], &(52u32).to_le_bytes(), "messageLength");
        assert_eq!(&decoded[28..36], &[0x77; 8], "the timestamp stays");
        let mut zeroed = decoded.clone();
        zeroed[36..40].fill(0);
        assert_eq!(
            &decoded[36..40],
            &zerodds_foundation::crc32c(&zeroed).to_be_bytes(),
            "CRC-32C of the message with the checksum zero, big endian"
        );
        // C=10 (CRC-64/XZ, 8 octets) and C=11 (MD5, 16 octets).
        for (flags, len) in [(0x47u8, 8usize), (0x67u8, 16usize)] {
            let mut he = alloc::vec![HEADER_EXTENSION_ID, flags, (4 + 8 + len) as u8, 0];
            he.extend_from_slice(&[0xAA; 4]);
            he.extend_from_slice(&[0x77; 8]);
            he.extend_from_slice(&alloc::vec![0xCC; len]);
            let decoded = decoded_rtps_message(&header, &he, &body).unwrap();
            let at = 24 + 4 + 8;
            let mut zeroed = decoded.clone();
            zeroed[at..at + len].fill(0);
            let expected = if len == 8 {
                zerodds_foundation::crc64_xz(&zeroed).to_be_bytes().to_vec()
            } else {
                zerodds_foundation::md5(&zeroed).to_vec()
            };
            assert_eq!(
                &decoded[at..at + len],
                expected.as_slice(),
                "C bits {flags:#x}"
            );
        }
    }

    /// DDS-Security 1.2 §10.5.2 Table 77, §10.5.2.1.1: an AES128-GMAC key
    /// (kind 0x01) with a 16-octet `master_salt` goes from one instance to
    /// another, and its SRTPS messages verify there. Kind 0x01 was read as
    /// HMAC-SHA256, and a `master_salt` other than 32 octets was refused.
    #[test]
    fn an_aes128_gmac_key_with_a_short_salt_crosses_instances() {
        let shared = alloc::vec![0x44u8; 32];
        let pa = ArcA::new(MemProvider::new());
        let pb = ArcA::new(MemProvider::new());
        pa.insert(SharedSecretHandle(1), shared.clone());
        pb.insert(SharedSecretHandle(1), shared);
        let mut alice = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gmac,
            ArcA::clone(&pa) as ArcA<dyn SharedSecretProvider>,
        );
        let mut bob = AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gmac,
            ArcA::clone(&pb) as ArcA<dyn SharedSecretProvider>,
        );
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let _ = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        let tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        assert_eq!(&tok[..4], &[0, 0, 0, 1], "AES128_GMAC");
        assert_eq!(&tok[4..8], &16u32.to_be_bytes(), "a 16-octet master_salt");
        bob.set_remote_participant_crypto_tokens(bl, b2a, &tok)
            .unwrap();
        // One INFO_TS submessage: the receiver walks the submessages to the
        // SRTPS_POSTFIX (DDSI-RTPS 2.5 §9.4.5.1.3).
        let info_ts = [0x09, 0x01, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 7, 8];
        let msg = [b"RTPS\x02\x05".as_slice(), &[0u8; 14], &info_ts].concat();
        let wire = alice.encode_rtps_message_cyclone(al, &msg).unwrap();
        assert_eq!(bob.decode_rtps_message_cyclone(&wire).unwrap(), msg);
        // A timestamp octet after the RTPS header and the SRTPS_PREFIX.
        let mut forged = wire.clone();
        let at = 20 + 24 + 4 + 1;
        forged[at] ^= 0x01;
        assert!(bob.decode_rtps_message_cyclone(&forged).is_err());
    }

    /// DDS-Security 1.2 §10.5.3.3.2 Table 81, §10.5.3.3.4: after
    /// `max_blocks_per_session` blocks the SessionId is incremented and a
    /// new session key derived; the receiver follows it from the
    /// CryptoHeader. The SessionId never changed.
    #[test]
    fn the_session_id_changes_after_max_blocks_per_session() {
        let (mut alice, mut bob) = dual_key_pair(0x77);
        alice.set_max_blocks_per_session(2);
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let _ = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        let tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bl, b2a, &tok)
            .unwrap();
        let msg = [b"RTPS\x02\x05".as_slice(), &[0u8; 14], &[0x5A; 8]].concat();
        let mut sessions = Vec::new();
        for _ in 0..3 {
            let wire = alice.encode_rtps_message_cyclone(al, &msg).unwrap();
            // SRTPS_PREFIX(4) | transform_kind(4) | key_id(4) | session_id(4)
            sessions.push(<[u8; 4]>::try_from(&wire[32..36]).unwrap());
            assert_eq!(bob.decode_rtps_message_cyclone(&wire).unwrap(), msg);
        }
        // INFO_SRC(24) and the 8 octets of the body: 2 blocks per message.
        assert_ne!(
            sessions[0], sessions[1],
            "2 blocks each, at most 2 per session"
        );
        assert_eq!(
            u32::from_be_bytes(sessions[1]),
            u32::from_be_bytes(sessions[0]).wrapping_add(1),
            "incremented"
        );
        assert_ne!(sessions[1], sessions[2]);
    }

    /// §10.5.2.2: the transformation_key_id identifies the key material;
    /// the keys of one side have distinct ids, and none has 0, the id of
    /// the Kx key.
    #[test]
    fn the_local_keys_have_distinct_nonzero_key_ids() {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let mut ids = BTreeSet::new();
        for _ in 0..512 {
            let h = p.register_local_endpoint(local, true, &[]).unwrap();
            let token = p
                .create_local_participant_crypto_tokens(h, CryptoHandle(0))
                .unwrap();
            let salt_len = u32::from_be_bytes([token[4], token[5], token[6], token[7]]) as usize;
            let key_id: [u8; 4] = token[8 + salt_len..12 + salt_len].try_into().unwrap();
            assert_ne!(key_id, [0; 4]);
            assert!(ids.insert(key_id), "a key id twice");
        }
    }

    /// §7.3.17, §7.3.20.2: the revision is part of the key material; it
    /// survives the keymat and the CryptoHeader of what the key protects
    /// carries it, and the same key id under another revision is another key.
    #[test]
    fn the_key_revision_is_carried_in_the_keymat_and_the_header() {
        let rng = SystemRandom::new();
        let mut mat = KeyMaterial::new_random(Suite::Aes256Gcm, &rng).unwrap();
        mat.key_revision = [0, 0, 22];
        let keymat = mat.serialize_keymat_cyclone();
        assert_eq!(&keymat[..4], &[0, 0, 22, 4]);
        let back = KeyMaterial::from_keymat_cyclone(&keymat).unwrap();
        assert_eq!(back.key_revision, [0, 0, 22]);
        let (header, ct, tag) = mat.seal_cyclone(b"payload").unwrap();
        assert_eq!(&header[..4], &[0, 0, 22, 4]);
        assert_eq!(back.open_cyclone(&header, &ct, &tag).unwrap(), b"payload");
        let mut other = KeyMaterial::from_keymat_cyclone(&keymat).unwrap();
        other.key_revision = [0, 0, 23];
        assert!(
            other.open_cyclone(&header, &ct, &tag).is_err(),
            "another revision of the same key id"
        );
    }

    /// DDS-Security 1.2 §7.3.20.2: "The 3-tuple (sender_participant_guid,
    /// transformation_key_id, transformation_key_revision) uniquely identifies
    /// the Key Material". The same key id of the same sender in two revisions
    /// is two keys, the same key id of another sender a third, and a revision
    /// no token delivered is no key at all.
    #[test]
    fn a_key_is_found_by_sender_key_id_and_revision() {
        let plugin = AesGcmCryptoPlugin::new();
        let rng = SystemRandom::new();
        let key_with = |revision: [u8; 3]| {
            let mut mat = KeyMaterial::new_random(Suite::Aes256Gcm, &rng).unwrap();
            mat.transformation_key_id = [0, 0, 0, 7];
            mat.key_revision = revision;
            mat
        };
        let (alice, carol) = (CryptoHandle(1001), CryptoHandle(1002));
        plugin.index_remote_key(alice, key_with([0, 0, 0])).unwrap();
        plugin.index_remote_key(alice, key_with([0, 0, 1])).unwrap();
        plugin.index_remote_key(carol, key_with([0, 0, 0])).unwrap();
        let slot = |owner, revision| {
            plugin
                .key_id_slot(Some(owner), [0, 0, 0, 7], revision)
                .unwrap()
        };
        let (a0, a1, c0) = (
            slot(alice, [0, 0, 0]).expect("revision 0"),
            slot(alice, [0, 0, 1]).expect("revision 1"),
            slot(carol, [0, 0, 0]).expect("another sender"),
        );
        assert!(a0 != a1 && a0 != c0 && a1 != c0, "three keys");
        assert_eq!(slot(alice, [0, 0, 2]), None, "a revision no token gave");
        assert_eq!(slot(carol, [0, 0, 1]), None, "carol has no revision 1");
    }

    #[test]
    fn srtps_cyclone_cross_instance_roundtrip() {
        // Cyclone-conformant message-level SRTPS: alice encodes with the local key
        // (CryptoHeader with key_id in the SRTPS_PREFIX), bob decodes by key_id
        // (remote_by_key_id via the participant token). Cross-instance = the real
        // cross-vendor/cyclone case (part 2).
        let (mut alice, mut bob) = dual_key_pair(0x55);
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let _a2b = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        let tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bl, b2a, &tok)
            .unwrap();
        let msg = [
            b"RTPS\x02\x05".as_slice(),
            &[0u8; 14],
            b"[SEDP secure body]",
        ]
        .concat();
        let wire = alice.encode_rtps_message_cyclone(al, &msg).unwrap();
        assert_eq!(wire[20], 0x33, "SRTPS_PREFIX after 20-byte header");
        let back = bob
            .decode_rtps_message_cyclone(&wire)
            .expect("cross-instance SRTPS decode");
        assert_eq!(back, msg, "cross-instance SRTPS cyclone roundtrip");
    }

    /// DDS-Security 1.2 §9.5.1.8: `unregister_local_endpoint` releases the
    /// key material of the endpoint, `unregister_matched_remote_participant`
    /// the keys of a remote participant. Both stayed for the life of the
    /// plugin.
    #[test]
    fn unregistering_releases_the_key_material() {
        let (mut alice, mut bob) = dual_key_pair(0x57);
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let _ = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        let tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bl, b2a, &tok)
            .unwrap();
        let msg = [b"RTPS\x02\x05".as_slice(), &[0u8; 14], b"[body]"].concat();
        let wire = alice.encode_rtps_message_cyclone(al, &msg).unwrap();
        assert_eq!(bob.decode_rtps_message_cyclone(&wire).unwrap(), msg);
        // Bob forgets Alice: her key no longer decodes.
        bob.unregister_matched_remote_participant(b2a).unwrap();
        assert!(bob.decode_rtps_message_cyclone(&wire).is_err());
        // Alice releases an endpoint: its handle is gone.
        let ep = alice.register_local_endpoint(al, true, &[]).unwrap();
        assert!(
            alice
                .create_local_participant_crypto_tokens(ep, CryptoHandle(0))
                .is_ok()
        );
        alice.unregister_local_endpoint(ep).unwrap();
        assert!(
            alice
                .create_local_participant_crypto_tokens(ep, CryptoHandle(0))
                .is_err()
        );
    }

    /// §7.3.18, §7.3.20.1: `transformation_kind` is the key revision (three
    /// octets) and the algorithm id (the fourth). A CryptoHeader whose kind
    /// is not the one of the key it selects (AES128_GCM = 0x02 here) is
    /// refused; the kind is not under the tag, so the tag alone does not
    /// show the change.
    #[test]
    fn a_header_kind_other_than_the_keys_is_refused() {
        let (mut alice, mut bob) = dual_key_pair(0x56);
        let al = alice
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let bl = bob
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let _ = alice
            .register_matched_remote_participant(al, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let b2a = bob
            .register_matched_remote_participant(bl, IdentityHandle(1), SharedSecretHandle(1))
            .unwrap();
        let tok = alice
            .create_local_participant_crypto_tokens(al, CryptoHandle(0))
            .unwrap();
        bob.set_remote_participant_crypto_tokens(bl, b2a, &tok)
            .unwrap();
        let msg = [b"RTPS\x02\x05".as_slice(), &[0u8; 14], b"[body]"].concat();
        let wire = alice.encode_rtps_message_cyclone(al, &msg).unwrap();
        // RTPS header (20), SRTPS_PREFIX submessage header (4), CryptoHeader.
        let kind = 20 + 4;
        assert_eq!(&wire[kind..kind + 4], &[0, 0, 0, 0x02], "AES128_GCM");
        assert_eq!(bob.decode_rtps_message_cyclone(&wire).unwrap(), msg);
        for (octet, value) in [(3, 0x04), (3, 0x03), (0, 0x01)] {
            let mut forged = wire.clone();
            forged[kind + octet] = value;
            assert!(
                bob.decode_rtps_message_cyclone(&forged).is_err(),
                "kind octet {octet} = {value:#04x}"
            );
        }
    }
}
