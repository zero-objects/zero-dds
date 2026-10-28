// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE object-repr types from Spec §7.7.7 - §7.7.13.
//!
//! This module implements the wire structures that occur in the
//! CREATE/STATUS/GET_INFO/INFO submessage bodies:
//!
//! * **§7.7.7** [`ResultStatusCode`] — 11 spec-conformant status codes
//!   plus the generic `ResultStatus` (status_code + implementation_status).
//! * **§7.7.8** [`BaseObjectRequest`] — RequestId(2) + ObjectId(2).
//! * **§7.7.9** [`BaseObjectReply`] — BaseObjectRequest + ResultStatus.
//! * **§7.7.10** [`RelatedObjectRequest`] — a `typedef` for
//!   [`BaseObjectRequest`], not a distinct shape (see its own doc for the
//!   citation).
//! * **§7.7.12** [`ActivityInfoVariant`] — discriminated union with
//!   AgentActivity or DataReader/Writer activity.
//! * **§7.7.13** [`ObjectInfo`] — compound type for the GET_INFO reply.
//!
//! `InfoMask`, `ActivityInfoVariant` and `ObjectInfo` follow Annex A: XCDR2
//! with scalars aligned to their size (at most 4), counted from the start of
//! the payload, the byte order of the submessage's E flag (§8.3.4.2), a DHEADER
//! in front of the `@extensibility(APPENDABLE)` activity structs, and
//! `@optional` members as a presence octet and the member. The functions
//! with `_into` and `_at` take the position in the payload, as an `INFO`
//! has a `BaseObjectReply` in front of its `ObjectInfo`.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{
    Endianness, UnannotatedTypes, begin_unannotated, end_unannotated, read_dheader,
    unannotated_types,
};
use crate::error::XrceError;
use crate::handshake::{AgentRepresentation, ClientRepresentation};
use crate::object_id::ObjectId;
use crate::object_kind::ObjectKind;
use crate::object_repr::{ObjectVariant, ObjectVariantExtra};
use crate::transport_locator::{TransportLocator, decode_locator_seq, encode_locator_seq};

// ============================================================================
// §7.7.7 ResultStatus
// ============================================================================

/// Spec §7.7.7 `StatusValue` (`@bit_bound(8) enum`, Annex A IDL Types):
/// 10 status codes (`STATUS_OK`/`STATUS_OK_MATCHED` + 8 error classes).
///
/// Each code is 1 byte; values other than the 10 below are not in the spec
/// (there is no `STATUS_ERR_UNSUPPORTED` — the `StatusValue` enum literal
/// list stops at `@value(0x87) STATUS_ERR_RESOURCES`, and a byte this crate
/// does not reject as `0x88` would desynchronize from a real eProsima
/// Agent/Client, which never sends or expects one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResultStatusCode {
    /// `STATUS_OK` — operation successful.
    Ok = 0x00,
    /// `STATUS_OK_MATCHED` — operation successful, matched an existing object.
    OkMatched = 0x01,
    /// `STATUS_ERR_DDS_ERROR` — generic DDS error.
    DdsError = 0x80,
    /// `STATUS_ERR_MISMATCH` — cert/type/QoS mismatch between peers.
    Mismatch = 0x81,
    /// `STATUS_ERR_ALREADY_EXISTS` — an object with this ID already exists.
    AlreadyExists = 0x82,
    /// `STATUS_ERR_DENIED` — access control rejected the operation.
    Denied = 0x83,
    /// `STATUS_ERR_UNKNOWN_REFERENCE` — referenced resource is missing.
    UnknownReference = 0x84,
    /// `STATUS_ERR_INVALID_DATA` — wire body not parseable.
    InvalidData = 0x85,
    /// `STATUS_ERR_INCOMPATIBLE` — spec conflict (version etc.).
    Incompatible = 0x86,
    /// `STATUS_ERR_RESOURCES` — memory/slots exhausted.
    Resources = 0x87,
}

impl ResultStatusCode {
    /// Wire value (1 byte).
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Converts a byte. Values outside the spec list are
    /// rejected.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn from_u8(byte: u8) -> Result<Self, XrceError> {
        match byte {
            0x00 => Ok(Self::Ok),
            0x01 => Ok(Self::OkMatched),
            0x80 => Ok(Self::DdsError),
            0x81 => Ok(Self::Mismatch),
            0x82 => Ok(Self::AlreadyExists),
            0x83 => Ok(Self::Denied),
            0x84 => Ok(Self::UnknownReference),
            0x85 => Ok(Self::InvalidData),
            0x86 => Ok(Self::Incompatible),
            0x87 => Ok(Self::Resources),
            _ => Err(XrceError::ValueOutOfRange {
                message: "ResultStatusCode: unknown wire value",
            }),
        }
    }

    /// `true` if the status is `Ok` or `OkMatched`.
    #[must_use]
    pub fn is_success(self) -> bool {
        matches!(self, Self::Ok | Self::OkMatched)
    }
}

/// Spec §7.7.7: `ResultStatus { status_code, implementation_status }`.
///
/// `implementation_status` is vendor-specific and is passed through
/// transparently as 1 byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultStatus {
    /// Status code from Spec Tab.7.7.7.
    pub status_code: ResultStatusCode,
    /// Vendor-specific status (1 byte, opaque).
    pub implementation_status: u8,
}

impl ResultStatus {
    /// Size of the members: status + implementation_status (2 bytes). With
    /// [`UnannotatedTypes::Appendable`] a DHEADER of 4 bytes comes in front.
    pub const WIRE_SIZE: usize = 2;

