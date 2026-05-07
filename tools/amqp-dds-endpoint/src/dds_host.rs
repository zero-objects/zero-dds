// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Host Abstraktion fuer die AMQP-Daemon-Bridge.
//!
//! Spec dds-amqp-1.0:
//! * §2.1 Cl. 3 — Sender/Receiver-Links translaten zu
//!   DDS-DataWriter/DataReader-Operations.
//! * §7.4.4 DEADLINE — DDS-side deadline-tracking; Notifications
//!   ueber `$catalog`-Channel.
//! * §7.5 Discovery Bridging — `$catalog`-Address liefert Topic-
//!   Mappings.
//! * §7.7.1/7.7.2 — `dds:operation` lifecycle-Mapping.
//!
//! # Architektur
//!
//! Die Daemon-Crate kennt die DCPS-Crate nicht direkt; sie
//! definiert hier eine `DdsHost`-Trait, gegen die der Caller
//! seinen DDS-Stack anbindet:
//! * `InMemoryDdsHost` — mitgelieferte In-Memory-Implementierung
//!   (Tests + leichte Deployments ohne RTPS-Wire).
//! * Eine zukuenftige `DcpsDdsHost`-Variante wuerde die DCPS-Crate
//!   wirelevel anbinden (RTPS/UDP).
//!
//! # Integrations-Pattern
//!
//! 1. Daemon-Boot: `DdsHost::register_topic` pro `TopicMapping`
//!    aus der Configuration.
//! 2. Inbound-Attach (Sender-Link, AMQP→DDS): Daemon ruft
//!    `subscribe_dds_to_amqp` (kein Op auf DDS-Side; Bridge
//!    leitet AMQP-Transfers an `publish_to_dds`).
//! 3. Inbound-Attach (Receiver-Link, DDS→AMQP): Daemon ruft
//!    `subscribe_amqp_to_dds(topic, callback)`. DDS-Samples
//!    erzeugen Outbound-AMQP-Transfers via Callback.
//! 4. `$catalog`-Receiver: Daemon ruft `topics()` und schickt
//!    pro Eintrag ein Catalog-Sample.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use zerodds_amqp_endpoint::annex_a::{LinkDirection, TopicMapping};
use zerodds_amqp_endpoint::management::{CatalogDirection, CatalogEntry, CatalogTypeId};
use zerodds_amqp_endpoint::routing::AddressResolution;

/// Eindeutiger Topic-Identifier in der DdsHost-Registry.
pub type TopicId = u64;

/// Eindeutiger Subscription-Identifier.
pub type SubscriptionId = u64;

/// Sample-Callback: wird aufgerufen wenn ein DDS-Sample fuer eine
/// AMQP→Subscription verfuegbar ist.
pub type SampleCallback = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// DdsHost-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdsHostError {
    /// Topic existiert nicht in der Registry.
    UnknownTopic(TopicId),
    /// Topic-Address bereits registriert.
    DuplicateAddress(String),
    /// Subscription existiert nicht.
    UnknownSubscription(SubscriptionId),
}

impl core::fmt::Display for DdsHostError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownTopic(id) => write!(f, "unknown topic id {id}"),
            Self::DuplicateAddress(s) => write!(f, "duplicate amqp address: {s}"),
            Self::UnknownSubscription(id) => write!(f, "unknown subscription id {id}"),
        }
    }
}

impl std::error::Error for DdsHostError {}

/// DdsHost-Trait — das Bridge-Interface zur DDS-Side.
pub trait DdsHost {
    /// Spec §2.1 Cl. 3 — Topic-Mapping registrieren.
    /// Liefert die zugewiesene `TopicId`.
    ///
    /// # Errors
    /// `DuplicateAddress` wenn `mapping.amqp_address` bereits
    /// registriert ist.
    fn register_topic(&self, mapping: TopicMapping) -> Result<TopicId, DdsHostError>;

