// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Governance-XML-Parser (OMG DDS-Security 1.1 §9.4.1.2).
//!
//! Governance-XML legt pro Domain fest, welche **Topic-Klassen** wie
//! geschuetzt werden muessen (Discovery-Protection, Read-/Write-Access,
//! Metadata-/Data-Protection-Kind). Die Datei wird typischerweise
//! mit der Permissions-CA signiert und vom Access-Control-Plugin
//! beim Participant-Start geladen.
//!
//! # Scope
//!
//! * Parser fuer `<domain_access_rules>` → `<domain_rule>` →
//!   `<topic_access_rules>` → `<topic_rule>`.
//! * Domain-Filter via `<domains><id>N</id></domains>` (einfaches
//!   `id` oder `<id_range>min..max</id_range>`).
//! * Topic-Expression-Matching mit Wildcards via [`crate::topic_match`].
//! * Protection-Kinds fuer `metadata_protection_kind` und
//!   `data_protection_kind`.
//!
//! # Nicht-Ziele
//!
//! * XML-Signatur-Verifikation — **future-major**.
//! * `<allow_unauthenticated_participants>`-Enforcement — **future-major**.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::delegation_check::{DelegationProfile, TrustAnchor, TrustPolicy};
use zerodds_security_pki::SignatureAlgorithm;

use crate::topic_match::topic_match;
use crate::xml::PermissionsError;

/// Topic-Protection-Kind (Spec §9.4.1.2 Tabelle 48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtectionKind {
    /// Keine Schutzmassnahme.
    #[default]
    None,
    /// Nur Integrity (HMAC / Signature).
    Sign,
    /// Integrity + Confidentiality (AEAD).
    Encrypt,
    /// Wie `Sign`, aber pro Remote-Reader eigene MAC.
    SignWithOriginAuthentication,
    /// Wie `Encrypt`, aber pro Remote-Reader eigene MAC.
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
            _ => Self::None, // unbekannte → NONE (Fail-Open nur fuer
                             // Development; Produktion validiert via
                             // XML-Schema).
        }
    }
}

/// Regel fuer eine Topic-Klasse (oder Wildcard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRule {
    /// Topic-Pattern (Wildcards `*` `?` wie in Permissions).
    pub topic_expression: String,
    /// Discovery-Schutz — SEDP wird verschluesselt.
    pub enable_discovery_protection: bool,
    /// Liveliness-Schutz — `PARTICIPANT_MESSAGE` signiert.
    pub enable_liveliness_protection: bool,
    /// Read-Access per Permissions pruefen.
    pub enable_read_access_control: bool,
    /// Write-Access per Permissions pruefen.
    pub enable_write_access_control: bool,
    /// SEC_PREFIX-Schutz fuer Submessage-Metadaten.
    pub metadata_protection_kind: ProtectionKind,
    /// SEC_BODY-Schutz fuer Payload-Daten.
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

/// Domain-Filter: Liste von (min, max)-Ranges. Eine einzelne Id wird
/// als `min == max` gespeichert.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainFilter {
    /// Inklusive Ranges. Wenn leer: matcht alle Domains (Spec-Default).
    pub ranges: Vec<(u32, u32)>,
}

impl DomainFilter {
    /// `true` wenn `domain_id` in einem Range liegt oder die Filter-
    /// Liste leer ist.
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

/// Eine Domain-Regel im Governance-XML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainRule {
    /// Filter fuer Domain-IDs.
    pub domains: DomainFilter,
    /// Erlaubt unauthenticated Participants im Discovery. Default false.
    pub allow_unauthenticated_participants: bool,
    /// Pflicht-Access-Control auf Participant-Join.
    pub enable_join_access_control: bool,
    /// Discovery-Schutz auf Participant-Level.
    pub discovery_protection_kind: ProtectionKind,
    /// Liveliness-Schutz auf Participant-Level.
    pub liveliness_protection_kind: ProtectionKind,
    /// Signatur-Schutz fuer den RTPS-Header.
    pub rtps_protection_kind: ProtectionKind,
    /// Pro Topic-Klasse eine Regel.
    pub topic_rules: Vec<TopicRule>,
    /// ZeroDDS-Extension: Peer-Klassen fuer Heterogeneous-
    /// Security. Leer bei reinen OMG-Governance-Dokumenten — das ist
    /// der Legacy-Pfad. Namespace-scoped im XML:
    /// `<zerodds:peer_classes>`.
    pub peer_classes: Vec<PeerClass>,
    /// ZeroDDS-Extension: pro Interface-Name eine Regel,
    /// die Protection-Overrides und Peer-Class-Filter ausdrueckt.
    /// Namespace-scoped: `<zerodds:interface_bindings>`.
    pub interface_bindings: Vec<InterfaceBindingRule>,
}

