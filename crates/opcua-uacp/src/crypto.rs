// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Secured SecureChannel cryptography (OPC-UA Part 6 §6.7) — the secured
//! SecurityPolicies on top of the plaintext `None` framing in
//! [`crate::securechannel`]. Feature `crypto`.
//!
//! Implements, per Part 7 SecurityPolicies:
//!
//! - **`Basic256Sha256`** — asym RSA-OAEP(SHA1) + RSA-PKCS#1-v1.5(SHA256),
//!   sym AES-256-CBC + HMAC-SHA256, key derivation P-SHA256.
//! - **`Aes128_Sha256_RsaOaep`** — as above but AES-128-CBC.
//! - **`Aes256_Sha256_RsaPss`** — asym RSA-OAEP(SHA256) + RSA-PSS(SHA256),
//!   sym AES-256-CBC + HMAC-SHA256.
//!
//! The chunk security layout (§6.7.2): the region from the `SequenceHeader`
//! onward is `SequenceHeader || Body || Padding || Signature`; the signature
//! covers everything from the `MessageHeader` through the padding (computed on
//! the plaintext), then that region is encrypted. `Sign` mode keeps the
//! plaintext and appends the signature without padding/encryption.
//!
//! Key derivation (§6.7.6): `P_SHA256(secret = remoteNonce, seed = localNonce)`
//! yields `SigningKey || EncryptingKey || InitializationVector`.

use alloc::vec::Vec;

use aes::cipher::block_padding::NoPadding;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use rsa::rand_core::CryptoRng;
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, Pkcs1v15Sign};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use zerodds_opcua_pubsub::{UaReader, UaWriter};

use crate::connection::{ChunkType, MessageHeader, MessageType};

/// Re-exports for configuring secured Client/Server endpoints (the RSA key
/// types and the caller-supplied RNG trait).
pub use rsa::rand_core::{CryptoRngCore, RngCore};
pub use rsa::{RsaPrivateKey, RsaPublicKey};

// Note: `RsaPrivateKey`/`RsaPublicKey` are used internally too (via the
// re-export above).

type HmacSha256 = Hmac<Sha256>;
type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Symmetric AES-CBC block size (and IV length) in bytes.
pub const CBC_BLOCK: usize = 16;
/// HMAC-SHA256 signature length in bytes.
pub const HMAC_SHA256_LEN: usize = 32;

/// An error from a SecureChannel cryptographic operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// A key/IV had the wrong length for the cipher.
    BadKeyLength,
    /// RSA encryption/decryption failed.
    Rsa,
    /// A signature did not verify.
    BadSignature,
    /// The chunk was malformed (truncated, bad padding, wrong size).
    Malformed(&'static str),
}

impl core::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadKeyLength => write!(f, "bad key/IV length"),
            Self::Rsa => write!(f, "RSA operation failed"),
            Self::BadSignature => write!(f, "signature verification failed"),
            Self::Malformed(m) => write!(f, "malformed secured chunk: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CryptoError {}

// ---------------------------------------------------------------------------
// SecurityPolicy.
// ---------------------------------------------------------------------------

/// A secured OPC-UA SecurityPolicy (Part 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityPolicy {
    /// `Basic256Sha256`: AES-256-CBC, RSA-OAEP(SHA1), RSA-PKCS#1-v1.5(SHA256).
    Basic256Sha256,
    /// `Aes128_Sha256_RsaOaep`: AES-128-CBC, RSA-OAEP(SHA1), RSA-PKCS#1-v1.5.
    Aes128Sha256RsaOaep,
    /// `Aes256_Sha256_RsaPss`: AES-256-CBC, RSA-OAEP(SHA256), RSA-PSS(SHA256).
    Aes256Sha256RsaPss,
}

impl SecurityPolicy {
    /// The SecurityPolicy URI.
    #[must_use]
    pub const fn uri(self) -> &'static str {
        match self {
            Self::Basic256Sha256 => "http://opcfoundation.org/UA/SecurityPolicy#Basic256Sha256",
            Self::Aes128Sha256RsaOaep => {
                "http://opcfoundation.org/UA/SecurityPolicy#Aes128_Sha256_RsaOaep"
            }
            Self::Aes256Sha256RsaPss => {
                "http://opcfoundation.org/UA/SecurityPolicy#Aes256_Sha256_RsaPss"
            }
        }
    }

    /// Parses a SecurityPolicy from its URI.
    #[must_use]
    pub fn from_uri(uri: &str) -> Option<Self> {
        [
            Self::Basic256Sha256,
            Self::Aes128Sha256RsaOaep,
            Self::Aes256Sha256RsaPss,
        ]
        .into_iter()
        .find(|p| p.uri() == uri)
    }

    /// Symmetric encryption key length in bytes (AES-128 = 16, AES-256 = 32).
    #[must_use]
    pub const fn sym_enc_key_len(self) -> usize {
        match self {
            Self::Aes128Sha256RsaOaep => 16,
            Self::Basic256Sha256 | Self::Aes256Sha256RsaPss => 32,
        }
    }

    /// Symmetric signing (HMAC) key length in bytes.
    #[must_use]
    pub const fn sym_sig_key_len(self) -> usize {
        HMAC_SHA256_LEN
    }

    /// `true` if the asymmetric encryption uses RSA-OAEP with SHA-256 (else
    /// SHA-1).
    #[must_use]
    pub const fn oaep_sha256(self) -> bool {
        matches!(self, Self::Aes256Sha256RsaPss)
    }

    /// `true` if the asymmetric signature uses RSA-PSS (else RSA-PKCS#1-v1.5).
    #[must_use]
    pub const fn asym_pss(self) -> bool {
        matches!(self, Self::Aes256Sha256RsaPss)
    }

    /// RSA-OAEP plaintext overhead in bytes (`2*hashLen + 2`).
    #[must_use]
    pub const fn oaep_overhead(self) -> usize {
        if self.oaep_sha256() { 66 } else { 42 }
    }
}

