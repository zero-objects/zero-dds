// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP↔DDS Bridge-Logic.
//!
//! Spec dds-amqp-1.0:
//! * §2.1 Cl. 3 — Sender-/Receiver-Link translaten zu
//!   DDS-DataWriter/-Reader-Operations.
//! * §2.2 Cl. 2/3 — Bridge-Profile analog auf der Outbound-Seite.
//! * §7.5 Discovery Bridging — `$catalog`-Address-Stream.
//! * §7.7.1/7.7.2 — Instance-Lifecycle-Mapping.
//!
//! Diese Modul-Datei enthaelt die reine Bridge-Translation-Logik
//! (Attach/Transfer/Catalog), die der Connection-Handler aufruft.
//! Die Verbindung zur konkreten DDS-Wire-Schicht erfolgt ueber
//! [`crate::dds_host::DdsHost`].

use std::sync::Arc;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::performatives;
use zerodds_amqp_endpoint::management::{AddressKind, classify_address};
use zerodds_amqp_endpoint::metrics::MetricsHub;

use crate::dds_host::{DdsHost, DdsHostError, SubscriptionId, TopicId};
use crate::handler::HandlerError;

/// Spec §2.1 Cl. 3 — Resultat einer Inbound-Attach-Verarbeitung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachOutcome {
    /// Attach an einen DDS-Topic-Address akzeptiert. `topic_id`
    /// ist der DdsHost-Identifier; Caller muss ihn fuer
    /// Transfer-Dispatch merken.
    AttachedTopic {
        /// DdsHost-Topic-Id.
        topic_id: TopicId,
    },
    /// Attach an `$catalog` akzeptiert. Caller muss die
    /// Catalog-Entries als Outbound-Transfers schicken.
    AttachedCatalog,
    /// Attach an `$metrics` akzeptiert.
    AttachedMetrics,
    /// Attach an `$audit` akzeptiert.
    AttachedAudit,
    /// Spec §7.5.1 + §11.2 — Address nicht im Catalog und
    /// `permit_dynamic_topics = false` → Caller emittiert
    /// `amqp:not-found`.
    UnknownAddress,
}

/// Spec §2.1 Cl. 3 — Inbound Attach gegen DdsHost dispatchen.
///
/// `target_address` ist das `target.address`-Feld aus dem
/// Attach-Performative (Receiver-Side) bzw. `source.address`
/// (Sender-Side).
///
/// # Errors
/// `HandlerError::PerformativeDecode` wenn DdsHost-Lookup
/// schief geht (sollte nicht passieren mit InMemoryDdsHost).
pub fn dispatch_attach<H: DdsHost + ?Sized>(host: &H, target_address: &str) -> AttachOutcome {
    match classify_address(target_address) {
        AddressKind::Catalog => AttachOutcome::AttachedCatalog,
        AddressKind::Metrics => AttachOutcome::AttachedMetrics,
        AddressKind::Audit => AttachOutcome::AttachedAudit,
        AddressKind::Topic => match host.lookup(target_address) {
            Some(id) => AttachOutcome::AttachedTopic { topic_id: id },
            None => AttachOutcome::UnknownAddress,
        },
    }
}

/// Spec §2.1 Cl. 3 — Inbound Transfer auf DDS-Side publishen.
///
/// `body` ist der AMQP-Body (XCDR2/JSON/AMQP-Native je nach
/// Topic-Mode).
///
/// # Errors
/// `DdsHostError` aus dem Host (`UnknownTopic`).
pub fn dispatch_transfer<H: DdsHost + ?Sized>(
    host: &H,
    topic_id: TopicId,
    body: &[u8],
    metrics: &Arc<MetricsHub>,
) -> Result<(), DdsHostError> {
    metrics.on_transfer_received();
    host.publish_to_dds(topic_id, body)
}

