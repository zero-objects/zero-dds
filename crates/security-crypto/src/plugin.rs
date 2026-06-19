// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AES-GCM-128 / AES-GCM-256 CryptographicPlugin implementation.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`Arc<dyn SharedSecretProvider>` is held so the
//! plugin works with arbitrary authentication plugins. The
//! provider is a pure lookup interface with no safety implication.)

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle, SharedSecretProvider};
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin, ReceiverMac};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};

use ring::aead::{LessSafeKey, Nonce, UnboundKey};
use ring::digest;
use ring::hkdf;
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};

use crate::suite::Suite;

/// Session ID: 4-byte prefix of a 12-byte GCM nonce. Drawn
/// randomly per `CryptoHandle`; protects against nonce collision
/// between different keys.
type SessionId = [u8; 4];

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
    /// Master sender key. Length per `suite.key_len()` (16 or 32).
    master_key: Vec<u8>,
    /// `master_salt` (spec §10.5.2 Tab.73 + Tab.74) — 32 bytes; feeds
    /// into `derive_session_key`.
    master_salt: [u8; 32],
    /// Session ID — constant per slot, rotated on key refresh.
    session_id: SessionId,
    /// Nonce counter — monotonic per encrypt. On overflow (u64::MAX)
    /// the encrypt is rejected.
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
    // Spec-conformant token (de)serializer; the active path is serialize_keymat_cyclone.
    #[allow(dead_code)]
    fn from_serialized(serialized: &[u8]) -> SecurityResult<Self> {
        if serialized.len() < 1 + 4 + 4 + 32 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: token too short (at least 41 bytes before master_key)",
            ));
        }
        // Spec-conformant: AES128_GCM=0x02, AES256_GCM=0x04, HmacSha256
        // im AES128_GMAC-Slot=0x01 (siehe Suite::transform_kind_id).
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

    #[allow(dead_code)] // Reserved API, see from_serialized
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + 4 + 4 + 32 + self.master_key.len());
        out.push(self.suite.transform_kind_id());
        out.extend_from_slice(&self.session_id);
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
        let mut out = Vec::with_capacity(4 + 4 + 32 + 4 + 4 + self.master_key.len() + 8);
        out.extend_from_slice(&self.suite.transform_kind());
        out.extend_from_slice(&(self.master_salt.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.master_salt);
        out.extend_from_slice(&self.transformation_key_id);
        out.extend_from_slice(&(self.master_key.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.master_key);
        out.extend_from_slice(&[0u8; 4]); // receiver_specific_key_id = none
        out.extend_from_slice(&0u32.to_be_bytes()); // master_receiver_specific_key = empty
        out
    }

    /// Parses the cyclone/spec CDR format from [`Self::serialize_keymat_cyclone`].
    /// The receiver-specific key is ignored (the sender key suffices to decode).
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
        if salt.len() != 32 || master_key.len() != suite.key_len() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: keymat salt/key length does not match the suite",
            ));
        }
        let mut master_salt = [0u8; 32];
        master_salt.copy_from_slice(&salt);
        Ok(Self {
            suite,
            transformation_key_id: key_id,
            master_key,
            master_salt,
            session_id: [0u8; 4],
            counter: AtomicU64::new(0),
        })
    }

    /// Returns the spec-conformant `transformation_kind` as a 4-byte BE
    /// array (spec §10.5 Tab.79). Used in the AAD.
    fn transformation_kind_bytes(&self) -> [u8; 4] {
        self.suite.transform_kind()
    }

    /// **Cyclone-conformant** submessage crypto (DDS-Security §9.5.3.3, AES-GCM
    /// with an **empty AAD** — `crypto_cipher_encrypt_data` with `outpdata` set
    /// encrypts the whole input without a separate AAD buffer).
    /// Returns `(CryptoHeader[20], ciphertext, common_mac[16])`:
    ///
    /// ```text
    /// session_id  = self.session_id           (4 byte)
    /// iv_suffix   = ++counter                  (8 byte, BE)
    /// session_key = HMAC(master_key, "SessionKey" || master_salt[..keylen] || session_id)
    /// nonce(12)   = session_id || iv_suffix
    /// CryptoHeader(20, BE) = transform_kind(4) || key_id(4) || session_id(4) || iv_suffix(8)
    /// ```
    fn seal_cyclone(&self, plaintext: &[u8]) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let suffix = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        let iv_suffix = suffix.to_be_bytes();
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &self.session_id,
            key_len,
        );
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&self.session_id);
        nonce[4..].copy_from_slice(&iv_suffix);
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        // GMAC (SIGN, !is_aead): the payload stays cleartext, 16-byte auth tag over
        // the plaintext (as AAD), empty body. GCM (ENCRYPT): seal in place.
        let (buf, tag) = if self.suite.is_aead() {
            let mut b = plaintext.to_vec();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::empty(),
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
                ring::aead::Aad::from(plaintext),
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
        header.extend_from_slice(&self.suite.transform_kind());
        header.extend_from_slice(&self.transformation_key_id);
        header.extend_from_slice(&self.session_id);
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
        let suffix = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        let iv_suffix = suffix.to_be_bytes();
        let key_len = self.suite.key_len();
        let sk = crate::session_key::derive_session_key_cyclone(
            &self.master_key,
            &self.master_salt,
            &self.session_id,
            key_len,
        );
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&self.session_id);
        nonce[4..].copy_from_slice(&iv_suffix);
        let _ = aad_extension; // cyclone-conformant: SRTPS-GCM uses an empty AAD (like the data path)
        let key = key_from_bytes(self.suite, &sk[..key_len])?;
        // GMAC (rtps_protection=SIGN): submessages stay cleartext, 16-byte
        // auth tag over the plaintext (as AAD). GCM: seal in place.
        let (buf, tag) = if self.suite.is_aead() {
            let mut b = plaintext.to_vec();
            key.seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::empty(),
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
                ring::aead::Aad::from(plaintext),
                &mut sink,
            )
            .map_err(|_| {
                SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: srtps gmac failed")
            })?;
            (plaintext.to_vec(), sink)
        };
        let mut header = Vec::with_capacity(20);
        header.extend_from_slice(&self.suite.transform_kind());
        header.extend_from_slice(&self.transformation_key_id);
        header.extend_from_slice(&self.session_id);
        header.extend_from_slice(&iv_suffix);
        Ok((header, buf, tag))
    }

    /// Counterpart to [`Self::seal_rtps_cyclone`]. `aad_extension` = RTPS header.
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
                    ring::aead::Aad::empty(),
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
            // GMAC (SIGN): AAD = body-only (cyclone convention), symmetric to the seal.
            let _ = aad_extension;
            let mut buf = tag.to_vec();
            key.open_in_place(
                Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(ct),
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
                    ring::aead::Aad::empty(),
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
                ring::aead::Aad::from(ct),
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
    /// `session_key` and the AAD. Uses the C3.7 helpers from
    /// `zerodds_security_crypto::session_key`.
    fn derive_session_key_bytes(&self) -> Vec<u8> {
        self.derive_session_key_bytes_for(&self.session_id)
    }

    /// Like [`Self::derive_session_key_bytes`], but with an explicitly given
    /// `session_id`. The decode path reads the `session_id` from the **wire**
    /// (nonce prefix) instead of from the KeyMaterial — so the per-token
    /// exchanged data key (which carries **no** session_id, §9.5.2.1.1)
    /// is still decryptable.
    fn derive_session_key_bytes_for(&self, session_id: &[u8; 4]) -> Vec<u8> {
        let full =
            crate::session_key::derive_session_key(&self.master_key, &self.master_salt, session_id);
        full[..self.suite.key_len()].to_vec()
    }

    /// Builds the AAD for this submessage. `extension` is the RTPS
    /// header for `rtps_protection_kind != NONE` — for submessage
    /// protection `extension` is empty.
    fn aad(&self, extension: &[u8]) -> Vec<u8> {
        self.aad_for(self.session_id, extension)
    }

    /// Like [`Self::aad`], but with an explicitly given `session_id` (from the wire).
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
    ) -> Self {
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
        let master_salt = kx_part(challenge1, b"keyexchange salt", challenge2, shared_secret);
        let master_key =
            kx_part(challenge2, b"key exchange key", challenge1, shared_secret).to_vec();
        Self {
            suite: Suite::Aes256Gcm,
            transformation_key_id: [0u8; 4],
            master_key,
            master_salt,
            session_id: [0u8; 4],
            counter: AtomicU64::new(0),
        }
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
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"zerodds.crypto.shared-secret.v1");
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
        let master_salt_vec = expand("master_salt", 32)?;
        let mut master_salt = [0u8; 32];
        master_salt.copy_from_slice(&master_salt_vec);

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

