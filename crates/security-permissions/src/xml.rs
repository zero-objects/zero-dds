// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Permissions XML parser (OMG DDS-Security 1.1 §9.4.1.3).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionsError {
    /// XML parsing failed.
    InvalidXml(String),
    /// Structural error (missing mandatory element).
    Malformed(String),
}

impl core::fmt::Display for PermissionsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidXml(m) => write!(f, "invalid XML: {m}"),
            Self::Malformed(m) => write!(f, "malformed permissions: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PermissionsError {}

/// Validity period: `not_before <= now < not_after`. Values are
/// ISO-8601 strings from the XML; the parser converts them to
/// Unix epoch seconds (u64). Spec §9.4.1.3.2.2.
///
/// future-major: enforcement happens at access-check time via
/// [`Grant::is_valid_at`]. The plugin itself checks `now` against every
/// `check_create_*` call — clock-drift tolerance is up to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Validity {
    /// Inclusive lower bound (Unix epoch seconds). `0` = none.
    pub not_before: u64,
    /// Exclusive upper bound (Unix epoch seconds). `u64::MAX` = none.
    pub not_after: u64,
}

impl Default for Validity {
    fn default() -> Self {
        Self::unrestricted()
    }
}

impl Validity {
    /// Unrestricted validity — always valid.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            not_before: 0,
            not_after: u64::MAX,
        }
    }

    /// `true` if `not_before <= now < not_after`.
    #[must_use]
    pub const fn contains(&self, now: u64) -> bool {
        now >= self.not_before && now < self.not_after
    }
}

/// A grant entry: which topics are allowed per subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// Subject name from the certificate (e.g. `"CN=alice"`).
    pub subject_name: String,
    /// Topic patterns (glob) the subject may publish.
    pub allow_publish_topics: Vec<String>,
    /// Topic patterns the subject may subscribe to.
    pub allow_subscribe_topics: Vec<String>,
    /// Topic patterns explicitly forbidden (`<deny_rule>`,
    /// spec §10.4.1.3 Tab.51). Takes precedence over allow_*.
    pub deny_publish_topics: Vec<String>,
    /// Topic patterns explicitly forbidden for subscribe.
    pub deny_subscribe_topics: Vec<String>,
    /// Domain range — spec §10.4.1.3 `<domains>`. Empty list =
    /// all domains allowed.
    pub domains: Vec<DomainRange>,
    /// Partition patterns (`<partitions><partition>P</partition>
    /// </partitions>`).
    pub partitions: Vec<String>,
    /// Data tags (`<data_tags><tag><name>X</name><value>Y</value>
    /// </tag></data_tags>`). Spec §10.4.1.3 Tab.51 + DataTagging.
    pub data_tags: Vec<DataTag>,
    /// `true` = default-deny (everything not allow-listed is
    /// denied). `false` = default-allow (the allow list is an
    /// exception list on a DENY basis — rarely used).
    pub default_deny: bool,
    /// Validity period (spec §9.4.1.3.2.2). Default `unrestricted`.
    pub validity: Validity,
}

/// Domain-id range per spec §10.4.1.3 `<domains>` element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainRange {
    /// Lower domain id (inclusive).
    pub min: u32,
    /// Upper domain id (inclusive).
    pub max: u32,
}

impl DomainRange {
    /// Constructor for a single-id range.
    #[must_use]
    pub const fn single(id: u32) -> Self {
        Self { min: id, max: id }
    }

    /// `true` if `id` is in the range.
    #[must_use]
    pub const fn contains(&self, id: u32) -> bool {
        id >= self.min && id <= self.max
    }
}

/// Data tag (spec §10.4.1.3 + DataTagging QoS). (name, value) tuples
/// listable per subject that are mirrored into the `DataTags` of the QoS
/// policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTag {
    /// Tag-Name.
    pub name: String,
    /// Tag-Value.
    pub value: String,
}

