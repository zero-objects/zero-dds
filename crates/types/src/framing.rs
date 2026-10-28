// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two framings a `TypeObject` is written in.
//!
//! XTypes 1.3 §7.3.4 declares every `TypeObject` aggregate
//! `@extensibility(APPENDABLE)`, so under the encoding version 2 each one
//! is a `DHEADER` and a body. The general answer — which members are
//! written, in what order, behind which header — is
//! `zerodds_ast_layout`'s, and a `TypeObject` does not go through it
//! because its own shape is fixed by the specification rather than by a
//! model.
//!
//! ## Encoding version 1
//!
//! A `TypeObject` stream is XCDR2 everywhere but in one place: the
//! samples of the TypeLookup service. Their types carry no
//! `@extensibility` on the outermost level (§7.3.1.2.1.8 makes them
//! appendable), the built-in endpoints set no `DataRepresentationQosPolicy`
//! (an empty list is `XCDR`, §7.6.3.1.1), and Table 60 gives `CDR_LE` for
//! an appendable type under XCDR — which is also the one representation
//! of DDSI-RTPS 2.5 Table 10.2 an appendable type can take. The objects
//! inside a reply travel in that encoding with the rest of it.
//!
//! The helpers below answer both versions. Which one a stream is in is
//! the alignment cap it was opened with — four for XCDR2, eight for XCDR1
//! (§7.4.1.1.1 aligns a primitive to its own size, §7.4.2 caps that at
//! four) — so [`xcdr2_writer`] and [`xcdr1_writer`] are where a caller
//! chooses, and [`EncodingVersion::of_writer`] is how a helper asks. The
//! differences the helpers carry:
//!
//! - an appendable aggregate has no `DHEADER` (§7.4.1.1, PLAIN_CDR);
//! - a mutable aggregate is a parameter list ending in `PID_LIST_END`
//!   (§7.4.1.2), not a `DHEADER` over `EMHEADER`s;
//! - an `@optional` member of a final or appendable structure has a
//!   parameter header in front of it, with a length of zero when absent
//!   and no sentinel after it (§7.4.1.1.5.2), not a presence octet.
//!
//! The helpers sit on [`zerodds_wire`] and add no rule of their own.

use zerodds_wire::{Endianness, ParameterRole, Reader, WireError, Writer};

/// XCDR2's `MAXALIGN`. XTypes 1.3 §7.4.1.1.1, and
/// `zerodds_ast_layout::key::Encoding::max_alignment`, which was read off
/// Cyclone's and RTI Connext 7.7's own serialisers rather than off the
/// clause.
///
/// It has to be named at every buffer this crate opens. `Writer::new` and
/// `Reader::new` are **eight**, which is XCDR1's, and a `TypeObject`
/// stream is XCDR2 throughout — the hash input of §7.3.4.5 and the
/// `PID_TYPE_INFORMATION` body alike.
pub const XCDR2_MAX_ALIGN: usize = 4;

/// A writer for a `TypeObject` stream: little-endian, capped at four.
#[must_use]
pub fn xcdr2_writer() -> Writer {
    Writer::with_max_align(Endianness::Little, XCDR2_MAX_ALIGN)
}

/// A reader for a `TypeObject` stream: little-endian, capped at four.
#[must_use]
pub fn xcdr2_reader(bytes: &[u8]) -> Reader<'_> {
    Reader::with_max_align(bytes, Endianness::Little, XCDR2_MAX_ALIGN)
}

/// XCDR1's `MAXALIGN`: §7.4.1.1.1 aligns a primitive to its own size, and
/// the largest a `TypeObject` holds (`int64`, `float64`, `float128`) is
/// eight-aligned.
pub const XCDR1_MAX_ALIGN: usize = 8;

