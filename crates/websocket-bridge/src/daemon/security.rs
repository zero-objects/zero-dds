// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WS-Bridge §7.x security wireup: TLS acceptor, auth validation,
//! topic ACL. Sits between `accept()` and the WS handshake path
//! in [`super::server`].
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §7. The actual logic comes
//! from [`zerodds_bridge_security`] — this module is only the WS-
//! specific hook (`Authorization` header from the HTTP upgrade
//! request).

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, MissingClaim, Refusal,
    RotatingTlsConfig, SecurityConfig, SecurityCtx, SecurityError, authorize, build_ctx,
    extract_mtls_subject, serve_tls_handshake,
};

pub use super::config::listen_is_loopback;

/// Translates the daemon CLI/YAML `DaemonConfig` into a
/// [`SecurityCtx`] + optional [`RotatingTlsConfig`] for hot reload.
///
/// Used identically on daemon start and on SIGHUP reload.
///
/// # Errors
/// [`SecurityError`] on a TLS-load or auth-mode config error, and for
/// `auth.mode: none` on a listen address that is not loopback.
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
    // Spec §7.2: `none` is the development mode, served on a loopback
    // address only; `bearer` needs its token. At start and on a SIGHUP
    // reload.
    cfg.check_auth().map_err(SecurityError::Config)?;
    match cfg.auth_mode.as_str() {
        "jwt" => sc.apply_jwt(&cfg.auth_jwt)?,
        "mtls" if !cfg.tls_enabled || cfg.tls_client_ca_file.is_empty() => {
            return Err(SecurityError::Config(
                "auth.mode: mtls needs tls.enabled and tls.client_ca_file".into(),
            ));
        }
        _ => {}
    }
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

/// WS-specific auth hook: extracts the bearer/JWT token from the
/// HTTP upgrade request and checks it against [`AuthMode`].
///
/// `headers` is the raw header list from
/// `crate::handshake::ClientRequest::headers` (case-insensitive
/// match on `Authorization`).
///
/// # Errors
/// [`Refusal::Unauthenticated`] for credentials that are missing,
/// malformed or rejected (Spec §4.1: 401); [`Refusal::MissingClaim`] for a
/// token that verifies and lacks `auth.jwt.required_claim` (403).
pub fn authenticate_ws(
    auth: &AuthMode,
    headers: &[(String, String)],
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, Refusal> {
    let authorization_header = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str());
    auth.validate_scoped(&zerodds_bridge_security::auth::AuthInput {
        authorization_header,
        sasl_plain_blob: None,
        mtls_subject,
    })
}

