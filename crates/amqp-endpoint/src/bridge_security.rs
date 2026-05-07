// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP-Endpoint §7.x Bridge-Security-Wireup (separat von dem
//! DDS-Security-Mapper in [`crate::security`]).
//!
//! * §7.1 TLS — `amqps://` auf Port 5671 via rustls.
//! * §7.2 Auth — natürlicher Match auf SASL-PLAIN aus dem AMQP-
//!   SASL-Mech-Negotiation; alternativ Bearer-Token in einer
//!   `application-properties`-Map.
//! * §7.3 Topic-ACL — pro AMQP-Address (= DDS-Topic-Name).
//!
//! Spec: `zerodds-amqp-bridge-1.0.md` §7.

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityCtx,
    SecurityError, authorize, build_client_tls_connector, build_ctx, extract_mtls_subject,
    parse_server_name,
};

use std::path::Path;
use std::sync::Arc;

/// Konfiguration fuer den AMQP-Bridge-Client (out-bound zu Broker).
/// Mappt von Daemon-Config-Strings auf den [`build_client_tls_connector`]
/// + [`SecurityCtx`] in einem Aufruf.
#[derive(Debug, Clone, Default)]
pub struct AmqpSecurityConfig {
    /// `true` ⇒ wraps die Broker-Connection mit rustls-Client.
    pub tls_enabled: bool,
    /// PEM-CA-Bundle (Broker-Cert-Validation).
    pub tls_ca_file: String,
    /// Client-Cert (mTLS).
    pub tls_cert_file: String,
    /// Client-Key (mTLS).
    pub tls_key_file: String,
    /// Hostname-Override fuer SNI/Validation.
    pub tls_server_name: String,
    /// Auth-Mode: `none|bearer|sasl|sasl_plain|mtls`.
    pub auth_mode: String,
    /// Bearer-Token (XOAUTH2-Anlehnung).
    pub bearer_token: Option<String>,
    /// Bearer-Subject — lokaler ACL-Subject fuer den Bridge-Identity.
    pub bearer_subject: Option<String>,
    /// SASL-PLAIN Out-Bound User.
    pub sasl_username: Option<String>,
    /// SASL-PLAIN Out-Bound Password.
    pub sasl_password: Option<String>,
    /// Topic-ACL.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
}

/// Baut [`SecurityCtx`] + optional rustls-`ClientConfig` fuer die
/// AMQP-Out-Bound-Verbindung.
///
/// # Errors
/// [`SecurityError`] bei Lade-/Auth-Mode-Fehler.
pub fn ctx_from_amqp_config(
    cfg: &AmqpSecurityConfig,
) -> Result<(SecurityCtx, Option<Arc<rustls::ClientConfig>>), SecurityError> {
    let mut sc = SecurityConfig {
        auth_mode: cfg.auth_mode.clone(),
        ..Default::default()
    };
    if let (Some(tok), Some(subj)) = (cfg.bearer_token.as_ref(), cfg.bearer_subject.as_ref()) {
        sc.bearer_tokens.insert(tok.clone(), subj.clone());
    } else if let Some(tok) = cfg.bearer_token.as_ref() {
        sc.bearer_tokens.insert(tok.clone(), "anonymous".into());
    }
    if let (Some(u), Some(p)) = (cfg.sasl_username.as_ref(), cfg.sasl_password.as_ref()) {
        sc.sasl_users.insert(u.clone(), p.clone());
    }
    for (topic, (read, write)) in &cfg.topic_acl {
        sc.topic_acl.insert(
            topic.clone(),
            AclEntry {
                read: read.clone(),
                write: write.clone(),
            },
        );
    }
    let ctx = build_ctx(&sc)?;

    let client_tls = if cfg.tls_enabled {
        let ca: Option<&Path> = if cfg.tls_ca_file.is_empty() {
            None
        } else {
            Some(Path::new(&cfg.tls_ca_file))
        };
        let cert: Option<&Path> = if cfg.tls_cert_file.is_empty() {
            None
        } else {
            Some(Path::new(&cfg.tls_cert_file))
        };
        let key: Option<&Path> = if cfg.tls_key_file.is_empty() {
            None
        } else {
            Some(Path::new(&cfg.tls_key_file))
        };
        Some(build_client_tls_connector(ca, cert, key).map_err(SecurityError::Tls)?)
    } else {
        None
    };
    Ok((ctx, client_tls))
}

