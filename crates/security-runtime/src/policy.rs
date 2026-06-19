// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Heterogeneous security — `PolicyEngine` trait and data types
//!.
//!
//! This layer is the abstraction over the governance-XML stack.
//! The v1.4 state ([`crate::SharedSecurityGate`]) decides **one**
//! protection level per participant. System-of-systems deployments
//! (vehicle, tactical, edge) need finer granularity on the triple
//! axis `(peer, topic, interface)`.
//!
//! [`PolicyEngine`] encapsulates this decision:
//! * [`PolicyEngine::outbound_decision`] is called per matched reader
//!   before a wire packet is written.
//! * [`PolicyEngine::inbound_decision`] is called per incoming datagram.
//! * [`PolicyEngine::accept_peer`] is the admission check during
//!   SEDP matching.
//!
//! The default implementation ([`crate::GovernancePolicyEngine`], in
//! stage 1c) mirrors the current `SharedSecurityGate` semantics 1:1.
//! Users can plug in their own `PolicyEngine` impls, e.g. to derive
//! decisions from an external policy server or a vehicle-network
//! certification database.
//!
//! See `docs/architecture/08_heterogeneous_security.md` §3.1.

use alloc::string::String;
use alloc::vec::Vec;

use core::net::IpAddr;

use zerodds_security_crypto::Suite;
use zerodds_security_permissions::ProtectionKind;

use crate::caps::PeerCapabilities;
use crate::shared::PeerKey;

// ============================================================================
// Base types
// ============================================================================

/// Abstract protection level for the policy layer.
///
/// Unlike [`ProtectionKind`] (XML parser type, 5 variants
/// incl. origin authentication), `ProtectionLevel` carries only the 3
/// base classes. Policy decisions compare/order these
/// levels — the origin-auth refinement follows from
/// [`PolicyDecision::suite`] (receiver-specific MACs, RC1).
///
/// The order `None < Sign < Encrypt` is relevant for the "strongest
/// value wins" matching in stage 3 (SEDP endpoint caps)
/// — `Ord`/`PartialOrd` are defined accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ProtectionLevel {
    /// No protection — plaintext RTPS on the wire.
    #[default]
    None,
    /// Integrity protection (HMAC/AEAD tag), payload stays readable.
    Sign,
    /// Integrity + confidentiality (AEAD ciphertext).
    Encrypt,
}

impl ProtectionLevel {
    /// Mapping from the governance-XML [`ProtectionKind`]. Origin-auth
    /// variants collapse to their base class; the origin-auth
    /// property is transported via [`PolicyDecision::suite`] +
    /// receiver-specific MAC encoding.
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

    /// Reverse mapping to [`ProtectionKind`] without origin-auth refinement.
    #[must_use]
    pub fn to_protection_kind(self) -> ProtectionKind {
        match self {
            Self::None => ProtectionKind::None,
            Self::Sign => ProtectionKind::Sign,
            Self::Encrypt => ProtectionKind::Encrypt,
        }
    }

    /// Picks the stronger of two levels (e.g. writer
    /// vs. reader offer).
    #[must_use]
    pub fn stronger(self, other: Self) -> Self {
        if self >= other { self } else { other }
    }
}

/// Crypto-suite hint for the policy decision.
///
/// `SuiteHint` is a **wish** of the policy engine — the crypto
/// plugin may ignore it if it does not support the
/// algorithm. The concrete [`Suite`] enum (`security-crypto`)
/// is the plugin-internal type; this indirection allows future
/// suites (ChaCha20-Poly1305, AES-CCM) without a breaking change to the
/// policy API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuiteHint {
    /// AES-128-GCM — default suite v1.4.
    Aes128Gcm,
    /// AES-256-GCM — for long-term confidentiality / compliance.
    Aes256Gcm,
    /// HMAC-SHA256 auth-only (no confidentiality, SIGN level).
    HmacSha256,
}

impl SuiteHint {
    /// Mapping to the plugin-internal [`Suite`].
    #[must_use]
    pub fn to_suite(self) -> Suite {
        match self {
            Self::Aes128Gcm => Suite::Aes128Gcm,
            Self::Aes256Gcm => Suite::Aes256Gcm,
            Self::HmacSha256 => Suite::HmacSha256,
        }
    }

    /// Reverse mapping from [`Suite`].
    #[must_use]
    pub fn from_suite(suite: Suite) -> Self {
        match suite {
            Suite::Aes128Gcm => Self::Aes128Gcm,
            Suite::Aes256Gcm => Self::Aes256Gcm,
            // AES-256-GMAC is auth-only (SIGN) — for the capability advertisement
            // map it to the SIGN hint; the real key suite is set directly via
            // set_local_protection_suites, not via this hint.
            Suite::HmacSha256 | Suite::Aes256Gmac => Self::HmacSha256,
        }
    }

    /// Returns the natural protection level of this suite:
    /// AEAD suites → `Encrypt`, HMAC → `Sign`.
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

/// CIDR-like IPv4/IPv6 range, interpreted inclusively.
///
/// A minimal self-build (no `ipnet` dep, to keep the safety footprint
/// small). Prefix length in host bits: IPv4 up to 32, IPv6
/// up to 128. Parsing (e.g. from `"10.0.0.0/24"`) arrives in RC1
/// (governance XML); here the struct form suffices.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IpRange {
    /// Base address of the range (host bits are ignored).
    pub base: IpAddr,
    /// Prefix length in bits. Must be `<= 32` for v4, `<= 128` for v6.
    pub prefix_len: u8,
}