/// Peer-Klasse aus `<zerodds:peer_class>` (RC1, Spec: Architektur-
/// Doc §5).
///
/// Jeder Remote-Peer wird anhand seiner [`crate::PeerCapabilities`] +
/// Cert-CN einer Peer-Klasse zugeordnet. Die erste matchende Klasse
/// in [`DomainRule::peer_classes`] gewinnt — Reihenfolge im XML also
/// relevant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeerClass {
    /// Freier Name zum Diagnose-Zweck (z.B. `"legacy"`, `"fast"`,
    /// `"secure"`, `"highassurance"`).
    pub name: String,
    /// Protection-Level, das fuer Peers dieser Klasse durchgesetzt
    /// wird. Default `None`.
    pub protection: ProtectionKind,
    /// Match-Kriterien (wenn alle erfuellt, passt der Peer zu dieser
    /// Klasse).
    pub match_criteria: PeerClassMatch,
}

/// Match-Kriterien einer Peer-Klasse. Alle gesetzten Felder muessen
/// erfuellt sein (UND-Verknuepfung). `None`/Default-Werte werden
/// ignoriert.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeerClassMatch {
    /// Erwartete Auth-Plugin-Class (z.B. `"DDS:Auth:PKI-DH:1.2"`).
    /// Der Leerstring `""` matcht explizit Peers **ohne** Plugin
    /// (Legacy-Klassifikation). `None` = dieses Kriterium wird nicht
    /// geprueft.
    pub auth_plugin_class: Option<String>,
    /// Wildcard-Pattern fuer den Cert-CN (`*` joker). Beispiel:
    /// `"*.ha.example"` matcht `"writer1.ha.example"`.
    pub cert_cn_pattern: Option<String>,
    /// Suite-Anforderung. Der Peer muss diese Suite in seinen
    /// `supported_suites` listen. Beispiel: `"AES_128_GCM"`.
    pub suite: Option<String>,
    /// OCSP-Live-Check-Flag — der Peer muss einen gueltigen Cert-
    /// Status haben (spiegelt `has_valid_cert` in den Peer-Caps).
    pub require_ocsp: bool,
    /// Delegation-Profile-Referenz. Wenn gesetzt, MUSS
    /// der Peer eine [`DelegationChain`](zerodds_security_pki::DelegationChain)
    /// in seinen Capabilities haben, die gegen das Profil
    /// validiert. `None` = direkter Auth-Pfad ohne Delegation.
    pub delegation_profile: Option<String>,
}

/// Interface-spezifische Regel aus `<zerodds:interface_bindings>`
///.
///
/// Bindet logische Interface-Namen an Protection-Overrides und
/// zugelassene Peer-Klassen. Ergaenzt die socket-basierte Binding-
/// Struktur aus Stufe 6, ohne sie zu ersetzen — der Governance-
/// Eintrag ist die Policy-Sicht, das Socket-Binding ist die Transport-
/// Sicht.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InterfaceBindingRule {
    /// Name des Interfaces (muss mit `InterfaceBindingSpec::name` im
    /// dcps-Runtime-Config uebereinstimmen).
    pub name: String,
    /// Ueberschreibt das Domain-Protection-Kind auf diesem Interface.
    /// `None` = kein Override; Domain-Default gilt.
    pub protection_override: Option<ProtectionKind>,
    /// Zugelassene Peer-Klassen auf diesem Interface. Leer = keine
    /// Einschraenkung (alle Klassen erlaubt).
    pub peer_class_filter: Vec<String>,
    /// Minimales Protection-Level auf diesem Interface. Ergebnis ist
    /// `max(peer_class.protection, protection_min)`. `None` = kein
    /// Minimum.
    pub protection_min: Option<ProtectionKind>,
}

