// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `IdentityToken` (PKI-DH) — DDS-Security 1.2 §10.3.2.1.
//!
//! Wire-Form (Spec Tab.36):
//! ```text
//!   class_id  = "DDS:Auth:PKI-DH:1.0"
//!   properties:
//!     dds.cert.sn   — subject name of the identity cert (RFC 4514 string)
//!     dds.cert.algo — signature-algorithm OID name
//!     dds.ca.sn     — subject name of the issuing CA
//!     dds.ca.algo   — CA signature algorithm
//! ```
//!
//! Delivered as a built-in-topic entry to the discovery DataReader
//! and lets the remote check, before handshake setup, whether it
//! may talk to this identity at all (e.g. CA whitelist).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::identity::PkiError;

/// Class ID per spec §10.3.2.1.
pub const IDENTITY_TOKEN_CLASS_ID: &str = "DDS:Auth:PKI-DH:1.0";

/// `dds.cert.sn` — subject name (RFC 4514).
pub const KEY_CERT_SN: &str = "dds.cert.sn";
/// `dds.cert.algo` — signature algorithm.
pub const KEY_CERT_ALGO: &str = "dds.cert.algo";
/// `dds.ca.sn` — issuer subject name.
pub const KEY_CA_SN: &str = "dds.ca.sn";
/// `dds.ca.algo` — issuer signature algorithm.
pub const KEY_CA_ALGO: &str = "dds.ca.algo";

/// `IdentityToken` (PKI-DH).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdentityToken {
    /// Subject name of the local identity cert (RFC 4514 string).
    pub cert_sn: String,
    /// Signature algorithm of the cert (e.g. `EC-prime256v1`).
    pub cert_algo: String,
    /// Subject name of the issuing CA.
    pub ca_sn: String,
    /// Signature algorithm of the CA.
    pub ca_algo: String,
}

impl IdentityToken {
    /// Constructor.
    #[must_use]
    pub fn new(cert_sn: String, cert_algo: String, ca_sn: String, ca_algo: String) -> Self {
        Self {
            cert_sn,
            cert_algo,
            ca_sn,
            ca_algo,
        }
    }

    /// Encode to wire bytes.
    ///
    /// Spec standard: the IdentityToken IS a `DataHolder` (DDS-Security
    /// §7.2.7), CDR-LE serialized (class_id string + property sequence).
    /// EXACTLY this format is expected/parsed by FastDDS/Cyclone/RTI — the
    /// custom TLV encoding previously used here was not cross-vendor-readable.
    /// `DataHolder` also covers the RTPS parameter padding (its decoder
    /// reads structure-driven and ignores trailing padding).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        use zerodds_security::token::DataHolder;
        DataHolder::new(IDENTITY_TOKEN_CLASS_ID)
            .with_property(KEY_CERT_SN, self.cert_sn.as_str())
            .with_property(KEY_CERT_ALGO, self.cert_algo.as_str())
            .with_property(KEY_CA_SN, self.ca_sn.as_str())
            .with_property(KEY_CA_ALGO, self.ca_algo.as_str())
            .to_cdr_le()
    }

    /// Decode Wire-Bytes (CDR-LE `DataHolder`, siehe [`encode`](Self::encode)).
    ///
    /// # Errors
    /// `PkiError::InvalidPem` on format problems, missing
    /// mandatory properties, or a class-ID mismatch.
    pub fn decode(bytes: &[u8]) -> Result<Self, PkiError> {
        use zerodds_security::token::DataHolder;
        let dh = DataHolder::from_cdr_le(bytes)
            .map_err(|e| PkiError::InvalidPem(format!("IdentityToken CDR decode: {e:?}")))?;
        if dh.class_id != IDENTITY_TOKEN_CLASS_ID {
            return Err(PkiError::InvalidPem(format!(
                "IdentityToken class-id mismatch: `{}`",
                dh.class_id
            )));
        }
        let req = |k: &str| -> Result<String, PkiError> {
            dh.property(k)
                .map(ToString::to_string)
                .ok_or_else(|| PkiError::InvalidPem(format!("missing {k}")))
        };
        Ok(Self {
            cert_sn: req(KEY_CERT_SN)?,
            cert_algo: req(KEY_CERT_ALGO)?,
            ca_sn: req(KEY_CA_SN)?,
            ca_algo: req(KEY_CA_ALGO)?,
        })
    }
}

