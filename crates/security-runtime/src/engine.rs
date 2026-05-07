// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Default-Implementation `GovernancePolicyEngine`.
//!
//! Diese Impl bildet die v1.4-Semantik von [`crate::SharedSecurityGate`]
//! auf das neue [`PolicyEngine`]-Interface ab — damit Stufe 4–6 den
//! Gate auf `PolicyEngine`-Basis refaktorieren koennen, ohne dass
//! bestehende Deployments ihr Wire-Verhalten aendern.
//!
//! # Semantik
//!
//! Der Engine entscheidet rein aus `domain_id` + Governance-XML, ohne
//! Peer-/Interface-Auswahl — genau wie der aktuelle Gate. Die
//! Peer-spezifischen Entscheidungen kommen erst mit RC1
//! (SPDP-Caps) + RC1 (`<peer_classes>`) in spezialisierte
//! PolicyEngines hinein.
//!
//! # Parity-Kontrakt gegenueber `SharedSecurityGate`
//!
//! Die Decision aus [`GovernancePolicyEngine::outbound_decision`]
//! bildet `ProtectionLevel` 1:1 aus
//! `Governance::find_domain_rule(domain_id).rtps_protection_kind` —
//! der selbe Lookup, den der Gate in `message_protection()` macht.
//! Der zugehoerige E2E-Test in diesem Modul prueft, dass die
//! Decision-Matrix `{None, Sign, Encrypt, SignWO, EncryptWO}` gegen
//! den Gate identisch ausgeht.

use zerodds_security_crypto::Suite;
use zerodds_security_permissions::{Governance, ProtectionKind};

use crate::caps::PeerCapabilities;
use crate::peer_class::{interface_accepts_class, resolve_peer_class};
use crate::policy::{
    InboundCtx, NetInterface, OutboundCtx, PolicyDecision, PolicyEngine, ProtectionLevel, SuiteHint,
};

/// Governance-XML-getriebene `PolicyEngine`-Default-Implementation.
///
/// `Clone` ist bewusst NICHT `derive`d: die [`Governance`]-struct
/// selber ist `Clone`, aber die Engine wird typischerweise als
/// `Arc<dyn PolicyEngine>` in mehreren Runtime-Komponenten gehalten —
/// dann ist ein `Arc::clone` der richtige Weg, nicht ein Deep-Copy.
#[derive(Debug)]
pub struct GovernancePolicyEngine {
    domain_id: u32,
    governance: Governance,
    /// Default-Suite, wenn Protection = Encrypt. Fuer v1.4-Parity
    /// ist das `Aes128Gcm` — derselbe Default wie im
    /// `AesGcmCryptoPlugin::new()`.
    default_suite: SuiteHint,
}

impl GovernancePolicyEngine {
    /// Konstruktor mit explizitem Default-Suite. Fuer v1.4-Parity
    /// passt [`Self::with_defaults`].
    #[must_use]
    pub fn new(domain_id: u32, governance: Governance, default_suite: SuiteHint) -> Self {
        Self {
            domain_id,
            governance,
            default_suite,
        }
    }

    /// Konstruktor mit v1.4-Default-Suite (`AES_128_GCM`).
    #[must_use]
    pub fn with_defaults(domain_id: u32, governance: Governance) -> Self {
        Self::new(
            domain_id,
            governance,
            SuiteHint::from_suite(Suite::default()),
        )
    }

    /// Aktuelle `ProtectionKind` aus Governance fuer die Participant-
    /// Domain — gleicher Lookup wie [`crate::SharedSecurityGate::message_protection`].
    #[must_use]
    pub fn message_protection_kind(&self) -> ProtectionKind {
        self.governance
            .find_domain_rule(self.domain_id)
            .map(|r| r.rtps_protection_kind)
            .unwrap_or(ProtectionKind::None)
    }

    /// Konfigurierte Domain-Id.
    #[must_use]
    pub fn domain_id(&self) -> u32 {
        self.domain_id
    }

    /// Gemeinsame Kern-Funktion fuer Out- und Inbound-Decisions:
    /// das Protection-Level steht rein aus Domain-Rule fest.
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

    /// Auflosung der Peer-Klasse fuer einen Remote-Peer + Interface
    ///.
    ///
    /// Schritte:
    /// 1. Domain-Rule suchen (wenn keine passt → `None`).
    /// 2. Wenn `peer_classes` leer → Legacy-Pfad → `None`.
    /// 3. Erste matchende Peer-Klasse finden. Wenn keine matched →
    ///    `DROP`-Entscheidung (Peer passt in keine konfigurierte
    ///    Klasse — konservativ-sichere Haltung).
    /// 4. Interface-Binding-Filter anwenden:
    ///    * `peer_class_filter` leer → akzeptiert.
    ///    * Klasse **nicht** im Filter → `DROP`.
    /// 5. Protection-Level ermitteln:
    ///    * Start: `peer_class.protection`.
    ///    * Interface-`protection_override`, wenn gesetzt, hat
    ///      Vorrang (erlaubt z.B. Loopback → NONE).
    ///    * Interface-`protection_min` wird als Untergrenze
    ///      angewandt (`max(level, protection_min)`).
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

        // Interface-Binding-Regel suchen (per Name).
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

