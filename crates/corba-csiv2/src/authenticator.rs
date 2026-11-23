// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Per-request GSSUP authentication over the SAS state machine — the
//! glue between [`crate::session::SasTarget`] (which knows the protocol
//! but nothing about credentials) and [`crate::gssup_store::GssupUserStore`]
//! (which knows credentials but nothing about SAS messages).
//!
//! `docs/doc-map`'s recorded decision for this project's CORBA daemons
//! is a **stateless** target — every request carries its own
//! `EstablishContext` rather than reusing a retained context via
//! `MessageInContext` (CORBA 3.4 Part 2 §10.3 leaves the choice to the
//! target; §10.3's own text notes a stateless target "may choose to
//! authenticate every message", which is the simpler contract for a
//! daemon that does not track per-connection SAS state across the GIOP
//! connection pool a client may rebuild at any time).
//!
//! A caller (a GIOP request handler) extracts the raw
//! `IOP::ServiceContext` body for `SecurityAttributeService` (id 15,
//! `crate::sas` and every consumer's own `service_context.rs` agree on
//! the id) from the request it already decoded — this module takes
//! those bytes and nothing wire-format-specific from a caller's GIOP
//! library, so it stays usable from both `corba-dds-bridge` and
//! `zerodds-corba-cosnaming`'s daemon without either crate depending on
//! `corba-giop` from here.

extern crate alloc;

use alloc::string::String;

use crate::gssup::GssupCredentialToken;
use crate::gssup_store::GssupUserStore;
use crate::identity::{AssertedIdentity, AssertionTrust, IdentityError, ScopedUsername};
use crate::sas::{IdentityToken, SasMessage};
use crate::session::{
    CONTEXT_ERROR_INVALID_EVIDENCE, CONTEXT_ERROR_INVALID_MECHANISM, CONTEXT_ERROR_NO_CONTEXT,
    SasTarget, TargetAction,
};

/// Bit for [`IdentityToken::Anonymous`] in an `identity_types` mask.
/// Spec §24.2.5's `ITT*` discriminators are already powers of 2
/// (`identity_token_discriminator_matches_spec`, `crate::sas`'s tests),
/// so this mask is [`IdentityToken::discriminator`]'s own values OR'd
/// together — not a further bit position.
pub const IDENTITY_TYPE_ANONYMOUS: u32 = 1;
/// Bit for [`IdentityToken::PrincipalName`].
pub const IDENTITY_TYPE_PRINCIPAL_NAME: u32 = 2;
/// Bit for [`IdentityToken::X509CertChain`].
pub const IDENTITY_TYPE_X509_CERT_CHAIN: u32 = 4;
/// Bit for [`IdentityToken::DistinguishedName`].
pub const IDENTITY_TYPE_DISTINGUISHED_NAME: u32 = 8;

/// Outcome of evaluating one request's SAS context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SasOutcome {
    /// No `SecurityAttributeService` context was present at all — CSIv2
    /// is offered, not required by this evaluator; the caller decides
    /// whether another identity (an SSLIOP client certificate) already
    /// satisfies its ACL, or whether to refuse outright.
    NoContext,
    /// The GSSUP credentials validated (and, if an identity was
    /// asserted, `target_supports_identity_types` allowed its form).
    Accepted {
        /// The GIOP `IOP::ServiceContext` body to attach to the reply.
        reply: SasMessage,
        /// The effective principal for ACL purposes: the GSSUP username,
        /// or the asserted identity's text when one was presented and
        /// accepted.
        principal: String,
        /// `true` when `principal` came from an asserted `IdentityToken`
        /// rather than from the GSSUP username itself.
        asserted: bool,
    },
    /// The context did not decode, the credentials did not validate, or
    /// an asserted identity's form is not in `target_supports_identity_types`.
    Rejected {
        /// The GIOP `IOP::ServiceContext` body to attach to the reply
        /// (a `ContextError`).
        reply: SasMessage,
    },
}

