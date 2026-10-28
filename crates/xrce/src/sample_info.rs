// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `SampleInfo` / `SampleInfoDelta` (DDS-XRCE 1.0 §7.7.1, Annex A IDL Types).
//!
//! §7.7.1 names five wire representations for sample data, selected by
//! `DataFormat` (see [`crate::submessages::write_data::DataFormat`]):
//! `SampleData` (FORMAT_DATA), `Sample` (FORMAT_SAMPLE), `SampleDataSeq`
//! (FORMAT_DATA_SEQ), `SampleSeq` (FORMAT_SAMPLE_SEQ) and `PackedSamples`
//! (FORMAT_PACKED_SAMPLES). `SampleData`/FORMAT_DATA is implemented
//! directly in `submessages/write_data.rs`/`submessages/data.rs` (it
//! predates this module). This module is the shared piece the other four
//! formats build on: [`SampleInfo`] (used by `Sample`/`SampleSeq`) and
//! [`SampleInfoDelta`] (used by `PackedSamples`'s `SampleDelta`).
//!
//! ## `SampleInfo` — two definitions in the spec text, and the form that
//! reads both
//!
//! §7.7.1's body text (p. 15) defines it as a flat struct:
//!
//! ```text
//! struct SampleInfo {
//!      SampleInfoFlags state;
//!      unsigned long   sequence_number;
//!      unsigned long   session_time_offset; // ms, up to 53 days
//! };
//! ```
//!
//! Annex A (p. 106) gives `SampleInfo` a `SampleInfoDetail` union switched on
//! a `SampleInfoFormat` octet (`FORMAT_EMPTY`, `FORMAT_SEQNUM`,
//! `FORMAT_TIMESTAMP`, `FORMAT_SEQN_TIMS`); the printed union repeats the case
//! label `FORMAT_TIMESTAMP` and lets `FORMAT_EMPTY` fall through to
//! `FORMAT_SEQNUM`, the intended reading (no member, the sequence number, the
//! time offset, both) is the one used here.
//!
//! The two forms meet: a union of `FORMAT_SEQN_TIMS` is `state`, the
//! discriminator, two octets of padding, the sequence number and the time
//! offset, 12 octets, the same as the flat struct whose first padding octet is
//! the discriminator. The figures of Annex B.5.3, B.5.4 and B.5.5 show those
//! 12 octets (`submessageLength` 40 and 36 only add up against them). So
//! [`SampleInfoForm::AnnexA`](crate::encoding::SampleInfoForm) (the default)
//! writes `FORMAT_SEQN_TIMS` — a reader of the flat form ignores the octet —
//! and reads by the discriminator, with 2, 8, 8 or 12 octets. A client that
//! sends the flat form with zero padding is read as `FORMAT_EMPTY` there; for
//! it [`SampleInfoForm::Flat`](crate::encoding::SampleInfoForm) reads and
//! writes the 12 octets with the padding octet left alone (agent key
//! `sample_info: flat`).
//!
//! ## Sequence formats: this crate is type-agnostic, so it cannot split
//! `SampleDataSeq`/`SampleSeq`/`PackedSamples.sample_delta_seq` elements
//!
//! B.5.2 shows `SampleDataSeq` (`sequence<SampleData>`) as a 4-byte
//! element count followed by the elements' bytes concatenated **with no
//! per-element length** — `data_seq[0].data` runs straight into
//! `data_seq[1].data`. The same holds for `SampleSeq` (B.5.4:
//! `sample_seq[0].info` then `sample_seq[0].data` then
//! `sample_seq[1].info` with no length between them) and for
//! `PackedSamples.sample_delta_seq` (B.5.5). A `SampleData.serialized_data`
//! is, per §7.7.1, "the bytes resulting from serializing the
//! application-specific data type" — its length is only recoverable by
//! running the application type's own decoder over it, which is exactly
//! the knowledge this no_std, application-type-agnostic crate does not
//! have (see the crate doc and `write_data.rs`'s identical reasoning for
//! why lone `SampleData` carries no length either). So this module gives
//! the daemon the two *fixed-size* building blocks it needs to walk a
//! sequence itself ([`SampleInfo::decode`]/[`SampleInfoDelta::decode`],
//! each a known number of bytes), and the four-format payload types in
//! `write_data.rs`/`data.rs` carry the element count plus the raw,
//! undivided remainder — splitting that remainder into `count` samples is
//! the job of whatever layer knows the topic's type (`zerodds-dcps`'s
//! `SerializedTypeSupport`, in the daemon).

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u16, read_u32, write_u16, write_u32};
use crate::error::XrceError;

