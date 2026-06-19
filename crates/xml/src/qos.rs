// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data model for the DDS-XML 1.0 §7.3.2 QoS profile library.
//!
//! Spec source: OMG DDS-XML 1.0 §7.3.2 (QoS Library Building Block) +
//! DDS 1.4 §2.2.3 (22 standard QoS policies).
//!
//! # Layer discipline
//!
//! The individual policy structures (`DurabilityQosPolicy`, …) and the
//! aggregate containers (`WriterQos`, `ReaderQos`) live in the crate
//! [`zerodds_qos`] (wire-format source). `zerodds-xml` reuses them here
//! directly — *no duplicates*.
//!
//! The XML-specific containers [`EntityQos`] (six variants
//! for DataWriter/DataReader/Topic/Publisher/Subscriber/
//! DomainParticipant) carry an `Option<…>` per policy. `None` =
//! "spec default" (taken from `zerodds-qos::*::Default`); `Some(p)` =
//! explicitly set in the XML. The override semantics of inheritance
//! (see [`crate::qos_inheritance`]) operate on these options:
//! a child profile with `Some(p)` overrides the inherited `Some/None`,
//! a child profile with `None` inherits unchanged.
//!
//! # XML element to Rust type mapping (DDS-XML 1.0 §7.3.2 + DDS 1.4 §2.2.3)
//!
//! ```text
//! XML element                            | Rust type
//! ---------------------------------------+----------------------------------
//! <qos_library name=…>                   | QosLibrary
//! <qos_profile name=… base_name=…>       | QosProfile
//! <topic_filter>                         | QosProfile.topic_filter (String)
//! <datawriter_qos>                       | EntityQos (DataWriter)
//! <datareader_qos>                       | EntityQos (DataReader)
//! <topic_qos>                            | EntityQos (Topic)
//! <publisher_qos>                        | EntityQos (Publisher)
//! <subscriber_qos>                       | EntityQos (Subscriber)
//! <domainparticipant_qos>                | EntityQos (DomainParticipant)
//! <durability><kind>                     | DurabilityQosPolicy
//! <durability_service>                   | DurabilityServiceQosPolicy
//! <presentation>                         | PresentationQosPolicy
//! <deadline><period>                     | DeadlineQosPolicy
//! <latency_budget><duration>             | LatencyBudgetQosPolicy
//! <ownership><kind>                      | OwnershipQosPolicy
//! <ownership_strength><value>            | OwnershipStrengthQosPolicy
//! <liveliness><kind><lease_duration>     | LivelinessQosPolicy
//! <time_based_filter><minimum_separation>| TimeBasedFilterQosPolicy
//! <partition><name>g1</name>…            | PartitionQosPolicy
//! <reliability><kind><max_blocking_time> | ReliabilityQosPolicy
//! <transport_priority><value>            | TransportPriorityQosPolicy
//! <lifespan><duration>                   | LifespanQosPolicy
//! <destination_order><kind>              | DestinationOrderQosPolicy
//! <history><kind><depth>                 | HistoryQosPolicy
//! <resource_limits>…                     | ResourceLimitsQosPolicy
//! <entity_factory><autoenable_…>         | EntityFactoryQosPolicy
//! <writer_data_lifecycle>                | WriterDataLifecycleQosPolicy
//! <reader_data_lifecycle>                | ReaderDataLifecycleQosPolicy
//! <user_data><value>BASE64</value>       | UserDataQosPolicy
//! <topic_data><value>BASE64</value>      | TopicDataQosPolicy
//! <group_data><value>BASE64</value>      | GroupDataQosPolicy
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderQosPolicy, DurabilityQosPolicy, DurabilityServiceQosPolicy,
    EntityFactoryQosPolicy, GroupDataQosPolicy, HistoryQosPolicy, LatencyBudgetQosPolicy,
    LifespanQosPolicy, LivelinessQosPolicy, OwnershipQosPolicy, OwnershipStrengthQosPolicy,
    PartitionQosPolicy, PresentationQosPolicy, ReaderDataLifecycleQosPolicy, ReaderQos,
    ReliabilityQosPolicy, ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy, WriterQos,
};

/// Container for 1+ QoS profiles per DDS-XML 1.0 §7.3.2.
///
/// Multiple libraries may appear per document; each library carries
/// a `name` attribute. Lookups across libraries happen via the
/// 2-segment path `library::profile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosLibrary {
    /// Name of the library (`<qos_library name="…">` attribute).
    pub name: String,
    /// Profiles in this library, in document order.
    pub profiles: Vec<QosProfile>,
}

impl QosLibrary {
    /// Looks up a profile within this library by name.
    #[must_use]
    pub fn profile(&self, name: &str) -> Option<&QosProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }
}

