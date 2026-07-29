// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Emits XCDR2 golden byte vectors for the endpoint SDK byte-identity tests
//! (ADR 0013). Two vectors:
//!   * golden_{le,be}.bin        -- a @final primitives+string+seq<octet> type
//!   * golden_nested_{le,be}.bin -- @appendable Outer{ id, Inner one,
//!     sequence<Inner> many, string } exercising DHEADER + nesting +
//!     sequence<non-primitive>.
//!
//! The field sequences MUST match endpoints/*/test/*.
//!
//! usage: zerodds-endpoint-golden <out-dir>

#![allow(clippy::print_stderr, clippy::expect_used)]

use std::path::Path;

use zerodds_cdr::struct_enc::{MutableStructEncoder, encode_appendable};
use zerodds_cdr::{BufferWriter, EncodeError, Endianness};

/// @final SensorReading — identical to `fill_sample` in the C test.
fn encode_final(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    w.write_u32(0xA1B2_C3D4).expect("id");
    w.write_u16(0x1234).expect("kind");
    w.write_u8(0x5A).expect("flags");
    w.write_u32(3.5f32.to_bits()).expect("value");
    w.write_u64(0x0102_0304_0506_0708).expect("stamp");
    w.write_string("bay-12").expect("label");
    w.write_u32(4).expect("raw len");
    w.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF]).expect("raw");
    w.into_bytes()
}

/// @appendable Inner { uint16 a; uint32 b; } -- DHEADER + tight body.
fn encode_inner(w: &mut BufferWriter, a: u16, b: u32) -> Result<(), EncodeError> {
    encode_appendable(w, |ib| {
        ib.write_u16(a)?;
        ib.write_u32(b)?;
        Ok(())
    })
}

/// @appendable Outer { uint32 id; Inner one; sequence<Inner> many; string s; }.
fn encode_nested(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    encode_appendable(&mut w, |b| {
        b.write_u32(0xCAFE_BABE)?;
        encode_inner(b, 0x1111, 0x2222_3333)?;
        // sequence<Inner>: non-primitive element -> collection DHEADER wrapping
        // (uint32 count + each element). Mirrors composite::write_with_dheader.
        let mut sub = BufferWriter::new(b.endianness()).with_max_alignment(b.max_alignment());
        sub.write_u32(2)?;
        encode_inner(&mut sub, 0xAAAA, 0xBBBB_CCCC)?;
        encode_inner(&mut sub, 0xDDDD, 0xEEEE_FFFF)?;
        let body = sub.into_bytes();
        b.write_u32(body.len() as u32)?;
        b.write_bytes(&body)?;
        b.write_string("nested")?;
        Ok(())
    })
    .expect("outer");
    w.into_bytes()
}

/// typedef transparency: `typedef unsigned long Id; typedef Id AliasId;
/// typedef string Label; typedef sequence<octet> Blob;
/// @final struct Rec { AliasId id; Label name; Blob data; };`
/// A typedef is a wire-transparent alias, so this equals the raw-type struct.
fn encode_typedef(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    w.write_u32(0xCAFE_BABE).expect("id");
    w.write_string("typedef").expect("name");
    w.write_u32(3).expect("blob len");
    w.write_bytes(&[0x01, 0x02, 0x03]).expect("blob");
    w.into_bytes()
}

/// Fixed arrays (XCDR2 §7.4.3.5.3): primitive-element arrays are the elements
/// inline, row-major, no DHEADER (a @final struct adds none either).
/// `@final struct Arr { long xs[3]; short m[2][2]; octet bs[4]; };`
fn encode_array(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    for v in [0x1111_1111_u32, 0x2222_2222, 0x3333_3333] {
        w.write_u32(v).expect("xs");
    }
    // short m[2][2] row-major: {{0x0102,0x0304},{0x0506,0x0708}}.
    for v in [0x0102_u16, 0x0304, 0x0506, 0x0708] {
        w.write_u16(v).expect("m");
    }
    for v in [0xAA_u8, 0xBB, 0xCC, 0xDD] {
        w.write_u8(v).expect("bs");
    }
    w.into_bytes()
}

/// Union (XCDR2 §7.4.3.5.4): discriminator inline, then the selected member.
/// `@final union U switch(long d) { case 1: unsigned long a; case 2: unsigned
/// short b; default: octet c; };` encoded with d=2 → member `b` (proves the
/// switch skips case 1 and selects a non-first case; no default).
fn encode_union(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    w.write_u32(2).expect("disc"); // i32 discriminator = 2
    w.write_u16(0x1234).expect("b");
    w.into_bytes()
}

