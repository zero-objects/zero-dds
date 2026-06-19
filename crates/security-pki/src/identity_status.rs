// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `IdentityStatusToken` — DDS-Security 1.2 §9.3.2.5.1.2.
//!
//! Emitted by the authentication plugin to signal the current status
//! of a remote identity (cert revocation,
//! permissions expiry). Belongs to the identity-status topic
//! (`ParticipantStatelessMessage` topic with a special class ID).
//!
//! ```text
//!   class_id  = "DDS:Auth:PKI-DH:1.0+IdentityStatus"
//!   properties = { ocsp_status, ocsp_response, expiry_time, ... }
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::identity::PkiError;

/// Class ID per spec §9.3.2.5.1.2.
pub const IDENTITY_STATUS_CLASS_ID: &str = "DDS:Auth:PKI-DH:1.0+IdentityStatus";

/// Property keys.
pub const KEY_OCSP_STATUS: &str = "ocsp_status";
/// `ocsp_response` property key.
pub const KEY_OCSP_RESPONSE: &str = "ocsp_response";
/// `expiry_time` property key (UTC Unix seconds, BE u64).
pub const KEY_EXPIRY_TIME: &str = "expiry_time";

/// Status values (RFC 6960 OCSP code mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityStatusKind {
    /// `good` — the cert is active and valid.
    Good,
    /// `revoked` — the cert is revoked, every handshake must
    /// be rejected.
    Revoked,
    /// `unknown` — the OCSP responder does not know the cert; the caller
    /// decides whether to tolerate it.
    Unknown,
}

impl IdentityStatusKind {
    /// Wire value (1 byte).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Good => 0,
            Self::Revoked => 1,
            Self::Unknown => 2,
        }
    }

    /// `u8 → IdentityStatusKind`.
    ///
    /// # Errors
    /// `PkiError::InvalidPem` if the code is not 0/1/2.
    pub fn from_u8(v: u8) -> Result<Self, PkiError> {
        match v {
            0 => Ok(Self::Good),
            1 => Ok(Self::Revoked),
            2 => Ok(Self::Unknown),
            _ => Err(PkiError::InvalidPem(format!(
                "unknown OCSP status code: {v}"
            ))),
        }
    }
}

/// `IdentityStatusToken`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityStatusToken {
    /// OCSP status of the remote identity.
    pub ocsp_status: IdentityStatusKind,
    /// Optional raw OCSP response DER (the caller can
    /// signature-verify it itself).
    pub ocsp_response: Option<Vec<u8>>,
    /// Cert expiry as UTC Unix seconds.
    pub expiry_time: u64,
}

impl IdentityStatusToken {
    /// Constructor with `Good` status and no OCSP response body.
    #[must_use]
    pub fn good(expiry_time: u64) -> Self {
        Self {
            ocsp_status: IdentityStatusKind::Good,
            ocsp_response: None,
            expiry_time,
        }
    }

    /// Constructor with `Revoked`.
    #[must_use]
    pub fn revoked(expiry_time: u64, ocsp_response: Option<Vec<u8>>) -> Self {
        Self {
            ocsp_status: IdentityStatusKind::Revoked,
            ocsp_response,
            expiry_time,
        }
    }

    /// Encode zu Wire-Bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // Class-ID.
        out.push(IDENTITY_STATUS_CLASS_ID.len() as u8);
        out.extend_from_slice(IDENTITY_STATUS_CLASS_ID.as_bytes());
        // ocsp_status — 1 byte.
        out.push(KEY_OCSP_STATUS.len() as u8);
        out.extend_from_slice(KEY_OCSP_STATUS.as_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.push(self.ocsp_status.to_u8());
        // ocsp_response — optional.
        if let Some(resp) = &self.ocsp_response {
            out.push(KEY_OCSP_RESPONSE.len() as u8);
            out.extend_from_slice(KEY_OCSP_RESPONSE.as_bytes());
            out.extend_from_slice(&(resp.len() as u16).to_be_bytes());
            out.extend_from_slice(resp);
        }
        // expiry_time — 8 bytes BE.
        out.push(KEY_EXPIRY_TIME.len() as u8);
        out.extend_from_slice(KEY_EXPIRY_TIME.as_bytes());
        out.extend_from_slice(&8u16.to_be_bytes());
        out.extend_from_slice(&self.expiry_time.to_be_bytes());
        out
    }