    /// Success status (`Ok`).
    #[must_use]
    pub fn ok() -> Self {
        Self {
            status_code: ResultStatusCode::Ok,
            implementation_status: 0,
        }
    }

    /// Appends the struct to `out`, which starts the payload: `ResultStatus`
    /// carries no `@extensibility` (Annex A), so it is `APPENDABLE` with a
    /// DHEADER (4-aligned from the start of the payload) unless
    /// [`UnannotatedTypes::Final`] is set.
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) {
        let members = begin_unannotated(out);
        out.push(self.status_code.as_u8());
        out.push(self.implementation_status);
        end_unannotated(out, members, e);
    }

    /// Encodes a `ResultStatus` that starts its payload, in the byte order `e`.
    #[must_use]
    pub fn encode_with(&self, e: Endianness) -> Vec<u8> {
        let mut out = Vec::with_capacity(6);
        self.encode_into(&mut out, e);
        out
    }

    /// Encodes a `ResultStatus` that starts its payload, little-endian.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(Endianness::Little)
    }

    /// Decodes the `ResultStatus` that starts at `pos` of `bytes`, which start
    /// the payload. Returns it and the position after it. The form the
    /// setting names is tried first, the other one when that does not parse.
    ///
    /// # Errors
    /// `UnexpectedEof` on too few bytes; `ValueOutOfRange` on an unknown
    /// status code.
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let members = |at: usize| -> Result<Self, XrceError> {
            let m = bytes
                .get(at..at + Self::WIRE_SIZE)
                .ok_or(XrceError::UnexpectedEof {
                    needed: at + Self::WIRE_SIZE,
                    offset: bytes.len(),
                })?;
            Ok(Self {
                status_code: ResultStatusCode::from_u8(m[0])?,
                implementation_status: m[1],
            })
        };
        let appendable = || -> Result<(Self, usize), XrceError> {
            let (start, end) = read_dheader(bytes, pos, e).ok_or(XrceError::UnexpectedEof {
                needed: pos.div_ceil(4) * 4 + 4 + Self::WIRE_SIZE,
                offset: bytes.len(),
            })?;
            if end - start < Self::WIRE_SIZE {
                return Err(XrceError::ValueOutOfRange {
                    message: "ResultStatus: DHEADER shorter than the members",
                });
            }
            Ok((members(start)?, end))
        };
        let direct = || members(pos).map(|v| (v, pos + Self::WIRE_SIZE));
        match unannotated_types() {
            UnannotatedTypes::Appendable => appendable().or_else(|_| direct()),
            UnannotatedTypes::Final => direct().or_else(|_| appendable()),
        }
    }

    /// Decodes a `ResultStatus` that starts its payload, in the byte order `e`.
    ///
    /// # Errors
    /// As [`Self::decode_at`].
    pub fn decode_with(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_at(bytes, 0, e)
    }

    /// Decodes a `ResultStatus` that starts its payload, little-endian.
    ///
    /// # Errors
    /// `UnexpectedEof` on too few bytes; `ValueOutOfRange` on an
    /// unknown status code.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        Self::decode_with(bytes, Endianness::Little)
    }
}

// ============================================================================
// §7.7.8 BaseObjectRequest
// ============================================================================

/// Spec §7.7.8: `BaseObjectRequest { RequestId(2 octets), ObjectId(2 octets) }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BaseObjectRequest {
    /// Request identifier (2 octets, unique per stream).
    pub request_id: [u8; 2],
    /// Target object of the operation.
    pub object_id: ObjectId,
}

impl BaseObjectRequest {
    /// Wire size = 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Encode as 4 bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let oid = self.object_id.to_bytes();
        [self.request_id[0], self.request_id[1], oid[0], oid[1]]
    }

    /// Decode from 4 bytes.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` on ObjectId problems.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let request_id = [bytes[0], bytes[1]];
        let object_id = ObjectId::from_bytes(&bytes[2..4])?;
        Ok((
            Self {
                request_id,
                object_id,
            },
            Self::WIRE_SIZE,
        ))
    }
}

// ============================================================================
// §7.7.9 BaseObjectReply
// ============================================================================

/// Spec §7.7.9: `BaseObjectReply { related_request: BaseObjectRequest,
/// result: ResultStatus }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseObjectReply {
    /// Original request the reply refers to.
    pub related_request: BaseObjectRequest,
    /// Result status of the operation.
    pub result: ResultStatus,
}

impl BaseObjectReply {
    /// Size of the reply in the `FINAL` form of its `ResultStatus`: 4 + 2.
    /// In the `APPENDABLE` form the `ResultStatus` has a DHEADER (4 + 4 + 2).
    pub const WIRE_SIZE: usize = BaseObjectRequest::WIRE_SIZE + ResultStatus::WIRE_SIZE;

    /// Appends the reply to `out`, which starts the payload.
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) {
        out.extend_from_slice(&self.related_request.encode());
        self.result.encode_into(out, e);
    }

    /// Encodes a reply that starts its payload, in the byte order `e`.
    #[must_use]
    pub fn encode_with(&self, e: Endianness) -> Vec<u8> {
        let mut out = Vec::with_capacity(10);
        self.encode_into(&mut out, e);
        out
    }

    /// Encodes a reply that starts its payload, little-endian.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(Endianness::Little)
    }

    /// Decodes the reply that starts at `pos` of `bytes`, which start the
    /// payload. Returns it and the position after it.
    ///
    /// # Errors
    /// As [`BaseObjectRequest::decode`] / [`ResultStatus::decode_at`].
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let rest = bytes.get(pos..).ok_or(XrceError::UnexpectedEof {
            needed: pos,
            offset: bytes.len(),
        })?;
        let (related_request, n1) = BaseObjectRequest::decode(rest)?;
        let (result, end) = ResultStatus::decode_at(bytes, pos + n1, e)?;
        Ok((
            Self {
                related_request,
                result,
            },
            end,
        ))
    }

    /// Decodes a reply that starts its payload, in the byte order `e`.
    ///
    /// # Errors
    /// As [`Self::decode_at`].
    pub fn decode_with(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_at(bytes, 0, e)
    }

    /// Decodes a reply that starts its payload, little-endian.
    ///
    /// # Errors
    /// As [`BaseObjectRequest::decode`] / [`ResultStatus::decode`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        Self::decode_with(bytes, Endianness::Little)
    }
}

