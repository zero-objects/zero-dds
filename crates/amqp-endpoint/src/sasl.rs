// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SASL frame layer for the AMQP endpoint.
//!
//! Spec source: dds-amqp-1.0-beta1.pdf §10.2 SASL Mechanisms.
//! Mandatory: PLAIN (RFC 4616), ANONYMOUS (RFC 4505),
//! EXTERNAL (RFC 4422 §7).

use alloc::string::{String, ToString};

/// Spec §10.2 — SASL mechanism identifier.
///
/// Mandatory set: PLAIN, ANONYMOUS, EXTERNAL.
/// Optional set: SCRAM-SHA-256 (Spec: "implementations MAY
/// additionally support SCRAM-SHA-256"; RFC 7677).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    /// PLAIN — Username + Password (REQUIRES TLS).
    Plain,
    /// ANONYMOUS — no authentication token.
    Anonymous,
    /// EXTERNAL — authentication delegated to the transport layer
    /// (typically mTLS).
    External,
    /// SCRAM-SHA-256 — Salted Challenge Response Authentication
    /// Mechanism (RFC 7677). Optional per Spec §10.2.
    ScramSha256,
}

impl SaslMechanism {
    /// Spec §10.2 — mechanism name as an ASCII symbol on the wire.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Plain => "PLAIN",
            Self::Anonymous => "ANONYMOUS",
            Self::External => "EXTERNAL",
            Self::ScramSha256 => "SCRAM-SHA-256",
        }
    }

    /// Parse the mechanism from the wire symbol.
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "PLAIN" => Some(Self::Plain),
            "ANONYMOUS" => Some(Self::Anonymous),
            "EXTERNAL" => Some(Self::External),
            "SCRAM-SHA-256" => Some(Self::ScramSha256),
            _ => None,
        }
    }

    /// `true` if the mechanism belongs to the mandatory set
    /// (Spec §10.2 lists PLAIN/ANONYMOUS/EXTERNAL as mandatory).
    #[must_use]
    pub const fn is_mandatory(self) -> bool {
        matches!(self, Self::Plain | Self::Anonymous | Self::External)
    }
}

/// SASL outcome code per OASIS AMQP 1.0 §5.3.3.6 (wire protocol).
///
/// The codes are spec-normative; they are written directly as a
/// 1-byte value onto the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SaslCode {
    /// `0` — ok. Connection authentication succeeded.
    Ok = 0,
    /// `1` — auth. Authentication failed due to an unspecified problem
    /// with the supplied credentials.
    Auth = 1,
    /// `2` — sys. Transient system error.
    Sys = 2,
    /// `3` — sys-perm. Permanent system error.
    SysPerm = 3,
    /// `4` — sys-temp. Temporary system error.
    SysTemp = 4,
}

impl SaslCode {
    /// Wire value.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// `u8 -> SaslCode`.
    ///
    /// # Errors
    /// `()` if the value is not a registered spec code.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::Ok),
            1 => Ok(Self::Auth),
            2 => Ok(Self::Sys),
            3 => Ok(Self::SysPerm),
            4 => Ok(Self::SysTemp),
            _ => Err(()),
        }
    }
}

/// SASL-State-Machine-Outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaslOutcome {
    /// Authentication succeeded (wire code `ok` = 0).
    Authenticated {
        /// Authenticated subject identifier (e.g. user name).
        subject: String,
    },
    /// Authentication failed — carries the spec wire code
    /// per §5.3.3.6 (typically `auth` = 1).
    Failed {
        /// Wire code per OASIS AMQP 1.0 §5.3.3.6.
        code: SaslCode,
        /// Reason (internal diagnostic string, not wire).
        reason: String,
    },
}

impl SaslOutcome {
    /// Constructor for the common case `Failed{code:Auth, reason}`.
    #[must_use]
    pub fn auth_failed(reason: impl Into<String>) -> Self {
        Self::Failed {
            code: SaslCode::Auth,
            reason: reason.into(),
        }
    }

    /// `true` if the outcome was a successful authentication.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }

    /// Wire code per §5.3.3.6.
    #[must_use]
    pub fn wire_code(&self) -> SaslCode {
        match self {
            Self::Authenticated { .. } => SaslCode::Ok,
            Self::Failed { code, .. } => *code,
        }
    }
}

/// SASL connection state (simplified layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaslState {
    /// Mechanisms offered by the server.
    pub offered: alloc::vec::Vec<SaslMechanism>,
    /// Current outcome (None = handshake not yet complete).
    pub outcome: Option<SaslOutcome>,
}

