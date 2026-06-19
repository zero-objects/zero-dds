// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WS-Security 1.1 — OASIS WSS-1.1.
//!
//! Spec: `http://docs.oasis-open.org/wss-m/wss/v1.1/`.
//!
//! Wir liefern die zentralen Token-Typen:
//!
//! * UsernameToken (UsernameToken Profile 1.1, §3.1).
//! * X.509-Token (Token Profile 1.1, §3.2).
//! * Timestamp (WS-Security Core 1.1 §10.2).
//!
//! Cipher material + signature computation is the caller layer.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// WS-Security 1.1 SecurityHeader-Namespace.
pub const WSSE_NS: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
/// WS-Security 1.1 Utility-Namespace (`wsu:`).
pub const WSU_NS: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";

/// UsernameToken — WSS UsernameToken Profile 1.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UsernameToken {
    /// Username.
    pub username: String,
    /// Password (cleartext or digest — caller decides).
    pub password: String,
    /// `Type` attribute on the `<wsse:Password>` element.
    pub password_type: PasswordType,
    /// Optional Nonce (Base64-codiert).
    pub nonce: Option<String>,
    /// Optional Created-Timestamp (XSD-DateTime).
    pub created: Option<String>,
}

/// Password-Type. WSS UsernameToken §3.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordType {
    /// `#PasswordText` — Cleartext.
    #[default]
    Text,
    /// `#PasswordDigest` — the caller provides SHA1(nonce + created +
    /// password) base64-encoded.
    Digest,
}

impl PasswordType {
    /// Spec-URI.
    #[must_use]
    pub fn type_uri(self) -> &'static str {
        match self {
            Self::Text => {
                "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordText"
            }
            Self::Digest => {
                "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest"
            }
        }
    }
}

/// X.509-Token — WSS X.509 Token Profile 1.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X509Token {
    /// Cert (Base64- or hex-encoded).
    pub cert_b64: String,
    /// `EncodingType`-Attribut auf `<wsse:BinarySecurityToken>`.
    pub encoding_type: String,
    /// `ValueType`-Attribut.
    pub value_type: String,
}

impl Default for X509Token {
    fn default() -> Self {
        Self {
            cert_b64: String::new(),
            encoding_type: "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary".into(),
            value_type: "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-x509-token-profile-1.0#X509v3".into(),
        }
    }
}

/// `<wsu:Timestamp>` — WS-Security Core §10.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timestamp {
    /// `<wsu:Created>` — XSD-DateTime.
    pub created: String,
    /// Optional `<wsu:Expires>`.
    pub expires: Option<String>,
}

/// SecurityHeader — kombiniert Tokens + Timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecurityHeader {
    /// Username-Tokens.
    pub usernames: Vec<UsernameToken>,
    /// X.509-Tokens.
    pub x509: Vec<X509Token>,
    /// Optional Timestamp.
    pub timestamp: Option<Timestamp>,
    /// `mustUnderstand`-Flag (default false).
    pub must_understand: bool,
}

