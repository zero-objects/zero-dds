// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mapping configuration between CORBA objects and DDS topics.
//!
//! A `BridgeMapping` aggregates several `BridgeRoute` entries. Each
//! route binds a CORBA object (by `repository_id` + `object_key`)
//! bidirectionally to one or more DDS topics.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Direction of a bridge route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// CORBA request → DDS sample (servant mode).
    CorbaToDds,
    /// DDS sample → CORBA request (forwarder mode).
    DdsToCorba,
    /// Bidirectional (`request topic` + `reply topic`).
    Bidirectional,
}

/// QoS profile reference for a topic — resolved by the caller layer
/// (e.g. via `crates/qos/`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicQosRef {
    /// Optional: reliability kind ("RELIABLE"/"BEST_EFFORT").
    pub reliability: Option<String>,
    /// Optional: durability kind ("VOLATILE"/"TRANSIENT_LOCAL"/...).
    pub durability: Option<String>,
    /// Optional: profile name from a QoS XML profile.
    pub profile_name: Option<String>,
}

/// Mapping of a single IDL operation onto a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationMapping {
    /// IDL operation name (wire form).
    pub operation: String,
    /// DDS topic for the request sample.
    pub request_topic: String,
    /// DDS topic for the reply sample (or empty for fire-and-forget).
    pub reply_topic: String,
    /// QoS for both topics.
    pub qos: TopicQosRef,
}

/// A bridge route — binds a CORBA object to operation topics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeRoute {
    /// CORBA repository ID of the object type (e.g.
    /// `IDL:demo/Echo:1.0`).
    pub repository_id: String,
    /// CORBA object key (POA-created).
    pub object_key: Vec<u8>,
    /// Direction of the bridge.
    pub direction: Direction,
    /// Operation mapping per IDL method.
    pub operations: Vec<OperationMapping>,
}

impl BridgeRoute {
    /// Returns the `OperationMapping` for an operation name.
    #[must_use]
    pub fn operation(&self, name: &str) -> Option<&OperationMapping> {
        self.operations.iter().find(|o| o.operation == name)
    }
}

/// Top-level mapping with O(1) lookup by `(repository_id, object_key)`.
#[derive(Debug, Clone, Default)]
pub struct BridgeMapping {
    routes: BTreeMap<RouteKey, BridgeRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RouteKey {
    repository_id: String,
    object_key: Vec<u8>,
}

impl BridgeMapping {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a route.
    pub fn add_route(&mut self, route: BridgeRoute) {
        let key = RouteKey {
            repository_id: route.repository_id.clone(),
            object_key: route.object_key.clone(),
        };
        self.routes.insert(key, route);
    }

    /// Looks up a route.
    #[must_use]
    pub fn lookup(&self, repository_id: &str, object_key: &[u8]) -> Option<&BridgeRoute> {
        let key = RouteKey {
            repository_id: repository_id.into(),
            object_key: object_key.to_vec(),
        };
        self.routes.get(&key)
    }

    /// Number of routes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// `true` if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// List of all routes.
    #[must_use]
    pub fn all_routes(&self) -> Vec<&BridgeRoute> {
        self.routes.values().collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn echo_route() -> BridgeRoute {
        BridgeRoute {
            repository_id: "IDL:demo/Echo:1.0".into(),
            object_key: alloc::vec![0xab],
            direction: Direction::Bidirectional,
            operations: alloc::vec![OperationMapping {
                operation: "ping".into(),
                request_topic: "demo/Echo/ping/Request".into(),
                reply_topic: "demo/Echo/ping/Reply".into(),
                qos: TopicQosRef {
                    reliability: Some("RELIABLE".into()),
                    durability: Some("VOLATILE".into()),
                    profile_name: None,
                },
            }],
        }
    }

    #[test]
    fn add_and_lookup_route() {
        let mut m = BridgeMapping::new();
        m.add_route(echo_route());
        let r = m.lookup("IDL:demo/Echo:1.0", &[0xab]).unwrap();
        assert_eq!(r.direction, Direction::Bidirectional);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn lookup_unknown_yields_none() {
        let m = BridgeMapping::new();
        assert!(m.lookup("IDL:demo/Echo:1.0", &[0xff]).is_none());
    }

    #[test]
    fn operation_mapping_by_name() {
        let r = echo_route();
        let op = r.operation("ping").unwrap();
        assert_eq!(op.request_topic, "demo/Echo/ping/Request");
        assert_eq!(op.reply_topic, "demo/Echo/ping/Reply");
        assert!(r.operation("nope").is_none());
    }

    #[test]
    fn add_route_replaces_existing() {
        let mut m = BridgeMapping::new();
        m.add_route(echo_route());
        let mut r2 = echo_route();
        r2.direction = Direction::CorbaToDds;
        m.add_route(r2);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.lookup("IDL:demo/Echo:1.0", &[0xab]).unwrap().direction,
            Direction::CorbaToDds
        );
    }

    #[test]
    fn all_routes_listing() {
        let mut m = BridgeMapping::new();
        m.add_route(echo_route());
        let mut other = echo_route();
        other.repository_id = "IDL:demo/Other:1.0".into();
        m.add_route(other);
        assert_eq!(m.all_routes().len(), 2);
    }

    #[test]
    fn direction_variants_distinct() {
        assert_ne!(Direction::CorbaToDds, Direction::DdsToCorba);
        assert_ne!(Direction::CorbaToDds, Direction::Bidirectional);
    }
}
