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
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use core::fmt;

use zerodds_rtps::datagram::{
    ParsedSubmessage, ReceiveContext, decode_datagram, encode_data_datagram,
};
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
    /// The SPDP DATA carries no sample, only the instance key
    /// (`D=0`, DDSI-RTPS 2.5 §8.3.7.2.5) — the announcement a
    /// participant sends when it leaves the domain (§8.5.3.3). The
    /// GuidPrefix is the receiver state's source (the RTPS header's, or
    /// the last INFO_SRC's), because a key-only payload is not a
    /// `ParticipantBuiltinTopicData` and cannot be parsed as one.
    ///
    /// Reported and not skipped: a caller that treated the departure as
    /// "nothing arrived" keeps the participant until its lease expires,
    /// and one that fed the key holder to `from_pl_cdr_le` would build a
    /// participant out of whatever the parameter list happened to
    /// contain.
    Dispose(GuidPrefix),
    /// The datagram carries this many SPDP samples, where
    /// [`SpdpReader::parse_datagram`] reads exactly one. OpenSplice's
    /// DDSI2 service announces every participant it hosts in one
    /// datagram, switching the source with INFO_SRC (§8.3.7.9);
    /// [`SpdpReader::samples`] reads them all.
    Bundled(usize),
}

/// One SPDP sample of a datagram (DDSI-RTPS 2.5 §8.5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpdpSample {
    /// A participant's `ParticipantBuiltinTopicData` (`D=1`).
    Announced(Box<DiscoveredParticipant>),
    /// The key-only DATA (`D=0`) a participant sends when it leaves the
    /// domain (§8.5.3.3); the GuidPrefix is the receiver state's source.
    Departed(GuidPrefix),
    /// A `D=1` sample whose parameter list does not decode.
    Unreadable {
        /// The receiver state's source prefix.
        source: GuidPrefix,
        /// What the decoder reported.
        error: WireError,
    },
}

/// The writer an SPDP sample came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpdpChannel {
    /// The best-effort SPDP writer (`DCPSParticipants`), the bootstrap of
    /// the authentication.
    Plain,
    /// The reliable secure SPDP writer (`DCPSParticipantsSecure`,
    /// DDS-Security 1.2 §7.5.1.6).
    Secure,
}

impl fmt::Display for SpdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "spdp wire error: {e}"),
            Self::NotSpdp => f.write_str("spdp: datagram is not an SPDP DATA submessage"),
            Self::Dispose(prefix) => {
                write!(f, "spdp: key-only DATA (participant leaving): {prefix:?}")
            }
            Self::Bundled(n) => {
                write!(f, "spdp: datagram carries {n} SPDP samples, not one")
            }
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
    /// Sequence number of the current change of the reliable secure SPDP
    /// writer (0xff0101c2, DDS-Security 1.2 §7.5.1.6); `0` before the first
    /// announcement. Its own SN space, separate from the plain SPDP writer.
    /// A new change (other participant data, or the departure) takes the
    /// next number; the same data sent again keeps its number.
    pub secure_sn: i64,
    /// The payload `secure_sn` stands for.
    secure_payload: Option<Vec<u8>>,
    /// `false` (DDSI-RTPS 2.5 §8.5.3.1): the HistoryCache of the SPDP writer
    /// holds one data-object, and each period resends that change with its
    /// sequence number; a new number goes with a change of the participant
    /// data. `true`: every announcement carries a new number, as Cyclone DDS,
    /// OpenDDS and ZeroDDS before it send it (recorded sessions under
    /// `tools/wire-capture/rtps-sessions`: Cyclone 1, 2; OpenDDS 1, 2, 3, 4;
    /// Fast DDS 1, 1, 1, 2, 2; RTI 1).
    pub new_sn_each_period: bool,
    /// The payload of the change that holds `next_sn - 1`.
    held_payload: Option<Vec<u8>>,
}

impl SpdpBeacon {
    /// Constructor.
    #[must_use]
    pub fn new(data: ParticipantBuiltinTopicData) -> Self {
        Self {
            data,
            vendor_id: VendorId::ZERODDS,
            next_sn: 1,
            secure_sn: 0,
            secure_payload: None,
            new_sn_each_period: false,
            held_payload: None,
        }
    }

