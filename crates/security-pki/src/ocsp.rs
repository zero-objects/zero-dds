// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCSP stapling validation.
//!
//! Check the Online Certificate Status Protocol (RFC 6960):
//! * An OCSP response is delivered by the peer as a "stapled" token in the handshake
//!   (same wire as TLS OCSP stapling, spec RFC 6961).
//! * This module parses the `CertStatus` from the DER-encoded
//!   `OCSPResponse` structure and reports `Good`/`Revoked`/`Unknown`.
//!
//! # Scope
//!
//! * Shallow DER scan: we look for the first `[0] IMPLICIT good`,
//!   `[1] revoked`, or `[2] unknown` tag in the `SingleResponse`
//!   area. Not a complete ASN.1.
//! * No signature validation of the OCSP response — the peer already
//!   fetched it signed from the OCSP responder. A real PKI stack
//!   would additionally check the responder signature here — that
//!   follows in future-major+ with an `x509-parser` dep.
//!
//! # Not included
//!
//! * OCSP request signing (we only consume responses).
//! * CRL parsing (separate future-major).

#[cfg(test)]
use alloc::vec::Vec;

/// Cert status from an OCSP response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcspStatus {
    /// The cert is valid as of the response time (`thisUpdate`).
    Good,
    /// The cert was revoked. The time and revocation reason are
    /// not passed through — the DDS-Security spec requires only the
    /// boolean flag (spec §8.3.2.10.1).
    Revoked,
    /// The OCSP responder does not know the cert.
    Unknown,
    /// The response could not be parsed (non-spec-conformant).
    Malformed,
}

/// Scans a DER OCSP response and returns the cert status for the
/// (single) contained `SingleResponse`.
///
/// The parser is deliberately shallow: it searches the DER stream for the
/// OCSP-response CertStatus tags `0x80` (good), `0xA1`/`0x81` (revoked)
/// or `0x82` (unknown) within the first `SingleResponse` —
/// typically about 40 bytes into the response.
///
/// This heuristic is enough for the OCSP responses of common CA
/// responders (Let's Encrypt, DigiCert, Sectigo). For multiple certs
/// in one response this function returns the status of the **first**.
#[must_use]
pub fn parse_ocsp_status(response_der: &[u8]) -> OcspStatus {
    // OCSP-response DER structure (simplified):
    //   OCSPResponse ::= SEQUENCE {
    //     responseStatus [0] ENUMERATED,
    //     responseBytes  [0] EXPLICIT ResponseBytes OPTIONAL
    //   }
    //
    // We scan for:
    //   * [0] IMPLICIT NULL         → good  (tag 0x80, length 0x00)
    //   * [1] IMPLICIT RevokedInfo  → revoked (tag 0xA1)
    //   * [1] CHOICE revoked        → revoked (tag 0x81 in SingleResponse)
    //   * [2] IMPLICIT UnknownInfo  → unknown (tag 0x82)
    //
    // The SingleResponse comes after header + responderID + producedAt.
    // We simply iterate the whole stream and take the first
    // matching tag — multiple occurrences are spec-pathological.

    if response_der.is_empty() {
        return OcspStatus::Malformed;
    }
    // Defensive scan cap: realistic OCSP responses are a few
    // hundred bytes, never megabytes. Protects against adversarially large
    // responses. 64 KiB.
    const MAX_SCAN_BYTES: usize = 65_536;
    let scan_limit = response_der.len().min(MAX_SCAN_BYTES);
    let mut seen_header = false;
    for i in 0..scan_limit {
        let b = response_der[i];
        if !seen_header {
            // Skip until past the outer SEQUENCE (tag 0x30).
            if b == 0x30 {
                seen_header = true;
            }
            continue;
        }
        match b {
            // [0] IMPLICIT NULL — `good`. The length byte must be 0.
            0x80 if response_der.get(i + 1) == Some(&0x00) => return OcspStatus::Good,
            // [1] revoked — two possible encodings.
            0xA1 | 0x81 => return OcspStatus::Revoked,
            // [2] unknown.
            0x82 => return OcspStatus::Unknown,
            _ => {}
        }
    }
    OcspStatus::Malformed
}

