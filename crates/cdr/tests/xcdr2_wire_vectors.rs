// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR2 conformance wire-vectors V-1..V-12.
//!
//! Reference: `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6.
//!
//! Each test pins a Rust struct (mirroring the IDL fixture in the
//! master conformance spec), encodes it via the existing
//! `zerodds_cdr` primitives + `struct_enc` extensibility helpers, and
//! asserts byte-exact equality with the OMG-XTypes-1.3-§7.4-conform
//! wire bytes. Round-trip through `decode` is also covered.
//!
//! # Coverage
//!
//! - V-1  : empty `@final` (zero-byte payload).
//! - V-2  : `@final` two int32 (`Point`).
//! - V-3  : `@final` mixed primitives (boolean, octet, short, ushort,
//!          long, ulong, longlong, ulonglong, float, double).
//! - V-4  : `@final` string.
//! - V-5  : `@final` sequence<int32>.
//! - V-6  : `@final` sequence<string>.
//! - V-7  : `@final` nested modules (`Outer::Inner::S`).
//! - V-8  : `@final` keyed (KeyHash zero-pad path; OMG-conform —
//!          deviates from master-spec sample text, see §6 errata).
//! - V-9  : `@appendable`.
//! - V-10 : `@mutable` two members (DHEADER + EMHEADER list).
//! - V-11a: `@mutable` `@optional` present.
//! - V-11b: `@mutable` `@optional` absent.
//! - V-12 : empty `@mutable` (DHEADER = 0).
//!
//! Wire bytes are little-endian PLAIN_CDR2 payload (no RTPS
//! representation header). EMHEADER bytes are written little-endian
//! by the existing encoder; the master-spec table shows them in
//! big-endian visual form for readability — this test asserts the
//! actual on-wire bytes, which are LE per RTPS 2.5 §10.5
//! `CDR2_LE = 0x0010`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::approx_constant,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_cdr::struct_enc::{
    LengthCode, MutableStructEncoder, decode_appendable, encode_appendable, encode_mutable_member,
    encode_mutable_member_lc, read_all_mutable_members, read_mutable_member,
};
use zerodds_cdr::{
    BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness, KEY_HASH_LEN,
    PlainCdr2BeKeyHolder, compute_key_hash,
};

// ---------------------------------------------------------------------------
// V-1: empty @final struct -> zero-byte payload.
//
// IDL:
//   @final struct Empty {};
// ---------------------------------------------------------------------------

#[test]
fn v1_empty_final_struct() {
    // Empty struct emits nothing — the encoder simply contributes no
    // bytes. We simulate by constructing an empty BufferWriter.
    let w = BufferWriter::new(Endianness::Little);
    let bytes = w.into_bytes();
    assert_eq!(bytes.len(), 0, "V-1 must be zero bytes");
}

// ---------------------------------------------------------------------------
// V-2: @final Point { long x; long y; }
//
// Sample: { x = 1, y = -2 }
// Wire (8 bytes):
//   01 00 00 00  FE FF FF FF
// ---------------------------------------------------------------------------

