// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The families of spec §8.2.

#[allow(unused_imports)]
use super::prelude::*;

/// Reply status as the `status` label of `requests_total`.
#[derive(Clone, Copy)]
pub(crate) enum Outcome {
    Ok = 0,
    User = 1,
    System = 2,
}

/// The families of spec §8.2 for one mapping. Their label values come from
/// the configuration, never from the wire: an operation name a client sends
/// would otherwise grow the label set without bound.
pub(crate) struct OpMetrics {
    pub(crate) requests: [Arc<zerodds_monitor::Counter>; 3],
    pub(crate) replies_sent: Arc<zerodds_monitor::Counter>,
    pub(crate) pending: Arc<zerodds_monitor::Gauge>,
    pub(crate) latency: Arc<zerodds_monitor::LabeledHistogram>,
    pub(crate) timeouts: Arc<zerodds_monitor::Counter>,
    /// `dds_samples_published{dds_topic}` of the request topic.
    pub(crate) published: Arc<zerodds_monitor::Counter>,
    /// `dds_samples_received{dds_topic}` of the reply or notify topic.
    pub(crate) received: Arc<zerodds_monitor::Counter>,
}

/// The `reason` labels of `csiv2_failures_total`: no SAS context where
/// trust in the client is required, then the `ContextError` major status
/// of a refused `EstablishContext` (CSIv2: 1 invalid evidence, 2 invalid
/// mechanism, 3 conflicting evidence, 4 no context).
pub(crate) const CSIV2_REASONS: [&str; 6] = [
    "missing",
    "invalid_evidence",
    "invalid_mechanism",
    "conflicting_evidence",
    "no_context",
    "other",
];

/// The `reason` of a refused `EstablishContext`, from its `ContextError`.
pub(crate) fn csiv2_reason(reply: &zerodds_corba_csiv2::sas::SasMessage) -> &'static str {
    match reply {
        zerodds_corba_csiv2::sas::SasMessage::ContextError(e) => match e.major_status {
            1 => "invalid_evidence",
            2 => "invalid_mechanism",
            3 => "conflicting_evidence",
            4 => "no_context",
            _ => "other",
        },
        _ => "other",
    }
}

/// Spec §8.2: `zerodds_corba_bridge_*`.
pub(crate) struct SpecMetrics {
    pub(crate) ops: Vec<OpMetrics>,
    pub(crate) locate_requests: Arc<zerodds_monitor::Counter>,
    pub(crate) giop_message_errors: Arc<zerodds_monitor::Counter>,
    /// `csiv2_failures_total{reason}`, in the order of [`CSIV2_REASONS`].
    pub(crate) csiv2_failures: Vec<Arc<zerodds_monitor::Counter>>,
}

impl SpecMetrics {
    /// Counts a refusal of CSIv2 with `reason`.
    pub(crate) fn csiv2_failure(&self, reason: &str) {
        if let Some(i) = CSIV2_REASONS.iter().position(|r| *r == reason)
            && let Some(c) = self.csiv2_failures.get(i)
        {
            c.inc();
        }
    }
}

impl SpecMetrics {
    /// The families of `cfg`'s mappings; `label` (`<tenant>/` or empty)
    /// goes before each operation and topic label (§1 L6).
    pub(crate) fn register(
        registry: &zerodds_monitor::Registry,
        cfg: &DaemonConfig,
        label: &str,
    ) -> Self {
        use zerodds_monitor::Labels;
        for (name, help) in [
            (
                "zerodds_corba_bridge_requests_total",
                "GIOP requests of a mapped operation, by reply status",
            ),
            (
                "zerodds_corba_bridge_replies_sent_total",
                "GIOP replies sent, TIMEOUT included",
            ),
            (
                "zerodds_corba_bridge_pending_requests",
                "Requests waiting for their DDS reply",
            ),
            (
                "zerodds_corba_bridge_request_latency_seconds",
                "Request to DDS reply",
            ),
            (
                "zerodds_corba_bridge_timeouts_total",
                "Requests answered with CORBA::TIMEOUT",
            ),
            (
                "zerodds_corba_bridge_dds_samples_received",
                "DDS samples taken from a reply or notify topic",
            ),
            (
                "zerodds_corba_bridge_dds_samples_published",
                "DDS samples written to a request topic",
            ),
            (
                "zerodds_corba_bridge_locate_requests_total",
                "GIOP LocateRequests",
            ),
            (
                "zerodds_corba_bridge_csiv2_failures_total",
                "Requests CSIv2 refused, by reason",
            ),
            (
                "zerodds_corba_bridge_giop_message_errors_total",
                "GIOP messages refused as malformed or too large",
            ),
        ] {
            registry.set_help(name, help);
        }
        let ops = cfg
            .mappings
            .iter()
            .map(|m| {
                let op = || Labels::new().with("operation", format!("{label}{}", m.operation));
                let status = |s: &str| {
                    registry.counter(
                        "zerodds_corba_bridge_requests_total",
                        op().with("status", s.to_string()),
                    )
                };
                let received_topic = if m.receives_giop() {
                    &m.reply_topic.dds_name
                } else {
                    &m.notify_topic.dds_name
                };
                OpMetrics {
                    requests: [
                        status("no_exception"),
                        status("user_exception"),
                        status("system_exception"),
                    ],
                    replies_sent: registry.counter("zerodds_corba_bridge_replies_sent_total", op()),
                    pending: registry.gauge("zerodds_corba_bridge_pending_requests", op()),
                    latency: registry
                        .histogram("zerodds_corba_bridge_request_latency_seconds", op()),
                    timeouts: registry.counter("zerodds_corba_bridge_timeouts_total", op()),
                    published: registry.counter(
                        "zerodds_corba_bridge_dds_samples_published",
                        Labels::new()
                            .with("dds_topic", format!("{label}{}", m.request_topic.dds_name)),
                    ),
                    received: registry.counter(
                        "zerodds_corba_bridge_dds_samples_received",
                        Labels::new().with("dds_topic", format!("{label}{received_topic}")),
                    ),
                }
            })
            .collect();
        Self {
            ops,
            locate_requests: registry
                .counter("zerodds_corba_bridge_locate_requests_total", Labels::new()),
            giop_message_errors: registry.counter(
                "zerodds_corba_bridge_giop_message_errors_total",
                Labels::new(),
            ),
            csiv2_failures: CSIV2_REASONS
                .iter()
                .map(|r| {
                    registry.counter(
                        "zerodds_corba_bridge_csiv2_failures_total",
                        Labels::new().with("reason", (*r).to_string()),
                    )
                })
                .collect(),
        }
    }
}

/// A request that got its reply (or TIMEOUT, or a refusal after it was
/// pending): the families of its mapping.
pub(crate) fn count_reply(d: &Dispatch, p: &Pending, outcome: Outcome) {
    if let Some(m) = d.spec.as_ref().and_then(|s| s.ops.get(p.mapping)) {
        m.pending.dec();
        m.replies_sent.inc();
        m.requests[outcome as usize].inc();
        m.latency
            .record_seconds(p.ctx.started.elapsed().as_secs_f64());
    }
}

/// A request of mapping `index` refused before it went to DDS.
pub(crate) fn count_refused(d: &Dispatch, index: usize) {
    if let Some(m) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
        m.replies_sent.inc();
        m.requests[Outcome::System as usize].inc();
    }
}