    /// Sets [`Self::new_sn_each_period`].
    pub fn set_new_sn_each_period(&mut self, new_sn: bool) {
        self.new_sn_each_period = new_sn;
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
        // DDS-Security 1.2 §9.3.2.2, §7.5.1.6: the IdentityStatusToken "shall
        // be [...] included in the DCPSParticipantSecure builtin Topic", not in
        // DCPSParticipants.
        let payload = if self.data.identity_status_token.is_some() {
            let mut plain = self.data.clone();
            plain.identity_status_token = None;
            plain.to_pl_cdr_le()
        } else {
            self.data.to_pl_cdr_le()
        };
        // §8.5.3.1: one data-object, resent by `unsent_changes_reset`; its
        // sequence number changes with the participant data.
        let held = self.held_payload.as_deref() == Some(payload.as_slice());
        let sn = if held && !self.new_sn_each_period {
            SequenceNumber(self.next_sn - 1)
        } else {
            let sn = SequenceNumber(self.next_sn);
            self.next_sn = self
                .next_sn
                .checked_add(1)
                .ok_or(WireError::ValueOutOfRange {
                    message: "spdp beacon sequence overflow",
                })?;
            self.held_payload = Some(payload.clone());
            sn
        };
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SPDP_BUILTIN_PARTICIPANT_READER,
            writer_id: EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            writer_sn: sn,
            inline_qos: None,
            // A beacon carries the whole `ParticipantBuiltinTopicData`,
            // so D=1 and K=0 (§8.3.7.2.5). The flag is explicit because
            // the same submessage type also carries the key-only form.
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.data.guid.prefix);
        encode_data_datagram(header, &[data])
    }

    /// Encodes the announcement that this participant leaves the domain
    /// (DDSI-RTPS 2.5 §8.5.3.3): a key-only DATA (`D=0`, `K=1`) on the SPDP
    /// writer, its inline QoS `PID_KEY_HASH` (the participant GUID) and
    /// `PID_STATUS_INFO` disposed|unregistered, its `serializedKey`
    /// `PID_PARTICIPANT_GUID`. A peer that reads it drops the participant
    /// at once instead of keeping it, and sending to its closed sockets,
    /// until the lease runs out. The shape is the one the SEDP dispose
    /// takes, which Cyclone and OpenDDS were measured to send.
    ///
    /// # Errors
    /// `WireError` on encoding failure or sequence overflow.
    pub fn serialize_departure(&mut self) -> Result<Vec<u8>, WireError> {
        self.held_payload = None;
        let guid = self.data.guid.to_bytes();
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
            inline_qos: Some(zerodds_rtps::inline_qos::lifecycle_inline_qos(
                guid,
                zerodds_rtps::inline_qos::status_info::DISPOSED
                    | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
            )),
            data_flag: false,
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: zerodds_rtps::parameter_list::spdp_participant_key_pl_cdr_le(guid)
                .into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.data.guid.prefix);
        encode_data_datagram(header, &[data])
    }

    /// The sequence number of the current change of the secure SPDP writer:
    /// the participant data as it is now. DDS-Security 1.2 §7.5.1.6: "The
    /// ParticipantBuiltinTopicData contains information, such as participant
    /// Locators, which may change at run-time. These changes shall be sent
    /// using the DCPSParticipantsSecure builtin Topic." The writer is
    /// reliable, so a change is a new sample with the next number, and a
    /// repetition of the same data (a periodic announcement, a repair) is the
    /// same sample.
    ///
    /// # Errors
    /// `WireError` on sequence overflow.
    pub fn secure_sequence_number(&mut self) -> Result<i64, WireError> {
        let payload = self.data.to_pl_cdr_le();
        if self.secure_payload.as_deref() != Some(payload.as_slice()) {
            self.secure_sn = self
                .secure_sn
                .checked_add(1)
                .ok_or(WireError::ValueOutOfRange {
                    message: "secure spdp sequence overflow",
                })?;
            self.secure_payload = Some(payload);
        }
        Ok(self.secure_sn)
    }

    /// Encodes the beacon on the **reliable secure SPDP writer**
    /// (0xff0101c2/c7, `ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_
    /// SECURE_*`). Content of the plain beacon plus the IdentityStatusToken
    /// (§9.3.2.2); the writer/reader EntityId and its own SN space (see
    /// [`Self::secure_sequence_number`]) differ.
    ///
    /// # Errors
    /// `WireError` like [`Self::serialize`].
    pub fn serialize_secure(&mut self) -> Result<Vec<u8>, WireError> {
        let sn = SequenceNumber(self.secure_sequence_number()?);
        let payload = self.data.to_pl_cdr_le();
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            writer_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            writer_sn: sn,
            inline_qos: None,
            // A beacon carries the whole `ParticipantBuiltinTopicData`,
            // so D=1 and K=0 (§8.3.7.2.5). The flag is explicit because
            // the same submessage type also carries the key-only form.
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.data.guid.prefix);
        encode_data_datagram(header, &[data])
    }

    /// Encodes the deletion of this participant on the secure SPDP writer
    /// (DDS-Security 1.2 §7.5.1.6: "The deletion of a DomainParticipant
    /// shall also be sent using the DCPSParticipantsSecure builtin Topic"):
    /// a key-only DATA like [`Self::serialize_departure`], with the next
    /// sequence number of the secure writer.
    ///
    /// # Errors
    /// `WireError` on encoding failure or sequence overflow.
    pub fn serialize_secure_departure(&mut self) -> Result<Vec<u8>, WireError> {
        // The departure is a change of its own; it supersedes the data.
        self.secure_sequence_number()?;
        let sn = SequenceNumber(self.secure_sn.checked_add(1).ok_or(
            WireError::ValueOutOfRange {
                message: "secure spdp sequence overflow",
            },
        )?);
        self.secure_sn = sn.0;
        self.secure_payload = None;
        let guid = self.data.guid.to_bytes();
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            writer_id: EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            writer_sn: sn,
            inline_qos: Some(zerodds_rtps::inline_qos::lifecycle_inline_qos(
                guid,
                zerodds_rtps::inline_qos::status_info::DISPOSED
                    | zerodds_rtps::inline_qos::status_info::UNREGISTERED,
            )),
            data_flag: false,
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: zerodds_rtps::parameter_list::spdp_participant_key_pl_cdr_le(guid)
                .into(),
        };
        let header = RtpsHeader::new(self.vendor_id, self.data.guid.prefix);
        encode_data_datagram(header, &[data])
    }
}

