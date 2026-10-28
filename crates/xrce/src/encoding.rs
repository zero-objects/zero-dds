// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endianness helpers for XRCE submessage bodies.
//!
//! XRCE submessage bodies are, per Spec §8.3.4, either little-endian
//! or big-endian (E-flag). Within a stream the endianness is
//! constant (§8.2.3). Submessage-specific scalar fields (e.g. the
//! `short` fields in the ACKNACK body) follow this endianness.
//!
//! This file encapsulates the few primitives we need:
//! `u8`, `u16`, `u32`, as well as the canonical read/write helpers.
//!
//! Full XCDR2 structures are encoded directly via `zerodds-cdr` in
//! C6.2.B; in C6.2.A we treat the submessage payloads as an
//! opaque byte sequence with explicitly named scalar prefix fields.

use crate::error::XrceError;

/// Wire endianness for XRCE submessage bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Endianness {
    /// Big-endian (E-flag = 0).
    Big,
    /// Little-endian (E-flag = 1) — default in almost all XRCE stacks.
    #[default]
    Little,
}

impl Endianness {
    /// `true` if LE.
    #[must_use]
    pub fn is_little(self) -> bool {
        matches!(self, Self::Little)
    }
}

/// Writes `u16` into `out[..2]` with the desired endianness.
///
/// # Errors
/// `WriteOverflow` if `out.len() < 2`.
pub fn write_u16(out: &mut [u8], value: u16, e: Endianness) -> Result<(), XrceError> {
    if out.len() < 2 {
        return Err(XrceError::WriteOverflow {
            needed: 2,
            available: out.len(),
        });
    }
    let bytes = if e.is_little() {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    out[..2].copy_from_slice(&bytes);
    Ok(())
}

/// Reads `u16` from `bytes[..2]` with the desired endianness.
///
/// # Errors
/// `UnexpectedEof` if `bytes.len() < 2`.
pub fn read_u16(bytes: &[u8], e: Endianness) -> Result<u16, XrceError> {
    if bytes.len() < 2 {
        return Err(XrceError::UnexpectedEof {
            needed: 2,
            offset: bytes.len(),
        });
    }
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&bytes[..2]);
    if e.is_little() {
        Ok(u16::from_le_bytes(buf))
    } else {
        Ok(u16::from_be_bytes(buf))
    }
}

/// Writes `i16` into `out[..2]`.
///
/// # Errors
/// `WriteOverflow` if `out.len() < 2`.
pub fn write_i16(out: &mut [u8], value: i16, e: Endianness) -> Result<(), XrceError> {
    write_u16(out, value as u16, e)
}

/// Reads `i16` from `bytes[..2]`.
///
/// # Errors
/// `UnexpectedEof` if `bytes.len() < 2`.
pub fn read_i16(bytes: &[u8], e: Endianness) -> Result<i16, XrceError> {
    Ok(read_u16(bytes, e)? as i16)
}

/// Writes `u32` into `out[..4]`.
///
/// # Errors
/// `WriteOverflow` if `out.len() < 4`.
pub fn write_u32(out: &mut [u8], value: u32, e: Endianness) -> Result<(), XrceError> {
    if out.len() < 4 {
        return Err(XrceError::WriteOverflow {
            needed: 4,
            available: out.len(),
        });
    }
    let bytes = if e.is_little() {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    out[..4].copy_from_slice(&bytes);
    Ok(())
}

/// Reads `u32` from `bytes[..4]`.
///
/// # Errors
/// `UnexpectedEof` if `bytes.len() < 4`.
pub fn read_u32(bytes: &[u8], e: Endianness) -> Result<u32, XrceError> {
    if bytes.len() < 4 {
        return Err(XrceError::UnexpectedEof {
            needed: 4,
            offset: bytes.len(),
        });
    }
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&bytes[..4]);
    if e.is_little() {
        Ok(u32::from_le_bytes(buf))
    } else {
        Ok(u32::from_be_bytes(buf))
    }
}

// ===========================================================================
// Types of Annex A without @extensibility
// ===========================================================================

