// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Management-surface producer.
//!
//! Spec sources:
//! * dds-amqp-1.0 §7.5 Discovery Bridging — the `$catalog` address
//!   provides the topic-mapping entries.
//! * §7.9.1 Catalog Address — format of the entries.
//! * §7.9.2 Metrics Address — `$metrics` sample producer
//!   (reads from [`MetricsHub`]).
//! * §sec:audit-channel — `$audit` event stream.
//!
//! This module provides the in-process producer layer that
//! generates AMQP sample bodies (map bodies per the spec tables).
//! Wiring to receiver links lives in the daemon layer.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::mapping::BodyEncodingMode;
use crate::metrics::{MANDATORY_METRIC_NAMES, MetricsHub};
use crate::routing::AddressResolution;

/// Spec §7.9.1 — reserved-address constants.
pub mod addresses {
    /// `$catalog` receiver address.
    pub const CATALOG: &str = "$catalog";
    /// `$metrics` receiver address.
    pub const METRICS: &str = "$metrics";
    /// `$audit` receiver address (Spec §sec:audit-channel).
    pub const AUDIT: &str = "$audit";
}

// ============================================================
// Catalog (§7.5 + §7.9.1)
// ============================================================

/// Spec §7.5 — forward direction of the topic mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogDirection {
    /// AMQP producer writes to DDS.
    ProducerToDds,
    /// DDS writes to AMQP consumer.
    DdsToConsumer,
    /// Bidirectional.
    Both,
}

impl CatalogDirection {
    /// AMQP symbol form per Spec §7.5.
    #[must_use]
    pub const fn as_symbol(self) -> &'static str {
        match self {
            Self::ProducerToDds => "producer-to-dds",
            Self::DdsToConsumer => "dds-to-consumer",
            Self::Both => "both",
        }
    }
}

/// Spec §7.5 — form of the TypeIdentifier field (`DESC_FULL` →
/// AMQP `symbol`, `DESC_TRUNCATED` → AMQP `ulong` 8B BE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogTypeId {
    /// `DESC_FULL` (Spec §7.2.1.2): full 14-byte symbol string.
    Symbolic(String),
    /// `DESC_TRUNCATED` (Spec §7.2.1.1): 8-byte BE ulong.
    Truncated(u64),
}

impl CatalogTypeId {
    fn into_amqp(self) -> AmqpExtValue {
        match self {
            Self::Symbolic(s) => AmqpExtValue::Symbol(s),
            Self::Truncated(u) => AmqpExtValue::Ulong(u),
        }
    }
}

/// Spec §7.5 — catalog entry.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// AMQP address (e.g. `domain://0/Sensor` or an alias).
    pub amqp_address: String,
    /// DDS topic name + domain + partitions.
    pub dds: AddressResolution,
    /// DDS type name (e.g. `org::ros2::Pose`).
    pub dds_type_name: String,
    /// TypeIdentifier (Spec §7.2.1).
    pub type_id: CatalogTypeId,
    /// Reachable directions.
    pub direction: CatalogDirection,
}

/// Catalog producer.
///
/// Holds the current set of topic mappings; produces one
/// AMQP `map` body per entry, which a receiver on
/// `$catalog` receives as a sample.
#[derive(Debug, Default)]
pub struct CatalogProducer {
    entries: Vec<CatalogEntry>,
}

impl CatalogProducer {
    /// Fresh empty producer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry and increment the `topics.exposed` counter.
    pub fn add(&mut self, entry: CatalogEntry, metrics: &MetricsHub) {
        self.entries.push(entry);
        metrics.on_topic_added();
    }

