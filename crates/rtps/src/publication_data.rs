// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PublicationBuiltinTopicData (DDSI-RTPS 2.5 §8.5.4.2, §9.6.2.2.3).
//!
//! Content of the SEDP publications DATA submessage that a participant
//! sends to make a local DataWriter known to remote participants.
//! Serialized as a PL_CDR_LE-encoded ParameterList in the
//! `serialized_payload` of a DATA submessage.
//!
//! topic_name + type_name + GUIDs +
//! minimal QoS fields (durability, reliability). No deadline,
//! liveliness, lifespan, ownership, partition etc. — those are read and
//! stored in the `extra` vec, but not typed.
//!
//! **QoS enums local here** — once WP 1.5 brings full QoS matching,
//! DurabilityKind/ReliabilityKind move to `zerodds-qos`.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::endpoint_security_info::EndpointSecurityInfo;
use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::participant_data::{Duration, ENCAPSULATION_PL_CDR_LE};
use crate::wire_types::{Guid, Locator};

/// Durability-QoS Kind.
///
/// Canonical in [`zerodds_qos::DurabilityKind`]; RTPS re-exports for
/// backward compatibility.
pub use zerodds_qos::DurabilityKind;

/// Reliability-QoS Kind.
///
/// Canonical in [`zerodds_qos::ReliabilityKind`]; RTPS re-exportiert.
pub use zerodds_qos::ReliabilityKind;

/// DurabilityService QoS value (DDS 1.4 §2.2.3.5) — configures a
/// persistence service: cleanup delay + history kind/depth + resource
/// caps. Carried on `PID_DURABILITY_SERVICE` in a writer's DATA(w),
/// independent of the writer's own [`DurabilityKind`] (a writer may
/// declare it even at `VOLATILE`, though only a `TRANSIENT`/`PERSISTENT`
/// writer's value is meaningful to a durability service).
///
/// Canonical in [`zerodds_qos::DurabilityServiceQosPolicy`]; RTPS re-exports.
pub use zerodds_qos::DurabilityServiceQosPolicy;

/// Reliability QoS value: kind + max_blocking_time.
///
/// Canonical in [`zerodds_qos::ReliabilityQosPolicy`]; RTPS re-exportiert
/// under the historical alias `ReliabilityQos`.
pub use zerodds_qos::ReliabilityQosPolicy as ReliabilityQos;

/// Presentation QoS value: access_scope + coherent_access + ordered_access.
///
/// Canonical in [`zerodds_qos::PresentationQosPolicy`]; RTPS re-exportiert.
pub use zerodds_qos::PresentationQosPolicy;

/// `DataRepresentationId` — XTypes 1.3 §7.6.3.1.1 + RTPS 2.5 PID 0x0073.
///
/// Per spec: 16-bit signed integer; values 0..2 are normatively
/// defined. Per RTI/Cyclone/FastDDS convention, further values are
/// reserved as vendor-specific.
pub mod data_representation {
    /// XCDR1 (legacy CDR Plain-CDR + PL_CDR mutable). Default when the
    /// PID is not present (spec §7.6.3.1.2).
    pub const XCDR: i16 = 0;
    /// XML (rare, for CFP profiles). Not in our default list.
    pub const XML: i16 = 1;
    /// XCDR2 (PLAIN_CDR2 + DELIMITED_CDR2 + PL_CDR2 mutable).
    /// ZeroDDS' native encap (`0x0007`/`0x0009`/`0x000B`).
    pub const XCDR2: i16 = 2;

    /// The default announce list for writers and readers: `[XCDR1]`.
    ///
    /// DDS-XTypes 1.3 §7.6.3.1.1: "The default value of the
    /// DataRepresentationQosPolicy shall be an empty list of preferences. An
    /// empty list of preferences shall be taken to be equivalent to a list
    /// containing the single element XCDR." A writer puts on the wire the
    /// representation of the first element (`user_payload_encap` in
    /// `zerodds-dcps`: `CDR_LE`/`PL_CDR_LE` for XCDR1), and §7.6.3.1.2 asks
    /// the writer's first element to be in the reader's list.
    ///
    /// The offer of a writer. A reader requests XCDR2 and XCDR beside it
    /// (§7.6.3.1.1, readers from XTYPES 1.2 on: "shall request XCDR2 encoding"
    /// and, as it also deserializes version 1, "shall also request XCDR").
    /// `[XCDR1, XCDR2]` is [`OFFER_XCDR1_XCDR2`], the setting
    /// `wire.data_representation_offer = XCDR1,XCDR2` and the one of the ROS
    /// profiles. Measured on the lab host through
    /// `tools/dcps-shapes-interop.sh`: a Cyclone or Fast DDS reader left at the
    /// DDS default `[XCDR1]` matches a writer whose first element is XCDR1 and
    /// rejects one whose first element is XCDR2 with `DATAREPRESENTATION`.
    pub const DEFAULT_OFFER: [i16; 1] = [XCDR];

    /// The offer of a peer that also writes and reads XCDR2: `[XCDR1, XCDR2]`,
    /// XCDR1 first. DDS-XTypes 1.3 §7.6.3.1.1 gives `[XCDR]` as the default
    /// and this list is a configuration (`wire.data_representation_offer`,
    /// the ROS profiles).
    pub const OFFER_XCDR1_XCDR2: [i16; 2] = [XCDR, XCDR2];

    /// `DataRepMatchMode` — determines how writer and reader compare
    /// DataRep lists.
    ///
    /// * **Strict** (XTypes 1.3 §7.6.3.1.2 normative): the writer's FIRST
    ///   element must be in the reader's list. Exactly like RTI Connext.
    /// * **Tolerant** (industry norm, Cyclone + FastDDS): match when the
    ///   lists overlap (any-overlap), wire format = first-overlap.
    ///
    /// Default in ZeroDDS: `Strict`, the clause (XTypes 1.3 §7.6.3.1.1);
    /// `Tolerant` is the setting `wire.data_representation_match` for a
    /// peer that matches on any overlap.
    ///
    /// **Where the two still differ, and where they no longer do.** The
    /// difference is real on the **reader** side: our reader decodes from the
    /// received encapsulation header, so any overlap with a remote writer's
    /// offer is a match this reader can honour, wherever in its list the
    /// overlap sits.
    ///
    /// On the **writer** side the two now decide alike, and not because the
    /// mode is ignored: `wire_writer_to_remote_reader` rejects a match whose
    /// negotiated representation is not the one the writer emits, and the
    /// writer emits its own first element for every matched reader. A tolerant
    /// overlap that lands anywhere else would have matched a reader and then
    /// sent it octets it had not accepted.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum DataRepMatchMode {
        /// Strict-spec match: Writer.first ∈ Reader.list.
        #[default]
        Strict,
        /// Tolerant match: any element in Writer.list ∈ Reader.list,
        /// as Cyclone DDS and Fast DDS match.
        Tolerant,
    }

    /// Determines the negotiated wire format between writer and reader.
    ///
    /// Returns `Some(id)` with the DataRepresentationId to emit on the
    /// wire. `None` means: no overlap — no match.
    ///
    /// `writer_offered` can contain multiple values (e.g.
    /// `[XCDR2, XCDR1]`); `reader_accepted` likewise.
    /// Both lists can be empty — the spec default is `[XCDR1]` in that
    /// case (spec §7.6.3.1.2).
    #[must_use]
    pub fn negotiate(
        writer_offered: &[i16],
        reader_accepted: &[i16],
        mode: DataRepMatchMode,
    ) -> Option<i16> {
        // Spec defaults for empty lists.
        let w_default = [XCDR];
        let r_default = [XCDR];
        let w: &[i16] = if writer_offered.is_empty() {
            &w_default
        } else {
            writer_offered
        };
        let r: &[i16] = if reader_accepted.is_empty() {
            &r_default
        } else {
            reader_accepted
        };

        match mode {
            DataRepMatchMode::Strict => {
                // §7.6.3.1.2: the writer's first element must be in the reader's list.
                let first = w.first().copied()?;
                if r.contains(&first) {
                    Some(first)
                } else {
                    None
                }
            }
            DataRepMatchMode::Tolerant => {
                // Industry: any overlap. We prefer the writer's
                // preference order (first-match wins), but see the FULL
                // writer list, not just first.
                w.iter().copied().find(|id| r.contains(id))
            }
        }
    }

    /// Encap header (4 byte) for @final structs under the given
    /// DataRep. For @appendable/@mutable a different encap code is
    /// needed — see `encap_for_extensibility`.
    ///
    /// Return format: `[byte0, byte1, byte2, byte3]` where byte0 is
    /// always `0x00` and byte1 the repr id per RTPS 2.5 §10.5.
    #[must_use]
    pub fn encap_for_final_le(id: i16) -> [u8; 4] {
        match id {
            XCDR2 => [0x00, 0x07, 0x00, 0x00], // PLAIN_CDR2_LE
            _ => [0x00, 0x01, 0x00, 0x00],     // CDR_LE (XCDR1 default)
        }
    }
}

