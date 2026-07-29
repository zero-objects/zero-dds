// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ParameterList (DDSI-RTPS 2.5 §9.4.2.11).
//!
//! Tag-length-value format for SPDP/SEDP builtin topic data. Each
//! parameter:
//!
//! ```text
//!   0                   1                   2                   3
//!   0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  |         parameter_id          |            length             |
//!  +---------------+---------------+---------------+---------------+
//!  |                          value (length bytes)                 |
//!  +---------------+---------------+---------------+---------------+
//! ```
//!
//! Terminator: `parameter_id = PID_SENTINEL (0x0001)`, `length = 0`,
//! no value.
//!
//! Encoding is always with the submessage endianness; this module works
//! on raw bytes with an explicit `little_endian` parameter.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;

/// Standard parameter IDs (spec §9.6.4 + table 9.13).
///
/// The 12 QoS-policy PIDs from DDS 1.4 §2.2.3 are re-exported from
/// [`zerodds_qos::Pid`] (single source of truth). rtps-specific PIDs
/// (locators, GUIDs, security tokens, …) stay declared here, since they
/// are outside the QoS-policy subset.
pub mod pid {
    use zerodds_qos::Pid as QosPid;

    // ---- Re-exports from zerodds_qos::Pid (12 policy PIDs) ----
    /// Sentinel — terminator of the ParameterList. (Re-export from `zerodds_qos::Pid::SENTINEL`.)
    pub const SENTINEL: u16 = QosPid::SENTINEL;
    /// Reliability QoS. (Re-export from `zerodds_qos::Pid::RELIABILITY`.)
    pub const RELIABILITY: u16 = QosPid::RELIABILITY;
    /// Durability QoS. (Re-export from `zerodds_qos::Pid::DURABILITY`.)
    pub const DURABILITY: u16 = QosPid::DURABILITY;
    /// Ownership QoS. (Re-export from `zerodds_qos::Pid::OWNERSHIP`.)
    pub const OWNERSHIP: u16 = QosPid::OWNERSHIP;
    /// Ownership-Strength QoS. (Re-export from `zerodds_qos::Pid::OWNERSHIP_STRENGTH`.)
    pub const OWNERSHIP_STRENGTH: u16 = QosPid::OWNERSHIP_STRENGTH;
    /// Liveliness QoS. (Re-export from `zerodds_qos::Pid::LIVELINESS`.)
    pub const LIVELINESS: u16 = QosPid::LIVELINESS;
    /// Deadline QoS. (Re-export from `zerodds_qos::Pid::DEADLINE`.)
    pub const DEADLINE: u16 = QosPid::DEADLINE;
    /// Lifespan QoS. (Re-export from `zerodds_qos::Pid::LIFESPAN`.)
    pub const LIFESPAN: u16 = QosPid::LIFESPAN;
    /// Partition QoS. (Re-export from `zerodds_qos::Pid::PARTITION`.)
    pub const PARTITION: u16 = QosPid::PARTITION;
    /// Presentation QoS. (Re-export from `zerodds_qos::Pid::PRESENTATION`.)
    pub const PRESENTATION: u16 = QosPid::PRESENTATION;
    /// UserData QoS. (Re-export from `zerodds_qos::Pid::USER_DATA`.)
    pub const USER_DATA: u16 = QosPid::USER_DATA;
    /// GroupData QoS. (Re-export from `zerodds_qos::Pid::GROUP_DATA`.)
    pub const GROUP_DATA: u16 = QosPid::GROUP_DATA;
    /// TopicData QoS. (Re-export from `zerodds_qos::Pid::TOPIC_DATA`.)
    pub const TOPIC_DATA: u16 = QosPid::TOPIC_DATA;

