// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Permissions XML parser (OMG DDS-Security 1.2 §10.4.1.5).

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

/// Validity period: `not_before <= now < not_after`, Unix epoch seconds in
/// UTC (§10.4.1.5.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Validity {
    /// Inclusive lower bound (Unix epoch seconds).
    pub not_before: u64,
    /// Exclusive upper bound (Unix epoch seconds).
    pub not_after: u64,
}

impl Validity {
    /// Unrestricted validity — always valid. A document always names its
    /// bounds; this is for grants built in code.
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

/// Whom a grant is for (§10.4.1.5.3: `subject_name` or
/// `subject_name_expression`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantSubject {
    /// `<subject_name>`: an X.509 subject name (§10.4.1.5.3.1).
    Name(String),
    /// `<subject_name_expression>`: the attribute values are `fnmatch()`
    /// expressions (§10.4.1.5.3.2). It was not read.
    Expression(String),
}

/// The three actions of a rule (§10.4.1.5.3.4.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `<publish>`.
    Publish,
    /// `<subscribe>`.
    Subscribe,
    /// `<relay>`. It was not read.
    Relay,
}

/// One `<publish>`, `<subscribe>` or `<relay>` of a rule (XSD `Criteria`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Criteria {
    /// `<topics>`: topic names or `fnmatch()` expressions, at least one.
    pub topics: Vec<String>,
    /// `<partitions>`; `None` when the element is absent, which means the
    /// empty-string partition in an allow rule and `*` in a deny rule.
    pub partitions: Option<Vec<String>>,
    /// `<data_tags>`; `None` when absent: the empty set in an allow rule,
    /// every tag in a deny rule.
    pub data_tags: Option<Vec<DataTag>>,
}

/// An `<allow_rule>` or `<deny_rule>` (§10.4.1.5.3.4.1, §10.4.1.5.3.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// `true` for `<allow_rule>`.
    pub allow: bool,
    /// The domains the rule applies to.
    pub domains: crate::governance::DomainFilter,
    /// The `<publish>` criteria.
    pub publish: Vec<Criteria>,
    /// The `<subscribe>` criteria.
    pub subscribe: Vec<Criteria>,
    /// The `<relay>` criteria.
    pub relay: Vec<Criteria>,
}

impl Rule {
    fn criteria(&self, action: Action) -> &[Criteria] {
        match action {
            Action::Publish => &self.publish,
            Action::Subscribe => &self.subscribe,
            Action::Relay => &self.relay,
        }
    }
}

/// How the partitions of an allow rule match (§10.4.1.5.3.4.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PartitionMatching {
    /// Every partition of the entity is in the set of the rule.
    #[default]
    Subset,
    /// "Legacy matching behavior": one of the partitions of the entity is
    /// in the set. The spec requires a way to select it.
    Legacy,
}

/// An operation the permissions are asked about.
#[derive(Debug, Clone, Copy)]
pub struct AccessRequest<'a> {
    /// The action.
    pub action: Action,
    /// The domain of the participant.
    pub domain: crate::governance::Domain<'a>,
    /// The topic name.
    pub topic: &'a str,
    /// The partitions of the Publisher or Subscriber; none is the
    /// empty-string partition.
    pub partitions: &'a [&'a str],
    /// The data tags of the DataWriter or DataReader.
    pub data_tags: &'a [DataTag],
}

/// A grant (§10.4.1.5.3): subject, validity, rules in document order and
/// the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The `name` attribute.
    pub name: String,
    /// The subject the grant is for.
    pub subject: GrantSubject,
    /// `<validity>`.
    pub validity: Validity,
    /// The rules, in the order of the document.
    pub rules: Vec<Rule>,
    /// `<default>DENY</default>`, also when `<default>` is absent
    /// (§10.4.1.5.3.4: "the absence of the default rule shall be treated as
    /// a <default>DENY</default>").
    pub default_deny: bool,
}

/// Domain-id range of a `<domains>` element.
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

/// A data tag (§10.4.1.5.3.4.1.5): name and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTag {
    /// Tag-Name.
    pub name: String,
    /// Tag-Value.
    pub value: String,
}

/// The outcome of [`Grant::decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// An allow rule matched first.
    Allow,
    /// A deny rule matched first.
    Deny,
    /// No rule matched; the default decided.
    Default {
        /// `true` for `<default>ALLOW</default>`.
        allow: bool,
    },
}

impl Decision {
    /// `true` for an allowed operation.
    #[must_use]
    pub const fn allowed(self) -> bool {
        matches!(self, Self::Allow | Self::Default { allow: true })
    }
}

impl Grant {
    /// `true` if the grant is active at time `now` (Unix seconds).
    #[must_use]
    pub const fn is_valid_at(&self, now: u64) -> bool {
        self.validity.contains(now)
    }

    /// §10.4.1.5.3.4: "The rules are applied in the same order that appear
    /// in the document. If the criteria for the rule matches [...] then the
    /// allow or deny decision is applied. [...] If all rules have been
    /// examined without a match, then the decision specified by the
    /// 'default' rule is applied." The rules were folded into two lists and
    /// any deny beat every allow; the partitions and domains of a deny rule
    /// were not read, and the partitions of all rules held for each.
    #[must_use]
    pub fn decide(&self, req: &AccessRequest<'_>, matching: PartitionMatching) -> Decision {
        for rule in &self.rules {
            if !rule.domains.matches(req.domain) {
                continue;
            }
            let hit = rule
                .criteria(req.action)
                .iter()
                .any(|c| criteria_match(c, rule.allow, req, matching));
            if hit {
                return if rule.allow {
                    Decision::Allow
                } else {
                    Decision::Deny
                };
            }
        }
        Decision::Default {
            allow: !self.default_deny,
        }
    }