fn refuse(client_context_id: u64, status: (u32, u32)) -> SasMessage {
    let (major_status, minor_status) = status;
    SasMessage::ContextError(crate::sas::ContextError {
        client_context_id,
        major_status,
        minor_status,
        error_token: alloc::vec::Vec::new(),
    })
}

/// Evaluates one request's raw `SecurityAttributeService` context bytes
/// against `store`, honoring `target_supports_identity_types` (an
/// `IDENTITY_TYPE_*` bitmask — configuration's `corba.csiv2.target_supports`)
/// for identity assertion.
///
/// The rejections follow [`authenticate_gssup_with`]: a
/// `SASContextBody` that does not decode, a GSSUP token that does not
/// decode, a username/password that does not authenticate, malformed
/// identity octets and an identity the user may not assert are
/// [`CONTEXT_ERROR_INVALID_EVIDENCE`] (1, 1), the status JacORB 3.9
/// answers its own equivalent rejections with (see `crate::session`'s
/// doc block on the constant); an identity type outside
/// `target_supports_identity_types`, a principal name of another
/// mechanism than GSSUP and an identity extension are
/// [`CONTEXT_ERROR_INVALID_MECHANISM`] (2, 1) (§10.2.5, Table 10.10).
///
/// An identity assertion is evaluated with [`AssertionTrust::new`]: the
/// authenticated user may assert exactly itself and the anonymous
/// identity; [`authenticate_gssup_with`] takes the target's trust rules.
#[must_use]
pub fn authenticate_gssup(
    sas_context_data: &[u8],
    store: &GssupUserStore,
    target_supports_identity_types: u32,
) -> SasOutcome {
    authenticate_gssup_with(
        sas_context_data,
        store,
        target_supports_identity_types,
        &AssertionTrust::new(),
    )
}

/// [`authenticate_gssup`] with the target's rules for backward trust
/// evaluation (CORBA 3.3 Part 2 §10.3.1.1): an identity token is
/// accepted when its type is in `target_supports_identity_types`, its
/// octets name an identity (a GSSUP exported name for
/// `ITTPrincipalName`, directly or in an encapsulation, §10.2.5) and
/// `trust` lets the authenticated GSSUP user assert it. A type the
/// target does not support, a principal name of another mechanism and
/// an identity extension are `ContextError` invalid mechanism (2, 1)
/// (§10.2.5, Table 10.10); malformed identity octets and an identity
/// the user is not trusted to assert are invalid evidence (1, 1).
#[must_use]
pub fn authenticate_gssup_with(
    sas_context_data: &[u8],
    store: &GssupUserStore,
    target_supports_identity_types: u32,
    trust: &AssertionTrust,
) -> SasOutcome {
    authenticate_with_verifier(
        sas_context_data,
        &|username, password| store.verify(username, password),
        target_supports_identity_types,
        trust,
    )
}