/// Discovered Publication / lokaler DataWriter — Subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationBuiltinTopicData {
    /// Endpoint-GUID (= Writer-GUID).
    pub key: Guid,
    /// GUID of the participant the writer belongs to.
    pub participant_key: Guid,
    /// The writer's Publisher (`PID_GROUP_GUID`, DDSI-RTPS 2.5 Table 9.19
    /// `WriterProxy::remoteGroupGuid`); `None` for `GUID_UNKNOWN`, its
    /// default. A GROUP-scope subscriber tells the writers of one remote
    /// publisher by it (§8.7.5).
    pub group_guid: Option<Guid>,
    /// Topic name (DDS-Topic, e.g. "ChatterTopic").
    pub topic_name: String,
    /// IDL type name (e.g. "std_msgs::String").
    pub type_name: String,
    /// Durability-QoS.
    pub durability: DurabilityKind,
    /// DurabilityService-QoS (spec §2.2.3.5, PID 0x001e). Absent on the wire
    /// decodes to [`DurabilityServiceQosPolicy::default`] (KEEP_LAST(1), all
    /// caps unlimited), per §9.6.3.1 (a policy absent from an announcement is
    /// that policy at its default).
    pub durability_service: DurabilityServiceQosPolicy,
    /// Reliability-QoS.
    pub reliability: ReliabilityQos,
    /// Ownership-QoS (Spec §2.2.3.23). Default Shared.
    pub ownership: zerodds_qos::OwnershipKind,
    /// Ownership strength (spec §2.2.3.24). Only relevant when
    /// `ownership == Exclusive`; default 0.
    pub ownership_strength: i32,
    /// Liveliness QoS (spec §2.2.3.11).
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// Deadline QoS (spec §2.2.3.7).
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// LatencyBudget QoS (spec §2.2.3.10, PID 0x0027) — offered duration.
    /// RxO-matched: `offered.duration <= requested.duration`.
    pub latency_budget: zerodds_qos::LatencyBudgetQosPolicy,
    /// DestinationOrder QoS (spec §2.2.3.18, PID 0x0025) — offered kind.
    /// RxO-matched: `offered.kind >= requested.kind`
    /// (BY_SOURCE_TIMESTAMP > BY_RECEPTION_TIMESTAMP).
    pub destination_order: zerodds_qos::DestinationOrderQosPolicy,
    /// Lifespan QoS (spec §2.2.3.16) — writer-only.
    pub lifespan: zerodds_qos::LifespanQosPolicy,
    /// Presentation QoS (spec §2.2.3.6, PID 0x0021). Publisher/Subscriber-group
    /// scope, inherited by every contained writer/reader. Default
    /// `PresentationQosPolicy::default()` (INSTANCE scope, no coherent/ordered
    /// access) when the peer does not send the PID (legacy vendors).
    pub presentation: PresentationQosPolicy,
    /// Partition QoS (spec §2.2.3.13). Empty list = "default partition" ("").
    pub partition: Vec<String>,
    /// UserData QoS (spec §2.2.3.1) — opaque `sequence<octet>`,
    /// discovery-propagated. Empty vec = not set.
    pub user_data: Vec<u8>,
    /// TopicData QoS (spec §2.2.3.3) — opaque `sequence<octet>`,
    /// propagated from the topic via pub discovery.
    pub topic_data: Vec<u8>,
    /// GroupData QoS (spec §2.2.3.2) — opaque `sequence<octet>`,
    /// propagated from the publisher via pub discovery.
    pub group_data: Vec<u8>,
    /// Type information (TypeIdentifier hashes + dependencies, XTypes
    /// §7.6.3.2.2). Opaque bytes: the structure lives in `zerodds-types`,
    /// but we transport the serialized blob to avoid circular crate
    /// dependencies.
    pub type_information: Option<Vec<u8>>,
    /// `type_id` (`TypeIdV1`, `PID` 0x0069), XTypes 1.3 Annex D.
    pub type_id_v1: Option<zerodds_types::type_object_v1::TypeId>,
    /// `type` (`TypeObjectV1`, `PID` 0x0072), XTypes 1.3 Annex D and
    /// §7.6.3.2.2.
    pub type_object_v1: Option<zerodds_types::type_object_v1::TypeObjectV1>,
    /// `PID_TYPE_MAX_SIZE_SERIALIZED` (0x0060,
    /// `WriterProxy::dataMaxSizeSerialized`, DDSI-RTPS 2.5 Tables 8.74 and
    /// 9.19): "the maximum size of any SerializedPayload that may be sent by
    /// the matched Writer", in octets, the 4-octet encapsulation header
    /// included. Optional; `None` when the writer announces none.
    pub data_max_size_serialized: Option<i32>,
    /// `PID_ENTITY_NAME` (0x0062, DDSI-RTPS 2.5 Table 9.18, `EntityName_t`):
    /// the name of the writer. `None` when it announces none.
    pub entity_name: Option<String>,
    /// Accepted data representations (0=XCDR1, 1=XML, 2=XCDR2, ...).
    /// Spec: XTypes 1.3 §7.6.3.1.1 / RTPS 2.5 PID 0x0073.
    /// The default list when empty is `[XCDR1]` per XTypes §7.6.3.1.2 —
    /// we always emit the PID explicitly so strict vendors like
    /// RTI 7.7.0 can SEDP-match.
    pub data_representation: Vec<i16>,
    /// Endpoint security info (PID 0x1004, DDS-Security 1.1 §7.4.1.5).
    /// `None` for legacy peers without a security PID. WP 4H-c matches on
    /// it: writer/reader pairs with incompatible protection levels are
    /// rejected.
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
    /// service-instance name of an RPC endpoint. `None` for ordinary
    /// pub/sub topics.
    pub service_instance_name: Option<String>,
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.6.2.1) — GUID of the
    /// counterpart endpoint in an RPC endpoint pair. For a request
    /// writer it points to the reply reader of the same requester; for a
    /// reply writer to the request reader of the same replier.
    pub related_entity_guid: Option<Guid>,
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.6.2.1) — alternative topic names
    /// for routing/compat layers. Order is significant.
    pub topic_aliases: Option<Vec<String>>,
    /// `PID_VENDORID` (0x0016) — the vendor of the middleware that owns
    /// the announced writer, as the announcement itself states it.
    ///
    /// DDSI-RTPS 2.5 Table 9.19 maps `PID_VENDORID` to
    /// `ParticipantProxy::vendorId` and to no field of a
    /// `DiscoveredWriterData`, so the parameter is an extension on this
    /// announcement — one that Cyclone DDS 11.0.1, Fast DDS 3.6.1 and RTI
    /// Connext 7.7.0 all send, each with its own vendor id, measured in
    /// `crates/discovery/tests/fixtures/vendor-discovery/`. OpenDDS 3.34.0
    /// sends none, which is `None` here.
    ///
    /// It is read and written as it stands, never substituted: a decoded
    /// announcement carries the vendor of the peer that wrote it, and
    /// re-encoding it must not put this tree's vendor on another product's
    /// announcement. What the value decides is who a vendor-specific
    /// parameter belongs to (Table 9.6: "Vendor-specific ParameterId. Will
    /// not be recognized by other vendors' implementations."), so
    /// `PID_ZERODDS_TYPE_ID` (0x8002) is read *and written* only while this
    /// is [`crate::wire_types::VendorId::ZERODDS`].
    ///
    /// A locally created announcement sets `Some(VendorId::ZERODDS)`.
    pub vendor_id: Option<crate::wire_types::VendorId>,
    /// PID_ZERODDS_TYPE_ID (vendor PID 0x8002) — XTypes 1.3 §7.3.4.2
    /// TypeIdentifier of the writer type for XTypes-aware reader matching
    /// (XTypes §7.6.3.4 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
    pub type_identifier: zerodds_types::TypeIdentifier,
    /// Endpoint unicast locators (DDSI-RTPS 2.5 §8.5.3.3:
    /// `DiscoveredWriterData.writerProxy.unicastLocatorList`). Where
    /// peers send user data to *this* writer endpoint. Empty = the peer
    /// falls back to the participant `DEFAULT_UNICAST_LOCATOR` from
    /// SPDP (§8.5.5). OpenDDS stores the real user locator exclusively
    /// here and sends only the placeholder 127.0.0.1:12345 as the SPDP
    /// default.
    pub unicast_locators: Vec<Locator>,
    /// Endpoint multicast locators (DDSI-RTPS 2.5 §8.5.3.3).
    pub multicast_locators: Vec<Locator>,
}

/// Emits a separate parameter per locator — an RTPS locator list is the
/// repeated PID (spec §8.3.5.2 ParameterList).
pub fn encode_locator_params(params: &mut ParameterList, pid_id: u16, locators: &[Locator]) {
    for loc in locators {
        params.push(Parameter::new(pid_id, loc.to_bytes_le().to_vec()));
    }
}

/// Collects all locators announced under `pid_id` (a repeated PID =
/// list), read in the list's byte order (PL_CDR_LE or PL_CDR_BE,
/// §9.4.2.11).
#[must_use]
pub fn collect_locator_params(
    pl: &ParameterList,
    pid_id: u16,
    little_endian: bool,
) -> Vec<Locator> {
    pl.parameters
        .iter()
        .filter(|p| p.id == pid_id && p.value.len() == Locator::WIRE_SIZE)
        .filter_map(|p| {
            let mut b = [0u8; 24];
            b.copy_from_slice(&p.value);
            if little_endian {
                Locator::from_bytes_le(b).ok()
            } else {
                Locator::from_bytes_be(b).ok()
            }
        })
        .collect()
}

