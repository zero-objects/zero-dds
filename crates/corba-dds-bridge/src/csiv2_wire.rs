// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CSIv2 (Common Secure Interoperability v2) wire hooks for the bridge
//! daemon — CORBA 3.4 Part 2 §10 / §24.
//!
//! Two-stage authentication:
//! 1. **Transport (SSLIOP)** — `bridge_security`/`daemon_runtime`'s
//!    rustls wire-up, independent of this module.
//! 2. **SAS `EstablishContext` per request** (§10.3: a stateless
//!    target re-evaluates every request rather than retaining a
//!    context — see `bridge_daemon::csiv2_mech_list`'s `stateful:
//!    false`) — the `SecurityAttributeService` GIOP service context
//!    (id 15, §24.7.2), evaluated by
//!    `zerodds_corba_csiv2::authenticate_gssup` against the daemon's
//!    `GssupUserStore`.
//!
//! This module is the glue between `corba-giop`'s `ServiceContextList`
//! and `corba-csiv2`'s `authenticate_gssup`; the actual protocol and
//! credential logic lives in `corba-csiv2` (shared with
//! `zerodds-naming-svc`, which offers the same CSIv2 gate over its own
//! transport).

use alloc::string::String;

use zerodds_corba_csiv2::{GssupUserStore, SasOutcome, authenticate_gssup};
use zerodds_corba_giop::{ServiceContext, ServiceContextList, ServiceContextTag};
use zerodds_wire::Endianness;

/// `service_context_id` for SAS — CORBA 3.4 Part 2 §24.7.2.
#[must_use]
pub fn security_attribute_service_id() -> u32 {
    ServiceContextTag::SecurityAttributeService.as_u32()
}

/// What evaluating one request's service contexts against CSIv2 found.
#[derive(Debug)]
pub enum Csiv2Outcome {
    /// No `SecurityAttributeService` context was present. The caller
    /// decides whether another identity (an SSLIOP client certificate)
    /// already satisfies its ACL.
    NotOffered,
    /// Valid GSSUP credentials (or a supported identity assertion).
    Accepted {
        /// Effective principal for ACL purposes.
        principal: String,
        /// SAS reply to attach to the GIOP reply's service context.
        reply: zerodds_corba_csiv2::sas::SasMessage,
    },
    /// The context did not decode, the credentials were wrong, or an
    /// asserted identity's form is not supported. The caller sends
    /// `NO_PERMISSION` and attaches `reply` (a `ContextError`).
    Rejected {
        /// SAS `ContextError` to attach to the GIOP reply.
        reply: zerodds_corba_csiv2::sas::SasMessage,
    },
}

/// Evaluates a request's `service_context` against `store`.
#[must_use]
pub fn evaluate(
    service_context: &ServiceContextList,
    store: &GssupUserStore,
    target_supports: u32,
) -> Csiv2Outcome {
    let sas_id = security_attribute_service_id();
    let Some(sc) = service_context.0.iter().find(|c| c.context_id == sas_id) else {
        return Csiv2Outcome::NotOffered;
    };
    match authenticate_gssup(&sc.context_data, store, target_supports) {
        SasOutcome::Accepted {
            principal, reply, ..
        } => Csiv2Outcome::Accepted { principal, reply },
        SasOutcome::Rejected { reply } => Csiv2Outcome::Rejected { reply },
        // `authenticate_gssup` only returns this for an empty input,
        // which `sc.context_data` is not (it decoded from the wire).
        SasOutcome::NoContext => Csiv2Outcome::NotOffered,
    }
}

/// Encodes a SAS reply as the `ServiceContext` a GIOP `Reply` carries it
/// in, if it encodes at all (a CDR overflow is the only failure mode
/// and never happens for these small, fixed-shape messages in
/// practice; a caller that cannot attach it still sends the reply
/// without the context rather than dropping the reply itself).
#[must_use]
pub fn service_context_for(
    reply: &zerodds_corba_csiv2::sas::SasMessage,
    endianness: Endianness,
) -> Option<ServiceContext> {
    reply
        .encode_encapsulation(endianness)
        .ok()
        .map(|bytes| ServiceContext::new(security_attribute_service_id(), bytes))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_corba_csiv2::gssup::GssupCredentialToken;
    use zerodds_corba_csiv2::gssup_store::format_entry;
    use zerodds_corba_csiv2::sas::{EstablishContext, IdentityToken, SasMessage};

    fn store() -> GssupUserStore {
        GssupUserStore::parse(&format_entry("alice", "secret", b"s", 1000)).unwrap()
    }

    fn context_list(username: &str, password: &str) -> ServiceContextList {
        let gssup = GssupCredentialToken::new(username.into(), password.into(), Vec::new())
            .to_gss_token(Endianness::Big)
            .unwrap();
        let sas = SasMessage::EstablishContext(EstablishContext {
            client_context_id: 0,
            authorization_token: Vec::new(),
            identity_token: IdentityToken::Absent,
            client_authentication_token: gssup,
        })
        .encode_encapsulation(Endianness::Big)
        .unwrap();
        let mut list = ServiceContextList::default();
        list.0
            .push(ServiceContext::new(security_attribute_service_id(), sas));
        list
    }

    #[test]
    fn id_constant_is_fifteen_per_spec() {
        // CORBA 3.4 Part 2 §24.7.2 — SecurityAttributeService = 15.
        assert_eq!(security_attribute_service_id(), 15);
    }

    #[test]
    fn valid_credentials_are_accepted_with_the_username_as_principal() {
        let outcome = evaluate(&context_list("alice", "secret"), &store(), 0);
        match outcome {
            Csiv2Outcome::Accepted { principal, .. } => assert_eq!(principal, "alice"),
            other => panic!("expected Accepted, got {other:?}"),
        }
    }

    #[test]
    fn wrong_password_is_rejected() {
        let outcome = evaluate(&context_list("alice", "wrong"), &store(), 0);
        assert!(matches!(outcome, Csiv2Outcome::Rejected { .. }));
    }

    #[test]
    fn no_context_at_all_is_not_offered() {
        let outcome = evaluate(&ServiceContextList::default(), &store(), 0);
        assert!(matches!(outcome, Csiv2Outcome::NotOffered));
    }

    #[test]
    fn a_context_of_a_different_id_is_ignored() {
        let mut list = ServiceContextList::default();
        list.0.push(ServiceContext::new(999, vec![0; 4]));
        let outcome = evaluate(&list, &store(), 0);
        assert!(matches!(outcome, Csiv2Outcome::NotOffered));
    }

    #[test]
    fn service_context_for_round_trips_through_the_reply() {
        let outcome = evaluate(&context_list("alice", "secret"), &store(), 0);
        let Csiv2Outcome::Accepted { reply, .. } = outcome else {
            panic!("expected Accepted");
        };
        let sc = service_context_for(&reply, Endianness::Big).expect("encodes");
        assert_eq!(sc.context_id, security_attribute_service_id());
        assert!(!sc.context_data.is_empty());
    }
}
