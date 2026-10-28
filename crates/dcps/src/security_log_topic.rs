// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The BuiltinLoggingTopic wire types and their `DdsType` codecs
//! (DDS-Security 1.2 §10.6).
//!
//! The builtin `DDS:Logging:DDS_LogTopic` plugin publishes security events on
//! the DDS topic `DDS:Security:LogTopicV2` with type `BuiltinLoggingTypeV2`,
//! and optionally on the legacy `DDS:Security:LogTopic` with type
//! `BuiltinLoggingType`. Both are `@extensibility(APPENDABLE)` structs defined
//! in the clause's IDL; this module holds their `DdsType` implementations
//! (the XCDR2 codec) and the [`LogTopicPublisher`]
//! (zerodds_security_logging::LogTopicPublisher) that writes them through a
//! `DataWriter`.
//!
//! The plain data types live in `zerodds-security-logging`, which cannot
//! depend on this crate (this crate depends on it); `DdsType` is local here,
//! so the impls are written here rather than there.
//!
//! # Wire form
//!
//! XCDR2 PLAIN_CDR2 (little-endian) capped at `MAXALIGN` four (XTypes 1.3
//! §7.4.1.1.1), framed as DELIMITED_CDR: a four-octet `DHEADER` announcing the
//! member octets that follow (§7.4.3.4.2). The enum keeps the IDL default bit
//! bound (32). A `sequence`/`map` whose element is not primitive carries its
//! own delimiter (§7.4.3.5 rules 12 and 15), which is why
//! `NameValuePairSeq` and `structured_data` are delimited here.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_security::logging::{TOPIC_LOG_TOPIC, TOPIC_LOG_TOPIC_V2};
use zerodds_security_logging as sec_log;
use zerodds_wire::{Endianness, Reader, Writer};

use crate::dds_type::{DdsType, DecodeError, EncodeError, Extensibility};
use crate::participant::DomainParticipant;
use crate::publisher::DataWriter;
use crate::qos::{DataWriterQos, PublisherQos, TopicQos};

pub use zerodds_security_logging::{
    BuiltinLoggingType, BuiltinLoggingTypeV2, LogTopicLoggingPlugin, LogTopicPublishError,
    LogTopicPublisher, LoggingLevel,
};

/// The `LoggingLevel` enum of a decoded V2 sample, refusing a value the IDL
/// does not define (DDS-Security 1.2 §10.6).
fn severity_from_u32(value: u32) -> Result<LoggingLevel, DecodeError> {
    Ok(match value {
        0 => LoggingLevel::Emergency,
        1 => LoggingLevel::Alert,
        2 => LoggingLevel::Critical,
        3 => LoggingLevel::Error,
        4 => LoggingLevel::Warning,
        5 => LoggingLevel::Notice,
        6 => LoggingLevel::Informational,
        7 => LoggingLevel::Debug,
        _ => {
            return Err(DecodeError::Invalid {
                what: "LoggingLevel is one of the eight IDL enumerators",
            });
        }
    })
}

/// Reads the `DHEADER` and returns the absolute position its record ends at.
///
/// An `@appendable` type may grow members; a reader that knows the object's end
/// skips whatever it does not declare rather than reading the next member out
/// of the wrong octet (XTypes 1.3 §7.4.3.4.2).
fn record_end(reader: &mut Reader<'_>) -> Result<usize, DecodeError> {
    let length = reader.read_dheader()?;
    reader
        .position()
        .checked_add(length)
        .ok_or(DecodeError::Invalid {
            what: "DHEADER length overflows the address space",
        })
}

/// Writes the `map<string, NameValuePairSeq> structured_data` member
/// (DDS-Security 1.2 §10.6).
fn write_structured_data(
    writer: &mut Writer,
    data: &BTreeMap<String, sec_log::NameValuePairSeq>,
) -> Result<(), EncodeError> {
    writer.write_delimited_map(
        data,
        |w, key| {
            w.write_string(key);
            Ok(())
        },
        |w, pairs| {
            w.write_delimited_sequence(pairs, |w, pair| {
                w.write_string(&pair.name);
                w.write_string(&pair.value);
                Ok(())
            })
        },
    )?;
    Ok(())
}