/// Manual length struct for `ring::hkdf::Prk::expand`. The
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
    /// `endpoint_suite` (data_protection-kind) (only meta-sign-data), the
    /// endpoint uses the cyclone dual-key model: `slots` = submessage key
    /// (metadata-kind), `payload_slots` = payload key (data-kind).
    metadata_suite: Option<Suite>,
    // RwLock for `slots` — read-heavy (every encrypt/decrypt reads);
    // register happens rarely at setup.
    slots: RwLock<BTreeMap<CryptoHandle, KeyMaterial>>,
    /// Second (payload) key of a local datawriter endpoint in the dual-key
    /// model (metadata != data). Empty except for meta-sign-data.
    payload_slots: RwLock<BTreeMap<CryptoHandle, KeyMaterial>>,
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
    // CryptoHeader — hence index key_id → slot handle (multiple per peer;
    // the KeyMaterial lives in `slots`, not cloneable because of the AtomicU64 counter).
    remote_by_key_id: RwLock<BTreeMap<[u8; 4], CryptoHandle>>,
}

impl Default for AesGcmCryptoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AesGcmCryptoPlugin {
    /// Constructor with the default suite `AES-GCM-128`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_suite(Suite::Aes128Gcm)
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
            kx_slots: RwLock::new(BTreeMap::new()),
            remote_map: Mutex::new(BTreeMap::new()),
            remote_by_key_id: RwLock::new(BTreeMap::new()),
            secret_provider: None,
            participant_suite: None,
            endpoint_suite: None,
            metadata_suite: None,
        }
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
            kx_slots: RwLock::new(BTreeMap::new()),
            remote_map: Mutex::new(BTreeMap::new()),
            remote_by_key_id: RwLock::new(BTreeMap::new()),
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

    /// Forces a new master key in the slot. After `rotate_key` the
    /// old key is gone — the peer must pull a new token via
    /// [`CryptographicPlugin::create_local_participant_crypto_tokens`]
    /// and synchronize via
    /// [`CryptographicPlugin::set_remote_participant_crypto_tokens`].
    ///
    /// # Errors
    /// `BadArgument` if the handle does not exist; `CryptoFailed`
    /// if the RNG fails.
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

