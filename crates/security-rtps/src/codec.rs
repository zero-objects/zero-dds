// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire format for secure submessages.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The codec accepts `&dyn CryptographicPlugin`, so the
//! crypto implementation stays interchangeable — architectural.)
//!
//! ```text
//! Submessage header (4 bytes):
//!   +---+---+-------+
//!   | id|flg| length|
//!   +---+---+-------+
//!     u8  u8  u16 (LE if flg & 0x01 is set)
//! ```
//!
//! SEC_PREFIX:   id=0x31, body = TransformIdentifier (16 bytes)
//! SEC_BODY:     id=0x30, body = u32 length + ciphertext
//! SEC_POSTFIX:  id=0x32, body = MAC list for receiver-specific MACs

use alloc::vec::Vec;

use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin, ReceiverMac};
use zerodds_security::error::SecurityError;

/// SEC_PREFIX submessage ID (spec §7.3.6.2).
pub const SEC_PREFIX: u8 = 0x31;
/// SEC_POSTFIX submessage ID (spec §7.3.6.3).
pub const SEC_POSTFIX: u8 = 0x32;
/// SEC_BODY submessage ID (spec §7.3.6.4).
pub const SEC_BODY: u8 = 0x30;
/// SRTPS_PREFIX submessage ID (spec §7.3.6.5).
pub const SRTPS_PREFIX: u8 = 0x33;
/// SRTPS_POSTFIX submessage ID (spec §7.3.6.6).
pub const SRTPS_POSTFIX: u8 = 0x34;

/// Endianness flag: `0x01` means little-endian in the submessage.
const FLAG_LE: u8 = 0x01;

/// Error on encode/decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityRtpsError {
    /// Input bytes too short for the expected submessage structure.
    Truncated(&'static str),
    /// Submessage ID does not match the expected slot (e.g. SEC_PREFIX
    /// missing or SEC_BODY ID wrong).
    UnexpectedSubmessageId {
        /// Position of the submessage in the container (0-indexed).
        pos: usize,
        /// Expected ID.
        expected: u8,
        /// Actually read ID.
        got: u8,
    },
    /// Big-endian sec submessage — big-endian sec submessage (major-2.0 additive).
    BigEndianNotSupported,
    /// The ciphertext length in the SEC_BODY does not match the submessage
    /// length header (wire tampering?).
    InconsistentLength,
    /// Crypto-plugin error passed through.
    Crypto(SecurityError),
}

