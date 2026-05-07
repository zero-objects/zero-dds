// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SASL-Frame-Layer fuer den AMQP-Endpoint.
//!
//! Spec-Quelle: dds-amqp-1.0-beta1.pdf §10.2 SASL Mechanisms.
//! Mandatory: PLAIN (RFC 4616), ANONYMOUS (RFC 4505),
//! EXTERNAL (RFC 4422 §7).

use alloc::string::{String, ToString};

/// Spec §10.2 — SASL-Mechanism-Identifier.
///
/// Mandatory-Set: PLAIN, ANONYMOUS, EXTERNAL.
/// Optional-Set: SCRAM-SHA-256 (Spec: "implementations MAY
/// additionally support SCRAM-SHA-256"; RFC 7677).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    /// PLAIN — Username + Password (REQUIRES TLS).
    Plain,
    /// ANONYMOUS — kein Authentifizierungs-Token.
    Anonymous,
    /// EXTERNAL — Authentifizierung delegiert an Transport-Layer
    /// (typisch mTLS).
    External,
    /// SCRAM-SHA-256 — Salted Challenge Response Authentication
    /// Mechanism (RFC 7677). Optional gemaess Spec §10.2.
    ScramSha256,
}

impl SaslMechanism {
    /// Spec §10.2 — Mechanismus-Name als ASCII-Symbol auf der Wire.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Plain => "PLAIN",
            Self::Anonymous => "ANONYMOUS",
            Self::External => "EXTERNAL",
            Self::ScramSha256 => "SCRAM-SHA-256",
        }
    }

    /// Mechanismus aus dem Wire-Symbol parsen.
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

    /// `true` wenn der Mechanismus zum Mandatory-Set gehoert
    /// (Spec §10.2 listet PLAIN/ANONYMOUS/EXTERNAL als mandatory).
    #[must_use]
    pub const fn is_mandatory(self) -> bool {
        matches!(self, Self::Plain | Self::Anonymous | Self::External)
    }
}

/// SASL-Outcome-Code nach OASIS AMQP 1.0 §5.3.3.6 (Wire-Protocol).
///
/// Die Codes sind Spec-normiert; sie werden direkt als 1-Byte-Wert
/// auf den Wire geschrieben.
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
    /// Wire-Wert.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// `u8 -> SaslCode`.
    ///
    /// # Errors
    /// `()` wenn der Wert kein registrierter Spec-Code ist.
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
    /// Authentifizierung erfolgreich (Wire-Code `ok` = 0).
    Authenticated {
        /// Authenticated subject identifier (z.B. user-name).
        subject: String,
    },
    /// Authentifizierung fehlgeschlagen — traegt den Spec-Wire-Code
    /// nach §5.3.3.6 (typischerweise `auth` = 1).
    Failed {
        /// Wire-Code nach OASIS AMQP 1.0 §5.3.3.6.
        code: SaslCode,
        /// Begruendung (interner Diagnostic-String, nicht Wire).
        reason: String,
    },
}

impl SaslOutcome {
    /// Konstruktor fuer den haeufigen Fall `Failed{code:Auth, reason}`.
    #[must_use]
    pub fn auth_failed(reason: impl Into<String>) -> Self {
        Self::Failed {
            code: SaslCode::Auth,
            reason: reason.into(),
        }
    }

    /// `true` wenn der Outcome Authentifizierung erfolgreich war.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }

    /// Wire-Code nach §5.3.3.6.
    #[must_use]
    pub fn wire_code(&self) -> SaslCode {
        match self {
            Self::Authenticated { .. } => SaslCode::Ok,
            Self::Failed { code, .. } => *code,
        }
    }
}

/// SASL-Connection-State (vereinfachter Layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaslState {
    /// Vom Server angebotene Mechanismen.
    pub offered: alloc::vec::Vec<SaslMechanism>,
    /// Aktueller Outcome (None = Handshake noch nicht abgeschlossen).
    pub outcome: Option<SaslOutcome>,
}

impl SaslState {
    /// Konstruktor mit Default-Mechanismen-Set (PLAIN+EXTERNAL+
    /// ANONYMOUS, wenn TLS aktiv; sonst nur ANONYMOUS+EXTERNAL).
    #[must_use]
    pub fn new(tls_active: bool) -> Self {
        let mut offered = alloc::vec::Vec::new();
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

    /// Verifiziert PLAIN-Credentials gegen einen Caller-supplied
    /// Verifier.
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

    /// ANONYMOUS-Outcome — immer authenticated mit leerem Subject.
    pub fn authenticate_anonymous(&mut self) {
        if !self.offered.contains(&SaslMechanism::Anonymous) {
            self.outcome = Some(SaslOutcome::auth_failed("mechanism not offered"));
            return;
        }
        self.outcome = Some(SaslOutcome::Authenticated {
            subject: String::new(),
        });
    }

    /// EXTERNAL-Outcome — Subject vom Transport-Layer (z.B. mTLS-CN).
    pub fn authenticate_external(&mut self, transport_subject: &str) {
        if !self.offered.contains(&SaslMechanism::External) {
            self.outcome = Some(SaslOutcome::auth_failed("mechanism not offered"));
            return;
        }
        self.outcome = Some(SaslOutcome::Authenticated {
            subject: transport_subject.to_string(),
        });
    }

    /// Spec §2.2 Bridge-Profile Cl. 5 + §10.2.1 — Outbound-Initiator
    /// waehlt aus den vom Broker angebotenen Mechanismen.
    /// `tls_active = false` schliesst PLAIN aus.
    ///
    /// Returns den gewaehlten Mechanismus oder `None` wenn keiner
    /// der angebotenen Mechanismen unter den TLS-Constraints
    /// akzeptabel ist.
    #[must_use]
    pub fn select_outbound(offered: &[SaslMechanism], tls_active: bool) -> Option<SaslMechanism> {
        // Spec-Praezedenz: EXTERNAL (mTLS) > PLAIN (mit TLS) > ANONYMOUS.
        if offered.contains(&SaslMechanism::External) {
            return Some(SaslMechanism::External);
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
        // Broker bietet nur PLAIN, kein TLS aktiv → kein
        // akzeptabler Mechanismus.
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
        // Spec OASIS AMQP 1.0 §5.3.3.6 — Code `auth` (1) fuer
        // Mechanism-not-offered (kein eigener `unsupported`-Code).
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