/// XML-Namespace-URI fuer ZeroDDS-Extensions in Governance.xml.
pub const ZERODDS_NS: &str = "https://zerodds.org/schema/security/heterogeneous";

/// Edge-Identity-Mode.
///
/// Architektur-Referenz: `09_delegation.md` §5 (Edge-Identities).
/// `Static` = stabile GuidPrefix ueber Restart hinweg, manuell
/// konfiguriert. `Ephemeral` = pseudozufaellige GuidPrefix mit
/// Lifetime-Rotation, fuer Privacy-/Replay-Resistenz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum EdgeIdentityMode {
    /// Stabiler Prefix, kein Auto-Rotate.
    #[default]
    Static,
    /// Auto-Rotate nach `lifetime_seconds` ohne expliziten Trigger.
    Ephemeral,
}

/// Edge-Identity-Konfiguration aus `<zerodds:edge_identities>`.
///
/// Pro Edge ein Eintrag mit Name, Mode, optional fixer GuidPrefix
/// (12 byte; Static-Default), und Lifetime fuer Ephemeral-Rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeIdentityConfig {
    /// Logischer Edge-Name (z.B. `"lidar-A"`, `"turm-imu"`).
    pub name: String,
    /// Static oder Ephemeral.
    pub mode: EdgeIdentityMode,
    /// 12-byte GuidPrefix; Pflicht fuer `Static`, optional fuer
    /// `Ephemeral` (initialer Wert).
    pub guid_prefix: Option<[u8; 12]>,
    /// Lifetime in Sekunden — nur `Ephemeral`. `None` = Default 300s.
    pub lifetime_seconds: Option<u32>,
}

/// Default-Lifetime fuer Ephemeral-Edge-Identities (Sekunden).
pub const DEFAULT_EPHEMERAL_LIFETIME_SECS: u32 = 300;

impl EdgeIdentityConfig {
    /// Effektive Lifetime in Sekunden — mit Default-Fallback.
    #[must_use]
    pub fn effective_lifetime(&self) -> u32 {
        self.lifetime_seconds
            .unwrap_or(DEFAULT_EPHEMERAL_LIFETIME_SECS)
    }

    /// True wenn der Mode `Ephemeral` ist.
    #[must_use]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self.mode, EdgeIdentityMode::Ephemeral)
    }
}

/// Vollstaendige Governance-Config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Governance {
    /// Alle Domain-Regeln. Reihenfolge relevant (erster Match gewinnt).
    pub domain_rules: Vec<DomainRule>,
    /// Edge-Identity-Configs aus `<zerodds:edge_identities>`. Wird vom
    /// GatewayBridge gelesen.
    pub edge_identities: Vec<EdgeIdentityConfig>,
    /// Delegation-Profiles aus `<zerodds:delegation_profiles>`.
    /// Lookup per Profile-Name (Referenz aus
    /// [`PeerClassMatch::delegation_profile`]).
    pub delegation_profiles: BTreeMap<String, DelegationProfile>,
}

impl Governance {
    /// Findet die passende [`DomainRule`] fuer eine Domain-ID. `None`
    /// wenn keine matcht — Caller entscheidet Default-Policy.
    #[must_use]
    pub fn find_domain_rule(&self, domain_id: u32) -> Option<&DomainRule> {
        self.domain_rules
            .iter()
            .find(|r| r.domains.matches(domain_id))
    }

    /// Findet die passende [`TopicRule`] innerhalb einer Domain-Regel.
    /// Erster Match in `topic_rules` gewinnt; bei keiner Regel wird
    /// `TopicRule::default()` (keine Protection) zurueckgegeben.
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
}

/// Parst ein Governance-XML-Dokument.
///
/// Akzeptiert das Spec-Schema aus §9.4.1.2:
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
/// Siehe [`PermissionsError`] (wiederverwendet — Governance und
/// Permissions teilen XML-Parse-Fehler-Klasse).
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

