// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SAS context state machine — spec CORBA 3.3 Part 2 §10.3.
//!
//! §10.2 defines the four `SASContextBody` messages and their octets;
//! this module is the other half of §10.3: what a client and a target do
//! with them across a sequence of requests.
//!
//! A target answers an `EstablishContext` with a
//! `CompleteEstablishContext` whose `context_stateful` says whether it
//! retained the security context:
//!
//! * **stateful** — the target keeps the context under the client's
//!   `ContextId`. Every later request carries a `MessageInContext`
//!   naming that id instead of re-sending the credentials, until one of
//!   them sets `discard_context`.
//! * **stateless** — the target keeps nothing. `context_stateful` is
//!   false and the client re-establishes on every request.
//!
//! A `MessageInContext` that names a context the target does not hold —
//! never established, already discarded, or established against a
//! different target — is refused with a `ContextError`, which puts the
//! client back into the establish state.
//!
//! Neither half touches the wire: both take and return [`SasMessage`],
//! so the octets stay the ones `sas.rs` writes.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::sas::{
    CompleteEstablishContext, ContextError, EstablishContext, IdentityToken, MessageInContext,
    SasMessage,
};

/// The CSIv2 `ContextError.major_status` values these constants
/// implement (`minor_status` is 1 for every one of them here — CSIv2
/// leaves `minor_status` mechanism defined and GSSUP's own convention,
/// per this codebase's earlier reading, is a flat 1 rather than a
/// finer-grained code).
///
/// **Measured**, this round (2026-09-24), against JacORB 3.9's
/// `org.jacorb.security.sas.SASTargetInterceptor` (`javap -c` on the
/// class on the Linux host, reading the literal arguments passed to its
/// `makeContextError` calls): the rejection logged as `"Could not parse
/// service context"` and the one logged as `"Could not validate context
/// EstablishContext"` both build `(1, 1)`; "several other rejection
/// paths" in that interceptor also build `(1, 1)`; a distinct,
/// unidentified subset of its rejection paths builds `(2, 1)` — *which*
/// condition maps to `2` was not identified this round, only that the
/// value exists in JacORB's own output.
///
/// **Not verified from spec text this round**: the OMG CSIv2 / CORBA
/// Security Service `ContextError` values table itself was not read in
/// this session — the shape below (`1`=invalid evidence, `4`=no
/// context) is carried over from this crate's own earlier reading of
/// it (not re-checked now) plus JacORB's measured agreement on `1`.
/// `CONTEXT_ERROR_INVALID_MECHANISM = (2, 1)` is defined from that same
/// earlier reading; nothing in this crate emits it yet (see its own
/// doc). A third table entry some readings call "conflicting evidence"
/// is not represented here at all — neither this round's JacORB
/// measurement nor this crate's earlier one attached a confirmed value
/// or a caller to it, and guessing one would be exactly the kind of
/// unmeasured number this project's own discipline refuses to ship.
///
/// `<major_status, minor_status>` a target reports when a
/// `MessageInContext` names a context it does not hold. The client's
/// answer to it is to establish the context again.
pub const CONTEXT_ERROR_NO_CONTEXT: (u32, u32) = (4, 1);

/// `<major_status, minor_status>` a target reports when the evidence it
/// was asked to accept is not acceptable: a message it cannot evaluate
/// as evidence at all (a `CompleteEstablishContext` or a `ContextError`
/// arriving at a target — both travel target-to-client and never
/// legitimately arrive at one), a `SASContextBody` that does not decode
/// (JacORB 3.9's measured `"Could not parse service context"`), a GSSUP
/// username/password that does not authenticate (JacORB 3.9's measured
/// `"Could not validate context EstablishContext"`), or an
/// `IdentityToken` of a form `target_supports` does not list. JacORB
/// 3.9 answers every one of these with `(1, 1)` (measured this round,
/// see the block above); this module and `crate::authenticator` do the
/// same.
pub const CONTEXT_ERROR_INVALID_EVIDENCE: (u32, u32) = (1, 1);

