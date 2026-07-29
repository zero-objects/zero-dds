// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DiscoveredEndpointsCache` — in-memory cache of discovered SEDP
//! publications and subscriptions.
//!
//! Per in-flight participant we keep separate pub/sub maps with
//! LRU eviction on overflow of a configurable cap (analogous to
//! Fast-DDS `ReaderProxy.resource_limits.max_samples`, from WP 1.2
//! review research). Eviction **per remote participant**, not global,
//! so that a chatty remote does not displace the discoveries from other
//! participants.
//!
//! `on_participant_lost(prefix)` evicts all endpoints of a remote
//! participant at once — necessary when SPDP detects a lease timeout.

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{Guid, GuidPrefix};

/// Default cap for publications per remote participant.
pub const DEFAULT_MAX_PUBLICATIONS_PER_PARTICIPANT: usize = 256;
/// Default cap for subscriptions per remote participant.
pub const DEFAULT_MAX_SUBSCRIPTIONS_PER_PARTICIPANT: usize = 256;

/// Configuration (DoS caps).
#[derive(Debug, Clone, Copy)]
pub struct CacheCaps {
    /// Max. publications per remote participant. Overflow → oldest
    /// publication of this participant is discarded (LRU).
    pub max_publications_per_participant: usize,
    /// Max. subscriptions per remote participant. Same semantics.
    pub max_subscriptions_per_participant: usize,
}

impl Default for CacheCaps {
    fn default() -> Self {
        Self {
            max_publications_per_participant: DEFAULT_MAX_PUBLICATIONS_PER_PARTICIPANT,
            max_subscriptions_per_participant: DEFAULT_MAX_SUBSCRIPTIONS_PER_PARTICIPANT,
        }
    }
}

/// A discovered publication with a timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPublication {
    /// Wire data from the SEDP DATA submessage.
    pub data: PublicationBuiltinTopicData,
    /// Time of the (most recent) discovery — uptime-based, for LRU.
    pub discovered_at: Duration,
}

/// A discovered subscription with a timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSubscription {
    /// Wire data from the SEDP DATA submessage.
    pub data: SubscriptionBuiltinTopicData,
    /// Time of the (most recent) discovery.
    pub discovered_at: Duration,
}

/// Secondary index per remote participant, sorted by `discovered_at`.
/// Allows O(log n) count (`set.len()`) and O(log n) LRU eviction
/// (`set.iter().next()` = oldest entry). The index must be kept in sync
/// with `publications`/`subscriptions` on every update.
type PerPrefixIndex = BTreeMap<GuidPrefix, BTreeSet<(Duration, Guid)>>;

/// In-memory cache for SEDP-discovered endpoints.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredEndpointsCache {
    publications: BTreeMap<Guid, DiscoveredPublication>,
    subscriptions: BTreeMap<Guid, DiscoveredSubscription>,
    // F3 fix: secondary indices for O(log n) count + LRU eviction.
    // Validity: for every key k in `publications` there is exactly one
    // entry `(k-discovered_at, k)` in `pub_index[k.prefix]`. Same for
    // subscriptions/sub_index. The invariant is maintained in every
    // insert/update/remove path.
    pub_index: PerPrefixIndex,
    sub_index: PerPrefixIndex,
    caps: CacheCaps,
    evicted: u64,
}

impl DiscoveredEndpointsCache {
    /// New with the given caps.
    #[must_use]
    pub fn new(caps: CacheCaps) -> Self {
        Self {
            publications: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            pub_index: BTreeMap::new(),
            sub_index: BTreeMap::new(),
            caps,
            evicted: 0,
        }
    }

    /// Total number of publications.
    #[must_use]
    pub fn publications_len(&self) -> usize {
        self.publications.len()
    }

