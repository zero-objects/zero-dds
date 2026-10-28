// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DiscoveredTopicData` (DDSI-RTPS 2.5 §8.5.4.2): DDS 1.4 §2.2.5's
//! `TopicBuiltinTopicData` as the ParameterList the SEDP topics writer
//! `ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER` sends.
//!
//! Measured against Cyclone DDS 11.0.1 with
//! `Discovery/EnableTopicDiscoveryEndpoints` on the Linux test host: its
//! announcement is a `PL_CDR_LE` DATA with no in-line QoS, holding
//! `PID_TOPIC_NAME`, `PID_TYPE_NAME`, `PID_TOPIC_DATA`, the topic's QoS —
//! a policy at its default left out — `PID_DATA_REPRESENTATION`,
//! `PID_TYPE_INFORMATION`, `PID_PROTOCOL_VERSION`, `PID_VENDOR_ID` and a
//! vendor parameter of its own.
//!
//! The key is `PID_ENDPOINT_GUID`: DDSI-RTPS 2.5 Table 9.19 maps
//! `TopicBuiltinTopicData::key` to it. Cyclone does not send it; it keys
//! its topic announcements on `PID_CYCLONE_TOPIC_GUID` (`0x801b`,
//! `ddsi__protocol.h`), a vendor-specific parameter, which is read as the
//! key only from a message whose `vendorId` is Cyclone's (§9.4.2.11.1
//! Table 9.6: a vendor-specific parameter "will not be recognized by
//! other vendors' implementations").
//!
//! This leaves a policy at its default out, as Cyclone does, and reads an
//! absent policy as its default (DDSI-RTPS 2.5 §9.4.2.11: "If a
//! ParameterId is expected, but not present, the protocol will assume the
//! default value"; Table 9.19 gives the DDS defaults). A policy that is
//! present and does not decode is a decode error and not its default.
//!
//! Dust DDS 0.15.0 stops its executor on any `PID_HISTORY` it reads: its
//! `HistoryQosPolicyKind` has a `UINT8` holder, which its deserializer
//! does not take (`xtypes/deserializer.rs`, `TypeKind::ENUM`). A topic at
//! the default history sends it none.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderQosPolicy, DurabilityQosPolicy, DurabilityServiceQosPolicy,
    HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy, LivelinessQosPolicy,
    OwnershipQosPolicy, ReliabilityQosPolicy, ResourceLimitsQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy,
};
use zerodds_wire::{Endianness, Reader, Writer};

use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::participant_data::ENCAPSULATION_PL_CDR_LE;
use crate::publication_data::{
    STRING_256_MAX, decode_bounded_cdr_string, encode_bounded_cdr_string_le,
};
use crate::wire_types::{Guid, VendorId};

/// Cyclone DDS's `PID_CYCLONE_TOPIC_GUID`, the key of its topic
/// announcements; meaningful only from a message Cyclone sent.
pub const PID_CYCLONE_TOPIC_GUID: u16 = 0x801b;

/// One topic as the SEDP topics writer announces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicBuiltinTopicData {
    /// `PID_ENDPOINT_GUID`: the announcing participant's prefix and the
    /// topic's entity id.
    pub key: Guid,
    /// `PID_TOPIC_NAME`.
    pub topic_name: String,
    /// `PID_TYPE_NAME`.
    pub type_name: String,
    /// `PID_DURABILITY`.
    pub durability: DurabilityQosPolicy,
    /// `PID_DURABILITY_SERVICE`.
    pub durability_service: DurabilityServiceQosPolicy,
    /// `PID_DEADLINE`.
    pub deadline: DeadlineQosPolicy,
    /// `PID_LATENCY_BUDGET`.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// `PID_LIVELINESS`.
    pub liveliness: LivelinessQosPolicy,
    /// `PID_RELIABILITY`.
    pub reliability: ReliabilityQosPolicy,
    /// `PID_TRANSPORT_PRIORITY`.
    pub transport_priority: TransportPriorityQosPolicy,
    /// `PID_LIFESPAN`.
    pub lifespan: LifespanQosPolicy,
    /// `PID_DESTINATION_ORDER`.
    pub destination_order: DestinationOrderQosPolicy,
    /// `PID_HISTORY`.
    pub history: HistoryQosPolicy,
    /// `PID_RESOURCE_LIMITS`.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// `PID_OWNERSHIP`.
    pub ownership: OwnershipQosPolicy,
    /// `PID_TOPIC_DATA`.
    pub topic_data: TopicDataQosPolicy,
    /// `PID_DATA_REPRESENTATION` (XTypes 1.3 §7.6.3.1.1); empty when the
    /// announcement has none.
    pub data_representation: Vec<i16>,
    /// `PID_TYPE_INFORMATION` (XTypes 1.3 §7.6.3.2.2), kept as the octets
    /// of the parameter's value.
    pub type_information: Option<Vec<u8>>,
    /// `type_id` (`TypeIdV1`, `PID` 0x0069), XTypes 1.3 Annex D.
    pub type_id_v1: Option<zerodds_types::type_object_v1::TypeId>,
    /// `type` (`TypeObjectV1`, `PID` 0x0072), XTypes 1.3 Annex D and
    /// §7.6.3.2.2.
    pub type_object_v1: Option<zerodds_types::type_object_v1::TypeObjectV1>,
    /// `PID_VENDOR_ID`, as the announcement states it.
    pub vendor_id: Option<VendorId>,
}