// ===========================================================================
// SampleInfoFlags (Annex A: `@bit_bound(8) bitmask SampleInfoFlags`)
// ===========================================================================

/// `SampleInfoFlags` — combines DDS `SampleState`/`InstanceState`/
/// `ViewState` into one octet bitmask (§7.7.1, Annex A IDL Types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SampleInfoFlags(pub u8);

impl SampleInfoFlags {
    /// `@position(0)` — the instance was unregistered.
    pub const INSTANCE_STATE_UNREGISTERED: u8 = 1 << 0;
    /// `@position(1)` — the instance was disposed.
    pub const INSTANCE_STATE_DISPOSED: u8 = 1 << 1;
    /// `@position(2)` — first sample of a newly seen instance.
    pub const VIEW_STATE_NEW: u8 = 1 << 2;
    /// `@position(3)` — the sample was already read.
    pub const SAMPLE_STATE_READ: u8 = 1 << 3;

    /// `true` if the unregistered bit is set.
    #[must_use]
    pub fn is_unregistered(self) -> bool {
        (self.0 & Self::INSTANCE_STATE_UNREGISTERED) != 0
    }
    /// `true` if the disposed bit is set.
    #[must_use]
    pub fn is_disposed(self) -> bool {
        (self.0 & Self::INSTANCE_STATE_DISPOSED) != 0
    }
    /// `true` if the view-state-new bit is set.
    #[must_use]
    pub fn is_new(self) -> bool {
        (self.0 & Self::VIEW_STATE_NEW) != 0
    }
    /// `true` if the sample-already-read bit is set.
    #[must_use]
    pub fn is_read(self) -> bool {
        (self.0 & Self::SAMPLE_STATE_READ) != 0
    }
}

// ===========================================================================
// SampleInfo — flat form (§7.7.1 body text, confirmed by Annex B.5.3/4/5)
// ===========================================================================

/// `SampleInfo` (§7.7.1, flat form — see the module doc for why the
/// union-shaped Annex A restatement is not implemented).
///
/// Wire layout, 12 bytes total: `state`(1 byte) + 3 bytes padding (the
/// next member is a 4-aligned `unsigned long`) + `sequence_number`(4) +
/// `session_time_offset`(4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SampleInfo {
    /// Combined DDS `SampleState`/`InstanceState`/`ViewState` bits.
    pub state: SampleInfoFlags,
    /// Sample sequence number.
    pub sequence_number: u32,
    /// Milliseconds since the session timestamp established at
    /// `CREATE_CLIENT` (§7.8.2.1's `client_timestamp`), up to 53 days.
    pub session_time_offset: u32,
}

/// `SampleInfoFormat` (Annex A): which members of `SampleInfoDetail` follow.
pub mod info_format {
    /// `FORMAT_EMPTY`: no member.
    pub const EMPTY: u8 = 0x00;
    /// `FORMAT_SEQNUM`: the sequence number.
    pub const SEQNUM: u8 = 0x01;
    /// `FORMAT_TIMESTAMP`: the session time offset.
    pub const TIMESTAMP: u8 = 0x02;
    /// `FORMAT_SEQN_TIMS`: both.
    pub const SEQN_TIMS: u8 = 0x03;
}

impl SampleInfo {
    /// Wire size of the full form: 12 bytes (1 + 1 discriminator or padding +
    /// 2 padding + 4 + 4).
    pub const WIRE_SIZE: usize = 12;