// ============================================================================
// §7.7.10 RelatedObjectRequest
// ============================================================================

/// Spec §7.7.10: "the RelatedObjectRequest type is used to hold the common
/// parameters of the messages sent from the XRCE Agent back to the Client
/// that are indirectly related to a prior request \[...\] For example, DATA
/// messages that relate to a previous read operation." Its Annex A
/// definition is `typedef RelatedObjectRequest BaseObjectRequest;`, and the
/// prose is explicit that "the interpretation is the same as for the
/// related_request that appears in the BaseObjectReply" — i.e. plain
/// `request_id` + `object_id`, 4 bytes, **not** a second `ObjectId` field.
/// (An earlier version of this type modeled a 6-byte
/// `{base, related_object}` shape, guessing "related" meant "references a
/// second object"; nothing in this crate ever used it, and it does not
/// match either the spec text or the 4-byte `DATA_Payload_*` bodies
/// measured against real eProsima captures in `submessages/data.rs`.)
pub type RelatedObjectRequest = BaseObjectRequest;

// ============================================================================
// §7.7.12 InfoMask, ActivityInfoVariant
// ============================================================================

fn pad_to(out: &mut Vec<u8>, n: usize) {
    while out.len() % n != 0 {
        out.push(0);
    }
}

fn align_pos(pos: usize, n: usize) -> usize {
    pos.div_ceil(n) * n
}

fn put16(out: &mut Vec<u8>, v: u16, e: Endianness) {
    pad_to(out, 2);
    out.extend_from_slice(&match e {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    });
}

/// An 8-octet primitive: XCDR2 aligns it to 4 (its maximum alignment).
fn put64(out: &mut Vec<u8>, v: u64, e: Endianness) {
    pad_to(out, 4);
    out.extend_from_slice(&match e {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    });
}

fn take(bytes: &[u8], pos: usize, n: usize) -> Result<&[u8], XrceError> {
    bytes.get(pos..pos + n).ok_or(XrceError::UnexpectedEof {
        needed: pos + n,
        offset: bytes.len(),
    })
}

fn get16(bytes: &[u8], pos: usize, e: Endianness) -> Result<(u16, usize), XrceError> {
    let at = align_pos(pos, 2);
    let b = take(bytes, at, 2)?;
    let raw = [b[0], b[1]];
    Ok((
        match e {
            Endianness::Little => u16::from_le_bytes(raw),
            Endianness::Big => u16::from_be_bytes(raw),
        },
        at + 2,
    ))
}

fn get32(bytes: &[u8], pos: usize, e: Endianness) -> Result<(u32, usize), XrceError> {
    let at = align_pos(pos, 4);
    let b = take(bytes, at, 4)?;
    let raw = [b[0], b[1], b[2], b[3]];
    Ok((
        match e {
            Endianness::Little => u32::from_le_bytes(raw),
            Endianness::Big => u32::from_be_bytes(raw),
        },
        at + 4,
    ))
}

fn get64(bytes: &[u8], pos: usize, e: Endianness) -> Result<(u64, usize), XrceError> {
    let at = align_pos(pos, 4);
    let b = take(bytes, at, 8)?;
    let mut raw = [0u8; 8];
    raw.copy_from_slice(b);
    Ok((
        match e {
            Endianness::Little => u64::from_le_bytes(raw),
            Endianness::Big => u64::from_be_bytes(raw),
        },
        at + 8,
    ))
}

/// Spec §7.7.12: `bitmask InfoMask { @position(0) INFO_CONFIGURATION,
/// @position(1) INFO_ACTIVITY; }` — what a `GET_INFO` asks for. Without a
/// `@bit_bound` the bitmask has 32 bits (XTypes 1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InfoMask(pub u32);

impl InfoMask {
    /// `INFO_CONFIGURATION`, position 0.
    pub const INFO_CONFIGURATION: u32 = 1 << 0;
    /// `INFO_ACTIVITY`, position 1.
    pub const INFO_ACTIVITY: u32 = 1 << 1;
    /// Both kinds of information.
    pub const ALL: Self = Self(Self::INFO_CONFIGURATION | Self::INFO_ACTIVITY);
    /// Wire size: an `unsigned long`.
    pub const WIRE_SIZE: usize = 4;

    /// Whether the configuration is asked for.
    #[must_use]
    pub fn configuration(self) -> bool {
        self.0 & Self::INFO_CONFIGURATION != 0
    }

    /// Whether the activity is asked for.
    #[must_use]
    pub fn activity(self) -> bool {
        self.0 & Self::INFO_ACTIVITY != 0
    }

    /// Encodes as 4 octets in the byte order `e`.
    #[must_use]
    pub fn encode(self, e: Endianness) -> [u8; 4] {
        match e {
            Endianness::Little => self.0.to_le_bytes(),
            Endianness::Big => self.0.to_be_bytes(),
        }
    }

