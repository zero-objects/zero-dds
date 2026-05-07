// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Management-Surface Producer.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 §7.5 Discovery Bridging — `$catalog`-Address
//!   liefert die Topic-Mapping-Eintraege.
//! * §7.9.1 Catalog Address — Format der Eintraege.
//! * §7.9.2 Metrics Address — `$metrics`-Sample-Producer
//!   (liest aus [`MetricsHub`]).
//! * §sec:audit-channel — `$audit`-Event-Stream.
//!
//! Dieses Modul liefert die in-process Producer-Schicht, die
//! AMQP-Sample-Bodies (Map-Bodies gemaess Spec-Tabellen)
//! erzeugt. Die Wire-Anbindung an Receiver-Links liegt im
//! Daemon-Layer.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::mapping::BodyEncodingMode;
use crate::metrics::{MANDATORY_METRIC_NAMES, MetricsHub};
use crate::routing::AddressResolution;

/// Spec §7.9.1 — Reserved-Address-Konstanten.
pub mod addresses {
    /// `$catalog` Receiver-Address.
    pub const CATALOG: &str = "$catalog";
    /// `$metrics` Receiver-Address.
    pub const METRICS: &str = "$metrics";
    /// `$audit` Receiver-Address (Spec §sec:audit-channel).
    pub const AUDIT: &str = "$audit";
}

// ============================================================
// Catalog (§7.5 + §7.9.1)
// ============================================================

/// Spec §7.5 — Forward-Direction des Topic-Mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogDirection {
    /// AMQP-Producer schreibt nach DDS.
    ProducerToDds,
    /// DDS schreibt zu AMQP-Consumer.
    DdsToConsumer,
    /// Bidirektional.
    Both,
}

impl CatalogDirection {
    /// AMQP-symbol-form per Spec §7.5.
    #[must_use]
    pub const fn as_symbol(self) -> &'static str {
        match self {
            Self::ProducerToDds => "producer-to-dds",
            Self::DdsToConsumer => "dds-to-consumer",
            Self::Both => "both",
        }
    }
}

/// Spec §7.5 — Form des TypeIdentifier-Feldes (`DESC_FULL` →
/// AMQP `symbol`, `DESC_TRUNCATED` → AMQP `ulong` 8B BE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogTypeId {
    /// `DESC_FULL` (Spec §7.2.1.2): voller 14-Byte-Symbol-String.
    Symbolic(String),
    /// `DESC_TRUNCATED` (Spec §7.2.1.1): 8-Byte-BE-ulong.
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

/// Spec §7.5 — Catalog-Eintrag.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// AMQP-Address (z.B. `domain://0/Sensor` oder Alias).
    pub amqp_address: String,
    /// DDS-Topic-Name + Domain + Partitionen.
    pub dds: AddressResolution,
    /// DDS-Type-Name (z.B. `org::ros2::Pose`).
    pub dds_type_name: String,
    /// TypeIdentifier (Spec §7.2.1).
    pub type_id: CatalogTypeId,
    /// Erreichbare Richtungen.
    pub direction: CatalogDirection,
}

/// Catalog-Producer.
///
/// Haelt den aktuellen Set von Topic-Mappings; produziert pro
/// Eintrag ein AMQP-`map`-Body, das ein Receiver auf
/// `$catalog` als Sample empfaengt.
#[derive(Debug, Default)]
pub struct CatalogProducer {
    entries: Vec<CatalogEntry>,
}

impl CatalogProducer {
    /// Frischer leerer Producer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Eintrag hinzufuegen + `topics.exposed`-Counter inkrementieren.
    pub fn add(&mut self, entry: CatalogEntry, metrics: &MetricsHub) {
        self.entries.push(entry);
        metrics.on_topic_added();
    }

    /// Eintrag per `amqp_address` entfernen (gauge senken).
    /// Liefert `true` wenn entfernt.
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

