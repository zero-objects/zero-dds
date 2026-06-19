// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `AuthRequestMessageToken` — DDS-Security 1.2 §9.3.2.5.1.1.
//!
//! Pre-handshake token, sent by the initiator via the built-in topic
//! `ParticipantStatelessMessage` to an unknown remote
//! participant, to kick off the `handshake_request_message` sequence.
//!
//! ```text
//!   class_id  = "DDS:Auth:PKI-DH:1.0+AuthReq"
//!   properties = {}
//!   binary_properties = { future_challenge = <256-bit nonce> }
//! ```
//!
//! Spec §9.3.2.5.1.1: the `future_challenge` must be echoed in the subsequent
//! `HandshakeRequest` token as `challenge1`, to prevent replay
//! attacks.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::identity::PkiError;

/// Class ID per spec §9.3.2.5.1.1.
pub const AUTH_REQUEST_CLASS_ID: &str = "DDS:Auth:PKI-DH:1.0+AuthReq";

/// Property key for the future-challenge value.
pub const FUTURE_CHALLENGE_KEY: &str = "future_challenge";

/// `AuthRequestMessageToken`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRequestToken {
    /// 32-byte (256-bit) nonce that must reappear in the next HandshakeRequest
    /// as `challenge1`.
    pub future_challenge: [u8; 32],
}

impl AuthRequestToken {
    /// Constructor.
    #[must_use]
    pub fn new(challenge: [u8; 32]) -> Self {
        Self {
            future_challenge: challenge,
        }
    }

    /// Encode to wire bytes (TLV: 1-byte class-ID length + class ID +
    /// 1-byte key length + key + 2-byte BE value length + value).
    /// The caller layer maps this into the built-in-topic wire format.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let class_id = AUTH_REQUEST_CLASS_ID.as_bytes();
        let key = FUTURE_CHALLENGE_KEY.as_bytes();
        let mut out = Vec::with_capacity(1 + class_id.len() + 1 + key.len() + 2 + 32);
        out.push(class_id.len() as u8);
        out.extend_from_slice(class_id);
        out.push(key.len() as u8);
        out.extend_from_slice(key);
        out.extend_from_slice(&(32u16.to_be_bytes()));
        out.extend_from_slice(&self.future_challenge);
        out
    }

    /// Decode Wire-Bytes.
    ///
    /// # Errors
    /// `PkiError::InvalidPem` (re-used as a generic decode error) if:
    /// * class ID wrong
    /// * property key wrong
    /// * value length != 32
    /// * buffer truncated
    pub fn decode(bytes: &[u8]) -> Result<Self, PkiError> {
        let mut pos = 0usize;
        if bytes.len() <= pos {
            return Err(PkiError::InvalidPem("AuthReq truncated at class-id".into()));
        }
        let cid_len = bytes[pos] as usize;
        pos += 1;
        if bytes.len() < pos + cid_len {
            return Err(PkiError::InvalidPem("AuthReq class-id truncated".into()));
        }
        let cid = core::str::from_utf8(&bytes[pos..pos + cid_len])
            .map_err(|_| PkiError::InvalidPem("AuthReq class-id non-utf8".into()))?;
        if cid != AUTH_REQUEST_CLASS_ID {
            return Err(PkiError::InvalidPem(format!(
                "AuthReq class-id mismatch: got `{cid}`"
            )));
        }
        pos += cid_len;
        if bytes.len() <= pos {
            return Err(PkiError::InvalidPem(
                "AuthReq truncated at key-length".into(),
            ));
        }
        let key_len = bytes[pos] as usize;
        pos += 1;
        if bytes.len() < pos + key_len {
            return Err(PkiError::InvalidPem("AuthReq key truncated".into()));
        }
        let key = core::str::from_utf8(&bytes[pos..pos + key_len])
            .map_err(|_| PkiError::InvalidPem("AuthReq key non-utf8".into()))?;
        if key != FUTURE_CHALLENGE_KEY {
            return Err(PkiError::InvalidPem(format!(
                "AuthReq key mismatch: got `{key}`"
            )));
        }
        pos += key_len;
        if bytes.len() < pos + 2 {
            return Err(PkiError::InvalidPem(
                "AuthReq value-length truncated".into(),
            ));
        }
        let val_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos += 2;
        if val_len != 32 {
            return Err(PkiError::InvalidPem(format!(
                "AuthReq future_challenge must be 32 bytes, got {val_len}"
            )));
        }
        if bytes.len() < pos + val_len {
            return Err(PkiError::InvalidPem("AuthReq value truncated".into()));
        }
        let mut challenge = [0u8; 32];
        challenge.copy_from_slice(&bytes[pos..pos + 32]);
        Ok(Self {
            future_challenge: challenge,
        })
    }
}

/// Helper: property-list format for the DDS-Security plugin API
/// (the caller layer builds the built-in-topic sample from it).
#[must_use]
pub fn auth_request_properties(token: &AuthRequestToken) -> Vec<(String, Vec<u8>)> {
    alloc::vec![(
        FUTURE_CHALLENGE_KEY.to_string(),
        token.future_challenge.to_vec(),
    )]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_challenge() {
        let chal = [0x42u8; 32];
        let token = AuthRequestToken::new(chal);
        let bytes = token.encode();
        let back = AuthRequestToken::decode(&bytes).unwrap();
        assert_eq!(back, token);
    }

    #[test]
    fn class_id_must_match() {
        let mut bytes = AuthRequestToken::new([1u8; 32]).encode();
        // Corrupt class-id by flipping a byte.
        bytes[5] ^= 0xff;
        let err = AuthRequestToken::decode(&bytes).unwrap_err();
        assert!(matches!(err, PkiError::InvalidPem(_)));
    }

    #[test]
    fn truncated_buffer_rejected() {
        let bytes = AuthRequestToken::new([1u8; 32]).encode();
        for cut in 1..bytes.len() {
            assert!(
                AuthRequestToken::decode(&bytes[..cut]).is_err(),
                "buffer truncated at {cut} should fail"
            );
        }
    }

    #[test]
    fn wrong_value_length_rejected() {
        let mut bytes = AuthRequestToken::new([1u8; 32]).encode();
        // Replace value-length with 16 instead of 32.
        let val_len_pos = 1 + AUTH_REQUEST_CLASS_ID.len() + 1 + FUTURE_CHALLENGE_KEY.len();
        bytes[val_len_pos] = 0;
        bytes[val_len_pos + 1] = 16;
        let err = AuthRequestToken::decode(&bytes).unwrap_err();
        assert!(matches!(err, PkiError::InvalidPem(_)));
    }

    #[test]
    fn properties_helper_returns_kv_list() {
        let token = AuthRequestToken::new([7u8; 32]);
        let props = auth_request_properties(&token);
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].0, FUTURE_CHALLENGE_KEY);
        assert_eq!(props[0].1.len(), 32);
    }

    #[test]
    fn class_id_constant_matches_spec() {
        // Spec §9.3.2.5.1.1 — verbatim from the standard text.
        assert_eq!(AUTH_REQUEST_CLASS_ID, "DDS:Auth:PKI-DH:1.0+AuthReq");
    }

    #[test]
    fn empty_input_rejected() {
        assert!(AuthRequestToken::decode(&[]).is_err());
    }

    #[test]
    fn class_id_length_byte_too_large_rejected() {
        let bytes = alloc::vec![0xff, b'x'];
        assert!(AuthRequestToken::decode(&bytes).is_err());
    }
}
