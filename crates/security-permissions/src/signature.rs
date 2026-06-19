// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! S/MIME signature trait + dev helper for permissions/governance XML.
//!
//! Spec §9.4.1.2.1 requires that the permissions and governance XML be
//! signed with the **permissions CA**. The format is S/MIME
//! with a PKCS#7/CMS envelope — typical use: `openssl cms -sign`.
//! The production PKCS#7/CMS verifier lives in [`crate::cms`].
//!
//! # What is defined here
//!
//! * Trait [`XmlSignatureVerifier`] as an abstraction over the
//!   verify step.
//! * [`NoOpVerifier`] — an explicitly documented dev helper that
//!   skips the signature check (`SignedPermissionsXml::open` with
//!   `NoOpVerifier` is meant only for development and tests — production
//!   applications use `cms::CmsVerifier`).
//! * [`EnvelopeCheckVerifier`] — a formal smoke verifier that checks the
//!   S/MIME envelope for plausibility, **without** really checking the
//!   signature. Also a dev helper.
//! * [`open_signed_permissions`] encapsulates the "verify first, then
//!   parse" flow.

use alloc::string::String;
use alloc::vec::Vec;

use crate::xml::{Permissions, PermissionsError, parse_permissions_xml};

/// Abstraction over the S/MIME verify step.
pub trait XmlSignatureVerifier {
    /// Checks the signature of a permissions or governance XML.
    ///
    /// `signed_doc` is the **raw S/MIME container** (including
    /// PEM headers like `-----BEGIN PKCS7-----`). The verifier
    /// extracts the inner XML content and verifies the
    /// signature against the permissions CA.
    ///
    /// Returns: the **verified** inner XML bytes for
    /// downstream parsing.
    ///
    /// # Errors
    /// Implementation-specific; tends toward
    /// `PermissionsError::Malformed` on signature or
    /// format problems.
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError>;
}

/// No-op verifier for development — accepts **any** input as
/// valid and treats it as plaintext XML. **NEVER** use in production.
pub struct NoOpVerifier;

impl XmlSignatureVerifier for NoOpVerifier {
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
        Ok(signed_doc.to_vec())
    }
}

/// Simple envelope verifier for tests and a pseudo-signature.
///
/// Expects a wrapper format `-----BEGIN SIGNED-XML-----\n<XML>\n-----END SIGNED-XML-----`
/// and extracts the XML block. The signature part here is only the
/// envelope presence (no real crypto check) — the purpose is end-
/// to-end tests of the verifier call chain.
pub struct EnvelopeCheckVerifier;

impl XmlSignatureVerifier for EnvelopeCheckVerifier {
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
        const BEGIN: &str = "-----BEGIN SIGNED-XML-----\n";
        const END: &str = "\n-----END SIGNED-XML-----";
        let s = core::str::from_utf8(signed_doc)
            .map_err(|_| PermissionsError::Malformed("signed-xml is not UTF-8".into()))?;
        let body = s
            .strip_prefix(BEGIN)
            .and_then(|rest| rest.strip_suffix(END))
            .ok_or_else(|| {
                PermissionsError::Malformed(String::from(
                    "signed-xml: envelope BEGIN/END missing or malformed",
                ))
            })?;
        Ok(body.as_bytes().to_vec())
    }
}

/// High-level wrapper: verifies the signature, parses the permissions XML.
///
/// # Errors
/// Signature or XML parse error.
pub fn open_signed_permissions<V: XmlSignatureVerifier>(
    signed_doc: &[u8],
    verifier: &V,
) -> Result<Permissions, PermissionsError> {
    let inner = verifier.verify_and_extract(signed_doc)?;
    let xml = core::str::from_utf8(&inner)
        .map_err(|_| PermissionsError::Malformed("verified XML is not UTF-8".into()))?;
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
