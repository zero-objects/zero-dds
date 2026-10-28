// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `TransportLocator` wire encoding (DDS-XRCE 1.0 Annex A, §11.2.1, §11.3.1).
//!
//! ```text
//! @bit_bound(8) enum TransportLocatorFormat {
//!      ADDRESS_FORMAT_SMALL, ADDRESS_FORMAT_MEDIUM,
//!      ADDRESS_FORMAT_LARGE, ADDRESS_FORMAT_STRING };
//! struct TransportLocatorSmall  { octet address[2];  octet locator_port; };
//! struct TransportLocatorMedium { octet address[4];  unsigned short locator_port; };
//! struct TransportLocatorLarge  { octet address[16]; unsigned long locator_port; };
//! struct TransportLocatorString { string value; };
//! union TransportLocator switch (TransportLocatorFormat) { ... };
//! typedef sequence<TransportLocator> TransportLocatorSeq;
//! ```
//!
//! The union is a one-octet discriminator and the member. Scalars are
//! aligned to their size, at most 4 (XCDR2), counted from the start of the
//! payload they are part of, and written in the byte order of the
//! submessage's E flag (§8.3.4.2). So a `Medium` locator that starts at
//! offset 0 is the discriminator, four address octets, one padding octet,
//! and the port as a 16-bit number; a `Large` locator the discriminator, 16
//! address octets, three padding octets and the port as a 32-bit number.
//!
//! UDP v4 and TCP v4 use `Medium` (§11.2.1, §11.3.1), UDP v6 and TCP v6
//! `Large`, with the IPv6 address in `address` and the port number in the
//! port. `Small` is the format Annex A gives (two address octets and a
//! one-octet port) and no transport mapping selects.
//!
//! [`TransportLocator::encode_into`] and [`TransportLocator::decode_at`] take
//! the position of the locator in its payload, because the padding depends
//! on it; [`TransportLocator::encode`] and [`TransportLocator::decode`] are
//! the forms for a locator that starts a payload.
//!
//! This file encapsulates only the wire format. Resolving a string locator to
//! a `SocketAddr` is the task of the transport layer.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encoding::{
    Endianness, UnannotatedTypes, begin_unannotated, end_unannotated, read_dheader,
    unannotated_types,
};
use crate::error::XrceError;

/// `TransportLocatorFormat` (Annex A).
pub mod fmt_disc {
    /// `ADDRESS_FORMAT_SMALL`.
    pub const SMALL: u8 = 0x00;
    /// `ADDRESS_FORMAT_MEDIUM`.
    pub const MEDIUM: u8 = 0x01;
    /// `ADDRESS_FORMAT_LARGE`.
    pub const LARGE: u8 = 0x02;
    /// `ADDRESS_FORMAT_STRING` (e.g. `"udpv4://192.168.0.5:7400"`).
    pub const STRING: u8 = 0x03;
}

/// `TransportLocatorSmall { octet address[2]; octet locator_port; }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorSmall {
    /// The two address octets.
    pub address: [u8; 2],
    /// The one-octet port.
    pub port: u8,
}

/// `TransportLocatorMedium { octet address[4]; unsigned short locator_port; }`
/// — the locator of UDP v4 (§11.2.1) and TCP v4 (§11.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorMedium {
    /// The IPv4 address.
    pub address: [u8; 4],
    /// The port number.
    pub port: u16,
}

/// `TransportLocatorLarge { octet address[16]; unsigned long locator_port; }`
/// — the locator of UDP v6 (§11.2.1) and TCP v6 (§11.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportLocatorLarge {
    /// The IPv6 address.
    pub address: [u8; 16],
    /// The port number.
    pub port: u32,
}

/// Cap for the string of a string locator, against a malicious length: the
/// most a submessage can hold (`submessageLength` is 16 bit, §8.3.4.3).
pub const TRANSPORT_LOCATOR_STRING_MAX: usize = 65_535;

/// Cap for the number of locators in a `TransportLocatorSeq` on decode.
pub const TRANSPORT_LOCATOR_SEQ_MAX: usize = 1024;

