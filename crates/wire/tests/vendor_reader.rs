// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The reader against bytes this project did not produce.
//!
//! Every byte string below is copied out of
//! `crates/ast-layout/tests/fixtures/vendor_bytes.md`, which records
//! what Eclipse Cyclone DDS, RTI Connext 7.7 and eProsima Fast DDS
//! wrote for one IDL type and one fixed sample, run on 2026-08-09 on
//! the Linux test host. Nothing here came from `Writer`.
//!
//! ## Why a round trip is not enough
//!
//! `wire.rs` next door writes with `Writer` and reads back with
//! `Reader`. That passes a decoder which is wrong in exactly the way its
//! own encoder is wrong, and the write path had eleven such defects in
//! one sweep. The `docs/silent-drop-sweep.md` closing note says the
//! reader "has the same shapes ... and a silent drop there is a decode
//! that returns a plausible value for bytes that meant something else",
//! and that grep was the only tool it had. These are the bytes.
//!
//! ## Three things are asserted for each row
//!
//! 1. **The value.** What the vendor put in comes back out, and the
//!    reader lands exactly on the end. A decode that stops early has
//!    left a member unread and said nothing.
//! 2. **Truncation.** Every prefix of every row, at every length, is an
//!    error. Never a panic, and never a value.
//! 3. **A length that cannot be true.** Every length field named in the
//!    row, and then every four-byte word in it, replaced with the
//!    largest value it can hold. A named error at a stated offset, no
//!    panic, and no allocation on a count the stream cannot back.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    // The sample value is 3.14159, not pi — it is what the vendor runs
    // were given, and rounding it to the constant changes the bytes.
    clippy::approx_constant
)]

use zerodds_wire::{Endianness, PidFlagPolicy, PlCdr1Framing, Reader, WireError};

// -- what a decoded sample is ----------------------------------------

/// One decoded scalar. Enough to say "the value came back", without a
/// type system for samples that only this file would use.
#[derive(Debug, Clone, PartialEq)]
enum Field {
    Bool(bool),
    U(u64),
    I(i64),
    F(f64),
    S(String),
}

use Field::{Bool, F, I, S, U};

/// A reader over one recorded sample.
type Decode = fn(&mut Reader<'_>) -> Result<Vec<Field>, WireError>;

/// One row of `vendor_bytes.md`.
struct Row {
    /// `<case> <encoding> <vendor> <byte order>`, as the fixture names it.
    what: &'static str,
    /// The bytes, exactly as the fixture records them.
    hex: &'static str,
    /// `MAXALIGN`: eight under XCDR1, four under XCDR2. XTypes 1.3 §7.4.1.
    max_align: usize,
    order: Endianness,
    decode: Decode,
    /// What the vendor was given.
    expected: fn() -> Vec<Field>,
    /// Length fields in the stream, as `(offset, width in bytes)`, read
    /// off the fixture's own hex. Listed rather than derived: an offset
    /// the test worked out from the code under test would move with it.
    lengths: &'static [(usize, usize)],
}

/// The PL_CDR1 framing the row's vendor writes, taken from its name.
///
/// The reader is configured for the peer whose bytes it is reading,
/// which is what `--dialect` does on the probe. Reading Fast DDS's
/// stream under Cyclone's framing is an error and not a value, and
/// `a_stream_read_under_another_vendors_framing_is_a_named_error`
/// below is the test for that half.
/// Cyclone DDS's PL_CDR1 framing, written out.
///
/// It was `PlCdr1Framing::DEFAULT` until the default dialect moved from
/// `Dialect::Cyclone` to `Dialect::Specification`, and the two differ in
/// two fields — the extended header and the padded `@optional` length —
/// so a Cyclone row read under the default now reads under the wrong
/// framing. Written out beside the other two rather than taken from a
/// constant whose value is somebody else's decision.
const CYCLONE: PlCdr1Framing = PlCdr1Framing {
    pid_flags: PidFlagPolicy::SentinelMustUnderstand,
    highest_short_id: 0x3f00,
    pad_declared_length: true,
    pad_declared_optional_length: false,
    member_must_understand: true,
    require_extended: true,
    pad_after_value: true,
};

fn framing(row: &Row) -> PlCdr1Framing {
    if row.what.contains("FastDDS") {
        PlCdr1Framing {
            pid_flags: PidFlagPolicy::Bare,
            highest_short_id: 0x3f00,
            pad_declared_length: false,
            pad_declared_optional_length: false,
            member_must_understand: false,
            require_extended: false,
            pad_after_value: false,
        }
    } else if row.what.contains("RTI") {
        PlCdr1Framing {
            pid_flags: PidFlagPolicy::SentinelMustUnderstand,
            highest_short_id: 0x3f00,
            pad_declared_length: true,
            pad_declared_optional_length: true,
            member_must_understand: true,
            require_extended: false,
            pad_after_value: true,
        }
    } else {
        // Cyclone's, and the one the "all three" rows are read under —
        // they have no parameter list in them, so the framing does not
        // arise.
        CYCLONE
    }
}

fn bytes_of(hex: &str) -> Vec<u8> {
    assert!(hex.len() % 2 == 0, "an odd hex string: {hex}");
    (0..hex.len() / 2)
        .map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("hex"))
        .collect()
}