    /// Decode Wire-Bytes.
    ///
    /// # Errors
    /// `PkiError::InvalidPem` on format problems.
    pub fn decode(bytes: &[u8]) -> Result<Self, PkiError> {
        let mut pos = 0usize;
        if bytes.len() <= pos {
            return Err(PkiError::InvalidPem("IdentityStatus truncated".into()));
        }
        let cid_len = bytes[pos] as usize;
        pos += 1;
        if bytes.len() < pos + cid_len {
            return Err(PkiError::InvalidPem("IdentityStatus class-id trunc".into()));
        }
        let cid = core::str::from_utf8(&bytes[pos..pos + cid_len])
            .map_err(|_| PkiError::InvalidPem("class-id non-utf8".into()))?;
        if cid != IDENTITY_STATUS_CLASS_ID {
            return Err(PkiError::InvalidPem(format!(
                "IdentityStatus class-id mismatch: `{cid}`"
            )));
        }
        pos += cid_len;

        let mut ocsp_status: Option<IdentityStatusKind> = None;
        let mut ocsp_response: Option<Vec<u8>> = None;
        let mut expiry_time: Option<u64> = None;

        while pos < bytes.len() {
            let key_len = bytes[pos] as usize;
            pos += 1;
            if bytes.len() < pos + key_len {
                return Err(PkiError::InvalidPem("key trunc".into()));
            }
            let key = core::str::from_utf8(&bytes[pos..pos + key_len])
                .map_err(|_| PkiError::InvalidPem("key non-utf8".into()))?;
            pos += key_len;
            if bytes.len() < pos + 2 {
                return Err(PkiError::InvalidPem("value-length trunc".into()));
            }
            let val_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
            pos += 2;
            if bytes.len() < pos + val_len {
                return Err(PkiError::InvalidPem("value trunc".into()));
            }
            let val = &bytes[pos..pos + val_len];
            match key {
                KEY_OCSP_STATUS => {
                    if val.len() != 1 {
                        return Err(PkiError::InvalidPem("ocsp_status not 1 byte".into()));
                    }
                    ocsp_status = Some(IdentityStatusKind::from_u8(val[0])?);
                }
                KEY_OCSP_RESPONSE => {
                    ocsp_response = Some(val.to_vec());
                }
                KEY_EXPIRY_TIME => {
                    if val.len() != 8 {
                        return Err(PkiError::InvalidPem("expiry_time not 8 byte".into()));
                    }
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(val);
                    expiry_time = Some(u64::from_be_bytes(buf));
                }
                other => {
                    // Unknown properties — the spec says "tolerable",
                    // we skip them.
                    let _ = other;
                }
            }
            pos += val_len;
        }
        Ok(Self {
            ocsp_status: ocsp_status
                .ok_or_else(|| PkiError::InvalidPem("missing ocsp_status".into()))?,
            ocsp_response,
            expiry_time: expiry_time
                .ok_or_else(|| PkiError::InvalidPem("missing expiry_time".into()))?,
        })
    }

    /// `true` if the status must trigger a handshake reject.
    #[must_use]
    pub fn requires_reject(&self) -> bool {
        matches!(self.ocsp_status, IdentityStatusKind::Revoked)
    }
}

