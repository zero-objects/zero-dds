// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Security bridge for the AMQP endpoint.
//!
//! Spec sources:
//! * dds-amqp-1.0 §10.3.2 — IdentityToken class_id table per
//!   SASL mechanism (`zerodds:Auth:SASL-Username:1.0`,
//!   `zerodds:Auth:Anonymous:1.0`,
//!   `zerodds:Auth:SASL-SCRAM-SHA256:1.0`,
//!   `DDS:Auth:PKI-DH:1.0` only for EXTERNAL+X.509).
//! * §10.3.3 — permission evaluation against the AccessControl plugin.
//! * §10.3.5 — no-bypass guarantee.
//! * §10.4 — governance document mapping.
//! * §10.6 — Bridge Profile dual identity.
//! * §10.7 — per-link governance resolution.
//!
//! This layer does not know the DDS-Security plugin directly;
//! it provides trait-based adapters that the endpoint
//! daemon (or a test mock) binds against.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::sasl::SaslMechanism;

// ============================================================
// IdentityToken (§10.3.2)
// ============================================================

/// Spec §10.3.2 — class_id constants per SASL mechanism.
pub mod class_ids {
    /// PLAIN → vendor-prefixed username token.
    pub const SASL_USERNAME: &str = "zerodds:Auth:SASL-Username:1.0";
    /// ANONYMOUS → vendor-prefixed anonymous token.
    pub const ANONYMOUS: &str = "zerodds:Auth:Anonymous:1.0";
    /// SCRAM-SHA-256 → vendor-prefixed SCRAM token.
    pub const SCRAM_SHA256: &str = "zerodds:Auth:SASL-SCRAM-SHA256:1.0";
    /// EXTERNAL (mTLS X.509) → OMG-PKI-DH:1.0 (only this form is
    /// PKI-DH-compliant: it contains `certificate`).
    pub const PKI_DH: &str = "DDS:Auth:PKI-DH:1.0";
}

/// Spec §10.3.2 — IdentityToken (subset). DDS-Security 1.2 §8.4.1
/// specifies further fields (binary properties, etc.); for the
/// AMQP mapping, subject_name, certificate, and class_id suffice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityToken {
    /// Spec §10.3.2 — `class_id` from [`class_ids`].
    pub class_id: String,
    /// Spec §10.3.2 — `subject_name`, RFC 4514 DN form
    /// (`CN=alice` etc.).
    pub subject_name: String,
    /// Spec §10.3.2 — X.509 cert (DER bytes), set only for
    /// EXTERNAL/PKI-DH.
    pub certificate: Option<Vec<u8>>,
}

/// Spec §10.3.2 — build the IdentityToken from the SASL outcome.
///
/// The table maps:
/// * `Plain(authcid)` → `SASL-Username:1.0`, `subject_name = "CN=" + authcid`,
///   `certificate = None`.
/// * `Anonymous` → `Anonymous:1.0`, `subject_name = "CN=ANONYMOUS"`,
///   `certificate = None`.
/// * `External(cert_der, dn)` → `PKI-DH:1.0`, `subject_name = dn`
///   (as reported by the transport), `certificate = Some(cert_der)`.
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

/// Extended SASL outcome carrying the data needed for
/// IdentityToken construction.
#[derive(Debug, Clone)]
pub enum SaslSubject {
    /// SASL-PLAIN authenticated.
    Plain {
        /// Authenticated username.
        authcid: String,
    },
    /// SASL-ANONYMOUS.
    Anonymous,
    /// SASL-EXTERNAL with an X.509 cert from the transport.
    External {
        /// X.509 DER cert bytes.
        certificate: Vec<u8>,
        /// Subject DN from the cert (RFC 4514).
        subject_dn: String,
    },
    /// SASL-SCRAM-SHA-256.
    ScramSha256 {
        /// Authenticated username.
        authcid: String,
    },
}

