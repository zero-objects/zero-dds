// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Route configuration model.
//!
//! A [`RouterConfig`] is a named set of [`Route`]s. Each route couples an
//! input [`Endpoint`] (a reader on some domain/topic) to an output endpoint
//! (a writer on some — possibly different — domain/topic) with optional QoS
//! override, content filtering and field transformation.
//!
//! Three config surfaces, all producing the same model:
//! * **Programmatic** — build [`RouterConfig`] / [`Route`] directly.
//! * **JSON** — [`RouterConfig::from_json`] (serde).
//! * **XML** — [`RouterConfig::from_xml`], a focused subset of the RTI
//!   Routing Service schema plus a native `<zerodds_routing>` form.

use serde::{Deserialize, Serialize};

use crate::error::{Result, RoutingError};

/// A named collection of routes — the unit a router instance runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterConfig {
    /// Human-readable name (shown in logs/metrics).
    #[serde(default = "default_router_name")]
    pub name: String,
    /// Routes managed by this router.
    pub routes: Vec<Route>,
    /// Optional access-control list (O1). When present, every route's input and
    /// output endpoint must be permitted by a matching [`AclRule`] (scoped by
    /// the route's `tenant`), else the config is rejected. Absent = no
    /// restriction (opt-in, so existing configs are unaffected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_control: Option<AccessControl>,
}

/// Access-control list for a router: a set of allow rules. A route endpoint is
/// permitted iff at least one rule matches its tenant, domain, and topic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessControl {
    /// Allow rules; a route needs a match for both its input and output.
    pub allow: Vec<AclRule>,
}

/// One allow rule. `tenant`/`domain` are exact; `topics` are `*`/`?` globs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AclRule {
    /// Tenant this rule applies to. `None` = applies to any tenant (and to
    /// routes with no tenant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// DDS domain id the rule permits.
    pub domain: i32,
    /// Permitted topic-name globs (`*` any run, `?` one char). `["*"]` = any.
    pub topics: Vec<String>,
}

impl AccessControl {
    /// Whether an endpoint on `domain`/`topic` for `tenant` is permitted.
    #[must_use]
    pub fn permits(&self, tenant: Option<&str>, domain: i32, topic: &str) -> bool {
        self.allow.iter().any(|r| {
            (r.tenant.is_none() || r.tenant.as_deref() == tenant)
                && r.domain == domain
                && r.topics.iter().any(|g| glob_match(g, topic))
        })
    }
}

/// Minimal `*`/`?` glob match (`*` = any run incl. empty, `?` = one char).
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    /// zerodds-lint: recursion-depth 256 (glob backtracking; bounded by pattern + text length)
    fn m(p: &[u8], t: &[u8]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some(b'*') => m(&p[1..], t) || (!t.is_empty() && m(p, &t[1..])),
            Some(b'?') => !t.is_empty() && m(&p[1..], &t[1..]),
            Some(&c) => t.first() == Some(&c) && m(&p[1..], &t[1..]),
        }
    }
    m(pattern.as_bytes(), text.as_bytes())
}

fn default_router_name() -> String {
    "zerodds-router".to_string()
}

