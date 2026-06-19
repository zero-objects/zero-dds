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
pub struct RouterConfig {
    /// Human-readable name (shown in logs/metrics).
    #[serde(default = "default_router_name")]
    pub name: String,
    /// Routes managed by this router.
    pub routes: Vec<Route>,
}

fn default_router_name() -> String {
    "zerodds-router".to_string()
}

/// One input→output coupling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

fn default_true() -> bool {
    true
}

/// A reader (input) or writer (output) on a DDS domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

fn default_type() -> String {
    "*".to_string()
}

/// QoS knobs honoured by both the input reader and the output writer. DDS↔DDS
/// routing maps QoS directly (same model both sides), so a single spec covers
/// either role; role-irrelevant fields (e.g. `ownership_strength` on a reader)
/// are simply ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct RenameRule {
    /// Output member.
    pub to: String,
    /// Input member.
    pub from: String,
}

/// A single constant assignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        }
        Ok(())
    }

    /// Parses a [`RouterConfig`] from JSON.
    ///
    /// # Errors
    /// [`RoutingError::Config`] on malformed JSON or a failed [`Self::validate`].
    pub fn from_json(s: &str) -> Result<Self> {
        let cfg: Self =
            serde_json::from_str(s).map_err(|e| RoutingError::Config(format!("json: {e}")))?;
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn route(name: &str, in_dom: i32, out_dom: i32) -> Route {
        Route {
            name: name.into(),
            input: Endpoint {
                domain: in_dom,
                topic: "T".into(),
                type_name: "*".into(),
                keyed: false,
                partition: vec![],
                qos: QosSpec::default(),
            },
            output: Endpoint {
                domain: out_dom,
                topic: "T".into(),
                type_name: "*".into(),
                keyed: false,
                partition: vec![],
                qos: QosSpec::default(),
            },
            filter: None,
            transform: None,
            loop_guard: true,
            preserve_source_timestamp: false,
        }
    }

    #[test]
    fn validate_accepts_distinct_domains() {
        let c = RouterConfig {
            name: "r".into(),
            routes: vec![route("a", 0, 1)],
        };
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_rejects_duplicate_names() {
        let c = RouterConfig {
            name: "r".into(),
            routes: vec![route("a", 0, 1), route("a", 1, 2)],
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_rejects_self_loop_without_guard() {
        let mut r = route("a", 7, 7);
        r.loop_guard = false;
        let c = RouterConfig {
            name: "r".into(),
            routes: vec![r],
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_rejects_keyedness_mismatch() {
        let mut r = route("a", 0, 1);
        r.input.keyed = true;
        let c = RouterConfig {
            name: "r".into(),
            routes: vec![r],
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn json_roundtrip() {
        let c = RouterConfig {
            name: "r".into(),
            routes: vec![route("a", 0, 1)],
        };
        let js = c.to_json().unwrap();
        let back = RouterConfig::from_json(&js).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn durability_maps_to_qos() {
        assert_eq!(
            zerodds_qos::DurabilityKind::from(Durability::Transient),
            zerodds_qos::DurabilityKind::Transient
        );
    }
}