    /// Total number of subscriptions.
    #[must_use]
    pub fn subscriptions_len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Number of LRU evictions since start (DoS diagnostics).
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.evicted
    }

    /// Looks up a publication by endpoint GUID.
    #[must_use]
    pub fn publication(&self, key: Guid) -> Option<&DiscoveredPublication> {
        self.publications.get(&key)
    }

    /// Looks up a subscription by endpoint GUID.
    #[must_use]
    pub fn subscription(&self, key: Guid) -> Option<&DiscoveredSubscription> {
        self.subscriptions.get(&key)
    }

    /// Iterates all publications of a remote participant.
    pub fn publications_for(
        &self,
        prefix: GuidPrefix,
    ) -> impl Iterator<Item = &DiscoveredPublication> + '_ {
        self.publications
            .iter()
            .filter(move |(guid, _)| guid.prefix == prefix)
            .map(|(_, p)| p)
    }

    /// Iterates all subscriptions of a remote participant.
    pub fn subscriptions_for(
        &self,
        prefix: GuidPrefix,
    ) -> impl Iterator<Item = &DiscoveredSubscription> + '_ {
        self.subscriptions
            .iter()
            .filter(move |(guid, _)| guid.prefix == prefix)
            .map(|(_, p)| p)
    }

    /// Iterates all publications (for the matching scan).
    pub fn publications(&self) -> impl Iterator<Item = &DiscoveredPublication> + '_ {
        self.publications.values()
    }

    /// Iterates all subscriptions.
    pub fn subscriptions(&self) -> impl Iterator<Item = &DiscoveredSubscription> + '_ {
        self.subscriptions.values()
    }

    /// Insert or update of a publication. Idempotent: for an
    /// already-present GUID, `data` is overwritten and `discovered_at` is
    /// updated — **no** eviction round.
    ///
    /// On a new insert: if this participant's publication count reaches
    /// the cap, the oldest publication of the same participant is
    /// discarded (LRU) and `evicted_count` is incremented.
    ///
    /// Returns: `true` = newly inserted, `false` = update.
    pub fn insert_publication(&mut self, data: PublicationBuiltinTopicData, now: Duration) -> bool {
        let key = data.key;
        let prefix = key.prefix;
        if let Some(existing) = self.publications.get_mut(&key) {
            // Update path: move the index to the new `discovered_at`.
            let old_entry = (existing.discovered_at, key);
            if let Some(set) = self.pub_index.get_mut(&prefix) {
                set.remove(&old_entry);
                set.insert((now, key));
            }
            existing.data = data;
            existing.discovered_at = now;
            return false;
        }
        // Cap check + LRU eviction via secondary index — O(log n) instead
        // of the earlier O(N) (F3 fix).
        let set = self.pub_index.entry(prefix).or_default();
        if set.len() >= self.caps.max_publications_per_participant {
            if let Some(&oldest) = set.iter().next() {
                set.remove(&oldest);
                self.publications.remove(&oldest.1);
                self.evicted = self.evicted.saturating_add(1);
            }
        }
        set.insert((now, key));
        self.publications.insert(
            key,
            DiscoveredPublication {
                data,
                discovered_at: now,
            },
        );
        true
    }

    /// Insert or update of a subscription. Semantics analogous to
    /// [`Self::insert_publication`].
    pub fn insert_subscription(
        &mut self,
        data: SubscriptionBuiltinTopicData,
        now: Duration,
    ) -> bool {
        let key = data.key;
        let prefix = key.prefix;
        if let Some(existing) = self.subscriptions.get_mut(&key) {
            let old_entry = (existing.discovered_at, key);
            if let Some(set) = self.sub_index.get_mut(&prefix) {
                set.remove(&old_entry);
                set.insert((now, key));
            }
            existing.data = data;
            existing.discovered_at = now;
            return false;
        }
        let set = self.sub_index.entry(prefix).or_default();
        if set.len() >= self.caps.max_subscriptions_per_participant {
            if let Some(&oldest) = set.iter().next() {
                set.remove(&oldest);
                self.subscriptions.remove(&oldest.1);
                self.evicted = self.evicted.saturating_add(1);
            }
        }
        set.insert((now, key));
        self.subscriptions.insert(
            key,
            DiscoveredSubscription {
                data,
                discovered_at: now,
            },
        );
        true
    }

    /// Removes a publication by GUID (e.g. after an explicit withdrawal
    /// notification from the remote).
    pub fn remove_publication(&mut self, key: Guid) -> Option<DiscoveredPublication> {
        let removed = self.publications.remove(&key)?;
        if let Some(set) = self.pub_index.get_mut(&key.prefix) {
            set.remove(&(removed.discovered_at, key));
            if set.is_empty() {
                self.pub_index.remove(&key.prefix);
            }
        }
        Some(removed)
    }

    /// Removes a subscription by GUID.
    pub fn remove_subscription(&mut self, key: Guid) -> Option<DiscoveredSubscription> {
        let removed = self.subscriptions.remove(&key)?;
        if let Some(set) = self.sub_index.get_mut(&key.prefix) {
            set.remove(&(removed.discovered_at, key));
            if set.is_empty() {
                self.sub_index.remove(&key.prefix);
            }
        }
        Some(removed)
    }

    /// Removes all publications and subscriptions of a remote
    /// participant. Called e.g. on an SPDP lease timeout.
    ///
    /// Returns: (removed_pubs, removed_subs).
    pub fn on_participant_lost(&mut self, prefix: GuidPrefix) -> (usize, usize) {
        // The secondary index has the exact key list per prefix — O(k)
        // instead of an O(N) retain.
        let removed_pubs = if let Some(set) = self.pub_index.remove(&prefix) {
            for (_, g) in &set {
                self.publications.remove(g);
            }
            set.len()
        } else {
            0
        };
        let removed_subs = if let Some(set) = self.sub_index.remove(&prefix) {
            for (_, g) in &set {
                self.subscriptions.remove(g);
            }
            set.len()
        } else {
            0
        };
        (removed_pubs, removed_subs)
    }

    /// Finds all publications matching a `(topic_name, type_name)` —
    /// for reader-side matching (T4/T5).
    pub fn match_publications<'a>(
        &'a self,
        topic: &'a str,
        type_name: &'a str,
    ) -> impl Iterator<Item = &'a DiscoveredPublication> + 'a {
        self.publications
            .values()
            .filter(move |p| p.data.topic_name == topic && p.data.type_name == type_name)
    }

    /// Finds all subscriptions matching a `(topic_name, type_name)` —
    /// for writer-side matching.
    pub fn match_subscriptions<'a>(
        &'a self,
        topic: &'a str,
        type_name: &'a str,
    ) -> impl Iterator<Item = &'a DiscoveredSubscription> + 'a {
        self.subscriptions
            .values()
            .filter(move |s| s.data.topic_name == topic && s.data.type_name == type_name)
    }

    /// Collects the GUIDs of all publications (for snapshot tests /
    /// diagnostics).
    #[must_use]
    pub fn publication_keys(&self) -> Vec<Guid> {
        self.publications.keys().copied().collect()
    }

    /// True if any discovered publication OR subscription carries the given
    /// `topic_name` but a *different* `type_name` — the DDS 1.4 §2.2.4.2.4
    /// inconsistent-topic condition (same topic, incompatible type). Used by
    /// the matching path to raise `on_inconsistent_topic`.
    #[must_use]
    pub fn topic_name_conflicts(&self, topic: &str, type_name: &str) -> bool {
        self.publications
            .values()
            .any(|p| p.data.topic_name == topic && p.data.type_name != type_name)
            || self
                .subscriptions
                .values()
                .any(|s| s.data.topic_name == topic && s.data.type_name != type_name)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_rtps::participant_data::Duration as DdsDuration;
    use zerodds_rtps::publication_data::{
        DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
    };
    use zerodds_rtps::wire_types::EntityId;

    fn guid(prefix: [u8; 12], key: [u8; 3]) -> Guid {
        Guid::new(
            GuidPrefix::from_bytes(prefix),
            EntityId::user_writer_with_key(key),
        )
    }

    fn pub_data(prefix: [u8; 12], key: [u8; 3], topic: &str) -> PublicationBuiltinTopicData {
        PublicationBuiltinTopicData {
            key: guid(prefix, key),
            participant_key: Guid::new(GuidPrefix::from_bytes(prefix), EntityId::PARTICIPANT),
            topic_name: topic.into(),
            type_name: "T".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: DdsDuration::from_secs(1),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec::Vec::new(),
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        }
    }

    #[test]
    fn topic_name_conflicts_detects_type_mismatch() {
        let mut c = DiscoveredEndpointsCache::default();
        // pub_data sets type_name = "T" on topic "X".
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "X"), Duration::ZERO);
        // Same topic + same type → no conflict.
        assert!(!c.topic_name_conflicts("X", "T"));
        // Same topic + different type → inconsistent topic.
        assert!(c.topic_name_conflicts("X", "U"));
        // Different topic → no conflict regardless of type.
        assert!(!c.topic_name_conflicts("Y", "U"));
    }

    #[test]
    fn fresh_cache_is_empty() {
        let c = DiscoveredEndpointsCache::default();
        assert_eq!(c.publications_len(), 0);
        assert_eq!(c.subscriptions_len(), 0);
        assert_eq!(c.evicted_count(), 0);
    }

    #[test]
    fn insert_publication_returns_true_first_time() {
        let mut c = DiscoveredEndpointsCache::default();
        let inserted = c.insert_publication(pub_data([1; 12], [1, 0, 0], "T"), Duration::ZERO);
        assert!(inserted);
        assert_eq!(c.publications_len(), 1);
    }

    #[test]
    fn reinsert_same_guid_updates_in_place() {
        let mut c = DiscoveredEndpointsCache::default();
        let first = c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::ZERO);
        let second =
            c.insert_publication(pub_data([1; 12], [1, 0, 0], "B"), Duration::from_secs(1));
        assert!(first);
        assert!(!second, "reinsert should return false (update, not insert)");
        assert_eq!(c.publications_len(), 1);
        let p = c.publication(guid([1; 12], [1, 0, 0])).unwrap();
        assert_eq!(p.data.topic_name, "B");
        assert_eq!(p.discovered_at, Duration::from_secs(1));
    }

    #[test]
    fn publications_for_filters_by_prefix() {
        let mut c = DiscoveredEndpointsCache::default();
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::ZERO);
        c.insert_publication(pub_data([2; 12], [2, 0, 0], "B"), Duration::ZERO);
        c.insert_publication(pub_data([1; 12], [3, 0, 0], "C"), Duration::ZERO);
        let p1: Vec<_> = c
            .publications_for(GuidPrefix::from_bytes([1; 12]))
            .collect();
        assert_eq!(p1.len(), 2);
        let p2: Vec<_> = c
            .publications_for(GuidPrefix::from_bytes([2; 12]))
            .collect();
        assert_eq!(p2.len(), 1);
    }

    #[test]
    fn cap_evicts_oldest_of_same_participant() {
        let caps = CacheCaps {
            max_publications_per_participant: 2,
            max_subscriptions_per_participant: 2,
        };
        let mut c = DiscoveredEndpointsCache::new(caps);
        // 3 publications from the same participant, 1 second apart each.
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::from_secs(1));
        c.insert_publication(pub_data([1; 12], [2, 0, 0], "B"), Duration::from_secs(2));
        c.insert_publication(pub_data([1; 12], [3, 0, 0], "C"), Duration::from_secs(3));
        // Oldest (A) out, B+C in
        assert_eq!(c.publications_len(), 2);
        assert!(c.publication(guid([1; 12], [1, 0, 0])).is_none());
        assert!(c.publication(guid([1; 12], [2, 0, 0])).is_some());
        assert!(c.publication(guid([1; 12], [3, 0, 0])).is_some());
        assert_eq!(c.evicted_count(), 1);
    }

    #[test]
    fn cap_is_per_participant_not_global() {
        let caps = CacheCaps {
            max_publications_per_participant: 1,
            ..CacheCaps::default()
        };
        let mut c = DiscoveredEndpointsCache::new(caps);
        // 1 publication per participant — 3 participants total, so
        // 3 publications total, no eviction.
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::ZERO);
        c.insert_publication(pub_data([2; 12], [2, 0, 0], "B"), Duration::ZERO);
        c.insert_publication(pub_data([3; 12], [3, 0, 0], "C"), Duration::ZERO);
        assert_eq!(c.publications_len(), 3);
        assert_eq!(c.evicted_count(), 0);
    }

    #[test]
    fn on_participant_lost_removes_all_of_that_prefix() {
        let mut c = DiscoveredEndpointsCache::default();
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::ZERO);
        c.insert_publication(pub_data([1; 12], [2, 0, 0], "B"), Duration::ZERO);
        c.insert_publication(pub_data([2; 12], [3, 0, 0], "C"), Duration::ZERO);
        let (pubs, subs) = c.on_participant_lost(GuidPrefix::from_bytes([1; 12]));
        assert_eq!(pubs, 2);
        assert_eq!(subs, 0);
        assert_eq!(c.publications_len(), 1);
        assert!(c.publication(guid([2; 12], [3, 0, 0])).is_some());
    }

    #[test]
    fn remove_publication_returns_removed() {
        let mut c = DiscoveredEndpointsCache::default();
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::ZERO);
        let removed = c.remove_publication(guid([1; 12], [1, 0, 0]));
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().data.topic_name, "A");
        assert_eq!(c.publications_len(), 0);
        assert!(
            c.remove_publication(guid([1; 12], [1, 0, 0])).is_none(),
            "second remove is None"
        );
    }

    #[test]
    fn match_publications_filters_topic_and_type() {
        let mut c = DiscoveredEndpointsCache::default();
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "Chatter"), Duration::ZERO);
        let mut p = pub_data([1; 12], [2, 0, 0], "Chatter");
        p.type_name = "OtherType".into();
        c.insert_publication(p, Duration::ZERO);
        c.insert_publication(pub_data([1; 12], [3, 0, 0], "Weather"), Duration::ZERO);

        let matches: Vec<_> = c.match_publications("Chatter", "T").collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].data.topic_name, "Chatter");
        assert_eq!(matches[0].data.type_name, "T");
    }

    #[test]
    fn subscriptions_match_topic_type() {
        use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
        let mut c = DiscoveredEndpointsCache::default();
        let sub = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
            topic_name: "Chatter".into(),
            type_name: "T".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        c.insert_subscription(sub, Duration::ZERO);
        assert_eq!(c.subscriptions_len(), 1);
        assert_eq!(c.match_subscriptions("Chatter", "T").count(), 1);
        assert_eq!(c.match_subscriptions("Chatter", "Other").count(), 0);
    }

    #[test]
    fn update_does_not_evict_even_at_cap() {
        let caps = CacheCaps {
            max_publications_per_participant: 1,
            ..CacheCaps::default()
        };
        let mut c = DiscoveredEndpointsCache::new(caps);
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::from_secs(1));
        // Reinserting the same GUID must not trigger an eviction
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "B"), Duration::from_secs(2));
        assert_eq!(c.publications_len(), 1);
        assert_eq!(c.evicted_count(), 0, "update must not count as eviction");
    }

    #[test]
    fn publication_keys_enumerates_all_guids() {
        let mut c = DiscoveredEndpointsCache::default();
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::ZERO);
        c.insert_publication(pub_data([2; 12], [2, 0, 0], "B"), Duration::ZERO);
        let keys = c.publication_keys();
        assert_eq!(keys.len(), 2);
    }
}
