// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Governance XML parser (OMG DDS-Security 1.1 §9.4.1.2).
//!
//! The governance XML specifies per domain which **topic classes** must
//! be protected how (discovery protection, read/write access,
//! metadata/data protection kind). The file is typically
//! signed with the permissions CA and loaded by the access-control plugin
//! at participant start.
//!
//! # Scope
//!
//! * Parser for `<domain_access_rules>` → `<domain_rule>` →
//!   `<topic_access_rules>` → `<topic_rule>`.
//! * Domain filter via `<domains><id>N</id></domains>` (simple
//!   `id` or `<id_range>min..max</id_range>`).
//! * Topic-expression matching with wildcards via [`crate::topic_match`].
//! * Protection kinds for `metadata_protection_kind` and
//!   `data_protection_kind`.
//!
//! # Non-goals
//!
//! * XML signature verification — **future-major**.
//! * `<allow_unauthenticated_participants>` enforcement — **future-major**.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::delegation_check::{DelegationProfile, TrustAnchor, TrustPolicy};
use zerodds_security_pki::SignatureAlgorithm;

use crate::topic_match::topic_match;
use crate::xml::PermissionsError;

/// Topic protection kind (spec §9.4.1.2 table 48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtectionKind {
    /// No protection.
    #[default]
    None,
    /// Integrity only (HMAC / signature).
    Sign,
    /// Integrity + confidentiality (AEAD).
    Encrypt,
    /// Like `Sign`, but an own MAC per remote reader.
    SignWithOriginAuthentication,
    /// Like `Encrypt`, but an own MAC per remote reader.
    EncryptWithOriginAuthentication,
}

impl ProtectionKind {
    fn parse(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "NONE" => Self::None,
            "SIGN" => Self::Sign,
            "ENCRYPT" => Self::Encrypt,
            "SIGN_WITH_ORIGIN_AUTHENTICATION" => Self::SignWithOriginAuthentication,
            "ENCRYPT_WITH_ORIGIN_AUTHENTICATION" => Self::EncryptWithOriginAuthentication,
            _ => Self::None, // unknown → NONE (fail-open only for
                             // development; production validates via
                             // the XML schema).
        }
    }
}

/// Rule for a topic class (or wildcard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRule {
    /// Topic pattern (wildcards `*` `?` as in permissions).
    pub topic_expression: String,
    /// Discovery protection — SEDP is encrypted.
    pub enable_discovery_protection: bool,
    /// Liveliness protection — `PARTICIPANT_MESSAGE` signed.
    pub enable_liveliness_protection: bool,
    /// Check read access via permissions.
    pub enable_read_access_control: bool,
    /// Check write access via permissions.
    pub enable_write_access_control: bool,
    /// SEC_PREFIX protection for submessage metadata.
    pub metadata_protection_kind: ProtectionKind,
    /// SEC_BODY protection for payload data.
    pub data_protection_kind: ProtectionKind,
}

impl Default for TopicRule {
    fn default() -> Self {
        Self {
            topic_expression: "*".into(),
            enable_discovery_protection: false,
            enable_liveliness_protection: false,
            enable_read_access_control: false,
            enable_write_access_control: false,
            metadata_protection_kind: ProtectionKind::default(),
            data_protection_kind: ProtectionKind::default(),
        }
    }
}

/// Domain filter: list of (min, max) ranges. A single id is
/// stored as `min == max`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainFilter {
    /// Inclusive ranges. If empty: matches all domains (spec default).
    pub ranges: Vec<(u32, u32)>,
}

impl DomainFilter {
    /// `true` if `domain_id` is in a range or the filter
    /// list is empty.
    #[must_use]
    pub fn matches(&self, domain_id: u32) -> bool {
        if self.ranges.is_empty() {
            return true;
        }
        self.ranges
            .iter()
            .any(|(lo, hi)| domain_id >= *lo && domain_id <= *hi)
    }
}

/// A domain rule in the governance XML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainRule {
    /// Filter for domain ids.
    pub domains: DomainFilter,
    /// Allows unauthenticated participants in discovery. Default false.
    pub allow_unauthenticated_participants: bool,
    /// Mandatory access control on participant join.
    pub enable_join_access_control: bool,
    /// Discovery protection at participant level.
    pub discovery_protection_kind: ProtectionKind,
    /// Liveliness protection at participant level.
    pub liveliness_protection_kind: ProtectionKind,
    /// Signature protection for the RTPS header.
    pub rtps_protection_kind: ProtectionKind,
    /// One rule per topic class.
    pub topic_rules: Vec<TopicRule>,
    /// ZeroDDS extension: peer classes for heterogeneous
    /// security. Empty for pure OMG governance documents — that is
    /// the legacy path. Namespace-scoped in the XML:
    /// `<zerodds:peer_classes>`.
    pub peer_classes: Vec<PeerClass>,
    /// ZeroDDS extension: one rule per interface name,
    /// expressing protection overrides and peer-class filters.
    /// Namespace-scoped: `<zerodds:interface_bindings>`.
    pub interface_bindings: Vec<InterfaceBindingRule>,
}

/// Peer class from `<zerodds:peer_class>` (RC1, spec: architecture
/// doc §5).
///
/// Each remote peer is mapped to a peer class by its [`crate::PeerCapabilities`] +
/// cert CN. The first matching class
/// in [`DomainRule::peer_classes`] wins — so order in the XML is
/// relevant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeerClass {
    /// Free-form name for diagnostic purposes (e.g. `"legacy"`, `"fast"`,
    /// `"secure"`, `"highassurance"`).
    pub name: String,
    /// Protection level enforced for peers of this class.
    /// Default `None`.
    pub protection: ProtectionKind,
    /// Match criteria (if all are met, the peer fits this
    /// class).
    pub match_criteria: PeerClassMatch,
}

/// Match criteria of a peer class. All set fields must
/// be met (AND combination). `None`/default values are
/// ignored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeerClassMatch {
    /// Expected auth plugin class (e.g. `"DDS:Auth:PKI-DH:1.2"`).
    /// The empty string `""` explicitly matches peers **without** a plugin
    /// (legacy classification). `None` = this criterion is not
    /// checked.
    pub auth_plugin_class: Option<String>,
    /// Wildcard pattern for the cert CN (`*` joker). Example:
    /// `"*.ha.example"` matches `"writer1.ha.example"`.
    pub cert_cn_pattern: Option<String>,
    /// Suite requirement. The peer must list this suite in its
    /// `supported_suites`. Example: `"AES_128_GCM"`.
    pub suite: Option<String>,
    /// OCSP live-check flag — the peer must have a valid cert
    /// status (mirrors `has_valid_cert` in the peer caps).
    pub require_ocsp: bool,
    /// Delegation profile reference. If set, the
    /// peer MUST have a [`DelegationChain`](zerodds_security_pki::DelegationChain)
    /// in its capabilities that validates against the profile.
    /// `None` = direct auth path without delegation.
    pub delegation_profile: Option<String>,
}