/// Map (XCDR2 §7.4.3.5): `@final struct HasMap { map<long, unsigned long> m; };`
/// with m = {1: 0x11111111, 2: 0x22222222}. A primitive key/value pair carries
/// NO collection DHEADER — just `u32 count` + entries sorted ascending by key.
fn encode_map(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    w.write_u32(2).expect("count");
    w.write_u32(1).expect("k1"); // key long = 1
    w.write_u32(0x1111_1111).expect("v1");
    w.write_u32(2).expect("k2");
    w.write_u32(0x2222_2222).expect("v2");
    w.into_bytes()
}

/// Wide chars/strings (XCDR2): `@final struct W { wchar c; wstring s; };` with
/// c = U+03A9 (Ω) and s = "wπ". `wchar` is a wchar32 (UTF-32 code point, 4
/// bytes); `wstring` is a u32 octet-length (2·units, no BOM) + UTF-16 units,
/// matching `zerodds_cdr::WString`.
fn encode_wide(endian: Endianness) -> Vec<u8> {
    use zerodds_cdr::WString;
    let mut w = BufferWriter::new(endian).xcdr2();
    w.write_u32(0x03A9).expect("wchar"); // wchar32 for 'Ω'
    WString::from("wπ")
        .encode_with_bom(&mut w, false)
        .expect("wstring");
    w.into_bytes()
}

/// Widens an IEEE-754 `binary64` to `binary128` (sign + 15-bit exponent + 112-bit
/// mantissa). Exact for every finite `f64`. Returns `(hi, lo)` of the 128-bit value.
fn f64_to_binary128(v: f64) -> (u64, u64) {
    let bits = v.to_bits();
    let sign = bits >> 63;
    let exp = (bits >> 52) & 0x7FF;
    let mant = bits & 0xF_FFFF_FFFF_FFFF;
    if exp == 0 && mant == 0 {
        return (sign << 63, 0);
    }
    let hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4);
    let lo = (mant & 0xF) << 60;
    (hi, lo)
}

/// `@final struct L { long double d; };` with d = 1.1. `long double` is the IEEE
/// binary128 (16 bytes) widened from the f64 value — no native f128 needed.
fn encode_longdouble(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    let (hi, lo) = f64_to_binary128(1.1);
    let mut le = [0u8; 16];
    for i in 0..8 {
        le[i] = (lo >> (8 * i)) as u8;
        le[8 + i] = (hi >> (8 * i)) as u8;
    }
    if endian == Endianness::Big {
        le.reverse();
    }
    w.write_bytes(&le).expect("long double");
    w.into_bytes()
}

/// KeyHash (XTypes §7.6.8): `@final struct K { @key long a; @key unsigned short
/// b; long c; };` with a=0x01020304, b=0x0506. The @key members are serialized
/// PLAIN_CDR2-BE (a then b, member-id order); the max size (6) ≤ 16, so the
/// KeyHash is those bytes zero-padded to 16. No endian variants (always BE).
fn encode_keyhash() -> Vec<u8> {
    use zerodds_cdr::compute_key_hash;
    let mut w = BufferWriter::new(Endianness::Big).xcdr2();
    w.write_u32(0x0102_0304).expect("a");
    w.write_u16(0x0506).expect("b");
    compute_key_hash(&w.into_bytes(), 6).to_vec()
}

/// `@final struct KL { @key long a; @key long b; @key long c; @key long d;
/// @key long e; };` with a=0x01020304 … e=0x11121314. The five @key longs
/// serialize to 20 PLAIN_CDR2-BE bytes; max size (20) > 16, so the KeyHash is
/// MD5(bytes)[0..16] (XTypes 1.3 §7.6.8.4 step 5.2). No endian variants (BE).
fn encode_keyhash_md5() -> Vec<u8> {
    use zerodds_cdr::compute_key_hash;
    let mut w = BufferWriter::new(Endianness::Big).xcdr2();
    for v in [
        0x0102_0304u32,
        0x0506_0708,
        0x090A_0B0C,
        0x0D0E_0F10,
        0x1112_1314,
    ] {
        w.write_u32(v).expect("kl");
    }
    compute_key_hash(&w.into_bytes(), 20).to_vec()
}

/// @mutable M { @id(10) uint32 x; @id(20) string s; @id(30) uint16 k; }.
/// DHEADER-delimited, each member EMHEADER (LC4) + NEXTINT + body.
fn encode_mutable(endian: Endianness) -> Vec<u8> {
    let mut w = BufferWriter::new(endian).xcdr2();
    encode_appendable(&mut w, |b| {
        let mut enc = MutableStructEncoder::new(b, vec![10, 20, 30]);
        enc.encode_member(10, false, |mb| mb.write_u32(0xDEAD_BEEF))?;
        enc.encode_member(20, false, |mb| mb.write_string("mut"))?;
        enc.encode_member(30, false, |mb| mb.write_u16(0x0777))?;
        enc.finish()
    })
    .expect("mutable");
    w.into_bytes()
}