    /// Table 70, `check_create_participant` / `check_remote_participant`:
    /// the first rule for the domain decides — an allow rule permits, a
    /// deny rule without publish or subscribe actions refuses; otherwise
    /// the default.
    #[must_use]
    pub fn may_join<'a>(&self, domain: impl Into<crate::governance::Domain<'a>>) -> bool {
        let domain = domain.into();
        if let Some(rule) = self.rules.iter().find(|r| r.domains.matches(domain)) {
            if rule.allow {
                return true;
            }
            if rule.publish.is_empty() && rule.subscribe.is_empty() {
                return false;
            }
        }
        !self.default_deny
    }

    /// `true` when the grant is for the participant whose certificate has
    /// the subject `subject` (attribute pairs as
    /// `zerodds_security_pki::subject_attributes` gives them):
    /// §10.4.1.5.3.1.1 for a name, §10.4.1.5.3.2.1 for an expression.
    #[must_use]
    pub fn is_for(&self, subject: &[(String, String)]) -> bool {
        let wanted = normalized(subject.iter().map(|(n, v)| (n.as_str(), v.as_str())));
        match &self.subject {
            GrantSubject::Name(name) => subject_name_attributes(name).is_some_and(|a| a == wanted),
            GrantSubject::Expression(expr) => {
                subject_name_attributes(expr).is_some_and(|pattern| {
                    pattern.len() == wanted.len()
                        && pattern.iter().zip(&wanted).all(|((pn, pv), (wn, wv))| {
                            pn == wn
                                && crate::topic_match::fnmatch(
                                    pv,
                                    wv,
                                    crate::topic_match::FnmatchFlags::SUBJECT_NAME,
                                )
                        })
                })
            }
        }
    }

    /// [`Self::decide`] for an entity without data tags, partitions matched
    /// as subset.
    #[must_use]
    pub fn allows<'a>(
        &self,
        action: Action,
        domain: impl Into<crate::governance::Domain<'a>>,
        topic: &str,
        partitions: &[&str],
    ) -> bool {
        self.decide(
            &AccessRequest {
                action,
                domain: domain.into(),
                topic,
                partitions,
                data_tags: &[],
            },
            PartitionMatching::Subset,
        )
        .allowed()
    }

    /// Table 70, `check_create_topic` and `check_remote_topic`: the grant
    /// allows publishing, subscribing or relaying the topic (a relay-only
    /// reader of §9.4.2.9.11 is matched, so its topic is one of the grant).
    /// A topic has no partition
    /// and no data tag, so an allow rule counts whatever its partitions and
    /// tags are, and a deny rule only when it denies the topic on every
    /// partition and for every tag (a `<deny_rule>` without `<partitions>` and
    /// `<data_tags>`, §10.4.1.5.3.4.2.4, §10.4.1.5.3.4.2.5). The rules apply
    /// in document order; without a hit the default decides.
    #[must_use]
    pub fn allows_topic<'a>(
        &self,
        domain: impl Into<crate::governance::Domain<'a>>,
        topic: &str,
    ) -> bool {
        let domain = domain.into();
        [Action::Publish, Action::Subscribe, Action::Relay]
            .into_iter()
            .any(|action| self.topic_allowed(action, domain, topic))
    }

    fn topic_allowed(
        &self,
        action: Action,
        domain: crate::governance::Domain<'_>,
        topic: &str,
    ) -> bool {
        use crate::topic_match::topic_match;
        for rule in &self.rules {
            if !rule.domains.matches(domain) {
                continue;
            }
            for c in rule.criteria(action) {
                if !c.topics.iter().any(|t| topic_match(t, topic)) {
                    continue;
                }
                if rule.allow {
                    return true;
                }
                let every_partition = c
                    .partitions
                    .as_ref()
                    .is_none_or(|set| set.iter().any(|p| p == "*"));
                if every_partition && c.data_tags.is_none() {
                    return false;
                }
            }
        }
        !self.default_deny
    }

    /// The text of the subject, as the document has it.
    #[must_use]
    pub fn subject_text(&self) -> &str {
        match &self.subject {
            GrantSubject::Name(s) | GrantSubject::Expression(s) => s,
        }
    }
}

/// One criteria of a rule against the request (§10.4.1.5.3.4.1.3 to .5 for
/// an allow rule, §10.4.1.5.3.4.2.3 to .5 for a deny rule).
fn criteria_match(
    c: &Criteria,
    allow: bool,
    req: &AccessRequest<'_>,
    matching: PartitionMatching,
) -> bool {
    use crate::topic_match::topic_match;
    if !c.topics.iter().any(|t| topic_match(t, req.topic)) {
        return false;
    }
    let asked: &[&str] = if req.partitions.is_empty() {
        &[""]
    } else {
        req.partitions
    };
    let partitions_ok = if allow {
        // The entity's partitions are a subset of the rule's ("" without a
        // <partitions> element); legacy: one of them is in it.
        let in_set = |want: &&str| match &c.partitions {
            None => want.is_empty(),
            Some(set) => set.iter().any(|p| topic_match(p, want)),
        };
        match matching {
            PartitionMatching::Subset => asked.iter().all(in_set),
            PartitionMatching::Legacy => asked.iter().any(in_set),
        }
    } else {
        // One of the entity's partitions matches ("*" without the element).
        match &c.partitions {
            None => true,
            Some(set) => asked
                .iter()
                .any(|want| set.iter().any(|p| topic_match(p, want))),
        }
    };
    if !partitions_ok {
        return false;
    }
    if allow {
        // The entity's tags are a subset of the rule's (the empty set
        // without a <data_tags> element).
        let set = c.data_tags.as_deref().unwrap_or(&[]);
        req.data_tags.iter().all(|t| set.contains(t))
    } else {
        // One of the entity's tags is in the rule's set (every tag without
        // the element).
        match &c.data_tags {
            None => true,
            Some(set) => req.data_tags.iter().any(|t| set.contains(t)),
        }
    }
}

/// Complete permissions file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions {
    /// All grants, in document order.
    pub grants: Vec<Grant>,
}

impl Permissions {
    /// The grant whose `<subject_name>` is the text `subject_name`
    /// exactly.
    #[must_use]
    pub fn find_grant(&self, subject_name: &str) -> Option<&Grant> {
        self.grants
            .iter()
            .find(|g| g.subject_text() == subject_name)
    }

    /// The document reduced to the grant of the identity whose certificate
    /// has the SubjectName `subject` (attribute pairs as
    /// `zerodds_security_pki::subject_attributes` gives them). DDS-Security
    /// 1.2 §10.4.1.5.3.1: "A permissions section with a subject name that
    /// does not match the subject name given in the corresponding Identity
    /// Certificate will be ignored." The first matching grant counts ("Each
    /// subject name can only appear in a single `<permissions>` section"); a
    /// `subject_name_expression` matches as §10.4.1.5.3.2.1 says. `None`
    /// when no grant is for the subject.
    #[must_use]
    pub fn for_subject(&self, subject: &[(String, String)]) -> Option<Permissions> {
        self.grants
            .iter()
            .find(|g| g.is_for(subject))
            .map(|g| Permissions {
                grants: alloc::vec![g.clone()],
            })
    }
}

