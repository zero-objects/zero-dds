// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Object-Repr-Typen aus Spec §7.7.7 - §7.7.13.
//!
//! Modul implementiert die Wire-Strukturen, die in den
//! CREATE/STATUS/GET_INFO/INFO-Submessage-Bodies vorkommen:
//!
//! * **§7.7.7** [`ResultStatusCode`] — 11 Spec-konforme Status-Codes
//!   plus generischer `ResultStatus` (status_code + implementation_status).
//! * **§7.7.8** [`BaseObjectRequest`] — RequestId(2) + ObjectId(2).
//! * **§7.7.9** [`BaseObjectReply`] — BaseObjectRequest + ResultStatus.
//! * **§7.7.10** [`RelatedObjectRequest`] — BaseObjectRequest +
//!   ObjectIdRef (zweite ObjectId).
//! * **§7.7.12** [`ActivityInfoVariant`] — Discriminated-Union mit
//!   AgentActivity oder DataReader/Writer-Activity.
//! * **§7.7.13** [`ObjectInfo`] — Compound-Type fuer GET_INFO-Reply.
//!
//! Alle Wire-Codecs sind little-endian (Spec §8.3.4: Submessage-Body-
//! Endianness wird via Submessage-Header-Flag E gewaehlt; XCDR2 ohne
//! Padding zwischen u8/u16/u32-Feldern in dieser Struktur-Familie).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::object_id::ObjectId;

// ============================================================================
// §7.7.7 ResultStatus
// ============================================================================

/// Spec-Tab.7.7.7: 11 Status-Codes (OK/OK_MATCHED + 9 Error-Klassen).
///
/// Jeder Code ist 1 Byte; Werte > 10 sind nicht in der Spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResultStatusCode {
    /// `STATUS_OK` — Operation erfolgreich.
    Ok = 0x00,
    /// `STATUS_OK_MATCHED` — Operation erfolgreich, matched bestehendes Objekt.
    OkMatched = 0x01,
    /// `STATUS_ERR_DDS_ERROR` — generischer DDS-Fehler.
    DdsError = 0x80,
    /// `STATUS_ERR_MISMATCH` — Cert/Type/QoS-Mismatch zwischen Peers.
    Mismatch = 0x81,
    /// `STATUS_ERR_ALREADY_EXISTS` — Object mit dieser ID existiert bereits.
    AlreadyExists = 0x82,
    /// `STATUS_ERR_DENIED` — AccessControl hat Operation abgelehnt.
    Denied = 0x83,
    /// `STATUS_ERR_UNKNOWN_REFERENCE` — Referenzierte Resource fehlt.
    UnknownReference = 0x84,
    /// `STATUS_ERR_INVALID_DATA` — Wire-Body nicht parsbar.
    InvalidData = 0x85,
    /// `STATUS_ERR_INCOMPATIBLE` — Spec-Konflikt (Version etc.).
    Incompatible = 0x86,
    /// `STATUS_ERR_RESOURCES` — Speicher/Slots aufgebraucht.
    Resources = 0x87,
    /// `STATUS_ERR_UNSUPPORTED` — Profile/Operation nicht unterstuetzt.
    Unsupported = 0x88,
}

impl ResultStatusCode {
    /// Wire-Wert (1 byte).
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Konvertiert ein Byte. Werte ausserhalb der Spec-Liste werden
    /// abgelehnt.
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

    /// `true` wenn Status `Ok` oder `OkMatched`.
    #[must_use]
    pub fn is_success(self) -> bool {
        matches!(self, Self::Ok | Self::OkMatched)
    }
}

/// Spec §7.7.7: `ResultStatus { status_code, implementation_status }`.
///
/// `implementation_status` ist Vendor-spezifisch und wird als 1 Byte
/// transparent durchgereicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultStatus {
    /// Status-Code aus Spec-Tab.7.7.7.
    pub status_code: ResultStatusCode,
    /// Vendor-spezifischer Status (1 Byte, opak).
    pub implementation_status: u8,
}

impl ResultStatus {
    /// Wire-Size (2 Bytes: status + implementation_status).
    pub const WIRE_SIZE: usize = 2;

    /// Erfolgs-Status (`Ok`).
    #[must_use]
    pub fn ok() -> Self {
        Self {
            status_code: ResultStatusCode::Ok,
            implementation_status: 0,
        }
    }

    /// Encode als 2 Bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 2] {
        [self.status_code.as_u8(), self.implementation_status]
    }

    /// Decode aus 2 Bytes.
    ///
    /// # Errors
    /// `UnexpectedEof` bei < 2 Bytes; `ValueOutOfRange` bei
    /// unbekanntem Status-Code.
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
    /// Request-Identifier (2 octets, eindeutig pro Stream).
    pub request_id: [u8; 2],
    /// Ziel-Objekt der Operation.
    pub object_id: ObjectId,
}