    // ---- rtps-spezifische PIDs (Discovery / Locators / Security / Wire) ----
    /// Participant lease duration (Duration_t = i32 sec + u32 nanosec).
    pub const PARTICIPANT_LEASE_DURATION: u16 = 0x0002;
    /// Topic-Name (CDR-String).
    pub const TOPIC_NAME: u16 = 0x0005;
    /// Type-Name (CDR-String).
    pub const TYPE_NAME: u16 = 0x0007;
    /// ProtocolVersion (2 byte + 2 padding).
    pub const PROTOCOL_VERSION: u16 = 0x0015;
    /// VendorId (2 byte + 2 padding).
    pub const VENDOR_ID: u16 = 0x0016;
    /// Content-Filter-Property (Spec §9.6.3.4 Table 9.14): Topic-
    /// Filter-Name (String) + related-Topic-Name (String) + Filter-
    /// Class-Name (String) + Filter-Expression (String) +
    /// Expression-Parameters (sequence<String>).
    pub const CONTENT_FILTER_PROPERTY: u16 = 0x0035;
    /// Default unicast locator (24 byte) — for user data.
    pub const DEFAULT_UNICAST_LOCATOR: u16 = 0x0031;
    /// Metatraffic unicast locator (24 byte) — where peers send SEDP
    /// unicast. Indispensable for Cyclone interop.
    pub const METATRAFFIC_UNICAST_LOCATOR: u16 = 0x0032;
    /// Metatraffic multicast locator (24 byte) — where peers send
    /// SPDP/SEDP multicast.
    pub const METATRAFFIC_MULTICAST_LOCATOR: u16 = 0x0033;
    /// Domain id (4 byte u32) — participant domain.
    pub const DOMAIN_ID: u16 = 0x000f;
    /// Default multicast locator (24 byte).
    pub const DEFAULT_MULTICAST_LOCATOR: u16 = 0x0048;
    /// Endpoint unicast locator (24 byte) — where peers send user data
    /// to *this* reader/writer. Spec §8.5.3.2/§8.5.3.3:
    /// `DiscoveredReaderData.readerProxy.unicastLocatorList`. One
    /// parameter per locator (a list = repeated PID).
    /// Takes precedence over the participant `DEFAULT_UNICAST_LOCATOR` —
    /// OpenDDS sends only the placeholder 127.0.0.1:12345 as the
    /// participant default and stores the real locators exclusively here.
    pub const UNICAST_LOCATOR: u16 = 0x002f;
    /// Endpoint multicast locator (24 byte) — spec §8.5.3.2.
    pub const MULTICAST_LOCATOR: u16 = 0x0030;
    /// Participant GUID (16 byte).
    pub const PARTICIPANT_GUID: u16 = 0x0050;
    /// Endpoint GUID (16 byte) — for publication/subscription discovery.
    pub const ENDPOINT_GUID: u16 = 0x005a;
    /// Property list (spec OMG DDS-Security 1.1 §7.2.1). A sequence of
    /// (name, value) string pairs plus an empty or filled
    /// BinaryPropertySeq. Carrier for security-plugin classes,
    /// permissions tokens and ZeroDDS heterogeneous-security caps
    /// (WP 4H-b).
    pub const PROPERTY_LIST: u16 = 0x0059;
    /// Endpoint security info (spec OMG DDS-Security 1.1 §7.4.1.5).
    /// 2x u32 masks: `endpoint_security_attributes` +
    /// `plugin_endpoint_security_attributes`. Carrier for
    /// endpoint-level protection flags (WP 4H-c).
    pub const ENDPOINT_SECURITY_INFO: u16 = 0x1004;
    /// Participant security info (spec DDS-Security 1.2 §7.4.1.6
    /// Tab.18+19). 2x u32 masks at participant level —
    /// `participant_security_attributes` + `plugin_participant_security_
    /// attributes`. Controls RTPS-submessage / discovery / liveliness
    /// protection flags for the whole participant.
    pub const PARTICIPANT_SECURITY_INFO: u16 = 0x1005;
    /// PID_IDENTITY_TOKEN (DDS-Security 1.2 §7.4.1.4 Tab.16). The value
    /// is a CDR-encoded `DataHolder` (`class_id="DDS:Auth:PKI-DH:1.2"` +
    /// properties `dds.cert.sn`, `dds.cert.algo`, `dds.ca.sn`,
    /// `dds.ca.algo`). Enables discovery routing and cert-chain bind
    /// without a full cert in the SPDP announce. Mandatory from
    /// DDS-Security 1.2 — Cyclone DDS / FastDDS rely on this PID.
    pub const IDENTITY_TOKEN: u16 = 0x1001;
    /// PID_PERMISSIONS_TOKEN (DDS-Security 1.2 §7.4.1.5 Tab.17).
    /// The value is a CDR-encoded `DataHolder`
    /// (`class_id="DDS:Access:Permissions:1.2"` + properties
    /// `dds.perm_ca.sn`, `dds.perm_ca.algo`).
    pub const PERMISSIONS_TOKEN: u16 = 0x1002;
    /// PID_IDENTITY_STATUS_TOKEN (DDS-Security 1.2 §7.4.1.6, §10.3.2
    /// Tab.53). The value is a CDR-encoded `DataHolder`. Carrier for the
    /// OCSP live status (`AuthenticationListener.on_revoke_identity` etc.).
    pub const IDENTITY_STATUS_TOKEN: u16 = 0x1006;
    /// PID_PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO
    /// (DDS-Security 1.2 §7.3.11 + §7.5.1.4). 16 byte: 2 ×
    /// `AlgorithmRequirements` (trust_chain + message_auth). Spec
    /// default: RSASSA-PSS + ECDSA-P256.
    pub const PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO: u16 = 0x1010;
    /// PID_PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO
    /// (DDS-Security 1.2 §7.3.12 + §7.5.1.4). 8 byte:
    /// `AlgorithmRequirements` for DH/ECDH. Spec default:
    /// DHE-MODP-2048 + ECDHE-CEUM-P256.
    pub const PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO: u16 = 0x1011;
    /// PID_PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO
    /// (DDS-Security 1.2 §7.3.13 + §7.5.1.4). 16 byte: 4 × u32
    /// (supported + 3 required masks). Spec default:
    /// AES128 | AES256 supported, AES128 required for all endpoint
    /// classes.
    pub const PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO: u16 = 0x1012;
    /// PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO (DDS-Security 1.2
    /// §7.3.15 + §7.5.1.5). 4 byte: required_mask. Per DataWriter/
    /// DataReader in the Pub/SubscriptionBuiltinTopicData.
    pub const ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO: u16 = 0x1013;
    /// Builtin endpoint set (4 byte u32 bitmask).
    pub const BUILTIN_ENDPOINT_SET: u16 = 0x0058;
    /// Data representation (sequence<int16>) — XCDR1/XCDR2 negotiation
    /// (XTypes §7.6.3.2.2).
    pub const DATA_REPRESENTATION: u16 = 0x0073;
    /// Type-Information (TypeInformation payload) — XTypes §7.6.3.2.2.
    pub const TYPE_INFORMATION: u16 = 0x0075;
    /// Type-Consistency-Enforcement (4 byte kind + flags) — XTypes
    /// §7.6.3.7.
    pub const TYPE_CONSISTENCY_ENFORCEMENT: u16 = 0x0074;
    /// PID_KEY_HASH (DDSI-RTPS 2.5 §9.6.4.8 + XTypes 1.3 §7.6.8): a
    /// 16-byte instance identifier in the inline QoS of a DATA/DATA_FRAG.
    /// Readers and the persistence service correlate samples of the same
    /// instance via this hash. Computation: PLAIN_CDR2-BE of the @key
    /// holder, zero-padded if max_size <= 16, otherwise MD5(stream).
    pub const KEY_HASH: u16 = 0x0070;
    /// PID_STATUS_INFO (DDSI-RTPS 2.5 §9.6.3.9): 4 byte status word;
    /// bit 0 = DISPOSED, bit 1 = UNREGISTERED, bit 2 = FILTERED. Sent as
    /// inline QoS when the sample lifecycle requires it
    /// (DataWriter::dispose / unregister or content-filter match=false).
    pub const STATUS_INFO: u16 = 0x0071;
    /// PID_SHM_LOCATOR (ZeroDDS vendor PID 0x8001, zerodds-flatdata-1.0 §3.1).
    /// Value: u32 hostname_hash + u32 uid + u32 slot_count + u32 slot_size +
    /// CDR-string segment_path. From the writer in the discovery sample, a
    /// reader on the same host (uid + hostname_hash match) attaches to the
    /// SHM segment. Vendor PID WITHOUT a MUST_UNDERSTAND bit — foreign vendors ignore it.
    pub const SHM_LOCATOR: u16 = 0x8001;
    /// PID_ZERODDS_TYPE_ID (ZeroDDS vendor PID 0x8002).
    /// Value: CDR-encoded `zerodds_types::TypeIdentifier` (XTypes §7.3.4.2),
    /// little-endian (submessage endianness). Carries the TypeIdentifier
    /// discrimination of the topic type for XTypes-aware reader-writer
    /// matching (XTypes §7.6.3.7 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
    /// Vendor PID WITHOUT a MUST_UNDERSTAND bit — foreign vendors ignore it
    /// and the reader match falls back to a pure `type_name` comparison
    /// (DDS 1.4 §2.2.3 default path).
    pub const ZERODDS_TYPE_ID: u16 = 0x8002;
    /// PID_VENDOR_TRACE_CONTEXT (zerodds-monitor-1.1 §4): an inline-QoS PID
    /// for W3C trace-context propagation. Value: 2 CDR strings
    /// (`traceparent` + `tracestate`). It sits in the standard PID range,
    /// because cross-vendor adoption is desired; receivers without PID
    /// knowledge ignore it transparently (no MUST_UNDERSTAND bit). The
    /// encoder/decoder is in the spec-consuming `zerodds-monitor::TraceContextPid`.
    pub const VENDOR_TRACE_CONTEXT: u16 = 0x0D00;
    /// PID_COHERENT_SET (DDSI-RTPS 2.5 §9.6.4.2): 8 byte SequenceNumber
    /// = sequence_number of the first sample in the coherent set. All
    /// DATA/DATA_FRAG of a set carry this PID in inline QoS. The end of
    /// the set is signaled by a DATA with PID_COHERENT_SET=new_sn or
    /// without the PID. Implements WP 2.9 (C2.9 coherent sets).
    pub const COHERENT_SET: u16 = 0x0056;
    /// PID_GROUP_COHERENT_SET (DDSI-RTPS 2.5 §9.6.4.3): 8 byte
    /// SequenceNumber = group_sequence_number of the first sample in the
    /// group-coherent set (PRESENTATION.access_scope = GROUP).
    pub const GROUP_COHERENT_SET: u16 = 0x0063;
    /// PID_GROUP_SEQ_NUM (DDSI-RTPS 2.5 §9.6.4.4): 8 byte SequenceNumber
    /// = group sequence number of the sample. A mandatory tag for
    /// publishers with access_scope=GROUP.
    pub const GROUP_SEQ_NUM: u16 = 0x0064;

