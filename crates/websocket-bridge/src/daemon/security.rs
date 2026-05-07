// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WS-Bridge §7.x Security-Wireup: TLS-Acceptor, Auth-Validation,
//! Topic-ACL. Sitzt zwischen `accept()` und dem WS-Handshake-Pfad
//! in [`super::server`].
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §7. Die eigentliche Logik kommt
//! aus [`zerodds_bridge_security`] — dieses Modul ist nur der WS-
//! spezifische Hook (`Authorization`-Header aus dem HTTP-Upgrade-
//! Request).

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, RotatingTlsConfig, SecurityConfig,
    SecurityCtx, SecurityError, authenticate as bs_authenticate, authorize, build_ctx,
    extract_mtls_subject, serve_tls_handshake,
};

/// Übersetzt die Daemon-CLI/YAML-`DaemonConfig` in einen
/// [`SecurityCtx`] + optionale [`RotatingTlsConfig`] für Hot-Reload.
///
/// Wird beim Daemon-Start und beim SIGHUP-Reload identisch genutzt.
///
/// # Errors
/// [`SecurityError`] bei TLS-Lade- oder Auth-Mode-Konfig-Fehler.
pub fn ctx_from_daemon_config(
    cfg: &super::config::DaemonConfig,
) -> Result<(SecurityCtx, Option<RotatingTlsConfig>), SecurityError> {
    let mut sc = SecurityConfig::default();
    if cfg.tls_enabled {
        if cfg.tls_cert_file.is_empty() || cfg.tls_key_file.is_empty() {
            return Err(SecurityError::Tls(
                zerodds_bridge_security::TlsConfigError::Rustls(
                    "tls.enabled=true requires cert_file + key_file".into(),
                ),
            ));
        }
        sc.tls_cert = Some(cfg.tls_cert_file.clone().into());
        sc.tls_key = Some(cfg.tls_key_file.clone().into());
        if !cfg.tls_client_ca_file.is_empty() {
            sc.client_ca = Some(cfg.tls_client_ca_file.clone().into());
        }
    }
    sc.auth_mode = cfg.auth_mode.clone();
    if let (Some(tok), Some(subj)) = (
        cfg.auth_bearer_token.as_ref(),
        cfg.auth_bearer_subject.as_ref(),
    ) {
        sc.bearer_tokens.insert(tok.clone(), subj.clone());
    } else if let Some(tok) = cfg.auth_bearer_token.as_ref() {
        sc.bearer_tokens.insert(tok.clone(), "anonymous".into());
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
    let rotating = match (cfg.tls_enabled, sc.tls_cert.clone(), sc.tls_key.clone()) {
        (true, Some(c), Some(k)) => {
            Some(RotatingTlsConfig::load(c, k, sc.client_ca.clone()).map_err(SecurityError::Tls)?)
        }
        _ => None,
    };
    Ok((ctx, rotating))
}

/// WS-spezifischer Auth-Hook: extrahiert Bearer-/JWT-Token aus dem
/// HTTP-Upgrade-Request und prüft gegen [`AuthMode`].
///
/// `headers` ist die rohe Header-Liste aus
/// [`crate::handshake::ClientRequest::headers`] (case-insensitive
/// match auf `Authorization`).
///
/// # Errors
/// [`AuthError`] bei missing/malformed/rejected.
pub fn authenticate_ws(
    auth: &AuthMode,
    headers: &[(String, String)],
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let auth_header = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str());
    bs_authenticate(auth, auth_header, None, mtls_subject)
}

/// Mini-HTTP-Header-Extractor für den Raw-Handshake-Request-String.
/// Liefert den Wert des ersten `Authorization:`-Headers (case-
/// insensitive) ohne CRLF-Trailer.
#[must_use]
pub fn extract_authorization_header(raw_request: &str) -> Option<String> {
    for line in raw_request.split("\r\n") {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("authorization") {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn ws_extracts_bearer_from_headers_case_insensitive() {
        let mut tokens = HashMap::new();
        tokens.insert("tok".to_string(), AuthSubject::new("alice"));
        let auth = AuthMode::Bearer { tokens };
        let headers = vec![
            ("Host".to_string(), "x".to_string()),
            ("authorization".to_string(), "Bearer tok".to_string()),
        ];
        let s = authenticate_ws(&auth, &headers, None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn ws_missing_authorization_header_returns_missing() {
        let auth = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = authenticate_ws(&auth, &[], None).unwrap_err();
        assert!(matches!(err, AuthError::MissingCredentials));
    }

    #[test]
    fn ws_none_auth_yields_anonymous_without_header() {
        let s = authenticate_ws(&AuthMode::None, &[], None).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    #[test]
    fn extract_auth_header_finds_case_insensitive() {
        let raw = "GET /x HTTP/1.1\r\nHost: y\r\nAuthorization: Bearer abc\r\n\r\n";
        assert_eq!(
            extract_authorization_header(raw).as_deref(),
            Some("Bearer abc")
        );
    }

    #[test]
    fn extract_auth_header_returns_none_when_missing() {
        let raw = "GET /x HTTP/1.1\r\nHost: y\r\n\r\n";
        assert!(extract_authorization_header(raw).is_none());
    }

    #[test]
    fn ctx_from_config_with_bearer_tokens() {
        use super::super::config::DaemonConfig;
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.auth_mode = "bearer".into();
        cfg.auth_bearer_token = Some("tok123".into());
        cfg.auth_bearer_subject = Some("alice".into());
        cfg.topic_acl
            .insert("Trade".into(), (vec!["alice".into()], vec!["alice".into()]));
        let (ctx, rot) = ctx_from_daemon_config(&cfg).expect("build");
        assert!(rot.is_none());
        assert!(matches!(*ctx.auth, AuthMode::Bearer { .. }));
        let alice = AuthSubject::new("alice");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "Trade"));
    }

    #[test]
    fn ctx_from_config_tls_enabled_without_paths_rejected() {
        use super::super::config::DaemonConfig;
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.tls_enabled = true;
        let err = ctx_from_daemon_config(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::Tls(_)));
    }

    #[test]
    fn ws_acl_check_allows_listed_user() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "Trade".into(),
            AclEntry {
                read: vec!["alice".into()],
                write: vec!["alice".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let alice = AuthSubject::new("alice");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "Trade"));
        assert!(authorize(&ctx.acl, &alice, AclOp::Write, "Trade"));
    }
}
