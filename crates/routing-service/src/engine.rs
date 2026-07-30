// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The router orchestrator: turns a [`RouterConfig`] into running
//! [`ForwardingSession`]s over a pool of participants (one input + one output
//! participant per domain), with a participant-level loop guard and per-route
//! metrics.

use std::collections::BTreeMap;
use std::sync::Arc;

use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
use zerodds_dcps::{DomainParticipant, DomainParticipantFactory, DomainParticipantQos};
use zerodds_qos::{DeadlineQosPolicy, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy};

use crate::config::{Endpoint, Ownership, Route, RouterConfig};
use crate::error::{Result, RoutingError};
use crate::forwarding::{ForwardingSession, SampleProcessor, SessionParts};
use crate::metrics::{RouteMetrics, RouteMetricsSnapshot};
use crate::transform::TypeRegistry;

/// A running router instance.
pub struct Router {
    name: String,
    /// One input participant per domain.
    inputs: BTreeMap<i32, DomainParticipant>,
    /// One output participant per domain.
    outputs: BTreeMap<i32, DomainParticipant>,
    /// Running forward pumps, keyed by route name.
    sessions: BTreeMap<String, ForwardingSession>,
    /// Per-route metrics handles, keyed by route name.
    metrics: BTreeMap<String, RouteMetrics>,
    /// Type shapes for content filtering / transformation (empty = byte-only).
    shapes: TypeRegistry,
}

impl Router {
    /// Builds and starts a router from `config` (byte-only — no type shapes).
    ///
    /// # Errors
    /// [`RoutingError`] on validation failure, an unresolved type name, an
    /// unsupported option, or a DDS runtime error.
    pub fn start(config: &RouterConfig) -> Result<Self> {
        Self::start_with_types(config, TypeRegistry::new())
    }