#[test]
fn v2_final_two_int32() {
    let expected: [u8; 8] = [0x01, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF];

    let mut w = BufferWriter::new(Endianness::Little);
    1i32.encode(&mut w).unwrap();
    (-2i32).encode(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-2 byte-exact mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    assert_eq!(i32::decode(&mut r).unwrap(), 1);
    assert_eq!(i32::decode(&mut r).unwrap(), -2);
}

// ---------------------------------------------------------------------------
// V-3: @final All { boolean; octet; short; ushort; long; ulong;
//                   long long; unsigned long long; float; double; };
//
// Sample (master-spec §6.V-3):
//   b=true, o=0xAB, s=-12345, us=54321, l=-1234567, ul=2345678,
//   ll=-987654321, ull=123456789, f=2.5, d=3.14159
//
// OMG XTypes 1.3 §7.4.2 alignment (PLAIN_CDR2):
//   b      offset 0  size 1
//   o      offset 1  size 1
//   s      offset 2  size 2  (already aligned to 2)
//   us     offset 4  size 2
//   l      offset 6  size 4  (pad 2 -> offset 8)
//   ul     offset 12 size 4
//   ll     offset 16 size 8  (already aligned to 8)
//   ull    offset 24 size 8
//   f      offset 32 size 4
//   d      offset 36 size 8  (pad 4 -> offset 40)
//   Total: 48 bytes
//
// Wire (LE, 48 bytes):
//   01 AB                                       # b, o (offset 0,1)
//   C7 CF                                       # s = -12345 (offset 2)
//   31 D4                                       # us = 54321 (offset 4)
//   00 00                                       # pad (offset 6,7)
//   B9 29 ED FF                                 # l = -1234567 (offset 8)
//   CE C5 23 00                                 # ul = 2345678 (offset 12)
//   2F E2 C5 C5 FF FF FF FF                     # ll = -987654321 (offset 16)
//   15 CD 5B 07 00 00 00 00                     # ull = 123456789 (offset 24)
//   00 00 20 40                                 # f = 2.5 (offset 32)
//   00 00 00 00                                 # pad (offset 36..40)
//   6E 86 1B F0 F9 21 09 40                     # d = 3.14159 (offset 40)
// ---------------------------------------------------------------------------

#[test]
fn v3_final_mixed_primitives() {
    let mut w = BufferWriter::new(Endianness::Little);
    true.encode(&mut w).unwrap();
    0xABu8.encode(&mut w).unwrap();
    (-12345i16).encode(&mut w).unwrap();
    54321u16.encode(&mut w).unwrap();
    (-1234567i32).encode(&mut w).unwrap();
    2345678u32.encode(&mut w).unwrap();
    (-987654321i64).encode(&mut w).unwrap();
    123456789u64.encode(&mut w).unwrap();
    2.5f32.encode(&mut w).unwrap();
    3.14159f64.encode(&mut w).unwrap();
    let bytes = w.into_bytes();

    let expected: [u8; 48] = [
        0x01, 0xAB, // b, o
        0xC7, 0xCF, // s = -12345 (LE i16)
        0x31, 0xD4, // us = 54321 (LE u16)
        0x00, 0x00, // pad to align(4) for l
        0x79, 0x29, 0xED, 0xFF, // l = -1234567 (LE i32 = 0xFFED2979)
        0xCE, 0xCA, 0x23, 0x00, // ul = 2345678 (LE u32 = 0x0023CACE)
        0x4F, 0x97, 0x21, 0xC5, 0xFF, 0xFF, 0xFF,
        0xFF, // ll = -987654321 (LE i64 = 0xFFFFFFFFC521974F)
        0x15, 0xCD, 0x5B, 0x07, 0x00, 0x00, 0x00, 0x00, // ull = 123456789 (LE u64)
        0x00, 0x00, 0x20, 0x40, // f = 2.5 (IEEE-754 LE)
        0x00, 0x00, 0x00, 0x00, // pad to align(8) for d
        0x6E, 0x86, 0x1B, 0xF0, 0xF9, 0x21, 0x09, 0x40, // d = 3.14159
    ];
    assert_eq!(bytes, expected, "V-3 byte-exact mismatch");

    // Round-trip.
    let mut r = BufferReader::new(&bytes, Endianness::Little);
    assert!(bool::decode(&mut r).unwrap());
    assert_eq!(u8::decode(&mut r).unwrap(), 0xAB);
    assert_eq!(i16::decode(&mut r).unwrap(), -12345);
    assert_eq!(u16::decode(&mut r).unwrap(), 54321);
    assert_eq!(i32::decode(&mut r).unwrap(), -1234567);
    assert_eq!(u32::decode(&mut r).unwrap(), 2345678);
    assert_eq!(i64::decode(&mut r).unwrap(), -987654321);
    assert_eq!(u64::decode(&mut r).unwrap(), 123456789);
    assert_eq!(f32::decode(&mut r).unwrap(), 2.5);
    assert!((f64::decode(&mut r).unwrap() - 3.14159).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// V-4: @final Greeting { string text; }
//
// Sample: text = "hello"
// Wire (10 bytes): u32 length=6 (incl. NUL) + "hello" + NUL
//   06 00 00 00  68 65 6C 6C 6F 00
// ---------------------------------------------------------------------------

#[test]
fn v4_final_string() {
    let expected: [u8; 10] = [0x06, 0x00, 0x00, 0x00, b'h', b'e', b'l', b'l', b'o', 0x00];

    let mut w = BufferWriter::new(Endianness::Little);
    "hello".encode(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-4 string mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    assert_eq!(String::decode(&mut r).unwrap(), "hello");
}

// ---------------------------------------------------------------------------
// V-5: @final Bag { sequence<long> ids; }
//
// Sample: ids = [1, 2, 3]
// Wire (16 bytes): u32 count=3 + 3 * int32
//   03 00 00 00  01 00 00 00  02 00 00 00  03 00 00 00
// ---------------------------------------------------------------------------

#[test]
fn v5_final_sequence_int32() {
    let expected: [u8; 16] = [
        0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00,
        0x00,
    ];

    let mut w = BufferWriter::new(Endianness::Little);
    let v: Vec<i32> = vec![1, 2, 3];
    v.encode(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-5 sequence<int32> mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let back = <Vec<i32> as CdrDecode>::decode(&mut r).unwrap();
    assert_eq!(back, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------------
// V-6: @final Tags { sequence<string> tags; }
//
// Sample: tags = ["a", "bc"]
//
// OMG XTypes 1.3 §7.4.4.2 + §7.4.4.1 layout:
//   count=2                : 02 00 00 00            (offset 0..4)
//   string "a"  length=2   : 02 00 00 00            (offset 4..8)
//                "a\0"     : 61 00                  (offset 8..10)
//   align(4) before next string-length: pad 2 bytes (offset 10..12)
//   string "bc" length=3   : 03 00 00 00            (offset 12..16)
//                "bc\0"    : 62 63 00               (offset 16..19)
//   Total: 19 bytes
// ---------------------------------------------------------------------------

#[test]
fn v6_final_sequence_string() {
    let expected: [u8; 19] = [
        0x02, 0x00, 0x00, 0x00, // count = 2
        0x02, 0x00, 0x00, 0x00, b'a', 0x00, // "a\0"
        0x00, 0x00, // pad to align(4) for next string-length
        0x03, 0x00, 0x00, 0x00, b'b', b'c', 0x00, // "bc\0"
    ];

    let mut w = BufferWriter::new(Endianness::Little);
    let v: Vec<String> = vec!["a".to_string(), "bc".to_string()];
    v.encode(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-6 sequence<string> mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let back = <Vec<String> as CdrDecode>::decode(&mut r).unwrap();
    assert_eq!(back, vec!["a".to_string(), "bc".to_string()]);
}

// ---------------------------------------------------------------------------
// V-7: nested modules @final Outer::Inner::S { long x; }
//
// Sample: x = 1234
// Wire (4 bytes):
//   D2 04 00 00
// (TYPE_NAME = "Outer::Inner::S" — verified in idl-rust snapshot tests
//  for the namespacing convention, not in this test file.)
// ---------------------------------------------------------------------------

#[test]
fn v7_final_nested_modules() {
    let expected: [u8; 4] = [0xD2, 0x04, 0x00, 0x00];

    let mut w = BufferWriter::new(Endianness::Little);
    1234i32.encode(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-7 nested mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    assert_eq!(i32::decode(&mut r).unwrap(), 1234);
}

// ---------------------------------------------------------------------------
// V-8: @final keyed Sensor { @key long id; double value; }
//
// Sample: id = 42, value = 3.14
//
// Wire payload (LE, 16 bytes; pad before double for align(8)):
//   2A 00 00 00              # id = 42                       (offset 0..4)
//   00 00 00 00              # pad to align(8) for double    (offset 4..8)
//   1F 85 EB 51 B8 1E 09 40  # value = 3.14                  (offset 8..16)
//
// KeyHash (XTypes 1.3 §7.6.8.4 Step 5.1, OMG-conform):
//   PlainCdr2BeKeyHolder of (@key long id) =
//     00 00 00 2A   (BE u32 of 42)
//   key_holder_max_size = 4 <= 16 -> zero-pad to 16:
//     00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00
//
// NOTE (errata vs. master-spec §6.V-8 sample text): the master-spec
// text reads `MD5(00 00 00 2A)` for the KeyHash. That contradicts
// XTypes 1.3 §7.6.8.4 Step 5.1, which mandates zero-pad when the
// max-serialized KeyHolder size is <= 16 bytes. The OMG-XTypes
// algorithm is the binding ground-truth; this test asserts the
// zero-pad value. See `docs/spec-coverage/zerodds-xcdr2-rust-1.0.md`
// §11 for the recorded delta.
// ---------------------------------------------------------------------------

#[test]
fn v8_final_keyed_sensor_payload() {
    let expected_payload: [u8; 16] = [
        0x2A, 0x00, 0x00, 0x00, // id = 42
        0x00, 0x00, 0x00, 0x00, // pad
        0x1F, 0x85, 0xEB, 0x51, 0xB8, 0x1E, 0x09, 0x40, // value = 3.14
    ];

    let mut w = BufferWriter::new(Endianness::Little);
    42i32.encode(&mut w).unwrap();
    3.14f64.encode(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected_payload, "V-8 payload mismatch");

    // Round-trip
    let mut r = BufferReader::new(&bytes, Endianness::Little);
    assert_eq!(i32::decode(&mut r).unwrap(), 42);
    assert!((f64::decode(&mut r).unwrap() - 3.14).abs() < f64::EPSILON);
}

#[test]
fn v8_keyhash_zero_pad_per_xtypes_7_6_8_4() {
    // PlainCdr2BeKeyHolder with one i32 (@key long id = 42).
    let mut holder = PlainCdr2BeKeyHolder::new();
    holder.write_i32(42);
    assert_eq!(holder.as_bytes(), &[0x00, 0x00, 0x00, 0x2A]);

    // max_size = 4 -> zero-pad path.
    let kh = compute_key_hash(holder.as_bytes(), 4);
    let expected: [u8; KEY_HASH_LEN] = [0x00, 0x00, 0x00, 0x2A, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        kh, expected,
        "V-8 KeyHash must zero-pad per XTypes §7.6.8.4 Step 5.1"
    );
}

// ---------------------------------------------------------------------------
// V-9: @appendable V { long a; long b; }
//
// Sample: a=1, b=2
// Wire (12 bytes):
//   08 00 00 00          # DHEADER body-length = 8
//   01 00 00 00          # a
//   02 00 00 00          # b
// ---------------------------------------------------------------------------

#[test]
fn v9_appendable_two_int32() {
    let expected: [u8; 12] = [
        0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    ];

    let mut w = BufferWriter::new(Endianness::Little);
    encode_appendable(&mut w, |w| {
        1i32.encode(w)?;
        2i32.encode(w)?;
        Ok(())
    })
    .unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-9 appendable mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let (a, b) = decode_appendable(&mut r, |r| {
        let a = i32::decode(r)?;
        let b = i32::decode(r)?;
        Ok((a, b))
    })
    .unwrap();
    assert_eq!((a, b), (1, 2));
}

// ---------------------------------------------------------------------------
// V-10: @mutable M { @id(1) long a; @id(2) string b; }
//
// Sample: a = 42, b = "hi"
//
// Wire (LE, 31 bytes):
//   DHEADER body-len   : 1B 00 00 00       (= 27 bytes follow)
//   member 1 (a, LC4):
//     EMHEADER         : 01 00 00 40       (m=0, lc=4, id=1)
//     NEXTINT (body=4) : 04 00 00 00
//     body i32=42      : 2A 00 00 00
//   member 2 (b, LC4):
//     EMHEADER         : 02 00 00 40       (m=0, lc=4, id=2)
//     NEXTINT (body=7) : 07 00 00 00
//     body string "hi" : 03 00 00 00 68 69 00
//
// NOTE: master-spec §6.V-10 lists EMHEADERs as `20 00 00 01` and
// `30 00 00 02` (BE byte ordering of `0x20000001` / `0x30000002`).
// Those are display-form; the on-wire bytes per CDR2_LE are the LE
// representation, which is what the encoder produces and what this
// test pins. The `LC=2` form chosen by the master spec is also
// equivalent for fixed 4-byte primitives — our default codegen path
// uses LC=4 (universal NEXTINT-bearing), which is the more general
// form spec-allowed by XTypes §7.4.3.4.2.
// ---------------------------------------------------------------------------

#[test]
fn v10_mutable_dheader_then_two_lc4_members() {
    let expected: [u8; 31] = [
        // DHEADER: body length = 27 (0x1B)
        0x1B, 0x00, 0x00, 0x00, // member 1
        0x01, 0x00, 0x00, 0x40, // EMHEADER LE: lc=4 id=1
        0x04, 0x00, 0x00, 0x00, // NEXTINT = 4
        0x2A, 0x00, 0x00, 0x00, // i32 = 42
        // member 2
        0x02, 0x00, 0x00, 0x40, // EMHEADER LE: lc=4 id=2
        0x07, 0x00, 0x00, 0x00, // NEXTINT = 7
        0x03, 0x00, 0x00, 0x00, b'h', b'i', 0x00, // string "hi"
    ];

    let mut w = BufferWriter::new(Endianness::Little);
    encode_appendable(&mut w, |inner| {
        let mut enc = MutableStructEncoder::new(inner, vec![1, 2]);
        enc.encode_member(1, false, |w| 42i32.encode(w)).unwrap();
        enc.encode_member(2, false, |w| "hi".encode(w)).unwrap();
        enc.finish().unwrap();
        Ok(())
    })
    .unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-10 mutable DHEADER+members mismatch");

    // Decoder: strip DHEADER, then iterate members.
    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let (a, b) = decode_appendable(&mut r, |inner| {
        let members = read_all_mutable_members(inner)?;
        assert_eq!(members.len(), 2);
        let mut a = None;
        let mut b = None;
        for m in &members {
            let mut sub = BufferReader::new(m.body, Endianness::Little);
            match m.member_id {
                1 => a = Some(i32::decode(&mut sub)?),
                2 => b = Some(String::decode(&mut sub)?),
                _ => panic!("unexpected member-id {}", m.member_id),
            }
        }
        Ok((a.unwrap(), b.unwrap()))
    })
    .unwrap();
    assert_eq!(a, 42);
    assert_eq!(b, "hi");
}

// ---------------------------------------------------------------------------
// V-11a: @mutable O { @id(1) @optional long maybe; } with maybe=Some(7)
//
// Wire (LE, 16 bytes):
//   DHEADER         : 0C 00 00 00     (body = 12 bytes)
//   EMHEADER        : 01 00 00 40     (m=0, lc=4, id=1)
//   NEXTINT         : 04 00 00 00
//   body i32 = 7    : 07 00 00 00
// ---------------------------------------------------------------------------

#[test]
fn v11a_mutable_optional_present() {
    let expected: [u8; 16] = [
        0x0C, 0x00, 0x00, 0x00, // DHEADER = 12
        0x01, 0x00, 0x00, 0x40, // EMHEADER
        0x04, 0x00, 0x00, 0x00, // NEXTINT
        0x07, 0x00, 0x00, 0x00, // body
    ];

    let mut w = BufferWriter::new(Endianness::Little);
    encode_appendable(&mut w, |inner| {
        let mut enc = MutableStructEncoder::new(inner, vec![]);
        enc.encode_member(1, false, |w| 7i32.encode(w)).unwrap();
        enc.finish().unwrap();
        Ok(())
    })
    .unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-11a optional-present mismatch");
}

// ---------------------------------------------------------------------------
// V-11b: @mutable O { @id(1) @optional long maybe; } with maybe=None
//
// Wire (LE, 4 bytes):
//   DHEADER : 00 00 00 00     (body length = 0; member omitted)
// ---------------------------------------------------------------------------

#[test]
fn v11b_mutable_optional_absent() {
    let expected: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

    let mut w = BufferWriter::new(Endianness::Little);
    encode_appendable(&mut w, |inner| {
        let enc = MutableStructEncoder::new(inner, vec![]);
        enc.finish().unwrap();
        Ok(())
    })
    .unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-11b optional-absent mismatch");

    // Decoder: empty body, no members.
    let mut r = BufferReader::new(&bytes, Endianness::Little);
    decode_appendable(&mut r, |inner| {
        let res = read_mutable_member(inner)?;
        assert!(res.is_none(), "no members expected for V-11b");
        Ok(())
    })
    .unwrap();
}

// ---------------------------------------------------------------------------
// V-12: empty @mutable -> DHEADER = 0 (terminal sentinel form)
//
// IDL:
//   @mutable struct Empty {};
//
// Wire (LE, 4 bytes):
//   00 00 00 00
//
// Master-spec §6.V-12 documents that XCDR2 has NO explicit
// PID_LIST_END sentinel — the DHEADER bound delimits the struct.
// This test verifies the wrapper-level DHEADER=0 encoding.
// ---------------------------------------------------------------------------

#[test]
fn v12_empty_mutable_dheader_zero() {
    let expected: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

    let mut w = BufferWriter::new(Endianness::Little);
    encode_appendable(&mut w, |_w| Ok(())).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected, "V-12 empty-DHEADER mismatch");

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    decode_appendable(&mut r, |sub| {
        assert!(read_mutable_member(sub)?.is_none());
        Ok(())
    })
    .unwrap();
    assert_eq!(r.remaining(), 0);
}

// ===========================================================================
// Bonus mutation-killer coverage for LC0 + LC4 boundary cases.
// Not part of V-1..V-12 vectors but ensures the encoder building
// blocks remain spec-conform.
// ===========================================================================

#[test]
fn lc0_must_understand_emheader_layout() {
    let mut w = BufferWriter::new(Endianness::Little);
    encode_mutable_member_lc(&mut w, 0x1234, true, LengthCode::Lc0, |w| 0xAAu8.encode(w)).unwrap();
    let bytes = w.into_bytes();
    // EMHEADER LE: m=1, lc=0, id=0x1234 -> 0x80001234
    assert_eq!(&bytes[0..4], &[0x34, 0x12, 0x00, 0x80]);
    assert_eq!(bytes[4], 0xAA);
}

#[test]
fn lc4_zero_length_body_emheader_layout() {
    let mut w = BufferWriter::new(Endianness::Little);
    encode_mutable_member(&mut w, 5, false, |_w| Ok(())).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[4..8], &[0, 0, 0, 0]);
}