/// `TransportLocator` (Annex A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportLocator {
    /// `ADDRESS_FORMAT_SMALL`.
    Small(TransportLocatorSmall),
    /// `ADDRESS_FORMAT_MEDIUM`.
    Medium(TransportLocatorMedium),
    /// `ADDRESS_FORMAT_LARGE`.
    Large(TransportLocatorLarge),
    /// `ADDRESS_FORMAT_STRING`.
    String(String),
}

fn align_out(out: &mut Vec<u8>, n: usize) {
    while out.len() % n != 0 {
        out.push(0);
    }
}

fn align_pos(pos: usize, n: usize) -> usize {
    pos.div_ceil(n) * n
}

fn eof(needed: usize, have: usize) -> XrceError {
    XrceError::UnexpectedEof {
        needed,
        offset: have,
    }
}

fn put_u16(out: &mut Vec<u8>, v: u16, e: Endianness) {
    out.extend_from_slice(&match e {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    });
}

fn put_u32(out: &mut Vec<u8>, v: u32, e: Endianness) {
    out.extend_from_slice(&match e {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    });
}

fn take(bytes: &[u8], pos: usize, n: usize) -> Result<&[u8], XrceError> {
    bytes
        .get(
            pos..pos
                .checked_add(n)
                .ok_or_else(|| eof(usize::MAX, bytes.len()))?,
        )
        .ok_or_else(|| eof(pos.saturating_add(n), bytes.len()))
}

impl TransportLocator {
    /// The `TransportLocatorFormat` discriminator.
    #[must_use]
    pub fn discriminator(&self) -> u8 {
        match self {
            Self::Small(_) => fmt_disc::SMALL,
            Self::Medium(_) => fmt_disc::MEDIUM,
            Self::Large(_) => fmt_disc::LARGE,
            Self::String(_) => fmt_disc::STRING,
        }
    }

