// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Prometheus families of spec §8.2 beside the generic `zerodds_grpc_*`
//! counters.

use super::*;

/// The bin's metric sets: the generic `zerodds_grpc_*` counters and the
/// `zerodds_grpc_bridge_*` families of spec §8.2.
pub(crate) struct DaemonMetrics {
    pub(crate) base: BridgeMetrics,
    pub(crate) spec: SpecMetrics,
}

/// Per-topic counters of §8.2, labelled `dds_topic`.
pub(crate) struct TopicMetrics {
    pub(crate) messages_in: Arc<zerodds_monitor::Counter>,
    pub(crate) messages_out: Arc<zerodds_monitor::Counter>,
    pub(crate) bytes_in: Arc<zerodds_monitor::Counter>,
    pub(crate) bytes_out: Arc<zerodds_monitor::Counter>,
    pub(crate) samples_received: Arc<zerodds_monitor::Counter>,
    pub(crate) samples_published: Arc<zerodds_monitor::Counter>,
    pub(crate) samples_dropped: Arc<zerodds_monitor::Counter>,
}

/// Spec §8.2. Label values come from the configuration or a fixed set, so
/// a client naming arbitrary services cannot grow the registry: a call to
/// a service that is not mapped counts as `service="unknown"`, a method
/// outside the bridge's own as `method="other"`.
pub(crate) struct SpecMetrics {
    pub(crate) registry: Arc<Registry>,
    pub(crate) topics: std::collections::HashMap<String, TopicMetrics>,
    pub(crate) streams: std::collections::HashMap<String, Arc<zerodds_monitor::Gauge>>,
}

pub(crate) const KNOWN_METHODS: &[&str] = &[
    "Publish",
    "PublishOne",
    "Subscribe",
    "Catalog",
    "ServerReflectionInfo",
];

impl SpecMetrics {
    pub(crate) fn register(registry: &Arc<Registry>, cfg: &DaemonConfig) -> Self {
        use zerodds_monitor::Labels;
        for (name, help) in [
            (
                "zerodds_grpc_bridge_rpcs_total",
                "gRPC calls answered, by status",
            ),
            ("zerodds_grpc_bridge_streams_active", "Calls being served"),
            (
                "zerodds_grpc_bridge_messages_in_total",
                "gRPC messages published to a topic",
            ),
            (
                "zerodds_grpc_bridge_messages_out_total",
                "gRPC messages sent from a topic",
            ),
            (
                "zerodds_grpc_bridge_bytes_in_total",
                "Bytes of the Length-Prefixed-Messages published",
            ),
            (
                "zerodds_grpc_bridge_bytes_out_total",
                "Bytes of the Length-Prefixed-Messages sent",
            ),
            (
                "zerodds_grpc_bridge_dds_samples_received",
                "DDS samples taken for Subscribe",
            ),
            (
                "zerodds_grpc_bridge_dds_samples_published",
                "DDS samples written by Publish",
            ),
            (
                "zerodds_grpc_bridge_auth_failures_total",
                "Calls refused by the auth check",
            ),
            (
                "zerodds_grpc_bridge_h2_resets_total",
                "RST_STREAM frames received",
            ),
            (
                "zerodds_grpc_bridge_samples_dropped_total",
                "DDS samples a best-effort Subscribe stream dropped for a full window queue",
            ),
        ] {
            registry.set_help(name, help);
        }
        let mut topics = std::collections::HashMap::new();
        let mut streams = std::collections::HashMap::new();
        // §1 L6: a tenant's topic and service are `<tenant>/<name>`.
        let every_topic = cfg
            .topics
            .iter()
            .map(|t| (None, t))
            .chain(cfg.tenants.iter().flat_map(|tenant| {
                tenant
                    .topics
                    .iter()
                    .map(|t| (Some(tenant.name.as_str()), t))
            }));
        for (tenant, t) in every_topic {
            let topic_label = tenant_label(tenant, &t.dds_name);
            let service_label = tenant_label(tenant, &t.grpc_service);
            let l = || Labels::new().with("dds_topic", topic_label.clone());
            topics.insert(
                topic_label.clone(),
                TopicMetrics {
                    messages_in: registry.counter("zerodds_grpc_bridge_messages_in_total", l()),
                    messages_out: registry.counter("zerodds_grpc_bridge_messages_out_total", l()),
                    bytes_in: registry.counter("zerodds_grpc_bridge_bytes_in_total", l()),
                    bytes_out: registry.counter("zerodds_grpc_bridge_bytes_out_total", l()),
                    samples_received: registry
                        .counter("zerodds_grpc_bridge_dds_samples_received", l()),
                    samples_published: registry
                        .counter("zerodds_grpc_bridge_dds_samples_published", l()),
                    samples_dropped: registry
                        .counter("zerodds_grpc_bridge_samples_dropped_total", l()),
                },
            );
            streams.insert(
                service_label.clone(),
                registry.gauge(
                    "zerodds_grpc_bridge_streams_active",
                    Labels::new().with("service", service_label.clone()),
                ),
            );
        }
        Self {
            registry: Arc::clone(registry),
            topics,
            streams,
        }
    }