/// `<major_status, minor_status>` for a client authentication token
/// this target's one supported mechanism (GSSUP) cannot parse at all —
/// as opposed to one that parses as GSSUP and fails on its own terms
/// (wrong password: [`CONTEXT_ERROR_INVALID_EVIDENCE`]). JacORB 3.9
/// builds `(2, 1)` for some rejection path this round's measurement did
/// not identify by name; this crate is not confident enough in the
/// mapping to emit it from `authenticate_gssup` yet — see that
/// function's own doc for which of its rejections use which constant,
/// and this module's own doc block above for what was and was not
/// checked.
pub const CONTEXT_ERROR_INVALID_MECHANISM: (u32, u32) = (2, 1);

/// What the target does with one incoming SAS message — §10.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAction {
    /// The request is accepted and the reply carries this SAS message.
    Accept(SasMessage),
    /// The request is accepted and the reply carries no SAS service
    /// context — the case of a `MessageInContext` on a live context.
    AcceptSilently,
    /// The request is refused and the reply carries this `ContextError`.
    Refuse(SasMessage),
}

/// One security context a stateful target retains — §10.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedContext {
    /// The client authentication token the context was established with.
    pub client_authentication_token: Vec<u8>,
    /// The asserted identity the context was established with.
    pub identity_token: IdentityToken,
}

/// Target half of the SAS state machine — §10.3.
///
/// A target is either stateful or stateless for the whole of its life;
/// the spec lets a target decide per context, and this type models the
/// decision as one flag because a target that answers differently to
/// two identical clients has no observable reason to.
#[derive(Debug, Clone)]
pub struct SasTarget {
    stateful: bool,
    contexts: BTreeMap<u64, RetainedContext>,
}

impl SasTarget {
    /// A target that retains the contexts it establishes.
    #[must_use]
    pub const fn stateful() -> Self {
        Self {
            stateful: true,
            contexts: BTreeMap::new(),
        }
    }

    /// A target that retains nothing; every request re-establishes.
    #[must_use]
    pub const fn stateless() -> Self {
        Self {
            stateful: false,
            contexts: BTreeMap::new(),
        }
    }

    /// `true` if this target retains established contexts.
    #[must_use]
    pub const fn is_stateful(&self) -> bool {
        self.stateful
    }

    /// Number of contexts currently retained.
    #[must_use]
    pub fn retained_contexts(&self) -> usize {
        self.contexts.len()
    }

    /// The context retained under `client_context_id`, if any.
    #[must_use]
    pub fn context(&self, client_context_id: u64) -> Option<&RetainedContext> {
        self.contexts.get(&client_context_id)
    }

    /// Processes one client-to-target SAS message — §10.3.
    pub fn accept(&mut self, message: &SasMessage) -> TargetAction {
        match message {
            SasMessage::EstablishContext(ec) => TargetAction::Accept(self.establish(ec)),
            SasMessage::MessageInContext(m) => self.in_context(m),
            // A target-to-client message arriving at a target is not
            // evidence the target can evaluate.
            SasMessage::CompleteEstablishContext(c) => {
                Self::refuse(c.client_context_id, CONTEXT_ERROR_INVALID_EVIDENCE)
            }
            SasMessage::ContextError(e) => {
                Self::refuse(e.client_context_id, CONTEXT_ERROR_INVALID_EVIDENCE)
            }
        }
    }

    fn establish(&mut self, ec: &EstablishContext) -> SasMessage {
        if self.stateful {
            self.contexts.insert(
                ec.client_context_id,
                RetainedContext {
                    client_authentication_token: ec.client_authentication_token.clone(),
                    identity_token: ec.identity_token.clone(),
                },
            );
        }
        SasMessage::CompleteEstablishContext(CompleteEstablishContext {
            client_context_id: ec.client_context_id,
            context_stateful: self.stateful,
            final_context_token: Vec::new(),
        })
    }

