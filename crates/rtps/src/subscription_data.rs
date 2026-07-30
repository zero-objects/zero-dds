// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SubscriptionBuiltinTopicData (DDSI-RTPS 2.5 §8.5.4.3, §9.6.2.2.4).
//!
//! Content of the SEDP subscriptions DATA submessage. Analogous to
//! [`crate::publication_data::PublicationBuiltinTopicData`] — the same
//! wire layout, minus writer-specific fields like
//! lifespan/ownership-strength, plus reader-specific ones like the
//! time-based filter.
//!
//! GUIDs + topic/type
//! + Durability + Reliability.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::endpoint_security_info::EndpointSecurityInfo;
use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::participant_data::{Duration, ENCAPSULATION_PL_CDR_LE};
use crate::publication_data::{
    DurabilityKind, PresentationQosPolicy, ReliabilityKind, ReliabilityQos, collect_locator_params,
    decode_cdr_string, decode_duration, decode_i32, decode_liveliness, decode_partition,
    decode_u32, encode_cdr_string_le, encode_duration_le, encode_liveliness_le,
    encode_locator_params, encode_partition_le, encode_u32_le, guid_from_param,
};
use crate::wire_types::{Guid, Locator};

/// Discovered Subscription / lokaler DataReader — Subset.
///
/// Wire-identical to [`PublicationBuiltinTopicData`] in phase 1; a
/// separate type so the SEDP publications and subscriptions caches stay
/// clearly distinguishable and extensions (e.g. `expects_inline_qos`,
/// `time_based_filter`) are not pulled through the publication type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionBuiltinTopicData {
    /// Endpoint GUID (= reader GUID).
    pub key: Guid,
    /// GUID of the participant the reader belongs to.
    pub participant_key: Guid,
    /// Topic name.
    pub topic_name: String,
    /// IDL type name.
    pub type_name: String,
    /// Durability-QoS.
    pub durability: DurabilityKind,
    /// Reliability-QoS.
    pub reliability: ReliabilityQos,
    /// Ownership-QoS (Spec §2.2.3.23). Default Shared.
    pub ownership: zerodds_qos::OwnershipKind,
    /// Liveliness-QoS (Spec §2.2.3.11).
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// Deadline-QoS (Spec §2.2.3.7).
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// LatencyBudget-QoS (Spec §2.2.3.10, PID 0x0027) — requested duration.
    /// RxO-matched: `offered.duration <= requested.duration`.
    pub latency_budget: zerodds_qos::LatencyBudgetQosPolicy,
    /// DestinationOrder-QoS (Spec §2.2.3.18, PID 0x0025) — requested kind.
    /// RxO-matched: `offered.kind >= requested.kind`.
    pub destination_order: zerodds_qos::DestinationOrderQosPolicy,
    /// Presentation-QoS (Spec §2.2.3.6, PID 0x0021). See
    /// [`crate::publication_data::PublicationBuiltinTopicData::presentation`].
    pub presentation: crate::publication_data::PresentationQosPolicy,
    /// Partition-QoS (Spec §2.2.3.13).
    pub partition: Vec<String>,
    /// UserData-QoS (Spec §2.2.3.1) — opaque sequence<octet>.
    pub user_data: Vec<u8>,
    /// TopicData-QoS (Spec §2.2.3.3) — opaque sequence<octet>.
    pub topic_data: Vec<u8>,
    /// GroupData-QoS (Spec §2.2.3.2) — opaque sequence<octet>.
    pub group_data: Vec<u8>,
    /// Type information as opaque bytes (XTypes §7.6.3.2.2).
    pub type_information: Option<alloc::vec::Vec<u8>>,
    /// Akzeptierte Data-Representations.
    pub data_representation: alloc::vec::Vec<i16>,
    /// Content-filter property (DDSI-RTPS §9.6.3.4 Table 9.14). Only
    /// set if the reader was created as a `ContentFilteredTopic`.
    pub content_filter: Option<ContentFilterProperty>,
    /// Endpoint security info (PID 0x1004, DDS-Security 1.1 §7.4.1.5).
    /// `None` for legacy peers. WP 4H-c matching checks writer/reader
    /// pairs for protection compatibility.
    pub security_info: Option<EndpointSecurityInfo>,
    /// PID_SERVICE_INSTANCE_NAME (DDS-RPC 1.0 §7.8.2) — logical
    /// service-instance name of an RPC endpoint.
    pub service_instance_name: Option<String>,
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.8.2) — GUID of the
    /// counterpart endpoint (e.g. reply reader → reply writer).
    pub related_entity_guid: Option<Guid>,
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.8.2).
    pub topic_aliases: Option<Vec<String>>,
    /// PID_ZERODDS_TYPE_ID (vendor PID 0x8002) — reader type identifier
    /// for XTypes-aware matching (see `PublicationBuiltinTopicData`).
    pub type_identifier: zerodds_types::TypeIdentifier,
    /// Endpoint unicast locators (DDSI-RTPS 2.5 §8.5.3.2:
    /// `DiscoveredReaderData.readerProxy.unicastLocatorList`). Where
    /// peers send user data to *this* reader endpoint. Empty = the peer
    /// falls back to the participant `DEFAULT_UNICAST_LOCATOR` from
    /// SPDP (§8.5.5). OpenDDS stores the real user locator exclusively
    /// here.
    pub unicast_locators: Vec<Locator>,
    /// Endpoint-Multicast-Locators (DDSI-RTPS 2.5 §8.5.3.2).
    pub multicast_locators: Vec<Locator>,
}