/// How the types of DDS-XRCE 1.0 Annex A that carry no `@extensibility`
/// annotation (`Time_t`, `ResultStatus`, `Property`, `TransportLocator*`,
/// `ACKNACK_Payload`) are coded.
///
/// DDS-XTypes 1.3 §7.3.1.2.1.8: "In the event that the representation of a
/// given type does not indicate the type's extensibility kind, the type shall
/// be considered appendable." An appendable type has a DHEADER in XCDR2.
/// The reference implementation (eProsima Micro-XRCE-DDS) writes none: a
/// `STATUS` payload of 6 octets, an `ACKNACK` of 5.
///
/// The writers follow this setting; the readers read the form it names and,
/// where that does not parse, the other one, so a peer of the other kind is
/// understood.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnannotatedTypes {
    /// `APPENDABLE`, with a DHEADER: what the specification says (default).
    #[default]
    Appendable,
    /// `FINAL`, without a DHEADER: the form of the reference implementation.
    Final,
}

static UNANNOTATED_FINAL: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "std")]
std::thread_local! {
    static UNANNOTATED_OVERRIDE: core::cell::Cell<Option<UnannotatedTypes>> =
        const { core::cell::Cell::new(None) };
}

/// Sets how the types of Annex A without `@extensibility` are coded in this
/// process ([`UnannotatedTypes`]); the default is
/// [`UnannotatedTypes::Appendable`].
pub fn set_unannotated_types(mode: UnannotatedTypes) {
    UNANNOTATED_FINAL.store(
        mode == UnannotatedTypes::Final,
        core::sync::atomic::Ordering::Relaxed,
    );
}

/// The setting in force: the one of [`with_unannotated_types`] on this thread
/// if there is one, else the one of [`set_unannotated_types`].
#[must_use]
pub fn unannotated_types() -> UnannotatedTypes {
    #[cfg(feature = "std")]
    if let Some(mode) = UNANNOTATED_OVERRIDE.with(core::cell::Cell::get) {
        return mode;
    }
    if UNANNOTATED_FINAL.load(core::sync::atomic::Ordering::Relaxed) {
        UnannotatedTypes::Final
    } else {
        UnannotatedTypes::Appendable
    }
}

/// Runs `f` with `mode` in force on this thread only (tests of both forms
/// side by side, in one process).
#[cfg(feature = "std")]
pub fn with_unannotated_types<R>(mode: UnannotatedTypes, f: impl FnOnce() -> R) -> R {
    let before = UNANNOTATED_OVERRIDE.with(|c| c.replace(Some(mode)));
    let result = f();
    UNANNOTATED_OVERRIDE.with(|c| c.set(before));
    result
}

/// Whether `CLIENT_Representation` and `AGENT_Representation` carry the
/// `client_timestamp` / `agent_timestamp` member.
///
/// DDS-XRCE 1.0 contradicts itself: §7.8.2.1 and §8.3.5.1.2 (prose) list
/// `Time_t client_timestamp` (and `agent_timestamp`) after `xrce_vendor_id`;
/// the IDL of Annex A and the bytes of the examples of Annex B.1 do not have
/// it, and the eProsima capture is 16 octets. The two forms cannot be told
/// apart by their bytes, so the form is a setting, not a guess: the writers
/// and the readers follow it. The default is the normative one of Annex A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RepresentationTimestamps {
    /// No timestamp member: the form of Annex A and Annex B.1 (default).
    #[default]
    Absent,
    /// The `Time_t` member after `xrce_vendor_id`, as the prose of §7.8.2.1.
    Present,
}

static TIMESTAMPS_PRESENT: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "std")]
std::thread_local! {
    static TIMESTAMPS_OVERRIDE: core::cell::Cell<Option<RepresentationTimestamps>> =
        const { core::cell::Cell::new(None) };
}

/// Sets the form of the representations in this process
/// ([`RepresentationTimestamps`]); the default is
/// [`RepresentationTimestamps::Absent`].
pub fn set_representation_timestamps(mode: RepresentationTimestamps) {
    TIMESTAMPS_PRESENT.store(
        mode == RepresentationTimestamps::Present,
        core::sync::atomic::Ordering::Relaxed,
    );
}

/// The setting in force: the one of [`with_representation_timestamps`] on
/// this thread if there is one, else the one of
/// [`set_representation_timestamps`].
#[must_use]
pub fn representation_timestamps() -> RepresentationTimestamps {
    #[cfg(feature = "std")]
    if let Some(mode) = TIMESTAMPS_OVERRIDE.with(core::cell::Cell::get) {
        return mode;
    }
    if TIMESTAMPS_PRESENT.load(core::sync::atomic::Ordering::Relaxed) {
        RepresentationTimestamps::Present
    } else {
        RepresentationTimestamps::Absent
    }
}