/// A single `<qos_profile>` element (§7.3.2.4).
///
/// Each `EntityQos` container is `Option<…>` — `None` means "not listed
/// in the XML", which on resolve transitions to the spec-default aggregate
/// type (see [`crate::qos_inheritance::resolve_profile`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QosProfile {
    /// Name of the profile (`name` attribute).
    pub name: String,
    /// Optional `base_name` for inheritance (Spec §7.3.2.4.2). Format:
    /// either `"profile"` (within the same library) or
    /// `"library::profile"` (cross-library reference).
    pub base_name: Option<String>,
    /// Optional topic filter (glob, `*`/`?`) for profile-to-topic
    /// binding. Multiple `<topic_filter>` are collapsed into **one** pattern
    /// (last wins) — the spec only meaningfully covers one filter
    /// per profile.
    pub topic_filter: Option<String>,
    /// `<datawriter_qos>` container.
    pub datawriter_qos: Option<EntityQos>,
    /// `<datareader_qos>` container.
    pub datareader_qos: Option<EntityQos>,
    /// `<topic_qos>` container.
    pub topic_qos: Option<EntityQos>,
    /// `<publisher_qos>` container.
    pub publisher_qos: Option<EntityQos>,
    /// `<subscriber_qos>` container.
    pub subscriber_qos: Option<EntityQos>,
    /// `<domainparticipant_qos>` container.
    pub domainparticipant_qos: Option<EntityQos>,
}

/// Optional-per-policy container for all 22 OMG-DDS-1.4 QoS policies.
///
/// An unset `Option` means: not listed in the XML → spec
/// default on resolve. *Reused as one container for all 6 entity types* —
/// DDS-XML 1.0 in principle allows all policies in
/// all 6 containers; semantic filtering (e.g. that
/// `OwnershipStrength` only makes sense on the writer) happens when
/// materializing into `WriterQos`/`ReaderQos` (see [`Self::into_writer_qos`] /
/// [`Self::into_reader_qos`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityQos {
    /// `<durability>` (DDS 1.4 §2.2.3.4).
    pub durability: Option<DurabilityQosPolicy>,
    /// `<durability_service>` (§2.2.3.5).
    pub durability_service: Option<DurabilityServiceQosPolicy>,
    /// `<presentation>` (§2.2.3.6).
    pub presentation: Option<PresentationQosPolicy>,
    /// `<deadline>` (§2.2.3.7).
    pub deadline: Option<DeadlineQosPolicy>,
    /// `<latency_budget>` (§2.2.3.10).
    pub latency_budget: Option<LatencyBudgetQosPolicy>,
    /// `<ownership>` (§2.2.3.23).
    pub ownership: Option<OwnershipQosPolicy>,
    /// `<ownership_strength>` (§2.2.3.24).
    pub ownership_strength: Option<OwnershipStrengthQosPolicy>,
    /// `<liveliness>` (§2.2.3.11).
    pub liveliness: Option<LivelinessQosPolicy>,
    /// `<time_based_filter>` (§2.2.3.12).
    pub time_based_filter: Option<TimeBasedFilterQosPolicy>,
    /// `<partition>` (§2.2.3.13).
    pub partition: Option<PartitionQosPolicy>,
    /// `<reliability>` (§2.2.3.14).
    pub reliability: Option<ReliabilityQosPolicy>,
    /// `<transport_priority>` (§2.2.3.15).
    pub transport_priority: Option<TransportPriorityQosPolicy>,
    /// `<lifespan>` (§2.2.3.16).
    pub lifespan: Option<LifespanQosPolicy>,
    /// `<destination_order>` (§2.2.3.18).
    pub destination_order: Option<DestinationOrderQosPolicy>,
    /// `<history>` (§2.2.3.17).
    pub history: Option<HistoryQosPolicy>,
    /// `<resource_limits>` (§2.2.3.19).
    pub resource_limits: Option<ResourceLimitsQosPolicy>,
    /// `<entity_factory>` (§2.2.3.27).
    pub entity_factory: Option<EntityFactoryQosPolicy>,
    /// `<writer_data_lifecycle>` (§2.2.3.21).
    pub writer_data_lifecycle: Option<WriterDataLifecycleQosPolicy>,
    /// `<reader_data_lifecycle>` (§2.2.3.20).
    pub reader_data_lifecycle: Option<ReaderDataLifecycleQosPolicy>,
    /// `<user_data>` (§2.2.3.1).
    pub user_data: Option<UserDataQosPolicy>,
    /// `<topic_data>` (§2.2.3.2).
    pub topic_data: Option<TopicDataQosPolicy>,
    /// `<group_data>` (§2.2.3.3).
    pub group_data: Option<GroupDataQosPolicy>,
}