    /// Remove an entry by `amqp_address` (lower the gauge).
    /// Returns `true` if removed.
    pub fn remove(&mut self, amqp_address: &str, metrics: &MetricsHub) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.amqp_address != amqp_address);
        if self.entries.len() < before {
            metrics.on_topic_removed();
            true
        } else {
            false
        }
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Spec §7.5 — all entries as AMQP sample bodies for the
    /// `$catalog` receiver.
    ///
    /// One `AmqpExtValue::Map` is produced per entry with the
    /// keys from the spec's itemized list.
    #[must_use]
    pub fn snapshot(&self) -> Vec<AmqpExtValue> {
        self.entries
            .iter()
            .cloned()
            .map(catalog_entry_to_map)
            .collect()
    }
}

fn catalog_entry_to_map(e: CatalogEntry) -> AmqpExtValue {
    let mut entries: Vec<(AmqpExtValue, AmqpExtValue)> = alloc::vec![
        (
            AmqpExtValue::Symbol("amqp-address".to_string()),
            AmqpExtValue::Str(e.amqp_address),
        ),
        (
            AmqpExtValue::Symbol("dds-topic".to_string()),
            AmqpExtValue::Str(e.dds.topic),
        ),
        (
            AmqpExtValue::Symbol("dds-type-name".to_string()),
            AmqpExtValue::Str(e.dds_type_name),
        ),
        (
            AmqpExtValue::Symbol("type-id".to_string()),
            e.type_id.into_amqp(),
        ),
        (
            AmqpExtValue::Symbol("direction".to_string()),
            AmqpExtValue::Symbol(e.direction.as_symbol().to_string()),
        ),
    ];
    if !e.dds.partitions.is_empty() {
        let parts: Vec<AmqpExtValue> = e
            .dds
            .partitions
            .iter()
            .map(|p| AmqpExtValue::Str(p.clone()))
            .collect();
        entries.push((
            AmqpExtValue::Symbol("partitions".to_string()),
            AmqpExtValue::List(parts),
        ));
    }
    AmqpExtValue::Map(entries)
}

// ============================================================
// Metrics ($metrics)
// ============================================================

/// Spec §7.9.2 — snapshot of all mandatory metrics as
/// AMQP sample bodies (one message per metric with a
/// `{name, value, unit, timestamp}` map).
///
/// `now_ms` is the caller clock (Unix ms since epoch); it is
/// embedded into each sample as `timestamp`.
#[must_use]
pub fn metrics_snapshot(hub: &MetricsHub, now_ms: i64) -> Vec<AmqpExtValue> {
    MANDATORY_METRIC_NAMES
        .iter()
        .filter_map(|name| {
            let value = hub.snapshot(name)?;
            let unit = MetricsHub::unit_of(name)?;
            Some(metric_sample(name, value, unit, now_ms))
        })
        .collect()
}

fn metric_sample(name: &str, value: i64, unit: &str, ts_ms: i64) -> AmqpExtValue {
    let entries: Vec<(AmqpExtValue, AmqpExtValue)> = alloc::vec![
        (
            AmqpExtValue::Str("name".to_string()),
            AmqpExtValue::Str(name.to_string()),
        ),
        (
            AmqpExtValue::Str("value".to_string()),
            AmqpExtValue::Long(value),
        ),
        (
            AmqpExtValue::Str("unit".to_string()),
            AmqpExtValue::Symbol(unit.to_string()),
        ),
        (
            AmqpExtValue::Str("timestamp".to_string()),
            AmqpExtValue::Timestamp(ts_ms),
        ),
    ];
    AmqpExtValue::Map(entries)
}

// ============================================================
// Audit ($audit)
// ============================================================