/// Decode `bytes` fully: the fields, and nothing left over.
fn decode_all(row: &Row, bytes: &[u8]) -> Result<Vec<Field>, WireError> {
    let mut reader =
        Reader::with_max_align(bytes, row.order, row.max_align).with_pl_cdr1_framing(framing(row));
    let out = (row.decode)(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(WireError::OutOfRange {
            at: reader.position(),
            expected: format!("the whole sample read; {} bytes left", reader.remaining()),
        });
    }
    Ok(out)
}

// -- the shapes ------------------------------------------------------

fn point(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(vec![I(i64::from(r.read_i32()?)), F(r.read_f64()?)])
}
fn point_values() -> Vec<Field> {
    vec![I(1), F(2.0)]
}

fn odd(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(vec![
        I(i64::from(r.read_i32()?)),
        I(i64::from(r.read_i16()?)),
    ])
}
fn odd_values() -> Vec<Field> {
    vec![I(1), I(2)]
}

fn mixed(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(vec![
        Bool(r.read_bool()?),
        U(u64::from(r.read_u8()?)),
        I(i64::from(r.read_i16()?)),
        U(u64::from(r.read_u16()?)),
        I(i64::from(r.read_i32()?)),
        U(u64::from(r.read_u32()?)),
        I(r.read_i64()?),
        U(r.read_u64()?),
        F(f64::from(r.read_f32()?)),
        F(r.read_f64()?),
    ])
}
fn mixed_values() -> Vec<Field> {
    vec![
        Bool(true),
        U(0xAB),
        I(-12345),
        U(54321),
        I(-1_234_567),
        U(2_345_678),
        I(-987_654_321),
        U(123_456_789),
        F(2.5),
        F(3.14159),
    ]
}

fn string_member(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(vec![S(r.read_string()?), I(i64::from(r.read_i32()?))])
}
fn string_member_values() -> Vec<Field> {
    vec![S("abc".to_string()), I(7)]
}

fn seq_long(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_sequence(|r| r.read_i32().map(|v| I(i64::from(v))))
}
fn seq_long_values() -> Vec<Field> {
    vec![I(1), I(2), I(3)]
}

fn array_case(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let mut out = r.read_array(|r| r.read_i32().map(|v| I(i64::from(v))), 3)?;
    out.extend(r.read_array(|r| r.read_f64().map(F), 2)?);
    Ok(out)
}
fn array_values() -> Vec<Field> {
    vec![I(1), I(2), I(3), F(1.5), F(2.5)]
}

fn two_longs(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(vec![
        I(i64::from(r.read_i32()?)),
        I(i64::from(r.read_i32()?)),
    ])
}
fn enumeration_values() -> Vec<Field> {
    vec![I(2), I(9)]
}
fn keyed_values() -> Vec<Field> {
    vec![I(5), I(6)]
}
fn union_case_values() -> Vec<Field> {
    vec![I(1), I(42)]
}

fn typedef_array(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let mut out = r.read_array(|r| r.read_i32().map(|v| I(i64::from(v))), 6)?;
    out.push(I(i64::from(r.read_i32()?)));
    Ok(out)
}
fn typedef_array_values() -> Vec<Field> {
    vec![
        I(1),
        I(2),
        I(3),
        I(-1),
        I(2_147_483_647),
        I(-1_234_567),
        I(-1),
    ]
}

fn robot(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_array(|r| r.read_f64().map(F), 6)
}
fn robot_values() -> Vec<Field> {
    vec![F(1.0), F(2.0), F(3.0), F(4.0), F(5.0), F(6.0)]
}

/// `Cell { long a; long b; }`.
fn cell(r: &mut Reader<'_>) -> Result<Field, WireError> {
    let a = r.read_i32()?;
    let b = r.read_i32()?;
    Ok(U(
        (u64::from(a.cast_unsigned()) << 32) | u64::from(b.cast_unsigned())
    ))
}
fn two_cells() -> Vec<Field> {
    vec![U(0x0000_0001_0000_0002), U(0x0000_0003_0000_0004)]
}