    fn in_context(&mut self, m: &MessageInContext) -> TargetAction {
        if !self.contexts.contains_key(&m.client_context_id) {
            return Self::refuse(m.client_context_id, CONTEXT_ERROR_NO_CONTEXT);
        }
        if m.discard_context {
            self.contexts.remove(&m.client_context_id);
        }
        TargetAction::AcceptSilently
    }

    fn refuse(client_context_id: u64, status: (u32, u32)) -> TargetAction {
        let (major_status, minor_status) = status;
        TargetAction::Refuse(SasMessage::ContextError(ContextError {
            client_context_id,
            major_status,
            minor_status,
            error_token: Vec::new(),
        }))
    }
}

/// Client half of the SAS state machine — §10.3.
///
/// The client decides per request whether to send credentials again or
/// to name the context the target said it retained. It learns which
/// from the reply, so a target that turns stateless — or loses the
/// context — costs one refused request and no more.
#[derive(Debug, Clone)]
pub struct SasClient {
    next_context_id: u64,
    established: Option<u64>,
    identity_token: IdentityToken,
    client_authentication_token: Vec<u8>,
}

impl SasClient {
    /// A client that presents `identity_token` and
    /// `client_authentication_token` whenever it establishes a context.
    #[must_use]
    pub const fn new(identity_token: IdentityToken, client_authentication_token: Vec<u8>) -> Self {
        Self {
            // §10.2: a `ContextId` of 0 is reserved for the stateless
            // case, so an allocated id starts at 1.
            next_context_id: 1,
            established: None,
            identity_token,
            client_authentication_token,
        }
    }

    /// The context id the client believes the target retains, if any.
    #[must_use]
    pub const fn established_context(&self) -> Option<u64> {
        self.established
    }

    /// The SAS message for the next request — an `EstablishContext`
    /// while no context is held, a `MessageInContext` once one is.
    pub fn next_request(&mut self) -> SasMessage {
        match self.established {
            Some(id) => SasMessage::MessageInContext(MessageInContext {
                client_context_id: id,
                discard_context: false,
            }),
            None => {
                let id = self.next_context_id;
                self.next_context_id += 1;
                SasMessage::EstablishContext(EstablishContext {
                    client_context_id: id,
                    authorization_token: Vec::new(),
                    identity_token: self.identity_token.clone(),
                    client_authentication_token: self.client_authentication_token.clone(),
                })
            }
        }
    }

    /// The message that releases the context the client holds, if any.
    /// After it the client establishes again.
    pub fn discard_request(&mut self) -> Option<SasMessage> {
        let id = self.established.take()?;
        Some(SasMessage::MessageInContext(MessageInContext {
            client_context_id: id,
            discard_context: true,
        }))
    }

