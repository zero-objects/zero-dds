// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Governance XML parser (OMG DDS-Security 1.2 §10.4.1.2).
//!
//! The governance XML specifies per domain which **topic classes** must
//! be protected how (discovery protection, read/write access,
//! metadata/data protection kind). The file is typically
//! signed with the permissions CA and loaded by the access-control plugin
//! at participant start.
//!
//! # Scope
//!
//! * Parser for `<dds><domain_access_rules>` → `<domain_rule>` →
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
use zerodds_security::logging::{TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2};
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
    /// The spelling of the governance XSD (DDS-Security 1.2 §10.4.1.2.3).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Sign => "SIGN",
            Self::Encrypt => "ENCRYPT",
            Self::SignWithOriginAuthentication => "SIGN_WITH_ORIGIN_AUTHENTICATION",
            Self::EncryptWithOriginAuthentication => "ENCRYPT_WITH_ORIGIN_AUTHENTICATION",
        }
    }

    /// The ProtectionKind of the governance XSD (DDS-Security 1.2
    /// §10.4.1.2.3), case-insensitive. Any other text makes the document
    /// invalid: it was read as NONE, so that a typo in `ENCRYPT` turned the
    /// protection off.
    fn parse(s: &str) -> Result<Self, PermissionsError> {
        match s.trim().to_uppercase().as_str() {
            "NONE" => Ok(Self::None),
            "SIGN" => Ok(Self::Sign),
            "ENCRYPT" => Ok(Self::Encrypt),
            "SIGN_WITH_ORIGIN_AUTHENTICATION" => Ok(Self::SignWithOriginAuthentication),
            "ENCRYPT_WITH_ORIGIN_AUTHENTICATION" => Ok(Self::EncryptWithOriginAuthentication),
            other => Err(PermissionsError::Malformed(alloc::format!(
                "governance: {other:?} is not a ProtectionKind"
            ))),
        }
    }

    /// The ProtectionKind of the governance XSD (DDS-Security 1.2
    /// §10.4.1.2.3) as an `xs:string` enumeration is: exactly one of the five
    /// spellings, upper case. [`Self::parse`] also reads lower case and is
    /// for the attributes of the ZeroDDS extensions.
    fn parse_xsd(s: &str) -> Result<Self, PermissionsError> {
        match s.trim() {
            "NONE" => Ok(Self::None),
            "SIGN" => Ok(Self::Sign),
            "ENCRYPT" => Ok(Self::Encrypt),
            "SIGN_WITH_ORIGIN_AUTHENTICATION" => Ok(Self::SignWithOriginAuthentication),
            "ENCRYPT_WITH_ORIGIN_AUTHENTICATION" => Ok(Self::EncryptWithOriginAuthentication),
            other => Err(PermissionsError::Malformed(alloc::format!(
                "governance: {other:?} is not a ProtectionKind of the XSD"
            ))),
        }
    }

    /// [`Self::parse_xsd`] of an element's text; an element without text is
    /// no ProtectionKind either.
    fn of(node: roxmltree::Node<'_, '_>) -> Result<Self, PermissionsError> {
        crate::xml::check_known_element(node, "governance")?;
        Self::parse_xsd(node.text().unwrap_or(""))
    }

    /// The `BasicProtectionKind` of the XSD (NONE, SIGN, ENCRYPT) of an
    /// element's text: `data_protection_kind` and
    /// `rtps_psk_protection_kind`.
    fn basic_of(node: roxmltree::Node<'_, '_>) -> Result<Self, PermissionsError> {
        let kind = Self::of(node)?;
        if kind.is_basic() {
            Ok(kind)
        } else {
            Err(PermissionsError::Malformed(alloc::format!(
                "governance: <{}> is a BasicProtectionKind (NONE, SIGN, ENCRYPT), not {}",
                node.tag_name().name(),
                kind.as_str()
            )))
        }
    }

    /// `true` for NONE, SIGN and ENCRYPT (`BasicProtectionKind`).
    #[must_use]
    pub const fn is_basic(self) -> bool {
        matches!(self, Self::None | Self::Sign | Self::Encrypt)
    }

    /// `true` unless NONE.
    #[must_use]
    pub const fn is_protected(self) -> bool {
        !matches!(self, Self::None)
    }

    /// `true` for ENCRYPT and ENCRYPT_WITH_ORIGIN_AUTHENTICATION: the
    /// `is_*_encrypted` attributes of §10.4.2.3 and §10.4.2.5.
    #[must_use]
    pub const fn is_encrypted(self) -> bool {
        matches!(self, Self::Encrypt | Self::EncryptWithOriginAuthentication)
    }

    /// `true` for SIGN_WITH_ORIGIN_AUTHENTICATION and
    /// ENCRYPT_WITH_ORIGIN_AUTHENTICATION: the `is_*_origin_authenticated`
    /// attributes.
    #[must_use]
    pub const fn is_origin_authenticated(self) -> bool {
        matches!(
            self,
            Self::SignWithOriginAuthentication | Self::EncryptWithOriginAuthentication
        )
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

/// The topic rule DDS-Security 1.2 §10.6 bakes into the
/// `DDS:Access:Permissions` plugin for the two BuiltinLoggingTopic names.
///
/// The clause's rule allows every participant that may join the domain to
/// write the topic (`enable_write_access_control = FALSE`) and requires a
/// permissions grant to read it (`enable_read_access_control = TRUE`), with
/// `SIGN` metadata and `ENCRYPT` data protection. It is builtin, so it wins
/// over a governance `<topic_rule>` that would otherwise match one of these
/// names — a catch-all `<topic_expression>*</topic_expression>` must not turn
/// logging into a write-controlled topic.
///
/// `None` for every other topic name.
#[must_use]
pub fn builtin_topic_rule(topic_name: &str) -> Option<TopicRule> {
    match topic_name {
        TOPIC_LOG_TOPIC | TOPIC_LOG_TOPIC_V2 => Some(TopicRule {
            topic_expression: TOPIC_LOG_TOPIC.to_string(),
            enable_discovery_protection: false,
            enable_liveliness_protection: false,
            enable_read_access_control: true,
            enable_write_access_control: false,
            metadata_protection_kind: ProtectionKind::Sign,
            data_protection_kind: ProtectionKind::Encrypt,
        }),
        _ => None,
    }
}

/// A DDS domain as DDS-Security 1.2 §10.4.1.2.5.1 identifies it: its
/// DomainId and its DomainTag. A participant that names no tag has the
/// tag `""`; a bare domain id converts to that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain<'a> {
    /// The DomainId.
    pub id: u32,
    /// The DomainTag, `""` for none.
    pub tag: &'a str,
}

impl From<u32> for Domain<'static> {
    fn from(id: u32) -> Self {
        Self { id, tag: "" }
    }
}

impl<'a> From<(u32, &'a str)> for Domain<'a> {
    fn from((id, tag): (u32, &'a str)) -> Self {
        Self { id, tag }
    }
}

/// A `<tag>` or `<tag_expression>` of a `<domains>` element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainTagMatch {
    /// `<tag>`: the DomainTag itself.
    Tag(String),
    /// `<tag_expression>`: an `fnmatch()` expression over the DomainTag.
    Expression(String),
}

/// `<domains>` (DomainSet of DDS-Security 1.2 §10.4.1.2.5.1, also used by
/// the permissions document): id ranges, a single id stored as
/// `min == max`, and DomainTags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainFilter {
    /// Inclusive ranges. If empty: matches all domain ids.
    pub ranges: Vec<(u32, u32)>,
    /// The DomainTags. "If no DomainTag and DomainTagExpression is
    /// specified, then it shall be treated as if the empty string ("") was
    /// specified as the only DomainTag value."
    pub tags: Vec<DomainTagMatch>,
}

impl DomainFilter {
    /// `true` if the domain's id is in a range (or the filter has no range)
    /// and its tag is one of the tags, `""` when the filter names none. The
    /// tags were read and not applied: a rule for `Robot15` held for every
    /// tag.
    #[must_use]
    pub fn matches<'a>(&self, domain: impl Into<Domain<'a>>) -> bool {
        let domain = domain.into();
        let id_ok = self.ranges.is_empty()
            || self
                .ranges
                .iter()
                .any(|(lo, hi)| domain.id >= *lo && domain.id <= *hi);
        let tag_ok = if self.tags.is_empty() {
            domain.tag.is_empty()
        } else {
            self.tags.iter().any(|t| match t {
                DomainTagMatch::Tag(tag) => tag == domain.tag,
                DomainTagMatch::Expression(e) => topic_match(e, domain.tag),
            })
        };
        id_ok && tag_ok
    }
}

/// A domain rule in the governance XML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainRule {
    /// Filter for domain ids.
    pub domains: DomainFilter,
    /// Allows unauthenticated participants in discovery. Without the
    /// element TRUE (Table 70, `get_participant_security_config`).
    pub allow_unauthenticated_participants: bool,
    /// Mandatory access control on participant join; without the element
    /// TRUE (Table 70: `is_access_protected`).
    pub enable_join_access_control: bool,
    /// Discovery protection at participant level.
    pub discovery_protection_kind: ProtectionKind,
    /// Liveliness protection at participant level.
    pub liveliness_protection_kind: ProtectionKind,
    /// Signature protection for the RTPS header.
    pub rtps_protection_kind: ProtectionKind,
    /// `enable_key_revision` (§10.4.1.2.5.4): the participant revises its key
    /// material on certain events (`is_key_revision_enabled`). Without the
    /// element FALSE.
    pub enable_key_revision: bool,
    /// `rtps_psk_protection_kind` (§10.4.1.2.5.8, a `BasicProtectionKind`):
    /// the protection of the RTPS messages that no key exchanged after
    /// authentication protects, with a pre-shared key. Without the element
    /// NONE.
    pub rtps_psk_protection_kind: ProtectionKind,
    /// `allowed_crypto_algorithms` (§10.4.1.2.5.9); `None` without the
    /// section: every algorithm of §8 is allowed.
    pub allowed_crypto_algorithms: Option<AllowedCryptoAlgorithms>,
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