/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in Praxis).
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

/// 16-byte (32-hex-char) GUID-Parser fuer Trust-Anchor.
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

/// Base64-Decoder fuer Trust-Anchor-PubKey-Bytes.
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

/// Sucht rekursiv nach `<zerodds:edge_identities>`-Elementen und parst
/// deren `<edge>`-Kinder.
///
/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in Praxis).
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

/// Parst Hex-encoded 12-byte GuidPrefix mit optionalen Trennzeichen
/// (`:`, `-`, whitespace). Beispiele:
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

/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in Praxis).
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
            // RC1: zerodds-Extensions. Wir matchen uebers Namespace-
            // URI — Elementname ist nur `peer_classes` ohne Praefix
            // (roxmltree resolved das bereits).
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
        // Wir akzeptieren `<match>` sowohl mit als auch ohne Namespace-
        // Praefix — praktischer fuer XML-Autoren, die das Element
        // innerhalb des parent-Namespace schreiben.
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

/// Wildcard-Matcher fuer Cert-CN-Patterns. Einziger Joker ist `*`,
/// matcht beliebig viele Zeichen (inkl. `.`). Leeres Pattern matcht
/// nur leere Strings. Fuer `*.fast.example` gilt:
/// `"w1.fast.example"` → `true`, `"fast.example"` → `false`.
#[must_use]
pub fn cn_pattern_match(pattern: &str, cn: &str) -> bool {
    // Splitten an `*`, dann iterativ im Haystack finden.
    // Keine Regex-Dependency — das Projekt haelt den Safety-Crate-
    // Footprint klein.
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == cn;
    }
    let mut idx = 0usize;
    // Prefix muss am Anfang passen.
    if !parts[0].is_empty() {
        if !cn.starts_with(parts[0]) {
            return false;
        }
        idx = parts[0].len();
    }
    // Mittelstuecke finden.
    for (i, p) in parts.iter().enumerate().skip(1) {
        if p.is_empty() {
            // Zwei `*` hintereinander → leeres Mittelstueck; skip.
            continue;
        }
        let is_last = i == parts.len() - 1;
        if is_last {
            // Letztes Stueck muss am Ende stehen.
            if !cn[idx..].ends_with(p) {
                return false;
            }
            // Und darf nicht ueberlappen mit bereits matchten Bytes.
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
        // 21 liegt ausserhalb aller Ranges (0-0, 10-20), also None.
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
    // RC1 Stufe 8 — Peer-Classes + Interface-Bindings (zerodds-ns)
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
        // Ein Governance-Dokument ohne zerodds:-Namespace soll exakt
        // wie heute arbeiten (Abwaerts-Kompatibilitaet).
        let g = parse_governance_xml(SAMPLE).unwrap();
        for rule in &g.domain_rules {
            assert!(
                rule.peer_classes.is_empty(),
                "OMG-only-Doc darf keine peer_classes triggern"
            );
            assert!(
                rule.interface_bindings.is_empty(),
                "OMG-only-Doc darf keine interface_bindings triggern"
            );
        }
    }

    #[test]
    fn cyclone_style_without_namespace_declaration_ignores_zerodds_elements() {
        // Cyclone-Perspektive: sie parsen Governance-XML und werfen
        // unbekannte Namespaces weg. Wir simulieren das indem ein XML
        // die zerodds-Elemente ohne Namespace-Deklaration benutzt —
        // dann matched unser Namespace-Filter nicht und das Element
        // wird still ignoriert.
        //
        // Das ist die Vendor-Interop-Garantie: Cyclone/FastDDS sehen
        // zerodds:-Tags, ignorieren sie wenn sie den Namespace nicht
        // kennen, und fallen auf rtps_protection_kind zurueck.
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
            "peer_classes ohne zerodds-namespace muss ignoriert werden"
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
        // Ohne zerodds:-Namespace darf nichts geparst werden — ZeroDDS-
        // Extension-Pflicht.
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

    /// Test-Helper — generiert ein PubKey im richtigen Format und encodiert
    /// es als Base64.
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
        // Default = DirectOrDelegated wenn Wert nicht parsebar.
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