// ============================================================================
// Beacon-Receiver
// ============================================================================

/// SPDP beacon receiver. Stateless: takes a datagram and extracts the
/// SPDP samples in it.
#[derive(Debug, Clone, Default)]
pub struct SpdpReader;

impl SpdpReader {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Every SPDP sample of a datagram, in the order they stand in it.
    ///
    /// Each DATA is attributed to the receiver state's source
    /// (DDSI-RTPS 2.5 §8.3.4, INFO_SRC §8.3.7.9), not to the datagram
    /// header: OpenSplice's DDSI2 service announces every participant it
    /// hosts in one datagram, and only one of them — not necessarily the
    /// first — carries the SEDP detectors a peer has to announce its
    /// endpoints to.
    ///
    /// # Errors
    /// - `SpdpError::Wire` if the datagram does not decode.
    /// - `SpdpError::NotSpdp` if there is no SPDP DATA submessage in it.
    pub fn samples(&self, datagram: &[u8]) -> Result<Vec<SpdpSample>, SpdpError> {
        self.samples_in(datagram, &ReceiveContext::default())
    }

    /// [`Self::samples`] for a datagram that arrived in `context`: its
    /// source and reception clock (DDSI-RTPS 2.5 §8.3.4).
    ///
    /// # Errors
    /// As [`Self::samples`].
    pub fn samples_in(
        &self,
        datagram: &[u8],
        context: &ReceiveContext,
    ) -> Result<Vec<SpdpSample>, SpdpError> {
        Ok(self
            .samples_with_channel_in(datagram, context)?
            .into_iter()
            .map(|(_, sample)| sample)
            .collect())
    }

    /// As [`Self::samples`], each with the writer it came from: the plain
    /// SPDP writer or the secure one (DDS-Security 1.2 §7.5.1.6).
    ///
    /// # Errors
    /// As [`Self::samples`].
    pub fn samples_with_channel(
        &self,
        datagram: &[u8],
    ) -> Result<Vec<(SpdpChannel, SpdpSample)>, SpdpError> {
        self.samples_with_channel_in(datagram, &ReceiveContext::default())
    }