/// SRTPS message-protection wire (DDS-Security §7.3.7): RTPS header(20, plaintext)
/// + SRTPS_PREFIX(CryptoHeader[20]) + SEC_BODY(ct) + SRTPS_POSTFIX(common_mac).
fn assemble_srtps_message(rtps_header: &[u8], cheader: &[u8], ct: &[u8], tag: &[u8]) -> Vec<u8> {
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
        out.push(SEC_BODY_ID);
        out.push(SUBMSG_FLAG_LE);
        out.extend_from_slice(&((4 + ct.len()) as u16).to_le_bytes());
        out.extend_from_slice(&(ct.len() as u32).to_be_bytes());
        out.extend_from_slice(ct);
    }
    out.push(SRTPS_POSTFIX_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&((tag.len() + 4) as u16).to_le_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}

/// Decomposed SRTPS message: (rtps_header[20], CryptoHeader[20], ciphertext, common_mac[16]).
type SrtpsParts = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

/// Parses the SRTPS wire -> (rtps_header[20], CryptoHeader[20], ct, common_mac[16]).
fn parse_srtps_message(wire: &[u8]) -> SecurityResult<SrtpsParts> {
    if wire.len() < 20 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "crypto: srtps too short",
        ));
    }
    let rtps_header = wire[..20].to_vec();
    let (ch, ct, tag) = parse_srtps_body(&wire[20..])?;
    Ok((rtps_header, ch, ct, tag))
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
        // RTPS §8.3.3.2.3: octetsToNextHeader==0 => the submessage is the LAST
        // and reaches to the message end. OpenDDS sets that for the SRTPS_POSTFIX
        // (otn=0); cyclone/ZeroDDS carry an explicit length. Without this
        // handling ZeroDDS read the POSTFIX body as 0 bytes -> post.len() < 16 ->
        // "expected SRTPS_POSTFIX(16)" -> OpenDDS' SRTPS (secure SEDP + user DATA)
        // was COMPLETELY discarded (cross-vendor rtps-enc reverse-decode stall,
        // SEC_TRACE-attested: 59x crypto_error for OpenDDS-0x33).
        let len = if len == 0 {
            w.len().saturating_sub(4)
        } else {
            len
        };
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
        // Fast path (cyclone/zerodds): the POSTFIX is the last submessage, fixed 24 B
        // (id+flags+len + common_mac(16) + receiver_macs.len(4)=0).
        let split = r1.len() - 24;
        let post = &r1[split..];
        if post[0] == SRTPS_POSTFIX_ID {
            return Ok((pfx.to_vec(), r1[..split].to_vec(), post[4..20].to_vec()));
        }
        // Fallback (FastDDS): after the POSTFIX there is another vendor submessage
        // (e.g. 0x80), or the POSTFIX carries receiver_specific_macs -> NOT the
        // last 24 B. Walk forward to the SRTPS_POSTFIX. The ct submessages
        // are not message-final (the POSTFIX follows), so they carry no otn=0 -> the
        // walk terminates deterministically at the first 0x34.
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
                return Ok((pfx.to_vec(), r1[..consumed].to_vec(), body[..16].to_vec()));
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
    Ok((pfx.to_vec(), ct, post[..16].to_vec()))
}

