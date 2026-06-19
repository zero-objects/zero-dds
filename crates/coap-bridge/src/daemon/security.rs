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
/// [`SecurityCtx`]. CoAP-specific: TLS stays rejected (DTLS in next
/// phase), but auth + ACL are fully wired.
///
/// # Errors
/// [`SecurityError`] on auth-mode config error.
pub fn ctx_from_daemon_config(
    cfg: &super::config::DaemonConfig,
) -> Result<SecurityCtx, SecurityError> {
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
    for (topic, (read, write)) in &cfg.topic_acl {
        sc.topic_acl.insert(
            topic.clone(),
            AclEntry {
                read: read.clone(),
                write: write.clone(),
            },
        );
    }
    forbid_tls_until_dtls_lib(&sc)?;
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
) -> Result<AuthSubject, AuthError> {
    let header_str = match auth_token_option {
        Some(b) => Some(
            core::str::from_utf8(b)
                .map_err(|_| AuthError::MalformedCredentials("auth-token utf8".into()))?,
        ),
        None => None,
    };
    zerodds_bridge_security::authenticate(auth, header_str, None, None)
}

/// Throws a clear error when TLS cert paths are set but the CoAP
/// daemon does not yet have a DTLS library.
///
/// # Errors
/// [`SecurityError::Tls`] with a "DTLS in next phase" hint.
pub fn forbid_tls_until_dtls_lib(cfg: &SecurityConfig) -> Result<(), SecurityError> {
    if cfg.tls_cert.is_some() || cfg.tls_key.is_some() {
        return Err(SecurityError::Tls(
            zerodds_bridge_security::TlsConfigError::Rustls(
                "CoAP §7.1: DTLS server not yet wired; rustls 0.23 is TCP-TLS only. \
                 Track: \"DTLS in next phase\"."
                    .into(),
            ),
        ));
    }
    Ok(())
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
        let s = authenticate_coap(&auth, Some(b"Bearer tok")).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn coap_missing_option_with_bearer_rejected() {
        let auth = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = authenticate_coap(&auth, None).unwrap_err();
        assert!(matches!(err, AuthError::MissingCredentials));
    }

    #[test]
    fn coap_none_auth_yields_anonymous() {
        let s = authenticate_coap(&AuthMode::None, None).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    #[test]
    fn coap_dtls_phase_marker_blocks_tls_cli() {
        let cfg = SecurityConfig {
            tls_cert: Some("/x".into()),
            tls_key: Some("/y".into()),
            ..Default::default()
        };
        let err = forbid_tls_until_dtls_lib(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::Tls(_)));
    }

    #[test]
    fn coap_no_tls_args_passes_phase_check() {
        forbid_tls_until_dtls_lib(&SecurityConfig::default()).unwrap();
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
}