    /// [`Self::samples_with_channel`] for a datagram that arrived in
    /// `context`.
    ///
    /// # Errors
    /// As [`Self::samples`].
    pub fn samples_with_channel_in(
        &self,
        datagram: &[u8],
        context: &ReceiveContext,
    ) -> Result<Vec<(SpdpChannel, SpdpSample)>, SpdpError> {
        let parsed = decode_datagram(datagram)?;
        let mut samples = Vec::new();
        // No filter on destGuidPrefix, so the seed value is immaterial.
        for (state, sub) in parsed.receiver_walk_in(GuidPrefix::UNKNOWN, context) {
            let ParsedSubmessage::Data(d) = sub else {
                continue;
            };
            // Plain SPDP (0x000100c2) or Fast DDS' reliable secure SPDP
            // (0xff0101c2) — both carry ParticipantBuiltinTopicData.
            if d.writer_id != EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
                && d.writer_id != EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER
            {
                continue;
            }
            // D=0 — no sample in the payload. §8.5.3.3's departure
            // announcement has this shape, and every vendor on the
            // measurement host sends one on shutdown. Parsing its key
            // holder as a participant is what the flag exists to prevent.
            let channel = if d.writer_id == EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER {
                SpdpChannel::Plain
            } else {
                SpdpChannel::Secure
            };
            if !d.data_flag {
                samples.push((channel, SpdpSample::Departed(state.source_guid_prefix)));
                continue;
            }
            samples.push((
                channel,
                match ParticipantBuiltinTopicData::from_pl_cdr_le(&d.serialized_payload) {
                    Ok(data) => SpdpSample::Announced(Box::new(DiscoveredParticipant {
                        sender_prefix: state.source_guid_prefix,
                        sender_vendor: state.source_vendor_id,
                        data,
                    })),
                    Err(error) => SpdpSample::Unreadable {
                        source: state.source_guid_prefix,
                        error,
                    },
                },
            ));
        }
        if samples.is_empty() {
            return Err(SpdpError::NotSpdp);
        }
        Ok(samples)
    }

    /// The participant of a datagram that carries exactly one SPDP
    /// sample, as a beacon does.
    ///
    /// # Errors
    /// - `SpdpError::Wire` if the datagram or the sample does not decode.
    /// - `SpdpError::NotSpdp` if there is no SPDP DATA submessage in it.
    /// - `SpdpError::Dispose` if the sample is a departure (`D=0`).
    /// - `SpdpError::Bundled` if it carries more than one sample; read
    ///   those with [`Self::samples`].
    pub fn parse_datagram(&self, datagram: &[u8]) -> Result<DiscoveredParticipant, SpdpError> {
        let mut samples = self.samples(datagram)?;
        if samples.len() > 1 {
            return Err(SpdpError::Bundled(samples.len()));
        }
        match samples.pop() {
            Some(SpdpSample::Announced(participant)) => Ok(*participant),
            Some(SpdpSample::Departed(prefix)) => Err(SpdpError::Dispose(prefix)),
            Some(SpdpSample::Unreadable { error, .. }) => Err(SpdpError::Wire(error)),
            None => Err(SpdpError::NotSpdp),
        }
    }

    /// The SPDP sample `source` announced in `datagram`, as a datagram of
    /// its own: an RTPS header with that source's version, vendor and
    /// prefix, followed by its DATA submessage with the payload it
    /// arrived with. For a caller that passes one participant's
    /// announcement on, such as a discovery-server relay, when the
    /// datagram bundles several.
    ///
    /// # Errors
    /// - `SpdpError::Wire` if the datagram does not decode or the DATA
    ///   does not encode.
    /// - `SpdpError::NotSpdp` if the datagram has no SPDP DATA from
    ///   `source`.
    pub fn sample_datagram(
        &self,
        datagram: &[u8],
        source: GuidPrefix,
    ) -> Result<Vec<u8>, SpdpError> {
        self.sample_datagram_in(datagram, source, &ReceiveContext::default())
    }