/// The HTTP response of the opening handshake to credentials the daemon
/// refuses (Spec §4.1, RFC 6750 §3): 401 with a `Bearer` challenge when
/// they authenticate no one, 403 with `insufficient_scope` when the token
/// verifies and lacks the required claim. The body names the status only.
#[must_use]
pub fn render_refusal(refusal: &Refusal) -> String {
    let (status, body, challenge) = match refusal {
        Refusal::Unauthenticated(AuthError::MissingCredentials) => (
            "401 Unauthorized",
            "unauthorized",
            "Bearer realm=\"zerodds-ws\"".to_string(),
        ),
        Refusal::Unauthenticated(_) => (
            "401 Unauthorized",
            "unauthorized",
            "Bearer realm=\"zerodds-ws\", error=\"invalid_token\"".to_string(),
        ),
        Refusal::MissingClaim(m) => (
            "403 Forbidden",
            "forbidden",
            format!(
                "Bearer realm=\"zerodds-ws\", error=\"insufficient_scope\", scope=\"{}\"",
                challenge_value(&m.value)
            ),
        ),
    };
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nWWW-Authenticate: {challenge}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// `value` as the content of a quoted-string of a challenge (RFC 9110
/// §5.6.4): the characters a scope token may carry (RFC 6749 §3.3), others
/// left out, so that a configured claim value cannot end the header field.
fn challenge_value(value: &str) -> String {
    value
        .chars()
        .filter(|c| matches!(c, '\x21' | '\x23'..='\x5b' | '\x5d'..='\x7e' | ' '))
        .collect()
}

/// Mini HTTP header extractor for the raw handshake request string.
/// Returns the value of the first `Authorization:` header (case-
/// insensitive) without the CRLF trailer.
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
    fn wsbridge_7_2_ws_extracts_bearer_from_headers_case_insensitive() {
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
    fn wsbridge_7_2_ws_missing_authorization_header_returns_missing() {
        let auth = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = authenticate_ws(&auth, &[], None).unwrap_err();
        assert_eq!(err, Refusal::Unauthenticated(AuthError::MissingCredentials));
    }

    #[test]
    fn wsbridge_7_2_ws_none_auth_yields_anonymous_without_header() {
        let s = authenticate_ws(&AuthMode::None, &[], None).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    /// Spec §4.1 with RFC 6750 §3, §3.1: 401 with a `Bearer` challenge for
    /// credentials that authenticate no one (no `error` when none came),
    /// 403 with `insufficient_scope` and the scope for a verified token
    /// without the required claim.
    #[test]
    fn wsbridge_4_1_the_refusal_is_401_or_403_with_its_challenge() {
        let missing = render_refusal(&Refusal::Unauthenticated(AuthError::MissingCredentials));
        assert!(missing.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(missing.contains("WWW-Authenticate: Bearer realm=\"zerodds-ws\"\r\n"));
        let rejected = render_refusal(&Refusal::Unauthenticated(AuthError::Rejected(
            "jwt: expired".into(),
        )));
        assert!(rejected.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(rejected.contains("error=\"invalid_token\""));
        assert!(!rejected.contains("expired"), "the reason stays in the log");
        let scope = render_refusal(&Refusal::MissingClaim(Box::new(MissingClaim {
            subject: AuthSubject::new("alice"),
            claim: "scope".into(),
            value: "dds.read\"\r\nX-Injected: 1".into(),
        })));
        assert!(scope.starts_with("HTTP/1.1 403 Forbidden\r\n"));
        assert!(scope.contains(
            "WWW-Authenticate: Bearer realm=\"zerodds-ws\", error=\"insufficient_scope\", scope=\"dds.readX-Injected: 1\"\r\n"
        ));
        assert!(scope.ends_with("\r\n\r\nforbidden"));
        for response in [&missing, &rejected, &scope] {
            assert!(response.contains("Connection: close\r\n"));
        }
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
    fn wsbridge_7_2_ctx_from_config_with_bearer_tokens() {
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
    fn ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected() {
        use super::super::config::DaemonConfig;
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.tls_enabled = true;
        let err = ctx_from_daemon_config(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::Tls(_)));
    }

    /// Spec §7.2: `none` (and the empty mode, its default) only on a
    /// loopback listen address; the other modes on any address.
    #[test]
    fn wsbridge_7_2_none_needs_a_loopback_listen_address() {
        use super::super::config::DaemonConfig;
        for (listen, loopback) in [
            ("127.0.0.1:8080", true),
            ("127.0.0.53:8080", true),
            ("[::1]:8080", true),
            ("localhost:8080", true),
            ("0.0.0.0:8080", false),
            ("[::]:8080", false),
            ("192.0.2.1:8080", false),
            ("no port", false),
        ] {
            assert_eq!(listen_is_loopback(listen), loopback, "{listen}");
        }
        let mut cfg = DaemonConfig::default_for_dev();
        for mode in ["none", ""] {
            cfg.auth_mode = mode.into();
            cfg.listen = "127.0.0.1:0".into();
            assert!(ctx_from_daemon_config(&cfg).is_ok(), "{mode} on loopback");
            cfg.listen = "0.0.0.0:8080".into();
            match ctx_from_daemon_config(&cfg) {
                Err(SecurityError::Config(m)) => assert!(m.contains("0.0.0.0:8080"), "{m}"),
                Err(e) => panic!("{mode}: {e}"),
                Ok(_) => panic!("{mode}: accepted on 0.0.0.0"),
            }
        }
        cfg.auth_mode = "bearer".into();
        cfg.auth_bearer_token = Some("t".into());
        assert!(ctx_from_daemon_config(&cfg).is_ok());
    }

    /// Spec §7.2 `bearer` checks against `auth.bearer_token`: without one,
    /// or with an empty one (an unset `${VAR}`), the mode cannot be served.
    #[test]
    fn wsbridge_7_2_bearer_needs_a_token() {
        use super::super::config::DaemonConfig;
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.auth_mode = "bearer".into();
        for token in [None, Some(String::new())] {
            cfg.auth_bearer_token = token.clone();
            match ctx_from_daemon_config(&cfg) {
                Err(SecurityError::Config(m)) => assert!(m.contains("bearer_token"), "{m}"),
                Err(e) => panic!("{token:?}: {e}"),
                Ok(_) => panic!("{token:?}: bearer without a token"),
            }
        }
    }

    #[test]
    fn wsbridge_7_3_ws_acl_check_allows_listed_user() {
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