// ---------------------------------------------------------------------------
// Key derivation (§6.7.6).
// ---------------------------------------------------------------------------

/// Derived symmetric keys for one direction (§6.7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedKeys {
    /// HMAC signing key.
    pub signing: Vec<u8>,
    /// AES encryption key.
    pub encrypting: Vec<u8>,
    /// AES-CBC initialization vector.
    pub iv: Vec<u8>,
}

fn hmac(secret: &[u8], data: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut m =
        <HmacSha256 as Mac>::new_from_slice(secret).map_err(|_| CryptoError::BadKeyLength)?;
    m.update(data);
    Ok(m.finalize().into_bytes().into())
}

/// The P-SHA256 pseudo-random function (RFC 5246 §5 / OPC-UA §6.7.6).
///
/// # Errors
/// [`CryptoError::BadKeyLength`] only if the HMAC key is unusable (never for a
/// nonce).
pub fn p_sha256(secret: &[u8], seed: &[u8], out_len: usize) -> Result<Vec<u8>, CryptoError> {
    let mut out = Vec::with_capacity(out_len + 32);
    // A(1) = HMAC(secret, seed).
    let mut a = hmac(secret, seed)?;
    while out.len() < out_len {
        // output += HMAC(secret, A(i) || seed)
        let mut m =
            <HmacSha256 as Mac>::new_from_slice(secret).map_err(|_| CryptoError::BadKeyLength)?;
        m.update(&a);
        m.update(seed);
        out.extend_from_slice(&m.finalize().into_bytes());
        // A(i+1) = HMAC(secret, A(i))
        a = hmac(secret, &a)?;
    }
    out.truncate(out_len);
    Ok(out)
}

/// Derives the keys used to protect messages this endpoint **sends to** /
/// **receives from** the peer.
///
/// Per §6.7.6 a sender derives its keys with `secret = remoteNonce`,
/// `seed = localNonce`; the receiver mirrors this. Returns `SigningKey`,
/// `EncryptingKey`, `IV` for `secret`/`seed`.
///
/// # Errors
/// [`CryptoError`] only on an unusable HMAC key (never for a nonce).
pub fn derive_keys(
    policy: SecurityPolicy,
    secret_nonce: &[u8],
    seed_nonce: &[u8],
) -> Result<DerivedKeys, CryptoError> {
    let sig = policy.sym_sig_key_len();
    let enc = policy.sym_enc_key_len();
    let total = sig + enc + CBC_BLOCK;
    let block = p_sha256(secret_nonce, seed_nonce, total)?;
    Ok(DerivedKeys {
        signing: block[..sig].to_vec(),
        encrypting: block[sig..sig + enc].to_vec(),
        iv: block[sig + enc..].to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Symmetric primitives.
// ---------------------------------------------------------------------------

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    match key.len() {
        16 => Ok(Aes128CbcEnc::new_from_slices(key, iv)
            .map_err(|_| CryptoError::BadKeyLength)?
            .encrypt_padded_vec_mut::<NoPadding>(data)),
        32 => Ok(Aes256CbcEnc::new_from_slices(key, iv)
            .map_err(|_| CryptoError::BadKeyLength)?
            .encrypt_padded_vec_mut::<NoPadding>(data)),
        _ => Err(CryptoError::BadKeyLength),
    }
}

fn aes_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    match key.len() {
        16 => Aes128CbcDec::new_from_slices(key, iv)
            .map_err(|_| CryptoError::BadKeyLength)?
            .decrypt_padded_vec_mut::<NoPadding>(data)
            .map_err(|_| CryptoError::Malformed("AES-CBC block misalignment")),
        32 => Aes256CbcDec::new_from_slices(key, iv)
            .map_err(|_| CryptoError::BadKeyLength)?
            .decrypt_padded_vec_mut::<NoPadding>(data)
            .map_err(|_| CryptoError::Malformed("AES-CBC block misalignment")),
        _ => Err(CryptoError::BadKeyLength),
    }
}

fn hmac_sign(key: &[u8], data: &[u8]) -> Result<[u8; 32], CryptoError> {
    hmac(key, data)
}

fn hmac_verify(key: &[u8], data: &[u8], sig: &[u8]) -> Result<(), CryptoError> {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).map_err(|_| CryptoError::BadKeyLength)?;
    m.update(data);
    m.verify_slice(sig).map_err(|_| CryptoError::BadSignature)
}

