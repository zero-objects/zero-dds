// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ParameterList (DDSI-RTPS 2.5 §9.4.2.11).
//!
//! Tag-Length-Value-Format fuer SPDP/SEDP Builtin-Topic-Daten. Jeder
//! Parameter:
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
//! kein Value.
//!
//! Encoding ist immer mit der Submessage-Endianness; dieses Modul
//! arbeitet auf rohen Bytes mit explizitem `little_endian`-Parameter.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;

/// Standard-Parameter-IDs (Spec §9.6.4 + Tabelle 9.13).
///
/// Die 12 QoS-Policy-PIDs aus DDS 1.4 §2.2.3 werden aus
/// [`zerodds_qos::Pid`] re-exportiert (Single-Source-of-Truth).
/// rtps-spezifische PIDs (Locators, GUIDs, Security-Tokens, …) bleiben
/// hier deklariert, da sie ausserhalb des QoS-Policy-Subsets sind.
pub mod pid {
    use zerodds_qos::Pid as QosPid;

    // ---- Re-exports aus zerodds_qos::Pid (12 Policy-PIDs) ----
    /// Sentinel — Terminator der ParameterList. (Re-export aus `zerodds_qos::Pid::SENTINEL`.)
    pub const SENTINEL: u16 = QosPid::SENTINEL;
    /// Reliability QoS. (Re-export aus `zerodds_qos::Pid::RELIABILITY`.)
    pub const RELIABILITY: u16 = QosPid::RELIABILITY;
    /// Durability QoS. (Re-export aus `zerodds_qos::Pid::DURABILITY`.)
    pub const DURABILITY: u16 = QosPid::DURABILITY;
    /// Ownership QoS. (Re-export aus `zerodds_qos::Pid::OWNERSHIP`.)
    pub const OWNERSHIP: u16 = QosPid::OWNERSHIP;
    /// Ownership-Strength QoS. (Re-export aus `zerodds_qos::Pid::OWNERSHIP_STRENGTH`.)
    pub const OWNERSHIP_STRENGTH: u16 = QosPid::OWNERSHIP_STRENGTH;
    /// Liveliness QoS. (Re-export aus `zerodds_qos::Pid::LIVELINESS`.)
    pub const LIVELINESS: u16 = QosPid::LIVELINESS;
    /// Deadline QoS. (Re-export aus `zerodds_qos::Pid::DEADLINE`.)
    pub const DEADLINE: u16 = QosPid::DEADLINE;
    /// Lifespan QoS. (Re-export aus `zerodds_qos::Pid::LIFESPAN`.)
    pub const LIFESPAN: u16 = QosPid::LIFESPAN;
    /// Partition QoS. (Re-export aus `zerodds_qos::Pid::PARTITION`.)
    pub const PARTITION: u16 = QosPid::PARTITION;
    /// UserData QoS. (Re-export aus `zerodds_qos::Pid::USER_DATA`.)
    pub const USER_DATA: u16 = QosPid::USER_DATA;
    /// GroupData QoS. (Re-export aus `zerodds_qos::Pid::GROUP_DATA`.)
    pub const GROUP_DATA: u16 = QosPid::GROUP_DATA;
    /// TopicData QoS. (Re-export aus `zerodds_qos::Pid::TOPIC_DATA`.)
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
    /// Default-Unicast-Locator (24 byte) — fuer User-Daten.
    pub const DEFAULT_UNICAST_LOCATOR: u16 = 0x0031;
    /// Metatraffic-Unicast-Locator (24 byte) — wohin Peers SEDP-Unicast
    /// schicken. Unverzichtbar fuer Cyclone-Interop.
    pub const METATRAFFIC_UNICAST_LOCATOR: u16 = 0x0032;
    /// Metatraffic-Multicast-Locator (24 byte) — wohin Peers SPDP/SEDP
    /// Multicast schicken.
    pub const METATRAFFIC_MULTICAST_LOCATOR: u16 = 0x0033;
    /// Domain-Id (4 byte u32) — Participant-Domain.
    pub const DOMAIN_ID: u16 = 0x000f;
    /// Default-Multicast-Locator (24 byte).
    pub const DEFAULT_MULTICAST_LOCATOR: u16 = 0x0048;
    /// Participant-Guid (16 byte).
    pub const PARTICIPANT_GUID: u16 = 0x0050;
    /// Endpoint-Guid (16 byte) — fuer Publication/Subscription-Discovery.
    pub const ENDPOINT_GUID: u16 = 0x005a;
    /// Property-List (Spec OMG DDS-Security 1.1 §7.2.1). Sequence von
    /// (name, value)-String-Paaren plus einer leeren oder gefuellten
    /// BinaryPropertySeq. Traeger fuer Security-Plugin-Klassen,
    /// Permissions-Tokens und ZeroDDS-Heterogeneous-Security-Caps
    /// (WP 4H-b).
    pub const PROPERTY_LIST: u16 = 0x0059;
    /// Endpoint-Security-Info (Spec OMG DDS-Security 1.1 §7.4.1.5).
    /// 2x u32 Masken: `endpoint_security_attributes` +
    /// `plugin_endpoint_security_attributes`. Traeger fuer Endpoint-
    /// Level-Protection-Flags (WP 4H-c).
    pub const ENDPOINT_SECURITY_INFO: u16 = 0x1004;
    /// Participant-Security-Info (Spec DDS-Security 1.2 §7.4.1.6
    /// Tab.18+19). 2x u32 Masken auf Participant-Level —
    /// `participant_security_attributes` + `plugin_participant_security_
    /// attributes`. Steuert RTPS-Submessage / Discovery / Liveliness-
    /// Protection-Flags fuer den ganzen Participant.
    pub const PARTICIPANT_SECURITY_INFO: u16 = 0x1005;
    /// PID_IDENTITY_TOKEN (DDS-Security 1.2 §7.4.1.4 Tab.16). Wert ist
    /// ein CDR-encoded `DataHolder` (`class_id="DDS:Auth:PKI-DH:1.2"` +
    /// Properties `dds.cert.sn`, `dds.cert.algo`, `dds.ca.sn`,
    /// `dds.ca.algo`). Erlaubt Discovery-Routing und Cert-Chain-
    /// Bind ohne Voll-Cert im SPDP-Announce. Pflicht ab DDS-Security
    /// 1.2 — Cyclone DDS / FastDDS verlassen sich auf diesen PID.
    pub const IDENTITY_TOKEN: u16 = 0x1001;
    /// PID_PERMISSIONS_TOKEN (DDS-Security 1.2 §7.4.1.5 Tab.17).
    /// Wert ist CDR-encoded `DataHolder`
    /// (`class_id="DDS:Access:Permissions:1.2"` + Properties
    /// `dds.perm_ca.sn`, `dds.perm_ca.algo`).
    pub const PERMISSIONS_TOKEN: u16 = 0x1002;
    /// PID_IDENTITY_STATUS_TOKEN (DDS-Security 1.2 §7.4.1.6, §10.3.2
    /// Tab.53). Wert ist CDR-encoded `DataHolder`. Traeger fuer OCSP-
    /// Live-Status (`AuthenticationListener.on_revoke_identity` etc.).
    pub const IDENTITY_STATUS_TOKEN: u16 = 0x1006;
    /// PID_PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO
    /// (DDS-Security 1.2 §7.3.11 + §7.5.1.4). 16 byte: 2 ×
    /// `AlgorithmRequirements` (trust_chain + message_auth). Spec-
    /// Default: RSASSA-PSS + ECDSA-P256.
    pub const PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO: u16 = 0x1010;
    /// PID_PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO
    /// (DDS-Security 1.2 §7.3.12 + §7.5.1.4). 8 byte:
    /// `AlgorithmRequirements` fuer DH/ECDH. Spec-Default:
    /// DHE-MODP-2048 + ECDHE-CEUM-P256.
    pub const PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO: u16 = 0x1011;
    /// PID_PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO
    /// (DDS-Security 1.2 §7.3.13 + §7.5.1.4). 16 byte: 4 × u32
    /// (supported + 3 required-Masks). Spec-Default:
    /// AES128 | AES256 supported, AES128 fuer alle Endpoint-Klassen
    /// required.
    pub const PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO: u16 = 0x1012;
    /// PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO (DDS-Security 1.2
    /// §7.3.15 + §7.5.1.5). 4 byte: required_mask. Pro DataWriter/
    /// DataReader im Pub/SubscriptionBuiltinTopicData.
    pub const ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO: u16 = 0x1013;
    /// Builtin-Endpoint-Set (4 byte u32 Bitmask).
    pub const BUILTIN_ENDPOINT_SET: u16 = 0x0058;
    /// Data-Representation (sequence<int16>) — XCDR1/XCDR2-Negotiation
    /// (XTypes §7.6.3.2.2).
    pub const DATA_REPRESENTATION: u16 = 0x0073;
    /// Type-Information (TypeInformation payload) — XTypes §7.6.3.2.2.
    pub const TYPE_INFORMATION: u16 = 0x0075;
    /// Type-Consistency-Enforcement (4 byte kind + flags) — XTypes
    /// §7.6.3.7.
    pub const TYPE_CONSISTENCY_ENFORCEMENT: u16 = 0x0074;
    /// PID_KEY_HASH (DDSI-RTPS 2.5 §9.6.4.8 + XTypes 1.3 §7.6.8): 16-Byte
    /// Instance-Identifier in Inline-QoS einer DATA/DATA_FRAG. Reader und
    /// Persistence-Service korrelieren Samples derselben Instanz ueber
    /// diesen Hash. Berechnung: PLAIN_CDR2-BE des @key-Holders, bei
    /// max_size <= 16 zero-padded, sonst MD5(stream).
    pub const KEY_HASH: u16 = 0x0070;
    /// PID_STATUS_INFO (DDSI-RTPS 2.5 §9.6.3.9): 4 byte Statusword;
    /// Bit 0 = DISPOSED, Bit 1 = UNREGISTERED, Bit 2 = FILTERED. Wird
    /// als Inline-QoS gesendet, wenn der Sample-Lifecycle das verlangt
    /// (DataWriter::dispose / unregister oder ContentFilter-Match=False).
    pub const STATUS_INFO: u16 = 0x0071;
    /// PID_SHM_LOCATOR (ZeroDDS Vendor-PID 0x8001, zerodds-flatdata-1.0 §3.1).
    /// Wert: u32 hostname_hash + u32 uid + u32 slot_count + u32 slot_size +
    /// CDR-String segment_path. Vom Writer im Discovery-Sample, Reader auf
    /// demselben Host (uid + hostname_hash match) attached an SHM-Segment.
    /// Vendor-PID OHNE MUST_UNDERSTAND-Bit — fremde Vendoren ignorieren.
    pub const SHM_LOCATOR: u16 = 0x8001;
    /// PID_ZERODDS_TYPE_ID (ZeroDDS Vendor-PID 0x8002).
    /// Wert: CDR-encoded `zerodds_types::TypeIdentifier` (XTypes §7.3.4.2),
    /// Little-Endian (Submessage-Endianness). Trägt die TypeIdentifier-
    /// Diskrimination der Topic-Type fuer XTypes-aware Reader-Writer-
    /// Matching (XTypes §7.6.3.7 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
    /// Vendor-PID OHNE MUST_UNDERSTAND-Bit — fremde Vendoren ignorieren
    /// und der Reader-Match faellt zurueck auf reinen `type_name`-Vergleich
    /// (DDS 1.4 §2.2.3 Default-Path).
    pub const ZERODDS_TYPE_ID: u16 = 0x8002;
    /// PID_VENDOR_TRACE_CONTEXT (zerodds-monitor-1.0 §4): Inline-QoS-PID
    /// fuer W3C-Trace-Context-Propagation. Wert: 2 CDR-Strings
    /// (`traceparent` + `tracestate`). Steht im Standard-PID-Range, weil
    /// Cross-Vendor-Adoption gewuenscht ist; Empfaenger ohne PID-Kenntnis
    /// ignorieren transparent (kein MUST_UNDERSTAND-Bit). Encoder/Decoder
    /// im Spec-konsumierenden `zerodds-monitor::TraceContextPid`.
    pub const VENDOR_TRACE_CONTEXT: u16 = 0x0D00;
    /// PID_COHERENT_SET (DDSI-RTPS 2.5 §9.6.4.2): 8 byte SequenceNumber
    /// = sequence_number des ersten Sample im Coherent-Set. Alle DATA/
    /// DATA_FRAG eines Sets tragen diesen PID in Inline-QoS. Set-Ende
    /// signalisiert eine DATA mit PID_COHERENT_SET=neue_sn oder ohne PID.
    /// Implementiert WP 2.9 (C2.9 Coherent-Sets).
    pub const COHERENT_SET: u16 = 0x0056;
    /// PID_GROUP_COHERENT_SET (DDSI-RTPS 2.5 §9.6.4.3): 8 byte
    /// SequenceNumber = group_sequence_number des ersten Sample im
    /// Group-Coherent-Set (PRESENTATION.access_scope = GROUP).
    pub const GROUP_COHERENT_SET: u16 = 0x0063;
    /// PID_GROUP_SEQ_NUM (DDSI-RTPS 2.5 §9.6.4.4): 8 byte SequenceNumber
    /// = group sequence number des Sample. Pflicht-Tag fuer Publisher
    /// mit access_scope=GROUP.
    pub const GROUP_SEQ_NUM: u16 = 0x0064;