impl SaslSubject {
    /// Which SASL mechanism is present?
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

/// Spec §10.3.3 — AccessControl plugin operation that the
/// endpoint invokes before every link attach + every pre-transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessOp {
    /// Attach a sender link (corresponds to
    /// `check_create_datawriter`).
    AttachSender,
    /// Attach a receiver link (corresponds to
    /// `check_create_datareader`).
    AttachReceiver,
    /// Send a sample (pre-transfer hook for no-bypass §10.3.5).
    SendSample,
    /// Receive a sample (receiver-side pre-decode hook).
    ReceiveSample,
}

/// Spec §10.3.3 — plugin result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    /// Operation may be performed.
    Allow,
    /// Operation rejected → AMQP `amqp:unauthorized-access`.
    Deny,
}

/// Spec §10.3.3 — AccessControl plugin trait.
///
/// `check` is invoked with the already-validated `IdentityToken`,
/// the address topic, and the operation. The plugin
/// decides `Allow`/`Deny`. The plugin SHALL be deterministic
/// (Spec §10.3.4) — same input, same answer.
pub trait AccessControlPlugin {
    /// Permission check.
    fn check(&self, identity: &IdentityToken, address: &str, op: AccessOp) -> AccessDecision;
}

/// Allow-all plugin (test default; **not** for production).
#[derive(Debug, Default)]
pub struct AllowAll;

impl AccessControlPlugin for AllowAll {
    fn check(&self, _identity: &IdentityToken, _address: &str, _op: AccessOp) -> AccessDecision {
        AccessDecision::Allow
    }
}

/// Static allow-list plugin (by `subject_name` match).
///
/// The plugin denies everything not listed in `allow`.
/// Pattern matching is exact; wildcards would be a plugin-specific
/// feature.
#[derive(Debug, Default)]
pub struct StaticAllowList {
    allow: BTreeMap<String, Vec<(String, AccessOp)>>,
}

impl StaticAllowList {
    /// Fresh list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry: `subject_name` may perform `op` on `address`.
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

/// Spec §10.4 — domain governance rule (subset).
/// A real DDS-Security implementation reads this from the
/// XML governance document; here we provide the data structure
/// that the XML loader (a separate crate) populates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceRule {
    /// DDS topic name or glob pattern (`Sensor*`).
    pub topic_pattern: String,
    /// SHALL the topic be discoverable at all? (`enable_discovery`).
    pub enable_discovery: bool,
    /// SHALL liveliness be signaled? (`enable_liveliness`).
    pub enable_liveliness: bool,
    /// Encryption mode.
    pub data_protection_kind: DataProtectionKind,
}

/// Spec §10.4 — `data_protection_kind` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataProtectionKind {
    /// No sample encryption (wire-clear body).
    None,
    /// Sign but do not encrypt.
    SignOnly,
    /// Sign + encrypt.
    SignAndEncrypt,
}

/// Spec §10.4 — governance document.
#[derive(Debug, Default)]
pub struct GovernanceDocument {
    rules: Vec<GovernanceRule>,
}

impl GovernanceDocument {
    /// Fresh empty document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule (typically from the XML loader).
    pub fn add_rule(&mut self, rule: GovernanceRule) {
        self.rules.push(rule);
    }

    /// Return the first rule whose `topic_pattern` matches.
    /// Pattern `*` matches everything, `prefix*` is a prefix match,
    /// `*suffix` is a suffix match, everything else is an exact match.
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

/// Spec §10.7 — per-link resolved governance + permission.
#[derive(Debug, Clone)]
pub struct LinkGovernance {
    /// Identity against which per-link authorization was performed.
    pub identity: IdentityToken,
    /// Address (topic) of the link terminus.
    pub address: String,
    /// Resolved governance rule (`None` if `default-rule`).
    pub rule: Option<GovernanceRule>,
    /// Permission cache: per operation `Allow`/`Deny`.
    pub cached_decisions: BTreeMap<AccessOp, AccessDecision>,
}

impl LinkGovernance {
    /// Fresh entry; the cache is empty and gets filled via
    /// `evaluate(plugin, op)`.
    #[must_use]
    pub fn new(identity: IdentityToken, address: String, rule: Option<GovernanceRule>) -> Self {
        Self {
            identity,
            address,
            rule,
            cached_decisions: BTreeMap::new(),
        }
    }