/// Spec §sec:audit-channel — event types.
#[derive(Debug, Clone)]
pub enum AuditEvent {
    /// Connection accepted successfully.
    ConnectionOpened {
        /// Authenticated subject (e.g. PLAIN username or
        /// EXTERNAL cert subject).
        subject: String,
        /// Remote address (best-effort string, e.g. `1.2.3.4:5672`).
        remote: String,
    },
    /// Connection closed.
    ConnectionClosed {
        /// Authenticated subject (see `ConnectionOpened`).
        subject: String,
        /// Reason (close performative `error.condition` or
        /// `tcp-reset`).
        reason: String,
    },
    /// SASL negotiation succeeded.
    SaslSuccess {
        /// Authenticated subject.
        subject: String,
        /// Mechanism (`PLAIN`/`ANONYMOUS`/`EXTERNAL`/`SCRAM-SHA-256`).
        mechanism: String,
    },
    /// SASL negotiation failed.
    SaslFailure {
        /// Reason (`auth`/`sys`/`sys-perm`/...).
        reason: String,
    },
    /// AccessControl plugin reject.
    Unauthorized {
        /// Authenticated subject.
        subject: String,
        /// Resource address (topic / address).
        resource: String,
    },
    /// Link attach succeeded.
    LinkAttached {
        /// Authenticated subject.
        subject: String,
        /// Link name (Spec §2.6.1).
        link: String,
        /// AMQP address of the terminus.
        address: String,
    },
}

impl AuditEvent {
    /// Spec symbol for the `event-type` field.
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::ConnectionOpened { .. } => "connection.opened",
            Self::ConnectionClosed { .. } => "connection.closed",
            Self::SaslSuccess { .. } => "sasl.success",
            Self::SaslFailure { .. } => "sasl.failure",
            Self::Unauthorized { .. } => "access.unauthorized",
            Self::LinkAttached { .. } => "link.attach.success",
        }
    }

    fn into_map_entries(self, ts_ms: i64) -> Vec<(AmqpExtValue, AmqpExtValue)> {
        let event_type = self.event_type().to_string();
        let mut e: Vec<(AmqpExtValue, AmqpExtValue)> = Vec::new();
        e.push((
            AmqpExtValue::Str("event-type".to_string()),
            AmqpExtValue::Symbol(event_type),
        ));
        e.push((
            AmqpExtValue::Str("timestamp".to_string()),
            AmqpExtValue::Timestamp(ts_ms),
        ));
        match self {
            Self::ConnectionOpened { subject, remote } => {
                e.push((
                    AmqpExtValue::Str("subject".to_string()),
                    AmqpExtValue::Str(subject),
                ));
                e.push((
                    AmqpExtValue::Str("remote".to_string()),
                    AmqpExtValue::Str(remote),
                ));
            }
            Self::ConnectionClosed { subject, reason } => {
                e.push((
                    AmqpExtValue::Str("subject".to_string()),
                    AmqpExtValue::Str(subject),
                ));
                e.push((
                    AmqpExtValue::Str("reason".to_string()),
                    AmqpExtValue::Str(reason),
                ));
            }
            Self::SaslSuccess { subject, mechanism } => {
                e.push((
                    AmqpExtValue::Str("subject".to_string()),
                    AmqpExtValue::Str(subject),
                ));
                e.push((
                    AmqpExtValue::Str("mechanism".to_string()),
                    AmqpExtValue::Symbol(mechanism),
                ));
            }
            Self::SaslFailure { reason } => {
                e.push((
                    AmqpExtValue::Str("reason".to_string()),
                    AmqpExtValue::Str(reason),
                ));
            }
            Self::Unauthorized { subject, resource } => {
                e.push((
                    AmqpExtValue::Str("subject".to_string()),
                    AmqpExtValue::Str(subject),
                ));
                e.push((
                    AmqpExtValue::Str("resource".to_string()),
                    AmqpExtValue::Str(resource),
                ));
            }
            Self::LinkAttached {
                subject,
                link,
                address,
            } => {
                e.push((
                    AmqpExtValue::Str("subject".to_string()),
                    AmqpExtValue::Str(subject),
                ));
                e.push((
                    AmqpExtValue::Str("link".to_string()),
                    AmqpExtValue::Str(link),
                ));
                e.push((
                    AmqpExtValue::Str("address".to_string()),
                    AmqpExtValue::Str(address),
                ));
            }
        }
        e
    }
}