    /// Appends the locator to `out`, which starts the payload: the padding
    /// is counted from `out[0]`.
    ///
    /// # Errors
    /// `PayloadTooLarge` for a string longer than
    /// [`TRANSPORT_LOCATOR_STRING_MAX`].
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        // The union and the struct of its case carry no @extensibility (Annex
        // A): APPENDABLE, each with a DHEADER, unless the setting is `Final`.
        let union_members = begin_unannotated(out);
        out.push(self.discriminator());
        let case_members = begin_unannotated(out);
        self.encode_member(out, e)?;
        end_unannotated(out, case_members, e);
        end_unannotated(out, union_members, e);
        Ok(())
    }

    fn encode_member(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        match self {
            Self::Small(s) => {
                out.extend_from_slice(&s.address);
                out.push(s.port);
            }
            Self::Medium(m) => {
                out.extend_from_slice(&m.address);
                align_out(out, 2);
                put_u16(out, m.port, e);
            }
            Self::Large(l) => {
                out.extend_from_slice(&l.address);
                align_out(out, 4);
                put_u32(out, l.port, e);
            }
            Self::String(s) => {
                if s.len() > TRANSPORT_LOCATOR_STRING_MAX {
                    return Err(XrceError::PayloadTooLarge {
                        limit: TRANSPORT_LOCATOR_STRING_MAX,
                        actual: s.len(),
                    });
                }
                let len = u32::try_from(s.len() + 1).map_err(|_| XrceError::ValueOutOfRange {
                    message: "TransportLocator string too long",
                })?;
                align_out(out, 4);
                put_u32(out, len, e);
                out.extend_from_slice(s.as_bytes());
                out.push(0);
            }
        }
        Ok(())
    }

    /// Encodes a locator that starts its payload.
    ///
    /// # Errors
    /// As [`Self::encode_into`].
    pub fn encode(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::new();
        self.encode_into(&mut out, e)?;
        Ok(out)
    }

    /// Reads the locator that starts at `pos` of `bytes`, which starts the
    /// payload. Returns the locator and the position after it.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` (an unknown format, a string that is
    /// not UTF-8 or lacks its NUL), `PayloadTooLarge`.
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let appendable = || -> Result<(Self, usize), XrceError> {
            let (start, end) =
                read_dheader(bytes, pos, e).ok_or_else(|| eof(pos + 8, bytes.len()))?;
            let disc = *bytes
                .get(start)
                .ok_or_else(|| eof(start + 1, bytes.len()))?;
            let (case_start, case_end) =
                read_dheader(bytes, start + 1, e).ok_or_else(|| eof(start + 5, bytes.len()))?;
            let (locator, next) = Self::decode_member(disc, bytes, case_start, e)?;
            if next != case_end || case_end != end {
                return Err(XrceError::ValueOutOfRange {
                    message: "TransportLocator: the DHEADERs do not match the case",
                });
            }
            Ok((locator, end))
        };
        let direct = || -> Result<(Self, usize), XrceError> {
            let disc = *bytes.get(pos).ok_or_else(|| eof(pos + 1, bytes.len()))?;
            Self::decode_member(disc, bytes, pos + 1, e)
        };
        match unannotated_types() {
            UnannotatedTypes::Appendable => appendable().or_else(|_| direct()),
            UnannotatedTypes::Final => direct().or_else(|_| appendable()),
        }
    }

    /// The member of the case `disc`, which starts at `at`.
    fn decode_member(
        disc: u8,
        bytes: &[u8],
        at: usize,
        e: Endianness,
    ) -> Result<(Self, usize), XrceError> {
        let mut at = at;
        match disc {
            fmt_disc::SMALL => {
                let b = take(bytes, at, 3)?;
                Ok((
                    Self::Small(TransportLocatorSmall {
                        address: [b[0], b[1]],
                        port: b[2],
                    }),
                    at + 3,
                ))
            }
            fmt_disc::MEDIUM => {
                let mut address = [0u8; 4];
                address.copy_from_slice(take(bytes, at, 4)?);
                at = align_pos(at + 4, 2);
                let b = take(bytes, at, 2)?;
                let port = match e {
                    Endianness::Little => u16::from_le_bytes([b[0], b[1]]),
                    Endianness::Big => u16::from_be_bytes([b[0], b[1]]),
                };
                Ok((
                    Self::Medium(TransportLocatorMedium { address, port }),
                    at + 2,
                ))
            }
            fmt_disc::LARGE => {
                let mut address = [0u8; 16];
                address.copy_from_slice(take(bytes, at, 16)?);
                at = align_pos(at + 16, 4);
                let b = take(bytes, at, 4)?;
                let raw = [b[0], b[1], b[2], b[3]];
                let port = match e {
                    Endianness::Little => u32::from_le_bytes(raw),
                    Endianness::Big => u32::from_be_bytes(raw),
                };
                Ok((Self::Large(TransportLocatorLarge { address, port }), at + 4))
            }
            fmt_disc::STRING => {
                at = align_pos(at, 4);
                let b = take(bytes, at, 4)?;
                let raw = [b[0], b[1], b[2], b[3]];
                let len = match e {
                    Endianness::Little => u32::from_le_bytes(raw),
                    Endianness::Big => u32::from_be_bytes(raw),
                } as usize;
                at += 4;
                if len == 0 || len > TRANSPORT_LOCATOR_STRING_MAX + 1 {
                    return Err(XrceError::PayloadTooLarge {
                        limit: TRANSPORT_LOCATOR_STRING_MAX,
                        actual: len,
                    });
                }
                let raw = take(bytes, at, len)?;
                let Some((&0, text)) = raw.split_last() else {
                    return Err(XrceError::ValueOutOfRange {
                        message: "TransportLocator string lacks its NUL terminator",
                    });
                };
                let s = core::str::from_utf8(text)
                    .map(alloc::string::ToString::to_string)
                    .map_err(|_| XrceError::ValueOutOfRange {
                        message: "TransportLocator string is not valid utf-8",
                    })?;
                Ok((Self::String(s), at + len))
            }
            _ => Err(XrceError::ValueOutOfRange {
                message: "unknown TransportLocator discriminator",
            }),
        }
    }

    /// Decodes a locator that starts its payload. Returns the locator and
    /// the bytes consumed.
    ///
    /// # Errors
    /// As [`Self::decode_at`].
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_at(bytes, 0, e)
    }
}

