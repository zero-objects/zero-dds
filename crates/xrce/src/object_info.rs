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
//! * **§7.7.10** [`RelatedObjectRequest`] — BaseObjectRequest +
//!   ObjectIdRef (second ObjectId).
//! * **§7.7.12** [`ActivityInfoVariant`] — discriminated union with
//!   AgentActivity or DataReader/Writer activity.
//! * **§7.7.13** [`ObjectInfo`] — compound type for the GET_INFO reply.
//!
//! All wire codecs are little-endian (Spec §8.3.4: the submessage body
//! endianness is chosen via submessage header flag E; XCDR2 with no
//! padding between u8/u16/u32 fields in this struct family).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::object_id::ObjectId;

// ============================================================================
// §7.7.7 ResultStatus
// ============================================================================

/// Spec Tab.7.7.7: 11 status codes (OK/OK_MATCHED + 9 error classes).
///
/// Each code is 1 byte; values > 10 are not in the spec.
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
    /// `STATUS_ERR_UNSUPPORTED` — profile/operation not supported.
    Unsupported = 0x88,
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
            0x88 => Ok(Self::Unsupported),
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
    /// Wire size (2 bytes: status + implementation_status).
    pub const WIRE_SIZE: usize = 2;

    /// Success status (`Ok`).
    #[must_use]
    pub fn ok() -> Self {
        Self {
            status_code: ResultStatusCode::Ok,
            implementation_status: 0,
        }
    }

    /// Encode as 2 bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 2] {
        [self.status_code.as_u8(), self.implementation_status]
    }

    /// Decode from 2 bytes.
    ///
    /// # Errors
    /// `UnexpectedEof` on < 2 bytes; `ValueOutOfRange` on an
    /// unknown status code.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let status_code = ResultStatusCode::from_u8(bytes[0])?;
        let implementation_status = bytes[1];
        Ok((
            Self {
                status_code,
                implementation_status,
            },
            Self::WIRE_SIZE,
        ))
    }
}

// ============================================================================
// §7.7.8 BaseObjectRequest
// ============================================================================

/// Spec §7.7.8: `BaseObjectRequest { RequestId(2 octets), ObjectId(2 octets) }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Wire size = 6 bytes.
    pub const WIRE_SIZE: usize = BaseObjectRequest::WIRE_SIZE + ResultStatus::WIRE_SIZE;

    /// Encode as 6 bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 6] {
        let req = self.related_request.encode();
        let res = self.result.encode();
        [req[0], req[1], req[2], req[3], res[0], res[1]]
    }

    /// Decode from 6 bytes.
    ///
    /// # Errors
    /// As [`BaseObjectRequest::decode`] / [`ResultStatus::decode`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        let (related_request, n1) = BaseObjectRequest::decode(bytes)?;
        let (result, n2) = ResultStatus::decode(&bytes[n1..])?;
        Ok((
            Self {
                related_request,
                result,
            },
            n1 + n2,
        ))
    }
}

// ============================================================================
// §7.7.10 RelatedObjectRequest
// ============================================================================

/// Spec §7.7.10: `RelatedObjectRequest { base: BaseObjectRequest,
/// related_object: ObjectId }`.
///
/// Used when an operation must reference two objects
/// (e.g. CREATE_DATAWRITER + its associated topic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelatedObjectRequest {
    /// Base request with RequestId + primary ObjectId.
    pub base: BaseObjectRequest,
    /// Secondary ObjectId (related object).
    pub related_object: ObjectId,
}

impl RelatedObjectRequest {
    /// Wire size = 6 bytes.
    pub const WIRE_SIZE: usize = BaseObjectRequest::WIRE_SIZE + 2;

    /// Encode as 6 bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 6] {
        let b = self.base.encode();
        let r = self.related_object.to_bytes();
        [b[0], b[1], b[2], b[3], r[0], r[1]]
    }

    /// Decode from 6 bytes.
    ///
    /// # Errors
    /// See [`BaseObjectRequest::decode`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        let (base, n1) = BaseObjectRequest::decode(bytes)?;
        if bytes.len() < n1 + 2 {
            return Err(XrceError::UnexpectedEof {
                needed: n1 + 2,
                offset: n1,
            });
        }
        let related_object = ObjectId::from_bytes(&bytes[n1..n1 + 2])?;
        Ok((
            Self {
                base,
                related_object,
            },
            n1 + 2,
        ))
    }
}

