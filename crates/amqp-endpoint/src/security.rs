// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security-Bridge fuer den AMQP-Endpoint.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 §10.3.2 — IdentityToken-class_id-Tabelle pro
//!   SASL-Mechanismus (`zerodds:Auth:SASL-Username:1.0`,
//!   `zerodds:Auth:Anonymous:1.0`,
//!   `zerodds:Auth:SASL-SCRAM-SHA256:1.0`,
//!   `DDS:Auth:PKI-DH:1.0` nur fuer EXTERNAL+X.509).
//! * §10.3.3 — Permission Evaluation an AccessControl-Plugin.
//! * §10.3.5 — No-Bypass-Guarantee.
//! * §10.4 — Governance-Document-Mapping.
//! * §10.6 — Bridge-Profile Dual Identity.
//! * §10.7 — Per-Link Governance Resolution.
//!
//! Diese Schicht kennt das DDS-Security-Plugin nicht direkt;
//! sie liefert Trait-basierte Adapter, gegen die der Endpoint-
//! Daemon (oder ein Test-Mock) bindet.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::sasl::SaslMechanism;

// ============================================================
// IdentityToken (§10.3.2)
// ============================================================

/// Spec §10.3.2 — class_id-Konstanten pro SASL-Mechanismus.
pub mod class_ids {
    /// PLAIN → vendor-prefixed Username-Token.
    pub const SASL_USERNAME: &str = "zerodds:Auth:SASL-Username:1.0";
    /// ANONYMOUS → vendor-prefixed Anonymous-Token.
    pub const ANONYMOUS: &str = "zerodds:Auth:Anonymous:1.0";
    /// SCRAM-SHA-256 → vendor-prefixed SCRAM-Token.
    pub const SCRAM_SHA256: &str = "zerodds:Auth:SASL-SCRAM-SHA256:1.0";
    /// EXTERNAL (mTLS-X.509) → OMG-PKI-DH:1.0 (nur diese Form ist
    /// PKI-DH-konform: enthaelt `certificate`).
    pub const PKI_DH: &str = "DDS:Auth:PKI-DH:1.0";
}

/// Spec §10.3.2 — IdentityToken (subset). DDS-Security 1.2 §8.4.1
/// spezifiziert weitere Felder (binary properties, etc.); fuer das
/// AMQP-Mapping reichen subject_name, certificate, class_id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityToken {
    /// Spec §10.3.2 — `class_id` aus [`class_ids`].
    pub class_id: String,
    /// Spec §10.3.2 — `subject_name`, RFC-4514-DN-Form
    /// (`CN=alice` etc.).
    pub subject_name: String,
    /// Spec §10.3.2 — X.509-Cert (DER-bytes), nur bei EXTERNAL/PKI-DH
    /// gesetzt.
    pub certificate: Option<Vec<u8>>,
}

/// Spec §10.3.2 — IdentityToken aus SASL-Outcome bauen.
///
/// Die Tabelle bildet:
/// * `Plain(authcid)` → `SASL-Username:1.0`, `subject_name = "CN=" + authcid`,
///   `certificate = None`.
/// * `Anonymous` → `Anonymous:1.0`, `subject_name = "CN=ANONYMOUS"`,
///   `certificate = None`.
/// * `External(cert_der, dn)` → `PKI-DH:1.0`, `subject_name = dn`
///   (wie vom Transport gemeldet), `certificate = Some(cert_der)`.
/// * `ScramSha256(authcid)` → `SASL-SCRAM-SHA256:1.0`,
///   `subject_name = "CN=" + authcid`.
#[must_use]
pub fn build_identity_token(input: &SaslSubject) -> IdentityToken {
    match input {
        SaslSubject::Plain { authcid } => IdentityToken {
            class_id: class_ids::SASL_USERNAME.to_string(),
            subject_name: alloc::format!("CN={authcid}"),
            certificate: None,
        },
        SaslSubject::Anonymous => IdentityToken {
            class_id: class_ids::ANONYMOUS.to_string(),
            subject_name: "CN=ANONYMOUS".to_string(),
            certificate: None,
        },
        SaslSubject::External {
            certificate,
            subject_dn,
        } => IdentityToken {
            class_id: class_ids::PKI_DH.to_string(),
            subject_name: subject_dn.clone(),
            certificate: Some(certificate.clone()),
        },
        SaslSubject::ScramSha256 { authcid } => IdentityToken {
            class_id: class_ids::SCRAM_SHA256.to_string(),
            subject_name: alloc::format!("CN={authcid}"),
            certificate: None,
        },
    }
}