impl Grant {
    /// `true` if the grant is active at time `now` (Unix seconds).
    /// The caller provides `now` — typically from a
    /// `SystemTime::now()` wrapper.
    #[must_use]
    pub const fn is_valid_at(&self, now: u64) -> bool {
        self.validity.contains(now)
    }

    /// Spec §10.4.1.3: is publish access allowed? Order:
    /// deny_rule (precedence) → allow_rule → default.
    #[must_use]
    pub fn is_publish_allowed(&self, topic: &str) -> bool {
        if self
            .deny_publish_topics
            .iter()
            .any(|p| crate::topic_match::topic_match(p, topic))
        {
            return false;
        }
        if self
            .allow_publish_topics
            .iter()
            .any(|p| crate::topic_match::topic_match(p, topic))
        {
            return true;
        }
        !self.default_deny
    }

    /// Is subscribe access allowed? Same order as publish.
    #[must_use]
    pub fn is_subscribe_allowed(&self, topic: &str) -> bool {
        if self
            .deny_subscribe_topics
            .iter()
            .any(|p| crate::topic_match::topic_match(p, topic))
        {
            return false;
        }
        if self
            .allow_subscribe_topics
            .iter()
            .any(|p| crate::topic_match::topic_match(p, topic))
        {
            return true;
        }
        !self.default_deny
    }

    /// Spec §10.4.1.3 `<domains>`: is the domain id in the allowed
    /// range? Empty domain list = all domains allowed.
    #[must_use]
    pub fn matches_domain(&self, domain_id: u32) -> bool {
        if self.domains.is_empty() {
            return true;
        }
        self.domains.iter().any(|r| r.contains(domain_id))
    }
}

/// Complete permissions file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions {
    /// All grants. Order matters (first match wins).
    pub grants: Vec<Grant>,
}

impl Permissions {
    /// Finds the grant for a subject. If no grant matches,
    /// `None`.
    #[must_use]
    pub fn find_grant(&self, subject_name: &str) -> Option<&Grant> {
        self.grants.iter().find(|g| g.subject_name == subject_name)
    }
}

/// Parses a permissions XML document.
///
/// Accepts the simplified schema from spec §9.4.1.3; ignores
/// `<validity>` and `<default>` at the root-grant level (the default is
/// derived from `<default>DENY</default>` / `ALLOW` — unknown
/// text → default-DENY).
///
/// # Errors
/// See [`PermissionsError`].
pub fn parse_permissions_xml(xml: &str) -> Result<Permissions, PermissionsError> {
    let doc =
        roxmltree::Document::parse(xml).map_err(|e| PermissionsError::InvalidXml(e.to_string()))?;
    let root = doc.root_element();

    // The root is typically `<dds>` or directly `<permissions>`. We
    // search recursively for `<grant>` nodes — tolerant of the
    // three known hierarchy variants (Cyclone / FastDDS /
    // Connext).
    let mut grants = Vec::new();
    walk_grants(root, &mut grants)?;
    Ok(Permissions { grants })
}

/// zerodds-lint: recursion-depth = xml-tree-depth (≤ 16 in practice;
/// `roxmltree` protects against >1000-level recursion in the parser
/// itself, we delegate implicitly to that).
fn walk_grants(
    node: roxmltree::Node<'_, '_>,
    out: &mut Vec<Grant>,
) -> Result<(), PermissionsError> {
    if node.tag_name().name() == "grant" {
        out.push(parse_grant(node)?);
        return Ok(());
    }
    for child in node.children().filter(roxmltree::Node::is_element) {
        walk_grants(child, out)?;
    }
    Ok(())
}

