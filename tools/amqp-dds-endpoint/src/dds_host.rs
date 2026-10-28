// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS host abstraction for the AMQP daemon bridge.
//!
//! Spec dds-amqp-1.0:
//! * §2.1 Cl. 3 — translate Sender/Receiver links to
//!   DDS-DataWriter/DataReader-Operations.
//! * §7.4.4 DEADLINE — DDS-side deadline-tracking; Notifications
//!   via the `$catalog` channel.
//! * §7.5 discovery bridging — the `$catalog` address returns topic
//!   Mappings.
//! * §7.7.1/7.7.2 — `dds:operation` lifecycle mapping.
//!
//! # Architecture
//!
//! The daemon crate does not know the DCPS crate directly; it
//! defines a `DdsHost` trait here that the caller
//! its DDS stack:
//! * `InMemoryDdsHost` — bundled in-memory implementation
//!   (tests + lightweight deployments without an RTPS wire).
//! * A future `DcpsDdsHost` variant would use the DCPS crate
//!   wirelevel anbinden (RTPS/UDP).
//!
//! # Integrations-Pattern
//!
//! 1. Daemon-Boot: `DdsHost::register_topic` pro `TopicMapping`
//!    from the configuration.
//! 2. Inbound attach (sender link, AMQP→DDS): the daemon calls
//!    `subscribe_dds_to_amqp` (no-op on the DDS side; bridge
//!    leitet AMQP-Transfers an `publish_to_dds`).
//! 3. Inbound attach (receiver link, DDS→AMQP): the daemon calls
//!    `subscribe_amqp_to_dds(topic, callback)`. DDS-Samples
//!    generate outbound AMQP transfers via callback.
//! 4. `$catalog` receiver: the daemon calls `topics()` and sends
//!    one catalog sample per entry.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use zerodds_amqp_endpoint::annex_a::{LinkDirection, TopicMapping};
use zerodds_amqp_endpoint::management::{CatalogDirection, CatalogEntry, CatalogTypeId};
use zerodds_amqp_endpoint::routing::AddressResolution;

/// Unique topic identifier in the DdsHost registry.
pub type TopicId = u64;

/// Unique subscription identifier.
pub type SubscriptionId = u64;

/// Sample callback: invoked when a DDS sample for a
/// AMQP→subscription is available.
pub type SampleCallback = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// DdsHost error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdsHostError {
    /// Topic does not exist in the registry.
    UnknownTopic(TopicId),
    /// Topic address already registered.
    DuplicateAddress(String),
    /// Subscription does not exist.
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

/// DdsHost trait — the bridge interface to the DDS side.
pub trait DdsHost {
    /// Spec §2.1 Cl. 3 — register topic mapping.
    /// Returns the assigned `TopicId`.
    ///
    /// # Errors
    /// `DuplicateAddress` if `mapping.amqp_address` is already
    /// is already registered.
    fn register_topic(&self, mapping: TopicMapping) -> Result<TopicId, DdsHostError>;

    /// Spec §2.1 Cl. 3 — AMQP → DDS Publish.
    ///
    /// `bytes` is the XCDR2/JSON/AMQP-native body depending on
    /// `mapping.mode`. The host is responsible for the
    /// DDS wire encode (or pass-through for MODE_PASSTHROUGH).
    ///
    /// # Errors
    /// `UnknownTopic`.
    fn publish_to_dds(&self, topic: TopicId, bytes: &[u8]) -> Result<(), DdsHostError>;

    /// Spec §2.1 Cl. 3 — DDS → AMQP Subscribe.
    ///
    /// `callback` is invoked as soon as DDS delivers a sample.
    /// Returns a `SubscriptionId` for later teardown.
    ///
    /// # Errors
    /// `UnknownTopic`.
    fn subscribe_amqp_to_dds(
        &self,
        topic: TopicId,
        callback: SampleCallback,
    ) -> Result<SubscriptionId, DdsHostError>;

    /// End subscription.
    ///
    /// # Errors
    /// `UnknownSubscription`.
    fn unsubscribe(&self, subscription: SubscriptionId) -> Result<(), DdsHostError>;

    /// Spec §7.5 — all registered topic mappings as
    /// Catalog-Entries.
    fn topics(&self) -> Vec<CatalogEntry>;

    /// Topic-Lookup per AMQP-Address.
    fn lookup(&self, amqp_address: &str) -> Option<TopicId>;
}

// ============================================================
// In-memory implementation
// ============================================================

/// In-memory `DdsHost` — tests + lightweight deployments without
/// actual DCPS wire.
///
/// Topics are held in a `BTreeMap<TopicId, ...>`.
/// `publish_to_dds(topic, bytes)` invokes all active
/// Subscriptions on the topic synchronously with the bytes.
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
    /// Fresh host without topics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Test helper: count of registered topics.
    #[must_use]
    pub fn topic_count(&self) -> usize {
        self.with_state(|s| s.topics.len())
    }

    /// Test helper: number of subscriptions on a topic.
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
        // We collect the callbacks under lock and invoke them
        // outside, so that a subscriber callback does not
        // may call publish() re-entrantly.
        let callbacks: Vec<SampleCallback> = self.with_state(|s| {
            s.topics
                .get(&topic)
                .map(|t| t.subscriptions.values().cloned().collect())
                .unwrap_or_default()
        });
        // Topic exists?
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
        // Spec §7.5: ulong for DESC_TRUNCATED, symbol for DESC_FULL.
        // Since we do not know the hash, we return a stub value
        // — the caller must enrich this with a real TypeIdentifier
        // if it needs spec-compliant catalog entries.
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
