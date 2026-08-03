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
use alloc::string::{String, ToString};
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
}

// ---------------------------------------------------------------------
// 2) DCPSTopic
// ---------------------------------------------------------------------

/// Sample type of the `DCPSTopic` built-in topic (DDS 1.4 §2.2.5.2).
///
/// Minimal subset (topic name + type name + durability + reliability)
/// from which end-user tooling can render the discovered topics. The
/// full topic-QoS table (DDS 1.4 §2.2.5.2) remains an optional
/// extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicBuiltinTopicData {
    /// Stable identity of the topic entry (synthetic, from a hash of
    /// topic name + type name; see [`Self::synthesize_key`]).
    pub key: Guid,
    /// Topic name (e.g. `"ChatterTopic"`).
    pub name: String,
    /// IDL type name.
    pub type_name: String,
    /// Durability QoS kind.
    pub durability: zerodds_qos::DurabilityKind,
    /// Reliability QoS kind.
    pub reliability: zerodds_qos::ReliabilityKind,
}

impl TopicBuiltinTopicData {
    /// Produces a synthetic GUID key from topic + type name. Stable:
    /// the same topic+type yields the same key. This makes the
    /// `DCPSTopic` built-in topic deliver idempotent updates instead of
    /// insert-per-endpoint (spec §2.2.5.2).
    #[must_use]
    pub fn synthesize_key(topic: &str, type_name: &str) -> Guid {
        // FNV-1a 64-bit, twice — deterministic, no_std, sufficiently
        // collision-safe for a few thousand topics.
        let h1 = fnv1a64(topic.as_bytes(), 0xcbf2_9ce4_8422_2325);
        let h2 = fnv1a64(type_name.as_bytes(), h1);
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&h1.to_le_bytes());
        bytes[8..16].copy_from_slice(&h2.to_le_bytes());
        Guid::from_bytes(bytes)
    }

    /// Constructs from a discovered publication wire data type.
    #[must_use]
    pub fn from_publication(w: &wire_pub::PublicationBuiltinTopicData) -> Self {
        Self {
            key: Self::synthesize_key(&w.topic_name, &w.type_name),
            name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: w.durability,
            reliability: w.reliability.kind,
        }
    }

    /// Constructs from a discovered subscription wire data type.
    #[must_use]
    pub fn from_subscription(w: &wire_sub::SubscriptionBuiltinTopicData) -> Self {
        Self {
            key: Self::synthesize_key(&w.topic_name, &w.type_name),
            name: w.topic_name.clone(),
            type_name: w.type_name.clone(),
            durability: w.durability,
            reliability: w.reliability.kind,
        }
    }
}

impl DdsType for TopicBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::TopicBuiltinTopicData";
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.key.to_bytes());
        encode_string_le(&self.name, out)?;
        encode_string_le(&self.type_name, out)?;
        out.push(self.durability as u8);
        out.push(self.reliability as u8);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let key = r.read_guid()?;
        let name = r.read_string()?;
        let type_name = r.read_string()?;
        let durability = decode_durability(r.read_u8()?)?;
        let reliability = decode_reliability(r.read_u8()?)?;
        Ok(Self {
            key,
            name,
            type_name,
            durability,
            reliability,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }
}

// ---------------------------------------------------------------------
// 3) DCPSPublication
// ---------------------------------------------------------------------

/// Sample type of the `DCPSPublication` built-in topic (DDS 1.4 §2.2.5.3).
///
/// Represents a discovered remote `DataWriter`.
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
    /// Durability QoS kind.
    pub durability: zerodds_qos::DurabilityKind,
    /// Reliability QoS kind.
    pub reliability: zerodds_qos::ReliabilityKind,
    /// Ownership QoS kind.
    pub ownership: zerodds_qos::OwnershipKind,
    /// Ownership strength.
    pub ownership_strength: i32,
    /// Liveliness lease (seconds, rounded).
    pub liveliness_lease_seconds: i32,
    /// Deadline period (seconds, rounded).
    pub deadline_seconds: i32,
    /// Lifespan duration (seconds, rounded).
    pub lifespan_seconds: i32,
    /// Partition list.
    pub partition: Vec<String>,
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
            durability: w.durability,
            reliability: w.reliability.kind,
            ownership: w.ownership,
            ownership_strength: w.ownership_strength,
            liveliness_lease_seconds: w.liveliness.lease_duration.seconds,
            deadline_seconds: w.deadline.period.seconds,
            lifespan_seconds: w.lifespan.duration.seconds,
            partition: w.partition.clone(),
        }
    }
}

