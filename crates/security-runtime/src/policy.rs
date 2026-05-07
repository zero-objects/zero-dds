// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Heterogeneous-Security — `PolicyEngine`-Trait und Datentypen
//!.
//!
//! Diese Schicht ist die Abstraktion ueber dem Governance-XML-Stack.
//! Der v1.4-Stand ([`crate::SharedSecurityGate`]) entscheidet **ein**
//! Protection-Level pro Participant. System-of-Systems-Deployments
//! (Vehicle, Tactical, Edge) brauchen feinere Koernung auf der Tripel-
//! Achse `(peer, topic, interface)`.
//!
//! [`PolicyEngine`] kapselt diese Entscheidung:
//! * [`PolicyEngine::outbound_decision`] wird pro matched Reader
//!   aufgerufen, bevor ein Wire-Paket geschrieben wird.
//! * [`PolicyEngine::inbound_decision`] wird pro eingehendem Datagramm
//!   aufgerufen.
//! * [`PolicyEngine::accept_peer`] ist der Admission-Check waehrend
//!   SEDP-Matching.
//!
//! Die Default-Implementation ([`crate::GovernancePolicyEngine`], in
//! Stufe 1c) bildet die aktuelle `SharedSecurityGate`-Semantik 1:1 nach.
//! Nutzer koennen eigene `PolicyEngine`-Impls einstecken, z.B. um
//! Entscheidungen aus einem externen Policy-Server oder einer Vehicle-
//! Network-Certification-Datenbank zu beziehen.
//!
//! Siehe `docs/architecture/08_heterogeneous_security.md` §3.1.

use alloc::string::String;
use alloc::vec::Vec;

use core::net::IpAddr;

use zerodds_security_crypto::Suite;
use zerodds_security_permissions::ProtectionKind;

use crate::caps::PeerCapabilities;
use crate::shared::PeerKey;

// ============================================================================
// Grundtypen
// ============================================================================

/// Abstraktes Schutz-Level fuer die Policy-Ebene.
///
/// Im Gegensatz zu [`ProtectionKind`] (XML-Parser-Typ, 5 Varianten
/// inkl. Origin-Authentication) traegt `ProtectionLevel` nur die 3
/// Grund-Klassen. Policy-Entscheidungen vergleichen/ordnen diese
/// Stufen — die Origin-Auth-Verfeinerung folgt aus
/// [`PolicyDecision::suite`] (Receiver-Specific-MACs, RC1).
///
/// Die Reihenfolge `None < Sign < Encrypt` ist fuer das "staerkster
/// Wert gewinnt"-Matching in Stufe 3 (SEDP-Endpoint-Caps) relevant
/// — `Ord`/`PartialOrd` sind entsprechend definiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ProtectionLevel {
    /// Kein Schutz — plaintext-RTPS auf dem Wire.
    #[default]
    None,
    /// Integrity-Schutz (HMAC/AEAD-Tag), Payload bleibt lesbar.
    Sign,
    /// Integrity + Confidentiality (AEAD-Ciphertext).
    Encrypt,
}

impl ProtectionLevel {
    /// Mapping aus Governance-XML-[`ProtectionKind`]. Origin-Auth-
    /// Varianten kollabieren auf ihre Grund-Klasse; die Origin-Auth-
    /// Eigenschaft wird per [`PolicyDecision::suite`] +
    /// Receiver-Specific-MAC-Encoding transportiert.
    #[must_use]
    pub fn from_protection_kind(kind: ProtectionKind) -> Self {
        match kind {
            ProtectionKind::None => Self::None,
            ProtectionKind::Sign | ProtectionKind::SignWithOriginAuthentication => Self::Sign,
            ProtectionKind::Encrypt | ProtectionKind::EncryptWithOriginAuthentication => {
                Self::Encrypt
            }
        }
    }

    /// Rueck-Mapping auf [`ProtectionKind`] ohne Origin-Auth-Verfeinerung.
    #[must_use]
    pub fn to_protection_kind(self) -> ProtectionKind {
        match self {
            Self::None => ProtectionKind::None,
            Self::Sign => ProtectionKind::Sign,
            Self::Encrypt => ProtectionKind::Encrypt,
        }
    }

    /// Wahl des staerkeren von zwei Leveln (z.B. Writer-
    /// vs. Reader-Offer).
    #[must_use]
    pub fn stronger(self, other: Self) -> Self {
        if self >= other { self } else { other }
    }
}