/// The attributes of a `<subject_name>` as sorted `(NAME, value)` pairs:
/// RFC 2253/4514 form (`CN=alice,O=Example\, Inc.,C=DE`) or the OpenSSL
/// one-line form (`/C=DE/O=Example/CN=alice`), the order not significant —
/// Fast DDS compares the names as names (`rfc2253_string_compare`), Cyclone
/// attribute by attribute (`ac_check_subjects_are_equal`). `None` for a
/// text that is not a name.
#[must_use]
pub fn subject_name_attributes(subject_name: &str) -> Option<Vec<(String, String)>> {
    let text = subject_name.trim();
    let mut parts: Vec<String> = Vec::new();
    if let Some(rest) = text.strip_prefix('/') {
        parts.extend(rest.split('/').map(alloc::string::ToString::to_string));
    } else {
        // Split on unescaped ',', ';' and '+', and drop the escapes.
        let mut current = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                ',' | ';' | '+' => parts.push(core::mem::take(&mut current)),
                _ => current.push(c),
            }
        }
        parts.push(current);
    }
    let mut pairs = Vec::new();
    for part in parts {
        if part.trim().is_empty() {
            continue;
        }
        let (name, value) = part.split_once('=')?;
        pairs.push((name.trim().to_string(), value.trim().to_string()));
    }
    if pairs.is_empty() {
        return None;
    }
    Some(normalized(
        pairs.iter().map(|(n, v)| (n.as_str(), v.as_str())),
    ))
}

/// Sorted pairs with the attribute names upper-cased and their aliases
/// (`E`, `EMAIL` for `EMAILADDRESS`; `S` for `ST`) merged.
fn normalized<'a>(pairs: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = pairs
        .map(|(n, v)| {
            let name = n.trim().to_ascii_uppercase();
            let name = match name.as_str() {
                "E" | "EMAIL" => "EMAILADDRESS".to_string(),
                "S" => "ST".to_string(),
                _ => name,
            };
            (name, v.trim().to_string())
        })
        .collect();
    out.sort();
    out
}

/// Parses a permissions document after the XSD of DDS-Security 1.2
/// §10.4.1.5: `<dds><permissions>` with one or more `<grant>`. An element
/// the XSD does not define is an error unless it carries
/// `must_interpret="false"` (§10.4.1.6, §10.4.1.3).
///
/// # Errors
/// See [`PermissionsError`].
pub fn parse_permissions_xml(xml: &str) -> Result<Permissions, PermissionsError> {
    zerodds_foundation::check_nesting(xml, zerodds_foundation::XML_NESTING_LIMIT)
        .map_err(|e| PermissionsError::InvalidXml(e.to_string()))?;
    let doc =
        roxmltree::Document::parse(xml).map_err(|e| PermissionsError::InvalidXml(e.to_string()))?;
    let root = doc.root_element();
    let permissions = match root.tag_name().name() {
        "dds" => {
            check_known_element(root, "permissions")?;
            let mut found = None;
            for child in root.children().filter(roxmltree::Node::is_element) {
                if child.has_tag_name("permissions") && found.is_none() {
                    check_known_element(child, "permissions")?;
                    found = Some(child);
                } else {
                    extension_element(child)?;
                }
            }
            found.ok_or_else(|| {
                PermissionsError::Malformed("permissions: <dds> without <permissions>".into())
            })?
        }
        other => {
            return Err(PermissionsError::Malformed(alloc::format!(
                "permissions: root element <{other}>, expected <dds>"
            )));
        }
    };
    let mut grants = Vec::new();
    for child in permissions.children().filter(roxmltree::Node::is_element) {
        if child.has_tag_name("grant") {
            grants.push(parse_grant(child)?);
        } else {
            extension_element(child)?;
        }
    }
    if grants.is_empty() {
        return Err(PermissionsError::Malformed(
            "permissions: <permissions> without <grant>".into(),
        ));
    }
    Ok(Permissions { grants })
}

/// §10.4.1.6 with §10.4.1.3: an element outside the XSD is ignored with
/// `must_interpret` "false", otherwise the document is invalid.
fn extension_element(node: roxmltree::Node<'_, '_>) -> Result<(), PermissionsError> {
    extension_element_of(node, "permissions")
}

/// The namespace of `xsi:noNamespaceSchemaLocation`, which a document may
/// carry on its root.
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// DDS-Security 1.2 §10.4.1.3 (governance) and §10.4.1.6 (permissions): the
/// `must_interpret` attribute "can take the values "true", "false", "TRUE" or
/// "FALSE""; any other value makes the document invalid.
pub(crate) fn check_must_interpret_value(
    node: roxmltree::Node<'_, '_>,
    document: &str,
) -> Result<(), PermissionsError> {
    match node.attribute("must_interpret") {
        None | Some("true" | "false" | "TRUE" | "FALSE") => Ok(()),
        Some(other) => Err(PermissionsError::Malformed(alloc::format!(
            "{document}: <{}> has must_interpret={other:?}, which is not true, false, TRUE or FALSE",
            node.tag_name().name()
        ))),
    }
}

/// An element the XSD defines, with attributes: "Any unrecognized XML
/// attribute that appears in an otherwise recognized XML element [...] shall
/// be considered a 'governance extension attribute'" (§10.4.1.3, §10.4.1.6).
/// Without `must_interpret` on its element, or with it "true", such an
/// attribute makes the document invalid; with "false" it is ignored.
pub(crate) fn check_known_element(
    node: roxmltree::Node<'_, '_>,
    document: &str,
) -> Result<(), PermissionsError> {
    check_known_element_with(node, document, &[])
}

/// [`check_known_element`] for an element whose XSD defines the attributes
/// `allowed`.
pub(crate) fn check_known_element_with(
    node: roxmltree::Node<'_, '_>,
    document: &str,
    allowed: &[&str],
) -> Result<(), PermissionsError> {
    check_must_interpret_value(node, document)?;
    let ignorable = matches!(node.attribute("must_interpret"), Some("false" | "FALSE"));
    for attribute in node.attributes() {
        match attribute.namespace() {
            Some(XSI_NS) => continue,
            None if attribute.name() == "must_interpret" || allowed.contains(&attribute.name()) => {
                continue;
            }
            _ => {}
        }
        if !ignorable {
            return Err(PermissionsError::Malformed(alloc::format!(
                "{document}: <{}> has the attribute {:?}, which is not in the {document} XSD, \
                 and no must_interpret=\"false\"",
                node.tag_name().name(),
                attribute.name()
            )));
        }
    }
    Ok(())
}

