// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CRL validation (certificate revocation list, RFC 5280 §5).
//!
//! Spec OMG DDS-Security 1.2 §8.8 requires a CRL fallback for the case
//! that no OCSP responder is reachable. This module parses a
//! DER-encoded `CertificateList`, extracts the list of revoked
//! serial numbers and provides a `validate_crl(crl, cert_serial)`
//! that returns `Err(AuthenticationFailed)` as soon as `cert_serial`
//! is in the list.
//!
//! # ASN.1-Struktur (RFC 5280 §5.1)
//!
//! ```text
//! CertificateList ::= SEQUENCE {
//!     tbsCertList         TBSCertList,
//!     signatureAlgorithm  AlgorithmIdentifier,
//!     signatureValue      BIT STRING
//! }
//!
//! TBSCertList ::= SEQUENCE {
//!     version             INTEGER OPTIONAL,
//!     signature           AlgorithmIdentifier,
//!     issuer              Name,
//!     thisUpdate          Time,
//!     nextUpdate          Time OPTIONAL,
//!     revokedCertificates SEQUENCE OF SEQUENCE {
//!         userCertificate     CertificateSerialNumber,  -- INTEGER
//!         revocationDate      Time,
//!         crlEntryExtensions  Extensions OPTIONAL
//!     } OPTIONAL,
//!     crlExtensions [0] EXPLICIT Extensions OPTIONAL
//! }
//! ```
//!
//! # Scope
//!
//! * Pragmatic DER walker — not a complete ASN.1 parser.
//! * Detects the `revokedCertificates` SEQUENCE heuristically via the
//!   inner pattern (INTEGER + UTCTime/GeneralizedTime), without modeling
//!   issuer / signatureAlgorithm explicitly.
//! * No signature validation of the CRL — the caller obtained it over
//!   a trustworthy path (HTTPS, fixed bundle). A
//!   later extension can hook in `webpki::SignedData`.
//!
//! # API
//!
//! * [`parse_crl_serials`] — extracts all revoked serials.
//! * [`validate_crl`] — checks `cert_serial` against the CRL.

extern crate alloc;

use alloc::vec::Vec;

use zerodds_security::error::{SecurityError, SecurityErrorKind};

const TAG_INTEGER: u8 = 0x02;
const TAG_SEQUENCE: u8 = 0x30;
const TAG_UTCTIME: u8 = 0x17;
const TAG_GENERALIZED_TIME: u8 = 0x18;

/// Local parse error. Mapped by [`validate_crl`] into
/// `SecurityError::BadArgument`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrlParseError {
    /// Input was empty.
    Empty,
    /// DER header (tag/length) could not be read.
    Truncated,
    /// The outer CertificateList is not a SEQUENCE.
    NotASequence,
    /// TBSCertList is missing or non-conformant.
    MissingTbsCertList,
    /// The `revokedCertificates` structure could not be located
    /// (may legitimately be empty — see [`parse_crl_serials`]).
    MissingRevokedList,
    /// A revoked entry contained no serial INTEGER.
    MissingSerial,
}

/// Parses a DER CRL and returns the list of all revoked serial bytes
/// (big-endian, INTEGER-encoded — a leading null byte for
/// non-negative serials is kept, RFC-5280-conformant).
///
/// A CRL **without** `revokedCertificates` (empty list) returns
/// `Ok(Vec::new())`.
///
/// # Errors
/// * [`CrlParseError`] on structural DER errors.
pub fn parse_crl_serials(crl_der: &[u8]) -> Result<Vec<Vec<u8>>, CrlParseError> {
    if crl_der.is_empty() {
        return Err(CrlParseError::Empty);
    }
    // CertificateList SEQUENCE
    let (tag, certlist_inner, _rest) = read_tlv(crl_der)?;
    if tag != TAG_SEQUENCE {
        return Err(CrlParseError::NotASequence);
    }
    // TBSCertList SEQUENCE (first inner element).
    let (tbs_tag, tbs_inner, _) = read_tlv(certlist_inner)?;
    if tbs_tag != TAG_SEQUENCE {
        return Err(CrlParseError::MissingTbsCertList);
    }
    // Iterate the TBS content; look for a SEQUENCE whose first inner element
    // is again a SEQUENCE that begins with INTEGER + UTCTime/GeneralizedTime
    // — that is `revokedCertificates`.
    let mut cursor = tbs_inner;
    while !cursor.is_empty() {
        let (t, inner, rest) = read_tlv(cursor).map_err(|_| CrlParseError::MissingRevokedList)?;
        cursor = rest;
        if t != TAG_SEQUENCE {
            continue;
        }
        if let Some(serials) = try_parse_revoked_list(inner)? {
            return Ok(serials);
        }
    }
    // No revokedCertificates structure → empty list (the RFC allows that).
    Ok(Vec::new())
}

