// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §7.2 Auth-Modes — `none|bearer|jwt|mtls|sasl`.
//!
//! Eingangspunkt: [`AuthMode::validate`] → [`AuthSubject`] oder
//! [`AuthError`]. Pro Bridge wird der Aufruf an die jeweilige
//! Connection-Phase gehängt:
//!
//! * HTTP-basiert (ws/grpc): nach dem Request-Header-Lesen.
//! * MQTT: in der CONNECT-Packet-Auswertung.
//! * AMQP: nach SASL-PLAIN-Negotiation.
//! * CORBA: aus dem CSIv2-Token-Parsing oder dem TLS-Layer (mtls).

use std::collections::HashMap;

/// Auth-Subject — wird nach erfolgreichem [`AuthMode::validate`] an
/// nachfolgende ACL-Prüfungen (`§7.3`) gereicht.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthSubject {
    /// Stable Identity (e.g. JWT `sub`, Bearer-Mapping, mTLS-CN, SASL-User).
    pub name: String,
    /// Group-Memberships (z.B. JWT `groups`-Claim).
    pub groups: Vec<String>,
    /// Free-form Claims (jeder String-Wert).
    pub claims: HashMap<String, String>,
}

impl AuthSubject {
    /// Anonymous-Subject für `AuthMode::None`.
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            name: "anonymous".to_string(),
            groups: Vec::new(),
            claims: HashMap::new(),
        }
    }

    /// Plain-Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            groups: Vec::new(),
            claims: HashMap::new(),
        }
    }

    /// Fluent-Add Group.
    #[must_use]
    pub fn with_group(mut self, g: impl Into<String>) -> Self {
        self.groups.push(g.into());
        self
    }

    /// Fluent-Add Claim.
    #[must_use]
    pub fn with_claim(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.claims.insert(k.into(), v.into());
        self
    }
}

/// Fehler bei der Auth-Validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Header / Frame fehlt komplett (z.B. kein `Authorization`).
    MissingCredentials,
    /// Header-Format invalid (z.B. `Authorization` ohne `Bearer `).
    MalformedCredentials(String),
    /// Token nicht akzeptiert (Bearer mismatch / JWT-Signature fail / ...).
    Rejected(String),
    /// Konfiguration broken (z.B. JWT-Public-Key kann nicht geladen werden).
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

/// Auth-Mode pro Daemon (CLI `--auth-mode <MODE>`).
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// Keine Auth. Subject = "anonymous".
    None,
    /// Bearer-Token-Vergleich (HTTP-Header `Authorization: Bearer …`).
    /// Multi-Token via Map `token → subject`.
    Bearer {
        /// Mapping vom Token-String auf das resultierende Subject.
        tokens: HashMap<String, AuthSubject>,
    },
    /// JWT — RS256-Signature-Validation gegen RSA-Public-Key (DER-Bytes
    /// wie aus `RsaPublicKey::to_pkcs1_der`).
    Jwt {
        /// PKCS#1-DER des RSA-Public-Keys.
        pkcs1_pubkey_der: Vec<u8>,
        /// Optional Issuer-Match (`iss`-Claim).
        expected_issuer: Option<String>,
    },
    /// mTLS — Identity wird vom TLS-Layer (Client-Cert) geliefert,
    /// nicht von dieser Funktion. [`AuthMode::validate`] mit `Mtls`
    /// erwartet `presented_subject = Some(...)`.
    Mtls,
    /// SASL-PLAIN — `username\0username\0password`-Frame, mit Map
    /// `username → password`.
    SaslPlain {
        /// Mapping `user → password`.
        users: HashMap<String, String>,
    },
}

/// Was der Caller (Daemon) für `validate` mitbringt.
#[derive(Debug, Clone, Default)]
pub struct AuthInput<'a> {
    /// Bearer/JWT: Inhalt des `Authorization`-Headers (komplett).
    pub authorization_header: Option<&'a str>,
    /// SASL-PLAIN: Raw-Frame-Bytes (`user\0user\0pass`).
    pub sasl_plain_blob: Option<&'a [u8]>,
    /// mTLS: vom rustls-Connection bereits validiertes Subject (z.B.
    /// Cert-CN). Wenn mTLS aber kein Cert präsentiert wurde: `None`.
    pub mtls_subject: Option<AuthSubject>,
}

impl AuthMode {
    /// Validate. Liefert das Subject oder einen [`AuthError`].
    ///
    /// # Errors
    /// [`AuthError`] mit Sub-Variante.
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

    // Header muss alg=RS256 sagen.
    let header_str = core::str::from_utf8(&header_bytes)
        .map_err(|_| AuthError::MalformedCredentials("jwt header utf8".into()))?;
    if !json_field_eq(header_str, "alg", "RS256") {
        return Err(AuthError::Rejected("jwt: alg must be RS256".into()));
    }

    // Signature über `<h_b64>.<p_b64>` mit RSA-PKCS#1-v1.5-SHA256 prüfen.
    let signed = {
        let mut v = Vec::with_capacity(h_b64.len() + 1 + p_b64.len());
        v.extend_from_slice(h_b64.as_bytes());
        v.push(b'.');
        v.extend_from_slice(p_b64.as_bytes());
        v
    };
    let pubkey = ring::signature::UnparsedPublicKey::new(
        &ring::signature::RSA_PKCS1_2048_8192_SHA256,
        pkcs1_pubkey_der,
    );
    pubkey
        .verify(&signed, &sig_bytes)
        .map_err(|_| AuthError::Rejected("jwt: signature invalid".into()))?;

    // Payload-Claims extrahieren.
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

/// Mini-JSON-Field-Extractor für die JWT-Header- und Payload-Maps.
/// Akzeptiert flach-strukturiertes JSON wie es JWT-Libraries emittieren.
/// Keine vollständige JSON-Parser-Implementation — Spec §7.2 sagt
/// JWT-Lib darf kompakter Subset sein.
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
        // Numeric / bool — wir nehmen bis zum nächsten , oder }.
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
        // Bogus pubkey + bogus token. Wir testen nur den Reject-Pfad —
        // ein voll-signierter Round-Trip-Test braucht eine RSA-Key-Gen
        // Lib (rcgen kann das nicht für JWT). Der Code-Pfad selbst ist
        // gegen `ring::signature::UnparsedPublicKey::verify` gefahren.
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