impl PublicationBuiltinTopicData {
    /// Encodes to PL_CDR_LE bytes (with a 4-byte encapsulation header).
    /// The output is directly usable as the `serialized_payload` of a DATA
    /// submessage.
    ///
    /// # Errors
    /// `ValueOutOfRange` if a string is longer than u32::MAX.
    pub fn to_pl_cdr_le(&self) -> Result<Vec<u8>, WireError> {
        let params = self.parameters()?;
        let mut out = Vec::with_capacity(params.parameters.len() * 24 + 16);
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&params.to_bytes(true));
        Ok(out)
    }

    /// The writer's QoS as in-line parameters, for a reader that expects
    /// them (DDSI-RTPS 2.5 §8.4.7.5, §8.7.1): of what this announcement
    /// carries, the policies Table 8.85 marks "May appear as in-line QoS",
    /// and `PID_TOPIC_NAME`, which §8.7.2.1 adds so that the DATA names
    /// its topic. Little-endian, the same octets SEDP sends for them; a
    /// policy SEDP leaves out at its default is left out here too, and
    /// §8.7.2.1 has the reader assume the default for it.
    ///
    /// `TRANSPORT_PRIORITY` is in Table 8.85 and not in the announcement
    /// (DDS 1.4 §2.2.5's `PublicationBuiltinTopicData` has no such
    /// member): the writer's value is `transport_priority`, appended when
    /// it is not the default 0 (§8.7.2.1).
    ///
    /// # Errors
    /// As [`Self::to_pl_cdr_le`].
    pub fn inline_qos(&self, transport_priority: i32) -> Result<ParameterList, WireError> {
        const IN_LINE: [u16; 12] = [
            pid::TOPIC_NAME,
            pid::DURABILITY,
            pid::PRESENTATION,
            pid::DEADLINE,
            pid::LATENCY_BUDGET,
            pid::OWNERSHIP,
            pid::OWNERSHIP_STRENGTH,
            pid::LIVELINESS,
            pid::PARTITION,
            pid::RELIABILITY,
            pid::LIFESPAN,
            pid::DESTINATION_ORDER,
        ];
        let mut list = self.parameters()?;
        list.parameters.retain(|p| IN_LINE.contains(&p.id));
        if transport_priority != 0 {
            list.push(Parameter::new(
                pid::TRANSPORT_PRIORITY,
                transport_priority.to_le_bytes().to_vec(),
            ));
        }
        Ok(list)
    }

    /// The parameters of [`Self::to_pl_cdr_le`], without the encapsulation.
    fn parameters(&self) -> Result<ParameterList, WireError> {
        let mut params = ParameterList::new();

        // PARTICIPANT_GUID: 16 Byte
        params.push(Parameter::new(
            pid::PARTICIPANT_GUID,
            self.participant_key.to_bytes().to_vec(),
        ));

        // GROUP_GUID: 16 Byte, left out at its default GUID_UNKNOWN.
        if let Some(group) = self.group_guid {
            params.push(Parameter::new(pid::GROUP_GUID, group.to_bytes().to_vec()));
        }

        // ENDPOINT_GUID: 16 Byte
        params.push(Parameter::new(
            pid::ENDPOINT_GUID,
            self.key.to_bytes().to_vec(),
        ));

        // TOPIC_NAME: CDR-String (4 byte len + UTF-8 + null)
        params.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_bounded_cdr_string_le(&self.topic_name, STRING_256_MAX)?,
        ));

        // TYPE_NAME: CDR-String
        params.push(Parameter::new(
            pid::TYPE_NAME,
            encode_bounded_cdr_string_le(&self.type_name, STRING_256_MAX)?,
        ));

        // DURABILITY: 4 Byte u32
        params.push(Parameter::new(
            pid::DURABILITY,
            (self.durability as u32).to_le_bytes().to_vec(),
        ));

        // DURABILITY_SERVICE: Duration (8) + history_kind (4) +
        // history_depth (4) + 3 × resource cap (4 each) = 28 byte. Always
        // emitted — a durability service (ADR 0009) reads this PID to
        // derive its retention contract and must see the writer's actual
        // value even when it equals the spec default.
        {
            let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
            self.durability_service.encode_into(&mut w);
            params.push(Parameter::new(pid::DURABILITY_SERVICE, w.finish()));
        }

        // RELIABILITY: 4 Byte kind + 8 Byte max_blocking_time
        let mut rel = Vec::with_capacity(12);
        rel.extend_from_slice(&(self.reliability.kind as u32).to_le_bytes());
        rel.extend_from_slice(&self.reliability.max_blocking_time.to_bytes_le());
        params.push(Parameter::new(pid::RELIABILITY, rel));

        // OWNERSHIP: 4 Byte u32 kind
        params.push(Parameter::new(
            pid::OWNERSHIP,
            encode_u32_le(self.ownership as u32).to_vec(),
        ));

        // OWNERSHIP_STRENGTH: 4 byte int32 (only meaningful for
        // Exclusive, but we always send it — the reader ignores it for Shared).
        params.push(Parameter::new(
            pid::OWNERSHIP_STRENGTH,
            encode_u32_le(self.ownership_strength as u32).to_vec(),
        ));

        // LIVELINESS: 4 Byte kind + 8 Byte lease_duration. It, DEADLINE and
        // LIFESPAN, the policies whose default is a Duration_t INFINITE, are
        // left out at their default: a reader takes the default for an
        // absent parameter (DDSI-RTPS 2.5 §9.4.2.11). Cyclone 11.0.1,
        // Connext 7.7.0 and OpenDDS 3.34.0 leave them out as well, Fast DDS
        // sends them (`crates/discovery/tests/fixtures/vendor-discovery/`);
        // dust_dds 0.16.1 reads an announced INFINITE as a finite duration
        // and refuses the endpoint as incompatible (measured, OMG dds-rtps
        // from and to ZeroDDS: LIVELINESS at its reader, DEADLINE at its
        // writer).
        if self.liveliness != zerodds_qos::LivelinessQosPolicy::default() {
            params.push(Parameter::new(
                pid::LIVELINESS,
                encode_liveliness_le(self.liveliness),
            ));
        }

        // DEADLINE: 8 Byte Duration_t, left out at its default (see LIVELINESS).
        if self.deadline != zerodds_qos::DeadlineQosPolicy::default() {
            params.push(Parameter::new(
                pid::DEADLINE,
                encode_duration_le(self.deadline.period).to_vec(),
            ));
        }

        // LATENCY_BUDGET: 8 Byte Duration_t (spec §2.2.3.10 / PID 0x0027).
        // Always emitted so a strict peer can RxO-match on it.
        params.push(Parameter::new(
            pid::LATENCY_BUDGET,
            encode_duration_le(self.latency_budget.duration).to_vec(),
        ));

        // DESTINATION_ORDER: 4 Byte u32 kind (spec §2.2.3.18 / PID 0x0025).
        params.push(Parameter::new(
            pid::DESTINATION_ORDER,
            encode_u32_le(self.destination_order.kind as u32).to_vec(),
        ));

        // LIFESPAN: 8 Byte Duration_t, left out at its default (see LIVELINESS).
        if self.lifespan != zerodds_qos::LifespanQosPolicy::default() {
            params.push(Parameter::new(
                pid::LIFESPAN,
                encode_duration_le(self.lifespan.duration).to_vec(),
            ));
        }

        // PRESENTATION: 4 byte access_scope + 1 byte coherent + 1 byte
        // ordered + 2 byte padding = 8 byte (spec §2.2.3.6 / PID 0x0021).
        // Always emitted (like RELIABILITY/DURABILITY) so a strict peer
        // (e.g. RTI) can SEDP-match on it.
        {
            let mut w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Little);
            self.presentation.encode_into(&mut w);
            params.push(Parameter::new(pid::PRESENTATION, w.finish()));
        }

        // PARTITION: only if non-empty — an empty list = default (= "").
        if !self.partition.is_empty() {
            params.push(Parameter::new(
                pid::PARTITION,
                encode_partition_le(&self.partition)?,
            ));
        }

        // USER_DATA / TOPIC_DATA / GROUP_DATA: opaque sequence<octet>.
        // Wire = 4 byte u32 length + N byte data. Only if set
        // (an empty vec = default, we leave it out of the ParameterList).
        if !self.user_data.is_empty() {
            params.push(Parameter::new(
                pid::USER_DATA,
                encode_octet_seq_le(&self.user_data)?,
            ));
        }
        if !self.topic_data.is_empty() {
            params.push(Parameter::new(
                pid::TOPIC_DATA,
                encode_octet_seq_le(&self.topic_data)?,
            ));
        }
        if !self.group_data.is_empty() {
            params.push(Parameter::new(
                pid::GROUP_DATA,
                encode_octet_seq_le(&self.group_data)?,
            ));
        }

        // TYPE_INFORMATION: serialized TypeInformation blob
        // (optional, XTypes §7.6.3.2.2).
        if let Some(ti) = &self.type_information {
            params.push(Parameter::new(pid::TYPE_INFORMATION, ti.clone()));
        }
        crate::type_v1::push(
            &mut params,
            self.type_id_v1.as_ref(),
            self.type_object_v1.as_ref(),
        )?;

        if let Some(max) = self.data_max_size_serialized {
            params.push(Parameter::new(
                pid::TYPE_MAX_SIZE_SERIALIZED,
                max.to_le_bytes().to_vec(),
            ));
        }
        if let Some(name) = &self.entity_name {
            params.push(Parameter::new(
                pid::ENTITY_NAME,
                encode_cdr_string_le(name)?,
            ));
        }

        // ENDPOINT_SECURITY_INFO: 2x u32 masks (§7.4.1.5). Only if set,
        // otherwise legacy behavior (Cyclone/Fast-DDS without security
        // leave the PID out).
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

        // PID_VENDORID (0x0016). A vendor-specific parameter means what
        // the vendor that wrote it says it means (DDSI-RTPS 2.5 Table 9.6:
        // "Vendor-specific ParameterId. Will not be recognized by other
        // vendors' implementations."), so a receiver can only read one
        // after it knows whose it is. A ZeroDDS announcement carries
        // `PID_ZERODDS_TYPE_ID`, so it states the vendor — RTI Connext,
        // Cyclone DDS and Fast DDS state theirs in the same parameter, and
        // it is what `from_pl_cdr_le` reads before it touches 0x8002.
        //
        // The value written is `self.vendor_id`, not a constant: this
        // announcement may have been decoded from another vendor, and
        // stamping this tree's id on it would claim its vendor parameters
        // as ours. `None` — OpenDDS 3.34.0 states no vendor on SEDP —
        // writes nothing.
        if let Some(vendor) = self.vendor_id {
            params.push(Parameter::new(
                pid::VENDOR_ID,
                alloc::vec![vendor.0[0], vendor.0[1], 0, 0],
            ));
        }

        // PID_ZERODDS_TYPE_ID (F-TYPES-3 Wire-up). Written only under this
        // tree's own vendor id: 0x8002 is in the range Table 9.6 gives to
        // every vendor, so writing it on an announcement that states
        // another vendor would offer that vendor's parameter id carrying
        // this tree's `TypeIdentifier`. A foreign announcement's type
        // identity travels in the standard `PID_TYPE_INFORMATION` below,
        // which is where it was read from.
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

        // DATA_REPRESENTATION: sequence<int16> — u32 length + 2*N bytes.
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

        // Endpoint locators (§8.5.3.3) — one parameter per locator.
        encode_locator_params(&mut params, pid::UNICAST_LOCATOR, &self.unicast_locators);
        encode_locator_params(
            &mut params,
            pid::MULTICAST_LOCATOR,
            &self.multicast_locators,
        );
        Ok(params)
    }

    /// Decodes from PL_CDR_LE bytes (with an encapsulation header).
    ///
    /// # Errors
    /// `UnexpectedEof` on too-short bytes,
    /// `UnsupportedEncapsulation` on unknown encoding,
    /// `ValueOutOfRange` if mandatory PIDs are missing or values have the
    /// wrong length.
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

        // PARTICIPANT_GUID is technically optional (can be derived from
        // the ENDPOINT_GUID prefix), but we require it when present.
        let participant_key = pl
            .find(pid::PARTICIPANT_GUID)
            .and_then(guid_from_param)
            .unwrap_or_else(|| {
                // Fallback: participant = ENDPOINT_GUID.prefix + PARTICIPANT EntityId
                Guid::new(key.prefix, crate::wire_types::EntityId::PARTICIPANT)
            });
        let group_guid = group_guid_of(&pl, key);

        let topic_name = pl
            .find(pid::TOPIC_NAME)
            .map(|p| decode_bounded_cdr_string(&p.value, little_endian, STRING_256_MAX))
            .transpose()?
            .ok_or(WireError::ValueOutOfRange {
                message: "TOPIC_NAME missing",
            })?;

        let type_name = pl
            .find(pid::TYPE_NAME)
            .map(|p| decode_bounded_cdr_string(&p.value, little_endian, STRING_256_MAX))
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

        // DURABILITY_SERVICE: reuse the policy's own Writer/Reader-based
        // codec (`zerodds_qos::DurabilityServiceQosPolicy`) rather than
        // hand-rolling the 28-byte layout a second time; `Reader` is
        // endianness-aware, so no separate BE branch is needed here.
        let durability_service = pl
            .find(pid::DURABILITY_SERVICE)
            .and_then(|p| {
                let endianness = if little_endian {
                    zerodds_wire::Endianness::Little
                } else {
                    zerodds_wire::Endianness::Big
                };
                let mut r = zerodds_wire::Reader::new(&p.value, endianness);
                DurabilityServiceQosPolicy::decode_from(&mut r).ok()
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
                        // BE decoding: interpret seconds+fraction as BE
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
            // An absent `PID_RELIABILITY` is RELIABLE here, and not
            // `ReliabilityQos::default()`.
            //
            // DDS 1.4 §2.2.3, Table 12 gives RELIABILITY two defaults:
            // BEST_EFFORT for a DataReader and RELIABLE for a DataWriter.
            // This is the *publication*, so it is the writer's, and
            // `Default` carries the reader's — which is right for
            // `subscription_data.rs` and wrong here.
            //
            // Cyclone omits the parameter from its writer announcement
            // for exactly this reason: it is the writer default. Read as
            // BEST_EFFORT, a reliable ZeroDDS reader then refused the
            // match by RxO while Cyclone matched ZeroDDS's writer in the
            // other direction, so the pair discovered each other, ran
            // the whole DDS-Security handshake, exchanged crypto tokens
            // and SEDP — and delivered no sample. Measured plain and
            // secured, both directions, the Linux test host 2026-08-14.
            .unwrap_or(ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(100),
            });

        let ownership = pl
            .find(pid::OWNERSHIP)
            .and_then(|p| decode_u32(&p.value, little_endian))
            .map(zerodds_qos::OwnershipKind::from_u32)
            .unwrap_or_default();

        let ownership_strength = pl
            .find(pid::OWNERSHIP_STRENGTH)
            .and_then(|p| decode_i32(&p.value, little_endian))
            .unwrap_or(0);

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

        let lifespan = pl
            .find(pid::LIFESPAN)
            .and_then(|p| decode_duration(&p.value, little_endian))
            .map(|duration| zerodds_qos::LifespanQosPolicy { duration })
            .unwrap_or_default();

        // PRESENTATION (PID 0x0021): default INSTANCE/no-coherent/no-ordered
        // if the peer does not send it (legacy vendor without PRESENTATION
        // on the wire, or LE/BE mismatch on the padding bytes — decode
        // failure falls back to the spec default, never a hard error).
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
            .and_then(|p| decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();
        let topic_data = pl
            .find(pid::TOPIC_DATA)
            .and_then(|p| decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();
        let group_data = pl
            .find(pid::GROUP_DATA)
            .and_then(|p| decode_octet_seq(&p.value, little_endian))
            .unwrap_or_default();

        let type_information = pl.find(pid::TYPE_INFORMATION).map(|p| p.value.clone());
        let (type_id_v1, type_object_v1) = crate::type_v1::read(&pl, little_endian);
        let data_max_size_serialized = crate::participant_data::four_octets(
            &pl,
            pid::TYPE_MAX_SIZE_SERIALIZED,
            "PID_TYPE_MAX_SIZE_SERIALIZED is not 4 octets",
            little_endian,
        )?
        .map(|v| v as i32);
        let entity_name = match pl.find(pid::ENTITY_NAME) {
            Some(p) => Some(decode_cdr_string(&p.value, little_endian)?),
            None => None,
        };

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

        // PID_ZERODDS_TYPE_ID is 0x8002, and 0x8002 is in the range
        // DDSI-RTPS 2.5 §9.4.2.11.2 Table 9.6 hands to *each* vendor at
        // once ("Vendor-specific ParameterId. Will not be recognized by
        // other vendors' implementations."): the parameter
        // means what the sender's vendor says it means. Reading it from a
        // peer that is not ZeroDDS reads someone else's parameter as a
        // `TypeIdentifier`.
        //
        // Measured: RTI Connext 7.7.0 announces a 0x8002 of its own beside
        // its `PID_TYPE_INFORMATION`. ZeroDDS decoded that parameter's
        // first octet, `0x01`, as the discriminator `TK_BOOLEAN`, so every
        // RTI publication and subscription arrived carrying
        // `TypeIdentifier::Primitive(Boolean)`. That is not `None`, so the
        // standard `PID_TYPE_INFORMATION` below was never consulted, and
        // the assignability check compared a boolean against the local
        // type and refused the endpoint with `kinds do not match` — both
        // `ci-rti` cells, all six `cross-vendor-rich/rti` cells, 0 samples.
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
            // No `PID_VENDORID` is the same answer as a foreign one: the
            // parameter cannot be attributed, so it is not read. Every
            // ZeroDDS announcement states the vendor (see `to_pl_cdr_le`),
            // and a peer that does not is served by the standard
            // `PID_TYPE_INFORMATION` path below, which is where a foreign
            // vendor's type identity comes from anyway.
            zerodds_types::TypeIdentifier::None
        };

        // Issue #30: non-ZeroDDS peers (Cyclone, FastDDS, RTI, …) do not emit
        // the vendor PID_ZERODDS_TYPE_ID; they carry the standard
        // PID_TYPE_INFORMATION blob (XTypes 1.3 §7.6.3.2.2). Derive a
        // comparable TypeIdentifier from it (minimal preferred) so the
        // match-time assignability guard is exercised cross-vendor instead of
        // silently falling back to a name-only match. On a parse failure the
        // identifier stays None and the raw blob is retained — the match layer
        // (`wire_reader_to_remote_writer`) then emits a loud signal rather than
        // dropping the assignability check silently.
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

        // XTypes 1.1 announced the type as a `TypeObjectV1` and 1.3 keeps the
        // member (§7.6.3.2.2): an endpoint without `PID_TYPE_INFORMATION`
        // that has one is identified by it.
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
                // DoS cap: Vec::with_capacity(n) could reserve ~4 GB at
                // n=u32::MAX/2. Cap to actually readable elements:
                // (v.len()-4)/2 i16 elements.
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
            data_max_size_serialized,
            entity_name,
            topic_name,
            type_name,
            durability,
            durability_service,
            reliability,
            ownership,
            ownership_strength,
            liveliness,
            deadline,
            latency_budget,
            destination_order,
            lifespan,
            presentation,
            partition,
            user_data,
            topic_data,
            group_data,
            type_information,
            type_id_v1,
            type_object_v1,
            data_representation,
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
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// ADR-0006 / zerodds-flatdata-1.0 §3.1: injects PID_SHM_LOCATOR
/// (vendor PID 0x8001) into an already PL-CDR-LE-encoded
/// `PublicationBuiltinTopicData` byte sequence. The vendor PID carries
/// NO MUST_UNDERSTAND bit — foreign vendors ignore it safely, ZeroDDS
/// readers on the same host attach to SHM.
///
/// Side-map pattern: the field does not move into the wire struct
/// (otherwise 21+ construction sites cross-workspace), but lives as a
/// `BTreeMap<EntityId, Vec<u8>>` in `DcpsRuntime` and is injected via
/// this helper at the wire-encode end.
///
/// The content of `locator_bytes` is the already-packed SHM locator
/// structure (see zerodds-flatdata-1.0 §3.1.2:
/// `u32 hostname_hash` plus `u32 uid` plus `u32 slot_count` plus
/// `u32 slot_size` plus CDR-string `segment_path`). The caller
/// serializes that before the call.
///
/// # Errors
/// `ValueOutOfRange` if `bytes` has no sentinel trailer
/// (`0x01 0x00 0x00 0x00`) at the end or is too short.
pub fn inject_pid_shm_locator(bytes: &mut Vec<u8>, locator_bytes: &[u8]) -> Result<(), WireError> {
    use crate::parameter_list::pid;
    if bytes.len() < 4 {
        return Err(WireError::ValueOutOfRange {
            message: "inject_pid_shm_locator: bytes too short",
        });
    }
    let sentinel_pos = bytes.len() - 4;
    // Sentinel tag = 0x01 0x00 (PID_SENTINEL) + 0x00 0x00 (length).
    if bytes[sentinel_pos..] != [0x01, 0x00, 0x00, 0x00] {
        return Err(WireError::ValueOutOfRange {
            message: "inject_pid_shm_locator: missing PID_SENTINEL trailer",
        });
    }
    // Padded to a 4-byte boundary for ParameterList conformance.
    let padded_len = (locator_bytes.len() + 3) & !3;
    if padded_len > u16::MAX as usize {
        return Err(WireError::ValueOutOfRange {
            message: "inject_pid_shm_locator: locator > u16::MAX",
        });
    }
    let mut inject = Vec::with_capacity(4 + padded_len + 4);
    inject.extend_from_slice(&pid::SHM_LOCATOR.to_le_bytes());
    inject.extend_from_slice(&(padded_len as u16).to_le_bytes());
    inject.extend_from_slice(locator_bytes);
    // Zero-pad to a 4-byte boundary.
    inject.resize(inject.len() + (padded_len - locator_bytes.len()), 0);
    // Append the (removed) sentinel trailer.
    inject.extend_from_slice(&bytes[sentinel_pos..]);
    bytes.truncate(sentinel_pos);
    bytes.extend_from_slice(&inject);
    Ok(())
}

/// The `PID_VENDORID` (0x0016) of an endpoint announcement, when it
/// states one.
///
/// DDSI-RTPS 2.5 §9.4.2.11.2 Table 9.6 gives the parameter ids
/// 0x8000..0xFFFF to vendors, one range shared by all of them: `0x8002` is
/// `PID_ZERODDS_TYPE_ID` in a ZeroDDS announcement and something else
/// entirely in RTI Connext's. Reading one without knowing the sender is
/// reading another product's parameter as one's own, and the value that
/// comes out of it is a well-formed lie — no length is wrong, no
/// discriminator is unknown, nothing reports anything.
///
/// `None` means the announcement did not state a vendor, and callers
/// treat that exactly like a foreign one: an unattributable parameter is
/// not read.
///
/// The field is two octets (§8.3.3.1) and the parameter is padded to
/// four; both lengths are accepted, because the padding is the sender's
/// business.
pub(crate) fn decoded_vendor_id(pl: &ParameterList) -> Option<crate::wire_types::VendorId> {
    let p = pl.find(pid::VENDOR_ID)?;
    if p.value.len() < 2 {
        return None;
    }
    Some(crate::wire_types::VendorId([p.value[0], p.value[1]]))
}

/// An endpoint's group from `PID_GROUP_GUID`, or from
/// `PID_GROUP_ENTITY_ID` on the endpoint's own participant (DDSI-RTPS 2.5
/// Table 9.19); `None` for neither or `GUID_UNKNOWN`.
pub(crate) fn group_guid_of(pl: &ParameterList, endpoint: Guid) -> Option<Guid> {
    let group = pl
        .find(pid::GROUP_GUID)
        .and_then(guid_from_param)
        .or_else(|| {
            let p = pl.find(pid::GROUP_ENTITY_ID)?;
            let id: [u8; 4] = p.value.get(..4)?.try_into().ok()?;
            Some(Guid::new(
                endpoint.prefix,
                crate::wire_types::EntityId::from_bytes(id),
            ))
        })?;
    (group.entity_id != crate::wire_types::EntityId::UNKNOWN).then_some(group)
}

pub(crate) fn guid_from_param(p: &Parameter) -> Option<Guid> {
    if p.value.len() == 16 {
        let mut g = [0u8; 16];
        g.copy_from_slice(&p.value);
        Some(Guid::from_bytes(g))
    } else {
        None
    }
}

/// CDR string as value bytes (incl. a 4-byte length prefix + null
/// terminator, plus possible padding to a 4-byte boundary). LE only —
/// ParameterList values are written in the endianness of the
/// submessage, which we have fixed to LE.
/// Encodes an 8-byte LE Duration_t. No trailing padding (out-aligned).
pub(crate) fn encode_duration_le(d: Duration) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&d.seconds.to_le_bytes());
    out[4..].copy_from_slice(&d.fraction.to_le_bytes());
    out
}

