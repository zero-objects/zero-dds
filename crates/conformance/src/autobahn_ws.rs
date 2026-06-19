// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Autobahn|TestSuite WebSocket conformance subset — RFC 6455.
//!
//! We implement the spec test vectors from RFC 6455 directly
//! ourselves — the external Autobahn suite (Python) can additionally
//! run in the `live-interop` job, but is not gating.

use crate::{CaseResult, TestCase};

use zerodds_websocket_bridge::{
    CloseCode, ClosePayload, NegotiationError, PermessageDeflateParams, append_tail,
    compute_accept, decode_close_payload, encode_close_payload, parse_offer, render_accept,
    strip_tail,
};

// ============================================================================
// Section 1 — RFC 6455 §1.3 Handshake
// ============================================================================

/// Spec §1.3 — Sec-WebSocket-Accept test vector (sample nonce).
fn case_1_1_accept_sample_nonce() -> CaseResult {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    if compute_accept(key) == expected {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§1.3 sample nonce accept mismatch".into())
    }
}

/// Spec §4.2.2 Step 5 — Accept is exactly case-sensitive.
fn case_1_2_accept_case_sensitive() -> CaseResult {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    if compute_accept(key) == expected.to_uppercase() {
        CaseResult::Fail("accept must NOT be uppercased".into())
    } else {
        CaseResult::Pass
    }
}

// ============================================================================
// Section 2 — RFC 6455 §7.4 Close-Codes
// ============================================================================

/// Spec §7.4.1 — close code 1000 (Normal Closure) round-trip.
fn case_2_1_close_normal_round_trip() -> CaseResult {
    let p = ClosePayload {
        code: CloseCode::Normal,
        reason: "bye".into(),
    };
    let bytes = encode_close_payload(&p);
    if bytes[0..2] != [0x03, 0xe8] {
        return CaseResult::Fail("§7.4.1 code 1000 should encode as 03 e8 BE".into());
    }
    match decode_close_payload(&bytes) {
        Ok(back) if back == p => CaseResult::Pass,
        Ok(_) => CaseResult::Fail("close payload round-trip mismatch".into()),
        Err(()) => CaseResult::Fail("close payload decode error".into()),
    }
}

/// Spec §7.4.2 — reserved codes (1005, 1006, 1015) MUST be rejected on the wire.
fn case_2_2_reserved_codes_rejected() -> CaseResult {
    for code in [1005u16, 1006, 1015] {
        let buf = code.to_be_bytes();
        if decode_close_payload(&buf).is_ok() {
            return CaseResult::Fail(alloc::format!(
                "§7.4.2: reserved code {code} must be rejected"
            ));
        }
    }
    CaseResult::Pass
}

/// Spec §5.5.1 — close reason max 123 bytes.
fn case_2_3_close_reason_size_limit() -> CaseResult {
    let mut buf = alloc::vec![0x03, 0xe8];
    buf.extend(core::iter::repeat_n(b'a', 124));
    if decode_close_payload(&buf).is_ok() {
        CaseResult::Fail("§5.5.1: 124-byte reason must be rejected".into())
    } else {
        CaseResult::Pass
    }
}

// ============================================================================
// Section 3 — RFC 7692 permessage-deflate
// ============================================================================

/// RFC 7692 §7.1 — tail marker `00 00 FF FF` strip/append.
fn case_3_1_deflate_tail_round_trip() -> CaseResult {
    let raw = b"hello";
    let with_tail = append_tail(raw);
    if with_tail != b"hello\x00\x00\xff\xff" {
        return CaseResult::Fail("§7.1 tail-marker append".into());
    }
    if strip_tail(&with_tail) != raw {
        return CaseResult::Fail("§7.1 tail-marker strip".into());
    }
    CaseResult::Pass
}