/// Spec §7.5 — `$catalog`-Stream produzieren.
///
/// Liefert pro registriertem Topic eine AMQP-Map als Sample-Body,
/// die der Caller als Outbound-Transfer verschicken soll.
#[must_use]
pub fn produce_catalog_transfers<H: DdsHost + ?Sized>(host: &H) -> Vec<AmqpExtValue> {
    let entries = host.topics();
    entries.into_iter().map(catalog_entry_to_map).collect()
}

/// Spec §7.5 — Catalog-Entry als AMQP-Map serialisieren.
fn catalog_entry_to_map(e: zerodds_amqp_endpoint::management::CatalogEntry) -> AmqpExtValue {
    use zerodds_amqp_endpoint::management::CatalogTypeId;
    let mut entries: Vec<(AmqpExtValue, AmqpExtValue)> = vec![
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
            match e.type_id {
                CatalogTypeId::Symbolic(s) => AmqpExtValue::Symbol(s),
                CatalogTypeId::Truncated(u) => AmqpExtValue::Ulong(u),
            },
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
            .into_iter()
            .map(AmqpExtValue::Str)
            .collect();
        entries.push((
            AmqpExtValue::Symbol("partitions".to_string()),
            AmqpExtValue::List(parts),
        ));
    }
    AmqpExtValue::Map(entries)
}

/// Spec §2.1 Cl. 3 + §7.5 — Spec-konformer Wire-Body fuer ein
/// `$catalog`-Sample (described composite per Performatives-
/// Konvention).
///
/// # Errors
/// `HandlerError::PerformativeDecode` wenn Encode fehlschlaegt.
pub fn encode_catalog_sample(entry: AmqpExtValue) -> Result<Vec<u8>, HandlerError> {
    // Wir nutzen einen ad-hoc-Descriptor fuer Catalog-Samples
    // (nicht spec-normativ; Spec §7.5 laesst das Format offen,
    // beschreibt aber Map-Form). Wir wraappen in einen
    // amqp-value-Section-Performative-Header, sodass der Body
    // unter §3.2.8-Wire-Form direkt empfangbar ist.
    let amqp_value_descriptor: u64 = 0x0000_0000_0000_0077;
    performatives::encode_performative(amqp_value_descriptor, &entry)
        .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))
}