/// Reads the `map<string, NameValuePairSeq> structured_data` member.
fn read_structured_data(
    reader: &mut Reader<'_>,
) -> Result<BTreeMap<String, sec_log::NameValuePairSeq>, DecodeError> {
    Ok(reader.read_delimited_map(
        |r| r.read_string(),
        |r| {
            r.read_delimited_sequence(|r| {
                let name = r.read_string()?;
                let value = r.read_string()?;
                Ok(sec_log::NameValuePair { name, value })
            })
        },
    )?)
}

impl DdsType for sec_log::BuiltinLoggingTypeV2 {
    const TYPE_NAME: &'static str = "BuiltinLoggingTypeV2";
    /// DDS-Security 1.2 §10.6 declares the struct
    /// `@extensibility(APPENDABLE)`.
    const EXTENSIBILITY: Extensibility = Extensibility::Appendable;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let mut writer = Writer::with_max_align(Endianness::Little, 4);
        let header = writer.write_dheader();
        writer.write_u8(self.facility);
        writer.write_u32(self.severity as u32);
        writer.write_i64(self.timestamp.sec);
        writer.write_u32(self.timestamp.nanosec);
        writer.write_string(&self.hostname);
        writer.write_string(&self.hostip);
        writer.write_string(&self.appname);
        writer.write_string(&self.procid);
        writer.write_string(&self.msgid);
        writer.write_string(&self.message);
        write_structured_data(&mut writer, &self.structured_data)?;
        writer.finish_dheader(header);
        out.extend_from_slice(writer.as_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::with_max_align(bytes, Endianness::Little, 4);
        let end = record_end(&mut reader)?;
        let facility = reader.read_u8()?;
        let severity = severity_from_u32(reader.read_u32()?)?;
        let timestamp = sec_log::Time {
            sec: reader.read_i64()?,
            nanosec: reader.read_u32()?,
        };
        let hostname = reader.read_string()?;
        let hostip = reader.read_string()?;
        let appname = reader.read_string()?;
        let procid = reader.read_string()?;
        let msgid = reader.read_string()?;
        let message = reader.read_string()?;
        let structured_data = read_structured_data(&mut reader)?;
        reader.skip_to(end)?;
        Ok(Self {
            facility,
            severity,
            timestamp,
            hostname,
            hostip,
            appname,
            procid,
            msgid,
            message,
            structured_data,
        })
    }
}

impl DdsType for sec_log::BuiltinLoggingType {
    const TYPE_NAME: &'static str = "BuiltinLoggingType";
    /// DDS-Security 1.2 §10.6 declares the struct
    /// `@extensibility(APPENDABLE)`.
    const EXTENSIBILITY: Extensibility = Extensibility::Appendable;

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let mut writer = Writer::with_max_align(Endianness::Little, 4);
        let header = writer.write_dheader();
        writer.write_u8(self.facility);
        writer.write_u32(self.severity as u32);
        // `LegacyTime_t.sec` is a 32-bit `long`; the unsigned `nanosec` is
        // still 32 bits.
        writer.write_i32(self.timestamp.sec);
        writer.write_u32(self.timestamp.nanosec);
        writer.write_string(&self.hostname);
        writer.write_string(&self.hostip);
        writer.write_string(&self.appname);
        writer.write_string(&self.procid);
        writer.write_string(&self.msgid);
        writer.write_string(&self.message);
        write_structured_data(&mut writer, &self.structured_data)?;
        writer.finish_dheader(header);
        out.extend_from_slice(writer.as_bytes());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut reader = Reader::with_max_align(bytes, Endianness::Little, 4);
        let end = record_end(&mut reader)?;
        let facility = reader.read_u8()?;
        let severity = severity_from_u32(reader.read_u32()?)?;
        let timestamp = sec_log::LegacyTime {
            sec: reader.read_i32()?,
            nanosec: reader.read_u32()?,
        };
        let hostname = reader.read_string()?;
        let hostip = reader.read_string()?;
        let appname = reader.read_string()?;
        let procid = reader.read_string()?;
        let msgid = reader.read_string()?;
        let message = reader.read_string()?;
        let structured_data = read_structured_data(&mut reader)?;
        reader.skip_to(end)?;
        Ok(Self {
            facility,
            severity,
            timestamp,
            hostname,
            hostip,
            appname,
            procid,
            msgid,
            message,
            structured_data,
        })
    }
}

