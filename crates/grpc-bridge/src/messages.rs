// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The protobuf messages of the generated topic services
//! (`zerodds-grpc-bridge-1.0` §4.2): `Sample`, `PublishAck`,
//! `SubscribeReq`, `CatalogReq`, `CatalogResp` and `TopicEntry`.
//!
//! Hand-written proto3 wire format (Protocol Buffers Encoding: varint,
//! LEN): a field with its default value is not written, a decoder skips
//! fields it does not know and fields whose wire type is not the one the
//! field is declared with (proto3 parsers treat both as unknown fields),
//! and a later occurrence of a scalar field wins over an earlier one. A
//! truncated field, a varint above ten octets, a length beyond the message
//! and the reserved wire types 3, 4, 6 and 7 are a [`DecodeError`].

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A message that is not valid proto3 wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// A varint runs past the end of the message or beyond ten octets.
    Varint,
    /// A length-delimited field reaches beyond the message.
    Length,
    /// A fixed32/fixed64 field reaches beyond the message.
    Truncated,
    /// Wire type 3, 4 (groups), 6 or 7.
    WireType(u8),
    /// Field number 0.
    FieldZero,
    /// A `string` field that is not UTF-8.
    Utf8,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Varint => f.write_str("a varint runs past the message or beyond ten octets"),
            Self::Length => f.write_str("a length-delimited field reaches beyond the message"),
            Self::Truncated => f.write_str("a fixed-size field reaches beyond the message"),
            Self::WireType(t) => write!(f, "wire type {t} is not one proto3 writes"),
            Self::FieldZero => f.write_str("field number 0"),
            Self::Utf8 => f.write_str("a string field is not UTF-8"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

/// `Sample.flags` bit 0: the sample is a dispose of the instance named by
/// `key_hash` (§4.2 "dispose-bit").
pub const FLAG_DISPOSE: u32 = 0x1;
/// `Sample.flags` bit 1: the sample is an unregister of the instance named
/// by `key_hash`.
pub const FLAG_UNREGISTER: u32 = 0x2;
/// The flags this version of the bridge knows; a `Publish` with another bit
/// set is refused.
pub const KNOWN_FLAGS: u32 = FLAG_DISPOSE | FLAG_UNREGISTER;

/// `message Sample` (§4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sample {
    /// Field 1: the CDR bytes including the encapsulation header (§4.4).
    pub cdr_payload: Vec<u8>,
    /// Field 2: [`FLAG_DISPOSE`], [`FLAG_UNREGISTER`].
    pub flags: u32,
    /// Field 3: the 16-octet KeyHash of the instance (DDSI-RTPS 2.5
    /// §9.6.4.8), empty for a keyless topic or an unknown key.
    pub key_hash: Vec<u8>,
    /// Field 4: the source timestamp in nanoseconds since the Unix epoch;
    /// 0 is "none".
    pub source_ts_ns: u64,
    /// Field 5: the DDS type name.
    pub type_name: String,
    /// Field 6: the DDS topic name.
    pub topic_name: String,
}

/// `message PublishAck` (§4.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PublishAck {
    /// Field 1: the RTPS sequence number of the last sample written.
    pub sequence_number: u64,
    /// Field 2: when the bridge acknowledged, nanoseconds since the Unix
    /// epoch.
    pub acked_at_ns: u64,
}

/// `message SubscribeReq` (§4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubscribeReq {
    /// Field 1: the partition expression of the stream's DataReader; empty
    /// is the default partition.
    pub partition_filter: String,
    /// Field 2: dispose and unregister samples go out as `Sample`s with
    /// their flag set.
    pub include_dispose: bool,
    /// Field 3: how many retained samples the stream starts with; 0 is no
    /// burst.
    pub initial_burst: u32,
}

/// `message TopicEntry` (§4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicEntry {
    /// Field 1.
    pub dds_name: String,
    /// Field 2.
    pub dds_type: String,
    /// Field 3: the fully-qualified service name.
    pub grpc_service: String,
    /// Field 4: where the IDL of the type is served (§5.2).
    pub idl_url: String,
}

/// `message CatalogResp` (§4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogResp {
    /// Field 1.
    pub topics: Vec<TopicEntry>,
}

