// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Daemon-facing convenience: `SecurityConfig` (CLI/YAML surface) →
//! [`SecurityCtx`] (resolved). Used identically by all six bridge
//! daemons — the only difference is the connection path into which the
//! ctx is hooked.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::acl::{Acl, AclEntry, AclOp};
use crate::auth::{AuthError, AuthInput, AuthMode, AuthSubject};
use crate::tls::{TlsConfigError, load_server_config, load_server_config_with_client_auth};

/// Resolved security config — the output of this layer.
#[derive(Clone)]
pub struct SecurityCtx {
    /// `Some(...)` ⇒ TLS active; rustls ServerConfig (with or without mTLS).
    pub tls: Option<Arc<rustls::ServerConfig>>,
    /// Auth mode (none/bearer/jwt/mtls/sasl).
    pub auth: Arc<AuthMode>,
    /// Topic ACL.
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

/// Raw config from YAML/CLI.
#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    /// PEM cert path (`--tls-cert`).
    pub tls_cert: Option<PathBuf>,
    /// PEM key path (`--tls-key`).
    pub tls_key: Option<PathBuf>,
    /// PEM CA bundle for mTLS client cert validation (`--client-ca`).
    pub client_ca: Option<PathBuf>,
    /// Auth mode string (`none|bearer|jwt|mtls|sasl`).
    pub auth_mode: String,
    /// Bearer tokens as a map `token → subject name`.
    pub bearer_tokens: HashMap<String, String>,
    /// JWT RSA public key (PKCS#1 DER).
    pub jwt_pubkey_der: Option<Vec<u8>>,
    /// JWT expected `iss` claim.
    pub jwt_expected_iss: Option<String>,
    /// SASL-PLAIN: `user → password` map (for AMQP/MQTT).
    pub sasl_users: HashMap<String, String>,
    /// ACL per topic name.
    pub topic_acl: HashMap<String, AclEntry>,
    /// ACL default entry for unknown topics. `None` = deny-by-default.
    pub topic_acl_default: Option<AclEntry>,
}

/// Setup error.
#[derive(Debug)]
pub enum SecurityError {
    /// Cert/key loader reports an error.
    Tls(TlsConfigError),
    /// Auth mode string unknown.
    UnknownAuthMode(String),
    /// Auth mode requires input that is missing.
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

/// Builds [`SecurityCtx`] from [`SecurityConfig`]. Called at daemon
/// start and on SIGHUP reload.
///
/// # Errors
/// [`SecurityError`] on TLS config error or unknown auth mode.
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

/// Authentication wrapper. Each daemon fills the inputs relevant to it:
/// * HTTP/WS/gRPC: `authorization_header`
/// * mTLS (all TCP bridges): `mtls_subject` extracted from `rustls::ServerConnection::peer_certificates()`
/// * MQTT/AMQP SASL-PLAIN: `sasl_plain_blob`
///
/// # Errors
/// [`AuthError`] on any form of reject/missing/malformed.
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

/// Convenience: topic ACL check.
#[must_use]
pub fn authorize(acl: &Acl, subject: &AuthSubject, op: AclOp, topic: &str) -> bool {
    acl.check(subject, op, topic)
}

/// Extracts an `AuthSubject` from a `rustls::ServerConnection` peer cert
/// (for `AuthMode::Mtls`). Returns `None` if no cert was presented.
///
/// Subject name = X.509 subject DN as a string (DER bytes hex-encoded
/// as a fallback if X.500 does not parse).
#[must_use]
pub fn extract_mtls_subject(conn: &rustls::ServerConnection) -> Option<AuthSubject> {
    let certs = conn.peer_certificates()?;
    let leaf = certs.first()?;
    // We use the cert DER SHA256 fingerprint as a stable identity.
    // Spec §7.2: mTLS subject = SubjectDN OR hash. We choose hash for
    // determinism without an X.500 DN parser dep.
    let hash = sha256_hex(leaf.as_ref());
    Some(AuthSubject::new(format!("mtls:{hash}")))
}

fn sha256_hex(data: &[u8]) -> String {
    use crate::backend::digest::{Context, SHA256};
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