/// RFC 7692 §7.2 — negotiation with all 4 parameters.
fn case_3_2_deflate_full_negotiation() -> CaseResult {
    let offer = "permessage-deflate; server_no_context_takeover; \
                 client_no_context_takeover; \
                 server_max_window_bits=12; client_max_window_bits=10";
    let p = match parse_offer(offer) {
        Ok(p) => p,
        Err(e) => return CaseResult::Fail(alloc::format!("offer parse: {e}")),
    };
    if !p.server_no_takeover
        || !p.client_no_takeover
        || p.server_max_window_bits != 12
        || p.client_max_window_bits != 10
    {
        return CaseResult::Fail("§7.2 parameter parsing".into());
    }
    let rendered = render_accept(&p);
    if !rendered.contains("server_no_context_takeover") {
        return CaseResult::Fail("§7.2 render takeover flag".into());
    }
    CaseResult::Pass
}

/// RFC 7692 §7.1 — window bits outside 8..=15 must be rejected.
fn case_3_3_deflate_invalid_window_rejected() -> CaseResult {
    for v in [7, 16, 0, 255] {
        let offer = alloc::format!("permessage-deflate; server_max_window_bits={v}");
        if parse_offer(&offer).is_ok() {
            return CaseResult::Fail(alloc::format!("§7.1: window_bits={v} must be rejected"));
        }
    }
    // Boolean param with a value must be rejected.
    if !matches!(
        parse_offer("permessage-deflate; server_no_context_takeover=yes"),
        Err(NegotiationError::BooleanWithValue(_))
    ) {
        return CaseResult::Fail("§7.1: boolean param with value must reject".into());
    }
    CaseResult::Pass
}

/// Spec §1.3 GUID constant is spec-verbatim.
fn case_4_1_guid_verbatim() -> CaseResult {
    use zerodds_websocket_bridge::handshake::WEBSOCKET_GUID;
    if WEBSOCKET_GUID == "258EAFA5-E914-47DA-95CA-C5AB0DC85B11" {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§1.3 GUID constant".into())
    }
}

/// Default profile = "permessage-deflate" raw (all 4 fields are spec defaults).
fn case_4_2_default_profile_renders_bare() -> CaseResult {
    let s = render_accept(&PermessageDeflateParams::default());
    if s == "permessage-deflate" {
        CaseResult::Pass
    } else {
        CaseResult::Fail("default render".into())
    }
}

/// Complete test suite.
pub const SUITE: &[TestCase] = &[
    TestCase {
        name: "rfc6455-1.3-accept-sample-nonce",
        run: case_1_1_accept_sample_nonce,
    },
    TestCase {
        name: "rfc6455-4.2.2-accept-case-sensitive",
        run: case_1_2_accept_case_sensitive,
    },
    TestCase {
        name: "rfc6455-7.4.1-close-normal-roundtrip",
        run: case_2_1_close_normal_round_trip,
    },
    TestCase {
        name: "rfc6455-7.4.2-reserved-codes-rejected",
        run: case_2_2_reserved_codes_rejected,
    },
    TestCase {
        name: "rfc6455-5.5.1-close-reason-size-limit",
        run: case_2_3_close_reason_size_limit,
    },
    TestCase {
        name: "rfc7692-7.1-deflate-tail-roundtrip",
        run: case_3_1_deflate_tail_round_trip,
    },
    TestCase {
        name: "rfc7692-7.2-deflate-full-negotiation",
        run: case_3_2_deflate_full_negotiation,
    },
    TestCase {
        name: "rfc7692-7.1-deflate-invalid-window-rejected",
        run: case_3_3_deflate_invalid_window_rejected,
    },
    TestCase {
        name: "rfc6455-1.3-guid-verbatim",
        run: case_4_1_guid_verbatim,
    },
    TestCase {
        name: "rfc7692-default-renders-bare",
        run: case_4_2_default_profile_renders_bare,
    },
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn full_suite_passes() {
        let (p, s, f) = crate::run_suite(SUITE);
        assert_eq!(f, 0, "no Autobahn cases must fail");
        assert_eq!(p + s, SUITE.len());
        assert!(p >= 9, "at least 9 cases pass (got {p})");
    }
}
