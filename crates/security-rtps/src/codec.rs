// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire format for secure submessages.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The codec accepts `&dyn CryptographicPlugin`, so the
//! crypto implementation stays interchangeable — architectural.)
//!
//! DDS-Security 1.2 §7.4.7.6, §7.4.7.5, §7.4.7.7 and §10.5.3.3.4:
//!
//! ```text
//! SEC_PREFIX  (0x31): CryptoHeader (20 octets: transformation_kind,
//!                     transformation_key_id, session_id,
//!                     initialization_vector_suffix)
//! SEC_BODY    (0x30): CryptoContent (length + ciphertext); with
//!                     authentication only the plain submessage stands here
//! SEC_POSTFIX (0x32): CryptoFooter (common_mac, receiver_specific_macs)
//! ```
//!
//! The CryptoHeader names the key the submessage is protected with
//! (§10.5.2.3: `transformation_key_id` is the `sender_key_id` of the key
//! material), so only the plugin that holds the key can write it: the
//! codec checks the frame and leaves the transformation to
//! [`CryptographicPlugin::encode_data_datawriter_submessage`] and
//! [`CryptographicPlugin::decode_data_datawriter_submessage`]. Before, the
//! codec wrote 16 zero octets as SEC_PREFIX body, which no other
//! implementation reads.

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

/// Length of the CryptoHeader in the SEC_PREFIX and the SRTPS_PREFIX
/// (DDS-Security 1.2 §7.4.7.6.1, §10.5.2.3: 4 + 4 + 4 + 8 octets).
pub const CRYPTO_HEADER_LEN: usize = 20;

/// Encodes a plain submessage as a **secured** submessage sequence
/// (SEC_PREFIX + SEC_BODY + SEC_POSTFIX, DDS-Security 1.2 §7.4.7.6 and
/// §10.5.3.3.4) with the key material the plugin holds under `local`.
///
/// The receiver-specific MACs are those of the key material: the plugin
/// appends one per reader key it registered under `local`; `remote_list`
/// is not read.
///
/// # Errors
/// A passed-through crypto error; `Crypto` with `NotImplemented` from a
/// plugin that does not frame submessages.
pub fn encode_secured_submessage(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote_list: &[CryptoHandle],
    plaintext: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    let _ = remote_list;
    Ok(plugin.encode_data_datawriter_submessage(local, plaintext)?)
}

/// Decodes a secure-submessage sequence and returns the plaintext
/// submessage, with the key material the plugin holds under `remote` (the
/// sender's key; `local` is not read).
///
/// # Errors
/// A first submessage that is no SEC_PREFIX with a CryptoHeader,
/// big-endian, cut short, or a crypto verify fail.
pub fn decode_secured_submessage(
    plugin: &dyn CryptographicPlugin,
    local: CryptoHandle,
    remote: CryptoHandle,
    secured_bytes: &[u8],
) -> Result<Vec<u8>, SecurityRtpsError> {
    let _ = local;
    check_prefix(secured_bytes, SEC_PREFIX, "SEC_PREFIX")?;
    Ok(plugin.decode_data_datawriter_submessage(remote, secured_bytes)?)
}