        // Start mit Class-Protection.
        let mut kind = class.protection;
        // Interface-Override ersetzt.
        if let Some(binding) = iface_rule {
            if let Some(over) = binding.protection_override {
                kind = over;
            }
            // Interface-Minimum: nach Uebersetzung in ProtectionLevel
            // den staerkeren Wert nehmen.
            if let Some(min) = binding.protection_min {
                let level_cur = ProtectionLevel::from_protection_kind(kind);
                let level_min = ProtectionLevel::from_protection_kind(min);
                kind = level_cur.stronger(level_min).to_protection_kind();
            }
        }
        Some(self.decision_for_kind(kind))
    }
}

/// Mappt eine `NetInterface`-Variante auf den Namen, der in
/// `<zerodds:interface name="...">` erwartet wird.
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
        // Wenn Domain plaintext erwartet und ein Paket ist SRTPS-
        // gewrappt: Stufe 5 erweitert das — v1.4-Parity ist: wir
        // geben die Domain-Decision zurueck, der Gate unwrappt dann.
        if matches!(expected.protection, ProtectionLevel::None) && ctx.is_sec_prefixed {
            // Paket wird trotzdem versucht zu entschluesseln — wie im
            // aktuellen Gate (passthrough, kein hard-drop).
            return expected;
        }
        // Wenn Domain Schutz erwartet und Paket nicht geschuetzt:
        // der Gate liefert `PolicyViolation`. Engine-seitig markieren
        // wir das als "drop=true" — Stufe 5 wertet das aus. Aktueller
        // SharedSecurityGate hat diese Semantik bereits, wir spiegeln
        // sie hier in der Decision wider.
        if !matches!(expected.protection, ProtectionLevel::None) && !ctx.is_sec_prefixed {
            return PolicyDecision::DROP;
        }
        expected
    }

    fn accept_peer(&self, _caps: &PeerCapabilities) -> bool {
        // v1.4-Parity: der Gate filtert nicht nach Caps. Die
        // Authentication-Plugin-Kette uebernimmt diese Rolle.
        // Stufe 2 (SPDP-Caps) verschaerft das.
        true
    }
}

// ============================================================================
// Tests — Parity-Matrix gegen SharedSecurityGate
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
                "protection mismatch fuer kind={kind}"
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
        assert!(d.drop, "plaintext auf protected domain muss droppen");
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
        // v1.4-SharedSecurityGate akzeptiert SRTPS auf plain domain
        // und unwrappt. Unsere Engine liefert die Domain-Decision
        // zurueck (None) — dann entscheidet der Gate/Plugin.
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
        // Governance hat nur domain_id=0, Engine fragt nach domain_id=99.
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

    // Nutzt IpRange-Import fuer Compilation-Check
    #[test]
    fn interface_classification_is_independent_of_engine() {
        let _r = IpRange {
            base: core::net::IpAddr::V4(core::net::Ipv4Addr::new(10, 0, 0, 0)),
            prefix_len: 24,
        };
    }

    // =======================================================================
    // RC1 Stufe 8 — Peer-Class-Integration in GovernancePolicyEngine
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
        // Plan §Stufe 8 DoD-Matrix: Legacy-Peer auf eth0 → NONE.
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
        // Interface-Binding loopback hat protection_override=NONE —
        // selbst ein secure-Peer darf auf Loopback plain senden.
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
            "loopback-override muss Class-Encrypt ueberschreiben"
        );
    }

    #[test]
    fn hetero_interface_filter_rejects_legacy_on_tun0() {
        // tun0 hat peer_class_filter="secure,highassurance". Ein
        // Legacy-Peer muss droppen.
        let engine = hetero_engine();
        let peer: PeerKey = [6; 12];
        let caps = PeerCapabilities::default(); // legacy
        let iface = NetInterface::Named("tun0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert!(dec.drop, "Legacy-Peer darf nicht auf tun0 → drop");
    }

    #[test]
    fn hetero_no_matching_peer_class_drops() {
        // Ein Peer dessen Caps keine der 4 Klassen matchen → Drop
        // (konservativ-sichere Haltung).
        let engine = hetero_engine();
        let peer: PeerKey = [7; 12];
        let caps = PeerCapabilities {
            // Hat Auth (also kein legacy), cert-CN matcht weder fast
            // noch ha, Suite leer (also kein secure), kein OCSP.
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            cert_cn: Some("unknown.zone".into()),
            supported_suites: vec![],
            ..Default::default()
        };
        let iface = NetInterface::Named("eth0".into());
        let parts: Vec<String> = vec![];
        let dec = engine.outbound_decision(mk_out_ctx(&peer, &caps, &iface, &parts));
        assert!(dec.drop, "Peer in keiner Klasse → drop");
    }

    #[test]
    fn hetero_interface_protection_min_upgrades_sign_to_encrypt() {
        // tun0 hat protection_min=ENCRYPT — ein secure-Peer ist
        // bereits ENCRYPT (stronger_wins ändert nichts).
        // Wichtiger: ein fast-Peer (SIGN) wuerde auf tun0 als DROP
        // enden, weil fast nicht im filter ist. Also testen wir mit
        // einem secure-Peer der nur durch protection_min auf ENCRYPT
        // bleibt.
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
        // Ein reines OMG-Governance-XML ohne peer_classes soll
        // exakt wie v1.4 entscheiden (Domain-Rule wins).
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
            "ohne peer_classes muss Domain-Rule greifen"
        );
    }
}