/// Checks whether a given cert (by serial bytes) is valid against a
/// CRL.
///
/// # Errors
/// * `BadArgument` if the CRL is not parseable (matches the
///   OCSP module behavior).
/// * `AuthenticationFailed` if `cert_serial` is in the revoked
///   list.
pub fn validate_crl(crl_der: &[u8], cert_serial: &[u8]) -> Result<(), SecurityError> {
    let revoked = parse_crl_serials(crl_der).map_err(|e| {
        SecurityError::new(SecurityErrorKind::BadArgument, crl_parse_error_message(e))
    })?;
    if revoked.iter().any(|s| s.as_slice() == cert_serial) {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "crl: cert is revoked",
        ));
    }
    Ok(())
}

fn crl_parse_error_message(e: CrlParseError) -> &'static str {
    match e {
        CrlParseError::Empty => "crl: empty input",
        CrlParseError::Truncated => "crl: truncated DER",
        CrlParseError::NotASequence => "crl: outer is not a SEQUENCE",
        CrlParseError::MissingTbsCertList => "crl: TBSCertList missing",
        CrlParseError::MissingRevokedList => "crl: revokedCertificates malformed",
        CrlParseError::MissingSerial => "crl: revoked entry without serial",
    }
}

/// Tries to interpret `inner` as the content of the
/// `revokedCertificates` SEQUENCE. Returns `Ok(Some(serials))` on a match,
/// `Ok(None)` if the content does not fit the RevokedCert form
/// (the caller keeps iterating), `Err` on structural defects in a
/// path recognized as a match.
fn try_parse_revoked_list(inner: &[u8]) -> Result<Option<Vec<Vec<u8>>>, CrlParseError> {
    if inner.is_empty() {
        return Ok(None);
    }
    // Check the first inner TLV — must be a SEQUENCE whose inner begins with
    // INTEGER + UTCTime/GeneralizedTime.
    let (first_tag, first_inner, _) = match read_tlv(inner) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if first_tag != TAG_SEQUENCE {
        return Ok(None);
    }
    let (serial_tag, _serial_bytes, after_serial) = match read_tlv(first_inner) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if serial_tag != TAG_INTEGER {
        return Ok(None);
    }
    let (time_tag, _, _) = match read_tlv(after_serial) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if time_tag != TAG_UTCTIME && time_tag != TAG_GENERALIZED_TIME {
        return Ok(None);
    }

    // Match — iterate all revoked-cert entries and collect serials.
    let mut serials = Vec::new();
    let mut cursor = inner;
    while !cursor.is_empty() {
        let (t, entry_inner, rest) = read_tlv(cursor).map_err(|_| CrlParseError::Truncated)?;
        cursor = rest;
        if t != TAG_SEQUENCE {
            return Err(CrlParseError::MissingRevokedList);
        }
        let (serial_tag, serial_bytes, _) =
            read_tlv(entry_inner).map_err(|_| CrlParseError::MissingSerial)?;
        if serial_tag != TAG_INTEGER {
            return Err(CrlParseError::MissingSerial);
        }
        serials.push(serial_bytes.to_vec());
    }
    Ok(Some(serials))
}