/// Interface-specific rule from `<zerodds:interface_bindings>`.
///
/// Binds logical interface names to protection overrides and
/// allowed peer classes. Complements the socket-based binding
/// structure from stage 6, without replacing it — the governance
/// entry is the policy view, the socket binding is the transport
/// view.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InterfaceBindingRule {
    /// Name of the interface (must match `InterfaceBindingSpec::name` in
    /// the dcps runtime config).
    pub name: String,
    /// Overrides the domain protection kind on this interface.
    /// `None` = no override; the domain default applies.
    pub protection_override: Option<ProtectionKind>,
    /// Allowed peer classes on this interface. Empty = no
    /// restriction (all classes allowed).
    pub peer_class_filter: Vec<String>,
    /// Minimum protection level on this interface. The result is
    /// `max(peer_class.protection, protection_min)`. `None` = no
    /// minimum.
    pub protection_min: Option<ProtectionKind>,
}

/// XML namespace URI for ZeroDDS extensions in Governance.xml.
pub const ZERODDS_NS: &str = "https://zerodds.org/schema/security/heterogeneous";

/// Edge identity mode.
///
/// Architecture reference: `09_delegation.md` §5 (edge identities).
/// `Static` = stable GuidPrefix across restart, manually
/// configured. `Ephemeral` = pseudo-random GuidPrefix with
/// lifetime rotation, for privacy/replay resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum EdgeIdentityMode {
    /// Stable prefix, no auto-rotate.
    #[default]
    Static,
    /// Auto-rotate after `lifetime_seconds` without an explicit trigger.
    Ephemeral,
}

/// Edge identity configuration from `<zerodds:edge_identities>`.
///
/// One entry per edge with name, mode, optionally a fixed GuidPrefix
/// (12 bytes; static default), and lifetime for ephemeral rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeIdentityConfig {
    /// Logical edge name (e.g. `"lidar-A"`, `"turm-imu"`).
    pub name: String,
    /// Static or ephemeral.
    pub mode: EdgeIdentityMode,
    /// 12-byte GuidPrefix; mandatory for `Static`, optional for
    /// `Ephemeral` (initial value).
    pub guid_prefix: Option<[u8; 12]>,
    /// Lifetime in seconds — `Ephemeral` only. `None` = default 300s.
    pub lifetime_seconds: Option<u32>,
}

/// Default lifetime for ephemeral edge identities (seconds).
pub const DEFAULT_EPHEMERAL_LIFETIME_SECS: u32 = 300;

impl EdgeIdentityConfig {
    /// Effective lifetime in seconds — with a default fallback.
    #[must_use]
    pub fn effective_lifetime(&self) -> u32 {
        self.lifetime_seconds
            .unwrap_or(DEFAULT_EPHEMERAL_LIFETIME_SECS)
    }

    /// True if the mode is `Ephemeral`.
    #[must_use]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self.mode, EdgeIdentityMode::Ephemeral)
    }
}

/// Complete governance config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Governance {
    /// All domain rules. Order matters (first match wins).
    pub domain_rules: Vec<DomainRule>,
    /// Edge identity configs from `<zerodds:edge_identities>`. Read by the
    /// GatewayBridge.
    pub edge_identities: Vec<EdgeIdentityConfig>,
    /// Delegation profiles from `<zerodds:delegation_profiles>`.
    /// Looked up by profile name (referenced from
    /// [`PeerClassMatch::delegation_profile`]).
    pub delegation_profiles: BTreeMap<String, DelegationProfile>,
}

impl Governance {
    /// Finds the matching [`DomainRule`] for a domain id. `None`
    /// if none matches — the caller decides the default policy.
    #[must_use]
    pub fn find_domain_rule(&self, domain_id: u32) -> Option<&DomainRule> {
        self.domain_rules
            .iter()
            .find(|r| r.domains.matches(domain_id))
    }

    /// Finds the matching [`TopicRule`] within a domain rule.
    /// The first match in `topic_rules` wins; if no rule matches,
    /// `TopicRule::default()` (no protection) is returned.
    #[must_use]
    pub fn find_topic_rule<'a>(
        &'a self,
        domain_id: u32,
        topic_name: &str,
    ) -> Option<&'a TopicRule> {
        let dr = self.find_domain_rule(domain_id)?;
        dr.topic_rules
            .iter()
            .find(|r| topic_match(&r.topic_expression, topic_name))
    }

    /// DDS-Security §8.4.2.9.3 `check_create_participant` — the spec-correct
    /// participant-create gate, consulting **both** governance and permissions:
    ///
    /// - No domain rule covering `domain_id` → deny.
    /// - `enable_join_access_control = false` → allow (open join; no permission
    ///   needed).
    /// - `enable_join_access_control = true` → allow **iff** `permissions` holds
    ///   a grant valid at `now` whose `<domains>` matches `domain_id`.
    ///
    /// This is the **only** participant-create gate; it consults the
    /// permissions grant, not just the governance topology. A fully
    /// access-controlled governance (`enable_join_access_control=TRUE` + every
    /// topic read+write-AC=TRUE) is **NOT** un-joinable per se — it is joinable
    /// by a participant whose permissions grant the domain (verified
    /// empirically: Cyclone DDS and Fast DDS both join such a governance with a
    /// matching grant; the SROS2 full-lockdown profile relies on exactly this).
    /// An earlier governance-topology-only gate denied it unconditionally — a
    /// spec bug that blocked ZeroDDS from every fully-locked secured domain.
    /// (OpenDDS reads Table 63 literally and self-rejects such a governance;
    /// that is an OpenDDS-specific stance, not binding on conformant peers.)
    #[must_use]
    pub fn check_create_participant(
        &self,
        permissions: &crate::xml::Permissions,
        domain_id: u32,
        now: u64,
    ) -> bool {
        match self.find_domain_rule(domain_id) {
            None => false,
            Some(dr) => {
                if !dr.enable_join_access_control {
                    return true;
                }
                permissions
                    .grants
                    .iter()
                    .any(|g| g.is_valid_at(now) && g.matches_domain(domain_id))
            }
        }
    }
}

/// Parses a governance XML document.
///
/// Accepts the spec schema from §9.4.1.2:
/// ```xml
/// <dds>
///   <domain_access_rules>
///     <domain_rule>
///       <domains><id>0</id></domains>
///       <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
///       <enable_join_access_control>TRUE</enable_join_access_control>
///       <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
///       <liveliness_protection_kind>SIGN</liveliness_protection_kind>
///       <rtps_protection_kind>NONE</rtps_protection_kind>
///       <topic_access_rules>
///         <topic_rule>
///           <topic_expression>*</topic_expression>
///           <enable_discovery_protection>TRUE</enable_discovery_protection>
///           <enable_read_access_control>TRUE</enable_read_access_control>
///           <enable_write_access_control>TRUE</enable_write_access_control>
///           <metadata_protection_kind>SIGN</metadata_protection_kind>
///           <data_protection_kind>ENCRYPT</data_protection_kind>
///         </topic_rule>
///       </topic_access_rules>
///     </domain_rule>
///   </domain_access_rules>
/// </dds>
/// ```
///
/// # Errors
/// See [`PermissionsError`] (reused — governance and
/// permissions share the XML-parse error class).
pub fn parse_governance_xml(xml: &str) -> Result<Governance, PermissionsError> {
    let doc =
        roxmltree::Document::parse(xml).map_err(|e| PermissionsError::InvalidXml(e.to_string()))?;
    let root = doc.root_element();
    let mut rules = Vec::new();
    walk_domain_rules(root, &mut rules)?;
    let mut edge_identities = Vec::new();
    walk_edge_identities(root, &mut edge_identities)?;
    let mut delegation_profiles = BTreeMap::new();
    walk_delegation_profiles(root, &mut delegation_profiles)?;
    Ok(Governance {
        domain_rules: rules,
        edge_identities,
        delegation_profiles,
    })
}

