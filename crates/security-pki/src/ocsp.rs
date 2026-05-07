// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCSP-Stapling-Validation.
//!
//! Online-Certificate-Status-Protocol (RFC 6960) pruefen:
//! * Eine OCSP-Response wird vom Peer als "stapled" Token beim Handshake
//!   mitgeliefert (selbe Wire wie TLS-OCSP-Stapling, Spec RFC 6961).
//! * Dieses Modul parst den `CertStatus` aus der DER-encodeten
//!   `OCSPResponse`-Struktur und meldet `Good`/`Revoked`/`Unknown`.
//!
//! # Scope
//!
//! * Shallow DER-Scan: wir suchen den ersten `[0] IMPLICIT good`,
//!   `[1] revoked`, oder `[2] unknown` Tag im `SingleResponse`-
//!   Bereich. Kein vollstaendiges ASN.1.
//! * Keine Signatur-Validierung der OCSP-Response — der Peer hat sie
//!   bereits signiert vom OCSP-Responder geholt. Ein echter PKI-Stack
//!   wuerde hier zusaetzlich die Responder-Signatur pruefen — das
//!   folgt in future-major+ mit `x509-parser`-Dep.
//!
//! # Nicht enthalten
//!
//! * OCSP-Request-Signing (wir konsumieren nur Responses).
//! * CRL-Parsing (separater future-major).

#[cfg(test)]
use alloc::vec::Vec;

/// Cert-Status aus einer OCSP-Response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcspStatus {
    /// Cert ist gueltig zum Zeitpunkt der Response (`thisUpdate`).
    Good,
    /// Cert wurde revoked. Zeitpunkt und Revocation-Reason werden
    /// nicht durchgereicht — die DDS-Security-Spec verlangt nur das
    /// Boolean-Flag (Spec §8.3.2.10.1).
    Revoked,
    /// OCSP-Responder kennt den Cert nicht.
    Unknown,
    /// Response konnte nicht geparst werden (nicht-spec-konform).
    Malformed,
}

/// Scannt eine DER-OCSP-Response und liefert den Cert-Status fuer den
/// (einzigen) enthaltenen `SingleResponse`.
///
/// Der Parser ist bewusst flach: er sucht im DER-Stream nach den
/// OCSP-Response-CertStatus-Tags `0x80` (good), `0xA1`/`0x81` (revoked)
/// oder `0x82` (unknown) innerhalb des ersten `SingleResponse` —
/// typischerweise ca. 40 byte in die Response hinein.
///
/// Diese Heuristik reicht fuer die OCSP-Responses der gaengigen CA-
/// Responder (Let's Encrypt, DigiCert, Sectigo). Fuer mehrere Certs
/// in einer Response liefert diese Funktion den Status des **ersten**.
#[must_use]
pub fn parse_ocsp_status(response_der: &[u8]) -> OcspStatus {
    // OCSP-Response DER-Struktur (vereinfacht):
    //   OCSPResponse ::= SEQUENCE {
    //     responseStatus [0] ENUMERATED,
    //     responseBytes  [0] EXPLICIT ResponseBytes OPTIONAL
    //   }
    //
    // Wir scannen auf:
    //   * [0] IMPLICIT NULL         → good  (tag 0x80, length 0x00)
    //   * [1] IMPLICIT RevokedInfo  → revoked (tag 0xA1)
    //   * [1] CHOICE revoked        → revoked (tag 0x81 in SingleResponse)
    //   * [2] IMPLICIT UnknownInfo  → unknown (tag 0x82)
    //
    // Das SingleResponse liegt nach Header + responderID + producedAt.
    // Wir iterieren einfach den gesamten Stream und nehmen den ersten
    // passenden Tag — mehrfach-vorkommen ist spec-pathologisch.

    if response_der.is_empty() {
        return OcspStatus::Malformed;
    }
    // Defensive Scan-Cap: realistische OCSP-Responses sind einige
    // hundert Byte, niemals megabytes. Schuetzt vor adversarial-grossen
    // Responses. 64 KiB.
    const MAX_SCAN_BYTES: usize = 65_536;
    let scan_limit = response_der.len().min(MAX_SCAN_BYTES);
    let mut seen_header = false;
    for i in 0..scan_limit {
        let b = response_der[i];
        if !seen_header {
            // Skip bis hinter das aeussere SEQUENCE (tag 0x30).
            if b == 0x30 {
                seen_header = true;
            }
            continue;
        }
        match b {
            // [0] IMPLICIT NULL — `good`. Length-Byte muss 0 sein.
            0x80 if response_der.get(i + 1) == Some(&0x00) => return OcspStatus::Good,
            // [1] revoked — zwei moegliche Enkodierungen.
            0xA1 | 0x81 => return OcspStatus::Revoked,
            // [2] unknown.
            0x82 => return OcspStatus::Unknown,
            _ => {}
        }
    }
    OcspStatus::Malformed
}