impl IpRange {
    /// Checks whether `addr` lies in the range. Mixed families
    /// (v4 in a v6 range) are **not** a match.
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

/// Classification of a network interface for the policy decision.
///
/// The engine can decide differently based on it:
/// * `Loopback` + `LocalHost` → often `ProtectionLevel::None` (bytes
///   do not leave the host).
/// * `LocalSubnet` → management networks with `Sign` instead of `Encrypt`.
/// * `Wan` → the most restrictive policy.
/// * `Named` → user-configured classification (e.g. `tun0`
///   as VPN-protected, `can0-gw` as a vehicle-bus gateway).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetInterface {
    /// 127.0.0.0/8 or `::1`.
    Loopback,
    /// Host-local transport outside loopback (shared memory,
    /// Unix domain socket) — bytes do not leave the host kernel.
    LocalHost,
    /// Address in a configured private range (e.g.
    /// `10.0.0.0/24`, `192.168.0.0/16`).
    LocalSubnet(IpRange),
    /// Everything else — public IP, unknown interface.
    Wan,
    /// User-configured interface class by name.
    Named(String),
}

/// Runtime configuration of the interface classifier.
///
/// Built by the user at participant start and passed to the
/// [`PolicyEngine`]. Empty configuration → every non-
/// loopback address lands in [`NetInterface::Wan`].
///
/// The `named` list allows patterns like "all addresses in the tun0 subnet
/// should become `NetInterface::Named("vpn".into())`". The list
/// is processed in order — first match wins.
#[derive(Debug, Clone, Default)]
pub struct InterfaceConfig {
    /// Private subnets classified as [`NetInterface::LocalSubnet`].
    pub local_subnets: Vec<IpRange>,
    /// Named-interface mappings: `(range, name)` — first match
    /// yields [`NetInterface::Named`].
    pub named: Vec<(IpRange, String)>,
}

/// Classifies an IP address into the `NetInterface` taxonomy.
///
/// Order:
/// 1. Loopback (127/8, `::1`).
/// 2. Named matches from `config.named` (first match wins).
/// 3. Local-subnet matches from `config.local_subnets`.
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
// Policy decision
// ============================================================================

/// Decision of the [`PolicyEngine`] for a concrete
/// packet/peer/interface triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    /// Required protection level.
    pub protection: ProtectionLevel,
    /// Desired crypto suite. `None` for `ProtectionLevel::None`
    /// or when the engine leaves the choice to the plugin.
    pub suite: Option<SuiteHint>,
    /// Hard drop — the packet is not delivered, the peer not
    /// accepted. If `true`, `protection`/`suite` are irrelevant.
    pub drop: bool,
}

impl PolicyDecision {
    /// Shorthand: "plaintext accepted/expected".
    pub const PLAIN: Self = Self {
        protection: ProtectionLevel::None,
        suite: None,
        drop: false,
    };

    /// Shorthand: "hard drop".
    pub const DROP: Self = Self {
        protection: ProtectionLevel::None,
        suite: None,
        drop: true,
    };

    /// Builds a decision from protection + suite. For
    /// `ProtectionLevel::None`, `suite` is forced to `None`.
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
// Context objects
// ============================================================================

/// Outbound decision context: a writer sends to a peer.
#[derive(Debug)]
pub struct OutboundCtx<'a> {
    /// Domain the writer lives in.
    pub domain_id: u32,
    /// Topic name (for governance `topic_rule` matching).
    pub topic: &'a str,
    /// Partition names (for the permissions check).
    pub partition: &'a [String],
    /// Class of the interface the packet goes out on.
    pub interface: &'a NetInterface,
    /// GuidPrefix of the remote peer.
    pub remote_peer: &'a PeerKey,
    /// Capability snapshot of the remote peer (from SPDP/SEDP).
    pub remote_caps: &'a PeerCapabilities,
}

/// Inbound decision context: a datagram has come in.
#[derive(Debug)]
pub struct InboundCtx<'a> {
    /// Domain the receiver lives in.
    pub domain_id: u32,
    /// GuidPrefix of the sender (from RTPS header bytes 8..20).
    pub source_peer: &'a PeerKey,
    /// Class of the receive interface.
    pub source_iface: &'a NetInterface,
    /// Capability snapshot of the sender. `None` if the peer has never
    /// sent an SPDP announce (legacy vendor or
    /// pre-discovery).
    pub source_caps: Option<&'a PeerCapabilities>,
    /// `true` if the packet begins with `SRTPS_PREFIX` (i.e. according to the
    /// wire format it is already protected).
    pub is_sec_prefixed: bool,
}

// ============================================================================
// Trait
// ============================================================================

/// Policy engine: decides the protection level for a concrete `(peer, topic,
/// interface)` triple.
///
/// # Safety classification
///
/// The trait is `Send + Sync` so it can be used via `Arc<dyn PolicyEngine>` in
/// a multi-thread runtime. This triggers
/// `zerodds-lint: allow no_dyn_in_safe` (documented in
/// `08_heterogeneous_security.md` §7).
///
/// # Default contract
///
/// * Implementations must be **deterministic**: same
///   context inputs → same decision. No randomness, no
///   time-dependent branches (otherwise replay attacks are possible).
/// * `accept_peer` may return `false` if the peer does not
///   meet the minimal requirements (e.g. a missing `auth_plugin_class`
///   for a domain with `allow_unauthenticated_participants=false`).
/// * `outbound_decision`/`inbound_decision` must **not**
///   block — they run in the hot path.
pub trait PolicyEngine: Send + Sync {
    /// Outbound path: which protection level should the wire packet have?
    fn outbound_decision(&self, ctx: OutboundCtx<'_>) -> PolicyDecision;

    /// Inbound path: accept / drop / decrypt the packet?
    fn inbound_decision(&self, ctx: InboundCtx<'_>) -> PolicyDecision;

    /// SEDP admission: is this peer (according to its capabilities)
    /// fundamentally acceptable for a match?
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