    /// Spec §2.1 Cl. 3 — AMQP → DDS Publish.
    ///
    /// `bytes` ist der XCDR2-/JSON-/AMQP-Native-Body je nach
    /// `mapping.mode`. Der Host ist verantwortlich fuer den
    /// DDS-Wire-Encode (oder Pass-Through bei MODE_PASSTHROUGH).
    ///
    /// # Errors
    /// `UnknownTopic`.
    fn publish_to_dds(&self, topic: TopicId, bytes: &[u8]) -> Result<(), DdsHostError>;

    /// Spec §2.1 Cl. 3 — DDS → AMQP Subscribe.
    ///
    /// `callback` wird invoked sobald DDS ein Sample liefert.
    /// Liefert eine `SubscriptionId` zur spaeteren Aufhebung.
    ///
    /// # Errors
    /// `UnknownTopic`.
    fn subscribe_amqp_to_dds(
        &self,
        topic: TopicId,
        callback: SampleCallback,
    ) -> Result<SubscriptionId, DdsHostError>;

    /// Subscription beenden.
    ///
    /// # Errors
    /// `UnknownSubscription`.
    fn unsubscribe(&self, subscription: SubscriptionId) -> Result<(), DdsHostError>;

    /// Spec §7.5 — alle registrierten Topic-Mappings als
    /// Catalog-Entries.
    fn topics(&self) -> Vec<CatalogEntry>;

    /// Topic-Lookup per AMQP-Address.
    fn lookup(&self, amqp_address: &str) -> Option<TopicId>;
}

// ============================================================
// In-Memory-Implementierung
// ============================================================

/// In-Memory `DdsHost` — Tests + leichte Deployments ohne
/// echte DCPS-Wire.
///
/// Topics werden in einer `BTreeMap<TopicId, ...>` gehalten.
/// `publish_to_dds(topic, bytes)` invoked alle aktiven
/// Subscriptions auf dem Topic synchron mit den Bytes.
#[derive(Debug, Default)]
pub struct InMemoryDdsHost {
    inner: Mutex<HostState>,
}

#[derive(Debug, Default)]
struct HostState {
    next_topic_id: TopicId,
    next_subscription_id: SubscriptionId,
    topics: BTreeMap<TopicId, TopicEntry>,
    address_index: BTreeMap<String, TopicId>,
}

struct TopicEntry {
    mapping: TopicMapping,
    subscriptions: BTreeMap<SubscriptionId, SampleCallback>,
}

impl core::fmt::Debug for TopicEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TopicEntry")
            .field("mapping", &self.mapping)
            .field("subscription_count", &self.subscriptions.len())
            .finish()
    }
}

impl InMemoryDdsHost {
    /// Frischer Host ohne Topics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Test-helper: Anzahl registrierter Topics.
    #[must_use]
    pub fn topic_count(&self) -> usize {
        self.with_state(|s| s.topics.len())
    }

    /// Test-helper: Anzahl Subscriptions auf einem Topic.
    #[must_use]
    pub fn subscription_count(&self, topic: TopicId) -> usize {
        self.with_state(|s| s.topics.get(&topic).map_or(0, |t| t.subscriptions.len()))
    }

    fn with_state<R>(&self, f: impl FnOnce(&HostState) -> R) -> R {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    fn with_state_mut<R>(&self, f: impl FnOnce(&mut HostState) -> R) -> R {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    }
}

impl DdsHost for InMemoryDdsHost {
    fn register_topic(&self, mapping: TopicMapping) -> Result<TopicId, DdsHostError> {
        self.with_state_mut(|s| {
            if s.address_index.contains_key(&mapping.amqp_address) {
                return Err(DdsHostError::DuplicateAddress(mapping.amqp_address.clone()));
            }
            s.next_topic_id = s.next_topic_id.saturating_add(1);
            let id = s.next_topic_id;
            s.address_index.insert(mapping.amqp_address.clone(), id);
            s.topics.insert(
                id,
                TopicEntry {
                    mapping,
                    subscriptions: BTreeMap::new(),
                },
            );
            Ok(id)
        })
    }

