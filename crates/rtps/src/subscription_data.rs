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
    decode_u32, decoded_vendor_id, encode_cdr_string_le, encode_duration_le, encode_liveliness_le,
    encode_locator_params, encode_partition_le, encode_u32_le, guid_from_param,
    service_instance_name_param, topic_aliases_param,
};
use crate::wire_types::{Guid, Locator};

/// Discovered Subscription / lokaler DataReader — Subset.
///
/// Wire-identical to [`PublicationBuiltinTopicData`](crate::publication_data::PublicationBuiltinTopicData) in phase 1; a
/// separate type so the SEDP publications and subscriptions caches stay
/// clearly distinguishable and extensions (e.g. `expects_inline_qos`,
/// `time_based_filter`) are not pulled through the publication type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionBuiltinTopicData {
    /// Endpoint GUID (= reader GUID).
    pub key: Guid,
    /// GUID of the participant the reader belongs to.
    pub participant_key: Guid,
    /// The reader's Subscriber (`PID_GROUP_GUID`, DDSI-RTPS 2.5 Table 9.19
    /// `ReaderProxy::remoteGroupGuid`); `None` for `GUID_UNKNOWN`, its
    /// default.
    pub group_guid: Option<Guid>,
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
    /// TimeBasedFilter-QoS (Spec §2.2.3.12, PID 0x0004). DDS 1.4 §2.2.5
    /// lists it in the SubscriptionBuiltinTopicData; it is not RxO.
    pub time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy,
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
    /// UserData-QoS (Spec §2.2.3.1) — opaque `sequence<octet>`.
    pub user_data: Vec<u8>,
    /// TopicData-QoS (Spec §2.2.3.3) — opaque `sequence<octet>`.
    pub topic_data: Vec<u8>,
    /// GroupData-QoS (Spec §2.2.3.2) — opaque `sequence<octet>`.
    pub group_data: Vec<u8>,
    /// Type information as opaque bytes (XTypes §7.6.3.2.2).
    pub type_information: Option<alloc::vec::Vec<u8>>,
    /// `type_id` (`TypeIdV1`, `PID` 0x0069), XTypes 1.3 Annex D.
    pub type_id_v1: Option<zerodds_types::type_object_v1::TypeId>,
    /// `type` (`TypeObjectV1`, `PID` 0x0072), XTypes 1.3 Annex D and
    /// §7.6.3.2.2.
    pub type_object_v1: Option<zerodds_types::type_object_v1::TypeObjectV1>,
    /// `PID_ENTITY_NAME` (0x0062, DDSI-RTPS 2.5 Table 9.18, `EntityName_t`):
    /// the name of the reader. `None` when it announces none.
    pub entity_name: Option<alloc::string::String>,
    /// Akzeptierte Data-Representations.
    pub data_representation: alloc::vec::Vec<i16>,
    /// Content-filter property (DDSI-RTPS §9.6.3.4 Table 9.14). Only
    /// set if the reader was created as a `ContentFilteredTopic`.
    pub content_filter: Option<ContentFilterProperty>,
    /// `PID_CONTENT_FILTER_PROPERTY` was in the announcement and did not
    /// read: a name over the bound of `string<256>` (§9.3.2.1), or another
    /// fault. `content_filter` is then `None`, the reader is taken without a
    /// filter, and the receiver reports it. Always `false` for a locally built
    /// value; not encoded.
    pub content_filter_unreadable: bool,
    /// Endpoint security info (PID 0x1004, DDS-Security 1.1 §7.4.1.5).
    /// `None` for legacy peers. WP 4H-c matching checks writer/reader
    /// pairs for protection compatibility.
    pub security_info: Option<EndpointSecurityInfo>,
    /// `EndpointSecuritySymmetricCipherAlgorithmInfo` (DDS-Security 1.2
    /// §7.3.15, `PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO` 0x1013): the
    /// algorithms a peer must support to interoperate with the endpoint.
    /// `None`: not sent, and for a received one the
    /// `user_endpoints_default_required_mask` of its participant (§7.3.15.2).
    pub sym_cipher_algo_info:
        Option<crate::security_algo_info::EndpointSecuritySymmetricCipherAlgorithmInfo>,
    /// The `PropertyQosPolicy` of the endpoint, only the entries with
    /// `propagate = TRUE` (DDS-Security 1.2 §7.3.21, §7.5.1.7, §7.5.1.8,
    /// `PID_PROPERTY_LIST` 0x0059). Empty: not sent.
    pub properties: crate::property_list::WirePropertyList,
    /// The `DataTagQosPolicy` of the endpoint (DDS-Security 1.2 §7.3.21,
    /// `PID_DATA_TAGS` 0x1003). Empty: not sent.
    pub data_tags: crate::property_list::WireDataTags,
    /// PID_SERVICE_INSTANCE_NAME (DDS-RPC 1.0 §7.6.2.1) — logical
    /// service-instance name of an RPC endpoint.
    pub service_instance_name: Option<String>,
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.6.2.1) — GUID of the
    /// counterpart endpoint (e.g. reply reader → reply writer).
    pub related_entity_guid: Option<Guid>,
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.6.2.1).
    pub topic_aliases: Option<Vec<String>>,
    /// The reader's `TypeConsistencyEnforcementQosPolicy`
    /// (XTypes 1.3 §7.6.3.4, PID 0x0074).
    ///
    /// Reader-only: §7.6.3.4 gives the policy to the `DataReader` and
    /// there is no offered counterpart, so the writer's half of
    /// §7.6.3.4.2 has no policy of its own to decide with — it has to
    /// use this one, announced here. Until this field existed the
    /// writer took `TypeConsistencyEnforcement::default()` and every
    /// reader that had set a flag was judged against a policy it never
    /// asked for, in the one place the clause says the verdict has to
    /// be reached "for both the DataReader's Topic and the DataWriter's
    /// Topic".
    pub type_consistency: zerodds_types::qos::TypeConsistencyEnforcement,
    /// Whether the announcement carried PID 0x0074. A reader that did not
    /// is held to DISALLOW_TYPE_COERCION (XTypes 1.3 §7.6.3.4.1), which is
    /// what [`Self::type_consistency`] then says; this tells that apart from
    /// a reader that announced the same policy itself.
    pub type_consistency_announced: bool,
    /// `PID_VENDORID` (0x0016) — the vendor of the middleware that owns
    /// the announced reader, as the announcement itself states it. See the
    /// twin in [`crate::publication_data::PublicationBuiltinTopicData`] for
    /// the clauses and the measurement: it is read and written as it
    /// stands, and it is what decides whether 0x8002 is this tree's
    /// parameter.
    pub vendor_id: Option<crate::wire_types::VendorId>,
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
    /// `PID_EXPECTS_INLINE_QOS` (0x0043): the reader expects the writer's
    /// QoS in-line with every DATA (DDSI-RTPS 2.5 §8.4.7.5, §8.7.1). A
    /// stateless reader asks for it; this tree's readers keep the
    /// writer's QoS from discovery and do not. Absent is `false`
    /// (Table 9.19).
    pub expects_inline_qos: bool,
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
/// `sequence<string>`).
///
/// # Errors
/// `ValueOutOfRange` if a string is too long.
pub fn encode_content_filter_property_le(
    cfp: &ContentFilterProperty,
) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    // §9.3.2.1: the first three are `string<256>`; the expression is not
    // bounded.
    let bounded = |s: &str| {
        crate::publication_data::encode_bounded_cdr_string_le(
            s,
            crate::publication_data::STRING_256_MAX,
        )
    };
    out.extend_from_slice(&bounded(&cfp.content_filtered_topic_name)?);
    out.extend_from_slice(&bounded(&cfp.related_topic_name)?);
    out.extend_from_slice(&bounded(&cfp.filter_class_name)?);
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
    if [&s1, &s2, &s3]
        .iter()
        .any(|s| s.len() > crate::publication_data::STRING_256_MAX)
    {
        return None;
    }
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
    /// [`PublicationBuiltinTopicData`](crate::publication_data::PublicationBuiltinTopicData)) so that extension PIDs that are
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
        if let Some(group) = self.group_guid {
            params.push(Parameter::new(pid::GROUP_GUID, group.to_bytes().to_vec()));
        }
        params.push(Parameter::new(
            pid::ENDPOINT_GUID,
            self.key.to_bytes().to_vec(),
        ));
        params.push(Parameter::new(
            pid::TOPIC_NAME,
            crate::publication_data::encode_bounded_cdr_string_le(
                &self.topic_name,
                crate::publication_data::STRING_256_MAX,
            )?,
        ));
        params.push(Parameter::new(
            pid::TYPE_NAME,
            crate::publication_data::encode_bounded_cdr_string_le(
                &self.type_name,
                crate::publication_data::STRING_256_MAX,
            )?,
        ));
        params.push(Parameter::new(
            pid::DURABILITY,
            (self.durability as u32).to_le_bytes().to_vec(),
        ));

        let mut rel = Vec::with_capacity(12);
        rel.extend_from_slice(&(self.reliability.kind as u32).to_le_bytes());
        rel.extend_from_slice(&self.reliability.max_blocking_time.to_bytes_le());
        params.push(Parameter::new(pid::RELIABILITY, rel));

        // OWNERSHIP (reader-side: "I accept only this ownership mode").
        params.push(Parameter::new(
            pid::OWNERSHIP,
            encode_u32_le(self.ownership as u32).to_vec(),
        ));

        // LIVELINESS — requested lease — and DEADLINE — requested period —
        // are left out at their default (Duration_t INFINITE), as in
        // `PublicationBuiltinTopicData::parameters`: dust_dds 0.16.1 reads an
        // announced INFINITE as finite and refuses the reader (DEADLINE).
        if self.liveliness != zerodds_qos::LivelinessQosPolicy::default() {
            params.push(Parameter::new(
                pid::LIVELINESS,
                encode_liveliness_le(self.liveliness),
            ));
        }
        if self.deadline != zerodds_qos::DeadlineQosPolicy::default() {
            params.push(Parameter::new(
                pid::DEADLINE,
                encode_duration_le(self.deadline.period).to_vec(),
            ));
        }

        // TIME_BASED_FILTER — the reader's minimum separation (DDS 1.4
        // §2.2.5, PID 0x0004).
        params.push(Parameter::new(
            pid::TIME_BASED_FILTER,
            encode_duration_le(self.time_based_filter.minimum_separation).to_vec(),
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
            let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
            self.presentation.encode_into(&mut w);
            params.push(Parameter::new(pid::PRESENTATION, w.finish()));
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
        crate::type_v1::push(
            &mut params,
            self.type_id_v1.as_ref(),
            self.type_object_v1.as_ref(),
        )?;
        if let Some(name) = &self.entity_name {
            params.push(Parameter::new(
                pid::ENTITY_NAME,
                crate::publication_data::encode_cdr_string_le(name)?,
            ));
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
        if let Some(info) = self.sym_cipher_algo_info {
            params.push(Parameter::new(
                pid::ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO,
                info.to_bytes(true).to_vec(),
            ));
        }
        if !self.data_tags.is_empty() {
            params.push(Parameter::new(pid::DATA_TAGS, self.data_tags.encode(true)?));
        }
        // §7.3.21: the `propagate = TRUE` entries of the PropertyQosPolicy.
        if !self.properties.is_empty() || !self.properties.binary_entries.is_empty() {
            params.push(Parameter::new(
                pid::PROPERTY_LIST,
                self.properties.encode(true)?,
            ));
        }

        // ----------------------------------------------------------------
        // DDS-RPC 1.0 discovery PIDs (§7.6.2.1) — only if set.
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

        // PID_VENDORID (0x0016) — see the twin in `PublicationBuiltinTopicData`:
        // a vendor-specific parameter can only be read by a receiver that
        // knows whose it is (DDSI-RTPS 2.5 Table 9.6), and the vendor
        // written is the one the announcement carries, never a constant.
        if let Some(vendor) = self.vendor_id {
            params.push(Parameter::new(
                pid::VENDOR_ID,
                alloc::vec![vendor.0[0], vendor.0[1], 0, 0],
            ));
        }

        // PID_ZERODDS_TYPE_ID (F-TYPES-3 Wire-up) — written only under this
        // tree's own vendor id, for the reason given at the twin.
        if self.vendor_id == Some(crate::wire_types::VendorId::ZERODDS)
            && self.type_identifier != zerodds_types::TypeIdentifier::None
        {
            // A `TypeIdentifier` is XCDR2 wherever it travels (XTypes 1.3
            // §7.3.4); the cap of the writer is what says so to the codec.
            let mut w = zerodds_types::framing::xcdr2_writer();
            self.type_identifier
                .encode_into(&mut w)
                .map_err(|_| WireError::ValueOutOfRange {
                    message: "type_identifier encoding failed",
                })?;
            params.push(Parameter::new(pid::ZERODDS_TYPE_ID, w.finish()));
        }

        // PID_TYPE_CONSISTENCY_ENFORCEMENT (0x0074). Written for every
        // reader that announced it, which every local reader does, as
        // RTI Connext 7.7 and Fast DDS both do even at the defaults —
        // `crates/discovery/tests/fixtures/vendor-discovery/`. A writer
        // that has to reach §7.6.3.4.2's verdict for the reader's Topic
        // as well as its own needs the policy on every announcement, not
        // only on the ones that differ from the default. An announcement
        // read without the PID (`type_consistency_announced`) is written
        // back without it, so it reads back as it came.
        if self.type_consistency_announced {
            params.push(Parameter::new(
                pid::TYPE_CONSISTENCY_ENFORCEMENT,
                self.type_consistency.to_bytes(true),
            ));
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
        // FALSE is the default (Table 9.19) and is left out, as every other
        // default here is.
        if self.expects_inline_qos {
            params.push(Parameter::new(
                pid::EXPECTS_INLINE_QOS,
                crate::publication_data::boolean_value(true),
            ));
        }

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
        let group_guid = crate::publication_data::group_guid_of(&pl, key);
        let topic_name = pl
            .find(pid::TOPIC_NAME)
            .map(|p| {
                crate::publication_data::decode_bounded_cdr_string(
                    &p.value,
                    little_endian,
                    crate::publication_data::STRING_256_MAX,
                )
            })
            .transpose()?
            .ok_or(WireError::ValueOutOfRange {
                message: "TOPIC_NAME missing",
            })?;
        let type_name = pl
            .find(pid::TYPE_NAME)
            .map(|p| {
                crate::publication_data::decode_bounded_cdr_string(
                    &p.value,
                    little_endian,
                    crate::publication_data::STRING_256_MAX,
                )
            })
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

        let time_based_filter = pl
            .find(pid::TIME_BASED_FILTER)
            .and_then(|p| decode_duration(&p.value, little_endian))
            .map(|minimum_separation| zerodds_qos::TimeBasedFilterQosPolicy { minimum_separation })
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
                    zerodds_wire::Endianness::Little
                } else {
                    zerodds_wire::Endianness::Big
                };
                let mut r = zerodds_wire::Reader::new(&p.value, endianness);
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

        let type_consistency_param = pl.find(pid::TYPE_CONSISTENCY_ENFORCEMENT);
        let type_consistency_announced = type_consistency_param.is_some();
        let type_consistency = type_consistency_param
            .map(|p| {
                zerodds_types::qos::TypeConsistencyEnforcement::from_bytes(&p.value, little_endian)
            })
            // §7.6.3.4.1: no PID 0x0074 means DISALLOW_TYPE_COERCION.
            .unwrap_or(zerodds_types::qos::TypeConsistencyEnforcement::not_announced());

        let type_information = pl.find(pid::TYPE_INFORMATION).map(|p| p.value.clone());
        let (type_id_v1, type_object_v1) = crate::type_v1::read(&pl, little_endian);
        let entity_name = match pl.find(pid::ENTITY_NAME) {
            Some(p) => Some(crate::publication_data::decode_cdr_string(
                &p.value,
                little_endian,
            )?),
            None => None,
        };
        let content_filter_param = pl.find(pid::CONTENT_FILTER_PROPERTY);
        let content_filter = content_filter_param
            .and_then(|p| decode_content_filter_property(&p.value, little_endian));
        let content_filter_unreadable = content_filter_param.is_some() && content_filter.is_none();

        let security_info = pl
            .find(pid::ENDPOINT_SECURITY_INFO)
            .and_then(|p| EndpointSecurityInfo::from_bytes(&p.value, little_endian).ok());
        let sym_cipher_algo_info = pl
            .find(pid::ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO)
            .and_then(|p| {
                crate::security_algo_info::EndpointSecuritySymmetricCipherAlgorithmInfo::from_bytes(
                    &p.value,
                    little_endian,
                )
                .ok()
            });
        let data_tags = match pl.find(pid::DATA_TAGS) {
            Some(p) => crate::property_list::WireDataTags::decode(&p.value, little_endian)?,
            None => crate::property_list::WireDataTags::default(),
        };
        // A PropertyList that does not decode rejects the sample; it is not
        // read as an empty list.
        let properties = match pl.find(pid::PROPERTY_LIST) {
            Some(p) => crate::property_list::WirePropertyList::decode(&p.value, little_endian)?,
            None => crate::property_list::WirePropertyList::default(),
        };

        let service_instance_name = pl
            .find(pid::SERVICE_INSTANCE_NAME)
            .map(|p| service_instance_name_param(&p.value, little_endian))
            .transpose()?
            .flatten();
        let related_entity_guid = pl.find(pid::RELATED_ENTITY_GUID).and_then(guid_from_param);
        let topic_aliases = pl
            .find(pid::TOPIC_ALIASES)
            .map(|p| topic_aliases_param(&p.value, little_endian))
            .transpose()?
            .flatten();

        // Only ZeroDDS's own 0x8002 is a `TypeIdentifier`; every other
        // vendor's 0x8002 is that vendor's parameter. See the twin in
        // `PublicationBuiltinTopicData::from_pl_cdr_le` for the measurement
        // that made this a bug rather than a hypothetical.
        let announced_vendor = decoded_vendor_id(&pl);
        let mut type_identifier = if announced_vendor == Some(crate::wire_types::VendorId::ZERODDS)
        {
            pl.find(pid::ZERODDS_TYPE_ID)
                .and_then(|p| {
                    let mut r = zerodds_types::framing::xcdr2_reader(&p.value);
                    zerodds_types::TypeIdentifier::decode_from(&mut r).ok()
                })
                .unwrap_or_default()
        } else {
            zerodds_types::TypeIdentifier::None
        };

        // Issue #30 (symmetric to `PublicationBuiltinTopicData`): derive a
        // TypeIdentifier from the standard PID_TYPE_INFORMATION blob of a
        // non-ZeroDDS peer so the reader announcement carries a comparable
        // identity for the writer-side assignability guard. Parse failure
        // leaves it None with the raw blob retained for a loud signal at the
        // match layer.
        if type_identifier == zerodds_types::TypeIdentifier::None {
            if let Some(ti_bytes) = &type_information {
                let endianness = if little_endian {
                    zerodds_wire::Endianness::Little
                } else {
                    zerodds_wire::Endianness::Big
                };
                if let Ok(ti) = zerodds_types::type_information::TypeInformation::from_bytes(
                    ti_bytes, endianness,
                ) {
                    if let Some(id) = ti.preferred_type_identifier() {
                        type_identifier = id;
                    }
                }
            }
        }

        // See `PublicationBuiltinTopicData`: the `TypeObjectV1` identifies the
        // type of an endpoint that announces no `PID_TYPE_INFORMATION`.
        if type_identifier == zerodds_types::TypeIdentifier::None && type_information.is_none() {
            if let Some(id) = type_object_v1
                .as_ref()
                .and_then(crate::type_v1::identifier_of)
            {
                type_identifier = id;
            }
        }

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
            group_guid,
            topic_name,
            type_name,
            durability,
            reliability,
            ownership,
            liveliness,
            deadline,
            time_based_filter,
            latency_budget,
            destination_order,
            presentation,
            partition,
            user_data,
            topic_data,
            group_data,
            type_consistency,
            type_consistency_announced,
            type_information,
            type_id_v1,
            type_object_v1,
            entity_name,
            data_representation,
            content_filter,
            content_filter_unreadable,
            security_info,
            sym_cipher_algo_info,
            properties,
            data_tags,
            service_instance_name,
            related_entity_guid,
            topic_aliases,
            vendor_id: announced_vendor,
            type_identifier,
            unicast_locators: collect_locator_params(&pl, pid::UNICAST_LOCATOR, little_endian),
            multicast_locators: collect_locator_params(&pl, pid::MULTICAST_LOCATOR, little_endian),
            expects_inline_qos: crate::publication_data::boolean_param(
                &pl,
                pid::EXPECTS_INLINE_QOS,
            )?,
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
            content_filter_unreadable: false,
            vendor_id: Some(crate::wire_types::VendorId::ZERODDS),
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
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
        };
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, s);
    }

    /// §7.6.3.4.1: an announcement without PID 0x0074 is read as
    /// DISALLOW_TYPE_COERCION, with the options as though
    /// prevent_type_widening were true and the others false.
    #[test]
    fn a_reader_announcing_no_type_consistency_is_read_as_disallow() {
        let mut s = SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            vendor_id: Some(crate::wire_types::VendorId::ZERODDS),
            key: Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT),
            topic_name: "T".into(),
            type_name: "X".into(),
            durability: DurabilityKind::Volatile,
            reliability: ReliabilityQos::default(),
            ownership: zerodds_qos::OwnershipKind::Shared,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
        };
        let bytes = s.to_pl_cdr_le().unwrap();

        // Drop parameter 0x0074 from the encoded list (4-octet
        // encapsulation header, then id/length/value with 4-aligned values).
        let mut stripped = bytes[..4].to_vec();
        let mut at = 4;
        let mut dropped = false;
        while at + 4 <= bytes.len() {
            let id = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            let len = usize::from(u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]));
            let end = at + 4 + len;
            if id == pid::TYPE_CONSISTENCY_ENFORCEMENT {
                dropped = true;
            } else {
                stripped.extend_from_slice(&bytes[at..end]);
            }
            at = end;
            if id == pid::SENTINEL {
                break;
            }
        }
        assert!(dropped, "the encoder writes PID 0x0074");

        s.type_consistency = zerodds_types::qos::TypeConsistencyEnforcement::not_announced();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&stripped).unwrap();
        assert_eq!(decoded.type_consistency, s.type_consistency);
        assert_eq!(
            decoded.type_consistency.kind,
            zerodds_types::qos::TypeConsistencyKind::DisallowTypeCoercion
        );
        assert!(decoded.type_consistency.prevent_type_widening);
        assert!(!decoded.type_consistency_announced);
        // Written back, it still has no PID 0x0074.
        let rewritten =
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&decoded.to_pl_cdr_le().unwrap()).unwrap();
        assert_eq!(rewritten, decoded);

        // The same policy, announced, is told apart from the absent one.
        let announced =
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&s.to_pl_cdr_le().unwrap()).unwrap();
        assert_eq!(announced.type_consistency, decoded.type_consistency);
        assert!(announced.type_consistency_announced);
    }

    /// DDS-Security 1.2 §7.3.21, §7.5.1.8: the `propagate = TRUE`
    /// entries of the PropertyQosPolicy go out as `PID_PROPERTY_LIST` 0x0059
    /// (string and binary part); absent stays absent; an entry that does not
    /// decode rejects the sample.
    /// DDS-Security 1.2 §7.5.1.7, §7.5.1.8: the DataTags of an endpoint are
    /// `PID_DATA_TAGS` 0x1003; absent stays absent.
    #[test]
    fn endpoint_data_tags_roundtrip() {
        use crate::property_list::WireProperty;
        let mut d = sample_sub();
        assert!(d.data_tags.is_empty());
        let plain = d.to_pl_cdr_le().unwrap();
        assert!(
            !plain.windows(2).any(|w| w == [0x03, 0x10]),
            "not sent when absent"
        );
        d.data_tags.tags.push(WireProperty::new("level", "secret"));
        let bytes = d.to_pl_cdr_le().unwrap();
        assert!(bytes.windows(2).any(|w| w == [0x03, 0x10]));
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.data_tags, d.data_tags);
    }

    #[test]
    fn endpoint_property_list_roundtrip() {
        use crate::property_list::{WireBinaryProperty, WireProperty};
        let mut d = sample_sub();
        assert!(d.properties.is_empty());
        let plain = d.to_pl_cdr_le().unwrap();
        assert!(
            !plain.windows(2).any(|w| w == [0x59, 0x00]),
            "not sent when absent"
        );
        d.properties.push(WireProperty::new("dds.sec.x", "v"));
        d.properties = d
            .properties
            .with_binary(WireBinaryProperty::new("blob", alloc::vec![1_u8, 2, 3]));
        let bytes = d.to_pl_cdr_le().unwrap();
        assert!(bytes.windows(2).any(|w| w == [0x59, 0x00]));
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.properties, d.properties);
        assert_eq!(decoded.properties.get("dds.sec.x"), Some("v"));
    }

    #[test]
    fn security_info_roundtrip() {
        use crate::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};
        let s = SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            vendor_id: Some(crate::wire_types::VendorId::ZERODDS),
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
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: Some(EndpointSecurityInfo {
                endpoint_security_attributes: attrs::IS_VALID | attrs::IS_PAYLOAD_PROTECTED,
                plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                    | plugin_attrs::IS_PAYLOAD_ENCRYPTED,
            }),
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
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

    /// DDSI-RTPS 2.5 §9.3.2.1: the three names of a `ContentFilterProperty_t`
    /// are `string<256>`; the filter expression is not.
    #[test]
    fn content_filter_names_are_bounded_by_string_256() {
        let cfp = |name: usize, class: usize, expression: usize| ContentFilterProperty {
            content_filtered_topic_name: "c".repeat(name),
            related_topic_name: "r".into(),
            filter_class_name: "k".repeat(class),
            filter_expression: "e".repeat(expression),
            expression_parameters: alloc::vec![],
        };
        let ok = cfp(256, 256, 1000);
        let bytes = encode_content_filter_property_le(&ok).unwrap();
        assert_eq!(decode_content_filter_property(&bytes, true), Some(ok));
        assert!(encode_content_filter_property_le(&cfp(257, 1, 1)).is_err());
        assert!(encode_content_filter_property_le(&cfp(1, 257, 1)).is_err());
    }

    /// A ContentFilterProperty with a name over the bound of `string<256>`
    /// does not read: the subscription is taken without a filter and says so
    /// in `content_filter_unreadable`, for the receiver to count and log.
    #[test]
    fn an_over_long_content_filter_name_is_reported_as_unreadable() {
        let mut value = Vec::new();
        value.extend_from_slice(&encode_cdr_string_le(&"c".repeat(257)).unwrap());
        value.extend_from_slice(&encode_cdr_string_le("related").unwrap());
        value.extend_from_slice(&encode_cdr_string_le("DDSSQL").unwrap());
        value.extend_from_slice(&encode_cdr_string_le("x > 1").unwrap());
        value.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode_content_filter_property(&value, true), None);

        let mut bytes = sample_sub().to_pl_cdr_le().unwrap();
        let sentinel = bytes.split_off(bytes.len() - 4);
        bytes.extend_from_slice(&pid::CONTENT_FILTER_PROPERTY.to_le_bytes());
        bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&value);
        bytes.extend_from_slice(&sentinel);
        let read = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(read.content_filter, None);
        assert!(read.content_filter_unreadable);
        // Without the parameter nothing is unreadable.
        let plain =
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&sample_sub().to_pl_cdr_le().unwrap())
                .unwrap();
        assert!(!plain.content_filter_unreadable);
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
            content_filter_unreadable: false,
            vendor_id: Some(crate::wire_types::VendorId::ZERODDS),
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
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: Some(cfp.clone()),
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
        };
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.content_filter, Some(cfp));
    }

    /// DDSI-RTPS 2.5 §10.6: the SerializedPayload with which the
    /// SEDPbuiltinSubscriptionsWriter declares a DataReader of "Square" /
    /// "ShapeType", DESTINATION_ORDER BY_SOURCE_TIMESTAMP, DEADLINE 3 s. The
    /// octets are those printed in the spec.
    const SPEC_10_6: [u8; 84] = [
        0x00, 0x03, 0x00, 0x00, // PL_CDR_LE, options 0
        0x5A, 0x00, 0x10, 0x00, // PID_ENDPOINT_GUID, 16
        0xC0, 0xA8, 0x02, 0x05, 0x00, 0x00, 0x3A, 0x20, 0x00, 0x00, 0x00, 0x02, 0x80, 0x00, 0x00,
        0x07, //
        0x05, 0x00, 0x0C, 0x00, // PID_TOPIC_NAME, 12
        0x07, 0x00, 0x00, 0x00, b'S', b'q', b'u', b'a', b'r', b'e', 0x00, 0x00, //
        0x07, 0x00, 0x10, 0x00, // PID_TYPE_NAME, 16
        0x0A, 0x00, 0x00, 0x00, b'S', b'h', b'a', b'p', b'e', b'T', b'y', b'p', b'e', 0x00, 0x00,
        0x00, //
        0x25, 0x00, 0x04, 0x00, // PID_DESTINATION_ORDER, 4
        0x01, 0x00, 0x00, 0x00, //
        0x23, 0x00, 0x08, 0x00, // PID_DEADLINE, 8
        0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x01, 0x00, 0x00, 0x00, // PID_SENTINEL
    ];

    /// §10.6: the octets of the spec are read as the DataReader they declare.
    #[test]
    fn the_example_of_10_6_is_read() {
        let d = SubscriptionBuiltinTopicData::from_pl_cdr_le(&SPEC_10_6).unwrap();
        assert_eq!(
            d.key,
            Guid::new(
                GuidPrefix::from_bytes([
                    0xC0, 0xA8, 0x02, 0x05, 0x00, 0x00, 0x3A, 0x20, 0x00, 0x00, 0x00, 0x02
                ]),
                EntityId::from_bytes([0x80, 0x00, 0x00, 0x07]),
            )
        );
        assert_eq!(d.topic_name, "Square");
        assert_eq!(d.type_name, "ShapeType");
        assert_eq!(
            d.destination_order.kind,
            zerodds_qos::DestinationOrderKind::BySourceTimestamp
        );
        assert_eq!(d.deadline.period, zerodds_qos::Duration::from_secs(3));
    }

    /// §10.6: what we write for those five members is the octets of the
    /// spec (the spec leaves the members at their defaults out; we also
    /// write them, so the comparison is per parameter).
    #[test]
    fn the_parameters_of_the_example_of_10_6_are_written_as_the_spec_prints_them() {
        let mut d = SubscriptionBuiltinTopicData::from_pl_cdr_le(&SPEC_10_6).unwrap();
        d.participant_key = Guid::new(d.key.prefix, EntityId::PARTICIPANT);
        let bytes = d.to_pl_cdr_le().unwrap();
        assert_eq!(&bytes[..4], &SPEC_10_6[..4], "PL_CDR_LE, options 0");
        // The parameter list of the spec, one parameter at a time.
        let mut at = 4;
        while at + 4 <= SPEC_10_6.len() {
            let id = u16::from_le_bytes([SPEC_10_6[at], SPEC_10_6[at + 1]]);
            let len = usize::from(u16::from_le_bytes([SPEC_10_6[at + 2], SPEC_10_6[at + 3]]));
            if id == pid::SENTINEL {
                break;
            }
            let spec_param = &SPEC_10_6[at..at + 4 + len];
            let found = bytes.windows(spec_param.len()).any(|w| w == spec_param);
            assert!(found, "parameter 0x{id:04x} as the spec prints it");
            at += 4 + len;
        }
        // A list ends with the sentinel.
        assert_eq!(&bytes[bytes.len() - 4..], &SPEC_10_6[SPEC_10_6.len() - 4..]);
    }

    /// §10.3: "The sender shall set the representation_options to zero. The
    /// receiver shall ignore the value of the representation_options."
    #[test]
    fn the_receiver_ignores_the_representation_options() {
        let mut with_options = SPEC_10_6;
        with_options[2] = 0xAB;
        with_options[3] = 0xCD;
        assert_eq!(
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&with_options).unwrap(),
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&SPEC_10_6).unwrap()
        );
        // The sender writes zero.
        let written = sample_sub().to_pl_cdr_le().unwrap();
        assert_eq!(&written[2..4], &[0, 0]);
    }

    /// A reader's LIVELINESS and DEADLINE at their default (Duration_t
    /// INFINITE) are left out and read back as the default; set, they are
    /// sent and read back.
    #[test]
    fn the_readers_infinite_defaults_are_left_out_and_read_back() {
        let ids = |d: &SubscriptionBuiltinTopicData| -> Vec<u16> {
            let bytes = d.to_pl_cdr_le().unwrap();
            ParameterList::from_bytes(&bytes[4..], true)
                .unwrap()
                .parameters
                .iter()
                .map(|p| p.id)
                .collect()
        };
        let d = sample_sub();
        assert!(!ids(&d).contains(&pid::LIVELINESS));
        assert!(!ids(&d).contains(&pid::DEADLINE));
        let back =
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_le().unwrap()).unwrap();
        assert_eq!(back.liveliness, zerodds_qos::LivelinessQosPolicy::default());
        assert_eq!(back.deadline, zerodds_qos::DeadlineQosPolicy::default());
        let mut set = sample_sub();
        set.liveliness.lease_duration = Duration::from_secs(2);
        set.deadline.period = Duration::from_secs(3);
        assert!(ids(&set).contains(&pid::LIVELINESS));
        assert!(ids(&set).contains(&pid::DEADLINE));
        let back =
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&set.to_pl_cdr_le().unwrap()).unwrap();
        assert_eq!(
            (back.liveliness, back.deadline),
            (set.liveliness, set.deadline)
        );
    }

    fn sample_sub() -> SubscriptionBuiltinTopicData {
        SubscriptionBuiltinTopicData {
            content_filter_unreadable: false,
            vendor_id: Some(crate::wire_types::VendorId::ZERODDS),
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
            time_based_filter: zerodds_qos::TimeBasedFilterQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
            content_filter: None,
            security_info: None,
            sym_cipher_algo_info: None,
            data_tags: Default::default(),
            properties: Default::default(),
            service_instance_name: None,
            related_entity_guid: None,
            topic_aliases: None,
            type_identifier: zerodds_types::TypeIdentifier::None,
            unicast_locators: alloc::vec::Vec::new(),
            multicast_locators: alloc::vec::Vec::new(),
            expects_inline_qos: false,
            group_guid: None,
            type_consistency_announced: true,
            entity_name: None,
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

    /// DDS-RPC 1.0 §7.6.2.1.2: `service_instance_name` is a `string<256>`
    /// and `topic_aliases` a `sequence<string<256>>`. A longer string is
    /// refused where it used to be dropped from the data.
    #[test]
    fn a_discovery_string_beyond_the_bound_is_refused_subscription() {
        let mut s = sample_sub();
        s.service_instance_name = Some("x".repeat(256));
        s.topic_aliases = Some(alloc::vec!["y".repeat(256)]);
        let bytes = s.to_pl_cdr_le().unwrap();
        let decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.service_instance_name.as_deref().map(str::len),
            Some(256)
        );
        assert_eq!(decoded.topic_aliases.as_ref().map(Vec::len), Some(1));

        let mut name = sample_sub();
        name.service_instance_name = Some("x".repeat(257));
        let bytes = name.to_pl_cdr_le().unwrap();
        assert!(matches!(
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes),
            Err(WireError::ValueOutOfRange { .. })
        ));

        let mut alias = sample_sub();
        alias.topic_aliases = Some(alloc::vec!["ok".into(), "y".repeat(257)]);
        let bytes = alias.to_pl_cdr_le().unwrap();
        assert!(matches!(
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes),
            Err(WireError::ValueOutOfRange { .. })
        ));
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

    /// The twin of
    /// `publication_data::tests::a_foreign_vendor_id_survives_read_and_write_back`:
    /// a reader announcement keeps the vendor it states, and this tree's
    /// 0x8002 stays off an announcement that states another vendor.
    /// `PID_EXPECTS_INLINE_QOS` (0x0043, DDSI-RTPS 2.5 Table 9.18): TRUE
    /// goes out as one octet and three of padding and reads back; FALSE,
    /// the default (Table 9.19), is left out, and an announcement that
    /// states it reads as FALSE; an octet other than 0 or 1 is refused.
    #[test]
    fn expects_inline_qos_goes_out_when_set_and_reads_back() {
        let mut sub = sample_sub();
        let plain = sub.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&plain[4..], true).unwrap();
        assert!(pl.find(pid::EXPECTS_INLINE_QOS).is_none());
        assert!(
            !SubscriptionBuiltinTopicData::from_pl_cdr_le(&plain)
                .unwrap()
                .expects_inline_qos
        );

        sub.expects_inline_qos = true;
        let mut le = sub.to_pl_cdr_le().unwrap();
        let pid_and_value = [0x43, 0x00, 0x04, 0x00, 0x01, 0, 0, 0];
        let at = le
            .windows(pid_and_value.len())
            .position(|w| w == pid_and_value)
            .expect("PID 0x0043, length 4, TRUE");
        assert!(
            SubscriptionBuiltinTopicData::from_pl_cdr_le(&le)
                .unwrap()
                .expects_inline_qos
        );
        le[at + 4] = 0;
        assert!(
            !SubscriptionBuiltinTopicData::from_pl_cdr_le(&le)
                .unwrap()
                .expects_inline_qos
        );
        le[at + 4] = 2;
        assert!(SubscriptionBuiltinTopicData::from_pl_cdr_le(&le).is_err());
    }

    #[test]
    fn a_foreign_vendor_id_survives_read_and_write_back() {
        use zerodds_types::type_identifier::PrimitiveKind;

        let cyclone = crate::wire_types::VendorId([0x01, 0x10]);
        let bytes = {
            let mut b = sample_sub().to_pl_cdr_le().unwrap();
            replace_vendor_id(&mut b, cyclone);
            b
        };

        let mut decoded = SubscriptionBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.vendor_id,
            Some(cyclone),
            "the announced vendor has to reach the decoded announcement"
        );

        decoded.type_identifier = zerodds_types::TypeIdentifier::Primitive(PrimitiveKind::Int64);
        let back = decoded.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&back[4..], true).unwrap();
        assert_eq!(
            decoded_vendor_id(&pl),
            Some(cyclone),
            "the write-back replaced the announced vendor with this tree's"
        );
        assert!(
            pl.find(pid::ZERODDS_TYPE_ID).is_none(),
            "0x8002 was written on an announcement that states another vendor"
        );
    }

    /// Overwrites the `PID_VENDORID` value of an encoded announcement.
    fn replace_vendor_id(bytes: &mut [u8], vendor: crate::wire_types::VendorId) {
        let mut at = 4; // past the encapsulation header
        while at + 4 <= bytes.len() {
            let id = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            let len = u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]) as usize;
            if id == pid::SENTINEL {
                break;
            }
            if id == pid::VENDOR_ID && len >= 2 {
                bytes[at + 4] = vendor.0[0];
                bytes[at + 5] = vendor.0[1];
                return;
            }
            at += 4 + len;
        }
        panic!("the announcement carries no PID_VENDORID");
    }
}
