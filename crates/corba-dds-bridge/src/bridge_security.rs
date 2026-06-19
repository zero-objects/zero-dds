// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA-DDS-Bridge §7.x bridge-security wire-up.
//!
//! * §7.1 TLS — SSLIOP TaggedComponent 0x06 via rustls (the CORBA
//!   profile body carries the TLS listen address).
//! * §7.2 Auth — either via a CSIv2 token (see
//!   [`crate::csiv2_wire`]) OR an mTLS client cert. For RC1 we use
//!   the `Authorization: Bearer …` path as a service-context mapping
//!   (CORBA service-context ID = 256, vendor extension).
//! * §7.3 Topic ACL — per DDS-topic bridge mapping.
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §7.

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityCtx,
    SecurityError, authorize, build_ctx, extract_mtls_subject,
};

/// CORBA-specific auth hook: extracts the bearer token from a
/// GIOP service context (`context_id = 256`, vendor extension)
/// or passes through the mTLS subject from the TLS layer.
///
/// `service_context_token` is the byte block from
/// `IOP::ServiceContext::context_data` (vendor schema:
/// `\0bearer\0<utf8 token>`). In `mTLS` mode the `mtls_subject`
/// from the rustls peer cert is used instead.
///
/// # Errors
/// [`AuthError`] on missing/malformed/rejected.
pub fn authenticate_corba(
    auth: &AuthMode,
    service_context_token: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let header = match service_context_token {
        Some(b) => {
            // Expected format: `\0bearer\0<token>` (vendor schema).
            let mut parts = b.splitn(3, |x| *x == 0);
            let _ = parts.next();
            let kind = parts
                .next()
                .ok_or_else(|| AuthError::MalformedCredentials("svc-ctx empty".into()))?;
            let tok = parts
                .next()
                .ok_or_else(|| AuthError::MalformedCredentials("svc-ctx no token".into()))?;
            if !kind.eq_ignore_ascii_case(b"bearer") {
                return Err(AuthError::MalformedCredentials(
                    "svc-ctx expected bearer".into(),
                ));
            }
            let s = core::str::from_utf8(tok)
                .map_err(|_| AuthError::MalformedCredentials("svc-ctx token utf8".into()))?;
            Some(format!("Bearer {s}"))
        }
        None => None,
    };
    zerodds_bridge_security::authenticate(auth, header.as_deref(), None, mtls_subject)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn corba_bearer_via_service_context() {
        let mut tokens = HashMap::new();
        tokens.insert("tk".into(), AuthSubject::new("alice"));
        let auth = AuthMode::Bearer { tokens };
        let blob = b"\0bearer\0tk";
        let s = authenticate_corba(&auth, Some(blob), None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn corba_malformed_service_context_rejected() {
        let auth = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let blob = b"\0badscheme\0xx";
        let err = authenticate_corba(&auth, Some(blob), None).unwrap_err();
        assert!(matches!(err, AuthError::MalformedCredentials(_)));
    }

    #[test]
    fn corba_mtls_subject_pass_through() {
        let auth = AuthMode::Mtls;
        let s = authenticate_corba(&auth, None, Some(AuthSubject::new("CN=cli"))).unwrap();
        assert_eq!(s.name, "CN=cli");
    }

    #[test]
    fn corba_acl_topic_check() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "DdsTopic::Foo".into(),
            AclEntry {
                read: vec!["alice".into()],
                write: vec!["*".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let alice = AuthSubject::new("alice");
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "DdsTopic::Foo"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Read, "DdsTopic::Foo"));
        assert!(authorize(&ctx.acl, &bob, AclOp::Write, "DdsTopic::Foo"));
    }

    #[test]
    fn corba_none_yields_anonymous() {
        let s = authenticate_corba(&AuthMode::None, None, None).unwrap();
        assert_eq!(s.name, "anonymous");
    }
}