impl SecurityHeader {
    /// Render to XML — as the content of a `<soap:Header>`. Spec
    /// `<wsse:Security>` Skeleton.
    #[must_use]
    pub fn to_xml(&self) -> String {
        let mu = if self.must_understand {
            " soap:mustUnderstand=\"1\""
        } else {
            ""
        };
        let mut out =
            format!("<wsse:Security xmlns:wsse=\"{WSSE_NS}\" xmlns:wsu=\"{WSU_NS}\"{mu}>");
        if let Some(ts) = &self.timestamp {
            out.push_str("<wsu:Timestamp>");
            out.push_str(&format!("<wsu:Created>{}</wsu:Created>", ts.created));
            if let Some(exp) = &ts.expires {
                out.push_str(&format!("<wsu:Expires>{exp}</wsu:Expires>"));
            }
            out.push_str("</wsu:Timestamp>");
        }
        for u in &self.usernames {
            out.push_str("<wsse:UsernameToken>");
            out.push_str(&format!("<wsse:Username>{}</wsse:Username>", u.username));
            out.push_str(&format!(
                "<wsse:Password Type=\"{}\">{}</wsse:Password>",
                u.password_type.type_uri(),
                xml_escape(&u.password)
            ));
            if let Some(n) = &u.nonce {
                out.push_str(&format!("<wsse:Nonce>{n}</wsse:Nonce>"));
            }
            if let Some(c) = &u.created {
                out.push_str(&format!("<wsu:Created>{c}</wsu:Created>"));
            }
            out.push_str("</wsse:UsernameToken>");
        }
        for x in &self.x509 {
            out.push_str(&format!(
                "<wsse:BinarySecurityToken EncodingType=\"{}\" ValueType=\"{}\">{}</wsse:BinarySecurityToken>",
                x.encoding_type, x.value_type, x.cert_b64
            ));
        }
        out.push_str("</wsse:Security>");
        out
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn empty_header_still_emits_security_element() {
        let h = SecurityHeader::default();
        let xml = h.to_xml();
        assert!(xml.starts_with("<wsse:Security"));
        assert!(xml.ends_with("</wsse:Security>"));
    }

    #[test]
    fn username_token_text_password_emits_correct_type() {
        let mut h = SecurityHeader::default();
        h.usernames.push(UsernameToken {
            username: "alice".into(),
            password: "secret".into(),
            password_type: PasswordType::Text,
            ..UsernameToken::default()
        });
        let xml = h.to_xml();
        assert!(xml.contains("<wsse:Username>alice</wsse:Username>"));
        assert!(xml.contains("PasswordText"));
        assert!(xml.contains(">secret<"));
    }

    #[test]
    fn username_token_digest_uses_digest_uri() {
        let mut h = SecurityHeader::default();
        h.usernames.push(UsernameToken {
            username: "alice".into(),
            password: "abc=".into(),
            password_type: PasswordType::Digest,
            nonce: Some("nonce==".into()),
            created: Some("2026-04-01T00:00:00Z".into()),
        });
        let xml = h.to_xml();
        assert!(xml.contains("PasswordDigest"));
        assert!(xml.contains("<wsse:Nonce>nonce=="));
        assert!(xml.contains("<wsu:Created>2026-04-01T00:00:00Z</wsu:Created>"));
    }

    #[test]
    fn timestamp_emits_created_and_expires() {
        let h = SecurityHeader {
            timestamp: Some(Timestamp {
                created: "2026-04-01T00:00:00Z".into(),
                expires: Some("2026-04-01T00:05:00Z".into()),
            }),
            ..SecurityHeader::default()
        };
        let xml = h.to_xml();
        assert!(xml.contains("<wsu:Created>2026-04-01T00:00:00Z</wsu:Created>"));
        assert!(xml.contains("<wsu:Expires>2026-04-01T00:05:00Z</wsu:Expires>"));
    }

    #[test]
    fn x509_token_default_uses_v3_value_type() {
        let mut h = SecurityHeader::default();
        h.x509.push(X509Token {
            cert_b64: "MIIB...".into(),
            ..X509Token::default()
        });
        let xml = h.to_xml();
        assert!(xml.contains("BinarySecurityToken"));
        assert!(xml.contains("X509v3"));
        assert!(xml.contains("MIIB..."));
    }

    #[test]
    fn must_understand_flag_emitted() {
        let h = SecurityHeader {
            must_understand: true,
            ..SecurityHeader::default()
        };
        let xml = h.to_xml();
        assert!(xml.contains("soap:mustUnderstand=\"1\""));
    }

    #[test]
    fn password_xml_escaped() {
        let mut h = SecurityHeader::default();
        h.usernames.push(UsernameToken {
            username: "u".into(),
            password: "<bad>&".into(),
            password_type: PasswordType::Text,
            ..UsernameToken::default()
        });
        let xml = h.to_xml();
        assert!(xml.contains("&lt;bad&gt;&amp;"));
        assert!(!xml.contains("<bad>"));
    }

    #[test]
    fn password_type_uris_match_spec() {
        assert!(PasswordType::Text.type_uri().contains("PasswordText"));
        assert!(PasswordType::Digest.type_uri().contains("PasswordDigest"));
    }
}