/// Spec §7.4.3 cert-bind: `subject_match` checks whether a local
/// IdentityToken matches a permissions-document subject.
///
/// Both sides must produce an identical, canonical RFC-4514 form.
/// We compare with trim+lowercase because the subject
/// renderers sometimes differ in the capitalization of OID names
/// (e.g. `CN` vs `cn`).
#[must_use]
pub fn subject_match(token_subject: &str, permissions_subject: &str) -> bool {
    canonicalize_subject(token_subject) == canonicalize_subject(permissions_subject)
}

fn canonicalize_subject(s: &str) -> String {
    // Remove whitespace around `=` and `,`, all lowercase.
    let mut out = String::with_capacity(s.len());
    let mut prev_was_sep = true;
    for c in s.chars() {
        if c.is_whitespace() && prev_was_sep {
            continue;
        }
        if c == ',' || c == '=' {
            // If the last output character is whitespace, remove it.
            while let Some(last) = out.chars().last() {
                if last.is_whitespace() {
                    out.pop();
                } else {
                    break;
                }
            }
            out.push(c);
            prev_was_sep = true;
            continue;
        }
        prev_was_sep = false;
        for lc in c.to_lowercase() {
            out.push(lc);
        }
    }
    out.trim().to_string()
}

/// Extracts subject + issuer from a PEM identity cert + PEM CA cert
/// and builds a complete `IdentityToken` from them.
///
/// `cert_pem`/`ca_pem` must each contain the **first** cert in a
/// PEM block.
///
/// # Errors
/// `PkiError::InvalidPem` on parse errors.
pub fn build_identity_token_from_pem(
    cert_pem: &[u8],
    ca_pem: &[u8],
) -> Result<IdentityToken, PkiError> {
    use x509_cert::Certificate;
    use x509_cert::der::DecodePem;

    let cert_str =
        core::str::from_utf8(cert_pem).map_err(|_| PkiError::InvalidPem("cert pem utf8".into()))?;
    let ca_str =
        core::str::from_utf8(ca_pem).map_err(|_| PkiError::InvalidPem("ca pem utf8".into()))?;
    let cert = Certificate::from_pem(cert_str.as_bytes())
        .map_err(|e| PkiError::InvalidPem(format!("cert: {e}")))?;
    let ca = Certificate::from_pem(ca_str.as_bytes())
        .map_err(|e| PkiError::InvalidPem(format!("ca: {e}")))?;
    Ok(identity_token_from_certs(&cert, &ca))
}

/// Like [`build_identity_token_from_pem`], but from DER bytes — for the
/// plugin path, where identity + trust anchor are already available as DER
/// (`ParsedIdentity`). Needed for `get_identity_token` (SPDP announce).
///
/// # Errors
/// `PkiError::InvalidPem` (the variant also covers DER decode errors)
/// on non-parseable certificates.
pub fn build_identity_token_from_der(
    cert_der: &[u8],
    ca_der: &[u8],
) -> Result<IdentityToken, PkiError> {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;

    let cert = Certificate::from_der(cert_der)
        .map_err(|e| PkiError::InvalidPem(format!("cert der: {e}")))?;
    let ca =
        Certificate::from_der(ca_der).map_err(|e| PkiError::InvalidPem(format!("ca der: {e}")))?;
    Ok(identity_token_from_certs(&cert, &ca))
}

