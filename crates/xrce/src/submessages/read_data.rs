// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `READ_DATA` Submessage (id=8, DDS-XRCE 1.0 §8.3.5.9).
//!
//! Direction: Client → Agent. Spec-conformant wire form:
//! `READ_DATA_Payload` derives from `BaseObjectRequest` (§7.7.8) and adds a
//! `ReadSpecification` — `request_id` (2) + `object_id` (2, the DataReader) +
//! the read specification. Without the leading `BaseObjectRequest` a foreign
//! agent cannot associate the request with a DataReader.
//!
//! `ReadSpecification` (§8.3.5.9):
//! ```text
//! struct ReadSpecification {
//!     StreamId preferred_stream_id;
//!     DataFormat data_format;
//!     @optional ContentFilterExpression content_filter_expression;
//!     @optional DataDeliveryControl   delivery_control;
//! };
//! ```
//! **Conservative Phase-1 encoding of the two `@optional` members:** a 1-byte
//! presence flag (`1` present / `0` absent) precedes each optional payload —
//! the same self-consistent convention this crate already uses for
//! `ObjectInfo` (§7.7.13). The full XCDR2 `@optional` member header (EMHEADER)
//! form is Phase 2; the presence-flag form is documented here so a peer that
//! shares this crate's codec round-trips exactly.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u16, read_u32, write_u16, write_u32};
use crate::error::XrceError;
use crate::object_info::BaseObjectRequest;
use crate::submessages::write_data::DataFormat;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Cap for the `content_filter_expression` string — guards against a
/// `length = u32::MAX` decoder bomb. The submessage body is itself bounded to
/// 65 535 bytes, so 64 KiB is a generous ceiling.
pub const READ_SPEC_MAX_FILTER_BYTES: usize = 65_536;

/// `DataDeliveryControl` (§8.3.5.9): pacing/limits for the delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataDeliveryControl {
    /// Maximum number of samples to deliver.
    pub max_samples: u16,
    /// Maximum elapsed time (seconds) the delivery may run.
    pub max_elapsed_time: u16,
    /// Maximum bytes per second (rate limit).
    pub max_bytes_per_second: u16,
    /// Minimum pace period between samples (milliseconds).
    pub min_pace_period: u16,
}

impl DataDeliveryControl {
    /// Wire size: four `unsigned short` = 8 bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Appends the 8-byte encoding.
    ///
    /// # Errors
    /// `WriteOverflow` (impossible with the fixed 8-byte buffer).
    fn encode_into(&self, e: Endianness, out: &mut Vec<u8>) -> Result<(), XrceError> {
        let mut buf = [0u8; Self::WIRE_SIZE];
        write_u16(&mut buf[0..2], self.max_samples, e)?;
        write_u16(&mut buf[2..4], self.max_elapsed_time, e)?;
        write_u16(&mut buf[4..6], self.max_bytes_per_second, e)?;
        write_u16(&mut buf[6..8], self.min_pace_period, e)?;
        out.extend_from_slice(&buf);
        Ok(())
    }

    /// Decodes 8 bytes.
    ///
    /// # Errors
    /// `UnexpectedEof` on fewer than 8 bytes.
    fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        Ok((
            Self {
                max_samples: read_u16(&bytes[0..2], e)?,
                max_elapsed_time: read_u16(&bytes[2..4], e)?,
                max_bytes_per_second: read_u16(&bytes[4..6], e)?,
                min_pace_period: read_u16(&bytes[6..8], e)?,
            },
            Self::WIRE_SIZE,
        ))
    }
}

/// `ReadSpecification` (§8.3.5.9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadSpecification {
    /// Preferred return stream for the delivered `DATA`.
    pub preferred_stream_id: u8,
    /// Desired sample `DataFormat` for the reply.
    pub data_format: DataFormat,
    /// Optional content-filter expression (SQL-like `WHERE`).
    pub content_filter_expression: Option<String>,
    /// Optional delivery pacing/limits.
    pub delivery_control: Option<DataDeliveryControl>,
}

impl ReadSpecification {
    /// Appends the encoding (conservative presence-flag optionals).
    ///
    /// # Errors
    /// `PayloadTooLarge` if the filter exceeds `READ_SPEC_MAX_FILTER_BYTES`;
    /// `ValueOutOfRange` on a length overflow.
    fn encode_into(&self, e: Endianness, out: &mut Vec<u8>) -> Result<(), XrceError> {
        out.push(self.preferred_stream_id);
        out.push(self.data_format.raw());
        match &self.content_filter_expression {
            Some(cf) => {
                let bytes = cf.as_bytes();
                if bytes.len() > READ_SPEC_MAX_FILTER_BYTES {
                    return Err(XrceError::PayloadTooLarge {
                        limit: READ_SPEC_MAX_FILTER_BYTES,
                        actual: bytes.len(),
                    });
                }
                out.push(1);
                let len = u32::try_from(bytes.len()).map_err(|_| XrceError::ValueOutOfRange {
                    message: "content_filter_expression length exceeds u32",
                })?;
                let mut len_buf = [0u8; 4];
                write_u32(&mut len_buf, len, e)?;
                out.extend_from_slice(&len_buf);
                out.extend_from_slice(bytes);
            }
            None => out.push(0),
        }
        match &self.delivery_control {
            Some(dc) => {
                out.push(1);
                dc.encode_into(e, out)?;
            }
            None => out.push(0),
        }
        Ok(())
    }

    /// Decodes a `ReadSpecification`, returning `(spec, bytes_consumed)`.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`, `PayloadTooLarge`.
    fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        if bytes.len() < 3 {
            return Err(XrceError::UnexpectedEof {
                needed: 3,
                offset: 0,
            });
        }
        let preferred_stream_id = bytes[0];
        let data_format = DataFormat::from_raw(bytes[1])?;
        let mut cursor = 2usize;