/// One input→output coupling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// Route name — must be unique within a [`RouterConfig`]; used as the
    /// metric label and the loop-guard tag.
    pub name: String,
    /// Where samples are read from.
    pub input: Endpoint,
    /// Where samples are written to.
    pub output: Endpoint,
    /// Optional SQL content filter (DDS 1.4 §2.2.8.4 grammar) applied to each
    /// input sample. Requires `output.type_name` to resolve to a known
    /// [`crate::transform::TypeShape`] (XTypes DynamicData) — otherwise route
    /// creation fails. `None` = forward everything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<ContentFilter>,
    /// Optional field transformation (rename topic is on the endpoint; this is
    /// per-field). Requires a resolvable type shape. `None` = byte pass-through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,
    /// When `true` (default), on any domain that is both an input and an output
    /// domain the router makes its input participant and output participant
    /// mutually invisible (`ignore_participant`), so no router output is ever
    /// re-ingested by a router input on the same domain — preventing
    /// router-internal forwarding loops in bidirectional or multi-route
    /// topologies. (Cross-*process* router loops need a discovery discriminator;
    /// see the crate docs §Limitations.)
    #[serde(default = "default_true")]
    pub loop_guard: bool,
    /// When `true`, the output writer preserves the input sample's source
    /// timestamp (for `DESTINATION_ORDER = BY_SOURCE_TIMESTAMP` correctness)
    /// instead of stamping the forward time. Default `false` (forward time).
    #[serde(default)]
    pub preserve_source_timestamp: bool,
    /// Optional forward rate limit in samples per second (O1 throttle). When
    /// set, the route forwards at most this many alive samples per second and
    /// drops the excess (counted in `dds_router_samples_dropped_throttle_total`);
    /// a short burst up to one second's budget is allowed. `None` = unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_samples_per_second: Option<u32>,
    /// Tenant this route belongs to (O1 multi-tenancy). Used to scope the
    /// access-control rules that must permit this route's endpoints. `None` =
    /// no tenant (matched by rules with no tenant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// Relay mode (DDS-Security 1.2 §7.1.1.4 with DDSI-RTPS 2.5 §8.7.9):
    /// the route forwards every sample and every dispose or unregister
    /// unchanged, on behalf of the writer that wrote it, on the same domain
    /// and topic. Its input is a relay reader: under data protection it
    /// receives the samples encrypted and forwards them without opening
    /// them, which the router participant's grant has to allow with
    /// `<relay>`, and its output writer needs `<publish>`. A reader that
    /// receives a sample from its writer and from the route takes it once,
    /// as that writer's. The source timestamp is always kept. Excludes a
    /// filter, a transform, another domain, another topic and another
    /// type. Default `false`.
    #[serde(default)]
    pub relay: bool,
}

fn default_true() -> bool {
    true
}

/// A reader (input) or writer (output) on a DDS domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    /// DDS domain id.
    pub domain: i32,
    /// Topic name. For the output endpoint this may differ from the input
    /// topic (topic renaming).
    pub topic: String,
    /// IDL type name. For the type-agnostic byte path the input and output
    /// type names should match the on-the-wire type. `"*"` is accepted as a
    /// wildcard meaning "match the input type name".
    #[serde(default = "default_type")]
    pub type_name: String,
    /// Whether the topic is keyed (RTPS entityKind, Spec §9.3.1.2). Input and
    /// output MUST agree; mismatched keyedness is rejected.
    #[serde(default)]
    pub keyed: bool,
    /// Partition names. Empty = the default partition (`""`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition: Vec<String>,
    /// QoS for this endpoint's reader/writer.
    #[serde(default)]
    pub qos: QosSpec,
    /// `type_file`, output endpoint of a keyed route only: a serialized
    /// COMPLETE TypeObject of the output type (the format
    /// `zerodds-zenoh-bridged` and `zerodds-amqp-bridged` read). The output
    /// writer reads instance keys with it: each forwarded sample carries its
    /// KeyHash, and a forwarded dispose or unregister carries the
    /// `serializedKey` Cyclone 11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 need
    /// to act on it — without it the marker has `PID_KEY_HASH` alone. The
    /// type is not announced; peers still match by type name. Empty = none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub type_file: String,
}

fn default_type() -> String {
    "*".to_string()
}