/// Spec §2.1 Cl. 3 — Subscribe-Helper: registriert einen Outbound-
/// Listener auf einem Topic.
///
/// Liefert `SubscriptionId` zur spaeteren Aufhebung.
///
/// # Errors
/// `DdsHostError::UnknownTopic`.
pub fn subscribe_outbound<H: DdsHost + ?Sized>(
    host: &H,
    topic_id: TopicId,
    callback: crate::dds_host::SampleCallback,
) -> Result<SubscriptionId, DdsHostError> {
    host.subscribe_amqp_to_dds(topic_id, callback)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dds_host::InMemoryDdsHost;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zerodds_amqp_endpoint::annex_a::TopicMapping;
    use zerodds_amqp_endpoint::management::addresses;
    use zerodds_amqp_endpoint::metrics::MetricsHub;

    fn host_with(topics: &[(&str, &str)]) -> InMemoryDdsHost {
        let h = InMemoryDdsHost::new();
        for (addr, dds_topic) in topics {
            let mut m = TopicMapping {
                amqp_address: (*addr).into(),
                dds_topic: (*dds_topic).into(),
                dds_type_name: "T".into(),
                ..TopicMapping::default()
            };
            m.dds_type_name = "T".into();
            h.register_topic(m).unwrap();
        }
        h
    }

    // --- Attach-Dispatch ---

    #[test]
    fn dispatch_attach_to_known_topic_returns_topic_id() {
        let h = host_with(&[("Sensor", "SensorTopic")]);
        let outcome = dispatch_attach(&h, "Sensor");
        match outcome {
            AttachOutcome::AttachedTopic { topic_id } => assert!(topic_id > 0),
            other => panic!("expected AttachedTopic, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_attach_to_unknown_topic_yields_unknown_address() {
        let h = host_with(&[("Sensor", "SensorTopic")]);
        let outcome = dispatch_attach(&h, "NonExistent");
        assert_eq!(outcome, AttachOutcome::UnknownAddress);
    }

    #[test]
    fn dispatch_attach_to_catalog_recognised() {
        let h = host_with(&[]);
        assert_eq!(
            dispatch_attach(&h, addresses::CATALOG),
            AttachOutcome::AttachedCatalog
        );
    }

    #[test]
    fn dispatch_attach_to_metrics_recognised() {
        let h = host_with(&[]);
        assert_eq!(
            dispatch_attach(&h, addresses::METRICS),
            AttachOutcome::AttachedMetrics
        );
    }

    #[test]
    fn dispatch_attach_to_audit_recognised() {
        let h = host_with(&[]);
        assert_eq!(
            dispatch_attach(&h, addresses::AUDIT),
            AttachOutcome::AttachedAudit
        );
    }

    // --- Transfer-Dispatch ---

    #[test]
    fn dispatch_transfer_publishes_to_dds_and_increments_metric() {
        let h = host_with(&[("Sensor", "SensorTopic")]);
        let topic_id = h.lookup("Sensor").unwrap();
        let received = Arc::new(AtomicUsize::new(0));
        let received_cb = received.clone();
        h.subscribe_amqp_to_dds(
            topic_id,
            Arc::new(move |bytes| {
                assert_eq!(bytes, b"sample-bytes");
                received_cb.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();

        let metrics = Arc::new(MetricsHub::new());
        dispatch_transfer(&h, topic_id, b"sample-bytes", &metrics).unwrap();
        assert_eq!(received.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.snapshot("transfers.received"), Some(1));
    }

    #[test]
    fn dispatch_transfer_unknown_topic_yields_error() {
        let h = host_with(&[]);
        let metrics = Arc::new(MetricsHub::new());
        let err = dispatch_transfer(&h, 99, b"x", &metrics).unwrap_err();
        assert!(matches!(err, DdsHostError::UnknownTopic(_)));
    }

    // --- Catalog-Producer ---

    #[test]
    fn produce_catalog_transfers_returns_one_per_topic() {
        let h = host_with(&[("AddrA", "TopicA"), ("AddrB", "TopicB")]);
        let samples = produce_catalog_transfers(&h);
        assert_eq!(samples.len(), 2);
        // Jedes Sample hat amqp-address als Pflichtfeld.
        for s in samples {
            let entries = match s {
                AmqpExtValue::Map(v) => v,
                _ => panic!("expected Map"),
            };
            let has_addr = entries
                .iter()
                .any(|(k, _)| matches!(k, AmqpExtValue::Symbol(s) if s == "amqp-address"));
            assert!(has_addr);
        }
    }

    #[test]
    fn produce_catalog_transfers_empty_for_empty_host() {
        let h = host_with(&[]);
        assert!(produce_catalog_transfers(&h).is_empty());
    }

    // --- Encode-Catalog-Sample ---

    #[test]
    fn encode_catalog_sample_produces_described_composite() {
        let h = host_with(&[("X", "T")]);
        let mut samples = produce_catalog_transfers(&h);
        let body = encode_catalog_sample(samples.remove(0)).unwrap();
        // Erstes Byte ist der described-prefix 0x00.
        assert_eq!(body[0], 0x00);
        assert!(body.len() > 4);
    }

    // --- Subscribe-Outbound ---

    #[test]
    fn subscribe_outbound_invokes_callback_on_publish() {
        let h = host_with(&[("X", "T")]);
        let topic_id = h.lookup("X").unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_cb = counter.clone();
        let _sid = subscribe_outbound(
            &h,
            topic_id,
            Arc::new(move |_| {
                counter_cb.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();

        let metrics = Arc::new(MetricsHub::new());
        dispatch_transfer(&h, topic_id, b"data", &metrics).unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
}
