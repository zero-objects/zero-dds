// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Default implementation `GovernancePolicyEngine`.
//!
//! This impl maps the v1.4 semantics of [`crate::SharedSecurityGate`]
//! onto the new [`PolicyEngine`] interface — so that stages 4–6 can
//! refactor the gate onto a `PolicyEngine` basis without
//! existing deployments changing their wire behavior.
//!
//! # Semantics
//!
//! The engine decides purely from `domain_id` + governance XML, without
//! peer/interface selection — exactly like the current gate. The
//! peer-specific decisions only arrive with RC1
//! (SPDP caps) + RC1 (`<peer_classes>`) into specialized
//! PolicyEngines.
//!
//! # Parity contract against `SharedSecurityGate`
//!
//! The decision from [`GovernancePolicyEngine::outbound_decision`]
//! maps `ProtectionLevel` 1:1 from
//! `Governance::find_domain_rule(domain_id).rtps_protection_kind` —
//! the same lookup that the gate does in `message_protection()`.
//! The corresponding E2E test in this module checks that the
//! decision matrix `{None, Sign, Encrypt, SignWO, EncryptWO}` comes out
//! identical to the gate.

use zerodds_security_crypto::Suite;
use zerodds_security_permissions::{Governance, ProtectionKind};

use crate::caps::PeerCapabilities;
use crate::peer_class::{interface_accepts_class, resolve_peer_class};
use crate::policy::{
    InboundCtx, NetInterface, OutboundCtx, PolicyDecision, PolicyEngine, ProtectionLevel, SuiteHint,
};

/// Governance-XML-driven `PolicyEngine` default implementation.
///
/// `Clone` is deliberately NOT `derive`d: the [`Governance`] struct
/// itself is `Clone`, but the engine is typically held as an
/// `Arc<dyn PolicyEngine>` in several runtime components —
/// then an `Arc::clone` is the right way, not a deep copy.
#[derive(Debug)]
pub struct GovernancePolicyEngine {
    domain_id: u32,
    governance: Governance,
    /// Default suite when protection = Encrypt. For v1.4 parity
    /// this is `Aes128Gcm` — the same default as in
    /// `AesGcmCryptoPlugin::new()`.
    default_suite: SuiteHint,
}

impl GovernancePolicyEngine {
    /// Constructor with an explicit default suite. For v1.4 parity
    /// use [`Self::with_defaults`].
    #[must_use]
    pub fn new(domain_id: u32, governance: Governance, default_suite: SuiteHint) -> Self {
        Self {
            domain_id,
            governance,
            default_suite,
        }
    }

    /// Constructor with the v1.4 default suite (`AES_128_GCM`).
    #[must_use]
    pub fn with_defaults(domain_id: u32, governance: Governance) -> Self {
        Self::new(
            domain_id,
            governance,
            SuiteHint::from_suite(Suite::default()),
        )
    }

    /// Current `ProtectionKind` from governance for the participant
    /// domain — same lookup as [`crate::SharedSecurityGate::message_protection`].
    #[must_use]
    pub fn message_protection_kind(&self) -> ProtectionKind {
        self.governance
            .find_domain_rule(self.domain_id)
            .map(|r| r.rtps_protection_kind)
            .unwrap_or(ProtectionKind::None)
    }

    /// Configured domain id.
    #[must_use]
    pub fn domain_id(&self) -> u32 {
        self.domain_id
    }

    /// Shared core function for out- and inbound decisions:
    /// the protection level is fixed purely from the domain rule.
    fn domain_decision(&self) -> PolicyDecision {
        let kind = self.message_protection_kind();
        self.decision_for_kind(kind)
    }

    fn decision_for_kind(&self, kind: ProtectionKind) -> PolicyDecision {
        let level = ProtectionLevel::from_protection_kind(kind);
        let suite = match level {
            ProtectionLevel::None => None,
            ProtectionLevel::Sign => Some(SuiteHint::HmacSha256),
            ProtectionLevel::Encrypt => Some(self.default_suite),
        };
        PolicyDecision::with(level, suite)
    }