/// QoS knobs honoured by both the input reader and the output writer. DDS↔DDS
/// routing maps QoS directly (same model both sides), so a single spec covers
/// either role; role-irrelevant fields (e.g. `ownership_strength` on a reader)
/// are simply ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QosSpec {
    /// `true` = RELIABLE, `false` = BEST_EFFORT.
    #[serde(default = "default_true")]
    pub reliable: bool,
    /// Durability kind.
    #[serde(default)]
    pub durability: Durability,
    /// Ownership mode.
    #[serde(default)]
    pub ownership: Ownership,
    /// Strength for `Exclusive` ownership (writer side; ignored for `Shared`).
    #[serde(default)]
    pub ownership_strength: i32,
    /// DataRepresentation offer/accept list (`0` = XCDR1, `2` = XCDR2). `None`
    /// = the runtime default. The byte path defaults the input reader to
    /// accept both so it ingests whatever representation the source emits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_representation: Option<Vec<i16>>,
}

impl Default for QosSpec {
    fn default() -> Self {
        Self {
            reliable: true,
            durability: Durability::default(),
            ownership: Ownership::default(),
            ownership_strength: 0,
            data_representation: None,
        }
    }
}

/// Durability kind (serde-friendly mirror of [`zerodds_qos::DurabilityKind`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Durability {
    /// No history kept beyond live delivery.
    #[default]
    Volatile,
    /// Writer-local history, delivered to late joiners while the writer lives.
    TransientLocal,
    /// Service-backed history surviving the writer.
    Transient,
    /// Durable storage surviving a restart.
    Persistent,
}

impl From<Durability> for zerodds_qos::DurabilityKind {
    fn from(d: Durability) -> Self {
        match d {
            Durability::Volatile => zerodds_qos::DurabilityKind::Volatile,
            Durability::TransientLocal => zerodds_qos::DurabilityKind::TransientLocal,
            Durability::Transient => zerodds_qos::DurabilityKind::Transient,
            Durability::Persistent => zerodds_qos::DurabilityKind::Persistent,
        }
    }
}

/// Ownership mode (serde-friendly mirror of [`zerodds_qos::OwnershipKind`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    /// Multiple writers may publish the same instance.
    #[default]
    Shared,
    /// Highest-strength writer owns the instance.
    Exclusive,
}

impl From<Ownership> for zerodds_qos::OwnershipKind {
    fn from(o: Ownership) -> Self {
        match o {
            Ownership::Shared => zerodds_qos::OwnershipKind::Shared,
            Ownership::Exclusive => zerodds_qos::OwnershipKind::Exclusive,
        }
    }
}

/// SQL content filter (DDS 1.4 §2.2.8.4 grammar). Evaluated per input sample
/// after the sample is decoded to DynamicData via the route's type shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFilter {
    /// The filter expression, e.g. `temperature > 50 AND zone = 'A'`.
    pub expression: String,
    /// Positional `%n` parameters bound into the expression.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<String>,
}

/// Per-field transformation: rename and/or constant assignment. Field paths are
/// flat member names of the (final/appendable) struct.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    /// Output member name → input member name (copy with rename).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rename: Vec<RenameRule>,
    /// Output member name → constant literal (overwrite/inject).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_const: Vec<SetConstRule>,
    /// Member names dropped from the output (only valid when the output type is
    /// `@appendable`/`@mutable` and the member is optional/trailing).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop: Vec<String>,
}

/// A single rename mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameRule {
    /// Output member.
    pub to: String,
    /// Input member.
    pub from: String,
}

/// A single constant assignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetConstRule {
    /// Output member.
    pub field: String,
    /// Literal value (parsed against the member's type).
    pub value: String,
}

