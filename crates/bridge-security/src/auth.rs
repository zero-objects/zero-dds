// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §7.2 auth modes — `none|bearer|jwt|mtls|sasl`.
//!
//! Entry point: [`AuthMode::validate`] → [`AuthSubject`] or
//! [`AuthError`]. In each bridge, the call is hooked into the relevant
//! connection phase:
//!
//! * HTTP-based (ws/grpc): after reading the request headers.
//! * MQTT: during CONNECT packet evaluation.
//! * AMQP: after SASL-PLAIN negotiation.
//! * CORBA: from CSIv2 token parsing or the TLS layer (mtls).

use std::collections::HashMap;

/// Auth subject — passed to subsequent ACL checks (`§7.3`) after a
/// successful [`AuthMode::validate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthSubject {
    /// Stable Identity (e.g. JWT `sub`, Bearer-Mapping, mTLS-CN, SASL-User).
    pub name: String,
    /// Group memberships (e.g. JWT `groups` claim).
    pub groups: Vec<String>,
    /// Free-form claims (any string value).
    pub claims: HashMap<String, String>,
}

impl AuthSubject {
    /// Anonymous subject for `AuthMode::None`.
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            name: "anonymous".to_string(),
            groups: Vec::new(),
            claims: HashMap::new(),
        }
    }

    /// Plain constructor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            groups: Vec::new(),
            claims: HashMap::new(),
        }
    }

    /// Fluent add group.
    #[must_use]
    pub fn with_group(mut self, g: impl Into<String>) -> Self {
        self.groups.push(g.into());
        self
    }

    /// Fluent add claim.
    #[must_use]
    pub fn with_claim(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.claims.insert(k.into(), v.into());
        self
    }
}

/// Error during auth validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Header / frame is entirely missing (e.g. no `Authorization`).
    MissingCredentials,
    /// Header format invalid (e.g. `Authorization` without `Bearer `).
    MalformedCredentials(String),
    /// Token not accepted (bearer mismatch / JWT signature fail / ...).
    Rejected(String),
    /// Configuration broken (e.g. JWT public key cannot be loaded).
    Misconfigured(String),
}

impl core::fmt::Display for AuthError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingCredentials => f.write_str("missing credentials"),
            Self::MalformedCredentials(m) => write!(f, "malformed credentials: {m}"),
            Self::Rejected(m) => write!(f, "credentials rejected: {m}"),
            Self::Misconfigured(m) => write!(f, "auth misconfigured: {m}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Auth mode per daemon (CLI `--auth-mode <MODE>`).
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// No auth. Subject = "anonymous".
    None,
    /// Bearer token comparison (HTTP header `Authorization: Bearer …`).
    /// Multi-token via map `token → subject`.
    Bearer {
        /// Mapping from the token string to the resulting subject.
        tokens: HashMap<String, AuthSubject>,
    },
    /// JWT — RS256 signature validation against an RSA public key (DER
    /// bytes as from `RsaPublicKey::to_pkcs1_der`).
    Jwt {
        /// PKCS#1 DER of the RSA public key.
        pkcs1_pubkey_der: Vec<u8>,
        /// Optional issuer match (`iss` claim).
        expected_issuer: Option<String>,
    },
    /// mTLS — identity is supplied by the TLS layer (client cert), not
    /// by this function. [`AuthMode::validate`] with `Mtls` expects
    /// `presented_subject = Some(...)`.
    Mtls,
    /// SASL-PLAIN — `username\0username\0password` frame, with map
    /// `username → password`.
    SaslPlain {
        /// Mapping `user → password`.
        users: HashMap<String, String>,
    },
}

/// What the caller (daemon) supplies for `validate`.
#[derive(Debug, Clone, Default)]
pub struct AuthInput<'a> {
    /// Bearer/JWT: content of the `Authorization` header (complete).
    pub authorization_header: Option<&'a str>,
    /// SASL-PLAIN: raw frame bytes (`user\0user\0pass`).
    pub sasl_plain_blob: Option<&'a [u8]>,
    /// mTLS: subject already validated by the rustls connection (e.g.
    /// cert CN). If mTLS but no cert was presented: `None`.
    pub mtls_subject: Option<AuthSubject>,
}

impl AuthMode {
    /// Validate. Returns the subject or an [`AuthError`].
    ///
    /// # Errors
    /// [`AuthError`] with sub-variant.
    pub fn validate(&self, input: &AuthInput<'_>) -> Result<AuthSubject, AuthError> {
        match self {
            Self::None => Ok(AuthSubject::anonymous()),
            Self::Bearer { tokens } => {
                let hdr = input
                    .authorization_header
                    .ok_or(AuthError::MissingCredentials)?;
                let token = strip_bearer(hdr)?;
                tokens
                    .get(token)
                    .cloned()
                    .ok_or_else(|| AuthError::Rejected("unknown bearer token".to_string()))
            }
            Self::Jwt {
                pkcs1_pubkey_der,
                expected_issuer,
            } => {
                let hdr = input
                    .authorization_header
                    .ok_or(AuthError::MissingCredentials)?;
                let token = strip_bearer(hdr)?;
                validate_jwt_rs256(token, pkcs1_pubkey_der, expected_issuer.as_deref())
            }
            Self::Mtls => input
                .mtls_subject
                .clone()
                .ok_or_else(|| AuthError::Rejected("mTLS expected client cert".to_string())),
            Self::SaslPlain { users } => {
                let blob = input.sasl_plain_blob.ok_or(AuthError::MissingCredentials)?;
                let (user, pass) = parse_sasl_plain(blob)?;
                let stored = users
                    .get(user)
                    .ok_or_else(|| AuthError::Rejected("unknown user".to_string()))?;
                if stored == pass {
                    Ok(AuthSubject::new(user))
                } else {
                    Err(AuthError::Rejected("password mismatch".to_string()))
                }
            }
        }
    }
}