pub(crate) fn decode_duration(value: &[u8], little_endian: bool) -> Option<Duration> {
    if value.len() < 8 {
        return None;
    }
    let mut s = [0u8; 4];
    s.copy_from_slice(&value[..4]);
    let mut f = [0u8; 4];
    f.copy_from_slice(&value[4..8]);
    if little_endian {
        Some(Duration {
            seconds: i32::from_le_bytes(s),
            fraction: u32::from_le_bytes(f),
        })
    } else {
        Some(Duration {
            seconds: i32::from_be_bytes(s),
            fraction: u32::from_be_bytes(f),
        })
    }
}

/// Encode u32 LE.
pub(crate) fn encode_u32_le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

pub(crate) fn decode_u32(value: &[u8], little_endian: bool) -> Option<u32> {
    if value.len() < 4 {
        return None;
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&value[..4]);
    if little_endian {
        Some(u32::from_le_bytes(b))
    } else {
        Some(u32::from_be_bytes(b))
    }
}

pub(crate) fn decode_i32(value: &[u8], little_endian: bool) -> Option<i32> {
    decode_u32(value, little_endian).map(|u| u as i32)
}

/// The value of a `boolean` parameter (DDSI-RTPS 2.5 Table 9.18), `false`
/// where it is absent — the default Table 9.19 gives
/// `PID_EXPECTS_INLINE_QOS`.
///
/// # Errors
/// `ValueOutOfRange` when the first octet is neither 0 nor 1 or there is
/// none: CDR has no third boolean (DDS-XTypes 1.3 §7.4.1.1.1, Table 31:
/// "0 for false, 1 for true"), and reading it as either would decide
/// for the sender.
pub(crate) fn boolean_param(pl: &ParameterList, id: u16) -> Result<bool, WireError> {
    match pl.find(id).map(|p| p.value.first().copied()) {
        None => Ok(false),
        Some(Some(0)) => Ok(false),
        Some(Some(1)) => Ok(true),
        Some(_) => Err(WireError::ValueOutOfRange {
            message: "a boolean parameter whose octet is neither 0 nor 1",
        }),
    }
}

/// A `boolean` parameter: the octet and three of padding (DDSI-RTPS 2.5
/// §9.4.2.11, a parameter's length is a multiple of four).
pub(crate) fn boolean_value(value: bool) -> Vec<u8> {
    alloc::vec![u8::from(value), 0, 0, 0]
}

/// Encode LivelinessQos: 4 byte kind + 8 byte lease_duration = 12 byte.
pub(crate) fn encode_liveliness_le(l: zerodds_qos::LivelinessQosPolicy) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&(l.kind as u32).to_le_bytes());
    out.extend_from_slice(&encode_duration_le(l.lease_duration));
    out
}

