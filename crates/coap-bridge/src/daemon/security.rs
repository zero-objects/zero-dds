// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP bridge §7.x security wireup.
//!
//! Spec: `zerodds-coap-bridge-1.0.md` §7.
//!
//! **§7.1 TLS status**: rustls 0.23 has no DTLS server mode (DTLS
//! sits on UDP, rustls only handles TLS over TCP). A pure-Rust
//! DTLS server library (e.g. webrtc-dtls) is not approved in the
//! workspace. Spec §7.1 for CoAP is therefore fulfilled in two
//! sub-steps:
//!
//! 1. **This layer**: auth (§7.2 — bearer in CoAP option `Auth-Token`
//!    via option number 65000 as a private extension, not IANA-fixed)
//!    + topic ACL (§7.3) fully wired.
//! 2. **Follow-up phase**: DTLS acceptor wireup with a pure-Rust DTLS
//!    library (tracking: `coap-bridge` issue "DTLS in next phase").
//!
//! When the CLI sets `--tls-cert <FILE>`, the daemon setup emits a
//! clear error that DTLS is coming in the follow-up phase — no
//! silent failure.
//!
//! Auth hook for CoAP: the first byte array from the request that is
//! present in the `Auth-Token` option (typically `Bearer <jwt>` format).

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityCtx,
    SecurityError, authorize, build_ctx,
};

/// Translates a [`super::config::DaemonConfig`] into a
/// [`SecurityCtx`]: auth and ACL. DTLS is the transport's
/// (`coap.dtls.*`, `dtls_transport`); with it, `auth.mode: mtls` takes the
/// subject from the handshake.
///
/// # Errors
/// [`SecurityError`] on auth-mode config error.
pub fn ctx_from_daemon_config(
    cfg: &super::config::DaemonConfig,
) -> Result<SecurityCtx, SecurityError> {
    let mut sc = SecurityConfig::default();
    sc.auth_mode = cfg.auth_mode.clone();
    match cfg.auth_mode.as_str() {
        "jwt" => sc.apply_jwt(&cfg.auth_jwt)?,
        // The identity comes from the DTLS handshake, so plain CoAP cannot
        // serve it and every request would be 4.01.
        "mtls" if !cfg.dtls_enabled => {
            return Err(SecurityError::Config(
                "auth.mode: mtls needs coap.dtls.enabled".into(),
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
    // Spec §7.3 `acl.default_deny`: `false` opens every resource that no
    // rule names; `true` (the default) leaves the ACL fail-closed.
    if !cfg.acl_default_deny {
        sc.topic_acl_default = Some(AclEntry::allow_all());
    } else if cfg.acl_default_deny_set {
        // An explicit `default_deny: true`: a default entry that names no
        // subject denies the unnamed rest, and keeps `build_ctx` from its open
        // fallback for an ACL nobody configured.
        sc.topic_acl_default = Some(AclEntry {
            read: Vec::new(),
            write: Vec::new(),
        });
    }
    build_ctx(&sc)
}

/// CoAP-Option-Number for Bearer-Token: 65000 (private extension range
/// per RFC 7252 §12.2). Spec §7.2.
pub const COAP_OPTION_AUTH_TOKEN: u16 = 65000;

/// CoAP-specific auth hook: the bearer token comes from a
/// CoAP option (option number = 65000; we use the
/// "private extension" range, RFC 7252 §12.2).
///
/// The daemon code reads the option from the request and passes it
/// in here. The format is expected to be `Bearer <token>` (RFC 6750).
///
/// # Errors
/// [`AuthError`] on missing/malformed/rejected.
pub fn authenticate_coap(
    auth: &AuthMode,
    auth_token_option: Option<&[u8]>,
    dtls_peer: Option<&AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let header_str = match auth_token_option {
        Some(b) => Some(
            core::str::from_utf8(b)
                .map_err(|_| AuthError::MalformedCredentials("auth-token utf8".into()))?,
        ),
        None => None,
    };
    zerodds_bridge_security::authenticate(auth, header_str, None, dtls_peer.cloned())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn coap_bearer_in_option_accepted() {
        let mut tokens = HashMap::new();
        tokens.insert("tok".into(), AuthSubject::new("alice"));
        let auth = AuthMode::Bearer { tokens };
        let s = authenticate_coap(&auth, Some(b"Bearer tok"), None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn coap_missing_option_with_bearer_rejected() {
        let auth = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = authenticate_coap(&auth, None, None).unwrap_err();
        assert!(matches!(err, AuthError::MissingCredentials));
    }

    #[test]
    fn coap_none_auth_yields_anonymous() {
        let s = authenticate_coap(&AuthMode::None, None, None).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    #[test]
    fn coap_acl_subscribe_check() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "rd/temp".into(),
            AclEntry {
                read: vec!["*".into()],
                write: vec!["sensor".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &bob, AclOp::Read, "rd/temp"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Write, "rd/temp"));
    }

    /// `auth.mode: jwt` reads `auth.jwt.public_key`; without it the setting
    /// cannot be served and the context is refused as a configuration error.
    #[test]
    fn jwt_without_a_public_key_is_a_config_error() {
        let mut cfg = crate::daemon::config::DaemonConfig::default_for_dev();
        cfg.auth_mode = "jwt".into();
        let err = ctx_from_daemon_config(&cfg).unwrap_err();
        assert!(
            matches!(&err, SecurityError::Config(m) if m.contains("auth.jwt.public_key")),
            "{err}"
        );
    }
}
