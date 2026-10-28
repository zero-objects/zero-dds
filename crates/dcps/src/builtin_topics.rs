// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Built-in-topic data types — DCPS API view (DDS 1.4 §2.2.5).
//!
//! The spec defines four "built-in topics":
//!
//! | Topic name        | Sample type                      | Source |
//! |-------------------|----------------------------------|--------|
//! | `DCPSParticipant` | [`ParticipantBuiltinTopicData`]  | SPDP   |
//! | `DCPSTopic`       | [`TopicBuiltinTopicData`]        | SEDP topic announce (RTPS 2.5 §9.6.2.2.4) |
//! | `DCPSPublication` | [`PublicationBuiltinTopicData`]  | SEDP pub announce |
//! | `DCPSSubscription`| [`SubscriptionBuiltinTopicData`] | SEDP sub announce |
//!
//! These DCPS structs are **application-friendly**: fixed fields, no
//! PL_CDR wire format. The wire decoders for pub/sub/participant from
//! the `zerodds-rtps` crate are called by the runtime and the result is
//! converted into the DCPS types defined here (see the `From` impls
//! further below) before they are delivered to user code via the
//! built-in subscriber.
//!
//! # `DdsType` implementation
//!
//! Since the built-in topics are also emitted via `DataReader::take()`,
//! they need a `DdsType` impl. We use a minimal internal encoding
//! (ZeroDDS-internal PL_CDR_LE) that is roundtrip-safe — the built-in
//! data is never transmitted "on the wire" to another vendor, so this
//! suffices.
//!
//! Spec references:
//! - DDS-DCPS 1.4 §2.2.5 (Built-in Topics)
//! - DDSI-RTPS 2.5 §8.5.4 (SEDP Built-in Endpoints)

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::dds_type::{DdsType, DecodeError, EncodeError};

use zerodds_rtps::participant_data as wire_part;
use zerodds_rtps::publication_data as wire_pub;
use zerodds_rtps::subscription_data as wire_sub;
use zerodds_rtps::wire_types::Guid;

// ---------------------------------------------------------------------
// Topic names (spec §2.2.5).
// ---------------------------------------------------------------------

/// Topic name `"DCPSParticipant"` (spec §2.2.5.1).
pub const TOPIC_NAME_DCPS_PARTICIPANT: &str = "DCPSParticipant";
/// Topic name `"DCPSTopic"` (spec §2.2.5.2).
pub const TOPIC_NAME_DCPS_TOPIC: &str = "DCPSTopic";
/// Topic name `"DCPSPublication"` (spec §2.2.5.3).
pub const TOPIC_NAME_DCPS_PUBLICATION: &str = "DCPSPublication";
/// Topic name `"DCPSSubscription"` (spec §2.2.5.4).
pub const TOPIC_NAME_DCPS_SUBSCRIPTION: &str = "DCPSSubscription";

/// Topic name `"DCPSParticipantsSecure"` (DDS-Security 1.2 §7.5.1.6).
pub const TOPIC_NAME_DCPS_PARTICIPANTS_SECURE: &str = "DCPSParticipantsSecure";
/// Topic name `"DCPSPublicationsSecure"` (DDS-Security 1.2 §7.5.1.7).
pub const TOPIC_NAME_DCPS_PUBLICATIONS_SECURE: &str = "DCPSPublicationsSecure";
/// Topic name `"DCPSSubscriptionsSecure"` (DDS-Security 1.2 §7.5.1.8).
pub const TOPIC_NAME_DCPS_SUBSCRIPTIONS_SECURE: &str = "DCPSSubscriptionsSecure";

/// The key GUID of a built-in instance, from the sixteen octets its key
/// holder is. XTypes 1.3 §7.6.8 step 4 writes the holder with no length and
/// no alignment, so there is nothing else in it to skip.
fn builtin_key_from_holder(bytes: &[u8]) -> core::result::Result<Guid, DecodeError> {
    let key: [u8; 16] =
        bytes
            .get(..16)
            .and_then(|b| b.try_into().ok())
            .ok_or(DecodeError::Invalid {
                what: "truncated built-in topic key holder",
            })?;
    Ok(Guid::from_bytes(key))
}

// ---------------------------------------------------------------------
// 1) DCPSParticipant
// ---------------------------------------------------------------------

/// Sample type of the `DCPSParticipant` built-in topic (DDS 1.4 §2.2.5.1).
///
/// Represents a discovered remote `DomainParticipant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantBuiltinTopicData {
    /// Stable identifier of the participant (16-byte GUID; corresponds
    /// to the spec's `BuiltinTopicKey_t` — for us these are the last
    /// 16 bytes of the RTPS GUID).
    pub key: Guid,
    /// USER_DATA QoS bytes (spec §2.2.5.1: `user_data`). Optional in the
    /// SPDP beacon — filled from `DomainParticipantQos::user_data`,
    /// wire-encoded as PID_USER_DATA (DDSI-RTPS §9.6.3.2).
    pub user_data: Vec<u8>,
}

/// The values of a fresh instance: the unknown GUID and no USER_DATA.
impl Default for ParticipantBuiltinTopicData {
    fn default() -> Self {
        Self {
            key: Guid::UNKNOWN,
            user_data: Vec::new(),
        }
    }
}

impl ParticipantBuiltinTopicData {
    /// Constructs from the wire data type (`zerodds-rtps`).
    #[must_use]
    pub fn from_wire(w: &wire_part::ParticipantBuiltinTopicData) -> Self {
        Self {
            key: w.guid,
            user_data: w.user_data.clone(),
        }
    }
}

impl DdsType for ParticipantBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::ParticipantBuiltinTopicData";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.key.to_bytes());
        encode_bytes_le(&self.user_data, out)
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let key = r.read_guid()?;
        let user_data = r.read_bytes()?;
        Ok(Self { key, user_data })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }

    /// The inverse. The holder is the sixteen octets of the key GUID and
    /// nothing else — no length and no alignment — so it is read as those
    /// octets rather than through `Self::decode`, which reads the type's own
    /// stream. DDS 1.4 §2.2.2.5.1.13: on the dispose of a built-in instance
    /// only the key is meaningful, and the members outside it take the values
    /// a fresh instance has.
    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let key = builtin_key_from_holder(bytes)?;
        Ok(Self {
            key,
            user_data: Vec::new(),
        })
    }
}

/// One QoS policy out of a builtin-topic sample's stream, a failure there the
/// sample's.
macro_rules! policy {
    ($r:ident, $policy:ident) => {
        zerodds_qos::$policy::decode_from(&mut $r).map_err(decode_failed)?
    };
}

// ---------------------------------------------------------------------
// 2) DCPSTopic
// ---------------------------------------------------------------------

/// Sample type of the `DCPSTopic` built-in topic, with every member DDS 1.4
/// §2.2.5 lists for the TopicBuiltinTopicData.
///
/// Two sources fill it. A peer that has the SEDP topics endpoints
/// (DDSI-RTPS 2.5 §8.5.4.2) announces each topic it created with all of
/// them ([`Self::from_wire`]). For a peer without them the sample is
/// derived from its endpoint announcements, which carry neither HISTORY,
/// RESOURCE_LIMITS nor TRANSPORT_PRIORITY: those three take their default
/// there ([`Self::from_publication`], [`Self::from_subscription`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicBuiltinTopicData {
    /// Stable identity of the topic entry (synthetic, from a hash of the
    /// participant and the topic and type name; see
    /// [`Self::synthesize_key`]): one instance per participant, topic and
    /// type.
    pub key: Guid,
    /// Topic name (e.g. `"ChatterTopic"`).
    pub name: String,
    /// IDL type name.
    pub type_name: String,
    /// DURABILITY.
    pub durability: zerodds_qos::DurabilityQosPolicy,
    /// DURABILITY_SERVICE.
    pub durability_service: zerodds_qos::DurabilityServiceQosPolicy,
    /// DEADLINE.
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// LATENCY_BUDGET.
    pub latency_budget: zerodds_qos::LatencyBudgetQosPolicy,
    /// LIVELINESS.
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// RELIABILITY.
    pub reliability: zerodds_qos::ReliabilityQosPolicy,
    /// TRANSPORT_PRIORITY.
    pub transport_priority: zerodds_qos::TransportPriorityQosPolicy,
    /// LIFESPAN.
    pub lifespan: zerodds_qos::LifespanQosPolicy,
    /// DESTINATION_ORDER.
    pub destination_order: zerodds_qos::DestinationOrderQosPolicy,
    /// HISTORY.
    pub history: zerodds_qos::HistoryQosPolicy,
    /// RESOURCE_LIMITS.
    pub resource_limits: zerodds_qos::ResourceLimitsQosPolicy,
    /// OWNERSHIP.
    pub ownership: zerodds_qos::OwnershipQosPolicy,
    /// TOPIC_DATA.
    pub topic_data: zerodds_qos::TopicDataQosPolicy,
}