    /// [`Self::sample_datagram`] for a datagram that arrived in `context`.
    ///
    /// # Errors
    /// As [`Self::sample_datagram`].
    pub fn sample_datagram_in(
        &self,
        datagram: &[u8],
        source: GuidPrefix,
        context: &ReceiveContext,
    ) -> Result<Vec<u8>, SpdpError> {
        let parsed = decode_datagram(datagram)?;
        for (state, sub) in parsed.receiver_walk_in(GuidPrefix::UNKNOWN, context) {
            let ParsedSubmessage::Data(d) = sub else {
                continue;
            };
            if state.source_guid_prefix == source
                && (d.writer_id == EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
                    || d.writer_id == EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER)
            {
                let header = RtpsHeader {
                    protocol_version: state.source_version,
                    vendor_id: state.source_vendor_id,
                    guid_prefix: source,
                };
                return Ok(encode_data_datagram(header, core::slice::from_ref(d))?);
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

/// Discovered participants and private monotonic observation timestamps.
/// The caller drives lease expiry explicitly with [`Self::drain_expired`].
#[derive(Debug, Clone, Default)]
pub struct DiscoveredParticipantsCache {
    inner: BTreeMap<GuidPrefix, DiscoveredParticipant>,
    observed_at: BTreeMap<GuidPrefix, core::time::Duration>,
}

impl DiscoveredParticipantsCache {
    /// Empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
            observed_at: BTreeMap::new(),
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

    /// Inserts a live SPDP observation in the caller's monotonic time base.
    /// Legacy [`Self::insert`] remains available for untimed/offline caches.
    pub fn observe(&mut self, p: DiscoveredParticipant, now: core::time::Duration) -> bool {
        self.observed_at.insert(p.data.guid.prefix, now);
        self.insert(p)
    }

    /// Renews the lease of a known participant without changing what is
    /// known about it. `false` if the participant is not in the cache.
    pub fn touch(&mut self, prefix: &GuidPrefix, now: core::time::Duration) -> bool {
        if self.inner.contains_key(prefix) {
            self.observed_at.insert(*prefix, now);
            true
        } else {
            false
        }
    }

    /// Removes a participant and its observation timestamp.
    pub fn remove(&mut self, prefix: &GuidPrefix) -> Option<DiscoveredParticipant> {
        self.observed_at.remove(prefix);
        let removed = self.inner.remove(prefix);
        #[cfg(feature = "metrics")]
        crate::metrics::set_participants_known(self.inner.len());
        removed
    }

    /// Removes observations whose finite advertised lease elapsed.
    /// Infinite leases and legacy untimed inserts do not expire.
    pub fn drain_expired(&mut self, now: core::time::Duration) -> Vec<DiscoveredParticipant> {
        let prefixes: Vec<_> = self
            .observed_at
            .iter()
            .filter_map(|(prefix, observed)| {
                let participant = self.inner.get(prefix)?;
                let lease = participant.data.lease_duration;
                (!lease.is_infinite()
                    && now.saturating_sub(*observed).as_nanos() >= lease.to_nanos())
                .then_some(*prefix)
            })
            .collect();
        prefixes
            .into_iter()
            .filter_map(|prefix| self.remove(&prefix))
            .collect()
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
            domain_tag: Default::default(),
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
            available_builtin_endpoints_ext: 0,
            participant_security_info: None,
            expects_inline_qos: false,
            manual_liveliness_count: None,
            builtin_endpoint_qos: None,
            entity_name: None,
        }
    }

    /// DDS-Security 1.2 §7.5.1.6: `identity_status_token` (PID 0x1006) is a
    /// member of ParticipantBuiltinTopicDataSecure only; the plain SPDP does
    /// not carry it, the secure SPDP does.
    #[test]
    fn the_identity_status_token_is_in_the_secure_participant_data_only() {
        let mut data = sample_participant();
        data.identity_status_token = Some(vec![1, 2, 3, 4]);
        let mut beacon = SpdpBeacon::new(data);
        let payload_of = |datagram: &[u8]| {
            let parsed = decode_datagram(datagram).unwrap();
            let data = parsed
                .submessages
                .into_iter()
                .find_map(|f| match f.submessage {
                    ParsedSubmessage::Data(d) => Some(d),
                    _ => None,
                })
                .unwrap();
            ParticipantBuiltinTopicData::from_pl_cdr_le(&data.serialized_payload).unwrap()
        };
        assert_eq!(
            payload_of(&beacon.serialize().unwrap()).identity_status_token,
            None
        );
        assert_eq!(
            payload_of(&beacon.serialize_secure().unwrap()).identity_status_token,
            Some(vec![1, 2, 3, 4])
        );
    }

    fn sn_of(datagram: &[u8]) -> i64 {
        let parsed = decode_datagram(datagram).unwrap();
        parsed
            .submessages
            .into_iter()
            .find_map(|f| match f.submessage {
                ParsedSubmessage::Data(d) => Some(d.writer_sn.0),
                _ => None,
            })
            .unwrap()
    }

    /// DDS-Security 1.2 §7.5.1.6: changes of the participant data "shall be
    /// sent using the DCPSParticipantsSecure builtin Topic" by a reliable
    /// writer: a change is a new sample with the next sequence number, the
    /// same data sent again is the same sample (it was always number 1, so a
    /// reliable reader took every change for a duplicate).
    #[test]
    fn secure_spdp_takes_a_new_number_for_a_change_only() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let first = beacon.serialize_secure().unwrap();
        assert_eq!(sn_of(&first), 1);
        assert_eq!(sn_of(&beacon.serialize_secure().unwrap()), 1, "repeated");
        assert_eq!(beacon.secure_sequence_number().unwrap(), 1);
        beacon
            .data
            .default_unicast_locators
            .push(Locator::udp_v4([127, 0, 0, 2], 7411));
        let changed = beacon.serialize_secure().unwrap();
        assert_eq!(sn_of(&changed), 2, "changed locators");
        assert_eq!(sn_of(&beacon.serialize_secure().unwrap()), 2);
        // The plain writer has its own numbers.
        assert_eq!(sn_of(&beacon.serialize().unwrap()), 1);
    }

    /// DDS-Security 1.2 §7.5.12: the SPDP datagrams, the plain and the secure
    /// one, hold the DATA of their own writer and no submessage of another
    /// builtin topic or of an application topic.
    #[test]
    fn an_spdp_datagram_holds_the_submessages_of_its_writer_only() {
        use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
        let mut beacon = SpdpBeacon::new(sample_participant());
        let cases = [
            (
                beacon.serialize().unwrap(),
                EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            ),
            (
                beacon.serialize_secure().unwrap(),
                EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            ),
        ];
        for (bytes, own) in cases {
            let mut data = 0;
            for sm in decode_datagram(&bytes).unwrap().submessages {
                match sm.submessage {
                    ParsedSubmessage::Data(d) => {
                        data += 1;
                        assert_eq!(d.writer_id, own);
                    }
                    ParsedSubmessage::InfoDestination(_) | ParsedSubmessage::InfoTimestamp(_) => {}
                    other => panic!("a submessage of another topic: {other:?}"),
                }
            }
            assert_eq!(data, 1);
        }
    }

    /// DDS-Security 1.2 §7.5.1.6: "The deletion of a DomainParticipant shall
    /// also be sent using the DCPSParticipantsSecure builtin Topic": a
    /// key-only DATA on the secure writer, a change of its own.
    #[test]
    fn secure_spdp_departure_is_a_key_only_change_on_the_secure_writer() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        beacon.serialize_secure().unwrap();
        let departure = beacon.serialize_secure_departure().unwrap();
        assert_eq!(sn_of(&departure), 2);
        let samples = SpdpReader::new().samples_with_channel(&departure).unwrap();
        assert_eq!(
            samples,
            alloc::vec![(
                SpdpChannel::Secure,
                SpdpSample::Departed(GuidPrefix::from_bytes([0xA; 12]))
            )]
        );
        // The data after the departure is a new change again.
        assert_eq!(sn_of(&beacon.serialize_secure().unwrap()), 3);
    }

    #[test]
    fn samples_name_the_writer_they_came_from() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let plain = beacon.serialize().unwrap();
        let secure = beacon.serialize_secure().unwrap();
        let reader = SpdpReader::new();
        assert_eq!(
            reader.samples_with_channel(&plain).unwrap()[0].0,
            SpdpChannel::Plain
        );
        assert_eq!(
            reader.samples_with_channel(&secure).unwrap()[0].0,
            SpdpChannel::Secure
        );
    }

