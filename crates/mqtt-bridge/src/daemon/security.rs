// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT bridge §7.x security wireup: TLS (mqtts:// on port 8883) +
//! auth (username/password from the CONNECT packet → `bearer` or `sasl`)
//! + topic ACL per subscribe/publish.
//!
//! Spec: `zerodds-mqtt-bridge-1.0.md` §7. The logic comes from
//! [`zerodds_bridge_security`]; this module provides the MQTT-specific
//! auth hook (CONNECT packet `username` + `password`).

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, RotatingTlsConfig, SecurityConfig,
    SecurityCtx, SecurityError, authorize, build_client_tls_connector, build_ctx,
    extract_mtls_subject, parse_server_name,
};

use rustls::ClientConfig;
use std::path::PathBuf;
use std::sync::Arc;

/// MQTT-specific auth hook: checks username/password from the
/// CONNECT packet (MQTT v5 §3.1.3.5/3.1.3.6) against [`AuthMode`].
///
/// `username` and `password` come from `zerodds_mqtt_bridge::
/// control_packets::ConnectPacket::user_name` /`password`.
///
/// Mapping:
/// * `AuthMode::None` → ignores the credentials, returns anonymous.
/// * `AuthMode::Bearer` → expects `password` as a bearer token; the
///   `username` is ignored (it does not override the subject name,
///   which comes from the token-map lookup).
/// * `AuthMode::SaslPlain` → the natural form for MQTT: user/pass
///   directly from the CONNECT fields.
/// * `AuthMode::Mtls` → `mtls_subject` must come from the TLS layer.
/// * `AuthMode::Jwt` → `password` contains the JWT.
///
/// # Errors
/// [`AuthError`] on any form of reject/missing/malformed.
pub fn authenticate_mqtt(
    auth: &AuthMode,
    username: Option<&str>,
    password: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    match auth {
        AuthMode::None => Ok(AuthSubject::anonymous()),
        AuthMode::Bearer { .. } | AuthMode::Jwt { .. } => {
            // Password = "<token>" → repackage into an `Authorization: Bearer …`
            // header so the standard path applies.
            let pw = password.ok_or(AuthError::MissingCredentials)?;
            let token = core::str::from_utf8(pw)
                .map_err(|_| AuthError::MalformedCredentials("password not utf8".into()))?;
            let hdr = format!("Bearer {token}");
            zerodds_bridge_security::authenticate(auth, Some(&hdr), None, mtls_subject)
        }
        AuthMode::SaslPlain { .. } => {
            let user = username.ok_or(AuthError::MissingCredentials)?;
            let pass = password.ok_or(AuthError::MissingCredentials)?;
            // SASL-PLAIN-Frame `\0user\0pass` zusammenbauen.
            let mut blob = Vec::with_capacity(2 + user.len() + pass.len());
            blob.push(0);
            blob.extend_from_slice(user.as_bytes());
            blob.push(0);
            blob.extend_from_slice(pass);
            zerodds_bridge_security::authenticate(auth, None, Some(&blob), mtls_subject)
        }
        AuthMode::Mtls => zerodds_bridge_security::authenticate(auth, None, None, mtls_subject),
    }
}

/// Translates the daemon `DaemonConfig` into a [`SecurityCtx`] and —
/// for `mqtts://` paths — a rustls `ClientConfig` to the broker.
///
/// The MQTT bridge is a bridge client, so we need no
/// server-side TLS acceptor (no [`RotatingTlsConfig`]); instead
/// we provide the outbound `Arc<ClientConfig>` for the TCP→TLS wrap
/// in [`super::client::MqttClient`].
///
/// # Errors
/// [`SecurityError`] on a TLS-load or auth-mode config error.
pub fn ctx_from_daemon_config(
    cfg: &super::config::DaemonConfig,
) -> Result<(SecurityCtx, Option<Arc<ClientConfig>>), SecurityError> {
    let mut sc = SecurityConfig::default();
    sc.auth_mode = cfg.auth_mode.clone();
    if let (Some(tok), Some(subj)) = (
        cfg.auth_bearer_token.as_ref(),
        cfg.auth_bearer_subject.as_ref(),
    ) {
        sc.bearer_tokens.insert(tok.clone(), subj.clone());
    } else if let Some(tok) = cfg.auth_bearer_token.as_ref() {
        sc.bearer_tokens.insert(tok.clone(), "anonymous".into());
    }
    if !cfg.sasl_users.is_empty() {
        for (u, p) in &cfg.sasl_users {
            sc.sasl_users.insert(u.clone(), p.clone());
        }
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
    let client_tls = if cfg.broker_tls_enabled {
        let ca: Option<PathBuf> = if cfg.broker_tls_ca_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(&cfg.broker_tls_ca_file))
        };
        let cert: Option<PathBuf> = if cfg.broker_tls_client_cert_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(&cfg.broker_tls_client_cert_file))
        };
        let key: Option<PathBuf> = if cfg.broker_tls_client_key_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(&cfg.broker_tls_client_key_file))
        };
        Some(
            build_client_tls_connector(ca.as_deref(), cert.as_deref(), key.as_deref())
                .map_err(SecurityError::Tls)?,
        )
    } else {
        None
    };
    Ok((ctx, client_tls))
}

/// Maps the [`AuthMode`] + optional user/pass to what must go out in the
/// MQTT CONNECT packet as `username`/`password`.
///
/// * `AuthMode::None` → no creds.
/// * `AuthMode::Bearer` → `password = <token>`, `username = ""`.
/// * `AuthMode::SaslPlain` → `username/password` from config.
#[must_use]
pub fn outbound_credentials(
    cfg: &super::config::DaemonConfig,
) -> (Option<String>, Option<Vec<u8>>) {
    match cfg.auth_mode.as_str() {
        "bearer" => {
            let pw = cfg
                .auth_bearer_token
                .as_ref()
                .map(|s| s.as_bytes().to_vec());
            (None, pw)
        }
        "sasl" | "sasl_plain" => {
            let user = cfg.outbound_username.clone();
            let pass = cfg
                .outbound_password
                .as_ref()
                .map(|s| s.as_bytes().to_vec());
            (user, pass)
        }
        _ => (
            cfg.outbound_username.clone(),
            cfg.outbound_password
                .as_ref()
                .map(|s| s.as_bytes().to_vec()),
        ),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn mqtt_sasl_plain_accepts_user_pass() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let auth = AuthMode::SaslPlain { users };
        let s = authenticate_mqtt(&auth, Some("alice"), Some(b"wonderland"), None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn mqtt_sasl_plain_rejects_wrong_password() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let auth = AuthMode::SaslPlain { users };
        let err = authenticate_mqtt(&auth, Some("alice"), Some(b"wrong"), None).unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn mqtt_bearer_treats_password_as_token() {
        let mut tokens = HashMap::new();
        tokens.insert("tok1".into(), AuthSubject::new("alice"));
        let auth = AuthMode::Bearer { tokens };
        let s = authenticate_mqtt(&auth, None, Some(b"tok1"), None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn mqtt_none_yields_anonymous_without_creds() {
        let s = authenticate_mqtt(&AuthMode::None, None, None, None).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    #[test]
    fn mqtt_acl_publish_check() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "sensors/temp".into(),
            AclEntry {
                read: vec!["*".into()],
                write: vec!["alice".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let alice = AuthSubject::new("alice");
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &alice, AclOp::Write, "sensors/temp"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Write, "sensors/temp"));
        assert!(authorize(&ctx.acl, &bob, AclOp::Read, "sensors/temp"));
    }
}