/// Crypto-Suite-Hinweis fuer die Policy-Entscheidung.
///
/// `SuiteHint` ist ein **Wunsch** der Policy-Engine — das Crypto-
/// Plugin kann ihn ignorieren, wenn es den Algorithmus nicht
/// unterstuetzt. Das konkrete [`Suite`]-Enum (`security-crypto`)
/// ist der Plugin-interne Typ; diese Indirektion erlaubt zukuenftige
/// Suiten (ChaCha20-Poly1305, AES-CCM) ohne Breaking Change am
/// Policy-API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuiteHint {
    /// AES-128-GCM — Default-Suite v1.4.
    Aes128Gcm,
    /// AES-256-GCM — fuer Langzeit-Vertraulichkeit / Compliance.
    Aes256Gcm,
    /// HMAC-SHA256 Auth-only (keine Confidentiality, SIGN-Level).
    HmacSha256,
}

impl SuiteHint {
    /// Mapping auf das Plugin-interne [`Suite`].
    #[must_use]
    pub fn to_suite(self) -> Suite {
        match self {
            Self::Aes128Gcm => Suite::Aes128Gcm,
            Self::Aes256Gcm => Suite::Aes256Gcm,
            Self::HmacSha256 => Suite::HmacSha256,
        }
    }

    /// Rueck-Mapping aus [`Suite`].
    #[must_use]
    pub fn from_suite(suite: Suite) -> Self {
        match suite {
            Suite::Aes128Gcm => Self::Aes128Gcm,
            Suite::Aes256Gcm => Self::Aes256Gcm,
            Suite::HmacSha256 => Self::HmacSha256,
        }
    }

    /// Liefert das natuerliche Protection-Level dieser Suite:
    /// AEAD-Suiten → `Encrypt`, HMAC → `Sign`.
    #[must_use]
    pub fn protection_level(self) -> ProtectionLevel {
        match self {
            Self::Aes128Gcm | Self::Aes256Gcm => ProtectionLevel::Encrypt,
            Self::HmacSha256 => ProtectionLevel::Sign,
        }
    }
}

// ============================================================================
// NetInterface
// ============================================================================

/// CIDR-artige IPv4/IPv6-Range, inklusiv interpretiert.
///
/// Minimaler Selbstbau (kein `ipnet`-Dep, um den Safety-Footprint
/// klein zu halten). Praefix-Laenge in Host-Bits: IPv4 bis 32, IPv6
/// bis 128. Parsing (z.B. aus `"10.0.0.0/24"`) kommt in RC1
/// (Governance-XML); hier genuegt die struct-Form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IpRange {
    /// Basis-Adresse der Range (Host-Bits werden ignoriert).
    pub base: IpAddr,
    /// Praefix-Laenge in Bits. Muss `<= 32` fuer v4, `<= 128` fuer v6.
    pub prefix_len: u8,
}

impl IpRange {
    /// Prueft, ob `addr` in der Range liegt. Gemischte Familien
    /// (v4 in v6-Range) sind **kein** Match.
    #[must_use]
    pub fn contains(&self, addr: &IpAddr) -> bool {
        match (self.base, addr) {
            (IpAddr::V4(base), IpAddr::V4(a)) => {
                if self.prefix_len > 32 {
                    return false;
                }
                let shift = 32 - u32::from(self.prefix_len);
                let base_u = u32::from(base);
                let a_u = u32::from(*a);
                if self.prefix_len == 0 {
                    true
                } else {
                    (base_u >> shift) == (a_u >> shift)
                }
            }
            (IpAddr::V6(base), IpAddr::V6(a)) => {
                if self.prefix_len > 128 {
                    return false;
                }
                let base_u = u128::from(base);
                let a_u = u128::from(*a);
                if self.prefix_len == 0 {
                    true
                } else {
                    let shift = 128 - u32::from(self.prefix_len);
                    (base_u >> shift) == (a_u >> shift)
                }
            }
            _ => false,
        }
    }
}