/// [``LogTopicPublisher``] over the `DDS:Security:LogTopicV2` DataWriter and,
/// when present, the legacy `DDS:Security:LogTopic` DataWriter.
///
/// A write error is handed to the caller as [`LogTopicPublishError`], so the
/// sink reports it instead of losing the event.
pub struct LogTopicDataWriterPublisher {
    v2: DataWriter<sec_log::BuiltinLoggingTypeV2>,
    legacy: Option<DataWriter<sec_log::BuiltinLoggingType>>,
    domain_id: crate::participant::DomainId,
}

impl LogTopicDataWriterPublisher {
    /// Wraps an already-created V2 writer, an optional legacy writer, and the
    /// participant's domain id (which goes into the `DDS` structured-data
    /// entry, DDS-Security 1.2 §9.6.2.2.2, Table 47).
    #[must_use]
    pub fn new(
        v2: DataWriter<sec_log::BuiltinLoggingTypeV2>,
        legacy: Option<DataWriter<sec_log::BuiltinLoggingType>>,
        domain_id: crate::participant::DomainId,
    ) -> Self {
        Self {
            v2,
            legacy,
            domain_id,
        }
    }
}

/// Adds the `domain_id` pair to a sample's `DDS` structured-data entry
/// (DDS-Security 1.2 §9.6.2.2.2, Table 47). The sink fills the other pairs;
/// only the participant knows the domain.
fn add_domain_id(data: &mut BTreeMap<String, sec_log::NameValuePairSeq>, domain_id: i32) {
    let entry = data.entry(String::from("DDS")).or_default();
    let value = alloc::format!("{domain_id}");
    match entry.iter_mut().find(|p| p.name == "domain_id") {
        Some(pair) => pair.value = value,
        None => entry.push(sec_log::NameValuePair {
            name: String::from("domain_id"),
            value,
        }),
    }
}

impl LogTopicPublisher for LogTopicDataWriterPublisher {
    fn publish_v2(
        &self,
        event: &sec_log::BuiltinLoggingTypeV2,
    ) -> Result<(), LogTopicPublishError> {
        let mut event = event.clone();
        add_domain_id(&mut event.structured_data, self.domain_id);
        self.v2.write(&event).map_err(|e| LogTopicPublishError {
            topic: TOPIC_LOG_TOPIC_V2,
            reason: alloc::format!("{e}"),
        })
    }

    fn publish_legacy(
        &self,
        event: &sec_log::BuiltinLoggingType,
    ) -> Result<(), LogTopicPublishError> {
        match &self.legacy {
            Some(writer) => {
                let mut event = event.clone();
                add_domain_id(&mut event.structured_data, self.domain_id);
                writer.write(&event).map_err(|e| LogTopicPublishError {
                    topic: TOPIC_LOG_TOPIC,
                    reason: alloc::format!("{e}"),
                })
            }
            None => Err(LogTopicPublishError {
                topic: TOPIC_LOG_TOPIC,
                reason: String::from(
                    "the legacy DDS:Security:LogTopic DataWriter was not created \
                     (org.zerodds.sec.log.builtin.DDS_LogTopic.legacy_topic is off)",
                ),
            }),
        }
    }
}

/// The writer QoS for a Log Topic DataWriter: XCDR2 only.
///
/// The logging types have no XCDR1 codec (`XCDR1_MATCHES_XCDR2` is false — the
/// `int64`/`uint64` members and the non-primitive sequence and map are framed
/// differently under XCDR1), so a writer that offered XCDR1 first would try to
/// encode one and refuse. The offer names XCDR2, which is the encoding this
/// codec implements.
fn log_topic_writer_qos() -> DataWriterQos {
    DataWriterQos {
        data_representation: Some(alloc::vec![
            zerodds_rtps::publication_data::data_representation::XCDR2
        ]),
        ..DataWriterQos::default()
    }
}