fn identity_token_from_certs(
    cert: &x509_cert::Certificate,
    ca: &x509_cert::Certificate,
) -> IdentityToken {
    IdentityToken {
        cert_sn: subject_oneline(&cert.tbs_certificate.subject),
        cert_algo: spec_cert_algo(&cert.signature_algorithm.oid.to_string()),
        ca_sn: subject_oneline(&ca.tbs_certificate.subject),
        ca_algo: spec_cert_algo(&ca.signature_algorithm.oid.to_string()),
    }
}

/// OpenSSL `X509_NAME_print_ex(.., XN_FLAG_ONELINE)` short name for an
/// attribute OID. For unknown OIDs the numeric OID string (like OpenSSL).
fn oid_short_name(oid: &str) -> String {
    match oid {
        "2.5.4.3" => "CN",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.5" => "serialNumber",
        "2.5.4.4" => "SN",
        "2.5.4.12" => "title",
        "2.5.4.42" => "GN",
        "1.2.840.113549.1.9.1" => "emailAddress",
        "0.9.2342.19200300.100.1.25" => "DC",
        "0.9.2342.19200300.100.1.1" => "UID",
        other => return other.to_string(),
    }
    .to_string()
}

/// Formats an X.509 subject name in OpenSSL `XN_FLAG_ONELINE` style:
/// DER order (NOT reversed), `Attr = value`, `, ` separator.
///
/// OpenDDS' `Certificate::subject_name_to_str` uses exactly
/// `XN_FLAG_ONELINE` by default and compares the received `dds.ca.sn` by
/// string equality (`AuthenticationBuiltInImpl::validate_remote_identity`).
/// RFC-4514 (`CN=...`, reversed, without spaces) would be rejected there as
/// "dds.ca.sn doesn't match local" → no auth handshake.
/// cyclone/FastDDS do not compare the property (lenient), so the
/// ONELINE form is cross-vendor universal.
fn subject_oneline(name: &x509_cert::name::Name) -> String {
    let mut parts: Vec<String> = Vec::new();
    for rdn in name.0.iter() {
        for atv in rdn.0.iter() {
            let attr = oid_short_name(&atv.oid.to_string());
            let val = core::str::from_utf8(atv.value.value())
                .unwrap_or_default()
                .to_string();
            parts.push(format!("{attr} = {val}"));
        }
    }
    parts.join(", ")
}