impl BaseObjectRequest {
    /// Wire-Size = 4 Bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Encode als 4 Bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let oid = self.object_id.to_bytes();
        [self.request_id[0], self.request_id[1], oid[0], oid[1]]
    }

    /// Decode aus 4 Bytes.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` bei ObjectId-Problemen.
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
    /// Originaler Request, auf den sich die Reply bezieht.
    pub related_request: BaseObjectRequest,
    /// Result-Status der Operation.
    pub result: ResultStatus,
}

impl BaseObjectReply {
    /// Wire-Size = 6 Bytes.
    pub const WIRE_SIZE: usize = BaseObjectRequest::WIRE_SIZE + ResultStatus::WIRE_SIZE;

    /// Encode als 6 Bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 6] {
        let req = self.related_request.encode();
        let res = self.result.encode();
        [req[0], req[1], req[2], req[3], res[0], res[1]]
    }

    /// Decode aus 6 Bytes.
    ///
    /// # Errors
    /// Wie [`BaseObjectRequest::decode`] / [`ResultStatus::decode`].
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
/// Wird verwendet, wenn eine Operation zwei Objekte referenzieren
/// muss (z.B. CREATE_DATAWRITER + dazugehoeriges Topic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelatedObjectRequest {
    /// Basis-Request mit RequestId + Primary-ObjectId.
    pub base: BaseObjectRequest,
    /// Sekundaere ObjectId (Related-Object).
    pub related_object: ObjectId,
}

impl RelatedObjectRequest {
    /// Wire-Size = 6 Bytes.
    pub const WIRE_SIZE: usize = BaseObjectRequest::WIRE_SIZE + 2;

    /// Encode als 6 Bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; 6] {
        let b = self.base.encode();
        let r = self.related_object.to_bytes();
        [b[0], b[1], b[2], b[3], r[0], r[1]]
    }

    /// Decode aus 6 Bytes.
    ///
    /// # Errors
    /// Siehe [`BaseObjectRequest::decode`].
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

/// Spec §7.7.12: Discriminated-Union mit Stream- oder Object-Activity.
///
/// Wir modellieren beide produktiven Varianten:
/// * **AgentActivity** — fuer OBJK_AGENT (`address_seq` + `availability`).
/// * **DataReaderActivity** — fuer OBJK_DATAREADER
///   (`highest_acked_num` + `unread_sample_count`).
/// * **DataWriterActivity** — fuer OBJK_DATAWRITER
///   (`unacked_sample_count` + `sample_seq_num`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityInfoVariant {
    /// Agent-Liveness (1 byte availability + IP-List).
    Agent {
        /// `1` = available, `0` = not.
        availability: u8,
        /// Liste von IP-Adress-Octets (transparent).
        address_seq: Vec<u8>,
    },
    /// DataReader-Activity.
    DataReader {
        /// Hoechste bestaetigte Sample-Nummer.
        highest_acked_num: i16,
        /// Anzahl ungelesener Samples im Reader-Buffer.
        unread_sample_count: u32,
    },
    /// DataWriter-Activity.
    DataWriter {
        /// Anzahl unacked Samples.
        unacked_sample_count: u32,
        /// Letzte gesendete Sample-Sequence-Number.
        sample_seq_num: i64,
    },
}

impl ActivityInfoVariant {
    /// Discriminator — entspricht dem ObjectKind, fuer den die Activity
    /// gilt (siehe `OBJK_AGENT` / `OBJK_DATAREADER` / `OBJK_DATAWRITER`
    /// in `object_kind.rs`).
    #[must_use]
    pub fn discriminator(&self) -> u8 {
        match self {
            Self::Agent { .. } => 0x0D,      // OBJK_AGENT
            Self::DataReader { .. } => 0x06, // OBJK_DATAREADER
            Self::DataWriter { .. } => 0x05, // OBJK_DATAWRITER
        }
    }

    /// Encode mit fuehrendem Discriminator-Byte + Variant-spezifischem
    /// Payload.
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

    /// Decode mit fuehrendem Discriminator-Byte.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` bei unbekanntem Discriminator.
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

/// Spec §7.7.13: GET_INFO-Reply-Payload.
///
/// Spec definiert ObjectInfo als optional config + optional activity.
/// Wir modellieren es als zwei Optionals mit fuehrenden Presence-Bytes
/// (1 = present, 0 = absent), gefolgt vom jeweiligen Payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectInfo {
    /// XCDR2-encoded `OBJK_*Representation` (opake Bytes); `None`
    /// wenn keine Config-Info verfuegbar.
    pub config: Option<Vec<u8>>,
    /// Activity-Info des Objects.
    pub activity: Option<ActivityInfoVariant>,
}

impl ObjectInfo {
    /// Encode mit zwei Presence-Bytes.
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

    /// Decode mit zwei Presence-Bytes.
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
        // Werte zwischen 0x02 und 0x7F sind nicht in der Spec.
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
        // Discriminator OK, body fehlt
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
        // Presence-Byte signalisiert config, aber length-Bytes fehlen.
        assert!(ObjectInfo::decode(&[1u8]).is_err());
    }
}
