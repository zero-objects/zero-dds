// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA-DDS-Bridge §7.x Bridge-Security-Wireup.
//!
//! * §7.1 TLS — SSLIOP TaggedComponent 0x06 via rustls (CORBA-Profile-
//!   Body trägt die TLS-Listen-Address).
//! * §7.2 Auth — entweder über CSIv2-Token (siehe
//!   [`crate::csiv2_wire`]) ODER mTLS-Client-Cert. Für RC1 nutzen wir
//!   den `Authorization: Bearer …`-Pfad als Service-Context-Mapping
//!   (CORBA Service-Context ID = 256, Vendor-extension).
//! * §7.3 Topic-ACL — pro DDS-Topic-Bridge-Mapping.
//!
//! Spec: `zerodds-corba-bridge-1.0.md` §7.

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityCtx,
    SecurityError, authorize, build_ctx, extract_mtls_subject,
};

/// CORBA-spezifischer Auth-Hook: extrahiert das Bearer-Token aus
/// einem GIOP-Service-Context (`context_id = 256`, Vendor-Extension)
/// oder reicht das mTLS-Subject aus dem TLS-Layer durch.
///
/// `service_context_token` ist der byte-block aus
/// `IOP::ServiceContext::context_data` (Vendor-Schema:
/// `\0bearer\0<utf8 token>`). Bei `mTLS`-Mode wird stattdessen
/// das `mtls_subject` aus dem rustls-Peer-Cert genommen.
///
/// # Errors
/// [`AuthError`] bei missing/malformed/rejected.
pub fn authenticate_corba(
    auth: &AuthMode,
    service_context_token: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let header = match service_context_token {
        Some(b) => {
            // Erwartetes Format: `\0bearer\0<token>` (Vendor-Schema).
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