/// Klassifikation eines Netz-Interfaces fuer die Policy-Entscheidung.
///
/// Die Engine kann darauf basierend unterschiedlich entscheiden:
/// * `Loopback` + `LocalHost` → oft `ProtectionLevel::None` (Bytes
///   verlassen den Host nicht).
/// * `LocalSubnet` → Management-Netze mit `Sign` statt `Encrypt`.
/// * `Wan` → restriktivste Policy.
/// * `Named` → benutzer-konfigurierte Klassifikation (z.B. `tun0`
///   als VPN-geschuetzt, `can0-gw` als Vehicle-Bus-Gateway).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetInterface {
    /// 127.0.0.0/8 oder `::1`.
    Loopback,
    /// Host-lokaler Transport ausserhalb von Loopback (Shared-Memory,
    /// Unix-Domain-Socket) — Bytes verlassen den Host-Kernel nicht.
    LocalHost,
    /// Adresse in einer konfigurierten privaten Range (z.B.
    /// `10.0.0.0/24`, `192.168.0.0/16`).
    LocalSubnet(IpRange),
    /// Alles andere — oeffentliche IP, unbekanntes Interface.
    Wan,
    /// Benutzer-konfigurierte Interface-Klasse per Name.
    Named(String),
}

/// Laufzeit-Konfiguration des Interface-Classifiers.
///
/// Wird vom Nutzer beim Participant-Start gebaut und an den
/// [`PolicyEngine`] uebergeben. Leere Konfiguration → jede nicht-
/// Loopback-Adresse landet in [`NetInterface::Wan`].
///
/// Die `named`-Liste erlaubt Muster wie "alle Adressen im Tun0-Subnet
/// sollen `NetInterface::Named("vpn".into())` werden". Die Liste
/// wird in Reihenfolge abgearbeitet — erstes Match gewinnt.
#[derive(Debug, Clone, Default)]
pub struct InterfaceConfig {
    /// Private Subnetze, die als [`NetInterface::LocalSubnet`]
    /// klassifiziert werden.
    pub local_subnets: Vec<IpRange>,
    /// Named-Interface-Zuordnungen: `(range, name)` — erstes Match
    /// liefert [`NetInterface::Named`].
    pub named: Vec<(IpRange, String)>,
}

/// Klassifiziert eine IP-Adresse in die `NetInterface`-Taxonomie.
///
/// Reihenfolge:
/// 1. Loopback (127/8, `::1`).
/// 2. Named-Matches aus `config.named` (erstes Match gewinnt).
/// 3. Local-Subnet-Matches aus `config.local_subnets`.
/// 4. Fallback `Wan`.
#[must_use]
pub fn classify_interface(addr: &IpAddr, config: &InterfaceConfig) -> NetInterface {
    if addr.is_loopback() {
        return NetInterface::Loopback;
    }
    for (range, name) in &config.named {
        if range.contains(addr) {
            return NetInterface::Named(name.clone());
        }
    }
    for range in &config.local_subnets {
        if range.contains(addr) {
            return NetInterface::LocalSubnet(range.clone());
        }
    }
    NetInterface::Wan
}

// ============================================================================
// Policy-Decision
// ============================================================================

/// Entscheidung der [`PolicyEngine`] fuer ein konkretes
/// Paket/Peer/Interface-Tripel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    /// Verlangtes Schutz-Level.
    pub protection: ProtectionLevel,
    /// Gewuenschte Crypto-Suite. `None` bei `ProtectionLevel::None`
    /// oder wenn die Engine dem Plugin die Wahl ueberlaesst.
    pub suite: Option<SuiteHint>,
    /// Harter Drop — Paket wird nicht zugestellt, Peer nicht
    /// akzeptiert. Falls `true` sind `protection`/`suite` irrelevant.
    pub drop: bool,
}

impl PolicyDecision {
    /// Kurzform: "plaintext akzeptiert/erwartet".
    pub const PLAIN: Self = Self {
        protection: ProtectionLevel::None,
        suite: None,
        drop: false,
    };

    /// Kurzform: "hart droppen".
    pub const DROP: Self = Self {
        protection: ProtectionLevel::None,
        suite: None,
        drop: true,
    };

    /// Baut eine Decision aus Protection + Suite. Bei
    /// `ProtectionLevel::None` wird `suite` auf `None` gezwungen.
    #[must_use]
    pub fn with(protection: ProtectionLevel, suite: Option<SuiteHint>) -> Self {
        let suite = if matches!(protection, ProtectionLevel::None) {
            None
        } else {
            suite
        };
        Self {
            protection,
            suite,
            drop: false,
        }
    }
}

// ============================================================================
// Context-Objects
// ============================================================================

