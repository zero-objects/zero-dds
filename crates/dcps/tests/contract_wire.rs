// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire`, from its non-test sources:
//! - `Writer::with_max_align(Big, 4)` / `Writer::appending(Big, 4, buffer)`
//!   with `write_bool`, `write_u8`, `write_i8`, `write_u16`, `write_i16`,
//!   `write_u32`, `write_i32`, `write_u64`, `write_i64`, `write_f32`,
//!   `write_f64`, `write_string`, `write_bytes`, `position`, `as_bytes`,
//!   `finish` for the PLAIN_CDR2 big-endian key holder (`key_hash.rs`), and
//!   `md5` for a KeyHash of a holder longer than sixteen octets
//!   (`key_hash.rs`, `runtime.rs::payload_key_hash`).
//! - `Writer::new(Little)`, `write_bytes`, `write_string`, `finish` and
//!   `Reader::new(bytes, Little)`, `read_bytes(16)`, `read_string`,
//!   `read_u32` for the built-in topic samples (`builtin_topics.rs`) and
//!   `DdsAny` (`dds_type.rs`).
//! - `Writer::with_max_align` / `appending` with `write_dheader` and
//!   `finish_dheader`, `Reader::with_max_align` with `read_dheader`,
//!   `position`, `remaining`, `skip_to` for the `@appendable` built-in
//!   types (`builtin_types.rs`), the ShapesDemo type (`interop.rs`) and the
//!   security log topic (`security_log_topic.rs`).
//! - `Reader::with_pl_cdr1_reading(PlCdr1Reading::{Strict, Tolerant})` for
//!   TypeLookup requests, `remaining` and `read_bytes` for the padding behind
//!   them (`runtime.rs`).
//! - `WireError`: all six variants matched exhaustively and narrowed into
//!   `EncodeError`/`DecodeError` (`dds_type.rs`), `Discarded` kept apart as
//!   `DecodeError::Discarded`; `OutOfRange` and `ImpossibleLength` built by
//!   hand (`dds_type.rs`, `builtin_types.rs`).
//!
//! Oracles: DDS-XTypes 1.3 §7.4.1 (MAXALIGN 8 under XCDR1 and 4 under
//! XCDR2), §7.4.1.1 (a string's length counts its NUL), §7.4.1.2.1
//! (`PID_EXTENDED` 0x3f01 and its eight-octet short length), §7.4.3.4.2 (the
//! DHEADER counts the octets behind it), §7.2.4.1 (DISCARD), §7.6.8
//! Example 2 (the key holder and KeyHash octets the specification prints),
//! RFC 1321 Appendix A.5 (the MD5 test suite), IEEE 754 (binary32 and
//! binary64 bit patterns).
#![allow(clippy::unwrap_used)]

use zerodds_dcps::key_hash::compute_key_hash;
use zerodds_dcps::{DecodeError, EncodeError};
use zerodds_wire::{
    Endianness, Narrowed, PlCdr1Reading, Reader, TryConstruct, WireError, Writer, fit_string, md5,
};

// ---------------------------------------------------------------------
// The key holder and the KeyHash
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_4_1_the_key_holder_is_big_endian_with_maxalign_four() {
    let mut w = Writer::with_max_align(Endianness::Big, 4);
    w.write_bool(true);
    w.write_u16(0x0203);
    w.write_u8(0x04);
    w.write_u64(0x1112_1314_1516_1718);
    w.write_i8(-1);
    w.write_i16(-2);
    w.write_i32(-3);
    w.write_i64(-4);
    w.write_f32(1.0);
    w.write_f64(-2.0);
    w.write_u32(5);
    w.write_string("ab");
    w.write_bytes(&[0xEE, 0xEE]);
    let parts: [&[u8]; 11] = [
        &[0x01, 0x00, 0x02, 0x03],                         // bool, pad, u16
        &[0x04, 0x00, 0x00, 0x00],                         // u8, pad to 4
        &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18], // u64 at 8: aligned to 4
        &[0xFF, 0x00, 0xFF, 0xFE],                         // i8, pad, i16
        &[0xFF, 0xFF, 0xFF, 0xFD],                         // i32
        &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFC], // i64 at 24
        &[0x3F, 0x80, 0x00, 0x00],                         // 1.0f32
        &[0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // -2.0f64 at 36
        &[0x00, 0x00, 0x00, 0x05],                         // u32
        &[0x00, 0x00, 0x00, 0x03, b'a', b'b', 0x00],       // string, NUL counted
        &[0xEE, 0xEE],                                     // raw octets
    ];
    let expected = parts.concat();
    assert_eq!(w.position(), expected.len());
    assert_eq!(w.as_bytes(), &expected[..]);
    assert_eq!(w.finish(), expected);

    // The reusing holder appends to a cleared buffer: the same octets.
    let mut again = Writer::appending(Endianness::Big, 4, Vec::with_capacity(64));
    again.write_u8(0x04);
    again.write_u64(0x1112_1314_1516_1718);
    assert_eq!(
        again.finish(),
        [
            0x04, 0, 0, 0, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18
        ]
    );
}