fn assemble_secure_submessages(header: &[u8], ct: &[u8], tag: &[u8]) -> Vec<u8> {
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
        out.push(SEC_BODY_ID);
        out.push(SUBMSG_FLAG_LE);
        out.extend_from_slice(&((4 + ct.len()) as u16).to_le_bytes());
        out.extend_from_slice(&(ct.len() as u32).to_be_bytes());
        out.extend_from_slice(ct);
    }
    // SEC_POSTFIX: common_mac + receiver_specific_macs._length(BE)=0
    out.push(SEC_POSTFIX_ID);
    out.push(SUBMSG_FLAG_LE);
    out.extend_from_slice(&((tag.len() + 4) as u16).to_le_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}

/// Parses the `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX` sequence back to
/// `(CryptoHeader[20], ciphertext, common_mac[16])`. Counterpart to
/// [`assemble_secure_submessages`]; tolerant of the endianness flag.
fn parse_secure_submessages(wire: &[u8]) -> SecurityResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
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
        // RTPS §8.3.3.2.3: octetsToNextHeader==0 => last submessage, to the message
        // end. OpenDDS sets that for the SEC_POSTFIX (per-submessage protection,
        // discovery_/data_protection=ENCRYPT); without this handling ZeroDDS read the
        // POSTFIX body as 0 B → "expected SEC_POSTFIX(16)" → could not decode OpenDDS'
        // secure SEDP/DATA → the user-datawriter token exchange never happened →
        // OpenDDS "Crypto Key not found" (like the SRTPS otn=0 fix in parse_srtps_body).
        let len = if len == 0 {
            w.len().saturating_sub(4)
        } else {
            len
        };
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
        return Ok((prefix_body.to_vec(), ct, postfix_body[..16].to_vec()));
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
    let tag = postfix_body[..16].to_vec();
    Ok((prefix_body.to_vec(), ct, tag))
}