/// Spec §sec:audit-channel — audit event as an AMQP sample body.
///
/// Map form with `event-type` (symbol), `timestamp` (timestamp)
/// and event-specific fields. The caller can stream the list to a
/// receiver on `$audit`.
#[must_use]
pub fn audit_event_sample(event: AuditEvent, now_ms: i64) -> AmqpExtValue {
    AmqpExtValue::Map(event.into_map_entries(now_ms))
}

/// Audit producer with a ring-buffered queue.
///
/// The producer holds a FIFO of the last `cap` events;
/// AMQP receiver links read events out-of-band (via `pop()` or
/// `drain_into_samples`). Spec §sec:audit-channel does not require
/// a persistent audit trail.
#[derive(Debug)]
pub struct AuditProducer {
    cap: usize,
    queue: alloc::collections::VecDeque<(AuditEvent, i64)>,
}

impl AuditProducer {
    /// Capacity-bounded audit queue.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            queue: alloc::collections::VecDeque::with_capacity(cap),
        }
    }

    /// Record an event; when the queue is full the oldest event is
    /// evicted (ring buffer).
    pub fn push(&mut self, event: AuditEvent, ts_ms: i64) {
        if self.queue.len() == self.cap {
            self.queue.pop_front();
        }
        self.queue.push_back((event, ts_ms));
    }

    /// Pull pending events as a sample list (FIFO).
    pub fn drain_samples(&mut self) -> Vec<AmqpExtValue> {
        let mut out = Vec::with_capacity(self.queue.len());
        while let Some((event, ts)) = self.queue.pop_front() {
            out.push(audit_event_sample(event, ts));
        }
        out
    }

    /// Current queue length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Queue empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ============================================================
// Address-Recognition (Caller-Helper)
// ============================================================

/// Classifies an AMQP address as `$catalog`/`$metrics`/
/// `$audit` or `Topic` (everything else). The caller dispatches
/// accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    /// Receiver link on `$catalog`.
    Catalog,
    /// Receiver link on `$metrics`.
    Metrics,
    /// Receiver link on `$audit`.
    Audit,
    /// User topic.
    Topic,
}

/// Address classification.
#[must_use]
pub fn classify_address(address: &str) -> AddressKind {
    match address {
        addresses::CATALOG => AddressKind::Catalog,
        addresses::METRICS => AddressKind::Metrics,
        addresses::AUDIT => AddressKind::Audit,
        _ => AddressKind::Topic,
    }
}