/// A writer for a stream in the encoding version 1, capped at eight.
///
/// Its PL_CDR1 framing is [`zerodds_wire::PlCdr1Framing::DEFAULT`], the
/// one §7.4.1.2 describes: `PID_EXTENDED` and `PID_LIST_END` with the
/// must-understand bit (Table 34) and the exact length of each value.
#[must_use]
pub fn xcdr1_writer(endianness: Endianness) -> Writer {
    Writer::with_max_align(endianness, XCDR1_MAX_ALIGN)
}

/// A reader for a stream in the encoding version 1, capped at eight.
#[must_use]
pub fn xcdr1_reader(bytes: &[u8], endianness: Endianness) -> Reader<'_> {
    Reader::with_max_align(bytes, endianness, XCDR1_MAX_ALIGN)
}

/// The Extended CDR encoding version a stream is written in (XTypes 1.3
/// §7.4.1 and §7.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingVersion {
    /// Encoding version 1: PLAIN_CDR and PL_CDR.
    Xcdr1,
    /// Encoding version 2: PLAIN_CDR2, DELIMITED_CDR and PL_CDR2.
    Xcdr2,
}

impl EncodingVersion {
    /// The version of a stream this crate's helpers write into, read off
    /// the writer's alignment cap (see the module documentation).
    #[must_use]
    pub const fn of_writer(w: &Writer) -> Self {
        Self::of_max_align(w.max_align())
    }

    /// The version of a stream this crate's helpers read from.
    #[must_use]
    pub const fn of_reader(r: &Reader<'_>) -> Self {
        Self::of_max_align(r.max_align())
    }

    const fn of_max_align(max_align: usize) -> Self {
        if max_align >= XCDR1_MAX_ALIGN {
            Self::Xcdr1
        } else {
            Self::Xcdr2
        }
    }
}

/// A reader over `bytes` that reads them as `outer` would: same byte
/// order, alignment cap and PL_CDR1 framing, alignment counted from the
/// first of them.
fn inner_reader<'a>(outer: &Reader<'_>, bytes: &'a [u8]) -> Reader<'a> {
    Reader::with_max_align(bytes, outer.endianness(), outer.max_align())
        .with_pl_cdr1_framing(outer.pl_cdr1_framing())
        .with_pl_cdr1_reading(outer.pl_cdr1_reading())
}

/// A PL_CDR1 parameter header for `member_id`: the short form while the
/// id fits it, `PID_EXTENDED` above (§7.4.1.2.2).
fn write_parameter_header(
    w: &mut Writer,
    member_id: u32,
    role: ParameterRole,
) -> Result<zerodds_wire::writer::Pid, WireError> {
    match u16::try_from(member_id) {
        Ok(short) if short <= w.pl_cdr1_framing().highest_short_id => {
            w.write_pid(short, role, false)
        }
        _ => w.write_pid_extended(member_id, role, false),
    }
}

/// Write a `DHEADER`, then `body`, then patch the length. Under the
/// encoding version 1 the body alone.
///
/// The body's alignment is counted from the stream, not from the body:
/// [`Writer::write_dheader`] aligns to four and the cap for XCDR2 is
/// four, so the body always starts on the boundary it would have
/// started on at offset zero of a buffer of its own. The two are the
/// same bytes, and one writer is one buffer.
///
/// # Errors
/// Whatever `body` returns. A refused body leaves the placeholder
/// unpatched, which is harmless because the caller discards the buffer.
pub fn encode_appendable<E>(
    w: &mut Writer,
    body: impl FnOnce(&mut Writer) -> Result<(), E>,
) -> Result<(), E> {
    if EncodingVersion::of_writer(w) == EncodingVersion::Xcdr1 {
        // §7.4.1.1: PLAIN_CDR for final and appendable types alike.
        return body(w);
    }
    let header = w.write_dheader();
    body(w)?;
    w.finish_dheader(header);
    Ok(())
}