#[test]
fn xtypes_7_4_1_the_xcdr1_writer_and_reader_align_eight_octet_values_to_eight() {
    // `Writer::new` and `Reader::new` are what the XCDR1 paths take.
    let mut w = Writer::new(Endianness::Little);
    assert_eq!(w.max_align(), 8);
    w.write_u8(1);
    w.write_u64(2);
    assert_eq!(w.finish(), [1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(Reader::new(&[], Endianness::Little).max_align(), 8);
    assert_eq!(
        Reader::with_max_align(&[], Endianness::Little, 4).max_align(),
        4
    );
}

#[test]
fn xtypes_7_6_8_example_2_holder_and_md5_key_hash() {
    // `@final struct Foo { @key string<12> label; @key long long id; ... }`
    // with label "BLUE", id 0x123456789abcdef0: the holder and the hash
    // as the specification prints them.
    let mut holder = Writer::with_max_align(Endianness::Big, 4);
    holder.write_string("BLUE");
    holder.write_i64(0x1234_5678_9abc_def0);
    let holder = holder.finish();
    assert_eq!(
        holder,
        [
            0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34,
            0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0
        ]
    );
    let hash = [
        0xf9, 0x1a, 0x59, 0xe3, 0x2e, 0x45, 0x35, 0xd9, 0xa6, 0x9c, 0xd5, 0xd9, 0xf5, 0xb6, 0xe3,
        0x6e,
    ];
    assert_eq!(md5(&holder), hash);
    // Its maximum size is 4 + 13 + 3 + 8 = 28 > 16: the MD5 branch.
    assert_eq!(compute_key_hash(&holder, 28), hash);
}

#[test]
fn rfc_1321_appendix_a_5_md5_test_suite() {
    let hex = |digest: [u8; 16]| {
        digest
            .iter()
            .map(|octet| format!("{octet:02x}"))
            .collect::<String>()
    };
    let suite: [(&[u8], &str); 5] = [
        (b"", "d41d8cd98f00b204e9800998ecf8427e"),
        (b"a", "0cc175b9c0f1b6a831c399e269772661"),
        (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
        (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (
            b"abcdefghijklmnopqrstuvwxyz",
            "c3fcd3d76192e4007dfb496cca67e13b",
        ),
    ];
    for (input, digest) in suite {
        assert_eq!(hex(md5(input)), digest, "{input:?}");
    }
}

// ---------------------------------------------------------------------
// Samples: raw octets, strings, the DHEADER
// ---------------------------------------------------------------------

#[test]
fn raw_octets_then_strings_as_the_builtin_topic_samples_lay_them_out() {
    let key = [0x11; 16];
    let participant = [0x22; 16];
    let mut w = Writer::new(Endianness::Little);
    w.write_bytes(&key);
    w.write_bytes(&participant);
    w.write_string("Square");
    w.write_string("ShapeType");
    let bytes = w.finish();
    let mut expected = [key, participant].concat();
    expected.extend_from_slice(&[7, 0, 0, 0]);
    expected.extend_from_slice(b"Square\0");
    expected.push(0); // offset 43 → the next length at 44
    expected.extend_from_slice(&[10, 0, 0, 0]);
    expected.extend_from_slice(b"ShapeType\0");
    assert_eq!(bytes, expected);

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_bytes(16).unwrap(), key);
    assert_eq!(r.read_bytes(16).unwrap(), participant);
    assert_eq!(r.read_string().unwrap(), "Square");
    assert_eq!(r.read_string().unwrap(), "ShapeType");
    assert_eq!(r.remaining(), 0);
    // A GUID cut short is refused, not zero-filled.
    let mut short = Reader::new(&bytes[..10], Endianness::Little);
    assert!(matches!(
        short.read_bytes(16),
        Err(WireError::Truncated {
            at: 0,
            needed: 16,
            available: 10
        })
    ));
}

#[test]
fn xtypes_7_4_3_4_2_the_dheader_frames_the_body_and_a_reader_skips_what_it_does_not_know() {
    let little: [&[u8]; 5] = [
        &[0x10, 0, 0, 0],
        &[3, 0, 0, 0, b'h', b'i', 0],
        &[0],
        &[5, 0, 0, 0],
        &[9, 0, 0, 0],
    ];
    let big: [&[u8]; 5] = [
        &[0, 0, 0, 0x10],
        &[0, 0, 0, 3, b'h', b'i', 0],
        &[0],
        &[0, 0, 0, 5],
        &[0, 0, 0, 9],
    ];
    for (endianness, expected) in [
        (Endianness::Little, little.concat()),
        (Endianness::Big, big.concat()),
    ] {
        let mut w = Writer::with_max_align(endianness, 4);
        let header = w.write_dheader();
        w.write_string("hi");
        w.write_u32(5);
        w.write_u32(9); // a member the reader below does not declare
        w.finish_dheader(header);
        let bytes = w.finish();
        // 7 (string) + 1 (pad) + 4 + 4 = 16 octets behind the DHEADER.
        assert_eq!(bytes, expected);

        let mut r = Reader::with_max_align(&bytes, endianness, 4);
        let length = r.read_dheader().unwrap();
        assert_eq!(length, 16);
        let end = r.position() + length;
        assert_eq!(r.read_string().unwrap(), "hi");
        assert_eq!(r.read_u32().unwrap(), 5);
        r.skip_to(end).unwrap();
        assert_eq!(r.remaining(), 0);
        // Neither past the stream nor back behind the cursor.
        assert!(matches!(
            r.skip_to(end + 1),
            Err(WireError::Truncated { .. })
        ));
        assert!(matches!(r.skip_to(4), Err(WireError::OutOfRange { .. })));
    }

    // A DHEADER claiming more than the stream holds.
    let mut r = Reader::with_max_align(&[0x10, 0, 0, 0, 1, 2, 3, 4], Endianness::Little, 4);
    assert_eq!(
        r.read_dheader(),
        Err(WireError::ImpossibleLength {
            at: 0,
            claimed: 16,
            available: 4
        })
    );
}

#[test]
fn an_appending_writer_counts_alignment_from_where_the_buffer_ends() {
    // `interop.rs` hands its output buffer over, which may already hold
    // the encapsulation header or anything else: the body aligns as if it
    // started at offset 0, and the buffer comes back with the body behind.
    let mut w = Writer::appending(Endianness::Little, 4, vec![0xAA]);
    let header = w.write_dheader();
    w.write_u8(7);
    w.write_u32(1);
    w.finish_dheader(header);
    assert_eq!(w.finish(), [0xAA, 8, 0, 0, 0, 7, 0, 0, 0, 1, 0, 0, 0]);
}

#[test]
fn xtypes_7_4_1_1_a_string_counts_its_nul_and_a_disagreeing_one_is_refused() {
    let mut w = Writer::new(Endianness::Little);
    w.write_string("");
    assert_eq!(w.finish(), [1, 0, 0, 0, 0]);

    let read = |bytes: &[u8]| Reader::new(bytes, Endianness::Little).read_string();
    assert_eq!(read(&[1, 0, 0, 0, 0]).unwrap(), "");
    // A length of 0 cannot count a NUL.
    assert!(matches!(
        read(&[0, 0, 0, 0]),
        Err(WireError::ImpossibleLength { claimed: 0, .. })
    ));
    // A length past the stream.
    assert!(matches!(
        read(&[5, 0, 0, 0, b'a', b'b']),
        Err(WireError::ImpossibleLength { claimed: 5, .. })
    ));
    // The octet the length counts as the NUL is not one.
    assert!(matches!(
        read(&[3, 0, 0, 0, b'a', b'b', b'c']),
        Err(WireError::OutOfRange { at: 6, .. })
    ));
    assert!(matches!(
        read(&[2, 0, 0, 0, 0xFF, 0]),
        Err(WireError::NotUtf8 { at: 0 })
    ));
    assert!(matches!(read(&[1, 0]), Err(WireError::Truncated { .. })));
}

// ---------------------------------------------------------------------
// The error kinds the runtime narrows
// ---------------------------------------------------------------------

#[test]
fn the_six_wire_error_kinds_narrow_apart_and_only_discarded_stays_a_discard() {
    let kinds = [
        WireError::Truncated {
            at: 1,
            needed: 4,
            available: 2,
        },
        WireError::ImpossibleLength {
            at: 2,
            claimed: 9,
            available: 3,
        },
        WireError::NotUtf8 { at: 3 },
        WireError::OutOfRange {
            at: 4,
            expected: String::from("a DdsAny payload of at most 4294967295 octets"),
        },
        WireError::Discarded {
            at: 5,
            expected: String::from("at most 3 octets"),
            found: String::from("4 of them"),
        },
        WireError::WrongFraming {
            at: 6,
            expected: String::from("PID_EXTENDED with must-understand"),
            found: String::from("a bare PID_EXTENDED"),
            fits: Some("--pl-cdr1-reading tolerant"),
        },
    ];
    let mut reasons: Vec<String> = Vec::new();
    for kind in kinds {
        let discard = kind.is_discarded();
        assert_eq!(discard, matches!(kind, WireError::Discarded { .. }));
        let decoded = DecodeError::from(kind.clone());
        assert_eq!(decoded.is_discarded(), discard, "{kind:?}");
        assert_eq!(
            matches!(decoded, DecodeError::Invalid { .. }),
            !discard,
            "{kind:?}"
        );
        reasons.push(EncodeError::from(kind).to_string());
    }
    reasons.sort();
    reasons.dedup();
    assert_eq!(reasons.len(), 6, "each kind keeps its own reason");

    // XTypes 1.3 §7.2.4.1: DISCARD is what a narrowing that does not fit
    // raises; it is the `Discarded` the runtime keeps apart.
    let refused = fit_string(12, String::from("ORANGE"), 4, TryConstruct::Discard);
    assert!(matches!(refused, Err(ref e) if e.is_discarded()));
    assert!(DecodeError::from(refused.unwrap_err()).is_discarded());
    assert_eq!(
        fit_string(12, String::from("RED"), 4, TryConstruct::Discard),
        Ok(Narrowed::Value(String::from("RED")))
    );
}

#[test]
fn xtypes_7_4_1_2_1_a_bare_pid_extended_is_read_only_when_tolerant() {
    // PID_EXTENDED written without the must-understand flag (0x3f01 rather
    // than 0x7f01), short length 8, member id 1, length 4, the value.
    let bytes: [u8; 16] = [
        0x01, 0x3F, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00,
        0x00,
    ];
    let mut strict = Reader::new(&bytes, Endianness::Little);
    assert_eq!(strict.pl_cdr1_reading(), PlCdr1Reading::Strict);
    assert!(matches!(
        strict.read_pid(),
        Err(WireError::WrongFraming { .. })
    ));

    let mut tolerant =
        Reader::new(&bytes, Endianness::Little).with_pl_cdr1_reading(PlCdr1Reading::Tolerant);
    assert!(tolerant.pl_cdr1_reading().is_tolerant());
    let header = tolerant.read_pid().unwrap();
    assert_eq!((header.pid, header.length, header.end), (1, 4, false));
    assert_eq!(tolerant.read_u32().unwrap(), 5);
}
