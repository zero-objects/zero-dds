// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PublicationBuiltinTopicData (DDSI-RTPS 2.5 §8.5.4.2, §9.6.2.2.3).
//!
//! Inhalt der SEDP-Publications-DATA-Submessage, die ein Participant
//! sendet, um einen lokalen DataWriter bei Remote-Participants bekannt
//! zu machen. Serialisiert als PL_CDR_LE-encoded ParameterList in der
//! `serialized_payload` einer DATA-Submessage.
//!
//! topic_name + type_name + GUIDs +
//! minimale QoS-Felder (durability, reliability). Keine Deadline,
//! Liveliness, Lifespan, Ownership, Partition etc. — die werden
//! gelesen und in `extra`-Vec gespeichert, aber nicht typisiert.
//!
//! **QoS-Enums hier lokal** — sobald WP 1.5 volles QoS-Matching
//! bringt, wandern DurabilityKind/ReliabilityKind nach `zerodds-qos`.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::endpoint_security_info::EndpointSecurityInfo;
use crate::error::WireError;
use crate::parameter_list::{Parameter, ParameterList, pid};
use crate::participant_data::{Duration, ENCAPSULATION_PL_CDR_LE};
use crate::wire_types::Guid;

/// Durability-QoS Kind.
///
/// Canonical in [`zerodds_qos::DurabilityKind`]; RTPS re-exportiert für
/// Abwärtskompatibilität.
pub use zerodds_qos::DurabilityKind;

/// Reliability-QoS Kind.
///
/// Canonical in [`zerodds_qos::ReliabilityKind`]; RTPS re-exportiert.
pub use zerodds_qos::ReliabilityKind;

/// Reliability-QoS Wert: Kind + max_blocking_time.
///
/// Canonical in [`zerodds_qos::ReliabilityQosPolicy`]; RTPS re-exportiert
/// unter dem historischen Alias `ReliabilityQos`.
pub use zerodds_qos::ReliabilityQosPolicy as ReliabilityQos;

/// `DataRepresentationId` — XTypes 1.3 §7.6.3.1.1 + RTPS 2.5 PID 0x0073.
///
/// Pro Spec: 16-bit signed integer; Werte 0..2 sind normativ definiert.
/// Pro RTI/Cyclone/FastDDS Convention werden weitere Werte als
/// vendor-specific reserviert.
pub mod data_representation {
    /// XCDR1 (legacy CDR Plain-CDR + PL_CDR mutable). Default wenn das
    /// PID nicht present ist (Spec §7.6.3.1.2).
    pub const XCDR: i16 = 0;
    /// XML (rare, für CFP-Profile). Nicht in unserer Default-Liste.
    pub const XML: i16 = 1;
    /// XCDR2 (PLAIN_CDR2 + DELIMITED_CDR2 + PL_CDR2 mutable).
    /// ZeroDDS' nativer Encap (`0x0007`/`0x0009`/`0x000B`).
    pub const XCDR2: i16 = 2;

    /// ZeroDDS-Default-Announce-Liste fuer Writer und Reader.
    ///
    /// **XCDR1 first** = legacy preferred (matched von strict-Spec-
    /// Vendoren wie RTI Connext + RTI Shapes Demo deren Reader nur
    /// `[XCDR1]` akzeptieren). **XCDR2 second** als modern-Fallback
    /// fuer Peers die XCDR1 nicht koennen.
    ///
    /// Per Spec strict (XTypes 1.3 §7.6.3.1.2): Writer-Match besteht
    /// wenn Writer's first-Element in Reader's accepted-list ist.
    /// `[XCDR1, XCDR2]` matched also:
    /// - Reader [XCDR1] (legacy/RTI strict): writer.first=XCDR1 ∈ {XCDR1} ✓
    /// - Reader [XCDR1, XCDR2] (ZeroDDS/Cyclone/FastDDS): ✓
    /// - Reader [XCDR2] (modern only): writer.first=XCDR1 ∉ {XCDR2} ✗
    ///   → fuer XCDR2-only-Peers muss der User die Liste umstellen.
    ///
    /// User-Override: pro Writer/Reader via QoS, oder global durch
    /// `RuntimeConfig::data_representation_offer` (TBD).
    pub const DEFAULT_OFFER: [i16; 2] = [XCDR, XCDR2];