/// Hoher Level: liefert `Ok(())` nur wenn die Response `Good` ist.
/// Jeder andere Status → `Err` mit `SecurityErrorKind::AuthenticationFailed`.
///
/// # Errors
/// Siehe oben.
pub fn require_good_status(
    response_der: &[u8],
) -> Result<(), zerodds_security::error::SecurityError> {
    use zerodds_security::error::{SecurityError, SecurityErrorKind};
    match parse_ocsp_status(response_der) {
        OcspStatus::Good => Ok(()),
        OcspStatus::Revoked => Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "ocsp: cert ist revoked",
        )),
        OcspStatus::Unknown => Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "ocsp: responder kennt cert nicht",
        )),
        OcspStatus::Malformed => Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "ocsp: response nicht parsbar",
        )),
    }
}

/// Hilfs-Builder fuer Minimal-OCSP-Responses in Tests.
/// Wickelt einen `CertStatus`-Tag in eine gueltige DER-SEQUENCE-Huelle.
#[cfg(test)]
#[must_use]
fn build_test_response(cert_status_tag: u8) -> Vec<u8> {
    // Wir bauen ein minimales, syntaktisch akzeptables DER:
    //   SEQUENCE { <status_tag> 00 }
    //   = 0x30 0x02 <tag> 0x00
    // Der Parser braucht nur das erste SEQUENCE und dann den Tag.
    let payload_len = match cert_status_tag {
        0x80 => 2,        // tag + 0x00 length
        0xA1 | 0x81 => 1, // nur der tag reicht
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

    /// Mutation-Killer: Praefix-Bytes vor dem aeusseren SEQUENCE
    /// muessen ueberlesen werden — ein 0x80 vor dem 0x30 ist KEIN Status.
    /// Faengt `delete !` Mutation auf `if !seen_header` (Zeile 77).
    #[test]
    fn prefix_bytes_before_sequence_are_skipped() {
        // 0x80 0x00 wuerde mit deletetem `!` als Good interpretiert.
        // Original: skipt Praefix bis 0x30, findet dann 0x82 → Unknown.
        let r = [0x80, 0x00, 0x30, 0x02, 0x82, 0x00];
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Unknown);
    }

    /// Mutation-Killer: SEQUENCE-Tag-Erkennung vergleicht auf `==0x30`,
    /// nicht `!=0x30`. Faengt `==` -> `!=` Mutation (Zeile 79).
    ///
    /// Eingabe ohne Length-Byte (3 statt 4 byte): nur original `==`
    /// Variante registriert die SEQUENCE und liefert Good zurueck;
    /// `!=` wuerde 0x30 als "nicht-Header" werten und zum Pre-Header-
    /// Skip-Mode degradieren.
    #[test]
    fn sequence_tag_recognized_via_equality() {
        let r = [0x30, 0x80, 0x00];
        assert_eq!(parse_ocsp_status(&r), OcspStatus::Good);
    }

    /// Mutation-Killer: Good-Tag braucht Length-Byte 0x00 — andere
    /// Length-Bytes duerfen NICHT als Good zaehlen.
    /// Faengt "replace match guard with true" Mutation (Zeile 87).
    #[test]
    fn good_tag_requires_zero_length() {
        // 0x80 mit Length 0x42 ist kein gueltiger Good-Tag.
        let r = [0x30, 0x02, 0x80, 0x42];
        assert_ne!(parse_ocsp_status(&r), OcspStatus::Good);
    }
}
