// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the DCPS scenarios share: one keyed topic type whose bytes are
//! stated by the specification rather than taken from ZeroDDS, the QoS the
//! scenarios ask for, and a participant made through the public API.

use std::time::Duration;

use zerodds_dcps::dds_type::{DecodeError, EncodeError, PlainCdr2BeKeyHolder};
use zerodds_dcps::qos::{HistoryKind, ReliabilityKind};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsType, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos,
};

/// `@final struct Reading { @key unsigned long id; unsigned long seq; }`.
///
/// Its XCDR2 little-endian body is the two members, four octets each, with
/// no header of its own (DDS-XTypes 1.3 §7.4.3.5); [`Reading::xcdr2`]
/// writes the whole serialized payload with the PLAIN_CDR2 little-endian
/// encapsulation header `00 07 00 00` (§7.6.3.1.2, Table 60).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reading {
    /// The key.
    pub id: u32,
    /// The sequence number of the sample.
    pub seq: u32,
}

impl Reading {
    /// The type name every scenario registers.
    pub const NAME: &'static str = "integration::Reading";

    /// The serialized payload as the specification lays it out: the
    /// encapsulation header and the body.
    #[must_use]
    pub fn xcdr2(&self) -> Vec<u8> {
        let mut out = vec![0x00, 0x07, 0x00, 0x00];
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        out
    }

    /// The XCDR2 little-endian body alone, without the encapsulation header.
    #[must_use]
    pub fn xcdr2_body(&self) -> Vec<u8> {
        self.xcdr2()[4..].to_vec()
    }

    /// [`Reading::xcdr2`] as lower-case hex.
    #[must_use]
    pub fn xcdr2_hex(&self) -> String {
        self.xcdr2().iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl DdsType for Reading {
    const TYPE_NAME: &'static str = Self::NAME;
    const XCDR1_MATCHES_XCDR2: bool = true;
    const HAS_KEY: bool = true;
    const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.seq.to_le_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let word = |at: usize| -> Result<u32, DecodeError> {
            bytes
                .get(at..at + 4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or(DecodeError::Invalid {
                    what: "truncated Reading",
                })
        };
        Ok(Self {
            id: word(0)?,
            seq: word(4)?,
        })
    }

    fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
        holder.write_u32(self.id);
    }

    fn decode_key_holder_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        let id = bytes
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes)
            .ok_or(DecodeError::Invalid {
                what: "truncated Reading key holder",
            })?;
        Ok(Self { id, seq: 0 })
    }
}

/// The DDS-XTypes 1.3 data representation id of XCDR2 (§7.6.3.1.1).
pub const XCDR2: i16 = 2;

/// The DDS-XTypes 1.3 data representation id of XCDR, version 1
/// (§7.6.3.1.1).
pub const XCDR: i16 = 0;

/// A RELIABLE, KEEP_ALL writer that writes XCDR2.
#[must_use]
pub fn reliable_writer() -> DataWriterQos {
    let mut q = DataWriterQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    q.data_representation = Some(vec![XCDR2]);
    q
}

/// A RELIABLE, KEEP_ALL reader.
#[must_use]
pub fn reliable_reader() -> DataReaderQos {
    let mut q = DataReaderQos::default();
    q.reliability.kind = ReliabilityKind::Reliable;
    q.history.kind = HistoryKind::KeepAll;
    q
}

/// The runtime settings of a scenario participant: a 20 ms tick and a
/// 100 ms SPDP period, so discovery over loopback takes well under a second.
#[must_use]
pub fn quick_config() -> RuntimeConfig {
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    }
}

/// A participant on `domain` with [`quick_config`].
///
/// # Panics
/// When the participant cannot be created.
#[must_use]
pub fn participant(domain: i32) -> DomainParticipant {
    // zerodds-lint: allow no_panic_in_safe — a scenario without its
    // participant has no result.
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), quick_config())
        .unwrap_or_else(|e| panic!("participant on domain {domain}: {e:?}"))
}