/// Seal (AES-GCM or HMAC-auth-only) with the given `KeyMaterial`.
/// Wire format: `[nonce(12) | ciphertext + tag(16)]` (AEAD) or
/// `[nonce(12) | plaintext | hmac(32)]` (auth-only). Shared core
/// of `encrypt_submessage` (data slot) and `encode_kx_submessage`
/// (Kx slot).
fn seal_with(mat: &KeyMaterial, plaintext: &[u8], aad_extension: &[u8]) -> SecurityResult<Vec<u8>> {
    let nonce = mat.next_nonce()?;
    // Spec §10.5.2 Tab.74: per-submessage session_key; spec §8.1 Tab.78: AAD.
    let session_key = mat.derive_session_key_bytes();
    let aad_bytes = mat.aad(aad_extension);

    if !mat.suite.is_aead() {
        // HMAC auth-only path: `aad || nonce || plaintext` — domain-separated.
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
    let mut payload: Vec<u8> = plaintext.to_vec();
    let nonce_obj = Nonce::assume_unique_for_key(nonce);
    key.seal_in_place_append_tag(nonce_obj, ring::aead::Aad::from(&aad_bytes), &mut payload)
        .map_err(|_| SecurityError::new(SecurityErrorKind::CryptoFailed, "crypto: seal failed"))?;
    let mut out = Vec::with_capacity(12 + payload.len());
    out.extend_from_slice(&nonce);
    out.extend(payload);
    Ok(out)
}

/// Open (AES-GCM or HMAC-auth-only) with the given `KeyMaterial`.
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
        if ciphertext.len() < 12 + 32 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: hmac buffer too short for nonce+tag",
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
        .open_in_place(nonce_obj, ring::aead::Aad::from(&aad_bytes), &mut buf)
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
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        let mat = KeyMaterial::new_random(
            self.participant_suite.unwrap_or(self.local_suite),
            &self.rng,
        )?;
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
        let data_placeholder = KeyMaterial::new_random(self.local_suite, &self.rng)?;
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
                    Some((c1, c2)) => KeyMaterial::from_kx_shared_secret(&secret, &c1, &c2),
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

    fn register_local_endpoint(
        &mut self,
        _participant: CryptoHandle,
        is_writer: bool,
        _properties: &[(&str, &str)],
    ) -> SecurityResult<CryptoHandle> {
        // Submessage key suite = metadata_protection-kind (reader ACKNACK +
        // writer DATA submessage); payload key suite = data_protection-kind.
        // If both are equal (all profiles except meta-sign-data) -> ONE key as
        // before. Different -> cyclone dual-key model (§9.5.3.3): separate
        // submessage key in `slots` + payload key in `payload_slots`, the token
        // then carries 2 keymats (num_key_mat=2). Only writers have a
        // payload layer; readers need only the submessage key.
        let data_suite = self.endpoint_suite.unwrap_or(self.local_suite);
        let msg_suite = self.metadata_suite.unwrap_or(data_suite);
        let msg = KeyMaterial::new_random(msg_suite, &self.rng)?;
        let handle = self.insert(msg)?;
        if is_writer && msg_suite != data_suite {
            let pay = KeyMaterial::new_random(data_suite, &self.rng)?;
            self.payload_slots
                .write()
                .map_err(|_| poisoned())?
                .insert(handle, pay);
        }
        Ok(handle)
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
        _remote: CryptoHandle,
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
        Ok(mat.serialize_keymat_cyclone())
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        _local: CryptoHandle,
        remote: CryptoHandle,
        tokens: &[u8],
    ) -> SecurityResult<()> {
        let mat = KeyMaterial::from_keymat_cyclone(tokens)?;
        let key_id = mat.transformation_key_id;
        // Per-endpoint index (key_id → own slot handle): multiple keys of a
        // peer (secure built-in endpoints + user endpoints) coexist, so that
        // decode_data_by_key_id finds the right one. One stable slot per key_id
        // (re-installing the same key_id only overwrites its key).
        let slot_handle = self
            .remote_by_key_id
            .read()
            .map_err(|_| poisoned())?
            .get(&key_id)
            .copied()
            // Use the same allocator as `insert()` (next_id = fetch_add+1),
            // otherwise the slot handle (fetch_add WITHOUT +1) collides with the
            // last-assigned local endpoint handle and overwrites
            // its KeyMaterial.
            .unwrap_or_else(|| CryptoHandle(self.next_id()));
        self.remote_by_key_id
            .write()
            .map_err(|_| poisoned())?
            .insert(key_id, slot_handle);
        let mut slots = self.slots.write().map_err(|_| poisoned())?;
        slots.insert(slot_handle, mat);
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
        let (header, ct, tag) = parse_secure_submessages(wire)?;
        mat.open_cyclone(&header, &ct, &tag)
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
        Ok(assemble_secure_submessages(&header, &ct, &tag))
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
        let (header, ct, tag) = parse_secure_submessages(wire)?;
        mat.open_cyclone(&header, &ct, &tag)
    }

    fn decode_data_by_key_id(&self, wire: &[u8]) -> SecurityResult<Vec<u8>> {
        let (header, ct, tag) = parse_secure_submessages(wire)?;
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
        let handle = self
            .remote_by_key_id
            .read()
            .map_err(|_| poisoned())?
            .get(&key_id)
            .copied()
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: no remote key for this transformation_key_id",
                )
            })?;
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: key_id slot disappeared",
            )
        })?;
        mat.open_cyclone(&header, &ct, &tag)
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
        let (rtps_header, body) = message.split_at(20);
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&local).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown local handle",
            )
        })?;
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
        let (ch, ct, tag) = mat.seal_rtps_cyclone(&inner, rtps_header)?;
        Ok(assemble_srtps_message(rtps_header, &ch, &ct, &tag))
    }

    fn decode_rtps_message_cyclone(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        let (rtps_header, ch, ct, tag) = parse_srtps_message(message)?;
        if ch.len() < 8 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: SRTPS CryptoHeader too short",
            ));
        }
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&ch[4..8]);
        let handle = self
            .remote_by_key_id
            .read()
            .map_err(|_| poisoned())?
            .get(&key_id)
            .copied();
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = match handle {
            Some(h) => slots.get(&h),
            // Fallback: slot with a matching key_id (self/same-handle test).
            None => slots.values().find(|m| m.transformation_key_id == key_id),
        }
        .ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: no key for SRTPS transformation_key_id",
            )
        })?;
        let body = mat.open_rtps_cyclone(&ch, &ct, &tag, &rtps_header)?;
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
        let mut out = rtps_header.clone();
        out.extend_from_slice(stripped);
        Ok(out)
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

    /// §8.5.1.9.4 / §9.5.3.3.1 `decode_serialized_payload`. Counterpart to
    /// [`Self::encode_serialized_payload`]; key via the `transformation_key_id`
    /// in the CryptoHeader (index `remote_by_key_id`).
    fn decode_serialized_payload(&self, encoded: &[u8]) -> SecurityResult<Vec<u8>> {
        if encoded.len() < 20 + 4 + 16 + 4 {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: encoded serialized_payload too short",
            ));
        }
        let mut key_id = [0u8; 4];
        key_id.copy_from_slice(&encoded[4..8]);
        let handle = self
            .remote_by_key_id
            .read()
            .map_err(|_| poisoned())?
            .get(&key_id)
            .copied()
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: no remote key for serialized_payload key_id",
                )
            })?;
        self.decode_serialized_payload_with(handle, encoded)
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
            (
                &encoded[24..24 + ct_len],
                &encoded[24 + ct_len..24 + ct_len + 16],
            )
        } else {
            let n = encoded.len();
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

    fn encrypt_submessage_multi(
        &self,
        local: CryptoHandle,
        receivers: &[(CryptoHandle, u32)],
        plaintext: &[u8],
        aad_extension: &[u8],
    ) -> SecurityResult<(Vec<u8>, Vec<ReceiverMac>)> {
        let handles: Vec<CryptoHandle> = receivers.iter().map(|(h, _)| *h).collect();
        let ciphertext = self.encrypt_submessage(local, &handles, plaintext, aad_extension)?;

        // Form a truncated HMAC-SHA256 over the ciphertext per remote.
        // The HMAC key is the per-receiver-slot master_key.
        let slots = self.slots.read().map_err(|_| poisoned())?;
        // Spec §9.5.3.3.4: the ReceiverSpecificSessionKey uses the
        // session_id of the COMMON submessage session — it is in the
        // crypto header (= the first 4 bytes of the nonce, see `next_nonce`),
        // NOT in the per-receiver KeyMaterial. The exchanged cyclone
        // KeyMaterial (§9.5.2.1.1) carries NO session_id; pulling it from the
        // received slot diverges sender- vs. receiver-side
        // (sender copy sid=0 from the token, receiver slot sid=random). Both
        // sides MUST use the wire session_id.
        let wire_session_id: [u8; 4] = ciphertext
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: ciphertext too short for session_id header",
                )
            })?;
        let mut macs = Vec::with_capacity(receivers.len());
        for (remote, key_id) in receivers {
            let mat = slots.get(remote).ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "crypto: unknown remote handle for receiver-specific mac",
                )
            })?;
            // Receiver-specific key: master_recv_key/master_salt from the
            // per-receiver slot (round-trips correctly via the token), session_id
            // from the wire (common session) — so sender + all
            // receivers agree.
            let receiver_session_key = crate::session_key::derive_session_hmac_key(
                &mat.master_key,
                &mat.master_salt,
                &wire_session_id,
            );
            let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, &receiver_session_key);
            let tag = hmac::sign(&hmac_key, &ciphertext);
            // 16-byte truncation (spec §7.3.6.3 `receiver_mac octet[16]`).
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
            // Single-MAC path: no multi-MAC in the datagram → normal
            // decrypt path with the same AAD extension.
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

        // Verify: HMAC(own_receiver_key, ciphertext) against our_mac.
        let slots = self.slots.read().map_err(|_| poisoned())?;
        let mat = slots.get(&own_mac_key_handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "crypto: unknown own_mac_key_handle",
            )
        })?;
        // Spec §9.5.3.3.4 — receiver side: same derivation as in the sender
        // (encrypt_submessage_multi). session_id from the wire header
        // (ciphertext[0..4]), NOT from our own slot — the exchanged
        // KeyMaterial carries no session_id, the local slot value would
        // diverge sender- vs. receiver-side.
        let wire_session_id: [u8; 4] = ciphertext
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::CryptoFailed,
                    "crypto: ciphertext too short for session_id header",
                )
            })?;
        let receiver_session_key = crate::session_key::derive_session_hmac_key(
            &mat.master_key,
            &mat.master_salt,
            &wire_session_id,
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

        // MAC matches → normal decrypt path with the sender key (same
        // AAD extension the sender used).
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

    #[test]
    fn cyclone_volatile_submessage_roundtrips() {
        // VolatileSecure-Kx-Key (cyclone calculate_kx_keys) + cyclone-Wire-Format
        // seal -> assemble(SEC_PREFIX/BODY/POSTFIX) -> parse -> open.
        let mat = KeyMaterial::from_kx_shared_secret(
            b"a-32-byte-handshake-shared-secret",
            &[0x11; 32],
            &[0x22; 32],
        );
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

        // Tampering im Ciphertext -> Tag-Mismatch.
        let mut bad = ct.clone();
        bad[0] ^= 0xFF;
        assert!(mat.open_cyclone(&header, &bad, &tag).is_err());
    }

    #[test]
    fn cyclone_kx_key_matches_both_directions() {
        // Both peers derive the same Kx key (symmetric) -> A seals, B opens.
        let secret = b"shared-secret-from-ecdh-handshake";
        let (c1, c2) = ([0xAAu8; 32], [0xBBu8; 32]);
        let a = KeyMaterial::from_kx_shared_secret(secret, &c1, &c2);
        let b = KeyMaterial::from_kx_shared_secret(secret, &c1, &c2);
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
        // Wire format: nonce(12) + plaintext + hmac(32). The plaintext
        // must be present **unencrypted**.
        assert!(
            signed.windows(plain.len()).any(|w| w == plain),
            "HMAC suite should NOT encrypt plaintext"
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
        // Flip byte in plaintext (after nonce, before tag).
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
        assert_eq!(m1.session_id, m2.session_id);
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
}