// Doc tie to the §7.9.2 body-mode expectation — plainly: the producer
// is wire-format agnostic.
const _: BodyEncodingMode = BodyEncodingMode::PassThrough;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::routing::AddressResolution;

    fn entry(addr: &str, topic: &str) -> CatalogEntry {
        CatalogEntry {
            amqp_address: addr.to_string(),
            dds: AddressResolution {
                topic: topic.to_string(),
                domain_id: 0,
                partitions: Vec::new(),
            },
            dds_type_name: "Foo".to_string(),
            type_id: CatalogTypeId::Truncated(0xDEAD_BEEF_CAFE_BABE),
            direction: CatalogDirection::Both,
        }
    }

    // --- Address classification ---

    #[test]
    fn classify_address_recognises_reserved() {
        assert_eq!(classify_address("$catalog"), AddressKind::Catalog);
        assert_eq!(classify_address("$metrics"), AddressKind::Metrics);
        assert_eq!(classify_address("$audit"), AddressKind::Audit);
        assert_eq!(classify_address("MyTopic"), AddressKind::Topic);
    }

    // --- Catalog ---

    #[test]
    fn catalog_add_remove_balances_topics_exposed() {
        let metrics = MetricsHub::new();
        let mut cat = CatalogProducer::new();
        cat.add(entry("$T1", "T1"), &metrics);
        cat.add(entry("$T2", "T2"), &metrics);
        assert_eq!(cat.len(), 2);
        assert_eq!(metrics.snapshot("topics.exposed"), Some(2));
        assert!(cat.remove("$T1", &metrics));
        assert_eq!(cat.len(), 1);
        assert_eq!(metrics.snapshot("topics.exposed"), Some(1));
        assert!(!cat.remove("$NOPE", &metrics));
    }

    #[test]
    fn catalog_snapshot_emits_map_per_entry() {
        let metrics = MetricsHub::new();
        let mut cat = CatalogProducer::new();
        cat.add(entry("AddrA", "TopicA"), &metrics);
        cat.add(entry("AddrB", "TopicB"), &metrics);
        let s = cat.snapshot();
        assert_eq!(s.len(), 2);
        for body in s {
            let entries = match body {
                AmqpExtValue::Map(v) => v,
                other => panic!("expected map, got {other:?}"),
            };
            // Required keys present.
            let keys: Vec<String> = entries
                .iter()
                .map(|(k, _)| match k {
                    AmqpExtValue::Symbol(s) => s.clone(),
                    _ => panic!(),
                })
                .collect();
            assert!(keys.contains(&"amqp-address".to_string()));
            assert!(keys.contains(&"dds-topic".to_string()));
            assert!(keys.contains(&"dds-type-name".to_string()));
            assert!(keys.contains(&"type-id".to_string()));
            assert!(keys.contains(&"direction".to_string()));
        }
    }

    #[test]
    fn catalog_type_id_truncated_is_amqp_ulong() {
        let metrics = MetricsHub::new();
        let mut cat = CatalogProducer::new();
        cat.add(entry("X", "T"), &metrics);
        let s = cat.snapshot();
        let entries = match &s[0] {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let tid = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Symbol(s) if s == "type-id"))
            .map(|(_, v)| v.clone())
            .unwrap();
        match tid {
            AmqpExtValue::Ulong(u) => assert_eq!(u, 0xDEAD_BEEF_CAFE_BABE),
            other => panic!("expected ulong, got {other:?}"),
        }
    }

    #[test]
    fn catalog_type_id_full_is_amqp_symbol() {
        let metrics = MetricsHub::new();
        let mut cat = CatalogProducer::new();
        let mut e = entry("X", "T");
        e.type_id = CatalogTypeId::Symbolic("dds:type:abcdef0123456789abcdef".to_string());
        cat.add(e, &metrics);
        let s = cat.snapshot();
        let entries = match &s[0] {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let tid = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Symbol(s) if s == "type-id"))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert!(matches!(tid, AmqpExtValue::Symbol(_)));
    }

    #[test]
    fn catalog_partitions_emitted_when_set() {
        let metrics = MetricsHub::new();
        let mut cat = CatalogProducer::new();
        let mut e = entry("X", "T");
        e.dds.partitions = alloc::vec!["alpha".into(), "beta".into()];
        cat.add(e, &metrics);
        let s = cat.snapshot();
        let entries = match &s[0] {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let parts = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Symbol(s) if s == "partitions"))
            .map(|(_, v)| v.clone())
            .unwrap();
        match parts {
            AmqpExtValue::List(items) => assert_eq!(items.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn catalog_direction_symbols_match_spec() {
        assert_eq!(
            CatalogDirection::ProducerToDds.as_symbol(),
            "producer-to-dds"
        );
        assert_eq!(
            CatalogDirection::DdsToConsumer.as_symbol(),
            "dds-to-consumer"
        );
        assert_eq!(CatalogDirection::Both.as_symbol(), "both");
    }

    // --- Metrics ---

    #[test]
    fn metrics_snapshot_emits_one_sample_per_mandatory_metric() {
        let hub = MetricsHub::new();
        hub.on_connection_open();
        hub.on_dropped_loop();
        let s = metrics_snapshot(&hub, 1_700_000_000_000);
        assert_eq!(s.len(), MANDATORY_METRIC_NAMES.len());
        for sample in s {
            let entries = match sample {
                AmqpExtValue::Map(v) => v,
                _ => panic!(),
            };
            let keys: Vec<String> = entries
                .iter()
                .map(|(k, _)| match k {
                    AmqpExtValue::Str(s) => s.clone(),
                    _ => panic!(),
                })
                .collect();
            for required in ["name", "value", "unit", "timestamp"] {
                assert!(keys.contains(&required.to_string()));
            }
        }
    }

    #[test]
    fn metrics_snapshot_carries_value() {
        let hub = MetricsHub::new();
        hub.on_connection_open();
        hub.on_connection_open();
        let s = metrics_snapshot(&hub, 0);
        let connections_active = s.iter().find_map(|m| {
            if let AmqpExtValue::Map(entries) = m {
                let name = entries
                    .iter()
                    .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "name"))
                    .map(|(_, v)| v.clone())?;
                if name == AmqpExtValue::Str("connections.active".to_string()) {
                    return entries
                        .iter()
                        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "value"))
                        .map(|(_, v)| v.clone());
                }
            }
            None
        });
        assert_eq!(connections_active, Some(AmqpExtValue::Long(2)));
    }

    // --- Audit ---

    #[test]
    fn audit_event_types_are_spec_symbols() {
        // Spec §sec:audit-channel + §C.1.14.
        assert_eq!(
            AuditEvent::ConnectionOpened {
                subject: "x".into(),
                remote: "y".into()
            }
            .event_type(),
            "connection.opened"
        );
        assert_eq!(
            AuditEvent::LinkAttached {
                subject: "s".into(),
                link: "L".into(),
                address: "A".into()
            }
            .event_type(),
            "link.attach.success"
        );
        assert_eq!(
            AuditEvent::Unauthorized {
                subject: "s".into(),
                resource: "r".into()
            }
            .event_type(),
            "access.unauthorized"
        );
    }

    #[test]
    fn audit_sample_carries_subject_and_link() {
        // §C.1.14 requires subject_name in the audit record.
        let s = audit_event_sample(
            AuditEvent::LinkAttached {
                subject: "alice".into(),
                link: "L1".into(),
                address: "Sensor".into(),
            },
            1_000,
        );
        let entries = match s {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let subject = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "subject"))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(subject, AmqpExtValue::Str("alice".into()));
        let link = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "link"))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(link, AmqpExtValue::Str("L1".into()));
    }

    #[test]
    fn audit_producer_ringbuffer_evicts_oldest() {
        let mut p = AuditProducer::new(2);
        p.push(
            AuditEvent::ConnectionOpened {
                subject: "a".into(),
                remote: "x".into(),
            },
            1,
        );
        p.push(
            AuditEvent::ConnectionOpened {
                subject: "b".into(),
                remote: "y".into(),
            },
            2,
        );
        p.push(
            AuditEvent::ConnectionOpened {
                subject: "c".into(),
                remote: "z".into(),
            },
            3,
        );
        assert_eq!(p.len(), 2);
        let s = p.drain_samples();
        // The oldest (subject=a) was evicted; b then c.
        assert_eq!(s.len(), 2);
        // Verify that subject 'a' is no longer present.
        let any_a = s.iter().any(|m| {
            if let AmqpExtValue::Map(entries) = m {
                entries
                    .iter()
                    .any(|(_, v)| matches!(v, AmqpExtValue::Str(s) if s == "a"))
            } else {
                false
            }
        });
        assert!(!any_a);
        assert!(p.is_empty());
    }

    #[test]
    fn audit_producer_drain_empties_queue() {
        let mut p = AuditProducer::new(8);
        p.push(
            AuditEvent::SaslFailure {
                reason: "auth".into(),
            },
            1,
        );
        let s = p.drain_samples();
        assert_eq!(s.len(), 1);
        assert!(p.is_empty());
    }
}