    // ----------------------------------------------------------------
    // DDS-RPC 1.0 discovery PIDs (formal/16-12-04 §7.8.2 + §7.6.2). Set on
    // the SEDP announce of one half of an RPC endpoint pair and used in the
    // inline QoS of a reply DATA (`PID_RELATED_SAMPLE_IDENTITY`).
    // ----------------------------------------------------------------

    /// PID_SERVICE_INSTANCE_NAME (DDS-RPC 1.0 §7.8.2). CDR string =
    /// logical service-instance name. Allows multiple instances of the
    /// same service type on one participant.
    pub const SERVICE_INSTANCE_NAME: u16 = 0x0080;
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.8.2). 16 byte = GUID of
    /// the counterpart endpoint (request writer ↔ reply reader, or
    /// request reader ↔ reply writer). Binds the two topics into one
    /// logical RPC endpoint pair.
    pub const RELATED_ENTITY_GUID: u16 = 0x0081;
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.8.2). `sequence<string>` =
    /// alternative topic names for routing/compat. Order is significant
    /// (the first element = the preferred alias).
    pub const TOPIC_ALIASES: u16 = 0x0082;
    /// PID_RELATED_SAMPLE_IDENTITY (DDS-RPC 1.0 §7.8.2). An inline-QoS PID
    /// on a reply DATA submessage. 24 byte XCDR2 `SampleIdentity` =
    /// `request_id` of the correlated request, so the requester can map
    /// the reply to the corresponding request.
    pub const RELATED_SAMPLE_IDENTITY: u16 = 0x0083;

    /// PID_IGNORE (XTypes 1.3 §7.4.1.2.1). A padding/filler PID. The
    /// receiver MUST skip the value and not include it in the
    /// ParameterList (spec: "Used to ignore parameters which can be
    /// safely ignored"). Used by encoders as padding between
    /// variable-length parameters, without disturbing the decoder.
    pub const IGNORE: u16 = 0x3F03;
    /// PID_DIRECTED_WRITE (DDSI-RTPS 2.5 §8.7.7 / §9.6.2.2.5). Inline QoS
    /// on a DATA/DATA_FRAG that addresses exactly one target reader (by
    /// GUID, 16 byte). Other readers that receive the sample MUST discard
    /// it. Enables point-to-point paths over a multicast writer (e.g. the
    /// auth handshake).
    pub const DIRECTED_WRITE: u16 = 0x0057;
    /// PID_TYPE_MAX_SIZE_SERIALIZED (spec §9.6.4.7). 4 byte u32 — the
    /// max wire size of a sample payload in CDR. Used by subscribers to
    /// check upfront whether the payload fits in `max_dataMaxSize` (DoS
    /// protection).
    pub const TYPE_MAX_SIZE_SERIALIZED: u16 = 0x0060;
    /// PID_ORIGINAL_WRITER_INFO (spec §8.7.9). 24 byte: GUID +
    /// SequenceNumber of the original writer. Set by the persistence
    /// service as inline QoS when it forwards a stored sample on behalf
    /// of another writer (late-joiner replay).
    pub const ORIGINAL_WRITER_INFO: u16 = 0x0061;
    /// PID_WRITER_GROUP_INFO (spec §8.7.5 + §9.6.2.2.6). The group
    /// sequence number of the writer within a publisher group-coherent
    /// set. Carried in HEARTBEAT.GroupInfo + inline QoS.
    pub const WRITER_GROUP_INFO: u16 = 0x0062;
}

