// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Daemon-facing Convenience: `SecurityConfig` (CLI/YAML-Surface) →
//! [`SecurityCtx`] (resolved). Wird von allen sechs Bridge-Daemons
//! identisch verwendet — der Unterschied ist nur der Connection-Pfad,
//! in den der Ctx gehängt wird.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::acl::{Acl, AclEntry, AclOp};
use crate::auth::{AuthError, AuthInput, AuthMode, AuthSubject};
use crate::tls::{TlsConfigError, load_server_config, load_server_config_with_client_auth};

/// Aufgelöste Security-Config — Output dieser Schicht.
#[derive(Clone)]
pub struct SecurityCtx {
    /// `Some(...)` ⇒ TLS aktiv; rustls-ServerConfig (mit oder ohne mTLS).
    pub tls: Option<Arc<rustls::ServerConfig>>,
    /// Auth-Mode (none/bearer/jwt/mtls/sasl).
    pub auth: Arc<AuthMode>,
    /// Topic-ACL.
    pub acl: Arc<Acl>,
}

impl core::fmt::Debug for SecurityCtx {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SecurityCtx")
            .field("tls_enabled", &self.tls.is_some())
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

/// Roh-Config aus YAML/CLI.
#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    /// PEM-Cert-Pfad (`--tls-cert`).
    pub tls_cert: Option<PathBuf>,
    /// PEM-Key-Pfad (`--tls-key`).
    pub tls_key: Option<PathBuf>,
    /// PEM-CA-Bundle für mTLS Client-Cert-Validation (`--client-ca`).
    pub client_ca: Option<PathBuf>,
    /// Auth-Mode-String (`none|bearer|jwt|mtls|sasl`).
    pub auth_mode: String,
    /// Bearer-Tokens als Map `token → subject-name`.
    pub bearer_tokens: HashMap<String, String>,
    /// JWT-RSA-Public-Key (PKCS#1-DER).
    pub jwt_pubkey_der: Option<Vec<u8>>,
    /// JWT erwarteter `iss`-Claim.
    pub jwt_expected_iss: Option<String>,
    /// SASL-PLAIN: `user → password`-Map (für AMQP/MQTT).
    pub sasl_users: HashMap<String, String>,
    /// ACL pro Topic-Name.
    pub topic_acl: HashMap<String, AclEntry>,
    /// ACL Default-Entry für unbekannte Topics. `None` = deny-by-default.
    pub topic_acl_default: Option<AclEntry>,
}

/// Setup-Fehler.
#[derive(Debug)]
pub enum SecurityError {
    /// Cert-/Key-Loader meldet Fehler.
    Tls(TlsConfigError),
    /// Auth-Mode-String unbekannt.
    UnknownAuthMode(String),
    /// Auth-Mode benötigt Input der fehlt.
    MissingAuthInput(&'static str),
}

impl core::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Tls(e) => write!(f, "tls: {e}"),
            Self::UnknownAuthMode(m) => write!(f, "unknown auth-mode: {m}"),
            Self::MissingAuthInput(s) => write!(f, "missing auth input: {s}"),
        }
    }
}

impl std::error::Error for SecurityError {}

impl From<TlsConfigError> for SecurityError {
    fn from(e: TlsConfigError) -> Self {
        Self::Tls(e)
    }
}

/// Baut [`SecurityCtx`] aus [`SecurityConfig`]. Wird beim Daemon-Start
/// und beim SIGHUP-Reload aufgerufen.
///
/// # Errors
/// [`SecurityError`] bei TLS-Config-Fehler oder unbekanntem Auth-Mode.
pub fn build_ctx(cfg: &SecurityConfig) -> Result<SecurityCtx, SecurityError> {
    let tls = match (&cfg.tls_cert, &cfg.tls_key) {
        (Some(c), Some(k)) => match &cfg.client_ca {
            Some(ca) => Some(load_server_config_with_client_auth(c, k, ca)?),
            None => Some(load_server_config(c, k)?),
        },
        (None, None) => None,
        _ => {
            return Err(SecurityError::Tls(TlsConfigError::Rustls(
                "tls_cert and tls_key must be set together".to_string(),
            )));
        }
    };

    let auth = match cfg.auth_mode.as_str() {
        "" | "none" => AuthMode::None,
        "bearer" => {
            let mut tokens: HashMap<String, AuthSubject> = HashMap::new();
            for (tok, name) in &cfg.bearer_tokens {
                tokens.insert(tok.clone(), AuthSubject::new(name.clone()));
            }
            AuthMode::Bearer { tokens }
        }
        "jwt" => {
            let der = cfg
                .jwt_pubkey_der
                .clone()
                .ok_or(SecurityError::MissingAuthInput("jwt_pubkey_der"))?;
            AuthMode::Jwt {
                pkcs1_pubkey_der: der,
                expected_issuer: cfg.jwt_expected_iss.clone(),
            }
        }
        "mtls" => AuthMode::Mtls,
        "sasl" => {
            if cfg.sasl_users.is_empty() {
                return Err(SecurityError::MissingAuthInput("sasl_users"));
            }
            AuthMode::SaslPlain {
                users: cfg.sasl_users.clone(),
            }
        }
        other => return Err(SecurityError::UnknownAuthMode(other.to_string())),
    };

    let mut acl = if cfg.topic_acl.is_empty() && cfg.topic_acl_default.is_none() {
        if matches!(auth, AuthMode::None) {
            Acl::allow_all()
        } else {
            Acl::deny_all()
        }
    } else {
        Acl::deny_all()
    };
    for (topic, entry) in &cfg.topic_acl {
        acl.set(topic.clone(), entry.clone());
    }
    if let Some(d) = &cfg.topic_acl_default {
        acl.set_default(d.clone());
    }

    Ok(SecurityCtx {
        tls,
        auth: Arc::new(auth),
        acl: Arc::new(acl),
    })
}