impl SaslState {
    /// Constructor with the default mechanism set. SCRAM-SHA-256 + ANONYMOUS +
    /// EXTERNAL are always offered; PLAIN only with TLS (it puts the password on
    /// the wire). SCRAM is offered regardless of TLS — its challenge/response
    /// never transmits the password, so it is the strong choice on plaintext too.
    #[must_use]
    pub fn new(tls_active: bool) -> Self {
        let mut offered = alloc::vec::Vec::new();
        offered.push(SaslMechanism::ScramSha256);
        if tls_active {
            offered.push(SaslMechanism::Plain);
        }
        offered.push(SaslMechanism::Anonymous);
        offered.push(SaslMechanism::External);
        Self {
            offered,
            outcome: None,
        }
    }

    /// Map the result of a [`crate::scram::ScramServerExchange`] onto the SASL
    /// outcome. The two-round SCRAM message flow is driven by the SASL frame
    /// handler (server-first challenge then client-final); this records the
    /// terminal step. SCRAM never requires TLS to be offered.
    pub fn finish_scram(&mut self, step: crate::scram::ScramStep) {
        if !self.offered.contains(&SaslMechanism::ScramSha256) {
            self.outcome = Some(SaslOutcome::auth_failed("mechanism not offered"));
            return;
        }
        self.outcome = Some(match step {
            crate::scram::ScramStep::Success { username, .. } => {
                SaslOutcome::Authenticated { subject: username }
            }
            crate::scram::ScramStep::Failure(reason) => SaslOutcome::auth_failed(reason),
        });
    }

    /// Verifies PLAIN credentials against a caller-supplied
    /// verifier.
    pub fn authenticate_plain<F>(&mut self, username: &str, password: &str, verifier: F)
    where
        F: Fn(&str, &str) -> bool,
    {
        if !self.offered.contains(&SaslMechanism::Plain) {
            self.outcome = Some(SaslOutcome::auth_failed("mechanism not offered"));
            return;
        }
        if verifier(username, password) {
            self.outcome = Some(SaslOutcome::Authenticated {
                subject: username.to_string(),
            });
        } else {
            self.outcome = Some(SaslOutcome::auth_failed("credentials rejected"));
        }
    }

    /// ANONYMOUS outcome — always authenticated with an empty subject.
    pub fn authenticate_anonymous(&mut self) {
        if !self.offered.contains(&SaslMechanism::Anonymous) {
            self.outcome = Some(SaslOutcome::auth_failed("mechanism not offered"));
            return;
        }
        self.outcome = Some(SaslOutcome::Authenticated {
            subject: String::new(),
        });
    }

    /// EXTERNAL outcome — subject from the transport layer (e.g. mTLS CN).
    pub fn authenticate_external(&mut self, transport_subject: &str) {
        if !self.offered.contains(&SaslMechanism::External) {
            self.outcome = Some(SaslOutcome::auth_failed("mechanism not offered"));
            return;
        }
        self.outcome = Some(SaslOutcome::Authenticated {
            subject: transport_subject.to_string(),
        });
    }

