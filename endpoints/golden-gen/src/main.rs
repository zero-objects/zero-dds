// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Emits XCDR2 golden byte vectors for the endpoint SDK byte-identity tests
//! (ADR 0013). Two vectors:
//!   * golden_{le,be}.bin        -- a @final primitives+string+`seq<octet>` type
//!   * golden_nested_{le,be}.bin -- @appendable Outer{ id, Inner one,
//!     `sequence<Inner>` many, string } exercising DHEADER + nesting +
//!     `sequence<non-primitive>`.
//!
//! The field sequences MUST match endpoints/*/test/*.
//!
//! usage: zerodds-endpoint-golden [--write-xrce-client-fixtures] `<out-dir>`
//!
//! `--write-xrce-client-fixtures` also rewrites the committed fixtures in
//! endpoints/testdata/xrce-client/; without it nothing outside `<out-dir>` is
//! written.

#![allow(clippy::print_stderr, clippy::expect_used)]

use std::path::Path;

use zerodds_wire::{Endianness, Writer};

mod xrce_client_fixtures;

/// XCDR2 caps alignment at four (XTypes 1.3 §7.4.2.5.4). `Writer::new`
/// caps at eight, which is XCDR1, so every writer here is built with the
/// bound rather than with the default: the `uint64` of `encode_final`
/// and the `long double` of `encode_longdouble` are the members that
/// tell the two apart.
fn xcdr2(endian: Endianness) -> Writer {
    Writer::with_max_align(endian, 4)
}

/// @final SensorReading — identical to `fill_sample` in the C test.
fn encode_final(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    w.write_u32(0xA1B2_C3D4);
    w.write_u16(0x1234);
    w.write_u8(0x5A);
    w.write_u32(3.5f32.to_bits());
    w.write_u64(0x0102_0304_0506_0708);
    w.write_string("bay-12");
    w.write_u32(4);
    w.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF]);
    w.finish()
}

/// @appendable Inner { uint16 a; uint32 b; } -- DHEADER + tight body.
///
/// The DHEADER is written as a placeholder and patched once the body is
/// in, so the body aligns against the stream it actually lands in. That
/// is the same result the old sub-writer produced only because the
/// header leaves the stream four-aligned and `MAXALIGN` is four: every
/// alignment inside the body then falls on the same offset either way.
fn encode_inner(w: &mut Writer, a: u16, b: u32) {
    let dh = w.write_dheader();
    w.write_u16(a);
    w.write_u32(b);
    w.finish_dheader(dh);
}

/// @appendable Outer { uint32 id; Inner one; `sequence<Inner>` many; string s; }.
fn encode_nested(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    let outer = w.write_dheader();
    w.write_u32(0xCAFE_BABE);
    encode_inner(&mut w, 0x1111, 0x2222_3333);
    // sequence<Inner>: non-primitive element -> collection DHEADER wrapping
    // (uint32 count + each element).
    let many = w.write_dheader();
    w.write_u32(2);
    encode_inner(&mut w, 0xAAAA, 0xBBBB_CCCC);
    encode_inner(&mut w, 0xDDDD, 0xEEEE_FFFF);
    w.finish_dheader(many);
    w.write_string("nested");
    w.finish_dheader(outer);
    w.finish()
}

/// typedef transparency: `typedef unsigned long Id; typedef Id AliasId;
/// typedef string Label; typedef sequence<octet> Blob;
/// @final struct Rec { AliasId id; Label name; Blob data; };`
/// A typedef is a wire-transparent alias, so this equals the raw-type struct.
fn encode_typedef(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    w.write_u32(0xCAFE_BABE);
    w.write_string("typedef");
    w.write_u32(3);
    w.write_bytes(&[0x01, 0x02, 0x03]);
    w.finish()
}

/// Fixed arrays (XCDR2 §7.4.3.5.3): primitive-element arrays are the elements
/// inline, row-major, no DHEADER (a @final struct adds none either).
/// `@final struct Arr { long xs[3]; short m[2][2]; octet bs[4]; };`
fn encode_array(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    for v in [0x1111_1111_u32, 0x2222_2222, 0x3333_3333] {
        w.write_u32(v);
    }
    // short m[2][2] row-major: {{0x0102,0x0304},{0x0506,0x0708}}.
    for v in [0x0102_u16, 0x0304, 0x0506, 0x0708] {
        w.write_u16(v);
    }
    for v in [0xAA_u8, 0xBB, 0xCC, 0xDD] {
        w.write_u8(v);
    }
    w.finish()
}

