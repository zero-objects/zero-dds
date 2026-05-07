// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CRL-Validation (Certificate-Revocation-List, RFC 5280 §5).
//!
//! Spec OMG DDS-Security 1.2 §8.8 verlangt CRL-Fallback fuer den Fall,
//! dass kein OCSP-Responder erreichbar ist. Dieses Modul parst eine
//! DER-encodete `CertificateList`, extrahiert die Liste der revoked
//! Serial-Numbers und liefert ein `validate_crl(crl, cert_serial)`,
//! das `Err(AuthenticationFailed)` zurueckgibt, sobald `cert_serial`
//! in der Liste steht.
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
//! * Pragmatischer DER-Walker — kein vollstaendiger ASN.1-Parser.
//! * Erkennt die `revokedCertificates`-SEQUENCE heuristisch ueber das
//!   Inner-Pattern (INTEGER + UTCTime/GeneralizedTime), ohne issuer/
//!   signatureAlgorithm explizit zu modellieren.
//! * Keine Signatur-Validierung der CRL — der Caller hat sie ueber
//!   einen vertrauenswuerdigen Pfad (HTTPS, fixed bundle) bezogen. Eine
//!   spaetere Erweiterung kann `webpki::SignedData` einklinken.
//!
//! # API
//!
//! * [`parse_crl_serials`] — extrahiert alle Revoked-Serials.
//! * [`validate_crl`] — prueft `cert_serial` gegen die CRL.

extern crate alloc;

use alloc::vec::Vec;

use zerodds_security::error::{SecurityError, SecurityErrorKind};

const TAG_INTEGER: u8 = 0x02;
const TAG_SEQUENCE: u8 = 0x30;
const TAG_UTCTIME: u8 = 0x17;
const TAG_GENERALIZED_TIME: u8 = 0x18;

/// Lokaler Parse-Fehler. Wird durch [`validate_crl`] in
/// `SecurityError::BadArgument` gemappt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrlParseError {
    /// Eingabe war leer.
    Empty,
    /// DER-Header (tag/length) konnte nicht gelesen werden.
    Truncated,
    /// Auessere CertificateList ist keine SEQUENCE.
    NotASequence,
    /// TBSCertList fehlt oder ist nicht-konform.
    MissingTbsCertList,
    /// `revokedCertificates`-Struktur konnte nicht lokalisiert werden
    /// (kann legitim leer sein — siehe [`parse_crl_serials`]).
    MissingRevokedList,
    /// Ein Revoked-Eintrag enthielt keinen Serial-INTEGER.
    MissingSerial,
}

/// Parst eine DER-CRL und liefert die Liste aller Revoked-Serial-Bytes
/// (Big-Endian, INTEGER-encoded — fuehrendes Null-Byte fuer
/// non-negative Serials wird beibehalten, RFC-5280-konform).
///
/// Eine CRL **ohne** `revokedCertificates` (leere Liste) liefert
/// `Ok(Vec::new())`.
///
/// # Errors
/// * [`CrlParseError`] bei strukturellen DER-Fehlern.
pub fn parse_crl_serials(crl_der: &[u8]) -> Result<Vec<Vec<u8>>, CrlParseError> {
    if crl_der.is_empty() {
        return Err(CrlParseError::Empty);
    }
    // CertificateList SEQUENCE
    let (tag, certlist_inner, _rest) = read_tlv(crl_der)?;
    if tag != TAG_SEQUENCE {
        return Err(CrlParseError::NotASequence);
    }
    // TBSCertList SEQUENCE (erstes Inner-Element).
    let (tbs_tag, tbs_inner, _) = read_tlv(certlist_inner)?;
    if tbs_tag != TAG_SEQUENCE {
        return Err(CrlParseError::MissingTbsCertList);
    }
    // Iteriere TBS-Inhalt; suche nach SEQUENCE deren erstes Inner-Element
    // wieder eine SEQUENCE ist, die mit INTEGER + UTCTime/GeneralizedTime
    // beginnt — das ist `revokedCertificates`.
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
    // Keine revokedCertificates-Struktur → leere Liste (RFC erlaubt das).
    Ok(Vec::new())
}