// ============================================================================
// §7.7.12 ActivityInfoVariant
// ============================================================================

/// Spec §7.7.12: discriminated union with stream or object activity.
///
/// We model both productive variants:
/// * **AgentActivity** — for OBJK_AGENT (`address_seq` + `availability`).
/// * **DataReaderActivity** — for OBJK_DATAREADER
///   (`highest_acked_num` + `unread_sample_count`).
/// * **DataWriterActivity** — for OBJK_DATAWRITER
///   (`unacked_sample_count` + `sample_seq_num`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityInfoVariant {
    /// Agent liveness (1 byte availability + IP list).
    Agent {
        /// `1` = available, `0` = not.
        availability: u8,
        /// List of IP address octets (transparent).
        address_seq: Vec<u8>,
    },
    /// DataReader activity.
    DataReader {
        /// Highest acknowledged sample number.
        highest_acked_num: i16,
        /// Number of unread samples in the reader buffer.
        unread_sample_count: u32,
    },
    /// DataWriter activity.
    DataWriter {
        /// Number of unacked samples.
        unacked_sample_count: u32,
        /// Last sent sample sequence number.
        sample_seq_num: i64,
    },
}

impl ActivityInfoVariant {
    /// Discriminator — corresponds to the ObjectKind for which the activity
    /// applies (see `OBJK_AGENT` / `OBJK_DATAREADER` / `OBJK_DATAWRITER`
    /// in `object_kind.rs`).
    #[must_use]
    pub fn discriminator(&self) -> u8 {
        match self {
            Self::Agent { .. } => 0x0D,      // OBJK_AGENT
            Self::DataReader { .. } => 0x06, // OBJK_DATAREADER
            Self::DataWriter { .. } => 0x05, // OBJK_DATAWRITER
        }
    }

    /// Encode with a leading discriminator byte + variant-specific
    /// payload.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16);
        out.push(self.discriminator());
        match self {
            Self::Agent {
                availability,
                address_seq,
            } => {
                out.push(*availability);
                let len = u16::try_from(address_seq.len()).unwrap_or(u16::MAX);
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(address_seq);
            }
            Self::DataReader {
                highest_acked_num,
                unread_sample_count,
            } => {
                out.extend_from_slice(&highest_acked_num.to_le_bytes());
                out.extend_from_slice(&unread_sample_count.to_le_bytes());
            }
            Self::DataWriter {
                unacked_sample_count,
                sample_seq_num,
            } => {
                out.extend_from_slice(&unacked_sample_count.to_le_bytes());
                out.extend_from_slice(&sample_seq_num.to_le_bytes());
            }
        }
        out
    }

    /// Decode with a leading discriminator byte.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` on an unknown discriminator.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        if bytes.is_empty() {
            return Err(XrceError::UnexpectedEof {
                needed: 1,
                offset: 0,
            });
        }
        let disc = bytes[0];
        let rest = &bytes[1..];
        match disc {
            0x0D => {
                if rest.len() < 3 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 3,
                        offset: 1,
                    });
                }
                let availability = rest[0];
                let len = u16::from_le_bytes([rest[1], rest[2]]) as usize;
                if rest.len() < 3 + len {
                    return Err(XrceError::UnexpectedEof {
                        needed: 3 + len,
                        offset: 1,
                    });
                }
                let address_seq = rest[3..3 + len].to_vec();
                Ok((
                    Self::Agent {
                        availability,
                        address_seq,
                    },
                    1 + 3 + len,
                ))
            }
            0x06 => {
                if rest.len() < 6 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 6,
                        offset: 1,
                    });
                }
                let highest_acked_num = i16::from_le_bytes([rest[0], rest[1]]);
                let unread_sample_count = u32::from_le_bytes([rest[2], rest[3], rest[4], rest[5]]);
                Ok((
                    Self::DataReader {
                        highest_acked_num,
                        unread_sample_count,
                    },
                    7,
                ))
            }
            0x05 => {
                if rest.len() < 12 {
                    return Err(XrceError::UnexpectedEof {
                        needed: 12,
                        offset: 1,
                    });
                }
                let unacked_sample_count = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
                let mut snn = [0u8; 8];
                snn.copy_from_slice(&rest[4..12]);
                let sample_seq_num = i64::from_le_bytes(snn);
                Ok((
                    Self::DataWriter {
                        unacked_sample_count,
                        sample_seq_num,
                    },
                    13,
                ))
            }
            _ => Err(XrceError::ValueOutOfRange {
                message: "ActivityInfoVariant: unknown discriminator",
            }),
        }
    }
}

