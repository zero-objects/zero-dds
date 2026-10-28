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
//! `ReadSpecification` (§7.7.14, Annex A):
//! ```text
//! @extensibility(FINAL) struct ReadSpecification {
//!     StreamId preferred_stream_id;
//!     DataFormat data_format;                 // 0x00, 0x02, 0x08, 0x0A, 0x0E
//!     @optional string content_filter_expression;
//!     @optional DataDeliveryControl delivery_control;
//! };
//! @extensibility(APPENDABLE) struct DataDeliveryControl {
//!     unsigned short max_samples, max_elapsed_time,
//!                    max_bytes_per_second, min_pace_period;
//! };
//! ```
//! An `@optional` member is a presence octet (`1` present / `0` absent) and
//! the member. The string is an XCDR2 `string`: its length (an `unsigned
//! long`, aligned to 4 from the start of the payload) counts the NUL, the
//! characters, the NUL. `DataDeliveryControl` is `APPENDABLE`: a DHEADER
//! (an `unsigned long` aligned to 4, the length of the members: 8) precedes
//! its four `unsigned short`s (Annex B.4.2, which writes the DHEADER as
//! `0x80000008`; bit 31 is ignored when reading).
//!
//! The four real `READ_DATA` datagrams of eProsima Micro-XRCE-DDS Client 3.0.1
//! (`tools/wire-capture/xrce-sessions/client-to-agent.hex` lines 6, 29, 47,
//! 366) have the `DataDeliveryControl` **without** the DHEADER: each is exactly
//! `BaseObjectRequest`(4) + `preferred_stream_id`(1) + `data_format`(1) +
//! `cf_present=0x00`(1) + `dc_present=0x01`(1) + `DataDeliveryControl`(8,
//! `max_samples=0xFFFF`) = 16 bytes. Reading accepts both forms;
//! [`ReadDataPayload::into_submessage`] writes the DHEADER of the spec,
//! [`ReadDataPayload::into_submessage_with`] with
//! [`DeliveryControlForm::Bare`] writes the form that client sends.

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

/// How the `DataDeliveryControl` of a `ReadSpecification` is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeliveryControlForm {
    /// With the DHEADER of an `@extensibility(APPENDABLE)` struct (XCDR2,
    /// §7.7.14, Annex B.4.2): what the spec says.
    #[default]
    Appendable,
    /// Without it: what the eProsima Micro-XRCE-DDS Client sends.
    Bare,
}

fn pad_to(out: &mut Vec<u8>, n: usize) {
    while out.len() % n != 0 {
        out.push(0);
    }
}

