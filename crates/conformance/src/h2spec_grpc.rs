// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! h2spec + gRPC-interop Conformance — RFC 7540 + RFC 7541 + gRPC.

use crate::{CaseResult, TestCase};

use zerodds_hpack::{Decoder, Encoder, HeaderField};
use zerodds_http2::frame::DEFAULT_MAX_FRAME_SIZE;
use zerodds_http2::stream::StreamEvent;
use zerodds_http2::{
    ErrorCode, Flags, FrameHeader, FrameType, StreamState, decode_frame, encode_frame,
};

// ============================================================================
// HTTP/2 — RFC 7540
// ============================================================================

fn case_1_1_frame_header_round_trip() -> CaseResult {
    let h = FrameHeader {
        length: 5,
        frame_type: FrameType::Data,
        flags: Flags(Flags::END_STREAM),
        stream_id: 7,
    };
    let payload = alloc::vec![1u8, 2, 3, 4, 5];
    let mut buf = alloc::vec![0u8; 32];
    let n = match encode_frame(&h, &payload, &mut buf, DEFAULT_MAX_FRAME_SIZE) {
        Ok(n) => n,
        Err(e) => return CaseResult::Fail(alloc::format!("encode: {e}")),
    };
    match decode_frame(&buf[..n], DEFAULT_MAX_FRAME_SIZE) {
        Ok((f, _)) if f.header == h && f.payload == payload => CaseResult::Pass,
        Ok(_) => CaseResult::Fail("§4.1 frame round-trip mismatch".into()),
        Err(e) => CaseResult::Fail(alloc::format!("decode: {e}")),
    }
}

fn case_1_2_r_bit_stripped() -> CaseResult {
    // R-Bit (MSB von Stream-Id) MUSS auf Decode ignoriert werden
    // (Spec §4.1).
    let mut buf = alloc::vec![0u8; 9];
    buf[3] = FrameType::Settings as u8;
    buf[5] = 0x80; // R-bit
    match decode_frame(&buf, DEFAULT_MAX_FRAME_SIZE) {
        Ok((f, _)) if f.header.stream_id == 0 => CaseResult::Pass,
        _ => CaseResult::Fail("§4.1 R-bit must be stripped".into()),
    }
}

fn case_1_3_state_idle_to_open() -> CaseResult {
    use zerodds_http2::stream::transition;
    match transition(StreamState::Idle, StreamEvent::RecvHeaders) {
        Ok(StreamState::Open) => CaseResult::Pass,
        _ => CaseResult::Fail("§5.1 idle→open via RecvHeaders".into()),
    }
}

fn case_1_4_stream_reset_terminates() -> CaseResult {
    use zerodds_http2::stream::transition;
    for s in [
        StreamState::Open,
        StreamState::HalfClosedLocal,
        StreamState::HalfClosedRemote,
    ] {
        match transition(s, StreamEvent::Reset) {
            Ok(StreamState::Closed) => {}
            _ => return CaseResult::Fail(alloc::format!("§5.1: reset from {s:?}")),
        }
    }
    CaseResult::Pass
}

fn case_1_5_error_code_table_complete() -> CaseResult {
    // Spec §7 Tab.7: 14 standard error codes (0x0..=0xd).
    for v in 0x0..=0xdu32 {
        if (ErrorCode::from_u32(v) as u32) != v {
            return CaseResult::Fail(alloc::format!("§7: error code {v:#x}"));
        }
    }
    if (ErrorCode::from_u32(0xfff0) as u32) >= 0xd {
        // Unknown should map to InternalError (= 0x2).
        if ErrorCode::from_u32(0xfff0) != ErrorCode::InternalError {
            return CaseResult::Fail("§7: unknown should map to InternalError".into());
        }
    }
    CaseResult::Pass
}

// ============================================================================
// HPACK — RFC 7541
// ============================================================================

fn case_2_1_integer_c1_test_vectors() -> CaseResult {
    // Spec §C.1.1 — value 10 in 5-bit prefix → 0x0a (one byte)
    let buf = zerodds_hpack::encode_integer(10, 5, 0);
    if buf != alloc::vec![0x0au8] {
        return CaseResult::Fail("§C.1.1".into());
    }
    // Spec §C.1.2 — value 1337 in 5-bit prefix → 0x1f, 0x9a, 0x0a
    let buf = zerodds_hpack::encode_integer(1337, 5, 0);
    if buf != alloc::vec![0x1fu8, 0x9a, 0x0a] {
        return CaseResult::Fail("§C.1.2".into());
    }
    CaseResult::Pass
}