        let cf_present = bytes[cursor];
        cursor += 1;
        let content_filter_expression = if cf_present == 1 {
            if bytes.len() < cursor + 4 {
                return Err(XrceError::UnexpectedEof {
                    needed: cursor + 4,
                    offset: cursor,
                });
            }
            let len = read_u32(&bytes[cursor..cursor + 4], e)?;
            let len = usize::try_from(len).map_err(|_| XrceError::ValueOutOfRange {
                message: "content_filter_expression length exceeds usize",
            })?;
            if len > READ_SPEC_MAX_FILTER_BYTES {
                return Err(XrceError::PayloadTooLarge {
                    limit: READ_SPEC_MAX_FILTER_BYTES,
                    actual: len,
                });
            }
            cursor += 4;
            if bytes.len() < cursor + len {
                return Err(XrceError::UnexpectedEof {
                    needed: cursor + len,
                    offset: cursor,
                });
            }
            let s = core::str::from_utf8(&bytes[cursor..cursor + len]).map_err(|_| {
                XrceError::ValueOutOfRange {
                    message: "content_filter_expression is not valid utf-8",
                }
            })?;
            cursor += len;
            Some(s.into())
        } else {
            None
        };

        if bytes.len() < cursor + 1 {
            return Err(XrceError::UnexpectedEof {
                needed: cursor + 1,
                offset: cursor,
            });
        }
        let dc_present = bytes[cursor];
        cursor += 1;
        let delivery_control = if dc_present == 1 {
            let (dc, n) = DataDeliveryControl::decode(&bytes[cursor..], e)?;
            cursor += n;
            Some(dc)
        } else {
            None
        };

        Ok((
            Self {
                preferred_stream_id,
                data_format,
                content_filter_expression,
                delivery_control,
            },
            cursor,
        ))
    }
}

/// `READ_DATA_Payload` (§8.3.5.9): `BaseObjectRequest` + `ReadSpecification`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReadDataPayload {
    /// `request_id` + `object_id` (the DataReader to read from). Spec §7.7.8.
    pub base: BaseObjectRequest,
    /// How the client wants the samples delivered.
    pub read_specification: ReadSpecification,
}

impl ReadDataPayload {
    /// Encodes the submessage body.
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut body = Vec::with_capacity(BaseObjectRequest::WIRE_SIZE + 4);
        body.extend_from_slice(&self.base.encode());
        self.read_specification.encode_into(e, &mut body)?;
        Ok(body)
    }

    /// Packs into a `Submessage` (little-endian body).
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::ReadData, FLAG_E_LITTLE_ENDIAN, body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong; `UnexpectedEof` on truncation.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::ReadData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not READ_DATA",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n) = BaseObjectRequest::decode(&sm.body)?;
        let (read_specification, _) = ReadSpecification::decode(&sm.body[n..], e)?;
        Ok(Self {
            base,
            read_specification,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::object_id::ObjectId;
    use crate::object_kind::ObjectKind;

    fn reader_base() -> BaseObjectRequest {
        BaseObjectRequest {
            request_id: [0x07, 0x08],
            object_id: ObjectId::new(0x055, ObjectKind::DataReader).unwrap(),
        }
    }

    #[test]
    fn read_data_minimal_roundtrip() {
        let p = ReadDataPayload {
            base: reader_base(),
            read_specification: ReadSpecification {
                preferred_stream_id: 0x01,
                data_format: DataFormat::Data,
                content_filter_expression: None,
                delivery_control: None,
            },
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(ReadDataPayload::try_from_submessage(&sm).unwrap(), p);
    }

    #[test]
    fn read_data_base_object_request_precedes_the_spec() {
        let p = ReadDataPayload {
            base: reader_base(),
            read_specification: ReadSpecification::default(),
        };
        let sm = p.into_submessage().unwrap();
        let oid = reader_base().object_id.to_bytes();
        assert_eq!(sm.body[0], 0x07, "request_id[0]");
        assert_eq!(sm.body[1], 0x08, "request_id[1]");
        assert_eq!(sm.body[2], oid[0], "object_id[0] = DataReader");
        assert_eq!(sm.body[3], oid[1], "object_id[1] = DataReader");
    }

    #[test]
    fn read_data_full_spec_roundtrip() {
        let p = ReadDataPayload {
            base: reader_base(),
            read_specification: ReadSpecification {
                preferred_stream_id: 0x80,
                data_format: DataFormat::Data,
                content_filter_expression: Some("temperature > 20".into()),
                delivery_control: Some(DataDeliveryControl {
                    max_samples: 100,
                    max_elapsed_time: 5,
                    max_bytes_per_second: 4096,
                    min_pace_period: 10,
                }),
            },
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(ReadDataPayload::try_from_submessage(&sm).unwrap(), p);
    }

    #[test]
    fn read_data_rejects_wrong_id() {
        let sm = Submessage::new(SubmessageId::WriteData, 0x01, alloc::vec![0; 8]).unwrap();
        assert!(ReadDataPayload::try_from_submessage(&sm).is_err());
    }

    #[test]
    fn read_spec_rejects_reserved_data_format() {
        // preferred_stream_id, data_format=0b010 (reserved), then no optionals.
        let mut body = reader_base().encode().to_vec();
        body.extend_from_slice(&[0x01, 0b010, 0, 0]);
        let sm = Submessage::new(SubmessageId::ReadData, FLAG_E_LITTLE_ENDIAN, body).unwrap();
        assert!(matches!(
            ReadDataPayload::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }
}