    /// Decodes 4 octets in the byte order `e`.
    ///
    /// # Errors
    /// `UnexpectedEof` on fewer than 4 octets.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        let b = take(bytes, 0, Self::WIRE_SIZE)?;
        let raw = [b[0], b[1], b[2], b[3]];
        Ok((
            Self(match e {
                Endianness::Little => u32::from_le_bytes(raw),
                Endianness::Big => u32::from_be_bytes(raw),
            }),
            Self::WIRE_SIZE,
        ))
    }
}

/// Spec §7.7.12 / Annex A: `union ActivityInfoVariant switch (ObjectKind)`,
/// `@extensibility(FINAL)`; its members are `@extensibility(APPENDABLE)`
/// structs, so each carries a DHEADER (XCDR2) with the length of its
/// members.
///
/// * **Agent** — `AGENT_ActivityInfo { short availability;
///   TransportLocatorSeq address_seq; }`, discriminator `OBJK_AGENT`
///   (Annex A writes the case as `OBJECTID_AGENT`, a two-octet `ObjectId`
///   that is no value of `ObjectKind`).
/// * **DataReader** — `DATAREADER_ActivityInfo { short highest_acked_num; }`.
/// * **DataWriter** — `DATAWRITER_ActivityInfo { unsigned long long
///   sample_seq_num; short stream_seq_num; }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityInfoVariant {
    /// `AGENT_ActivityInfo`.
    Agent {
        /// `availability`.
        availability: i16,
        /// The transport addresses the agent can be reached at
        /// (§7.8.2.2: "to inform the XRCE Client of the transport addresses
        /// over which it can be reached and can receive calls to
        /// create_client").
        address_seq: Vec<TransportLocator>,
    },
    /// `DATAREADER_ActivityInfo`.
    DataReader {
        /// `highest_acked_num`.
        highest_acked_num: i16,
    },
    /// `DATAWRITER_ActivityInfo`.
    DataWriter {
        /// `sample_seq_num`.
        sample_seq_num: u64,
        /// `stream_seq_num`.
        stream_seq_num: i16,
    },
}

impl ActivityInfoVariant {
    /// Discriminator — the `ObjectKind` the activity is of.
    #[must_use]
    pub fn discriminator(&self) -> u8 {
        match self {
            Self::Agent { .. } => 0x0D,      // OBJK_AGENT
            Self::DataReader { .. } => 0x06, // OBJK_DATAREADER
            Self::DataWriter { .. } => 0x05, // OBJK_DATAWRITER
        }
    }

    /// Encodes a variant that starts its payload, little-endian.
    ///
    /// # Errors
    /// `PayloadTooLarge` for a string locator beyond the size of a
    /// submessage.
    pub fn encode(&self) -> Result<Vec<u8>, XrceError> {
        self.encode_with(Endianness::Little)
    }

    /// As [`Self::encode`], in the byte order of the submessage's E flag
    /// (DDS-XRCE 1.0 §8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::encode`].
    pub fn encode_with(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::with_capacity(16);
        self.encode_into(&mut out, e)?;
        Ok(out)
    }

    /// Appends the variant to `out`, which starts the payload: the discriminator
    /// octet, then the DHEADER (an `unsigned long` aligned to 4) and the
    /// members, aligned counted from `out[0]`.
    ///
    /// # Errors
    /// As [`Self::encode`].
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        out.push(self.discriminator());
        pad_to(out, 4);
        let dheader_at = out.len();
        out.extend_from_slice(&[0; 4]);
        let members_at = out.len();
        match self {
            Self::Agent {
                availability,
                address_seq,
            } => {
                put16(out, *availability as u16, e);
                encode_locator_seq(address_seq, out, e)?;
            }
            Self::DataReader { highest_acked_num } => put16(out, *highest_acked_num as u16, e),
            Self::DataWriter {
                sample_seq_num,
                stream_seq_num,
            } => {
                put64(out, *sample_seq_num, e);
                put16(out, *stream_seq_num as u16, e);
            }
        }
        let len =
            u32::try_from(out.len() - members_at).map_err(|_| XrceError::ValueOutOfRange {
                message: "ActivityInfo members exceed u32",
            })?;
        out[dheader_at..dheader_at + 4].copy_from_slice(&match e {
            Endianness::Little => len.to_le_bytes(),
            Endianness::Big => len.to_be_bytes(),
        });
        Ok(())
    }

    /// Decodes a variant that starts its payload, little-endian. Returns it
    /// and the bytes consumed.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` on an unknown discriminator.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        Self::decode_with(bytes, Endianness::Little)
    }

    /// As [`Self::decode`], in the byte order of the submessage's E flag
    /// (§8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_with(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_at(bytes, 0, e)
    }

    /// Decodes the variant that starts at `pos` of `bytes`, which starts the
    /// payload. Returns it and the position after it. Members behind the
    /// ones this knows (an appended member of a later version) are skipped
    /// by the DHEADER.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let disc = *bytes.get(pos).ok_or(XrceError::UnexpectedEof {
            needed: pos + 1,
            offset: bytes.len(),
        })?;
        let (len, members_at) = get32(bytes, pos + 1, e)?;
        let end = members_at
            .checked_add(len as usize)
            .filter(|end| *end <= bytes.len())
            .ok_or(XrceError::UnexpectedEof {
                needed: members_at.saturating_add(len as usize),
                offset: bytes.len(),
            })?;
        // Members are read inside the DHEADER's extent only.
        let body = &bytes[..end];
        let variant = match disc {
            0x0D => {
                let (availability, at) = get16(body, members_at, e)?;
                let (address_seq, _) = decode_locator_seq(body, at, e)?;
                Self::Agent {
                    availability: availability as i16,
                    address_seq,
                }
            }
            0x06 => {
                let (highest_acked_num, _) = get16(body, members_at, e)?;
                Self::DataReader {
                    highest_acked_num: highest_acked_num as i16,
                }
            }
            0x05 => {
                let (sample_seq_num, at) = get64(body, members_at, e)?;
                let (stream_seq_num, _) = get16(body, at, e)?;
                Self::DataWriter {
                    sample_seq_num,
                    stream_seq_num: stream_seq_num as i16,
                }
            }
            _ => {
                return Err(XrceError::ValueOutOfRange {
                    message: "ActivityInfoVariant: unknown discriminator",
                });
            }
        };
        Ok((variant, end))
    }
}

