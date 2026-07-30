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

    /// ZeroDDS default announce list for writers and readers.
    ///
    /// **XCDR2-only.** The codegen (`idl-rust`/`idl-cpp`) emits **real
    /// XCDR2** since the version-aware alignment cap (XTypes 1.3
    /// §7.4.1.1.1, 64-bit to 4) — the encap (`user_payload_encap`)
    /// derives from `offer_first`. Since the body is fixed XCDR2-aligned,
    /// `offer_first` MUST be XCDR2; XCDR1 must NOT be in the list,
    /// otherwise an XCDR1-only reader matches wrongly (tolerant mode) and
    /// reads the XCDR2 body with the wrong alignment.
    ///
    /// Match behavior:
    /// - Reader [XCDR2] / [XCDR1, XCDR2] (Cyclone/FastDDS/modern RTI): ✓
    /// - Reader [XCDR1] (legacy-strict, e.g. old RTI shapes): ✗ — such
    ///   peers need an XCDR1 codegen (separate feature), not just an
    ///   offer change.
    ///
    /// User override: globally via `RuntimeConfig::data_representation_offer`
    /// or env `ZERODDS_DATA_REPR_OFFER`.
    pub const DEFAULT_OFFER: [i16; 1] = [XCDR2];

    /// `DataRepMatchMode` — determines how writer and reader compare
    /// DataRep lists.
    ///
    /// * **Strict** (XTypes 1.3 §7.6.3.1.2 normative): the writer's FIRST
    ///   element must be in the reader's list. Exactly like RTI Connext.
    /// * **Tolerant** (industry norm, Cyclone + FastDDS): match when the
    ///   lists overlap (any-overlap), wire format = first-overlap.
    ///
    /// Default in ZeroDDS: `Tolerant` — maximizes interop, because our
    /// own readers also match RTI writers even when the first element is
    /// not 100% identical.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum DataRepMatchMode {
        /// Strict-spec match: Writer.first ∈ Reader.list.
        Strict,
        /// Tolerant match: any element in Writer.list ∈ Reader.list.
        /// Industry default.
        #[default]
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
    /// Topic-Name (DDS-Topic, z.B. "ChatterTopic").
    pub topic_name: String,
    /// IDL-Type-Name (z.B. "std_msgs::String").
    pub type_name: String,
    /// Durability-QoS.
    pub durability: DurabilityKind,
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
    /// UserData QoS (spec §2.2.3.1) — opaque sequence<octet>,
    /// discovery-propagated. Empty vec = not set.
    pub user_data: Vec<u8>,
    /// TopicData QoS (spec §2.2.3.3) — opaque sequence<octet>,
    /// propagated from the topic via pub discovery.
    pub topic_data: Vec<u8>,
    /// GroupData QoS (spec §2.2.3.2) — opaque sequence<octet>,
    /// propagated from the publisher via pub discovery.
    pub group_data: Vec<u8>,
    /// Type information (TypeIdentifier hashes + dependencies, XTypes
    /// §7.6.3.2.2). Opaque bytes: the structure lives in `zerodds-types`,
    /// but we transport the serialized blob to avoid circular crate
    /// dependencies.
    pub type_information: Option<Vec<u8>>,
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
    /// PID_SERVICE_INSTANCE_NAME (DDS-RPC 1.0 §7.8.2) — logical
    /// service-instance name of an RPC endpoint. `None` for ordinary
    /// pub/sub topics.
    pub service_instance_name: Option<String>,
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.8.2) — GUID of the
    /// counterpart endpoint in an RPC endpoint pair. For a request
    /// writer it points to the reply reader of the same requester; for a
    /// reply writer to the request reader of the same replier.
    pub related_entity_guid: Option<Guid>,
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.8.2) — alternative topic names
    /// for routing/compat layers. Order is significant.
    pub topic_aliases: Option<Vec<String>>,
    /// PID_ZERODDS_TYPE_ID (vendor PID 0x8002) — XTypes 1.3 §7.3.4.2
    /// TypeIdentifier of the writer type for XTypes-aware reader matching
    /// (XTypes §7.6.3.7 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
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
/// list). BE locators are skipped — consistent with
/// `participant_data::decode_locator` (BE locator not implemented).
#[must_use]
pub fn collect_locator_params(
    pl: &ParameterList,
    pid_id: u16,
    little_endian: bool,
) -> Vec<Locator> {
    if !little_endian {
        return Vec::new();
    }
    pl.parameters
        .iter()
        .filter(|p| p.id == pid_id && p.value.len() == Locator::WIRE_SIZE)
        .filter_map(|p| {
            let mut b = [0u8; 24];
            b.copy_from_slice(&p.value);
            Locator::from_bytes_le(b).ok()
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
        let mut params = ParameterList::new();

        // PARTICIPANT_GUID: 16 Byte
        params.push(Parameter::new(
            pid::PARTICIPANT_GUID,
            self.participant_key.to_bytes().to_vec(),
        ));

        // ENDPOINT_GUID: 16 Byte
        params.push(Parameter::new(
            pid::ENDPOINT_GUID,
            self.key.to_bytes().to_vec(),
        ));

        // TOPIC_NAME: CDR-String (4 byte len + UTF-8 + null)
        params.push(Parameter::new(
            pid::TOPIC_NAME,
            encode_cdr_string_le(&self.topic_name)?,
        ));

        // TYPE_NAME: CDR-String
        params.push(Parameter::new(
            pid::TYPE_NAME,
            encode_cdr_string_le(&self.type_name)?,
        ));

        // DURABILITY: 4 Byte u32
        params.push(Parameter::new(
            pid::DURABILITY,
            (self.durability as u32).to_le_bytes().to_vec(),
        ));

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

        // LIVELINESS: 4 Byte kind + 8 Byte lease_duration
        params.push(Parameter::new(
            pid::LIVELINESS,
            encode_liveliness_le(self.liveliness),
        ));

        // DEADLINE: 8 Byte Duration_t
        params.push(Parameter::new(
            pid::DEADLINE,
            encode_duration_le(self.deadline.period).to_vec(),
        ));

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

        // LIFESPAN: 8 Byte Duration_t
        params.push(Parameter::new(
            pid::LIFESPAN,
            encode_duration_le(self.lifespan.duration).to_vec(),
        ));

        // PRESENTATION: 4 byte access_scope + 1 byte coherent + 1 byte
        // ordered + 2 byte padding = 8 byte (spec §2.2.3.6 / PID 0x0021).
        // Always emitted (like RELIABILITY/DURABILITY) so a strict peer
        // (e.g. RTI) can SEDP-match on it.
        {
            let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little);
            self.presentation
                .encode_into(&mut w)
                .map_err(|_| WireError::ValueOutOfRange {
                    message: "presentation encoding failed",
                })?;
            params.push(Parameter::new(pid::PRESENTATION, w.into_bytes()));
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

        // ENDPOINT_SECURITY_INFO: 2x u32 masks (§7.4.1.5). Only if set,
        // otherwise legacy behavior (Cyclone/Fast-DDS without security
        // leave the PID out).
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

        let mut out = Vec::with_capacity(params.parameters.len() * 24 + 16);
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&params.to_bytes(true));
        Ok(out)
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
            .unwrap_or_default();

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
            topic_name,
            type_name,
            durability,
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
            data_representation,
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

/// Partition = sequence<string>. CDR layout: u32 count + N × CDR string
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
    fn legacy_from_u32_still_defaults_for_sedp_forward_compat() {
        // forward-compat path: unknown → default (the SEDP parser uses this).
        assert_eq!(DurabilityKind::from_u32(99), DurabilityKind::Volatile);
        assert_eq!(ReliabilityKind::from_u32(99), ReliabilityKind::BestEffort);
    }
    use crate::wire_types::{EntityId, GuidPrefix};
    use alloc::vec;

    fn sample_data() -> PublicationBuiltinTopicData {
        PublicationBuiltinTopicData {
            key: Guid::new(
                GuidPrefix::from_bytes([1; 12]),
                EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
            ),
            participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
            topic_name: "ChatterTopic".into(),
            type_name: "std_msgs::String".into(),
            durability: DurabilityKind::Volatile,
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