/// One field of a message: its number and its value.
pub(crate) enum Field<'a> {
    Varint(u64),
    Len(&'a [u8]),
    /// fixed32 and fixed64; no field of §4.2 uses them.
    Fixed,
}

/// The fields of `msg` in order.
pub(crate) fn fields(
    msg: &[u8],
) -> impl Iterator<Item = Result<(u32, Field<'_>), DecodeError>> + '_ {
    let mut at = 0usize;
    core::iter::from_fn(move || {
        if at >= msg.len() {
            return None;
        }
        let next = (|| {
            let (key, n) = read_varint(&msg[at..])?;
            at += n;
            let number = u32::try_from(key >> 3).map_err(|_| DecodeError::FieldZero)?;
            if number == 0 {
                return Err(DecodeError::FieldZero);
            }
            let value = match (key & 7) as u8 {
                0 => {
                    let (v, n) = read_varint(&msg[at..])?;
                    at += n;
                    Field::Varint(v)
                }
                1 | 5 => {
                    let size = if key & 7 == 1 { 8 } else { 4 };
                    if msg.len() - at < size {
                        return Err(DecodeError::Truncated);
                    }
                    at += size;
                    Field::Fixed
                }
                2 => {
                    let (len, n) = read_varint(&msg[at..])?;
                    at += n;
                    let len = usize::try_from(len).map_err(|_| DecodeError::Length)?;
                    if msg.len() - at < len {
                        return Err(DecodeError::Length);
                    }
                    let v = &msg[at..at + len];
                    at += len;
                    Field::Len(v)
                }
                other => return Err(DecodeError::WireType(other)),
            };
            Ok((number, value))
        })();
        if next.is_err() {
            // Nothing after a malformed field can be read.
            at = msg.len();
        }
        Some(next)
    })
}

fn read_varint(b: &[u8]) -> Result<(u64, usize), DecodeError> {
    let mut value = 0u64;
    for (i, &byte) in b.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
    }
    Err(DecodeError::Varint)
}

pub(crate) fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub(crate) fn put_uint(out: &mut Vec<u8>, number: u32, v: u64) {
    if v != 0 {
        put_varint(out, u64::from(number) << 3);
        put_varint(out, v);
    }
}

pub(crate) fn put_bytes(out: &mut Vec<u8>, number: u32, v: &[u8]) {
    if !v.is_empty() {
        put_varint(out, (u64::from(number) << 3) | 2);
        put_varint(out, v.len() as u64);
        out.extend_from_slice(v);
    }
}

pub(crate) fn utf8(v: &[u8]) -> Result<String, DecodeError> {
    core::str::from_utf8(v)
        .map(String::from)
        .map_err(|_| DecodeError::Utf8)
}

/// proto3 `uint32` from a varint: the low 32 bits (Protocol Buffers
/// Encoding, "a uint32 truncates").
#[allow(clippy::cast_possible_truncation)]
fn low32(v: u64) -> u32 {
    v as u32
}

impl Sample {
    /// The wire form of the message.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.cdr_payload.len() + 64);
        put_bytes(&mut out, 1, &self.cdr_payload);
        put_uint(&mut out, 2, u64::from(self.flags));
        put_bytes(&mut out, 3, &self.key_hash);
        put_uint(&mut out, 4, self.source_ts_ns);
        put_bytes(&mut out, 5, self.type_name.as_bytes());
        put_bytes(&mut out, 6, self.topic_name.as_bytes());
        out
    }

    /// The message of `msg`.
    ///
    /// # Errors
    /// [`DecodeError`] for a message that is not proto3 wire format.
    pub fn decode(msg: &[u8]) -> Result<Self, DecodeError> {
        let mut s = Self::default();
        for field in fields(msg) {
            match field? {
                (1, Field::Len(v)) => s.cdr_payload = v.to_vec(),
                (2, Field::Varint(v)) => s.flags = low32(v),
                (3, Field::Len(v)) => s.key_hash = v.to_vec(),
                (4, Field::Varint(v)) => s.source_ts_ns = v,
                (5, Field::Len(v)) => s.type_name = utf8(v)?,
                (6, Field::Len(v)) => s.topic_name = utf8(v)?,
                _ => {}
            }
        }
        Ok(s)
    }
}

