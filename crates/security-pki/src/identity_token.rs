// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `IdentityToken` (PKI-DH) — DDS-Security 1.2 §10.3.2.1.
//!
//! Wire-Form (Spec Tab.36):
//! ```text
//!   class_id  = "DDS:Auth:PKI-DH:1.0"
//!   properties:
//!     dds.cert.sn   — Subject-Name des Identity-Cert (RFC 4514 String)
//!     dds.cert.algo — Signature-Algorithm-OID-Name
//!     dds.ca.sn     — Subject-Name der Issuing-CA
//!     dds.ca.algo   — CA-Signature-Algorithm
//! ```
//!
//! Wird als Builtin-Topic-Eintrag in den Discovery-DataReader geliefert
//! und erlaubt dem Remote, vor dem Handshake-Setup zu pruefen, ob er
//! ueberhaupt mit dieser Identity sprechen darf (z.B. CA-Whitelist).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::identity::PkiError;

/// Class-ID laut Spec §10.3.2.1.
pub const IDENTITY_TOKEN_CLASS_ID: &str = "DDS:Auth:PKI-DH:1.0";

/// `dds.cert.sn` — Subject-Name (RFC 4514).
pub const KEY_CERT_SN: &str = "dds.cert.sn";
/// `dds.cert.algo` — Signature-Algorithm.
pub const KEY_CERT_ALGO: &str = "dds.cert.algo";
/// `dds.ca.sn` — Issuer-Subject-Name.
pub const KEY_CA_SN: &str = "dds.ca.sn";
/// `dds.ca.algo` — Issuer-Signature-Algorithm.
pub const KEY_CA_ALGO: &str = "dds.ca.algo";

/// `IdentityToken` (PKI-DH).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdentityToken {
    /// Subject-Name des lokalen Identity-Certs (RFC 4514 String).
    pub cert_sn: String,
    /// Signatur-Algorithmus des Certs (z.B. `EC-prime256v1`).
    pub cert_algo: String,
    /// Subject-Name der Issuing-CA.
    pub ca_sn: String,
    /// Signatur-Algorithmus der CA.
    pub ca_algo: String,
}

impl IdentityToken {
    /// Konstruktor.
    #[must_use]
    pub fn new(cert_sn: String, cert_algo: String, ca_sn: String, ca_algo: String) -> Self {
        Self {
            cert_sn,
            cert_algo,
            ca_sn,
            ca_algo,
        }
    }