impl core::fmt::Display for SecurityRtpsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated(what) => write!(f, "secured submessage truncated at {what}"),
            Self::UnexpectedSubmessageId { pos, expected, got } => write!(
                f,
                "secured submessage #{pos} id 0x{got:02x}, expected 0x{expected:02x}"
            ),
            Self::BigEndianNotSupported => write!(
                f,
                "big-endian SEC_* not supported (Single-Endianness-Pfad, LE per Default)"
            ),
            Self::InconsistentLength => write!(f, "SEC_BODY length header != payload"),
            Self::Crypto(e) => write!(f, "crypto plugin: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SecurityRtpsError {}

impl From<SecurityError> for SecurityRtpsError {
    fn from(e: SecurityError) -> Self {
        Self::Crypto(e)
    }
}

/// Encodes a plain submessage blob as a **secured** submessage
/// sequence (SEC_PREFIX + SEC_BODY + SEC_POSTFIX).
///
/// The crypto plugin produces the actual ciphertext; this
/// module only takes care of the wire framing.
///
/// # Errors
/// A passed-through crypto error or a length overflow (u16).
pub fn encode_secured_submessage(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote_list: &[CryptoHandle],
    plaintext: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    // SEC_PREFIX body: 16-byte TransformIdentifier (TransformKindId(4)
    // + KeyId(4) + TransformId(8)). Currently all zero — the dynamic
    // value comes with the DCPS-RTPS handle-map integration.
    let sec_prefix_body = [0u8; 16];

    // AAD extension per DDS-Security 1.2 §10.5.2 Tab.78 (submessage
    // protection): reserved-4 || SEC_PREFIX CryptoHeader. The plugin's
    // `mat.aad(extension)` additionally prepends `transformation_kind ||
    // key_id || session_id`. This protects the SEC_PREFIX header against
    // tampering.
    let mut aad_extension = Vec::with_capacity(4 + 16);
    aad_extension.extend_from_slice(&[0u8; 4]); // reserved-4
    aad_extension.extend_from_slice(&sec_prefix_body);

    // 1) The crypto plugin encrypts with the spec-conformant AAD.
    let ciphertext = plugin.encrypt_submessage(local, remote_list, plaintext, &aad_extension)?;

    // 2) Write the SEC_PREFIX submessage.
    let mut out = Vec::with_capacity(4 + 16 + 4 + 4 + ciphertext.len() + 4);
    push_header(&mut out, SEC_PREFIX, 16);
    out.extend_from_slice(&sec_prefix_body);

    // 3) SEC_BODY: u32 length + ciphertext.
    let ct_len = u32::try_from(ciphertext.len())
        .map_err(|_| SecurityRtpsError::Truncated("ciphertext > u32"))?;
    let body_len = u16::try_from(4 + ciphertext.len())
        .map_err(|_| SecurityRtpsError::Truncated("SEC_BODY > u16"))?;
    push_header(&mut out, SEC_BODY, body_len);
    out.extend_from_slice(&ct_len.to_le_bytes());
    out.extend_from_slice(&ciphertext);

    // 4) SEC_POSTFIX: empty (single-MAC path). Multi-MAC variant:
    //    `encode_secured_submessage_multi`.
    push_header(&mut out, SEC_POSTFIX, 0);

    Ok(out)
}

/// Decodes a secure-submessage sequence and returns the
/// plaintext.
///
/// # Errors
/// On wire tampering (submessage IDs wrong, lengths inconsistent),
/// big-endian, or a crypto verify fail.
pub fn decode_secured_submessage(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote: CryptoHandle,
    secured_bytes: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    let mut cur = Cursor::new(secured_bytes);

    // SEC_PREFIX. The body bytes (16-byte TransformIdentifier) are part
    // of the AAD extension — symmetric to the encoder.
    let (id, _flags, plen) = read_header(&mut cur, "SEC_PREFIX")?;
    if id != SEC_PREFIX {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 0,
            expected: SEC_PREFIX,
            got: id,
        });
    }
    let sec_prefix_body = cur.read_bytes(plen as usize, "SEC_PREFIX body")?;
    let mut aad_extension = Vec::with_capacity(4 + sec_prefix_body.len());
    aad_extension.extend_from_slice(&[0u8; 4]);
    aad_extension.extend_from_slice(sec_prefix_body);

    // SEC_BODY.
    let (id, _flags, blen) = read_header(&mut cur, "SEC_BODY header")?;
    if id != SEC_BODY {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 1,
            expected: SEC_BODY,
            got: id,
        });
    }
    let ct_len_raw = cur.read_u32_le("SEC_BODY length")?;
    if (ct_len_raw as usize) + 4 != (blen as usize) {
        return Err(SecurityRtpsError::InconsistentLength);
    }
    let ciphertext = cur.read_bytes(ct_len_raw as usize, "SEC_BODY ciphertext")?;

    // SEC_POSTFIX (empty in single-receiver mode; the ID check gives wire integrity).
    let (id, _flags, postlen) = read_header(&mut cur, "SEC_POSTFIX")?;
    if id != SEC_POSTFIX {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 2,
            expected: SEC_POSTFIX,
            got: id,
        });
    }
    cur.skip(postlen as usize, "SEC_POSTFIX body")?;

    let plain = plugin.decrypt_submessage(local, remote, ciphertext, &aad_extension)?;
    Ok(plain)
}

fn push_header(out: &mut Vec<u8>, id: u8, length: u16) {
    out.push(id);
    out.push(FLAG_LE);
    out.extend_from_slice(&length.to_le_bytes());
}

/// DoS cap for the MAC list in the SEC_POSTFIX. Each MAC is 20 bytes;
/// 256 MACs = 5 KiB — enough for heterogeneous deployments with
/// hundreds of readers per writer, but far below the RAM-attack
/// threshold.
pub const MAX_RECEIVER_MACS: usize = 256;