impl PublishAck {
    /// The wire form of the message.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(22);
        put_uint(&mut out, 1, self.sequence_number);
        put_uint(&mut out, 2, self.acked_at_ns);
        out
    }

    /// The message of `msg`.
    ///
    /// # Errors
    /// [`DecodeError`] for a message that is not proto3 wire format.
    pub fn decode(msg: &[u8]) -> Result<Self, DecodeError> {
        let mut a = Self::default();
        for field in fields(msg) {
            match field? {
                (1, Field::Varint(v)) => a.sequence_number = v,
                (2, Field::Varint(v)) => a.acked_at_ns = v,
                _ => {}
            }
        }
        Ok(a)
    }
}

impl SubscribeReq {
    /// The wire form of the message.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_bytes(&mut out, 1, self.partition_filter.as_bytes());
        put_uint(&mut out, 2, u64::from(self.include_dispose));
        put_uint(&mut out, 3, u64::from(self.initial_burst));
        out
    }

    /// The message of `msg`.
    ///
    /// # Errors
    /// [`DecodeError`] for a message that is not proto3 wire format.
    pub fn decode(msg: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Self::default();
        for field in fields(msg) {
            match field? {
                (1, Field::Len(v)) => r.partition_filter = utf8(v)?,
                (2, Field::Varint(v)) => r.include_dispose = v != 0,
                (3, Field::Varint(v)) => r.initial_burst = low32(v),
                _ => {}
            }
        }
        Ok(r)
    }
}

/// `message CatalogReq {}`: every well-formed message is one.
///
/// # Errors
/// [`DecodeError`] for a message that is not proto3 wire format.
pub fn decode_catalog_req(msg: &[u8]) -> Result<(), DecodeError> {
    for field in fields(msg) {
        field?;
    }
    Ok(())
}

impl TopicEntry {
    fn encode_into(&self, out: &mut Vec<u8>) {
        put_bytes(out, 1, self.dds_name.as_bytes());
        put_bytes(out, 2, self.dds_type.as_bytes());
        put_bytes(out, 3, self.grpc_service.as_bytes());
        put_bytes(out, 4, self.idl_url.as_bytes());
    }

    fn decode(msg: &[u8]) -> Result<Self, DecodeError> {
        let mut t = Self::default();
        for field in fields(msg) {
            match field? {
                (1, Field::Len(v)) => t.dds_name = utf8(v)?,
                (2, Field::Len(v)) => t.dds_type = utf8(v)?,
                (3, Field::Len(v)) => t.grpc_service = utf8(v)?,
                (4, Field::Len(v)) => t.idl_url = utf8(v)?,
                _ => {}
            }
        }
        Ok(t)
    }
}