/// An element the XSD does not define: ignored, with its children, when it
/// carries `must_interpret` "false" or "FALSE", otherwise the document is
/// invalid (§10.4.1.3: an element without the attribute is read as "TRUE").
pub(crate) fn extension_element_of(
    node: roxmltree::Node<'_, '_>,
    document: &str,
) -> Result<(), PermissionsError> {
    check_must_interpret_value(node, document)?;
    match node.attribute("must_interpret") {
        Some("false" | "FALSE") => Ok(()),
        _ => Err(PermissionsError::Malformed(alloc::format!(
            "{document}: <{}> is not in the {document} XSD and has no must_interpret=\"false\"",
            node.tag_name().name()
        ))),
    }
}

/// A `<grant>` (XSD `Grant`): `subject_name` or `subject_name_expression`,
/// `validity`, one or more `allow_rule`/`deny_rule`, and `default`. The
/// grant-level `<domains>` and `<data_tags>`, a `<topic>` without
/// `<topics>` and a grant without `<validity>` were taken; they are outside
/// the XSD, and Fast DDS refuses them too (`PermissionsParser::parse_grant`).
fn parse_grant(grant: roxmltree::Node<'_, '_>) -> Result<Grant, PermissionsError> {
    check_known_element_with(grant, "permissions", &["name"])?;
    let name = grant.attribute("name").unwrap_or("").to_string();
    let mut subject = None;
    let mut validity = None;
    let mut rules = Vec::new();
    let mut default_deny = None;
    for child in grant.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "subject_name" | "subject_name_expression" if subject.is_none() => {
                check_known_element(child, "permissions")?;
                let text = child.text().unwrap_or("").trim().to_string();
                subject = Some(if child.has_tag_name("subject_name") {
                    GrantSubject::Name(text)
                } else {
                    GrantSubject::Expression(text)
                });
            }
            "validity" if validity.is_none() => validity = Some(parse_validity(child)?),
            "allow_rule" | "deny_rule" => rules.push(parse_rule(child)?),
            // `<default>` (§10.4.1.5.3.5, XSD `DefaultAction`): ALLOW or
            // DENY. Any other text makes the document invalid; it was read
            // as ALLOW, a typo in DENY included.
            "default" if default_deny.is_none() => {
                check_known_element(child, "permissions")?;
                default_deny = Some(match child.text().unwrap_or("").trim() {
                    "DENY" => true,
                    "ALLOW" => false,
                    other => {
                        return Err(PermissionsError::Malformed(alloc::format!(
                            "permissions: <default> {other:?} is neither ALLOW nor DENY"
                        )));
                    }
                });
            }
            _ => extension_element(child)?,
        }
    }
    let subject = subject.ok_or_else(|| {
        PermissionsError::Malformed(alloc::format!(
            "permissions: <grant name={name:?}> without <subject_name>"
        ))
    })?;
    let validity = validity.ok_or_else(|| {
        PermissionsError::Malformed(alloc::format!(
            "permissions: <grant name={name:?}> without <validity>"
        ))
    })?;
    if rules.is_empty() {
        return Err(PermissionsError::Malformed(alloc::format!(
            "permissions: <grant name={name:?}> without <allow_rule> or <deny_rule>"
        )));
    }
    Ok(Grant {
        name,
        subject,
        validity,
        rules,
        default_deny: default_deny.unwrap_or(true),
    })
}

/// An `<allow_rule>` or `<deny_rule>` (XSD `Rule`): `<domains>`, then
/// `<publish>`, `<subscribe>` and `<relay>` criteria.
fn parse_rule(rule: roxmltree::Node<'_, '_>) -> Result<Rule, PermissionsError> {
    check_known_element(rule, "permissions")?;
    let mut domains = None;
    let mut out = Rule {
        allow: rule.has_tag_name("allow_rule"),
        domains: crate::governance::DomainFilter::default(),
        publish: Vec::new(),
        subscribe: Vec::new(),
        relay: Vec::new(),
    };
    for child in rule.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "domains" if domains.is_none() => {
                domains = Some(crate::governance::parse_domain_filter(child).map_err(
                    |e| match e {
                        PermissionsError::Malformed(m) => {
                            PermissionsError::Malformed(m.replace("governance:", "permissions:"))
                        }
                        other => other,
                    },
                )?);
            }
            "publish" => out.publish.push(parse_criteria(child)?),
            "subscribe" => out.subscribe.push(parse_criteria(child)?),
            "relay" => out.relay.push(parse_criteria(child)?),
            _ => extension_element(child)?,
        }
    }
    out.domains = domains.ok_or_else(|| {
        PermissionsError::Malformed(alloc::format!(
            "permissions: <{}> without <domains>",
            rule.tag_name().name()
        ))
    })?;
    Ok(out)
}

/// A `<publish>`, `<subscribe>` or `<relay>` (XSD `Criteria`): `<topics>`
/// is required, `<partitions>` and `<data_tags>` are optional.
fn parse_criteria(node: roxmltree::Node<'_, '_>) -> Result<Criteria, PermissionsError> {
    check_known_element(node, "permissions")?;
    let mut out = Criteria::default();
    let mut has_topics = false;
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "topics" if !has_topics => {
                check_known_element(child, "permissions")?;
                has_topics = true;
                out.topics = texts_of(child, "topic")?;
            }
            "partitions" if out.partitions.is_none() => {
                check_known_element(child, "permissions")?;
                out.partitions = Some(texts_of(child, "partition")?);
            }
            "data_tags" if out.data_tags.is_none() => {
                check_known_element(child, "permissions")?;
                let mut tags = Vec::new();
                for tag in child.children().filter(roxmltree::Node::is_element) {
                    if !tag.has_tag_name("tag") {
                        extension_element(tag)?;
                        continue;
                    }
                    check_known_element(tag, "permissions")?;
                    let field = |name: &str| {
                        tag.children()
                            .find(|c| c.has_tag_name(name))
                            .and_then(|c| c.text())
                            .map(|t| t.trim().to_string())
                    };
                    let (Some(name), Some(value)) = (field("name"), field("value")) else {
                        return Err(PermissionsError::Malformed(
                            "permissions: <tag> without <name> and <value>".into(),
                        ));
                    };
                    tags.push(DataTag { name, value });
                }
                out.data_tags = Some(tags);
            }
            _ => extension_element(child)?,
        }
    }
    if !has_topics || out.topics.is_empty() {
        return Err(PermissionsError::Malformed(alloc::format!(
            "permissions: <{}> without <topics><topic>",
            node.tag_name().name()
        )));
    }
    Ok(out)
}