fn parse_grant(grant: roxmltree::Node<'_, '_>) -> Result<Grant, PermissionsError> {
    // Subject name: child element `<subject_name>` or attribute `name`
    // (depending on the vendor).
    let subject_name = grant
        .children()
        .find(|c| c.has_tag_name("subject_name"))
        .and_then(|c| c.text().map(str::trim).map(str::to_owned))
        .or_else(|| grant.attribute("name").map(str::to_owned))
        .ok_or_else(|| {
            PermissionsError::Malformed("<grant> without <subject_name> or name=".into())
        })?;

    let mut allow_publish_topics = Vec::new();
    let mut allow_subscribe_topics = Vec::new();
    let mut deny_publish_topics = Vec::new();
    let mut deny_subscribe_topics = Vec::new();
    let mut partitions = Vec::new();

    for rule in grant.children().filter(|c| c.has_tag_name("allow_rule")) {
        for op in rule.children().filter(roxmltree::Node::is_element) {
            match op.tag_name().name() {
                "publish" => {
                    collect_topics(op, &mut allow_publish_topics);
                    collect_partitions(op, &mut partitions);
                }
                "subscribe" => {
                    collect_topics(op, &mut allow_subscribe_topics);
                    collect_partitions(op, &mut partitions);
                }
                _ => {}
            }
        }
    }
    // Spec §10.4.1.3 Tab.51 — `<deny_rule>` takes precedence over allow_rule.
    for rule in grant.children().filter(|c| c.has_tag_name("deny_rule")) {
        for op in rule.children().filter(roxmltree::Node::is_element) {
            match op.tag_name().name() {
                "publish" => collect_topics(op, &mut deny_publish_topics),
                "subscribe" => collect_topics(op, &mut deny_subscribe_topics),
                _ => {}
            }
        }
    }
    // Domain range at grant level.
    let domains = parse_domains(grant);
    // Data tags.
    let data_tags = parse_data_tags(grant);

    // Default handling: `<default>DENY</default>` is spec-conformant
    // (everything not allow-listed is denied).
    let default_deny = grant
        .children()
        .find(|c| c.has_tag_name("default"))
        .and_then(|c| c.text())
        .map(|t| {
            let t = t.trim().to_uppercase();
            t == "DENY" || t == "DISALLOW"
        })
        // If no `<default>` is given, like FastDDS we take
        // DENY as the safe default.
        .unwrap_or(true);

    // Validity period (spec §9.4.1.3.2.2). Missing children → 0 /
    // u64::MAX (unbounded).
    let validity = parse_validity(grant);

    Ok(Grant {
        subject_name,
        allow_publish_topics,
        allow_subscribe_topics,
        deny_publish_topics,
        deny_subscribe_topics,
        domains,
        partitions,
        data_tags,
        default_deny,
        validity,
    })
}

fn parse_domains(grant: roxmltree::Node<'_, '_>) -> Vec<DomainRange> {
    // Spec §9.4.1.3 / DDS-Security XSD: `<domains>` lives under each
    // `<allow_rule>`; some simplified permissions files put it directly under
    // `<grant>`. Collect from BOTH so the grant's domain scope is honoured
    // either way. (Previously this only read a grant-direct `<domains>`, so the
    // spec/SROS2/cross-vendor-bench layout `<grant><allow_rule><domains>` was
    // silently dropped → every grant matched ALL domains, defeating
    // `<domains>`-based access control.)
    let mut out = Vec::new();
    let direct = grant.children().filter(|c| c.has_tag_name("domains"));
    let in_allow = grant
        .children()
        .filter(|c| c.has_tag_name("allow_rule"))
        .flat_map(|r| r.children().filter(|c| c.has_tag_name("domains")));
    for dnode in direct.chain(in_allow) {
        parse_domain_ranges(dnode, &mut out);
    }
    out
}

/// Parses the `<id>` / `<id_range>` children of a single `<domains>` node into
/// `out`.
fn parse_domain_ranges(dnode: roxmltree::Node<'_, '_>, out: &mut Vec<DomainRange>) {
    for child in dnode.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            // Single-Id: `<id>5</id>`.
            "id" => {
                if let Some(id) = child.text().and_then(|t| t.trim().parse::<u32>().ok()) {
                    out.push(DomainRange::single(id));
                }
            }
            // Range: `<id_range><min>5</min><max>10</max></id_range>`.
            "id_range" => {
                let min = child
                    .children()
                    .find(|c| c.has_tag_name("min"))
                    .and_then(|c| c.text())
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                let max = child
                    .children()
                    .find(|c| c.has_tag_name("max"))
                    .and_then(|c| c.text())
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .unwrap_or(u32::MAX);
                out.push(DomainRange { min, max });
            }
            _ => {}
        }
    }
}