impl CatalogResp {
    /// The wire form of the message. An entry with every field empty is
    /// written as an empty element, so the count of entries survives.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for t in &self.topics {
            let mut entry = Vec::new();
            t.encode_into(&mut entry);
            put_varint(&mut out, (1 << 3) | 2);
            put_varint(&mut out, entry.len() as u64);
            out.extend_from_slice(&entry);
        }
        out
    }

    /// The message of `msg`.
    ///
    /// # Errors
    /// [`DecodeError`] for a message that is not proto3 wire format.
    pub fn decode(msg: &[u8]) -> Result<Self, DecodeError> {
        let mut c = Self::default();
        for field in fields(msg) {
            if let (1, Field::Len(v)) = field? {
                c.topics.push(TopicEntry::decode(v)?);
            }
        }
        Ok(c)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// §4.2: each of the six fields of `Sample` goes out under its number
    /// and comes back; a default field is not written.
    #[test]
    fn grpcbridge_4_2_sample_carries_its_six_fields() {
        let s = Sample {
            cdr_payload: alloc::vec![0, 1, 0, 0, 7],
            flags: FLAG_DISPOSE,
            key_hash: alloc::vec![9; 16],
            source_ts_ns: 1_700_000_000_123_456_789,
            type_name: "Chat::Message".into(),
            topic_name: "Chat/Room".into(),
        };
        let wire = s.encode();
        assert_eq!(&wire[..7], &[0x0a, 5, 0, 1, 0, 0, 7]);
        assert_eq!(&wire[7..9], &[0x10, 1]);
        assert_eq!(wire[9], 0x1a);
        assert_eq!(wire[27], 0x20);
        assert!(wire.ends_with(b"\x32\x09Chat/Room"));
        assert_eq!(Sample::decode(&wire).unwrap(), s);
        assert!(Sample::default().encode().is_empty());
        assert_eq!(
            Sample::decode(&[0x0a, 3, 1, 2, 3]).unwrap().cdr_payload,
            [1, 2, 3]
        );
    }

    /// §4.2: `PublishAck` names the sequence number and the time.
    #[test]
    fn grpcbridge_4_2_publish_ack_carries_sequence_number_and_time() {
        let a = PublishAck {
            sequence_number: 300,
            acked_at_ns: 5,
        };
        assert_eq!(a.encode(), [0x08, 0xac, 0x02, 0x10, 0x05]);
        assert_eq!(PublishAck::decode(&a.encode()).unwrap(), a);
    }

    /// §4.2: the three fields of `SubscribeReq` are read.
    #[test]
    fn grpcbridge_4_2_subscribe_req_is_read() {
        let r = SubscribeReq::decode(&[0x0a, 2, b'a', b'*', 0x10, 1, 0x18, 4]).unwrap();
        assert_eq!(
            r,
            SubscribeReq {
                partition_filter: "a*".into(),
                include_dispose: true,
                initial_burst: 4,
            }
        );
        assert_eq!(SubscribeReq::decode(&r.encode()).unwrap(), r);
        assert_eq!(SubscribeReq::decode(&[]).unwrap(), SubscribeReq::default());
    }

    /// §4.2: `CatalogResp` is a repeated `TopicEntry`; an empty entry keeps
    /// its place.
    #[test]
    fn grpcbridge_4_2_catalog_resp_lists_topic_entries() {
        let c = CatalogResp {
            topics: alloc::vec![
                TopicEntry {
                    dds_name: "Chat::Message".into(),
                    dds_type: "Chat::Message".into(),
                    grpc_service: "zerodds.chat.v1.ChatMessageStream".into(),
                    idl_url: "/idl/Chat::Message".into(),
                },
                TopicEntry::default(),
            ],
        };
        let wire = c.encode();
        assert_eq!(wire[0], 0x0a);
        assert!(wire.ends_with(&[0x0a, 0]));
        assert_eq!(CatalogResp::decode(&wire).unwrap(), c);
        assert!(decode_catalog_req(&[]).is_ok());
        assert!(decode_catalog_req(&[0x08]).is_err());
    }

    /// Unknown fields, and known fields under another wire type, are
    /// skipped; the last occurrence of a scalar wins.
    #[test]
    fn grpcbridge_4_2_unknown_fields_are_skipped() {
        let msg = [
            0x78, 0x05, // field 15 varint
            0x0a, 0x01, 0x07, // cdr_payload
            0x12, 0x01, 0x01, // field 2 as LEN: not `flags`
            0x25, 1, 2, 3, 4, // field 4 as fixed32: not `source_ts_ns`
            0x10, 0x01, 0x10, 0x02, // flags twice
        ];
        let s = Sample::decode(&msg).unwrap();
        assert_eq!(s.cdr_payload, [7]);
        assert_eq!(s.flags, 2);
        assert_eq!(s.source_ts_ns, 0);
    }

    /// A message that is not proto3 wire format is a decode error.
    #[test]
    fn grpcbridge_4_2_malformed_messages_are_errors() {
        assert_eq!(Sample::decode(&[0x0a, 5, 1]), Err(DecodeError::Length));
        assert_eq!(Sample::decode(&[0x10]), Err(DecodeError::Varint));
        let mut eleven = alloc::vec![0x10];
        eleven.extend_from_slice(&[0xff; 10]);
        eleven.push(0x01);
        assert_eq!(Sample::decode(&eleven), Err(DecodeError::Varint));
        assert_eq!(Sample::decode(&[0x0b]), Err(DecodeError::WireType(3)));
        assert_eq!(Sample::decode(&[0x00, 0x01]), Err(DecodeError::FieldZero));
        assert_eq!(Sample::decode(&[0x2a, 1, 0xff]), Err(DecodeError::Utf8));
        assert_eq!(Sample::decode(&[0x21, 1, 2]), Err(DecodeError::Truncated));
    }
}