    // ----------------------------------------------------------------
    // DDS-RPC 1.0 Discovery-PIDs (formal/16-12-04 §7.8.2 + §7.6.2). Werden
    // beim SEDP-Announce einer RPC-Endpoint-Pair-Haelfte gesetzt und in der
    // Inline-QoS einer Reply-DATA (`PID_RELATED_SAMPLE_IDENTITY`) genutzt.
    // ----------------------------------------------------------------

    /// PID_SERVICE_INSTANCE_NAME (DDS-RPC 1.0 §7.8.2). CDR-String =
    /// logischer Service-Instance-Name. Erlaubt mehrere Instanzen
    /// desselben Service-Typs auf einem Participant.
    pub const SERVICE_INSTANCE_NAME: u16 = 0x0080;
    /// PID_RELATED_ENTITY_GUID (DDS-RPC 1.0 §7.8.2). 16 byte = GUID des
    /// Pendant-Endpoints (Request-Writer ↔ Reply-Reader bzw.
    /// Request-Reader ↔ Reply-Writer). Bindet die zwei Topics zu einem
    /// logischen RPC-Endpoint-Pair.
    pub const RELATED_ENTITY_GUID: u16 = 0x0081;
    /// PID_TOPIC_ALIASES (DDS-RPC 1.0 §7.8.2). `sequence<string>` =
    /// alternative Topic-Namen fuer Routing/Compat. Reihenfolge ist
    /// signifikant (erstes Element = bevorzugter Alias).
    pub const TOPIC_ALIASES: u16 = 0x0082;
    /// PID_RELATED_SAMPLE_IDENTITY (DDS-RPC 1.0 §7.8.2). Inline-QoS-PID
    /// auf einer Reply-DATA-Submessage. 24 byte XCDR2-`SampleIdentity` =
    /// `request_id` der korrelierten Request, damit der Requester den
    /// Reply der zugehoerigen Anfrage zuordnen kann.
    pub const RELATED_SAMPLE_IDENTITY: u16 = 0x0083;

