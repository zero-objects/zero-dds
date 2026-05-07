// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Simple Participant Discovery Protocol (DDSI-RTPS 2.5 §8.5.3).
//!
//! Best-Effort-Beacon. Beacon-Sender baut ein DATA-Datagram mit
//! `ParticipantBuiltinTopicData` als PL_CDR_LE-Payload; der Caller
//! sendet es periodisch via Multicast. Beacon-Receiver parst
//! eingehende DATA-Submessages mit der SPDP-Reader-EntityId und liefert
//! ein `DiscoveredParticipant` an den Cache.
//!
//! Lease-Tracking: `DiscoveredParticipantsCache` führt `last_seen` pro
//! Participant; Caller (DCPS-Runtime) räumt abgelaufene Einträge gemäss
//! `participant.lease_duration` (PID_PARTICIPANT_LEASE_DURATION) auf.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use core::fmt;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram, encode_data_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::participant_data::ParticipantBuiltinTopicData;
use zerodds_rtps::submessages::DataSubmessage;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix, SequenceNumber, VendorId};

/// SPDP-spezifische Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpdpError {
    /// Wire-Decoding-Fehler (Datagram, Submessage, ParameterList).
    Wire(WireError),
    /// Datagram enthaelt keine SPDP-DATA-Submessage (Reader/Writer-IDs
    /// passen nicht).
    NotSpdp,
}

impl fmt::Display for SpdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "spdp wire error: {e}"),
            Self::NotSpdp => f.write_str("spdp: datagram is not an SPDP DATA submessage"),
        }
    }
}

impl From<WireError> for SpdpError {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SpdpError {}

// ============================================================================
// Beacon-Sender
// ============================================================================

/// SPDP-Beacon-Sender. Stateless: ruft `serialize()` auf, um ein
/// fertiges Datagram zu produzieren, das der Caller via Multicast
/// sendet.
#[derive(Debug, Clone)]
pub struct SpdpBeacon {
    /// Eigene Participant-Daten.
    pub data: ParticipantBuiltinTopicData,
    /// VendorId fuer den RTPS-Header (default ZeroDDS).
    pub vendor_id: VendorId,
    /// Naechste Sequence-Number fuer DATA-Submessages.
    pub next_sn: i64,
}

impl SpdpBeacon {
    /// Konstruktor.
    #[must_use]
    pub fn new(data: ParticipantBuiltinTopicData) -> Self {
        Self {
            data,
            vendor_id: VendorId::ZERODDS,
            next_sn: 1,
        }
    }

    /// Setzt eine bestimmte VendorId (sonst Default `ZERODDS`).
    pub fn set_vendor_id(&mut self, vendor: VendorId) {
        self.vendor_id = vendor;
    }

    /// Encoded ein SPDP-Beacon-Datagram.
    ///
    /// # Errors
    /// `WireError`, wenn DATA-Body groesser als u16::MAX oder Encoding
    /// scheitert.
    pub fn serialize(&mut self) -> Result<Vec<u8>, WireError> {
        #[cfg(feature = "metrics")]
        crate::metrics::inc_spdp_announcement_sent();
        let payload = self.data.to_pl_cdr_le();
        let sn = SequenceNumber(self.next_sn);
        self.next_sn = self
            .next_sn
            .checked_add(1)
            .ok_or(WireError::ValueOutOfRange {
                message: "spdp beacon sequence overflow",
            })?;
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SPDP_BUILTIN_PARTICIPANT_READER,
            writer_id: EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            writer_sn: sn,
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.data.guid.prefix);
        encode_data_datagram(header, &[data])
    }
}

// ============================================================================
// Beacon-Receiver
// ============================================================================

/// SPDP-Beacon-Receiver. Stateless: nimmt ein Datagram und versucht,
/// daraus eine DiscoveredParticipant-Info zu extrahieren.
#[derive(Debug, Clone, Default)]
pub struct SpdpReader;

impl SpdpReader {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Versucht, einen DiscoveredParticipant aus einem Datagram zu
    /// extrahieren.
    ///
    /// # Errors
    /// - `SpdpError::Wire` bei Decoder-Fehler.
    /// - `SpdpError::NotSpdp` wenn keine SPDP-DATA-Submessage darin ist.
    pub fn parse_datagram(&self, datagram: &[u8]) -> Result<DiscoveredParticipant, SpdpError> {
        let parsed = decode_datagram(datagram)?;
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                if d.writer_id == EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER {
                    match ParticipantBuiltinTopicData::from_pl_cdr_le(&d.serialized_payload) {
                        Ok(data) => {
                            return Ok(DiscoveredParticipant {
                                sender_prefix: parsed.header.guid_prefix,
                                sender_vendor: parsed.header.vendor_id,
                                data,
                            });
                        }
                        // Fremde Encapsulation (z.B. PL_CDR2 von Cyclone/Fast-DDS):
                        // kein echter Bug, einfach stillschweigend ueberspringen.
                        // Wird bei XCDR2-Rollout (Phase 1/2) ersetzt.
                        Err(WireError::UnsupportedEncapsulation { .. }) => continue,
                        Err(e) => return Err(SpdpError::Wire(e)),
                    }
                }
            }
        }
        Err(SpdpError::NotSpdp)
    }
}

/// Ergebnis einer SPDP-Beacon-Reception.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredParticipant {
    /// GuidPrefix aus dem RTPS-Header (sollte mit `data.guid.prefix`
    /// uebereinstimmen, kann bei Mis-Configuration abweichen).
    pub sender_prefix: GuidPrefix,
    /// VendorId aus dem RTPS-Header.
    pub sender_vendor: VendorId,
    /// Geparste Participant-Daten.
    pub data: ParticipantBuiltinTopicData,
}