/// Content-filter property — projects a content-filtered-topic
/// reference over SEDP.
///
/// Spec: OMG DDS 1.4 §9.6.3.4 Table 9.14 / DDSI-RTPS 2.5 §9.6.3.4.
/// Five strings + one string sequence, all CDR-serialized.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContentFilterProperty {
    /// Name of the content-filtered topic (derivable from the reader
    /// identifier).
    pub content_filtered_topic_name: String,
    /// Name of the underlying topic (same as `topic_name` in the
    /// SubscriptionBuiltinTopicData).
    pub related_topic_name: String,
    /// Filter class, e.g. `"DDSSQL"`. For type constants see
    /// [`filter_class`].
    pub filter_class_name: String,
    /// Filter expression (SQL subset).
    pub filter_expression: String,
    /// Positional parameters (`%0`, `%1`, ...).
    pub expression_parameters: Vec<String>,
}

/// Standard filter-class names.
pub mod filter_class {
    /// OMG SQL filter (DDSSQL) — the only standardized kind.
    pub const DDSSQL: &str = "DDSSQL";
}

/// Encodes a ContentFilterProperty as a PL value (five CDR strings +
/// sequence<string>).
///
/// # Errors
/// `ValueOutOfRange` if a string is too long.
pub fn encode_content_filter_property_le(
    cfp: &ContentFilterProperty,
) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    out.extend_from_slice(&encode_cdr_string_le(&cfp.content_filtered_topic_name)?);
    out.extend_from_slice(&encode_cdr_string_le(&cfp.related_topic_name)?);
    out.extend_from_slice(&encode_cdr_string_le(&cfp.filter_class_name)?);
    out.extend_from_slice(&encode_cdr_string_le(&cfp.filter_expression)?);
    out.extend_from_slice(&encode_partition_le(&cfp.expression_parameters)?);
    Ok(out)
}

/// Decodes a ContentFilterProperty from a PL value. `None` on a wire error.
pub fn decode_content_filter_property(
    value: &[u8],
    little_endian: bool,
) -> Option<ContentFilterProperty> {
    let (s1, rest1) = take_cdr_string(value, little_endian)?;
    let (s2, rest2) = take_cdr_string(rest1, little_endian)?;
    let (s3, rest3) = take_cdr_string(rest2, little_endian)?;
    let (s4, rest4) = take_cdr_string(rest3, little_endian)?;
    let params = decode_partition(rest4, little_endian)?;
    Some(ContentFilterProperty {
        content_filtered_topic_name: s1,
        related_topic_name: s2,
        filter_class_name: s3,
        filter_expression: s4,
        expression_parameters: params,
    })
}