// ---------------------------------------------------------------------------
// Asymmetric primitives.
// ---------------------------------------------------------------------------

/// RSA-OAEP encrypts `plaintext` (already a multiple of the plaintext block
/// size) to the peer's public key, block by block.
fn rsa_oaep_encrypt<R: RngCore + CryptoRng>(
    rng: &mut R,
    pubkey: &RsaPublicKey,
    sha256: bool,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let plain_block = pubkey.size() - if sha256 { 66 } else { 42 };
    let mut out = Vec::new();
    for chunk in plaintext.chunks(plain_block) {
        let ct = if sha256 {
            pubkey.encrypt(rng, Oaep::new::<Sha256>(), chunk)
        } else {
            pubkey.encrypt(rng, Oaep::new::<Sha1>(), chunk)
        }
        .map_err(|_| CryptoError::Rsa)?;
        out.extend_from_slice(&ct);
    }
    Ok(out)
}

/// RSA-OAEP decrypts `ciphertext` (a multiple of the key size) with our private
/// key, block by block.
fn rsa_oaep_decrypt(
    privkey: &RsaPrivateKey,
    sha256: bool,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = privkey.size();
    if ciphertext.is_empty() || ciphertext.len() % key_bytes != 0 {
        return Err(CryptoError::Malformed(
            "RSA ciphertext not a key-size multiple",
        ));
    }
    let mut out = Vec::new();
    for block in ciphertext.chunks(key_bytes) {
        let pt = if sha256 {
            privkey.decrypt(Oaep::new::<Sha256>(), block)
        } else {
            privkey.decrypt(Oaep::new::<Sha1>(), block)
        }
        .map_err(|_| CryptoError::Rsa)?;
        out.extend_from_slice(&pt);
    }
    Ok(out)
}

/// RSA signs `data` (RSA-PSS or RSA-PKCS#1-v1.5, both over SHA-256).
fn rsa_sign<R: RngCore + CryptoRng>(
    rng: &mut R,
    privkey: &RsaPrivateKey,
    pss: bool,
    data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if pss {
        use rsa::pss::BlindedSigningKey;
        use rsa::signature::{RandomizedSigner, SignatureEncoding};
        let sk = BlindedSigningKey::<Sha256>::new(privkey.clone());
        Ok(sk.sign_with_rng(rng, data).to_vec())
    } else {
        let digest = Sha256::digest(data);
        privkey
            .sign(Pkcs1v15Sign::new::<Sha256>(), &digest)
            .map_err(|_| CryptoError::Rsa)
    }
}

/// RSA verifies a signature over `data`.
fn rsa_verify(
    pubkey: &RsaPublicKey,
    pss: bool,
    data: &[u8],
    sig: &[u8],
) -> Result<(), CryptoError> {
    if pss {
        use rsa::pss::{Signature, VerifyingKey};
        use rsa::signature::Verifier;
        let vk = VerifyingKey::<Sha256>::new(pubkey.clone());
        let signature = Signature::try_from(sig).map_err(|_| CryptoError::BadSignature)?;
        vk.verify(data, &signature)
            .map_err(|_| CryptoError::BadSignature)
    } else {
        let digest = Sha256::digest(data);
        pubkey
            .verify(Pkcs1v15Sign::new::<Sha256>(), &digest, sig)
            .map_err(|_| CryptoError::BadSignature)
    }
}

/// The SHA-1 thumbprint of a DER certificate (used in the
/// `AsymmetricAlgorithmSecurityHeader.ReceiverCertificateThumbprint`).
#[must_use]
pub fn sha1_thumbprint(der_certificate: &[u8]) -> Vec<u8> {
    Sha1::digest(der_certificate).to_vec()
}

// ---------------------------------------------------------------------------
// Symmetric chunk security (MSG / CLO).
// ---------------------------------------------------------------------------

/// Whether a chunk is signed only, or signed and encrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecuredMode {
    /// `MessageSecurityMode::Sign` — signature appended, no encryption/padding.
    Sign,
    /// `MessageSecurityMode::SignAndEncrypt` — padded, signed, then encrypted.
    SignAndEncrypt,
}

