// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! FU1 repro: ZeroDDS' SPDP parser against a REAL FastDDS SPDP
//! datagram (from the Linux test host, domain 205, plain, extracted
//! from the tcpdump capture on 2026-05-29). ZeroDDS as a late joiner
//! does not discover FastDDS — hypothesis: parse_datagram rejects
//! FastDDS' SPDP.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use zerodds_discovery::spdp::SpdpReader;

fn hex_to_bytes(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// A real FastDDS 3.6 SPDP multicast datagram (536 bytes,
/// EtherType UDP payload, vendor 0x010f).
const FASTDDS_SPDP_HEX: &str = "525450530203010f010fa72bbbebc90100000000090108006850196a57e882b41505b80100001000000100c7000100c2000000000100000000030000150004000203000016000400010f000000800400030601000f000400cd00000050001000010fa72bbbebc90100000000000001c107800400010000000380280021000000653830653630353335646339343432636231306537323239313038653135666600000000320018000100000024e50000000000000000000000000000c0a8b273310018000100000025e50000000000000000000000000000c0a8b273020008001400000000000000580004003ffc0f006200140010000000525450535061727469636970616e74005900cc0004000000110000005041525449434950414e545f54595045000000000700000053494d504c4500001b000000666173746464732e706879736963616c5f646174612e686f73740000210000006538306536303533356463393434326362313065373232393130386531356666000000001b000000666173746464732e706879736963616c5f646174612e75736572000005000000726f6f74000000001e000000666173746464732e706879736963616c5f646174612e70726f636573730000000600000036303334370000000100000080013800010000001ae50000000000000000000000000000efff00016850196a72ca8cb4020000000000000030040000000000000000000000000000";

#[test]
fn zerodds_parses_real_fastdds_spdp() {
    let bytes = hex_to_bytes(FASTDDS_SPDP_HEX);
    assert_eq!(bytes.len(), 536, "capture length");
    assert_eq!(&bytes[..4], b"RTPS", "RTPS-Magic");

    let reader = SpdpReader::new();
    let res = reader.parse_datagram(&bytes);
    match &res {
        Ok(dp) => {
            eprintln!(
                "OK: ZeroDDS entdeckt FastDDS — guid={:?} vendor={:?}",
                dp.data.guid, dp.sender_vendor
            );
            eprintln!(
                "  metatraffic_unicast={:?} default_unicast={:?}",
                dp.data.metatraffic_unicast_locator, dp.data.default_unicast_locator
            );
        }
        Err(e) => eprintln!("ERR: parse_datagram rejects FastDDS SPDP: {e:?}"),
    }
    assert!(
        res.is_ok(),
        "ZeroDDS must be able to parse FastDDS' SPDP datagram (FU1)"
    );
}