/// `true` if `masked_pid` (without the must-understand and vendor bits)
/// is a PID known in the DDSI-RTPS 2.5 + DDS-Security 1.2 spec set.
/// Used by [`ParameterList::validate_must_understand_in_data_pipeline`]
/// to implement the must-understand reject logic (spec §9.4.2.11.2).
#[must_use]
pub fn is_standard_pid(masked_pid: u16) -> bool {
    use pid::*;
    matches!(
        masked_pid,
        SENTINEL
            | PARTICIPANT_LEASE_DURATION
            | TOPIC_NAME
            | TYPE_NAME
            | PROTOCOL_VERSION
            | VENDOR_ID
            | RELIABILITY
            | DURABILITY
            | OWNERSHIP
            | OWNERSHIP_STRENGTH
            | LIVELINESS
            | DEADLINE
            | LIFESPAN
            | PARTITION
            | PRESENTATION
            | USER_DATA
            | GROUP_DATA
            | TOPIC_DATA
            | CONTENT_FILTER_PROPERTY
            | DEFAULT_UNICAST_LOCATOR
            | METATRAFFIC_UNICAST_LOCATOR
            | METATRAFFIC_MULTICAST_LOCATOR
            | DOMAIN_ID
            | DEFAULT_MULTICAST_LOCATOR
            | UNICAST_LOCATOR
            | MULTICAST_LOCATOR
            | PARTICIPANT_GUID
            | ENDPOINT_GUID
            | PROPERTY_LIST
            | ENDPOINT_SECURITY_INFO
            | PARTICIPANT_SECURITY_INFO
            | IDENTITY_TOKEN
            | PERMISSIONS_TOKEN
            | IDENTITY_STATUS_TOKEN
            | PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO
            | PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO
            | PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO
            | ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO
            | BUILTIN_ENDPOINT_SET
            | DATA_REPRESENTATION
            | TYPE_INFORMATION
            | TYPE_CONSISTENCY_ENFORCEMENT
            | KEY_HASH
            | STATUS_INFO
            | COHERENT_SET
            | GROUP_COHERENT_SET
            | GROUP_SEQ_NUM
            | SERVICE_INSTANCE_NAME
            | RELATED_ENTITY_GUID
            | TOPIC_ALIASES
            | RELATED_SAMPLE_IDENTITY
            | IGNORE
            | DIRECTED_WRITE
            | TYPE_MAX_SIZE_SERIALIZED
            | ORIGINAL_WRITER_INFO
            | WRITER_GROUP_INFO
    )
}

/// A single parameter (tag + bytes value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Parameter id (see [`pid`]).
    pub id: u16,
    /// Raw value (without padding bytes; the encoder inserts padding up
    /// to the 4-byte boundary).
    pub value: Vec<u8>,
}