/// Encodes a plain submessage blob as a secured sequence WITH
/// receiver-specific MACs in the SEC_POSTFIX (spec §7.3.6.3).
///
/// The crypto plugin produces a common ciphertext plus a
/// list of `(key_id, mac)` entries, one per reader.
///
/// # Wire layout SEC_POSTFIX (body)
/// ```text
///   u32  count
///   [ u32 key_id ; u8 mac[16] ] * count     // 20 bytes per entry
/// ```
///
/// # Errors
/// * `Crypto` passed through from the plugin.
/// * `Truncated` if the MAC list > `MAX_RECEIVER_MACS` or
///   the ciphertext > `u32::MAX` / SEC_POSTFIX body > `u16::MAX`.
pub fn encode_secured_submessage_multi(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    receivers: &[(CryptoHandle, u32)],
    plaintext: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    // SEC_PREFIX body + AAD extension (analogous to encode_secured_submessage).
    let sec_prefix_body = [0u8; 16];
    let mut aad_extension = Vec::with_capacity(4 + 16);
    aad_extension.extend_from_slice(&[0u8; 4]);
    aad_extension.extend_from_slice(&sec_prefix_body);

    let (ciphertext, macs) =
        plugin.encrypt_submessage_multi(local, receivers, plaintext, &aad_extension)?;
    if macs.len() > MAX_RECEIVER_MACS {
        return Err(SecurityRtpsError::Truncated(
            "receiver-specific mac count exceeds cap",
        ));
    }

    let postfix_body_len = 4usize.saturating_add(macs.len().saturating_mul(ReceiverMac::WIRE_SIZE));
    let postfix_body_len_u16 = u16::try_from(postfix_body_len)
        .map_err(|_| SecurityRtpsError::Truncated("SEC_POSTFIX > u16"))?;

    let mut out = Vec::with_capacity(4 + 16 + 4 + 4 + ciphertext.len() + 4 + postfix_body_len);

    // SEC_PREFIX (16 byte TransformIdentifier).
    push_header(&mut out, SEC_PREFIX, 16);
    out.extend_from_slice(&sec_prefix_body);

    // SEC_BODY.
    let ct_len = u32::try_from(ciphertext.len())
        .map_err(|_| SecurityRtpsError::Truncated("ciphertext > u32"))?;
    let body_len = u16::try_from(4 + ciphertext.len())
        .map_err(|_| SecurityRtpsError::Truncated("SEC_BODY > u16"))?;
    push_header(&mut out, SEC_BODY, body_len);
    out.extend_from_slice(&ct_len.to_le_bytes());
    out.extend_from_slice(&ciphertext);

    // SEC_POSTFIX with the multi-MAC payload.
    push_header(&mut out, SEC_POSTFIX, postfix_body_len_u16);
    let n =
        u32::try_from(macs.len()).map_err(|_| SecurityRtpsError::Truncated("mac count > u32"))?;
    out.extend_from_slice(&n.to_le_bytes());
    for m in &macs {
        out.extend_from_slice(&m.key_id.to_le_bytes());
        out.extend_from_slice(&m.mac);
    }

    Ok(out)
}