    /// `DataRepMatchMode` — bestimmt, wie Writer und Reader DataRep-
    /// Listen vergleichen.
    ///
    /// * **Strict** (XTypes 1.3 §7.6.3.1.2 normativ): Writer's FIRST
    ///   Element muss in Reader's List sein. Genau wie RTI Connext.
    /// * **Tolerant** (Industry-Norm, Cyclone + FastDDS): Match wenn
    ///   die Listen ueberlappen (any-overlap), Wire-Format = first-overlap.
    ///
    /// Default in ZeroDDS: `Tolerant` — maximiert Interop, weil unsere
    /// eigenen Reader auch dann RTI-Writer matchen, wenn das first-Element
    /// nicht 100% deckungsgleich ist.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum DataRepMatchMode {
        /// Strict-Spec Match: Writer.first ∈ Reader.list.
        Strict,
        /// Tolerant Match: any element in Writer.list ∈ Reader.list.
        /// Industry-Default.
        #[default]
        Tolerant,
    }

    /// Bestimmt das ausgehandelte Wire-Format zwischen Writer und Reader.
    ///
    /// Liefert `Some(id)` mit der DataRepresentationId die im Wire
    /// emittiert werden soll. `None` heisst: keine Ueberlappung —
    /// kein Match.
    ///
    /// `writer_offered` kann mehrere Werte enthalten (z.B.
    /// `[XCDR2, XCDR1]`); `reader_accepted` ebenfalls.
    /// Beide Listen koennen leer sein — Spec-Default ist `[XCDR1]`
    /// in dem Fall (Spec §7.6.3.1.2).
    #[must_use]
    pub fn negotiate(
        writer_offered: &[i16],
        reader_accepted: &[i16],
        mode: DataRepMatchMode,
    ) -> Option<i16> {
        // Spec-Defaults bei leeren Listen.
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
                // §7.6.3.1.2: Writer's first element muss in Reader's list sein.
                let first = w.first().copied()?;
                if r.contains(&first) {
                    Some(first)
                } else {
                    None
                }
            }
            DataRepMatchMode::Tolerant => {
                // Industry: any overlap. Wir bevorzugen Writer's
                // Praeferenz-Reihenfolge (first-match wins), aber
                // sehen die VOLLE writer-Liste, nicht nur first.
                w.iter().copied().find(|id| r.contains(id))
            }
        }
    }

    /// Encap-Header (4 byte) fuer @final-Structs unter der gegebenen
    /// DataRep. Fuer @appendable/@mutable wird ein anderer
    /// Encap-Code benoetigt — siehe `encap_for_extensibility`.
    ///
    /// Zurueckgabe-Format: `[byte0, byte1, byte2, byte3]` wobei
    /// byte0 immer `0x00` und byte1 die Repr-ID nach RTPS 2.5
    /// §10.5.
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
    /// GUID des Participants, dem der Writer gehoert.
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
    /// Ownership-Strength (Spec §2.2.3.24). Nur relevant wenn
    /// `ownership == Exclusive`; Default 0.
    pub ownership_strength: i32,
    /// Liveliness-QoS (Spec §2.2.3.11).
    pub liveliness: zerodds_qos::LivelinessQosPolicy,
    /// Deadline-QoS (Spec §2.2.3.7).
    pub deadline: zerodds_qos::DeadlineQosPolicy,
    /// Lifespan-QoS (Spec §2.2.3.16) — writer-only.
    pub lifespan: zerodds_qos::LifespanQosPolicy,
    /// Partition-QoS (Spec §2.2.3.13). Leere Liste = "default partition" ("").
    pub partition: Vec<String>,
    /// UserData-QoS (Spec §2.2.3.1) — opaque sequence<octet>, Discovery-
    /// propagiert. Leerer Vec = nicht gesetzt.
    pub user_data: Vec<u8>,
    /// TopicData-QoS (Spec §2.2.3.3) — opaque sequence<octet>, vom
    /// Topic via Pub-Discovery propagiert.
    pub topic_data: Vec<u8>,
    /// GroupData-QoS (Spec §2.2.3.2) — opaque sequence<octet>, vom
    /// Publisher via Pub-Discovery propagiert.
    pub group_data: Vec<u8>,
    /// Type-Information (TypeIdentifier-Hashes + Dependencies, XTypes
    /// §7.6.3.2.2). Opaque bytes: die Struktur lebt in `zerodds-types`,
    /// aber wir transportieren den serialisierten Blob, um zirkulaere
    /// Crate-Abhaengigkeiten zu vermeiden.
    pub type_information: Option<Vec<u8>>,
    /// Akzeptierte Data-Representations (0=XCDR1, 1=XML, 2=XCDR2, ...).
    /// Spec: XTypes 1.3 §7.6.3.1.1 / RTPS 2.5 PID 0x0073.
    /// Default-Liste bei leer ist `[XCDR1]` per XTypes §7.6.3.1.2 — wir
    /// emittieren das PID immer explicit, damit Strict-Vendoren wie
    /// RTI 7.7.0 SEDP-matchen koennen.
    pub data_representation: Vec<i16>,
    /// Endpoint-Security-Info (PID 0x1004, DDS-Security 1.1 §7.4.1.5).
    /// `None` bei Legacy-Peers ohne Security-PID. WP 4H-c matched
    /// darauf: Writer/Reader-Paare mit inkompatiblen Protection-Leveln
    /// werden abgelehnt.
    pub security_info: Option<EndpointSecurityInfo>,
    /// PID_SERVICE_INSTANCE_NAME (DDS-RPC 1.0 §7.8.2) — logischer
    /// Service-Instance-Name eines RPC-Endpoints. `None` fuer
    /// gewoehnliche Pub/Sub-Topics.
    pub service_instance_name: Option<String>,
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.8.2) — GUID des
    /// Pendant-Endpoints in einem RPC-Endpoint-Pair. Bei einem
    /// Request-Writer zeigt das auf den Reply-Reader desselben
    /// Requesters; bei einem Reply-Writer auf den Request-Reader
    /// desselben Repliers.
    pub related_entity_guid: Option<Guid>,
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.8.2) — alternative Topic-
    /// Namen fuer Routing-/Compat-Layer. Reihenfolge ist signifikant.
    pub topic_aliases: Option<Vec<String>>,
    /// PID_ZERODDS_TYPE_ID (Vendor-PID 0x8002) — XTypes-1.3 §7.3.4.2
    /// TypeIdentifier des Writer-Type für XTypes-aware Reader-Match
    /// (XTypes §7.6.3.7 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
    pub type_identifier: zerodds_types::TypeIdentifier,
}