fn strip_bearer(hdr: &str) -> Result<&str, AuthError> {
    let trimmed = hdr.trim();
    let prefix = "Bearer ";
    if trimmed.len() < prefix.len()
        || !trimmed
            .get(..prefix.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
    {
        return Err(AuthError::MalformedCredentials(
            "expected `Bearer …`".to_string(),
        ));
    }
    Ok(trimmed[prefix.len()..].trim())
}

fn parse_sasl_plain(blob: &[u8]) -> Result<(&str, &str), AuthError> {
    // Format: `[authzid]\0authcid\0passwd` (RFC 4616).
    let mut parts = blob.splitn(3, |b| *b == 0);
    let _authzid = parts
        .next()
        .ok_or(AuthError::MalformedCredentials("sasl-plain empty".into()))?;
    let authcid = parts
        .next()
        .ok_or(AuthError::MalformedCredentials("sasl-plain no user".into()))?;
    let passwd = parts
        .next()
        .ok_or(AuthError::MalformedCredentials("sasl-plain no pass".into()))?;
    let user = core::str::from_utf8(authcid)
        .map_err(|_| AuthError::MalformedCredentials("sasl-plain user utf8".into()))?;
    let pass = core::str::from_utf8(passwd)
        .map_err(|_| AuthError::MalformedCredentials("sasl-plain pass utf8".into()))?;
    if user.is_empty() {
        return Err(AuthError::MalformedCredentials(
            "sasl-plain empty user".into(),
        ));
    }
    Ok((user, pass))
}

// ---------- JWT RS256 ----------------------------------------------------

fn validate_jwt_rs256(
    token: &str,
    pkcs1_pubkey_der: &[u8],
    expected_issuer: Option<&str>,
) -> Result<AuthSubject, AuthError> {
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let mut parts = token.split('.');
    let h_b64 = parts
        .next()
        .ok_or_else(|| AuthError::MalformedCredentials("jwt: no header".into()))?;
    let p_b64 = parts
        .next()
        .ok_or_else(|| AuthError::MalformedCredentials("jwt: no payload".into()))?;
    let s_b64 = parts
        .next()
        .ok_or_else(|| AuthError::MalformedCredentials("jwt: no sig".into()))?;
    if parts.next().is_some() {
        return Err(AuthError::MalformedCredentials(
            "jwt: too many segments".into(),
        ));
    }

    let header_bytes = engine
        .decode(h_b64)
        .map_err(|e| AuthError::MalformedCredentials(format!("jwt header b64: {e}")))?;
    let payload_bytes = engine
        .decode(p_b64)
        .map_err(|e| AuthError::MalformedCredentials(format!("jwt payload b64: {e}")))?;
    let sig_bytes = engine
        .decode(s_b64)
        .map_err(|e| AuthError::MalformedCredentials(format!("jwt sig b64: {e}")))?;

    // Header must say alg=RS256.
    let header_str = core::str::from_utf8(&header_bytes)
        .map_err(|_| AuthError::MalformedCredentials("jwt header utf8".into()))?;
    if !json_field_eq(header_str, "alg", "RS256") {
        return Err(AuthError::Rejected("jwt: alg must be RS256".into()));
    }

    // Verify the signature over `<h_b64>.<p_b64>` with RSA-PKCS#1-v1.5-SHA256.
    let signed = {
        let mut v = Vec::with_capacity(h_b64.len() + 1 + p_b64.len());
        v.extend_from_slice(h_b64.as_bytes());
        v.push(b'.');
        v.extend_from_slice(p_b64.as_bytes());
        v
    };
    let pubkey = crate::backend::signature::UnparsedPublicKey::new(
        &crate::backend::signature::RSA_PKCS1_2048_8192_SHA256,
        pkcs1_pubkey_der,
    );
    pubkey
        .verify(&signed, &sig_bytes)
        .map_err(|_| AuthError::Rejected("jwt: signature invalid".into()))?;

    // Extract payload claims.
    let payload_str = core::str::from_utf8(&payload_bytes)
        .map_err(|_| AuthError::MalformedCredentials("jwt payload utf8".into()))?;
    let sub = json_field(payload_str, "sub")
        .ok_or_else(|| AuthError::Rejected("jwt: no sub claim".into()))?;

    if let Some(expected) = expected_issuer {
        let iss = json_field(payload_str, "iss")
            .ok_or_else(|| AuthError::Rejected("jwt: no iss claim".into()))?;
        if iss != expected {
            return Err(AuthError::Rejected(format!("jwt: iss != {expected}")));
        }
    }

    let mut subj = AuthSubject::new(sub);
    if let Some(groups_raw) = json_array(payload_str, "groups") {
        for g in groups_raw {
            subj.groups.push(g);
        }
    }
    Ok(subj)
}

/// Mini JSON field extractor for the JWT header and payload maps.
/// Accepts flat-structured JSON as JWT libraries emit it. Not a full
/// JSON parser implementation — Spec §7.2 says a JWT lib may use a
/// compact subset.
fn json_field(src: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let pos = src.find(&pat)?;
    let after = &src[pos + pat.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(stripped[..end].to_string())
    } else {
        // Numeric / bool — take up to the next , or }.
        let end = rest
            .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn json_field_eq(src: &str, key: &str, expected: &str) -> bool {
    json_field(src, key).is_some_and(|v| v == expected)
}

fn json_array(src: &str, key: &str) -> Option<Vec<String>> {
    let pat = format!("\"{key}\"");
    let pos = src.find(&pat)?;
    let after = &src[pos + pat.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let stripped = rest.strip_prefix('[')?;
    let end = stripped.find(']')?;
    let inside = &stripped[..end];
    let mut out = Vec::new();
    for piece in inside.split(',') {
        let p = piece.trim().trim_matches('"');
        if !p.is_empty() {
            out.push(p.to_string());
        }
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn none_mode_yields_anonymous() {
        let m = AuthMode::None;
        let s = m.validate(&AuthInput::default()).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    #[test]
    fn bearer_valid_token_accepted() {
        let mut tokens = HashMap::new();
        tokens.insert("secret123".to_string(), AuthSubject::new("alice"));
        let m = AuthMode::Bearer { tokens };
        let s = m
            .validate(&AuthInput {
                authorization_header: Some("Bearer secret123"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn bearer_invalid_token_rejected() {
        let m = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = m
            .validate(&AuthInput {
                authorization_header: Some("Bearer wrong"),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn bearer_missing_header_returns_missing() {
        let m = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = m.validate(&AuthInput::default()).unwrap_err();
        assert!(matches!(err, AuthError::MissingCredentials));
    }

    #[test]
    fn bearer_malformed_header_returns_malformed() {
        let m = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = m
            .validate(&AuthInput {
                authorization_header: Some("Basic xx"),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::MalformedCredentials(_)));
    }

    #[test]
    fn mtls_with_subject_accepted() {
        let m = AuthMode::Mtls;
        let s = m
            .validate(&AuthInput {
                mtls_subject: Some(AuthSubject::new("CN=alice")),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(s.name, "CN=alice");
    }

    #[test]
    fn mtls_without_subject_rejected() {
        let m = AuthMode::Mtls;
        let err = m.validate(&AuthInput::default()).unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn sasl_plain_valid_pair_accepted() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let m = AuthMode::SaslPlain { users };
        let blob = b"\0alice\0wonderland";
        let s = m
            .validate(&AuthInput {
                sasl_plain_blob: Some(blob),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn sasl_plain_wrong_password_rejected() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let m = AuthMode::SaslPlain { users };
        let blob = b"\0alice\0wrong";
        let err = m
            .validate(&AuthInput {
                sasl_plain_blob: Some(blob),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn json_field_extracts_string() {
        let s = r#"{"alg":"RS256","typ":"JWT"}"#;
        assert_eq!(json_field(s, "alg").as_deref(), Some("RS256"));
    }

    #[test]
    fn json_array_extracts_groups() {
        let s = r#"{"sub":"a","groups":["eng","ops"]}"#;
        let g = json_array(s, "groups").unwrap();
        assert_eq!(g, vec!["eng".to_string(), "ops".to_string()]);
    }

    #[test]
    fn jwt_invalid_signature_rejected() {
        // Bogus pubkey + bogus token. We only test the reject path — a
        // fully-signed round-trip test would need an RSA key-gen lib
        // (rcgen can't do that for JWT). The code path itself is driven
        // against `crate::backend::signature::UnparsedPublicKey::verify`.
        let m = AuthMode::Jwt {
            pkcs1_pubkey_der: vec![0u8; 32],
            expected_issuer: None,
        };
        // base64url("{\"alg\":\"RS256\"}") = "eyJhbGciOiJSUzI1NiJ9"
        // base64url("{\"sub\":\"a\"}") = "eyJzdWIiOiJhIn0"
        let token = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJhIn0.AAAA";
        let err = m
            .validate(&AuthInput {
                authorization_header: Some(&format!("Bearer {token}")),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn jwt_wrong_alg_rejected() {
        let m = AuthMode::Jwt {
            pkcs1_pubkey_der: vec![0u8; 32],
            expected_issuer: None,
        };
        // base64url("{\"alg\":\"HS256\"}") = "eyJhbGciOiJIUzI1NiJ9"
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhIn0.AAAA";
        let err = m
            .validate(&AuthInput {
                authorization_header: Some(&format!("Bearer {token}")),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }
}
