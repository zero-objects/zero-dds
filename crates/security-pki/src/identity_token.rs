// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `IdentityToken` (PKI-DH) — DDS-Security 1.2 §10.3.2.1.
//!
//! Wire form (DDS-Security 1.2 §10.3.2.1 Table 51):
//! ```text
//!   class_id  = "DDS:Auth:PKI-DH:1.0"
//!   properties (the presence of each is optional):
//!     dds.cert.sn   — subject name of the identity cert
//!     dds.cert.algo — CryptoAlgorithmName of the digital signature algorithm (Table 25)
//!     dds.ca.sn     — subject name of the issuing CA
//!     dds.ca.algo   — CryptoAlgorithmName of the digital signature algorithm (Table 25)
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
        // "The presence of each of properties is optional" (Table 51): a
        // property without a value is left out.
        let mut token = DataHolder::new(IDENTITY_TOKEN_CLASS_ID);
        for (name, value) in [
            (KEY_CERT_SN, &self.cert_sn),
            (KEY_CERT_ALGO, &self.cert_algo),
            (KEY_CA_SN, &self.ca_sn),
            (KEY_CA_ALGO, &self.ca_algo),
        ] {
            if !value.is_empty() {
                token.set_property(name, value.as_str());
            }
        }
        token.to_cdr_le()
    }

    /// Decodes wire bytes (CDR-LE `DataHolder`, see [`encode`](Self::encode)).
    ///
    /// A property the token does not carry is empty ("The presence of each of
    /// properties is optional", Table 51); one the plugin does not know is
    /// ignored.
    ///
    /// # Errors
    /// `PkiError::InvalidPem` on format problems or a class-ID mismatch.
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
        let optional =
            |k: &str| -> String { dh.property(k).map(ToString::to_string).unwrap_or_default() };
        Ok(Self {
            cert_sn: optional(KEY_CERT_SN),
            cert_algo: optional(KEY_CERT_ALGO),
            ca_sn: optional(KEY_CA_SN),
            ca_algo: optional(KEY_CA_ALGO),
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
    build_identity_token_from_pem_named(cert_pem, ca_pem, false)
}

/// [`build_identity_token_from_pem`] with the choice of the algorithm names:
/// the `CryptoAlgorithmName`s of DDS-Security 1.2 Table 25, or with
/// `legacy_names` those of 1.1 (`EC-prime256v1`, `RSA-2048`).
///
/// # Errors
/// `PkiError::InvalidPem` on parse errors.
pub fn build_identity_token_from_pem_named(
    cert_pem: &[u8],
    ca_pem: &[u8],
    legacy_names: bool,
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
    Ok(identity_token_from_certs(
        &cert,
        &ca,
        legacy_names,
        zerodds_security::SubjectNameForm::default(),
    ))
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
    build_identity_token_from_der_named(cert_der, ca_der, false)
}

/// [`build_identity_token_from_der`] with the choice of the algorithm names,
/// see [`build_identity_token_from_pem_named`].
///
/// # Errors
/// `PkiError::InvalidPem` on non-parseable certificates.
pub fn build_identity_token_from_der_named(
    cert_der: &[u8],
    ca_der: &[u8],
    legacy_names: bool,
) -> Result<IdentityToken, PkiError> {
    build_identity_token_from_der_with(
        cert_der,
        ca_der,
        legacy_names,
        zerodds_security::SubjectNameForm::default(),
    )
}

/// [`build_identity_token_from_der_named`] with the form of the subject names
/// (`dds.cert.sn`, `dds.ca.sn`), see [`zerodds_security::SubjectNameForm`].
///
/// # Errors
/// `PkiError::InvalidPem` on non-parseable certificates.
pub fn build_identity_token_from_der_with(
    cert_der: &[u8],
    ca_der: &[u8],
    legacy_names: bool,
    form: zerodds_security::SubjectNameForm,
) -> Result<IdentityToken, PkiError> {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;

    let cert = Certificate::from_der(cert_der)
        .map_err(|e| PkiError::InvalidPem(format!("cert der: {e}")))?;
    let ca =
        Certificate::from_der(ca_der).map_err(|e| PkiError::InvalidPem(format!("ca der: {e}")))?;
    Ok(identity_token_from_certs(&cert, &ca, legacy_names, form))
}

fn identity_token_from_certs(
    cert: &x509_cert::Certificate,
    ca: &x509_cert::Certificate,
    legacy_names: bool,
    form: zerodds_security::SubjectNameForm,
) -> IdentityToken {
    let name = |certificate: &x509_cert::Certificate| {
        certificate_algorithm_name(
            &certificate.signature_algorithm.oid.to_string(),
            legacy_names,
        )
    };
    IdentityToken {
        cert_sn: format_subject_name(&cert.tbs_certificate.subject, form),
        cert_algo: name(cert),
        ca_sn: format_subject_name(&ca.tbs_certificate.subject, form),
        ca_algo: name(ca),
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

/// The string of an X.509 subject name in the form `form` names, for the
/// properties `dds.cert.sn`, `dds.ca.sn` and `dds.perm_ca.sn`.
///
/// * [`SubjectNameForm::Rfc4514`](zerodds_security::SubjectNameForm::Rfc4514):
///   RFC 4514 (DDS-Security 1.2 §9.4.1.2.3), `CN=y,O=x,C=DE`.
/// * `X509NameOneline`: OpenSSL `X509_NAME_oneline(name, 0, 0)`, each
///   attribute as `/SN=value` in certificate order, a byte outside
///   0x20..=0x7E as `\xHH`.
/// * `XnFlagOneline`: `subject_oneline`.
#[must_use]
pub fn format_subject_name(
    name: &x509_cert::name::Name,
    form: zerodds_security::SubjectNameForm,
) -> String {
    use zerodds_security::SubjectNameForm;
    match form {
        SubjectNameForm::Rfc4514 => name.to_string(),
        SubjectNameForm::XnFlagOneline => subject_oneline(name),
        SubjectNameForm::X509NameOneline => {
            let mut out = String::new();
            for rdn in name.0.iter() {
                for atv in rdn.0.iter() {
                    out.push('/');
                    out.push_str(&oid_short_name(&atv.oid.to_string()));
                    out.push('=');
                    for byte in atv.value.value() {
                        if (0x20..=0x7e).contains(byte) {
                            out.push(char::from(*byte));
                        } else {
                            out.push_str(&format!("\\x{byte:02X}"));
                        }
                    }
                }
            }
            out
        }
    }
}

/// Subject of a certificate DER, in the `XN_FLAG_ONELINE` spelling
/// [`subject_oneline`] produces.
///
/// # Errors
/// [`PkiError::CertInvalid`] if the DER does not parse as a certificate.
pub(crate) fn subject_of_der(der: &[u8]) -> Result<String, crate::identity::PkiError> {
    use x509_cert::der::Decode;
    let cert = x509_cert::Certificate::from_der(der)
        .map_err(|e| crate::identity::PkiError::CertInvalid(format!("subject: parse: {e}")))?;
    Ok(subject_oneline(&cert.tbs_certificate.subject))
}

/// Formats an X.509 subject name in OpenSSL `XN_FLAG_ONELINE` style:
/// DER order (NOT reversed), `Attr = value`, `, ` separator.
///
/// OpenDDS' `Certificate::subject_name_to_str` uses exactly
/// `XN_FLAG_ONELINE` by default and compares the received `dds.ca.sn` by
/// string equality (`AuthenticationBuiltInImpl::validate_remote_identity`).
/// This is also the internal form in which ZeroDDS compares subject names
/// (`subject_match`); the form of the tokens it sends is
/// [`format_subject_name`] (RFC 4514 by default).
pub(crate) fn subject_oneline(name: &x509_cert::name::Name) -> String {
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

/// The `CryptoAlgorithmName` of DDS-Security 1.2 §8.2 Table 25 for the
/// signature algorithm of a certificate, which `dds.cert.algo` and
/// `dds.ca.algo` carry (§10.3.2.1 Table 51). A signature algorithm without a
/// name in Table 25 (for example ecdsa-with-SHA512) gives an empty string,
/// and the property is left out: its presence is optional.
fn crypto_algorithm_name(oid: &str) -> String {
    match oid {
        // ecdsa-with-SHA256
        "1.2.840.10045.4.3.2" => "ECDSA+P256+SHA256",
        // ecdsa-with-SHA384
        "1.2.840.10045.4.3.3" => "ECDSA+P384+SHA384",
        // sha256WithRSAEncryption
        "1.2.840.113549.1.1.11" => "RSASSA-PKCS1-V1_5+2048+SHA256",
        // id-RSASSA-PSS
        "1.2.840.113549.1.1.10" => "RSASSA-PSS-MGF1SHA256+2048+SHA256",
        _ => "",
    }
    .to_string()
}

/// The names of DDS-Security 1.1 (`EC-prime256v1`, `RSA-2048`) for the
/// signature algorithm OID of a certificate: what OpenDDS compares with its
/// own value. Not a spec 1.2 name, only written with the setting
/// `org.zerodds.sec.auth.algorithm_names=legacy`. An OID without a name
/// stays as the OID text.
fn legacy_cert_algo(oid: &str) -> String {
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

/// The name a token carries for the signature algorithm `oid` of a
/// certificate: `dds.cert.algo` and `dds.ca.algo` of the IdentityToken
/// (Table 51) and `dds.perm_ca.algo` of the PermissionsToken (Table 65). The
/// name of Table 25, or with `legacy` the one of DDS-Security 1.1
/// (`EC-prime256v1`, `RSA-2048`). An empty string where Table 25 has no name.
#[must_use]
pub fn certificate_algorithm_name(oid: &str, legacy: bool) -> String {
    if legacy {
        legacy_cert_algo(oid)
    } else {
        crypto_algorithm_name(oid)
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

    /// Table 51: "The presence of each of properties is optional".
    #[test]
    fn a_token_without_the_optional_properties_decodes() {
        use zerodds_security::token::DataHolder;
        let bytes = DataHolder::new(IDENTITY_TOKEN_CLASS_ID)
            .with_property(KEY_CERT_SN, "CN = x")
            .to_cdr_le();
        let token = IdentityToken::decode(&bytes).unwrap();
        assert_eq!(token.cert_sn, "CN = x");
        assert!(token.cert_algo.is_empty() && token.ca_sn.is_empty() && token.ca_algo.is_empty());
        let bare = DataHolder::new(IDENTITY_TOKEN_CLASS_ID).to_cdr_le();
        assert_eq!(
            IdentityToken::decode(&bare).unwrap(),
            IdentityToken::default()
        );
    }

    /// A property without a value is not written.
    #[test]
    fn an_empty_property_is_left_out_of_the_token() {
        use zerodds_security::token::DataHolder;
        let token = IdentityToken::new(
            "CN = a".into(),
            String::new(),
            "CN = ca".into(),
            String::new(),
        );
        let holder = DataHolder::from_cdr_le(&token.encode()).unwrap();
        let names: Vec<&str> = holder.properties.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, [KEY_CERT_SN, KEY_CA_SN]);
    }

    /// Table 51: `dds.cert.algo` and `dds.ca.algo` are names of Table 25.
    #[test]
    fn the_algorithms_are_the_names_of_table_25() {
        let cases = [
            ("1.2.840.10045.4.3.2", "ECDSA+P256+SHA256"),
            ("1.2.840.10045.4.3.3", "ECDSA+P384+SHA384"),
            ("1.2.840.113549.1.1.11", "RSASSA-PKCS1-V1_5+2048+SHA256"),
            ("1.2.840.113549.1.1.10", "RSASSA-PSS-MGF1SHA256+2048+SHA256"),
            // ecdsa-with-SHA512 has no name in Table 25.
            ("1.2.840.10045.4.3.4", ""),
        ];
        for (oid, name) in cases {
            assert_eq!(crypto_algorithm_name(oid), name, "{oid}");
        }
    }

    /// The names of 1.1 are the opt-in for OpenDDS.
    #[test]
    fn the_names_of_1_1_are_the_opt_in() {
        let params = rcgen::CertificateParams::new(alloc::vec!["alice.example".to_string()])
            .expect("rcgen params");
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let pem = cert.pem();
        let spec = build_identity_token_from_pem(pem.as_bytes(), pem.as_bytes()).unwrap();
        assert_eq!(spec.cert_algo, "ECDSA+P256+SHA256");
        assert_eq!(spec.ca_algo, "ECDSA+P256+SHA256");
        let legacy =
            build_identity_token_from_pem_named(pem.as_bytes(), pem.as_bytes(), true).unwrap();
        assert_eq!(legacy.cert_algo, "EC-prime256v1");
        assert_eq!(legacy.ca_algo, "EC-prime256v1");
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
    /// The forms of `dds.cert.sn` and `dds.ca.sn`. The expected strings are
    /// those of the sources: RFC 4514 (DDS-Security 1.2 §9.4.1.2.3),
    /// `X509_NAME_oneline` (Fast DDS `FileProvider.cpp`, Cyclone DDS
    /// `auth_utils.c`) and `XN_FLAG_ONELINE` (OpenDDS
    /// `AuthenticationBuiltInImpl.cpp`) for the name `C=DE, O=x, CN=y` in
    /// certificate order.
    #[test]
    fn the_subject_names_have_the_form_that_is_set() {
        use zerodds_security::SubjectNameForm;
        let mut params = rcgen::CertificateParams::new(alloc::vec!["y".to_string()]).unwrap();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CountryName, "DE");
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "x");
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "y");
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let der = cert.der().to_vec();
        let token = |form| build_identity_token_from_der_with(&der, &der, false, form).unwrap();
        let default = build_identity_token_from_der(&der, &der).unwrap();
        assert_eq!(default.cert_sn, "CN=y,O=x,C=DE", "the default is RFC 4514");
        assert_eq!(default.ca_sn, "CN=y,O=x,C=DE");
        for (form, expected) in [
            (SubjectNameForm::Rfc4514, "CN=y,O=x,C=DE"),
            (SubjectNameForm::X509NameOneline, "/C=DE/O=x/CN=y"),
            (SubjectNameForm::XnFlagOneline, "C = DE, O = x, CN = y"),
        ] {
            let t = token(form);
            assert_eq!(t.cert_sn, expected, "{form:?}");
            assert_eq!(t.ca_sn, expected, "{form:?}");
        }
    }

    /// `X509_NAME_oneline` writes a byte outside 0x20..=0x7E as `\xHH`.
    #[test]
    fn x509_name_oneline_escapes_what_openssl_escapes() {
        use zerodds_security::SubjectNameForm;
        let mut params = rcgen::CertificateParams::new(alloc::vec!["y".to_string()]).unwrap();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "caf\u{e9}");
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let token = build_identity_token_from_der_with(
            cert.der(),
            cert.der(),
            false,
            SubjectNameForm::X509NameOneline,
        )
        .unwrap();
        assert_eq!(token.cert_sn, "/CN=caf\\xC3\\xA9");
    }
}