impl TopicBuiltinTopicData {
    /// A topic with every policy at its default (DDS 1.4 §2.2.3, Topic
    /// column).
    #[must_use]
    pub fn new(key: Guid, topic_name: &str, type_name: &str) -> Self {
        Self {
            key,
            topic_name: String::from(topic_name),
            type_name: String::from(type_name),
            durability: DurabilityQosPolicy::default(),
            durability_service: DurabilityServiceQosPolicy::default(),
            deadline: DeadlineQosPolicy::default(),
            latency_budget: LatencyBudgetQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            reliability: ReliabilityQosPolicy::default(),
            transport_priority: TransportPriorityQosPolicy::default(),
            lifespan: LifespanQosPolicy::default(),
            destination_order: DestinationOrderQosPolicy::default(),
            history: HistoryQosPolicy::default(),
            resource_limits: ResourceLimitsQosPolicy::default(),
            ownership: OwnershipQosPolicy::default(),
            topic_data: TopicDataQosPolicy::default(),
            data_representation: Vec::new(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            vendor_id: Some(VendorId::ZERODDS),
        }
    }

    /// The `PL_CDR_LE` serialized payload of a DATA submessage, with its
    /// encapsulation header.
    ///
    /// # Errors
    /// [`WireError::ValueOutOfRange`] for a string or a sequence past what
    /// its length word holds.
    pub fn to_pl_cdr_le(&self) -> Result<Vec<u8>, WireError> {
        let params = self.parameters()?;
        let mut out = Vec::new();
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&params.to_bytes(true));
        Ok(out)
    }