impl PublicationBuiltinTopicData {
    /// Encoded zu PL_CDR_LE-Bytes (mit 4-byte Encapsulation-Header).
    /// Output ist direkt als `serialized_payload` einer DATA-
    /// Submessage verwendbar.
    ///
    /// # Errors
    /// `ValueOutOfRange` wenn ein String laenger als u32::MAX ist.
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

        // OWNERSHIP_STRENGTH: 4 Byte int32 (nur sinnvoll bei Exclusive,
        // aber wir schicken's immer — Reader ignoriert bei Shared).
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

        // LIFESPAN: 8 Byte Duration_t
        params.push(Parameter::new(
            pid::LIFESPAN,
            encode_duration_le(self.lifespan.duration).to_vec(),
        ));

        // PARTITION: nur wenn non-empty — leere Liste = Default (= "").
        if !self.partition.is_empty() {
            params.push(Parameter::new(
                pid::PARTITION,
                encode_partition_le(&self.partition)?,
            ));
        }

        // USER_DATA / TOPIC_DATA / GROUP_DATA: opaque sequence<octet>.
        // Wire = 4 byte u32 length + N byte data. Nur wenn gesetzt
        // (leerer Vec = Default, lassen wir aus dem ParameterList).
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

        // TYPE_INFORMATION: serialisierter TypeInformation-Blob
        // (optional, XTypes §7.6.3.2.2).
        if let Some(ti) = &self.type_information {
            params.push(Parameter::new(pid::TYPE_INFORMATION, ti.clone()));
        }

        // ENDPOINT_SECURITY_INFO: 2x u32 masks (§7.4.1.5). Nur wenn gesetzt,
        // sonst Legacy-Verhalten (Cyclone/Fast-DDS ohne Security lassen
        // die PID weg).
        if let Some(info) = self.security_info {
            params.push(Parameter::new(
                pid::ENDPOINT_SECURITY_INFO,
                info.to_bytes(true).to_vec(),
            ));
        }

        // ----------------------------------------------------------------
        // DDS-RPC 1.0 Discovery-PIDs (§7.8.2) — nur wenn gesetzt.
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

        // DATA_REPRESENTATION: sequence<int16> — u32 Laenge + 2*N bytes.
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