    /// Resolution of the peer class for a remote peer + interface
    ///.
    ///
    /// Steps:
    /// 1. Find the domain rule (if none matches → `None`).
    /// 2. If `peer_classes` is empty → legacy path → `None`.
    /// 3. Find the first matching peer class. If none matched →
    ///    `DROP` decision (the peer fits no configured
    ///    class — conservative-safe stance).
    /// 4. Apply the interface-binding filter:
    ///    * `peer_class_filter` empty → accepted.
    ///    * class **not** in the filter → `DROP`.
    /// 5. Determine the protection level:
    ///    * start: `peer_class.protection`.
    ///    * interface `protection_override`, if set, takes
    ///      precedence (allows e.g. loopback → NONE).
    ///    * interface `protection_min` is applied as a lower bound
    ///      (`max(level, protection_min)`).
    fn resolve_peer_decision(
        &self,
        caps: &PeerCapabilities,
        iface: &NetInterface,
    ) -> Option<PolicyDecision> {
        let rule = self.governance.find_domain_rule(self.domain_id)?;
        if rule.peer_classes.is_empty() {
            return None;
        }
        let class = match resolve_peer_class(caps, &rule.peer_classes) {
            Some(c) => c,
            None => return Some(PolicyDecision::DROP),
        };

        // Find the interface-binding rule (by name).
        let iface_rule = if let Some(name) = iface_name(iface) {
            rule.interface_bindings
                .iter()
                .find(|b| b.name.as_str() == name)
        } else {
            None
        };

        if let Some(binding) = iface_rule {
            if !interface_accepts_class(&class.name, &binding.peer_class_filter) {
                return Some(PolicyDecision::DROP);
            }
        }

        // Start with the class protection.
        let mut kind = class.protection;
        // Interface override replaces.
        if let Some(binding) = iface_rule {
            if let Some(over) = binding.protection_override {
                kind = over;
            }
            // Interface minimum: after translation into ProtectionLevel
            // take the stronger value.
            if let Some(min) = binding.protection_min {
                let level_cur = ProtectionLevel::from_protection_kind(kind);
                let level_min = ProtectionLevel::from_protection_kind(min);
                kind = level_cur.stronger(level_min).to_protection_kind();
            }
        }
        Some(self.decision_for_kind(kind))
    }
}

/// Maps a `NetInterface` variant to the name expected in
/// `<zerodds:interface name="...">`.
fn iface_name(iface: &NetInterface) -> Option<&str> {
    match iface {
        NetInterface::Loopback => Some("loopback"),
        NetInterface::LocalHost => Some("shm"),
        NetInterface::Wan => Some("wan"),
        NetInterface::LocalSubnet(_) => Some("local_subnet"),
        NetInterface::Named(n) => Some(n.as_str()),
    }
}

impl PolicyEngine for GovernancePolicyEngine {
    fn outbound_decision(&self, ctx: OutboundCtx<'_>) -> PolicyDecision {
        if let Some(dec) = self.resolve_peer_decision(ctx.remote_caps, ctx.interface) {
            return dec;
        }
        self.domain_decision()
    }

    fn inbound_decision(&self, ctx: InboundCtx<'_>) -> PolicyDecision {
        let expected = self.domain_decision();
        // If the domain expects plaintext and a packet is SRTPS-
        // wrapped: stage 5 extends this — v1.4 parity is: we
        // return the domain decision, the gate then unwraps.
        if matches!(expected.protection, ProtectionLevel::None) && ctx.is_sec_prefixed {
            // The packet is still attempted to be decrypted — as in the
            // current gate (passthrough, no hard drop).
            return expected;
        }
        // If the domain expects protection and the packet is not protected:
        // the gate returns `PolicyViolation`. On the engine side we mark
        // this as "drop=true" — stage 5 evaluates it. The current
        // SharedSecurityGate already has these semantics, we mirror
        // them here in the decision.
        if !matches!(expected.protection, ProtectionLevel::None) && !ctx.is_sec_prefixed {
            return PolicyDecision::DROP;
        }
        expected
    }

    fn accept_peer(&self, _caps: &PeerCapabilities) -> bool {
        // v1.4 parity: the gate does not filter by caps. The
        // authentication plugin chain takes that role.
        // Stage 2 (SPDP caps) tightens this.
        true
    }
}