/// Union (XCDR2 §7.4.3.5.4): discriminator inline, then the selected member.
/// `@final union U switch(long d) { case 1: unsigned long a; case 2: unsigned
/// short b; default: octet c; };` encoded with d=2 → member `b` (proves the
/// switch skips case 1 and selects a non-first case; no default).
fn encode_union(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    w.write_u32(2); // i32 discriminator = 2
    w.write_u16(0x1234);
    w.finish()
}

/// Map (XCDR2 §7.4.3.5): `@final struct HasMap { map<long, unsigned long> m; };`
/// with m = {1: 0x11111111, 2: 0x22222222}. A primitive key/value pair carries
/// NO collection DHEADER — just `u32 count` + entries sorted ascending by key.
fn encode_map(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    w.write_u32(2); // count
    w.write_u32(1); // key long = 1
    w.write_u32(0x1111_1111);
    w.write_u32(2);
    w.write_u32(0x2222_2222);
    w.finish()
}

/// Wide chars/strings (XCDR2): `@final struct W { wchar c; wstring s; };` with
/// c = U+03A9 (Ω) and s = "wπ". `wchar` is a wchar32 (UTF-32 code point, 4
/// bytes); `wstring` is a u32 octet-length (2·units, no BOM) + UTF-16 units.
///
/// `zerodds-wire` has no `wstring` writer — the width, the framing and
/// the unit order of a `wstring` are the layout plan's answers, and the
/// runtime refuses the member until it has them. This golden pins one
/// specific form (octet length, no byte-order mark, units in the
/// stream's own byte order), so it writes that form out of the
/// primitives rather than asking a writer to choose one.
fn encode_wide(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    w.write_u32(0x03A9); // wchar32 for 'Ω'
    let units: Vec<u16> = "wπ".encode_utf16().collect();
    w.write_u32(u32::try_from(units.len() * 2).expect("wstring octet length"));
    for unit in units {
        w.write_u16(unit);
    }
    w.finish()
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
    let mut w = xcdr2(endian);
    let (hi, lo) = f64_to_binary128(1.1);
    let mut le = [0u8; 16];
    for i in 0..8 {
        le[i] = (lo >> (8 * i)) as u8;
        le[8 + i] = (hi >> (8 * i)) as u8;
    }
    if endian == Endianness::Big {
        le.reverse();
    }
    w.write_bytes(&le);
    w.finish()
}

/// KeyHash (XTypes §7.6.8): `@final struct K { @key long a; @key unsigned short
/// b; long c; };` with a=0x01020304, b=0x0506. The @key members are serialized
/// PLAIN_CDR2-BE (a then b, member-id order); the max size (6) ≤ 16, so the
/// KeyHash is those bytes zero-padded to 16. No endian variants (always BE).
fn encode_keyhash() -> Vec<u8> {
    let mut w = xcdr2(Endianness::Big);
    w.write_u32(0x0102_0304);
    w.write_u16(0x0506);
    // Holder max size is 6, which is ≤ 16: step 5.1, zero-padded.
    w.zero_padded().to_vec()
}

/// `@final struct KL { @key long a; @key long b; @key long c; @key long d;
/// @key long e; };` with a=0x01020304 … e=0x11121314. The five @key longs
/// serialize to 20 PLAIN_CDR2-BE bytes; max size (20) > 16, so the KeyHash is
/// MD5(bytes)[0..16] (XTypes 1.3 §7.6.8.4 step 5.2). No endian variants (BE).
fn encode_keyhash_md5() -> Vec<u8> {
    let mut w = xcdr2(Endianness::Big);
    for v in [
        0x0102_0304u32,
        0x0506_0708,
        0x090A_0B0C,
        0x0D0E_0F10,
        0x1112_1314,
    ] {
        w.write_u32(v);
    }
    // Holder max size is 20, which is > 16: step 5.2, MD5 of the holder.
    w.md5().to_vec()
}

/// @mutable M { @id(10) uint32 x; @id(20) string s; @id(30) uint16 k; }.
/// DHEADER-delimited, each member EMHEADER (LC4) + NEXTINT + body.
fn encode_mutable(endian: Endianness) -> Vec<u8> {
    let mut w = xcdr2(endian);
    let dh = w.write_dheader();
    // LC4 for all three: the length code is the layout plan's decision
    // and this golden pins the always-NEXTINT form. `Writer` shortens an
    // LC4 to LC0..LC3 only under `with_short_length_codes`, which is off
    // — member 30's two-byte body would otherwise lose its NEXTINT.
    let m10 = w.write_emheader(10, 4, false).expect("member id 10");
    w.write_u32(0xDEAD_BEEF);
    w.finish_emheader(m10)
        .expect("the member is the length its code declares");

    let m20 = w.write_emheader(20, 4, false).expect("member id 20");
    w.write_string("mut");
    w.finish_emheader(m20)
        .expect("the member is the length its code declares");

    let m30 = w.write_emheader(30, 4, false).expect("member id 30");
    w.write_u16(0x0777);
    w.finish_emheader(m30)
        .expect("the member is the length its code declares");

    w.finish_dheader(dh);
    w.finish()
}