impl RouterConfig {
    /// Validates the config: non-empty route names, unique names, agreeing
    /// keyedness, and no self-loop (same domain+topic+partition in and out
    /// without a loop guard).
    ///
    /// # Errors
    /// [`RoutingError::Config`] describing the first problem found.
    pub fn validate(&self) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for r in &self.routes {
            if r.name.trim().is_empty() {
                return Err(RoutingError::Config("route with empty name".into()));
            }
            if !seen.insert(r.name.clone()) {
                return Err(RoutingError::Config(format!(
                    "duplicate route name '{}'",
                    r.name
                )));
            }
            if r.input.keyed != r.output.keyed {
                return Err(RoutingError::Config(format!(
                    "route '{}': input/output keyedness disagree",
                    r.name
                )));
            }
            if !r.input.type_file.is_empty() {
                return Err(RoutingError::Config(format!(
                    "route '{}': type_file is read on the output endpoint, the one whose \
                     writer keys the forwarded instances",
                    r.name
                )));
            }
            if !r.output.type_file.is_empty() && !r.output.keyed {
                return Err(RoutingError::Config(format!(
                    "route '{}': type_file on a route that is not keyed has nothing to key",
                    r.name
                )));
            }
            if r.relay {
                let input_type = r.input.type_name.as_str();
                let output_type = r.output.type_name.as_str();
                let why = if r.input.domain != r.output.domain {
                    Some("another domain")
                } else if r.input.topic != r.output.topic {
                    Some("another topic")
                } else if input_type != "*" && output_type != "*" && input_type != output_type {
                    Some("another type")
                } else if r.filter.is_some() {
                    Some("a filter")
                } else if r.transform.is_some() {
                    Some("a transform")
                } else {
                    None
                };
                if let Some(why) = why {
                    return Err(RoutingError::Config(format!(
                        "route '{}': a relay route forwards unchanged on the writer's behalf \
                         (DDS-Security 1.2 §7.1.1.4), it cannot have {why}",
                        r.name
                    )));
                }
            }
            let same_endpoint = r.input.domain == r.output.domain
                && r.input.topic == r.output.topic
                && r.input.partition == r.output.partition;
            if same_endpoint && !r.loop_guard {
                return Err(RoutingError::Config(format!(
                    "route '{}': identical input and output endpoint without loop_guard \
                     would forward to itself",
                    r.name
                )));
            }
            // O1 access control: when an ACL is present, both endpoints must be
            // permitted for this route's tenant.
            if let Some(acl) = &self.access_control {
                let t = r.tenant.as_deref();
                for (side, ep) in [("input", &r.input), ("output", &r.output)] {
                    if !acl.permits(t, ep.domain, &ep.topic) {
                        return Err(RoutingError::Config(format!(
                            "route '{}': {side} endpoint (tenant {:?}, domain {}, topic '{}') \
                             is not permitted by the access-control list",
                            r.name, t, ep.domain, ep.topic
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Parses a [`RouterConfig`] from JSON.
    ///
    /// # Errors
    /// [`RoutingError::Config`] on malformed JSON or a failed [`Self::validate`].
    ///
    /// A key the model does not read is refused (it used to be dropped
    /// without a word, and `validate` reported OK). Keys beginning with `_`
    /// are comments (`"_comment": "…"`), JSON having no other form, and are
    /// removed before the check.
    pub fn from_json(s: &str) -> Result<Self> {
        let mut value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| RoutingError::Config(format!("json: {e}")))?;
        strip_comments(&mut value);
        let cfg: Self = serde_json::from_value(value)
            .map_err(|e| RoutingError::Config(format!("json: {e}")))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Serializes to pretty JSON.
    ///
    /// # Errors
    /// [`RoutingError::Config`] if serialization fails (should not happen for a
    /// valid model).
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| RoutingError::Config(format!("json: {e}")))
    }

    /// Parses a [`RouterConfig`] from XML. Two accepted root forms:
    ///
    /// * `<zerodds_routing name="...">` with `<route>` children (native).
    /// * `<dds><routing_service>...<domain_route><session><topic_route>` —
    ///   an RTI-Routing-Service-compatible subset.
    ///
    /// # Errors
    /// [`RoutingError::Config`] on malformed XML, an unsupported structure, or
    /// a failed [`Self::validate`].
    pub fn from_xml(s: &str) -> Result<Self> {
        let cfg = crate::xml::parse_router_xml(s)?;
        cfg.validate()?;
        Ok(cfg)
    }
}

/// Removes every object key beginning with `_`, at any depth.
fn strip_comments(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| !k.starts_with('_'));
            map.values_mut().for_each(strip_comments);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_comments),
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    //! The oracle of these tests is the configuration as the crate's README
    //! documents it ("Config — JSON", "Config — validation") and the DDS
    //! 1.4 kinds the QoS keys name (§2.2.3).
    use super::*;

    /// A route `a` from `in_dom` to `out_dom` on topic `T`, as JSON members.
    fn route_json(extra: &str, in_dom: i32, out_dom: i32) -> String {
        format!(
            r#"{{ "name": "a", {extra}
                 "input":  {{ "domain": {in_dom}, "topic": "T" }},
                 "output": {{ "domain": {out_dom}, "topic": "T" }} }}"#
        )
    }

    fn refused(json: &str, why: &str) {
        let err = RouterConfig::from_json(json).unwrap_err().to_string();
        assert!(err.contains(why), "{why}: {err}");
    }

    /// An unknown key is refused; a `_`-key is a comment.
    #[test]
    fn an_unknown_key_is_refused_and_an_underscore_key_is_a_comment() {
        let good = r#"{"_comment": "x", "name": "r", "routes": [{"_note": "y", "name": "a",
            "input": {"domain": 0, "topic": "T"}, "output": {"domain": 1, "topic": "T"}}]}"#;
        assert!(RouterConfig::from_json(good).is_ok());
        refused(&good.replace("\"_note\"", "\"loop_gard\""), "loop_gard");
    }

    /// The QoS keys of an endpoint name the DDS 1.4 policies and kinds
    /// (§2.2.3.4 DURABILITY, §2.2.3.14 RELIABILITY, §2.2.3.23 OWNERSHIP):
    /// each spelling reads as its kind.
    #[test]
    fn the_qos_keys_read_as_the_dds_kinds_they_name() {
        for (spelling, kind) in [
            ("volatile", zerodds_qos::DurabilityKind::Volatile),
            (
                "transient_local",
                zerodds_qos::DurabilityKind::TransientLocal,
            ),
            ("transient", zerodds_qos::DurabilityKind::Transient),
            ("persistent", zerodds_qos::DurabilityKind::Persistent),
        ] {
            let json = format!(
                r#"{{ "routes": [ {{ "name": "a", "input": {{ "domain": 0, "topic": "T" }},
                     "output": {{ "domain": 1, "topic": "T", "qos": {{ "durability": "{spelling}",
                     "reliable": false, "ownership": "exclusive", "ownership_strength": 7 }} }} }} ] }}"#
            );
            let cfg = RouterConfig::from_json(&json).unwrap();
            let qos = &cfg.routes[0].output.qos;
            assert_eq!(
                zerodds_qos::DurabilityKind::from(qos.durability),
                kind,
                "{spelling}"
            );
            assert!(!qos.reliable);
            assert_eq!(
                zerodds_qos::OwnershipKind::from(qos.ownership),
                zerodds_qos::OwnershipKind::Exclusive
            );
            assert_eq!(qos.ownership_strength, 7);
            // Left out: RELIABLE, SHARED (README, "Config — JSON").
            let input = &cfg.routes[0].input.qos;
            assert!(input.reliable);
            assert_eq!(
                zerodds_qos::OwnershipKind::from(input.ownership),
                zerodds_qos::OwnershipKind::Shared
            );
        }
    }

    /// The configurations `validate` refuses, each with the reason it
    /// names: a duplicate route name, disagreeing keyedness, a route onto
    /// its own endpoint without the loop guard, a `type_file` on the input
    /// or on an unkeyed route.
    #[test]
    fn validate_refuses_what_cannot_be_routed() {
        refused(
            &format!(
                r#"{{ "routes": [ {}, {} ] }}"#,
                route_json("", 0, 1),
                route_json("", 1, 2)
            ),
            "duplicate route name 'a'",
        );
        refused(
            r#"{ "routes": [ { "name": "a", "input": { "domain": 0, "topic": "T", "keyed": true },
                 "output": { "domain": 1, "topic": "T" } } ] }"#,
            "keyedness disagree",
        );
        refused(
            &format!(
                r#"{{ "routes": [ {} ] }}"#,
                route_json(r#""loop_guard": false,"#, 7, 7)
            ),
            "would forward to itself",
        );
        refused(
            r#"{ "routes": [ { "name": "a", "input": { "domain": 0, "topic": "T", "type_file": "x" },
                 "output": { "domain": 1, "topic": "T" } } ] }"#,
            "type_file is read on the output endpoint",
        );
        refused(
            r#"{ "routes": [ { "name": "a", "input": { "domain": 0, "topic": "T" },
                 "output": { "domain": 1, "topic": "T", "type_file": "x" } } ] }"#,
            "nothing to key",
        );
        // The same route on its own endpoint with the guard is accepted.
        assert!(
            RouterConfig::from_json(&format!(r#"{{ "routes": [ {} ] }}"#, route_json("", 7, 7)))
                .is_ok()
        );
    }

    /// A relay route forwards unchanged on the writer's behalf: the same
    /// domain, topic and type, no filter and no transform.
    #[test]
    fn a_relay_route_stays_on_its_domain_topic_and_type() {
        let relay = |extra: &str, out_dom: i32, out_topic: &str, out_type: &str| {
            format!(
                r#"{{ "routes": [ {{ "name": "a", "relay": true, {extra}
                     "input":  {{ "domain": 3, "topic": "T", "type_name": "A" }},
                     "output": {{ "domain": {out_dom}, "topic": "{out_topic}", "type_name": "{out_type}" }} }} ] }}"#
            )
        };
        assert!(RouterConfig::from_json(&relay("", 3, "T", "A")).is_ok());
        refused(&relay("", 4, "T", "A"), "another domain");
        refused(&relay("", 3, "U", "A"), "another topic");
        refused(&relay("", 3, "T", "B"), "another type");
        refused(
            &relay(r#""filter": { "expression": "x > 1" },"#, 3, "T", "A"),
            "a filter",
        );
    }

    /// With an access-control list a route needs a rule for each of its
    /// endpoints: the rule's tenant (none: any), its domain, and one of its
    /// topic globs, where `*` matches any run of characters and `?` one.
    #[test]
    fn the_access_control_list_needs_a_rule_for_each_endpoint() {
        let config = |tenant: &str, rules: &str| {
            format!(
                r#"{{ "routes": [ {{ "name": "a", {tenant}
                     "input":  {{ "domain": 0, "topic": "SensorA" }},
                     "output": {{ "domain": 1, "topic": "SensorAB" }} }} ],
                     "access_control": {{ "allow": [ {rules} ] }} }}"#
            )
        };
        let rule = |tenant: &str, domain: i32, glob: &str| {
            format!(r#"{{ {tenant} "domain": {domain}, "topics": ["{glob}"] }}"#)
        };
        // Both endpoints permitted, by `?` and by `*`.
        assert!(
            RouterConfig::from_json(&config(
                "",
                &format!("{}, {}", rule("", 0, "Sensor?"), rule("", 1, "S*B"))
            ))
            .is_ok()
        );
        // `?` is one character: `SensorAB` on domain 1 is not `Sensor?`.
        refused(
            &config(
                "",
                &format!("{}, {}", rule("", 0, "Sensor?"), rule("", 1, "Sensor?")),
            ),
            "output endpoint",
        );
        // No rule for domain 1.
        refused(&config("", &rule("", 0, "*")), "output endpoint");
        // A rule of another tenant does not apply.
        refused(
            &config(
                r#""tenant": "acme","#,
                &format!(
                    "{}, {}",
                    rule(r#""tenant": "other","#, 0, "*"),
                    rule(r#""tenant": "other","#, 1, "*")
                ),
            ),
            "input endpoint",
        );
    }
}