/// The values of a fresh instance: the unknown GUID, empty names and the
/// default of every policy.
impl Default for TopicBuiltinTopicData {
    fn default() -> Self {
        Self {
            key: Guid::UNKNOWN,
            name: String::new(),
            type_name: String::new(),
            durability: Default::default(),
            durability_service: Default::default(),
            deadline: Default::default(),
            latency_budget: Default::default(),
            liveliness: Default::default(),
            reliability: Default::default(),
            transport_priority: Default::default(),
            lifespan: Default::default(),
            destination_order: Default::default(),
            history: Default::default(),
            resource_limits: Default::default(),
            ownership: Default::default(),
            topic_data: Default::default(),
        }
    }
}

impl TopicBuiltinTopicData {
    /// Produces a synthetic GUID key from the participant, the topic name and
    /// the type name. Stable: the same participant with the same topic and
    /// type yields the same key. The entry of a Topic is one per participant
    /// (DDS 1.4 §2.2.5: "entry created when a Topic object is created",
    /// "key … to distinguish entries"): the participants that create the same
    /// topic and type are two entries, an announcement of the topic and the
    /// endpoints of a participant on it one (`DCPSTopic` delivers idempotent
    /// updates, not an insert per endpoint, spec §2.2.5.2).
    #[must_use]
    pub fn synthesize_key(
        participant: zerodds_rtps::wire_types::GuidPrefix,
        topic: &str,
        type_name: &str,
    ) -> Guid {
        // FNV-1a 64-bit, twice — deterministic, no_std, sufficiently
        // collision-safe for a few thousand topics.
        let h0 = fnv1a64(&participant.to_bytes(), 0xcbf2_9ce4_8422_2325);
        let h1 = fnv1a64(topic.as_bytes(), h0);
        let h2 = fnv1a64(type_name.as_bytes(), h1);
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&h1.to_le_bytes());
        bytes[8..16].copy_from_slice(&h2.to_le_bytes());
        Guid::from_bytes(bytes)
    }

    /// Constructs from a topic a peer announced over its SEDP topics
    /// writer. The key is this topic's synthetic one of the announcing
    /// participant ([`Self::synthesize_key`]), not the announced topic GUID:
    /// the endpoints of that participant on the topic are the same entry.
    #[must_use]
    pub fn from_wire(w: &zerodds_rtps::topic_data::TopicBuiltinTopicData) -> Self {
        Self {
            key: Self::synthesize_key(w.key.prefix, &w.topic_name, &w.type_name),
            name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: w.durability,
            durability_service: w.durability_service,
            deadline: w.deadline,
            latency_budget: w.latency_budget,
            liveliness: w.liveliness,
            reliability: w.reliability,
            transport_priority: w.transport_priority,
            lifespan: w.lifespan,
            destination_order: w.destination_order,
            history: w.history,
            resource_limits: w.resource_limits,
            ownership: w.ownership,
            topic_data: w.topic_data.clone(),
        }
    }

    /// Constructs from a discovered publication wire data type.
    #[must_use]
    pub fn from_publication(w: &wire_pub::PublicationBuiltinTopicData) -> Self {
        Self {
            key: Self::synthesize_key(w.participant_key.prefix, &w.topic_name, &w.type_name),
            name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: zerodds_qos::DurabilityQosPolicy { kind: w.durability },
            durability_service: w.durability_service,
            deadline: w.deadline,
            latency_budget: w.latency_budget,
            liveliness: w.liveliness,
            reliability: w.reliability,
            lifespan: w.lifespan,
            destination_order: w.destination_order,
            ownership: zerodds_qos::OwnershipQosPolicy { kind: w.ownership },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: w.topic_data.clone(),
            },
            ..Self::default()
        }
    }

    /// Constructs from a discovered subscription wire data type.
    #[must_use]
    pub fn from_subscription(w: &wire_sub::SubscriptionBuiltinTopicData) -> Self {
        Self {
            key: Self::synthesize_key(w.participant_key.prefix, &w.topic_name, &w.type_name),
            name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: zerodds_qos::DurabilityQosPolicy { kind: w.durability },
            deadline: w.deadline,
            latency_budget: w.latency_budget,
            liveliness: w.liveliness,
            reliability: w.reliability,
            destination_order: w.destination_order,
            ownership: zerodds_qos::OwnershipQosPolicy { kind: w.ownership },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: w.topic_data.clone(),
            },
            ..Self::default()
        }
    }
}

impl DdsType for TopicBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::TopicBuiltinTopicData";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
        w.write_bytes(&self.key.to_bytes());
        w.write_string(&self.name);
        w.write_string(&self.type_name);
        self.durability.encode_into(&mut w);
        self.durability_service.encode_into(&mut w);
        self.deadline.encode_into(&mut w);
        self.latency_budget.encode_into(&mut w);
        self.liveliness.encode_into(&mut w);
        self.reliability.encode_into(&mut w);
        self.transport_priority.encode_into(&mut w);
        self.lifespan.encode_into(&mut w);
        self.destination_order.encode_into(&mut w);
        self.history.encode_into(&mut w);
        self.resource_limits.encode_into(&mut w);
        self.ownership.encode_into(&mut w);
        self.topic_data.encode_into(&mut w).map_err(encode_failed)?;
        out.extend_from_slice(&w.finish());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = zerodds_wire::Reader::new(bytes, zerodds_wire::Endianness::Little);
        Ok(Self {
            key: read_guid(&mut r)?,
            name: r.read_string().map_err(decode_failed)?,
            type_name: r.read_string().map_err(decode_failed)?,
            durability: policy!(r, DurabilityQosPolicy),
            durability_service: policy!(r, DurabilityServiceQosPolicy),
            deadline: policy!(r, DeadlineQosPolicy),
            latency_budget: policy!(r, LatencyBudgetQosPolicy),
            liveliness: policy!(r, LivelinessQosPolicy),
            reliability: policy!(r, ReliabilityQosPolicy),
            transport_priority: policy!(r, TransportPriorityQosPolicy),
            lifespan: policy!(r, LifespanQosPolicy),
            destination_order: policy!(r, DestinationOrderQosPolicy),
            history: policy!(r, HistoryQosPolicy),
            resource_limits: policy!(r, ResourceLimitsQosPolicy),
            ownership: policy!(r, OwnershipQosPolicy),
            topic_data: policy!(r, TopicDataQosPolicy),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }

    /// The inverse. The holder is the sixteen octets of the key GUID and
    /// nothing else — no length and no alignment — so it is read as those
    /// octets rather than through `Self::decode`, which reads the type's own
    /// stream. DDS 1.4 §2.2.2.5.1.13: on the dispose of a built-in instance
    /// only the key is meaningful, and the members outside it take the values
    /// a fresh instance has.
    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            key: builtin_key_from_holder(bytes)?,
            ..Self::default()
        })
    }
}

// ---------------------------------------------------------------------
// 3) DCPSPublication
// ---------------------------------------------------------------------

/// Sample type of the `DCPSPublication` built-in topic (DDS 1.4 §2.2.5.3).
///
/// Represents a discovered `DataWriter`, with every member DDS 1.4 §2.2.5
/// lists for the PublicationBuiltinTopicData, each as its QoS policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationBuiltinTopicData {
    /// Identity of the endpoint (= writer GUID).
    pub key: Guid,
    /// GUID of the participant that owns the writer.
    pub participant_key: Guid,
    /// Topic name.
    pub topic_name: String,
    /// IDL type name.
    pub type_name: String,
    /// DURABILITY.
    pub durability: zerodds_qos::DurabilityQosPolicy,
    /// DURABILITY_SERVICE (Spec §2.2.3.5) — configures a persistence
    /// service. A standalone Durability-Service daemon (ADR 0009) reads
    /// this to derive its retention contract for a TRANSIENT/PERSISTENT
    /// writer, via `zerodds_durability_store::Contract::from_qos`.
    pub durability_service: zerodds_qos::DurabilityServiceQosPolicy,
    /// DEADLINE.
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// LATENCY_BUDGET.
    pub latency_budget: zerodds_qos::LatencyBudgetQosPolicy,
    /// LIVELINESS.
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// RELIABILITY.
    pub reliability: zerodds_qos::ReliabilityQosPolicy,
    /// LIFESPAN.
    pub lifespan: zerodds_qos::LifespanQosPolicy,
    /// USER_DATA.
    pub user_data: zerodds_qos::UserDataQosPolicy,
    /// OWNERSHIP.
    pub ownership: zerodds_qos::OwnershipQosPolicy,
    /// OWNERSHIP_STRENGTH.
    pub ownership_strength: zerodds_qos::OwnershipStrengthQosPolicy,
    /// DESTINATION_ORDER.
    pub destination_order: zerodds_qos::DestinationOrderQosPolicy,
    /// PRESENTATION of the writer's publisher.
    pub presentation: zerodds_qos::PresentationQosPolicy,
    /// PARTITION of the writer's publisher.
    pub partition: zerodds_qos::PartitionQosPolicy,
    /// TOPIC_DATA of the writer's topic.
    pub topic_data: zerodds_qos::TopicDataQosPolicy,
    /// GROUP_DATA of the writer's publisher.
    pub group_data: zerodds_qos::GroupDataQosPolicy,
    /// DataTags of the writer (DDS-Security 1.2 §9.7.2, `PID_DATA_TAGS`):
    /// the tags its DataWriterQos carries, as the discovery announced them.
    pub data_tags: zerodds_qos::DataTagQosPolicy,
}