/// `request_id` every endpoint golden uses: the message's own `sequence_nr`
/// (DDS-XRCE 1.0 §7.7.8 only requires uniqueness per stream, and that is what
/// every endpoint SDK derives it from).
const GOLDEN_REQUEST_ID: [u8; 2] = [0x00, 0x01];
/// Raw id of the DataWriter/DataReader in the endpoint goldens.
const GOLDEN_RAW_ID: u16 = 0x001;

/// A full XRCE Message: header (session/stream/seq, LE) + one WRITE_DATA
/// submessage whose body is the `BaseObjectRequest` (§7.7.8) followed by the
/// SensorReading XCDR body. Encoded by `zerodds_xrce::WriteDataPayload` — not
/// by hand — so the goldens can only ever describe what a `crates/xrce` agent
/// actually parses.
fn encode_xrce_frame() -> Vec<u8> {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::object_id::ObjectId;
    use zerodds_xrce::object_info::BaseObjectRequest;
    use zerodds_xrce::object_kind::ObjectKind;
    use zerodds_xrce::submessages::{Message, WriteDataPayload};

    let sample = encode_final(Endianness::Little);
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16(1),
    )
    .expect("header");
    let sm = WriteDataPayload {
        base: BaseObjectRequest {
            request_id: GOLDEN_REQUEST_ID,
            object_id: ObjectId::new(GOLDEN_RAW_ID, ObjectKind::DataWriter).expect("writer id"),
        },
        serialized_data: sample,
    }
    .into_submessage()
    .expect("submessage");
    Message::new(header, std::vec![sm])
        .expect("message")
        .encode()
        .expect("encode")
}

/// A DATA message (agent -> client, submessage id=9) -- the receive-path
/// counterpart the endpoint decodes. Encoded by `zerodds_xrce::DataPayload`,
/// so its `BaseObjectRequest` names the DataReader the sample came from.
fn encode_data_frame() -> Vec<u8> {
    use zerodds_xrce::SerialNumber16;
    use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
    use zerodds_xrce::object_id::ObjectId;
    use zerodds_xrce::object_info::BaseObjectRequest;
    use zerodds_xrce::object_kind::ObjectKind;
    use zerodds_xrce::submessages::{DataPayload, Message};

    let sample = encode_final(Endianness::Little);
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16(1),
    )
    .expect("header");
    let sm = DataPayload {
        base: BaseObjectRequest {
            request_id: GOLDEN_REQUEST_ID,
            object_id: ObjectId::new(GOLDEN_RAW_ID, ObjectKind::DataReader).expect("reader id"),
        },
        serialized_data: sample,
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

/// The same ACKNACK in the FINAL form of the reference implementation (the
/// `final` setting of the SDKs): a 5-octet body, 13 octets in all.
fn encode_acknack_final_frame() -> Vec<u8> {
    zerodds_xrce::encoding::with_unannotated_types(
        zerodds_xrce::encoding::UnannotatedTypes::Final,
        encode_acknack_frame,
    )
}

/// ACKNACK (client -> agent): first_unacked=1, bitmap all-received. The
/// default form: ACKNACK_Payload has no @extensibility, so it is appendable
/// (DDS-XTypes 1.3 7.3.1.2.1.8) and carries a DHEADER: 17 octets.
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
    let mut write_xrce_client_fixtures = false;
    let mut out = None;
    for arg in std::env::args().skip(1) {
        if arg == "--write-xrce-client-fixtures" {
            write_xrce_client_fixtures = true;
        } else if out.is_none() {
            out = Some(arg);
        }
    }
    let out = out.unwrap_or_else(|| ".".to_string());
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
    std::fs::write(
        dir.join("golden_acknack_final_le.bin"),
        encode_acknack_final_frame(),
    )
    .expect("write an final");
    // The same XRCE message framed for a serial link (Annex C.1), from the
    // endpoint at address 1 to the agent at address 0.
    std::fs::write(
        dir.join("golden_serial_le.bin"),
        zerodds_xrce::transport_serial::encode_frame(1, 0, &xrce).expect("serial frame"),
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
    // The endpoint XRCE-client ports' own fixtures are committed at a fixed
    // location (endpoints/testdata/xrce-client/). A run that only wants the
    // golden files above (the endpoint tests) must not rewrite them: they are
    // written when asked for (see xrce_client_fixtures's module doc).
    if write_xrce_client_fixtures {
        xrce_client_fixtures::write_all();
    }
}