        let mut out = Vec::with_capacity(params.parameters.len() * 24 + 16);
        out.extend_from_slice(&ENCAPSULATION_PL_CDR_LE);
        out.extend_from_slice(&[0, 0]); // options
        out.extend_from_slice(&params.to_bytes(true));
        Ok(out)
    }

    /// Decoded aus PL_CDR_LE-Bytes (mit Encapsulation-Header).
    ///
    /// # Errors
    /// `UnexpectedEof` bei zu kurzen Bytes,
    /// `UnsupportedEncapsulation` bei unbekanntem Encoding,
    /// `ValueOutOfRange` wenn Pflicht-PIDs fehlen oder Werte
    /// falsche Laenge haben.
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

        // PARTICIPANT_GUID ist technisch optional (kann aus ENDPOINT_GUID-
        // Prefix abgeleitet werden), aber wir verlangen es, wenn es da ist.
        let participant_key = pl
            .find(pid::PARTICIPANT_GUID)
            .and_then(guid_from_param)
            .unwrap_or_else(|| {
                // Fallback: Participant = ENDPOINT_GUID.prefix + PARTICIPANT-EntityId
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
                        // BE-Decoding: seconds+fraction als BE interpretieren
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

        let lifespan = pl
            .find(pid::LIFESPAN)
            .and_then(|p| decode_duration(&p.value, little_endian))
            .map(|duration| zerodds_qos::LifespanQosPolicy { duration })
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
                // DoS-Cap: Vec::with_capacity(n) koennte bei n=u32::MAX/2
                // ca. 4 GB reservieren. Kappen auf tatsaechlich lesbare
                // Elemente: (v.len()-4)/2 i16-Elemente.
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
            lifespan,
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
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// ADR-0006 / zerodds-flatdata-1.0 §3.1: injiziert PID_SHM_LOCATOR
/// (Vendor-PID 0x8001) in eine bereits PL-CDR-LE-encodierte
/// `PublicationBuiltinTopicData` Bytes-Sequenz. Das Vendor-PID
/// traegt KEIN MUST_UNDERSTAND-Bit — fremde Vendoren ignorieren
/// es safe, ZeroDDS-Reader auf demselben Host attachen an SHM.
///
/// Side-Map-Pattern: das Feld wandert nicht in den Wire-Struct
/// (sonst 21+ Construction-Sites cross-workspace), sondern liegt
/// als `BTreeMap<EntityId, Vec<u8>>` in `DcpsRuntime` und wird
/// via diese Helper am Wire-Encode-Ende eingebracht.
///
/// Der Inhalt von `locator_bytes` ist die bereits gepackte
/// SHM-Locator-Struktur (siehe zerodds-flatdata-1.0 §3.1.2:
/// `u32 hostname_hash` plus `u32 uid` plus `u32 slot_count` plus
/// `u32 slot_size` plus CDR-String `segment_path`). Der Caller
/// serialisiert das vor dem Aufruf.
///
/// # Errors
/// `ValueOutOfRange` wenn `bytes` keinen Sentinel-Trailer
/// (`0x01 0x00 0x00 0x00`) am Ende hat oder zu kurz ist.
pub fn inject_pid_shm_locator(bytes: &mut Vec<u8>, locator_bytes: &[u8]) -> Result<(), WireError> {
    use crate::parameter_list::pid;
    if bytes.len() < 4 {
        return Err(WireError::ValueOutOfRange {
            message: "inject_pid_shm_locator: bytes too short",
        });
    }
    let sentinel_pos = bytes.len() - 4;
    // Sentinel-Tag = 0x01 0x00 (PID_SENTINEL) + 0x00 0x00 (length).
    if bytes[sentinel_pos..] != [0x01, 0x00, 0x00, 0x00] {
        return Err(WireError::ValueOutOfRange {
            message: "inject_pid_shm_locator: missing PID_SENTINEL trailer",
        });
    }
    // Padded auf 4-Byte-Boundary fuer ParameterList-Konformitaet.
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
    // Zero-Pad auf 4-Byte-Boundary.
    inject.resize(inject.len() + (padded_len - locator_bytes.len()), 0);
    // Append den (entfernten) Sentinel-Trailer.
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

/// CDR-String als Value-Bytes (inkl. 4-Byte-Length-Prefix + Null-
/// Terminator, plus evtl. Padding auf 4-Byte-Boundary). LE only —
/// ParameterList-Values werden in der Endianness der Submessage
/// geschrieben, die wir auf LE festgelegt haben.
/// Encoded 8 Byte LE Duration_t. Kein trailing padding (aus-aligned).
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

/// LivelinessQos encoden: 4 Byte kind + 8 Byte lease_duration = 12 Byte.
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

/// Partition = sequence<string>. CDR-Layout: u32 count + N × CDR-String
/// (jeder CDR-String mit eigenem Alignment-Padding).
/// Encoded eine opaque `sequence<octet>` als `u32 length + N byte data`,
/// gepaddet auf 4-Byte-Boundary. DDS QoS UserData/TopicData/GroupData.
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

/// Decoded eine opaque `sequence<octet>` aus dem PID-Value.
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
        // Jeder nested String beginnt auf 4-Byte-Grenze relativ zum
        // Start der PARTITION-Value. Outer-count ist genau 4 Byte, also
        // ist der erste String schon aligned. Nach jedem String padden
        // wir erneut auf 4.
        out.extend_from_slice(&encode_cdr_string_le(p)?);
    }
    Ok(out)
}

pub(crate) fn decode_partition(value: &[u8], little_endian: bool) -> Option<Vec<String>> {
    let n = decode_u32(value, little_endian)? as usize;
    // DoS-Cap: maximal so viele Strings wie bei minimalem 1-Byte-String
    // + 4-byte-length + 4-byte-pad = 12 bytes pro Eintrag im Puffer
    // Platz haetten. Bei n=u32::MAX sonst 4 GB reservation.
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
        // Pad auf 4-Byte-Boundary.
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
    // Padding auf 4-Byte-Boundary (pro ParameterList-Wert gefordert)
    while out.len() % 4 != 0 {
        out.push(0);
    }
    Ok(out)
}

/// CDR-String aus Value-Bytes dekodieren. Ignoriert Trailing-Padding.
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
        // 0 ist kein gueltiger Reliability-Wire-Wert.
        assert_eq!(ReliabilityKind::try_from_u32(0), None);
        assert_eq!(ReliabilityKind::try_from_u32(42), None);
    }

    #[test]
    fn legacy_from_u32_still_defaults_for_sedp_forward_compat() {
        // forward-compat Path: unbekannt → Default (SEDP-Parser nutzt das).
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
        // Neuen unbekannten PID (0x7FFF, 4 byte) vor Sentinel einfuegen.
        // Der Sentinel-Parameter ist die letzten 4 Bytes.
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
        // Locator-Body: 16 byte (vier u32) + 8 byte CDR-String "x"
        // (4 byte len = 2, 1 byte 'x', 1 byte null, 2 byte pad).
        let mut locator = Vec::new();
        locator.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes()); // hostname_hash
        locator.extend_from_slice(&1000u32.to_le_bytes()); // uid
        locator.extend_from_slice(&64u32.to_le_bytes()); // slot_count
        locator.extend_from_slice(&4096u32.to_le_bytes()); // slot_size
        // CDR-String "/dev/shm/zd-1\0":
        let path = "/dev/shm/zd-1";
        locator.extend_from_slice(&((path.len() as u32) + 1).to_le_bytes());
        locator.extend_from_slice(path.as_bytes());
        locator.push(0);
        // Pad auf 4-Byte-Boundary.
        let pad = (4 - locator.len() % 4) % 4;
        locator.resize(locator.len() + pad, 0);

        let mut bytes = sample_data().to_pl_cdr_le().unwrap();
        let len_before = bytes.len();
        super::inject_pid_shm_locator(&mut bytes, &locator).unwrap();
        // Bytes sind gewachsen (PID-Header 4 + locator).
        assert!(bytes.len() > len_before);
        // Und decodieren immer noch — Vendor-PID wird als unbekannt
        // ignoriert (kein MUST_UNDERSTAND-Bit), der Rest bleibt
        // identisch.
        let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).unwrap();
        assert_eq!(decoded, sample_data());
        // Sanity: PID 0x8001 ist tatsaechlich enthalten.
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
        // Bauen wir ein PL ohne PARTICIPANT_GUID: der Decoder soll
        // participant_key aus ENDPOINT_GUID.prefix + PARTICIPANT ableiten.
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
        // Cyclone-Compat-Snapshot: PID 0x0080..0x0083 muessen byte-genau
        // im Stream auftauchen, sonst kann ein Cyclone-Reader sie nicht
        // dispatchen.
        let mut d = sample_data();
        d.service_instance_name = Some("X".into());
        d.related_entity_guid = Some(Guid::new(
            crate::wire_types::GuidPrefix::from_bytes([1; 12]),
            crate::wire_types::EntityId::PARTICIPANT,
        ));
        d.topic_aliases = Some(alloc::vec!["A".into()]);
        let bytes = d.to_pl_cdr_le().unwrap();
        // PIDs sind 2-byte little-endian.
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