/// `<allowed_crypto_algorithms>` (DDS-Security 1.2 §10.4.1.2.5.9): the
/// algorithms that may be used in the domain, as `CryptoAlgorithmBit` masks
/// of §8.1 to §8.3. An element that is absent is `None`: all algorithms of
/// §8 are allowed (§10.4.1.2.5.9.1 to §10.4.1.2.5.9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AllowedCryptoAlgorithms {
    /// `<digital_signature>`: the signatures with the participant's key
    /// (Table 25).
    pub digital_signature: Option<u32>,
    /// `<digital_signature_trust_chain>` (the XSD spells it
    /// `digital_signature_identity_trust_chain`, both are read): the
    /// signatures of the certificates and documents. Without the element the
    /// set of `<digital_signature>`.
    pub digital_signature_trust_chain: Option<u32>,
    /// `<key_establishment>` (Table 26).
    pub key_establishment: Option<u32>,
    /// `<symmetric_cipher>` (Table 22).
    pub symmetric_cipher: Option<u32>,
}

/// Peer class from `<zerodds:peer_class>` (RC1, spec: architecture
/// doc §5).
///
/// Each remote peer is mapped to a peer class by its `zerodds_security_runtime::PeerCapabilities` +
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
    /// How the partitions of a permissions allow rule match; not part of
    /// the document (§10.4.1.5.3.4.1.4).
    pub partition_matching: crate::xml::PartitionMatching,
}

impl Governance {
    /// Finds the matching [`DomainRule`] for a domain id. `None`
    /// if none matches — the caller decides the default policy.
    #[must_use]
    pub fn find_domain_rule<'d>(&self, domain: impl Into<Domain<'d>>) -> Option<&DomainRule> {
        let domain = domain.into();
        self.domain_rules.iter().find(|r| r.domains.matches(domain))
    }

    /// Finds the matching [`TopicRule`] within a domain rule.
    /// The first match in `topic_rules` wins; if no rule matches,
    /// `TopicRule::default()` (no protection) is returned.
    #[must_use]
    pub fn find_topic_rule<'a, 'd>(
        &'a self,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
    ) -> Option<&'a TopicRule> {
        let dr = self.find_domain_rule(domain)?;
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
    pub fn check_create_participant<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        now: u64,
    ) -> bool {
        let domain = domain.into();
        match self.find_domain_rule(domain) {
            None => false,
            Some(dr) => {
                if !dr.enable_join_access_control {
                    return true;
                }
                // Table 70: the grant's first matching rule for the domain
                // decides; any grant with the domain in a rule was taken.
                permissions
                    .grants
                    .iter()
                    .any(|g| g.is_valid_at(now) && g.may_join(domain))
            }
        }
    }

    /// DDS-Security 1.2 §10.4.1.2.7: "If no Topic Rule matches the Entity
    /// topic name the operation under consideration shall fail with a
    /// suitable 'permissions error'." `true` when a topic rule of the
    /// domain's rule, or the builtin rule of the logging topic, covers
    /// `topic_name`.
    #[must_use]
    pub fn has_topic_rule<'d>(&self, domain: impl Into<Domain<'d>>, topic_name: &str) -> bool {
        builtin_topic_rule(topic_name).is_some()
            || self.find_topic_rule(domain, topic_name).is_some()
    }

    /// The [`TopicRule`] that decides `topic_name`, after the builtin rules
    /// of DDS-Security 1.2 §10.6.
    ///
    /// A builtin rule (the two BuiltinLoggingTopic names) wins over the
    /// governance document; otherwise the first matching governance
    /// `<topic_rule>` decides, and no match is [`TopicRule::default`] (no
    /// protection).
    #[must_use]
    pub fn effective_topic_rule<'d>(
        &self,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
    ) -> TopicRule {
        if let Some(builtin) = builtin_topic_rule(topic_name) {
            return builtin;
        }
        self.find_topic_rule(domain, topic_name)
            .cloned()
            .unwrap_or_default()
    }

    /// `true` when the governance gates DataWriter creation on `topic_name`
    /// behind the permissions — `<enable_write_access_control>` of the
    /// **matching** topic rule (§9.4.1.2 Table 47), not of the first one.
    #[must_use]
    pub fn write_access_controlled<'d>(
        &self,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
    ) -> bool {
        self.effective_topic_rule(domain, topic_name)
            .enable_write_access_control
    }

    /// `true` when the governance gates DataReader creation on `topic_name`
    /// behind the permissions — `<enable_read_access_control>` of the
    /// matching topic rule.
    #[must_use]
    pub fn read_access_controlled<'d>(
        &self,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
    ) -> bool {
        self.effective_topic_rule(domain, topic_name)
            .enable_read_access_control
    }

    /// DDS-Security 1.2 §9.4.2.9.4 `check_create_datawriter`, Table 70: the
    /// topic is not write-access-controlled, or the grant allows publishing
    /// it on all the Publisher's partitions with all the DataWriter's data
    /// tags. No domain rule, or no topic rule (§10.4.1.2.7), denies. Also
    /// `check_remote_datawriter` with the grant of the remote participant.
    #[must_use]
    pub fn check_create_datawriter<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
        partitions: &[&str],
        now: u64,
    ) -> bool {
        self.check_datawriter(permissions, domain, topic_name, partitions, &[], now)
    }

    /// [`Self::check_create_datawriter`] with the data tags of the writer.
    #[must_use]
    pub fn check_datawriter<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
        partitions: &[&str],
        data_tags: &[crate::xml::DataTag],
        now: u64,
    ) -> bool {
        let domain = domain.into();
        if self.find_domain_rule(domain).is_none() || !self.has_topic_rule(domain, topic_name) {
            return false;
        }
        if !self.write_access_controlled(domain, topic_name) {
            return true;
        }
        self.granted(
            permissions,
            &crate::xml::AccessRequest {
                action: crate::xml::Action::Publish,
                domain,
                topic: topic_name,
                partitions,
                data_tags,
            },
            now,
        )
    }

    /// DDS-Security 1.2 §9.4.2.9.5 `check_create_datareader` — the mirror of
    /// [`Self::check_create_datawriter`] over `<enable_read_access_control>`
    /// and `<subscribe>`.
    #[must_use]
    pub fn check_create_datareader<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
        partitions: &[&str],
        now: u64,
    ) -> bool {
        self.check_datareader(permissions, domain, topic_name, partitions, &[], now)
            == ReaderAccess::Allowed
    }

    /// The reader checks of Table 70 with the data tags of the reader:
    /// [`ReaderAccess::Allowed`] when the topic is not read-access-controlled
    /// or the grant allows subscribing; [`ReaderAccess::RelayOnly`] when it
    /// allows only relaying the topic (`check_remote_datareader`,
    /// `allow_relay_only`; §9.4.2.9.11). `<relay>` was not read.
    #[must_use]
    pub fn check_datareader<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
        partitions: &[&str],
        data_tags: &[crate::xml::DataTag],
        now: u64,
    ) -> ReaderAccess {
        let domain = domain.into();
        if self.find_domain_rule(domain).is_none() || !self.has_topic_rule(domain, topic_name) {
            return ReaderAccess::Denied;
        }
        if !self.read_access_controlled(domain, topic_name) {
            return ReaderAccess::Allowed;
        }
        let mut req = crate::xml::AccessRequest {
            action: crate::xml::Action::Subscribe,
            domain,
            topic: topic_name,
            partitions,
            data_tags,
        };
        if self.granted(permissions, &req, now) {
            return ReaderAccess::Allowed;
        }
        req.action = crate::xml::Action::Relay;
        if self.granted(permissions, &req, now) {
            return ReaderAccess::RelayOnly;
        }
        ReaderAccess::Denied
    }

    /// DDS-Security 1.2 §7.1.1.4 and the `<relay>` action of
    /// §10.4.1.5.3.4.1.2 on the receiving side: may the participant of
    /// `permissions` relay `topic_name` on all the partitions with all the
    /// data tags. A relay reader receives and stores the topic without the
    /// key to its data; it needs this where the topic is
    /// read-access-controlled, and everyone may relay a topic that is not.
    /// No domain rule, or no topic rule (§10.4.1.2.7), denies.
    #[must_use]
    pub fn check_relay_reader<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
        partitions: &[&str],
        data_tags: &[crate::xml::DataTag],
        now: u64,
    ) -> bool {
        let domain = domain.into();
        if self.find_domain_rule(domain).is_none() || !self.has_topic_rule(domain, topic_name) {
            return false;
        }
        !self.read_access_controlled(domain, topic_name)
            || self.granted_relay(permissions, domain, topic_name, partitions, data_tags, now)
    }

    /// DDS-Security 1.2 §7.1.1.4 on the forwarding side: "Bob, Mallory and
    /// others need to accept Trent as someone who is able to write on Topic
    /// T and relay messages from Alice". A writer whose participant's grant
    /// allows `<relay>` on `topic_name` is taken as forwarder: a sample it
    /// sends on behalf of another writer counts as that writer's. On a topic
    /// that is not write-access-controlled every writer is. No domain rule,
    /// or no topic rule (§10.4.1.2.7), denies.
    #[must_use]
    pub fn check_relay_writer<'d>(
        &self,
        permissions: &crate::xml::Permissions,
        domain: impl Into<Domain<'d>>,
        topic_name: &str,
        partitions: &[&str],
        data_tags: &[crate::xml::DataTag],
        now: u64,
    ) -> bool {
        let domain = domain.into();
        if self.find_domain_rule(domain).is_none() || !self.has_topic_rule(domain, topic_name) {
            return false;
        }
        !self.write_access_controlled(domain, topic_name)
            || self.granted_relay(permissions, domain, topic_name, partitions, data_tags, now)
    }

    /// A grant of `permissions` allows `<relay>` on the topic.
    fn granted_relay(
        &self,
        permissions: &crate::xml::Permissions,
        domain: Domain<'_>,
        topic_name: &str,
        partitions: &[&str],
        data_tags: &[crate::xml::DataTag],
        now: u64,
    ) -> bool {
        self.granted(
            permissions,
            &crate::xml::AccessRequest {
                action: crate::xml::Action::Relay,
                domain,
                topic: topic_name,
                partitions,
                data_tags,
            },
            now,
        )
    }

    /// How the partitions of an allow rule match; [`Self::set_partition_matching`].
    #[must_use]
    pub fn partition_matching(&self) -> crate::xml::PartitionMatching {
        self.partition_matching
    }

    /// Selects the "legacy matching behavior" of §10.4.1.5.3.4.1.4 ("For
    /// legacy reasons DDS-Security implementations shall provide a way to
    /// select an alternative 'legacy matching' behavior") or the default.
    pub fn set_partition_matching(&mut self, matching: crate::xml::PartitionMatching) {
        self.partition_matching = matching;
    }

    /// A grant of `permissions` valid at `now` allows `req`. The document
    /// holds the grant of the participant's subject
    /// ([`crate::xml::Permissions::for_subject`]).
    fn granted(
        &self,
        permissions: &crate::xml::Permissions,
        req: &crate::xml::AccessRequest<'_>,
        now: u64,
    ) -> bool {
        permissions
            .grants
            .iter()
            .any(|g| g.is_valid_at(now) && g.decide(req, self.partition_matching).allowed())
    }
}