// ============================================================================
// §7.7.13 ObjectInfo
// ============================================================================

/// The `config` of an [`ObjectInfo`]: an `ObjectVariant` (Annex A, a union
/// over `ObjectKind`), which for the agent is its `AGENT_Representation`
/// (§7.8.2.2), and for any other object the representation it was created
/// with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectConfig {
    /// `case OBJK_AGENT: AGENT_Representation`.
    Agent(AgentRepresentation),
    /// `case OBJK_CLIENT: CLIENT_Representation`.
    Client(ClientRepresentation),
    /// Any other kind: the `..._Representation` of `kind`.
    Variant {
        /// The kind of the object.
        kind: ObjectKind,
        /// Its representation.
        variant: ObjectVariant,
        /// The trailing field of the kind (the domain of a participant, the
        /// container of an endpoint).
        extra: ObjectVariantExtra,
    },
}

impl ObjectConfig {
    fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        match self {
            Self::Agent(rep) => {
                out.push(ObjectKind::Agent.to_u8());
                rep.encode_into(out, e)
            }
            Self::Client(rep) => {
                out.push(ObjectKind::Client.to_u8());
                rep.encode_into(out, e)
            }
            Self::Variant {
                kind,
                variant,
                extra,
            } => variant.encode_for_kind_into(out, *kind, *extra, e),
        }
    }

    fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let kind_byte = *bytes.get(pos).ok_or(XrceError::UnexpectedEof {
            needed: pos + 1,
            offset: bytes.len(),
        })?;
        match ObjectKind::from_u8(kind_byte)? {
            ObjectKind::Agent => {
                let (rep, end) = AgentRepresentation::decode_at(bytes, pos + 1, e)?;
                Ok((Self::Agent(rep), end))
            }
            ObjectKind::Client => {
                let (rep, end) = ClientRepresentation::decode_at(bytes, pos + 1, e)?;
                Ok((Self::Client(rep), end))
            }
            _ => {
                let (kind, variant, extra, end) = ObjectVariant::decode_for_kind_at(bytes, pos, e)?;
                Ok((
                    Self::Variant {
                        kind,
                        variant,
                        extra,
                    },
                    end,
                ))
            }
        }
    }
}

/// Spec §7.7.13: `@extensibility(FINAL) struct ObjectInfo { @optional
/// ActivityInfoVariant activity; @optional ObjectVariant config; }` — what a
/// `GET_INFO` is answered with. An `@optional` member is a presence octet and,
/// when set, the member.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectInfo {
    /// The activity of the object, if it was asked for and there is one.
    pub activity: Option<ActivityInfoVariant>,
    /// The configuration of the object, if it was asked for.
    pub config: Option<ObjectConfig>,
}

impl ObjectInfo {
    /// Encodes an `ObjectInfo` that starts its payload, little-endian.
    ///
    /// # Errors
    /// As [`ActivityInfoVariant::encode`]; `PayloadTooLarge` for a
    /// representation beyond `REPR_MAX_INLINE_BYTES`.
    pub fn encode(&self) -> Result<Vec<u8>, XrceError> {
        self.encode_with(Endianness::Little)
    }

    /// As [`Self::encode`], in the byte order of the submessage's E flag
    /// (DDS-XRCE 1.0 §8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::encode`].
    pub fn encode_with(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::with_capacity(16);
        self.encode_into(&mut out, e)?;
        Ok(out)
    }