/// Authentication-Wrapper. Pro Daemon werden die jeweils relevanten
/// Inputs gefüllt:
/// * HTTP/WS/gRPC: `authorization_header`
/// * mTLS (alle TCP-Bridges): `mtls_subject` aus `rustls::ServerConnection::peer_certificates()` extrahiert
/// * MQTT/AMQP-SASL-PLAIN: `sasl_plain_blob`
///
/// # Errors
/// [`AuthError`] bei jeder Form von Reject/Missing/Malformed.
pub fn authenticate(
    auth: &AuthMode,
    authorization_header: Option<&str>,
    sasl_plain_blob: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let input = AuthInput {
        authorization_header,
        sasl_plain_blob,
        mtls_subject,
    };
    auth.validate(&input)
}

/// Convenience: Topic-ACL-Check.
#[must_use]
pub fn authorize(acl: &Acl, subject: &AuthSubject, op: AclOp, topic: &str) -> bool {
    acl.check(subject, op, topic)
}

/// Extrahiert ein `AuthSubject` aus einem `rustls::ServerConnection`
/// peer-cert (für `AuthMode::Mtls`). Liefert `None` wenn kein Cert
/// präsentiert wurde.
///
/// Subject-Name = X.509-Subject-DN als String (DER-Bytes hex-encoded
/// als Fallback, falls X.500 nicht parst).
#[must_use]
pub fn extract_mtls_subject(conn: &rustls::ServerConnection) -> Option<AuthSubject> {
    let certs = conn.peer_certificates()?;
    let leaf = certs.first()?;
    // Wir nehmen den Cert-DER-SHA256-Fingerprint als stable Identity.
    // Spec §7.2: mTLS-Subject = SubjectDN ODER hash. Wir wählen hash für
    // Determinismus ohne X.500-DN-Parser-Dep.
    let hash = sha256_hex(leaf.as_ref());
    Some(AuthSubject::new(format!("mtls:{hash}")))
}

fn sha256_hex(data: &[u8]) -> String {
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(data);
    let d = ctx.finish();
    let mut s = String::with_capacity(64);
    for b in d.as_ref() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn build_default_yields_none_auth_allow_all_acl() {
        let cfg = SecurityConfig::default();
        let ctx = build_ctx(&cfg).unwrap();
        assert!(ctx.tls.is_none());
        assert!(matches!(*ctx.auth, AuthMode::None));
        assert!(ctx.acl.check(&AuthSubject::anonymous(), AclOp::Read, "X"));
    }

    #[test]
    fn build_bearer_with_tokens() {
        let mut cfg = SecurityConfig {
            auth_mode: "bearer".into(),
            ..Default::default()
        };
        cfg.bearer_tokens.insert("t".into(), "alice".into());
        let ctx = build_ctx(&cfg).unwrap();
        let s = authenticate(&ctx.auth, Some("Bearer t"), None, None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn build_sasl_with_users() {
        let mut cfg = SecurityConfig {
            auth_mode: "sasl".into(),
            ..Default::default()
        };
        cfg.sasl_users.insert("u".into(), "p".into());
        let ctx = build_ctx(&cfg).unwrap();
        let s = authenticate(&ctx.auth, None, Some(b"\0u\0p"), None).unwrap();
        assert_eq!(s.name, "u");
    }

    #[test]
    fn build_sasl_without_users_rejected() {
        let cfg = SecurityConfig {
            auth_mode: "sasl".into(),
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::MissingAuthInput(_)));
    }

    #[test]
    fn unknown_auth_mode_rejected() {
        let cfg = SecurityConfig {
            auth_mode: "weird".into(),
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::UnknownAuthMode(_)));
    }

    #[test]
    fn jwt_without_key_rejected() {
        let cfg = SecurityConfig {
            auth_mode: "jwt".into(),
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::MissingAuthInput(_)));
    }

    #[test]
    fn explicit_acl_overrides_open_default() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "T".into(),
            AclEntry {
                read: vec!["alice".into()],
                write: vec!["alice".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let alice = AuthSubject::new("alice");
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "T"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Read, "T"));
        assert!(!authorize(&ctx.acl, &alice, AclOp::Read, "Other"));
    }

    #[test]
    fn explicit_acl_default_used_for_unknown() {
        let cfg = SecurityConfig {
            topic_acl_default: Some(AclEntry {
                read: vec!["*".into()],
                write: vec![],
            }),
            ..SecurityConfig::default()
        };
        let ctx = build_ctx(&cfg).unwrap();
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &bob, AclOp::Read, "Anything"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Write, "Anything"));
    }

    #[test]
    fn partial_tls_paths_rejected() {
        let cfg = SecurityConfig {
            tls_cert: Some("/x".into()),
            tls_key: None,
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::Tls(_)));
    }
}