// ============================================================================
// RC1: Delegation-Profiles XML
// ============================================================================

/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in practice).
fn walk_delegation_profiles(
    node: roxmltree::Node<'_, '_>,
    out: &mut BTreeMap<String, DelegationProfile>,
) -> Result<(), PermissionsError> {
    if node.tag_name().name() == "delegation_profiles"
        && node.tag_name().namespace() == Some(ZERODDS_NS)
    {
        for child in node.children().filter(roxmltree::Node::is_element) {
            if child.tag_name().name() == "profile"
                && child.tag_name().namespace() == Some(ZERODDS_NS)
            {
                let p = parse_delegation_profile(child)?;
                out.insert(p.name.clone(), p);
            }
        }
        return Ok(());
    }
    for child in node.children().filter(roxmltree::Node::is_element) {
        walk_delegation_profiles(child, out)?;
    }
    Ok(())
}

fn parse_delegation_profile(
    node: roxmltree::Node<'_, '_>,
) -> Result<DelegationProfile, PermissionsError> {
    use alloc::collections::BTreeSet;
    let name = node
        .attribute("name")
        .ok_or_else(|| PermissionsError::InvalidXml("<profile> missing name".into()))?
        .to_string();
    let mut trust_policy = TrustPolicy::DirectOrDelegated;
    let mut max_chain_depth = 3usize;
    let mut allowed_algorithms: BTreeSet<u8> = BTreeSet::new();
    let mut trust_anchors: Vec<TrustAnchor> = Vec::new();
    let mut require_ocsp = false;

    for child in node.children().filter(roxmltree::Node::is_element) {
        if child.tag_name().namespace() != Some(ZERODDS_NS) {
            continue;
        }
        match child.tag_name().name() {
            "trust_policy" => {
                trust_policy = parse_trust_policy(child.text().unwrap_or("").trim())
                    .unwrap_or(TrustPolicy::DirectOrDelegated);
            }
            "max_chain_depth" => {
                if let Ok(v) = child.text().unwrap_or("").trim().parse::<usize>() {
                    max_chain_depth = v;
                }
            }
            "require_ocsp" => {
                require_ocsp = parse_bool(child);
            }
            "allowed_algorithms" => {
                for algo_el in child.children().filter(roxmltree::Node::is_element) {
                    if algo_el.tag_name().name() == "algorithm"
                        && algo_el.tag_name().namespace() == Some(ZERODDS_NS)
                    {
                        if let Some(a) = parse_algorithm(algo_el.text().unwrap_or("").trim()) {
                            allowed_algorithms.insert(a.wire_id());
                        }
                    }
                }
            }
            "trust_anchors" => {
                for anchor_el in child.children().filter(roxmltree::Node::is_element) {
                    if anchor_el.tag_name().name() == "anchor"
                        && anchor_el.tag_name().namespace() == Some(ZERODDS_NS)
                    {
                        if let Some(a) = parse_trust_anchor(anchor_el)? {
                            trust_anchors.push(a);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(DelegationProfile {
        name,
        trust_policy,
        trust_anchors,
        max_chain_depth,
        allowed_algorithms,
        require_ocsp,
    })
}

fn parse_trust_policy(s: &str) -> Option<TrustPolicy> {
    match s.trim().to_lowercase().as_str() {
        "gateway-only" | "gateway_only" => Some(TrustPolicy::GatewayOnly),
        "direct-or-delegated" | "direct_or_delegated" => Some(TrustPolicy::DirectOrDelegated),
        "federation" => Some(TrustPolicy::Federation),
        "strict-delegated" | "strict_delegated" => Some(TrustPolicy::StrictDelegated),
        _ => None,
    }
}

fn parse_algorithm(s: &str) -> Option<SignatureAlgorithm> {
    match s.trim().to_lowercase().as_str() {
        "ecdsa-p256" | "ecdsa_p256" => Some(SignatureAlgorithm::EcdsaP256),
        "ecdsa-p384" | "ecdsa_p384" => Some(SignatureAlgorithm::EcdsaP384),
        "rsa-pss-2048" | "rsa_pss_2048" => Some(SignatureAlgorithm::RsaPss2048),
        "ed25519" => Some(SignatureAlgorithm::Ed25519),
        _ => None,
    }
}

fn parse_trust_anchor(
    node: roxmltree::Node<'_, '_>,
) -> Result<Option<TrustAnchor>, PermissionsError> {
    let subject_guid = match node
        .attribute("subject_guid")
        .and_then(parse_guid_prefix_hex_16)
    {
        Some(g) => g,
        None => {
            return Err(PermissionsError::InvalidXml(
                "<anchor> needs valid 16-byte hex subject_guid".into(),
            ));
        }
    };
    let algorithm = node
        .attribute("algorithm")
        .and_then(parse_algorithm)
        .ok_or_else(|| {
            PermissionsError::InvalidXml("<anchor> needs valid algorithm attribute".into())
        })?;
    let pk_b64 = node
        .attribute("public_key")
        .ok_or_else(|| PermissionsError::InvalidXml("<anchor> needs public_key (base64)".into()))?;
    let verify_public_key = base64_decode_anchor(pk_b64).ok_or_else(|| {
        PermissionsError::InvalidXml("<anchor> public_key is not valid base64".into())
    })?;
    Ok(Some(TrustAnchor {
        subject_guid,
        verify_public_key,
        algorithm,
    }))
}

/// 16-byte (32-hex-char) GUID parser for the trust anchor.
fn parse_guid_prefix_hex_16(s: &str) -> Option<[u8; 16]> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if cleaned.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte_pair) in cleaned.as_bytes().chunks(2).enumerate() {
        if i >= 16 {
            return None;
        }
        let s = core::str::from_utf8(byte_pair).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

/// Base64 decoder for trust-anchor public-key bytes.
fn base64_decode_anchor(input: &str) -> Option<Vec<u8>> {
    // Remove whitespace (PEM-style multiline).
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = cleaned.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let mut vals = [0u8; 4];
        let mut pad = 0usize;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                pad += 1;
                vals[i] = 0;
            } else if pad > 0 {
                return None;
            } else {
                vals[i] = match c {
                    b'A'..=b'Z' => c - b'A',
                    b'a'..=b'z' => c - b'a' + 26,
                    b'0'..=b'9' => c - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    _ => return None,
                };
            }
        }
        let n = (u32::from(vals[0]) << 18)
            | (u32::from(vals[1]) << 12)
            | (u32::from(vals[2]) << 6)
            | u32::from(vals[3]);
        out.push(((n >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if pad < 1 {
            out.push((n & 0xFF) as u8);
        }
    }
    Some(out)
}

/// Searches recursively for `<zerodds:edge_identities>` elements and parses
/// their `<edge>` children.
///
/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in practice).
fn walk_edge_identities(
    node: roxmltree::Node<'_, '_>,
    out: &mut Vec<EdgeIdentityConfig>,
) -> Result<(), PermissionsError> {
    if node.tag_name().name() == "edge_identities"
        && node.tag_name().namespace() == Some(ZERODDS_NS)
    {
        let default_mode = parse_edge_mode_attr(node, "default_mode").unwrap_or_default();
        for child in node.children().filter(roxmltree::Node::is_element) {
            if child.tag_name().name() == "edge" && child.tag_name().namespace() == Some(ZERODDS_NS)
            {
                out.push(parse_edge(child, default_mode)?);
            }
        }
        return Ok(());
    }
    for child in node.children().filter(roxmltree::Node::is_element) {
        walk_edge_identities(child, out)?;
    }
    Ok(())
}

fn parse_edge_mode_attr(node: roxmltree::Node<'_, '_>, attr: &str) -> Option<EdgeIdentityMode> {
    node.attribute(attr).and_then(|v| match v.trim() {
        "static" => Some(EdgeIdentityMode::Static),
        "ephemeral" => Some(EdgeIdentityMode::Ephemeral),
        _ => None,
    })
}

fn parse_edge(
    node: roxmltree::Node<'_, '_>,
    default_mode: EdgeIdentityMode,
) -> Result<EdgeIdentityConfig, PermissionsError> {
    let name = node
        .attribute("name")
        .ok_or_else(|| PermissionsError::InvalidXml("<edge> missing name attribute".into()))?
        .to_string();
    let mode = parse_edge_mode_attr(node, "mode").unwrap_or(default_mode);
    let guid_prefix = node
        .attribute("guid_prefix")
        .and_then(parse_guid_prefix_hex);
    let lifetime_seconds = node
        .attribute("lifetime_seconds")
        .and_then(|s| s.trim().parse::<u32>().ok());
    Ok(EdgeIdentityConfig {
        name,
        mode,
        guid_prefix,
        lifetime_seconds,
    })
}

/// Parses a hex-encoded 12-byte GuidPrefix with optional separators
/// (`:`, `-`, whitespace). Examples:
/// * `"01020304050607080910111213"`  (24 hex chars, 12 byte)
/// * `"01:02:03:04:05:06:07:08:09:10:11:12"`
fn parse_guid_prefix_hex(s: &str) -> Option<[u8; 12]> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if cleaned.len() != 24 {
        return None;
    }
    let mut out = [0u8; 12];
    for (i, byte_pair) in cleaned.as_bytes().chunks(2).enumerate() {
        if i >= 12 {
            return None;
        }
        let s = core::str::from_utf8(byte_pair).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in practice).
fn walk_domain_rules(
    node: roxmltree::Node<'_, '_>,
    out: &mut Vec<DomainRule>,
) -> Result<(), PermissionsError> {
    if node.tag_name().name() == "domain_rule" {
        out.push(parse_domain_rule(node)?);
        return Ok(());
    }
    for child in node.children().filter(roxmltree::Node::is_element) {
        walk_domain_rules(child, out)?;
    }
    Ok(())
}

fn parse_domain_rule(rule: roxmltree::Node<'_, '_>) -> Result<DomainRule, PermissionsError> {
    let mut out = DomainRule::default();
    for child in rule.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "domains" => out.domains = parse_domain_filter(child),
            "allow_unauthenticated_participants" => {
                out.allow_unauthenticated_participants = parse_bool(child);
            }
            "enable_join_access_control" => {
                out.enable_join_access_control = parse_bool(child);
            }
            "discovery_protection_kind" => {
                if let Some(t) = child.text() {
                    out.discovery_protection_kind = ProtectionKind::parse(t);
                }
            }
            "liveliness_protection_kind" => {
                if let Some(t) = child.text() {
                    out.liveliness_protection_kind = ProtectionKind::parse(t);
                }
            }
            "rtps_protection_kind" => {
                if let Some(t) = child.text() {
                    out.rtps_protection_kind = ProtectionKind::parse(t);
                }
            }
            "topic_access_rules" => {
                for tr in child.children().filter(|c| c.has_tag_name("topic_rule")) {
                    out.topic_rules.push(parse_topic_rule(tr));
                }
            }
            // RC1: zerodds extensions. We match by the namespace
            // URI — the element name is only `peer_classes` without a prefix
            // (roxmltree already resolved that).
            "peer_classes" if child.tag_name().namespace() == Some(ZERODDS_NS) => {
                for pc in child.children().filter(roxmltree::Node::is_element) {
                    if pc.tag_name().name() == "peer_class"
                        && pc.tag_name().namespace() == Some(ZERODDS_NS)
                    {
                        out.peer_classes.push(parse_peer_class(pc));
                    }
                }
            }
            "interface_bindings" if child.tag_name().namespace() == Some(ZERODDS_NS) => {
                for ib in child.children().filter(roxmltree::Node::is_element) {
                    if ib.tag_name().name() == "interface"
                        && ib.tag_name().namespace() == Some(ZERODDS_NS)
                    {
                        out.interface_bindings.push(parse_interface_binding(ib));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

fn parse_peer_class(node: roxmltree::Node<'_, '_>) -> PeerClass {
    let mut out = PeerClass {
        name: node.attribute("name").unwrap_or("").to_string(),
        protection: node
            .attribute("protection")
            .map(ProtectionKind::parse)
            .unwrap_or_default(),
        match_criteria: PeerClassMatch::default(),
    };
    for child in node.children().filter(roxmltree::Node::is_element) {
        // We accept `<match>` both with and without a namespace
        // prefix — more convenient for XML authors who write the element
        // inside the parent namespace.
        if child.tag_name().name() != "match" {
            continue;
        }
        if let Some(v) = child.attribute("auth_plugin_class") {
            out.match_criteria.auth_plugin_class = Some(v.to_string());
        }
        if let Some(v) = child.attribute("cert_cn_pattern") {
            out.match_criteria.cert_cn_pattern = Some(v.to_string());
        }
        if let Some(v) = child.attribute("suite") {
            out.match_criteria.suite = Some(v.to_string());
        }
        if let Some(v) = child.attribute("require_ocsp") {
            out.match_criteria.require_ocsp =
                matches!(v.trim().to_uppercase().as_str(), "TRUE" | "1" | "YES");
        }
    }
    out
}

fn parse_interface_binding(node: roxmltree::Node<'_, '_>) -> InterfaceBindingRule {
    let name = node.attribute("name").unwrap_or("").to_string();
    let protection_override = node
        .attribute("protection_override")
        .map(ProtectionKind::parse);
    let protection_min = node.attribute("protection_min").map(ProtectionKind::parse);
    let peer_class_filter = node
        .attribute("peer_class_filter")
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    InterfaceBindingRule {
        name,
        protection_override,
        peer_class_filter,
        protection_min,
    }
}

/// Wildcard matcher for cert-CN patterns. The only joker is `*`,
/// matching any number of characters (incl. `.`). An empty pattern matches
/// only empty strings. For `*.fast.example`:
/// `"w1.fast.example"` → `true`, `"fast.example"` → `false`.
#[must_use]
pub fn cn_pattern_match(pattern: &str, cn: &str) -> bool {
    // Split on `*`, then find iteratively in the haystack.
    // No regex dependency — the project keeps the safety-crate
    // footprint small.
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == cn;
    }
    let mut idx = 0usize;
    // The prefix must match at the start.
    if !parts[0].is_empty() {
        if !cn.starts_with(parts[0]) {
            return false;
        }
        idx = parts[0].len();
    }
    // Find the middle pieces.
    for (i, p) in parts.iter().enumerate().skip(1) {
        if p.is_empty() {
            // Two `*` in a row → empty middle piece; skip.
            continue;
        }
        let is_last = i == parts.len() - 1;
        if is_last {
            // The last piece must be at the end.
            if !cn[idx..].ends_with(p) {
                return false;
            }
            // And it must not overlap with already-matched bytes.
            let need = idx + p.len();
            if cn.len() < need {
                return false;
            }
            return true;
        }
        match cn[idx..].find(p) {
            Some(found) => idx += found + p.len(),
            None => return false,
        }
    }
    true
}

fn parse_domain_filter(node: roxmltree::Node<'_, '_>) -> DomainFilter {
    let mut ranges = Vec::new();
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "id" => {
                if let Some(t) = child.text() {
                    if let Ok(n) = t.trim().parse::<u32>() {
                        ranges.push((n, n));
                    }
                }
            }
            "id_range" => {
                let lo = child
                    .children()
                    .find(|c| c.has_tag_name("min"))
                    .and_then(|c| c.text())
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                let hi = child
                    .children()
                    .find(|c| c.has_tag_name("max"))
                    .and_then(|c| c.text())
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .unwrap_or(u32::MAX);
                ranges.push((lo, hi));
            }
            _ => {}
        }
    }
    DomainFilter { ranges }
}

fn parse_topic_rule(node: roxmltree::Node<'_, '_>) -> TopicRule {
    let mut out = TopicRule::default();
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "topic_expression" => {
                if let Some(t) = child.text() {
                    out.topic_expression = t.trim().to_string();
                }
            }
            "enable_discovery_protection" => out.enable_discovery_protection = parse_bool(child),
            "enable_liveliness_protection" => out.enable_liveliness_protection = parse_bool(child),
            "enable_read_access_control" => out.enable_read_access_control = parse_bool(child),
            "enable_write_access_control" => out.enable_write_access_control = parse_bool(child),
            "metadata_protection_kind" => {
                if let Some(t) = child.text() {
                    out.metadata_protection_kind = ProtectionKind::parse(t);
                }
            }
            "data_protection_kind" => {
                if let Some(t) = child.text() {
                    out.data_protection_kind = ProtectionKind::parse(t);
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_bool(node: roxmltree::Node<'_, '_>) -> bool {
    node.text()
        .map(|t| {
            let up = t.trim().to_uppercase();
            up == "TRUE" || up == "1" || up == "YES"
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn check_create_participant_consults_permissions() {
        use crate::xml::parse_permissions_xml;
        // Full-AC governance for domain 200 (join-AC + single * topic RW-AC).
        // This is NOT un-joinable per se — it is joinable by a participant
        // whose permissions grant the domain (§8.4.2.9.3), exactly as Cyclone
        // and Fast DDS treat it.
        let full_ac = parse_governance_xml(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>200</id></domains>
                 <enable_join_access_control>true</enable_join_access_control>
                 <topic_access_rules><topic_rule>
                   <topic_expression>*</topic_expression>
                   <enable_read_access_control>true</enable_read_access_control>
                   <enable_write_access_control>true</enable_write_access_control>
                 </topic_rule></topic_access_rules>
               </domain_rule></domain_access_rules></dds>"#,
        )
        .unwrap();

        // A grant allowing domain 200 → create ALLOWED (the fix; Cyclone/FastDDS
        // join such a domain with this grant).
        let grant_200 = parse_permissions_xml(
            r#"<permissions><grant><subject_name>CN=ping</subject_name>
                 <allow_rule><domains><id>200</id></domains>
                   <publish><topics><topic>*</topic></topics></publish></allow_rule>
               </grant></permissions>"#,
        )
        .unwrap();
        assert!(
            full_ac.check_create_participant(&grant_200, 200, 0),
            "full-AC + matching grant → joinable (spec §8.4.2.9.3)"
        );

        // A grant for a different domain → deny (no grant covers domain 200).
        let grant_5 = parse_permissions_xml(
            r#"<permissions><grant><subject_name>CN=ping</subject_name>
                 <allow_rule><domains><id>5</id></domains>
                   <publish><topics><topic>*</topic></topics></publish></allow_rule>
               </grant></permissions>"#,
        )
        .unwrap();
        assert!(
            !full_ac.check_create_participant(&grant_5, 200, 0),
            "no grant for the domain → deny"
        );

        // join-AC=false → allow regardless of permissions (open join).
        let open = parse_governance_xml(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>200</id></domains>
                 <enable_join_access_control>false</enable_join_access_control>
               </domain_rule></domain_access_rules></dds>"#,
        )
        .unwrap();
        assert!(
            open.check_create_participant(&grant_5, 200, 0),
            "open join → allow"
        );

        // No domain rule → deny.
        assert!(!full_ac.check_create_participant(&grant_200, 999, 0));
    }

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains>
        <id>0</id>
        <id_range><min>10</min><max>20</max></id_range>
      </domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>TRUE</enable_join_access_control>
      <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
      <liveliness_protection_kind>SIGN</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>Chatter</topic_expression>
          <enable_discovery_protection>TRUE</enable_discovery_protection>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>SIGN</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#;

    #[test]
    fn parses_domain_rule_with_ranges() {
        let g = parse_governance_xml(SAMPLE).expect("parse");
        assert_eq!(g.domain_rules.len(), 1);
        let d = &g.domain_rules[0];
        assert!(!d.allow_unauthenticated_participants);
        assert!(d.enable_join_access_control);
        assert_eq!(d.discovery_protection_kind, ProtectionKind::Encrypt);
        assert_eq!(d.rtps_protection_kind, ProtectionKind::None);
        assert_eq!(d.domains.ranges, vec![(0, 0), (10, 20)]);
    }

    #[test]
    fn topic_rule_matches_exact_topic_first() {
        let g = parse_governance_xml(SAMPLE).unwrap();
        let tr = g.find_topic_rule(0, "Chatter").expect("rule");
        assert_eq!(tr.metadata_protection_kind, ProtectionKind::Sign);
        assert_eq!(tr.data_protection_kind, ProtectionKind::Encrypt);
    }

    #[test]
    fn topic_rule_falls_through_to_wildcard() {
        let g = parse_governance_xml(SAMPLE).unwrap();
        let tr = g.find_topic_rule(0, "UnknownTopic").expect("wildcard");
        assert_eq!(tr.metadata_protection_kind, ProtectionKind::None);
    }

    #[test]
    fn domain_filter_id_range_matches_inclusive() {
        let g = parse_governance_xml(SAMPLE).unwrap();
        assert!(g.find_domain_rule(10).is_some());
        assert!(g.find_domain_rule(15).is_some());
        assert!(g.find_domain_rule(20).is_some());
        // 21 is outside all ranges (0-0, 10-20), so None.
        assert!(g.find_domain_rule(21).is_none());
    }

    #[test]
    fn empty_domains_matches_all() {
        let xml = r#"
<domain_access_rules>
  <domain_rule>
    <domains/>
    <topic_access_rules>
      <topic_rule><topic_expression>*</topic_expression></topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules>"#;
        let g = parse_governance_xml(xml).unwrap();
        assert!(g.find_domain_rule(42).is_some());
    }

    #[test]
    fn rejects_invalid_xml() {
        assert!(matches!(
            parse_governance_xml("<not-closed"),
            Err(PermissionsError::InvalidXml(_))
        ));
    }

    #[test]
    fn protection_kind_parses_case_insensitive() {
        assert_eq!(ProtectionKind::parse("encrypt"), ProtectionKind::Encrypt);
        assert_eq!(ProtectionKind::parse("Sign"), ProtectionKind::Sign);
        assert_eq!(ProtectionKind::parse("NONE"), ProtectionKind::None);
        assert_eq!(
            ProtectionKind::parse("encrypt_with_origin_authentication"),
            ProtectionKind::EncryptWithOriginAuthentication
        );
    }

    // =======================================================================
    // RC1 stage 8 — peer classes + interface bindings (zerodds-ns)
    // =======================================================================

    const HETERO_GOV: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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

    // ---- cn_pattern_match ----

    #[test]
    fn cn_pattern_exact_match_no_wildcard() {
        assert!(cn_pattern_match("alice.example", "alice.example"));
        assert!(!cn_pattern_match("alice.example", "bob.example"));
    }

    #[test]
    fn cn_pattern_leading_star_matches_suffix() {
        assert!(cn_pattern_match("*.fast.example", "writer1.fast.example"));
        assert!(cn_pattern_match("*.fast.example", "x.fast.example"));
        assert!(!cn_pattern_match("*.fast.example", "fast.example"));
        assert!(!cn_pattern_match("*.fast.example", "slow.example"));
    }

    #[test]
    fn cn_pattern_trailing_star_matches_prefix() {
        assert!(cn_pattern_match("writer*", "writer1"));
        assert!(cn_pattern_match("writer*", "writer.ha.domain"));
        assert!(!cn_pattern_match("writer*", "reader1"));
    }

    #[test]
    fn cn_pattern_middle_star_matches_infix() {
        assert!(cn_pattern_match("*.ha.*", "w1.ha.internal"));
        assert!(cn_pattern_match("*.ha.*", "reader.ha.corp.local"));
        assert!(!cn_pattern_match("*.ha.*", "w1.fast.example"));
    }

    #[test]
    fn cn_pattern_only_star_matches_any() {
        assert!(cn_pattern_match("*", "anything"));
        assert!(cn_pattern_match("*", ""));
    }

    #[test]
    fn cn_pattern_empty_matches_only_empty() {
        assert!(cn_pattern_match("", ""));
        assert!(!cn_pattern_match("", "non-empty"));
    }

    // ---- Peer-Classes XML-Parse ----

    #[test]
    fn hetero_gov_parses_four_peer_classes_in_order() {
        let g = parse_governance_xml(HETERO_GOV).unwrap();
        let rule = g.find_domain_rule(0).unwrap();
        assert_eq!(rule.peer_classes.len(), 4);
        assert_eq!(rule.peer_classes[0].name, "legacy");
        assert_eq!(rule.peer_classes[1].name, "fast");
        assert_eq!(rule.peer_classes[2].name, "secure");
        assert_eq!(rule.peer_classes[3].name, "highassurance");
    }

    #[test]
    fn hetero_gov_peer_class_protection_levels_correct() {
        let g = parse_governance_xml(HETERO_GOV).unwrap();
        let rule = g.find_domain_rule(0).unwrap();
        assert_eq!(rule.peer_classes[0].protection, ProtectionKind::None);
        assert_eq!(rule.peer_classes[1].protection, ProtectionKind::Sign);
        assert_eq!(rule.peer_classes[2].protection, ProtectionKind::Encrypt);
        assert_eq!(rule.peer_classes[3].protection, ProtectionKind::Encrypt);
    }

    #[test]
    fn hetero_gov_peer_class_match_criteria_parsed() {
        let g = parse_governance_xml(HETERO_GOV).unwrap();
        let rule = g.find_domain_rule(0).unwrap();

        // Legacy: explicit empty auth_plugin = "no plugin expected"
        assert_eq!(
            rule.peer_classes[0]
                .match_criteria
                .auth_plugin_class
                .as_deref(),
            Some("")
        );

        // Fast: cert-CN-Pattern
        assert_eq!(
            rule.peer_classes[1]
                .match_criteria
                .cert_cn_pattern
                .as_deref(),
            Some("*.fast.example")
        );

        // Secure: auth + suite
        assert_eq!(
            rule.peer_classes[2]
                .match_criteria
                .auth_plugin_class
                .as_deref(),
            Some("DDS:Auth:PKI-DH:1.2")
        );
        assert_eq!(
            rule.peer_classes[2].match_criteria.suite.as_deref(),
            Some("AES_128_GCM")
        );

        // HA: cert + suite + ocsp
        assert_eq!(
            rule.peer_classes[3]
                .match_criteria
                .cert_cn_pattern
                .as_deref(),
            Some("*.ha.*")
        );
        assert_eq!(
            rule.peer_classes[3].match_criteria.suite.as_deref(),
            Some("AES_256_GCM")
        );
        assert!(rule.peer_classes[3].match_criteria.require_ocsp);
    }

    // ---- Interface-Bindings XML-Parse ----

    #[test]
    fn hetero_gov_interface_bindings_parsed() {
        let g = parse_governance_xml(HETERO_GOV).unwrap();
        let rule = g.find_domain_rule(0).unwrap();
        assert_eq!(rule.interface_bindings.len(), 4);

        let lo = &rule.interface_bindings[0];
        assert_eq!(lo.name, "loopback");
        assert_eq!(lo.protection_override, Some(ProtectionKind::None));

        let eth0 = &rule.interface_bindings[2];
        assert_eq!(eth0.name, "eth0");
        assert_eq!(
            eth0.peer_class_filter,
            vec![
                "legacy".to_string(),
                "fast".to_string(),
                "secure".to_string()
            ]
        );

        let tun0 = &rule.interface_bindings[3];
        assert_eq!(tun0.name, "tun0");
        assert_eq!(tun0.protection_min, Some(ProtectionKind::Encrypt));
        assert_eq!(
            tun0.peer_class_filter,
            vec!["secure".to_string(), "highassurance".to_string()]
        );
    }

    // ---- OMG-Vendor-Interop ----

    #[test]
    fn pure_omg_governance_yields_empty_peer_classes_and_bindings() {
        // A governance document without the zerodds: namespace should work
        // exactly as today (backward compatibility).
        let g = parse_governance_xml(SAMPLE).unwrap();
        for rule in &g.domain_rules {
            assert!(
                rule.peer_classes.is_empty(),
                "an OMG-only doc must not trigger peer_classes"
            );
            assert!(
                rule.interface_bindings.is_empty(),
                "an OMG-only doc must not trigger interface_bindings"
            );
        }
    }

    #[test]
    fn cyclone_style_without_namespace_declaration_ignores_zerodds_elements() {
        // Cyclone perspective: they parse the governance XML and throw away
        // unknown namespaces. We simulate that by having an XML
        // use the zerodds elements without a namespace declaration —
        // then our namespace filter does not match and the element
        // is silently ignored.
        //
        // This is the vendor-interop guarantee: Cyclone/FastDDS see
        // zerodds: tags, ignore them if they do not know the namespace,
        // and fall back to rtps_protection_kind.
        const MIXED: &str = r#"<?xml version="1.0"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <peer_classes>
        <peer_class name="should-be-ignored" protection="NONE" />
      </peer_classes>
    </domain_rule>
  </domain_access_rules>
</dds>"#;
        let g = parse_governance_xml(MIXED).unwrap();
        let rule = g.find_domain_rule(0).unwrap();
        assert!(
            rule.peer_classes.is_empty(),
            "peer_classes without the zerodds namespace must be ignored"
        );
        assert_eq!(rule.rtps_protection_kind, ProtectionKind::Encrypt);
    }

    // ========================================================================
    // RC1: Edge-Identities XML
    // ========================================================================

    #[test]
    fn edge_identity_default_mode_is_static() {
        let cfg = EdgeIdentityConfig {
            name: "x".into(),
            mode: EdgeIdentityMode::default(),
            guid_prefix: None,
            lifetime_seconds: None,
        };
        assert_eq!(cfg.mode, EdgeIdentityMode::Static);
        assert_eq!(cfg.effective_lifetime(), 300);
        assert!(!cfg.is_ephemeral());
    }

    #[test]
    fn edge_identity_ephemeral_with_explicit_lifetime() {
        let cfg = EdgeIdentityConfig {
            name: "imu".into(),
            mode: EdgeIdentityMode::Ephemeral,
            guid_prefix: None,
            lifetime_seconds: Some(60),
        };
        assert!(cfg.is_ephemeral());
        assert_eq!(cfg.effective_lifetime(), 60);
    }

    #[test]
    fn parses_edge_identities_block_with_two_edges() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
    </domain_rule>
  </domain_access_rules>
  <zerodds:edge_identities default_mode="static">
    <zerodds:edge name="lidar-A" guid_prefix="010203040506070809101112" />
    <zerodds:edge name="turm-imu" mode="ephemeral" lifetime_seconds="60" />
  </zerodds:edge_identities>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        assert_eq!(g.edge_identities.len(), 2);

        let lidar = &g.edge_identities[0];
        assert_eq!(lidar.name, "lidar-A");
        assert_eq!(lidar.mode, EdgeIdentityMode::Static);
        assert_eq!(
            lidar.guid_prefix,
            Some([
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x10, 0x11, 0x12
            ])
        );

        let imu = &g.edge_identities[1];
        assert_eq!(imu.name, "turm-imu");
        assert_eq!(imu.mode, EdgeIdentityMode::Ephemeral);
        assert_eq!(imu.lifetime_seconds, Some(60));
    }

    #[test]
    fn edge_identity_inherits_default_mode() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:edge_identities default_mode="ephemeral">
    <zerodds:edge name="auto-rotated" />
  </zerodds:edge_identities>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        assert_eq!(g.edge_identities[0].mode, EdgeIdentityMode::Ephemeral);
    }

    #[test]
    fn edge_identity_with_colon_separated_guid() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:edge_identities>
    <zerodds:edge name="ecu-a" guid_prefix="aa:bb:cc:dd:ee:ff:11:22:33:44:55:66" />
  </zerodds:edge_identities>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        let p = g.edge_identities[0].guid_prefix.unwrap();
        assert_eq!(
            p,
            [
                0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66
            ]
        );
    }

    #[test]
    fn edge_identity_invalid_guid_is_none() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:edge_identities>
    <zerodds:edge name="bad" guid_prefix="ZZ" />
  </zerodds:edge_identities>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        assert!(g.edge_identities[0].guid_prefix.is_none());
    }

    #[test]
    fn edge_identity_without_namespace_is_ignored() {
        // Without the zerodds: namespace nothing may be parsed — the ZeroDDS
        // extension requirement.
        const XML: &str = r#"<?xml version="1.0"?>
<dds>
  <edge_identities>
    <edge name="ignored-no-ns" />
  </edge_identities>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        assert!(g.edge_identities.is_empty());
    }

    // ========================================================================
    // RC1: Delegation-Profile XML
    // ========================================================================

    /// Test helper — generates a public key in the right format and encodes
    /// it as Base64.
    fn ecdsa_p256_test_pubkey_base64() -> String {
        use ring::rand::SystemRandom;
        use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let kp = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        let raw = kp.public_key().as_ref();
        // Base64-encode (standard alphabet, with padding).
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut chunks = raw.chunks_exact(3);
        for chunk in &mut chunks {
            let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
            out.push(alphabet[((n >> 18) & 0x3F) as usize] as char);
            out.push(alphabet[((n >> 12) & 0x3F) as usize] as char);
            out.push(alphabet[((n >> 6) & 0x3F) as usize] as char);
            out.push(alphabet[(n & 0x3F) as usize] as char);
        }
        let rem = chunks.remainder();
        match rem.len() {
            1 => {
                let n = u32::from(rem[0]) << 16;
                out.push(alphabet[((n >> 18) & 0x3F) as usize] as char);
                out.push(alphabet[((n >> 12) & 0x3F) as usize] as char);
                out.push('=');
                out.push('=');
            }
            2 => {
                let n = (u32::from(rem[0]) << 16) | (u32::from(rem[1]) << 8);
                out.push(alphabet[((n >> 18) & 0x3F) as usize] as char);
                out.push(alphabet[((n >> 12) & 0x3F) as usize] as char);
                out.push(alphabet[((n >> 6) & 0x3F) as usize] as char);
                out.push('=');
            }
            _ => {}
        }
        out
    }

    #[test]
    fn parses_single_delegation_profile() {
        let pk_b64 = ecdsa_p256_test_pubkey_base64();
        let xml = alloc::format!(
            r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile name="vehicle-internal">
      <zerodds:trust_policy>direct-or-delegated</zerodds:trust_policy>
      <zerodds:max_chain_depth>3</zerodds:max_chain_depth>
      <zerodds:require_ocsp>false</zerodds:require_ocsp>
      <zerodds:allowed_algorithms>
        <zerodds:algorithm>ecdsa-p256</zerodds:algorithm>
        <zerodds:algorithm>ed25519</zerodds:algorithm>
      </zerodds:allowed_algorithms>
      <zerodds:trust_anchors>
        <zerodds:anchor subject_guid="01020304050607080910111213141516"
                        algorithm="ecdsa-p256"
                        public_key="{pk_b64}" />
      </zerodds:trust_anchors>
    </zerodds:profile>
  </zerodds:delegation_profiles>
</dds>"#
        );
        let g = parse_governance_xml(&xml).unwrap();
        assert_eq!(g.delegation_profiles.len(), 1);
        let p = g.delegation_profiles.get("vehicle-internal").unwrap();
        assert_eq!(p.name, "vehicle-internal");
        assert!(matches!(p.trust_policy, TrustPolicy::DirectOrDelegated));
        assert_eq!(p.max_chain_depth, 3);
        assert!(!p.require_ocsp);
        assert!(
            p.allowed_algorithms
                .contains(&SignatureAlgorithm::EcdsaP256.wire_id())
        );
        assert!(
            p.allowed_algorithms
                .contains(&SignatureAlgorithm::Ed25519.wire_id())
        );
        assert_eq!(p.trust_anchors.len(), 1);
        let a = &p.trust_anchors[0];
        assert_eq!(a.subject_guid[0], 0x01);
        assert_eq!(a.subject_guid[15], 0x16);
        assert!(matches!(a.algorithm, SignatureAlgorithm::EcdsaP256));
    }

    #[test]
    fn parses_all_four_trust_policies() {
        for (xml_val, expected) in [
            ("gateway-only", TrustPolicy::GatewayOnly),
            ("direct-or-delegated", TrustPolicy::DirectOrDelegated),
            ("federation", TrustPolicy::Federation),
            ("strict-delegated", TrustPolicy::StrictDelegated),
        ] {
            assert_eq!(parse_trust_policy(xml_val), Some(expected));
        }
        assert!(parse_trust_policy("unknown").is_none());
    }

    #[test]
    fn parses_all_four_algorithms() {
        assert_eq!(
            parse_algorithm("ecdsa-p256"),
            Some(SignatureAlgorithm::EcdsaP256)
        );
        assert_eq!(
            parse_algorithm("ECDSA-P384"),
            Some(SignatureAlgorithm::EcdsaP384)
        );
        assert_eq!(
            parse_algorithm("rsa-pss-2048"),
            Some(SignatureAlgorithm::RsaPss2048)
        );
        assert_eq!(
            parse_algorithm("ed25519"),
            Some(SignatureAlgorithm::Ed25519)
        );
        assert!(parse_algorithm("xyz").is_none());
    }

    #[test]
    fn unknown_trust_policy_falls_back_to_default() {
        let pk = ecdsa_p256_test_pubkey_base64();
        let xml = alloc::format!(
            r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile name="bad">
      <zerodds:trust_policy>nonsense-mode</zerodds:trust_policy>
      <zerodds:trust_anchors>
        <zerodds:anchor subject_guid="01020304050607080910111213141516"
                        algorithm="ecdsa-p256"
                        public_key="{pk}" />
      </zerodds:trust_anchors>
    </zerodds:profile>
  </zerodds:delegation_profiles>
</dds>"#
        );
        let g = parse_governance_xml(&xml).unwrap();
        let p = g.delegation_profiles.get("bad").unwrap();
        // Default = DirectOrDelegated if the value is not parseable.
        assert!(matches!(p.trust_policy, TrustPolicy::DirectOrDelegated));
    }

    #[test]
    fn anchor_with_invalid_guid_is_error() {
        let pk = ecdsa_p256_test_pubkey_base64();
        let xml = alloc::format!(
            r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile name="bad">
      <zerodds:trust_anchors>
        <zerodds:anchor subject_guid="ZZ"
                        algorithm="ecdsa-p256"
                        public_key="{pk}" />
      </zerodds:trust_anchors>
    </zerodds:profile>
  </zerodds:delegation_profiles>
</dds>"#
        );
        let err = parse_governance_xml(&xml).expect_err("must fail");
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    #[test]
    fn anchor_without_public_key_is_error() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile name="bad">
      <zerodds:trust_anchors>
        <zerodds:anchor subject_guid="01020304050607080910111213141516"
                        algorithm="ecdsa-p256" />
      </zerodds:trust_anchors>
    </zerodds:profile>
  </zerodds:delegation_profiles>
</dds>"#;
        let err = parse_governance_xml(XML).expect_err("must fail");
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    #[test]
    fn delegation_profile_without_namespace_is_ignored() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds>
  <delegation_profiles>
    <profile name="ignored" />
  </delegation_profiles>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        assert!(g.delegation_profiles.is_empty());
    }

    #[test]
    fn profile_without_name_is_error() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile />
  </zerodds:delegation_profiles>
</dds>"#;
        let err = parse_governance_xml(XML).expect_err("must fail");
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    #[test]
    fn profile_with_two_anchors_for_federation() {
        let pk1 = ecdsa_p256_test_pubkey_base64();
        let pk2 = ecdsa_p256_test_pubkey_base64();
        let xml = alloc::format!(
            r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile name="federation">
      <zerodds:trust_policy>federation</zerodds:trust_policy>
      <zerodds:max_chain_depth>5</zerodds:max_chain_depth>
      <zerodds:allowed_algorithms>
        <zerodds:algorithm>ecdsa-p256</zerodds:algorithm>
      </zerodds:allowed_algorithms>
      <zerodds:trust_anchors>
        <zerodds:anchor subject_guid="01020304050607080910111213141516"
                        algorithm="ecdsa-p256"
                        public_key="{pk1}" />
        <zerodds:anchor subject_guid="aabbccddeeff00112233445566778899"
                        algorithm="ecdsa-p256"
                        public_key="{pk2}" />
      </zerodds:trust_anchors>
    </zerodds:profile>
  </zerodds:delegation_profiles>
</dds>"#
        );
        let g = parse_governance_xml(&xml).unwrap();
        let p = g.delegation_profiles.get("federation").unwrap();
        assert!(matches!(p.trust_policy, TrustPolicy::Federation));
        assert_eq!(p.max_chain_depth, 5);
        assert_eq!(p.trust_anchors.len(), 2);
    }

    #[test]
    fn edge_without_name_attribute_returns_error() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:edge_identities>
    <zerodds:edge guid_prefix="010203040506070809101112" />
  </zerodds:edge_identities>
</dds>"#;
        let err = parse_governance_xml(XML).expect_err("must fail");
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }
}