/// Builds a fully secured symmetric (`MSG`/`CLO`) chunk:
/// `MessageHeader || ChannelId || TokenId || protect(SequenceHeader || Body)`.
///
/// `keys` are this endpoint's **send** keys. The symmetric signature is always
/// HMAC-SHA256 across the secured policies, so no policy argument is needed.
///
/// # Errors
/// [`CryptoError`] on a bad key/IV length.
#[allow(clippy::too_many_arguments)]
pub fn build_symmetric_chunk(
    mode: SecuredMode,
    keys: &DerivedKeys,
    message_type: MessageType,
    channel_id: u32,
    token_id: u32,
    sequence_number: u32,
    request_id: u32,
    body: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let sig_len = HMAC_SHA256_LEN;

    // Plaintext region after the headers: SequenceHeader || Body.
    let mut plain = Vec::with_capacity(8 + body.len());
    plain.extend_from_slice(&sequence_number.to_le_bytes());
    plain.extend_from_slice(&request_id.to_le_bytes());
    plain.extend_from_slice(body);

    // Padding (SignAndEncrypt only): make SeqHeader+Body+1+pad+sig a block
    // multiple.
    let padding = match mode {
        SecuredMode::Sign => Vec::new(),
        SecuredMode::SignAndEncrypt => {
            let pre = plain.len() + 1 + sig_len;
            let pad_count = (CBC_BLOCK - (pre % CBC_BLOCK)) % CBC_BLOCK;
            let pad_byte = u8::try_from(pad_count).map_err(|_| CryptoError::BadKeyLength)?;
            // PaddingSize byte (== pad_byte) followed by pad_count bytes (each
            // == pad_byte): all 1 + pad_count bytes hold the same value.
            alloc::vec![pad_byte; 1 + pad_count]
        }
    };

    // MessageSize is known now (CBC preserves length).
    let signed_len = 8 + 4 + 4 + plain.len() + padding.len();
    let message_size = u32::try_from(signed_len + sig_len)
        .map_err(|_| CryptoError::Malformed("message too large"))?;

    let mut header = Vec::with_capacity(16);
    {
        let mut w = UaWriter::new();
        MessageHeader {
            message_type,
            chunk_type: ChunkType::Final,
            message_size,
        }
        .encode(&mut w);
        header.extend_from_slice(w.as_slice());
    }
    header.extend_from_slice(&channel_id.to_le_bytes());
    header.extend_from_slice(&token_id.to_le_bytes());

    // Signature over MessageHeader..Padding (plaintext).
    let mut signed = Vec::with_capacity(signed_len);
    signed.extend_from_slice(&header);
    signed.extend_from_slice(&plain);
    signed.extend_from_slice(&padding);
    let signature = hmac_sign(&keys.signing, &signed)?;

    let mut out = Vec::with_capacity(message_size as usize);
    out.extend_from_slice(&header);
    match mode {
        SecuredMode::Sign => {
            out.extend_from_slice(&plain);
            out.extend_from_slice(&signature);
        }
        SecuredMode::SignAndEncrypt => {
            let mut region = Vec::with_capacity(plain.len() + padding.len() + sig_len);
            region.extend_from_slice(&plain);
            region.extend_from_slice(&padding);
            region.extend_from_slice(&signature);
            let ct = aes_cbc_encrypt(&keys.encrypting, &keys.iv, &region)?;
            out.extend_from_slice(&ct);
        }
    }
    Ok(out)
}

/// The recovered plaintext of a secured chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedChunk {
    /// Message type.
    pub message_type: MessageType,
    /// SecureChannel id.
    pub channel_id: u32,
    /// Token id (symmetric) — `None` for asymmetric chunks.
    pub token_id: Option<u32>,
    /// Sequence number.
    pub sequence_number: u32,
    /// Request id.
    pub request_id: u32,
    /// The recovered service message body.
    pub body: Vec<u8>,
}

