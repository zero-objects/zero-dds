// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC-Bridge §7.x Bridge-Security-Wireup.
//!
//! * §7.1 TLS — HTTP/2 ALPN-`h2` via rustls (Standard für gRPC).
//! * §7.2 Auth — `Authorization: Bearer <jwt>` Header (gRPC-Metadata),
//!   alternativ `mtls`.
//! * §7.3 Topic-ACL — gRPC-Path = `"/<package>.<service>/<method>"`;
//!   wir mappen Method = DDS-Topic-Name, ACL-Check pro Call.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §7.

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityCtx,
    SecurityError, authorize, build_ctx, extract_mtls_subject,
};

/// gRPC-spezifischer Auth-Hook: extrahiert `authorization`-Header
/// aus den HTTP/2 Metadata (HEADERS-Frame der Stream-Initiation).
///
/// `metadata` ist die Map aus `[(name, value)]`-Pairs wie sie aus
/// dem dekodierten HPACK-Block kommen. Wir suchen
/// (case-insensitive) nach `authorization`.
///
/// # Errors
/// [`AuthError`] bei missing/malformed/rejected.
pub fn authenticate_grpc(
    auth: &AuthMode,
    metadata: &[(String, String)],
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let auth_header = metadata
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str());
    zerodds_bridge_security::authenticate(auth, auth_header, None, mtls_subject)
}

/// gRPC-spezifischer ACL-Check: gRPC-Path "/svc.X/method" wird auf
/// "method" gemapped (das ist die DDS-Topic-Identität nach Spec).
#[must_use]
pub fn authorize_grpc(acl: &Acl, subject: &AuthSubject, op: AclOp, grpc_path: &str) -> bool {
    let topic = grpc_path.rsplit('/').next().unwrap_or(grpc_path);
    authorize(acl, subject, op, topic)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn grpc_bearer_in_metadata() {
        let mut tokens = HashMap::new();
        tokens.insert("tk".into(), AuthSubject::new("alice"));
        let auth = AuthMode::Bearer { tokens };
        let md = vec![
            (":method".to_string(), "POST".to_string()),
            ("authorization".to_string(), "Bearer tk".to_string()),
        ];
        let s = authenticate_grpc(&auth, &md, None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn grpc_missing_metadata_authorization_rejected() {
        let auth = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = authenticate_grpc(&auth, &[], None).unwrap_err();
        assert!(matches!(err, AuthError::MissingCredentials));
    }

    #[test]
    fn grpc_path_mapped_to_topic_for_acl() {
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
        assert!(authorize_grpc(
            &ctx.acl,
            &alice,
            AclOp::Read,
            "/com.zerodds.svc/Trade"
        ));
        assert!(!authorize_grpc(
            &ctx.acl,
            &alice,
            AclOp::Read,
            "/com.zerodds.svc/Other"
        ));
    }

    #[test]
    fn grpc_none_yields_anonymous() {
        let s = authenticate_grpc(&AuthMode::None, &[], None).unwrap();
        assert_eq!(s.name, "anonymous");
    }
}