    /// Evaluate + cache the per-op permission.
    ///
    /// Spec §10.7 — permission is re-evaluated per link; once
    /// cached, the result for the same op does not change
    /// (determinism §10.3.4).
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

/// Spec §10.6 — dual-identity configuration of the bridge.
///
/// The broker-side SASL credential and the DDS-side IdentityToken
/// are kept strictly separate. The AccessControl plugin SHALL use
/// only the `dds_identity`.
#[derive(Debug, Clone)]
pub struct DualIdentity {
    /// Spec §10.6 — identity for broker authentication
    /// (e.g. `Alice` as the SASL username).
    pub broker_identity: IdentityToken,
    /// Spec §10.6 — identity presented in the DDS domain
    /// (e.g. `CN=Bridge-1`).
    pub dds_identity: IdentityToken,
}

impl DualIdentity {
    /// Constructor.
    #[must_use]
    pub fn new(broker_identity: IdentityToken, dds_identity: IdentityToken) -> Self {
        Self {
            broker_identity,
            dds_identity,
        }
    }

    /// Spec §10.6 — the identity relevant for DDS-Security calls.
    /// Plugin calls SHALL be given exclusively this one.
    #[must_use]
    pub fn for_dds(&self) -> &IdentityToken {
        &self.dds_identity
    }

    /// Identity for broker SASL negotiation.
    #[must_use]
    pub fn for_broker(&self) -> &IdentityToken {
        &self.broker_identity
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // --- IdentityToken builder ---

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
        // Spec §10.3.2 table: concrete strings.
        assert_eq!(class_ids::SASL_USERNAME, "zerodds:Auth:SASL-Username:1.0");
        assert_eq!(class_ids::ANONYMOUS, "zerodds:Auth:Anonymous:1.0");
        assert_eq!(
            class_ids::SCRAM_SHA256,
            "zerodds:Auth:SASL-SCRAM-SHA256:1.0"
        );
        assert_eq!(class_ids::PKI_DH, "DDS:Auth:PKI-DH:1.0");
    }

    // --- AccessControl plugin ---

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
        // Test plugin that counts calls.
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
        // First evaluation → plugin call.
        assert_eq!(lg.evaluate(&p, AccessOp::SendSample), AccessDecision::Allow);
        assert_eq!(count.get(), 1);
        // Second evaluation of the same op → cache hit, no plugin call.
        assert_eq!(lg.evaluate(&p, AccessOp::SendSample), AccessDecision::Allow);
        assert_eq!(count.get(), 1);
        // Different op → new plugin call.
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
        // does not return broker_identity.
        assert_ne!(dual.for_dds().subject_name, dual.for_broker().subject_name);
    }

    #[test]
    fn dual_identity_for_dds_does_not_carry_broker_credential() {
        // Annex C C.2.6 — the bridge presents CN=Bridge-1
        // (not Alice) to DDS AccessControl.
        let broker = build_identity_token(&SaslSubject::Plain {
            authcid: "Alice".into(),
        });
        let dds = build_identity_token(&SaslSubject::External {
            certificate: alloc::vec![0x42],
            subject_dn: "CN=Bridge-1".to_string(),
        });
        let dual = DualIdentity::new(broker, dds);
        let ac = AllowAll;
        // Plugin call with dual.for_dds() — the subject_name is
        // CN=Bridge-1.
        assert_eq!(
            ac.check(dual.for_dds(), "X", AccessOp::AttachSender),
            AccessDecision::Allow
        );
        assert_eq!(dual.for_dds().subject_name, "CN=Bridge-1");
    }
}