// ============================================================================
// Cache
// ============================================================================

/// In-Memory-Cache aller bisher entdeckten Participants. `last_seen`
/// pro Eintrag wird vom Caller (DCPS-Runtime) gegen
/// `participant.lease_duration` geprüft, um abgelaufene Participants zu
/// purgen — der Cache selbst erzwingt kein Timeout.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredParticipantsCache {
    inner: BTreeMap<GuidPrefix, DiscoveredParticipant>,
}

impl DiscoveredParticipantsCache {
    /// Leerer Cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert/Update. Liefert `true` wenn ein NEUER Participant
    /// hinzugekommen ist (Caller kann darauf einen Listener aufrufen).
    pub fn insert(&mut self, p: DiscoveredParticipant) -> bool {
        let inserted = self.inner.insert(p.data.guid.prefix, p).is_none();
        if inserted {
            #[cfg(feature = "metrics")]
            crate::metrics::set_participants_known(self.inner.len());
        }
        inserted
    }

    /// Anzahl bekannte Participants.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` wenn leer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Lookup nach Prefix.
    #[must_use]
    pub fn get(&self, prefix: &GuidPrefix) -> Option<&DiscoveredParticipant> {
        self.inner.get(prefix)
    }

    /// Iter ueber alle bekannten Participants.
    pub fn iter(&self) -> impl Iterator<Item = &DiscoveredParticipant> {
        self.inner.values()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use zerodds_rtps::participant_data::{Duration, endpoint_flag};
    use zerodds_rtps::wire_types::{Guid, Locator, ProtocolVersion};

    fn sample_participant() -> ParticipantBuiltinTopicData {
        ParticipantBuiltinTopicData {
            guid: Guid::new(GuidPrefix::from_bytes([0xA; 12]), EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 1], 7410)),
            default_multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
            metatraffic_unicast_locator: None,
            metatraffic_multicast_locator: None,
            domain_id: None,
            builtin_endpoint_set: endpoint_flag::PARTICIPANT_ANNOUNCER
                | endpoint_flag::PARTICIPANT_DETECTOR,
            lease_duration: Duration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
        }
    }

    #[test]
    fn beacon_serializes_to_decodable_datagram() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let datagram = beacon.serialize().unwrap();
        let reader = SpdpReader::new();
        let discovered = reader.parse_datagram(&datagram).unwrap();
        assert_eq!(
            discovered.data.guid.prefix,
            GuidPrefix::from_bytes([0xA; 12])
        );
        assert_eq!(discovered.sender_vendor, VendorId::ZERODDS);
    }

    #[test]
    fn beacon_increments_sequence_number() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        beacon.serialize().unwrap();
        assert_eq!(beacon.next_sn, 2);
        beacon.serialize().unwrap();
        assert_eq!(beacon.next_sn, 3);
    }

    #[test]
    fn beacon_uses_spdp_builtin_writer_id() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let datagram = beacon.serialize().unwrap();
        let parsed = decode_datagram(&datagram).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::Data(d) => {
                assert_eq!(d.writer_id, EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER);
                assert_eq!(d.reader_id, EntityId::SPDP_BUILTIN_PARTICIPANT_READER);
            }
            other => panic!("expected DATA, got {other:?}"),
        }
    }

    #[test]
    fn reader_rejects_non_spdp_datagram() {
        // Ein normales User-DATA, kein SPDP.
        let header = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([1; 12]));
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::vec![1, 2, 3, 4].into(),
        };
        let datagram = encode_data_datagram(header, &[data]).unwrap();
        let reader = SpdpReader::new();
        let res = reader.parse_datagram(&datagram);
        assert!(matches!(res, Err(SpdpError::NotSpdp)));
    }

    #[test]
    fn reader_propagates_invalid_magic_as_wire_error() {
        let reader = SpdpReader::new();
        let res = reader.parse_datagram(&[0u8; 32]);
        assert!(matches!(res, Err(SpdpError::Wire(_))));
    }

    #[test]
    fn cache_starts_empty() {
        let c = DiscoveredParticipantsCache::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn cache_insert_returns_true_for_new_participant() {
        let mut c = DiscoveredParticipantsCache::new();
        let mut beacon = SpdpBeacon::new(sample_participant());
        let datagram = beacon.serialize().unwrap();
        let p = SpdpReader::new().parse_datagram(&datagram).unwrap();
        assert!(c.insert(p.clone()));
        assert_eq!(c.len(), 1);
        // Zweites Insert mit gleichem Prefix → false.
        assert!(!c.insert(p));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn cache_get_returns_inserted_participant() {
        let mut c = DiscoveredParticipantsCache::new();
        let mut beacon = SpdpBeacon::new(sample_participant());
        let datagram = beacon.serialize().unwrap();
        let p = SpdpReader::new().parse_datagram(&datagram).unwrap();
        let prefix = p.data.guid.prefix;
        c.insert(p);
        assert!(c.get(&prefix).is_some());
    }

    #[test]
    fn cache_iter_yields_all_known_participants() {
        let mut c = DiscoveredParticipantsCache::new();
        let mut p1 = sample_participant();
        let mut p2 = sample_participant();
        p1.guid = Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT);
        p2.guid = Guid::new(GuidPrefix::from_bytes([2; 12]), EntityId::PARTICIPANT);
        let mut b1 = SpdpBeacon::new(p1);
        let mut b2 = SpdpBeacon::new(p2);
        let d1 = b1.serialize().unwrap();
        let d2 = b2.serialize().unwrap();
        c.insert(SpdpReader::new().parse_datagram(&d1).unwrap());
        c.insert(SpdpReader::new().parse_datagram(&d2).unwrap());
        assert_eq!(c.iter().count(), 2);
    }
}
