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

use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
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
                "big-endian SEC_* not supported (single-endianness path, LE by default)"
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

    // AAD extension per DDS-Security 1.2 §10.5.3 Table 80 (submessage
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

    /// §9.5.2 / §7.3.6 — the secured submessage is a sequence of exactly
    /// three submessages in a fixed order: SEC_PREFIX, then the
    /// protected content as SEC_BODY, then SEC_POSTFIX. Walk the wire by
    /// `octetsToNextHeader` and check every header field, so that a
    /// reordering, a wrong id, a missing endianness flag, or a length
    /// that does not describe its own body is caught. (Scanning for the
    /// id bytes is not enough: 0x30 is also the digit `0` and shows up
    /// in ciphertext by chance.)
    #[test]
    fn three_submessages_carry_spec_ids_flags_and_lengths_in_order() {
        let (p, local, remote) = make_plugin();
        let plain = b"plain-rtps-submessage-bytes";
        let secured = encode_secured_submessage(&p, local, &[remote], plain).unwrap();

        /// (id, flags, octetsToNextHeader, body) at `at`.
        fn submessage(wire: &[u8], at: usize) -> (u8, u8, usize, &[u8]) {
            let len = u16::from_le_bytes([wire[at + 2], wire[at + 3]]) as usize;
            (wire[at], wire[at + 1], len, &wire[at + 4..at + 4 + len])
        }

        // 1) SEC_PREFIX: 16-byte TransformIdentifier, no more, no less.
        let (id, flags, len, prefix_body) = submessage(&secured, 0);
        assert_eq!(id, SEC_PREFIX, "first submessage id");
        assert_eq!(id, 0x31, "spec §7.3.6.2 SEC_PREFIX");
        assert_eq!(flags & 0x01, 0x01, "EndiannessFlag set (little endian)");
        assert_eq!(len, 16, "TransformIdentifier is 16 bytes");
        assert_eq!(prefix_body.len(), 16);

        // 2) SEC_BODY directly after it: u32 ciphertext length + ciphertext.
        let body_at = 4 + len;
        let (id, flags, len, body) = submessage(&secured, body_at);
        assert_eq!(id, SEC_BODY, "second submessage id");
        assert_eq!(id, 0x30, "spec §7.3.6.4 SEC_BODY");
        assert_eq!(flags & 0x01, 0x01, "EndiannessFlag set");
        let declared = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        assert_eq!(
            declared + 4,
            len,
            "the SEC_BODY length header must describe its own body",
        );
        assert_eq!(declared, body.len() - 4);
        assert_ne!(
            &body[4..],
            &plain[..],
            "SEC_BODY carries ciphertext, not the plaintext",
        );

        // 3) SEC_POSTFIX last, empty on the single-receiver path.
        let post_at = body_at + 4 + len;
        let (id, flags, len, post_body) = submessage(&secured, post_at);
        assert_eq!(id, SEC_POSTFIX, "third submessage id");
        assert_eq!(id, 0x32, "spec §7.3.6.3 SEC_POSTFIX");
        assert_eq!(flags & 0x01, 0x01, "EndiannessFlag set");
        assert_eq!(len, 0);
        assert!(post_body.is_empty());

        // Nothing follows the postfix.
        assert_eq!(
            post_at + 4,
            secured.len(),
            "the sequence ends with the SEC_POSTFIX",
        );
    }

    /// The decoder enforces the same order: a wire whose second
    /// submessage is not SEC_BODY is rejected at position 1, and one
    /// whose third is not SEC_POSTFIX at position 2.
    #[test]
    fn decoder_rejects_a_reordered_submessage_sequence() {
        let (p, local, remote) = make_plugin();
        let secured = encode_secured_submessage(&p, local, &[remote], b"0123456789abcdef").unwrap();

        let mut swapped_body = secured.clone();
        swapped_body[4 + 16] = SEC_POSTFIX;
        match decode_secured_submessage(&p, local, remote, &swapped_body) {
            Err(SecurityRtpsError::UnexpectedSubmessageId { pos, expected, got }) => {
                assert_eq!((pos, expected, got), (1, SEC_BODY, SEC_POSTFIX));
            }
            other => panic!("expected an id error at position 1, got {other:?}"),
        }

        let body_len = u16::from_le_bytes([secured[4 + 16 + 2], secured[4 + 16 + 3]]) as usize;
        let post_at = 4 + 16 + 4 + body_len;
        let mut swapped_post = secured.clone();
        swapped_post[post_at] = SEC_BODY;
        match decode_secured_submessage(&p, local, remote, &swapped_post) {
            Err(SecurityRtpsError::UnexpectedSubmessageId { pos, expected, got }) => {
                assert_eq!((pos, expected, got), (2, SEC_POSTFIX, SEC_BODY));
            }
            other => panic!("expected an id error at position 2, got {other:?}"),
        }
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
        secured[0] = 0x15; // some other submessage type
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
}
