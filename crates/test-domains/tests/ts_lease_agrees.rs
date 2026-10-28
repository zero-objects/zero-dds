// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript tests of `crates/ts-node` lease their domains with
//! `crates/ts-node/test/support/test-domains.ts`, which repeats the protocol
//! of this crate: the same lease files, the same band. A lease that read
//! another band, or wrote another file name, would hand a Node test a domain
//! a Rust test holds. This test holds the TypeScript copy to the constants
//! here, as `java_lease_agrees.rs` holds the Java one.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

fn typescript() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ts-node/test/support/test-domains.ts");
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

/// The value of `const NAME = <n>;` in the TypeScript source.
fn constant(text: &str, name: &str) -> u32 {
    let marker = format!("const {name} = ");
    let at = text
        .find(&marker)
        .unwrap_or_else(|| panic!("`{name}` is not declared"));
    let rest = &text[at + marker.len()..];
    rest[..rest.find(';').expect("a `;`")]
        .trim()
        .parse()
        .unwrap_or_else(|why| panic!("`{name}`: {why}"))
}

#[test]
fn the_typescript_band_is_the_band_of_this_crate() {
    let text = typescript();
    let marker = "RESERVED_DOMAINS: readonly number[] = [";
    let start = text.find(marker).expect("RESERVED_DOMAINS");
    let body = &text[start + marker.len()..];
    let body = &body[..body.find("];").expect("the end of the array")];
    let reserved: Vec<i32> = body
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().expect("a domain"))
        .collect();
    assert_eq!(reserved, zerodds_test_domains::RESERVED_DOMAINS);
    assert_eq!(
        i32::try_from(constant(&text, "MAX_DOMAIN")).unwrap(),
        zerodds_test_domains::MAX_DOMAIN
    );
    let base = constant(&text, "PORT_BASE");
    let gain = constant(&text, "DOMAIN_GAIN");
    let spdp = constant(&text, "OFFSET_SPDP_UNICAST");
    let participant = constant(&text, "PARTICIPANT_GAIN");
    let index = constant(&text, "MAX_PARTICIPANT_INDEX");
    for domain in [0_i32, 90, 92, 215, 231] {
        let low = base + gain * domain.unsigned_abs();
        assert_eq!(
            (low, low + spdp + participant * index),
            zerodds_test_domains::domain_port_window(domain),
            "the port window of domain {domain}"
        );
    }
}

#[test]
fn the_typescript_lease_writes_the_files_of_this_crate() {
    let text = typescript();
    for needle in [
        "\"zerodds-dcps-domains\"",
        "\"domain-\" + domain + \".lease\"",
        "\"takeover.lock\"",
        "\"pid \" + process.pid + \" since \"",
        "process.env.TMPDIR",
    ] {
        assert!(text.contains(needle), "test-domains.ts lacks {needle}");
    }
}
