// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-conformance` with `zerodds-coap-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-coap-bridge` (`src/coap_plugtest.rs`,
//! the CoAP plugtest vectors): `BlockValue` and `BlockReassembler` (RFC 7959
//! block-wise transfer), `ObserveRegistry` (RFC 7641), `ReliabilityTracker`
//! and `MAX_RETRANSMIT` (RFC 7252 §4), `CoreLink`, `encode_links` and
//! `decode_links` (RFC 6690). It relies on the block size of an SZX, the
//! retransmission default and the link-format grammar being the RFCs'.
//!
//! Oracles: RFC 7959 §2.2 (block size 2^(SZX + 4)), RFC 7252 §4.8
//! (`MAX_RETRANSMIT` 4), RFC 6690 §2 (a link is `<URI>` and attributes; a
//! quoted string may hold a comma).
#![allow(clippy::unwrap_used)]

use zerodds_coap_bridge::{BlockValue, CoreLink, MAX_RETRANSMIT, decode_links, encode_links};

#[test]
fn rfc7959_2_2_the_szx_names_the_block_size() {
    for (szx, size) in [16usize, 32, 64, 128, 256, 512, 1024]
        .into_iter()
        .enumerate()
    {
        let value = BlockValue {
            num: 0,
            more: false,
            szx: u8::try_from(szx).unwrap(),
        };
        assert_eq!(value.block_size().unwrap(), size, "SZX {szx}");
    }
    // SZX 7 is reserved.
    assert!(
        BlockValue {
            num: 0,
            more: false,
            szx: 7
        }
        .block_size()
        .is_err()
    );
}

#[test]
fn rfc7252_4_8_max_retransmit_is_four() {
    assert_eq!(MAX_RETRANSMIT, 4);
}

#[test]
fn rfc6690_2_a_link_is_a_bracketed_uri_and_a_quoted_string_keeps_its_comma() {
    let links = decode_links(r#"</p>;title="hello, world""#).unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].get("title"), Some("hello, world"));
    assert!(
        decode_links("/path;rt=x").is_err(),
        "a URI without <> is no link"
    );
    let written = encode_links(&[CoreLink::new("/sensor/0").attr("rt", "temperature")]);
    assert!(written.starts_with("</sensor/0>"), "{written}");
    assert!(written.contains(";rt="), "{written}");
}