pub(crate) fn decode_liveliness(
    value: &[u8],
    little_endian: bool,
) -> Option<zerodds_qos::LivelinessQosPolicy> {
    if value.len() < 12 {
        return None;
    }
    let kind_u = decode_u32(&value[..4], little_endian)?;
    let lease = decode_duration(&value[4..12], little_endian)?;
    Some(zerodds_qos::LivelinessQosPolicy {
        kind: zerodds_qos::LivelinessKind::from_u32(kind_u),
        lease_duration: lease,
    })
}

/// Partition = `sequence<string>`. CDR layout: u32 count + N × CDR string
/// (each CDR string with its own alignment padding).
/// Encodes an opaque `sequence<octet>` as `u32 length + N byte data`,
/// padded to a 4-byte boundary. DDS QoS UserData/TopicData/GroupData.
pub fn encode_octet_seq_le(data: &[u8]) -> Result<Vec<u8>, WireError> {
    let len = u32::try_from(data.len()).map_err(|_| WireError::ValueOutOfRange {
        message: "octet sequence length exceeds u32::MAX",
    })?;
    let mut out = Vec::with_capacity(4 + data.len() + 3);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    Ok(out)
}

/// Decodes an opaque `sequence<octet>` from the PID value.
pub fn decode_octet_seq(value: &[u8], little_endian: bool) -> Option<Vec<u8>> {
    let n = decode_u32(value, little_endian)? as usize;
    if 4 + n > value.len() {
        return None;
    }
    Some(value[4..4 + n].to_vec())
}

pub(crate) fn encode_partition_le(partitions: &[String]) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    let len = u32::try_from(partitions.len()).map_err(|_| WireError::ValueOutOfRange {
        message: "partition count exceeds u32::MAX",
    })?;
    out.extend_from_slice(&len.to_le_bytes());
    for p in partitions {
        // Each nested string starts on a 4-byte boundary relative to the
        // start of the PARTITION value. The outer count is exactly 4
        // bytes, so the first string is already aligned. After each
        // string we pad to 4 again.
        out.extend_from_slice(&encode_cdr_string_le(p)?);
    }
    Ok(out)
}

pub(crate) fn decode_partition(value: &[u8], little_endian: bool) -> Option<Vec<String>> {
    let n = decode_u32(value, little_endian)? as usize;
    // DoS cap: at most as many strings as would fit in the buffer with
    // a minimal 1-byte string + 4-byte length + 4-byte pad = 12 bytes
    // per entry. Otherwise a 4 GB reservation at n=u32::MAX.
    let cap = n.min(value.len().saturating_sub(4) / 5);
    let mut out = Vec::with_capacity(cap);
    let mut pos = 4;
    for _ in 0..n {
        if pos + 4 > value.len() {
            return None;
        }
        let mut lb = [0u8; 4];
        lb.copy_from_slice(&value[pos..pos + 4]);
        let slen = if little_endian {
            u32::from_le_bytes(lb)
        } else {
            u32::from_be_bytes(lb)
        } as usize;
        let next_raw_end = pos + 4 + slen;
        if next_raw_end > value.len() {
            return None;
        }
        let s =
            decode_cdr_string(&value[pos..next_raw_end.min(value.len())], little_endian).ok()?;
        out.push(s);
        // Pad to a 4-byte boundary.
        let padded_end = (next_raw_end + 3) & !3;
        pos = padded_end;
    }
    Some(out)
}

/// The bound of `string<256>` (DDSI-RTPS 2.5 Table 9.18: `PID_TOPIC_NAME`,
/// `PID_TYPE_NAME`, `PID_DOMAIN_TAG`; §9.3.2.1 `ContentFilterProperty_t`).
pub const STRING_256_MAX: usize = 256;

/// [`encode_cdr_string_le`] for a `string<N>`.
///
/// # Errors
/// `ValueOutOfRange` if `s` is longer than `max` octets.
pub(crate) fn encode_bounded_cdr_string_le(s: &str, max: usize) -> Result<Vec<u8>, WireError> {
    if s.len() > max {
        return Err(WireError::ValueOutOfRange {
            message: "string longer than the bound of string<256> (DDSI-RTPS 2.5 Table 9.18)",
        });
    }
    encode_cdr_string_le(s)
}

/// [`decode_cdr_string`] for a `string<N>`.
///
/// # Errors
/// As [`decode_cdr_string`]; `ValueOutOfRange` if the string is longer than
/// `max` octets.
pub(crate) fn decode_bounded_cdr_string(
    value: &[u8],
    little_endian: bool,
    max: usize,
) -> Result<String, WireError> {
    let s = decode_cdr_string(value, little_endian)?;
    if s.len() > max {
        return Err(WireError::ValueOutOfRange {
            message: "string longer than the bound of string<256> (DDSI-RTPS 2.5 Table 9.18)",
        });
    }
    Ok(s)
}

pub(crate) fn encode_cdr_string_le(s: &str) -> Result<Vec<u8>, WireError> {
    let bytes = s.as_bytes();
    let len =
        u32::try_from(bytes.len().saturating_add(1)).map_err(|_| WireError::ValueOutOfRange {
            message: "CDR string length exceeds u32::MAX",
        })?;
    let mut out = Vec::with_capacity(4 + bytes.len() + 4);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
    out.push(0); // null-terminator
    // Padding to a 4-byte boundary (required per ParameterList value)
    while out.len() % 4 != 0 {
        out.push(0);
    }
    Ok(out)
}

/// Decode a CDR string from value bytes. Ignores trailing padding.
/// The longest `string<256>` of DDS-RPC 1.0 §7.6.2.1.2, in octets.
pub(crate) const RPC_STRING_BOUND: usize = 256;

/// `service_instance_name`, `string<256>` (PID 0x0080, §7.6.2.1.2).
///
/// A string beyond the bound is refused; a value that is no CDR string
/// at all reads as absent, as it always has.
///
/// # Errors
/// [`WireError::ValueOutOfRange`] for a string longer than 256 octets.
pub(crate) fn service_instance_name_param(
    value: &[u8],
    little_endian: bool,
) -> Result<Option<String>, WireError> {
    match decode_cdr_string(value, little_endian) {
        Ok(name) if name.len() > RPC_STRING_BOUND => Err(WireError::ValueOutOfRange {
            message: "PID_SERVICE_INSTANCE_NAME is longer than string<256> (DDS-RPC 1.0 §7.6.2.1.2)",
        }),
        Ok(name) => Ok(Some(name)),
        Err(_) => Ok(None),
    }
}

/// `topic_aliases`, `sequence<string<256>>` (PID 0x0082, §7.6.2.1.2).
///
/// A member beyond the bound is refused; a value that does not decode
/// reads as absent, as it always has.
///
/// # Errors
/// [`WireError::ValueOutOfRange`] for an alias longer than 256 octets.
pub(crate) fn topic_aliases_param(
    value: &[u8],
    little_endian: bool,
) -> Result<Option<Vec<String>>, WireError> {
    match decode_partition(value, little_endian) {
        Some(aliases) if aliases.iter().any(|alias| alias.len() > RPC_STRING_BOUND) => {
            Err(WireError::ValueOutOfRange {
                message: "a PID_TOPIC_ALIASES entry is longer than string<256> (DDS-RPC 1.0 §7.6.2.1.2)",
            })
        }
        other => Ok(other),
    }
}