fn seq_cell_plain(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_sequence(cell)
}
fn seq_cell_delimited(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_delimited_sequence(cell)
}
fn array_cell_plain(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_array(cell, 2)
}
fn array_cell_delimited(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_delimited_array(cell, 2)
}

/// `ArrOdd { Odd items[2]; octet tail; }` — the stride case.
fn array_stride(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let mut out = Vec::new();
    for _ in 0..2 {
        out.push(I(i64::from(r.read_i32()?)));
        out.push(I(i64::from(r.read_i16()?)));
    }
    out.push(U(u64::from(r.read_u8()?)));
    Ok(out)
}
fn array_stride_delimited(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let items = r.read_delimited_array(
        |r| {
            let a = r.read_i32()?;
            let b = r.read_i16()?;
            Ok(U(
                (u64::from(a.cast_unsigned()) << 16) | u64::from(b.cast_unsigned())
            ))
        },
        2,
    )?;
    let mut out = items;
    out.push(U(u64::from(r.read_u8()?)));
    Ok(out)
}
fn array_stride_values() -> Vec<Field> {
    vec![I(1), I(2), I(3), I(4), U(0x77)]
}
fn array_stride_delimited_values() -> Vec<Field> {
    vec![U(0x0000_0001_0002), U(0x0000_0003_0004), U(0x77)]
}

/// `Ext { long a; double b; short c; }`, unframed.
fn ext_plain(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(vec![
        I(i64::from(r.read_i32()?)),
        F(r.read_f64()?),
        I(i64::from(r.read_i16()?)),
    ])
}
fn ext_values() -> Vec<Field> {
    vec![I(7), F(1.5), I(-3)]
}

/// The same behind a DHEADER, which is `@appendable` under XCDR2.
fn ext_appendable(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let extent = r.read_dheader()?;
    let end = r.position() + extent;
    let out = ext_plain(r)?;
    filled(r, end)?;
    Ok(out)
}

/// The value has to end exactly where its delimiter said it would.
///
/// An error and not an assertion: this walk is handed deliberately
/// hostile bytes, and a harness that panicked on them would report a
/// reader defect that is its own.
fn filled(r: &Reader<'_>, end: usize) -> Result<(), WireError> {
    if r.position() == end {
        return Ok(());
    }
    Err(WireError::OutOfRange {
        at: r.position(),
        expected: format!("a value ending at {end}"),
    })
}

/// A `@mutable` parameter list under XCDR2: a DHEADER, then an EMHEADER
/// per member.
fn ext_mutable_xcdr2(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let extent = r.read_dheader()?;
    let end = r.position() + extent;
    let mut out = Vec::new();
    while r.position() < end {
        let header = r.read_emheader()?;
        out.push(match header.member_id {
            0 => I(i64::from(r.read_i32()?)),
            1 => F(r.read_f64()?),
            _ => I(i64::from(r.read_i16()?)),
        });
    }
    Ok(out)
}

/// A PL_CDR1 parameter list: headers until the sentinel.
///
/// Read the same way whichever vendor wrote it — which is the point.
/// Cyclone sets the must-understand flag on every parameter and uses
/// `PID_EXTENDED` for all of them; RTI uses the short form and rounds
/// each declared length up to the alignment; Fast DDS uses the short
/// form with the member's own length and writes a sentinel without the
/// flag. `vendor_bytes.md` §2.
fn pl_cdr1_ext(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let mut out = Vec::new();
    loop {
        let header = r.read_pid()?;
        if header.end {
            return Ok(out);
        }
        out.push(match header.pid {
            0 => I(i64::from(r.read_i32()?)),
            1 => F(r.read_f64()?),
            _ => I(i64::from(r.read_i16()?)),
        });
        // `finish_pid` closes the parameter's block and steps to the
        // next header by alignment. Not by adding the declared length:
        // RTI rounds its up to the alignment and Fast DDS writes the
        // member's own, and one reader has to take both.
        r.finish_pid(header)?;
    }
}

/// `MutOpt { long a; @optional long b; }` with `b` absent.
fn mut_optional_xcdr2(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    let extent = r.read_dheader()?;
    let end = r.position() + extent;
    let mut out = Vec::new();
    while r.position() < end {
        let header = r.read_emheader()?;
        if header.member_id != 0 {
            return Err(WireError::OutOfRange {
                at: r.position(),
                expected: format!("member 0; `b` is absent, and {} was read", header.member_id),
            });
        }
        out.push(I(i64::from(r.read_i32()?)));
    }
    Ok(out)
}
fn mut_optional_values() -> Vec<Field> {
    vec![I(7)]
}