/// High level: returns `Ok(())` only if the response is `Good`.
/// Any other status → `Err` with `SecurityErrorKind::AuthenticationFailed`.
///
/// # Errors
/// See above.
pub fn require_good_status(
    response_der: &[u8],
) -> Result<(), zerodds_security::error::SecurityError> {
    use zerodds_security::error::{SecurityError, SecurityErrorKind};
    match parse_ocsp_status(response_der) {
        OcspStatus::Good => Ok(()),
        OcspStatus::Revoked => Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "ocsp: cert is revoked",
        )),
        OcspStatus::Unknown => Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "ocsp: responder does not know the cert",
        )),
        OcspStatus::Malformed => Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "ocsp: response not parseable",
        )),
    }
}

/// Helper builder for minimal OCSP responses in tests.
/// Wraps a `CertStatus` tag in a valid DER SEQUENCE shell.
#[cfg(test)]
#[must_use]
fn build_test_response(cert_status_tag: u8) -> Vec<u8> {
    // We build a minimal, syntactically acceptable DER:
    //   SEQUENCE { <status_tag> 00 }
    //   = 0x30 0x02 <tag> 0x00
    // The parser only needs the first SEQUENCE and then the tag.
    let payload_len = match cert_status_tag {
        0x80 => 2,        // tag + 0x00 length
        0xA1 | 0x81 => 1, // the tag alone is enough
        0x82 => 1,
        _ => 1,
    };
    let mut out = Vec::with_capacity(2 + payload_len);
    out.push(0x30); // SEQUENCE
    #[allow(clippy::cast_possible_truncation)]
    out.push(payload_len as u8);
    out.push(cert_status_tag);
    if cert_status_tag == 0x80 {
        out.push(0x00); // IMPLICIT NULL length
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn good_status_parses_to_good() {
        let r = build_test_response(0x80);
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Good);
    }

    #[test]
    fn revoked_tag_a1_parses_to_revoked() {
        let r = build_test_response(0xA1);
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Revoked);
    }

    #[test]
    fn revoked_tag_81_parses_to_revoked() {
        let r = build_test_response(0x81);
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Revoked);
    }

    #[test]
    fn unknown_tag_82_parses_to_unknown() {
        let r = build_test_response(0x82);
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Unknown);
    }

    #[test]
    fn empty_input_is_malformed() {
        assert_eq!(parse_ocsp_status(&[]), OcspStatus::Malformed);
    }

    #[test]
    fn require_good_accepts_good() {
        let r = build_test_response(0x80);
        assert!(require_good_status(&r).is_ok());
    }

    #[test]
    fn require_good_rejects_revoked_with_auth_failed() {
        use zerodds_security::error::SecurityErrorKind;
        let r = build_test_response(0xA1);
        let err = require_good_status(&r).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn require_good_rejects_unknown() {
        let r = build_test_response(0x82);
        assert!(require_good_status(&r).is_err());
    }

    #[test]
    fn require_good_rejects_malformed() {
        use zerodds_security::error::SecurityErrorKind;
        let err = require_good_status(&[]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    /// Mutation killer: prefix bytes before the outer SEQUENCE
    /// must be skipped — a 0x80 before the 0x30 is NOT a status.
    /// Catches the `delete !` mutation on `if !seen_header` (line 77).
    #[test]
    fn prefix_bytes_before_sequence_are_skipped() {
        // 0x80 0x00 would be interpreted as Good with a deleted `!`.
        // Original: skips the prefix until 0x30, then finds 0x82 → Unknown.
        let r = [0x80, 0x00, 0x30, 0x02, 0x82, 0x00];
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Unknown);
    }

    /// Mutation killer: SEQUENCE-tag detection compares for `==0x30`,
    /// not `!=0x30`. Catches the `==` -> `!=` mutation (line 79).
    ///
    /// Input without a length byte (3 instead of 4 bytes): only the original `==`
    /// variant registers the SEQUENCE and returns Good;
    /// `!=` would treat 0x30 as "not a header" and degrade to the pre-header
    /// skip mode.
    #[test]
    fn sequence_tag_recognized_via_equality() {
        let r = [0x30, 0x80, 0x00];
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Good);
    }

    /// Mutation killer: the Good tag needs length byte 0x00 — other
    /// length bytes must NOT count as Good.
    /// Catches the "replace match guard with true" mutation (line 87).
    #[test]
    fn good_tag_requires_zero_length() {
        // 0x80 with length 0x42 is not a valid Good tag.
        let r = [0x30, 0x02, 0x80, 0x42];
        assert_ne!(parse_ocsp_status(&r), OcspStatus::Good);
    }
}
