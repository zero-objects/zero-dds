// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! S/MIME-Signatur-Trait + Dev-Helper fuer Permissions/Governance-XML.
//!
//! Spec §9.4.1.2.1 verlangt, dass Permissions- und Governance-XML
//! mit der **Permissions-CA** signiert sind. Das Format ist S/MIME
//! mit PKCS#7/CMS-Envelope — typischer Gebrauch: `openssl cms -sign`.
//! Der produktive PKCS#7/CMS-Verifier lebt in [`crate::cms`].
//!
//! # Was hier definiert ist
//!
//! * Trait [`XmlSignatureVerifier`] als Abstraktion ueber den
//!   Verify-Schritt.
//! * [`NoOpVerifier`] — explizit dokumentierter Dev-Helper, der die
//!   Signatur-Pruefung ueberspringt (`SignedPermissionsXml::open` mit
//!   `NoOpVerifier` ist nur fuer Development und Tests gedacht — produktive
//!   Anwendungen verwenden `cms::CmsVerifier`).
//! * [`EnvelopeCheckVerifier`] — formaler Smoke-Verifier, der den
//!   S/MIME-Envelope auf Plausibilitaet prueft, **ohne** die Signatur
//!   wirklich zu pruefen. Auch Dev-Helper.
//! * [`open_signed_permissions`] kapselt den "erst verifizieren, dann
//!   parsen"-Flow.

use alloc::string::String;
use alloc::vec::Vec;

use crate::xml::{Permissions, PermissionsError, parse_permissions_xml};

/// Abstraktion fuer den S/MIME-Verify-Schritt.
pub trait XmlSignatureVerifier {
    /// Prueft die Signatur einer Permissions- oder Governance-XML.
    ///
    /// `signed_doc` ist der **rohe S/MIME-Container** (inklusive
    /// PEM-Headers wie `-----BEGIN PKCS7-----`). Der Verifier
    /// extrahiert den inneren XML-Content und verifiziert die
    /// Signatur gegen die Permissions-CA.
    ///
    /// Rueckgabe: die **verifizierten** inneren XML-Bytes fuer
    /// nachgelagertes Parsing.
    ///
    /// # Errors
    /// Implementierung-spezifisch; tendiert nach
    /// `PermissionsError::Malformed` bei Signatur- oder
    /// Format-Problemen.
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError>;
}

/// No-op-Verifier fuer Development — akzeptiert **jedes** Input als
/// gueltig und behandelt es als Klartext-XML. **NIE** in Produktion
/// einsetzen.
pub struct NoOpVerifier;

impl XmlSignatureVerifier for NoOpVerifier {
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
        Ok(signed_doc.to_vec())
    }
}

/// Simple-Envelope-Verifier fuer Tests und Pseudo-Signatur.
///
/// Erwartet ein Wrapper-Format `-----BEGIN SIGNED-XML-----\n<XML>\n-----END SIGNED-XML-----`
/// und extrahiert den XML-Block. Der Signatur-Teil ist hier nur die
/// Envelope-Praesenz (kein echter Crypto-Check) — der Zweck ist End-
/// to-End-Tests der Verifier-Aufruf-Kette.
pub struct EnvelopeCheckVerifier;

impl XmlSignatureVerifier for EnvelopeCheckVerifier {
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
        const BEGIN: &str = "-----BEGIN SIGNED-XML-----\n";
        const END: &str = "\n-----END SIGNED-XML-----";
        let s = core::str::from_utf8(signed_doc)
            .map_err(|_| PermissionsError::Malformed("signed-xml ist kein UTF-8".into()))?;
        let body = s
            .strip_prefix(BEGIN)
            .and_then(|rest| rest.strip_suffix(END))
            .ok_or_else(|| {
                PermissionsError::Malformed(String::from(
                    "signed-xml: envelope BEGIN/END fehlt oder ist fehlerhaft",
                ))
            })?;
        Ok(body.as_bytes().to_vec())
    }
}

/// High-Level-Wrapper: verifiziert Signatur, parst Permissions-XML.
///
/// # Errors
/// Signatur- oder XML-Parse-Fehler.
pub fn open_signed_permissions<V: XmlSignatureVerifier>(
    signed_doc: &[u8],
    verifier: &V,
) -> Result<Permissions, PermissionsError> {
    let inner = verifier.verify_and_extract(signed_doc)?;
    let xml = core::str::from_utf8(&inner)
        .map_err(|_| PermissionsError::Malformed("verified XML ist kein UTF-8".into()))?;
    parse_permissions_xml(xml)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const RAW_XML: &str = r#"
<permissions>
  <grant><subject_name>CN=alice</subject_name>
    <allow_rule><publish><topic>T</topic></publish></allow_rule>
  </grant>
</permissions>
"#;

    #[test]
    fn noop_verifier_passes_through() {
        let perms = open_signed_permissions(RAW_XML.as_bytes(), &NoOpVerifier).unwrap();
        assert_eq!(perms.grants.len(), 1);
    }

    #[test]
    fn envelope_verifier_extracts_inner_xml() {
        let wrapped =
            alloc::format!("-----BEGIN SIGNED-XML-----\n{RAW_XML}\n-----END SIGNED-XML-----");
        let perms = open_signed_permissions(wrapped.as_bytes(), &EnvelopeCheckVerifier).unwrap();
        assert_eq!(perms.grants.len(), 1);
        assert_eq!(perms.grants[0].subject_name, "CN=alice");
    }

    #[test]
    fn envelope_verifier_rejects_missing_begin() {
        let bad = b"no envelope here";
        let err = open_signed_permissions(bad, &EnvelopeCheckVerifier).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn envelope_verifier_rejects_missing_end() {
        let bad = b"-----BEGIN SIGNED-XML-----\n<permissions/>\n";
        let err = open_signed_permissions(bad, &EnvelopeCheckVerifier).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn verifier_failure_propagates_malformed() {
        struct AlwaysFail;
        impl XmlSignatureVerifier for AlwaysFail {
            fn verify_and_extract(&self, _doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
                Err(PermissionsError::Malformed("signature mismatch".into()))
            }
        }
        let err = open_signed_permissions(RAW_XML.as_bytes(), &AlwaysFail).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(m) if m.contains("mismatch")));
    }

    #[test]
    fn non_utf8_inner_is_rejected() {
        struct BinaryVerifier;
        impl XmlSignatureVerifier for BinaryVerifier {
            fn verify_and_extract(&self, _doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
                Ok(vec![0xff, 0xfe, 0x00]) // not UTF-8
            }
        }
        let err = open_signed_permissions(b"", &BinaryVerifier).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }
}