/// Maps the cert signature-algorithm OID to the OMG-DDS-Security `dds.cert.algo`
/// string (§9.3.2.1: `EC-prime256v1` | `RSA-2048`). FastDDS/Cyclone expect
/// these spec names, NOT the raw OID (`1.2.840.10045.4.3.2`) — otherwise the
/// foreign vendor rejects the IdentityToken. Unknown OIDs fall back to the
/// raw OID (better than nothing).
fn spec_cert_algo(oid: &str) -> String {
    match oid {
        // ecdsa-with-SHA256 / -SHA384 / -SHA512 → P-256 family (bench: P-256).
        "1.2.840.10045.4.3.2" | "1.2.840.10045.4.3.3" | "1.2.840.10045.4.3.4" => {
            "EC-prime256v1".to_string()
        }
        // sha256/384/512WithRSAEncryption + RSASSA-PSS.
        "1.2.840.113549.1.1.11"
        | "1.2.840.113549.1.1.12"
        | "1.2.840.113549.1.1.13"
        | "1.2.840.113549.1.1.10" => "RSA-2048".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_full_token() {
        let t = IdentityToken::new(
            "CN=alice,O=Example,C=DE".into(),
            "ecdsa-with-SHA256".into(),
            "CN=Example CA,O=Example,C=DE".into(),
            "ecdsa-with-SHA256".into(),
        );
        let bytes = t.encode();
        let back = IdentityToken::decode(&bytes).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn decode_tolerates_trailing_rtps_parameter_padding() {
        // FU2 S4: the identity_token is sent as an RTPS parameter (PID_IDENTITY_
        // TOKEN). RTPS pads parameter values to 4-byte alignment;
        // the parser returns the value INCL. the null-padding bytes.
        // `decode` MUST tolerate this trailing padding — otherwise the
        // cross-process/cross-vendor auth handshake fails (observed in reality:
        // 194-byte token → 196 on the wire → decode error).
        let t = IdentityToken::new(
            "CN=ping,O=ZeroDDS,C=DE".into(),
            "ecdsa-with-SHA256".into(),
            "CN=ZeroDDS CA,O=ZeroDDS,C=DE".into(),
            "ecdsa-with-SHA256".into(),
        );
        let mut bytes = t.encode();
        // Pad to the next 4-byte multiple (like RTPS).
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        bytes.push(0);
        bytes.push(0); // extra padding, better safe than sorry
        let back = IdentityToken::decode(&bytes).expect("decode must tolerate padding");
        assert_eq!(back, t);
    }

    #[test]
    fn class_id_constant_matches_spec() {
        assert_eq!(IDENTITY_TOKEN_CLASS_ID, "DDS:Auth:PKI-DH:1.0");
    }

    #[test]
    fn class_id_mismatch_rejected() {
        let mut bytes = IdentityToken::default().encode();
        bytes[5] ^= 0xff;
        assert!(IdentityToken::decode(&bytes).is_err());
    }

    #[test]
    fn missing_required_property_rejected() {
        // Encode without `dds.ca.algo`.
        let mut bytes = Vec::new();
        bytes.push(IDENTITY_TOKEN_CLASS_ID.len() as u8);
        bytes.extend_from_slice(IDENTITY_TOKEN_CLASS_ID.as_bytes());
        for (k, v) in [(KEY_CERT_SN, "x"), (KEY_CERT_ALGO, "x"), (KEY_CA_SN, "x")] {
            bytes.push(k.len() as u8);
            bytes.extend_from_slice(k.as_bytes());
            bytes.extend_from_slice(&(v.len() as u16).to_be_bytes());
            bytes.extend_from_slice(v.as_bytes());
        }
        let err = IdentityToken::decode(&bytes).unwrap_err();
        assert!(matches!(err, PkiError::InvalidPem(_)));
    }

    #[test]
    fn subject_match_canonicalizes_whitespace() {
        assert!(subject_match("CN=alice, O=Example", "CN=alice,O=Example"));
        assert!(subject_match(
            "CN = alice , O = Example",
            "cn=alice,o=example"
        ));
    }

    #[test]
    fn subject_match_case_insensitive() {
        assert!(subject_match("CN=Alice", "cn=alice"));
    }

    #[test]
    fn subject_match_rejects_different_cn() {
        assert!(!subject_match("CN=alice", "CN=bob"));
    }

    #[test]
    fn subject_match_rejects_extra_attribute() {
        assert!(!subject_match("CN=alice", "CN=alice,O=Example"));
    }

    #[test]
    fn unknown_property_skipped_in_decode() {
        let mut bytes = IdentityToken::new("a".into(), "b".into(), "c".into(), "d".into()).encode();
        // Appending an unknown property — should skip silently.
        let unk_key = b"future_field";
        bytes.push(unk_key.len() as u8);
        bytes.extend_from_slice(unk_key);
        bytes.extend_from_slice(&(3u16.to_be_bytes()));
        bytes.extend_from_slice(b"xyz");
        let t = IdentityToken::decode(&bytes).unwrap();
        assert_eq!(t.cert_sn, "a");
    }

    #[test]
    fn build_token_from_pem_cert() {
        // Self-signed cert via rcgen.
        let params = rcgen::CertificateParams::new(alloc::vec!["alice.example".to_string()])
            .expect("rcgen params");
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let pem = cert.pem();
        // Self-signed: cert is also CA.
        let token = build_identity_token_from_pem(pem.as_bytes(), pem.as_bytes()).unwrap();
        assert!(!token.cert_sn.is_empty());
        assert!(!token.cert_algo.is_empty());
        assert_eq!(token.cert_sn, token.ca_sn, "self-signed: subject == issuer");
    }
}