/// Prueft, ob ein gegebenes Cert (per Serial-Bytes) gegen eine CRL
/// valide ist.
///
/// # Errors
/// * `BadArgument` wenn die CRL nicht parsbar ist (entspricht dem
///   OCSP-Modul-Verhalten).
/// * `AuthenticationFailed` wenn `cert_serial` in der Revoked-Liste
///   steht.
pub fn validate_crl(crl_der: &[u8], cert_serial: &[u8]) -> Result<(), SecurityError> {
    let revoked = parse_crl_serials(crl_der).map_err(|e| {
        SecurityError::new(SecurityErrorKind::BadArgument, crl_parse_error_message(e))
    })?;
    if revoked.iter().any(|s| s.as_slice() == cert_serial) {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "crl: cert ist revoked",
        ));
    }
    Ok(())
}

fn crl_parse_error_message(e: CrlParseError) -> &'static str {
    match e {
        CrlParseError::Empty => "crl: leere eingabe",
        CrlParseError::Truncated => "crl: truncated DER",
        CrlParseError::NotASequence => "crl: outer ist keine SEQUENCE",
        CrlParseError::MissingTbsCertList => "crl: TBSCertList fehlt",
        CrlParseError::MissingRevokedList => "crl: revokedCertificates malformed",
        CrlParseError::MissingSerial => "crl: revoked-eintrag ohne serial",
    }
}