    /// Like [`start`](Self::start) but with a [`TypeRegistry`] supplying the
    /// type shapes content filtering and field transformation need. Routes
    /// without a filter/transform forward bytes verbatim and ignore the
    /// registry; routes with a filter/transform require their input and output
    /// types to be present.
    ///
    /// # Errors
    /// [`RoutingError`] as for [`start`](Self::start), plus a missing type
    /// shape for a filtering/transforming route.
    pub fn start_with_types(config: &RouterConfig, shapes: TypeRegistry) -> Result<Self> {
        config.validate()?;
        let factory = DomainParticipantFactory::instance();
        let mut router = Self {
            name: config.name.clone(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            sessions: BTreeMap::new(),
            metrics: BTreeMap::new(),
            shapes,
        };
        // Phase 1 — create every participant first, so their handles exist.
        for route in &config.routes {
            Self::participant(&mut router.inputs, factory, route.input.domain)?;
            Self::participant(&mut router.outputs, factory, route.output.domain)?;
        }
        // Phase 2 — install the loop guard BEFORE any endpoint is created, so
        // the mutual `ignore_participant` is in effect at match time. Adding it
        // after endpoints already discovered each other in-process would not
        // tear an existing match down (the ignore filter is consulted only at
        // discovery/match time).
        router.wire_loop_guard(config);
        // Phase 3 — now wire readers/writers and start the pumps.
        for route in &config.routes {
            router.add_route(route)?;
        }
        Ok(router)
    }

    /// Router instance name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Names of the active routes.
    #[must_use]
    pub fn route_names(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    /// Metrics snapshot for a route, or `None` if no such route.
    #[must_use]
    pub fn route_metrics(&self, route: &str) -> Option<RouteMetricsSnapshot> {
        self.metrics.get(route).map(RouteMetrics::snapshot)
    }

    /// Stops all pumps and tears the router down.
    pub fn shutdown(&mut self) {
        for (_, mut session) in core::mem::take(&mut self.sessions) {
            session.stop();
        }
    }

    // -- internals ---------------------------------------------------------

    fn participant(
        map: &mut BTreeMap<i32, DomainParticipant>,
        factory: &DomainParticipantFactory,
        domain: i32,
    ) -> Result<()> {
        if let std::collections::btree_map::Entry::Vacant(slot) = map.entry(domain) {
            let p = factory
                .create_participant(domain, DomainParticipantQos::default())
                .map_err(|e| RoutingError::Dds(format!("create_participant({domain}): {e:?}")))?;
            slot.insert(p);
        }
        Ok(())
    }

    fn add_route(&mut self, route: &Route) -> Result<()> {
        let in_type = resolve_type(&route.input, &route.output, &route.name)?;
        let out_type = if route.output.type_name == "*" {
            in_type.clone()
        } else {
            route.output.type_name.clone()
        };

        // Participants were created in phase 1 (so the loop guard could be set
        // up before any endpoint exists); here we only wire the endpoints.
        let in_rt = self
            .inputs
            .get(&route.input.domain)
            .and_then(DomainParticipant::runtime)
            .cloned()
            .ok_or(RoutingError::Internal("input runtime"))?;
        let out_rt = self
            .outputs
            .get(&route.output.domain)
            .and_then(DomainParticipant::runtime)
            .cloned()
            .ok_or(RoutingError::Internal("output runtime"))?;

        let (_reader_eid, rx) = in_rt
            .register_user_reader_kind(user_reader_cfg(&route.input, &in_type), route.input.keyed)
            .map_err(|e| RoutingError::Dds(format!("register input reader: {e:?}")))?;

        let writer_eid = out_rt
            .register_user_writer_kind(
                user_writer_cfg(&route.output, &out_type),
                route.output.keyed,
            )
            .map_err(|e| RoutingError::Dds(format!("register output writer: {e:?}")))?;

        let metrics = RouteMetrics::new(&route.name);
        self.metrics.insert(route.name.clone(), metrics.clone());

        let processor: Option<Box<dyn SampleProcessor>> =
            crate::transform::build_processor_with(route, &in_type, &out_type, &self.shapes)?;

        let session = ForwardingSession::start(SessionParts {
            route_name: route.name.clone(),
            rx,
            output_rt: out_rt,
            writer_eid,
            keyed: route.output.keyed,
            processor,
            metrics: Arc::new(metrics),
            preserve_source_timestamp: route.preserve_source_timestamp,
            max_samples_per_second: route.max_samples_per_second,
        })?;
        self.sessions.insert(route.name.clone(), session);
        Ok(())
    }

    /// Participant-level loop guard: on every domain present as both an input
    /// and an output domain, make the router's input participant and output
    /// participant mutually invisible. So no router output (from any route) is
    /// ever ingested by any router input on the same domain — preventing
    /// router-internal loops (bidirectional bridges, multi-route cycles) while
    /// application writers stay visible. Cross-*process* router loops need a
    /// discovery discriminator and are out of scope (see crate docs).
    fn wire_loop_guard(&self, config: &RouterConfig) {
        if !config.routes.iter().any(|r| r.loop_guard) {
            return;
        }
        let shared: Vec<i32> = self
            .inputs
            .keys()
            .filter(|d| self.outputs.contains_key(d))
            .copied()
            .collect();
        for d in shared {
            if let (Some(i), Some(o)) = (self.inputs.get(&d), self.outputs.get(&d)) {
                let _ = i.ignore_participant(o.participant_handle());
                let _ = o.ignore_participant(i.participant_handle());
            }
        }
    }
}

impl Drop for Router {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn resolve_type(input: &Endpoint, output: &Endpoint, route: &str) -> Result<String> {
    if input.type_name != "*" {
        return Ok(input.type_name.clone());
    }
    if output.type_name != "*" {
        return Ok(output.type_name.clone());
    }
    Err(RoutingError::Config(format!(
        "route '{route}': a concrete type_name is required on the input (or output) endpoint \
         — RTPS matches readers/writers by topic + type name, there is no wildcard type match"
    )))
}

/// Builds the input reader config (accepts both XCDR1 and XCDR2 so it ingests
/// whatever representation the source emits).
fn user_reader_cfg(ep: &Endpoint, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: ep.topic.clone(),
        type_name: type_name.to_string(),
        reliable: ep.qos.reliable,
        durability: ep.qos.durability.into(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: ep.qos.ownership.into(),
        presentation: Default::default(),
        partition: ep.partition.clone(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: ep
            .qos
            .data_representation
            .clone()
            .or_else(|| Some(vec![0, 2])),
    }
}

/// Builds the output writer config.
fn user_writer_cfg(ep: &Endpoint, type_name: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: ep.topic.clone(),
        type_name: type_name.to_string(),
        reliable: ep.qos.reliable,
        durability: ep.qos.durability.into(),
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        ownership: ep.qos.ownership.into(),
        ownership_strength: if ep.qos.ownership == Ownership::Exclusive {
            ep.qos.ownership_strength
        } else {
            0
        },
        presentation: Default::default(),
        partition: ep.partition.clone(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: ep.qos.data_representation.clone(),
    }
}