    /// Appends the `ObjectInfo` to `out`, which starts the payload (an `INFO`
    /// has its six-octet `BaseObjectReply` in front): the alignment is counted
    /// from `out[0]`.
    ///
    /// # Errors
    /// As [`Self::encode`].
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        match &self.activity {
            Some(activity) => {
                out.push(1);
                activity.encode_into(out, e)?;
            }
            None => out.push(0),
        }
        match &self.config {
            Some(config) => {
                out.push(1);
                config.encode_into(out, e)?;
            }
            None => out.push(0),
        }
        Ok(())
    }

    /// Decodes an `ObjectInfo` that starts its payload, little-endian.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        Self::decode_with(bytes, Endianness::Little)
    }

    /// As [`Self::decode`], in the byte order of the submessage's E flag
    /// (§8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_with(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_at(bytes, 0, e)
    }

    /// Decodes the `ObjectInfo` that starts at `pos` of `bytes`, which starts
    /// the payload. Returns it and the position after it.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let flag = |at: usize| {
            bytes.get(at).copied().ok_or(XrceError::UnexpectedEof {
                needed: at + 1,
                offset: bytes.len(),
            })
        };
        let mut at = pos;
        let activity = if flag(at)? == 1 {
            let (a, end) = ActivityInfoVariant::decode_at(bytes, at + 1, e)?;
            at = end;
            Some(a)
        } else {
            at += 1;
            None
        };
        let config = if flag(at)? == 1 {
            let (c, end) = ObjectConfig::decode_at(bytes, at + 1, e)?;
            at = end;
            Some(c)
        } else {
            at += 1;
            None
        };
        Ok((Self { activity, config }, at))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::object_kind::OBJK_PARTICIPANT;

    // ---- ResultStatusCode -------------------------------------------

    #[test]
    fn result_status_code_all_ten_spec_values_roundtrip() {
        for code in [
            ResultStatusCode::Ok,
            ResultStatusCode::OkMatched,
            ResultStatusCode::DdsError,
            ResultStatusCode::Mismatch,
            ResultStatusCode::AlreadyExists,
            ResultStatusCode::Denied,
            ResultStatusCode::UnknownReference,
            ResultStatusCode::InvalidData,
            ResultStatusCode::Incompatible,
            ResultStatusCode::Resources,
        ] {
            let byte = code.as_u8();
            let decoded = ResultStatusCode::from_u8(byte).expect("roundtrip");
            assert_eq!(decoded, code);
        }
    }

    #[test]
    fn result_status_code_unknown_byte_rejected() {
        // Values between 0x02 and 0x7F are not in the spec, and the enum
        // literal list stops at 0x87 — there is no 0x88.
        assert!(ResultStatusCode::from_u8(0x02).is_err());
        assert!(ResultStatusCode::from_u8(0x7F).is_err());
        assert!(ResultStatusCode::from_u8(0x88).is_err());
        assert!(ResultStatusCode::from_u8(0xFF).is_err());
    }

    #[test]
    fn result_status_code_is_success() {
        assert!(ResultStatusCode::Ok.is_success());
        assert!(ResultStatusCode::OkMatched.is_success());
        assert!(!ResultStatusCode::DdsError.is_success());
        assert!(!ResultStatusCode::Denied.is_success());
    }

    #[test]
    fn result_status_roundtrip() {
        let r = ResultStatus {
            status_code: ResultStatusCode::AlreadyExists,
            implementation_status: 0x42,
        };
        // Annex A gives ResultStatus no @extensibility: APPENDABLE, a DHEADER
        // of 2 in front of the two members (DDS-XTypes 1.3 §7.3.1.2.1.8).
        let bytes = r.encode();
        assert_eq!(bytes, [2, 0, 0, 0, 0x82, 0x42]);
        assert_eq!(ResultStatus::decode(&bytes).expect("decode"), (r, 6));
        assert_eq!(
            r.encode_with(Endianness::Big),
            [0, 0, 0, 2, 0x82, 0x42],
            "the DHEADER follows the byte order"
        );
        // The form of the reference implementation: no DHEADER.
        crate::encoding::with_unannotated_types(UnannotatedTypes::Final, || {
            let bytes = r.encode();
            assert_eq!(bytes, [0x82, 0x42]);
            assert_eq!(ResultStatus::decode(&bytes).expect("decode"), (r, 2));
        });
        // Either form is read whatever the setting is.
        assert_eq!(ResultStatus::decode(&[0x82, 0x42]).unwrap(), (r, 2));
        crate::encoding::with_unannotated_types(UnannotatedTypes::Final, || {
            assert_eq!(
                ResultStatus::decode(&[2, 0, 0, 0, 0x82, 0x42]).unwrap(),
                (r, 6)
            );
        });
    }

    #[test]
    fn result_status_decode_short_returns_eof() {
        assert!(ResultStatus::decode(&[]).is_err());
        assert!(ResultStatus::decode(&[0x00]).is_err());
    }

    // ---- BaseObjectRequest -----------------------------------------

    #[test]
    fn base_object_request_roundtrip() {
        let oid = ObjectId::new(
            0x010,
            crate::object_kind::ObjectKind::from_u8(OBJK_PARTICIPANT).unwrap(),
        )
        .unwrap();
        let r = BaseObjectRequest {
            request_id: [0xAB, 0xCD],
            object_id: oid,
        };
        let bytes = r.encode();
        let (r2, n) = BaseObjectRequest::decode(&bytes).expect("decode");
        assert_eq!(r2, r);
        assert_eq!(n, BaseObjectRequest::WIRE_SIZE);
    }

    #[test]
    fn base_object_request_decode_short_returns_eof() {
        assert!(BaseObjectRequest::decode(&[0x01, 0x02, 0x03]).is_err());
    }

    // ---- BaseObjectReply -------------------------------------------

    #[test]
    fn base_object_reply_roundtrip() {
        let oid = ObjectId::new(
            0x123,
            crate::object_kind::ObjectKind::from_u8(OBJK_PARTICIPANT).unwrap(),
        )
        .unwrap();
        let r = BaseObjectReply {
            related_request: BaseObjectRequest {
                request_id: [0x11, 0x22],
                object_id: oid,
            },
            result: ResultStatus {
                status_code: ResultStatusCode::OkMatched,
                implementation_status: 0,
            },
        };
        // The ResultStatus inside has its DHEADER: 4 + (4 + 2) octets.
        let bytes = r.encode();
        assert_eq!(bytes.len(), 10);
        assert_eq!(BaseObjectReply::decode(&bytes).expect("decode"), (r, 10));
        // The reference implementation's STATUS payload: 6 octets.
        crate::encoding::with_unannotated_types(UnannotatedTypes::Final, || {
            let bytes = r.encode();
            assert_eq!(bytes.len(), BaseObjectReply::WIRE_SIZE);
            assert_eq!(
                BaseObjectReply::decode(&bytes).expect("decode"),
                (r, BaseObjectReply::WIRE_SIZE)
            );
        });
    }

    // ---- RelatedObjectRequest --------------------------------------

    /// §7.7.10: `RelatedObjectRequest` is a `typedef` for
    /// `BaseObjectRequest`, so it shares the exact same 4-byte codec —
    /// this is a type-level check, not a new encoding.
    #[test]
    fn related_object_request_is_base_object_request() {
        let oid = ObjectId::new(
            0x010,
            crate::object_kind::ObjectKind::from_u8(OBJK_PARTICIPANT).unwrap(),
        )
        .unwrap();
        let r: RelatedObjectRequest = BaseObjectRequest {
            request_id: [0xAA, 0xBB],
            object_id: oid,
        };
        let bytes = r.encode();
        assert_eq!(bytes.len(), 4, "RelatedObjectRequest is 4 bytes, not 6");
        let (r2, n) = RelatedObjectRequest::decode(&bytes).expect("decode");
        assert_eq!(r2, r);
        assert_eq!(n, BaseObjectRequest::WIRE_SIZE);
    }

    // ---- InfoMask --------------------------------------------------

    /// §7.7.12: `INFO_CONFIGURATION` is position 0, `INFO_ACTIVITY` position 1
    /// of a 32-bit bitmask.
    #[test]
    fn info_mask_bits_and_wire_form() {
        let mask = InfoMask(InfoMask::INFO_CONFIGURATION);
        assert!(mask.configuration() && !mask.activity());
        assert!(InfoMask(2).activity());
        assert_eq!(InfoMask::ALL, InfoMask(3));
        assert_eq!(InfoMask::ALL.encode(Endianness::Little), [3, 0, 0, 0]);
        assert_eq!(InfoMask::ALL.encode(Endianness::Big), [0, 0, 0, 3]);
        assert_eq!(
            InfoMask::decode(&[0, 0, 0, 2], Endianness::Big).unwrap(),
            (InfoMask(2), 4)
        );
        assert!(InfoMask::decode(&[1, 0], Endianness::Little).is_err());
    }

    // ---- ActivityInfoVariant ---------------------------------------

    fn medium(port: u16) -> TransportLocator {
        TransportLocator::Medium(crate::transport_locator::TransportLocatorMedium {
            address: [10, 0, 0, 1],
            port,
        })
    }

    /// Annex A: `AGENT_ActivityInfo { short availability; TransportLocatorSeq
    /// address_seq; }` is `@extensibility(APPENDABLE)`: a DHEADER with the
    /// length of the members. The union has a one-octet `ObjectKind`
    /// discriminator, so the DHEADER sits at 4.
    #[test]
    fn agent_activity_info_has_a_dheader_and_a_locator_sequence() {
        // The locators of the sequence are written in the form of the
        // reference implementation here, to keep the octets listed.
        crate::encoding::with_unannotated_types(crate::encoding::UnannotatedTypes::Final, || {
            let a = ActivityInfoVariant::Agent {
                availability: 1,
                address_seq: alloc::vec![medium(2018)],
            };
            let le = a.encode().unwrap();
            assert_eq!(
                le,
                alloc::vec![
                    0x0D, 0, 0, 0, // OBJK_AGENT, padding
                    16, 0, 0,
                    0, // DHEADER: availability(2) + padding(2) + count(4) + locator(8)
                    1, 0, // availability
                    0, 0, // padding to 4
                    1, 0, 0, 0, // address_seq.length
                    1, 10, 0, 0, 1, // ADDRESS_FORMAT_MEDIUM, address
                    0, // padding to 2
                    0xE2, 0x07, // port 2018
                ]
            );
            assert_eq!(
                ActivityInfoVariant::decode(&le).unwrap(),
                (a.clone(), le.len())
            );
            let be = a.encode_with(Endianness::Big).unwrap();
            assert_eq!(&be[4..8], &[0, 0, 0, 16]);
            assert_eq!(&be[8..10], &[0, 1]);
            assert_eq!(
                ActivityInfoVariant::decode_with(&be, Endianness::Big).unwrap(),
                (a, be.len())
            );
        });
        // By default the locator carries the DHEADERs of its union and case.
        let a = ActivityInfoVariant::Agent {
            availability: 1,
            address_seq: alloc::vec![medium(2018)],
        };
        let le = a.encode().unwrap();
        assert_eq!(le.len(), 8 + 4 + 4 + 18);
        assert_eq!(ActivityInfoVariant::decode(&le).unwrap(), (a, le.len()));
    }

    /// `DATAREADER_ActivityInfo { short highest_acked_num; }`.
    #[test]
    fn data_reader_activity_info_is_a_dheader_and_a_short() {
        let a = ActivityInfoVariant::DataReader {
            highest_acked_num: -2,
        };
        let le = a.encode().unwrap();
        assert_eq!(le, alloc::vec![0x06, 0, 0, 0, 2, 0, 0, 0, 0xFE, 0xFF]);
        assert_eq!(ActivityInfoVariant::decode(&le).unwrap(), (a, 10));
    }

    /// `DATAWRITER_ActivityInfo { unsigned long long sample_seq_num; short
    /// stream_seq_num; }`: the 64-bit number is aligned to 4 in XCDR2.
    #[test]
    fn data_writer_activity_info_is_a_dheader_a_long_long_and_a_short() {
        let a = ActivityInfoVariant::DataWriter {
            sample_seq_num: 0x0102_0304_0506_0708,
            stream_seq_num: 3,
        };
        let le = a.encode().unwrap();
        assert_eq!(
            le,
            alloc::vec![0x05, 0, 0, 0, 10, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 3, 0]
        );
        assert_eq!(ActivityInfoVariant::decode(&le).unwrap(), (a.clone(), 18));
        let be = a.encode_with(Endianness::Big).unwrap();
        assert_eq!(&be[8..16], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            ActivityInfoVariant::decode_with(&be, Endianness::Big)
                .unwrap()
                .0,
            a
        );
    }

    /// A member an appended-to later version adds is skipped by the DHEADER.
    #[test]
    fn appended_members_of_an_activity_are_skipped() {
        let mut bytes = alloc::vec![
            0x06, 0, 0, 0, 6, 0, 0, 0, 0x05, 0x00, 0xAA, 0xBB, 0xCC, 0xDD
        ];
        let (a, n) = ActivityInfoVariant::decode(&bytes).unwrap();
        assert_eq!(
            a,
            ActivityInfoVariant::DataReader {
                highest_acked_num: 5
            }
        );
        assert_eq!(n, bytes.len());
        // A DHEADER that reaches past the bytes is truncation.
        bytes.truncate(10);
        assert!(ActivityInfoVariant::decode(&bytes).is_err());
    }

    #[test]
    fn activity_info_decode_unknown_discriminator_rejected() {
        let bytes = [0x77u8, 0, 0, 0, 0, 0, 0, 0];
        assert!(ActivityInfoVariant::decode(&bytes).is_err());
    }

    #[test]
    fn activity_info_decode_truncated_rejected() {
        assert!(ActivityInfoVariant::decode(&[0x06, 0x01]).is_err());
    }

    // ---- ObjectInfo ------------------------------------------------

    /// `ObjectInfo { @optional activity; @optional config; }`: two presence
    /// octets for an empty one.
    #[test]
    fn an_empty_object_info_is_two_presence_octets() {
        let info = ObjectInfo::default();
        assert_eq!(info.encode().unwrap(), alloc::vec![0, 0]);
        assert_eq!(ObjectInfo::decode(&[0, 0]).unwrap(), (info, 2));
    }

    /// The activity comes first, then the config; each behind its presence
    /// octet. With the 6 octets of a `BaseObjectReply` in front (an INFO)
    /// the DHEADER lands at 8.
    #[test]
    fn object_info_puts_the_activity_before_the_config() {
        let info = ObjectInfo {
            activity: Some(ActivityInfoVariant::DataReader {
                highest_acked_num: -2,
            }),
            config: None,
        };
        assert_eq!(
            info.encode().unwrap(),
            alloc::vec![1, 0x06, 0, 0, 2, 0, 0, 0, 0xFE, 0xFF, 0]
        );
        let mut payload = alloc::vec![0xEE; BaseObjectReply::WIRE_SIZE];
        info.encode_into(&mut payload, Endianness::Little).unwrap();
        // The presence octet at 6, the discriminator at 7, then the DHEADER
        // already at 8, which is aligned: no padding.
        assert_eq!(&payload[6..], &[1, 0x06, 2, 0, 0, 0, 0xFE, 0xFF, 0][..]);
        let (back, end) = ObjectInfo::decode_at(&payload, 6, Endianness::Little).unwrap();
        assert_eq!(back, info);
        assert_eq!(end, payload.len());
    }

    /// The `config` of the agent is an `ObjectVariant` of kind `OBJK_AGENT`,
    /// i.e. the `AGENT_Representation` behind the kind octet (§7.8.2.2).
    #[test]
    fn the_config_of_the_agent_is_its_agent_representation() {
        let rep = AgentRepresentation {
            timestamp: None,
            version: [1, 0],
            vendor_id: [1, 15],
            properties: None,
        };
        let info = ObjectInfo {
            activity: None,
            config: Some(ObjectConfig::Agent(rep.clone())),
        };
        let bytes = info.encode().unwrap();
        assert_eq!(
            bytes,
            alloc::vec![0, 1, 0x0D, b'X', b'R', b'C', b'E', 1, 0, 1, 15, 0]
        );
        assert_eq!(ObjectInfo::decode(&bytes).unwrap(), (info, bytes.len()));
        // The same representation, big-endian: nothing in it is a number.
        assert_eq!(
            ObjectInfo::decode_with(&bytes, Endianness::Big)
                .unwrap()
                .0
                .config,
            Some(ObjectConfig::Agent(rep))
        );
    }

    /// The `config` of any other object is the `ObjectVariant` it was
    /// created with: the kind octet, the representation, the trailing field.
    #[test]
    fn the_config_of_a_topic_is_the_object_variant_it_was_created_with() {
        let extra = ObjectVariantExtra {
            domain_id: None,
            parent: Some(ObjectId::from_raw(0x0011)),
        };
        let info = ObjectInfo {
            activity: Some(ActivityInfoVariant::DataReader {
                highest_acked_num: 7,
            }),
            config: Some(ObjectConfig::Variant {
                kind: ObjectKind::Topic,
                variant: ObjectVariant::ByXmlString("<dds><topic/></dds>".into()),
                extra,
            }),
        };
        for e in [Endianness::Little, Endianness::Big] {
            let mut payload = alloc::vec![0u8; BaseObjectReply::WIRE_SIZE];
            info.encode_into(&mut payload, e).unwrap();
            let (back, end) = ObjectInfo::decode_at(&payload, 6, e).unwrap();
            assert_eq!(back, info);
            assert_eq!(end, payload.len());
        }
    }

    #[test]
    fn object_info_truncated_returns_eof() {
        // The activity is announced and missing.
        assert!(ObjectInfo::decode(&[1u8]).is_err());
        assert!(ObjectInfo::decode(&[0u8]).is_err());
    }
}