impl EntityQos {
    /// Merges `override_` over `self`: each explicitly set policy in
    /// `override_` overrides the one in `self`. The operation is **not**
    /// commutative — order: `parent.merge(child)` means
    /// "child wins where the child explicitly set a value".
    #[must_use]
    pub fn merge(mut self, override_: &Self) -> Self {
        macro_rules! merge_field {
            ($field:ident) => {
                if let Some(v) = override_.$field.clone() {
                    self.$field = Some(v);
                }
            };
        }
        merge_field!(durability);
        merge_field!(durability_service);
        merge_field!(presentation);
        merge_field!(deadline);
        merge_field!(latency_budget);
        merge_field!(ownership);
        merge_field!(ownership_strength);
        merge_field!(liveliness);
        merge_field!(time_based_filter);
        merge_field!(partition);
        merge_field!(reliability);
        merge_field!(transport_priority);
        merge_field!(lifespan);
        merge_field!(destination_order);
        merge_field!(history);
        merge_field!(resource_limits);
        merge_field!(entity_factory);
        merge_field!(writer_data_lifecycle);
        merge_field!(reader_data_lifecycle);
        merge_field!(user_data);
        merge_field!(topic_data);
        merge_field!(group_data);
        self
    }

    /// Materializes a `WriterQos` from the policies set in the `EntityQos`.
    /// `None` entries are filled with the spec defaults from
    /// `zerodds_qos::WriterQos::default()`.
    ///
    /// Policies that make no sense for a writer (e.g.
    /// `time_based_filter`, `reader_data_lifecycle`) are
    /// deliberately ignored here — they are only materialized for ReaderQos.
    #[must_use]
    pub fn into_writer_qos(&self) -> WriterQos {
        let mut q = WriterQos::default();
        if let Some(p) = self.durability {
            q.durability = p;
        }
        if let Some(p) = self.durability_service {
            q.durability_service = p;
        }
        if let Some(p) = self.deadline {
            q.deadline = p;
        }
        if let Some(p) = self.latency_budget {
            q.latency_budget = p;
        }
        if let Some(p) = self.liveliness {
            q.liveliness = p;
        }
        if let Some(p) = self.reliability {
            q.reliability = p;
        }
        if let Some(p) = self.destination_order {
            q.destination_order = p;
        }
        if let Some(p) = self.history {
            q.history = p;
        }
        if let Some(p) = self.resource_limits {
            q.resource_limits = p;
        }
        if let Some(p) = self.transport_priority {
            q.transport_priority = p;
        }
        if let Some(p) = self.lifespan {
            q.lifespan = p;
        }
        if let Some(p) = self.ownership {
            q.ownership = p;
        }
        if let Some(p) = self.ownership_strength {
            q.ownership_strength = p;
        }
        if let Some(p) = self.presentation {
            q.presentation = p;
        }
        if let Some(p) = self.partition.clone() {
            q.partition = p;
        }
        if let Some(p) = self.writer_data_lifecycle {
            q.writer_data_lifecycle = p;
        }
        if let Some(p) = self.user_data.clone() {
            q.user_data = p;
        }
        if let Some(p) = self.topic_data.clone() {
            q.topic_data = p;
        }
        if let Some(p) = self.group_data.clone() {
            q.group_data = p;
        }
        q
    }

    /// Materializes a `ReaderQos` analogous to [`Self::into_writer_qos`].
    #[must_use]
    pub fn into_reader_qos(&self) -> ReaderQos {
        let mut q = ReaderQos::default();
        if let Some(p) = self.durability {
            q.durability = p;
        }
        if let Some(p) = self.deadline {
            q.deadline = p;
        }
        if let Some(p) = self.latency_budget {
            q.latency_budget = p;
        }
        if let Some(p) = self.liveliness {
            q.liveliness = p;
        }
        if let Some(p) = self.reliability {
            q.reliability = p;
        }
        if let Some(p) = self.destination_order {
            q.destination_order = p;
        }
        if let Some(p) = self.history {
            q.history = p;
        }
        if let Some(p) = self.resource_limits {
            q.resource_limits = p;
        }
        if let Some(p) = self.ownership {
            q.ownership = p;
        }
        if let Some(p) = self.time_based_filter {
            q.time_based_filter = p;
        }
        if let Some(p) = self.presentation {
            q.presentation = p;
        }
        if let Some(p) = self.partition.clone() {
            q.partition = p;
        }
        if let Some(p) = self.reader_data_lifecycle {
            q.reader_data_lifecycle = p;
        }
        if let Some(p) = self.user_data.clone() {
            q.user_data = p;
        }
        if let Some(p) = self.topic_data.clone() {
            q.topic_data = p;
        }
        if let Some(p) = self.group_data.clone() {
            q.group_data = p;
        }
        q
    }
}