pub(crate) fn decode_cdr_string(value: &[u8], little_endian: bool) -> Result<String, WireError> {
    if value.len() < 4 {
        return Err(WireError::UnexpectedEof {
            needed: 4,
            offset: 0,
        });
    }
    let mut lb = [0u8; 4];
    lb.copy_from_slice(&value[..4]);
    let len = if little_endian {
        u32::from_le_bytes(lb)
    } else {
        u32::from_be_bytes(lb)
    } as usize;
    if len == 0 {
        return Err(WireError::ValueOutOfRange {
            message: "CDR string length 0 (missing null terminator)",
        });
    }
    if value.len() < 4 + len {
        return Err(WireError::UnexpectedEof {
            needed: 4 + len,
            offset: 0,
        });
    }
    let raw = &value[4..4 + len];
    if raw[len - 1] != 0 {
        return Err(WireError::ValueOutOfRange {
            message: "CDR string missing null terminator",
        });
    }
    String::from_utf8(raw[..len - 1].to_vec()).map_err(|_| WireError::ValueOutOfRange {
        message: "CDR string is not valid UTF-8",
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// DDSI-RTPS 2.5 §9.4.2.11: a locator parameter of a PL_CDR_BE
    /// announcement is big-endian, and is read, not dropped.
    #[test]
    fn locator_params_of_a_big_endian_list_are_read() {
        let loc = Locator::udp_v4([192, 168, 1, 10], 7411);
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::UNICAST_LOCATOR,
            loc.to_bytes_be().to_vec(),
        ));
        assert_eq!(
            collect_locator_params(&pl, pid::UNICAST_LOCATOR, false),
            [loc]
        );
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::UNICAST_LOCATOR,
            loc.to_bytes_le().to_vec(),
        ));
        assert_eq!(
            collect_locator_params(&pl, pid::UNICAST_LOCATOR, true),
            [loc]
        );
    }

    #[test]
    fn durability_try_from_u32_rejects_unknown() {
        assert_eq!(
            DurabilityKind::try_from_u32(0),
            Some(DurabilityKind::Volatile)
        );
        assert_eq!(
            DurabilityKind::try_from_u32(1),
            Some(DurabilityKind::TransientLocal)
        );
        assert_eq!(
            DurabilityKind::try_from_u32(3),
            Some(DurabilityKind::Persistent)
        );
        assert_eq!(DurabilityKind::try_from_u32(99), None);
    }

    #[test]
    fn reliability_try_from_u32_rejects_unknown() {
        assert_eq!(
            ReliabilityKind::try_from_u32(1),
            Some(ReliabilityKind::BestEffort)
        );
        assert_eq!(
            ReliabilityKind::try_from_u32(2),
            Some(ReliabilityKind::Reliable)
        );
        // 0 is not a valid reliability wire value.
        assert_eq!(ReliabilityKind::try_from_u32(0), None);
        assert_eq!(ReliabilityKind::try_from_u32(42), None);
    }

    #[test]
    fn an_absent_reliability_parameter_reads_as_reliable_on_a_publication() {
        // DDS 1.4 §2.2.3 Table 12 gives RELIABILITY a per-entity default:
        // BEST_EFFORT for a DataReader, RELIABLE for a DataWriter. A
        // publication is the writer's, so an omitted `PID_RELIABILITY`
        // is RELIABLE — Cyclone omits it for that reason, and reading it
        // as BEST_EFFORT made a reliable ZeroDDS reader refuse the match
        // by RxO while the other direction matched, which is a pair that
        // discovers, authenticates and then delivers nothing.
        let mut pl = ParameterList::default();
        pl.push(Parameter::new(
            pid::TOPIC_NAME,
            string_param_le("RoundtripBench_Echo"),
        ));
        pl.push(Parameter::new(
            pid::TYPE_NAME,
            string_param_le("RoundtripBench::Roundtrip"),
        ));
        pl.push(Parameter::new(
            pid::ENDPOINT_GUID,
            Guid::new(
                GuidPrefix::from_bytes([2; 12]),
                EntityId::user_writer_no_key([0, 0, 3]),
            )
            .to_bytes()
            .to_vec(),
        ));
        let bytes = pl_cdr_le_of(&pl);
        let data = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes)
            .expect("a publication with no reliability parameter still decodes");
        assert_eq!(
            data.reliability.kind,
            ReliabilityKind::Reliable,
            "an omitted PID_RELIABILITY on a publication is the writer default"
        );
    }

    /// A PL_CDR_LE encapsulation around a parameter list, as the SEDP
    /// reader sees it.
    fn pl_cdr_le_of(pl: &ParameterList) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec::Vec::new();
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&pl.to_bytes(true));
        out
    }

    /// A CDR string parameter value: little-endian length including the
    /// NUL, then the bytes.
    fn string_param_le(s: &str) -> alloc::vec::Vec<u8> {
        let mut v = alloc::vec::Vec::new();
        let n = u32::try_from(s.len() + 1).unwrap_or(0);
        v.extend_from_slice(&n.to_le_bytes());
        v.extend_from_slice(s.as_bytes());
        v.push(0);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v
    }

    #[test]
    fn legacy_from_u32_still_defaults_for_sedp_forward_compat() {
        // forward-compat path: unknown → default (the SEDP parser uses this).
        assert_eq!(DurabilityKind::from_u32(99), DurabilityKind::Volatile);
        assert_eq!(ReliabilityKind::from_u32(99), ReliabilityKind::BestEffort);
    }
    use crate::wire_types::{EntityId, GuidPrefix};
    use alloc::vec;

    fn sample_data() -> PublicationBuiltinTopicData {
        PublicationBuiltinTopicData {
            vendor_id: Some(crate::wire_types::VendorId::ZERODDS),
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: "ChatterTopic".into(),
            type_name: "std_msgs::String".into(),
            durability: DurabilityKind::Volatile,
            durability_service: DurabilityServiceQosPolicy::default(),
            reliability: ReliabilityQos {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_secs(10),
            },
            ownership: zerodds_qos::OwnershipKind::Shared,
            ownership_strength: 0,
            liveliness: zerodds_qos::LivelinessQosPolicy::default(),
            deadline: zerodds_qos::DeadlineQosPolicy::default(),
            latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
            destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
            lifespan: zerodds_qos::LifespanQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            partition: alloc::vec::Vec::new(),
            user_data: alloc::vec::Vec::new(),
            topic_data: alloc::vec::Vec::new(),
            group_data: alloc::vec::Vec::new(),
            type_information: None,
            type_id_v1: None,
            type_object_v1: None,
            data_representation: alloc::vec::Vec::new(),
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
            group_guid: None,
            data_max_size_serialized: None,
            entity_name: None,
        }
    }

    /// LIVELINESS, DEADLINE and LIFESPAN at their default (Duration_t
    /// INFINITE) are left out of the announcement and read back as the
    /// default; other values are sent and read back as they are (DDSI-RTPS
    /// 2.5 §9.4.2.11).
    #[test]
    fn the_infinite_defaults_are_left_out_and_read_back() {
        let ids = |d: &PublicationBuiltinTopicData| -> Vec<u16> {
            d.parameters()
                .unwrap()
                .parameters
                .iter()
                .map(|p| p.id)
                .collect()
        };
        let d = sample_data();
        for id in [pid::LIVELINESS, pid::DEADLINE, pid::LIFESPAN] {
            assert!(!ids(&d).contains(&id), "{id:#06x} at its default");
        }
        let back = PublicationBuiltinTopicData::from_pl_cdr_le(&d.to_pl_cdr_le().unwrap()).unwrap();
        assert_eq!(back.liveliness, zerodds_qos::LivelinessQosPolicy::default());
        assert_eq!(back.deadline, zerodds_qos::DeadlineQosPolicy::default());
        assert_eq!(back.lifespan, zerodds_qos::LifespanQosPolicy::default());
        let mut set = sample_data();
        set.liveliness = zerodds_qos::LivelinessQosPolicy {
            kind: zerodds_qos::LivelinessKind::ManualByTopic,
            lease_duration: Duration::from_secs(2),
        };
        set.deadline.period = Duration::from_secs(3);
        set.lifespan.duration = Duration::from_secs(4);
        for id in [pid::LIVELINESS, pid::DEADLINE, pid::LIFESPAN] {
            assert!(ids(&set).contains(&id), "{id:#06x} set");
        }
        let back =
            PublicationBuiltinTopicData::from_pl_cdr_le(&set.to_pl_cdr_le().unwrap()).unwrap();
        assert_eq!(
            (back.liveliness, back.deadline, back.lifespan),
            (set.liveliness, set.deadline, set.lifespan)
        );
    }

    /// DDSI-RTPS 2.5 Table 9.18: `PID_TOPIC_NAME` and `PID_TYPE_NAME` are
    /// `string<256>`. 256 octets are encoded and read; 257 are refused in both
    /// directions, and the refusal is an error.
    #[test]
    fn topic_and_type_names_are_bounded_by_string_256() {
        let mut d = sample_data();
        d.topic_name = "t".repeat(256);
        d.type_name = "T".repeat(256);
        let bytes = d.to_pl_cdr_le().expect("256 octets fit");
        let back = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(back.topic_name.len(), 256);
        assert_eq!(back.type_name.len(), 256);

        let mut long_topic = sample_data();
        long_topic.topic_name = "t".repeat(257);
        assert!(long_topic.to_pl_cdr_le().is_err());
        let mut long_type = sample_data();
        long_type.type_name = "T".repeat(257);
        assert!(long_type.to_pl_cdr_le().is_err());

        // A peer's 257-octet topic name: build the list by hand.
        let mut bytes = sample_data().to_pl_cdr_le().unwrap();
        let name = encode_cdr_string_le(&"t".repeat(257)).unwrap();
        let at = bytes
            .windows(2)
            .position(|w| w == pid::TOPIC_NAME.to_le_bytes())
            .expect("PID_TOPIC_NAME");
        let old_len = u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]) as usize;
        let mut spliced = bytes[..at].to_vec();
        spliced.extend_from_slice(&pid::TOPIC_NAME.to_le_bytes());
        spliced.extend_from_slice(&(name.len() as u16).to_le_bytes());
        spliced.extend_from_slice(&name);
        spliced.extend_from_slice(&bytes[at + 4 + old_len..]);
        bytes = spliced;
        assert!(PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).is_err());
    }

    /// DDSI-RTPS 2.5 §8.7.2.1: the in-line list is `PID_TOPIC_NAME` and the
    /// Table 8.85 policies the announcement carries, in the octets SEDP
    /// sends for them; the GUIDs, the type, the locators and the
    /// `USER_DATA`/`TOPIC_DATA`/`GROUP_DATA` policies (Table 8.85: "No")
    /// stay out.
    #[test]
    fn inline_qos_is_the_announcements_in_line_policies_and_the_topic_name() {
        let mut data = sample_data();
        data.partition = alloc::vec!["p".into()];
        data.user_data = alloc::vec![1, 2, 3];
        data.ownership_strength = 7;
        let list = data.inline_qos(0).unwrap();
        let ids: alloc::vec::Vec<u16> = list.parameters.iter().map(|p| p.id).collect();
        assert_eq!(ids.first(), Some(&pid::TOPIC_NAME), "{ids:x?}");
        for id in [pid::PARTITION, pid::RELIABILITY, pid::OWNERSHIP_STRENGTH] {
            assert!(ids.contains(&id), "{id:#06x} missing from {ids:x?}");
        }
        for id in [
            pid::ENDPOINT_GUID,
            pid::PARTICIPANT_GUID,
            pid::TYPE_NAME,
            pid::USER_DATA,
            pid::DURABILITY_SERVICE,
            pid::VENDOR_ID,
        ] {
            assert!(!ids.contains(&id), "{id:#06x} is not in-line QoS");
        }
        let announced =
            ParameterList::from_bytes(&data.to_pl_cdr_le().unwrap()[4..], true).unwrap();
        for parameter in &list.parameters {
            assert_eq!(
                announced.find(parameter.id).map(|p| &p.value),
                Some(&parameter.value),
                "{:#06x} differs from the announcement",
                parameter.id
            );
        }
    }

    /// TRANSPORT_PRIORITY is in-line QoS (Table 8.85) and in no
    /// announcement: the writer's value goes into the in-line list when
    /// it is not the default 0 (§8.7.2.1).
    #[test]
    fn a_transport_priority_goes_in_line_and_not_into_the_announcement() {
        let data = sample_data();
        let with = data.inline_qos(0x28).unwrap();
        assert_eq!(
            with.find(pid::TRANSPORT_PRIORITY).map(|p| p.value.clone()),
            Some(0x28_i32.to_le_bytes().to_vec())
        );
        let without = data.inline_qos(0).unwrap();
        assert!(without.find(pid::TRANSPORT_PRIORITY).is_none());
        let announced =
            ParameterList::from_bytes(&data.to_pl_cdr_le().unwrap()[4..], true).unwrap();
        assert!(announced.find(pid::TRANSPORT_PRIORITY).is_none());
    }

    #[test]
    fn issue30_type_identifier_from_standard_type_information() {
        // #30: a foreign-vendor publication advertises the STANDARD
        // PID_TYPE_INFORMATION and NOT the ZeroDDS-proprietary ZERODDS_TYPE_ID.
        // ZeroDDS must still recover the TypeIdentifier from the standard blob so
        // the XTypes assignability path runs cross-vendor instead of falling back
        // to type-name equality (match-then-decode-fail).
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        use zerodds_types::type_identifier::PrimitiveKind;
        let b = TypeObjectBuilder::struct_type("::chat::Chatter")
            .extensibility(Extensibility::Appendable)
            .member(
                "id",
                zerodds_types::TypeIdentifier::Primitive(PrimitiveKind::Int64),
                |m| m.key(),
            );
        let minimal = zerodds_types::MinimalTypeObject::Struct(b.build_minimal());
        let complete = zerodds_types::CompleteTypeObject::Struct(b.build_complete());
        let ti =
            zerodds_types::TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
        // The minimal equivalence hash is preferred for match-time assignability
        // (it is the identity peers resolve structurally via TypeLookup); the
        // complete identifier is only the fallback when minimal is absent.
        let expected = ti.minimal.typeid_with_size.type_id.clone();
        assert_ne!(expected, zerodds_types::TypeIdentifier::None);

        let mut d = sample_data();
        d.type_information = Some(ti.to_bytes_le().unwrap());
        // Foreign vendor: no ZeroDDS-proprietary identifier.
        d.type_identifier = zerodds_types::TypeIdentifier::None;

        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.type_identifier, expected,
            "TypeIdentifier must be recovered from standard PID_TYPE_INFORMATION (#30)"
        );
    }

    /// A vendor-specific parameter belongs to the vendor that wrote it.
    ///
    /// Measured on the Linux test host: RTI Connext 7.7.0 announces a
    /// `0x8002` of its own beside its `PID_TYPE_INFORMATION`. ZeroDDS read
    /// it as `PID_ZERODDS_TYPE_ID`, decoded its first octet `0x01` as the
    /// discriminator `TK_BOOLEAN`, and every RTI endpoint arrived carrying
    /// `TypeIdentifier::Primitive(Boolean)`. Not `None`, so the standard
    /// blob was never consulted; the assignability check then refused the
    /// endpoint with `kinds do not match`.
    #[test]
    fn a_foreign_vendors_0x8002_is_not_read_as_a_type_identifier() {
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
        use zerodds_types::type_identifier::PrimitiveKind;

        let b = TypeObjectBuilder::struct_type("::interop::Robot")
            .extensibility(Extensibility::Final)
            .member(
                "id",
                zerodds_types::TypeIdentifier::Primitive(PrimitiveKind::UInt32),
                |m| m,
            );
        let minimal = zerodds_types::MinimalTypeObject::Struct(b.build_minimal());
        let complete = zerodds_types::CompleteTypeObject::Struct(b.build_complete());
        let ti =
            zerodds_types::TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
        let from_the_standard_blob = ti.minimal.typeid_with_size.type_id.clone();
        assert_ne!(from_the_standard_blob, zerodds_types::TypeIdentifier::None);

        // The announcement of a peer that is not ZeroDDS: its own vendor
        // id, the standard type information, and four octets under 0x8002
        // that mean whatever that vendor says they mean. RTI's begin with
        // `0x01`, which is `TK_BOOLEAN` in the discriminator space this
        // parameter carries for ZeroDDS.
        let mut d = sample_data();
        d.type_information = Some(ti.to_bytes_le().unwrap());
        d.type_identifier = zerodds_types::TypeIdentifier::None;
        let mut bytes = d.to_pl_cdr_le().unwrap();
        replace_vendor_id(&mut bytes, crate::wire_types::VendorId([0x01, 0x01]));
        inject_parameter(&mut bytes, pid::ZERODDS_TYPE_ID, &[0x01, 0x00, 0x00, 0x00]);

        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.type_identifier, from_the_standard_blob,
            "a foreign 0x8002 must be ignored and the standard blob used"
        );
        assert_ne!(
            decoded.type_identifier,
            zerodds_types::TypeIdentifier::Primitive(PrimitiveKind::Boolean),
            "the foreign parameter was read as a ZeroDDS TypeIdentifier"
        );
    }

    /// The other half: ZeroDDS's own `0x8002` still is one, and the
    /// announcement states the vendor that makes it readable.
    #[test]
    fn a_zerodds_0x8002_is_read_and_the_announcement_states_the_vendor() {
        use zerodds_types::type_identifier::PrimitiveKind;

        let mut d = sample_data();
        d.type_identifier = zerodds_types::TypeIdentifier::Primitive(PrimitiveKind::Int64);
        let bytes = d.to_pl_cdr_le().unwrap();

        let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
        assert_eq!(
            decoded_vendor_id(&pl),
            Some(crate::wire_types::VendorId::ZERODDS),
            "the announcement has to name the vendor whose 0x8002 it carries"
        );

        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.type_identifier, d.type_identifier);
    }

    /// The vendor an announcement states is the vendor it keeps.
    ///
    /// Reading a foreign announcement and writing it back put this tree's
    /// `PID_VENDORID` on it, because the encoder wrote the constant
    /// `VendorId::ZERODDS` while the decoder kept the announced value
    /// nowhere. Measured on the four captures in
    /// `crates/discovery/tests/fixtures/vendor-discovery/`: 14 Cyclone, 14
    /// Fast DDS and 2 RTI announcements came back as ZeroDDS's.
    ///
    /// Two things follow from the substitution, and both are asserted here:
    /// the announcement of a Cyclone endpoint said ZeroDDS, and a receiver
    /// reading that would have read every 0x8002 on it as this tree's
    /// `TypeIdentifier` — the very rule the vendor id is there to enforce
    /// (Table 9.6: "Vendor-specific ParameterId. Will not be recognized by
    /// other vendors' implementations.").
    #[test]
    fn a_foreign_vendor_id_survives_read_and_write_back() {
        use zerodds_types::type_identifier::PrimitiveKind;

        let rti = crate::wire_types::VendorId([0x01, 0x01]);
        let bytes = {
            let mut b = sample_data().to_pl_cdr_le().unwrap();
            replace_vendor_id(&mut b, rti);
            b
        };

        let mut decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.vendor_id,
            Some(rti),
            "the announced vendor has to reach the decoded announcement"
        );

        // A `TypeIdentifier` this tree holds for a foreign announcement
        // comes from the standard `PID_TYPE_INFORMATION` (#30). Writing it
        // back under 0x8002 would offer another vendor's parameter id
        // carrying this tree's value.
        decoded.type_identifier = zerodds_types::TypeIdentifier::Primitive(PrimitiveKind::Int64);
        let back = decoded.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&back[4..], true).unwrap();
        assert_eq!(
            decoded_vendor_id(&pl),
            Some(rti),
            "the write-back replaced the announced vendor with this tree's"
        );
        assert!(
            pl.find(pid::ZERODDS_TYPE_ID).is_none(),
            "0x8002 was written on an announcement that states another vendor"
        );
    }

    /// Appends one parameter in front of the sentinel of an encoded
    /// announcement — the shape of [`inject_pid_shm_locator`], for an
    /// arbitrary id.
    fn inject_parameter(bytes: &mut Vec<u8>, id: u16, value: &[u8]) {
        let sentinel_at = bytes.len() - 4;
        assert_eq!(
            bytes[sentinel_at..],
            [0x01, 0x00, 0x00, 0x00],
            "no PID_SENTINEL trailer"
        );
        let padded = (value.len() + 3) & !3;
        let mut inject = Vec::with_capacity(4 + padded + 4);
        inject.extend_from_slice(&id.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        inject.extend_from_slice(&(padded as u16).to_le_bytes());
        inject.extend_from_slice(value);
        inject.resize(inject.len() + (padded - value.len()), 0);
        inject.extend_from_slice(&bytes[sentinel_at..]);
        bytes.truncate(sentinel_at);
        bytes.extend_from_slice(&inject);
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

    #[test]
    fn endpoint_locators_roundtrip_le() {
        let mut d = sample_data();
        d.unicast_locators = alloc::vec![
            Locator::udp_v4([192, 168, 1, 5], 7411),
            Locator::udp_v4([10, 0, 0, 9], 7411),
        ];
        d.multicast_locators = alloc::vec![Locator::udp_v4([239, 255, 0, 2], 7401)];
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.unicast_locators, d.unicast_locators);
        assert_eq!(decoded.multicast_locators, d.multicast_locators);
    }

    #[test]
    fn roundtrip_le() {
        let d = sample_data();
        let bytes = d.to_pl_cdr_le().unwrap();
        assert_eq!(&bytes[..2], &[0x00, 0x03]); // PL_CDR_LE
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, d);
    }

    /// DDS 1.4 §2.2.3.1/.2/.3: USER_DATA, TOPIC_DATA and GROUP_DATA are
    /// `sequence<octet>` values whose only job is to reach the other side
    /// unaltered. The default sample carries all three empty, so a decoder
    /// that dropped a non-empty one would still pass `roundtrip_le`.
    #[test]
    fn user_topic_and_group_data_survive_the_sedp_roundtrip() {
        let mut d = sample_data();
        d.user_data = alloc::vec![0x00, 0x01, 0xFE, 0xFF, 0x7F];
        d.topic_data = alloc::vec![0xDE, 0xAD];
        d.group_data = (0..37u8).collect();
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.user_data, d.user_data);
        assert_eq!(decoded.topic_data, d.topic_data);
        assert_eq!(decoded.group_data, d.group_data);
        // And they are carried under their own three PIDs, not conflated.
        let pl = crate::parameter_list::ParameterList::from_bytes(&bytes[4..], true).unwrap();
        assert!(pl.find(pid::USER_DATA).is_some());
        assert!(pl.find(pid::TOPIC_DATA).is_some());
        assert!(pl.find(pid::GROUP_DATA).is_some());
    }

    #[test]
    fn security_info_roundtrip() {
        use crate::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};
        let mut d = sample_data();
        d.security_info = Some(EndpointSecurityInfo {
            endpoint_security_attributes: attrs::IS_VALID | attrs::IS_SUBMESSAGE_PROTECTED,
            plugin_endpoint_security_attributes: plugin_attrs::IS_VALID
                | plugin_attrs::IS_SUBMESSAGE_ENCRYPTED,
        });
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.security_info, d.security_info);
    }

    /// DDS-Security 1.2 §7.5.1.5 Table 13: the symmetric cipher info of an
    /// endpoint is `PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO` 0x1013, the
    /// `required_mask` as one u32; absent stays absent.
    #[test]
    fn endpoint_symmetric_cipher_info_roundtrip() {
        use crate::security_algo_info::{
            EndpointSecuritySymmetricCipherAlgorithmInfo, symmetric_bit,
        };
        let mut d = sample_data();
        assert!(d.sym_cipher_algo_info.is_none());
        let plain = d.to_pl_cdr_le().unwrap();
        assert!(
            !plain.windows(2).any(|w| w == [0x13, 0x10]),
            "not sent when absent"
        );
        d.sym_cipher_algo_info = Some(EndpointSecuritySymmetricCipherAlgorithmInfo {
            required_mask: symmetric_bit::AES128,
            supported_mask: 0,
        });
        let bytes = d.to_pl_cdr_le().unwrap();
        // PID 0x1013 LE = 13 10, length 4, value 1 (LE).
        assert!(
            bytes
                .windows(8)
                .any(|w| w == [0x13, 0x10, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00]),
            "PID 0x1013 with the required mask missing from the stream"
        );
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.sym_cipher_algo_info, d.sym_cipher_algo_info);
    }

    /// DDS-Security 1.2 §7.3.21, §7.5.1.7: the `propagate = TRUE`
    /// entries of the PropertyQosPolicy go out as `PID_PROPERTY_LIST` 0x0059
    /// (string and binary part); absent stays absent; an entry that does not
    /// decode rejects the sample.
    /// DDS-Security 1.2 §7.5.1.7, §7.5.1.8: the DataTags of an endpoint are
    /// `PID_DATA_TAGS` 0x1003; absent stays absent.
    #[test]
    fn endpoint_data_tags_roundtrip() {
        use crate::property_list::WireProperty;
        let mut d = sample_data();
        assert!(d.data_tags.is_empty());
        let plain = d.to_pl_cdr_le().unwrap();
        assert!(
            !plain.windows(2).any(|w| w == [0x03, 0x10]),
            "not sent when absent"
        );
        d.data_tags.tags.push(WireProperty::new("level", "secret"));
        let bytes = d.to_pl_cdr_le().unwrap();
        assert!(bytes.windows(2).any(|w| w == [0x03, 0x10]));
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.data_tags, d.data_tags);
    }

    #[test]
    fn endpoint_property_list_roundtrip() {
        use crate::property_list::{WireBinaryProperty, WireProperty};
        let mut d = sample_data();
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
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.properties, d.properties);
        assert_eq!(decoded.properties.get("dds.sec.x"), Some("v"));
    }

    #[test]
    fn legacy_peer_without_security_info_parses_ok() {
        let d = sample_data();
        assert!(d.security_info.is_none());
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.security_info.is_none());
    }

    #[test]
    fn roundtrip_utf8_topic_name() {
        let mut d = sample_data();
        d.topic_name = "Zählung".into();
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.topic_name, "Zählung");
    }

    #[test]
    fn decode_rejects_unknown_encapsulation() {
        let mut bytes = vec![0xFF, 0xFF, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 16]);
        let res = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnsupportedEncapsulation { .. })
        ));
    }

    #[test]
    fn decode_rejects_missing_topic_name() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::ENDPOINT_GUID, vec![0u8; 16]));
        pl.push(Parameter::new(
            pid::TYPE_NAME,
            encode_cdr_string_le("T").unwrap(),
        ));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&pl.to_bytes(true));
        let res = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes);
        assert!(
            matches!(res, Err(WireError::ValueOutOfRange { message }) if message.contains("TOPIC_NAME"))
        );
    }

    #[test]
    fn decode_rejects_missing_type_name() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::ENDPOINT_GUID, vec![0u8; 16]));
        pl.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_cdr_string_le("T").unwrap(),
        ));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&pl.to_bytes(true));
        let res = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes);
        assert!(
            matches!(res, Err(WireError::ValueOutOfRange { message }) if message.contains("TYPE_NAME"))
        );
    }

    #[test]
    fn decode_rejects_missing_endpoint_guid() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_cdr_string_le("T").unwrap(),
        ));
        pl.push(Parameter::new(
            pid::TYPE_NAME,
            encode_cdr_string_le("U").unwrap(),
        ));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&pl.to_bytes(true));
        let res = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes);
        assert!(
            matches!(res, Err(WireError::ValueOutOfRange { message }) if message.contains("ENDPOINT_GUID"))
        );
    }

    #[test]
    fn unknown_pids_are_skipped() {
        let mut bytes = sample_data().to_pl_cdr_le().unwrap();
        // Insert a new unknown PID (0x7FFF, 4 byte) before the sentinel.
        // The sentinel parameter is the last 4 bytes.
        let sentinel_pos = bytes.len() - 4;
        let mut inject = vec![0xFFu8, 0x7F, 4, 0, 0xDE, 0xAD, 0xBE, 0xEF];
        inject.extend_from_slice(&bytes[sentinel_pos..]);
        bytes.truncate(sentinel_pos);
        bytes.extend_from_slice(&inject);
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, sample_data());
    }

    #[test]
    fn inject_pid_shm_locator_appends_before_sentinel() {
        // Locator body: 16 byte (four u32) + 8 byte CDR string "x"
        // (4 byte len = 2, 1 byte 'x', 1 byte null, 2 byte pad).
        let mut locator = Vec::new();
        locator.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes()); // hostname_hash
        locator.extend_from_slice(&1000u32.to_le_bytes()); // uid
        locator.extend_from_slice(&64u32.to_le_bytes()); // slot_count
        locator.extend_from_slice(&4096u32.to_le_bytes()); // slot_size
        // CDR string "/dev/shm/zd-1\0":
        let path = "/dev/shm/zd-1";
        locator.extend_from_slice(&((path.len() as u32) + 1).to_le_bytes());
        locator.extend_from_slice(path.as_bytes());
        locator.push(0);
        // Pad to a 4-byte boundary.
        let pad = (4 - locator.len() % 4) % 4;
        locator.resize(locator.len() + pad, 0);

        let mut bytes = sample_data().to_pl_cdr_le().unwrap();
        let len_before = bytes.len();
        super::inject_pid_shm_locator(&mut bytes, &locator).unwrap();
        // The bytes have grown (PID header 4 + locator).
        assert!(bytes.len() > len_before);
        // And still decode — the vendor PID is ignored as unknown (no
        // MUST_UNDERSTAND bit), the rest stays identical.
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, sample_data());
        // Sanity: PID 0x8001 is actually present.
        let pid_found = bytes.windows(2).any(|w| w == 0x8001u16.to_le_bytes());
        assert!(pid_found, "PID_SHM_LOCATOR should appear in bytes");
    }

    #[test]
    fn inject_pid_shm_locator_rejects_missing_sentinel() {
        let mut bytes = vec![0u8; 8];
        let res = super::inject_pid_shm_locator(&mut bytes, &[0u8; 16]);
        assert!(res.is_err());
    }

    #[test]
    fn inject_pid_shm_locator_rejects_too_short() {
        let mut bytes = vec![0u8, 1u8];
        let res = super::inject_pid_shm_locator(&mut bytes, &[0u8; 16]);
        assert!(res.is_err());
    }

    /// §10.3: "The receiver shall ignore the value of the
    /// representation_options."
    #[test]
    fn the_receiver_ignores_the_representation_options_of_a_publication() {
        let mut bytes = sample_data().to_pl_cdr_le().unwrap();
        assert_eq!(&bytes[2..4], &[0, 0], "the sender writes zero");
        let plain = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        bytes[2] = 0x12;
        bytes[3] = 0x34;
        assert_eq!(
            PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap(),
            plain
        );
    }

    /// Table 8.72 `dataMaxSizeSerialized` (`PID_TYPE_MAX_SIZE_SERIALIZED`,
    /// a `long`) and Table 9.18 `PID_ENTITY_NAME` (`EntityName_t`, a CDR
    /// string) go out when set, read back, and are left out when not.
    #[test]
    fn the_max_payload_size_and_the_name_of_a_writer_travel() {
        let mut d = sample_data();
        assert_eq!(
            (d.data_max_size_serialized, d.entity_name.clone()),
            (None, None)
        );
        let bytes = d.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
        assert!(pl.find(pid::TYPE_MAX_SIZE_SERIALIZED).is_none());
        assert!(pl.find(pid::ENTITY_NAME).is_none());

        d.data_max_size_serialized = Some(1028);
        d.entity_name = Some("Chatter writer".into());
        let bytes = d.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
        assert_eq!(
            pl.find(pid::TYPE_MAX_SIZE_SERIALIZED)
                .map(|p| p.value.clone()),
            Some(1028i32.to_le_bytes().to_vec())
        );
        // CDR string: length 15 with the terminator, the characters, NUL;
        // the parameter value is padded to a multiple of 4.
        let mut name = 15u32.to_le_bytes().to_vec();
        name.extend_from_slice(b"Chatter writer\0\0");
        assert_eq!(
            pl.find(pid::ENTITY_NAME).map(|p| p.value.clone()),
            Some(name)
        );
        let back = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(back.data_max_size_serialized, Some(1028));
        assert_eq!(back.entity_name.as_deref(), Some("Chatter writer"));
    }

    /// DDSI-RTPS 2.5 Table 9.19: `PID_GROUP_GUID` carries
    /// `WriterProxy::remoteGroupGuid`, `PID_GROUP_ENTITY_ID` its `entityId`
    /// alone; both default to unknown and are then left out.
    #[test]
    fn the_writers_group_travels_as_pid_group_guid() {
        let mut d = sample_data();
        assert_eq!(d.group_guid, None);
        let bytes = d.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
        assert!(pl.find(pid::GROUP_GUID).is_none(), "unknown is left out");

        let publisher = Guid::new(
            d.key.prefix,
            crate::wire_types::EntityId::from_bytes([0, 0, 7, 0x08]),
        );
        d.group_guid = Some(publisher);
        let bytes = d.to_pl_cdr_le().unwrap();
        let pl = ParameterList::from_bytes(&bytes[4..], true).unwrap();
        assert_eq!(
            pl.find(pid::GROUP_GUID).map(|p| p.value.clone()),
            Some(publisher.to_bytes().to_vec())
        );
        assert_eq!(
            PublicationBuiltinTopicData::from_pl_cdr_le(&bytes)
                .unwrap()
                .group_guid,
            Some(publisher)
        );

        // PID_GROUP_ENTITY_ID alone names a group of the writer's own
        // participant.
        let mut pl =
            ParameterList::from_bytes(&sample_data().to_pl_cdr_le().unwrap()[4..], true).unwrap();
        pl.push(Parameter::new(pid::GROUP_ENTITY_ID, vec![0, 0, 7, 0x08]));
        assert_eq!(group_guid_of(&pl, d.key), Some(publisher));
    }

    #[test]
    fn participant_key_fallback_when_pid_missing() {
        // Build a PL without PARTICIPANT_GUID: the decoder should derive
        // participant_key from ENDPOINT_GUID.prefix + PARTICIPANT.
        let d = sample_data();
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::ENDPOINT_GUID,
            d.key.to_bytes().to_vec(),
        ));
        pl.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_cdr_string_le(&d.topic_name).unwrap(),
        ));
        pl.push(Parameter::new(
            pid::TYPE_NAME,
            encode_cdr_string_le(&d.type_name).unwrap(),
        ));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&pl.to_bytes(true));
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.participant_key.prefix, d.key.prefix);
        assert_eq!(decoded.participant_key.entity_id, EntityId::PARTICIPANT);
    }

    #[test]
    fn durability_kind_from_u32_unknown_defaults_volatile() {
        assert_eq!(DurabilityKind::from_u32(0), DurabilityKind::Volatile);
        assert_eq!(DurabilityKind::from_u32(1), DurabilityKind::TransientLocal);
        assert_eq!(DurabilityKind::from_u32(999), DurabilityKind::Volatile);
    }

    #[test]
    fn rpc_discovery_pids_roundtrip() {
        let mut d = sample_data();
        d.service_instance_name = Some("CalcInstance-1".into());
        d.related_entity_guid = Some(Guid::new(
            crate::wire_types::GuidPrefix::from_bytes([7; 12]),
            crate::wire_types::EntityId::user_reader_with_key([0xAA, 0xBB, 0xCC]),
        ));
        d.topic_aliases = Some(alloc::vec!["LegacyCalc_Request".into(), "v2_Req".into()]);

        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded.service_instance_name, d.service_instance_name);
        assert_eq!(decoded.related_entity_guid, d.related_entity_guid);
        assert_eq!(decoded.topic_aliases, d.topic_aliases);
    }

    /// DDS-RPC 1.0 §7.6.2.1.2: `service_instance_name` is a `string<256>`
    /// and `topic_aliases` a `sequence<string<256>>`. A longer string is
    /// refused where it used to be dropped from the data.
    #[test]
    fn a_discovery_string_beyond_the_bound_is_refused() {
        let mut d = sample_data();
        d.service_instance_name = Some("x".repeat(256));
        d.topic_aliases = Some(alloc::vec!["y".repeat(256)]);
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(
            decoded.service_instance_name.as_deref().map(str::len),
            Some(256)
        );
        assert_eq!(decoded.topic_aliases.as_ref().map(Vec::len), Some(1));

        let mut name = sample_data();
        name.service_instance_name = Some("x".repeat(257));
        let bytes = name.to_pl_cdr_le().unwrap();
        assert!(matches!(
            PublicationBuiltinTopicData::from_pl_cdr_le(&bytes),
            Err(WireError::ValueOutOfRange { .. })
        ));

        let mut alias = sample_data();
        alias.topic_aliases = Some(alloc::vec!["ok".into(), "y".repeat(257)]);
        let bytes = alias.to_pl_cdr_le().unwrap();
        assert!(matches!(
            PublicationBuiltinTopicData::from_pl_cdr_le(&bytes),
            Err(WireError::ValueOutOfRange { .. })
        ));
    }

    #[test]
    fn rpc_pids_optional_legacy_peer_parses_ok() {
        let d = sample_data();
        assert!(d.service_instance_name.is_none());
        assert!(d.related_entity_guid.is_none());
        assert!(d.topic_aliases.is_none());
        let bytes = d.to_pl_cdr_le().unwrap();
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert!(decoded.service_instance_name.is_none());
        assert!(decoded.related_entity_guid.is_none());
        assert!(decoded.topic_aliases.is_none());
    }

    #[test]
    fn rpc_pid_constants_in_emitted_bytes() {
        // Cyclone-compat snapshot: PID 0x0080..0x0083 must appear
        // byte-exact in the stream, otherwise a Cyclone reader cannot
        // dispatch them.
        let mut d = sample_data();
        d.service_instance_name = Some("X".into());
        d.related_entity_guid = Some(Guid::new(
            crate::wire_types::GuidPrefix::from_bytes([1; 12]),
            crate::wire_types::EntityId::PARTICIPANT,
        ));
        d.topic_aliases = Some(alloc::vec!["A".into()]);
        let bytes = d.to_pl_cdr_le().unwrap();
        // PIDs are 2-byte little-endian.
        let mut found_080 = false;
        let mut found_081 = false;
        let mut found_082 = false;
        for w in bytes.windows(2) {
            if w == [0x80, 0x00] {
                found_080 = true;
            }
            if w == [0x81, 0x00] {
                found_081 = true;
            }
            if w == [0x82, 0x00] {
                found_082 = true;
            }
        }
        assert!(found_080 && found_081 && found_082);
    }

    #[test]
    fn reliability_kind_from_u32_unknown_defaults_best_effort() {
        assert_eq!(ReliabilityKind::from_u32(1), ReliabilityKind::BestEffort);
        assert_eq!(ReliabilityKind::from_u32(2), ReliabilityKind::Reliable);
        assert_eq!(ReliabilityKind::from_u32(999), ReliabilityKind::BestEffort);
    }
}