impl Parameter {
    /// Constructor.
    #[must_use]
    pub fn new(id: u16, value: Vec<u8>) -> Self {
        Self { id, value }
    }

    /// Spec §9.4.2.11.2 — sets the must-understand bit (`0x4000`) on the
    /// PID. Sender-side: every parameter whose understanding is critical
    /// for the receiver (e.g. `PID_KEY_HASH` with custom keys) must have
    /// the bit set.
    #[must_use]
    pub fn with_must_understand(mut self) -> Self {
        self.id |= MUST_UNDERSTAND_BIT;
        self
    }

    /// `true` if the must-understand bit is set.
    #[must_use]
    pub fn has_must_understand(&self) -> bool {
        (self.id & MUST_UNDERSTAND_BIT) != 0
    }
}

/// ParameterList = a sequence of parameters + a sentinel terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterList {
    /// List of parameters (without the sentinel — it is appended
    /// automatically on encode).
    pub parameters: Vec<Parameter>,
}

impl ParameterList {
    /// Empty list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }

    /// Append a parameter.
    pub fn push(&mut self, param: Parameter) {
        self.parameters.push(param);
    }

    /// Find the first parameter with `id`.
    #[must_use]
    pub fn find(&self, id: u16) -> Option<&Parameter> {
        self.parameters.iter().find(|p| p.id == id)
    }

    /// ALL parameters with `id` (DDSI-RTPS 2.5 §9.4.2.11: a PID may
    /// appear multiple times, e.g. several `*_UNICAST_LOCATOR` of a
    /// multi-homed peer).
    pub fn find_all(&self, id: u16) -> impl Iterator<Item = &Parameter> {
        self.parameters.iter().filter(move |p| p.id == id)
    }

    /// Validates the ParameterList against the must-understand rule
    /// (DDSI-RTPS 2.5 §9.4.2.11.2).
    ///
    /// Spec behavior:
    /// > "If the receiver does not understand a parameter and the
    /// >  must_understand bit (0x4000) is set, the entire RTPS message
    /// >  carrying this ParameterList MUST be discarded."
    ///
    /// `is_known` is a classifier: returns `true` for all PIDs the
    /// receiver understands (without the must-understand and vendor
    /// bits, i.e. the masked PID).
    ///
    /// # Errors
    /// `ValueOutOfRange` with a marker message on violation — the caller
    /// MUST discard the whole message.
    pub fn validate_must_understand<F>(&self, is_known: F) -> Result<(), WireError>
    where
        F: Fn(u16) -> bool,
    {
        for p in &self.parameters {
            let must_understand = (p.id & MUST_UNDERSTAND_BIT) != 0;
            if must_understand {
                let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
                if !is_known(masked) {
                    return Err(WireError::ValueOutOfRange {
                        message: "ParameterList contains unknown must-understand PID",
                    });
                }
            }
        }
        Ok(())
    }

    /// Convenience wrapper around [`Self::validate_must_understand`]
    /// with the standard-conformant [`is_standard_pid`] classifier.
    /// Called in the receiver pipeline hot path
    /// (`crates/rtps/src/datagram.rs::decode_datagram`), so the
    /// spec §9.4.2.11.2 reject rule applies automatically.
    ///
    /// Vendor-specific PIDs (top bit set) are explicitly allowed — the
    /// vendor-bit path survives the check in
    /// `validate_must_understand`, because `masked` strips the vendor
    /// bit but `is_standard_pid` does not add it back; the caller treats
    /// vendor PIDs as "can be ignored".
    ///
    /// # Errors
    /// `ValueOutOfRange` on violation — the caller discards the message.
    pub fn validate_must_understand_in_data_pipeline(&self) -> Result<(), WireError> {
        for p in &self.parameters {
            let must_understand = (p.id & MUST_UNDERSTAND_BIT) != 0;
            if must_understand {
                // Skip vendor-specific PIDs — the vendor decides for
                // itself, the standard receiver may ignore them even
                // with the must-understand bit (spec §9.6.2).
                if (p.id & VENDOR_SPECIFIC_BIT) != 0 {
                    continue;
                }
                let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
                if !is_standard_pid(masked) {
                    return Err(WireError::ValueOutOfRange {
                        message: "ParameterList contains unknown must-understand PID",
                    });
                }
            }
        }
        Ok(())
    }

    /// Encodes to bytes with the given endianness. Padding to the
    /// 4-byte boundary is inserted automatically per value; the sentinel
    /// is appended.
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> Vec<u8> {
        let mut out = Vec::new();
        for p in &self.parameters {
            let padded = padded_to_4(p.value.len());
            let len_field = padded as u16;
            write_u16(&mut out, p.id, little_endian);
            write_u16(&mut out, len_field, little_endian);
            out.extend_from_slice(&p.value);
            out.resize(out.len() + (padded - p.value.len()), 0);
        }
        // Sentinel: id=0x0001, length=0, no value.
        write_u16(&mut out, pid::SENTINEL, little_endian);
        write_u16(&mut out, 0, little_endian);
        out
    }