/// Erweiterter SASL-Outcome mit den Daten, die fuer
/// IdentityToken-Konstruktion gebraucht werden.
#[derive(Debug, Clone)]
pub enum SaslSubject {
    /// SASL-PLAIN authentifiziert.
    Plain {
        /// Authenticated Username.
        authcid: String,
    },
    /// SASL-ANONYMOUS.
    Anonymous,
    /// SASL-EXTERNAL mit X.509-Cert vom Transport.
    External {
        /// X.509-DER-Cert-Bytes.
        certificate: Vec<u8>,
        /// Subject-DN aus dem Cert (RFC-4514).
        subject_dn: String,
    },
    /// SASL-SCRAM-SHA-256.
    ScramSha256 {
        /// Authenticated Username.
        authcid: String,
    },
}

impl SaslSubject {
    /// Welcher SASL-Mechanismus liegt vor?
    #[must_use]
    pub const fn mechanism(&self) -> SaslMechanism {
        match self {
            Self::Plain { .. } => SaslMechanism::Plain,
            Self::ScramSha256 { .. } => SaslMechanism::ScramSha256,
            Self::Anonymous => SaslMechanism::Anonymous,
            Self::External { .. } => SaslMechanism::External,
        }
    }
}

// ============================================================
// AccessControl-Plugin-Trait (§10.3.3 + §10.3.5)
// ============================================================

/// Spec §10.3.3 — AccessControl-Plugin-Operation, die der
/// Endpoint vor jedem Link-Attach + jedem Pre-Transfer aufruft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessOp {
    /// Sender-Link attachen (entspricht
    /// `check_create_datawriter`).
    AttachSender,
    /// Receiver-Link attachen (entspricht
    /// `check_create_datareader`).
    AttachReceiver,
    /// Sample senden (Pre-Transfer-Hook fuer No-Bypass §10.3.5).
    SendSample,
    /// Sample empfangen (Receiver-Side Pre-Decode-Hook).
    ReceiveSample,
}

/// Spec §10.3.3 — Plugin-Resultat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    /// Operation darf ausgefuehrt werden.
    Allow,
    /// Operation rejected → AMQP `amqp:unauthorized-access`.
    Deny,
}

/// Spec §10.3.3 — AccessControl-Plugin-Trait.
///
/// `check` wird mit der bereits validierten `IdentityToken`,
/// dem Address-Topic und der Operation aufgerufen. Plugin
/// entscheidet `Allow`/`Deny`. Plugin SHALL deterministisch sein
/// (Spec §10.3.4) — bei gleicher Eingabe gleiche Antwort.
pub trait AccessControlPlugin {
    /// Permission-Check.
    fn check(&self, identity: &IdentityToken, address: &str, op: AccessOp) -> AccessDecision;
}

/// Allow-All-Plugin (Test-Default; **nicht** fuer Produktion).
#[derive(Debug, Default)]
pub struct AllowAll;

impl AccessControlPlugin for AllowAll {
    fn check(&self, _identity: &IdentityToken, _address: &str, _op: AccessOp) -> AccessDecision {
        AccessDecision::Allow
    }
}

/// Static-Allow-List-Plugin (per `subject_name`-Match).
///
/// Plugin denied alles, was nicht in `allow` aufgelistet ist.
/// Pattern-Match ist exakt; Wildcards waeren ein Plugin-eigenes
/// Feature.
#[derive(Debug, Default)]
pub struct StaticAllowList {
    allow: BTreeMap<String, Vec<(String, AccessOp)>>,
}

impl StaticAllowList {
    /// Frische Liste.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Eintrag hinzufuegen: `subject_name` darf `op` auf `address`
    /// ausfuehren.
    pub fn allow(&mut self, subject_name: &str, address: &str, op: AccessOp) {
        self.allow
            .entry(subject_name.to_string())
            .or_default()
            .push((address.to_string(), op));
    }
}

impl AccessControlPlugin for StaticAllowList {
    fn check(&self, identity: &IdentityToken, address: &str, op: AccessOp) -> AccessDecision {
        if let Some(entries) = self.allow.get(&identity.subject_name) {
            if entries
                .iter()
                .any(|(addr, allowed_op)| addr == address && *allowed_op == op)
            {
                return AccessDecision::Allow;
            }
        }
        AccessDecision::Deny
    }
}

// ============================================================
// Governance Document Mapping (§10.4)
// ============================================================