/// Reads a DER TLV element and returns `(tag, value, rest)`.
fn read_tlv(buf: &[u8]) -> Result<(u8, &[u8], &[u8]), CrlParseError> {
    if buf.len() < 2 {
        return Err(CrlParseError::Truncated);
    }
    let tag = buf[0];
    let (len, header_len) = read_length(&buf[1..])?;
    let total = 1 + header_len + len;
    if buf.len() < total {
        return Err(CrlParseError::Truncated);
    }
    let value = &buf[1 + header_len..total];
    let rest = &buf[total..];
    Ok((tag, value, rest))
}

/// Reads a DER length field and returns `(length, length_field_bytes)`.
/// Supports short form (1 byte, < 0x80) and long form (0x81..0x84,
/// i.e. up to 4 length bytes — enough for all realistic CRLs).
fn read_length(buf: &[u8]) -> Result<(usize, usize), CrlParseError> {
    if buf.is_empty() {
        return Err(CrlParseError::Truncated);
    }
    let first = buf[0];
    if first < 0x80 {
        return Ok((first as usize, 1));
    }
    let n = (first & 0x7F) as usize;
    if n == 0 || n > 4 {
        // Indefinite length or too long, for DoS protection.
        return Err(CrlParseError::Truncated);
    }
    if buf.len() < 1 + n {
        return Err(CrlParseError::Truncated);
    }
    let mut len = 0usize;
    for &b in &buf[1..1 + n] {
        // Arithmetic form instead of `(len << 8) | b`: mathematically identical
        // for BE encoding (no bit overlap), but more mutation-detection-
        // friendly — `*` and `+` are not equivalent to each other.
        len = len * 256 + b as usize;
    }
    Ok((len, 1 + n))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Builds a minimal syntactically valid CRL with a
    /// `revokedCertificates` list of the given serials.
    ///
    /// Layout (all short form, length < 128):
    /// ```text
    ///   CertificateList SEQUENCE {
    ///     TBSCertList SEQUENCE {
    ///       <issuer-stub: SEQUENCE OF SET>          -- skipped by parser
    ///       <thisUpdate: UTCTime "260101000000Z">  -- skipped
    ///       revokedCertificates SEQUENCE OF SEQUENCE {
    ///         INTEGER serial,
    ///         UTCTime revocationDate
    ///       } *
    ///     }
    ///     -- omit signatureAlgorithm + signatureValue; the parser
    ///     -- does not need them.
    ///   }
    /// ```
    fn build_test_crl(revoked_serials: &[&[u8]]) -> Vec<u8> {
        // 1. issuer stub: SEQUENCE { SET {} } — empty, valid DER
        let issuer = der_seq(&der_set(&[]));
        // 2. thisUpdate: UTCTime "260101000000Z" (13 bytes)
        let this_update = der_utctime(b"260101000000Z");
        // 3. revokedCertificates SEQUENCE OF SEQUENCE
        let mut revoked_inner = Vec::new();
        for serial in revoked_serials {
            let entry = der_seq(
                &[
                    der_integer(serial).as_slice(),
                    der_utctime(b"260101000000Z").as_slice(),
                ]
                .concat(),
            );
            revoked_inner.extend_from_slice(&entry);
        }
        let revoked_seq = der_seq(&revoked_inner);
        // 4. TBSCertList = SEQUENCE { issuer, thisUpdate, revokedCertificates }
        let mut tbs = Vec::new();
        tbs.extend_from_slice(&issuer);
        tbs.extend_from_slice(&this_update);
        tbs.extend_from_slice(&revoked_seq);
        let tbs_seq = der_seq(&tbs);
        // 5. Outer CertificateList = SEQUENCE { TBSCertList }
        der_seq(&tbs_seq)
    }

    fn der_seq(inner: &[u8]) -> Vec<u8> {
        encode_tlv(TAG_SEQUENCE, inner)
    }

    fn der_set(inner: &[u8]) -> Vec<u8> {
        encode_tlv(0x31, inner)
    }

    fn der_integer(value: &[u8]) -> Vec<u8> {
        encode_tlv(TAG_INTEGER, value)
    }

    fn der_utctime(value: &[u8]) -> Vec<u8> {
        encode_tlv(TAG_UTCTIME, value)
    }

    fn encode_tlv(tag: u8, value: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(value.len() + 6);
        out.push(tag);
        encode_length(&mut out, value.len());
        out.extend_from_slice(value);
        out
    }

    fn encode_length(out: &mut Vec<u8>, len: usize) {
        if len < 0x80 {
            out.push(len as u8);
        } else if len < 0x100 {
            out.push(0x81);
            out.push(len as u8);
        } else {
            out.push(0x82);
            out.push((len >> 8) as u8);
            out.push((len & 0xFF) as u8);
        }
    }

    #[test]
    fn parse_serials_returns_all_revoked() {
        let crl = build_test_crl(&[&[0x01], &[0x02], &[0x03]]);
        let serials = parse_crl_serials(&crl).expect("parse");
        assert_eq!(serials, vec![vec![0x01], vec![0x02], vec![0x03]]);
    }

    #[test]
    fn parse_serials_empty_revocation_list() {
        let crl = build_test_crl(&[]);
        let serials = parse_crl_serials(&crl).expect("parse");
        assert!(serials.is_empty());
    }

    #[test]
    fn parse_serials_keeps_leading_zero_byte_for_positive_serials() {
        // RFC 5280: serials > 2^(8n-1) need a leading 0 byte.
        let crl = build_test_crl(&[&[0x00, 0xFF]]);
        let serials = parse_crl_serials(&crl).expect("parse");
        assert_eq!(serials, vec![vec![0x00, 0xFF]]);
    }

    #[test]
    fn parse_serials_handles_long_serial() {
        // 20 bytes — typical CA serial length.
        let serial: [u8; 20] = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
        ];
        let crl = build_test_crl(&[&serial]);
        let serials = parse_crl_serials(&crl).expect("parse");
        assert_eq!(serials.len(), 1);
        assert_eq!(serials[0], serial.to_vec());
    }

    #[test]
    fn validate_crl_known_revoked_rejects() {
        let crl = build_test_crl(&[&[0xAB, 0xCD]]);
        let err = validate_crl(&crl, &[0xAB, 0xCD]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn validate_crl_unknown_serial_passes() {
        let crl = build_test_crl(&[&[0xAB, 0xCD]]);
        assert!(validate_crl(&crl, &[0xFF, 0xEE]).is_ok());
    }

    #[test]
    fn validate_crl_against_empty_list_passes() {
        let crl = build_test_crl(&[]);
        assert!(validate_crl(&crl, &[0x01]).is_ok());
    }

    #[test]
    fn validate_crl_signature_invalid_rejects() {
        // Corrupt CRL — outer is not a SEQUENCE → BadArgument.
        let bad = vec![0x05, 0x00, 0x00, 0x00];
        let err = validate_crl(&bad, &[0x01]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn validate_crl_empty_input_returns_bad_argument() {
        let err = validate_crl(&[], &[0x01]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn validate_crl_truncated_input_returns_bad_argument() {
        // SEQUENCE header says 50 bytes, but only 5 present.
        let bad = vec![0x30, 0x32, 0x01, 0x02, 0x03];
        let err = validate_crl(&bad, &[0x01]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn parse_serials_rejects_non_sequence_outer() {
        // Outer is INTEGER instead of SEQUENCE.
        let bad = vec![0x02, 0x01, 0x00];
        let err = parse_crl_serials(&bad).unwrap_err();
        assert_eq!(err, CrlParseError::NotASequence);
    }

    #[test]
    fn parse_serials_rejects_empty_input() {
        let err = parse_crl_serials(&[]).unwrap_err();
        assert_eq!(err, CrlParseError::Empty);
    }

    #[test]
    fn parse_serials_handles_long_form_length() {
        // CRL with > 128 bytes of content → long-form length is needed.
        let mut serials_refs: Vec<Vec<u8>> = Vec::new();
        for i in 0..10u8 {
            serials_refs.push(vec![i, i.wrapping_add(1), i.wrapping_add(2)]);
        }
        let serials_slice: Vec<&[u8]> = serials_refs.iter().map(|v| v.as_slice()).collect();
        let crl = build_test_crl(&serials_slice);
        // The CRL must be > 128 bytes so the long-form length is triggered.
        assert!(crl.len() > 128, "test crl must trigger long-form length");
        let parsed = parse_crl_serials(&crl).expect("parse");
        assert_eq!(parsed.len(), 10);
        for (got, want) in parsed.iter().zip(serials_refs.iter()) {
            assert_eq!(got, want);
        }
    }

    #[test]
    fn parse_serials_rejects_indefinite_length() {
        // Indefinite length (0x80) is BER, not DER → Truncated.
        let bad = vec![0x30, 0x80, 0x00, 0x00];
        let err = parse_crl_serials(&bad).unwrap_err();
        // Length 0x80 = indefinite — we do not accept it.
        assert!(matches!(
            err,
            CrlParseError::Truncated | CrlParseError::MissingTbsCertList
        ));
    }

    #[test]
    fn validate_crl_with_two_revoked_finds_second() {
        // Make sure non-first serials are also detected.
        let crl = build_test_crl(&[&[0x01], &[0x02], &[0x03]]);
        let err = validate_crl(&crl, &[0x03]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    // ---- Mutation killers for crl_parse_error_message + read_length ----

    /// Catches mutation `crl_parse_error_message -> ""` and `-> "xyzzy"`.
    /// Each CrlParseError variant must return a specific message.
    #[test]
    fn parse_error_messages_are_specific_per_variant() {
        assert_eq!(
            crl_parse_error_message(CrlParseError::Empty),
            "crl: empty input"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::Truncated),
            "crl: truncated DER"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::NotASequence),
            "crl: outer is not a SEQUENCE"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::MissingTbsCertList),
            "crl: TBSCertList missing"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::MissingRevokedList),
            "crl: revokedCertificates malformed"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::MissingSerial),
            "crl: revoked entry without serial"
        );
    }

    /// Catches mutation `!=` -> `==` in `try_parse_revoked_list`.
    /// Time-tag check: a non-time tag must lead to Ok(None)
    /// (the caller keeps trying), not to a successful match.
    #[test]
    fn try_parse_revoked_list_rejects_non_time_tag() {
        // SEQUENCE { INTEGER 0x42, INTEGER 0x99 } — after the serial comes
        // another INTEGER, not UTCTime/GeneralizedTime → no match.
        let inner = [
            0x30, 0x06, // outer SEQUENCE, len=6
            0x02, 0x01, 0x42, // INTEGER serial=0x42
            0x02, 0x01, 0x99, // INTEGER (NICHT TIME!)
        ];
        let res = try_parse_revoked_list(&inner).expect("no err");
        assert!(res.is_none(), "non-time-tag must yield None, got {res:?}");
    }

    // --- read_length boundary tests ---

    /// Catches mutation `< 0x80` -> `<= 0x80` (line 228).
    /// 0x80 is the long-form marker (n=0 → DoS reject), not short form.
    #[test]
    fn read_length_0x80_is_long_form_marker_not_short() {
        // first=0x80, n=(0x80 & 0x7F)=0 → reject as indefinite/0
        let res = read_length(&[0x80, 0xAA]);
        assert!(matches!(res, Err(CrlParseError::Truncated)), "got {res:?}");
    }

    /// Catches mutation `||` -> `&&` (line 232).
    /// `n == 0 || n > 4` must reject BOTH cases — n=0 AND n=5+.
    /// Mutation `&&`: only n==0 AND n>4 (impossible) → the reject goes away.
    #[test]
    fn read_length_rejects_n_greater_than_four() {
        // first = 0x85 → n = 5 → must error.
        let buf = [0x85, 0x00, 0x00, 0x00, 0x00, 0x00];
        let res = read_length(&buf);
        assert!(matches!(res, Err(CrlParseError::Truncated)), "got {res:?}");
    }

    /// Catches mutation `>` -> `==` and `>` -> `>=` on `n > 4`.
    /// Boundary: n=4 must PASS (long form with 4 length bytes ok),
    /// n=5 must ERROR.
    #[test]
    fn read_length_n_equals_four_accepted() {
        // first=0x84 → n=4. Buffer has header + 4 length bytes + payload.
        let buf = [0x84, 0x00, 0x00, 0x01, 0x00, 0xAA, 0xBB];
        let (len, hdr) = read_length(&buf).expect("n=4 must be accepted");
        assert_eq!(len, 0x100);
        assert_eq!(hdr, 5);
    }

    /// Catches mutation `<` -> `==` and `<` -> `<=` on `buf.len() < 1+n`
    /// (line 236). A buffer of EXACTLY 1+n bytes must pass (enough).
    #[test]
    fn read_length_buf_exactly_one_plus_n_accepted() {
        // first=0x82 → n=2. 1 (length byte) + 2 (content) = 3-byte buffer.
        let buf = [0x82, 0x12, 0x34];
        let (len, hdr) = read_length(&buf).expect("buf.len()==1+n must succeed");
        assert_eq!(len, 0x1234);
        assert_eq!(hdr, 3);
    }

    /// Catches mutation `+` -> `*` on `1 + n`.
    /// With `*`: `buf.len() < 1*n = n`. For n=2: allows buf.len()<2,
    /// so a 1-byte buf would be Truncated, 2-byte ok. The original requires
    /// 3 bytes. A 2-byte buf MUST give Truncated.
    #[test]
    fn read_length_buf_one_plus_n_minus_one_truncated() {
        // first=0x82 → n=2. 1 length byte + 1 content byte = 2-byte buf.
        // Original: 2 < 3 → Truncated. Mutation `*`: 2 < 2 false → continues
        // → reads only 1 byte as length, panics via OOB on buf[2] or
        // an unexpected value.
        let buf = [0x82, 0x12];
        let res = read_length(&buf);
        assert!(matches!(res, Err(CrlParseError::Truncated)), "got {res:?}");
    }

    /// Catches mutation `<<` -> `>>` on `len << 8` (line 241).
    /// Multi-byte length encoding must be high-byte-first (BE).
    /// With `>>`: each iteration loses the previous content.
    #[test]
    fn read_length_two_byte_length_high_byte_first() {
        // first=0x82, length bytes=[0x01, 0x00] → len=256.
        // With the `>>` mutation: first iter: len = (0 >> 8) | 0x01 = 0x01.
        // second iter: len = (0x01 >> 8) | 0x00 = 0. → 0, not 256.
        let buf = [0x82, 0x01, 0x00];
        let (len, hdr) = read_length(&buf).expect("must parse");
        assert_eq!(len, 256, "multi-byte length must be BE — high byte first");
        assert_eq!(hdr, 3);
    }

    /// Catches mutation `|` -> `^` on the length accumulation (line 241).
    /// In this loop OR and XOR are mathematically equivalent (no
    /// bit overlap because of the `<< 8` shift). This test would NOT
    /// discriminate — the mutation is equivalent. cargo-mutants
    /// does not recognize that; the test serves as documentation, not as a
    /// killer.
    ///
    /// We assert the correct result for multiple set bytes anyway —
    /// if the shift changes (e.g. << 4),
    /// OR and XOR would suddenly diverge and the test
    /// catches it indirectly.
    #[test]
    fn read_length_three_byte_length_correct() {
        // 0x83 0xFF 0x00 0xFF → len = 0xFF00FF
        let buf = [0x83, 0xFF, 0x00, 0xFF];
        let (len, hdr) = read_length(&buf).expect("must parse");
        assert_eq!(len, 0xFF_00_FF);
        assert_eq!(hdr, 4);
    }
}