    /// Folds a target reply into the client state — §10.3.
    ///
    /// A `CompleteEstablishContext` with `context_stateful` set is the
    /// only thing that gives the client a context to name; a
    /// `ContextError` takes it away again.
    pub fn on_reply(&mut self, reply: &SasMessage) {
        match reply {
            SasMessage::CompleteEstablishContext(c) => {
                self.established = c.context_stateful.then_some(c.client_context_id);
            }
            SasMessage::ContextError(_) => self.established = None,
            SasMessage::EstablishContext(_) | SasMessage::MessageInContext(_) => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    fn client() -> SasClient {
        SasClient::new(
            IdentityToken::PrincipalName(b"alice@REALM".to_vec()),
            alloc::vec![0xaa, 0xbb],
        )
    }

    #[test]
    fn a_stateful_target_answers_establish_with_a_stateful_complete() {
        let mut target = SasTarget::stateful();
        let mut c = client();
        let request = c.next_request();
        let TargetAction::Accept(reply) = target.accept(&request) else {
            panic!("a stateful target accepts an EstablishContext");
        };
        let SasMessage::CompleteEstablishContext(cc) = &reply else {
            panic!("expected CompleteEstablishContext");
        };
        assert!(cc.context_stateful);
        assert_eq!(cc.client_context_id, 1);
        assert_eq!(target.retained_contexts(), 1);
    }

    #[test]
    fn a_stateless_target_retains_nothing_and_says_so() {
        let mut target = SasTarget::stateless();
        let mut c = client();
        let TargetAction::Accept(reply) = target.accept(&c.next_request()) else {
            panic!("a stateless target still accepts an EstablishContext");
        };
        let SasMessage::CompleteEstablishContext(cc) = &reply else {
            panic!("expected CompleteEstablishContext");
        };
        assert!(!cc.context_stateful);
        assert_eq!(target.retained_contexts(), 0);
    }

    #[test]
    fn the_second_request_on_a_stateful_context_is_a_message_in_context() {
        let mut target = SasTarget::stateful();
        let mut c = client();
        let first = c.next_request();
        let TargetAction::Accept(reply) = target.accept(&first) else {
            panic!("establish accepted");
        };
        c.on_reply(&reply);
        assert_eq!(c.established_context(), Some(1));

        let second = c.next_request();
        let SasMessage::MessageInContext(m) = &second else {
            panic!("the second request names the context instead of re-sending credentials");
        };
        assert_eq!(m.client_context_id, 1);
        assert!(!m.discard_context);
        assert_eq!(target.accept(&second), TargetAction::AcceptSilently);
    }

    #[test]
    fn a_stateless_client_re_establishes_on_every_request() {
        let mut target = SasTarget::stateless();
        let mut c = client();
        for expected_id in 1..=3u64 {
            let request = c.next_request();
            let SasMessage::EstablishContext(ec) = &request else {
                panic!("a stateless client establishes every time");
            };
            assert_eq!(ec.client_context_id, expected_id);
            let TargetAction::Accept(reply) = target.accept(&request) else {
                panic!("accepted");
            };
            c.on_reply(&reply);
            assert_eq!(c.established_context(), None);
        }
    }

    #[test]
    fn discarding_a_context_makes_the_next_reference_to_it_a_context_error() {
        let mut target = SasTarget::stateful();
        let mut c = client();
        let TargetAction::Accept(reply) = target.accept(&c.next_request()) else {
            panic!("establish accepted");
        };
        c.on_reply(&reply);

        let discard = c.discard_request().expect("a held context can be released");
        assert_eq!(target.accept(&discard), TargetAction::AcceptSilently);
        assert_eq!(target.retained_contexts(), 0);
        assert_eq!(c.established_context(), None);

        // The same id, named again, is now a context the target does not hold.
        let stale = SasMessage::MessageInContext(MessageInContext {
            client_context_id: 1,
            discard_context: false,
        });
        let TargetAction::Refuse(SasMessage::ContextError(e)) = target.accept(&stale) else {
            panic!("a discarded context is refused");
        };
        assert_eq!((e.major_status, e.minor_status), CONTEXT_ERROR_NO_CONTEXT);
        assert_eq!(e.client_context_id, 1);
    }

    #[test]
    fn the_context_error_status_pairs_match_the_jacorb_measurement() {
        // The caller (`crate::authenticator::authenticate_gssup`) builds
        // most of these `ContextError`s itself — `SasTarget` does not
        // know about credentials — but the status pairs live here so
        // every caller reports the same numbers. Values per this
        // module's own doc block: measured against JacORB 3.9 this
        // round, `NO_CONTEXT` from this crate's earlier (unreverified
        // this round) reading of the spec table.
        assert_eq!(CONTEXT_ERROR_INVALID_EVIDENCE, (1, 1));
        assert_eq!(CONTEXT_ERROR_INVALID_MECHANISM, (2, 1));
        assert_eq!(CONTEXT_ERROR_NO_CONTEXT, (4, 1));
        assert_ne!(CONTEXT_ERROR_INVALID_EVIDENCE, CONTEXT_ERROR_NO_CONTEXT);
        assert_ne!(
            CONTEXT_ERROR_INVALID_EVIDENCE,
            CONTEXT_ERROR_INVALID_MECHANISM
        );
    }

    #[test]
    fn a_message_in_context_for_an_unknown_id_is_refused() {
        let mut target = SasTarget::stateful();
        let stale = SasMessage::MessageInContext(MessageInContext {
            client_context_id: 0xdead_beef,
            discard_context: false,
        });
        let TargetAction::Refuse(SasMessage::ContextError(e)) = target.accept(&stale) else {
            panic!("an unestablished context is refused");
        };
        assert_eq!((e.major_status, e.minor_status), CONTEXT_ERROR_NO_CONTEXT);
    }

    #[test]
    fn a_context_error_puts_the_client_back_into_the_establish_state() {
        let mut target = SasTarget::stateful();
        let mut c = client();
        let TargetAction::Accept(reply) = target.accept(&c.next_request()) else {
            panic!("establish accepted");
        };
        c.on_reply(&reply);
        assert_eq!(c.established_context(), Some(1));

        // The target loses the context — a restart, an eviction.
        let mut restarted = SasTarget::stateful();
        let request = c.next_request();
        let TargetAction::Refuse(error) = restarted.accept(&request) else {
            panic!("the restarted target holds nothing");
        };
        c.on_reply(&error);
        assert_eq!(c.established_context(), None);

        // The next request carries the credentials again.
        let SasMessage::EstablishContext(ec) = c.next_request() else {
            panic!("the client re-establishes after a ContextError");
        };
        assert_eq!(ec.client_authentication_token, alloc::vec![0xaa, 0xbb]);
    }

    #[test]
    fn a_target_to_client_message_arriving_at_a_target_is_not_evidence() {
        let mut target = SasTarget::stateful();
        let misdirected = SasMessage::CompleteEstablishContext(CompleteEstablishContext {
            client_context_id: 5,
            context_stateful: true,
            final_context_token: Vec::new(),
        });
        let TargetAction::Refuse(SasMessage::ContextError(e)) = target.accept(&misdirected) else {
            panic!("refused");
        };
        assert_eq!(
            (e.major_status, e.minor_status),
            CONTEXT_ERROR_INVALID_EVIDENCE
        );
        assert_eq!(target.retained_contexts(), 0);
    }

    #[test]
    fn a_full_session_survives_the_wire_in_both_byte_orders() {
        // The state machine of §10.3 over the octets of §10.2: every
        // message crosses `encode_encapsulation`/`decode_encapsulation`
        // before the other half sees it.
        for e in [Endianness::Big, Endianness::Little] {
            let mut target = SasTarget::stateful();
            let mut c = client();

            let on_wire = |m: &SasMessage| {
                let bytes = m.encode_encapsulation(e).unwrap();
                SasMessage::decode_encapsulation(&bytes).unwrap()
            };

            let establish = on_wire(&c.next_request());
            let TargetAction::Accept(reply) = target.accept(&establish) else {
                panic!("establish accepted");
            };
            c.on_reply(&on_wire(&reply));
            assert_eq!(c.established_context(), Some(1));
            assert!(target.context(1).is_some());
            assert_eq!(
                target.context(1).unwrap().identity_token,
                IdentityToken::PrincipalName(b"alice@REALM".to_vec())
            );

            let second = on_wire(&c.next_request());
            assert_eq!(target.accept(&second), TargetAction::AcceptSilently);

            let discard = on_wire(&c.discard_request().unwrap());
            assert_eq!(target.accept(&discard), TargetAction::AcceptSilently);
            assert_eq!(target.retained_contexts(), 0);
        }
    }
}
