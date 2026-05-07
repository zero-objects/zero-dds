// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DiscoveredEndpointsCache` — in-memory cache entdeckter SEDP-
//! Publications und Subscriptions.
//!
//! Pro in-flight Participant halten wir separat Pub/Sub-Maps mit
//! LRU-Eviction bei Ueberlauf eines konfigurierbaren Caps (analog
//! Fast-DDS `ReaderProxy.resource_limits.max_samples`, Recherche aus
//! WP 1.2 Review). Eviction **pro Remote-Participant**, nicht global,
//! damit ein chatty Remote nicht die Discoveries aus anderen
//! Participants verdraengt.
//!
//! `on_participant_lost(prefix)` wirft alle Endpoints eines
//! Remote-Participants auf einmal raus — notwendig, wenn SPDP einen
//! Lease-Timeout erkennt.

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_rtps::publication_data::PublicationBuiltinTopicData;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{Guid, GuidPrefix};

/// Default-Cap fuer Publications pro Remote-Participant.
pub const DEFAULT_MAX_PUBLICATIONS_PER_PARTICIPANT: usize = 256;
/// Default-Cap fuer Subscriptions pro Remote-Participant.
pub const DEFAULT_MAX_SUBSCRIPTIONS_PER_PARTICIPANT: usize = 256;

/// Konfiguration (DoS-Caps).
#[derive(Debug, Clone, Copy)]
pub struct CacheCaps {
    /// Max. Publications pro Remote-Participant. Ueberlauf → aelteste
    /// Publication dieses Participants verworfen (LRU).
    pub max_publications_per_participant: usize,
    /// Max. Subscriptions pro Remote-Participant. Gleiche Semantik.
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

/// Eine entdeckte Publication mit Zeitstempel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPublication {
    /// Wire-Daten aus der SEDP-DATA-Submessage.
    pub data: PublicationBuiltinTopicData,
    /// Zeitpunkt der (letzten) Discovery — Uptime-basiert, fuer LRU.
    pub discovered_at: Duration,
}

/// Eine entdeckte Subscription mit Zeitstempel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSubscription {
    /// Wire-Daten aus der SEDP-DATA-Submessage.
    pub data: SubscriptionBuiltinTopicData,
    /// Zeitpunkt der (letzten) Discovery.
    pub discovered_at: Duration,
}

/// Sekundär-Index pro Remote-Participant, sortiert nach
/// `discovered_at`. Erlaubt O(log n) Count (`set.len()`) und O(log n)
/// LRU-Eviction (`set.iter().next()` = aeltester Eintrag). Der Index
/// muss bei jedem Update auf `publications`/`subscriptions`
/// synchronisiert werden.
type PerPrefixIndex = BTreeMap<GuidPrefix, BTreeSet<(Duration, Guid)>>;

/// In-memory Cache fuer SEDP-entdeckte Endpoints.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredEndpointsCache {
    publications: BTreeMap<Guid, DiscoveredPublication>,
    subscriptions: BTreeMap<Guid, DiscoveredSubscription>,
    // F3-Fix: Sekundaer-Indizes fuer O(log n) Count + LRU-Eviction.
    // Gueltigkeit: fuer jeden Schluessel k in `publications` existiert
    // genau ein Eintrag `(k-discovered_at, k)` in `pub_index[k.prefix]`.
    // Selbiges fuer subscriptions/sub_index. Invariante wird in jedem
    // Insert/Update/Remove-Pfad erhalten.
    pub_index: PerPrefixIndex,
    sub_index: PerPrefixIndex,
    caps: CacheCaps,
    evicted: u64,
}

impl DiscoveredEndpointsCache {
    /// Neu mit den gegebenen Caps.
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

    /// Anzahl Publications insgesamt.
    #[must_use]
    pub fn publications_len(&self) -> usize {
        self.publications.len()
    }