fn seq_enum(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_sequence(|r| r.read_i32().map(|v| I(i64::from(v))))
}
fn seq_enum_delimited(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_delimited_sequence(|r| r.read_i32().map(|v| I(i64::from(v))))
}
fn array_enum(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_array(|r| r.read_i32().map(|v| I(i64::from(v))), 2)
}
fn array_enum_delimited(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_delimited_array(|r| r.read_i32().map(|v| I(i64::from(v))), 2)
}
fn enum_pair_values() -> Vec<Field> {
    vec![I(0), I(2)]
}

/// A sequence of `@appendable` elements: the element carries its own
/// delimiter whether or not the collection does.
fn seq_appendable_delimited(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_delimited_sequence(|r| {
        let extent = r.read_dheader()?;
        let end = r.position() + extent;
        let value = cell(r)?;
        filled(r, end)?;
        Ok(value)
    })
}
fn seq_appendable_plain(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_sequence(|r| {
        let extent = r.read_dheader()?;
        let end = r.position() + extent;
        let value = cell(r)?;
        filled(r, end)?;
        Ok(value)
    })
}
fn seq_appendable_xcdr1(r: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    r.read_sequence(cell)
}

// -- the rows --------------------------------------------------------

const LE: Endianness = Endianness::Little;
const BE: Endianness = Endianness::Big;