/// The texts of the `<item>` children of `list`.
fn texts_of(list: roxmltree::Node<'_, '_>, item: &str) -> Result<Vec<String>, PermissionsError> {
    let mut out = Vec::new();
    check_known_element(list, "permissions")?;
    for child in list.children().filter(roxmltree::Node::is_element) {
        if child.has_tag_name(item) {
            check_known_element(child, "permissions")?;
            out.push(child.text().unwrap_or("").trim().to_string());
        } else {
            extension_element(child)?;
        }
    }
    Ok(out)
}

fn parse_validity(vnode: roxmltree::Node<'_, '_>) -> Result<Validity, PermissionsError> {
    check_known_element(vnode, "permissions")?;
    // Both bounds are required (XSD `Validity`). An unreadable `not_after`
    // was read as no end, an unreadable `not_before` as no beginning.
    let bound = |name: &str| -> Result<u64, PermissionsError> {
        let element = vnode.children().find(|c| c.has_tag_name(name));
        if let Some(element) = element {
            check_known_element(element, "permissions")?;
        }
        let text = element.and_then(|c| c.text()).unwrap_or("");
        parse_iso_seconds(text).ok_or_else(|| {
            PermissionsError::Malformed(alloc::format!(
                "<validity><{name}> {:?} is not an xs:dateTime",
                text.trim()
            ))
        })
    };
    Ok(Validity {
        not_before: bound("not_before")?,
        not_after: bound("not_after")?,
    })
}

