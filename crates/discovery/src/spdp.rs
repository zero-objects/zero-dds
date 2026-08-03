// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Simple Participant Discovery Protocol (DDSI-RTPS 2.5 §8.5.3).
//!
//! Best-effort beacon. The beacon sender builds a DATA datagram with
//! `ParticipantBuiltinTopicData` as PL_CDR_LE payload; the caller
//! sends it periodically via multicast. The beacon receiver parses
//! incoming DATA submessages with the SPDP reader EntityId and delivers
//! a `DiscoveredParticipant` to the cache.
//!
//! Lease tracking: `DiscoveredParticipantsCache` keeps `last_seen` per
//! participant; the caller (DCPS runtime) purges expired entries per
//! `participant.lease_duration` (PID_PARTICIPANT_LEASE_DURATION).

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

/// SPDP-specific errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpdpError {
    /// Wire decoding error (datagram, submessage, ParameterList).
    Wire(WireError),
    /// Datagram contains no SPDP DATA submessage (reader/writer IDs
    /// do not match).
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

/// SPDP beacon sender. Stateless: call `serialize()` to produce a
/// ready datagram that the caller sends via multicast.
#[derive(Debug, Clone)]
pub struct SpdpBeacon {
    /// Own participant data.
    pub data: ParticipantBuiltinTopicData,
    /// VendorId for the RTPS header (default ZeroDDS).
    pub vendor_id: VendorId,
    /// Next sequence number for DATA submessages.
    pub next_sn: i64,
    /// Sequence number for the reliable secure SPDP writer (0xff0101c2,
    /// FastDDS interop). Its own SN space, separate from the plain SPDP writer.
    pub secure_next_sn: i64,
}

impl SpdpBeacon {
    /// Constructor.
    #[must_use]
    pub fn new(data: ParticipantBuiltinTopicData) -> Self {
        Self {
            data,
            vendor_id: VendorId::ZERODDS,
            next_sn: 1,
            secure_next_sn: 1,
        }
    }

    /// Sets a specific VendorId (otherwise default `ZERODDS`).
    pub fn set_vendor_id(&mut self, vendor: VendorId) {
        self.vendor_id = vendor;
    }

    /// Encodes an SPDP beacon datagram.
    ///
    /// # Errors
    /// `WireError` if the DATA body is larger than u16::MAX or encoding
    /// fails.
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

    /// Encodes the beacon on the **reliable secure SPDP writer**
    /// (0xff0101c2/c7, FastDDS `ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_
    /// SECURE_*`). Content identical to the plain beacon; only the writer/reader
    /// EntityId + its own SN space. FastDDS announces its full secured
    /// participant data over this channel and gates the crypto-token
    /// reciprocation/endpoint matching on it (cyclone does not need it).
    ///
    /// # Errors
    /// `WireError` like [`Self::serialize`].
    pub fn serialize_secure(&mut self) -> Result<Vec<u8>, WireError> {
        let payload = self.data.to_pl_cdr_le();
        // Durable single sample: ALWAYS SN=1. The secure SPDP carries ONE
        // participant instance, reliably re-announced; a fixed SN makes
        // re-sends (Phase-2b NACK resend) duplicates instead of gaps — otherwise
        // FastDDS' reliable reader sees non-continuous SNs and NACKs
        // forever.
        let sn = SequenceNumber(1);
        let _ = &mut self.secure_next_sn;
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            writer_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
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

/// SPDP beacon receiver. Stateless: takes a datagram and tries to
/// extract a DiscoveredParticipant from it.
#[derive(Debug, Clone, Default)]
pub struct SpdpReader;

impl SpdpReader {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Tries to extract a DiscoveredParticipant from a datagram.
    ///
    /// # Errors
    /// - `SpdpError::Wire` on decoder error.
    /// - `SpdpError::NotSpdp` if there is no SPDP DATA submessage in it.
    pub fn parse_datagram(&self, datagram: &[u8]) -> Result<DiscoveredParticipant, SpdpError> {
        let parsed = decode_datagram(datagram)?;
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                // Plain-SPDP (0x000100c2) ODER FastDDS' reliabler Secure-SPDP
                // (0xff0101c2) — both carry ParticipantBuiltinTopicData. FastDDS
                // places its identity_token/security_info over the secure channel.
                if d.writer_id == EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
                    || d.writer_id == EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER
                {
                    match ParticipantBuiltinTopicData::from_pl_cdr_le(&d.serialized_payload) {
                        Ok(data) => {
                            return Ok(DiscoveredParticipant {
                                sender_prefix: parsed.header.guid_prefix,
                                sender_vendor: parsed.header.vendor_id,
                                data,
                            });
                        }
                        // Foreign encapsulation (e.g. PL_CDR2 from Cyclone/Fast-DDS):
                        // not a real bug, just skip it silently.
                        // Will be replaced at XCDR2 rollout (phase 1/2).
                        Err(WireError::UnsupportedEncapsulation { .. }) => continue,
                        Err(e) => return Err(SpdpError::Wire(e)),
                    }
                }
            }
        }
        Err(SpdpError::NotSpdp)
    }
}

/// Result of an SPDP beacon reception.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredParticipant {
    /// GuidPrefix from the RTPS header (should match `data.guid.prefix`,
    /// may diverge on misconfiguration).
    pub sender_prefix: GuidPrefix,
    /// VendorId from the RTPS header.
    pub sender_vendor: VendorId,
    /// Parsed participant data.
    pub data: ParticipantBuiltinTopicData,
}

// ============================================================================
// Cache
// ============================================================================

/// In-memory cache of all participants discovered so far. `last_seen`
/// per entry is checked by the caller (DCPS runtime) against
/// `participant.lease_duration` to purge expired participants —
/// the cache itself enforces no timeout.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredParticipantsCache {
    inner: BTreeMap<GuidPrefix, DiscoveredParticipant>,
}

impl DiscoveredParticipantsCache {
    /// Empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert/update. Returns `true` if a NEW participant
    /// was added (the caller can invoke a listener on it).
    pub fn insert(&mut self, p: DiscoveredParticipant) -> bool {
        let inserted = self.inner.insert(p.data.guid.prefix, p).is_none();
        if inserted {
            #[cfg(feature = "metrics")]
            crate::metrics::set_participants_known(self.inner.len());
        }
        inserted
    }

    /// Number of known participants.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Lookup by prefix.
    #[must_use]
    pub fn get(&self, prefix: &GuidPrefix) -> Option<&DiscoveredParticipant> {
        self.inner.get(prefix)
    }

    /// Iterate over all known participants.
    pub fn iter(&self) -> impl Iterator<Item = &DiscoveredParticipant> {
        self.inner.values()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use alloc::vec;
    use zerodds_rtps::participant_data::{Duration, endpoint_flag};
    use zerodds_rtps::wire_types::{Guid, Locator, ProtocolVersion};

    fn sample_participant() -> ParticipantBuiltinTopicData {
        ParticipantBuiltinTopicData {
            guid: Guid::new(GuidPrefix::from_bytes([0xA; 12]), EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locators: vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
            default_multicast_locators: vec![Locator::udp_v4([239, 255, 0, 1], 7400)],
            metatraffic_unicast_locators: Vec::new(),
            metatraffic_multicast_locators: Vec::new(),
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
            participant_security_info: None,
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
        // A normal user DATA, not SPDP.
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
        // Second insert with the same prefix → false.
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