/// Outbound-Entscheidungs-Context: ein Writer schickt an einen Peer.
#[derive(Debug)]
pub struct OutboundCtx<'a> {
    /// Domain, in der der Writer lebt.
    pub domain_id: u32,
    /// Topic-Name (fuer Governance-`topic_rule`-Matching).
    pub topic: &'a str,
    /// Partition-Namen (fuer Permissions-Check).
    pub partition: &'a [String],
    /// Klasse des Interfaces, auf dem das Paket rausgeht.
    pub interface: &'a NetInterface,
    /// GuidPrefix des Remote-Peers.
    pub remote_peer: &'a PeerKey,
    /// Capability-Snapshot des Remote-Peers (aus SPDP/SEDP).
    pub remote_caps: &'a PeerCapabilities,
}

/// Inbound-Entscheidungs-Context: ein Datagramm ist reingekommen.
#[derive(Debug)]
pub struct InboundCtx<'a> {
    /// Domain, in der der Empfaenger lebt.
    pub domain_id: u32,
    /// GuidPrefix des Senders (aus RTPS-Header Bytes 8..20).
    pub source_peer: &'a PeerKey,
    /// Klasse des Empfangs-Interfaces.
    pub source_iface: &'a NetInterface,
    /// Capability-Snapshot des Senders. `None` wenn der Peer noch nie
    /// ein SPDP-Announce geschickt hat (Legacy-Vendor oder
    /// Pre-Discovery).
    pub source_caps: Option<&'a PeerCapabilities>,
    /// `true` wenn das Paket mit `SRTPS_PREFIX` beginnt (also laut
    /// Wire-Format schon geschuetzt ist).
    pub is_sec_prefixed: bool,
}

// ============================================================================
// Trait
// ============================================================================