#[allow(clippy::too_many_lines)]
fn rows() -> Vec<Row> {
    vec![
        // -- all three agree, little endian (§"Where all three agree") --
        Row { what: "point XCDR1 all three LE", hex: "01000000000000000000000000000040", max_align: 8, order: LE, decode: point, expected: point_values, lengths: &[] },
        Row { what: "point XCDR2 all three LE", hex: "010000000000000000000040", max_align: 4, order: LE, decode: point, expected: point_values, lengths: &[] },
        Row { what: "mixed XCDR1 all three LE", hex: "01abc7cf31d400007929edffceca23004f9721c5ffffffff15cd5b070000000000002040000000006e861bf0f9210940", max_align: 8, order: LE, decode: mixed, expected: mixed_values, lengths: &[] },
        Row { what: "mixed XCDR2 all three LE", hex: "01abc7cf31d400007929edffceca23004f9721c5ffffffff15cd5b07000000000000204 06e861bf0f9210940", max_align: 4, order: LE, decode: mixed, expected: mixed_values, lengths: &[] },
        Row { what: "tail_pad both all three LE", hex: "010000000200", max_align: 8, order: LE, decode: odd, expected: odd_values, lengths: &[] },
        Row { what: "array_stride XCDR1 all three LE", hex: "01000000020000000300000004 0077", max_align: 8, order: LE, decode: array_stride, expected: array_stride_values, lengths: &[] },
        Row { what: "struct_inherit is covered by the corpus, not here", hex: "", max_align: 4, order: LE, decode: nothing, expected: Vec::new, lengths: &[] },

        // -- big endian, the table under §"The bytes" ------------------
        Row { what: "point XCDR1 all three BE", hex: "00000001000000004000000000000000", max_align: 8, order: BE, decode: point, expected: point_values, lengths: &[] },
        Row { what: "point XCDR2 all three BE", hex: "000000014000000000000000", max_align: 4, order: BE, decode: point, expected: point_values, lengths: &[] },
        Row { what: "mixed XCDR1 all three BE", hex: "01abcfc7d4310000ffed29790023caceffffffffc521974f00000000075bcd154020000000000000400921f9f01b866e", max_align: 8, order: BE, decode: mixed, expected: mixed_values, lengths: &[] },
        Row { what: "mixed XCDR2 all three BE", hex: "01abcfc7d4310000ffed29790023caceffffffffc521974f00000000075bcd1540200000400921f9f01b866e", max_align: 4, order: BE, decode: mixed, expected: mixed_values, lengths: &[] },
        Row { what: "tail_pad both all three BE", hex: "000000010002", max_align: 8, order: BE, decode: odd, expected: odd_values, lengths: &[] },
        Row { what: "array XCDR1 all three BE", hex: "000000010000000200000003000000003ff80000000000004004000000000000", max_align: 8, order: BE, decode: array_case, expected: array_values, lengths: &[] },
        Row { what: "array XCDR2 all three BE", hex: "0000000100000002000000033ff80000000000004004000000000000", max_align: 4, order: BE, decode: array_case, expected: array_values, lengths: &[] },
        Row { what: "enumeration both all three BE", hex: "0000000200000009", max_align: 4, order: BE, decode: two_longs, expected: enumeration_values, lengths: &[] },
        Row { what: "keyed both all three BE", hex: "0000000500000006", max_align: 4, order: BE, decode: two_longs, expected: keyed_values, lengths: &[] },
        Row { what: "union_case both all three BE", hex: "000000010000002a", max_align: 4, order: BE, decode: two_longs, expected: union_case_values, lengths: &[] },
        Row { what: "string_member both all three BE", hex: "000000046162630000000007", max_align: 4, order: BE, decode: string_member, expected: string_member_values, lengths: &[(0, 4)] },
        Row { what: "string_member both all three LE", hex: "040000006162630007000000", max_align: 4, order: LE, decode: string_member, expected: string_member_values, lengths: &[(0, 4)] },
        Row { what: "seq_long both all three BE", hex: "00000003000000010000000200000003", max_align: 4, order: BE, decode: seq_long, expected: seq_long_values, lengths: &[(0, 4)] },
        Row { what: "seq_long both all three LE", hex: "03000000010000000200000003000000", max_align: 4, order: LE, decode: seq_long, expected: seq_long_values, lengths: &[(0, 4)] },
        Row { what: "typedef_array both all three BE", hex: "000000010000000200000003ffffffff7fffffffffed2979ffffffff", max_align: 4, order: BE, decode: typedef_array, expected: typedef_array_values, lengths: &[] },
        Row { what: "robot XCDR1 Cyclone+RTI BE", hex: "3ff000000000000040000000000000004008000000000000401000000000000040140000000000004018000000000000", max_align: 8, order: BE, decode: robot, expected: robot_values, lengths: &[] },
        Row { what: "array_stride XCDR1 all three BE", hex: "000000010002000000000003000477", max_align: 8, order: BE, decode: array_stride, expected: array_stride_values, lengths: &[] },
        Row { what: "array_stride XCDR2 Cyclone+FastDDS BE", hex: "0000000e000000010002000000000003000477", max_align: 4, order: BE, decode: array_stride_delimited, expected: array_stride_delimited_values, lengths: &[(0, 4)] },
        Row { what: "array_stride XCDR2 RTI BE", hex: "000000010002000000000003000477", max_align: 4, order: BE, decode: array_stride, expected: array_stride_values, lengths: &[] },
        Row { what: "array_nested XCDR1 all three BE", hex: "00000001000000020000000300000004", max_align: 8, order: BE, decode: array_cell_plain, expected: two_cells, lengths: &[] },
        Row { what: "array_nested XCDR2 Cyclone+FastDDS BE", hex: "0000001000000001000000020000000300000004", max_align: 4, order: BE, decode: array_cell_delimited, expected: two_cells, lengths: &[(0, 4)] },
        Row { what: "array_nested XCDR2 RTI BE", hex: "00000001000000020000000300000004", max_align: 4, order: BE, decode: array_cell_plain, expected: two_cells, lengths: &[] },
        Row { what: "seq_struct XCDR1 all three BE", hex: "0000000200000001000000020000000300000004", max_align: 8, order: BE, decode: seq_cell_plain, expected: two_cells, lengths: &[(0, 4)] },
        Row { what: "seq_struct XCDR2 Cyclone+FastDDS BE", hex: "000000140000000200000001000000020000000300000004", max_align: 4, order: BE, decode: seq_cell_delimited, expected: two_cells, lengths: &[(0, 4), (4, 4)] },
        Row { what: "seq_struct XCDR2 RTI BE", hex: "0000000200000001000000020000000300000004", max_align: 4, order: BE, decode: seq_cell_plain, expected: two_cells, lengths: &[(0, 4)] },
        Row { what: "seq_appendable XCDR1 all three BE", hex: "0000000200000001000000020000000300000004", max_align: 8, order: BE, decode: seq_appendable_xcdr1, expected: two_cells, lengths: &[(0, 4)] },
        Row { what: "seq_appendable XCDR2 Cyclone+FastDDS BE", hex: "0000001c00000002000000080000000100000002000000080000000300000004", max_align: 4, order: BE, decode: seq_appendable_delimited, expected: two_cells, lengths: &[(0, 4), (4, 4), (8, 4), (20, 4)] },
        Row { what: "seq_appendable XCDR2 RTI BE", hex: "00000002000000080000000100000002000000080000000300000004", max_align: 4, order: BE, decode: seq_appendable_plain, expected: two_cells, lengths: &[(0, 4), (4, 4), (16, 4)] },
        Row { what: "seq_enum XCDR1 all three BE", hex: "000000020000000000000002", max_align: 8, order: BE, decode: seq_enum, expected: enum_pair_values, lengths: &[(0, 4)] },
        Row { what: "seq_enum XCDR2 Cyclone BE", hex: "0000000c000000020000000000000002", max_align: 4, order: BE, decode: seq_enum_delimited, expected: enum_pair_values, lengths: &[(0, 4), (4, 4)] },
        Row { what: "seq_enum XCDR2 RTI+FastDDS BE", hex: "000000020000000000000002", max_align: 4, order: BE, decode: seq_enum, expected: enum_pair_values, lengths: &[(0, 4)] },
        Row { what: "array_enum XCDR1 all three BE", hex: "0000000000000002", max_align: 8, order: BE, decode: array_enum, expected: enum_pair_values, lengths: &[] },
        Row { what: "array_enum XCDR2 Cyclone BE", hex: "000000080000000000000002", max_align: 4, order: BE, decode: array_enum_delimited, expected: enum_pair_values, lengths: &[(0, 4)] },
        Row { what: "array_enum XCDR2 RTI+FastDDS BE", hex: "0000000000000002", max_align: 4, order: BE, decode: array_enum, expected: enum_pair_values, lengths: &[] },
        Row { what: "ext_final XCDR1 all three BE", hex: "00000007000000003ff8000000000000fffd", max_align: 8, order: BE, decode: ext_plain, expected: ext_values, lengths: &[] },
        Row { what: "ext_final XCDR2 all three BE", hex: "000000073ff8000000000000fffd", max_align: 4, order: BE, decode: ext_plain, expected: ext_values, lengths: &[] },
        Row { what: "ext_appendable XCDR1 all three BE", hex: "00000007000000003ff8000000000000fffd", max_align: 8, order: BE, decode: ext_plain, expected: ext_values, lengths: &[] },
        Row { what: "ext_appendable XCDR2 all three BE", hex: "0000000e000000073ff8000000000000fffd", max_align: 4, order: BE, decode: ext_appendable, expected: ext_values, lengths: &[(0, 4)] },
        Row { what: "ext_mutable XCDR2 all three BE", hex: "0000001a2000000000000007300000013ff800000000000010000002fffd", max_align: 4, order: BE, decode: ext_mutable_xcdr2, expected: ext_values, lengths: &[(0, 4)] },

        // The three PL_CDR1 shapes. Cyclone's is the one the reader
        // could not parse at all before: `PID_EXTENDED` and the
        // sentinel both carry the must-understand bit.
        Row { what: "ext_mutable XCDR1 Cyclone BE", hex: "7f0100080000000000000004000000077f01000800000001000000083ff80000000000007f0100080000000200000004fffd00007f020000", max_align: 8, order: BE, decode: pl_cdr1_ext, expected: ext_values, lengths: &[(2, 2), (8, 4), (18, 2), (24, 4), (38, 2), (44, 4)] },
        Row { what: "ext_mutable XCDR1 RTI BE", hex: "0000000400000007000100083ff800000000000000020004fffd00007f020000", max_align: 8, order: BE, decode: pl_cdr1_ext, expected: ext_values, lengths: &[(2, 2), (10, 2), (22, 2)] },
        Row { what: "ext_mutable XCDR1 FastDDS BE", hex: "0000000400000007000100083ff800000000000000020002fffd00003f020000", max_align: 8, order: BE, decode: pl_cdr1_ext, expected: ext_values, lengths: &[(2, 2), (10, 2), (22, 2)] },

        Row { what: "mut_optional XCDR2 all three BE", hex: "000000082000000000000007", max_align: 4, order: BE, decode: mut_optional_xcdr2, expected: mut_optional_values, lengths: &[(0, 4)] },
        Row { what: "mut_optional XCDR1 Cyclone BE", hex: "7f0100080000000000000004000000077f020000", max_align: 8, order: BE, decode: pl_cdr1_ext, expected: mut_optional_values, lengths: &[(2, 2), (8, 4)] },
        Row { what: "mut_optional XCDR1 RTI BE", hex: "00000004000000077f020000", max_align: 8, order: BE, decode: pl_cdr1_ext, expected: mut_optional_values, lengths: &[(2, 2)] },
        Row { what: "mut_optional XCDR1 FastDDS BE", hex: "00000004000000073f020000", max_align: 8, order: BE, decode: pl_cdr1_ext, expected: mut_optional_values, lengths: &[(2, 2)] },
    ]
    .into_iter()
    .filter(|row| !row.hex.is_empty())
    .collect()
}