    #[test]
    fn touch_renews_the_lease_of_a_known_participant_only() {
        let mut cache = DiscoveredParticipantsCache::new();
        let p = DiscoveredParticipant {
            sender_prefix: GuidPrefix::from_bytes([0xA; 12]),
            sender_vendor: VendorId::ZERODDS,
            data: sample_participant(),
        };
        let prefix = p.data.guid.prefix;
        assert!(!cache.touch(&prefix, core::time::Duration::from_secs(1)));
        cache.observe(p, core::time::Duration::from_secs(0));
        // The lease is 100 s: touched at 90 s it holds at 150 s.
        assert!(cache.touch(&prefix, core::time::Duration::from_secs(90)));
        assert!(
            cache
                .drain_expired(core::time::Duration::from_secs(150))
                .is_empty()
        );
        assert_eq!(
            cache
                .drain_expired(core::time::Duration::from_secs(200))
                .len(),
            1
        );
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

    /// A datagram with an announcement, a departure and another
    /// announcement, the source switched by INFO_SRC between them as
    /// OpenSplice's DDSI2 service does.
    fn bundle() -> (Vec<u8>, [GuidPrefix; 3]) {
        use zerodds_rtps::datagram::{ParsedDatagram, SubmessageFrame, encode_datagram};
        use zerodds_rtps::submessages::InfoSourceSubmessage;
        let prefixes = [1, 2, 3].map(|b| GuidPrefix::from_bytes([b; 12]));
        let opensplice = VendorId([0x01, 0x02]);
        let data_of = |prefix| {
            let mut p = sample_participant();
            p.guid = Guid::new(prefix, EntityId::PARTICIPANT);
            let bytes = SpdpBeacon::new(p).serialize().unwrap();
            decode_datagram(&bytes).unwrap().submessages.remove(0)
        };
        let info_src = |prefix| SubmessageFrame {
            little_endian: true,
            octets_elided: false,
            submessage: ParsedSubmessage::InfoSource(InfoSourceSubmessage {
                unused: 0,
                protocol_version: ProtocolVersion::V2_1,
                vendor_id: opensplice,
                guid_prefix: prefix,
            }),
        };
        let mut departure = data_of(prefixes[1]);
        if let ParsedSubmessage::Data(d) = &mut departure.submessage {
            d.data_flag = false;
            d.key_flag = true;
            // PL_CDR_LE, PID_SENTINEL: the key holder is not read.
            d.serialized_payload = alloc::vec![0, 3, 0, 0, 1, 0, 0, 0].into();
        }
        let datagram = ParsedDatagram {
            header: RtpsHeader::new(opensplice, prefixes[0]),
            submessages: vec![
                data_of(prefixes[0]),
                info_src(prefixes[1]),
                departure,
                info_src(prefixes[2]),
                data_of(prefixes[2]),
            ],
            invalid_rest: None,
        };
        (encode_datagram(&datagram).unwrap(), prefixes)
    }

    /// The departure reads back as `Departed` of the sender, carries the
    /// participant GUID as key hash and as serialized key, and the
    /// status disposed|unregistered.
    #[test]
    fn a_departure_reads_back_as_departed_with_key_and_status() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let prefix = beacon.data.guid.prefix;
        let first = beacon.serialize().unwrap();
        let departure = beacon.serialize_departure().unwrap();
        assert_eq!(
            SpdpReader::new().samples(&departure).unwrap(),
            alloc::vec![SpdpSample::Departed(prefix)]
        );
        let parsed = decode_datagram(&departure).unwrap();
        let ParsedSubmessage::Data(d) = &parsed.submessages[0].submessage else {
            panic!("not a DATA");
        };
        assert!(!d.data_flag && d.key_flag);
        let qos = d.inline_qos.as_ref().unwrap();
        let guid = beacon.data.guid.to_bytes();
        assert_eq!(zerodds_rtps::inline_qos::find_key_hash(qos), Some(guid));
        assert_eq!(zerodds_rtps::inline_qos::find_status_info(qos), Some(3));
        assert_eq!(
            &d.serialized_payload[..],
            &zerodds_rtps::parameter_list::spdp_participant_key_pl_cdr_le(guid)[..]
        );
        let ParsedSubmessage::Data(beacon_data) =
            &decode_datagram(&first).unwrap().submessages[0].submessage
        else {
            panic!("not a DATA");
        };
        assert!(d.writer_sn > beacon_data.writer_sn, "after the last beacon");
    }