/// Policy-Engine: entscheidet fuer ein konkretes `(peer, topic,
/// interface)`-Tripel das Schutz-Level.
///
/// # Safety-Klassifikation
///
/// Trait ist `Send + Sync`, damit er via `Arc<dyn PolicyEngine>` in
/// Multi-Thread-Runtime genutzt werden kann. Das triggert
/// `zerodds-lint: allow no_dyn_in_safe` (dokumentiert in
/// `08_heterogeneous_security.md` §7).
///
/// # Default-Contract
///
/// * Implementationen muessen **deterministisch** sein: gleiche
///   Context-Inputs → gleiche Decision. Kein Zufall, keine
///   Zeit-abhaengigen Branches (sonst sind Replay-Angriffe moeglich).
/// * `accept_peer` darf `false` zurueckgeben, wenn der Peer nicht
///   die Minimal-Anforderungen (z.B. fehlendes `auth_plugin_class`
///   bei Domain mit `allow_unauthenticated_participants=false`)
///   erfuellt.
/// * `outbound_decision`/`inbound_decision` duerfen **nicht**
///   blockieren — sie laufen im Hot-Path.
pub trait PolicyEngine: Send + Sync {
    /// Outbound-Pfad: welches Schutz-Level soll das Wire-Paket haben?
    fn outbound_decision(&self, ctx: OutboundCtx<'_>) -> PolicyDecision;

    /// Inbound-Pfad: Paket akzeptieren / droppen / entschluesseln?
    fn inbound_decision(&self, ctx: InboundCtx<'_>) -> PolicyDecision;

    /// SEDP-Admission: ist dieser Peer (laut Capabilities)
    /// grundsaetzlich akzeptabel fuer einen Match?
    fn accept_peer(&self, caps: &PeerCapabilities) -> bool;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::net::{Ipv4Addr, Ipv6Addr};

    // ---- ProtectionLevel ----

    #[test]
    fn protection_level_orders_none_sign_encrypt() {
        assert!(ProtectionLevel::None < ProtectionLevel::Sign);
        assert!(ProtectionLevel::Sign < ProtectionLevel::Encrypt);
    }

    #[test]
    fn protection_level_stronger_picks_max() {
        assert_eq!(
            ProtectionLevel::Sign.stronger(ProtectionLevel::Encrypt),
            ProtectionLevel::Encrypt
        );
        assert_eq!(
            ProtectionLevel::Encrypt.stronger(ProtectionLevel::None),
            ProtectionLevel::Encrypt
        );
        assert_eq!(
            ProtectionLevel::None.stronger(ProtectionLevel::None),
            ProtectionLevel::None
        );
    }

    #[test]
    fn protection_level_from_kind_collapses_origin_auth() {
        assert_eq!(
            ProtectionLevel::from_protection_kind(ProtectionKind::None),
            ProtectionLevel::None
        );
        assert_eq!(
            ProtectionLevel::from_protection_kind(ProtectionKind::Sign),
            ProtectionLevel::Sign
        );
        assert_eq!(
            ProtectionLevel::from_protection_kind(ProtectionKind::SignWithOriginAuthentication),
            ProtectionLevel::Sign
        );
        assert_eq!(
            ProtectionLevel::from_protection_kind(ProtectionKind::Encrypt),
            ProtectionLevel::Encrypt
        );
        assert_eq!(
            ProtectionLevel::from_protection_kind(ProtectionKind::EncryptWithOriginAuthentication),
            ProtectionLevel::Encrypt
        );
    }

    #[test]
    fn protection_level_to_kind_roundtrip_without_origin_auth() {
        for lvl in [
            ProtectionLevel::None,
            ProtectionLevel::Sign,
            ProtectionLevel::Encrypt,
        ] {
            let kind = lvl.to_protection_kind();
            assert_eq!(ProtectionLevel::from_protection_kind(kind), lvl);
        }
    }

    #[test]
    fn protection_level_default_is_none() {
        assert_eq!(ProtectionLevel::default(), ProtectionLevel::None);
    }

    // ---- SuiteHint ----

    #[test]
    fn suite_hint_roundtrip_suite() {
        for s in [Suite::Aes128Gcm, Suite::Aes256Gcm, Suite::HmacSha256] {
            assert_eq!(SuiteHint::from_suite(s).to_suite(), s);
        }
    }

    #[test]
    fn suite_hint_protection_level_matches_semantics() {
        assert_eq!(
            SuiteHint::Aes128Gcm.protection_level(),
            ProtectionLevel::Encrypt
        );
        assert_eq!(
            SuiteHint::Aes256Gcm.protection_level(),
            ProtectionLevel::Encrypt
        );
        assert_eq!(
            SuiteHint::HmacSha256.protection_level(),
            ProtectionLevel::Sign
        );
    }

    // ---- IpRange ----

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn ip_range_v4_match_inside_prefix() {
        let r = IpRange {
            base: v4(10, 0, 0, 0),
            prefix_len: 24,
        };
        assert!(r.contains(&v4(10, 0, 0, 1)));
        assert!(r.contains(&v4(10, 0, 0, 255)));
        assert!(!r.contains(&v4(10, 0, 1, 0)));
        assert!(!r.contains(&v4(11, 0, 0, 0)));
    }

    #[test]
    fn ip_range_v4_prefix_zero_matches_all_v4() {
        let r = IpRange {
            base: v4(0, 0, 0, 0),
            prefix_len: 0,
        };
        assert!(r.contains(&v4(1, 2, 3, 4)));
        assert!(r.contains(&v4(255, 255, 255, 255)));
    }

    #[test]
    fn ip_range_v4_prefix_32_is_exact_host() {
        let r = IpRange {
            base: v4(192, 168, 1, 5),
            prefix_len: 32,
        };
        assert!(r.contains(&v4(192, 168, 1, 5)));
        assert!(!r.contains(&v4(192, 168, 1, 6)));
    }

    #[test]
    fn ip_range_v4_out_of_range_prefix_never_matches() {
        let r = IpRange {
            base: v4(10, 0, 0, 0),
            prefix_len: 40, // invalid for v4
        };
        assert!(!r.contains(&v4(10, 0, 0, 1)));
    }

    #[test]
    fn ip_range_v6_basic_match() {
        let base = IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0));
        let r = IpRange {
            base,
            prefix_len: 8,
        };
        assert!(r.contains(&IpAddr::V6(Ipv6Addr::new(0xfd01, 2, 3, 4, 5, 6, 7, 8))));
        assert!(!r.contains(&IpAddr::V6(Ipv6Addr::new(0xfe00, 0, 0, 0, 0, 0, 0, 0))));
    }