/// The values of a fresh instance: the unknown GUID, empty names and the
/// default of every policy.
impl Default for PublicationBuiltinTopicData {
    fn default() -> Self {
        Self {
            key: Guid::UNKNOWN,
            participant_key: Guid::UNKNOWN,
            topic_name: String::new(),
            type_name: String::new(),
            durability: Default::default(),
            durability_service: Default::default(),
            deadline: Default::default(),
            latency_budget: Default::default(),
            liveliness: Default::default(),
            reliability: Default::default(),
            lifespan: Default::default(),
            user_data: Default::default(),
            ownership: Default::default(),
            ownership_strength: Default::default(),
            destination_order: Default::default(),
            presentation: Default::default(),
            partition: Default::default(),
            topic_data: Default::default(),
            group_data: Default::default(),
            data_tags: Default::default(),
        }
    }
}

impl PublicationBuiltinTopicData {
    /// Constructs from the wire data type.
    #[must_use]
    pub fn from_wire(w: &wire_pub::PublicationBuiltinTopicData) -> Self {
        Self {
            key: w.key,
            participant_key: w.participant_key,
            topic_name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: zerodds_qos::DurabilityQosPolicy { kind: w.durability },
            durability_service: w.durability_service,
            deadline: w.deadline,
            latency_budget: w.latency_budget,
            liveliness: w.liveliness,
            reliability: w.reliability,
            lifespan: w.lifespan,
            user_data: zerodds_qos::UserDataQosPolicy {
                value: w.user_data.clone(),
            },
            ownership: zerodds_qos::OwnershipQosPolicy { kind: w.ownership },
            ownership_strength: zerodds_qos::OwnershipStrengthQosPolicy {
                value: w.ownership_strength,
            },
            destination_order: w.destination_order,
            presentation: w.presentation,
            partition: zerodds_qos::PartitionQosPolicy {
                names: w.partition.clone(),
            },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: w.topic_data.clone(),
            },
            group_data: zerodds_qos::GroupDataQosPolicy {
                value: w.group_data.clone(),
            },
            data_tags: crate::qos::data_tags_from_announced(&w.data_tags),
        }
    }
}

impl DdsType for PublicationBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::PublicationBuiltinTopicData";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
        write_endpoint_names(
            &mut w,
            &self.key,
            &self.participant_key,
            &self.topic_name,
            &self.type_name,
        );
        self.durability.encode_into(&mut w);
        self.durability_service.encode_into(&mut w);
        self.deadline.encode_into(&mut w);
        self.latency_budget.encode_into(&mut w);
        self.liveliness.encode_into(&mut w);
        self.reliability.encode_into(&mut w);
        self.lifespan.encode_into(&mut w);
        self.user_data.encode_into(&mut w).map_err(encode_failed)?;
        self.ownership.encode_into(&mut w);
        self.ownership_strength.encode_into(&mut w);
        self.destination_order.encode_into(&mut w);
        self.presentation.encode_into(&mut w);
        self.partition.encode_into(&mut w).map_err(encode_failed)?;
        self.topic_data.encode_into(&mut w).map_err(encode_failed)?;
        self.group_data.encode_into(&mut w).map_err(encode_failed)?;
        self.data_tags.encode_into(&mut w).map_err(encode_failed)?;
        out.extend_from_slice(&w.finish());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = zerodds_wire::Reader::new(bytes, zerodds_wire::Endianness::Little);
        let (key, participant_key, topic_name, type_name) = read_endpoint_names(&mut r)?;
        Ok(Self {
            key,
            participant_key,
            topic_name,
            type_name,
            durability: policy!(r, DurabilityQosPolicy),
            durability_service: policy!(r, DurabilityServiceQosPolicy),
            deadline: policy!(r, DeadlineQosPolicy),
            latency_budget: policy!(r, LatencyBudgetQosPolicy),
            liveliness: policy!(r, LivelinessQosPolicy),
            reliability: policy!(r, ReliabilityQosPolicy),
            lifespan: policy!(r, LifespanQosPolicy),
            user_data: policy!(r, UserDataQosPolicy),
            ownership: policy!(r, OwnershipQosPolicy),
            ownership_strength: policy!(r, OwnershipStrengthQosPolicy),
            destination_order: policy!(r, DestinationOrderQosPolicy),
            presentation: policy!(r, PresentationQosPolicy),
            partition: policy!(r, PartitionQosPolicy),
            topic_data: policy!(r, TopicDataQosPolicy),
            group_data: policy!(r, GroupDataQosPolicy),
            data_tags: policy!(r, DataTagQosPolicy),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }

    /// The inverse. The holder is the sixteen octets of the key GUID and
    /// nothing else — no length and no alignment — so it is read as those
    /// octets rather than through `Self::decode`, which reads the type's own
    /// stream. DDS 1.4 §2.2.2.5.1.13: on the dispose of a built-in instance
    /// only the key is meaningful, and the members outside it take the values
    /// a fresh instance has.
    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            key: builtin_key_from_holder(bytes)?,
            ..Self::default()
        })
    }
}

// ---------------------------------------------------------------------
// 4) DCPSSubscription
// ---------------------------------------------------------------------

/// Sample type of the `DCPSSubscription` built-in topic (DDS 1.4 §2.2.5.4).
///
/// Represents a discovered `DataReader`, with every member DDS 1.4 §2.2.5
/// lists for the SubscriptionBuiltinTopicData, each as its QoS policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionBuiltinTopicData {
    /// Identity of the endpoint (= reader GUID).
    pub key: Guid,
    /// GUID of the participant that owns the reader.
    pub participant_key: Guid,
    /// Topic name.
    pub topic_name: String,
    /// IDL type name.
    pub type_name: String,
    /// DURABILITY.
    pub durability: zerodds_qos::DurabilityQosPolicy,
    /// DEADLINE.
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// LATENCY_BUDGET.
    pub latency_budget: zerodds_qos::LatencyBudgetQosPolicy,
    /// LIVELINESS.
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// RELIABILITY.
    pub reliability: zerodds_qos::ReliabilityQosPolicy,
    /// OWNERSHIP.
    pub ownership: zerodds_qos::OwnershipQosPolicy,
    /// DESTINATION_ORDER.
    pub destination_order: zerodds_qos::DestinationOrderQosPolicy,
    /// USER_DATA.
    pub user_data: zerodds_qos::UserDataQosPolicy,
    /// TIME_BASED_FILTER.
    pub time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy,
    /// PRESENTATION of the reader's subscriber.
    pub presentation: zerodds_qos::PresentationQosPolicy,
    /// PARTITION of the reader's subscriber.
    pub partition: zerodds_qos::PartitionQosPolicy,
    /// TOPIC_DATA of the reader's topic.
    pub topic_data: zerodds_qos::TopicDataQosPolicy,
    /// GROUP_DATA of the reader's subscriber.
    pub group_data: zerodds_qos::GroupDataQosPolicy,
    /// DataTags of the reader (DDS-Security 1.2 §9.7.2, `PID_DATA_TAGS`):
    /// the tags its DataReaderQos carries, as the discovery announced them.
    pub data_tags: zerodds_qos::DataTagQosPolicy,
}