/// [`authenticate_gssup_with`] for a target whose GSSUP credentials are
/// checked by `verify(username, password)` instead of a
/// [`GssupUserStore`]: the same decoding, the same identity assertion
/// with backward trust evaluation and the same `ContextError` statuses.
#[must_use]
pub fn authenticate_with_verifier(
    sas_context_data: &[u8],
    verify: &dyn Fn(&str, &str) -> bool,
    target_supports_identity_types: u32,
    trust: &AssertionTrust,
) -> SasOutcome {
    let Ok(msg) = SasMessage::decode_encapsulation(sas_context_data) else {
        // JacORB 3.9 measured: "Could not parse service context" -> (1, 1).
        return SasOutcome::Rejected {
            reply: refuse(0, CONTEXT_ERROR_INVALID_EVIDENCE),
        };
    };
    let mut target = SasTarget::stateless();
    let ec = match &msg {
        SasMessage::EstablishContext(ec) => ec,
        _ => {
            // A stateless target retains nothing, so a MessageInContext
            // is always "no context" and the other two message types
            // never legitimately arrive at a target at all —
            // `SasTarget::accept` already reports the right one of the
            // two; reuse it rather than re-deriving the status here.
            let TargetAction::Refuse(reply) = target.accept(&msg) else {
                return SasOutcome::Rejected {
                    reply: refuse(0, CONTEXT_ERROR_INVALID_EVIDENCE),
                };
            };
            return SasOutcome::Rejected { reply };
        }
    };
    let TargetAction::Accept(complete) = target.accept(&msg) else {
        return SasOutcome::Rejected {
            reply: refuse(ec.client_context_id, CONTEXT_ERROR_NO_CONTEXT),
        };
    };

    let token = GssupCredentialToken::from_gss_token(&ec.client_authentication_token)
        .or_else(|_| GssupCredentialToken::decode_encapsulation(&ec.client_authentication_token));
    let Ok(token) = token else {
        return SasOutcome::Rejected {
            reply: refuse(ec.client_context_id, CONTEXT_ERROR_INVALID_EVIDENCE),
        };
    };
    // JacORB 3.9 measured: "Could not validate context EstablishContext" -> (1, 1).
    if !verify(&token.username, &token.password) {
        return SasOutcome::Rejected {
            reply: refuse(ec.client_context_id, CONTEXT_ERROR_INVALID_EVIDENCE),
        };
    }

    if matches!(ec.identity_token, IdentityToken::Absent) {
        return SasOutcome::Accepted {
            reply: complete,
            principal: token.username,
            asserted: false,
        };
    }
    let rejected = |status| SasOutcome::Rejected {
        reply: refuse(ec.client_context_id, status),
    };
    if ec.identity_token.discriminator() & target_supports_identity_types == 0 {
        return rejected(CONTEXT_ERROR_INVALID_MECHANISM);
    }
    let asserted = match AssertedIdentity::from_token(&ec.identity_token) {
        Ok(Some(a)) => a,
        Ok(None) | Err(IdentityError::Malformed(_)) => {
            return rejected(CONTEXT_ERROR_INVALID_EVIDENCE);
        }
        Err(IdentityError::UnsupportedMechanism(_)) => {
            return rejected(CONTEXT_ERROR_INVALID_MECHANISM);
        }
    };
    // §10.2.5.2: the GSSUP user name is a scoped-username.
    let Ok(proxy) = ScopedUsername::parse(&token.username) else {
        return rejected(CONTEXT_ERROR_INVALID_EVIDENCE);
    };
    if !trust.trusts(&proxy, &asserted) {
        return rejected(CONTEXT_ERROR_INVALID_EVIDENCE);
    }
    SasOutcome::Accepted {
        reply: complete,
        principal: asserted.principal_text(),
        asserted: true,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::gssup_store::format_entry;
    use crate::sas::EstablishContext;
    use zerodds_wire::Endianness;

    fn store() -> GssupUserStore {
        let line = format_entry("alice", "secret", b"salt", 1000);
        GssupUserStore::parse(&line).unwrap()
    }

    fn establish_bytes(username: &str, password: &str, identity: IdentityToken) -> Vec<u8> {
        let gssup = GssupCredentialToken::new(username.into(), password.into(), Vec::new())
            .to_gss_token(Endianness::Big)
            .unwrap();
        SasMessage::EstablishContext(EstablishContext {
            client_context_id: 0,
            authorization_token: Vec::new(),
            identity_token: identity,
            client_authentication_token: gssup,
        })
        .encode_encapsulation(Endianness::Big)
        .unwrap()
    }

    #[test]
    fn valid_credentials_with_no_identity_assertion_are_accepted() {
        let bytes = establish_bytes("alice", "secret", IdentityToken::Absent);
        let outcome = authenticate_gssup(&bytes, &store(), 0);
        match outcome {
            SasOutcome::Accepted {
                principal,
                asserted,
                ..
            } => {
                assert_eq!(principal, "alice");
                assert!(!asserted);
            }
            other => panic!("expected Accepted, got {other:?}"),
        }
    }

    #[test]
    fn wrong_password_is_rejected_with_invalid_evidence() {
        let bytes = establish_bytes("alice", "wrong", IdentityToken::Absent);
        let outcome = authenticate_gssup(&bytes, &store(), 0);
        match outcome {
            SasOutcome::Rejected {
                reply: SasMessage::ContextError(e),
            } => assert_eq!(
                (e.major_status, e.minor_status),
                CONTEXT_ERROR_INVALID_EVIDENCE
            ),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn unknown_user_is_rejected() {
        let bytes = establish_bytes("mallory", "whatever", IdentityToken::Absent);
        assert!(matches!(
            authenticate_gssup(&bytes, &store(), 0),
            SasOutcome::Rejected { .. }
        ));
    }

    fn principal(text: &str) -> IdentityToken {
        IdentityToken::PrincipalName(crate::identity::exported_gssup_name(
            &ScopedUsername::parse(text).unwrap(),
        ))
    }

    fn status(outcome: SasOutcome) -> (u32, u32) {
        match outcome {
            SasOutcome::Rejected {
                reply: SasMessage::ContextError(e),
            } => (e.major_status, e.minor_status),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn corba_p2_10_3_1_1_an_identity_the_user_is_not_trusted_to_assert_is_invalid_evidence() {
        // No rule: alice may assert alice and anonymous, nobody else.
        let other = establish_bytes("alice", "secret", principal("end-user@REALM"));
        assert_eq!(
            status(authenticate_gssup(
                &other,
                &store(),
                IDENTITY_TYPE_PRINCIPAL_NAME
            )),
            CONTEXT_ERROR_INVALID_EVIDENCE
        );
        let itself = establish_bytes("alice", "secret", principal("alice"));
        assert!(matches!(
            authenticate_gssup(&itself, &store(), IDENTITY_TYPE_PRINCIPAL_NAME),
            SasOutcome::Accepted { asserted: true, .. }
        ));
        let anonymous = establish_bytes("alice", "secret", IdentityToken::Anonymous);
        match authenticate_gssup(&anonymous, &store(), IDENTITY_TYPE_ANONYMOUS) {
            SasOutcome::Accepted { principal, .. } => assert_eq!(principal, "anonymous"),
            other => panic!("expected Accepted, got {other:?}"),
        }
        // Raw bytes that are no exported name are malformed evidence.
        let raw = establish_bytes(
            "alice",
            "secret",
            IdentityToken::PrincipalName(b"end-user@REALM".to_vec()),
        );
        assert_eq!(
            status(authenticate_gssup(
                &raw,
                &store(),
                IDENTITY_TYPE_PRINCIPAL_NAME
            )),
            CONTEXT_ERROR_INVALID_EVIDENCE
        );
    }

    #[test]
    fn corba_p2_10_2_5_a_principal_name_of_another_mechanism_is_invalid_mechanism() {
        let mut kerberos = alloc::vec![0x04, 0x01, 0x00, 0x0b];
        kerberos.extend_from_slice(&[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x12, 1, 2, 2]);
        kerberos.extend_from_slice(&[0, 0, 0, 1, b'k']);
        let bytes = establish_bytes("alice", "secret", IdentityToken::PrincipalName(kerberos));
        assert_eq!(
            status(authenticate_gssup(
                &bytes,
                &store(),
                IDENTITY_TYPE_PRINCIPAL_NAME
            )),
            CONTEXT_ERROR_INVALID_MECHANISM
        );
    }

    #[test]
    fn a_supported_identity_assertion_is_accepted_as_the_asserted_principal() {
        let bytes = establish_bytes("alice", "secret", principal("end-user@REALM"));
        let mut trust = AssertionTrust::new();
        trust.allow(
            ScopedUsername::parse("alice").unwrap(),
            AssertedIdentity::GssupPrincipal(ScopedUsername::parse("end-user@REALM").unwrap()),
        );
        let outcome =
            authenticate_gssup_with(&bytes, &store(), IDENTITY_TYPE_PRINCIPAL_NAME, &trust);
        match outcome {
            SasOutcome::Accepted {
                principal,
                asserted,
                ..
            } => {
                assert_eq!(principal, "end-user@REALM");
                assert!(asserted);
            }
            other => panic!("expected Accepted, got {other:?}"),
        }
    }

    #[test]
    fn an_unsupported_identity_assertion_form_is_rejected() {
        let bytes = establish_bytes(
            "alice",
            "secret",
            IdentityToken::X509CertChain(b"der-bytes".to_vec()),
        );
        // target_supports only PrincipalName, not X509CertChain.
        let outcome = authenticate_gssup(&bytes, &store(), IDENTITY_TYPE_PRINCIPAL_NAME);
        assert!(matches!(outcome, SasOutcome::Rejected { .. }));
    }

    #[test]
    fn a_malformed_context_is_rejected_with_invalid_evidence_not_panicking() {
        // JacORB 3.9 measured: "Could not parse service context" -> (1, 1).
        let outcome = authenticate_gssup(&[0xff, 0xff], &store(), 0);
        match outcome {
            SasOutcome::Rejected {
                reply: SasMessage::ContextError(e),
            } => assert_eq!(
                (e.major_status, e.minor_status),
                CONTEXT_ERROR_INVALID_EVIDENCE
            ),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn an_undecodable_gssup_token_is_rejected_with_invalid_evidence() {
        // A well-formed EstablishContext whose client_authentication_token
        // is neither a GSS-wrapped nor a raw GSSUP encapsulation.
        let msg = SasMessage::EstablishContext(EstablishContext {
            client_context_id: 0,
            authorization_token: Vec::new(),
            identity_token: IdentityToken::Absent,
            client_authentication_token: alloc::vec![0xff, 0xff, 0xff],
        })
        .encode_encapsulation(Endianness::Big)
        .unwrap();
        let outcome = authenticate_gssup(&msg, &store(), 0);
        match outcome {
            SasOutcome::Rejected {
                reply: SasMessage::ContextError(e),
            } => assert_eq!(
                (e.major_status, e.minor_status),
                CONTEXT_ERROR_INVALID_EVIDENCE
            ),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn corba_p2_10_2_5_an_identity_type_the_target_does_not_support_is_invalid_mechanism() {
        let bytes = establish_bytes(
            "alice",
            "secret",
            IdentityToken::X509CertChain(b"der-bytes".to_vec()),
        );
        let outcome = authenticate_gssup(&bytes, &store(), IDENTITY_TYPE_PRINCIPAL_NAME);
        assert_eq!(status(outcome), CONTEXT_ERROR_INVALID_MECHANISM);
    }

    #[test]
    fn a_message_in_context_at_a_stateless_target_is_rejected_as_no_context() {
        let msg = SasMessage::MessageInContext(crate::sas::MessageInContext {
            client_context_id: 5,
            discard_context: false,
        });
        let bytes = msg.encode_encapsulation(Endianness::Big).unwrap();
        let outcome = authenticate_gssup(&bytes, &store(), 0);
        match outcome {
            SasOutcome::Rejected {
                reply: SasMessage::ContextError(e),
            } => assert_eq!((e.major_status, e.minor_status), CONTEXT_ERROR_NO_CONTEXT),
            other => panic!("expected Rejected/NO_CONTEXT, got {other:?}"),
        }
    }

    #[test]
    fn the_jacorb_gss_wrapped_form_and_the_raw_encapsulation_both_authenticate() {
        // Cross-ORB: a foreign client's client_authentication_token is
        // GSS-wrapped (to_gss_token); this project's own client helper
        // may instead send the bare GSSUP encapsulation. Both decode.
        let raw = GssupCredentialToken::new("alice".into(), "secret".into(), Vec::new())
            .encode_encapsulation(Endianness::Big)
            .unwrap();
        let msg = SasMessage::EstablishContext(EstablishContext {
            client_context_id: 0,
            authorization_token: Vec::new(),
            identity_token: IdentityToken::Absent,
            client_authentication_token: raw,
        })
        .encode_encapsulation(Endianness::Big)
        .unwrap();
        assert!(matches!(
            authenticate_gssup(&msg, &store(), 0),
            SasOutcome::Accepted { .. }
        ));
    }
}
