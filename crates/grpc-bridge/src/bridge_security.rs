// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC bridge §7.x bridge-security wireup.
//!
//! * §7.1 TLS — HTTP/2 ALPN-`h2` via rustls (standard for gRPC).
//! * §7.2 Auth — `Authorization: Bearer <jwt>` header (gRPC metadata),
//!   alternatively `mtls`.
//! * §7.3 Topic ACL — gRPC path = `"/<package>.<service>/<method>"`;
//!   we map method = DDS topic name, ACL check per call.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §7.

pub use zerodds_bridge_security::{
    Acl, AclEntry, AclOp, AuthError, AuthMode, AuthSubject, SecurityConfig, SecurityCtx,
    SecurityError, authorize, build_ctx, extract_mtls_subject,
};

/// gRPC-specific auth hook: extracts the `authorization` header
/// from the HTTP/2 metadata (HEADERS frame of the stream initiation).
///
/// `metadata` is the map of `[(name, value)]` pairs as they come from
/// the decoded HPACK block. We search (case-insensitive) for
/// `authorization`.
///
/// # Errors
/// [`AuthError`] on missing/malformed/rejected.
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

/// gRPC-specific ACL check: the gRPC path "/svc.X/method" is mapped
/// to "method" (which is the DDS topic identity per spec).
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