fn case_2_2_static_table_61_entries() -> CaseResult {
    use zerodds_hpack::STATIC_TABLE;
    if STATIC_TABLE.len() == 61
        && STATIC_TABLE[0].name == ":authority"
        && STATIC_TABLE[1].name == ":method"
        && STATIC_TABLE[1].value == "GET"
        && STATIC_TABLE[60].name == "www-authenticate"
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("Appendix A static table layout".into())
    }
}

fn case_2_3_encode_decode_round_trip() -> CaseResult {
    let mut e = Encoder::new();
    let mut d = Decoder::new();
    let headers = alloc::vec![
        HeaderField {
            name: ":method".into(),
            value: "GET".into(),
        },
        HeaderField {
            name: ":scheme".into(),
            value: "https".into(),
        },
        HeaderField {
            name: "custom-key".into(),
            value: "custom-value".into(),
        },
    ];
    let buf = e.encode(&headers);
    match d.decode(&buf) {
        Ok(decoded) if decoded == headers => CaseResult::Pass,
        _ => CaseResult::Fail("HPACK encode/decode round-trip".into()),
    }
}

fn case_2_4_huffman_c4_1_www_example_com() -> CaseResult {
    // Spec §C.4.1: "www.example.com" Huffman → f1e3 c2e5 f23a 6ba0 ab90 f4ff
    let enc = zerodds_hpack::huffman::encode(b"www.example.com");
    let expected = [
        0xf1u8, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab, 0x90, 0xf4, 0xff,
    ];
    if enc == expected {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§C.4.1 huffman test vector".into())
    }
}

// ============================================================================
// gRPC — protocol-http2.md + protocol-web.md
// ============================================================================

fn case_3_1_lpm_round_trip() -> CaseResult {
    use zerodds_grpc_bridge::{decode_message, encode_message};
    let payload = b"hello world";
    let lpm = match encode_message(payload, false) {
        Ok(b) => b,
        Err(e) => return CaseResult::Fail(alloc::format!("LPM encode: {e:?}")),
    };
    match decode_message(&lpm) {
        Ok((_compressed, msg, _consumed)) if msg == payload => CaseResult::Pass,
        _ => CaseResult::Fail("LPM round-trip".into()),
    }
}

fn case_3_2_path_parse() -> CaseResult {
    use zerodds_grpc_bridge::parse_path;
    match parse_path("/dds.demo.Trader/PlaceOrder") {
        Ok((s, m)) if s == "dds.demo.Trader" && m == "PlaceOrder" => CaseResult::Pass,
        _ => CaseResult::Fail("path parse `/<service>/<method>`".into()),
    }
}

/// Komplette h2spec+gRPC Test-Suite.
pub const SUITE: &[TestCase] = &[
    TestCase {
        name: "rfc7540-4.1-frame-roundtrip",
        run: case_1_1_frame_header_round_trip,
    },
    TestCase {
        name: "rfc7540-4.1-r-bit-stripped",
        run: case_1_2_r_bit_stripped,
    },
    TestCase {
        name: "rfc7540-5.1-idle-to-open",
        run: case_1_3_state_idle_to_open,
    },
    TestCase {
        name: "rfc7540-5.1-stream-reset",
        run: case_1_4_stream_reset_terminates,
    },
    TestCase {
        name: "rfc7540-7-error-codes",
        run: case_1_5_error_code_table_complete,
    },
    TestCase {
        name: "rfc7541-c1-integer-vectors",
        run: case_2_1_integer_c1_test_vectors,
    },
    TestCase {
        name: "rfc7541-appendix-a-static-table",
        run: case_2_2_static_table_61_entries,
    },
    TestCase {
        name: "rfc7541-encode-decode-roundtrip",
        run: case_2_3_encode_decode_round_trip,
    },
    TestCase {
        name: "rfc7541-c4.1-huffman-www-example",
        run: case_2_4_huffman_c4_1_www_example_com,
    },
    TestCase {
        name: "grpc-lpm-roundtrip",
        run: case_3_1_lpm_round_trip,
    },
    TestCase {
        name: "grpc-path-parse",
        run: case_3_2_path_parse,
    },
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn full_suite_passes() {
        let (p, s, f) = crate::run_suite(SUITE);
        assert_eq!(f, 0, "no h2spec/gRPC cases must fail");
        assert_eq!(p + s, SUITE.len());
        assert!(p >= 10);
    }
}