    /// Encode zu Wire-Bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(IDENTITY_TOKEN_CLASS_ID.len() as u8);
        out.extend_from_slice(IDENTITY_TOKEN_CLASS_ID.as_bytes());
        for (k, v) in [
            (KEY_CERT_SN, &self.cert_sn),
            (KEY_CERT_ALGO, &self.cert_algo),
            (KEY_CA_SN, &self.ca_sn),
            (KEY_CA_ALGO, &self.ca_algo),
        ] {
            out.push(k.len() as u8);
            out.extend_from_slice(k.as_bytes());
            let v_bytes = v.as_bytes();
            out.extend_from_slice(&(v_bytes.len() as u16).to_be_bytes());
            out.extend_from_slice(v_bytes);
        }
        out
    }

    /// Decode Wire-Bytes.
    ///
    /// # Errors
    /// `PkiError::InvalidPem` bei Format-Problemen, fehlenden
    /// Pflicht-Properties oder Class-ID-Mismatch.
    pub fn decode(bytes: &[u8]) -> Result<Self, PkiError> {
        let mut pos = 0usize;
        if bytes.is_empty() {
            return Err(PkiError::InvalidPem("IdentityToken empty".into()));
        }
        let cid_len = bytes[pos] as usize;
        pos += 1;
        if bytes.len() < pos + cid_len {
            return Err(PkiError::InvalidPem("class-id trunc".into()));
        }
        let cid = core::str::from_utf8(&bytes[pos..pos + cid_len])
            .map_err(|_| PkiError::InvalidPem("class-id utf8".into()))?;
        if cid != IDENTITY_TOKEN_CLASS_ID {
            return Err(PkiError::InvalidPem(format!(
                "IdentityToken class-id mismatch: `{cid}`"
            )));
        }
        pos += cid_len;

        let mut cert_sn: Option<String> = None;
        let mut cert_algo: Option<String> = None;
        let mut ca_sn: Option<String> = None;
        let mut ca_algo: Option<String> = None;

        while pos < bytes.len() {
            let key_len = bytes[pos] as usize;
            pos += 1;
            if bytes.len() < pos + key_len {
                return Err(PkiError::InvalidPem("key trunc".into()));
            }
            let key = core::str::from_utf8(&bytes[pos..pos + key_len])
                .map_err(|_| PkiError::InvalidPem("key utf8".into()))?
                .to_string();
            pos += key_len;
            if bytes.len() < pos + 2 {
                return Err(PkiError::InvalidPem("val-len trunc".into()));
            }
            let val_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
            pos += 2;
            if bytes.len() < pos + val_len {
                return Err(PkiError::InvalidPem("val trunc".into()));
            }
            let val = core::str::from_utf8(&bytes[pos..pos + val_len])
                .map_err(|_| PkiError::InvalidPem("val utf8".into()))?
                .to_string();
            pos += val_len;
            match key.as_str() {
                KEY_CERT_SN => cert_sn = Some(val),
                KEY_CERT_ALGO => cert_algo = Some(val),
                KEY_CA_SN => ca_sn = Some(val),
                KEY_CA_ALGO => ca_algo = Some(val),
                _ => {
                    // Unbekannte Property — Spec sagt tolerable, skippen.
                }
            }
        }
        Ok(Self {
            cert_sn: cert_sn.ok_or_else(|| PkiError::InvalidPem("missing dds.cert.sn".into()))?,
            cert_algo: cert_algo
                .ok_or_else(|| PkiError::InvalidPem("missing dds.cert.algo".into()))?,
            ca_sn: ca_sn.ok_or_else(|| PkiError::InvalidPem("missing dds.ca.sn".into()))?,
            ca_algo: ca_algo.ok_or_else(|| PkiError::InvalidPem("missing dds.ca.algo".into()))?,
        })
    }
}

/// Spec §7.4.3 Cert-Bind: `subject_match` prueft ob ein lokales
/// IdentityToken mit einem Permissions-Document-Subject uebereinstimmt.
///
/// Beide Seiten muessen eine identische, kanonische RFC-4514-Form
/// liefern. Wir vergleichen mit Trim+lowercase weil die Subject-
/// Renderer manchmal in Capitalisierung von OIDs-Names abweichen
/// (z.B. `CN` vs `cn`).
#[must_use]
pub fn subject_match(token_subject: &str, permissions_subject: &str) -> bool {
    canonicalize_subject(token_subject) == canonicalize_subject(permissions_subject)
}

fn canonicalize_subject(s: &str) -> String {
    // Whitespace um `=` und `,` herum entfernen, alles lowercase.
    let mut out = String::with_capacity(s.len());
    let mut prev_was_sep = true;
    for c in s.chars() {
        if c.is_whitespace() && prev_was_sep {
            continue;
        }
        if c == ',' || c == '=' {
            // Wenn das letzte Output-Zeichen Whitespace ist, entferne es.
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

/// Extrahiert Subject + Issuer aus einem PEM-Identity-Cert + PEM-CA-Cert
/// und baut daraus einen vollstaendigen `IdentityToken`.
///
/// `cert_pem`/`ca_pem` muessen je das **erste** Cert in einem
/// PEM-Block enthalten.
///
/// # Errors
/// `PkiError::InvalidPem` bei Parse-Fehlern.
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
    Ok(IdentityToken {
        cert_sn: cert.tbs_certificate.subject.to_string(),
        cert_algo: cert.signature_algorithm.oid.to_string(),
        ca_sn: ca.tbs_certificate.subject.to_string(),
        ca_algo: ca.signature_algorithm.oid.to_string(),
    })
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
        // Encode ohne `dds.ca.algo`.
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
        // Anhaengen einer unbekannten Property — sollte still skippen.
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