    /// Appends the 12-byte encoding to `out` in the form of the process
    /// ([`crate::encoding::sample_info_form`]): the octet after `state` is
    /// `FORMAT_SEQN_TIMS` (Annex A) or zero padding (§7.7.1).
    ///
    /// # Errors
    /// None expected; kept `Result` for symmetry with the rest of the crate.
    pub fn encode_into(&self, e: Endianness, out: &mut Vec<u8>) -> Result<(), XrceError> {
        let mut buf = [0u8; Self::WIRE_SIZE];
        buf[0] = self.state.0;
        if crate::encoding::sample_info_form() == crate::encoding::SampleInfoForm::AnnexA {
            buf[1] = info_format::SEQN_TIMS;
        }
        // buf[2..4] stay zero: padding to the 4-aligned sequence_number.
        write_u32(&mut buf[4..8], self.sequence_number, e)?;
        write_u32(&mut buf[8..12], self.session_time_offset, e)?;
        out.extend_from_slice(&buf);
        Ok(())
    }

    /// Decodes a `SampleInfo`; returns it and the bytes consumed. In the form
    /// of Annex A the discriminator in the second octet selects the members
    /// (2 bytes for `FORMAT_EMPTY`, 8 for `FORMAT_SEQNUM` and
    /// `FORMAT_TIMESTAMP`, 12 for `FORMAT_SEQN_TIMS`; a member that is not
    /// there reads as 0); in the flat form it is padding and the info is 12
    /// bytes.
    ///
    /// # Errors
    /// `UnexpectedEof` if the bytes end inside the info; `ValueOutOfRange` for
    /// a `SampleInfoFormat` that is none of the four.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        let eof = |needed: usize| XrceError::UnexpectedEof {
            needed,
            offset: bytes.len(),
        };
        if bytes.len() < 2 {
            return Err(eof(Self::WIRE_SIZE));
        }
        let format = match crate::encoding::sample_info_form() {
            crate::encoding::SampleInfoForm::Flat => info_format::SEQN_TIMS,
            crate::encoding::SampleInfoForm::AnnexA => bytes[1],
        };
        let state = SampleInfoFlags(bytes[0]);
        match format {
            info_format::EMPTY => Ok((
                Self {
                    state,
                    sequence_number: 0,
                    session_time_offset: 0,
                },
                2,
            )),
            info_format::SEQNUM | info_format::TIMESTAMP => {
                let b = bytes.get(4..8).ok_or_else(|| eof(8))?;
                let value = read_u32(b, e)?;
                let (sequence_number, session_time_offset) = if format == info_format::SEQNUM {
                    (value, 0)
                } else {
                    (0, value)
                };
                Ok((
                    Self {
                        state,
                        sequence_number,
                        session_time_offset,
                    },
                    8,
                ))
            }
            info_format::SEQN_TIMS => {
                if bytes.len() < Self::WIRE_SIZE {
                    return Err(eof(Self::WIRE_SIZE));
                }
                Ok((
                    Self {
                        state,
                        sequence_number: read_u32(&bytes[4..8], e)?,
                        session_time_offset: read_u32(&bytes[8..12], e)?,
                    },
                    Self::WIRE_SIZE,
                ))
            }
            _ => Err(XrceError::ValueOutOfRange {
                message: "SampleInfoFormat is none of FORMAT_EMPTY, _SEQNUM, _TIMESTAMP, _SEQN_TIMS",
            }),
        }
    }
}

// ===========================================================================
// SampleInfoDelta / SampleDelta framing (FORMAT_PACKED_SAMPLES)
// ===========================================================================