    /// Anzahl Subscriptions insgesamt.
    #[must_use]
    pub fn subscriptions_len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Anzahl LRU-Evictions seit Start (DoS-Diagnose).
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.evicted
    }

    /// Lookup einer Publication per Endpoint-GUID.
    #[must_use]
    pub fn publication(&self, key: Guid) -> Option<&DiscoveredPublication> {
        self.publications.get(&key)
    }

    /// Lookup einer Subscription per Endpoint-GUID.
    #[must_use]
    pub fn subscription(&self, key: Guid) -> Option<&DiscoveredSubscription> {
        self.subscriptions.get(&key)
    }

    /// Iteriert alle Publications eines Remote-Participants.
    pub fn publications_for(
        &self,
        prefix: GuidPrefix,
    ) -> impl Iterator<Item = &DiscoveredPublication> + '_ {
        self.publications
            .iter()
            .filter(move |(guid, _)| guid.prefix == prefix)
            .map(|(_, p)| p)
    }

    /// Iteriert alle Subscriptions eines Remote-Participants.
    pub fn subscriptions_for(
        &self,
        prefix: GuidPrefix,
    ) -> impl Iterator<Item = &DiscoveredSubscription> + '_ {
        self.subscriptions
            .iter()
            .filter(move |(guid, _)| guid.prefix == prefix)
            .map(|(_, p)| p)
    }

    /// Iteriert alle Publications (fuer Matching-Scan).
    pub fn publications(&self) -> impl Iterator<Item = &DiscoveredPublication> + '_ {
        self.publications.values()
    }

    /// Iteriert alle Subscriptions.
    pub fn subscriptions(&self) -> impl Iterator<Item = &DiscoveredSubscription> + '_ {
        self.subscriptions.values()
    }

    /// Insert oder Update einer Publication. Idempotent:
    /// bei bereits vorhandener GUID wird `data` ueberschrieben und
    /// `discovered_at` aktualisiert — **keine** Eviction-Runde.
    ///
    /// Bei neuem Insert: wenn die Publications-Zahl dieses Participants
    /// den Cap erreicht, wird die aelteste Publication desselben
    /// Participants verworfen (LRU), `evicted_count` inkrementiert.
    ///
    /// Rueckgabe: `true` = neu eingefuegt, `false` = Update.
    pub fn insert_publication(&mut self, data: PublicationBuiltinTopicData, now: Duration) -> bool {
        let key = data.key;
        let prefix = key.prefix;
        if let Some(existing) = self.publications.get_mut(&key) {
            // Update-Pfad: Index auf neuen `discovered_at` nachziehen.
            let old_entry = (existing.discovered_at, key);
            if let Some(set) = self.pub_index.get_mut(&prefix) {
                set.remove(&old_entry);
                set.insert((now, key));
            }
            existing.data = data;
            existing.discovered_at = now;
            return false;
        }
        // Cap-Check + LRU-Eviction via Sekundaer-Index — O(log n) statt
        // frueheres O(N) (F3-Fix).
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

    /// Insert oder Update einer Subscription. Semantik analog
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

    /// Entfernt eine Publication per GUID (z.B. nach expliziter
    /// Withdrawal-Notification vom Remote).
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

    /// Entfernt eine Subscription per GUID.
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

    /// Entfernt alle Publications und Subscriptions eines Remote-
    /// Participants. Aufruf z.B. bei SPDP-Lease-Timeout.
    ///
    /// Rueckgabe: (removed_pubs, removed_subs).
    pub fn on_participant_lost(&mut self, prefix: GuidPrefix) -> (usize, usize) {
        // Sekundaer-Index hat die exakte Key-Liste pro prefix — O(k)
        // statt O(N)-retain.
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

    /// Findet alle Publications, die zu einem `(topic_name, type_name)`
    /// passen — fuer Reader-seitiges Matching (T4/T5).
    pub fn match_publications<'a>(
        &'a self,
        topic: &'a str,
        type_name: &'a str,
    ) -> impl Iterator<Item = &'a DiscoveredPublication> + 'a {
        self.publications
            .values()
            .filter(move |p| p.data.topic_name == topic && p.data.type_name == type_name)
    }

    /// Findet alle Subscriptions, die zu einem `(topic_name, type_name)`
    /// passen — fuer Writer-seitiges Matching.
    pub fn match_subscriptions<'a>(
        &'a self,
        topic: &'a str,
        type_name: &'a str,
    ) -> impl Iterator<Item = &'a DiscoveredSubscription> + 'a {
        self.subscriptions
            .values()
            .filter(move |s| s.data.topic_name == topic && s.data.type_name == type_name)
    }

    /// Sammelt die GUIDs aller Publications (fuer Snapshot-Tests /
    /// Diagnose).
    #[must_use]
    pub fn publication_keys(&self) -> Vec<Guid> {
        self.publications.keys().copied().collect()
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
        }
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
        // 3 Publications vom gleichen Participant, je 1 Sekunde Abstand.
        c.insert_publication(pub_data([1; 12], [1, 0, 0], "A"), Duration::from_secs(1));
        c.insert_publication(pub_data([1; 12], [2, 0, 0], "B"), Duration::from_secs(2));
        c.insert_publication(pub_data([1; 12], [3, 0, 0], "C"), Duration::from_secs(3));
        // Aelteste (A) raus, B+C drin
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
        // Pro Participant 1 Publication — insgesamt 3 Participants, also
        // 3 Publications insgesamt, keine Eviction.
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
        // Reinsert gleicher GUID darf keine Eviction triggern
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