fn parse_data_tags(grant: roxmltree::Node<'_, '_>) -> Vec<DataTag> {
    let Some(node) = grant.children().find(|c| c.has_tag_name("data_tags")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for tag in node.children().filter(|c| c.has_tag_name("tag")) {
        let name = tag
            .children()
            .find(|c| c.has_tag_name("name"))
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        let value = tag
            .children()
            .find(|c| c.has_tag_name("value"))
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        if !name.is_empty() {
            out.push(DataTag { name, value });
        }
    }
    out
}

fn collect_partitions(op: roxmltree::Node<'_, '_>, out: &mut Vec<String>) {
    if let Some(part_node) = op.children().find(|c| c.has_tag_name("partitions")) {
        for p in part_node.children().filter(|c| c.has_tag_name("partition")) {
            if let Some(t) = p.text() {
                let trimmed = t.trim().to_string();
                if !out.contains(&trimmed) {
                    out.push(trimmed);
                }
            }
        }
    }
}

fn parse_validity(grant: roxmltree::Node<'_, '_>) -> Validity {
    let Some(vnode) = grant.children().find(|c| c.has_tag_name("validity")) else {
        return Validity::unrestricted();
    };
    let not_before = vnode
        .children()
        .find(|c| c.has_tag_name("not_before"))
        .and_then(|c| c.text())
        .and_then(parse_iso_seconds)
        .unwrap_or(0);
    let not_after = vnode
        .children()
        .find(|c| c.has_tag_name("not_after"))
        .and_then(|c| c.text())
        .and_then(parse_iso_seconds)
        .unwrap_or(u64::MAX);
    Validity {
        not_before,
        not_after,
    }
}

/// Minimal ISO-8601 parser for the most common spec case:
/// `YYYY-MM-DDTHH:MM:SS` with an optional `Z` / `+00:00` suffix. Returns
/// Unix epoch seconds.
///
/// IMPORTANT: This parser does not implement full timezone
/// offsets — everything except `Z` or absent is interpreted as UTC.
/// For full ISO-8601 a `time`-crate adapter comes in future-major+
/// (currently MSRV-blocked).
fn parse_iso_seconds(s: &str) -> Option<u64> {
    let s = s.trim();
    // Format: YYYY-MM-DDTHH:MM:SS(Z|+HH:MM|-HH:MM|)
    // Length at least 19 (without a timezone).
    if s.len() < 19 {
        return None;
    }
    let bytes = s.as_bytes();
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let hour: u32 = s[11..13].parse().ok()?;
    let minute: u32 = s[14..16].parse().ok()?;
    let second: u32 = s[17..19].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    // Days from 1970-01-01 (Howard Hinnant civil_from_days algorithm,
    // without external deps).
    let y = year - i32::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // [0, 399]
    let m = month as i32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe as i32 * 365 + yoe as i32 / 4 - yoe as i32 / 100 + doy;
    let days_from_epoch = era as i64 * 146_097 + doe as i64 - 719_468;
    if days_from_epoch < 0 {
        return None;
    }
    let secs = days_from_epoch as u64 * 86_400
        + u64::from(hour) * 3_600
        + u64::from(minute) * 60
        + u64::from(second);
    Some(secs)
}

fn collect_topics(op: roxmltree::Node<'_, '_>, out: &mut Vec<String>) {
    // Accept both <topics><topic>X</topic></topics> and
    // directly <topic>X</topic> under <publish>/<subscribe>.
    for topic in op.descendants().filter(|c| c.has_tag_name("topic")) {
        if let Some(txt) = topic.text() {
            out.push(txt.trim().to_string());
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <permissions>
    <grant name="alice">
      <subject_name>CN=alice</subject_name>
      <allow_rule>
        <publish>
          <topics>
            <topic>Chatter</topic>
            <topic>sensor_*</topic>
          </topics>
        </publish>
        <subscribe>
          <topic>Echo</topic>
        </subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
    <grant>
      <subject_name>CN=bob</subject_name>
      <allow_rule>
        <publish><topic>Temperature</topic></publish>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
"#;

    #[test]
    fn parses_grants_and_topics() {
        let p = parse_permissions_xml(SAMPLE).expect("parse");
        assert_eq!(p.grants.len(), 2);

        let alice = p.find_grant("CN=alice").expect("alice");
        assert_eq!(
            alice.allow_publish_topics,
            vec!["Chatter".to_string(), "sensor_*".to_string()],
        );
        assert_eq!(alice.allow_subscribe_topics, vec!["Echo".to_string()]);
        assert!(alice.default_deny);

        let bob = p.find_grant("CN=bob").expect("bob");
        assert_eq!(bob.allow_publish_topics, vec!["Temperature".to_string()]);
        assert!(bob.allow_subscribe_topics.is_empty());
    }

    #[test]
    fn rejects_invalid_xml() {
        let err = parse_permissions_xml("<not-closed").unwrap_err();
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    #[test]
    fn missing_subject_name_is_malformed() {
        let xml = r#"<permissions><grant><allow_rule/></grant></permissions>"#;
        let err = parse_permissions_xml(xml).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn default_deny_without_explicit_tag() {
        let xml = r#"
            <permissions>
              <grant name="x"><subject_name>CN=x</subject_name></grant>
            </permissions>
        "#;
        let p = parse_permissions_xml(xml).unwrap();
        assert!(p.grants[0].default_deny);
    }

    // -------------------------------------------------------------
    // future-major — Validity-Periode
    // -------------------------------------------------------------

    #[test]
    fn missing_validity_defaults_to_unrestricted() {
        let p = parse_permissions_xml(SAMPLE).unwrap();
        assert_eq!(p.grants[0].validity, Validity::unrestricted());
        // Always valid — even at now=0 and u64::MAX.
        assert!(p.grants[0].is_valid_at(0));
        assert!(p.grants[0].is_valid_at(u64::MAX - 1));
    }

    #[test]
    fn iso_parser_unix_epoch() {
        assert_eq!(parse_iso_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso_seconds("1970-01-01T00:00:01Z"), Some(1));
        // 2026-04-24T00:00:00Z = 20567 days * 86400 = 1 776 988 800
        assert_eq!(
            parse_iso_seconds("2026-04-24T00:00:00Z"),
            Some(1_776_988_800)
        );
        // 2000-01-01T00:00:00Z = 946 684 800
        assert_eq!(parse_iso_seconds("2000-01-01T00:00:00Z"), Some(946_684_800));
    }

    #[test]
    fn iso_parser_rejects_malformed() {
        assert_eq!(parse_iso_seconds("not-a-date"), None);
        assert_eq!(parse_iso_seconds("2026/04/24"), None);
        assert_eq!(parse_iso_seconds("2026-13-01T00:00:00"), None); // Monat 13
        assert_eq!(parse_iso_seconds("2026-04-24T25:00:00"), None); // Stunde 25
    }

    #[test]
    fn validity_window_enforced() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <validity>
      <not_before>2026-01-01T00:00:00Z</not_before>
      <not_after>2027-01-01T00:00:00Z</not_after>
    </validity>
    <allow_rule><publish><topic>T</topic></publish></allow_rule>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        let not_before = parse_iso_seconds("2026-01-01T00:00:00Z").unwrap();
        let not_after = parse_iso_seconds("2027-01-01T00:00:00Z").unwrap();
        assert!(!g.is_valid_at(not_before - 1), "just before not_before");
        assert!(g.is_valid_at(not_before), "exactly not_before (inclusive)");
        assert!(g.is_valid_at(not_before + 3600), "mitten drin");
        assert!(!g.is_valid_at(not_after), "exactly not_after (exclusive)");
        assert!(!g.is_valid_at(u64::MAX / 2), "well after not_after");
    }

    #[test]
    fn validity_only_not_after_set() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=bob</subject_name>
    <validity><not_after>2030-01-01T00:00:00Z</not_after></validity>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        assert_eq!(p.grants[0].validity.not_before, 0);
        assert!(p.grants[0].validity.not_after < u64::MAX);
        assert!(p.grants[0].is_valid_at(0));
    }

    // ========================================================================
    // Erweiterte Permissions: deny_rule + domains + partitions + data_tags.
    // ========================================================================

    #[test]
    fn deny_rule_overrides_allow_for_publish() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <allow_rule><publish><topics><topic>*</topic></topics></publish></allow_rule>
    <deny_rule><publish><topics><topic>secret/*</topic></topics></publish></deny_rule>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert!(g.is_publish_allowed("public/news"));
        assert!(!g.is_publish_allowed("secret/keys"), "deny_rule must win");
    }

    #[test]
    fn deny_rule_overrides_allow_for_subscribe() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <allow_rule><subscribe><topics><topic>*</topic></topics></subscribe></allow_rule>
    <deny_rule><subscribe><topics><topic>internal/*</topic></topics></subscribe></deny_rule>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert!(g.is_subscribe_allowed("public/x"));
        assert!(!g.is_subscribe_allowed("internal/db"));
    }

    #[test]
    fn domains_single_id_parsed() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <domains><id>5</id><id>7</id></domains>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert_eq!(g.domains.len(), 2);
        assert!(g.matches_domain(5));
        assert!(g.matches_domain(7));
        assert!(!g.matches_domain(6));
    }

    #[test]
    fn domains_id_range_parsed() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <domains><id_range><min>10</min><max>20</max></id_range></domains>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert!(g.matches_domain(10));
        assert!(g.matches_domain(15));
        assert!(g.matches_domain(20));
        assert!(!g.matches_domain(9));
        assert!(!g.matches_domain(21));
    }

    #[test]
    fn empty_domains_means_all_allowed() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert!(g.domains.is_empty());
        assert!(g.matches_domain(0));
        assert!(g.matches_domain(u32::MAX));
    }

    #[test]
    fn partitions_collected_from_publish_rule() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <allow_rule>
      <publish>
        <topics><topic>T</topic></topics>
        <partitions>
          <partition>internal</partition>
          <partition>backup</partition>
        </partitions>
      </publish>
    </allow_rule>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert_eq!(g.partitions.len(), 2);
        assert!(g.partitions.contains(&"internal".to_string()));
        assert!(g.partitions.contains(&"backup".to_string()));
    }

    #[test]
    fn data_tags_parsed() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <data_tags>
      <tag><name>aws.region</name><value>eu-central-1</value></tag>
      <tag><name>clearance</name><value>secret</value></tag>
    </data_tags>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert_eq!(g.data_tags.len(), 2);
        assert_eq!(g.data_tags[0].name, "aws.region");
        assert_eq!(g.data_tags[0].value, "eu-central-1");
        assert_eq!(g.data_tags[1].name, "clearance");
    }

    #[test]
    fn data_tag_with_empty_name_skipped() {
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <data_tags>
      <tag><name></name><value>x</value></tag>
      <tag><name>k</name><value>v</value></tag>
    </data_tags>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        assert_eq!(p.grants[0].data_tags.len(), 1);
    }

    #[test]
    fn deny_only_grant_blocks_specific_topics() {
        // Default-Allow + deny_rule lockdown.
        let xml = r#"
<permissions>
  <grant>
    <subject_name>CN=alice</subject_name>
    <default>ALLOW</default>
    <deny_rule><publish><topics><topic>X</topic></topics></publish></deny_rule>
  </grant>
</permissions>
"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        assert!(g.is_publish_allowed("Y"), "ALLOW default permits Y");
        assert!(!g.is_publish_allowed("X"), "deny_rule wins over default");
    }

    #[test]
    fn domain_range_single_inclusive() {
        let r = DomainRange::single(42);
        assert!(r.contains(42));
        assert!(!r.contains(41));
        assert!(!r.contains(43));
    }
}