/// `SampleInfoDelta` (§7.7.1) — the compact per-sample delta
/// `PackedSamples.sample_delta_seq` elements carry against the packet's
/// common `info_base`.
///
/// Wire layout, 4 bytes, no padding needed (the 2-byte `timestamp_delta`
/// starts at offset 2, already 2-aligned): `state`(1) +
/// `seq_number_delta`(1, `octet`) + `timestamp_delta`(2,
/// `DeciSecond`=`unsigned short`, tenths of a second).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SampleInfoDelta {
    /// Combined DDS state bits for this sample.
    pub state: SampleInfoFlags,
    /// Added to `info_base.sequence_number` to get this sample's real
    /// sequence number (§7.7.1's `resulting_info` formula). §7.7.1 caps
    /// `PackedSamples` to samples within 256 of each other in sequence
    /// number, which is exactly `octet`'s range.
    pub seq_number_delta: u8,
    /// Tenths of a second, added to `info_base.session_time_offset` (as
    /// milliseconds: `timestamp_delta * 100`). §7.7.1 caps `PackedSamples`
    /// to samples within 100 minutes of each other.
    pub timestamp_delta: u16,
}

impl SampleInfoDelta {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Appends the 4-byte encoding to `out`.
    ///
    /// # Errors
    /// None expected.
    pub fn encode_into(&self, e: Endianness, out: &mut Vec<u8>) -> Result<(), XrceError> {
        out.push(self.state.0);
        out.push(self.seq_number_delta);
        let mut ts = [0u8; 2];
        write_u16(&mut ts, self.timestamp_delta, e)?;
        out.extend_from_slice(&ts);
        Ok(())
    }