/// The outcome of the reader checks of Table 70.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderAccess {
    /// The reader may subscribe.
    Allowed,
    /// The reader may only relay the topic (`allow_relay_only`).
    RelayOnly,
    /// Refused.
    Denied,
}

/// Parses a governance XML document.
///
/// Accepts the spec schema from §9.4.1.2:
/// ```xml
/// <dds>
///   <dds><domain_access_rules>
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
///   </domain_access_rules></dds>
/// </dds>
/// ```
///
/// # Errors
/// See [`PermissionsError`] (reused — governance and
/// permissions share the XML-parse error class).
pub fn parse_governance_xml(xml: &str) -> Result<Governance, PermissionsError> {
    zerodds_foundation::check_nesting(xml, zerodds_foundation::XML_NESTING_LIMIT)
        .map_err(|e| PermissionsError::InvalidXml(e.to_string()))?;
    let doc =
        roxmltree::Document::parse(xml).map_err(|e| PermissionsError::InvalidXml(e.to_string()))?;
    let root = doc.root_element();
    let mut rules = Vec::new();
    parse_dds_root(root, &mut rules)?;
    let mut edge_identities = Vec::new();
    walk_edge_identities(root, &mut edge_identities)?;
    let mut delegation_profiles = BTreeMap::new();
    walk_delegation_profiles(root, &mut delegation_profiles)?;
    Ok(Governance {
        domain_rules: rules,
        edge_identities,
        delegation_profiles,
        partition_matching: crate::xml::PartitionMatching::default(),
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
                // A duplicate profile name is rejected, not overwritten: the
                // map is keyed by the bare `name` attribute, and a second
                // <profile> of the same name would silently replace the trust
                // policy the first one declared.
                if out.contains_key(&p.name) {
                    return Err(PermissionsError::InvalidXml(alloc::format!(
                        "<profile> name '{}' declared twice",
                        p.name
                    )));
                }
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
                require_ocsp = parse_bool(child)?;
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

/// The document after the XSD of §10.4.1.2.3: `<dds>` holds one
/// `<domain_access_rules>`, which holds the `<domain_rule>` elements. A
/// `<domain_rule>` anywhere else was taken (the tree was searched for it); an
/// element outside the XSD is a governance extension element (§10.4.1.3),
/// the elements of the ZeroDDS namespace are read by their own walkers.
fn parse_dds_root(
    root: roxmltree::Node<'_, '_>,
    out: &mut Vec<DomainRule>,
) -> Result<(), PermissionsError> {
    // The XSD files of the specification declare no namespace; the sample
    // files of the OMG suite put the elements into `http://www.omg.org/spec/
    // DDS-SECURITY/<date>/omg_shared_ca_governance`.
    let omg_namespace = root
        .tag_name()
        .namespace()
        .is_none_or(|ns| ns.starts_with("http://www.omg.org/spec/DDS-SECURITY/"));
    if root.tag_name().name() != "dds" || !omg_namespace {
        return Err(PermissionsError::Malformed(alloc::format!(
            "governance: root element <{}>, expected <dds>",
            root.tag_name().name()
        )));
    }
    crate::xml::check_known_element(root, "governance")?;
    let mut seen_rules = false;
    for child in root.children().filter(roxmltree::Node::is_element) {
        if child.tag_name().namespace() == Some(ZERODDS_NS) {
            crate::xml::check_must_interpret_value(child, "governance")?;
            continue;
        }
        if child.tag_name().name() == "domain_access_rules" && !seen_rules {
            seen_rules = true;
            crate::xml::check_known_element(child, "governance")?;
            for rule in child.children().filter(roxmltree::Node::is_element) {
                if rule.tag_name().namespace() == Some(ZERODDS_NS) {
                    crate::xml::check_must_interpret_value(rule, "governance")?;
                } else if rule.has_tag_name("domain_rule") {
                    out.push(parse_domain_rule(rule)?);
                } else {
                    extension_element(rule)?;
                }
            }
        } else {
            extension_element(child)?;
        }
    }
    if !seen_rules {
        return Err(PermissionsError::Malformed(
            "governance: <dds> without <domain_access_rules>".into(),
        ));
    }
    if out.is_empty() {
        return Err(PermissionsError::Malformed(
            "governance: <domain_access_rules> without <domain_rule>".into(),
        ));
    }
    Ok(())
}

fn parse_domain_rule(rule: roxmltree::Node<'_, '_>) -> Result<DomainRule, PermissionsError> {
    // Table 70 (`get_participant_security_config`): only an element set to
    // FALSE makes either FALSE. A missing element was read as FALSE.
    let mut out = DomainRule {
        allow_unauthenticated_participants: true,
        enable_join_access_control: true,
        ..DomainRule::default()
    };
    let mut has_domains = false;
    crate::xml::check_known_element(rule, "governance")?;
    for child in rule.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "domains" => {
                out.domains = parse_domain_filter(child)?;
                has_domains = true;
            }
            "allow_unauthenticated_participants" => {
                out.allow_unauthenticated_participants = parse_bool(child)?;
            }
            "enable_join_access_control" => {
                out.enable_join_access_control = parse_bool(child)?;
            }
            "discovery_protection_kind" => {
                out.discovery_protection_kind = ProtectionKind::of(child)?;
            }
            "liveliness_protection_kind" => {
                out.liveliness_protection_kind = ProtectionKind::of(child)?;
            }
            "rtps_protection_kind" => out.rtps_protection_kind = ProtectionKind::of(child)?,
            // `get_participant_security_config` returns them (§10.4.1.2.5.4,
            // §10.4.1.2.5.8, §10.4.1.2.5.9, Table 70).
            "enable_key_revision" => out.enable_key_revision = parse_bool(child)?,
            "rtps_psk_protection_kind" => {
                out.rtps_psk_protection_kind = ProtectionKind::basic_of(child)?
            }
            "allowed_crypto_algorithms" => {
                out.allowed_crypto_algorithms = Some(parse_allowed_crypto_algorithms(child)?);
            }
            "topic_access_rules" => {
                crate::xml::check_known_element(child, "governance")?;
                for tr in child.children().filter(roxmltree::Node::is_element) {
                    if tr.has_tag_name("topic_rule") {
                        out.topic_rules.push(parse_topic_rule(tr)?);
                    } else {
                        extension_element(tr)?;
                    }
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
                        out.peer_classes.push(parse_peer_class(pc)?);
                    }
                }
            }
            "interface_bindings" if child.tag_name().namespace() == Some(ZERODDS_NS) => {
                for ib in child.children().filter(roxmltree::Node::is_element) {
                    if ib.tag_name().name() == "interface"
                        && ib.tag_name().namespace() == Some(ZERODDS_NS)
                    {
                        out.interface_bindings.push(parse_interface_binding(ib)?);
                    }
                }
            }
            _ => extension_element(child)?,
        }
    }
    // An absent <domains> would match every domain (`DomainFilter::matches`).
    if !has_domains {
        return Err(PermissionsError::Malformed(
            "governance: <domain_rule> without <domains>".into(),
        ));
    }
    // §9.4.2.4 Table 33, is_rtps_axk_protected: "If is_rtps_axk_protected is
    // TRUE then: (1) allow_unauthenticated_participants must be FALSE." The
    // combination was taken.
    if out.rtps_protection_kind != ProtectionKind::None && out.allow_unauthenticated_participants {
        return Err(PermissionsError::Malformed(
            "governance: rtps_protection_kind other than NONE requires \
             allow_unauthenticated_participants FALSE"
                .into(),
        ));
    }
    Ok(out)
}