    #[test]
    fn samples_reads_every_participant_a_datagram_bundles() {
        let (datagram, [a, b, c]) = bundle();
        let samples = SpdpReader::new().samples(&datagram).unwrap();
        assert_eq!(samples.len(), 3);
        assert!(
            matches!(&samples[0], SpdpSample::Announced(p) if p.sender_prefix == a && p.data.guid.prefix == a)
        );
        assert_eq!(samples[1], SpdpSample::Departed(b));
        assert!(
            matches!(&samples[2], SpdpSample::Announced(p) if p.sender_prefix == c && p.data.guid.prefix == c)
        );
    }

    #[test]
    fn parse_datagram_refuses_a_bundle_instead_of_reading_its_first_sample() {
        let (datagram, _) = bundle();
        assert_eq!(
            SpdpReader::new().parse_datagram(&datagram),
            Err(SpdpError::Bundled(3))
        );
    }

    #[test]
    fn sample_datagram_cuts_one_participants_announcement_out_of_a_bundle() {
        let (datagram, [_, _, c]) = bundle();
        let reader = SpdpReader::new();
        let alone = reader.sample_datagram(&datagram, c).unwrap();
        let p = reader.parse_datagram(&alone).unwrap();
        assert_eq!(p.sender_prefix, c);
        assert_eq!(p.sender_vendor, VendorId([0x01, 0x02]));
        assert_eq!(p.data.guid.prefix, c);
        assert_eq!(
            reader.sample_datagram(&datagram, GuidPrefix::from_bytes([9; 12])),
            Err(SpdpError::NotSpdp)
        );
    }