/// The values of a fresh instance, as for [`PublicationBuiltinTopicData`].
impl Default for SubscriptionBuiltinTopicData {
    fn default() -> Self {
        Self {
            key: Guid::UNKNOWN,
            participant_key: Guid::UNKNOWN,
            topic_name: String::new(),
            type_name: String::new(),
            durability: Default::default(),
            deadline: Default::default(),
            latency_budget: Default::default(),
            liveliness: Default::default(),
            reliability: Default::default(),
            ownership: Default::default(),
            destination_order: Default::default(),
            user_data: Default::default(),
            time_based_filter: Default::default(),
            presentation: Default::default(),
            partition: Default::default(),
            topic_data: Default::default(),
            group_data: Default::default(),
            data_tags: Default::default(),
        }
    }
}

impl SubscriptionBuiltinTopicData {
    /// Constructs from the wire data type.
    #[must_use]
    pub fn from_wire(w: &wire_sub::SubscriptionBuiltinTopicData) -> Self {
        Self {
            key: w.key,
            participant_key: w.participant_key,
            topic_name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: zerodds_qos::DurabilityQosPolicy { kind: w.durability },
            deadline: w.deadline,
            latency_budget: w.latency_budget,
            liveliness: w.liveliness,
            reliability: w.reliability,
            ownership: zerodds_qos::OwnershipQosPolicy { kind: w.ownership },
            destination_order: w.destination_order,
            user_data: zerodds_qos::UserDataQosPolicy {
                value: w.user_data.clone(),
            },
            time_based_filter: w.time_based_filter,
            presentation: w.presentation,
            partition: zerodds_qos::PartitionQosPolicy {
                names: w.partition.clone(),
            },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: w.topic_data.clone(),
            },
            group_data: zerodds_qos::GroupDataQosPolicy {
                value: w.group_data.clone(),
            },
            data_tags: crate::qos::data_tags_from_announced(&w.data_tags),
        }
    }
}

impl DdsType for SubscriptionBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::SubscriptionBuiltinTopicData";
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
        write_endpoint_names(
            &mut w,
            &self.key,
            &self.participant_key,
            &self.topic_name,
            &self.type_name,
        );
        self.durability.encode_into(&mut w);
        self.deadline.encode_into(&mut w);
        self.latency_budget.encode_into(&mut w);
        self.liveliness.encode_into(&mut w);
        self.reliability.encode_into(&mut w);
        self.ownership.encode_into(&mut w);
        self.destination_order.encode_into(&mut w);
        self.user_data.encode_into(&mut w).map_err(encode_failed)?;
        self.time_based_filter.encode_into(&mut w);
        self.presentation.encode_into(&mut w);
        self.partition.encode_into(&mut w).map_err(encode_failed)?;
        self.topic_data.encode_into(&mut w).map_err(encode_failed)?;
        self.group_data.encode_into(&mut w).map_err(encode_failed)?;
        self.data_tags.encode_into(&mut w).map_err(encode_failed)?;
        out.extend_from_slice(&w.finish());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = zerodds_wire::Reader::new(bytes, zerodds_wire::Endianness::Little);
        let (key, participant_key, topic_name, type_name) = read_endpoint_names(&mut r)?;
        Ok(Self {
            key,
            participant_key,
            topic_name,
            type_name,
            durability: policy!(r, DurabilityQosPolicy),
            deadline: policy!(r, DeadlineQosPolicy),
            latency_budget: policy!(r, LatencyBudgetQosPolicy),
            liveliness: policy!(r, LivelinessQosPolicy),
            reliability: policy!(r, ReliabilityQosPolicy),
            ownership: policy!(r, OwnershipQosPolicy),
            destination_order: policy!(r, DestinationOrderQosPolicy),
            user_data: policy!(r, UserDataQosPolicy),
            time_based_filter: policy!(r, TimeBasedFilterQosPolicy),
            presentation: policy!(r, PresentationQosPolicy),
            partition: policy!(r, PartitionQosPolicy),
            topic_data: policy!(r, TopicDataQosPolicy),
            group_data: policy!(r, GroupDataQosPolicy),
            data_tags: policy!(r, DataTagQosPolicy),
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }

    /// The inverse; see [`PublicationBuiltinTopicData`]'s.
    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            key: builtin_key_from_holder(bytes)?,
            ..Self::default()
        })
    }
}

/// The policy codecs of the publication and subscription samples write
/// through `zerodds_wire`; a failure there is the sample's.
fn encode_failed(_: zerodds_wire::WireError) -> EncodeError {
    EncodeError::Invalid {
        what: "builtin-topic sample does not encode",
    }
}

fn decode_failed(_: zerodds_wire::WireError) -> DecodeError {
    DecodeError::Invalid {
        what: "builtin-topic sample does not decode",
    }
}

// ---------------------------------------------------------------------
// 5) The secure built-in topics (DDS-Security 1.2 §7.5.1.6 to §7.5.1.8)
// ---------------------------------------------------------------------

/// Sample type of the `DCPSParticipantsSecure` built-in topic (DDS-Security
/// 1.2 §7.5.1.6): `ParticipantBuiltinTopicDataSecure`, the data of
/// `DCPSParticipant` and the optional `identity_status_token`
/// (`PID_IDENTITY_STATUS_TOKEN` 0x1006).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticipantBuiltinTopicDataSecure {
    /// The members of `ParticipantBuiltinTopicData`.
    pub base: ParticipantBuiltinTopicData,
    /// The CDR-encoded `IdentityStatusToken` of the participant, if it
    /// announced one.
    pub identity_status_token: Option<Vec<u8>>,
}

impl ParticipantBuiltinTopicDataSecure {
    /// Constructs from the wire data type.
    #[must_use]
    pub fn from_wire(w: &wire_part::ParticipantBuiltinTopicData) -> Self {
        Self {
            base: ParticipantBuiltinTopicData::from_wire(w),
            identity_status_token: w.identity_status_token.clone(),
        }
    }
}

/// The tags of a `DataTagQosPolicy` out of its wire form.
fn tags_from_wire(w: &zerodds_rtps::property_list::WireDataTags) -> zerodds_qos::DataTagQosPolicy {
    zerodds_qos::DataTagQosPolicy {
        tags: w
            .tags
            .iter()
            .map(|t| zerodds_qos::Tag::new(t.name.as_str(), t.value.as_str()))
            .collect(),
    }
}

fn encode_tags_le(
    tags: &zerodds_qos::DataTagQosPolicy,
    out: &mut Vec<u8>,
) -> core::result::Result<(), EncodeError> {
    let count = u32::try_from(tags.tags.len()).map_err(|_| EncodeError::Invalid {
        what: "builtin-topic data tags > 4 Gi",
    })?;
    out.extend_from_slice(&count.to_le_bytes());
    for tag in &tags.tags {
        encode_bytes_le(tag.name.as_bytes(), out)?;
        encode_bytes_le(tag.value.as_bytes(), out)?;
    }
    Ok(())
}

fn decode_tags_le(
    r: &mut Reader<'_>,
) -> core::result::Result<zerodds_qos::DataTagQosPolicy, DecodeError> {
    let count = r.read_u32()? as usize;
    let mut tags = Vec::new();
    for _ in 0..count {
        let name = String::from_utf8(r.read_bytes()?).map_err(|_| DecodeError::Invalid {
            what: "builtin-topic tag name is not UTF-8",
        })?;
        let value = String::from_utf8(r.read_bytes()?).map_err(|_| DecodeError::Invalid {
            what: "builtin-topic tag value is not UTF-8",
        })?;
        tags.push(zerodds_qos::Tag { name, value });
    }
    Ok(zerodds_qos::DataTagQosPolicy { tags })
}

impl DdsType for ParticipantBuiltinTopicDataSecure {
    const TYPE_NAME: &'static str = "DDS::Security::ParticipantBuiltinTopicDataSecure";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut base = Vec::new();
        self.base.encode(&mut base)?;
        encode_bytes_le(&base, out)?;
        match &self.identity_status_token {
            Some(token) => {
                out.push(1);
                encode_bytes_le(token, out)
            }
            None => {
                out.push(0);
                Ok(())
            }
        }
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let base = ParticipantBuiltinTopicData::decode(&r.read_bytes()?)?;
        r.need(1)?;
        let present = r.bytes[r.pos];
        r.pos += 1;
        let identity_status_token = match present {
            0 => None,
            1 => Some(r.read_bytes()?),
            _ => {
                return Err(DecodeError::Invalid {
                    what: "builtin-topic optional flag",
                });
            }
        };
        Ok(Self {
            base,
            identity_status_token,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        self.base.encode_key_holder_be(holder);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            base: ParticipantBuiltinTopicData::decode_key_holder_be(bytes)?,
            identity_status_token: None,
        })
    }
}