// ============================================================================
// §7.7.13 ObjectInfo
// ============================================================================

/// Spec §7.7.13: GET_INFO reply payload.
///
/// The spec defines ObjectInfo as optional config + optional activity.
/// We model it as two optionals with leading presence bytes
/// (1 = present, 0 = absent), followed by the respective payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectInfo {
    /// XCDR2-encoded `OBJK_*Representation` (opaque bytes); `None`
    /// when no config info is available.
    pub config: Option<Vec<u8>>,
    /// Activity info of the object.
    pub activity: Option<ActivityInfoVariant>,
}

impl ObjectInfo {
    /// Encode with two presence bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16);
        if let Some(cfg) = &self.config {
            out.push(1);
            let len = u16::try_from(cfg.len()).unwrap_or(u16::MAX);
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(cfg);
        } else {
            out.push(0);
        }
        if let Some(act) = &self.activity {
            out.push(1);
            out.extend_from_slice(&act.encode());
        } else {
            out.push(0);
        }
        out
    }

    /// Decode with two presence bytes.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        if bytes.is_empty() {
            return Err(XrceError::UnexpectedEof {
                needed: 1,
                offset: 0,
            });
        }
        let mut cursor = 0usize;
        let cfg_present = bytes[cursor];
        cursor += 1;
        let config = if cfg_present == 1 {
            if bytes.len() < cursor + 2 {
                return Err(XrceError::UnexpectedEof {
                    needed: cursor + 2,
                    offset: cursor,
                });
            }
            let len = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
            cursor += 2;
            if bytes.len() < cursor + len {
                return Err(XrceError::UnexpectedEof {
                    needed: cursor + len,
                    offset: cursor,
                });
            }
            let v = bytes[cursor..cursor + len].to_vec();
            cursor += len;
            Some(v)
        } else {
            None
        };
        if bytes.len() < cursor + 1 {
            return Err(XrceError::UnexpectedEof {
                needed: cursor + 1,
                offset: cursor,
            });
        }
        let act_present = bytes[cursor];
        cursor += 1;
        let activity = if act_present == 1 {
            let (a, n) = ActivityInfoVariant::decode(&bytes[cursor..])?;
            cursor += n;
            Some(a)
        } else {
            None
        };
        Ok((Self { config, activity }, cursor))
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
    fn result_status_code_all_11_spec_values_roundtrip() {
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
            ResultStatusCode::Unsupported,
        ] {
            let byte = code.as_u8();
            let decoded = ResultStatusCode::from_u8(byte).expect("roundtrip");
            assert_eq!(decoded, code);
        }
    }

    #[test]
    fn result_status_code_unknown_byte_rejected() {
        // Values between 0x02 and 0x7F are not in the spec.
        assert!(ResultStatusCode::from_u8(0x02).is_err());
        assert!(ResultStatusCode::from_u8(0x7F).is_err());
        assert!(ResultStatusCode::from_u8(0x89).is_err());
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
        let bytes = r.encode();
        assert_eq!(bytes, [0x82, 0x42]);
        let (r2, n) = ResultStatus::decode(&bytes).expect("decode");
        assert_eq!(r2, r);
        assert_eq!(n, 2);
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
        let bytes = r.encode();
        let (r2, n) = BaseObjectReply::decode(&bytes).expect("decode");
        assert_eq!(r2, r);
        assert_eq!(n, BaseObjectReply::WIRE_SIZE);
    }

    // ---- RelatedObjectRequest --------------------------------------

    #[test]
    fn related_object_request_roundtrip() {
        let oid_a = ObjectId::new(
            0x010,
            crate::object_kind::ObjectKind::from_u8(OBJK_PARTICIPANT).unwrap(),
        )
        .unwrap();
        let oid_b = ObjectId::new(
            0x020,
            crate::object_kind::ObjectKind::from_u8(OBJK_PARTICIPANT).unwrap(),
        )
        .unwrap();
        let r = RelatedObjectRequest {
            base: BaseObjectRequest {
                request_id: [0xAA, 0xBB],
                object_id: oid_a,
            },
            related_object: oid_b,
        };
        let bytes = r.encode();
        let (r2, n) = RelatedObjectRequest::decode(&bytes).expect("decode");
        assert_eq!(r2, r);
        assert_eq!(n, RelatedObjectRequest::WIRE_SIZE);
    }

    // ---- ActivityInfoVariant ---------------------------------------

    #[test]
    fn activity_info_agent_roundtrip() {
        let a = ActivityInfoVariant::Agent {
            availability: 1,
            address_seq: alloc::vec![10, 0, 0, 1, 0xE8, 0x1C],
        };
        let bytes = a.encode();
        let (a2, _) = ActivityInfoVariant::decode(&bytes).expect("roundtrip");
        assert_eq!(a2, a);
    }

    #[test]
    fn activity_info_data_reader_roundtrip() {
        let a = ActivityInfoVariant::DataReader {
            highest_acked_num: -3,
            unread_sample_count: 42,
        };
        let bytes = a.encode();
        let (a2, _) = ActivityInfoVariant::decode(&bytes).expect("roundtrip");
        assert_eq!(a2, a);
    }

    #[test]
    fn activity_info_data_writer_roundtrip() {
        let a = ActivityInfoVariant::DataWriter {
            unacked_sample_count: 7,
            sample_seq_num: 123_456_789_012i64,
        };
        let bytes = a.encode();
        let (a2, _) = ActivityInfoVariant::decode(&bytes).expect("roundtrip");
        assert_eq!(a2, a);
    }

    #[test]
    fn activity_info_decode_unknown_discriminator_rejected() {
        let bytes = [0xFF, 0x00, 0x01, 0x02];
        assert!(ActivityInfoVariant::decode(&bytes).is_err());
    }

    #[test]
    fn activity_info_decode_truncated_rejected() {
        // Discriminator OK, body missing
        assert!(ActivityInfoVariant::decode(&[0x06, 0x01]).is_err());
    }

    // ---- ObjectInfo ------------------------------------------------

    #[test]
    fn object_info_with_both_present_roundtrip() {
        let info = ObjectInfo {
            config: Some(alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]),
            activity: Some(ActivityInfoVariant::DataReader {
                highest_acked_num: 100,
                unread_sample_count: 5,
            }),
        };
        let bytes = info.encode();
        let (i2, _) = ObjectInfo::decode(&bytes).expect("roundtrip");
        assert_eq!(i2, info);
    }

    #[test]
    fn object_info_only_config_roundtrip() {
        let info = ObjectInfo {
            config: Some(alloc::vec![0x01, 0x02, 0x03]),
            activity: None,
        };
        let bytes = info.encode();
        let (i2, _) = ObjectInfo::decode(&bytes).expect("roundtrip");
        assert_eq!(i2, info);
    }

    #[test]
    fn object_info_only_activity_roundtrip() {
        let info = ObjectInfo {
            config: None,
            activity: Some(ActivityInfoVariant::Agent {
                availability: 1,
                address_seq: alloc::vec![],
            }),
        };
        let bytes = info.encode();
        let (i2, _) = ObjectInfo::decode(&bytes).expect("roundtrip");
        assert_eq!(i2, info);
    }

    #[test]
    fn object_info_empty_roundtrip() {
        let info = ObjectInfo::default();
        let bytes = info.encode();
        assert_eq!(bytes, [0, 0]);
        let (i2, _) = ObjectInfo::decode(&bytes).expect("roundtrip");
        assert_eq!(i2, info);
    }

    #[test]
    fn object_info_truncated_returns_eof() {
        // The presence byte signals config, but the length bytes are missing.
        assert!(ObjectInfo::decode(&[1u8]).is_err());
    }
}
