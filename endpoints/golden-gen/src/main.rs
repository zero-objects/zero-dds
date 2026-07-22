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
    }
    eprintln!("wrote golden_/nested_/mutable_{{le,be}}.bin to {out}");
}