    /// PID_IGNORE (XTypes 1.3 §7.4.1.2.1). Padding-/Filler-PID. Receiver
    /// MUSS Wert ueberlesen und nicht in die ParameterList aufnehmen
    /// (Spec: "Used to ignore parameters which can be safely ignored").
    /// Wird von Encodern als Padding zwischen variablen-laengen Parametern
    /// verwendet, ohne den Decoder zu stoeren.
    pub const IGNORE: u16 = 0x3F03;
    /// PID_DIRECTED_WRITE (DDSI-RTPS 2.5 §8.7.7 / §9.6.2.2.5). Inline-QoS
    /// auf einer DATA/DATA_FRAG, die exakt einen Ziel-Reader (per GUID,
    /// 16 byte) adressiert. Andere Reader, die das Sample empfangen,
    /// MUESSEN es verwerfen. Ermoeglicht Punkt-zu-Punkt-Pfade ueber
    /// einen Multicast-Writer (z.B. Auth-Handshake).
    pub const DIRECTED_WRITE: u16 = 0x0057;
    /// PID_TYPE_MAX_SIZE_SERIALIZED (Spec §9.6.4.7). 4 byte u32 — die
    /// max-Wire-Groesse eines Samples-Payloads in CDR. Wird fuer
    /// Subscriber genutzt, um vorab zu pruefen, ob die Payload in
    /// `max_dataMaxSize` paesst (DoS-Schutz).
    pub const TYPE_MAX_SIZE_SERIALIZED: u16 = 0x0060;
    /// PID_ORIGINAL_WRITER_INFO (Spec §8.7.9). 24 byte: GUID +
    /// SequenceNumber des urspruenglichen Writers. Wird vom
    /// Persistence-Service als Inline-QoS gesetzt, wenn er ein
    /// gespeichertes Sample im Auftrag eines anderen Writers
    /// weiterleitet (Late-Joiner-Replay).
    pub const ORIGINAL_WRITER_INFO: u16 = 0x0061;
    /// PID_WRITER_GROUP_INFO (Spec §8.7.5 + §9.6.2.2.6). Group-Sequence-
    /// Nummer des Writers innerhalb eines Publisher-Group-Coherent-
    /// Sets. Wird in HEARTBEAT.GroupInfo + InlineQoS getragen.
    pub const WRITER_GROUP_INFO: u16 = 0x0062;
}