fn nothing(_: &mut Reader<'_>) -> Result<Vec<Field>, WireError> {
    Ok(Vec::new())
}

// -- 1. the value ----------------------------------------------------

/// Every recorded sample decodes to what the vendor was given.
#[test]
fn every_recorded_sample_reads_back_as_the_value_that_went_in() {
    for row in rows() {
        let bytes = bytes_of(&row.hex.replace(' ', ""));
        match decode_all(&row, &bytes) {
            Ok(fields) => assert_eq!(
                fields,
                (row.expected)(),
                "{}: the values came back different",
                row.what
            ),
            Err(error) => panic!("{}: {error}", row.what),
        }
    }
}

/// The rows cover both byte orders, both encodings and all three
/// vendors. A table that quietly lost half of itself would pass every
/// assertion above.
#[test]
fn the_rows_cover_what_the_fixture_records() {
    let rows = rows();
    assert!(rows.len() >= 45, "only {} rows", rows.len());
    for needle in [
        "XCDR1",
        "XCDR2",
        "Cyclone",
        "RTI",
        "FastDDS",
        "all three",
        " LE",
        " BE",
    ] {
        assert!(
            rows.iter().any(|row| row.what.contains(needle)),
            "no row covers {needle}"
        );
    }
}

// -- 2. truncation ---------------------------------------------------