/// Spec §10.4 — Domain-Governance-Rule (subset).
/// Eine echte DDS-Security-Implementierung liest dies aus dem
/// XML-Governance-Document; wir liefern hier die Datenstruktur,
/// die der XML-Loader (separater Crate) befuellt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceRule {
    /// DDS-Topic-Name oder Glob-Pattern (`Sensor*`).
    pub topic_pattern: String,
    /// SHALL Topic ueberhaupt entdeckbar sein? (`enable_discovery`).
    pub enable_discovery: bool,
    /// SHALL Liveliness signalisiert werden? (`enable_liveliness`).
    pub enable_liveliness: bool,
    /// Encryption-Modus.
    pub data_protection_kind: DataProtectionKind,
}

/// Spec §10.4 — `data_protection_kind`-Werte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataProtectionKind {
    /// Keine Sample-Encryption (Wire-clear-Body).
    None,
    /// Sign aber nicht encrypt.
    SignOnly,
    /// Sign + Encrypt.
    SignAndEncrypt,
}

/// Spec §10.4 — Governance-Document.
#[derive(Debug, Default)]
pub struct GovernanceDocument {
    rules: Vec<GovernanceRule>,
}

impl GovernanceDocument {
    /// Frisches leeres Document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Regel hinzufuegen (typisch aus XML-Loader).
    pub fn add_rule(&mut self, rule: GovernanceRule) {
        self.rules.push(rule);
    }

    /// Erste Regel liefern, deren `topic_pattern` matched.
    /// Pattern `*` matched alles, `prefix*` Praefix-Match,
    /// `*suffix` Suffix-Match, alles andere exakter Match.
    #[must_use]
    pub fn resolve(&self, topic: &str) -> Option<&GovernanceRule> {
        self.rules
            .iter()
            .find(|r| match_pattern(&r.topic_pattern, topic))
    }
}

fn match_pattern(pattern: &str, topic: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(rest) = pattern.strip_suffix('*') {
        return topic.starts_with(rest);
    }
    if let Some(rest) = pattern.strip_prefix('*') {
        return topic.ends_with(rest);
    }
    pattern == topic
}

// ============================================================
// Per-Link Governance Cache (§10.7)
// ============================================================

/// Spec §10.7 — pro Link aufgeloeste Governance + Permission.
#[derive(Debug, Clone)]
pub struct LinkGovernance {
    /// Identitaet, gegen die per-Link autorisiert wurde.
    pub identity: IdentityToken,
    /// Address (Topic) des Link-Terminus.
    pub address: String,
    /// Resolved governance rule (`None` wenn `default-rule`).
    pub rule: Option<GovernanceRule>,
    /// Permission-Cache: pro Operation `Allow`/`Deny`.
    pub cached_decisions: BTreeMap<AccessOp, AccessDecision>,
}

impl LinkGovernance {
    /// Frischer Eintrag; Cache ist leer und wird per
    /// `evaluate(plugin, op)` gefuellt.
    #[must_use]
    pub fn new(identity: IdentityToken, address: String, rule: Option<GovernanceRule>) -> Self {
        Self {
            identity,
            address,
            rule,
            cached_decisions: BTreeMap::new(),
        }
    }

    /// Per-Op-Permission auswerten + cachen.
    ///
    /// Spec §10.7 — pro Link wird Permission neu evaluiert; einmal
    /// gecacht aendert sich das Resultat fuer dieselbe Op nicht
    /// (Determinism §10.3.4).
    pub fn evaluate<P: AccessControlPlugin>(&mut self, plugin: &P, op: AccessOp) -> AccessDecision {
        if let Some(d) = self.cached_decisions.get(&op) {
            return d.clone();
        }
        let d = plugin.check(&self.identity, &self.address, op);
        self.cached_decisions.insert(op, d.clone());
        d
    }
}

// ============================================================
// Bridge-Profile Dual Identity (§10.6)
// ============================================================

/// Spec §10.6 — Dual-Identity-Konfiguration der Bridge.
///
/// Broker-Side SASL-Credential und DDS-Side IdentityToken
/// werden strikt getrennt. AccessControl-Plugin SHALL nur den
/// `dds_identity` benutzen.
#[derive(Debug, Clone)]
pub struct DualIdentity {
    /// Spec §10.6 — Identity zur Broker-Authentifizierung
    /// (z.B. `Alice` als SASL-Username).
    pub broker_identity: IdentityToken,
    /// Spec §10.6 — Identity, die in der DDS-Domain praesentiert
    /// wird (z.B. `CN=Bridge-1`).
    pub dds_identity: IdentityToken,
}

impl DualIdentity {
    /// Konstruktor.
    #[must_use]
    pub fn new(broker_identity: IdentityToken, dds_identity: IdentityToken) -> Self {
        Self {
            broker_identity,
            dds_identity,
        }
    }