/// `true` wenn `masked_pid` (ohne Must-Understand- und Vendor-Bits)
/// ein im DDSI-RTPS 2.5 + DDS-Security 1.2 Spec-Set bekannter PID ist.
/// Wird von [`ParameterList::validate_must_understand_in_data_pipeline`]
/// genutzt, um Must-Understand-Reject-Logik (Spec §9.4.2.11.2) zu
/// implementieren.
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
            | USER_DATA
            | GROUP_DATA
            | TOPIC_DATA
            | CONTENT_FILTER_PROPERTY
            | DEFAULT_UNICAST_LOCATOR
            | METATRAFFIC_UNICAST_LOCATOR
            | METATRAFFIC_MULTICAST_LOCATOR
            | DOMAIN_ID
            | DEFAULT_MULTICAST_LOCATOR
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

/// Ein einzelner Parameter (Tag + Bytes-Wert).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Parameter-ID (siehe [`pid`]).
    pub id: u16,
    /// Roher Value (ohne Padding-Bytes; Encoder fuegt Padding bis
    /// zur 4-Byte-Boundary ein).
    pub value: Vec<u8>,
}

impl Parameter {
    /// Konstruktor.
    #[must_use]
    pub fn new(id: u16, value: Vec<u8>) -> Self {
        Self { id, value }
    }