/// Baut einen SASL-PLAIN-Init-Response-Blob (`\0user\0pass`) aus dem
/// AmqpSecurityConfig. Wird vom Daemon in den SASL-Handshake-Frame
/// gerendert. Returned `None` wenn `auth_mode` nicht SASL ist oder
/// keine Credentials gesetzt sind.
#[must_use]
pub fn sasl_plain_init_response(cfg: &AmqpSecurityConfig) -> Option<Vec<u8>> {
    if !matches!(cfg.auth_mode.as_str(), "sasl" | "sasl_plain") {
        return None;
    }
    let user = cfg.sasl_username.as_deref()?;
    let pass = cfg.sasl_password.as_deref()?;
    let mut buf = Vec::with_capacity(2 + user.len() + pass.len());
    buf.push(0);
    buf.extend_from_slice(user.as_bytes());
    buf.push(0);
    buf.extend_from_slice(pass.as_bytes());
    Some(buf)
}

/// AMQP-spezifischer Auth-Hook: SASL-PLAIN-Frame aus dem
/// SASL-Init-Performative, mit Fallback auf TLS-Client-Cert für mTLS.
///
/// `sasl_init_response` ist der Roh-Bytes-Block aus
/// `SaslInit::initial-response` (RFC 4422 / OASIS AMQP §5.3.3.5);
/// für PLAIN ist das `\0user\0pass` (RFC 4616).
///
/// # Errors
/// [`AuthError`] bei missing/malformed/rejected.
pub fn authenticate_amqp_sasl(
    auth: &AuthMode,
    sasl_init_response: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    zerodds_bridge_security::authenticate(auth, None, sasl_init_response, mtls_subject)
}

/// AMQP-spezifischer Auth-Hook für Bearer-Tokens, die als
/// `application-properties[zerodds:auth-token]` mitgegeben werden
/// (für Clients ohne SASL-PLAIN-Support).
///
/// # Errors
/// [`AuthError`] bei missing/malformed/rejected.
pub fn authenticate_amqp_bearer(
    auth: &AuthMode,
    bearer_value: Option<&str>,
) -> Result<AuthSubject, AuthError> {
    let header = bearer_value.map(|t| format!("Bearer {t}"));
    zerodds_bridge_security::authenticate(auth, header.as_deref(), None, None)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn amqp_sasl_plain_accepts() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let auth = AuthMode::SaslPlain { users };
        let s = authenticate_amqp_sasl(&auth, Some(b"\0alice\0wonderland"), None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn amqp_sasl_plain_rejects_unknown_user() {
        let users = HashMap::new();
        let auth = AuthMode::SaslPlain { users };
        let err = authenticate_amqp_sasl(&auth, Some(b"\0bob\0xx"), None).unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn amqp_bearer_via_application_properties() {
        let mut tokens = HashMap::new();
        tokens.insert("tk".into(), AuthSubject::new("alice"));
        let auth = AuthMode::Bearer { tokens };
        let s = authenticate_amqp_bearer(&auth, Some("tk")).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn amqp_acl_address_check() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "queue/orders".into(),
            AclEntry {
                read: vec!["alice".into()],
                write: vec!["alice".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let alice = AuthSubject::new("alice");
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "queue/orders"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Read, "queue/orders"));
    }

    #[test]
    fn amqp_none_mode_yields_anonymous_for_sasl_anonymous() {
        let s = authenticate_amqp_sasl(&AuthMode::None, None, None).unwrap();
        assert_eq!(s.name, "anonymous");
    }
}