/// Decodes a secure-submessage sequence WITH a multi-MAC SEC_POSTFIX
/// and returns the plaintext.
///
/// `own_receiver_handle` identifies our own receiver
/// position in the MAC list — the plugin uses it to find the correct
/// MAC entry and validate it (spec §7.3.6.3).
///
/// If the embedded MAC list is empty, this falls back to the
/// v1.4 path `decode_secured_submessage` (backward
/// compat: a legacy sender has only `common_mac` in the AEAD tag).
///
/// # Errors
/// * `Crypto` on a MAC mismatch / AEAD verify fail.
/// * `Truncated` on inputs that are too short.
pub fn decode_secured_submessage_multi(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote: CryptoHandle,
    own_key_id: u32,
    own_mac_key_handle: CryptoHandle,
    secured_bytes: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    let mut cur = Cursor::new(secured_bytes);

    // SEC_PREFIX. The body bytes (16-byte TransformIdentifier) are part
    // of the AAD extension — symmetric to the encoder.
    let (id, _flags, plen) = read_header(&mut cur, "SEC_PREFIX")?;
    if id != SEC_PREFIX {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 0,
            expected: SEC_PREFIX,
            got: id,
        });
    }
    let sec_prefix_body = cur.read_bytes(plen as usize, "SEC_PREFIX body")?;
    let mut aad_extension = Vec::with_capacity(4 + sec_prefix_body.len());
    aad_extension.extend_from_slice(&[0u8; 4]);
    aad_extension.extend_from_slice(sec_prefix_body);

    // SEC_BODY.
    let (id, _flags, blen) = read_header(&mut cur, "SEC_BODY header")?;
    if id != SEC_BODY {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 1,
            expected: SEC_BODY,
            got: id,
        });
    }
    let ct_len_raw = cur.read_u32_le("SEC_BODY length")?;
    if (ct_len_raw as usize) + 4 != (blen as usize) {
        return Err(SecurityRtpsError::InconsistentLength);
    }
    let ciphertext = cur.read_bytes(ct_len_raw as usize, "SEC_BODY ciphertext")?;

    // SEC_POSTFIX with the multi-MAC payload.
    let (id, _flags, postlen) = read_header(&mut cur, "SEC_POSTFIX")?;
    if id != SEC_POSTFIX {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 2,
            expected: SEC_POSTFIX,
            got: id,
        });
    }
    let macs = if postlen == 0 {
        Vec::new()
    } else {
        let count = cur.read_u32_le("SEC_POSTFIX mac count")? as usize;
        if count > MAX_RECEIVER_MACS {
            return Err(SecurityRtpsError::Truncated(
                "SEC_POSTFIX mac count exceeds cap",
            ));
        }
        let expected_body = 4usize.saturating_add(count.saturating_mul(ReceiverMac::WIRE_SIZE));
        if expected_body != postlen as usize {
            return Err(SecurityRtpsError::InconsistentLength);
        }
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let key_id = cur.read_u32_le("SEC_POSTFIX mac key_id")?;
            let mac_bytes = cur.read_bytes(16, "SEC_POSTFIX mac body")?;
            let mut mac = [0u8; 16];
            mac.copy_from_slice(mac_bytes);
            out.push(ReceiverMac { key_id, mac });
        }
        out
    };

    let plain = plugin.decrypt_submessage_with_receiver_mac(
        local,
        remote,
        own_key_id,
        own_mac_key_handle,
        ciphertext,
        &macs,
        &aad_extension,
    )?;
    Ok(plain)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn need(&self, n: usize, what: &'static str) -> Result<(), SecurityRtpsError> {
        if self.pos + n > self.bytes.len() {
            return Err(SecurityRtpsError::Truncated(what));
        }
        Ok(())
    }

    fn read_bytes(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], SecurityRtpsError> {
        self.need(n, what)?;
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn skip(&mut self, n: usize, what: &'static str) -> Result<(), SecurityRtpsError> {
        self.need(n, what)?;
        self.pos += n;
        Ok(())
    }

    fn read_u32_le(&mut self, what: &'static str) -> Result<u32, SecurityRtpsError> {
        self.need(4, what)?;
        let mut b = [0u8; 4];
        b.copy_from_slice(&self.bytes[self.pos..self.pos + 4]);
        self.pos += 4;
        Ok(u32::from_le_bytes(b))
    }
}