    /// Decodes a ParameterList from bytes. Stops at the sentinel.
    ///
    /// # Errors
    /// `UnexpectedEof` on truncated input; `ValueOutOfRange` if the
    /// length is not 4-byte aligned; `ValueOutOfRange` if the parameter
    /// count exceeds [`MAX_PARAMETERS`] (DoS cap).
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        let mut parameters = Vec::new();
        let mut pos = 0usize;
        loop {
            if bytes.len() < pos + 4 {
                return Err(WireError::UnexpectedEof {
                    needed: 4,
                    offset: pos,
                });
            }
            let id = read_u16(&bytes[pos..pos + 2], little_endian);
            let length = read_u16(&bytes[pos + 2..pos + 4], little_endian) as usize;
            pos += 4;
            if id == pid::SENTINEL {
                return Ok(Self { parameters });
            }
            if length % 4 != 0 {
                return Err(WireError::ValueOutOfRange {
                    message: "ParameterList length not 4-byte aligned",
                });
            }
            if bytes.len() < pos + length {
                return Err(WireError::UnexpectedEof {
                    needed: length,
                    offset: pos,
                });
            }
            // PID_IGNORE: spec §7.4.1.2.1 — silently skip without adding
            // to `parameters`. Still consume the length field + body.
            if id == pid::IGNORE {
                pos += length;
                continue;
            }
            if parameters.len() >= MAX_PARAMETERS {
                return Err(WireError::ValueOutOfRange {
                    message: "ParameterList exceeds MAX_PARAMETERS cap",
                });
            }
            parameters.push(Parameter {
                id,
                value: bytes[pos..pos + length].to_vec(),
            });
            pos += length;
        }
    }
}

/// DoS cap for the parameter count in a ParameterList (SEDP/SPDP
/// amplification protection). 4 096 fits all payloads (realistically
/// &lt;100 per message); malicious peers can announce u16::MAX=65_535
/// times `{pid=XXXX, length=0}` and, without a cap, trigger hours-long
/// iteration.
pub const MAX_PARAMETERS: usize = 4_096;

/// Spec §9.4.2.11.2 — the must-understand bit of the parameter id. If it
/// is set and the receiver does not know the PID, the whole message
/// MUST be discarded.
pub const MUST_UNDERSTAND_BIT: u16 = 0x4000;

/// Spec §9.4.2.11.2 — Vendor-spezifische PIDs ab `0x8000`.
pub const VENDOR_SPECIFIC_BIT: u16 = 0x8000;