/// Topic filter match (DDS-XML 1.0 §7.3.2.5).
///
/// POSIX-fnmatch style with `*` (zero or more characters) and `?` (exactly
/// one character). Used to check whether a
/// `<topic_filter>foo_*</topic_filter>` covers a concrete topic name.
///
/// Implemented via dynamic programming (see the duplicate in
/// `crates/security-permissions/src/topic_match.rs` — we duplicate
/// deliberately, to avoid creating a `zerodds-xml → zerodds-security-permissions`
/// dependency).
#[must_use]
pub fn topic_filter_matches(filter: &str, topic_name: &str) -> bool {
    let p: Vec<char> = filter.chars().collect();
    let n: Vec<char> = topic_name.chars().collect();
    let (m, k) = (n.len(), p.len());
    let mut dp = alloc::vec![alloc::vec![false; k + 1]; m + 1];
    dp[0][0] = true;
    for j in 1..=k {
        if p[j - 1] == '*' {
            dp[0][j] = dp[0][j - 1];
        }
    }
    for i in 1..=m {
        for j in 1..=k {
            let pc = p[j - 1];
            dp[i][j] = if pc == '*' {
                dp[i - 1][j] || dp[i][j - 1]
            } else if pc == '?' || pc == n[i - 1] {
                dp[i - 1][j - 1]
            } else {
                false
            };
        }
    }
    dp[m][k]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_qos::{DurabilityKind, ReliabilityKind};

    #[test]
    fn entity_qos_default_is_all_none() {
        let e = EntityQos::default();
        assert!(e.durability.is_none());
        assert!(e.reliability.is_none());
        assert!(e.history.is_none());
        assert!(e.partition.is_none());
        assert!(e.user_data.is_none());
    }

    #[test]
    fn entity_qos_into_writer_uses_defaults_for_unset() {
        let e = EntityQos::default();
        let wq = e.into_writer_qos();
        // Writer-Default: Reliable.
        assert_eq!(wq.reliability.kind, ReliabilityKind::Reliable);
    }

    #[test]
    fn entity_qos_into_reader_uses_defaults_for_unset() {
        let e = EntityQos::default();
        let rq = e.into_reader_qos();
        // Reader-Default: BestEffort.
        assert_eq!(rq.reliability.kind, ReliabilityKind::BestEffort);
    }

    #[test]
    fn merge_override_replaces_field() {
        let parent = EntityQos {
            durability: Some(DurabilityQosPolicy {
                kind: DurabilityKind::Volatile,
            }),
            history: Some(HistoryQosPolicy::default()),
            ..Default::default()
        };
        let child = EntityQos {
            durability: Some(DurabilityQosPolicy {
                kind: DurabilityKind::Persistent,
            }),
            ..Default::default()
        };

        let merged = parent.merge(&child);
        assert_eq!(
            merged.durability.unwrap().kind,
            DurabilityKind::Persistent,
            "child override should win"
        );
        assert!(merged.history.is_some(), "parent's history should be kept");
    }

    #[test]
    fn merge_none_does_not_clobber() {
        let parent = EntityQos {
            deadline: Some(DeadlineQosPolicy::default()),
            ..Default::default()
        };
        let child = EntityQos::default();
        let merged = parent.merge(&child);
        assert!(
            merged.deadline.is_some(),
            "child=None should not clobber parent"
        );
    }

    #[test]
    fn library_profile_lookup() {
        let lib = QosLibrary {
            name: "L".into(),
            profiles: alloc::vec![
                QosProfile {
                    name: "A".into(),
                    ..Default::default()
                },
                QosProfile {
                    name: "B".into(),
                    ..Default::default()
                }
            ],
        };
        assert!(lib.profile("A").is_some());
        assert!(lib.profile("B").is_some());
        assert!(lib.profile("Missing").is_none());
    }

    // ---- Topic-Filter-Match ------------------------------------------

    #[test]
    fn glob_star_matches_all() {
        assert!(topic_filter_matches("*", "foo"));
        assert!(topic_filter_matches("*", ""));
    }

    #[test]
    fn glob_prefix() {
        assert!(topic_filter_matches("foo_*", "foo_bar"));
        assert!(topic_filter_matches("foo_*", "foo_"));
        assert!(!topic_filter_matches("foo_*", "bar_foo"));
    }

    #[test]
    fn glob_question_mark() {
        assert!(topic_filter_matches("s?nsor", "sensor"));
        assert!(!topic_filter_matches("s?nsor", "snsor"));
        assert!(!topic_filter_matches("s?nsor", "sennsor"));
    }

    #[test]
    fn glob_exact() {
        assert!(topic_filter_matches("Chatter", "Chatter"));
        assert!(!topic_filter_matches("Chatter", "ChatterX"));
    }
}