    /// Spec §10.6 — die fuer DDS-Security-Calls relevante Identity.
    /// Plugin-Calls SHALL ausschliesslich diese liefern.
    #[must_use]
    pub fn for_dds(&self) -> &IdentityToken {
        &self.dds_identity
    }

    /// Identity fuer Broker-SASL-Negotiation.
    #[must_use]
    pub fn for_broker(&self) -> &IdentityToken {
        &self.broker_identity
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // --- IdentityToken-Builder ---

    #[test]
    fn plain_yields_sasl_username_class_id() {
        let t = build_identity_token(&SaslSubject::Plain {
            authcid: "alice".into(),
        });
        assert_eq!(t.class_id, class_ids::SASL_USERNAME);
        assert_eq!(t.subject_name, "CN=alice");
        assert!(t.certificate.is_none());
    }

    #[test]
    fn anonymous_yields_anonymous_class_id() {
        let t = build_identity_token(&SaslSubject::Anonymous);
        assert_eq!(t.class_id, class_ids::ANONYMOUS);
        assert_eq!(t.subject_name, "CN=ANONYMOUS");
    }

    #[test]
    fn external_yields_pki_dh_class_id_with_cert() {
        let t = build_identity_token(&SaslSubject::External {
            certificate: alloc::vec![1, 2, 3],
            subject_dn: "CN=Bridge-1,O=ZeroDDS".to_string(),
        });
        assert_eq!(t.class_id, class_ids::PKI_DH);
        assert_eq!(t.subject_name, "CN=Bridge-1,O=ZeroDDS");
        assert_eq!(t.certificate, Some(alloc::vec![1u8, 2, 3]));
    }

    #[test]
    fn scram_yields_scram_sha256_class_id() {
        let t = build_identity_token(&SaslSubject::ScramSha256 {
            authcid: "bob".into(),
        });
        assert_eq!(t.class_id, class_ids::SCRAM_SHA256);
        assert_eq!(t.subject_name, "CN=bob");
    }

    #[test]
    fn class_id_strings_match_spec_table() {
        // Spec §10.3.2-Tabelle: konkrete strings.
        assert_eq!(class_ids::SASL_USERNAME, "zerodds:Auth:SASL-Username:1.0");
        assert_eq!(class_ids::ANONYMOUS, "zerodds:Auth:Anonymous:1.0");
        assert_eq!(
            class_ids::SCRAM_SHA256,
            "zerodds:Auth:SASL-SCRAM-SHA256:1.0"
        );
        assert_eq!(class_ids::PKI_DH, "DDS:Auth:PKI-DH:1.0");
    }

    // --- AccessControl-Plugin ---

    #[test]
    fn allow_all_returns_allow() {
        let p = AllowAll;
        let id = build_identity_token(&SaslSubject::Plain {
            authcid: "x".into(),
        });
        assert_eq!(
            p.check(&id, "T", AccessOp::AttachSender),
            AccessDecision::Allow
        );
    }

    #[test]
    fn static_allow_list_per_op() {
        let mut p = StaticAllowList::new();
        let id = build_identity_token(&SaslSubject::Plain {
            authcid: "alice".into(),
        });
        p.allow("CN=alice", "Sensor", AccessOp::AttachSender);
        // Allowed op + addr + subject.
        assert_eq!(
            p.check(&id, "Sensor", AccessOp::AttachSender),
            AccessDecision::Allow
        );
        // Different op → deny.
        assert_eq!(
            p.check(&id, "Sensor", AccessOp::AttachReceiver),
            AccessDecision::Deny
        );
        // Different addr → deny.
        assert_eq!(
            p.check(&id, "OtherTopic", AccessOp::AttachSender),
            AccessDecision::Deny
        );
        // Unknown subject → deny.
        let id2 = build_identity_token(&SaslSubject::Plain {
            authcid: "eve".into(),
        });
        assert_eq!(
            p.check(&id2, "Sensor", AccessOp::AttachSender),
            AccessDecision::Deny
        );
    }

    // --- Governance ---

    #[test]
    fn governance_resolves_exact_match() {
        let mut g = GovernanceDocument::new();
        g.add_rule(GovernanceRule {
            topic_pattern: "Sensor".to_string(),
            enable_discovery: true,
            enable_liveliness: true,
            data_protection_kind: DataProtectionKind::SignOnly,
        });
        let r = g.resolve("Sensor").unwrap();
        assert_eq!(r.data_protection_kind, DataProtectionKind::SignOnly);
        assert!(g.resolve("Other").is_none());
    }

    #[test]
    fn governance_resolves_prefix_glob() {
        let mut g = GovernanceDocument::new();
        g.add_rule(GovernanceRule {
            topic_pattern: "Sensor*".to_string(),
            enable_discovery: true,
            enable_liveliness: true,
            data_protection_kind: DataProtectionKind::None,
        });
        assert!(g.resolve("SensorTemperature").is_some());
        assert!(g.resolve("Actuator").is_none());
    }

    #[test]
    fn governance_resolves_suffix_glob() {
        let mut g = GovernanceDocument::new();
        g.add_rule(GovernanceRule {
            topic_pattern: "*Cmd".to_string(),
            enable_discovery: false,
            enable_liveliness: false,
            data_protection_kind: DataProtectionKind::SignAndEncrypt,
        });
        assert!(g.resolve("MotorCmd").is_some());
        assert!(g.resolve("Status").is_none());
    }

    #[test]
    fn governance_wildcard_matches_all() {
        let mut g = GovernanceDocument::new();
        g.add_rule(GovernanceRule {
            topic_pattern: "*".to_string(),
            enable_discovery: true,
            enable_liveliness: true,
            data_protection_kind: DataProtectionKind::None,
        });
        assert!(g.resolve("Anything").is_some());
    }

    // --- LinkGovernance Cache (§10.7) ---

    #[test]
    fn link_governance_caches_decision() {
        let id = build_identity_token(&SaslSubject::Plain {
            authcid: "alice".into(),
        });
        let mut lg = LinkGovernance::new(id, "Sensor".to_string(), None);
        // Test-Plugin, das Calls zaehlt.
        struct Counting<'a> {
            count: &'a core::cell::Cell<u32>,
        }
        impl AccessControlPlugin for Counting<'_> {
            fn check(&self, _: &IdentityToken, _: &str, _: AccessOp) -> AccessDecision {
                self.count.set(self.count.get() + 1);
                AccessDecision::Allow
            }
        }
        let count = core::cell::Cell::new(0);
        let p = Counting { count: &count };
        // Erste Auswertung → Plugin-Call.
        assert_eq!(lg.evaluate(&p, AccessOp::SendSample), AccessDecision::Allow);
        assert_eq!(count.get(), 1);
        // Zweite Auswertung derselben Op → Cache-Hit, kein Plugin-Call.
        assert_eq!(lg.evaluate(&p, AccessOp::SendSample), AccessDecision::Allow);
        assert_eq!(count.get(), 1);
        // Andere Op → neuer Plugin-Call.
        assert_eq!(
            lg.evaluate(&p, AccessOp::ReceiveSample),
            AccessDecision::Allow
        );
        assert_eq!(count.get(), 2);
    }

    // --- Dual Identity (§10.6) ---

    #[test]
    fn dual_identity_keeps_broker_and_dds_separate() {
        let broker = build_identity_token(&SaslSubject::Plain {
            authcid: "Alice".into(),
        });
        let dds = build_identity_token(&SaslSubject::External {
            certificate: alloc::vec![0xCA],
            subject_dn: "CN=Bridge-1".to_string(),
        });
        let dual = DualIdentity::new(broker.clone(), dds.clone());
        assert_eq!(dual.for_dds().subject_name, "CN=Bridge-1");
        assert_eq!(dual.for_broker().subject_name, "CN=Alice");
        // Spec §10.6: broker-side SHALL NOT conflate; for_dds()
        // gibt nicht broker_identity zurueck.
        assert_ne!(dual.for_dds().subject_name, dual.for_broker().subject_name);
    }

    #[test]
    fn dual_identity_for_dds_does_not_carry_broker_credential() {
        // Annex C C.2.6 — Bridge praesentiert CN=Bridge-1
        // (nicht Alice) zu DDS-AccessControl.
        let broker = build_identity_token(&SaslSubject::Plain {
            authcid: "Alice".into(),
        });
        let dds = build_identity_token(&SaslSubject::External {
            certificate: alloc::vec![0x42],
            subject_dn: "CN=Bridge-1".to_string(),
        });
        let dual = DualIdentity::new(broker, dds);
        let ac = AllowAll;
        // Plugin-Call mit dual.for_dds() — der subject_name ist
        // CN=Bridge-1.
        assert_eq!(
            ac.check(dual.for_dds(), "X", AccessOp::AttachSender),
            AccessDecision::Allow
        );
        assert_eq!(dual.for_dds().subject_name, "CN=Bridge-1");
    }
}