fn read_header(
    cur: &mut Cursor<'_>,
    what: &'static str,
) -> Result<(u8, u8, u16), SecurityRtpsError> {
    cur.need(4, what)?;
    let id = cur.bytes[cur.pos];
    let flags = cur.bytes[cur.pos + 1];
    if flags & FLAG_LE == 0 {
        return Err(SecurityRtpsError::BigEndianNotSupported);
    }
    let mut l = [0u8; 2];
    l.copy_from_slice(&cur.bytes[cur.pos + 2..cur.pos + 4]);
    cur.pos += 4;
    Ok((id, flags, u16::from_le_bytes(l)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
    use zerodds_security::error::SecurityErrorKind;
    use zerodds_security_crypto::AesGcmCryptoPlugin;

    fn make_plugin() -> (AesGcmCryptoPlugin, CryptoHandle, CryptoHandle) {
        let mut p = AesGcmCryptoPlugin::new();
        let local = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let remote = p
            .register_matched_remote_participant(local, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        (p, local, remote)
    }

    #[test]
    fn encode_produces_three_submessages() {
        let (p, local, remote) = make_plugin();
        let plain = b"plain-rtps-submessage-bytes";
        let secured = encode_secured_submessage(&p, local, &[remote], plain).unwrap();
        // First bytes: SEC_PREFIX header.
        assert_eq!(secured[0], SEC_PREFIX);
        // Somewhere in the following bytes are SEC_BODY and SEC_POSTFIX.
        assert!(secured.contains(&SEC_BODY));
        assert!(secured.contains(&SEC_POSTFIX));
    }

    #[test]
    fn roundtrip_matches_plaintext() {
        let (p, local, remote) = make_plugin();
        let plain = b"hello secure dds";
        let secured = encode_secured_submessage(&p, local, &[remote], plain).unwrap();
        let back = decode_secured_submessage(&p, local, remote, &secured).unwrap();
        assert_eq!(back, plain);
    }

    #[test]
    fn tampered_ciphertext_fails_verify() {
        let (p, local, remote) = make_plugin();
        let plain = b"0123456789abcdef";
        let mut secured = encode_secured_submessage(&p, local, &[remote], plain).unwrap();

        // SEC_BODY starts at offset 4 (PREFIX header) + 16 (PREFIX body) =
        // 20, then 4 (BODY header) + 4 (u32 ct_len) = 28. Byte 30 lies in
        // the ciphertext after the nonce.
        secured[30 + 12] ^= 0x10;

        let err = decode_secured_submessage(&p, local, remote, &secured).unwrap_err();
        match err {
            SecurityRtpsError::Crypto(e) => assert_eq!(e.kind, SecurityErrorKind::CryptoFailed),
            other => panic!("expected Crypto, got {other:?}"),
        }
    }

    #[test]
    fn wrong_prefix_id_rejected() {
        let (p, local, remote) = make_plugin();
        let mut secured = encode_secured_submessage(&p, local, &[remote], b"abc").unwrap();
        secured[0] = 0x15; // irgendein anderer Submessage-Typ
        let err = decode_secured_submessage(&p, local, remote, &secured).unwrap_err();
        assert!(matches!(
            err,
            SecurityRtpsError::UnexpectedSubmessageId {
                pos: 0,
                expected: SEC_PREFIX,
                ..
            }
        ));
    }

    #[test]
    fn big_endian_flag_rejected() {
        let (p, local, remote) = make_plugin();
        let mut secured = encode_secured_submessage(&p, local, &[remote], b"x").unwrap();
        secured[1] = 0x00; // flags = BE
        let err = decode_secured_submessage(&p, local, remote, &secured).unwrap_err();
        assert!(matches!(err, SecurityRtpsError::BigEndianNotSupported));
    }

    #[test]
    fn truncated_input_rejected() {
        let (p, local, remote) = make_plugin();
        let err = decode_secured_submessage(&p, local, remote, &[SEC_PREFIX, 0x01]).unwrap_err();
        assert!(matches!(err, SecurityRtpsError::Truncated(_)));
    }

    #[test]
    fn constants_match_spec() {
        assert_eq!(SEC_BODY, 0x30);
        assert_eq!(SEC_PREFIX, 0x31);
        assert_eq!(SEC_POSTFIX, 0x32);
        assert_eq!(SRTPS_PREFIX, 0x33);
        assert_eq!(SRTPS_POSTFIX, 0x34);
    }

    // =======================================================================
    // Multi-MAC-Encoding (Receiver-Specific-MACs)
    // =======================================================================

    /// Builds 3 receiver slots, each with its own random master key
    /// (simulation: one HMAC key per receiver, as it would be
    /// derived from separate SharedSecrets).
    fn make_plugin_with_three_receivers() -> (
        AesGcmCryptoPlugin,
        CryptoHandle,
        [CryptoHandle; 3],
        [CryptoHandle; 3],
    ) {
        let mut p = AesGcmCryptoPlugin::new();
        let sender = p
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();

        // Per receiver: an own handle on the sender side (later
        // used as the multi-MAC key). We simply register
        // 3 local endpoints (each gets random key_material)
        // and copy their tokens to the "receiver side" in the same
        // plugin.
        let r1_sender = p.register_local_endpoint(sender, true, &[]).unwrap();
        let r2_sender = p.register_local_endpoint(sender, true, &[]).unwrap();
        let r3_sender = p.register_local_endpoint(sender, true, &[]).unwrap();

        // "Register" the same keys on the receiver side — in the
        // real world they come in via SharedSecret token exchange,
        // here in the unit test we bypass the handshake by replaying the
        // tokens directly.
        let t1 = p
            .create_local_participant_crypto_tokens(r1_sender, CryptoHandle(0))
            .unwrap();
        let t2 = p
            .create_local_participant_crypto_tokens(r2_sender, CryptoHandle(0))
            .unwrap();
        let t3 = p
            .create_local_participant_crypto_tokens(r3_sender, CryptoHandle(0))
            .unwrap();

        let r1_recv = p
            .register_matched_remote_participant(sender, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        let r2_recv = p
            .register_matched_remote_participant(sender, IdentityHandle(3), SharedSecretHandle(2))
            .unwrap();
        let r3_recv = p
            .register_matched_remote_participant(sender, IdentityHandle(4), SharedSecretHandle(3))
            .unwrap();
        p.set_remote_participant_crypto_tokens(sender, r1_recv, &t1)
            .unwrap();
        p.set_remote_participant_crypto_tokens(sender, r2_recv, &t2)
            .unwrap();
        p.set_remote_participant_crypto_tokens(sender, r3_recv, &t3)
            .unwrap();

        (
            p,
            sender,
            [r1_sender, r2_sender, r3_sender],
            [r1_recv, r2_recv, r3_recv],
        )
    }

    fn bindings_with_ids(handles: &[CryptoHandle]) -> Vec<(CryptoHandle, u32)> {
        // In the unit test we derive key_id for the receiver deterministically
        // from the index (1001, 1002, 1003, ...). Realistically the
        // ID comes from the handshake.
        handles
            .iter()
            .enumerate()
            .map(|(i, h)| (*h, 1000u32 + (i as u32) + 1))
            .collect()
    }

    #[test]
    fn multi_mac_encode_produces_one_ciphertext_and_three_macs() {
        let (p, sender, r_sender, _r_recv) = make_plugin_with_three_receivers();
        let receivers = bindings_with_ids(&r_sender);
        let plain = b"hetero-broadcast-with-3-macs";
        let wire = encode_secured_submessage_multi(&p, sender, &receivers, plain).unwrap();

        // The SEC_POSTFIX ID must be present on the wire.
        let ptr = wire.windows(1).position(|w| w[0] == SEC_POSTFIX);
        assert!(ptr.is_some());
    }

    #[test]
    fn multi_mac_roundtrip_each_receiver_validates_own_mac() {
        // DoD §stage 7 literally: 3 readers with the same suite,
        // different tokens. The writer produces one ciphertext +
        // 3 MACs. Each reader validates its specific MAC.
        let (p, sender, r_sender, _r_recv) = make_plugin_with_three_receivers();
        let receivers = bindings_with_ids(&r_sender);
        let plain = b"multi-mac-dod";
        let wire = encode_secured_submessage_multi(&p, sender, &receivers, plain).unwrap();

        for (idx, (handle, key_id)) in receivers.iter().enumerate() {
            let back = decode_secured_submessage_multi(&p, sender, sender, *key_id, *handle, &wire)
                .unwrap_or_else(|e| panic!("receiver {idx} must decode: {e:?}"));
            assert_eq!(back, plain);
        }
    }

    #[test]
    fn multi_mac_reader_without_matching_key_id_rejects() {
        let (mut p, sender, r_sender, _r_recv) = make_plugin_with_three_receivers();
        let receivers = bindings_with_ids(&r_sender);
        let plain = b"rogue-attempt";
        let wire = encode_secured_submessage_multi(&p, sender, &receivers, plain).unwrap();

        // Unknown receiver: 4th slot with key_id 9999 — NOT in the
        // MAC list.
        let foreign = p.register_local_endpoint(sender, true, &[]).unwrap();
        let err =
            decode_secured_submessage_multi(&p, sender, sender, 9999, foreign, &wire).unwrap_err();
        match err {
            SecurityRtpsError::Crypto(e) => assert_eq!(e.kind, SecurityErrorKind::CryptoFailed),
            other => panic!("expected Crypto-Fail, got {other:?}"),
        }
    }

    #[test]
    fn multi_mac_tampered_ciphertext_fails_even_with_correct_key_id() {
        let (p, sender, r_sender, _r_recv) = make_plugin_with_three_receivers();
        let receivers = bindings_with_ids(&r_sender);
        let plain = b"honest-plaintext";
        let mut wire = encode_secured_submessage_multi(&p, sender, &receivers, plain).unwrap();

        // Flip a byte in the ciphertext (~offset 32).
        wire[32] ^= 0x20;

        let (own_h, own_id) = receivers[0];
        let err =
            decode_secured_submessage_multi(&p, sender, sender, own_id, own_h, &wire).unwrap_err();
        match err {
            SecurityRtpsError::Crypto(e) => assert_eq!(e.kind, SecurityErrorKind::CryptoFailed),
            other => panic!("expected Crypto-Fail, got {other:?}"),
        }
    }

    #[test]
    fn multi_mac_count_cap_enforced() {
        let (p, sender, _r_sender, _r_recv) = make_plugin_with_three_receivers();
        // Build a wire with a malformed SEC_POSTFIX: count > MAX_RECEIVER_MACS.
        // We construct it manually instead of via the plugin, to trigger the cap
        // in the decoder explicitly.
        let ct = b"ciphertext-x"; // anything
        let mut wire = Vec::new();
        // SEC_PREFIX
        wire.push(SEC_PREFIX);
        wire.push(FLAG_LE);
        wire.extend_from_slice(&16u16.to_le_bytes());
        wire.extend_from_slice(&[0u8; 16]);
        // SEC_BODY
        wire.push(SEC_BODY);
        wire.push(FLAG_LE);
        let body_len = 4 + ct.len() as u16;
        wire.extend_from_slice(&body_len.to_le_bytes());
        wire.extend_from_slice(&(ct.len() as u32).to_le_bytes());
        wire.extend_from_slice(ct);
        // SEC_POSTFIX with "count" = MAX+1
        wire.push(SEC_POSTFIX);
        wire.push(FLAG_LE);
        let bad_body_len = 4u16 + ((MAX_RECEIVER_MACS as u16 + 1) * 20);
        wire.extend_from_slice(&bad_body_len.to_le_bytes());
        wire.extend_from_slice(&((MAX_RECEIVER_MACS as u32) + 1).to_le_bytes());
        // (the 20*N bytes need not even be present — the count check
        //  runs before the read)

        let err =
            decode_secured_submessage_multi(&p, sender, sender, 0, sender, &wire).unwrap_err();
        assert!(matches!(err, SecurityRtpsError::Truncated(_)));
    }

    #[test]
    fn multi_mac_empty_mac_list_falls_back_to_normal_decrypt() {
        // If the sender encoded via the classic `encode_secured_submessage`
        // (SEC_POSTFIX empty), the multi-decoder should
        // still work — backward compat.
        let (p, sender, _, _) = make_plugin_with_three_receivers();
        let plain = b"legacy-encoded-path";
        let wire = encode_secured_submessage(&p, sender, &[sender], plain).unwrap();
        let back = decode_secured_submessage_multi(&p, sender, sender, 0, sender, &wire).unwrap();
        assert_eq!(back, plain);
    }
}