    #[test]
    fn ip_range_v6_prefix_zero_matches_all_v6() {
        let r = IpRange {
            base: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            prefix_len: 0,
        };
        assert!(r.contains(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn ip_range_v6_out_of_range_prefix_never_matches() {
        let r = IpRange {
            base: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            prefix_len: 200, // invalid for v6
        };
        assert!(!r.contains(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn ip_range_mixed_family_never_matches() {
        let r = IpRange {
            base: v4(10, 0, 0, 0),
            prefix_len: 8,
        };
        assert!(!r.contains(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        let r6 = IpRange {
            base: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            prefix_len: 0,
        };
        assert!(!r6.contains(&v4(10, 0, 0, 1)));
    }

    // ---- classify_interface ----

    #[test]
    fn classify_loopback_v4() {
        let cfg = InterfaceConfig::default();
        assert_eq!(
            classify_interface(&v4(127, 0, 0, 1), &cfg),
            NetInterface::Loopback
        );
        assert_eq!(
            classify_interface(&v4(127, 1, 2, 3), &cfg),
            NetInterface::Loopback
        );
    }

    #[test]
    fn classify_loopback_v6() {
        let cfg = InterfaceConfig::default();
        assert_eq!(
            classify_interface(&IpAddr::V6(Ipv6Addr::LOCALHOST), &cfg),
            NetInterface::Loopback
        );
    }

    #[test]
    fn classify_local_subnet_after_loopback() {
        let cfg = InterfaceConfig {
            local_subnets: alloc::vec![IpRange {
                base: v4(10, 0, 0, 0),
                prefix_len: 24,
            }],
            ..InterfaceConfig::default()
        };
        match classify_interface(&v4(10, 0, 0, 5), &cfg) {
            NetInterface::LocalSubnet(r) => {
                assert_eq!(r.prefix_len, 24);
            }
            other => panic!("expected LocalSubnet, got {other:?}"),
        }
    }

    #[test]
    fn classify_wan_fallback() {
        let cfg = InterfaceConfig::default();
        assert_eq!(classify_interface(&v4(8, 8, 8, 8), &cfg), NetInterface::Wan);
    }

    #[test]
    fn classify_named_wins_over_local_subnet() {
        let vpn_range = IpRange {
            base: v4(10, 8, 0, 0),
            prefix_len: 16,
        };
        let mgmt_range = IpRange {
            base: v4(10, 0, 0, 0),
            prefix_len: 8,
        };
        let cfg = InterfaceConfig {
            local_subnets: alloc::vec![mgmt_range],
            named: alloc::vec![(vpn_range, "vpn".to_string())],
        };
        assert_eq!(
            classify_interface(&v4(10, 8, 1, 2), &cfg),
            NetInterface::Named("vpn".to_string())
        );
    }

    #[test]
    fn classify_named_first_match_wins() {
        let cfg = InterfaceConfig {
            named: alloc::vec![
                (
                    IpRange {
                        base: v4(10, 0, 0, 0),
                        prefix_len: 8,
                    },
                    "first".to_string()
                ),
                (
                    IpRange {
                        base: v4(10, 0, 0, 0),
                        prefix_len: 24,
                    },
                    "second".to_string()
                ),
            ],
            ..InterfaceConfig::default()
        };
        assert_eq!(
            classify_interface(&v4(10, 0, 0, 5), &cfg),
            NetInterface::Named("first".to_string())
        );
    }

    // ---- PolicyDecision ----

    #[test]
    fn policy_decision_plain_constant() {
        assert_eq!(
            PolicyDecision::PLAIN,
            PolicyDecision {
                protection: ProtectionLevel::None,
                suite: None,
                drop: false,
            }
        );
    }

    #[test]
    fn policy_decision_drop_constant() {
        assert_eq!(
            PolicyDecision::DROP,
            PolicyDecision {
                protection: ProtectionLevel::None,
                suite: None,
                drop: true,
            }
        );
        assert_ne!(PolicyDecision::DROP, PolicyDecision::PLAIN);
    }

    #[test]
    fn policy_decision_with_none_forces_suite_none() {
        let d = PolicyDecision::with(ProtectionLevel::None, Some(SuiteHint::Aes128Gcm));
        assert!(d.suite.is_none());
    }

    #[test]
    fn policy_decision_with_encrypt_keeps_suite() {
        let d = PolicyDecision::with(ProtectionLevel::Encrypt, Some(SuiteHint::Aes256Gcm));
        assert_eq!(d.suite, Some(SuiteHint::Aes256Gcm));
        assert_eq!(d.protection, ProtectionLevel::Encrypt);
        assert!(!d.drop);
    }
}