    /// The `service` label of a call.
    pub(crate) fn service_label(part: &Participant, service: &str) -> String {
        if let Some(t) = topic_for(&part.topics, service) {
            part.label(&t.grpc_service)
        } else if crate::reflection::is_reflection_service(service) {
            service.to_string()
        } else {
            "unknown".into()
        }
    }

    /// The `method` label: the bridge's own methods, else `other`.
    pub(crate) fn method_label(method: &str) -> String {
        if KNOWN_METHODS.contains(&method) {
            method.to_string()
        } else {
            "other".into()
        }
    }

    /// The `streams_active` gauge of a call on `service`, for a mapped
    /// topic.
    pub(crate) fn stream_gauge(
        &self,
        part: &Participant,
        service: &str,
    ) -> Option<Arc<zerodds_monitor::Gauge>> {
        let t = topic_for(&part.topics, service)?;
        self.streams.get(&part.label(&t.grpc_service)).cloned()
    }

    /// `rpcs_total` of a call that ended with `status`.
    pub(crate) fn count(&self, part: &Participant, service: &str, method: &str, status: Status) {
        self.registry
            .counter(
                "zerodds_grpc_bridge_rpcs_total",
                zerodds_monitor::Labels::new()
                    .with("service", Self::service_label(part, service))
                    .with("method", Self::method_label(method))
                    .with("status", status.name()),
            )
            .inc();
    }

    /// One `Publish` message to the topic labelled `topic_label`: its
    /// Length-Prefixed-Message of `wire_len` octets.
    pub(crate) fn received(&self, topic_label: &str, wire_len: usize) {
        if let Some(m) = self.topics.get(topic_label) {
            m.messages_in.inc();
            m.bytes_in.add(wire_len as u64);
        }
    }

    /// Spec §6 BEST_EFFORT: `count` samples a stream dropped.
    pub(crate) fn dropped(&self, topic_label: &str, count: u64) {
        if let Some(m) = self.topics.get(topic_label) {
            m.samples_dropped.add(count);
        }
    }

    /// One sample a `Publish` wrote to DDS.
    pub(crate) fn published(&self, topic_label: &str) {
        if let Some(m) = self.topics.get(topic_label) {
            m.samples_published.inc();
        }
    }

    /// One message of a Subscribe stream of the topic labelled
    /// `topic_label`, `len` octets of protobuf.
    pub(crate) fn streamed(&self, topic_label: &str, len: usize) {
        if let Some(m) = self.topics.get(topic_label) {
            m.samples_received.inc();
            m.messages_out.inc();
            // The Length-Prefixed-Message, prefix included, as `bytes_in`
            // counts the request's.
            m.bytes_out.add(len as u64 + 5);
        }
    }

    pub(crate) fn auth_failure(&self, e: &crate::bridge_security::AuthError) {
        use crate::bridge_security::AuthError;
        let reason = match e {
            AuthError::MissingCredentials => "missing",
            AuthError::MalformedCredentials(_) => "malformed",
            AuthError::Rejected(_) => "rejected",
            AuthError::Misconfigured(_) => "misconfigured",
        };
        self.registry
            .counter(
                "zerodds_grpc_bridge_auth_failures_total",
                zerodds_monitor::Labels::new().with("reason", reason),
            )
            .inc();
    }

    /// RFC 7540 §7 error code names; others count as `unknown`.
    pub(crate) fn h2_reset(&self, code: u32) {
        const NAMES: [&str; 14] = [
            "NO_ERROR",
            "PROTOCOL_ERROR",
            "INTERNAL_ERROR",
            "FLOW_CONTROL_ERROR",
            "SETTINGS_TIMEOUT",
            "STREAM_CLOSED",
            "FRAME_SIZE_ERROR",
            "REFUSED_STREAM",
            "CANCEL",
            "COMPRESSION_ERROR",
            "CONNECT_ERROR",
            "ENHANCE_YOUR_CALM",
            "INADEQUATE_SECURITY",
            "HTTP_1_1_REQUIRED",
        ];
        let name = usize::try_from(code)
            .ok()
            .and_then(|i| NAMES.get(i))
            .copied()
            .unwrap_or("unknown");
        self.registry
            .counter(
                "zerodds_grpc_bridge_h2_resets_total",
                zerodds_monitor::Labels::new().with("error_code", name),
            )
            .inc();
    }
}