    fn parameters(&self) -> Result<ParameterList, WireError> {
        let mut params = ParameterList::new();
        params.push(Parameter::new(
            pid::ENDPOINT_GUID,
            self.key.to_bytes().to_vec(),
        ));
        params.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_bounded_cdr_string_le(&self.topic_name, STRING_256_MAX)?,
        ));
        params.push(Parameter::new(
            pid::TYPE_NAME,
            encode_bounded_cdr_string_le(&self.type_name, STRING_256_MAX)?,
        ));
        // A policy at its default is left out; the reader takes the
        // default for it (DDSI-RTPS 2.5 §9.4.2.11, Table 9.19).
        let default = Self::new(self.key, "", "");
        if self.topic_data != default.topic_data {
            let mut topic_data = Writer::new(Endianness::Little);
            self.topic_data.encode_into(&mut topic_data).map_err(|_| {
                WireError::ValueOutOfRange {
                    message: "TOPIC_DATA longer than its length word holds",
                }
            })?;
            params.push(Parameter::new(pid::TOPIC_DATA, padded(topic_data)));
        }
        macro_rules! off_default {
            ($field:ident, $pid:expr) => {
                if self.$field != default.$field {
                    params.push(policy($pid, |w| self.$field.encode_into(w)));
                }
            };
        }
        off_default!(durability, pid::DURABILITY);
        off_default!(durability_service, pid::DURABILITY_SERVICE);
        off_default!(deadline, pid::DEADLINE);
        off_default!(latency_budget, pid::LATENCY_BUDGET);
        off_default!(liveliness, pid::LIVELINESS);
        off_default!(reliability, pid::RELIABILITY);
        off_default!(transport_priority, pid::TRANSPORT_PRIORITY);
        off_default!(lifespan, pid::LIFESPAN);
        off_default!(destination_order, pid::DESTINATION_ORDER);
        off_default!(history, pid::HISTORY);
        off_default!(resource_limits, pid::RESOURCE_LIMITS);
        off_default!(ownership, pid::OWNERSHIP);
        if !self.data_representation.is_empty() {
            let count = u32::try_from(self.data_representation.len()).map_err(|_| {
                WireError::ValueOutOfRange {
                    message: "DATA_REPRESENTATION longer than its length word holds",
                }
            })?;
            let mut w = Writer::new(Endianness::Little);
            w.write_u32(count);
            for id in &self.data_representation {
                w.write_u16(*id as u16);
            }
            params.push(Parameter::new(pid::DATA_REPRESENTATION, padded(w)));
        }
        if let Some(type_information) = &self.type_information {
            params.push(Parameter::new(
                pid::TYPE_INFORMATION,
                type_information.clone(),
            ));
        }
        crate::type_v1::push(
            &mut params,
            self.type_id_v1.as_ref(),
            self.type_object_v1.as_ref(),
        )?;
        if let Some(vendor) = self.vendor_id {
            params.push(Parameter::new(
                pid::VENDOR_ID,
                alloc::vec![vendor.0[0], vendor.0[1], 0, 0],
            ));
        }
        Ok(params)
    }

    /// Reads a `PL_CDR_LE` or `PL_CDR_BE` payload, encapsulation header
    /// included, from a message `sender` sent.
    ///
    /// # Errors
    /// [`WireError`] for another encapsulation, a missing key or name, or
    /// a parameter that is present and does not decode.
    pub fn from_pl_cdr(bytes: &[u8], sender: VendorId) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        }
        let little_endian = match &bytes[..2] {
            b if b == ENCAPSULATION_PL_CDR_LE => true,
            [0x00, 0x02] => false,
            other => {
                return Err(WireError::UnsupportedEncapsulation {
                    kind: [other[0], other[1]],
                });
            }
        };
        let endianness = if little_endian {
            Endianness::Little
        } else {
            Endianness::Big
        };
        let pl = ParameterList::from_bytes(&bytes[4..], little_endian)?;
        let name = |id: u16, what: &'static str| -> Result<String, WireError> {
            pl.find(id)
                .map(|p| decode_bounded_cdr_string(&p.value, little_endian, STRING_256_MAX))
                .transpose()?
                .ok_or(WireError::ValueOutOfRange { message: what })
        };
        let key = topic_key(&pl, sender).ok_or(WireError::ValueOutOfRange {
            message: "ENDPOINT_GUID missing",
        })?;
        let mut topic = Self::new(
            key,
            &name(pid::TOPIC_NAME, "TOPIC_NAME missing")?,
            &name(pid::TYPE_NAME, "TYPE_NAME missing")?,
        );
        topic.vendor_id = None;
        macro_rules! read_policy {
            ($field:ident, $pid:expr, $ty:ty) => {
                if let Some(p) = pl.find($pid) {
                    let mut r = Reader::new(&p.value, endianness);
                    topic.$field =
                        <$ty>::decode_from(&mut r).map_err(|_| WireError::ValueOutOfRange {
                            message: concat!(stringify!($field), " does not decode"),
                        })?;
                }
            };
        }
        read_policy!(durability, pid::DURABILITY, DurabilityQosPolicy);
        read_policy!(
            durability_service,
            pid::DURABILITY_SERVICE,
            DurabilityServiceQosPolicy
        );
        read_policy!(deadline, pid::DEADLINE, DeadlineQosPolicy);
        read_policy!(latency_budget, pid::LATENCY_BUDGET, LatencyBudgetQosPolicy);
        read_policy!(liveliness, pid::LIVELINESS, LivelinessQosPolicy);
        read_policy!(reliability, pid::RELIABILITY, ReliabilityQosPolicy);
        read_policy!(
            transport_priority,
            pid::TRANSPORT_PRIORITY,
            TransportPriorityQosPolicy
        );
        read_policy!(lifespan, pid::LIFESPAN, LifespanQosPolicy);
        read_policy!(
            destination_order,
            pid::DESTINATION_ORDER,
            DestinationOrderQosPolicy
        );
        read_policy!(history, pid::HISTORY, HistoryQosPolicy);
        read_policy!(
            resource_limits,
            pid::RESOURCE_LIMITS,
            ResourceLimitsQosPolicy
        );
        read_policy!(ownership, pid::OWNERSHIP, OwnershipQosPolicy);
        read_policy!(topic_data, pid::TOPIC_DATA, TopicDataQosPolicy);
        if let Some(p) = pl.find(pid::DATA_REPRESENTATION) {
            let mut r = Reader::new(&p.value, endianness);
            let bad = |_| WireError::ValueOutOfRange {
                message: "DATA_REPRESENTATION does not decode",
            };
            let count = r.read_u32().map_err(bad)?;
            for _ in 0..count {
                topic
                    .data_representation
                    .push(r.read_u16().map_err(bad)? as i16);
            }
        }
        topic.type_information = pl.find(pid::TYPE_INFORMATION).map(|p| p.value.clone());
        (topic.type_id_v1, topic.type_object_v1) =
            crate::type_v1::read(&pl, endianness == Endianness::Little);
        topic.vendor_id = pl
            .find(pid::VENDOR_ID)
            .filter(|p| p.value.len() >= 2)
            .map(|p| VendorId([p.value[0], p.value[1]]));
        Ok(topic)
    }
}

/// The key a topic announcement or dispose from `sender` carries:
/// `PID_ENDPOINT_GUID`, or Cyclone's `PID_CYCLONE_TOPIC_GUID` in a message
/// from Cyclone.
#[must_use]
pub fn topic_key(pl: &ParameterList, sender: VendorId) -> Option<Guid> {
    let guid = pl.find(pid::ENDPOINT_GUID).or_else(|| {
        (sender == VendorId::CYCLONE)
            .then(|| pl.find(PID_CYCLONE_TOPIC_GUID))
            .flatten()
    })?;
    Some(Guid::from_bytes(guid.value.as_slice().try_into().ok()?))
}

/// One policy's parameter, its value written little-endian by `write`.
fn policy(id: u16, write: impl FnOnce(&mut Writer)) -> Parameter {
    let mut w = Writer::new(Endianness::Little);
    write(&mut w);
    Parameter::new(id, padded(w))
}

/// A writer's octets, padded to the four-octet boundary a ParameterList
/// value ends on (DDSI-RTPS 2.5 §9.4.2.11).
fn padded(w: Writer) -> Vec<u8> {
    let mut out = w.finish();
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}