    fn publish_to_dds(&self, topic: TopicId, bytes: &[u8]) -> Result<(), DdsHostError> {
        // Wir sammeln die Callbacks unter Lock und invoken sie
        // ausserhalb, damit ein Subscriber-Callback nicht
        // re-entrant publish() aufrufen kann.
        let callbacks: Vec<SampleCallback> = self.with_state(|s| {
            s.topics
                .get(&topic)
                .map(|t| t.subscriptions.values().cloned().collect())
                .unwrap_or_default()
        });
        // Topic existiert?
        if self.with_state(|s| !s.topics.contains_key(&topic)) {
            return Err(DdsHostError::UnknownTopic(topic));
        }
        for cb in callbacks {
            cb(bytes);
        }
        Ok(())
    }

    fn subscribe_amqp_to_dds(
        &self,
        topic: TopicId,
        callback: SampleCallback,
    ) -> Result<SubscriptionId, DdsHostError> {
        self.with_state_mut(|s| {
            let entry = s
                .topics
                .get_mut(&topic)
                .ok_or(DdsHostError::UnknownTopic(topic))?;
            s.next_subscription_id = s.next_subscription_id.saturating_add(1);
            let id = s.next_subscription_id;
            entry.subscriptions.insert(id, callback);
            Ok(id)
        })
    }

    fn unsubscribe(&self, subscription: SubscriptionId) -> Result<(), DdsHostError> {
        self.with_state_mut(|s| {
            for entry in s.topics.values_mut() {
                if entry.subscriptions.remove(&subscription).is_some() {
                    return Ok(());
                }
            }
            Err(DdsHostError::UnknownSubscription(subscription))
        })
    }

    fn topics(&self) -> Vec<CatalogEntry> {
        self.with_state(|s| {
            s.topics
                .values()
                .map(|t| topic_mapping_to_catalog(&t.mapping))
                .collect()
        })
    }