/// Read a `DHEADER` and run `body` over exactly the bytes it announced.
/// Under the encoding version 1 there is no `DHEADER` and `body` reads on.
///
/// The body gets a reader of its own over that slice rather than the
/// outer one, for two reasons. A body that reads past its own length
/// would otherwise take the bytes of whatever follows and report
/// nothing — the silent shape. And a body that reads *fewer* bytes than
/// the header announced is what `@appendable` evolution looks like: a
/// peer that added a member wrote it, this reader does not know it, and
/// the surplus is skipped rather than mistaken for the next field.
///
/// # Errors
/// [`WireError::ImpossibleLength`] for a length longer than the stream,
/// [`WireError::Truncated`] when the announced bytes are not there, or
/// whatever `body` returns.
pub fn decode_appendable<T>(
    r: &mut Reader<'_>,
    body: impl FnOnce(&mut Reader<'_>) -> Result<T, WireError>,
) -> Result<T, WireError> {
    decode_appendable_with(r, body)
}

/// [`decode_appendable`] for a body whose error type is not
/// [`WireError`].
///
/// A `TypeObject` body reports `TypeCodecError::UnknownTypeKind` for an
/// equivalence kind the specification does not define, and a framing
/// helper that insisted on `WireError` would flatten that into an offset
/// and a sentence. `E: From<WireError>` is what lets the header's own
/// errors travel in the body's type.
///
/// The two are separate functions rather than one generic because a
/// single generic one leaves `E` ambiguous at every call site whose body
/// only ever uses `?` — seven of them in this crate — and each would
/// need a turbofish naming the type it already returns.
///
/// # Errors
/// As [`decode_appendable`].
pub fn decode_appendable_with<T, E>(
    r: &mut Reader<'_>,
    body: impl FnOnce(&mut Reader<'_>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<WireError>,
{
    if EncodingVersion::of_reader(r) == EncodingVersion::Xcdr1 {
        return body(r);
    }
    let length = r.read_dheader()?;
    let bytes = r.read_bytes(length)?;
    let mut inner = Reader::with_max_align(bytes, r.endianness(), r.max_align());
    body(&mut inner)
}

/// The `EMHEADER` length code this crate writes for a member of a
/// `@extensibility(MUTABLE)` aggregate: `LC = 4`, the form whose length
/// is a separate `NEXTINT` in front of the member data (XTypes 1.3
/// §7.4.3.4.2).
///
/// It is the one code that describes any member regardless of what the
/// member's own first four bytes happen to be, and it is what Cyclone
/// emits for both members of a `TypeInformation`.
pub const EMHEADER_LC4: u8 = 4;

/// Write one member of a `@mutable` aggregate: an `EMHEADER` with
/// `LC = 4`, the `NEXTINT` byte length, then the member itself.
///
/// The must-understand bit stays clear. Neither §7.6.3.2.1
/// (`TypeInformation`) nor §7.6.3.3.3 (the TypeLookup request and reply
/// types) marks a member must-understand, and setting it would ask every
/// peer to refuse the whole value over a member it may legitimately not
/// know — which is the opposite of what §7.6.3.3.3 says these types are
/// `MUTABLE` for ("they can be modified without breaking
/// interoperability").
///
/// # Errors
/// A member id outside the 28 bits of an `EMHEADER`, or whatever `body`
/// returns.
pub fn encode_mutable_member(
    w: &mut Writer,
    member_id: u32,
    body: impl FnOnce(&mut Writer) -> Result<(), WireError>,
) -> Result<(), WireError> {
    if EncodingVersion::of_writer(w) == EncodingVersion::Xcdr1 {
        return encode_parameter(w, member_id, body);
    }
    let header = w.write_emheader(member_id, EMHEADER_LC4, false)?;
    body(w)?;
    w.finish_emheader(header)
}

/// A mutable member whose first u32 already measures its remaining octets
/// (a DHEADER, string length or octet sequence count). LC5 reuses that word.
///
/// # Errors
/// Invalid member identifier or an error from the member encoder.
pub fn encode_mutable_length_prefixed_member(
    w: &mut Writer,
    member_id: u32,
    body: impl FnOnce(&mut Writer) -> Result<(), WireError>,
) -> Result<(), WireError> {
    if EncodingVersion::of_writer(w) == EncodingVersion::Xcdr1 {
        return encode_parameter(w, member_id, body);
    }
    let header = w.write_emheader(member_id, 5, false)?;
    body(w)?;
    w.finish_emheader(header)
}

/// Read a `@mutable` aggregate: the `DHEADER` over the member list, then
/// one `EMHEADER` per member, each member's body handed to `on_member`
/// over a reader of exactly its own extent.
///
/// A member id the caller does not want is stepped over rather than
/// guessed at: `read_emheader` honours all eight length codes of
/// §7.4.3.4.2 and reports the member's extent, which is what makes a
/// `MUTABLE` type extensible in the first place.
///
/// # Errors
/// [`WireError`] from the framing, or whatever `on_member` returns.
pub fn decode_mutable<T>(
    r: &mut Reader<'_>,
    init: T,
    mut on_member: impl FnMut(&mut T, u32, &mut Reader<'_>) -> Result<(), WireError>,
) -> Result<T, WireError> {
    if EncodingVersion::of_reader(r) == EncodingVersion::Xcdr1 {
        return decode_parameter_list(r, init, on_member);
    }
    decode_appendable(r, move |r| {
        let mut acc = init;
        // Four bytes is the shortest thing an EMHEADER can be followed
        // by; anything less is the padding that ends a member list.
        while r.remaining() >= 4 {
            let header = r.read_emheader()?;
            let body = r.read_bytes(header.extent)?;
            let mut inner = Reader::with_max_align(body, r.endianness(), r.max_align());
            on_member(&mut acc, header.member_id, &mut inner)?;
        }
        Ok(acc)
    })
}

/// A `@mutable` aggregate: `body` writes its members with
/// [`encode_mutable_member`]. Under the encoding version 2 a `DHEADER`
/// over the member list; under version 1 the member list followed by
/// `PID_LIST_END` (§7.4.1.2).
///
/// # Errors
/// Whatever `body` returns.
pub fn encode_mutable(
    w: &mut Writer,
    body: impl FnOnce(&mut Writer) -> Result<(), WireError>,
) -> Result<(), WireError> {
    if EncodingVersion::of_writer(w) == EncodingVersion::Xcdr1 {
        body(w)?;
        w.write_sentinel();
        return Ok(());
    }
    encode_appendable(w, body)
}

/// One parameter of a PL_CDR1 list: header, value, exact length.
fn encode_parameter(
    w: &mut Writer,
    member_id: u32,
    body: impl FnOnce(&mut Writer) -> Result<(), WireError>,
) -> Result<(), WireError> {
    let header = write_parameter_header(w, member_id, ParameterRole::Member)?;
    body(w)?;
    w.finish_pid(header);
    Ok(())
}

/// [`decode_mutable`] under the encoding version 1: parameters up to
/// `PID_LIST_END`, each member's value handed to `on_member` over a
/// reader of exactly its declared length.
///
/// `PID_IGNORE` and parameters with `FLAG_IMPL_EXTENSION` are another
/// implementation's own and are stepped over (§7.4.1.2.1, §7.4.1.2.2);
/// an id in the reserved range is no member and is refused.
fn decode_parameter_list<T>(
    r: &mut Reader<'_>,
    init: T,
    mut on_member: impl FnMut(&mut T, u32, &mut Reader<'_>) -> Result<(), WireError>,
) -> Result<T, WireError> {
    let mut acc = init;
    loop {
        let at = r.position();
        let header = r.read_pid()?;
        if header.end {
            r.finish_pid(header)?;
            return Ok(acc);
        }
        if header.ignore || header.impl_extension {
            r.skip_parameter(header)?;
            continue;
        }
        if header.reserved {
            return Err(WireError::OutOfRange {
                at,
                expected: alloc::format!(
                    "a member id, and 0x{:04x} is in the range §7.4.1.2.1 reserves",
                    header.pid
                ),
            });
        }
        let bytes = r.read_bytes(header.length)?;
        let mut inner = inner_reader(r, bytes);
        on_member(&mut acc, header.pid, &mut inner)?;
        r.finish_pid(header)?;
    }
}

/// The member of a `default:` arm of Annex B — `ExtendedTypeDefn`,
/// `ExtendedAnnotationParameterValue`, `CompleteExtendedType`,
/// `MinimalExtendedType` — given as `members`, the member list it holds
/// under the encoding version 2 (the bytes inside its `DHEADER`).
///
/// All four are `@extensibility(MUTABLE)` and empty in XTypes 1.3
/// ("Available for future extension"). Under XCDR2 the arm is a `DHEADER`
/// over the list, kept verbatim. Under XCDR1 it is a parameter list
/// (§7.4.1.2); the type has no member of its own, so the list is
/// `PID_LIST_END` alone. A non-empty list holds members of a later revision
/// that were read under XCDR2: their values are encoded for a type this
/// crate does not have and cannot be framed again as XCDR1 parameters
/// without it, so such a list is refused under XCDR1 rather than written in
/// a form that was guessed.
///
/// # Errors
/// [`WireError::OutOfRange`] for a non-empty list under XCDR1.
pub fn encode_extended_members(w: &mut Writer, members: &[u8]) -> Result<(), WireError> {
    if EncodingVersion::of_writer(w) == EncodingVersion::Xcdr1 {
        if !members.is_empty() {
            return Err(WireError::OutOfRange {
                at: w.position(),
                expected: alloc::format!(
                    "an extended (default:) arm whose {} octet(s) of members, read under \
                     XCDR2 for a type of a later revision, have no XCDR1 form without that type",
                    members.len()
                ),
            });
        }
        w.write_sentinel();
        return Ok(());
    }
    encode_appendable::<WireError>(w, |w| {
        w.write_bytes(members);
        Ok(())
    })
}

/// Read what [`encode_extended_members`] writes: under XCDR2 the member
/// list inside the `DHEADER`, verbatim; under XCDR1 the parameter list up
/// to `PID_LIST_END`. The type has no member in XTypes 1.3, so every
/// parameter of the XCDR1 list is one this reader does not know: it is
/// stepped over (§7.4.1.2.3, a mutable type's member the reader's type
/// does not have) unless it is must-understand, which discards the value
/// (§7.4.1.2.1), and the list read is empty.
///
/// # Errors
/// The stream ends inside the arm, or an XCDR1 member is must-understand.
pub fn decode_extended_members(r: &mut Reader<'_>) -> Result<alloc::vec::Vec<u8>, WireError> {
    if EncodingVersion::of_reader(r) == EncodingVersion::Xcdr1 {
        loop {
            let at = r.position();
            let header = r.read_pid()?;
            if header.end {
                r.finish_pid(header)?;
                return Ok(alloc::vec::Vec::new());
            }
            if header.must_understand && !header.ignore {
                return Err(WireError::OutOfRange {
                    at,
                    expected: alloc::format!(
                        "an extended (default:) arm without must-understand members; member \
                         0x{:x} is one and no member of this revision is known",
                        header.pid
                    ),
                });
            }
            r.skip_parameter(header)?;
        }
    }
    let length = r.read_dheader()?;
    Ok(r.read_bytes(length)?.to_vec())
}

/// An `@optional` member of a final or appendable structure, as member
/// `member_id` of that structure.
///
/// Encoding version 2: a presence octet, then the value when present
/// (§7.4.3.5.3). Encoding version 1: a parameter header, then the value;
/// the length is zero when absent and no `PID_LIST_END` follows
/// (§7.4.1.1.5.2, §7.4.1.2).
///
/// # Errors
/// Whatever `body` returns, or a member id outside the 28 bits of an
/// `eMemberHeader`.
pub fn encode_optional<T: ?Sized>(
    w: &mut Writer,
    member_id: u32,
    value: Option<&T>,
    body: impl FnOnce(&mut Writer, &T) -> Result<(), WireError>,
) -> Result<(), WireError> {
    if EncodingVersion::of_writer(w) == EncodingVersion::Xcdr1 {
        let header = write_parameter_header(w, member_id, ParameterRole::Optional)?;
        if let Some(value) = value {
            body(w, value)?;
        }
        w.finish_pid(header);
        return Ok(());
    }
    match value {
        None => {
            w.write_u8(0);
            Ok(())
        }
        Some(value) => {
            w.write_u8(1);
            body(w, value)
        }
    }
}

/// Read what [`encode_optional`] writes for member `member_id`.
///
/// Under the encoding version 1 the header has to name `member_id`: the
/// members of a final or appendable structure are neither omitted nor
/// reordered (§7.4.1.2.3), so another id there is a stream of another
/// type. The value has to fill the declared length exactly.
///
/// # Errors
/// The stream ends inside the member, the header names another member,
/// or `body` fails.
pub fn decode_optional<T>(
    r: &mut Reader<'_>,
    member_id: u32,
    body: impl FnOnce(&mut Reader<'_>) -> Result<T, WireError>,
) -> Result<Option<T>, WireError> {
    if EncodingVersion::of_reader(r) == EncodingVersion::Xcdr1 {
        let at = r.position();
        let header = r.read_pid_as(ParameterRole::Optional)?;
        if header.end || header.pid != member_id {
            return Err(WireError::OutOfRange {
                at,
                expected: alloc::format!(
                    "the parameter header of @optional member {member_id}, not of {}",
                    if header.end {
                        alloc::string::String::from("PID_LIST_END")
                    } else {
                        alloc::format!("member {}", header.pid)
                    }
                ),
            });
        }
        if header.length == 0 {
            r.finish_pid(header)?;
            return Ok(None);
        }
        let bytes = r.read_bytes(header.length)?;
        let mut inner = inner_reader(r, bytes);
        let value = body(&mut inner)?;
        if inner.remaining() != 0 {
            return Err(WireError::OutOfRange {
                at: at + inner.position(),
                expected: alloc::format!(
                    "@optional member {member_id} to fill its {} declared octet(s), \
                     and {} are left over",
                    header.length,
                    inner.remaining()
                ),
            });
        }
        r.finish_pid(header)?;
        return Ok(Some(value));
    }
    if r.read_u8()? == 0 {
        return Ok(None);
    }
    body(r).map(Some)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use zerodds_wire::Endianness;

    #[test]
    fn a_body_that_reads_short_leaves_the_outer_reader_past_the_whole_body() {
        let mut w = Writer::with_max_align(Endianness::Little, 4);
        encode_appendable::<WireError>(&mut w, |w| {
            w.write_u32(1);
            w.write_u32(2);
            Ok(())
        })
        .expect("infallible body");
        w.write_u32(0xDEAD_BEEF);
        let bytes: Vec<u8> = w.finish();

        let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
        // The body reads one of the two words it was given.
        let first = decode_appendable(&mut r, |r| r.read_u32()).expect("a body");
        assert_eq!(first, 1);
        // The word after the body is still the next thing the outer
        // reader sees: the surplus went with the body, not into it.
        assert_eq!(r.read_u32().expect("the trailing word"), 0xDEAD_BEEF);
    }

    #[test]
    fn a_body_that_reads_long_is_refused_rather_than_reading_what_follows() {
        let mut w = Writer::with_max_align(Endianness::Little, 4);
        encode_appendable::<WireError>(&mut w, |w| {
            w.write_u32(1);
            Ok(())
        })
        .expect("infallible body");
        w.write_u32(0xDEAD_BEEF);
        let bytes: Vec<u8> = w.finish();

        let mut r = Reader::with_max_align(&bytes, Endianness::Little, 4);
        let outcome = decode_appendable(&mut r, |r| {
            let _ = r.read_u32()?;
            r.read_u32()
        });
        assert!(
            matches!(outcome, Err(WireError::Truncated { .. })),
            "a body reading past its DHEADER must fail, not take the next member: {outcome:?}"
        );
    }
}