/// Creates the BuiltinLoggingTopic DataWriter(s) on `participant` and returns
/// the publisher the [`LogTopicLoggingPlugin`] writes through
/// (DDS-Security 1.2 §10.6).
///
/// The V2 topic is always created; the legacy topic and its second DataWriter
/// only when `publish_legacy` is set.
///
/// # Errors
/// Whatever `create_topic` / `create_datawriter` reports.
pub fn log_topic_publisher_for_participant(
    participant: &DomainParticipant,
    publish_legacy: bool,
) -> crate::Result<Arc<dyn LogTopicPublisher>> {
    let v2_topic = participant
        .create_topic::<sec_log::BuiltinLoggingTypeV2>(TOPIC_LOG_TOPIC_V2, TopicQos::default())?;
    let publisher = participant.create_publisher(PublisherQos::default());
    let v2 = publisher
        .create_datawriter::<sec_log::BuiltinLoggingTypeV2>(&v2_topic, log_topic_writer_qos())?;
    let legacy = if publish_legacy {
        let legacy_topic = participant
            .create_topic::<sec_log::BuiltinLoggingType>(TOPIC_LOG_TOPIC, TopicQos::default())?;
        let writer = publisher.create_datawriter::<sec_log::BuiltinLoggingType>(
            &legacy_topic,
            log_topic_writer_qos(),
        )?;
        Some(writer)
    } else {
        None
    };
    Ok(Arc::new(LogTopicDataWriterPublisher::new(
        v2,
        legacy,
        participant.domain_id(),
    )))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample() -> sec_log::BuiltinLoggingTypeV2 {
        let mut structured_data = BTreeMap::new();
        structured_data.insert(
            String::from("DDS"),
            alloc::vec![sec_log::NameValuePair {
                name: String::from("n"),
                value: String::from("v"),
            }],
        );
        sec_log::BuiltinLoggingTypeV2 {
            facility: sec_log::FACILITY_SEC_AUTH,
            severity: LoggingLevel::Critical,
            timestamp: sec_log::Time { sec: 1, nanosec: 2 },
            hostname: String::from("h"),
            hostip: String::from("i"),
            appname: String::from("a"),
            procid: String::from("p"),
            msgid: String::from("m"),
            message: String::from("x"),
            structured_data,
        }
    }

    /// The bytes the IDL in DDS-Security 1.2 §10.6 says one event becomes,
    /// built by hand: DELIMITED_CDR, little-endian, `MAXALIGN` four.
    fn hand_built_v2(event: &sec_log::BuiltinLoggingTypeV2) -> Vec<u8> {
        fn align4(b: &mut Vec<u8>) {
            while b.len() % 4 != 0 {
                b.push(0);
            }
        }
        fn u32b(b: &mut Vec<u8>, v: u32) {
            align4(b);
            b.extend_from_slice(&v.to_le_bytes());
        }
        fn i64b(b: &mut Vec<u8>, v: i64) {
            align4(b);
            b.extend_from_slice(&v.to_le_bytes());
        }
        // Strings on the wire: a `uint32` octet count that includes the NUL,
        // then the UTF-8 bytes, then the NUL (XTypes 1.3 §7.4.1.1.2).
        fn string(b: &mut Vec<u8>, s: &str) {
            u32b(b, (s.len() + 1) as u32);
            b.extend_from_slice(s.as_bytes());
            b.push(0);
        }
        let mut body = Vec::new();
        body.push(event.facility);
        u32b(&mut body, event.severity as u32);
        i64b(&mut body, event.timestamp.sec);
        u32b(&mut body, event.timestamp.nanosec);
        string(&mut body, &event.hostname);
        string(&mut body, &event.hostip);
        string(&mut body, &event.appname);
        string(&mut body, &event.procid);
        string(&mut body, &event.msgid);
        string(&mut body, &event.message);
        // structured_data: one entry whose value is a one-element sequence.
        // The sequence's content is its element count then the elements (each
        // a final NameValuePair); the map's content is its entry count, the
        // key and the delimited sequence.
        let mut seq_content = Vec::new();
        u32b(&mut seq_content, event.structured_data["DDS"].len() as u32);
        for pair in &event.structured_data["DDS"] {
            string(&mut seq_content, &pair.name);
            string(&mut seq_content, &pair.value);
        }
        let mut map_content = Vec::new();
        u32b(&mut map_content, event.structured_data.len() as u32);
        string(&mut map_content, "DDS");
        u32b(&mut map_content, seq_content.len() as u32); // sequence DHEADER
        map_content.extend_from_slice(&seq_content);

        // The struct content is the members plus the delimited map.
        let mut content = body;
        u32b(&mut content, map_content.len() as u32); // map DHEADER
        content.extend_from_slice(&map_content);

        let mut out = Vec::new();
        u32b(&mut out, content.len() as u32); // outer DHEADER
        out.extend_from_slice(&content);
        out
    }

    #[test]
    fn the_v2_codec_writes_the_idl_octets() {
        let event = sample();
        let expected = hand_built_v2(&event);
        let mut got = Vec::new();
        event.encode(&mut got).unwrap();
        assert_eq!(
            got, expected,
            "encoder disagrees with the hand-built IDL bytes"
        );
    }

    #[test]
    fn the_v2_codec_round_trips() {
        let event = sample();
        let mut bytes = Vec::new();
        event.encode(&mut bytes).unwrap();
        assert_eq!(
            sec_log::BuiltinLoggingTypeV2::decode(&bytes).unwrap(),
            event
        );
    }

    #[test]
    fn the_type_names_are_the_idl_names() {
        assert_eq!(
            <sec_log::BuiltinLoggingTypeV2 as DdsType>::TYPE_NAME,
            "BuiltinLoggingTypeV2"
        );
        assert_eq!(
            <sec_log::BuiltinLoggingType as DdsType>::TYPE_NAME,
            "BuiltinLoggingType"
        );
        assert_eq!(
            <sec_log::BuiltinLoggingTypeV2 as DdsType>::EXTENSIBILITY,
            Extensibility::Appendable
        );
        assert_eq!(
            <sec_log::BuiltinLoggingType as DdsType>::EXTENSIBILITY,
            Extensibility::Appendable
        );
    }

    #[test]
    fn an_unknown_severity_is_refused_not_guessed() {
        let event = sample();
        let mut bytes = Vec::new();
        event.encode(&mut bytes).unwrap();
        // Severity is the second member: one octet, then padding, then the
        // `uint32` at octets 8..12 (after the four-octet DHEADER).
        bytes[4 + 4..4 + 8].copy_from_slice(&99u32.to_le_bytes());
        let err = sec_log::BuiltinLoggingTypeV2::decode(&bytes).unwrap_err();
        assert!(matches!(err, DecodeError::Invalid { .. }), "{err:?}");
    }

    #[test]
    fn the_publisher_inserts_the_domain_id_pair() {
        let mut event = sample();
        add_domain_id(&mut event.structured_data, 7);
        let dds = event.structured_data.get("DDS").expect("DDS");
        assert!(dds.iter().any(|p| p.name == "domain_id" && p.value == "7"));
        // A second call overwrites rather than duplicating the pair.
        add_domain_id(&mut event.structured_data, 9);
        let dds = event.structured_data.get("DDS").expect("DDS");
        assert_eq!(dds.iter().filter(|p| p.name == "domain_id").count(), 1);
        assert!(dds.iter().any(|p| p.name == "domain_id" && p.value == "9"));
    }

    #[test]
    fn the_legacy_codec_round_trips_with_a_32_bit_second() {
        let event = sec_log::BuiltinLoggingType {
            facility: sec_log::FACILITY_SEC_AUTH,
            severity: LoggingLevel::Warning,
            timestamp: sec_log::LegacyTime {
                sec: 1_700_000_000,
                nanosec: 3,
            },
            hostname: String::from("h"),
            hostip: String::from("i"),
            appname: String::from("a"),
            procid: String::from("p"),
            msgid: String::from("m"),
            message: String::from("x"),
            structured_data: BTreeMap::new(),
        };
        let mut bytes = Vec::new();
        event.encode(&mut bytes).unwrap();
        assert_eq!(sec_log::BuiltinLoggingType::decode(&bytes).unwrap(), event);
    }

    #[test]
    fn a_reader_skips_unknown_trailing_member_octets() {
        // An `@appendable` writer one version ahead put four extra octets
        // inside the record; the reader must step over them rather than read
        // them as the next member.
        let event = sample();
        let mut encoded = Vec::new();
        event.encode(&mut encoded).unwrap();
        let content = &encoded[4..];
        let mut extended = Vec::new();
        extended.extend_from_slice(&(content.len() as u32 + 4).to_le_bytes());
        extended.extend_from_slice(content);
        extended.extend_from_slice(&[0xAB; 4]);
        assert_eq!(
            sec_log::BuiltinLoggingTypeV2::decode(&extended).unwrap(),
            event
        );
    }
}