/// Checks that `wire` starts with the submessage `id` (SEC_PREFIX or
/// SRTPS_PREFIX), little-endian, whose body holds a CryptoHeader and fits
/// in `wire`.
pub(crate) fn check_prefix(
    wire: &[u8],
    id: u8,
    what: &'static str,
) -> Result<(), SecurityRtpsError> {
    let mut cur = Cursor::new(wire);
    let (got, _flags, len) = read_header(&mut cur, what)?;
    if got != id {
        return Err(SecurityRtpsError::UnexpectedSubmessageId {
            pos: 0,
            expected: id,
            got,
        });
    }
    if usize::from(len) < CRYPTO_HEADER_LEN {
        return Err(SecurityRtpsError::Truncated("CryptoHeader"));
    }
    cur.skip(usize::from(len), what)?;
    Ok(())
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

    fn skip(&mut self, n: usize, what: &'static str) -> Result<(), SecurityRtpsError> {
        self.need(n, what)?;
        self.pos += n;
        Ok(())
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
    use zerodds_security_crypto::AesGcmCryptoPlugin;

    /// A HEARTBEAT submessage (id 0x07, E set, 28 octets).
    const HEARTBEAT: [u8; 32] = [
        0x07, 0x01, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00,
    ];

    /// Sender `a` with its CryptoToken (DDS-Security 1.2 Table 72: the big
    /// endian CDR of KeyMaterial_AES_GCM_GMAC), receiver `b` that knows the
    /// sender only from that token.
    struct Pair {
        a: AesGcmCryptoPlugin,
        local_a: CryptoHandle,
        token_a: Vec<u8>,
        b: AesGcmCryptoPlugin,
        a_at_b: CryptoHandle,
    }

    fn pair() -> Pair {
        let mut a = AesGcmCryptoPlugin::new();
        let local_a = a
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let token_a = a
            .create_local_participant_crypto_tokens(local_a, CryptoHandle(0))
            .unwrap();
        let mut b = AesGcmCryptoPlugin::new();
        let local_b = b
            .register_local_participant(IdentityHandle(1), &[])
            .unwrap();
        let a_at_b = b
            .register_matched_remote_participant(local_b, IdentityHandle(2), SharedSecretHandle(1))
            .unwrap();
        b.set_remote_participant_crypto_tokens(local_b, a_at_b, &token_a)
            .unwrap();
        Pair {
            a,
            local_a,
            token_a,
            b,
            a_at_b,
        }
    }

    /// `transformation_kind` and `sender_key_id` of a KeyMaterial_AES_GCM_GMAC
    /// (§10.5.2.1.1, big endian CDR): the kind, `sequence<octet, 32>
    /// master_salt`, then the key id.
    fn kind_and_key_id(keymat: &[u8]) -> ([u8; 4], [u8; 4]) {
        let salt = u32::from_be_bytes(keymat[4..8].try_into().unwrap()) as usize;
        (
            keymat[..4].try_into().unwrap(),
            keymat[8 + salt..12 + salt].try_into().unwrap(),
        )
    }

    /// The submessages of `wire` as (id, flags, body), read with DDSI-RTPS
    /// 2.5 §9.4.5.1 (little-endian lengths, E set).
    fn submessages(wire: &[u8]) -> Vec<(u8, u8, &[u8])> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < wire.len() {
            let len = usize::from(u16::from_le_bytes([wire[at + 2], wire[at + 3]]));
            out.push((wire[at], wire[at + 1], &wire[at + 4..at + 4 + len]));
            at += 4 + len;
        }
        assert_eq!(at, wire.len());
        out
    }

    /// §7.4.7.6.1, §10.5.2.3, §10.5.2.4, §10.5.2.5: SEC_PREFIX with a
    /// CryptoHeader of 20 octets whose transformation_kind and
    /// transformation_key_id are those of the sender's key material, a
    /// SEC_BODY whose CryptoContent length (big endian) covers the rest of
    /// the body, a SEC_POSTFIX with the 16-octet common_mac and the length
    /// of receiver_specific_macs. The receiver that has only the token reads
    /// the submessage back. Before, the SEC_PREFIX carried 16 zero octets.
    #[test]
    fn security_7_4_7_6_the_prefix_carries_the_crypto_header_of_the_senders_key() {
        let p = pair();
        let wire = encode_secured_submessage(&p.a, p.local_a, &[], &HEARTBEAT).unwrap();
        let parts = submessages(&wire);
        assert_eq!(
            parts.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
            [SEC_PREFIX, SEC_BODY, SEC_POSTFIX]
        );
        assert!(parts.iter().all(|(_, flags, _)| flags & 0x01 == 0x01));
        let (kind, key_id) = kind_and_key_id(&p.token_a);
        let header = parts[0].2;
        assert_eq!(header.len(), CRYPTO_HEADER_LEN);
        assert_eq!(header[..4], kind);
        assert_ne!(kind, [0; 4], "CRYPTO_TRANSFORMATION_KIND_NONE");
        assert_eq!(header[4..8], key_id);
        let body = parts[1].2;
        let content = u32::from_be_bytes(body[..4].try_into().unwrap()) as usize;
        assert_eq!(content + 4, body.len());
        let footer = parts[2].2;
        let macs = u32::from_be_bytes(footer[16..20].try_into().unwrap()) as usize;
        assert_eq!(footer.len(), 20 + macs * 20);
        assert!(!wire.windows(HEARTBEAT.len()).any(|w| w == HEARTBEAT));
        let back = decode_secured_submessage(&p.b, p.a_at_b, p.a_at_b, &wire).unwrap();
        assert_eq!(back, HEARTBEAT);
    }

    #[test]
    fn the_wire_is_checked_before_the_plugin_reads_it() {
        let p = pair();
        let wire = encode_secured_submessage(&p.a, p.local_a, &[], &HEARTBEAT).unwrap();
        assert_eq!(
            decode_secured_submessage(&p.b, p.a_at_b, p.a_at_b, &HEARTBEAT).unwrap_err(),
            SecurityRtpsError::UnexpectedSubmessageId {
                pos: 0,
                expected: SEC_PREFIX,
                got: 0x07,
            }
        );
        let mut big_endian = wire.clone();
        big_endian[1] &= !0x01;
        assert_eq!(
            decode_secured_submessage(&p.b, p.a_at_b, p.a_at_b, &big_endian).unwrap_err(),
            SecurityRtpsError::BigEndianNotSupported
        );
        assert!(matches!(
            decode_secured_submessage(&p.b, p.a_at_b, p.a_at_b, &wire[..10]),
            Err(SecurityRtpsError::Truncated(_))
        ));
        let mut changed = wire.clone();
        let in_body = 4 + CRYPTO_HEADER_LEN + 4 + 4 + 2;
        changed[in_body] ^= 0x01;
        assert!(matches!(
            decode_secured_submessage(&p.b, p.a_at_b, p.a_at_b, &changed),
            Err(SecurityRtpsError::Crypto(_))
        ));
    }

    /// DDSI-RTPS 2.5 §9.4.5.1.1.1 Table 9.14 (DDS-Security 0x30–0x3f),
    /// DDS-Security 1.2 §7.4.7: a protected submessage is SEC_PREFIX (0x31),
    /// SEC_BODY (0x30) and SEC_POSTFIX (0x32) in this order, each with E set
    /// and an octetsToNextHeader that ends where the next one begins.
    #[test]
    fn rtps_9_4_5_1_1_1_three_submessages_carry_spec_ids_flags_and_lengths_in_order() {
        let p = pair();
        let wire = encode_secured_submessage(&p.a, p.local_a, &[], &HEARTBEAT).unwrap();
        // `submessages` follows the lengths and fails unless they end with the wire.
        let parts = submessages(&wire);
        let ids: Vec<u8> = parts.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, [0x31, 0x30, 0x32]);
        assert!(
            parts.iter().all(|(_, flags, _)| flags & 0x01 == 0x01),
            "E set"
        );
        assert_eq!(parts[0].2.len(), CRYPTO_HEADER_LEN);
    }

    /// DDSI-RTPS 2.5 §9.4.5.1.1 / DDS-Security 1.2 §7.4.7: the submessage ids.
    #[test]
    fn rtps_9_4_5_1_1_1_constants_match_spec() {
        assert_eq!(SEC_BODY, 0x30);
        assert_eq!(SEC_PREFIX, 0x31);
        assert_eq!(SEC_POSTFIX, 0x32);
        assert_eq!(SRTPS_PREFIX, 0x33);
        assert_eq!(SRTPS_POSTFIX, 0x34);
    }
}