/// DDS-Security 1.2 §10.4.1.3: an element the XSD does not define is a
/// "governance extension element". With `must_interpret` "false" or
/// "FALSE" it is ignored; without the attribute, or with it "true", "the
/// governance document shall be considered invalid, and an error shall be
/// raised". It was ignored in every case.
fn extension_element(node: roxmltree::Node<'_, '_>) -> Result<(), PermissionsError> {
    crate::xml::extension_element_of(node, "governance")
}

fn parse_peer_class(node: roxmltree::Node<'_, '_>) -> Result<PeerClass, PermissionsError> {
    let mut out = PeerClass {
        name: node.attribute("name").unwrap_or("").to_string(),
        protection: node
            .attribute("protection")
            .map(ProtectionKind::parse)
            .transpose()?
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
    Ok(out)
}

fn parse_interface_binding(
    node: roxmltree::Node<'_, '_>,
) -> Result<InterfaceBindingRule, PermissionsError> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let protection_override = node
        .attribute("protection_override")
        .map(ProtectionKind::parse)
        .transpose()?;
    let protection_min = node
        .attribute("protection_min")
        .map(ProtectionKind::parse)
        .transpose()?;
    let peer_class_filter = node
        .attribute("peer_class_filter")
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Ok(InterfaceBindingRule {
        name,
        protection_override,
        peer_class_filter,
        protection_min,
    })
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

/// `<domains>` of the governance XSD (DomainSet): at least one `<id>` or
/// `<id_range>`, each a non-negative integer. A value that is none, or a
/// `<domains>` without one, makes the document invalid; such a rule was
/// read as one for every domain. `<tag>` and `<tag_expression>` name the
/// DomainTags (§10.4.1.2.5.1).
pub(crate) fn parse_domain_filter(
    node: roxmltree::Node<'_, '_>,
) -> Result<DomainFilter, PermissionsError> {
    let id = |n: roxmltree::Node<'_, '_>| -> Result<u32, PermissionsError> {
        let text = n.text().unwrap_or("").trim();
        text.parse::<u32>().map_err(|_| {
            PermissionsError::Malformed(alloc::format!("governance: domain id {text:?}"))
        })
    };
    let mut ranges = Vec::new();
    let mut tags = Vec::new();
    crate::xml::check_known_element(node, "governance")?;
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "id" => {
                crate::xml::check_known_element(child, "governance")?;
                let n = id(child)?;
                ranges.push((n, n));
            }
            "tag" => {
                crate::xml::check_known_element(child, "governance")?;
                tags.push(DomainTagMatch::Tag(
                    child.text().unwrap_or("").trim().to_string(),
                ));
            }
            "tag_expression" => {
                crate::xml::check_known_element(child, "governance")?;
                tags.push(DomainTagMatch::Expression(
                    child.text().unwrap_or("").trim().to_string(),
                ));
            }
            "id_range" => {
                crate::xml::check_known_element(child, "governance")?;
                let mut lo = None;
                let mut hi = None;
                for bound in child.children().filter(roxmltree::Node::is_element) {
                    match bound.tag_name().name() {
                        "min" => {
                            crate::xml::check_known_element(bound, "governance")?;
                            lo = Some(id(bound)?);
                        }
                        "max" => {
                            crate::xml::check_known_element(bound, "governance")?;
                            hi = Some(id(bound)?);
                        }
                        _ => extension_element(bound)?,
                    }
                }
                if lo.is_none() && hi.is_none() {
                    return Err(PermissionsError::Malformed(
                        "governance: <id_range> without <min> or <max>".into(),
                    ));
                }
                ranges.push((lo.unwrap_or(0), hi.unwrap_or(u32::MAX)));
            }
            _ => extension_element(child)?,
        }
    }
    if ranges.is_empty() {
        return Err(PermissionsError::Malformed(
            "governance: <domains> names no domain".into(),
        ));
    }
    Ok(DomainFilter { ranges, tags })
}

/// `<allowed_crypto_algorithms>` of the XSD (`AllowedCryptoAlgorithms`): each
/// of its four elements holds one or more `<algorithm>` with the
/// CryptoAlgorithmName of §8.1 (Table 22), §8.2 (Table 25) or §8.3
/// (Table 26); a name outside the XSD enumeration of the element makes the
/// document invalid.
fn parse_allowed_crypto_algorithms(
    node: roxmltree::Node<'_, '_>,
) -> Result<AllowedCryptoAlgorithms, PermissionsError> {
    crate::xml::check_known_element(node, "governance")?;
    let mut out = AllowedCryptoAlgorithms::default();
    for child in node.children().filter(roxmltree::Node::is_element) {
        let (slot, names): (&mut Option<u32>, &[(&str, u32)]) = match child.tag_name().name() {
            "digital_signature" => (&mut out.digital_signature, SIGNATURE_NAMES),
            "digital_signature_trust_chain" | "digital_signature_identity_trust_chain" => {
                (&mut out.digital_signature_trust_chain, SIGNATURE_NAMES)
            }
            "key_establishment" => (&mut out.key_establishment, KEY_ESTABLISHMENT_NAMES),
            "symmetric_cipher" => (&mut out.symmetric_cipher, SYMMETRIC_NAMES),
            _ => {
                extension_element(child)?;
                continue;
            }
        };
        crate::xml::check_known_element(child, "governance")?;
        if slot.is_some() {
            return Err(PermissionsError::Malformed(alloc::format!(
                "governance: <{}> twice in <allowed_crypto_algorithms>",
                child.tag_name().name()
            )));
        }
        let mut mask = 0u32;
        let mut any = false;
        for algorithm in child.children().filter(roxmltree::Node::is_element) {
            if !algorithm.has_tag_name("algorithm") {
                extension_element(algorithm)?;
                continue;
            }
            crate::xml::check_known_element(algorithm, "governance")?;
            let name = algorithm.text().unwrap_or("").trim();
            let Some((_, bit)) = names.iter().find(|(n, _)| *n == name) else {
                return Err(PermissionsError::Malformed(alloc::format!(
                    "governance: <algorithm> {name:?} in <{}> is no CryptoAlgorithmName of the XSD",
                    child.tag_name().name()
                )));
            };
            mask |= *bit;
            any = true;
        }
        if !any {
            return Err(PermissionsError::Malformed(alloc::format!(
                "governance: <{}> without <algorithm>",
                child.tag_name().name()
            )));
        }
        *slot = Some(mask);
    }
    Ok(out)
}

/// `DigitalSignatureKind` of the XSD with the bits of Table 25.
const SIGNATURE_NAMES: &[(&str, u32)] = &[
    (
        zerodds_security::algorithm::CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256,
        zerodds_security::algorithm::CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256,
    ),
    (
        zerodds_security::algorithm::CNAME_RSASSA_PKCS1_V15_2048_SHA256,
        zerodds_security::algorithm::CBIT_RSASSA_PKCS1_V15_2048_SHA256,
    ),
    (
        zerodds_security::algorithm::CNAME_ECDSA_P256_SHA256_NAME,
        zerodds_security::algorithm::CBIT_ECDSA_P256_SHA256,
    ),
    (
        zerodds_security::algorithm::CNAME_ECDSA_P384_SHA384,
        zerodds_security::algorithm::CBIT_ECDSA_P384_SHA384,
    ),
];

/// `KeyEstablishmentKind` of the XSD with the bits of Table 26.
const KEY_ESTABLISHMENT_NAMES: &[(&str, u32)] = &[
    (
        zerodds_security::algorithm::CNAME_DHE_MODP_2048_256,
        zerodds_security::algorithm::CBIT_DHE_MODP_2048_256,
    ),
    (
        zerodds_security::algorithm::CNAME_ECDHE_CEUM_P256,
        zerodds_security::algorithm::CBIT_ECDHE_CEUM_P256,
    ),
    (
        zerodds_security::algorithm::CNAME_ECDHE_CEUM_P384,
        zerodds_security::algorithm::CBIT_ECDHE_CEUM_P384,
    ),
];

/// `SymmetricCipherKind` of the XSD with the bits of Table 22 (the AEAD
/// names; the GMAC names equal them, §8.1).
const SYMMETRIC_NAMES: &[(&str, u32)] = &[
    (
        zerodds_security::algorithm::CNAME_AES128_GCM,
        zerodds_security::algorithm::CBIT_AES128_GCM,
    ),
    (
        zerodds_security::algorithm::CNAME_AES256_GCM,
        zerodds_security::algorithm::CBIT_AES256_GCM,
    ),
];

fn parse_topic_rule(node: roxmltree::Node<'_, '_>) -> Result<TopicRule, PermissionsError> {
    crate::xml::check_known_element(node, "governance")?;
    let mut out = TopicRule::default();
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "topic_expression" => {
                crate::xml::check_known_element(child, "governance")?;
                if let Some(t) = child.text() {
                    out.topic_expression = t.trim().to_string();
                }
            }
            "enable_discovery_protection" => out.enable_discovery_protection = parse_bool(child)?,
            "enable_liveliness_protection" => out.enable_liveliness_protection = parse_bool(child)?,
            "enable_read_access_control" => out.enable_read_access_control = parse_bool(child)?,
            "enable_write_access_control" => out.enable_write_access_control = parse_bool(child)?,
            "metadata_protection_kind" => out.metadata_protection_kind = ProtectionKind::of(child)?,
            "data_protection_kind" => out.data_protection_kind = ProtectionKind::basic_of(child)?,
            _ => extension_element(child)?,
        }
    }
    Ok(out)
}

