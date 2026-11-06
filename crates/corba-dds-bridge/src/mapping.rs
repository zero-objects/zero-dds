// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mapping configuration between CORBA objects and DDS topics, as code.
//!
//! A `BridgeMapping` aggregates several `BridgeRoute` entries. Each
//! route binds a CORBA object (by `repository_id` + `object_key`)
//! bidirectionally to one or more DDS topics. The daemon reads its
//! mappings from its configuration (`bridge_daemon`, spec §3); the
//! examples `examples/tutorials/dds-chat/integrations/corba` and
//! `examples/demos/dds-warehouse/stations/09-erp-corba` describe theirs
//! with these types.

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