/// A full XRCE Message: header (session/stream/seq, LE) + one WRITE_DATA
/// submessage carrying the SensorReading XCDR body (DataFormat::Sample). Built
/// with the real `zerodds-xrce`, so the C endpoint framing is proven against
/// what a `crates/xrce` agent accepts.
fn encode_xrce_frame() -> Vec<u8> {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::submessages::{DataFormat, Message, WriteDataPayload};

    let sample = encode_final(Endianness::Little);
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16(1),
    )
    .expect("header");
    let sm = WriteDataPayload {
        representation: sample,
        data_format: DataFormat::Sample,
    }
    .into_submessage()
    .expect("submessage");
    Message::new(header, std::vec![sm])
        .expect("message")
        .encode()
        .expect("encode")
}

/// A DATA message (agent -> client, submessage id=9) -- the receive-path
/// counterpart the endpoint decodes.
fn encode_data_frame() -> Vec<u8> {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::submessages::{DataFormat, DataPayload, Message};

    let sample = encode_final(Endianness::Little);
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16(1),
    )
    .expect("header");
    let sm = DataPayload {
        representation: sample,
        data_format: DataFormat::Sample,
    }
    .into_submessage()
    .expect("submessage");
    Message::new(header, std::vec![sm])
        .expect("message")
        .encode()
        .expect("encode")
}

/// HEARTBEAT (agent -> client) on a reliable stream: first=1, last=3.
fn encode_heartbeat_frame() -> Vec<u8> {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::submessages::{HeartbeatPayload, Message};
    let header =
        MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16(1))
            .expect("header");
    let sm = HeartbeatPayload {
        first_unacked_seq_nr: 1,
        last_unacked_seq_nr: 3,
        stream_id: 0x80,
    }
    .into_submessage()
    .expect("submessage");
    Message::new(header, std::vec![sm])
        .expect("m")
        .encode()
        .expect("e")
}

/// ACKNACK (client -> agent): first_unacked=1, bitmap all-received.
fn encode_acknack_frame() -> Vec<u8> {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::submessages::{AckNackPayload, Message};
    let header =
        MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16(1))
            .expect("header");
    let sm = AckNackPayload {
        first_unacked_seq_num: 1,
        nack_bitmap: [0x00, 0x00],
        stream_id: 0x80,
    }
    .into_submessage()
    .expect("submessage");
    Message::new(header, std::vec![sm])
        .expect("m")
        .encode()
        .expect("e")
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let dir = Path::new(&out);
    let xrce = encode_xrce_frame();
    std::fs::write(dir.join("golden_xrce_le.bin"), &xrce).expect("write xrce");
    std::fs::write(dir.join("golden_data_le.bin"), encode_data_frame()).expect("write data");
    std::fs::write(
        dir.join("golden_heartbeat_le.bin"),
        encode_heartbeat_frame(),
    )
    .expect("write hb");
    std::fs::write(dir.join("golden_acknack_le.bin"), encode_acknack_frame()).expect("write an");
    // The same XRCE message HDLC-framed for a serial link (Annex C).
    std::fs::write(
        dir.join("golden_serial_le.bin"),
        zerodds_xrce::transport_serial::encode_payload(&xrce),
    )
    .expect("write serial");
    for (name, endian) in [("le", Endianness::Little), ("be", Endianness::Big)] {
        std::fs::write(dir.join(format!("golden_{name}.bin")), encode_final(endian))
            .expect("write final");
        std::fs::write(
            dir.join(format!("golden_nested_{name}.bin")),
            encode_nested(endian),
        )
        .expect("write nested");
        std::fs::write(
            dir.join(format!("golden_mutable_{name}.bin")),
            encode_mutable(endian),
        )
        .expect("write mutable");
        std::fs::write(
            dir.join(format!("golden_typedef_{name}.bin")),
            encode_typedef(endian),
        )
        .expect("write typedef");
        std::fs::write(
            dir.join(format!("golden_array_{name}.bin")),
            encode_array(endian),
        )
        .expect("write array");
        std::fs::write(
            dir.join(format!("golden_union_{name}.bin")),
            encode_union(endian),
        )
        .expect("write union");
        std::fs::write(
            dir.join(format!("golden_map_{name}.bin")),
            encode_map(endian),
        )
        .expect("write map");
        std::fs::write(
            dir.join(format!("golden_wide_{name}.bin")),
            encode_wide(endian),
        )
        .expect("write wide");
        std::fs::write(
            dir.join(format!("golden_longdouble_{name}.bin")),
            encode_longdouble(endian),
        )
        .expect("write longdouble");
    }
    // KeyHash has no endian variant (always PLAIN_CDR2-BE, 16-byte array).
    std::fs::write(dir.join("golden_keyhash.bin"), encode_keyhash()).expect("write keyhash");
    std::fs::write(dir.join("golden_keyhash_md5.bin"), encode_keyhash_md5())
        .expect("write keyhash md5");
    eprintln!("wrote golden_…/wide_/longdouble_/keyhash.bin to {out}");
}