    /// Decodes 4 bytes. Returns `(delta, bytes_consumed)`.
    ///
    /// # Errors
    /// `UnexpectedEof` if fewer than 4 bytes are available.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        if bytes.len() < Self::WIRE_SIZE {
            return Err(XrceError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: bytes.len(),
            });
        }
        Ok((
            Self {
                state: SampleInfoFlags(bytes[0]),
                seq_number_delta: bytes[1],
                timestamp_delta: read_u16(&bytes[2..4], e)?,
            },
            Self::WIRE_SIZE,
        ))
    }

    /// Resolves the real `SampleInfo` this delta describes, given the
    /// packet's common `info_base` — §7.7.1's `resulting_info` formula:
    /// `resulting_info.state := info_delta.state`,
    /// `resulting_info.sequence_number := info_base.sequence_number +
    /// info_delta.seq_number_delta`, `resulting_info.session_time_offset :=
    /// info_base.session_time_offset + info_delta.timestamp_delta` (the
    /// last in the delta's native unit, tenths of a second; this method
    /// converts to `info_base`'s milliseconds).
    #[must_use]
    pub fn resolve(&self, info_base: &SampleInfo) -> SampleInfo {
        SampleInfo {
            state: self.state,
            sequence_number: info_base
                .sequence_number
                .wrapping_add(u32::from(self.seq_number_delta)),
            session_time_offset: info_base
                .session_time_offset
                .wrapping_add(u32::from(self.timestamp_delta) * 100),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sample_info_flags_bit_positions() {
        let f = SampleInfoFlags(SampleInfoFlags::INSTANCE_STATE_DISPOSED);
        assert!(f.is_disposed());
        assert!(!f.is_unregistered());
        assert!(!f.is_new());
        assert!(!f.is_read());
    }

    #[test]
    fn sample_info_flags_all_four_bits_independent() {
        let f = SampleInfoFlags(
            SampleInfoFlags::INSTANCE_STATE_UNREGISTERED
                | SampleInfoFlags::VIEW_STATE_NEW
                | SampleInfoFlags::SAMPLE_STATE_READ,
        );
        assert!(f.is_unregistered());
        assert!(!f.is_disposed());
        assert!(f.is_new());
        assert!(f.is_read());
    }

    #[test]
    fn sample_info_roundtrip_le() {
        let info = SampleInfo {
            state: SampleInfoFlags(SampleInfoFlags::SAMPLE_STATE_READ),
            sequence_number: 0x1234_5678,
            session_time_offset: 42,
        };
        let mut out = Vec::new();
        info.encode_into(Endianness::Little, &mut out).unwrap();
        assert_eq!(out.len(), SampleInfo::WIRE_SIZE);
        let (back, n) = SampleInfo::decode(&out, Endianness::Little).unwrap();
        assert_eq!(back, info);
        assert_eq!(n, 12);
    }

    #[test]
    fn sample_info_roundtrip_be() {
        let info = SampleInfo {
            state: SampleInfoFlags(0),
            sequence_number: 7,
            session_time_offset: 900_000,
        };
        let mut out = Vec::new();
        info.encode_into(Endianness::Big, &mut out).unwrap();
        let (back, _) = SampleInfo::decode(&out, Endianness::Big).unwrap();
        assert_eq!(back, info);
    }

    /// DDS-XRCE 1.0 Annex B.5.3 ("Receiving a single sample that includes
    /// SampleInfo") worked example: `info.state`/`info.sequence_number`/
    /// `info.session_time_offset` are three consecutive 4-byte rows — 1
    /// byte of `state`, 3 bytes padding, then two 4-byte `unsigned long`s.
    /// The spec's own example encodes big-endian; this crate always
    /// encodes little-endian (`FLAG_E_LITTLE_ENDIAN`, matching every other
    /// submessage body and the real eProsima capture), so this test checks
    /// the *layout* (offsets and widths) the example establishes rather
    /// than reproducing its byte values verbatim.
    #[test]
    fn sample_info_matches_annex_b_5_3_layout() {
        let info = SampleInfo {
            state: SampleInfoFlags(SampleInfoFlags::VIEW_STATE_NEW),
            sequence_number: 1,
            session_time_offset: 0,
        };
        let mut out = Vec::new();
        crate::encoding::with_sample_info_form(crate::encoding::SampleInfoForm::Flat, || {
            info.encode_into(Endianness::Little, &mut out).unwrap();
        });
        assert_eq!(out.len(), 12, "SampleInfo is 3 rows of 4 bytes");
        assert_eq!(out[0], SampleInfoFlags::VIEW_STATE_NEW, "row 1: state");
        assert_eq!(&out[1..4], &[0, 0, 0], "row 1: padding to the next u32");
        assert_eq!(&out[4..8], &1u32.to_le_bytes(), "row 2: sequence_number");
        assert_eq!(
            &out[8..12],
            &0u32.to_le_bytes(),
            "row 3: session_time_offset"
        );
    }

    #[test]
    fn sample_info_decode_short_is_eof() {
        let mut short = [0u8; 11];
        short[1] = info_format::SEQN_TIMS;
        let res = SampleInfo::decode(&short, Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 12, .. })
        ));
    }

    #[test]
    fn sample_info_delta_roundtrip() {
        let d = SampleInfoDelta {
            state: SampleInfoFlags(SampleInfoFlags::SAMPLE_STATE_READ),
            seq_number_delta: 200,
            timestamp_delta: 55,
        };
        let mut out = Vec::new();
        d.encode_into(Endianness::Little, &mut out).unwrap();
        assert_eq!(out.len(), 4);
        let (back, n) = SampleInfoDelta::decode(&out, Endianness::Little).unwrap();
        assert_eq!(back, d);
        assert_eq!(n, 4);
    }

    #[test]
    fn sample_info_delta_decode_short_is_eof() {
        let res = SampleInfoDelta::decode(&[0u8; 3], Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 4, .. })
        ));
    }

    /// §7.7.1's `resulting_info` formula, worked by hand.
    #[test]
    fn sample_info_delta_resolve_adds_to_base() {
        let base = SampleInfo {
            state: SampleInfoFlags(0),
            sequence_number: 1000,
            session_time_offset: 5_000,
        };
        let delta = SampleInfoDelta {
            state: SampleInfoFlags(SampleInfoFlags::SAMPLE_STATE_READ),
            seq_number_delta: 3,
            timestamp_delta: 12, // 1.2s = 1200ms
        };
        let resolved = delta.resolve(&base);
        assert_eq!(resolved.state, delta.state, "state comes from the delta");
        assert_eq!(resolved.sequence_number, 1003);
        assert_eq!(resolved.session_time_offset, 5_000 + 1_200);
    }

    #[test]
    fn sample_info_delta_resolve_wraps_on_overflow() {
        let base = SampleInfo {
            state: SampleInfoFlags(0),
            sequence_number: u32::MAX,
            session_time_offset: u32::MAX,
        };
        let delta = SampleInfoDelta {
            state: SampleInfoFlags(0),
            seq_number_delta: 1,
            timestamp_delta: 1,
        };
        let resolved = delta.resolve(&base);
        assert_eq!(resolved.sequence_number, 0);
        assert_eq!(resolved.session_time_offset, 99); // wraps: MAX + 100 - 2^32
    }

    /// Annex A: `state`, then `SampleInfoDetail` by its discriminator. The
    /// full form is the 12 octets of the flat struct with the discriminator in
    /// the first padding octet; the shorter ones carry one member or none.
    #[test]
    fn the_union_of_annex_a_is_read_by_its_discriminator() {
        use crate::encoding::{SampleInfoForm, with_sample_info_form};
        let le = |v: u32| v.to_le_bytes();
        with_sample_info_form(SampleInfoForm::AnnexA, || {
            // FORMAT_SEQN_TIMS: written and read back.
            let info = SampleInfo {
                state: SampleInfoFlags(SampleInfoFlags::INSTANCE_STATE_DISPOSED),
                sequence_number: 5,
                session_time_offset: 70,
            };
            let mut out = Vec::new();
            info.encode_into(Endianness::Little, &mut out).unwrap();
            assert_eq!(out[..4], [0x02, info_format::SEQN_TIMS, 0, 0]);
            assert_eq!(
                SampleInfo::decode(&out, Endianness::Little).unwrap(),
                (info, 12)
            );
            // FORMAT_EMPTY: the state alone.
            let (i, n) = SampleInfo::decode(&[0x04, 0x00, 9, 9], Endianness::Little).unwrap();
            assert_eq!(
                (i.state.0, i.sequence_number, i.session_time_offset, n),
                (4, 0, 0, 2)
            );
            // FORMAT_SEQNUM and FORMAT_TIMESTAMP: one number, aligned to 4.
            let mut b = vec![0x01, info_format::SEQNUM, 0, 0];
            b.extend_from_slice(&le(33));
            assert_eq!(
                SampleInfo::decode(&b, Endianness::Little).unwrap(),
                (
                    SampleInfo {
                        state: SampleInfoFlags(1),
                        sequence_number: 33,
                        session_time_offset: 0
                    },
                    8
                )
            );
            b[1] = info_format::TIMESTAMP;
            assert_eq!(
                SampleInfo::decode(&b, Endianness::Little)
                    .unwrap()
                    .0
                    .session_time_offset,
                33
            );
            // A discriminator of no format.
            assert!(
                SampleInfo::decode(&[0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Endianness::Little)
                    .is_err()
            );
        });
    }

    /// §7.7.1 flat form: the octet after `state` is padding whatever it holds,
    /// and the info is always 12 octets.
    #[test]
    fn the_flat_form_ignores_the_padding_octet() {
        use crate::encoding::{SampleInfoForm, with_sample_info_form};
        let mut flat = vec![0x08, 0x00, 0, 0];
        flat.extend_from_slice(&7u32.to_le_bytes());
        flat.extend_from_slice(&9u32.to_le_bytes());
        with_sample_info_form(SampleInfoForm::Flat, || {
            let (i, n) = SampleInfo::decode(&flat, Endianness::Little).unwrap();
            assert_eq!((i.sequence_number, i.session_time_offset, n), (7, 9, 12));
        });
        // The same octets are FORMAT_EMPTY to a reader of Annex A.
        with_sample_info_form(SampleInfoForm::AnnexA, || {
            assert_eq!(SampleInfo::decode(&flat, Endianness::Little).unwrap().1, 2);
        });
    }
}