/// Every prefix of every sample, at every length, is an error.
///
/// Not a panic — the test process would say so — and not a value: a
/// short stream that decoded is a decoder that stopped before the end
/// and reported success, which is the whole class.
#[test]
fn every_prefix_of_every_sample_is_a_named_error() {
    for row in rows() {
        let bytes = bytes_of(&row.hex.replace(' ', ""));
        for cut in 0..bytes.len() {
            let error = decode_all(&row, &bytes[..cut]);
            assert!(
                error.is_err(),
                "{}: {cut} of {} bytes decoded to {error:?}",
                row.what,
                bytes.len()
            );
            let at = offset_of(&error.unwrap_err());
            assert!(
                at <= cut,
                "{}: cut at {cut}, error reported at {at}, which the stream does not have",
                row.what
            );
        }
    }
}

/// Where an error says it happened.
fn offset_of(error: &WireError) -> usize {
    match error {
        WireError::Truncated { at, .. }
        | WireError::ImpossibleLength { at, .. }
        | WireError::NotUtf8 { at }
        | WireError::OutOfRange { at, .. }
        | WireError::Discarded { at, .. }
        | WireError::WrongFraming { at, .. } => *at,
    }
}

// -- 3. a length that cannot be true ---------------------------------

/// Each length field, replaced with the largest value it can hold.
///
/// The reader must refuse it and must name the field's own offset. An
/// EMHEADER's NEXTINT and a PL_CDR1 parameter length both used to be
/// returned unchecked, so `ffffffff` came back as a member extent and
/// whatever the caller did with it was the caller's problem.
#[test]
fn a_length_field_that_cannot_be_true_is_refused_at_its_own_offset() {
    let mut checked = 0usize;
    for row in rows() {
        let bytes = bytes_of(&row.hex.replace(' ', ""));
        for &(offset, width) in row.lengths {
            let mut hostile = bytes.clone();
            assert!(
                offset + width <= hostile.len(),
                "{}: no length field at {offset}",
                row.what
            );
            for byte in &mut hostile[offset..offset + width] {
                *byte = 0xFF;
            }
            let error = decode_all(&row, &hostile).expect_err(&format!(
                "{}: a length of all ones at {offset} was accepted",
                row.what
            ));
            assert_eq!(
                offset_of(&error),
                offset,
                "{}: the length at {offset} was refused as {error:?}",
                row.what
            );
            assert!(
                matches!(
                    error,
                    WireError::ImpossibleLength { .. } | WireError::OutOfRange { .. }
                ),
                "{}: a length of all ones at {offset} gave {error:?}",
                row.what
            );
            checked += 1;
        }
    }
    assert!(checked >= 40, "only {checked} length fields were corrupted");
}