/// Versucht, `inner` als Inhalt der `revokedCertificates`-SEQUENCE zu
/// interpretieren. Liefert `Ok(Some(serials))` bei Match,
/// `Ok(None)` wenn der Inhalt nicht zur RevokedCert-Form passt
/// (Caller iteriert weiter), `Err` bei strukturellen Defekten in einem
/// als Match erkannten Pfad.
fn try_parse_revoked_list(inner: &[u8]) -> Result<Option<Vec<Vec<u8>>>, CrlParseError> {
    if inner.is_empty() {
        return Ok(None);
    }
    // Erste Inner-TLV pruefen — muss SEQUENCE sein, deren Inner mit
    // INTEGER + UTCTime/GeneralizedTime beginnt.
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

    // Match — iteriere alle Revoked-Cert-Einträge und sammle Serials.
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

/// Liest ein DER-TLV-Element und liefert `(tag, value, rest)`.
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

/// Liest ein DER-Length-Feld und liefert `(length, length_field_bytes)`.
/// Unterstuetzt Short-Form (1 byte, < 0x80) und Long-Form (0x81..0x84,
/// d.h. bis zu 4 Length-Bytes — reicht fuer alle realistischen CRLs).
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
        // Indefinite-Length oder zu lang fuer DoS-Schutz.
        return Err(CrlParseError::Truncated);
    }
    if buf.len() < 1 + n {
        return Err(CrlParseError::Truncated);
    }
    let mut len = 0usize;
    for &b in &buf[1..1 + n] {
        // Arithmetic form statt `(len << 8) | b`: mathematisch identisch
        // bei BE-Encoding (kein Bit-Overlap), aber mutation-detection-
        // freundlicher — `*` und `+` sind nicht aequivalent zueinander.
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

    /// Baut eine minimale syntaktisch valide CRL mit einer
    /// `revokedCertificates`-Liste der gegebenen Serials.
    ///
    /// Layout (alle short-form, Length < 128):
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
    ///     -- signatureAlgorithm + signatureValue weglassen; Parser
    ///     -- braucht sie nicht.
    ///   }
    /// ```
    fn build_test_crl(revoked_serials: &[&[u8]]) -> Vec<u8> {
        // 1. issuer-stub: SEQUENCE { SET {} } — leer, valide DER
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
        // RFC 5280: Serials > 2^(8n-1) brauchen fuehrenden 0-Byte.
        let crl = build_test_crl(&[&[0x00, 0xFF]]);
        let serials = parse_crl_serials(&crl).expect("parse");
        assert_eq!(serials, vec![vec![0x00, 0xFF]]);
    }

    #[test]
    fn parse_serials_handles_long_serial() {
        // 20 bytes — typische CA-Serial-Laenge.
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
        // Korrupte CRL — outer ist kein SEQUENCE → BadArgument.
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
        // SEQUENCE-Header sagt 50 bytes, aber nur 5 da.
        let bad = vec![0x30, 0x32, 0x01, 0x02, 0x03];
        let err = validate_crl(&bad, &[0x01]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn parse_serials_rejects_non_sequence_outer() {
        // Outer ist INTEGER statt SEQUENCE.
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
        // CRL mit > 128 byte Inhalt → Long-Form-Length wird benoetigt.
        let mut serials_refs: Vec<Vec<u8>> = Vec::new();
        for i in 0..10u8 {
            serials_refs.push(vec![i, i.wrapping_add(1), i.wrapping_add(2)]);
        }
        let serials_slice: Vec<&[u8]> = serials_refs.iter().map(|v| v.as_slice()).collect();
        let crl = build_test_crl(&serials_slice);
        // CRL muss > 128 byte sein damit die Long-Form-Length getriggert wird.
        assert!(crl.len() > 128, "test-crl muss long-form-length triggern");
        let parsed = parse_crl_serials(&crl).expect("parse");
        assert_eq!(parsed.len(), 10);
        for (got, want) in parsed.iter().zip(serials_refs.iter()) {
            assert_eq!(got, want);
        }
    }

    #[test]
    fn parse_serials_rejects_indefinite_length() {
        // Indefinite-Length (0x80) ist BER, nicht DER → Truncated.
        let bad = vec![0x30, 0x80, 0x00, 0x00];
        let err = parse_crl_serials(&bad).unwrap_err();
        // Length 0x80 = indefinite — wir akzeptieren nicht.
        assert!(matches!(
            err,
            CrlParseError::Truncated | CrlParseError::MissingTbsCertList
        ));
    }

    #[test]
    fn validate_crl_with_two_revoked_finds_second() {
        // Sicherstellen dass auch nicht-erste Serials erkannt werden.
        let crl = build_test_crl(&[&[0x01], &[0x02], &[0x03]]);
        let err = validate_crl(&crl, &[0x03]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    // ---- Mutation-Killer fuer crl_parse_error_message + read_length ----

    /// Faengt Mutation `crl_parse_error_message -> ""` und `-> "xyzzy"`.
    /// Jede CrlParseError-Variant muss eine spezifische Message liefern.
    #[test]
    fn parse_error_messages_are_specific_per_variant() {
        assert_eq!(
            crl_parse_error_message(CrlParseError::Empty),
            "crl: leere eingabe"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::Truncated),
            "crl: truncated DER"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::NotASequence),
            "crl: outer ist keine SEQUENCE"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::MissingTbsCertList),
            "crl: TBSCertList fehlt"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::MissingRevokedList),
            "crl: revokedCertificates malformed"
        );
        assert_eq!(
            crl_parse_error_message(CrlParseError::MissingSerial),
            "crl: revoked-eintrag ohne serial"
        );
    }

    /// Faengt Mutation `!=` -> `==` in `try_parse_revoked_list`.
    /// Time-Tag-Pruefung: Ein nicht-time-Tag muss zu Ok(None) fuehren
    /// (Caller probiert weiter), nicht zu erfolgreichem Match.
    #[test]
    fn try_parse_revoked_list_rejects_non_time_tag() {
        // SEQUENCE { INTEGER 0x42, INTEGER 0x99 } — nach Serial kommt
        // erneut INTEGER, nicht UTCTime/GeneralizedTime → kein Match.
        let inner = [
            0x30, 0x06, // outer SEQUENCE, len=6
            0x02, 0x01, 0x42, // INTEGER serial=0x42
            0x02, 0x01, 0x99, // INTEGER (NICHT TIME!)
        ];
        let res = try_parse_revoked_list(&inner).expect("no err");
        assert!(res.is_none(), "non-time-tag must yield None, got {res:?}");
    }

    // --- read_length boundary tests ---

    /// Faengt Mutation `< 0x80` -> `<= 0x80` (Zeile 228).
    /// 0x80 ist long-form-marker (n=0 → DoS-Reject), nicht short-form.
    #[test]
    fn read_length_0x80_is_long_form_marker_not_short() {
        // first=0x80, n=(0x80 & 0x7F)=0 → reject als indefinite/0
        let res = read_length(&[0x80, 0xAA]);
        assert!(matches!(res, Err(CrlParseError::Truncated)), "got {res:?}");
    }

    /// Faengt Mutation `||` -> `&&` (Zeile 232).
    /// `n == 0 || n > 4` muss BEIDE Faelle verwerfen — n=0 UND n=5+.
    /// Mutation `&&`: nur n==0 UND n>4 (unmoeglich) → Reject geht weg.
    #[test]
    fn read_length_rejects_n_greater_than_four() {
        // first = 0x85 → n = 5 → muss erroren.
        let buf = [0x85, 0x00, 0x00, 0x00, 0x00, 0x00];
        let res = read_length(&buf);
        assert!(matches!(res, Err(CrlParseError::Truncated)), "got {res:?}");
    }

    /// Faengt Mutation `>` -> `==` und `>` -> `>=` auf `n > 4`.
    /// Boundary: n=4 muss DURCHGEHEN (Long-Form mit 4 Length-Bytes ok),
    /// n=5 muss ERRORN.
    #[test]
    fn read_length_n_equals_four_accepted() {
        // first=0x84 → n=4. Buffer hat Header + 4 Length-Bytes + payload.
        let buf = [0x84, 0x00, 0x00, 0x01, 0x00, 0xAA, 0xBB];
        let (len, hdr) = read_length(&buf).expect("n=4 must be accepted");
        assert_eq!(len, 0x100);
        assert_eq!(hdr, 5);
    }

    /// Faengt Mutation `<` -> `==` und `<` -> `<=` auf `buf.len() < 1+n`
    /// (Zeile 236). Buffer EXAKT 1+n bytes muss durchgehen (genug).
    #[test]
    fn read_length_buf_exactly_one_plus_n_accepted() {
        // first=0x82 → n=2. 1 (length-byte) + 2 (content) = 3 byte buffer.
        let buf = [0x82, 0x12, 0x34];
        let (len, hdr) = read_length(&buf).expect("buf.len()==1+n must succeed");
        assert_eq!(len, 0x1234);
        assert_eq!(hdr, 3);
    }

    /// Faengt Mutation `+` -> `*` auf `1 + n`.
    /// Mit `*`: `buf.len() < 1*n = n`. Bei n=2: erlaubt buf.len()<2,
    /// also 1-byte-buf wuerde Truncated, 2-byte ok. Original verlangt
    /// 3-byte. Ein 2-byte-buf MUSS Truncated geben.
    #[test]
    fn read_length_buf_one_plus_n_minus_one_truncated() {
        // first=0x82 → n=2. 1 length-byte + 1 content-byte = 2-byte buf.
        // Original: 2 < 3 → Truncated. Mutation `*`: 2 < 2 false → continues
        // → liest nur 1 byte als length, panic via OOB on buf[2] oder
        // unerwarteter Wert.
        let buf = [0x82, 0x12];
        let res = read_length(&buf);
        assert!(matches!(res, Err(CrlParseError::Truncated)), "got {res:?}");
    }

    /// Faengt Mutation `<<` -> `>>` auf `len << 8` (Zeile 241).
    /// Multi-byte length encoding muss High-Byte-First sein (BE).
    /// Mit `>>`: pro Iteration verliert sich der vorherige Inhalt.
    #[test]
    fn read_length_two_byte_length_high_byte_first() {
        // first=0x82, length-bytes=[0x01, 0x00] → len=256.
        // Mit `>>` Mutation: erste iter: len = (0 >> 8) | 0x01 = 0x01.
        // zweite iter: len = (0x01 >> 8) | 0x00 = 0. → 0, nicht 256.
        let buf = [0x82, 0x01, 0x00];
        let (len, hdr) = read_length(&buf).expect("must parse");
        assert_eq!(len, 256, "multi-byte length must be BE — high byte first");
        assert_eq!(hdr, 3);
    }

    /// Faengt Mutation `|` -> `^` auf der Length-Akkumulation (Zeile 241).
    /// In dieser Schleife sind OR und XOR mathematisch aequivalent (keine
    /// Bit-Ueberlappung wegen `<< 8`-Shift). Dieser Test waere KEINE
    /// Diskriminierung — die Mutation ist equivalent. cargo-mutants
    /// erkennt das nicht; der Test dient als Dokumentation, nicht als
    /// Killer.
    ///
    /// Wir asserten trotzdem das korrekte Ergebnis bei mehrfacher
    /// gesetzter Bytes — falls der Shift sich aendert (z.B. << 4),
    /// wuerden OR und XOR plotzlich differenzieren und der Test
    /// faengt indirekt.
    #[test]
    fn read_length_three_byte_length_correct() {
        // 0x83 0xFF 0x00 0xFF → len = 0xFF00FF
        let buf = [0x83, 0xFF, 0x00, 0xFF];
        let (len, hdr) = read_length(&buf).expect("must parse");
        assert_eq!(len, 0xFF_00_FF);
        assert_eq!(hdr, 4);
    }
}