    /// Spec §9.4.2.11.2 — setzt das Must-Understand-Bit (`0x4000`)
    /// auf der PID. Sender-Side: jeder Parameter, dessen Verstaendnis
    /// fuer den Receiver kritisch ist (z.B. `PID_KEY_HASH` bei
    /// custom keys), muss das Bit gesetzt haben.
    #[must_use]
    pub fn with_must_understand(mut self) -> Self {
        self.id |= MUST_UNDERSTAND_BIT;
        self
    }

    /// `true` wenn das Must-Understand-Bit gesetzt ist.
    #[must_use]
    pub fn has_must_understand(&self) -> bool {
        (self.id & MUST_UNDERSTAND_BIT) != 0
    }
}

/// ParameterList = Sequenz von Parametern + Sentinel-Terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterList {
    /// Liste der Parameter (ohne Sentinel — der wird automatisch
    /// beim Encode angefuegt).
    pub parameters: Vec<Parameter>,
}

impl ParameterList {
    /// Leere Liste.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }

    /// Parameter anhaengen.
    pub fn push(&mut self, param: Parameter) {
        self.parameters.push(param);
    }

    /// Erste Parameter mit `id` finden.
    #[must_use]
    pub fn find(&self, id: u16) -> Option<&Parameter> {
        self.parameters.iter().find(|p| p.id == id)
    }