/// Runs `f` with `mode` in force on this thread only (a client with its own
/// setting, tests of both forms in one process).
#[cfg(feature = "std")]
pub fn with_representation_timestamps<R>(
    mode: RepresentationTimestamps,
    f: impl FnOnce() -> R,
) -> R {
    let before = TIMESTAMPS_OVERRIDE.with(|c| c.replace(Some(mode)));
    let result = f();
    TIMESTAMPS_OVERRIDE.with(|c| c.set(before));
    result
}

/// How a `SampleInfo` is written and read (DDS-XRCE 1.0 §7.7.1, Annex A).
///
/// Annex A gives `SampleInfo` a `state` and a `SampleInfoDetail` union
/// switched on a `SampleInfoFormat` octet (`FORMAT_EMPTY` nothing,
/// `FORMAT_SEQNUM` the sequence number, `FORMAT_TIMESTAMP` the time offset,
/// `FORMAT_SEQN_TIMS` both; the printed union repeats a case label, the
/// intended reading is used). §7.7.1 and the figure of Annex B.5.3 show a flat
/// struct of `state`, three padding octets, `sequence_number` and
/// `session_time_offset`. With `FORMAT_SEQN_TIMS` the union is the same 12
/// octets as the flat struct, the discriminator being the first padding
/// octet; the forms differ in that octet, which the flat form leaves zero and
/// the union reads as `FORMAT_EMPTY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SampleInfoForm {
    /// The union of Annex A (default): written with `FORMAT_SEQN_TIMS`, read
    /// by its discriminator.
    #[default]
    AnnexA,
    /// The flat struct of §7.7.1: the octet after `state` is padding, always
    /// 12 octets (what a client writing zero padding sends).
    Flat,
}

static SAMPLE_INFO_FLAT: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "std")]
std::thread_local! {
    static SAMPLE_INFO_OVERRIDE: core::cell::Cell<Option<SampleInfoForm>> =
        const { core::cell::Cell::new(None) };
}

/// Sets the form of `SampleInfo` in this process ([`SampleInfoForm`]).
pub fn set_sample_info_form(form: SampleInfoForm) {
    SAMPLE_INFO_FLAT.store(
        form == SampleInfoForm::Flat,
        core::sync::atomic::Ordering::Relaxed,
    );
}

/// The form in force: the one of [`with_sample_info_form`] on this thread if
/// there is one, else the one of [`set_sample_info_form`].
#[must_use]
pub fn sample_info_form() -> SampleInfoForm {
    #[cfg(feature = "std")]
    if let Some(form) = SAMPLE_INFO_OVERRIDE.with(core::cell::Cell::get) {
        return form;
    }
    if SAMPLE_INFO_FLAT.load(core::sync::atomic::Ordering::Relaxed) {
        SampleInfoForm::Flat
    } else {
        SampleInfoForm::AnnexA
    }
}

/// Runs `f` with `form` in force on this thread only.
#[cfg(feature = "std")]
pub fn with_sample_info_form<R>(form: SampleInfoForm, f: impl FnOnce() -> R) -> R {
    let before = SAMPLE_INFO_OVERRIDE.with(|c| c.replace(Some(form)));
    let result = f();
    SAMPLE_INFO_OVERRIDE.with(|c| c.set(before));
    result
}

#[cfg(feature = "alloc")]
mod appendable {
    extern crate alloc;
    use super::{Endianness, UnannotatedTypes, read_u32, unannotated_types, write_u32};
    use crate::error::XrceError;
    use alloc::vec::Vec;

    /// Pads `out` with zero octets to a multiple of `n`, counted from the
    /// start of the payload (`out[0]`).
    pub fn pad_to(out: &mut Vec<u8>, n: usize) {
        while out.len() % n != 0 {
            out.push(0);
        }
    }

    /// Opens a struct of an unannotated type: with the setting
    /// [`UnannotatedTypes::Appendable`] a DHEADER is reserved (4-aligned) and
    /// the position of the first member returned; with
    /// [`UnannotatedTypes::Final`] nothing is written and `None` returned.
    /// Close it with [`end_unannotated`].
    pub fn begin_unannotated(out: &mut Vec<u8>) -> Option<usize> {
        if unannotated_types() == UnannotatedTypes::Final {
            return None;
        }
        pad_to(out, 4);
        out.extend_from_slice(&[0; 4]);
        Some(out.len())
    }