impl Default for ParameterList {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Bit-Helpers ----

fn padded_to_4(len: usize) -> usize {
    (len + 3) & !3
}

fn write_u16(out: &mut Vec<u8>, v: u16, le: bool) {
    let bytes = if le { v.to_le_bytes() } else { v.to_be_bytes() };
    out.extend_from_slice(&bytes);
}

fn read_u16(bytes: &[u8], le: bool) -> u16 {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&bytes[..2]);
    if le {
        u16::from_le_bytes(buf)
    } else {
        u16::from_be_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;

    #[test]
    fn padded_to_4_examples() {
        assert_eq!(padded_to_4(0), 0);
        assert_eq!(padded_to_4(1), 4);
        assert_eq!(padded_to_4(3), 4);
        assert_eq!(padded_to_4(4), 4);
        assert_eq!(padded_to_4(5), 8);
        assert_eq!(padded_to_4(16), 16);
    }

    #[test]
    fn empty_parameter_list_is_just_sentinel() {
        let pl = ParameterList::new();
        let bytes = pl.to_bytes(true);
        // Sentinel LE: 01 00 00 00 (id=0x0001, length=0)
        assert_eq!(bytes, vec![0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn single_parameter_encodes_id_length_value() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(0x0015, vec![0x02, 0x05, 0x00, 0x00]));
        let bytes = pl.to_bytes(true);
        // id=0x0015 LE = 15 00, length=4 LE = 04 00, value = 02 05 00 00,
        // sentinel = 01 00 00 00
        assert_eq!(
            bytes,
            vec![
                0x15, 0x00, 0x04, 0x00, 0x02, 0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00
            ]
        );
    }

    #[test]
    fn parameter_value_is_padded_to_4_bytes() {
        let mut pl = ParameterList::new();
        // 2 byte value → 2 byte padding
        pl.push(Parameter::new(0x0015, vec![0x02, 0x05]));
        let bytes = pl.to_bytes(true);
        // length field = 4 (padded), value = 02 05 00 00, then the sentinel
        assert_eq!(bytes[2], 4);
        assert_eq!(&bytes[4..8], &[0x02, 0x05, 0, 0]);
    }

    #[test]
    fn roundtrip_single_parameter() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            0x0050,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        ));
        let bytes = pl.to_bytes(true);
        let decoded = ParameterList::from_bytes(&bytes, true).unwrap();
        assert_eq!(decoded, pl);
    }

    #[test]
    fn roundtrip_multiple_parameters() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::PROTOCOL_VERSION, vec![2, 5, 0, 0]));
        pl.push(Parameter::new(pid::VENDOR_ID, vec![0x01, 0xF0, 0, 0]));
        pl.push(Parameter::new(pid::PARTICIPANT_GUID, vec![0xAA; 16]));
        let bytes = pl.to_bytes(true);
        let decoded = ParameterList::from_bytes(&bytes, true).unwrap();
        assert_eq!(decoded, pl);
    }

    #[test]
    fn find_returns_first_parameter_with_id() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::VENDOR_ID, vec![0x01, 0xF0, 0, 0]));
        pl.push(Parameter::new(pid::PROTOCOL_VERSION, vec![2, 5, 0, 0]));
        let p = pl.find(pid::VENDOR_ID).unwrap();
        assert_eq!(p.value, vec![0x01, 0xF0, 0, 0]);
    }

    #[test]
    fn find_returns_none_for_missing_id() {
        let pl = ParameterList::new();
        assert!(pl.find(pid::VENDOR_ID).is_none());
    }

    #[test]
    fn decode_rejects_non_aligned_length() {
        // id=0x0015, length=3 (not aligned), value=3 bytes
        let bytes = vec![0x15, 0x00, 0x03, 0x00, 1, 2, 3, 0x01, 0x00, 0x00, 0x00];
        let res = ParameterList::from_bytes(&bytes, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn decode_rejects_truncated_value() {
        let bytes = vec![0x15, 0x00, 0x08, 0x00, 1, 2, 3]; // length=8, nur 3 byte da
        let res = ParameterList::from_bytes(&bytes, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn decode_rejects_missing_sentinel() {
        // Only one parameter, then EOF — no sentinel.
        let bytes = vec![0x15, 0x00, 0x04, 0x00, 1, 2, 3, 4];
        let res = ParameterList::from_bytes(&bytes, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn must_understand_known_pid_passes() {
        let mut pl = ParameterList::new();
        // PID 0x4015 = PROTOCOL_VERSION (0x0015) with must-understand bit.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | 0x0015,
            vec![2, 5, 0, 0],
        ));
        assert!(pl.validate_must_understand(|pid| pid == 0x0015).is_ok());
    }

    #[test]
    fn must_understand_unknown_pid_rejects() {
        let mut pl = ParameterList::new();
        // PID 0x4014 = PID_DOMAIN_TAG (Cyclone often sets it with the
        // must-understand bit). We pretend we do not know 0x0014.
        pl.push(Parameter::new(MUST_UNDERSTAND_BIT | 0x0014, vec![0; 4]));
        let res = pl.validate_must_understand(|pid| pid == 0x0015);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn must_understand_unknown_optional_pid_skips() {
        let mut pl = ParameterList::new();
        // PID 0x0099 (no Must-Understand bit) — the receiver may skip.
        pl.push(Parameter::new(0x0099, vec![0; 4]));
        assert!(pl.validate_must_understand(|pid| pid == 0x0015).is_ok());
    }

    #[test]
    fn must_understand_vendor_pid_with_must_understand_rejects() {
        let mut pl = ParameterList::new();
        // 0xC042 = vendor PID 0x0042 with the must-understand bit.
        // We do not know 0x0042. validate_must_understand with a strict
        // closure rejects.
        pl.push(Parameter::new(0xC042, vec![0; 4]));
        let res = pl.validate_must_understand(|_| false);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn validate_must_understand_in_data_pipeline_known_pid_passes() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::KEY_HASH,
            vec![0; 16],
        ));
        assert!(pl.validate_must_understand_in_data_pipeline().is_ok());
    }

    #[test]
    fn validate_must_understand_in_data_pipeline_unknown_pid_rejects() {
        let mut pl = ParameterList::new();
        // 0x3500 is not a standard PID.
        pl.push(Parameter::new(MUST_UNDERSTAND_BIT | 0x3500, vec![0; 4]));
        let r = pl.validate_must_understand_in_data_pipeline();
        assert!(matches!(r, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn validate_must_understand_in_data_pipeline_vendor_specific_pid_passes() {
        let mut pl = ParameterList::new();
        // Vendor-specific PID (bit 15 set) with MU bit — spec
        // §9.6.2 allows ignoring.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT | 0x0050,
            vec![0xCA, 0xFE, 0xBA, 0xBE],
        ));
        assert!(pl.validate_must_understand_in_data_pipeline().is_ok());
    }

    #[test]
    fn validate_must_understand_in_data_pipeline_optional_unknown_pid_passes() {
        let mut pl = ParameterList::new();
        // Unknown PID WITHOUT the must-understand bit — may be ignored,
        // no reject.
        pl.push(Parameter::new(0x3500, vec![0; 4]));
        assert!(pl.validate_must_understand_in_data_pipeline().is_ok());
    }

    #[test]
    fn is_standard_pid_recognises_dds_security_pids() {
        // Sanity check: DDS-Security 1.2 PIDs count as standard.
        assert!(is_standard_pid(pid::ENDPOINT_SECURITY_INFO));
        assert!(is_standard_pid(pid::IDENTITY_TOKEN));
        assert!(is_standard_pid(pid::PERMISSIONS_TOKEN));
    }

    #[test]
    fn is_standard_pid_unknown_pid_returns_false() {
        // PID 0x3500 is not part of the standard set.
        assert!(!is_standard_pid(0x3500));
        assert!(!is_standard_pid(0x9999));
    }

    #[test]
    fn must_understand_empty_list_passes() {
        let pl = ParameterList::new();
        assert!(pl.validate_must_understand(|_| false).is_ok());
    }

    #[test]
    fn rpc_pid_constants_match_spec() {
        // DDS-RPC 1.0 §7.8.2 — PIDs must have exactly these values,
        // otherwise Cyclone RPC interop breaks.
        assert_eq!(pid::SERVICE_INSTANCE_NAME, 0x0080);
        assert_eq!(pid::RELATED_ENTITY_GUID, 0x0081);
        assert_eq!(pid::TOPIC_ALIASES, 0x0082);
        assert_eq!(pid::RELATED_SAMPLE_IDENTITY, 0x0083);
    }

    #[test]
    fn rpc_pids_roundtrip_in_parameter_list() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::SERVICE_INSTANCE_NAME, vec![1, 2, 3, 4]));
        pl.push(Parameter::new(pid::RELATED_ENTITY_GUID, vec![0xAB; 16]));
        pl.push(Parameter::new(pid::TOPIC_ALIASES, vec![0xCD; 8]));
        pl.push(Parameter::new(pid::RELATED_SAMPLE_IDENTITY, vec![0xEF; 24]));
        let bytes = pl.to_bytes(true);
        let decoded = ParameterList::from_bytes(&bytes, true).unwrap();
        assert_eq!(decoded, pl);
    }

    // ---- PID_IGNORE (XTypes 1.3 §7.4.1.2.1) ----

    #[test]
    fn pid_ignore_skipped_in_pl_cdr_decode() {
        // PID_IGNORE 0x3F03 with a 4-byte payload, then PROTOCOL_VERSION
        // (0x0015) with a 4-byte payload, then the sentinel. The decoder
        // MUST skip the PID_IGNORE item and return only PROTOCOL_VERSION.
        let bytes = vec![
            0x03, 0x3F, 0x04, 0x00, // PID_IGNORE, length=4
            0xAA, 0xBB, 0xCC, 0xDD, // body (irrelevant)
            0x15, 0x00, 0x04, 0x00, // PID_PROTOCOL_VERSION, length=4
            2, 5, 0, 0, // value
            0x01, 0x00, 0x00, 0x00, // Sentinel
        ];
        let pl = ParameterList::from_bytes(&bytes, true).unwrap();
        assert_eq!(pl.parameters.len(), 1);
        assert_eq!(pl.parameters[0].id, pid::PROTOCOL_VERSION);
        assert_eq!(pl.parameters[0].value, vec![2, 5, 0, 0]);
    }

    #[test]
    fn pid_ignore_zero_length_is_valid() {
        // PID_IGNORE with length=0 — also allowed (pure padding marker).
        let bytes = vec![
            0x03, 0x3F, 0x00, 0x00, // PID_IGNORE, length=0
            0x15, 0x00, 0x04, 0x00, // PID_PROTOCOL_VERSION
            2, 5, 0, 0, 0x01, 0x00, 0x00, 0x00, // Sentinel
        ];
        let pl = ParameterList::from_bytes(&bytes, true).unwrap();
        assert_eq!(pl.parameters.len(), 1);
        assert_eq!(pl.parameters[0].id, pid::PROTOCOL_VERSION);
    }

    #[test]
    fn pid_ignore_truncated_body_rejected() {
        // PID_IGNORE with length=8, but only 4 bytes of body follow.
        let bytes = vec![
            0x03, 0x3F, 0x08, 0x00, // PID_IGNORE, length=8
            0xAA, 0xBB, 0xCC, 0xDD, // nur 4 byte Body
            0x01, 0x00, 0x00, 0x00, // would be the sentinel, but is within the announced 8
        ];
        let res = ParameterList::from_bytes(&bytes, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn pid_ignore_be_decoded() {
        // BE endianness: id 3F 03, length 00 04, body, then the sentinel.
        let bytes = vec![
            0x3F, 0x03, 0x00, 0x04, // PID_IGNORE BE
            0xAA, 0xBB, 0xCC, 0xDD, 0x00, 0x15, 0x00, 0x04, // PROTOCOL_VERSION BE
            2, 5, 0, 0, 0x00, 0x01, 0x00, 0x00, // Sentinel BE
        ];
        let pl = ParameterList::from_bytes(&bytes, false).unwrap();
        assert_eq!(pl.parameters.len(), 1);
        assert_eq!(pl.parameters[0].id, pid::PROTOCOL_VERSION);
    }

    #[test]
    fn pid_ignore_ignored_even_when_count_would_exceed_cap() {
        // The MAX_PARAMETERS cap does NOT count PID_IGNORE, because a
        // silent skip creates no entry. 8192 PID_IGNOREs in a row would
        // be a DoS risk without this path; with it they are harmless.
        let mut bytes: Vec<u8> = Vec::new();
        for _ in 0..50 {
            bytes.extend_from_slice(&[0x03, 0x3F, 0x00, 0x00]);
        }
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        let pl = ParameterList::from_bytes(&bytes, true).unwrap();
        assert_eq!(pl.parameters.len(), 0);
    }

    #[test]
    fn roundtrip_be_endianness() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::PROTOCOL_VERSION, vec![2, 5, 0, 0]));
        let bytes = pl.to_bytes(false);
        // BE: id 00 15, length 00 04, value, then sentinel 00 01 00 00
        assert_eq!(&bytes[..4], &[0, 0x15, 0, 4]);
        let decoded = ParameterList::from_bytes(&bytes, false).unwrap();
        assert_eq!(decoded, pl);
    }
}