impl DdsType for PublicationBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::PublicationBuiltinTopicData";
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.key.to_bytes());
        out.extend_from_slice(&self.participant_key.to_bytes());
        encode_string_le(&self.topic_name, out)?;
        encode_string_le(&self.type_name, out)?;
        out.push(self.durability as u8);
        out.push(self.reliability as u8);
        out.push(self.ownership as u8);
        out.extend_from_slice(&self.ownership_strength.to_le_bytes());
        out.extend_from_slice(&self.liveliness_lease_seconds.to_le_bytes());
        out.extend_from_slice(&self.deadline_seconds.to_le_bytes());
        out.extend_from_slice(&self.lifespan_seconds.to_le_bytes());
        encode_string_seq_le(&self.partition, out)?;
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let key = r.read_guid()?;
        let participant_key = r.read_guid()?;
        let topic_name = r.read_string()?;
        let type_name = r.read_string()?;
        let durability = decode_durability(r.read_u8()?)?;
        let reliability = decode_reliability(r.read_u8()?)?;
        let ownership = decode_ownership(r.read_u8()?)?;
        let ownership_strength = r.read_i32()?;
        let liveliness_lease_seconds = r.read_i32()?;
        let deadline_seconds = r.read_i32()?;
        let lifespan_seconds = r.read_i32()?;
        let partition = r.read_string_seq()?;
        Ok(Self {
            key,
            participant_key,
            topic_name,
            type_name,
            durability,
            reliability,
            ownership,
            ownership_strength,
            liveliness_lease_seconds,
            deadline_seconds,
            lifespan_seconds,
            partition,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }
}

// ---------------------------------------------------------------------
// 4) DCPSSubscription
// ---------------------------------------------------------------------

/// Sample type of the `DCPSSubscription` built-in topic (DDS 1.4 §2.2.5.4).
///
/// Represents a discovered remote `DataReader`.
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
    /// Durability QoS kind.
    pub durability: zerodds_qos::DurabilityKind,
    /// Reliability QoS kind.
    pub reliability: zerodds_qos::ReliabilityKind,
    /// Ownership QoS kind.
    pub ownership: zerodds_qos::OwnershipKind,
    /// Liveliness lease (seconds).
    pub liveliness_lease_seconds: i32,
    /// Deadline period (seconds).
    pub deadline_seconds: i32,
    /// Partition list.
    pub partition: Vec<String>,
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
            durability: w.durability,
            reliability: w.reliability.kind,
            ownership: w.ownership,
            liveliness_lease_seconds: w.liveliness.lease_duration.seconds,
            deadline_seconds: w.deadline.period.seconds,
            partition: w.partition.clone(),
        }
    }
}

impl DdsType for SubscriptionBuiltinTopicData {
    const TYPE_NAME: &'static str = "DDS::SubscriptionBuiltinTopicData";
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.key.to_bytes());
        out.extend_from_slice(&self.participant_key.to_bytes());
        encode_string_le(&self.topic_name, out)?;
        encode_string_le(&self.type_name, out)?;
        out.push(self.durability as u8);
        out.push(self.reliability as u8);
        out.push(self.ownership as u8);
        out.extend_from_slice(&self.liveliness_lease_seconds.to_le_bytes());
        out.extend_from_slice(&self.deadline_seconds.to_le_bytes());
        encode_string_seq_le(&self.partition, out)?;
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let key = r.read_guid()?;
        let participant_key = r.read_guid()?;
        let topic_name = r.read_string()?;
        let type_name = r.read_string()?;
        let durability = decode_durability(r.read_u8()?)?;
        let reliability = decode_reliability(r.read_u8()?)?;
        let ownership = decode_ownership(r.read_u8()?)?;
        let liveliness_lease_seconds = r.read_i32()?;
        let deadline_seconds = r.read_i32()?;
        let partition = r.read_string_seq()?;
        Ok(Self {
            key,
            participant_key,
            topic_name,
            type_name,
            durability,
            reliability,
            ownership,
            liveliness_lease_seconds,
            deadline_seconds,
            partition,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
        holder.write_bytes(&self.key.to_bytes());
    }
}

// ---------------------------------------------------------------------
// Internal ZeroDDS encoding (not wire — stays process-internal).
// ---------------------------------------------------------------------