    fn lookup(&self, amqp_address: &str) -> Option<TopicId> {
        self.with_state(|s| s.address_index.get(amqp_address).copied())
    }
}

fn topic_mapping_to_catalog(m: &TopicMapping) -> CatalogEntry {
    let direction = match m.direction {
        LinkDirection::DirProducerToDds => CatalogDirection::ProducerToDds,
        LinkDirection::DirDdsToConsumer => CatalogDirection::DdsToConsumer,
        LinkDirection::DirBoth => CatalogDirection::Both,
    };
    CatalogEntry {
        amqp_address: m.amqp_address.clone(),
        dds: AddressResolution {
            topic: m.dds_topic.clone(),
            domain_id: m.dds_domain_id,
            partitions: m.dds_partition.clone(),
        },
        dds_type_name: m.dds_type_name.clone(),
        // Spec §7.5: ulong fuer DESC_TRUNCATED, symbol fuer DESC_FULL.
        // Da wir den Hash nicht kennen, geben wir einen Stub-Wert
        // zurueck — der Caller muss das mit echtem TypeIdentifier
        // anreichern, wenn er Spec-konforme Catalog-Entries braucht.
        type_id: CatalogTypeId::Truncated(0),
        direction,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn mapping(addr: &str, topic: &str) -> TopicMapping {
        TopicMapping {
            amqp_address: addr.to_string(),
            dds_topic: topic.to_string(),
            dds_type_name: "T".to_string(),
            ..TopicMapping::default()
        }
    }

    #[test]
    fn register_topic_returns_unique_ids() {
        let h = InMemoryDdsHost::new();
        let id1 = h.register_topic(mapping("a", "TopicA")).unwrap();
        let id2 = h.register_topic(mapping("b", "TopicB")).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(h.topic_count(), 2);
    }

    #[test]
    fn duplicate_address_yields_error() {
        let h = InMemoryDdsHost::new();
        h.register_topic(mapping("a", "TopicA")).unwrap();
        let err = h
            .register_topic(mapping("a", "DifferentTopic"))
            .unwrap_err();
        assert!(matches!(err, DdsHostError::DuplicateAddress(_)));
    }

    #[test]
    fn lookup_returns_topic_id() {
        let h = InMemoryDdsHost::new();
        let id = h.register_topic(mapping("foo", "T")).unwrap();
        assert_eq!(h.lookup("foo"), Some(id));
        assert_eq!(h.lookup("bar"), None);
    }

    #[test]
    fn subscribe_invokes_callback_on_publish() {
        let h = InMemoryDdsHost::new();
        let topic = h.register_topic(mapping("a", "T")).unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_cb = counter.clone();
        let cb: SampleCallback = Arc::new(move |bytes: &[u8]| {
            assert_eq!(bytes, b"hello");
            counter_cb.fetch_add(1, Ordering::Relaxed);
        });
        h.subscribe_amqp_to_dds(topic, cb).unwrap();

        h.publish_to_dds(topic, b"hello").unwrap();
        h.publish_to_dds(topic, b"hello").unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn multiple_subscribers_all_get_called() {
        let h = InMemoryDdsHost::new();
        let topic = h.register_topic(mapping("a", "T")).unwrap();
        let c1 = Arc::new(AtomicUsize::new(0));
        let c2 = Arc::new(AtomicUsize::new(0));
        let c1_cb = c1.clone();
        let c2_cb = c2.clone();
        h.subscribe_amqp_to_dds(
            topic,
            Arc::new(move |_| {
                c1_cb.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        h.subscribe_amqp_to_dds(
            topic,
            Arc::new(move |_| {
                c2_cb.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        assert_eq!(h.subscription_count(topic), 2);
        h.publish_to_dds(topic, b"x").unwrap();
        assert_eq!(c1.load(Ordering::Relaxed), 1);
        assert_eq!(c2.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unsubscribe_stops_callbacks() {
        let h = InMemoryDdsHost::new();
        let topic = h.register_topic(mapping("a", "T")).unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_cb = counter.clone();
        let sid = h
            .subscribe_amqp_to_dds(
                topic,
                Arc::new(move |_| {
                    counter_cb.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap();
        h.publish_to_dds(topic, b"a").unwrap();
        h.unsubscribe(sid).unwrap();
        h.publish_to_dds(topic, b"b").unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        assert_eq!(h.subscription_count(topic), 0);
    }

    #[test]
    fn unsubscribe_unknown_yields_error() {
        let h = InMemoryDdsHost::new();
        let err = h.unsubscribe(99999).unwrap_err();
        assert!(matches!(err, DdsHostError::UnknownSubscription(_)));
    }

    #[test]
    fn publish_to_unknown_topic_yields_error() {
        let h = InMemoryDdsHost::new();
        let err = h.publish_to_dds(42, b"x").unwrap_err();
        assert!(matches!(err, DdsHostError::UnknownTopic(_)));
    }

    #[test]
    fn topics_returns_catalog_entries_for_all_registrations() {
        let h = InMemoryDdsHost::new();
        let mut m1 = mapping("a", "TopicA");
        m1.direction = LinkDirection::DirProducerToDds;
        let mut m2 = mapping("b", "TopicB");
        m2.direction = LinkDirection::DirDdsToConsumer;
        h.register_topic(m1).unwrap();
        h.register_topic(m2).unwrap();
        let cat = h.topics();
        assert_eq!(cat.len(), 2);
        let dirs: Vec<CatalogDirection> = cat.iter().map(|c| c.direction).collect();
        assert!(dirs.contains(&CatalogDirection::ProducerToDds));
        assert!(dirs.contains(&CatalogDirection::DdsToConsumer));
    }

    #[test]
    fn fresh_host_has_no_topics() {
        let h = InMemoryDdsHost::new();
        assert_eq!(h.topic_count(), 0);
        assert!(h.topics().is_empty());
        assert!(h.lookup("anything").is_none());
    }

    #[test]
    fn register_topic_preserves_partitions() {
        let h = InMemoryDdsHost::new();
        let mut m = mapping("addr", "Topic");
        m.dds_partition = vec!["p1".into(), "p2".into()];
        h.register_topic(m).unwrap();
        let cat = h.topics();
        assert_eq!(cat[0].dds.partitions, vec!["p1", "p2"]);
    }
}