/// Sample type of the `DCPSPublicationsSecure` built-in topic (DDS-Security
/// 1.2 §7.5.1.7): `PublicationBuiltinTopicDataSecure`, the data of
/// `DCPSPublication` and the `data_tags` (`PID_DATA_TAGS` 0x1003).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublicationBuiltinTopicDataSecure {
    /// The members of `PublicationBuiltinTopicData`.
    pub base: PublicationBuiltinTopicData,
    /// The tags of the DataWriter.
    pub data_tags: zerodds_qos::DataTagQosPolicy,
}

impl PublicationBuiltinTopicDataSecure {
    /// Constructs from the wire data type.
    #[must_use]
    pub fn from_wire(w: &wire_pub::PublicationBuiltinTopicData) -> Self {
        Self {
            base: PublicationBuiltinTopicData::from_wire(w),
            data_tags: tags_from_wire(&w.data_tags),
        }
    }
}

impl DdsType for PublicationBuiltinTopicDataSecure {
    const TYPE_NAME: &'static str = "DDS::Security::PublicationBuiltinTopicDataSecure";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut base = Vec::new();
        self.base.encode(&mut base)?;
        encode_bytes_le(&base, out)?;
        encode_tags_le(&self.data_tags, out)
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let base = PublicationBuiltinTopicData::decode(&r.read_bytes()?)?;
        let data_tags = decode_tags_le(&mut r)?;
        Ok(Self { base, data_tags })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        self.base.encode_key_holder_be(holder);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            base: PublicationBuiltinTopicData::decode_key_holder_be(bytes)?,
            data_tags: zerodds_qos::DataTagQosPolicy::default(),
        })
    }
}

/// Sample type of the `DCPSSubscriptionsSecure` built-in topic (DDS-Security
/// 1.2 §7.5.1.8): `SubscriptionBuiltinTopicDataSecure`, the data of
/// `DCPSSubscription` and the `data_tags` (`PID_DATA_TAGS` 0x1003).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionBuiltinTopicDataSecure {
    /// The members of `SubscriptionBuiltinTopicData`.
    pub base: SubscriptionBuiltinTopicData,
    /// The tags of the DataReader.
    pub data_tags: zerodds_qos::DataTagQosPolicy,
}

impl SubscriptionBuiltinTopicDataSecure {
    /// Constructs from the wire data type.
    #[must_use]
    pub fn from_wire(w: &wire_sub::SubscriptionBuiltinTopicData) -> Self {
        Self {
            base: SubscriptionBuiltinTopicData::from_wire(w),
            data_tags: tags_from_wire(&w.data_tags),
        }
    }
}

impl DdsType for SubscriptionBuiltinTopicDataSecure {
    const TYPE_NAME: &'static str = "DDS::Security::SubscriptionBuiltinTopicDataSecure";
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        let mut base = Vec::new();
        self.base.encode(&mut base)?;
        encode_bytes_le(&base, out)?;
        encode_tags_le(&self.data_tags, out)
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let base = SubscriptionBuiltinTopicData::decode(&r.read_bytes()?)?;
        let data_tags = decode_tags_le(&mut r)?;
        Ok(Self { base, data_tags })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        self.base.encode_key_holder_be(holder);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            base: SubscriptionBuiltinTopicData::decode_key_holder_be(bytes)?,
            data_tags: zerodds_qos::DataTagQosPolicy::default(),
        })
    }
}

/// The two keys and the two names that open a publication and a
/// subscription sample alike.
fn write_endpoint_names(
    w: &mut zerodds_wire::Writer,
    key: &Guid,
    participant_key: &Guid,
    topic_name: &str,
    type_name: &str,
) {
    w.write_bytes(&key.to_bytes());
    w.write_bytes(&participant_key.to_bytes());
    w.write_string(topic_name);
    w.write_string(type_name);
}

/// The inverse of [`write_endpoint_names`].
fn read_endpoint_names(
    r: &mut zerodds_wire::Reader<'_>,
) -> core::result::Result<(Guid, Guid, String, String), DecodeError> {
    Ok((
        read_guid(r)?,
        read_guid(r)?,
        r.read_string().map_err(decode_failed)?,
        r.read_string().map_err(decode_failed)?,
    ))
}

fn read_guid(r: &mut zerodds_wire::Reader<'_>) -> core::result::Result<Guid, DecodeError> {
    let bytes: [u8; 16] = r
        .read_bytes(16)
        .map_err(decode_failed)?
        .try_into()
        .map_err(|_| DecodeError::Invalid {
            what: "builtin-topic GUID",
        })?;
    Ok(Guid::from_bytes(bytes))
}

// ---------------------------------------------------------------------
// Internal ZeroDDS encoding (not wire — stays process-internal).
// ---------------------------------------------------------------------