    /// Spec §2.2 Bridge Profile Cl. 5 + §10.2.1 — the outbound initiator
    /// selects from the mechanisms offered by the broker.
    /// `tls_active = false` excludes PLAIN.
    ///
    /// Returns the selected mechanism, or `None` if none of the
    /// offered mechanisms is acceptable under the TLS constraints.
    #[must_use]
    pub fn select_outbound(offered: &[SaslMechanism], tls_active: bool) -> Option<SaslMechanism> {
        // Spec precedence: EXTERNAL (mTLS) > SCRAM-SHA-256 > PLAIN (with TLS) >
        // ANONYMOUS. SCRAM ranks above PLAIN because it never exposes the
        // password and needs no TLS; PLAIN is the cleartext fallback.
        if offered.contains(&SaslMechanism::External) {
            return Some(SaslMechanism::External);
        }
        if offered.contains(&SaslMechanism::ScramSha256) {
            return Some(SaslMechanism::ScramSha256);
        }
        if tls_active && offered.contains(&SaslMechanism::Plain) {
            return Some(SaslMechanism::Plain);
        }
        if offered.contains(&SaslMechanism::Anonymous) {
            return Some(SaslMechanism::Anonymous);
        }
        None
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn name_round_trips() {
        for m in [
            SaslMechanism::Plain,
            SaslMechanism::Anonymous,
            SaslMechanism::External,
        ] {
            assert_eq!(SaslMechanism::from_name(m.name()), Some(m));
        }
    }

    #[test]
    fn unknown_name_yields_none() {
        assert!(SaslMechanism::from_name("UNKNOWN").is_none());
    }

    #[test]
    fn select_outbound_prefers_external() {
        let offered = [
            SaslMechanism::Plain,
            SaslMechanism::Anonymous,
            SaslMechanism::External,
        ];
        assert_eq!(
            SaslState::select_outbound(&offered, true),
            Some(SaslMechanism::External)
        );
    }

    #[test]
    fn select_outbound_falls_back_to_plain_with_tls() {
        let offered = [SaslMechanism::Plain, SaslMechanism::Anonymous];
        assert_eq!(
            SaslState::select_outbound(&offered, true),
            Some(SaslMechanism::Plain)
        );
    }

    #[test]
    fn select_outbound_skips_plain_without_tls() {
        let offered = [SaslMechanism::Plain, SaslMechanism::Anonymous];
        assert_eq!(
            SaslState::select_outbound(&offered, false),
            Some(SaslMechanism::Anonymous)
        );
    }

    #[test]
    fn select_outbound_anonymous_only() {
        let offered = [SaslMechanism::Anonymous];
        assert_eq!(
            SaslState::select_outbound(&offered, false),
            Some(SaslMechanism::Anonymous)
        );
    }

    #[test]
    fn select_outbound_no_acceptable_mechanism() {
        // Broker offers only PLAIN, no TLS active → no
        // acceptable mechanism.
        let offered = [SaslMechanism::Plain];
        assert_eq!(SaslState::select_outbound(&offered, false), None);
    }

    #[test]
    fn plain_offered_only_when_tls_active() {
        let s = SaslState::new(true);
        assert!(s.offered.contains(&SaslMechanism::Plain));
        let s = SaslState::new(false);
        assert!(!s.offered.contains(&SaslMechanism::Plain));
    }

    #[test]
    fn plain_authenticates_on_correct_credentials() {
        let mut s = SaslState::new(true);
        s.authenticate_plain("alice", "secret", |u, p| u == "alice" && p == "secret");
        assert!(matches!(
            s.outcome,
            Some(SaslOutcome::Authenticated { ref subject }) if subject == "alice"
        ));
    }

    #[test]
    fn plain_fails_on_wrong_credentials() {
        let mut s = SaslState::new(true);
        s.authenticate_plain("alice", "wrong", |_, _| false);
        assert!(matches!(s.outcome, Some(SaslOutcome::Failed { .. })));
    }

    #[test]
    fn plain_without_tls_yields_auth_failed() {
        let mut s = SaslState::new(false);
        s.authenticate_plain("a", "b", |_, _| true);
        // Spec OASIS AMQP 1.0 §5.3.3.6 — code `auth` (1) for
        // mechanism-not-offered (no dedicated `unsupported` code).
        assert!(matches!(
            s.outcome,
            Some(SaslOutcome::Failed {
                code: SaslCode::Auth,
                ..
            })
        ));
    }

    #[test]
    fn sasl_code_wire_values_match_spec() {
        assert_eq!(SaslCode::Ok.to_u8(), 0);
        assert_eq!(SaslCode::Auth.to_u8(), 1);
        assert_eq!(SaslCode::Sys.to_u8(), 2);
        assert_eq!(SaslCode::SysPerm.to_u8(), 3);
        assert_eq!(SaslCode::SysTemp.to_u8(), 4);
    }

    #[test]
    fn sasl_code_from_u8_round_trip() {
        for c in [
            SaslCode::Ok,
            SaslCode::Auth,
            SaslCode::Sys,
            SaslCode::SysPerm,
            SaslCode::SysTemp,
        ] {
            assert_eq!(SaslCode::from_u8(c.to_u8()).expect("ok"), c);
        }
    }

    #[test]
    fn sasl_code_unknown_value_rejected() {
        assert!(SaslCode::from_u8(99).is_err());
    }

    #[test]
    fn outcome_authenticated_wire_code_is_ok() {
        let o = SaslOutcome::Authenticated {
            subject: "u".into(),
        };
        assert_eq!(o.wire_code(), SaslCode::Ok);
        assert!(o.is_ok());
    }

    #[test]
    fn outcome_auth_failed_helper_uses_auth_code() {
        let o = SaslOutcome::auth_failed("bad credentials");
        assert_eq!(o.wire_code(), SaslCode::Auth);
        assert!(!o.is_ok());
    }

    #[test]
    fn anonymous_authenticates_without_credentials() {
        let mut s = SaslState::new(false);
        s.authenticate_anonymous();
        assert!(matches!(
            s.outcome,
            Some(SaslOutcome::Authenticated { ref subject }) if subject.is_empty()
        ));
    }

    #[test]
    fn external_uses_transport_subject() {
        let mut s = SaslState::new(true);
        s.authenticate_external("CN=alice");
        assert!(matches!(
            s.outcome,
            Some(SaslOutcome::Authenticated { ref subject }) if subject == "CN=alice"
        ));
    }
}