/// An `xs:boolean` of the governance XSD: `true`, `false`, `1`, `0`, and
/// the upper-case `TRUE` and `FALSE` of the specification's examples. Any
/// other text makes the document invalid: it was read as FALSE, so that a
/// typo in `enable_read_access_control` turned the access control off.
fn parse_bool(node: roxmltree::Node<'_, '_>) -> Result<bool, PermissionsError> {
    crate::xml::check_known_element(node, "governance")?;
    let text = node.text().unwrap_or("").trim();
    match text.to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(PermissionsError::Malformed(alloc::format!(
            "governance: <{}> {text:?} is not a boolean",
            node.tag_name().name()
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// DDS-Security 1.2 §7.1.1.4, §10.4.1.5.3.4.1.2: a grant with `<relay>`
    /// on the topic makes a relay reader and a forwarder on an
    /// access-controlled topic; a grant to publish or subscribe does not.
    /// A topic without access control lets everyone relay, and a topic
    /// without a rule no one.
    #[test]
    fn relaying_needs_the_relay_action_where_access_is_controlled() {
        use crate::xml::parse_permissions_xml;
        let gov = |read: bool, write: bool| {
            parse_governance_xml(&alloc::format!(
                r#"<dds><domain_access_rules><domain_rule>
                     <domains><id>0</id></domains>
                     <enable_join_access_control>false</enable_join_access_control>
                     <topic_access_rules><topic_rule>
                       <topic_expression>T</topic_expression>
                       <enable_read_access_control>{read}</enable_read_access_control>
                       <enable_write_access_control>{write}</enable_write_access_control>
                     </topic_rule></topic_access_rules>
                   </domain_rule></domain_access_rules></dds>"#
            ))
            .unwrap()
        };
        let perms = |action: &str| {
            parse_permissions_xml(&alloc::format!(
                r#"<dds><permissions><grant><subject_name>CN=trent</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                     <allow_rule><domains><id>0</id></domains>
                       <{action}><topics><topic>T</topic></topics></{action}></allow_rule>
                     <default>DENY</default>
                   </grant></permissions></dds>"#
            ))
            .unwrap()
        };
        let controlled = gov(true, true);
        let relay = perms("relay");
        assert!(controlled.check_relay_reader(&relay, 0, "T", &[], &[], 0));
        assert!(controlled.check_relay_writer(&relay, 0, "T", &[], &[], 0));
        for other in ["publish", "subscribe"] {
            let p = perms(other);
            assert!(
                !controlled.check_relay_reader(&p, 0, "T", &[], &[], 0),
                "{other}"
            );
            assert!(
                !controlled.check_relay_writer(&p, 0, "T", &[], &[], 0),
                "{other}"
            );
        }
        let open = gov(false, false);
        let publish = perms("publish");
        assert!(open.check_relay_reader(&publish, 0, "T", &[], &[], 0));
        assert!(open.check_relay_writer(&publish, 0, "T", &[], &[], 0));
        assert!(
            !open.check_relay_writer(&relay, 0, "U", &[], &[], 0),
            "no topic rule"
        );
        // Read and write access control are separate.
        let read_only = gov(true, false);
        assert!(!read_only.check_relay_reader(&publish, 0, "T", &[], &[], 0));
        assert!(read_only.check_relay_writer(&publish, 0, "T", &[], &[], 0));
    }

    #[test]
    fn check_create_participant_consults_permissions() {
        use crate::xml::parse_permissions_xml;
        // Full-AC governance for domain 200 (join-AC + single * topic RW-AC).
        // This is NOT un-joinable per se — it is joinable by a participant
        // whose permissions grant the domain (§8.4.2.9.3), exactly as Cyclone
        // and Fast DDS treat it.
        let full_ac = parse_governance_xml(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>200</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants>
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
            r#"<dds><permissions><grant><subject_name>CN=ping</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                 <allow_rule><domains><id>200</id></domains>
                   <publish><topics><topic>*</topic></topics></publish></allow_rule>
               </grant></permissions></dds>"#,
        )
        .unwrap();
        assert!(
            full_ac.check_create_participant(&grant_200, 200, 0),
            "full-AC + matching grant → joinable (spec §8.4.2.9.3)"
        );

        // A grant for a different domain → deny (no grant covers domain 200).
        let grant_5 = parse_permissions_xml(
            r#"<dds><permissions><grant><subject_name>CN=ping</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                 <allow_rule><domains><id>5</id></domains>
                   <publish><topics><topic>*</topic></topics></publish></allow_rule>
               </grant></permissions></dds>"#,
        )
        .unwrap();
        assert!(
            !full_ac.check_create_participant(&grant_5, 200, 0),
            "no grant for the domain → deny"
        );

        // join-AC=false → allow regardless of permissions (open join).
        let open = parse_governance_xml(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>200</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants>
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

    /// A `<domains>` without a domain, or a rule without `<domains>`, is
    /// outside the XSD (DomainSet: at least one `<id>` or `<id_range>`). It
    /// was read as a rule for every domain.
    #[test]
    fn a_rule_without_a_domain_is_refused() {
        let empty = r#"
<dds><domain_access_rules>
  <domain_rule>
    <domains/>
    <topic_access_rules>
      <topic_rule><topic_expression>*</topic_expression></topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>"#;
        assert!(parse_governance_xml(empty).is_err());
        let missing = r#"
<dds><domain_access_rules>
  <domain_rule>
    <topic_access_rules>
      <topic_rule><topic_expression>*</topic_expression></topic_rule>
    </topic_access_rules>
  </domain_rule>
</domain_access_rules></dds>"#;
        assert!(parse_governance_xml(missing).is_err());
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
        assert_eq!(
            ProtectionKind::parse("encrypt").unwrap(),
            ProtectionKind::Encrypt
        );
        assert_eq!(ProtectionKind::parse("Sign").unwrap(), ProtectionKind::Sign);
        assert_eq!(ProtectionKind::parse("NONE").unwrap(), ProtectionKind::None);
        assert_eq!(
            ProtectionKind::parse("encrypt_with_origin_authentication").unwrap(),
            ProtectionKind::EncryptWithOriginAuthentication
        );
    }

    /// DDS-Security 1.2 §10.4.1.2.3 and §10.4.1.3: a value outside the XSD
    /// and an unknown element without `must_interpret="false"` make the
    /// governance invalid; with it, the element is ignored. A typo read as
    /// NONE or FALSE, an unreadable domain id as every domain.
    #[test]
    fn a_governance_outside_the_xsd_is_refused() {
        let rule = |inner: &str| {
            alloc::format!(
                r#"<dds><domain_access_rules><domain_rule>
                     <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
                     <rtps_protection_kind>NONE</rtps_protection_kind>
                     <topic_access_rules><topic_rule>
                       <topic_expression>*</topic_expression>
                       {inner}
                     </topic_rule></topic_access_rules>
                   </domain_rule></domain_access_rules></dds>"#
            )
        };
        assert!(
            parse_governance_xml(&rule(
                "<data_protection_kind>ENCRPYT</data_protection_kind>"
            ))
            .is_err()
        );
        assert!(
            parse_governance_xml(&rule(
                "<enable_read_access_control>ture</enable_read_access_control>"
            ))
            .is_err()
        );
        assert!(parse_governance_xml(&rule("<vendor_knob>1</vendor_knob>")).is_err());
        assert!(
            parse_governance_xml(&rule(
                r#"<vendor_knob must_interpret="false">1</vendor_knob>"#
            ))
            .is_ok()
        );
        let unreadable_domain = r#"<dds><domain_access_rules><domain_rule>
              <domains><id>zero</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
              <topic_access_rules/></domain_rule></domain_access_rules></dds>"#;
        assert!(parse_governance_xml(unreadable_domain).is_err());
        let no_domain = r#"<dds><domain_access_rules><domain_rule>
              <domains></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
              <topic_access_rules/></domain_rule></domain_access_rules></dds>"#;
        assert!(parse_governance_xml(no_domain).is_err());
    }

    /// A governance document of one domain rule for domain 0 with `rule`
    /// inside the `<domain_rule>`, after `<domains>`.
    fn document(rule: &str) -> alloc::string::String {
        alloc::format!(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>0</id></domains>
                 <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
                 <enable_join_access_control>false</enable_join_access_control>
                 {rule}
                 <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
               </domain_rule></domain_access_rules></dds>"#
        )
    }

    /// DDS-Security 1.2 §10.4.1.2.5.4, §10.4.1.2.5.8, §10.4.1.2.5.9: the
    /// elements `enable_key_revision`, `rtps_psk_protection_kind` and
    /// `allowed_crypto_algorithms` are read into the rule; absent, they are
    /// FALSE, NONE and every algorithm.
    #[test]
    fn the_key_revision_psk_and_allowed_algorithm_elements_are_read() {
        use zerodds_security::algorithm as alg;
        let plain = parse_governance_xml(&document("")).unwrap();
        let rule = &plain.domain_rules[0];
        assert!(!rule.enable_key_revision);
        assert_eq!(rule.rtps_psk_protection_kind, ProtectionKind::None);
        assert_eq!(rule.allowed_crypto_algorithms, None);

        let given = parse_governance_xml(&document(
            r#"<enable_key_revision>TRUE</enable_key_revision>
               <rtps_psk_protection_kind>SIGN</rtps_psk_protection_kind>
               <allowed_crypto_algorithms>
                 <digital_signature>
                   <algorithm>ECDSA+P256+SHA256</algorithm>
                   <algorithm>RSASSA-PSS-MGF1SHA256+2048+SHA256</algorithm>
                 </digital_signature>
                 <digital_signature_identity_trust_chain>
                   <algorithm>ECDSA+P384+SHA384</algorithm>
                 </digital_signature_identity_trust_chain>
                 <key_establishment><algorithm>ECDHE-CEUM+P256</algorithm></key_establishment>
                 <symmetric_cipher><algorithm>AES128+GCM</algorithm></symmetric_cipher>
               </allowed_crypto_algorithms>"#,
        ))
        .unwrap();
        let rule = &given.domain_rules[0];
        assert!(rule.enable_key_revision);
        assert_eq!(rule.rtps_psk_protection_kind, ProtectionKind::Sign);
        let allowed = rule.allowed_crypto_algorithms.expect("the section");
        assert_eq!(
            allowed.digital_signature,
            Some(alg::CBIT_ECDSA_P256_SHA256 | alg::CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256)
        );
        assert_eq!(
            allowed.digital_signature_trust_chain,
            Some(alg::CBIT_ECDSA_P384_SHA384)
        );
        assert_eq!(allowed.key_establishment, Some(alg::CBIT_ECDHE_CEUM_P256));
        assert_eq!(allowed.symmetric_cipher, Some(alg::CBIT_AES128_GCM));
        // The text of §10.4.1.2.5.9.2 spells the trust chain element so.
        let text_spelling = parse_governance_xml(&document(
            r#"<allowed_crypto_algorithms><digital_signature_trust_chain>
                 <algorithm>ECDSA+P256+SHA256</algorithm>
               </digital_signature_trust_chain></allowed_crypto_algorithms>"#,
        ))
        .unwrap();
        assert_eq!(
            text_spelling.domain_rules[0]
                .allowed_crypto_algorithms
                .unwrap()
                .digital_signature_trust_chain,
            Some(alg::CBIT_ECDSA_P256_SHA256)
        );
    }

    /// The XSD names for each algorithm element the strings of §8; any other
    /// text, an element twice, and an element without an algorithm make the
    /// document invalid.
    #[test]
    fn the_allowed_algorithm_names_are_those_of_the_xsd() {
        let allowed = |inner: &str| {
            parse_governance_xml(&document(&alloc::format!(
                "<allowed_crypto_algorithms>{inner}</allowed_crypto_algorithms>"
            )))
        };
        assert!(
            allowed("<symmetric_cipher><algorithm>AES256+GCM</algorithm></symmetric_cipher>")
                .is_ok()
        );
        // A signature name in the cipher element, the 1.1 name, an empty element.
        assert!(
            allowed(
                "<symmetric_cipher><algorithm>ECDSA+P256+SHA256</algorithm></symmetric_cipher>"
            )
            .is_err()
        );
        assert!(
            allowed("<symmetric_cipher><algorithm>AES-256-GCM</algorithm></symmetric_cipher>")
                .is_err()
        );
        assert!(allowed("<symmetric_cipher></symmetric_cipher>").is_err());
        assert!(
            allowed(
                "<key_establishment><algorithm>ECDHE-CEUM+P256</algorithm></key_establishment>\
                 <key_establishment><algorithm>ECDHE-CEUM+P384</algorithm></key_establishment>"
            )
            .is_err()
        );
        // `rtps_psk_protection_kind` takes the three basic kinds.
        assert!(
            parse_governance_xml(&document(
                "<rtps_psk_protection_kind>SIGN_WITH_ORIGIN_AUTHENTICATION</rtps_psk_protection_kind>"
            ))
            .is_err()
        );
    }

    /// §10.4.1.2.3, §10.4.1.2.4: `<dds>` holds one `<domain_access_rules>`
    /// that holds the `<domain_rule>` elements; a `<domain_rule>` elsewhere
    /// in the tree is no domain rule (the tree was searched for it).
    #[test]
    fn the_structure_of_the_document_is_the_xsds() {
        let rule = r#"<domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule>"#;
        assert!(
            parse_governance_xml(&alloc::format!(
                "<dds><domain_access_rules>{rule}</domain_access_rules></dds>"
            ))
            .is_ok()
        );
        for bad in [
            // The wrong root.
            alloc::format!("<domain_access_rules>{rule}</domain_access_rules>"),
            alloc::format!(
                "<governance><domain_access_rules>{rule}</domain_access_rules></governance>"
            ),
            // The rule outside of <domain_access_rules>, or one level deeper.
            alloc::format!("<dds>{rule}</dds>"),
            alloc::format!(
                "<dds><domain_access_rules><wrapper>{rule}</wrapper></domain_access_rules></dds>"
            ),
            // No rule, no rules section.
            "<dds><domain_access_rules></domain_access_rules></dds>".into(),
            "<dds></dds>".into(),
        ] {
            assert!(parse_governance_xml(&bad).is_err(), "{bad}");
        }
        // An extension element the document marks as ignorable, even with a
        // rule inside it, is ignored with its children: the rule in it does
        // not count, and the document needs one of its own.
        assert!(
            parse_governance_xml(&alloc::format!(
                "<dds><domain_access_rules><wrapper must_interpret=\"false\">{rule}</wrapper></domain_access_rules></dds>"
            ))
            .is_err(),
            "a rule in an ignored element is ignored"
        );
        assert!(
            parse_governance_xml(&alloc::format!(
                "<dds><domain_access_rules><wrapper must_interpret=\"FALSE\"/>{rule}</domain_access_rules></dds>"
            ))
            .is_ok()
        );
    }

    /// The enumerations of the XSD are exact strings: ProtectionKind in
    /// capitals, `data_protection_kind` a BasicProtectionKind.
    #[test]
    fn the_protection_kind_enumerations_of_the_xsd_are_exact() {
        let topic = |inner: &str| {
            parse_governance_xml(&alloc::format!(
                r#"<dds><domain_access_rules><domain_rule>
                     <domains><id>0</id></domains>
                     <topic_access_rules><topic_rule><topic_expression>*</topic_expression>{inner}</topic_rule></topic_access_rules>
                   </domain_rule></domain_access_rules></dds>"#
            ))
        };
        assert!(topic("<data_protection_kind>ENCRYPT</data_protection_kind>").is_ok());
        assert!(
            topic("<metadata_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</metadata_protection_kind>")
                .is_ok()
        );
        // data_protection_kind is a BasicProtectionKind: the origin
        // authentication kinds are not.
        assert!(
            topic("<data_protection_kind>SIGN_WITH_ORIGIN_AUTHENTICATION</data_protection_kind>")
                .is_err()
        );
        assert!(
            topic(
                "<data_protection_kind>ENCRYPT_WITH_ORIGIN_AUTHENTICATION</data_protection_kind>"
            )
            .is_err()
        );
        // Lower case is not an enumeration value.
        assert!(topic("<data_protection_kind>encrypt</data_protection_kind>").is_err());
        assert!(topic("<metadata_protection_kind>Sign</metadata_protection_kind>").is_err());
        assert!(
            parse_governance_xml(&document(
                "<discovery_protection_kind>none</discovery_protection_kind>"
            ))
            .is_err()
        );
    }

    /// §10.4.1.3: an attribute the XSD does not define on an element it does
    /// is an extension attribute: it makes the document invalid, unless the
    /// element has `must_interpret` "false"; `must_interpret` itself takes
    /// the four spellings.
    #[test]
    fn an_extension_attribute_follows_must_interpret() {
        let with = |attributes: &str| {
            parse_governance_xml(&alloc::format!(
                r#"<dds><domain_access_rules><domain_rule>
                     <domains><id {attributes}>0</id></domains>
                     <topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
                   </domain_rule></domain_access_rules></dds>"#
            ))
        };
        assert!(with("").is_ok());
        assert!(
            with(r#"vendor="x""#).is_err(),
            "must_interpret is TRUE by default"
        );
        assert!(with(r#"vendor="x" must_interpret="true""#).is_err());
        assert!(with(r#"vendor="x" must_interpret="TRUE""#).is_err());
        assert!(with(r#"vendor="x" must_interpret="false""#).is_ok());
        assert!(with(r#"vendor="x" must_interpret="FALSE""#).is_ok());
        assert!(with(r#"must_interpret="true""#).is_ok());
        assert!(
            with(r#"must_interpret="yes""#).is_err(),
            "not one of the four values"
        );
        // The root may name the schema.
        assert!(
            parse_governance_xml(&alloc::format!(
                r#"<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="domain_governance.xsd">{}</dds>"#,
                "<domain_access_rules><domain_rule><domains><id>0</id></domains><topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules></domain_rule></domain_access_rules>"
            ))
            .is_ok()
        );
    }

    // =======================================================================
    // RC1 stage 8 — peer classes + interface bindings (zerodds-ns)
    // =======================================================================

    const HETERO_GOV: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
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

    /// The OMG sample files declare the namespace of their XSD on `<dds>`.
    #[test]
    fn a_governance_in_the_omg_namespace_parses() {
        let xml = r#"<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_governance"><domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules></dds>"#;
        assert!(parse_governance_xml(xml).is_ok());
        let other = xml.replace("http://www.omg.org/spec/DDS-SECURITY", "http://example.org");
        assert!(parse_governance_xml(&other).is_err());
    }

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

    /// The ZeroDDS elements without the ZeroDDS namespace are not in the
    /// governance XSD: §10.4.1.3 makes such a document invalid unless the
    /// element carries `must_interpret="false"`, and then it is ignored.
    /// Cyclone 11 refuses any unknown element (`access_control_parser.c`,
    /// "Unknown XML element"). The document was accepted and the element
    /// ignored.
    #[test]
    fn zerodds_elements_without_the_namespace_follow_must_interpret() {
        let doc = |attr: &str| {
            alloc::format!(
                r#"<?xml version="1.0"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <peer_classes{attr}>
        <peer_class name="should-be-ignored" protection="NONE" />
      </peer_classes>
    </domain_rule>
  </domain_access_rules>
</dds>"#
            )
        };
        assert!(parse_governance_xml(&doc("")).is_err());
        let g = parse_governance_xml(&doc(r#" must_interpret="false""#)).unwrap();
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
      <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
  <edge_identities must_interpret="false">
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
  <delegation_profiles must_interpret="false">
    <profile name="ignored" />
  </delegation_profiles>
</dds>"#;
        let g = parse_governance_xml(XML).unwrap();
        assert!(g.delegation_profiles.is_empty());
    }

    #[test]
    fn duplicate_profile_name_is_error() {
        // The map is keyed by the bare `name`. Without the collision check the
        // second <profile> silently replaces the trust policy of the first,
        // and the document parses as if only the last one had been written.
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
  <zerodds:delegation_profiles>
    <zerodds:profile name="dup" />
    <zerodds:profile name="dup" />
  </zerodds:delegation_profiles>
</dds>"#;
        let err = parse_governance_xml(XML).expect_err("must fail");
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    #[test]
    fn profile_without_name_is_error() {
        const XML: &str = r#"<?xml version="1.0"?>
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
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
  <domain_access_rules><domain_rule><domains><id>0</id></domains></domain_rule></domain_access_rules>
  <zerodds:edge_identities>
    <zerodds:edge guid_prefix="010203040506070809101112" />
  </zerodds:edge_identities>
</dds>"#;
        let err = parse_governance_xml(XML).expect_err("must fail");
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    // ========================================================================
    // DDS-Security 1.2 §9.4.2.9.4 / §9.4.2.9.5 — endpoint create
    // ========================================================================

    /// Shaped like the OMG dds-security suite's `TESTONLY_governance.xml`:
    /// the FIRST topic rule is the unprotected `OD_OA_OM_OD` one and the
    /// access-controlled rules stand behind it. A decision taken from
    /// `topic_rules.first()` instead of the matching rule reads NONE for
    /// every topic in this document.
    const AC_GOVERNANCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>TRUE</enable_join_access_control>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>OD_OA_OM_OD</topic_expression>
          <enable_read_access_control>FALSE</enable_read_access_control>
          <enable_write_access_control>FALSE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_RA_OM_OD</topic_expression>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>FALSE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_WA_OM_OD</topic_expression>
          <enable_read_access_control>FALSE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_RWA_OM_OD</topic_expression>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>PD_RWA_EM_ED</topic_expression>
          <enable_read_access_control>TRUE</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>"#;

    /// The suite's `..._permissions_write_signed.p7s`: publish-only on the
    /// write-capable profiles, `<default>DENY</default>`.
    const PERMS_WRITE: &str = r#"<dds><permissions>
  <grant name="w">
    <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule>
      <domains><id>0</id></domains>
      <publish><topics>
        <topic>PD_WA_OM_OD</topic><topic>PD_RWA_OM_OD</topic><topic>PD_RWA_EM_ED</topic>
      </topics></publish>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>"#;

    /// The suite's `..._permissions_read_signed.p7s`: subscribe-only.
    const PERMS_READ: &str = r#"<dds><permissions>
  <grant name="r">
    <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule>
      <domains><id>0</id></domains>
      <subscribe><topics>
        <topic>PD_RA_OM_OD</topic><topic>PD_RWA_OM_OD</topic><topic>PD_RWA_EM_ED</topic>
      </topics></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>"#;

    /// The suite's `..._permissions_join_signed.p7s`: the domain and nothing
    /// else.
    const PERMS_JOIN: &str = r#"<dds><permissions>
  <grant name="j">
    <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id>0</id></domains></allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>"#;

    fn ac_fixture(perms: &str) -> (Governance, crate::xml::Permissions) {
        (
            parse_governance_xml(AC_GOVERNANCE).expect("governance"),
            crate::xml::parse_permissions_xml(perms).expect("permissions"),
        )
    }

    /// The six refusals the OMG suite asks for, one assertion each. Every
    /// one of them is a topic whose rule is NOT the first in the document.
    #[test]
    fn the_six_omg_suite_refusals_are_denied() {
        let (gov, write) = ac_fixture(PERMS_WRITE);
        // sub PD_RA_OM_OD with write-only permissions.
        assert!(!gov.check_create_datareader(&write, 0, "PD_RA_OM_OD", &[], 0));
        // sub PD_RWA_OM_OD with write-only permissions.
        assert!(!gov.check_create_datareader(&write, 0, "PD_RWA_OM_OD", &[], 0));

        let (gov, read) = ac_fixture(PERMS_READ);
        // pub PD_WA_OM_OD with read-only permissions.
        assert!(!gov.check_create_datawriter(&read, 0, "PD_WA_OM_OD", &[], 0));
        // pub PD_RWA_OM_OD with read-only permissions.
        assert!(!gov.check_create_datawriter(&read, 0, "PD_RWA_OM_OD", &[], 0));

        let (gov, join) = ac_fixture(PERMS_JOIN);
        // sub / pub PD_RWA_EM_ED with join-only permissions.
        assert!(!gov.check_create_datareader(&join, 0, "PD_RWA_EM_ED", &[], 0));
        assert!(!gov.check_create_datawriter(&join, 0, "PD_RWA_EM_ED", &[], 0));
    }

    /// §10.4.1.2.7: "If no Topic Rule matches the Entity topic name the
    /// operation under consideration shall fail". A topic outside the
    /// suite's five rules is refused, also with write permissions. It was
    /// taken as unprotected and permitted.
    #[test]
    fn a_topic_without_a_rule_is_refused() {
        let (gov, write) = ac_fixture(PERMS_WRITE);
        assert!(!gov.has_topic_rule(0, "NotInTheGovernance"));
        assert!(!gov.check_create_datawriter(&write, 0, "NotInTheGovernance", &[], 0));
        assert!(!gov.check_create_datareader(&write, 0, "NotInTheGovernance", &[], 0));
        assert!(gov.has_topic_rule(0, TOPIC_LOG_TOPIC));
    }

    /// The permitted direction of the same profiles still passes — a check
    /// that denies everything is not access control.
    #[test]
    fn the_granted_direction_still_creates() {
        let (gov, write) = ac_fixture(PERMS_WRITE);
        assert!(gov.check_create_datawriter(&write, 0, "PD_WA_OM_OD", &[], 0));
        assert!(gov.check_create_datawriter(&write, 0, "PD_RWA_OM_OD", &[], 0));
        assert!(gov.check_create_datawriter(&write, 0, "PD_RWA_EM_ED", &[], 0));

        let (gov, read) = ac_fixture(PERMS_READ);
        assert!(gov.check_create_datareader(&read, 0, "PD_RA_OM_OD", &[], 0));
        assert!(gov.check_create_datareader(&read, 0, "PD_RWA_OM_OD", &[], 0));
        assert!(gov.check_create_datareader(&read, 0, "PD_RWA_EM_ED", &[], 0));
    }

    /// A topic whose rule switches the operation off is not gated at all —
    /// join-only permissions create both endpoints on `OD_OA_OM_OD`, and the
    /// half-controlled profiles gate only their own half.
    #[test]
    fn an_uncontrolled_topic_needs_no_grant() {
        let (gov, join) = ac_fixture(PERMS_JOIN);
        assert!(gov.check_create_datawriter(&join, 0, "OD_OA_OM_OD", &[], 0));
        assert!(gov.check_create_datareader(&join, 0, "OD_OA_OM_OD", &[], 0));
        // PD_RA_OM_OD is read-controlled only: writing it needs no grant.
        assert!(gov.check_create_datawriter(&join, 0, "PD_RA_OM_OD", &[], 0));
        assert!(!gov.check_create_datareader(&join, 0, "PD_RA_OM_OD", &[], 0));
        // PD_WA_OM_OD is write-controlled only.
        assert!(gov.check_create_datareader(&join, 0, "PD_WA_OM_OD", &[], 0));
        assert!(!gov.check_create_datawriter(&join, 0, "PD_WA_OM_OD", &[], 0));
    }

    /// The flags come from the rule that matches the topic, not from
    /// `topic_rules.first()`. Under the first-rule reading every topic of
    /// `AC_GOVERNANCE` would be uncontrolled, and every assertion above
    /// would pass for the wrong reason.
    #[test]
    fn access_control_flags_come_from_the_matching_rule() {
        let gov = parse_governance_xml(AC_GOVERNANCE).expect("governance");
        let first = gov
            .find_domain_rule(0)
            .and_then(|d| d.topic_rules.first())
            .expect("first rule");
        assert_eq!(first.topic_expression, "OD_OA_OM_OD");
        assert!(!first.enable_read_access_control);
        assert!(!first.enable_write_access_control);

        assert!(gov.read_access_controlled(0, "PD_RWA_EM_ED"));
        assert!(gov.write_access_controlled(0, "PD_RWA_EM_ED"));
        assert!(!gov.read_access_controlled(0, "OD_OA_OM_OD"));
    }

    /// An expired grant is no grant (§9.4.1.3.2.2), and a grant for another
    /// domain does not carry into this one.
    #[test]
    fn validity_and_domain_bound_the_grant() {
        let gov = parse_governance_xml(AC_GOVERNANCE).expect("governance");
        let expired = crate::xml::parse_permissions_xml(
            r#"<dds><permissions><grant name="e">
                 <subject_name>/C=DE/CN=A</subject_name>
                 <validity>
                   <not_before>2020-01-01T00:00:00</not_before>
                   <not_after>2021-01-01T00:00:00</not_after>
                 </validity>
                 <allow_rule><domains><id>0</id></domains>
                   <publish><topics><topic>*</topic></topics></publish>
                 </allow_rule>
                 <default>DENY</default>
               </grant></permissions></dds>"#,
        )
        .expect("permissions");
        // 2020-07-01 is inside the window, 2026-01-01 is past it.
        assert!(gov.check_create_datawriter(&expired, 0, "PD_WA_OM_OD", &[], 1_593_561_600));
        assert!(!gov.check_create_datawriter(&expired, 0, "PD_WA_OM_OD", &[], 1_767_225_600));

        let other_domain = crate::xml::parse_permissions_xml(
            r#"<dds><permissions><grant name="d">
                 <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                 <allow_rule><domains><id>7</id></domains>
                   <publish><topics><topic>*</topic></topics></publish>
                 </allow_rule>
                 <default>DENY</default>
               </grant></permissions></dds>"#,
        )
        .expect("permissions");
        assert!(!gov.check_create_datawriter(&other_domain, 0, "PD_WA_OM_OD", &[], 0));
    }

    /// A domain the governance does not cover has no endpoints, mirroring
    /// `check_create_participant`.
    #[test]
    fn an_uncovered_domain_creates_nothing() {
        let (gov, _) = ac_fixture(PERMS_WRITE);
        let allow_all = crate::xml::parse_permissions_xml(
            r#"<dds><permissions><grant name="a">
                 <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                 <allow_rule><domains><id>9</id></domains>
                   <publish><topics><topic>*</topic></topics></publish>
                   <subscribe><topics><topic>*</topic></topics></subscribe>
                 </allow_rule>
               </grant></permissions></dds>"#,
        )
        .expect("permissions");
        assert!(!gov.check_create_datawriter(&allow_all, 9, "PD_WA_OM_OD", &[], 0));
        assert!(!gov.check_create_datareader(&allow_all, 9, "PD_RA_OM_OD", &[], 0));
    }

    /// §9.4.1.3.2.3.1: a grant without `<partitions>` covers only the
    /// default partition, so a writer in a named partition is refused.
    #[test]
    fn partitions_scope_the_grant() {
        let (gov, write) = ac_fixture(PERMS_WRITE);
        assert!(gov.check_create_datawriter(&write, 0, "PD_WA_OM_OD", &[], 0));
        assert!(gov.check_create_datawriter(&write, 0, "PD_WA_OM_OD", &[""], 0));
        assert!(!gov.check_create_datawriter(&write, 0, "PD_WA_OM_OD", &["secret"], 0));
    }

    /// DDS-Security 1.2 §10.6: the two BuiltinLoggingTopic names carry a
    /// builtin rule — any participant that may join may write, reading needs a
    /// grant — that wins over the governance, including a catch-all that would
    /// otherwise make the topic write-controlled.
    #[test]
    fn the_builtin_log_topic_rule_allows_any_write_and_gates_reads() {
        let catch_all = parse_governance_xml(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants>
                 <enable_join_access_control>FALSE</enable_join_access_control>
                 <topic_access_rules><topic_rule>
                   <topic_expression>*</topic_expression>
                   <enable_read_access_control>FALSE</enable_read_access_control>
                   <enable_write_access_control>TRUE</enable_write_access_control>
                   <metadata_protection_kind>NONE</metadata_protection_kind>
                   <data_protection_kind>NONE</data_protection_kind>
                 </topic_rule></topic_access_rules>
               </domain_rule></domain_access_rules></dds>"#,
        )
        .expect("governance");

        for topic in log_topics::LOG_TOPICS {
            let rule = builtin_topic_rule(topic).expect("builtin rule");
            assert!(rule.enable_read_access_control);
            assert!(!rule.enable_write_access_control);
            assert_eq!(rule.metadata_protection_kind, ProtectionKind::Sign);
            assert_eq!(rule.data_protection_kind, ProtectionKind::Encrypt);

            // The catch-all says write-controlled; the builtin rule wins.
            assert!(!catch_all.write_access_controlled(0, topic), "{topic}");
            assert!(catch_all.read_access_controlled(0, topic), "{topic}");
        }
        assert!(builtin_topic_rule("Chatter").is_none());

        // Join-only permissions may write the topic (not gated) but not read
        // it (no subscribe grant).
        let (gov, join) = ac_fixture(PERMS_JOIN);
        assert!(gov.check_create_datawriter(&join, 0, TOPIC_LOG_TOPIC_V2, &[], 0));
        assert!(!gov.check_create_datareader(&join, 0, TOPIC_LOG_TOPIC_V2, &[], 0));

        // A grant that names the topic turns reading on.
        let read_log = crate::xml::parse_permissions_xml(
            r#"<dds><permissions><grant name="rl">
                 <subject_name>/C=DE/CN=A</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
                 <allow_rule><domains><id>0</id></domains>
                   <subscribe><topics><topic>DDS:Security:LogTopicV2</topic></topics></subscribe>
                 </allow_rule>
                 <default>DENY</default>
               </grant></permissions></dds>"#,
        )
        .expect("permissions");
        assert!(gov.check_create_datareader(&read_log, 0, TOPIC_LOG_TOPIC_V2, &[], 0));
        assert!(gov.check_create_datawriter(&read_log, 0, TOPIC_LOG_TOPIC_V2, &[], 0));
    }

    /// The two names the §10.6 builtin rule covers, spelled out so that
    /// renaming a constant cannot silently stop covering one.
    mod log_topics {
        pub(super) const LOG_TOPICS: [&str; 2] =
            ["DDS:Security:LogTopic", "DDS:Security:LogTopicV2"];
    }

    /// Table 70 (`get_participant_security_config`): only an element set to
    /// FALSE makes `allow_unauthenticated_participants` and
    /// `is_access_protected` FALSE; a missing one is TRUE. It was FALSE.
    #[test]
    fn missing_domain_rule_flags_are_true() {
        let g = parse_governance_xml(
            r#"<dds><domain_access_rules><domain_rule>
                 <domains><id>0</id></domains>
               </domain_rule></domain_access_rules></dds>"#,
        )
        .unwrap();
        let r = g.find_domain_rule(0).unwrap();
        assert!(r.allow_unauthenticated_participants);
        assert!(r.enable_join_access_control);
    }

    /// §9.4.2.4 Table 33: with `rtps_protection_kind` other than NONE,
    /// `allow_unauthenticated_participants` must be FALSE. The combination
    /// was taken.
    #[test]
    fn rtps_protection_with_unauthenticated_participants_is_refused() {
        let doc = |flag: &str| {
            alloc::format!(
                r#"<dds><domain_access_rules><domain_rule>
                     <domains><id>0</id></domains><enable_join_access_control>false</enable_join_access_control>{flag}
                     <rtps_protection_kind>SIGN</rtps_protection_kind>
                   </domain_rule></domain_access_rules></dds>"#
            )
        };
        assert!(parse_governance_xml(&doc("")).is_err(), "missing is TRUE");
        assert!(
            parse_governance_xml(&doc(
                "<allow_unauthenticated_participants>TRUE</allow_unauthenticated_participants>"
            ))
            .is_err()
        );
        assert!(
            parse_governance_xml(&doc(
                "<allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>"
            ))
            .is_ok()
        );
    }

    /// §10.4.1.2.5.1: a domain rule without `<tag>` holds for the empty
    /// DomainTag, one with tags for the tags it names. The tags were read
    /// and not applied.
    #[test]
    fn domain_rules_match_the_domain_tag() {
        let g = parse_governance_xml(
            r#"<dds><domain_access_rules>
                 <domain_rule><domains><id>0</id><tag>Robot15</tag></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
                   <rtps_protection_kind>NONE</rtps_protection_kind></domain_rule>
                 <domain_rule><domains><id>0</id><tag_expression>AGVS/*</tag_expression></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
                   <rtps_protection_kind>NONE</rtps_protection_kind></domain_rule>
                 <domain_rule><domains><id>0</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
                   <rtps_protection_kind>NONE</rtps_protection_kind></domain_rule>
               </domain_access_rules></dds>"#,
        )
        .unwrap();
        let rule_of = |tag: &str| {
            let r = g.find_domain_rule(Domain { id: 0, tag }).unwrap();
            g.domain_rules.iter().position(|x| x == r).unwrap()
        };
        assert_eq!(rule_of("Robot15"), 0);
        assert_eq!(rule_of("AGVS/7"), 1);
        assert_eq!(rule_of(""), 2);
        assert!(
            g.find_domain_rule(Domain {
                id: 0,
                tag: "other"
            })
            .is_none()
        );
    }
}

#[cfg(test)]
mod nesting_tests {
    use super::*;

    /// `roxmltree` recurses once per open element; a governance document from outside
    /// with a long run of unclosed start tags is refused before it reaches
    /// the parser, instead of overflowing the stack.
    #[test]
    fn a_deeply_nested_text_is_refused_before_the_parser() {
        let text = "<a>".repeat(100_000);
        assert!(matches!(
            parse_governance_xml(&text),
            Err(PermissionsError::InvalidXml(_))
        ));
    }
}