    #[test]
    fn beacon_increments_sequence_number() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        beacon.set_new_sn_each_period(true);
        beacon.serialize().unwrap();
        assert_eq!(beacon.next_sn, 2);
        beacon.serialize().unwrap();
        assert_eq!(beacon.next_sn, 3);
    }

    /// The writer sequence numbers of the DATA in `beacons`.
    fn sns_of(beacons: &[Vec<u8>]) -> Vec<i64> {
        beacons
            .iter()
            .map(
                |b| match &decode_datagram(b).unwrap().submessages[0].submessage {
                    ParsedSubmessage::Data(d) => d.writer_sn.0,
                    other => panic!("expected DATA, got {other:?}"),
                },
            )
            .collect()
    }

    /// DDSI-RTPS 2.5 §8.5.3.1: the SPDP writer's HistoryCache "contains a
    /// single data-object", resent each period by `unsent_changes_reset`: the
    /// same change keeps its sequence number, and a change of the participant
    /// data is a new change with the next one.
    #[test]
    fn the_resent_beacon_keeps_its_sequence_number_until_the_data_changes() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let first = [
            beacon.serialize().unwrap(),
            beacon.serialize().unwrap(),
            beacon.serialize().unwrap(),
        ];
        assert_eq!(sns_of(&first), vec![1, 1, 1]);
        beacon.data.user_data = vec![1, 2, 3];
        let changed = [beacon.serialize().unwrap(), beacon.serialize().unwrap()];
        assert_eq!(sns_of(&changed), vec![2, 2]);
        // The vendor form: a number for every announcement.
        beacon.set_new_sn_each_period(true);
        let each = [beacon.serialize().unwrap(), beacon.serialize().unwrap()];
        assert_eq!(sns_of(&each), vec![3, 4]);
    }

    #[test]
    fn beacon_uses_spdp_builtin_writer_id() {
        let mut beacon = SpdpBeacon::new(sample_participant());
        let datagram = beacon.serialize().unwrap();
        let parsed = decode_datagram(&datagram).unwrap();
        match &parsed.submessages[0].submessage {
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
            // A beacon carries the whole `ParticipantBuiltinTopicData`,
            // so D=1 and K=0 (§8.3.7.2.5). The flag is explicit because
            // the same submessage type also carries the key-only form.
            data_flag: true,
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
    fn participant_lease_lifecycle_observation_and_infinite() {
        let mut cache = DiscoveredParticipantsCache::new();
        let mut beacon = SpdpBeacon::new(sample_participant());
        let mut peer = SpdpReader::new()
            .parse_datagram(&beacon.serialize().unwrap())
            .unwrap();
        peer.data.lease_duration = Duration::from_secs(2);
        let now = core::time::Duration::from_secs;
        assert!(cache.observe(peer.clone(), now(0)));
        assert!(cache.drain_expired(now(1)).is_empty());
        assert!(!cache.observe(peer.clone(), now(1)));
        assert!(cache.drain_expired(now(2)).is_empty());
        assert_eq!(cache.drain_expired(now(3)).len(), 1);
        assert!(cache.is_empty());
        assert!(cache.observe(peer.clone(), now(3)));
        peer.data.lease_duration = Duration::INFINITE;
        cache.observe(peer, now(4));
        assert!(cache.drain_expired(now(u64::MAX)).is_empty());
        assert_eq!(cache.len(), 1);
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
