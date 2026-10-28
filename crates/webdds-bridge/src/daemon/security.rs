// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Security wireup: TLS acceptor, auth validation, per-Application ACL
//! (see `daemon::config`'s module doc for why Application, not topic).
//! The actual logic comes from [`zerodds_bridge_security`] — this
//! module is only the HTTP-specific hook (the `Authorization` header)
//! and the config -> `SecurityCtx` translation, the same shape every
//! other `zerodds-*-bridged` daemon uses.

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, RotatingTlsConfig, SecurityConfig,
    SecurityCtx, SecurityError, authenticate as bs_authenticate, authorize, build_ctx,
    extract_mtls_subject, serve_tls_handshake,
};

/// Translates the daemon CLI/YAML `DaemonConfig` into a
/// [`SecurityCtx`] + optional [`RotatingTlsConfig`] for hot reload.
///
/// # Errors
/// [`SecurityError`] on a TLS-load or auth-mode config error.
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
    for (app, (read, write)) in &cfg.app_acl {
        sc.topic_acl.insert(
            app.clone(),
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

/// HTTP-specific auth hook: extracts the bearer/JWT token from the
/// request's `Authorization` header and checks it against [`AuthMode`].
///
/// # Errors
/// [`AuthError`] on missing/malformed/rejected.
pub fn authenticate_http(
    auth: &AuthMode,
    authorization_header: Option<&str>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    bs_authenticate(auth, authorization_header, None, mtls_subject)
}

/// The Application name a request's path names — the ACL's resource
/// unit (see the module doc). `None` for a request path that names no
/// application (`/dds/rest1/applications` itself, `/wsdl/*`, an admin
/// endpoint) — those are `Read`-checked against `*`.
#[must_use]
pub fn application_of_path(path: &str) -> Option<String> {
    let marker = "/applications/";
    let start = path.find(marker)? + marker.len();
    let rest = &path[start..];
    let end = rest.find('/').unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty()).then(|| name.to_string())
}

/// The [`AclOp`] a request implies: a `GET`/`HEAD` reads, everything
/// else (`POST`/`PUT`/`DELETE`) writes.
#[must_use]
pub fn op_of_method(method: &str) -> AclOp {
    if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD") {
        AclOp::Read
    } else {
        AclOp::Write
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn application_of_path_extracts_the_name() {
        assert_eq!(
            application_of_path("/dds/rest1/applications/app1/domain_participants"),
            Some("app1".to_string())
        );
    }

    #[test]
    fn application_of_path_none_for_the_collection_itself() {
        assert_eq!(application_of_path("/dds/rest1/applications"), None);
    }

    #[test]
    fn op_of_method_maps_get_to_read() {
        assert_eq!(op_of_method("GET"), AclOp::Read);
        assert_eq!(op_of_method("HEAD"), AclOp::Read);
        assert_eq!(op_of_method("POST"), AclOp::Write);
        assert_eq!(op_of_method("DELETE"), AclOp::Write);
    }

    #[test]
    fn ctx_from_config_with_bearer_tokens() {
        let mut cfg = super::super::config::DaemonConfig::default_for_dev();
        cfg.auth_mode = "bearer".into();
        cfg.auth_bearer_token = Some("tok123".into());
        cfg.auth_bearer_subject = Some("alice".into());
        cfg.app_acl
            .insert("app1".into(), (vec!["alice".into()], vec!["alice".into()]));
        let (ctx, rot) = ctx_from_daemon_config(&cfg).expect("build");
        assert!(rot.is_none());
        assert!(matches!(*ctx.auth, AuthMode::Bearer { .. }));
        let alice = AuthSubject::new("alice");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "app1"));
    }

    #[test]
    fn ctx_from_config_tls_enabled_without_paths_rejected() {
        let mut cfg = super::super::config::DaemonConfig::default_for_dev();
        cfg.tls_enabled = true;
        assert!(matches!(
            ctx_from_daemon_config(&cfg).unwrap_err(),
            SecurityError::Tls(_)
        ));
    }
}