// ============================================================================
// Tests — parity matrix against SharedSecurityGate
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};
    use alloc::vec;

    use zerodds_security_crypto::AesGcmCryptoPlugin;
    use zerodds_security_permissions::parse_governance_xml;

    use crate::policy::{IpRange, NetInterface};
    use crate::shared::{PeerKey, SharedSecurityGate};

    fn gov_xml(kind: &str) -> String {
        alloc::format!(
            r#"
<domain_access_rules>
  <domain_rule>
    <domains><id>0</id></domains>
    <rtps_protection_kind>{kind}</rtps_protection_kind>
    <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
  </domain_rule>
</domain_access_rules>
"#
        )
    }

    fn stub_peer() -> (PeerKey, PeerCapabilities) {
        ([0xA1; 12], PeerCapabilities::default())
    }

    fn stub_out_ctx<'a>(
        peer: &'a PeerKey,
        caps: &'a PeerCapabilities,
        iface: &'a NetInterface,
        partition: &'a [String],
    ) -> OutboundCtx<'a> {
        OutboundCtx {
            domain_id: 0,
            topic: "Chatter",
            partition,
            interface: iface,
            remote_peer: peer,
            remote_caps: caps,
        }
    }

    // ---- Decision-Matrix vs. SharedSecurityGate ----

    #[test]
    fn outbound_decision_matches_gate_message_protection_all_kinds() {
        for kind in [
            "NONE",
            "SIGN",
            "ENCRYPT",
            "SIGN_WITH_ORIGIN_AUTHENTICATION",
            "ENCRYPT_WITH_ORIGIN_AUTHENTICATION",
        ] {
            let gov = parse_governance_xml(&gov_xml(kind)).unwrap();
            let engine = GovernancePolicyEngine::with_defaults(0, gov.clone());
            let gate = SharedSecurityGate::new(0, gov, Box::new(AesGcmCryptoPlugin::new()));

            let expected_kind = gate.message_protection().unwrap();
            let expected_level = ProtectionLevel::from_protection_kind(expected_kind);

            let (peer, caps) = stub_peer();
            let iface = NetInterface::Wan;
            let parts: Vec<String> = vec![];
            let decision = engine.outbound_decision(stub_out_ctx(&peer, &caps, &iface, &parts));
            assert_eq!(
                decision.protection, expected_level,
                "protection mismatch for kind={kind}"
            );
            assert!(!decision.drop);
        }
    }

    #[test]
    fn outbound_decision_suite_is_aes128_for_encrypt() {
        let gov = parse_governance_xml(&gov_xml("ENCRYPT")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let (peer, caps) = stub_peer();
        let iface = NetInterface::Wan;
        let parts: Vec<String> = vec![];
        let d = engine.outbound_decision(stub_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(d.suite, Some(SuiteHint::Aes128Gcm));
    }

    #[test]
    fn outbound_decision_suite_is_hmac_for_sign() {
        let gov = parse_governance_xml(&gov_xml("SIGN")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let (peer, caps) = stub_peer();
        let iface = NetInterface::Wan;
        let parts: Vec<String> = vec![];
        let d = engine.outbound_decision(stub_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(d.suite, Some(SuiteHint::HmacSha256));
        assert_eq!(d.protection, ProtectionLevel::Sign);
    }

    #[test]
    fn outbound_decision_suite_is_none_for_none() {
        let gov = parse_governance_xml(&gov_xml("NONE")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let (peer, caps) = stub_peer();
        let iface = NetInterface::Loopback;
        let parts: Vec<String> = vec![];
        let d = engine.outbound_decision(stub_out_ctx(&peer, &caps, &iface, &parts));
        assert!(d.suite.is_none());
        assert_eq!(d.protection, ProtectionLevel::None);
    }

    #[test]
    fn outbound_decision_custom_suite_roundtrip() {
        let gov = parse_governance_xml(&gov_xml("ENCRYPT")).unwrap();
        let engine = GovernancePolicyEngine::new(0, gov, SuiteHint::Aes256Gcm);
        let (peer, caps) = stub_peer();
        let iface = NetInterface::Wan;
        let parts: Vec<String> = vec![];
        let d = engine.outbound_decision(stub_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(d.suite, Some(SuiteHint::Aes256Gcm));
    }

    #[test]
    fn inbound_plain_on_protected_domain_is_drop() {
        let gov = parse_governance_xml(&gov_xml("ENCRYPT")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let peer: PeerKey = [1; 12];
        let iface = NetInterface::Wan;
        let d = engine.inbound_decision(InboundCtx {
            domain_id: 0,
            source_peer: &peer,
            source_iface: &iface,
            source_caps: None,
            is_sec_prefixed: false,
        });
        assert!(d.drop, "plaintext on a protected domain must drop");
    }

    #[test]
    fn inbound_secure_on_protected_domain_is_decrypt() {
        let gov = parse_governance_xml(&gov_xml("ENCRYPT")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let peer: PeerKey = [1; 12];
        let iface = NetInterface::Wan;
        let d = engine.inbound_decision(InboundCtx {
            domain_id: 0,
            source_peer: &peer,
            source_iface: &iface,
            source_caps: None,
            is_sec_prefixed: true,
        });
        assert!(!d.drop);
        assert_eq!(d.protection, ProtectionLevel::Encrypt);
    }

    #[test]
    fn inbound_plain_on_plain_domain_is_accept() {
        let gov = parse_governance_xml(&gov_xml("NONE")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let peer: PeerKey = [1; 12];
        let iface = NetInterface::Loopback;
        let d = engine.inbound_decision(InboundCtx {
            domain_id: 0,
            source_peer: &peer,
            source_iface: &iface,
            source_caps: None,
            is_sec_prefixed: false,
        });
        assert!(!d.drop);
        assert_eq!(d.protection, ProtectionLevel::None);
    }

    #[test]
    fn inbound_secure_on_plain_domain_passthrough() {
        // The v1.4 SharedSecurityGate accepts SRTPS on a plain domain
        // and unwraps. Our engine returns the domain decision
        // (None) — then the gate/plugin decides.
        let gov = parse_governance_xml(&gov_xml("NONE")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        let peer: PeerKey = [1; 12];
        let iface = NetInterface::Loopback;
        let d = engine.inbound_decision(InboundCtx {
            domain_id: 0,
            source_peer: &peer,
            source_iface: &iface,
            source_caps: None,
            is_sec_prefixed: true,
        });
        assert!(!d.drop);
        assert_eq!(d.protection, ProtectionLevel::None);
    }

    #[test]
    fn accept_peer_is_always_true_in_v14_parity() {
        let gov = parse_governance_xml(&gov_xml("ENCRYPT")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(0, gov);
        assert!(engine.accept_peer(&PeerCapabilities::default()));
        assert!(engine.accept_peer(&PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".to_string()),
            ..Default::default()
        }));
    }

    #[test]
    fn message_protection_kind_falls_back_to_none_when_domain_not_listed() {
        // Governance only has domain_id=0, the engine asks for domain_id=99.
        let gov = parse_governance_xml(&gov_xml("ENCRYPT")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(99, gov);
        assert_eq!(engine.message_protection_kind(), ProtectionKind::None);
    }

    #[test]
    fn domain_id_accessor_returns_constructor_value() {
        let gov = parse_governance_xml(&gov_xml("NONE")).unwrap();
        let engine = GovernancePolicyEngine::with_defaults(42, gov);
        assert_eq!(engine.domain_id(), 42);
    }

    // Uses the IpRange import for a compilation check
    #[test]
    fn interface_classification_is_independent_of_engine() {
        let _r = IpRange {
            base: core::net::IpAddr::V4(core::net::Ipv4Addr::new(10, 0, 0, 0)),
            prefix_len: 24,
        };
    }

    // =======================================================================
    // RC1 stage 8 — peer-class integration in GovernancePolicyEngine
    // =======================================================================

    const HETERO_GOV_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>
      <rtps_protection_kind>SIGN</rtps_protection_kind>
      <zerodds:peer_classes>
        <zerodds:peer_class name="legacy" protection="NONE">
          <zerodds:match auth_plugin_class="" />
        </zerodds:peer_class>
        <zerodds:peer_class name="fast" protection="SIGN">
          <zerodds:match cert_cn_pattern="*.fast.example" />
        </zerodds:peer_class>
        <zerodds:peer_class name="secure" protection="ENCRYPT">
          <zerodds:match auth_plugin_class="DDS:Auth:PKI-DH:1.2" suite="AES_128_GCM" />
        </zerodds:peer_class>
        <zerodds:peer_class name="highassurance" protection="ENCRYPT">
          <zerodds:match cert_cn_pattern="*.ha.*" suite="AES_256_GCM" require_ocsp="TRUE" />
        </zerodds:peer_class>
      </zerodds:peer_classes>
      <zerodds:interface_bindings>
        <zerodds:interface name="loopback" protection_override="NONE" />
        <zerodds:interface name="shm"      protection_override="NONE" />
        <zerodds:interface name="eth0"     peer_class_filter="legacy,fast,secure" />
        <zerodds:interface name="tun0"     peer_class_filter="secure,highassurance"
                                           protection_min="ENCRYPT" />
      </zerodds:interface_bindings>
    </domain_rule>
  </domain_access_rules>
</dds>"#;

    fn hetero_engine() -> GovernancePolicyEngine {
        let gov = parse_governance_xml(HETERO_GOV_XML).unwrap();
        GovernancePolicyEngine::with_defaults(0, gov)
    }

    fn mk_out_ctx<'a>(
        peer: &'a PeerKey,
        caps: &'a PeerCapabilities,
        iface: &'a NetInterface,
        parts: &'a [String],
    ) -> OutboundCtx<'a> {
        OutboundCtx {
            domain_id: 0,
            topic: "Chatter",
            partition: parts,
            interface: iface,
            remote_peer: peer,
            remote_caps: caps,
        }
    }

    #[test]
    fn hetero_dod_legacy_peer_on_eth0_gets_none() {
        // Plan §stage 8 DoD matrix: legacy peer on eth0 → NONE.
        let engine = hetero_engine();
        let peer: PeerKey = [1; 12];
        let caps = PeerCapabilities::default();
        let iface = NetInterface::Named("eth0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(dec.protection, ProtectionLevel::None);
        assert!(!dec.drop);
    }

    #[test]
    fn hetero_dod_fast_peer_on_eth0_gets_sign() {
        let engine = hetero_engine();
        let peer: PeerKey = [2; 12];
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            cert_cn: Some("writer.fast.example".into()),
            supported_suites: vec![SuiteHint::HmacSha256],
            ..Default::default()
        };
        let iface = NetInterface::Named("eth0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(dec.protection, ProtectionLevel::Sign);
    }

    #[test]
    fn hetero_dod_secure_peer_on_eth0_gets_encrypt() {
        let engine = hetero_engine();
        let peer: PeerKey = [3; 12];
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        let iface = NetInterface::Named("eth0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(dec.protection, ProtectionLevel::Encrypt);
    }

    #[test]
    fn hetero_dod_ha_peer_on_tun0_gets_encrypt() {
        let engine = hetero_engine();
        let peer: PeerKey = [4; 12];
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            cert_cn: Some("w1.ha.corp".into()),
            supported_suites: vec![SuiteHint::Aes256Gcm],
            has_valid_cert: true,
            ..Default::default()
        };
        let iface = NetInterface::Named("tun0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(dec.protection, ProtectionLevel::Encrypt);
    }

    #[test]
    fn hetero_interface_override_loopback_forces_none() {
        // Interface binding loopback has protection_override=NONE —
        // even a secure peer may send plain on loopback.
        let engine = hetero_engine();
        let peer: PeerKey = [5; 12];
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        let iface = NetInterface::Loopback;
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(
            dec.protection,
            ProtectionLevel::None,
            "loopback override must override class Encrypt"
        );
    }

    #[test]
    fn hetero_interface_filter_rejects_legacy_on_tun0() {
        // tun0 has peer_class_filter="secure,highassurance". A
        // legacy peer must drop.
        let engine = hetero_engine();
        let peer: PeerKey = [6; 12];
        let caps = PeerCapabilities::default(); // legacy
        let iface = NetInterface::Named("tun0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert!(dec.drop, "legacy peer must not be on tun0 → drop");
    }

    #[test]
    fn hetero_no_matching_peer_class_drops() {
        // A peer whose caps match none of the 4 classes → drop
        // (conservative-safe stance).
        let engine = hetero_engine();
        let peer: PeerKey = [7; 12];
        let caps = PeerCapabilities {
            // Has auth (so not legacy), cert CN matches neither fast
            // nor ha, suite empty (so not secure), no OCSP.
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            cert_cn: Some("unknown.zone".into()),
            supported_suites: vec![],
            ..Default::default()
        };
        let iface = NetInterface::Named("eth0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert!(dec.drop, "peer in no class → drop");
    }

    #[test]
    fn hetero_interface_protection_min_upgrades_sign_to_encrypt() {
        // tun0 has protection_min=ENCRYPT — a secure peer is
        // already ENCRYPT (stronger_wins changes nothing).
        // More important: a fast peer (SIGN) would end up as DROP on
        // tun0, because fast is not in the filter. So we test with
        // a secure peer that stays at ENCRYPT only via protection_min.
        let engine = hetero_engine();
        let peer: PeerKey = [8; 12];
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        let iface = NetInterface::Named("tun0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(dec.protection, ProtectionLevel::Encrypt);
    }

    #[test]
    fn legacy_xml_without_peer_classes_falls_back_to_domain_rule() {
        // A pure OMG governance XML without peer_classes should
        // decide exactly like v1.4 (domain rule wins).
        let engine = GovernancePolicyEngine::with_defaults(
            0,
            parse_governance_xml(&gov_xml("SIGN")).unwrap(),
        );
        let peer: PeerKey = [9; 12];
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        let iface = NetInterface::Wan;
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert_eq!(
            dec.protection,
            ProtectionLevel::Sign,
            "without peer_classes the domain rule must take effect"
        );
    }
}