fn encode_string_le(s: &str, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
    let len = u32::try_from(s.len()).map_err(|_| EncodeError::Invalid {
        what: "builtin-topic string > 4 GiB",
    })?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

fn encode_bytes_le(b: &[u8], out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
    let len = u32::try_from(b.len()).map_err(|_| EncodeError::Invalid {
        what: "builtin-topic bytes > 4 GiB",
    })?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(b);
    Ok(())
}

fn encode_string_seq_le(
    seq: &[String],
    out: &mut Vec<u8>,
) -> core::result::Result<(), EncodeError> {
    let len = u32::try_from(seq.len()).map_err(|_| EncodeError::Invalid {
        what: "builtin-topic seq len > 4 GiB",
    })?;
    out.extend_from_slice(&len.to_le_bytes());
    for s in seq {
        encode_string_le(s, out)?;
    }
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

    fn read_u8(&mut self) -> core::result::Result<u8, DecodeError> {
        self.need(1)?;
        let v = self.bytes[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u32(&mut self) -> core::result::Result<u32, DecodeError> {
        self.need(4)?;
        let mut a = [0u8; 4];
        a.copy_from_slice(&self.bytes[self.pos..self.pos + 4]);
        self.pos += 4;
        Ok(u32::from_le_bytes(a))
    }

    fn read_i32(&mut self) -> core::result::Result<i32, DecodeError> {
        self.read_u32().map(|v| v as i32)
    }

    fn read_guid(&mut self) -> core::result::Result<Guid, DecodeError> {
        self.need(16)?;
        let mut b = [0u8; 16];
        b.copy_from_slice(&self.bytes[self.pos..self.pos + 16]);
        self.pos += 16;
        Ok(Guid::from_bytes(b))
    }

    fn read_string(&mut self) -> core::result::Result<String, DecodeError> {
        let len = self.read_u32()? as usize;
        self.need(len)?;
        let s = core::str::from_utf8(&self.bytes[self.pos..self.pos + len])
            .map_err(|_| DecodeError::Invalid {
                what: "builtin-topic invalid utf8",
            })?
            .to_string();
        self.pos += len;
        Ok(s)
    }

    fn read_bytes(&mut self) -> core::result::Result<Vec<u8>, DecodeError> {
        let len = self.read_u32()? as usize;
        self.need(len)?;
        let v = self.bytes[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(v)
    }

    fn read_string_seq(&mut self) -> core::result::Result<Vec<String>, DecodeError> {
        let n = self.read_u32()? as usize;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.read_string()?);
        }
        Ok(v)
    }
}

fn decode_durability(b: u8) -> core::result::Result<zerodds_qos::DurabilityKind, DecodeError> {
    use zerodds_qos::DurabilityKind::*;
    Ok(match b {
        0 => Volatile,
        1 => TransientLocal,
        2 => Transient,
        3 => Persistent,
        _ => {
            return Err(DecodeError::Invalid {
                what: "durability kind out of range",
            });
        }
    })
}

fn decode_reliability(b: u8) -> core::result::Result<zerodds_qos::ReliabilityKind, DecodeError> {
    use zerodds_qos::ReliabilityKind::*;
    Ok(match b {
        1 => BestEffort,
        2 => Reliable,
        _ => {
            return Err(DecodeError::Invalid {
                what: "reliability kind out of range",
            });
        }
    })
}

fn decode_ownership(b: u8) -> core::result::Result<zerodds_qos::OwnershipKind, DecodeError> {
    use zerodds_qos::OwnershipKind::*;
    Ok(match b {
        0 => Shared,
        1 => Exclusive,
        _ => {
            return Err(DecodeError::Invalid {
                what: "ownership kind out of range",
            });
        }
    })
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
            key: TopicBuiltinTopicData::synthesize_key("Chatter", "std::String"),
            name: "Chatter".to_string(),
            type_name: "std::String".to_string(),
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            reliability: zerodds_qos::ReliabilityKind::Reliable,
        };
        let mut buf = Vec::new();
        t.encode(&mut buf).unwrap();
        let back = TopicBuiltinTopicData::decode(&buf).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn topic_synthesize_key_is_stable_and_distinct() {
        let a = TopicBuiltinTopicData::synthesize_key("X", "Foo");
        let a2 = TopicBuiltinTopicData::synthesize_key("X", "Foo");
        let b = TopicBuiltinTopicData::synthesize_key("X", "Bar");
        let c = TopicBuiltinTopicData::synthesize_key("Y", "Foo");
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(b, c);
    }

    #[test]
    fn publication_roundtrip() {
        let p = PublicationBuiltinTopicData {
            key: mk_guid(0xB0),
            participant_key: mk_guid(0xC0),
            topic_name: "T".to_string(),
            type_name: "Tt".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityKind::BestEffort,
            ownership: zerodds_qos::OwnershipKind::Exclusive,
            ownership_strength: 17,
            liveliness_lease_seconds: 30,
            deadline_seconds: 5,
            lifespan_seconds: 60,
            partition: alloc::vec!["A".to_string(), "B".to_string()],
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
            durability: zerodds_qos::DurabilityKind::Persistent,
            reliability: zerodds_qos::ReliabilityKind::Reliable,
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness_lease_seconds: 10,
            deadline_seconds: 1,
            partition: alloc::vec![],
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
            participant_security_info: None,
        };
        let dcps = ParticipantBuiltinTopicData::from_wire(&w);
        assert_eq!(dcps.key, g);
        assert!(dcps.user_data.is_empty());
    }

    #[test]
    fn publication_from_wire_copies_topic_and_type() {
        let g_w = mk_guid(1);
        let g_p = mk_guid(2);
        let w = wire_pub::PublicationBuiltinTopicData {
            key: g_w,
            participant_key: g_p,
            topic_name: "MyT".to_string(),
            type_name: "MyType".to_string(),
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            reliability: zerodds_qos::ReliabilityQosPolicy {
                kind: zerodds_qos::ReliabilityKind::Reliable,
                max_blocking_time: zerodds_qos::Duration::from_millis(100),
            },
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
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
        };
        let d = PublicationBuiltinTopicData::from_wire(&w);
        assert_eq!(d.key, g_w);
        assert_eq!(d.participant_key, g_p);
        assert_eq!(d.topic_name, "MyT");
        assert_eq!(d.type_name, "MyType");
        assert_eq!(d.durability, zerodds_qos::DurabilityKind::TransientLocal);
        assert_eq!(d.reliability, zerodds_qos::ReliabilityKind::Reliable);
    }

    #[test]
    fn subscription_from_wire_copies_topic_and_type() {
        let g_r = mk_guid(3);
        let g_p = mk_guid(4);
        let w = wire_sub::SubscriptionBuiltinTopicData {
            key: g_r,
            participant_key: g_p,
            topic_name: "S".to_string(),
            type_name: "St".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityQosPolicy::default(),
            ownership: zerodds_qos::OwnershipKind::Exclusive,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            partition: alloc::vec!["A".to_string()],
            user_data: alloc::vec![],
            topic_data: alloc::vec![],
            group_data: alloc::vec![],
            type_information: None,
            data_representation: alloc::vec![],
            content_filter: None,
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
        };
        let d = SubscriptionBuiltinTopicData::from_wire(&w);
        assert_eq!(d.key, g_r);
        assert_eq!(d.participant_key, g_p);
        assert_eq!(d.partition, alloc::vec!["A".to_string()]);
    }

    #[test]
    fn topic_from_publication_synthesizes_consistent_key() {
        let g_w = mk_guid(5);
        let g_p = mk_guid(6);
        let w = wire_pub::PublicationBuiltinTopicData {
            key: g_w,
            participant_key: g_p,
            topic_name: "Same".to_string(),
            type_name: "T".to_string(),
            durability: zerodds_qos::DurabilityKind::Volatile,
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
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: Vec::new(),
            multicast_locators: Vec::new(),
        };
        let t = TopicBuiltinTopicData::from_publication(&w);
        let expected_key = TopicBuiltinTopicData::synthesize_key("Same", "T");
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
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityKind::Reliable,
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
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityKind::Reliable,
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
            durability: zerodds_qos::DurabilityKind::Volatile,
            reliability: zerodds_qos::ReliabilityKind::Reliable,
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness_lease_seconds: 0,
            deadline_seconds: 0,
            lifespan_seconds: 0,
            partition: alloc::vec![],
        };
        let mut buf = Vec::new();
        p.encode(&mut buf).unwrap();
        // ownership byte is at offset: 16 (key) + 16 (pkey) + 4+1 (T) + 4+1 (T) + 1 (dur) + 1 (rel) = 44
        // bytes[44] should be ownership.
        buf[44] = 99;
        assert!(PublicationBuiltinTopicData::decode(&buf).is_err());
    }
}