/// Helper: reads **one** CDR string from a byte slice and returns the
/// rest after 4-byte align padding.
fn take_cdr_string(bytes: &[u8], little_endian: bool) -> Option<(String, &[u8])> {
    if bytes.len() < 4 {
        return None;
    }
    let mut lb = [0u8; 4];
    lb.copy_from_slice(&bytes[..4]);
    let len = if little_endian {
        u32::from_le_bytes(lb)
    } else {
        u32::from_be_bytes(lb)
    } as usize;
    let consumed_raw = 4 + len;
    if consumed_raw > bytes.len() {
        return None;
    }
    let s = decode_cdr_string(&bytes[..consumed_raw], little_endian).ok()?;
    // 4-byte align to the next element.
    let padded = (consumed_raw + 3) & !3;
    let next = padded.min(bytes.len());
    Some((s, &bytes[next..]))
}

impl SubscriptionBuiltinTopicData {
    /// Encodes to PL_CDR_LE bytes (with a 4-byte encapsulation header).
    ///
    /// Implemented directly (not delegated via
    /// [`PublicationBuiltinTopicData`]) so that extension PIDs that are
    /// writer-only (e.g. `PID_LIFESPAN`, `PID_OWNERSHIP_STRENGTH`) do
    /// not accidentally end up in the subscription payload.
    ///
    /// # Errors
    /// `ValueOutOfRange` if a string is longer than u32::MAX.
    pub fn to_pl_cdr_le(&self) -> Result<Vec<u8>, WireError> {
        let mut params = ParameterList::new();

        params.push(Parameter::new(
            pid::PARTICIPANT_GUID,
            self.participant_key.to_bytes().to_vec(),
        ));
        params.push(Parameter::new(
            pid::ENDPOINT_GUID,
            self.key.to_bytes().to_vec(),
        ));
        params.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_cdr_string_le(&self.topic_name)?,
        ));
        params.push(Parameter::new(
            pid::TYPE_NAME,
            encode_cdr_string_le(&self.type_name)?,
        ));
        params.push(Parameter::new(
            pid::DURABILITY,
            (self.durability as u32).to_le_bytes().to_vec(),
        ));

        let mut rel = Vec::with_capacity(12);
        rel.extend_from_slice(&(self.reliability.kind as u32).to_le_bytes());
        rel.extend_from_slice(&self.reliability.max_blocking_time.to_bytes_le());
        params.push(Parameter::new(pid::RELIABILITY, rel));

        // OWNERSHIP (reader-side: "ich akzeptiere nur diesen Ownership-Modus").
        params.push(Parameter::new(
            pid::OWNERSHIP,
            encode_u32_le(self.ownership as u32).to_vec(),
        ));

        // LIVELINESS — requested lease.
        params.push(Parameter::new(
            pid::LIVELINESS,
            encode_liveliness_le(self.liveliness),
        ));

        // DEADLINE — requested period.
        params.push(Parameter::new(
            pid::DEADLINE,
            encode_duration_le(self.deadline.period).to_vec(),
        ));

        // LATENCY_BUDGET — requested duration (spec §2.2.3.10 / PID 0x0027).
        params.push(Parameter::new(
            pid::LATENCY_BUDGET,
            encode_duration_le(self.latency_budget.duration).to_vec(),
        ));

        // DESTINATION_ORDER — requested kind (spec §2.2.3.18 / PID 0x0025).
        params.push(Parameter::new(
            pid::DESTINATION_ORDER,
            encode_u32_le(self.destination_order.kind as u32).to_vec(),
        ));

        // PRESENTATION — requested access_scope/coherent/ordered (spec
        // §2.2.3.6 / PID 0x0021). Symmetric to the writer-side encoding in
        // `publication_data::encode_into`.
        {
            let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little);
            self.presentation
                .encode_into(&mut w)
                .map_err(|_| WireError::ValueOutOfRange {
                    message: "presentation encoding failed",
                })?;
            params.push(Parameter::new(pid::PRESENTATION, w.into_bytes()));
        }

        // PARTITION.
        if !self.partition.is_empty() {
            params.push(Parameter::new(
                pid::PARTITION,
                encode_partition_le(&self.partition)?,
            ));
        }

        // USER_DATA / TOPIC_DATA / GROUP_DATA — opaque sequence<octet>.
        if !self.user_data.is_empty() {
            params.push(Parameter::new(
                pid::USER_DATA,
                crate::publication_data::encode_octet_seq_le(&self.user_data)?,
            ));
        }
        if !self.topic_data.is_empty() {
            params.push(Parameter::new(
                pid::TOPIC_DATA,
                crate::publication_data::encode_octet_seq_le(&self.topic_data)?,
            ));
        }
        if !self.group_data.is_empty() {
            params.push(Parameter::new(
                pid::GROUP_DATA,
                crate::publication_data::encode_octet_seq_le(&self.group_data)?,
            ));
        }

        if let Some(ti) = &self.type_information {
            params.push(Parameter::new(pid::TYPE_INFORMATION, ti.clone()));
        }

        if let Some(cfp) = &self.content_filter {
            params.push(Parameter::new(
                pid::CONTENT_FILTER_PROPERTY,
                encode_content_filter_property_le(cfp)?,
            ));
        }

        if let Some(info) = self.security_info {
            params.push(Parameter::new(
                pid::ENDPOINT_SECURITY_INFO,
                info.to_bytes(true).to_vec(),
            ));
        }

        // ----------------------------------------------------------------
        // DDS-RPC 1.0 discovery PIDs (§7.8.2) — only if set.
        // ----------------------------------------------------------------
        if let Some(name) = &self.service_instance_name {
            params.push(Parameter::new(
                pid::SERVICE_INSTANCE_NAME,
                encode_cdr_string_le(name)?,
            ));
        }
        if let Some(guid) = self.related_entity_guid {
            params.push(Parameter::new(
                pid::RELATED_ENTITY_GUID,
                guid.to_bytes().to_vec(),
            ));
        }
        if let Some(aliases) = &self.topic_aliases {
            params.push(Parameter::new(
                pid::TOPIC_ALIASES,
                encode_partition_le(aliases)?,
            ));
        }

        // PID_ZERODDS_TYPE_ID (F-TYPES-3 Wire-up).
        if self.type_identifier != zerodds_types::TypeIdentifier::None {
            let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little);
            self.type_identifier
                .encode_into(&mut w)
                .map_err(|_| WireError::ValueOutOfRange {
                    message: "type_identifier encoding failed",
                })?;
            params.push(Parameter::new(pid::ZERODDS_TYPE_ID, w.into_bytes()));
        }

        if !self.data_representation.is_empty() {
            let mut dr = Vec::with_capacity(4 + 2 * self.data_representation.len());
            let len = u32::try_from(self.data_representation.len()).map_err(|_| {
                WireError::ValueOutOfRange {
                    message: "data_representation length exceeds u32::MAX",
                }
            })?;
            dr.extend_from_slice(&len.to_le_bytes());
            for rep in &self.data_representation {
                dr.extend_from_slice(&rep.to_le_bytes());
            }
            params.push(Parameter::new(pid::DATA_REPRESENTATION, dr));
        }

        // Endpoint locators (§8.5.3.2) — one parameter per locator.
        encode_locator_params(&mut params, pid::UNICAST_LOCATOR, &self.unicast_locators);
        encode_locator_params(
            &mut params,
            pid::MULTICAST_LOCATOR,
            &self.multicast_locators,
        );

        let mut out = Vec::with_capacity(params.parameters.len() * 24 + 16);
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&params.to_bytes(true));
        Ok(out)
    }

    /// Decoded from PL_CDR_LE bytes (with encapsulation header).
    ///
    /// # Errors
    /// `UnexpectedEof` on too-short bytes,
    /// `UnsupportedEncapsulation` on an unknown encoding,
    /// `ValueOutOfRange` if mandatory PIDs are missing.
    pub fn from_pl_cdr_le(bytes: &[u8]) -> Result<Self, WireError> {
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
        let pl = ParameterList::from_bytes(&bytes[4..], little_endian)?;

        let key = pl
            .find(pid::ENDPOINT_GUID)
            .and_then(guid_from_param)
            .ok_or(WireError::ValueOutOfRange {
                message: "ENDPOINT_GUID missing or wrong length",
            })?;
        let participant_key = pl
            .find(pid::PARTICIPANT_GUID)
            .and_then(guid_from_param)
            .unwrap_or_else(|| Guid::new(key.prefix, crate::wire_types::EntityId::PARTICIPANT));
        let topic_name = pl
            .find(pid::TOPIC_NAME)
            .map(|p| decode_cdr_string(&p.value, little_endian))
            .transpose()?
            .ok_or(WireError::ValueOutOfRange {
                message: "TOPIC_NAME missing",
            })?;
        let type_name = pl
            .find(pid::TYPE_NAME)
            .map(|p| decode_cdr_string(&p.value, little_endian))
            .transpose()?
            .ok_or(WireError::ValueOutOfRange {
                message: "TYPE_NAME missing",
            })?;

        let durability = pl
            .find(pid::DURABILITY)
            .and_then(|p| {
                if p.value.len() >= 4 {
                    let mut b = [0u8; 4];
                    b.copy_from_slice(&p.value[..4]);
                    Some(DurabilityKind::from_u32(if little_endian {
                        u32::from_le_bytes(b)
                    } else {
                        u32::from_be_bytes(b)
                    }))
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let reliability = pl
            .find(pid::RELIABILITY)
            .and_then(|p| {
                if p.value.len() >= 12 {
                    let mut k = [0u8; 4];
                    k.copy_from_slice(&p.value[..4]);
                    let kind = ReliabilityKind::from_u32(if little_endian {
                        u32::from_le_bytes(k)
                    } else {
                        u32::from_be_bytes(k)
                    });
                    let mut d = [0u8; 8];
                    d.copy_from_slice(&p.value[4..12]);
                    let max_blocking_time = if little_endian {
                        Duration::from_bytes_le(d)
                    } else {
                        let mut s = [0u8; 4];
                        s.copy_from_slice(&d[..4]);
                        let mut f = [0u8; 4];
                        f.copy_from_slice(&d[4..]);
                        Duration {
                            seconds: i32::from_be_bytes(s),
                            fraction: u32::from_be_bytes(f),
                        }
                    };
                    Some(ReliabilityQos {
                        kind,
                        max_blocking_time,
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let ownership = pl
            .find(pid::OWNERSHIP)
            .and_then(|p| decode_u32(&p.value, little_endian))
            .map(zerodds_qos::OwnershipKind::from_u32)
            .unwrap_or_default();
        // OWNERSHIP_STRENGTH ignored (writer-only).
        let _ = decode_i32;

        let liveliness = pl
            .find(pid::LIVELINESS)
            .and_then(|p| decode_liveliness(&p.value, little_endian))
            .unwrap_or_default();

        let deadline = pl
            .find(pid::DEADLINE)
            .and_then(|p| decode_duration(&p.value, little_endian))
            .map(|period| zerodds_qos::DeadlineQosPolicy { period })
            .unwrap_or_default();

        let latency_budget = pl
            .find(pid::LATENCY_BUDGET)
            .and_then(|p| decode_duration(&p.value, little_endian))
            .map(|duration| zerodds_qos::LatencyBudgetQosPolicy { duration })
            .unwrap_or_default();

        let destination_order = pl
            .find(pid::DESTINATION_ORDER)
            .and_then(|p| decode_u32(&p.value, little_endian))
            .map(|v| zerodds_qos::DestinationOrderQosPolicy {
                kind: zerodds_qos::DestinationOrderKind::from_u32(v),
            })
            .unwrap_or_default();

        // PRESENTATION (PID 0x0021) — see `publication_data::decode_from`.
        let presentation = pl
            .find(pid::PRESENTATION)
            .and_then(|p| {
                let endianness = if little_endian {
                    zerodds_cdr::Endianness::Little
                } else {
                    zerodds_cdr::Endianness::Big
                };
                let mut r = zerodds_cdr::BufferReader::new(&p.value, endianness);
                PresentationQosPolicy::decode_from(&mut r).ok()
            })
            .unwrap_or_default();

        let partition = pl
            .find(pid::PARTITION)
            .and_then(|p| decode_partition(&p.value, little_endian))
            .unwrap_or_default();

        let user_data = pl
            .find(pid::USER_DATA)
            .and_then(|p| crate::publication_data::decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();
        let topic_data = pl
            .find(pid::TOPIC_DATA)
            .and_then(|p| crate::publication_data::decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();
        let group_data = pl
            .find(pid::GROUP_DATA)
            .and_then(|p| crate::publication_data::decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();

        let type_information = pl.find(pid::TYPE_INFORMATION).map(|p| p.value.clone());
        let content_filter = pl
            .find(pid::CONTENT_FILTER_PROPERTY)
            .and_then(|p| decode_content_filter_property(&p.value, little_endian));

        let security_info = pl
            .find(pid::ENDPOINT_SECURITY_INFO)
            .and_then(|p| EndpointSecurityInfo::from_bytes(&p.value, little_endian).ok());

        let service_instance_name = pl
            .find(pid::SERVICE_INSTANCE_NAME)
            .map(|p| decode_cdr_string(&p.value, little_endian))
            .transpose()
            .ok()
            .flatten();
        let related_entity_guid = pl.find(pid::RELATED_ENTITY_GUID).and_then(guid_from_param);
        let topic_aliases = pl
            .find(pid::TOPIC_ALIASES)
            .and_then(|p| decode_partition(&p.value, little_endian));

        let type_identifier = pl
            .find(pid::ZERODDS_TYPE_ID)
            .and_then(|p| {
                let mut r =
                    zerodds_cdr::BufferReader::new(&p.value, zerodds_cdr::Endianness::Little);
                zerodds_types::TypeIdentifier::decode_from(&mut r).ok()
            })
            .unwrap_or_default();

        let data_representation = pl
            .find(pid::DATA_REPRESENTATION)
            .map(|p| {
                let v = &p.value;
                if v.len() < 4 {
                    return Vec::new();
                }
                let mut n_bytes = [0u8; 4];
                n_bytes.copy_from_slice(&v[..4]);
                let n = if little_endian {
                    u32::from_le_bytes(n_bytes)
                } else {
                    u32::from_be_bytes(n_bytes)
                } as usize;
                let cap = n.min(v.len().saturating_sub(4) / 2);
                let mut reps = Vec::with_capacity(cap);
                for i in 0..n {
                    let off = 4 + i * 2;
                    if off + 2 > v.len() {
                        break;
                    }
                    let mut b = [0u8; 2];
                    b.copy_from_slice(&v[off..off + 2]);
                    reps.push(if little_endian {
                        i16::from_le_bytes(b)
                    } else {
                        i16::from_be_bytes(b)
                    });
                }
                reps
            })
            .unwrap_or_default();

        Ok(Self {
            key,
            participant_key,
            topic_name,
            type_name,
            durability,
            reliability,
            ownership,
            liveliness,
            deadline,
            latency_budget,
            destination_order,
            presentation,
            partition,
            user_data,
            topic_data,
            group_data,
            type_information,
            data_representation,
            content_filter,
            security_info,
            service_instance_name,
            related_entity_guid,
            topic_aliases,
            type_identifier,
            unicast_locators: collect_locator_params(&pl, pid::UNICAST_LOCATOR, little_endian),
            multicast_locators: collect_locator_params(&pl, pid::MULTICAST_LOCATOR, little_endian),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::participant_data::Duration;
    use crate::publication_data::ReliabilityKind;
    use crate::wire_types::{EntityId, GuidPrefix};

    #[test]
    fn roundtrip_le() {
        let s = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
            topic_name: "ChatterTopic".into(),
            type_name: "std_msgs::String".into(),
            durability: DurabilityKind::TransientLocal,
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_secs(5),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
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
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn security_info_roundtrip() {
        use crate::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};
        let s = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([4; 12]),
                EntityId::user_reader_with_key([0x10, 0x20, 0x30]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([4; 12]), EntityId::PARTICIPANT),
            topic_name: "ST".into(),
            type_name: "Foo".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: Some(EndpointSecurityInfo {
                endpoint_security_attributes: attrs::IS_VALID | attrs::IS_PAYLOAD_PROTECTED,
                plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                    | plugin_attrs::IS_PAYLOAD_ENCRYPTED,
            }),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.security_info, s.security_info);
    }

    #[test]
    fn content_filter_property_roundtrip_le() {
        let cfp = ContentFilterProperty {
            content_filtered_topic_name: "FilteredShapes".into(),
            related_topic_name: "Square".into(),
            filter_class_name: filter_class::DDSSQL.into(),
            filter_expression: "color = %0 AND x > %1".into(),
            expression_parameters: alloc::vec!["'RED'".into(), "50".into()],
        };
        let bytes = encode_content_filter_property_le(&cfp).unwrap();
        let decoded = decode_content_filter_property(&bytes, true).unwrap();
        assert_eq!(decoded, cfp);
    }

    #[test]
    fn subscription_with_content_filter_roundtrip_le() {
        let cfp = ContentFilterProperty {
            content_filtered_topic_name: "Filt".into(),
            related_topic_name: "Square".into(),
            filter_class_name: "DDSSQL".into(),
            filter_expression: "x > %0".into(),
            expression_parameters: alloc::vec!["42".into()],
        };
        let s = SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([3; 12]),
                EntityId::user_reader_with_key([1, 2, 3]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([3; 12]), EntityId::PARTICIPANT),
            topic_name: "Square".into(),
            type_name: "ShapeType".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: Some(cfp.clone()),
            security_info: None,
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
        };
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.content_filter, Some(cfp));
    }

    fn sample_sub() -> SubscriptionBuiltinTopicData {
        SubscriptionBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([5; 12]),
                EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([5; 12]), EntityId::PARTICIPANT),
            topic_name: "Calc_Reply".into(),
            type_name: "Calc::Reply".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
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
        }
    }

    #[test]
    fn rpc_discovery_pids_roundtrip_subscription() {
        let mut s = sample_sub();
        s.service_instance_name = Some("SrvInst".into());
        s.related_entity_guid = Some(Guid::new(
            GuidPrefix::from_bytes([5; 12]),
            EntityId::user_writer_with_key([1, 2, 3]),
        ));
        s.topic_aliases = Some(alloc::vec!["Alias1".into(), "Alias2".into()]);
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.service_instance_name, s.service_instance_name);
        assert_eq!(decoded.related_entity_guid, s.related_entity_guid);
        assert_eq!(decoded.topic_aliases, s.topic_aliases);
    }

    #[test]
    fn rpc_pids_optional_legacy_subscription_parses_ok() {
        let s = sample_sub();
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.service_instance_name.is_none());
        assert!(decoded.related_entity_guid.is_none());
        assert!(decoded.topic_aliases.is_none());
    }
}