    /// Writes the DHEADER (the length of the members) that
    /// [`begin_unannotated`] reserved.
    pub fn end_unannotated(out: &mut [u8], members_start: Option<usize>, e: Endianness) {
        if let Some(start) = members_start {
            let len = u32::try_from(out.len() - start).unwrap_or(u32::MAX);
            let _ = write_u32(&mut out[start - 4..start], len, e);
        }
    }

    /// Decodes a struct of an unannotated type whose members take `fixed`
    /// octets, from `pos` of `bytes` (the start of the payload): in the form
    /// the setting names first and, when that does not parse, in the other.
    /// `parse` reads the members from a slice that starts with them. Returns
    /// the value and the position after the struct.
    ///
    /// # Errors
    /// `UnexpectedEof` when the bytes hold neither form; the error of
    /// `parse` for the form tried last.
    pub fn decode_unannotated<T>(
        bytes: &[u8],
        pos: usize,
        e: Endianness,
        fixed: usize,
        parse: impl Fn(&[u8]) -> Result<T, XrceError>,
    ) -> Result<(T, usize), XrceError> {
        let appendable = || -> Result<(T, usize), XrceError> {
            let (start, end) = read_dheader(bytes, pos, e).ok_or(XrceError::UnexpectedEof {
                needed: pos.div_ceil(4) * 4 + 4 + fixed,
                offset: bytes.len(),
            })?;
            if end - start < fixed {
                return Err(XrceError::ValueOutOfRange {
                    message: "DHEADER shorter than the members of the type",
                });
            }
            Ok((parse(&bytes[start..end])?, end))
        };
        let direct = || -> Result<(T, usize), XrceError> {
            let members = bytes
                .get(pos..pos + fixed)
                .ok_or(XrceError::UnexpectedEof {
                    needed: fixed,
                    offset: bytes.len().saturating_sub(pos),
                })?;
            Ok((parse(members)?, pos + fixed))
        };
        match unannotated_types() {
            UnannotatedTypes::Appendable => appendable().or_else(|_| direct()),
            UnannotatedTypes::Final => direct().or_else(|_| appendable()),
        }
    }

    /// Reads the DHEADER at the 4-aligned position at or after `pos`:
    /// `(members_start, members_end)`; `None` when the bytes do not hold
    /// one that fits.
    #[must_use]
    pub fn read_dheader(bytes: &[u8], pos: usize, e: Endianness) -> Option<(usize, usize)> {
        let at = pos.div_ceil(4) * 4;
        let len = read_u32(bytes.get(at..)?, e).ok()?;
        let start = at + 4;
        let end = start.checked_add(usize::try_from(len).ok()?)?;
        (end <= bytes.len()).then_some((start, end))
    }
}
#[cfg(feature = "alloc")]
pub use appendable::{
    begin_unannotated, decode_unannotated, end_unannotated, pad_to, read_dheader,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn u16_le_roundtrip() {
        let mut buf = [0u8; 2];
        write_u16(&mut buf, 0x1234, Endianness::Little).unwrap();
        assert_eq!(buf, [0x34, 0x12]);
        assert_eq!(read_u16(&buf, Endianness::Little).unwrap(), 0x1234);
    }

    #[test]
    fn u16_be_roundtrip() {
        let mut buf = [0u8; 2];
        write_u16(&mut buf, 0x1234, Endianness::Big).unwrap();
        assert_eq!(buf, [0x12, 0x34]);
        assert_eq!(read_u16(&buf, Endianness::Big).unwrap(), 0x1234);
    }

    #[test]
    fn u16_overflow_on_short_buffer() {
        let mut buf = [0u8; 1];
        let res = write_u16(&mut buf, 1, Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::WriteOverflow { needed: 2, .. })
        ));
    }

    #[test]
    fn i16_negative_roundtrip() {
        let mut buf = [0u8; 2];
        write_i16(&mut buf, -1, Endianness::Little).unwrap();
        assert_eq!(buf, [0xFF, 0xFF]);
        assert_eq!(read_i16(&buf, Endianness::Little).unwrap(), -1);
    }

    #[test]
    fn u32_roundtrip_le() {
        let mut buf = [0u8; 4];
        write_u32(&mut buf, 0xDEADBEEF, Endianness::Little).unwrap();
        assert_eq!(buf, [0xEF, 0xBE, 0xAD, 0xDE]);
        assert_eq!(read_u32(&buf, Endianness::Little).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn read_u16_short_buffer_returns_eof() {
        let buf = [0u8; 1];
        let res = read_u16(&buf, Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 2, .. })
        ));
    }
}