/// Appends a `TransportLocatorSeq` to `out`, which starts the payload: the
/// element count as an `unsigned long`, then the locators.
///
/// # Errors
/// As [`TransportLocator::encode_into`].
pub fn encode_locator_seq(
    locators: &[TransportLocator],
    out: &mut Vec<u8>,
    e: Endianness,
) -> Result<(), XrceError> {
    let count = u32::try_from(locators.len()).map_err(|_| XrceError::ValueOutOfRange {
        message: "TransportLocatorSeq length exceeds u32",
    })?;
    align_out(out, 4);
    put_u32(out, count, e);
    for l in locators {
        l.encode_into(out, e)?;
    }
    Ok(())
}

/// Reads a `TransportLocatorSeq` that starts at `pos` of `bytes`, which
/// starts the payload. Returns the locators and the position after them.
///
/// # Errors
/// As [`TransportLocator::decode_at`]; `PayloadTooLarge` for more than
/// [`TRANSPORT_LOCATOR_SEQ_MAX`] locators or a count the bytes cannot hold.
pub fn decode_locator_seq(
    bytes: &[u8],
    pos: usize,
    e: Endianness,
) -> Result<(Vec<TransportLocator>, usize), XrceError> {
    let at = align_pos(pos, 4);
    let b = take(bytes, at, 4)?;
    let raw = [b[0], b[1], b[2], b[3]];
    let count = match e {
        Endianness::Little => u32::from_le_bytes(raw),
        Endianness::Big => u32::from_be_bytes(raw),
    } as usize;
    // A locator takes at least four octets.
    if count > TRANSPORT_LOCATOR_SEQ_MAX || count > bytes.len().saturating_sub(at + 4) / 4 + 1 {
        return Err(XrceError::PayloadTooLarge {
            limit: TRANSPORT_LOCATOR_SEQ_MAX,
            actual: count,
        });
    }
    let mut at = at + 4;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let (l, next) = TransportLocator::decode_at(bytes, at, e)?;
        out.push(l);
        at = next;
    }
    Ok((out, at))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    /// The form of the reference implementation: no DHEADER.
    fn final_form<R>(f: impl FnOnce() -> R) -> R {
        crate::encoding::with_unannotated_types(crate::encoding::UnannotatedTypes::Final, f)
    }

    /// The union and the struct of its case carry no @extensibility (Annex
    /// A): APPENDABLE, a DHEADER each (DDS-XTypes 1.3 §7.3.1.2.1.8).
    #[test]
    fn by_default_the_union_and_its_case_have_a_dheader() {
        let le = medium(7400).encode(Endianness::Little).unwrap();
        assert_eq!(
            le,
            alloc::vec![
                14, 0, 0, 0, 1, 0, 0, 0, 6, 0, 0, 0, 192, 168, 0, 5, 0xE8, 0x1C
            ]
        );
        let (back, n) = TransportLocator::decode(&le, Endianness::Little).unwrap();
        assert_eq!(back, medium(7400));
        assert_eq!(n, le.len());
        let small = TransportLocator::Small(TransportLocatorSmall {
            address: [10, 1],
            port: 77,
        });
        let enc = small.encode(Endianness::Big).unwrap();
        assert_eq!(
            enc,
            alloc::vec![0, 0, 0, 11, 0, 0, 0, 0, 0, 0, 0, 3, 10, 1, 77]
        );
        assert_eq!(
            TransportLocator::decode(&enc, Endianness::Big).unwrap().0,
            small
        );
        // Both forms are read in either setting.
        let direct = final_form(|| medium(7400).encode(Endianness::Little).unwrap());
        assert_eq!(direct, alloc::vec![1, 192, 168, 0, 5, 0, 0xE8, 0x1C]);
        assert_eq!(
            TransportLocator::decode(&direct, Endianness::Little)
                .unwrap()
                .0,
            medium(7400)
        );
        final_form(|| {
            assert_eq!(
                TransportLocator::decode(&le, Endianness::Little).unwrap().0,
                medium(7400)
            );
        });
    }

    fn medium(port: u16) -> TransportLocator {
        TransportLocator::Medium(TransportLocatorMedium {
            address: [192, 168, 0, 5],
            port,
        })
    }

    /// Annex A: `struct TransportLocatorMedium { octet address[4]; unsigned
    /// short locator_port; }` as the member of a union with a one-octet
    /// discriminator. The `unsigned short` is aligned to 2: one padding octet
    /// after the address, then the port in the byte order of the E flag.
    #[test]
    fn medium_has_a_padding_octet_and_a_port_in_the_byte_order_of_the_flag() {
        final_form(|| {
            let le = medium(7400).encode(Endianness::Little).unwrap();
            assert_eq!(le, alloc::vec![1, 192, 168, 0, 5, 0, 0xE8, 0x1C]);
            let be = medium(7400).encode(Endianness::Big).unwrap();
            assert_eq!(be, alloc::vec![1, 192, 168, 0, 5, 0, 0x1C, 0xE8]);
            for (bytes, e) in [(le, Endianness::Little), (be, Endianness::Big)] {
                let (back, n) = TransportLocator::decode(&bytes, e).unwrap();
                assert_eq!(back, medium(7400));
                assert_eq!(n, 8);
            }
        });
    }

    /// `TransportLocatorLarge { octet address[16]; unsigned long
    /// locator_port; }`: the `unsigned long` is aligned to 4, three padding
    /// octets after the 16 address octets.
    #[test]
    fn large_has_three_padding_octets_and_a_32_bit_port() {
        final_form(|| {
            let address = [
                0xFE, 0x80, 0, 0, 0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
            ];
            let l = TransportLocator::Large(TransportLocatorLarge {
                address,
                port: 7400,
            });
            let le = l.encode(Endianness::Little).unwrap();
            assert_eq!(le.len(), 1 + 16 + 3 + 4);
            assert_eq!(le[0], 2);
            assert_eq!(&le[1..17], &address);
            assert_eq!(&le[17..20], &[0, 0, 0]);
            assert_eq!(&le[20..], &[0xE8, 0x1C, 0, 0]);
            let be = l.encode(Endianness::Big).unwrap();
            assert_eq!(&be[20..], &[0, 0, 0x1C, 0xE8]);
            assert_eq!(
                TransportLocator::decode(&le, Endianness::Little).unwrap(),
                (l.clone(), 24)
            );
            assert_eq!(
                TransportLocator::decode(&be, Endianness::Big).unwrap(),
                (l, 24)
            );
        });
    }

    /// Annex A: `TransportLocatorSmall { octet address[2]; octet
    /// locator_port; }` — three octets after the discriminator, nothing
    /// aligned.
    #[test]
    fn small_is_two_address_octets_and_a_one_octet_port() {
        final_form(|| {
            let l = TransportLocator::Small(TransportLocatorSmall {
                address: [10, 1],
                port: 77,
            });
            let enc = l.encode(Endianness::Little).unwrap();
            assert_eq!(enc, alloc::vec![0, 10, 1, 77]);
            assert_eq!(
                TransportLocator::decode(&enc, Endianness::Big).unwrap(),
                (l, 4)
            );
        });
    }

    /// A `string` is an `unsigned long` length (aligned to 4, so three
    /// padding octets after the discriminator), the characters and a NUL.
    #[test]
    fn string_length_is_aligned_to_four() {
        final_form(|| {
            let l = TransportLocator::String("udpv4://1.2.3.4:5".into());
            let enc = l.encode(Endianness::Little).unwrap();
            assert_eq!(enc[0], 3);
            assert_eq!(&enc[1..4], &[0, 0, 0]);
            assert_eq!(&enc[4..8], &18u32.to_le_bytes());
            assert_eq!(&enc[8..25], b"udpv4://1.2.3.4:5");
            assert_eq!(enc[25], 0);
            assert_eq!(
                TransportLocator::decode(&enc, Endianness::Little).unwrap(),
                (l.clone(), enc.len())
            );
            let be = l.encode(Endianness::Big).unwrap();
            assert_eq!(&be[4..8], &18u32.to_be_bytes());
            assert_eq!(TransportLocator::decode(&be, Endianness::Big).unwrap().0, l);
        });
    }

    /// The spec puts no cap of 256 on the string; only the size of a
    /// submessage limits it.
    #[test]
    fn a_long_string_locator_is_not_cut_at_256() {
        let l = TransportLocator::String("a".repeat(1000));
        let enc = l.encode(Endianness::Little).unwrap();
        assert_eq!(
            TransportLocator::decode(&enc, Endianness::Little)
                .unwrap()
                .0,
            l
        );
        let too_long = TransportLocator::String("a".repeat(TRANSPORT_LOCATOR_STRING_MAX + 1));
        assert!(matches!(
            too_long.encode(Endianness::Little),
            Err(XrceError::PayloadTooLarge { .. })
        ));
    }

    /// The padding depends on where the locator sits in its payload: after a
    /// one-octet prefix the same Medium locator needs no padding before its
    /// port (the port then starts at offset 6).
    #[test]
    fn padding_is_counted_from_the_start_of_the_payload() {
        final_form(|| {
            let mut out = alloc::vec![0xAAu8];
            medium(7400)
                .encode_into(&mut out, Endianness::Little)
                .unwrap();
            assert_eq!(out, alloc::vec![0xAA, 1, 192, 168, 0, 5, 0xE8, 0x1C]);
            let (back, end) = TransportLocator::decode_at(&out, 1, Endianness::Little).unwrap();
            assert_eq!(back, medium(7400));
            assert_eq!(end, out.len());
        });
    }

    #[test]
    fn a_locator_sequence_is_a_count_and_aligned_locators() {
        let locators = alloc::vec![
            medium(2018),
            TransportLocator::Large(TransportLocatorLarge {
                address: [1; 16],
                port: 2019,
            }),
            TransportLocator::String("tcpv4://h:1".into()),
        ];
        for e in [Endianness::Little, Endianness::Big] {
            let mut out = Vec::new();
            encode_locator_seq(&locators, &mut out, e).unwrap();
            assert_eq!(
                &out[..4],
                &match e {
                    Endianness::Little => 3u32.to_le_bytes(),
                    Endianness::Big => 3u32.to_be_bytes(),
                }
            );
            let (back, end) = decode_locator_seq(&out, 0, e).unwrap();
            assert_eq!(back, locators);
            assert_eq!(end, out.len());
        }
        // An empty sequence is the count alone.
        let mut out = Vec::new();
        encode_locator_seq(&[], &mut out, Endianness::Little).unwrap();
        assert_eq!(out, alloc::vec![0, 0, 0, 0]);
        // A count the bytes cannot hold is refused.
        assert!(decode_locator_seq(&[0xFF, 0xFF, 0, 0], 0, Endianness::Little).is_err());
    }

    #[test]
    fn unknown_discriminator_rejected() {
        let res = TransportLocator::decode(&[0x99, 0, 0, 0, 0, 0, 0], Endianness::Little);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
        // 0x04 and 0x05 are no TransportLocatorFormat.
        for d in [4u8, 5] {
            assert!(
                TransportLocator::decode(&[d, 0, 0, 0, 0, 0, 0, 0], Endianness::Little).is_err()
            );
        }
    }

    #[test]
    fn truncated_locators_return_eof() {
        for bytes in [
            alloc::vec![fmt_disc::SMALL, 1, 2],
            alloc::vec![fmt_disc::MEDIUM, 1, 2, 3, 4, 0],
            alloc::vec![fmt_disc::LARGE, 1, 2, 3],
            alloc::vec![fmt_disc::STRING, 0, 0, 0, 9, 0, 0, 0, b'a'],
        ] {
            let res = TransportLocator::decode(&bytes, Endianness::Little);
            assert!(
                matches!(res, Err(XrceError::UnexpectedEof { .. })),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn a_string_without_its_nul_is_refused() {
        let bytes = alloc::vec![fmt_disc::STRING, 0, 0, 0, 2, 0, 0, 0, b'a', b'b'];
        assert!(matches!(
            TransportLocator::decode(&bytes, Endianness::Little),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    /// §11.2.1: the UDP locator of an agent carries the agent's port as a
    /// number, whatever the byte order of the message.
    #[test]
    fn a_udp_locator_carries_the_default_agent_port() {
        for domain in [0u16, 1, 5] {
            let port = crate::transport_udp::agent_default_port(domain);
            for e in [Endianness::Little, Endianness::Big] {
                let enc = medium(port).encode(e).unwrap();
                match TransportLocator::decode(&enc, e).unwrap().0 {
                    TransportLocator::Medium(m) => assert_eq!(m.port, 7400 + 4 * domain),
                    other => panic!("not a Medium locator: {other:?}"),
                }
            }
        }
    }
}