impl ReadSpecification {
    /// Appends the encoding. `out` holds only the specification, which
    /// follows the four octets of the `BaseObjectRequest`: padding counted
    /// from its start is padding counted from the start of the payload, as
    /// the request is 4 octets.
    ///
    /// # Errors
    /// `PayloadTooLarge` if the filter exceeds `READ_SPEC_MAX_FILTER_BYTES`;
    /// `ValueOutOfRange` on a length overflow.
    fn encode_into(
        &self,
        e: Endianness,
        form: DeliveryControlForm,
        out: &mut Vec<u8>,
    ) -> Result<(), XrceError> {
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
                // An XCDR2 string: the length counts the NUL; the length is
                // aligned to 4.
                let len =
                    u32::try_from(bytes.len() + 1).map_err(|_| XrceError::ValueOutOfRange {
                        message: "content_filter_expression length exceeds u32",
                    })?;
                pad_to(out, 4);
                let mut len_buf = [0u8; 4];
                write_u32(&mut len_buf, len, e)?;
                out.extend_from_slice(&len_buf);
                out.extend_from_slice(bytes);
                out.push(0);
            }
            None => out.push(0),
        }
        match &self.delivery_control {
            Some(dc) => {
                out.push(1);
                if form == DeliveryControlForm::Appendable {
                    pad_to(out, 4);
                    let mut dheader = [0u8; 4];
                    write_u32(&mut dheader, DataDeliveryControl::WIRE_SIZE as u32, e)?;
                    out.extend_from_slice(&dheader);
                }
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
            // The length is an `unsigned long` aligned to 4.
            cursor = cursor.next_multiple_of(4);
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
            // The length counts the NUL terminator.
            if len == 0 || len > READ_SPEC_MAX_FILTER_BYTES + 1 {
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
            let raw = &bytes[cursor..cursor + len];
            let Some((&0, text)) = raw.split_last() else {
                return Err(XrceError::ValueOutOfRange {
                    message: "content_filter_expression lacks its NUL terminator",
                });
            };
            let s = core::str::from_utf8(text).map_err(|_| XrceError::ValueOutOfRange {
                message: "content_filter_expression is not valid utf-8",
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
            // Exactly the eight octets of the members left: the form without
            // a DHEADER. Otherwise the DHEADER of an APPENDABLE struct (bit
            // 31, which Annex B.4.2 sets, is not part of the length).
            if bytes.len() - cursor == DataDeliveryControl::WIRE_SIZE {
                let (dc, n) = DataDeliveryControl::decode(&bytes[cursor..], e)?;
                cursor += n;
                Some(dc)
            } else {
                let at = cursor.next_multiple_of(4);
                if bytes.len() < at + 4 {
                    return Err(XrceError::UnexpectedEof {
                        needed: at + 4,
                        offset: cursor,
                    });
                }
                let length = (read_u32(&bytes[at..at + 4], e)? & 0x7FFF_FFFF) as usize;
                if length < DataDeliveryControl::WIRE_SIZE || bytes.len() < at + 4 + length {
                    return Err(XrceError::ValueOutOfRange {
                        message: "DataDeliveryControl DHEADER does not match its members",
                    });
                }
                let (dc, _) = DataDeliveryControl::decode(&bytes[at + 4..], e)?;
                // Members a later version appended are skipped.
                cursor = at + 4 + length;
                Some(dc)
            }
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
        self.encode_body_with(e, DeliveryControlForm::Appendable)
    }

    /// As [`Self::encode_body`], with the `DataDeliveryControl` written in
    /// `form`.
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn encode_body_with(
        &self,
        e: Endianness,
        form: DeliveryControlForm,
    ) -> Result<Vec<u8>, XrceError> {
        let mut body = Vec::with_capacity(BaseObjectRequest::WIRE_SIZE + 4);
        body.extend_from_slice(&self.base.encode());
        self.read_specification.encode_into(e, form, &mut body)?;
        Ok(body)
    }

    /// Packs into a `Submessage` (little-endian body).
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        self.into_submessage_with(Endianness::Little, DeliveryControlForm::Appendable)
    }

    /// Packs into a `Submessage` in the byte order `e`, with the
    /// `DataDeliveryControl` written in `form`.
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn into_submessage_with(
        self,
        e: Endianness,
        form: DeliveryControlForm,
    ) -> Result<Submessage, XrceError> {
        let body = self.encode_body_with(e, form)?;
        let flags = if e.is_little() {
            FLAG_E_LITTLE_ENDIAN
        } else {
            0
        };
        Submessage::new(SubmessageId::ReadData, flags, body)
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
        // preferred_stream_id, data_format=0x04 (reserved, 0b010 once the
        // mask is applied), then no optionals.
        let mut body = reader_base().encode().to_vec();
        body.extend_from_slice(&[0x01, 0x04, 0, 0]);
        let sm = Submessage::new(SubmessageId::ReadData, FLAG_E_LITTLE_ENDIAN, body).unwrap();
        assert!(matches!(
            ReadDataPayload::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    /// `tools/wire-capture/xrce-sessions/client-to-agent.hex` line 6 — a
    /// real `READ_DATA` from eProsima Micro-XRCE-DDS Client 3.0.1:
    /// `request_id={0x00,0x0e}`, `object_id=0x0016` (a DataReader),
    /// `preferred_stream_id=0x80` (reliable), `data_format=FORMAT_DATA`,
    /// no content filter, `delivery_control` present with
    /// `max_samples=MAX_SAMPLES_UNLIMITED` and the other three fields 0.
    /// Confirms the presence-flag encoding matches the reference
    /// implementation byte for byte — see the module doc.
    #[test]
    fn read_data_matches_the_captured_read_data_example() {
        let captured: [u8; 16] = [
            0x00, 0x0e, 0x00, 0x16, 0x80, 0x00, 0x00, 0x01, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let sm = Submessage::new(
            SubmessageId::ReadData,
            FLAG_E_LITTLE_ENDIAN,
            captured.to_vec(),
        )
        .unwrap();
        let p = ReadDataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p.base.request_id, [0x00, 0x0e]);
        assert_eq!(p.base.object_id.raw(), 0x0016);
        assert_eq!(p.read_specification.preferred_stream_id, 0x80);
        assert_eq!(p.read_specification.data_format, DataFormat::Data);
        assert_eq!(p.read_specification.content_filter_expression, None);
        assert_eq!(
            p.read_specification.delivery_control,
            Some(DataDeliveryControl {
                max_samples: 0xFFFF,
                max_elapsed_time: 0,
                max_bytes_per_second: 0,
                min_pace_period: 0,
            })
        );
        // Byte-identical re-encoding in the form that client sends ...
        let re = p
            .clone()
            .into_submessage_with(Endianness::Little, DeliveryControlForm::Bare)
            .unwrap();
        assert_eq!(re.body, captured);
        // ... and, in the form of the spec, with the DHEADER of the APPENDABLE
        // DataDeliveryControl (8 members, aligned to 4: padding after the
        // presence octet at 7 is none, the DHEADER sits at 8 of the spec).
        let spec = p.into_submessage().unwrap();
        assert_eq!(spec.body.len(), 20);
        assert_eq!(&spec.body[..8], &captured[..8]);
        assert_eq!(&spec.body[8..12], &[0x08, 0x00, 0x00, 0x00]);
        assert_eq!(&spec.body[12..], &captured[8..]);
        assert_eq!(
            ReadDataPayload::try_from_submessage(&spec)
                .unwrap()
                .read_specification
                .delivery_control,
            Some(DataDeliveryControl {
                max_samples: 0xFFFF,
                max_elapsed_time: 0,
                max_bytes_per_second: 0,
                min_pace_period: 0,
            })
        );
    }

    /// DDS-XRCE 1.0 Annex B.4.2: READ_DATA for samples that match "x>100",
    /// FORMAT_DATA_SEQ, with a DataDeliveryControl. The bytes of the figure
    /// from the BaseObjectRequest on (the figure's DHEADER is
    /// `0x80000008`, bit 31 being no part of an XCDR2 DHEADER).
    ///
    /// Spec offset 0 stream 0x00; 1 data_format 0x08; 2 content_filter? 1;
    /// 3 padding; 4..8 length 6; 8..14 "x>100\0"; 14 delivery_control? 1; 15
    /// padding; 16..20 DHEADER; 20..28 the four unsigned shorts.
    #[test]
    fn annex_b_4_2_read_data_with_filter_and_delivery_control() {
        let mut body = reader_base().encode().to_vec();
        body.extend_from_slice(&[0x00, 0x08, 0x01, 0x00]);
        body.extend_from_slice(&[0x06, 0x00, 0x00, 0x00]);
        body.extend_from_slice(b"x>100\0");
        body.extend_from_slice(&[0x01, 0x00]);
        body.extend_from_slice(&[0x08, 0x00, 0x00, 0x80]);
        body.extend_from_slice(&[0x64, 0x00, 0x30, 0x75, 0x00, 0x04, 0xE8, 0x03]);
        let sm =
            Submessage::new(SubmessageId::ReadData, FLAG_E_LITTLE_ENDIAN, body.clone()).unwrap();
        let p = ReadDataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p.read_specification.preferred_stream_id, 0);
        assert_eq!(p.read_specification.data_format, DataFormat::DataSeq);
        assert_eq!(
            p.read_specification.content_filter_expression.as_deref(),
            Some("x>100")
        );
        assert_eq!(
            p.read_specification.delivery_control,
            Some(DataDeliveryControl {
                max_samples: 100,
                max_elapsed_time: 30_000,
                max_bytes_per_second: 1024,
                min_pace_period: 1000,
            })
        );
        // Written back: the same bytes but the plain DHEADER 8.
        let again = p.into_submessage().unwrap();
        body[20..24].copy_from_slice(&[0x08, 0x00, 0x00, 0x00]);
        assert_eq!(again.body, body);
    }

    /// The filter is an XCDR2 string: its length counts the NUL and is
    /// aligned to 4; a string without its NUL is refused.
    #[test]
    fn the_content_filter_is_a_string_with_a_nul_and_an_aligned_length() {
        let p = ReadDataPayload {
            base: reader_base(),
            read_specification: ReadSpecification {
                preferred_stream_id: 0x01,
                data_format: DataFormat::Data,
                content_filter_expression: Some("a>1".into()),
                delivery_control: None,
            },
        };
        let sm = p.clone().into_submessage().unwrap();
        // 4 base + stream, format, cf? at 4..7 of the payload, padding to 8,
        // length 4 at 8..12, "a>1\0", dc? 0.
        assert_eq!(
            &sm.body[4..],
            &[
                0x01, 0x00, 0x01, 0x00, 0x04, 0x00, 0x00, 0x00, b'a', b'>', b'1', 0x00, 0x00
            ]
        );
        assert_eq!(ReadDataPayload::try_from_submessage(&sm).unwrap(), p);
        let mut no_nul = sm.body.clone();
        no_nul[15] = b'1';
        let sm = Submessage::new(SubmessageId::ReadData, FLAG_E_LITTLE_ENDIAN, no_nul).unwrap();
        assert!(ReadDataPayload::try_from_submessage(&sm).is_err());
    }

    /// Annex A: the `data_format` octet carries FORMAT_DATA 0x00,
    /// FORMAT_SAMPLE 0x02, FORMAT_DATA_SEQ 0x08, FORMAT_SAMPLE_SEQ 0x0A and
    /// FORMAT_PACKED_SAMPLES 0x0E.
    #[test]
    fn the_data_format_octet_has_the_values_of_annex_a() {
        for (format, octet) in [
            (DataFormat::Data, 0x00u8),
            (DataFormat::Sample, 0x02),
            (DataFormat::DataSeq, 0x08),
            (DataFormat::SampleSeq, 0x0A),
            (DataFormat::PackedSamples, 0x0E),
        ] {
            let p = ReadDataPayload {
                base: reader_base(),
                read_specification: ReadSpecification {
                    preferred_stream_id: 0,
                    data_format: format,
                    content_filter_expression: None,
                    delivery_control: None,
                },
            };
            let sm = p.clone().into_submessage().unwrap();
            assert_eq!(sm.body[5], octet);
            assert_eq!(ReadDataPayload::try_from_submessage(&sm).unwrap(), p);
        }
    }
}