    /// Anzahl Eintraege.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Leer?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Spec §7.5 — alle Eintraege als AMQP-Sample-Bodies fuer
    /// `$catalog`-Receiver.
    ///
    /// Pro Eintrag wird ein `AmqpExtValue::Map` erzeugt mit den
    /// Schluesseln aus der Spec-Itemize-Liste.
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

/// Spec §7.9.2 — Snapshot aller Mandatory-Metriken als
/// AMQP-Sample-Bodies (eine Message pro Metric mit
/// `{name, value, unit, timestamp}`-Map).
///
/// `now_ms` ist die Caller-Clock (Unix-ms-since-epoch); wird
/// in jedes Sample als `timestamp` eingebettet.
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

/// Spec §sec:audit-channel — Event-Typen.
#[derive(Debug, Clone)]
pub enum AuditEvent {
    /// Connection erfolgreich akzeptiert.
    ConnectionOpened {
        /// Authentifiziertes Subject (z.B. PLAIN-Username oder
        /// EXTERNAL-Cert-Subject).
        subject: String,
        /// Remote-Address (best-effort String, z.B. `1.2.3.4:5672`).
        remote: String,
    },
    /// Connection geschlossen.
    ConnectionClosed {
        /// Authentifiziertes Subject (siehe `ConnectionOpened`).
        subject: String,
        /// Begruendung (close-Performative `error.condition` oder
        /// `tcp-reset`).
        reason: String,
    },
    /// SASL-Negotiation erfolgreich.
    SaslSuccess {
        /// Authentifiziertes Subject.
        subject: String,
        /// Mechanismus (`PLAIN`/`ANONYMOUS`/`EXTERNAL`/`SCRAM-SHA-256`).
        mechanism: String,
    },
    /// SASL-Negotiation fehlgeschlagen.
    SaslFailure {
        /// Begruendung (`auth`/`sys`/`sys-perm`/...).
        reason: String,
    },
    /// AccessControl-Plugin reject.
    Unauthorized {
        /// Authentifiziertes Subject.
        subject: String,
        /// Resource-Address (Topic / Address).
        resource: String,
    },
    /// Link-Attach erfolgreich.
    LinkAttached {
        /// Authentifiziertes Subject.
        subject: String,
        /// Link-Name (Spec §2.6.1).
        link: String,
        /// AMQP-Address des Terminus.
        address: String,
    },
}

impl AuditEvent {
    /// Spec-Symbol fuer das `event-type`-Feld.
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

/// Spec §sec:audit-channel — Audit-Event als AMQP-Sample-Body.
///
/// Map-Form mit `event-type` (symbol), `timestamp` (timestamp)
/// und event-spezifischen Feldern. Caller kann die Liste an einen
/// Receiver auf `$audit` streamen.
#[must_use]
pub fn audit_event_sample(event: AuditEvent, now_ms: i64) -> AmqpExtValue {
    AmqpExtValue::Map(event.into_map_entries(now_ms))
}

/// Audit-Producer mit ringbuffered Queue.
///
/// Der Producer haelt einen FIFO der letzten `cap` Events;
/// AMQP-Receiver-Links lesen Events out-of-band (per `pop()` oder
/// `drain_into_samples`). Spec §sec:audit-channel verlangt
/// keinen persistenten Audit-Trail.
#[derive(Debug)]
pub struct AuditProducer {
    cap: usize,
    queue: alloc::collections::VecDeque<(AuditEvent, i64)>,
}

impl AuditProducer {
    /// Capacity-bounded Audit-Queue.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            queue: alloc::collections::VecDeque::with_capacity(cap),
        }
    }

    /// Event aufnehmen; bei voller Queue wird das aelteste Event
    /// verdraengt (ringbuffer).
    pub fn push(&mut self, event: AuditEvent, ts_ms: i64) {
        if self.queue.len() == self.cap {
            self.queue.pop_front();
        }
        self.queue.push_back((event, ts_ms));
    }

    /// Pendente Events als Sample-Liste herausziehen (FIFO).
    pub fn drain_samples(&mut self) -> Vec<AmqpExtValue> {
        let mut out = Vec::with_capacity(self.queue.len());
        while let Some((event, ts)) = self.queue.pop_front() {
            out.push(audit_event_sample(event, ts));
        }
        out
    }

    /// Aktuelle Queue-Laenge.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Queue leer?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ============================================================
// Address-Recognition (Caller-Helper)
// ============================================================

/// Klassifiziert eine AMQP-Address als `$catalog`/`$metrics`/
/// `$audit` oder `Topic` (alles andere). Caller dispatcht
/// entsprechend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    /// Receiver-Link auf `$catalog`.
    Catalog,
    /// Receiver-Link auf `$metrics`.
    Metrics,
    /// Receiver-Link auf `$audit`.
    Audit,
    /// Anwender-Topic.
    Topic,
}

/// Adressklassifizierung.
#[must_use]
pub fn classify_address(address: &str) -> AddressKind {
    match address {
        addresses::CATALOG => AddressKind::Catalog,
        addresses::METRICS => AddressKind::Metrics,
        addresses::AUDIT => AddressKind::Audit,
        _ => AddressKind::Topic,
    }
}

// Doc-tie zur §7.9.2-Body-Mode-Erwartung — nuechtern: Producer
// ist Wire-Format-agnostic.
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

    // --- Address-Klassifizierung ---

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
            // Pflicht-Schluessel enthalten.
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
        // §C.1.14 verlangt subject_name im audit-record.
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
        // Aelteste (subject=a) wurde verdraengt; b dann c.
        assert_eq!(s.len(), 2);
        // Verifiziere dass subject 'a' nicht mehr da ist.
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