/// An `xs:dateTime` of the permissions XSD (`YYYY-MM-DDThh:mm:ss`, an
/// optional fraction, an optional `Z` or `±hh:mm`) as Unix epoch seconds
/// in UTC. Without a zone the time is taken as UTC. The offset was not
/// applied: a validity in `+02:00` ended two hours late.
fn parse_iso_seconds(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.len() < 19 || !s.is_char_boundary(19) {
        return None;
    }
    let (clock, rest) = s.split_at(19);
    // An optional fraction of a second, then an optional zone.
    let zone = match rest.strip_prefix('.') {
        Some(frac) => {
            let digits = frac.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                return None;
            }
            &frac[digits..]
        }
        None => rest,
    };
    let offset_secs: i64 = match zone {
        "" | "Z" => 0,
        _ => {
            let zb = zone.as_bytes();
            if zb.len() != 6 || !matches!(zb[0], b'+' | b'-') || zb[3] != b':' {
                return None;
            }
            let h: i64 = zone.get(1..3)?.parse().ok()?;
            let m: i64 = zone.get(4..6)?.parse().ok()?;
            if h > 14 || m > 59 {
                return None;
            }
            let v = h * 3_600 + m * 60;
            if zb[0] == b'+' { v } else { -v }
        }
    };
    let s = clock;
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
    let local = days_from_epoch * 86_400
        + i64::from(hour) * 3_600
        + i64::from(minute) * 60
        + i64::from(second);
    u64::try_from(local - offset_secs).ok()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::governance::Domain;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    const VALIDITY: &str = "<validity><not_before>2020-01-01T00:00:00</not_before>\
        <not_after>2040-01-01T00:00:00</not_after></validity>";

    /// A document of one grant for `CN=alice` with `rules` and `default`.
    fn doc(rules: &str, default: &str) -> String {
        alloc::format!(
            r#"<dds><permissions><grant name="g"><subject_name>CN=alice</subject_name>
               {VALIDITY}{rules}{default}</grant></permissions></dds>"#
        )
    }

    fn grant(rules: &str, default: &str) -> Grant {
        parse_permissions_xml(&doc(rules, default))
            .expect("valid permissions")
            .grants
            .remove(0)
    }

    fn req<'a>(
        action: Action,
        topic: &'a str,
        partitions: &'a [&'a str],
        data_tags: &'a [DataTag],
    ) -> AccessRequest<'a> {
        AccessRequest {
            action,
            domain: Domain::from(0),
            topic,
            partitions,
            data_tags,
        }
    }

    fn allowed(g: &Grant, action: Action, topic: &str, partitions: &[&str]) -> bool {
        g.decide(
            &req(action, topic, partitions, &[]),
            PartitionMatching::Subset,
        )
        .allowed()
    }

    /// DDS-Security 1.2 §10.4.1.6 with §10.4.1.3: the permissions document is
    /// extended as the governance is: an unknown element or attribute makes it
    /// invalid unless the element carries `must_interpret` "false" (or
    /// "FALSE"); `must_interpret` takes "true", "false", "TRUE" or "FALSE".
    /// `name` on `<grant>` is the XSD's.
    #[test]
    fn the_permissions_document_is_extended_by_must_interpret() {
        let grant = |grant_attributes: &str, inside: &str, publish_attributes: &str| {
            parse_permissions_xml(&alloc::format!(
                r#"<dds><permissions><grant {grant_attributes}><subject_name>CN=alice</subject_name>{VALIDITY}
                     <allow_rule><domains><id>0</id></domains>
                       <publish {publish_attributes}><topics><topic>T</topic></topics></publish>
                     </allow_rule>
                     {inside}
                     <default>DENY</default></grant></permissions></dds>"#
            ))
        };
        assert!(grant(r#"name="g""#, "", "").is_ok());
        // Unknown element.
        assert!(grant("", "<vendor/>", "").is_err());
        assert!(grant("", r#"<vendor must_interpret="true"/>"#, "").is_err());
        assert!(grant("", r#"<vendor must_interpret="false"/>"#, "").is_ok());
        assert!(grant("", r#"<vendor must_interpret="FALSE"><deep/></vendor>"#, "").is_ok());
        // Unknown attribute on a known element.
        assert!(grant(r#"vendor="x""#, "", "").is_err());
        assert!(grant("", "", r#"vendor="x""#).is_err());
        assert!(grant("", "", r#"vendor="x" must_interpret="false""#).is_ok());
        assert!(grant("", "", r#"vendor="x" must_interpret="TRUE""#).is_err());
        // The value of must_interpret.
        assert!(grant("", "", r#"must_interpret="yes""#).is_err());
        assert!(grant("", "", r#"must_interpret="true""#).is_ok());
        // On the root and the `<permissions>` element too.
        assert!(
            parse_permissions_xml(&alloc::format!(
                r#"<dds vendor="x"><permissions><grant><subject_name>CN=a</subject_name>{VALIDITY}
                     <allow_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics></publish></allow_rule>
                     <default>DENY</default></grant></permissions></dds>"#
            ))
            .is_err()
        );
    }

    /// The RFC 2253 and the OpenSSL one-line form name the same subject,
    /// in any order of the attributes; an escaped comma stays in the value.
    #[test]
    fn a_subject_name_reads_in_both_forms_and_any_order() {
        let rfc = subject_name_attributes("CN=alice, O=Example\\, Inc., C=DE").unwrap();
        let openssl = subject_name_attributes("/C=DE/O=Example, Inc./CN=alice").unwrap();
        assert_eq!(rfc, openssl);
        assert!(rfc.contains(&("O".to_string(), "Example, Inc.".to_string())));
        assert!(subject_name_attributes("not a name").is_none());
        assert!(subject_name_attributes("").is_none());
    }

    /// DDS-Security 1.2 §10.4.1.5.3.1: only the grant whose subject name is
    /// the certificate's counts; a document without one gives nothing. Every
    /// grant counted for every identity.
    #[test]
    fn only_the_grant_of_the_subject_counts() {
        let xml = alloc::format!(
            r#"<dds><permissions>
            <grant name="a"><subject_name>CN=alice,C=DE</subject_name>{VALIDITY}
              <allow_rule><domains><id>0</id></domains><publish><topics><topic>A</topic></topics></publish></allow_rule>
              <default>DENY</default></grant>
            <grant name="b"><subject_name>/C=DE/CN=bob</subject_name>{VALIDITY}
              <allow_rule><domains><id>0</id></domains><publish><topics><topic>B</topic></topics></publish></allow_rule>
              <default>DENY</default></grant>
            </permissions></dds>"#
        );
        let doc = parse_permissions_xml(&xml).unwrap();
        let bob = doc
            .for_subject(&pairs(&[("C", "DE"), ("CN", "bob")]))
            .expect("bob's grant");
        assert_eq!(bob.grants.len(), 1);
        assert_eq!(bob.grants[0].subject_text(), "/C=DE/CN=bob");
        assert!(
            doc.for_subject(&pairs(&[("C", "DE"), ("CN", "carol")]))
                .is_none()
        );
        // An attribute more or less is another name.
        assert!(doc.for_subject(&pairs(&[("CN", "alice")])).is_none());
    }

    /// §10.4.1.5.3.2.1: a `subject_name_expression` matches attribute by
    /// attribute with `fnmatch()`, `FNM_PATHNAME`; it was not read.
    #[test]
    fn a_subject_name_expression_matches_by_fnmatch() {
        let xml = alloc::format!(
            r#"<dds><permissions><grant name="agv">
               <subject_name_expression>CN=AGV/*, O=ACME</subject_name_expression>{VALIDITY}
               <allow_rule><domains><id>0</id></domains>
                 <publish><topics><topic>T</topic></topics></publish></allow_rule>
               <default>DENY</default></grant></permissions></dds>"#
        );
        let doc = parse_permissions_xml(&xml).unwrap();
        assert!(matches!(doc.grants[0].subject, GrantSubject::Expression(_)));
        assert!(
            doc.for_subject(&pairs(&[("CN", "AGV/agv1"), ("O", "ACME")]))
                .is_some()
        );
        assert!(
            doc.for_subject(&pairs(&[("CN", "AGV/a/b"), ("O", "ACME")]))
                .is_none(),
            "FNM_PATHNAME: * does not match /"
        );
        assert!(
            doc.for_subject(&pairs(&[("CN", "AGV/agv1"), ("O", "Other")]))
                .is_none()
        );
    }

    /// §10.4.1.5.3.4: "The rules are applied in the same order that appear
    /// in the document": an allow before a deny for the same topic allows.
    /// Every deny beat every allow.
    #[test]
    fn the_first_matching_rule_decides() {
        let allow_then_deny = grant(
            "<allow_rule><domains><id>0</id></domains><publish><topics><topic>Square</topic></topics></publish></allow_rule>\
             <deny_rule><domains><id>0</id></domains><publish><topics><topic>*</topic></topics></publish></deny_rule>",
            "<default>ALLOW</default>",
        );
        assert!(allowed(&allow_then_deny, Action::Publish, "Square", &[]));
        assert!(!allowed(&allow_then_deny, Action::Publish, "Circle", &[]));
        let deny_then_allow = grant(
            "<deny_rule><domains><id>0</id></domains><publish><topics><topic>Square</topic></topics></publish></deny_rule>\
             <allow_rule><domains><id>0</id></domains><publish><topics><topic>*</topic></topics></publish></allow_rule>",
            "<default>DENY</default>",
        );
        assert!(!allowed(&deny_then_allow, Action::Publish, "Square", &[]));
        assert!(allowed(&deny_then_allow, Action::Publish, "Circle", &[]));
        assert!(
            !allowed(&deny_then_allow, Action::Subscribe, "Circle", &[]),
            "no rule for subscribe: the default"
        );
    }

    /// §10.4.1.5.3.4.2.1: the domains of a deny rule scope it; it held for
    /// every domain.
    #[test]
    fn a_deny_rule_holds_only_in_its_domains() {
        let g = grant(
            "<deny_rule><domains><id>5</id></domains><publish><topics><topic>T</topic></topics></publish></deny_rule>",
            "<default>ALLOW</default>",
        );
        let mut r = req(Action::Publish, "T", &[], &[]);
        assert!(g.decide(&r, PartitionMatching::Subset).allowed());
        r.domain = Domain::from(5);
        assert_eq!(g.decide(&r, PartitionMatching::Subset), Decision::Deny);
    }

    /// §10.4.1.5.3.4.1.4: the partitions of an allow rule must contain all
    /// of the entity's ("" without the element); the legacy matching needs
    /// one. §10.4.1.5.3.4.2.4: a deny rule needs one of them ("*" without
    /// the element). The partitions of all rules held for each.
    #[test]
    fn partitions_of_allow_and_deny_rules() {
        let g = grant(
            "<allow_rule><domains><id>0</id></domains>\
               <publish><topics><topic>A</topic></topics><partitions><partition>P*</partition></partitions></publish>\
               <publish><topics><topic>B</topic></topics></publish></allow_rule>",
            "<default>DENY</default>",
        );
        assert!(allowed(&g, Action::Publish, "A", &["P1", "P2"]));
        assert!(!allowed(&g, Action::Publish, "A", &["P1", "X"]));
        assert!(
            !allowed(&g, Action::Publish, "A", &[]),
            "\"\" is not in {{P*}}"
        );
        assert!(allowed(&g, Action::Publish, "B", &[]));
        assert!(!allowed(&g, Action::Publish, "B", &["P1"]), "B: only \"\"");
        let legacy = g.decide(
            &req(Action::Publish, "A", &["P1", "X"], &[]),
            PartitionMatching::Legacy,
        );
        assert!(legacy.allowed(), "legacy: one partition suffices");

        let deny = grant(
            "<deny_rule><domains><id>0</id></domains>\
               <publish><topics><topic>A</topic></topics><partitions><partition>secret</partition></partitions></publish>\
               <publish><topics><topic>B</topic></topics></publish></deny_rule>",
            "<default>ALLOW</default>",
        );
        assert!(!allowed(&deny, Action::Publish, "A", &["public", "secret"]));
        assert!(allowed(&deny, Action::Publish, "A", &["public"]));
        assert!(!allowed(&deny, Action::Publish, "B", &["anything"]), "*");
    }

    /// §10.4.1.5.3.4.1.5 and §10.4.1.5.3.4.2.5: data tags of the entity are
    /// a subset of an allow rule's (the empty set without the element), and
    /// one of them in a deny rule's (every tag without it). They were read
    /// at the grant level and not applied.
    #[test]
    fn data_tags_of_allow_and_deny_rules() {
        let g = grant(
            "<allow_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics>\
               <data_tags><tag><name>k</name><value>v</value></tag></data_tags></publish>\
               <publish><topics><topic>U</topic></topics></publish></allow_rule>",
            "<default>DENY</default>",
        );
        let kv = [DataTag {
            name: "k".into(),
            value: "v".into(),
        }];
        let other = [DataTag {
            name: "k".into(),
            value: "w".into(),
        }];
        let ok = |topic, tags: &[DataTag]| {
            g.decide(
                &req(Action::Publish, topic, &[], tags),
                PartitionMatching::Subset,
            )
            .allowed()
        };
        assert!(ok("T", &kv));
        assert!(ok("T", &[]), "no tag is a subset");
        assert!(!ok("T", &other));
        assert!(ok("U", &[]));
        assert!(!ok("U", &kv), "U: the empty set only");

        let deny = grant(
            "<deny_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics>\
               <data_tags><tag><name>k</name><value>v</value></tag></data_tags></publish></deny_rule>",
            "<default>ALLOW</default>",
        );
        let denied = |tags: &[DataTag]| {
            !deny
                .decide(
                    &req(Action::Publish, "T", &[], tags),
                    PartitionMatching::Subset,
                )
                .allowed()
        };
        assert!(denied(&kv));
        assert!(!denied(&other));
        assert!(!denied(&[]));
    }

    /// §10.4.1.5.3.4.1.2: `<relay>` is an action of its own; it was not
    /// read.
    #[test]
    fn relay_is_an_action_of_its_own() {
        let g = grant(
            "<allow_rule><domains><id>0</id></domains><relay><topics><topic>T</topic></topics></relay></allow_rule>",
            "<default>DENY</default>",
        );
        assert!(allowed(&g, Action::Relay, "T", &[]));
        assert!(!allowed(&g, Action::Subscribe, "T", &[]));
    }

    /// Table 70 `check_create_participant`: the first rule for the domain
    /// decides; a deny rule without publish or subscribe refuses, one with
    /// them leaves it to the default.
    #[test]
    fn joining_follows_the_first_rule_for_the_domain() {
        let allow = grant(
            "<allow_rule><domains><id>0</id></domains></allow_rule>",
            "<default>DENY</default>",
        );
        assert!(allow.may_join(0));
        assert!(!allow.may_join(1), "no rule: the default");
        let deny_all = grant(
            "<deny_rule><domains><id>0</id></domains></deny_rule>",
            "<default>ALLOW</default>",
        );
        assert!(!deny_all.may_join(0));
        let deny_topic = grant(
            "<deny_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics></publish></deny_rule>",
            "<default>ALLOW</default>",
        );
        assert!(deny_topic.may_join(0));
    }

    /// §10.4.1.2.5.1: a rule without `<tag>` is for the empty DomainTag; a
    /// `<tag>` or `<tag_expression>` for the tags it names.
    #[test]
    fn rules_match_the_domain_tag() {
        let g = grant(
            "<allow_rule><domains><id>0</id><tag>Robot15</tag><tag_expression>AGVS/*</tag_expression></domains>\
               <publish><topics><topic>T</topic></topics></publish></allow_rule>\
             <allow_rule><domains><id>0</id></domains><publish><topics><topic>U</topic></topics></publish></allow_rule>",
            "<default>DENY</default>",
        );
        let at = |tag: &'static str, topic| {
            g.decide(
                &AccessRequest {
                    domain: Domain { id: 0, tag },
                    ..req(Action::Publish, topic, &[], &[])
                },
                PartitionMatching::Subset,
            )
            .allowed()
        };
        assert!(at("Robot15", "T"));
        assert!(at("AGVS/1", "T"));
        assert!(!at("", "T"));
        assert!(at("", "U"));
        assert!(!at("Robot15", "U"));
    }

    /// The XSD (§10.4.1.5): `<validity>`, a rule, `<domains>` in each rule
    /// and `<topics>` in each action are required; an element outside the
    /// XSD needs `must_interpret="false"` (§10.4.1.6). A grant without
    /// `<validity>` was unbounded, and the grant-level forms were taken.
    #[test]
    fn a_document_outside_the_xsd_is_refused() {
        let rule = "<allow_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics></publish></allow_rule>";
        assert!(parse_permissions_xml(&doc(rule, "<default>DENY</default>")).is_ok());
        let no_validity = alloc::format!(
            r#"<dds><permissions><grant name="g"><subject_name>CN=a</subject_name>{rule}</grant></permissions></dds>"#
        );
        assert!(parse_permissions_xml(&no_validity).is_err());
        assert!(
            parse_permissions_xml(&doc("", "<default>DENY</default>")).is_err(),
            "no rule"
        );
        assert!(
            parse_permissions_xml(&doc(
                "<allow_rule><publish><topics><topic>T</topic></topics></publish></allow_rule>",
                ""
            ))
            .is_err(),
            "rule without <domains>"
        );
        assert!(
            parse_permissions_xml(&doc(
                "<allow_rule><domains><id>0</id></domains><publish><topic>T</topic></publish></allow_rule>",
                ""
            ))
            .is_err(),
            "<topic> without <topics>"
        );
        assert!(
            parse_permissions_xml(&doc(
                &alloc::format!("{rule}<vendor_knob>1</vendor_knob>"),
                ""
            ))
            .is_err()
        );
        assert!(
            parse_permissions_xml(&doc(
                &alloc::format!(r#"{rule}<vendor_knob must_interpret="false">1</vendor_knob>"#),
                ""
            ))
            .is_ok()
        );
        let bare_root = alloc::format!(
            r#"<permissions><grant name="g"><subject_name>CN=a</subject_name>{VALIDITY}{rule}</grant></permissions>"#
        );
        assert!(
            parse_permissions_xml(&bare_root).is_err(),
            "the root is <dds>"
        );
    }

    /// §10.4.1.5.3.4: without `<default>` the grant denies.
    #[test]
    fn default_deny_without_explicit_tag() {
        let g = grant(
            "<allow_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics></publish></allow_rule>",
            "",
        );
        assert!(g.default_deny);
        assert!(!allowed(&g, Action::Publish, "U", &[]));
    }

    #[test]
    fn rejects_invalid_xml() {
        let err = parse_permissions_xml("<not-closed").unwrap_err();
        assert!(matches!(err, PermissionsError::InvalidXml(_)));
    }

    #[test]
    fn missing_subject_name_is_malformed() {
        let xml = alloc::format!(
            r#"<dds><permissions><grant name="g">{VALIDITY}<allow_rule><domains><id>0</id></domains></allow_rule></grant></permissions></dds>"#
        );
        let err = parse_permissions_xml(&xml).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
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
        assert_eq!(parse_iso_seconds("2026-13-01T00:00:00"), None); // Month 13
        assert_eq!(parse_iso_seconds("2026-04-24T25:00:00"), None); // Hour 25
        assert_eq!(parse_iso_seconds("2026-04-24T00:00:00 garbage"), None);
        assert_eq!(parse_iso_seconds("2026-04-24T00:00:00+0200"), None);
    }

    /// An `xs:dateTime` with a zone is converted to UTC, with a fraction
    /// of a second too. The offset was dropped.
    #[test]
    fn iso_parser_applies_the_zone() {
        let utc = parse_iso_seconds("2000-01-01T00:00:00Z").unwrap();
        assert_eq!(parse_iso_seconds("2000-01-01T02:00:00+02:00"), Some(utc));
        assert_eq!(parse_iso_seconds("1999-12-31T19:00:00-05:00"), Some(utc));
        assert_eq!(parse_iso_seconds("2000-01-01T00:00:00.250Z"), Some(utc));
    }

    /// §10.4.1.5.3.3 and the XSD: a `<validity>` with an unreadable bound,
    /// and a `<default>` that is neither ALLOW nor DENY, make the document
    /// invalid. They were read as no end and as ALLOW.
    #[test]
    fn an_unreadable_validity_or_default_is_refused() {
        let rule = "<allow_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics></publish></allow_rule>";
        let with = |validity: &str, default: &str| {
            alloc::format!(
                r#"<dds><permissions><grant name="g"><subject_name>CN=alice</subject_name>
                   {validity}{rule}<default>{default}</default></grant></permissions></dds>"#
            )
        };
        assert!(parse_permissions_xml(&with(VALIDITY, "DENY")).is_ok());
        assert!(parse_permissions_xml(&with(VALIDITY, "DENNY")).is_err());
        let bad = "<validity><not_before>2020-01-01T00:00:00</not_before><not_after>someday</not_after></validity>";
        assert!(parse_permissions_xml(&with(bad, "DENY")).is_err());
        let half = "<validity><not_after>2030-01-01T00:00:00Z</not_after></validity>";
        assert!(parse_permissions_xml(&with(half, "DENY")).is_err());
    }

    #[test]
    fn validity_window_enforced() {
        let xml = r#"<dds><permissions><grant name="g">
    <subject_name>CN=alice</subject_name>
    <validity>
      <not_before>2026-01-01T00:00:00Z</not_before>
      <not_after>2027-01-01T00:00:00Z</not_after>
    </validity>
    <allow_rule><domains><id>0</id></domains><publish><topics><topic>T</topic></topics></publish></allow_rule>
  </grant></permissions></dds>"#;
        let p = parse_permissions_xml(xml).unwrap();
        let g = &p.grants[0];
        let not_before = parse_iso_seconds("2026-01-01T00:00:00Z").unwrap();
        let not_after = parse_iso_seconds("2027-01-01T00:00:00Z").unwrap();
        assert!(!g.is_valid_at(not_before - 1), "just before not_before");
        assert!(g.is_valid_at(not_before), "exactly not_before (inclusive)");
        assert!(g.is_valid_at(not_before + 3600), "in the middle");
        assert!(!g.is_valid_at(not_after), "exactly not_after (exclusive)");
        assert!(!g.is_valid_at(u64::MAX / 2), "well after not_after");
    }

    #[test]
    fn domain_ranges_of_a_rule() {
        let g = grant(
            "<allow_rule><domains><id>5</id><id_range><min>10</min><max>20</max></id_range></domains>\
               <publish><topics><topic>T</topic></topics></publish></allow_rule>",
            "",
        );
        for (id, ok) in [(5, true), (6, false), (10, true), (20, true), (21, false)] {
            let r = AccessRequest {
                domain: Domain::from(id),
                ..req(Action::Publish, "T", &[], &[])
            };
            assert_eq!(
                g.decide(&r, PartitionMatching::Subset).allowed(),
                ok,
                "{id}"
            );
        }
    }

    #[test]
    fn domain_range_single_inclusive() {
        let r = DomainRange::single(42);
        assert!(r.contains(42));
        assert!(!r.contains(41));
        assert!(!r.contains(43));
    }
}

#[cfg(test)]
mod nesting_tests {
    use super::*;

    /// `roxmltree` recurses once per open element; a permissions document from outside
    /// with a long run of unclosed start tags is refused before it reaches
    /// the parser, instead of overflowing the stack.
    #[test]
    fn a_deeply_nested_text_is_refused_before_the_parser() {
        let text = "<a>".repeat(100_000);
        assert!(matches!(
            parse_permissions_xml(&text),
            Err(PermissionsError::InvalidXml(_))
        ));
    }
}