/// Every four-byte word of every sample, replaced with all ones.
///
/// Broader and weaker than the assertion above: most words are payload
/// and a payload of `ffffffff` is a legal sample. What is asserted is
/// what must hold whatever the bytes say — no panic, and an error that
/// names an offset the stream has. A reader that indexed a claimed
/// length would come apart here.
#[test]
fn no_word_of_any_sample_can_be_made_to_panic_the_reader() {
    for row in rows() {
        let bytes = bytes_of(&row.hex.replace(' ', ""));
        for offset in 0..bytes.len() {
            for pattern in [0xFFu8, 0x80, 0x7F, 0x01] {
                let mut hostile = bytes.clone();
                let end = (offset + 4).min(hostile.len());
                for byte in &mut hostile[offset..end] {
                    *byte = pattern;
                }
                if let Err(error) = decode_all(&row, &hostile) {
                    assert!(
                        offset_of(&error) <= hostile.len(),
                        "{}: an error at {} for {} bytes",
                        row.what,
                        offset_of(&error),
                        hostile.len()
                    );
                }
            }
        }
    }
}

// -- 4. the wrong framing ---------------------------------------------

/// Each vendor's PL_CDR1 stream, read under the other two's framing.
///
/// This is the half that matters. A reader configured for the wrong
/// peer must say so and name the setting that fits, not return a
/// plausible sample. Where the bytes cannot tell the two apart — a
/// parameter whose value already fills its four-byte slot declares the
/// same length either way — the read is allowed to succeed, and then it
/// has to return the **sample**, which is the assertion that keeps
/// "undecidable" from meaning "unchecked".
#[test]
fn a_stream_read_under_another_vendors_framing_is_a_named_error() {
    let mut distinguished = 0usize;
    let mut undecidable: Vec<&str> = Vec::new();
    for row in rows() {
        if !row.what.contains("XCDR1")
            || row.what.contains("all three")
            || !(row.what.contains("mutable") || row.what.contains("optional"))
        {
            continue;
        }
        let bytes = bytes_of(&row.hex.replace(' ', ""));
        let own = framing(&row);
        for other in [
            CYCLONE,
            PlCdr1Framing {
                pid_flags: PidFlagPolicy::SentinelMustUnderstand,
                highest_short_id: 0x3f00,
                pad_declared_length: true,
                pad_declared_optional_length: true,
                member_must_understand: true,
                require_extended: false,
                pad_after_value: true,
            },
            PlCdr1Framing {
                pid_flags: PidFlagPolicy::Bare,
                highest_short_id: 0x3f00,
                pad_declared_length: false,
                pad_declared_optional_length: false,
                member_must_understand: false,
                require_extended: false,
                pad_after_value: false,
            },
        ] {
            if other == own {
                continue;
            }
            let mut reader = Reader::with_max_align(&bytes, row.order, row.max_align)
                .with_pl_cdr1_framing(other);
            match (row.decode)(&mut reader) {
                Err(WireError::WrongFraming { fits, .. }) => {
                    assert!(
                        fits.is_some(),
                        "{}: the error does not name a setting that fits",
                        row.what
                    );
                    distinguished += 1;
                }
                Err(error) => panic!(
                    "{}: read under another framing gave `{error}`, which does not say \
                     that the framing is the problem",
                    row.what
                ),
                Ok(fields) => {
                    // Not "allowed to succeed" — required to be right.
                    // A read the framing cannot distinguish is a read of
                    // the same bytes under the same rules, so it has to
                    // hand back the sample the vendor was given.
                    assert_eq!(
                        fields,
                        (row.expected)(),
                        "{}: read under an indistinguishable framing and came back different",
                        row.what
                    );
                    undecidable.push(row.what);
                }
            }
        }
    }
    assert!(
        distinguished >= 6,
        "only {distinguished} cross-framing read(s) were caught; the table has lost rows"
    );
    // Two shapes the bytes cannot separate, and the second is a
    // measurement rather than a slack.
    //
    // `mut_optional` is one `long` in one parameter: four bytes of value
    // in a four-byte slot, so RTI's and Fast DDS's streams are the same
    // bytes but for the sentinel.
    //
    // `ext_mutable XCDR1 Cyclone` reads under RTI's framing because
    // Cyclone and RTI answer **every** field of the framing alike: the
    // same padded length, the same `7f02`, and — since the
    // `PID_EXTENDED` marker's flags left `PlCdr1HeaderPolicy` for
    // `PidFlagPolicy` — the same `7f01`. The only difference left is
    // `require_extended`, which is a reader's demand rather than a
    // property of the bytes, and a reader that demands less reads more.
    // Before that correction this row was separated by a marker flag
    // this project attributed to the framing form and RTI does not.
    assert!(
        undecidable
            .iter()
            .all(|what| what.contains("mut_optional") || what.contains("ext_mutable")),
        "a row was read under the wrong framing and came back a value: {undecidable:?}"
    );
}
