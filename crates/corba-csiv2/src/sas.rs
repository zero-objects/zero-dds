// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SAS-Protocol — Spec §24.2.
//!
//! Vier Message-Typen:
//! * `EstablishContext` (msg-id 0) — Client startet Security-Context.
//! * `CompleteEstablishContext` (msg-id 1) — Server-Response.
//! * `MessageInContext` (msg-id 2) — Folge-Request mit Reused-Context.
//! * `ContextError` (msg-id 4) — Fehler in Context-Etablierung.
//!
//! Wire-Form: SAS-Message wird als `IOP::ServiceContext` mit Tag
//! `TAG_SECURITY_ATTRIBUTE_SERVICE = 15` transportiert.

use alloc::vec::Vec;

/// SAS-Message-Discriminator (Spec §24.2.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SasMsgType {
    /// `MTEstablishContext = 0`.
    EstablishContext = 0,
    /// `MTCompleteEstablishContext = 1`.
    CompleteEstablishContext = 1,
    /// `MTContextError = 4` (Spec assigniert nicht-konsekutive IDs).
    ContextError = 4,
    /// `MTMessageInContext = 2`.
    MessageInContext = 2,
}

/// `IdentityToken`-Discriminator (Spec §24.2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityToken {
    /// `ITTAbsent = 0` — kein Token.
    Absent,
    /// `ITTAnonymous = 1` — anonymer Caller.
    Anonymous,
    /// `ITTPrincipalName = 2` — DER-encoded GSS-Name.
    PrincipalName(Vec<u8>),
    /// `ITTX509CertChain = 4`.
    X509CertChain(Vec<u8>),
    /// `ITTDistinguishedName = 8` — RFC-1779 DN.
    DistinguishedName(Vec<u8>),
}

impl IdentityToken {
    /// Discriminator-Wert (Spec §24.2.5).
    #[must_use]
    pub const fn discriminator(&self) -> u32 {
        match self {
            Self::Absent => 0,
            Self::Anonymous => 1,
            Self::PrincipalName(_) => 2,
            Self::X509CertChain(_) => 4,
            Self::DistinguishedName(_) => 8,
        }
    }
}

/// `EstablishContext` (Spec §24.2.6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstablishContext {
    /// `ContextId` (`unsigned long long`). 0 = stateless.
    pub client_context_id: u64,
    /// `AuthorizationToken` (typisch leer).
    pub authorization_token: Vec<u8>,
    /// `IdentityToken`.
    pub identity_token: IdentityToken,
    /// `ClientAuthenticationToken` (typisch GSSUP-encapsuliert).
    pub client_authentication_token: Vec<u8>,
}

/// `CompleteEstablishContext` (Spec §24.2.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteEstablishContext {
    /// Korrelierende ContextId.
    pub client_context_id: u64,
    /// `context_stateful` — `true` wenn Server den Context behaelt.
    pub context_stateful: bool,
    /// `final_context_token` — typisch leer fuer GSSUP.
    pub final_context_token: Vec<u8>,
}

/// `MessageInContext` (Spec §24.2.6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageInContext {
    /// Korrelierende ContextId.
    pub client_context_id: u64,
    /// `discard_context` — `true` bedeutet Server soll Context verwerfen.
    pub discard_context: bool,
}

/// `ContextError` (Spec §24.2.6.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextError {
    /// Korrelierende ContextId.
    pub client_context_id: u64,
    /// Major status code (GSS-API-Style).
    pub major_status: u32,
    /// Minor status code.
    pub minor_status: u32,
    /// `error_token` (DER-encoded GSS-Error).
    pub error_token: Vec<u8>,
}

/// SAS-Message-Union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SasMessage {
    /// EstablishContext.
    EstablishContext(EstablishContext),
    /// CompleteEstablishContext.
    CompleteEstablishContext(CompleteEstablishContext),
    /// ContextError.
    ContextError(ContextError),
    /// MessageInContext.
    MessageInContext(MessageInContext),
}

impl SasMessage {
    /// Discriminator.
    #[must_use]
    pub fn msg_type(&self) -> SasMsgType {
        match self {
            Self::EstablishContext(_) => SasMsgType::EstablishContext,
            Self::CompleteEstablishContext(_) => SasMsgType::CompleteEstablishContext,
            Self::ContextError(_) => SasMsgType::ContextError,
            Self::MessageInContext(_) => SasMsgType::MessageInContext,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sas_msg_type_values_match_spec() {
        // Spec §24.2.6.1.
        assert_eq!(SasMsgType::EstablishContext as u32, 0);
        assert_eq!(SasMsgType::CompleteEstablishContext as u32, 1);
        assert_eq!(SasMsgType::MessageInContext as u32, 2);
        assert_eq!(SasMsgType::ContextError as u32, 4);
    }

    #[test]
    fn identity_token_discriminator_matches_spec() {
        // Spec §24.2.5: ITT* = power-of-2.
        assert_eq!(IdentityToken::Absent.discriminator(), 0);
        assert_eq!(IdentityToken::Anonymous.discriminator(), 1);
        assert_eq!(
            IdentityToken::PrincipalName(alloc::vec![]).discriminator(),
            2
        );
        assert_eq!(
            IdentityToken::X509CertChain(alloc::vec![]).discriminator(),
            4
        );
        assert_eq!(
            IdentityToken::DistinguishedName(alloc::vec![]).discriminator(),
            8
        );
    }

    #[test]
    fn establish_context_holds_full_payload() {
        let ec = EstablishContext {
            client_context_id: 42,
            authorization_token: alloc::vec![],
            identity_token: IdentityToken::PrincipalName(b"alice@REALM".to_vec()),
            client_authentication_token: alloc::vec![0xab, 0xcd],
        };
        let msg = SasMessage::EstablishContext(ec);
        assert_eq!(msg.msg_type(), SasMsgType::EstablishContext);
    }

    #[test]
    fn complete_context_indicates_stateful() {
        let cc = CompleteEstablishContext {
            client_context_id: 42,
            context_stateful: true,
            final_context_token: alloc::vec![],
        };
        let msg = SasMessage::CompleteEstablishContext(cc);
        match msg {
            SasMessage::CompleteEstablishContext(c) => assert!(c.context_stateful),
            _ => panic!(),
        }
    }

    #[test]
    fn message_in_context_can_request_discard() {
        let m = MessageInContext {
            client_context_id: 42,
            discard_context: true,
        };
        let msg = SasMessage::MessageInContext(m);
        assert_eq!(msg.msg_type(), SasMsgType::MessageInContext);
    }

    #[test]
    fn context_error_carries_gss_status() {
        let e = ContextError {
            client_context_id: 42,
            major_status: 0x0007_0000, // GSS_S_DEFECTIVE_TOKEN
            minor_status: 0,
            error_token: alloc::vec![],
        };
        let msg = SasMessage::ContextError(e);
        assert_eq!(msg.msg_type(), SasMsgType::ContextError);
    }
}