fn encode_bytes_le(b: &[u8], out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
    let len = u32::try_from(b.len()).map_err(|_| EncodeError::Invalid {
        what: "builtin-topic bytes > 4 GiB",
    })?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(b);
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn need(&self, n: usize) -> core::result::Result<(), DecodeError> {
        if self.pos.saturating_add(n) > self.bytes.len() {
            return Err(DecodeError::Invalid {
                what: "builtin-topic truncated",
            });
        }
        Ok(())
    }

    fn read_u32(&mut self) -> core::result::Result<u32, DecodeError> {
        self.need(4)?;
        let mut a = [0u8; 4];
        a.copy_from_slice(&self.bytes[self.pos..self.pos + 4]);
        self.pos += 4;
        Ok(u32::from_le_bytes(a))
    }

    fn read_guid(&mut self) -> core::result::Result<Guid, DecodeError> {
        self.need(16)?;
        let mut b = [0u8; 16];
        b.copy_from_slice(&self.bytes[self.pos..self.pos + 16]);
        self.pos += 16;
        Ok(Guid::from_bytes(b))
    }

    fn read_bytes(&mut self) -> core::result::Result<Vec<u8>, DecodeError> {
        let len = self.read_u32()? as usize;
        self.need(len)?;
        let v = self.bytes[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(v)
    }
}

/// FNV-1a 64-bit Hash (no_std, deterministic).
fn fnv1a64(data: &[u8], seed: u64) -> u64 {
    let mut h = seed;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_rtps::wire_types::GuidPrefix;

    fn mk_guid(seed: u8) -> Guid {
        let mut b = [0u8; 16];
        for (i, slot) in b.iter_mut().enumerate() {
            *slot = seed.wrapping_add(i as u8);
        }
        Guid::from_bytes(b)
    }

    #[test]
    fn participant_dds_type_name_matches_spec() {
        assert_eq!(
            <ParticipantBuiltinTopicData as DdsType>::TYPE_NAME,
            "DDS::ParticipantBuiltinTopicData"
        );
    }

    #[test]
    fn topic_dds_type_name_matches_spec() {
        assert_eq!(
            <TopicBuiltinTopicData as DdsType>::TYPE_NAME,
            "DDS::TopicBuiltinTopicData"
        );
    }

    #[test]
    fn publication_dds_type_name_matches_spec() {
        assert_eq!(
            <PublicationBuiltinTopicData as DdsType>::TYPE_NAME,
            "DDS::PublicationBuiltinTopicData"
        );
    }

    #[test]
    fn subscription_dds_type_name_matches_spec() {
        assert_eq!(
            <SubscriptionBuiltinTopicData as DdsType>::TYPE_NAME,
            "DDS::SubscriptionBuiltinTopicData"
        );
    }

    #[test]
    fn participant_roundtrip() {
        let p = ParticipantBuiltinTopicData {
            key: mk_guid(0xA0),
            user_data: alloc::vec![1, 2, 3],
        };
        let mut buf = Vec::new();
        p.encode(&mut buf).unwrap();
        let back = ParticipantBuiltinTopicData::decode(&buf).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn topic_roundtrip() {
        let t = TopicBuiltinTopicData {
            key: TopicBuiltinTopicData::synthesize_key(
                GuidPrefix::from_bytes([1; 12]),
                "Chatter",
                "std::String",
            ),
            name: "Chatter".to_string(),
            type_name: "std::String".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::TransientLocal,
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = Vec::new();
        t.encode(&mut buf).unwrap();
        let back = TopicBuiltinTopicData::decode(&buf).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn topic_synthesize_key_is_stable_and_distinct() {
        let p = GuidPrefix::from_bytes([1; 12]);
        let q = GuidPrefix::from_bytes([2; 12]);
        let a = TopicBuiltinTopicData::synthesize_key(p, "X", "Foo");
        let a2 = TopicBuiltinTopicData::synthesize_key(p, "X", "Foo");
        let b = TopicBuiltinTopicData::synthesize_key(p, "X", "Bar");
        let c = TopicBuiltinTopicData::synthesize_key(p, "Y", "Foo");
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(b, c);
        // DDS 1.4 §2.2.5: the entry of a Topic is one per participant.
        let d = TopicBuiltinTopicData::synthesize_key(q, "X", "Foo");
        assert_ne!(a, d, "two participants, two entries");
    }

    fn ms(v: i32) -> zerodds_qos::Duration {
        zerodds_qos::Duration::from_millis(v)
    }

    /// Every member, each away from its default, comes back as it went in;
    /// a duration keeps its fraction of a second.
    #[test]
    fn publication_roundtrip() {
        let p = PublicationBuiltinTopicData {
            key: mk_guid(0xB0),
            participant_key: mk_guid(0xC0),
            topic_name: "T".to_string(),
            type_name: "Tt".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::TransientLocal,
            },
            durability_service: zerodds_qos::DurabilityServiceQosPolicy {
                history_depth: 9,
                ..Default::default()
            },
            deadline: zerodds_qos::DeadlineQosPolicy { period: ms(1_250) },
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy { duration: ms(5) },
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: ms(30_500),
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: ms(40),
            },
            lifespan: zerodds_qos::LifespanQosPolicy {
                duration: ms(60_001),
            },
            user_data: zerodds_qos::UserDataQosPolicy {
                value: alloc::vec![1, 2],
            },
            ownership: zerodds_qos::OwnershipQosPolicy {
                kind: zerodds_qos::OwnershipKind::Exclusive,
            },
            ownership_strength: zerodds_qos::OwnershipStrengthQosPolicy { value: 17 },
            destination_order: zerodds_qos::DestinationOrderQosPolicy {
                kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
            },
            presentation: zerodds_qos::PresentationQosPolicy {
                access_scope: zerodds_qos::PresentationAccessScope::Group,
                coherent_access: true,
                ordered_access: true,
            },
            partition: zerodds_qos::PartitionQosPolicy {
                names: alloc::vec!["A".to_string(), "B".to_string()],
            },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: alloc::vec![3],
            },
            group_data: zerodds_qos::GroupDataQosPolicy {
                value: alloc::vec![4, 5, 6],
            },
            data_tags: zerodds_qos::DataTagQosPolicy::new()
                .with("classification", "secret")
                .with("releasability", "NATO"),
        };
        let mut buf = Vec::new();
        p.encode(&mut buf).unwrap();
        let back = PublicationBuiltinTopicData::decode(&buf).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn subscription_roundtrip() {
        let s = SubscriptionBuiltinTopicData {
            key: mk_guid(0xD0),
            participant_key: mk_guid(0xE0),
            topic_name: "T2".to_string(),
            type_name: "T2t".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Persistent,
            },
            deadline: zerodds_qos::DeadlineQosPolicy { period: ms(1_500) },
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy { duration: ms(7) },
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByParticipant,
                lease_duration: ms(10_250),
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: ms(20),
            },
            ownership: zerodds_qos::OwnershipQosPolicy {
                kind: zerodds_qos::OwnershipKind::Exclusive,
            },
            destination_order: zerodds_qos::DestinationOrderQosPolicy {
                kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
            },
            user_data: zerodds_qos::UserDataQosPolicy {
                value: alloc::vec![9],
            },
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy {
                minimum_separation: ms(250),
            },
            presentation: zerodds_qos::PresentationQosPolicy {
                access_scope: zerodds_qos::PresentationAccessScope::Topic,
                coherent_access: false,
                ordered_access: true,
            },
            partition: zerodds_qos::PartitionQosPolicy {
                names: alloc::vec!["C".to_string()],
            },
            topic_data: zerodds_qos::TopicDataQosPolicy {
                value: alloc::vec![8, 7],
            },
            group_data: zerodds_qos::GroupDataQosPolicy {
                value: alloc::vec![6],
            },
            data_tags: zerodds_qos::DataTagQosPolicy::new().with("classification", "open"),
        };
        let mut buf = Vec::new();
        s.encode(&mut buf).unwrap();
        let back = SubscriptionBuiltinTopicData::decode(&buf).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn participant_keyhash_is_first_16_bytes_of_guid() {
        let p = ParticipantBuiltinTopicData {
            key: mk_guid(7),
            user_data: alloc::vec![],
        };
        let kh = p.compute_key_hash().expect("keyed");
        assert_eq!(kh, p.key.to_bytes());
    }

    #[test]
    fn participant_from_wire_strips_to_guid() {
        use zerodds_rtps::wire_types::{EntityId, ProtocolVersion, VendorId};
        let g = Guid::new(GuidPrefix::from_bytes([7; 12]), EntityId::PARTICIPANT);
        let w = wire_part::ParticipantBuiltinTopicData {
            guid: g,
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locators: Vec::new(),
            default_multicast_locators: Vec::new(),
            metatraffic_unicast_locators: Vec::new(),
            metatraffic_multicast_locators: Vec::new(),
            domain_id: None,
            domain_tag: Default::default(),
            builtin_endpoint_set: 0,
            lease_duration: zerodds_qos::Duration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            available_builtin_endpoints_ext: 0,
            participant_security_info: None,
            expects_inline_qos: false,
            manual_liveliness_count: None,
            builtin_endpoint_qos: None,
            entity_name: None,
        };
        let dcps = ParticipantBuiltinTopicData::from_wire(&w);
        assert_eq!(dcps.key, g);
        assert!(dcps.user_data.is_empty());
    }

    /// The sample carries what the announcement carried: the data
    /// policies and the durations to the fraction, not whole seconds.
    #[test]
    fn publication_from_wire_carries_every_policy() {
        let g_w = mk_guid(1);
        let g_p = mk_guid(2);
        let w = wire_pub::PublicationBuiltinTopicData {
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: g_w,
            participant_key: g_p,
            topic_name: "MyT".to_string(),
            type_name: "MyType".to_string(),
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: zerodds_qos::Duration::from_millis(100),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy { period: ms(1_250) },
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy { duration: ms(5) },
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy { duration: ms(500) },
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec!["P".to_string()],
            user_data: alloc::vec![1],
            topic_data: alloc::vec![2],
            group_data: alloc::vec![3],
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec![],
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
        };
        let d = PublicationBuiltinTopicData::from_wire(&w);
        assert_eq!(d.key, g_w);
        assert_eq!(d.participant_key, g_p);
        assert_eq!(d.topic_name, "MyT");
        assert_eq!(d.type_name, "MyType");
        assert_eq!(
            d.durability.kind,
            zerodds_qos::DurabilityKind::TransientLocal
        );
        assert_eq!(d.reliability, w.reliability);
        assert_eq!(d.deadline.period, ms(1_250));
        assert_eq!(d.latency_budget.duration, ms(5));
        assert_eq!(d.lifespan.duration, ms(500));
        assert_eq!(d.partition.names, alloc::vec!["P".to_string()]);
        assert_eq!(d.user_data.value, alloc::vec![1]);
        assert_eq!(d.topic_data.value, alloc::vec![2]);
        assert_eq!(d.group_data.value, alloc::vec![3]);
    }

    fn wire_pub_with_tags(
        key: Guid,
        tags: zerodds_rtps::property_list::WireDataTags,
    ) -> wire_pub::PublicationBuiltinTopicData {
        wire_pub::PublicationBuiltinTopicData {
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key,
            participant_key: mk_guid(22),
            topic_name: "T".to_string(),
            type_name: "Ty".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            reliability: zerodds_qos::ReliabilityQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_information: None,
            data_representation: alloc::vec![],
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: tags,
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
            type_id_v1: None,
            type_object_v1: None,
        }
    }

    fn wire_sub_with_tags(
        key: Guid,
        tags: zerodds_rtps::property_list::WireDataTags,
    ) -> wire_sub::SubscriptionBuiltinTopicData {
        wire_sub::SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key,
            participant_key: mk_guid(24),
            topic_name: "T".to_string(),
            type_name: "Ty".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            data_representation: alloc::vec![],
            content_filter: None,
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: tags,
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
            type_id_v1: None,
            type_object_v1: None,
        }
    }

    /// DDS-Security 1.2 §9.7.2: the DataTag of the DataWriter is propagated
    /// in the PublicationBuiltinTopicData, and the DataTag of the DataReader
    /// in the SubscriptionBuiltinTopicData; the sample a builtin reader reads
    /// carries the tags the announcement carried, in order.
    #[test]
    fn the_data_tags_of_an_announcement_reach_the_builtin_topic_samples() {
        let tags = zerodds_rtps::property_list::WireDataTags {
            tags: alloc::vec![
                zerodds_rtps::property_list::WireProperty::new("classification", "secret"),
                zerodds_rtps::property_list::WireProperty::new("releasability", "NATO"),
            ],
        };
        let want = zerodds_qos::DataTagQosPolicy::new()
            .with("classification", "secret")
            .with("releasability", "NATO");
        let p =
            PublicationBuiltinTopicData::from_wire(&wire_pub_with_tags(mk_guid(21), tags.clone()));
        let s = SubscriptionBuiltinTopicData::from_wire(&wire_sub_with_tags(mk_guid(23), tags));
        assert_eq!(p.data_tags, want);
        assert_eq!(s.data_tags, want);
        // The sample survives the codec of the built-in topic.
        let mut buf = Vec::new();
        p.encode(&mut buf).unwrap();
        assert_eq!(
            PublicationBuiltinTopicData::decode(&buf).unwrap().data_tags,
            want
        );
        let mut buf = Vec::new();
        s.encode(&mut buf).unwrap();
        assert_eq!(
            SubscriptionBuiltinTopicData::decode(&buf)
                .unwrap()
                .data_tags,
            want
        );
        // An endpoint without tags announces none.
        let none = PublicationBuiltinTopicData::from_wire(&wire_pub_with_tags(
            mk_guid(25),
            Default::default(),
        ));
        assert!(none.data_tags.is_empty());
    }

    #[test]
    fn subscription_from_wire_copies_topic_and_type() {
        let g_r = mk_guid(3);
        let g_p = mk_guid(4);
        let w = wire_sub::SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: g_r,
            participant_key: g_p,
            topic_name: "S".to_string(),
            type_name: "St".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Exclusive,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy {
                minimum_separation: ms(250),
            },
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec!["A".to_string()],
            user_data: alloc::vec![4],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec![],
            content_filter: None,
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
        };
        let d = SubscriptionBuiltinTopicData::from_wire(&w);
        assert_eq!(d.key, g_r);
        assert_eq!(d.participant_key, g_p);
        assert_eq!(d.topic_name, "S");
        assert_eq!(d.type_name, "St");
        assert_eq!(d.partition.names, alloc::vec!["A".to_string()]);
        assert_eq!(d.time_based_filter.minimum_separation, ms(250));
        assert_eq!(d.user_data.value, alloc::vec![4]);
    }

    /// DDS 1.4 §2.2.5 `TopicBuiltinTopicData`: each member of the sample is
    /// the value the announcement carried, away from the default.
    #[test]
    fn topic_sample_carries_every_member() {
        let mut w = zerodds_rtps::topic_data::TopicBuiltinTopicData::new(
            mk_guid(0x10),
            "Chatter",
            "Chat::Msg",
        );
        w.durability = zerodds_qos::DurabilityQosPolicy {
            kind: zerodds_qos::DurabilityKind::Transient,
        };
        w.durability_service = zerodds_qos::DurabilityServiceQosPolicy {
            history_depth: 7,
            max_samples: 70,
            ..Default::default()
        };
        w.deadline = zerodds_qos::DeadlineQosPolicy { period: ms(1_250) };
        w.latency_budget = zerodds_qos::LatencyBudgetQosPolicy { duration: ms(5) };
        w.liveliness = zerodds_qos::LivelinessQosPolicy {
            kind: zerodds_qos::LivelinessKind::ManualByTopic,
            lease_duration: ms(30_500),
        };
        w.reliability = zerodds_qos::ReliabilityQosPolicy {
            kind: zerodds_qos::ReliabilityKind::Reliable,
            max_blocking_time: ms(40),
        };
        w.lifespan = zerodds_qos::LifespanQosPolicy {
            duration: ms(60_001),
        };
        w.destination_order = zerodds_qos::DestinationOrderQosPolicy {
            kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
        };
        w.ownership = zerodds_qos::OwnershipQosPolicy {
            kind: zerodds_qos::OwnershipKind::Exclusive,
        };
        w.topic_data = zerodds_qos::TopicDataQosPolicy {
            value: alloc::vec![9, 8],
        };
        let d = TopicBuiltinTopicData::from_wire(&w);
        assert_eq!(d.name, "Chatter");
        assert_eq!(d.type_name, "Chat::Msg");
        assert_eq!(d.durability, w.durability);
        assert_eq!(d.durability_service, w.durability_service);
        assert_eq!(d.deadline.period, ms(1_250));
        assert_eq!(d.latency_budget.duration, ms(5));
        assert_eq!(d.liveliness, w.liveliness);
        assert_eq!(d.reliability, w.reliability);
        assert_eq!(d.lifespan.duration, ms(60_001));
        assert_eq!(d.destination_order, w.destination_order);
        assert_eq!(d.ownership, w.ownership);
        assert_eq!(d.topic_data.value, alloc::vec![9, 8]);
    }

    /// `PublicationBuiltinTopicData`: the members that were set and not
    /// asserted before.
    #[test]
    fn publication_sample_carries_ownership_liveliness_and_presentation() {
        let w = wire_pub::PublicationBuiltinTopicData {
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: mk_guid(0x20),
            participant_key: mk_guid(0x21),
            topic_name: "T".to_string(),
            type_name: "Tt".to_string(),
            durability: zerodds_qos::DurabilityKind::Persistent,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy {
                history_depth: 9,
                ..Default::default()
            },
            reliability: zerodds_qos::ReliabilityQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Exclusive,
            ownership_strength: 42,
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByParticipant,
                lease_duration: ms(2_500),
            },
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy {
                kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
            },
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy {
                access_scope: zerodds_qos::PresentationAccessScope::Group,
                coherent_access: true,
                ordered_access: true,
            },
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec![],
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
            data_tags: Default::default(),
            properties: Default::default(),
            sym_cipher_algo_info: None,
        };
        let d = PublicationBuiltinTopicData::from_wire(&w);
        assert_eq!(d.durability_service.history_depth, 9);
        assert_eq!(d.liveliness, w.liveliness);
        assert_eq!(d.ownership.kind, zerodds_qos::OwnershipKind::Exclusive);
        assert_eq!(d.ownership_strength.value, 42);
        assert_eq!(d.destination_order, w.destination_order);
        assert_eq!(d.presentation, w.presentation);
    }

    /// `SubscriptionBuiltinTopicData`: every member of the sample.
    #[test]
    fn subscription_sample_carries_every_member() {
        let w = wire_sub::SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: mk_guid(0x30),
            participant_key: mk_guid(0x31),
            topic_name: "SubTopic".to_string(),
            type_name: "Sub::Type".to_string(),
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: ms(40),
            },
            ownership: zerodds_qos::OwnershipKind::Exclusive,
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: ms(7_500),
            },
            deadline: zerodds_qos::DeadlineQosPolicy { period: ms(900) },
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy { duration: ms(12) },
            destination_order: zerodds_qos::DestinationOrderQosPolicy {
                kind: zerodds_qos::DestinationOrderKind::BySourceTimestamp,
            },
            presentation: zerodds_qos::PresentationQosPolicy {
                access_scope: zerodds_qos::PresentationAccessScope::Group,
                coherent_access: true,
                ordered_access: false,
            },
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![5, 6],
            group_data: alloc::vec![7],
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec![],
            content_filter: None,
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
            data_tags: Default::default(),
            properties: Default::default(),
            sym_cipher_algo_info: None,
        };
        let d = SubscriptionBuiltinTopicData::from_wire(&w);
        assert_eq!(d.topic_name, "SubTopic");
        assert_eq!(d.type_name, "Sub::Type");
        assert_eq!(
            d.durability.kind,
            zerodds_qos::DurabilityKind::TransientLocal
        );
        assert_eq!(d.deadline.period, ms(900));
        assert_eq!(d.latency_budget.duration, ms(12));
        assert_eq!(d.liveliness, w.liveliness);
        assert_eq!(d.reliability, w.reliability);
        assert_eq!(d.ownership.kind, zerodds_qos::OwnershipKind::Exclusive);
        assert_eq!(d.destination_order, w.destination_order);
        assert_eq!(d.presentation, w.presentation);
        assert_eq!(d.topic_data.value, alloc::vec![5, 6]);
        assert_eq!(d.group_data.value, alloc::vec![7]);
    }

    /// `ParticipantBuiltinTopicData`: a non-empty `user_data` comes through.
    #[test]
    fn participant_sample_carries_user_data() {
        use zerodds_rtps::wire_types::{EntityId, ProtocolVersion, VendorId};
        let g = Guid::new(GuidPrefix::from_bytes([8; 12]), EntityId::PARTICIPANT);
        let w = wire_part::ParticipantBuiltinTopicData {
            guid: g,
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locators: Vec::new(),
            default_multicast_locators: Vec::new(),
            metatraffic_unicast_locators: Vec::new(),
            metatraffic_multicast_locators: Vec::new(),
            domain_id: None,
            domain_tag: Default::default(),
            builtin_endpoint_set: 0,
            lease_duration: zerodds_qos::Duration::from_secs(100),
            user_data: alloc::vec![0xAB, 0xCD],
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
            participant_security_info: None,
            expects_inline_qos: false,
            manual_liveliness_count: None,
            builtin_endpoint_qos: None,
            entity_name: None,
            available_builtin_endpoints_ext: 0,
        };
        let dcps = ParticipantBuiltinTopicData::from_wire(&w);
        assert_eq!(dcps.user_data, alloc::vec![0xAB, 0xCD]);
    }

    #[test]
    fn topic_from_publication_synthesizes_consistent_key() {
        let g_w = mk_guid(5);
        let g_p = mk_guid(6);
        let w = wire_pub::PublicationBuiltinTopicData {
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: g_w,
            participant_key: g_p,
            topic_name: "Same".to_string(),
            type_name: "T".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            reliability: zerodds_qos::ReliabilityQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec![],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec![],
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
        };
        let t = TopicBuiltinTopicData::from_publication(&w);
        let expected_key =
            TopicBuiltinTopicData::synthesize_key(w.participant_key.prefix, "Same", "T");
        assert_eq!(t.key, expected_key);
        assert_eq!(t.name, "Same");
    }

    #[test]
    fn decode_rejects_truncated() {
        let bad = alloc::vec![1u8, 2, 3];
        assert!(ParticipantBuiltinTopicData::decode(&bad).is_err());
        assert!(TopicBuiltinTopicData::decode(&bad).is_err());
        assert!(PublicationBuiltinTopicData::decode(&bad).is_err());
        assert!(SubscriptionBuiltinTopicData::decode(&bad).is_err());
    }

    #[test]
    fn decode_rejects_invalid_durability_kind() {
        // Encode a topic with valid prefix, then patch the durability byte to invalid.
        let t = TopicBuiltinTopicData {
            key: mk_guid(0),
            name: "x".to_string(),
            type_name: "y".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Volatile,
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = Vec::new();
        t.encode(&mut buf).unwrap();
        // last 2 bytes are durability (penult.) + reliability (last).
        let dur_idx = buf.len() - 2;
        buf[dur_idx] = 99;
        assert!(TopicBuiltinTopicData::decode(&buf).is_err());
    }

    #[test]
    fn decode_rejects_invalid_reliability_kind() {
        let t = TopicBuiltinTopicData {
            key: mk_guid(0),
            name: "x".to_string(),
            type_name: "y".to_string(),
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Volatile,
            },
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = Vec::new();
        t.encode(&mut buf).unwrap();
        *buf.last_mut().unwrap() = 99;
        assert!(TopicBuiltinTopicData::decode(&buf).is_err());
    }

    #[test]
    fn decode_rejects_invalid_ownership_kind() {
        let p = PublicationBuiltinTopicData {
            key: mk_guid(0),
            participant_key: mk_guid(1),
            topic_name: "T".to_string(),
            type_name: "T".to_string(),
            ..PublicationBuiltinTopicData::default()
        };
        let mut buf = Vec::new();
        p.encode(&mut buf).unwrap();
        // The ownership byte is found, not counted: its offset moves with
        // every field encoded before it (the DURABILITY_SERVICE policy
        // moved it once). It is the one byte that differs between a Shared
        // and an Exclusive encoding of the same record.
        let exclusive = PublicationBuiltinTopicData {
            ownership: zerodds_qos::OwnershipQosPolicy {
                kind: zerodds_qos::OwnershipKind::Exclusive,
            },
            ..p.clone()
        };
        let mut other = Vec::new();
        exclusive.encode(&mut other).unwrap();
        assert_eq!(buf.len(), other.len());
        let differing: Vec<usize> = (0..buf.len()).filter(|&i| buf[i] != other[i]).collect();
        assert_eq!(differing.len(), 1, "ownership is one byte: {differing:?}");
        buf[differing[0]] = 99;
        assert!(PublicationBuiltinTopicData::decode(&buf).is_err());
    }

    /// DDS-Security 1.2 §7.5.1.6 to §7.5.1.8: the Secure types are the types of
    /// the plain topics and the member of Table 14, 15 and 16: the optional
    /// `identity_status_token`, the `data_tags`. They keep what they hold across
    /// the internal encoding, and the key holder is that of the plain type.
    #[test]
    fn the_secure_types_carry_their_added_member() {
        use crate::dds_type::DdsType;
        use crate::key_hash::PlainCdr2BeKeyHolder;
        let base = ParticipantBuiltinTopicData {
            key: mk_guid(1),
            user_data: alloc::vec![1, 2, 3],
        };
        for token in [None, Some(alloc::vec![9u8, 8, 7])] {
            let sample = ParticipantBuiltinTopicDataSecure {
                base: base.clone(),
                identity_status_token: token,
            };
            let mut bytes = Vec::new();
            sample.encode(&mut bytes).unwrap();
            assert_eq!(
                ParticipantBuiltinTopicDataSecure::decode(&bytes).unwrap(),
                sample
            );
        }
        let tags = zerodds_qos::DataTagQosPolicy::new()
            .with("classification", "secret")
            .with("releasability", "nato");
        let publication = PublicationBuiltinTopicDataSecure {
            base: PublicationBuiltinTopicData {
                key: mk_guid(2),
                topic_name: "T".to_string(),
                type_name: "T".to_string(),
                ..PublicationBuiltinTopicData::default()
            },
            data_tags: tags.clone(),
        };
        let mut bytes = Vec::new();
        publication.encode(&mut bytes).unwrap();
        assert_eq!(
            PublicationBuiltinTopicDataSecure::decode(&bytes).unwrap(),
            publication
        );
        let subscription = SubscriptionBuiltinTopicDataSecure {
            base: SubscriptionBuiltinTopicData {
                key: mk_guid(3),
                ..SubscriptionBuiltinTopicData::default()
            },
            data_tags: tags,
        };
        let mut bytes = Vec::new();
        subscription.encode(&mut bytes).unwrap();
        assert_eq!(
            SubscriptionBuiltinTopicDataSecure::decode(&bytes).unwrap(),
            subscription
        );
        // The key is the key GUID of the plain type.
        let (mut plain, mut secure) = (
            PlainCdr2BeKeyHolder::default(),
            PlainCdr2BeKeyHolder::default(),
        );
        publication.base.encode_key_holder_be(&mut plain);
        publication.encode_key_holder_be(&mut secure);
        assert_eq!(plain.into_bytes(), secure.into_bytes());
        // The names of the topics.
        assert_eq!(
            TOPIC_NAME_DCPS_PARTICIPANTS_SECURE,
            "DCPSParticipantsSecure"
        );
        assert_eq!(
            TOPIC_NAME_DCPS_PUBLICATIONS_SECURE,
            "DCPSPublicationsSecure"
        );
        assert_eq!(
            TOPIC_NAME_DCPS_SUBSCRIPTIONS_SECURE,
            "DCPSSubscriptionsSecure"
        );
    }

    /// The Secure types are made from the wire data of the secure topics, with
    /// the `identity_status_token` and the `data_tags` as the peer sent them.
    #[test]
    fn the_secure_types_are_made_from_the_wire_data() {
        use zerodds_rtps::property_list::{WireDataTags, WireProperty};
        let mut wire = wire_pub::PublicationBuiltinTopicData {
            vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
            key: mk_guid(1),
            participant_key: mk_guid(2),
            topic_name: "MyT".to_string(),
            type_name: "MyType".to_string(),
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: zerodds_qos::Duration::from_millis(100),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy { period: ms(1_250) },
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy { duration: ms(5) },
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy { duration: ms(500) },
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec!["P".to_string()],
            user_data: alloc::vec![1],
            topic_data: alloc::vec![2],
            group_data: alloc::vec![3],
            type_information: None,
            data_representation: alloc::vec![],
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
            type_id_v1: None,
            type_object_v1: None,
        };
        wire.data_tags = WireDataTags {
            tags: alloc::vec![WireProperty::new("level", "secret")],
        };
        let sample = PublicationBuiltinTopicDataSecure::from_wire(&wire);
        assert_eq!(sample.data_tags.tags.len(), 1);
        assert_eq!(sample.data_tags.tags[0].name, "level");
        assert_eq!(sample.data_tags.tags[0].value, "secret");
    }
}
