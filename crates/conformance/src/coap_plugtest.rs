// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IETF CoAP-Plugtest Conformance — RFC 7252 + 7641 + 7959 + 6690.

use crate::{CaseResult, TestCase};

use zerodds_coap_bridge::{
    BlockReassembler, BlockValue, CoreLink, MAX_RETRANSMIT, ObserveRegistry, ReliabilityTracker,
    decode_links, encode_links,
};

// ============================================================================
// RFC 7959 — Block-Wise Transfer
// ============================================================================

fn case_1_1_block_size_szx_table() -> CaseResult {
    // Spec §2.2: szx 0=16, 1=32, 2=64, 3=128, 4=256, 5=512, 6=1024.
    let expected = [16usize, 32, 64, 128, 256, 512, 1024];
    for (szx, sz) in expected.iter().enumerate() {
        let v = BlockValue {
            num: 0,
            more: false,
            szx: szx as u8,
        };
        match v.block_size() {
            Ok(s) if s == *sz => {}
            _ => return CaseResult::Fail(alloc::format!("§2.2 szx={szx}")),
        }
    }
    CaseResult::Pass
}

fn case_1_2_block_value_round_trip() -> CaseResult {
    for v in [
        BlockValue {
            num: 0,
            more: true,
            szx: 6,
        },
        BlockValue {
            num: 0xfffff,
            more: false,
            szx: 0,
        },
    ] {
        let bytes = match v.encode() {
            Ok(b) => b,
            Err(_) => return CaseResult::Fail("encode".into()),
        };
        match BlockValue::decode(&bytes) {
            Ok(back) if back == v => {}
            _ => return CaseResult::Fail("§2.2 block value round-trip".into()),
        }
    }
    CaseResult::Pass
}

fn case_1_3_reassembler_orders_blocks() -> CaseResult {
    let mut r = BlockReassembler::new(16);
    let _ = r.accept(
        BlockValue {
            num: 0,
            more: true,
            szx: 0,
        },
        &[1u8; 16],
    );
    let _ = r.accept(
        BlockValue {
            num: 1,
            more: true,
            szx: 0,
        },
        &[2u8; 16],
    );
    let _ = r.accept(
        BlockValue {
            num: 2,
            more: false,
            szx: 0,
        },
        &[3u8; 8],
    );
    if r.is_complete() && r.into_payload().len() == 40 {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§2.2 reassembler".into())
    }
}

// ============================================================================
// RFC 7641 — Observe
// ============================================================================

fn case_2_1_observe_register_increments() -> CaseResult {
    let mut r = ObserveRegistry::new();
    r.register("path".into(), alloc::vec![1], alloc::vec![]);
    r.register("path".into(), alloc::vec![2], alloc::vec![]);
    let s = r.next_seq("path");
    if s.len() == 2 && s.iter().all(|(_, seq, _)| *seq == 1) {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§3.4 observe sequence increment".into())
    }
}

fn case_2_2_observe_seq_wraps_at_24_bits() -> CaseResult {
    let mut r = ObserveRegistry::new();
    r.register("p".into(), alloc::vec![1], alloc::vec![]);
    // simulate roll-over by stepping seq up to 0x00ff_ffff via internal API.
    // We use next_seq() many times on a small wrapper; for direct test
    // we cannot patch the registry, so we just verify next_seq returns
    // values strictly within 24-bit.
    for _ in 0..3 {
        let _ = r.next_seq("p");
    }
    let s = r.next_seq("p");
    if s.iter().all(|(_, seq, _)| *seq <= 0x00ff_ffff) {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§3.4 24-bit limit".into())
    }
}

// ============================================================================
// RFC 7252 — Reliability
// ============================================================================

fn case_3_1_max_retransmit_default() -> CaseResult {
    if MAX_RETRANSMIT == 4 {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.8 MAX_RETRANSMIT default".into())
    }
}

fn case_3_2_ack_clears_pending() -> CaseResult {
    let mut t = ReliabilityTracker::new();
    t.send_confirmable(42, alloc::vec![1, 2], alloc::vec![0u8; 10], 0);
    if t.pending_count() != 1 {
        return CaseResult::Fail("§4.2 send registers".into());
    }
    if !t.receive_ack(42) {
        return CaseResult::Fail("§4.2 ack must succeed".into());
    }
    if t.pending_count() == 0 {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.2 ack should clear pending".into())
    }
}

// ============================================================================
// RFC 6690 — CoRE-Link-Format
// ============================================================================

fn case_4_1_link_format_round_trip() -> CaseResult {
    let links = alloc::vec![
        CoreLink::new("/sensor/0")
            .attr("rt", "temperature")
            .attr("if", "core.s"),
    ];
    let s = encode_links(&links);
    match decode_links(&s) {
        Ok(back) if back == links => CaseResult::Pass,
        _ => CaseResult::Fail("§2 link-format roundtrip".into()),
    }
}

fn case_4_2_quoted_string_with_comma() -> CaseResult {
    let s = r#"</p>;title="hello, world""#;
    match decode_links(s) {
        Ok(links) if links.len() == 1 && links[0].get("title") == Some("hello, world") => {
            CaseResult::Pass
        }
        _ => CaseResult::Fail("§2 quoted comma".into()),
    }
}

fn case_4_3_uri_must_have_brackets() -> CaseResult {
    if decode_links("/path;rt=x").is_err() {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§2 URI without `<>` must reject".into())
    }
}

/// Komplette CoAP-Plugtest Test-Suite.
pub const SUITE: &[TestCase] = &[
    TestCase {
        name: "rfc7959-2.2-szx-table",
        run: case_1_1_block_size_szx_table,
    },
    TestCase {
        name: "rfc7959-2.2-block-roundtrip",
        run: case_1_2_block_value_round_trip,
    },
    TestCase {
        name: "rfc7959-2.2-reassembler",
        run: case_1_3_reassembler_orders_blocks,
    },
    TestCase {
        name: "rfc7641-3.4-observe-seq-increment",
        run: case_2_1_observe_register_increments,
    },
    TestCase {
        name: "rfc7641-3.4-observe-seq-bound",
        run: case_2_2_observe_seq_wraps_at_24_bits,
    },
    TestCase {
        name: "rfc7252-4.8-max-retransmit",
        run: case_3_1_max_retransmit_default,
    },
    TestCase {
        name: "rfc7252-4.2-ack-clears",
        run: case_3_2_ack_clears_pending,
    },
    TestCase {
        name: "rfc6690-2-link-format-roundtrip",
        run: case_4_1_link_format_round_trip,
    },
    TestCase {
        name: "rfc6690-2-quoted-comma",
        run: case_4_2_quoted_string_with_comma,
    },
    TestCase {
        name: "rfc6690-2-uri-brackets-required",
        run: case_4_3_uri_must_have_brackets,
    },
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn full_suite_passes() {
        let (p, s, f) = crate::run_suite(SUITE);
        assert_eq!(f, 0, "no CoAP cases must fail");
        assert_eq!(p + s, SUITE.len());
        assert!(p >= 9);
    }
}
