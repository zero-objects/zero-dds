// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT-Bridge §7.x Security-Wireup: TLS (mqtts:// auf Port 8883) +
//! Auth (Username/Password aus CONNECT-Packet → `bearer` oder `sasl`)
//! + Topic-ACL pro Subscribe/Publish.
//!
//! Spec: `zerodds-mqtt-bridge-1.0.md` §7. Logik kommt aus
//! [`zerodds_bridge_security`]; dieses Modul bietet den MQTT-spezifischen
//! Auth-Hook (CONNECT-Packet `username` + `password`).

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, RotatingTlsConfig, SecurityConfig,
    SecurityCtx, SecurityError, authorize, build_client_tls_connector, build_ctx,
    extract_mtls_subject, parse_server_name,
};

use rustls::ClientConfig;
use std::path::PathBuf;
use std::sync::Arc;

/// MQTT-spezifischer Auth-Hook: prüft Username/Password aus dem
/// CONNECT-Packet (MQTT v5 §3.1.3.5/3.1.3.6) gegen [`AuthMode`].
///
/// `username` und `password` kommen aus `zerodds_mqtt_bridge::
/// control_packets::ConnectPacket::user_name` /`password`.
///
/// Mapping:
/// * `AuthMode::None` → ignoriert die Credentials, gibt anonymous.
/// * `AuthMode::Bearer` → erwartet `password` als Bearer-Token; das
///   `username` wird ignoriert (es überschreibt aber den Subject-Namen
///   nicht, der kommt aus dem Token-Map-Lookup).
/// * `AuthMode::SaslPlain` → bei MQTT die natürliche Form: User/Pass
///   direkt aus den CONNECT-Feldern.
/// * `AuthMode::Mtls` → `mtls_subject` muss aus dem TLS-Layer kommen.
/// * `AuthMode::Jwt` → `password` enthält das JWT.
///
/// # Errors
/// [`AuthError`] bei jeder Form von Reject/Missing/Malformed.
pub fn authenticate_mqtt(
    auth: &AuthMode,
    username: Option<&str>,
    password: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    match auth {
        AuthMode::None => Ok(AuthSubject::anonymous()),
        AuthMode::Bearer { .. } | AuthMode::Jwt { .. } => {
            // Password = "<token>" → in einen `Authorization: Bearer …`
            // Header umverpacken, damit der Standard-Pfad greift.
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

/// Übersetzt die Daemon-`DaemonConfig` in einen [`SecurityCtx`] und —
/// für `mqtts://`-Pfade — eine rustls-`ClientConfig` zum Broker.
///
/// MQTT-Bridge ist ein Bridge-Client, also brauchen wir keinen
/// Server-Side-TLS-Acceptor (kein [`RotatingTlsConfig`]); stattdessen
/// liefern wir den Out-Bound-`Arc<ClientConfig>` für den TCP→TLS-Wrap
/// im [`super::client::MqttClient`].
///
/// # Errors
/// [`SecurityError`] bei TLS-Lade- oder Auth-Mode-Konfig-Fehler.
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

/// Mappt die [`AuthMode`]+optional User-/Pass auf das, was im
/// MQTT-CONNECT-Packet als `username`/`password` rausgehen muss.
///
/// * `AuthMode::None` → no creds.
/// * `AuthMode::Bearer` → `password = <token>`, `username = ""`.
/// * `AuthMode::SaslPlain` → `username/password` aus Config.
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