    /// Validiert die ParameterList gegen die Must-Understand-Regel
    /// (DDSI-RTPS 2.5 §9.4.2.11.2).
    ///
    /// Spec-Verhalten:
    /// > "If the receiver does not understand a parameter and the
    /// >  must_understand bit (0x4000) is set, the entire RTPS message
    /// >  carrying this ParameterList MUST be discarded."
    ///
    /// `is_known` ist ein Klassifikator: liefert `true` fuer alle PIDs,
    /// die der Receiver versteht (ohne Must-Understand- und Vendor-
    /// Bits, also masked PID).
    ///
    /// # Errors
    /// `ValueOutOfRange` mit Marker-Message bei Verletzung — der
    /// Caller MUSS die Ganz-Message verwerfen.
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

    /// Convenience-Wrapper um [`Self::validate_must_understand`] mit
    /// dem standard-konformen [`is_standard_pid`]-Klassifikator.
    /// Wird im Receiver-Pipeline-Hot-Path gerufen
    /// (`crates/rtps/src/datagram.rs::decode_datagram`), sodass die
    /// Spec-§9.4.2.11.2-Reject-Regel automatisch greift.
    ///
    /// Vendor-Specific-PIDs (oberstes Bit gesetzt) sind explizit
    /// erlaubt — der Vendor-Bit-Pfad ueberlebt die Pruefung in
    /// `validate_must_understand`, weil das `masked` den Vendor-Bit
    /// strippt aber `is_standard_pid` ihn nicht zurueckaddiert; der
    /// Caller behandelt Vendor-PIDs als "kann ignoriert werden".
    ///
    /// # Errors
    /// `ValueOutOfRange` bei Verletzung — Caller verwirft die
    /// Message.
    pub fn validate_must_understand_in_data_pipeline(&self) -> Result<(), WireError> {
        for p in &self.parameters {
            let must_understand = (p.id & MUST_UNDERSTAND_BIT) != 0;
            if must_understand {
                // Vendor-Specific-PIDs ueberspringen — Vendor entscheidet
                // selbst, der Standard-Receiver darf sie auch mit
                // Must-Understand-Bit ignorieren (Spec §9.6.2).
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

    /// Encoded zu Bytes mit der gegebenen Endianness. Padding zu
    /// 4-Byte-Boundary wird pro Value automatisch eingefuegt; der
    /// Sentinel wird angefuegt.
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

    /// Decoded eine ParameterList aus Bytes. Stoppt am Sentinel.
    ///
    /// # Errors
    /// `UnexpectedEof` bei truncated Eingabe; `ValueOutOfRange` wenn
    /// Length nicht 4-Byte-aligned ist; `ValueOutOfRange` wenn die
    /// Parameter-Zahl [`MAX_PARAMETERS`] uebersteigt (DoS-Cap).
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
            // PID_IGNORE: Spec §7.4.1.2.1 — silently skip ohne Aufnahme
            // in `parameters`. Length-Feld + Body trotzdem konsumieren.
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

/// DoS-Cap fuer Parameter-Anzahl in einer ParameterList (SEDP/SPDP-
/// Amplification-Schutz). 4 096 passt für alle Payloads
/// (realistisch &lt;100 pro Message); boese Peers koennen u16::MAX=65_535
/// Mal `{pid=XXXX, length=0}` ankuendigen und ohne Cap Stunden-lange
/// Iteration auslosen.
pub const MAX_PARAMETERS: usize = 4_096;

/// Spec §9.4.2.11.2 — Must-Understand-Bit der Parameter-ID. Ist es
/// gesetzt und kennt der Receiver den PID nicht, MUSS die ganze
/// Message verworfen werden.
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
        // length-Feld = 4 (padded), value = 02 05 00 00, dann sentinel
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
        // id=0x0015, length=3 (nicht aligned), value=3 bytes
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
        // Nur ein Parameter, dann EOF — kein Sentinel.
        let bytes = vec![0x15, 0x00, 0x04, 0x00, 1, 2, 3, 4];
        let res = ParameterList::from_bytes(&bytes, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn must_understand_known_pid_passes() {
        let mut pl = ParameterList::new();
        // PID 0x4015 = PROTOCOL_VERSION (0x0015) mit Must-Understand-Bit.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | 0x0015,
            vec![2, 5, 0, 0],
        ));
        assert!(pl.validate_must_understand(|pid| pid == 0x0015).is_ok());
    }

    #[test]
    fn must_understand_unknown_pid_rejects() {
        let mut pl = ParameterList::new();
        // PID 0x4014 = PID_DOMAIN_TAG (Cyclone setzt das oft mit
        // Must-Understand-Bit). Wir tun so, als kennten wir 0x0014 nicht.
        pl.push(Parameter::new(MUST_UNDERSTAND_BIT | 0x0014, vec![0; 4]));
        let res = pl.validate_must_understand(|pid| pid == 0x0015);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn must_understand_unknown_optional_pid_skips() {
        let mut pl = ParameterList::new();
        // PID 0x0099 (kein Must-Understand-Bit) — Receiver darf skippen.
        pl.push(Parameter::new(0x0099, vec![0; 4]));
        assert!(pl.validate_must_understand(|pid| pid == 0x0015).is_ok());
    }

    #[test]
    fn must_understand_vendor_pid_with_must_understand_rejects() {
        let mut pl = ParameterList::new();
        // 0xC042 = vendor-PID 0x0042 mit Must-Understand-Bit.
        // Wir kennen 0x0042 nicht. validate_must_understand mit
        // strikter Closure rejected.
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
        // 0x3500 ist kein Standard-PID.
        pl.push(Parameter::new(MUST_UNDERSTAND_BIT | 0x3500, vec![0; 4]));
        let r = pl.validate_must_understand_in_data_pipeline();
        assert!(matches!(r, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn validate_must_understand_in_data_pipeline_vendor_specific_pid_passes() {
        let mut pl = ParameterList::new();
        // Vendor-spezifischer PID (Bit 15 gesetzt) mit MU-Bit — Spec
        // §9.6.2 erlaubt ignorieren.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT | 0x0050,
            vec![0xCA, 0xFE, 0xBA, 0xBE],
        ));
        assert!(pl.validate_must_understand_in_data_pipeline().is_ok());
    }

    #[test]
    fn validate_must_understand_in_data_pipeline_optional_unknown_pid_passes() {
        let mut pl = ParameterList::new();
        // Unbekannter PID OHNE Must-Understand-Bit — darf ignoriert
        // werden, kein Reject.
        pl.push(Parameter::new(0x3500, vec![0; 4]));
        assert!(pl.validate_must_understand_in_data_pipeline().is_ok());
    }

    #[test]
    fn is_standard_pid_recognises_dds_security_pids() {
        // Sanity-Check: DDS-Security 1.2 PIDs zaehlen als Standard.
        assert!(is_standard_pid(pid::ENDPOINT_SECURITY_INFO));
        assert!(is_standard_pid(pid::IDENTITY_TOKEN));
        assert!(is_standard_pid(pid::PERMISSIONS_TOKEN));
    }

    #[test]
    fn is_standard_pid_unknown_pid_returns_false() {
        // PID 0x3500 ist nicht Teil des Standard-Sets.
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
        // DDS-RPC 1.0 §7.8.2 — PIDs muessen exakt diese Werte haben,
        // sonst bricht Cyclone-RPC-Interop.
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
        // PID_IGNORE 0x3F03 mit 4-byte payload, dann PROTOCOL_VERSION
        // (0x0015) mit 4-byte payload, dann Sentinel. Decoder MUSS das
        // PID_IGNORE-Item ueberlesen und nur PROTOCOL_VERSION zurueckgeben.
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
        // PID_IGNORE mit length=0 — auch zulaessig (reines Padding-Marker).
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
        // PID_IGNORE mit length=8, aber nur 4 byte Body folgen.
        let bytes = vec![
            0x03, 0x3F, 0x08, 0x00, // PID_IGNORE, length=8
            0xAA, 0xBB, 0xCC, 0xDD, // nur 4 byte Body
            0x01, 0x00, 0x00,
            0x00, // wuerde Sentinel sein, ist aber innerhalb der angekuendigten 8
        ];
        let res = ParameterList::from_bytes(&bytes, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn pid_ignore_be_decoded() {
        // BE-Endianness: id 3F 03, length 00 04, body, dann Sentinel.
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
        // MAX_PARAMETERS-Cap zaehlt PID_IGNORE NICHT mit, weil silent-skip
        // keinen Eintrag erzeugt. 8192 PID_IGNOREs am Stueck waeren ohne
        // diesen Pfad ein DoS-Risiko, mit dem Pfad sind sie harmlos.
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