/// Helper: property list for the plugin API.
#[must_use]
pub fn identity_status_properties(token: &IdentityStatusToken) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::with_capacity(3);
    out.push((
        KEY_OCSP_STATUS.to_string(),
        alloc::vec![token.ocsp_status.to_u8()],
    ));
    if let Some(r) = &token.ocsp_response {
        out.push((KEY_OCSP_RESPONSE.to_string(), r.clone()));
    }
    out.push((
        KEY_EXPIRY_TIME.to_string(),
        token.expiry_time.to_be_bytes().to_vec(),
    ));
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_good_status() {
        let t = IdentityStatusToken::good(1_700_000_000);
        let bytes = t.encode();
        let back = IdentityStatusToken::decode(&bytes).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn round_trip_revoked_with_ocsp_response() {
        let t =
            IdentityStatusToken::revoked(1_700_000_000, Some(alloc::vec![0xCA, 0xFE, 0xBA, 0xBE]));
        let bytes = t.encode();
        let back = IdentityStatusToken::decode(&bytes).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn requires_reject_only_for_revoked() {
        assert!(!IdentityStatusToken::good(0).requires_reject());
        assert!(IdentityStatusToken::revoked(0, None).requires_reject());
        let unknown = IdentityStatusToken {
            ocsp_status: IdentityStatusKind::Unknown,
            ocsp_response: None,
            expiry_time: 0,
        };
        assert!(!unknown.requires_reject(), "Unknown is not auto-reject");
    }

    #[test]
    fn ocsp_status_round_trip() {
        for s in [
            IdentityStatusKind::Good,
            IdentityStatusKind::Revoked,
            IdentityStatusKind::Unknown,
        ] {
            assert_eq!(IdentityStatusKind::from_u8(s.to_u8()).unwrap(), s);
        }
    }

    #[test]
    fn ocsp_status_unknown_code_rejected() {
        assert!(IdentityStatusKind::from_u8(0xff).is_err());
    }

    #[test]
    fn missing_required_property_rejected() {
        // Encode minimal — class-id only.
        let mut bytes = Vec::new();
        bytes.push(IDENTITY_STATUS_CLASS_ID.len() as u8);
        bytes.extend_from_slice(IDENTITY_STATUS_CLASS_ID.as_bytes());
        let err = IdentityStatusToken::decode(&bytes).unwrap_err();
        assert!(matches!(err, PkiError::InvalidPem(_)));
    }

    #[test]
    fn class_id_mismatch_rejected() {
        let mut bytes = IdentityStatusToken::good(0).encode();
        bytes[5] ^= 0xff;
        let err = IdentityStatusToken::decode(&bytes).unwrap_err();
        assert!(matches!(err, PkiError::InvalidPem(_)));
    }

    #[test]
    fn properties_helper_round_trip() {
        let t = IdentityStatusToken::revoked(42, Some(alloc::vec![0x01, 0x02]));
        let props = identity_status_properties(&t);
        assert_eq!(props.len(), 3);
        assert_eq!(props[0].0, KEY_OCSP_STATUS);
        assert_eq!(props[1].0, KEY_OCSP_RESPONSE);
        assert_eq!(props[2].0, KEY_EXPIRY_TIME);
    }

    #[test]
    fn class_id_constant_matches_spec() {
        assert_eq!(
            IDENTITY_STATUS_CLASS_ID,
            "DDS:Auth:PKI-DH:1.0+IdentityStatus"
        );
    }

    #[test]
    fn unknown_property_keys_skipped_not_rejected() {
        // Build manually with an extra property the decoder doesn't know.
        let mut bytes = Vec::new();
        bytes.push(IDENTITY_STATUS_CLASS_ID.len() as u8);
        bytes.extend_from_slice(IDENTITY_STATUS_CLASS_ID.as_bytes());
        // ocsp_status = Good
        bytes.push(KEY_OCSP_STATUS.len() as u8);
        bytes.extend_from_slice(KEY_OCSP_STATUS.as_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(0);
        // unknown property "future_field"
        let unk = b"future_field";
        bytes.push(unk.len() as u8);
        bytes.extend_from_slice(unk);
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(b"\x01\x02\x03");
        // expiry_time = 99
        bytes.push(KEY_EXPIRY_TIME.len() as u8);
        bytes.extend_from_slice(KEY_EXPIRY_TIME.as_bytes());
        bytes.extend_from_slice(&8u16.to_be_bytes());
        bytes.extend_from_slice(&99u64.to_be_bytes());

        let t = IdentityStatusToken::decode(&bytes).unwrap();
        assert_eq!(t.ocsp_status, IdentityStatusKind::Good);
        assert_eq!(t.expiry_time, 99);
    }
}