/// Verifies and decrypts a secured symmetric chunk. `keys` are this endpoint's
/// **receive** keys (i.e. the peer's send keys).
///
/// # Errors
/// [`CryptoError`] on a bad signature, bad padding, or truncation.
pub fn open_symmetric_chunk(
    mode: SecuredMode,
    keys: &DerivedKeys,
    bytes: &[u8],
) -> Result<OpenedChunk, CryptoError> {
    if bytes.len() < 16 {
        return Err(CryptoError::Malformed("chunk shorter than headers"));
    }
    let mut r = UaReader::new(bytes);
    let header = MessageHeader::decode(&mut r).map_err(|_| CryptoError::Malformed("bad header"))?;
    let channel_id = u32::from_le_bytes(slice4(bytes, 8)?);
    let token_id = u32::from_le_bytes(slice4(bytes, 12)?);
    let rest = &bytes[16..];
    let sig_len = HMAC_SHA256_LEN;

    let plain = match mode {
        SecuredMode::Sign => {
            if rest.len() < sig_len + 8 {
                return Err(CryptoError::Malformed("sign-only chunk too short"));
            }
            let split = rest.len() - sig_len;
            hmac_verify(&keys.signing, &bytes[..16 + split], &rest[split..])?;
            rest[..split].to_vec()
        }
        SecuredMode::SignAndEncrypt => {
            let mut decrypted = aes_cbc_decrypt(&keys.encrypting, &keys.iv, rest)?;
            if decrypted.len() < sig_len + 1 + 8 {
                return Err(CryptoError::Malformed("encrypted chunk too short"));
            }
            let sig_start = decrypted.len() - sig_len;
            // Verify over MessageHeader..Padding (everything before signature).
            let mut signed = Vec::with_capacity(16 + sig_start);
            signed.extend_from_slice(&bytes[..16]);
            signed.extend_from_slice(&decrypted[..sig_start]);
            hmac_verify(&keys.signing, &signed, &decrypted[sig_start..])?;
            // Strip signature + padding.
            let pad_byte = decrypted[sig_start - 1] as usize;
            if sig_start < 1 + pad_byte + 8 {
                return Err(CryptoError::Malformed("padding larger than payload"));
            }
            decrypted.truncate(sig_start - 1 - pad_byte);
            decrypted
        }
    };

    if plain.len() < 8 {
        return Err(CryptoError::Malformed("missing SequenceHeader"));
    }
    Ok(OpenedChunk {
        message_type: header.message_type,
        channel_id,
        token_id: Some(token_id),
        sequence_number: u32::from_le_bytes(slice4(&plain, 0)?),
        request_id: u32::from_le_bytes(slice4(&plain, 4)?),
        body: plain[8..].to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Asymmetric chunk security (OPN).
// ---------------------------------------------------------------------------

/// The asymmetric (`OPN`) crypto material for one direction.
pub struct AsymmetricContext<'a> {
    /// SecurityPolicy.
    pub policy: SecurityPolicy,
    /// Our DER application certificate (sent in the header).
    pub sender_certificate: &'a [u8],
    /// Our RSA private key (signs the request, decrypts the response).
    pub sender_private_key: &'a RsaPrivateKey,
    /// The peer's DER certificate (its thumbprint goes in the header).
    pub receiver_certificate: &'a [u8],
    /// The peer's RSA public key (encrypts to it).
    pub receiver_public_key: &'a RsaPublicKey,
}

/// Builds a secured asymmetric (`OPN`) chunk: header + asym security header +
/// `RSA-encrypt(SequenceHeader || Body || Padding || RSA-Signature)`.
///
/// # Errors
/// [`CryptoError`] on an RSA failure.
pub fn build_asymmetric_chunk<R: RngCore + CryptoRng>(
    rng: &mut R,
    ctx: &AsymmetricContext<'_>,
    channel_id: u32,
    sequence_number: u32,
    request_id: u32,
    body: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = ctx.receiver_public_key.size();
    let plain_block = key_bytes - ctx.policy.oaep_overhead();
    let sig_len = ctx.sender_private_key.to_public_key().size();
    let extra = usize::from(key_bytes > 256); // ExtraPaddingByte for keys > 2048 bit

    // SecurityHeader.
    let mut sec_header = Vec::new();
    {
        let mut w = UaWriter::new();
        write_string(&mut w, ctx.policy.uri());
        write_byte_string(&mut w, ctx.sender_certificate);
        write_byte_string(&mut w, &sha1_thumbprint(ctx.receiver_certificate));
        sec_header.extend_from_slice(w.as_slice());
    }

    // Plaintext: SequenceHeader || Body.
    let mut plain = Vec::with_capacity(8 + body.len());
    plain.extend_from_slice(&sequence_number.to_le_bytes());
    plain.extend_from_slice(&request_id.to_le_bytes());
    plain.extend_from_slice(body);

    // Padding so that (plain + 1 + extra + pad + signature) % plain_block == 0.
    let pre = plain.len() + 1 + extra + sig_len;
    let pad_count = (plain_block - (pre % plain_block)) % plain_block;
    let low = u8::try_from(pad_count & 0xFF).map_err(|_| CryptoError::BadKeyLength)?;
    // PaddingSize byte + pad_count padding bytes, all == low.
    let mut padding = alloc::vec![low; 1 + pad_count];
    if extra == 1 {
        padding.push(u8::try_from(pad_count >> 8).map_err(|_| CryptoError::BadKeyLength)?);
    }

    let cipher_input_len = plain.len() + padding.len() + sig_len;
    let num_blocks = cipher_input_len / plain_block;
    let cipher_len = num_blocks * key_bytes;
    let message_size = u32::try_from(8 + 4 + sec_header.len() + cipher_len)
        .map_err(|_| CryptoError::Malformed("OPN too large"))?;

    let mut header = Vec::new();
    {
        let mut w = UaWriter::new();
        MessageHeader {
            message_type: MessageType::OpenSecureChannel,
            chunk_type: ChunkType::Final,
            message_size,
        }
        .encode(&mut w);
        header.extend_from_slice(w.as_slice());
    }
    header.extend_from_slice(&channel_id.to_le_bytes());
    header.extend_from_slice(&sec_header);

    // Signature over MessageHeader || ChannelId || SecHeader || Plain || Pad.
    let mut signed = Vec::with_capacity(header.len() + plain.len() + padding.len());
    signed.extend_from_slice(&header);
    signed.extend_from_slice(&plain);
    signed.extend_from_slice(&padding);
    let signature = rsa_sign(rng, ctx.sender_private_key, ctx.policy.asym_pss(), &signed)?;

    // Encrypt SequenceHeader..Signature with the receiver's public key.
    let mut region = Vec::with_capacity(cipher_input_len);
    region.extend_from_slice(&plain);
    region.extend_from_slice(&padding);
    region.extend_from_slice(&signature);
    let ciphertext = rsa_oaep_encrypt(
        rng,
        ctx.receiver_public_key,
        ctx.policy.oaep_sha256(),
        &region,
    )?;

    let mut out = Vec::with_capacity(message_size as usize);
    out.extend_from_slice(&header);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Verifies and decrypts a secured asymmetric (`OPN`) chunk. We decrypt with
/// our own private key and verify with the peer's public key.
///
/// # Errors
/// [`CryptoError`] on an RSA failure, bad signature, or truncation.
pub fn open_asymmetric_chunk(
    policy: SecurityPolicy,
    our_private_key: &RsaPrivateKey,
    sender_public_key: &RsaPublicKey,
    bytes: &[u8],
) -> Result<OpenedChunk, CryptoError> {
    let mut r = UaReader::new(bytes);
    let header = MessageHeader::decode(&mut r).map_err(|_| CryptoError::Malformed("bad header"))?;
    let channel_id = r
        .read_u32()
        .map_err(|_| CryptoError::Malformed("bad channel id"))?;
    // SecurityHeader: SecurityPolicyUri, SenderCertificate, ReceiverThumbprint.
    read_string(&mut r).map_err(|_| CryptoError::Malformed("bad policy uri"))?;
    let _sender_cert = read_byte_string(&mut r).map_err(|_| CryptoError::Malformed("bad cert"))?;
    let _thumb = read_byte_string(&mut r).map_err(|_| CryptoError::Malformed("bad thumbprint"))?;
    let sec_header_end = bytes.len() - r.remaining();
    let ciphertext = &bytes[sec_header_end..];

    let plaintext = rsa_oaep_decrypt(our_private_key, policy.oaep_sha256(), ciphertext)?;
    let sig_len = sender_public_key.size();
    if plaintext.len() < sig_len + 1 + 8 {
        return Err(CryptoError::Malformed("OPN plaintext too short"));
    }
    let sig_start = plaintext.len() - sig_len;

    // Verify over MessageHeader || ChannelId || SecHeader || Plain || Pad.
    let mut signed = Vec::with_capacity(sec_header_end + sig_start);
    signed.extend_from_slice(&bytes[..sec_header_end]);
    signed.extend_from_slice(&plaintext[..sig_start]);
    rsa_verify(
        sender_public_key,
        policy.asym_pss(),
        &signed,
        &plaintext[sig_start..],
    )?;

    // Strip signature + padding (with optional extra padding byte).
    let extra = usize::from(our_private_key.size() > 256);
    let pad_low = plaintext[sig_start - 1 - extra] as usize;
    let pad_count = if extra == 1 {
        pad_low + ((plaintext[sig_start - 1] as usize) << 8)
    } else {
        pad_low
    };
    if sig_start < 1 + extra + pad_count + 8 {
        return Err(CryptoError::Malformed("OPN padding larger than payload"));
    }
    let body_end = sig_start - 1 - extra - pad_count;

    Ok(OpenedChunk {
        message_type: header.message_type,
        channel_id,
        token_id: None,
        sequence_number: u32::from_le_bytes(slice4(&plaintext, 0)?),
        request_id: u32::from_le_bytes(slice4(&plaintext, 4)?),
        body: plaintext[8..body_end].to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Small local codec helpers (mirroring securechannel.rs).
// ---------------------------------------------------------------------------

fn slice4(b: &[u8], at: usize) -> Result<[u8; 4], CryptoError> {
    b.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or(CryptoError::Malformed("truncated u32"))
}

fn write_string(w: &mut UaWriter, s: &str) {
    w.write_i32(i32::try_from(s.len()).unwrap_or(-1));
    w.write_bytes(s.as_bytes());
}

fn write_byte_string(w: &mut UaWriter, b: &[u8]) {
    if b.is_empty() {
        w.write_i32(-1);
    } else {
        w.write_i32(i32::try_from(b.len()).unwrap_or(-1));
        w.write_bytes(b);
    }
}

fn read_string(r: &mut UaReader<'_>) -> Result<(), CryptoError> {
    read_byte_string(r).map(|_| ())
}

fn read_byte_string(r: &mut UaReader<'_>) -> Result<Vec<u8>, CryptoError> {
    let len = r
        .read_i32()
        .map_err(|_| CryptoError::Malformed("truncated length"))?;
    if len < 0 {
        return Ok(Vec::new());
    }
    Ok(r.read_bytes(len as usize)
        .map_err(|_| CryptoError::Malformed("truncated bytes"))?
        .to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic SHA-256-CTR RNG for reproducible tests (NOT for
    /// production randomness).
    struct TestRng {
        seed: [u8; 32],
        ctr: u64,
    }
    impl TestRng {
        fn new(tag: u8) -> Self {
            Self {
                seed: [tag; 32],
                ctr: 0,
            }
        }
    }
    impl RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            let mut b = [0u8; 4];
            self.fill_bytes(&mut b);
            u32::from_le_bytes(b)
        }
        fn next_u64(&mut self) -> u64 {
            let mut b = [0u8; 8];
            self.fill_bytes(&mut b);
            u64::from_le_bytes(b)
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(32) {
                let mut h = Sha256::new();
                h.update(self.seed);
                h.update(self.ctr.to_le_bytes());
                let d = h.finalize();
                chunk.copy_from_slice(&d[..chunk.len()]);
                self.ctr += 1;
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }
    impl CryptoRng for TestRng {}

    #[test]
    fn p_sha256_is_deterministic_and_sized() {
        let a = p_sha256(b"secret", b"seed", 64).unwrap();
        let b = p_sha256(b"secret", b"seed", 64).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        // Different seed → different output; prefix-stability of P_hash.
        assert_ne!(a, p_sha256(b"secret", b"seed2", 64).unwrap());
        assert_eq!(p_sha256(b"secret", b"seed", 32).unwrap(), a[..32].to_vec());
    }

    #[test]
    fn derive_keys_lengths_and_mirror() {
        let client_nonce = [1u8; 32];
        let server_nonce = [2u8; 32];
        // Client send keys = derive(secret=serverNonce, seed=clientNonce).
        let client_send =
            derive_keys(SecurityPolicy::Basic256Sha256, &server_nonce, &client_nonce).unwrap();
        // Server receive keys = derive(secret=serverNonce, seed=clientNonce) — same.
        let server_recv =
            derive_keys(SecurityPolicy::Basic256Sha256, &server_nonce, &client_nonce).unwrap();
        assert_eq!(client_send, server_recv);
        assert_eq!(client_send.signing.len(), 32);
        assert_eq!(client_send.encrypting.len(), 32);
        assert_eq!(client_send.iv.len(), 16);

        let aes128 = derive_keys(
            SecurityPolicy::Aes128Sha256RsaOaep,
            &server_nonce,
            &client_nonce,
        )
        .unwrap();
        assert_eq!(aes128.encrypting.len(), 16);
    }

    fn sym_round_trip(policy: SecurityPolicy, mode: SecuredMode) {
        let keys = derive_keys(policy, &[7u8; 32], &[9u8; 32]).unwrap();
        let body = b"\x01\x02service-payload-of-some-length\xAA\xBB";
        let chunk = build_symmetric_chunk(mode, &keys, MessageType::Message, 42, 7, 1, 99, body)
            .expect("build");
        // MessageSize header matches the actual length.
        let declared = u32::from_le_bytes(slice4(&chunk, 4).unwrap()) as usize;
        assert_eq!(declared, chunk.len());

        let opened = open_symmetric_chunk(mode, &keys, &chunk).expect("open");
        assert_eq!(opened.channel_id, 42);
        assert_eq!(opened.token_id, Some(7));
        assert_eq!(opened.sequence_number, 1);
        assert_eq!(opened.request_id, 99);
        assert_eq!(opened.body, body);
    }

    #[test]
    fn symmetric_sign_and_encrypt_round_trip() {
        sym_round_trip(SecurityPolicy::Basic256Sha256, SecuredMode::SignAndEncrypt);
        sym_round_trip(
            SecurityPolicy::Aes128Sha256RsaOaep,
            SecuredMode::SignAndEncrypt,
        );
        sym_round_trip(
            SecurityPolicy::Aes256Sha256RsaPss,
            SecuredMode::SignAndEncrypt,
        );
    }

    #[test]
    fn symmetric_sign_only_round_trip() {
        sym_round_trip(SecurityPolicy::Basic256Sha256, SecuredMode::Sign);
    }

    #[test]
    fn symmetric_tamper_is_rejected() {
        let policy = SecurityPolicy::Basic256Sha256;
        let keys = derive_keys(policy, &[7u8; 32], &[9u8; 32]).unwrap();
        let mut chunk = build_symmetric_chunk(
            SecuredMode::SignAndEncrypt,
            &keys,
            MessageType::Message,
            1,
            1,
            1,
            1,
            b"payload",
        )
        .expect("build");
        let last = chunk.len() - 1;
        chunk[last] ^= 0xFF;
        assert!(open_symmetric_chunk(SecuredMode::SignAndEncrypt, &keys, &chunk).is_err());
    }

    fn asym_round_trip(policy: SecurityPolicy) {
        let mut rng = TestRng::new(1);
        let client_key = RsaPrivateKey::new(&mut rng, 2048).expect("client key");
        let server_key = RsaPrivateKey::new(&mut rng, 2048).expect("server key");
        let client_cert = b"client-der-cert".to_vec();
        let server_cert = b"server-der-cert".to_vec();
        let server_pub = server_key.to_public_key();
        let client_pub = client_key.to_public_key();

        let ctx = AsymmetricContext {
            policy,
            sender_certificate: &client_cert,
            sender_private_key: &client_key,
            receiver_certificate: &server_cert,
            receiver_public_key: &server_pub,
        };
        let body = b"\x01OpenSecureChannelRequest-body-payload\xFF";
        let mut rng2 = TestRng::new(2);
        let chunk = build_asymmetric_chunk(&mut rng2, &ctx, 0, 1, 55, body).expect("build");
        let declared = u32::from_le_bytes(slice4(&chunk, 4).unwrap()) as usize;
        assert_eq!(declared, chunk.len());

        // Server opens it: decrypt with server key, verify with client pubkey.
        let opened = open_asymmetric_chunk(policy, &server_key, &client_pub, &chunk).expect("open");
        assert_eq!(opened.message_type, MessageType::OpenSecureChannel);
        assert_eq!(opened.token_id, None);
        assert_eq!(opened.sequence_number, 1);
        assert_eq!(opened.request_id, 55);
        assert_eq!(opened.body, body);
    }

    #[test]
    fn asymmetric_round_trip_oaep_pkcs15() {
        asym_round_trip(SecurityPolicy::Basic256Sha256);
        asym_round_trip(SecurityPolicy::Aes128Sha256RsaOaep);
    }

    #[test]
    fn asymmetric_round_trip_oaep_sha256_pss() {
        asym_round_trip(SecurityPolicy::Aes256Sha256RsaPss);
    }

    #[test]
    fn asymmetric_wrong_signer_is_rejected() {
        let policy = SecurityPolicy::Basic256Sha256;
        let mut rng = TestRng::new(3);
        let client_key = RsaPrivateKey::new(&mut rng, 2048).expect("client");
        let server_key = RsaPrivateKey::new(&mut rng, 2048).expect("server");
        let attacker = RsaPrivateKey::new(&mut rng, 2048).expect("attacker");
        let ctx = AsymmetricContext {
            policy,
            sender_certificate: b"c",
            sender_private_key: &client_key,
            receiver_certificate: b"s",
            receiver_public_key: &server_key.to_public_key(),
        };
        let mut rng2 = TestRng::new(4);
        let chunk = build_asymmetric_chunk(&mut rng2, &ctx, 0, 1, 1, b"body").expect("build");
        // Verifying against the attacker's public key must fail.
        let err = open_asymmetric_chunk(policy, &server_key, &attacker.to_public_key(), &chunk);
        assert_eq!(err, Err(CryptoError::BadSignature));
    }

    #[test]
    fn thumbprint_is_sha1() {
        assert_eq!(sha1_thumbprint(b"abc").len(), 20);
    }

    /// Ties the whole secured handshake together: an asymmetric OPN carries the
    /// nonces, both sides derive the §6.7.6 keys, then symmetric MSGs flow both
    /// ways — exactly the sequence a secured Client/Server performs.
    #[test]
    fn full_secured_handshake_flow() {
        let policy = SecurityPolicy::Basic256Sha256;
        let mut kg = TestRng::new(10);
        let client_key = RsaPrivateKey::new(&mut kg, 2048).expect("client key");
        let server_key = RsaPrivateKey::new(&mut kg, 2048).expect("server key");
        let client_cert = b"client-cert".to_vec();
        let server_cert = b"server-cert".to_vec();
        let client_nonce = [0x11u8; 32];
        let server_nonce = [0x22u8; 32];

        // 1. Client → OPN (carries client_nonce as the "body" here).
        let ctx = AsymmetricContext {
            policy,
            sender_certificate: &client_cert,
            sender_private_key: &client_key,
            receiver_certificate: &server_cert,
            receiver_public_key: &server_key.to_public_key(),
        };
        let mut r = TestRng::new(11);
        let opn = build_asymmetric_chunk(&mut r, &ctx, 0, 1, 1, &client_nonce).expect("opn");

        // 2. Server opens OPN, recovers client_nonce.
        let opened = open_asymmetric_chunk(policy, &server_key, &client_key.to_public_key(), &opn)
            .expect("open opn");
        assert_eq!(opened.body, client_nonce);

        // 3. Both derive the two key sets (§6.7.6).
        // client→server: secret=serverNonce seed=clientNonce.
        let c2s_send = derive_keys(policy, &server_nonce, &client_nonce).unwrap();
        let c2s_recv = derive_keys(policy, &server_nonce, &client_nonce).unwrap();
        // server→client: secret=clientNonce seed=serverNonce.
        let s2c_send = derive_keys(policy, &client_nonce, &server_nonce).unwrap();
        let s2c_recv = derive_keys(policy, &client_nonce, &server_nonce).unwrap();

        // 4. Client sends a secured MSG; server opens it with its recv keys.
        let req = b"\x01ReadRequest-payload";
        let msg = build_symmetric_chunk(
            SecuredMode::SignAndEncrypt,
            &c2s_send,
            MessageType::Message,
            1,
            7,
            1,
            42,
            req,
        )
        .expect("msg");
        let got =
            open_symmetric_chunk(SecuredMode::SignAndEncrypt, &c2s_recv, &msg).expect("srv open");
        assert_eq!(got.body, req);
        assert_eq!(got.request_id, 42);

        // 5. Server replies with a secured MSG; client opens it.
        let resp = b"\x01ReadResponse-payload";
        let reply = build_symmetric_chunk(
            SecuredMode::SignAndEncrypt,
            &s2c_send,
            MessageType::Message,
            1,
            7,
            1,
            42,
            resp,
        )
        .expect("reply");
        let back =
            open_symmetric_chunk(SecuredMode::SignAndEncrypt, &s2c_recv, &reply).expect("cli open");
        assert_eq!(back.body, resp);
    }
}
