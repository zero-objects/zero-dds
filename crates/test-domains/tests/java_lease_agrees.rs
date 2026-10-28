// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java tests lease their domains with
//! `crates/java-omgdds/java/src/test/java/org/zerodds/testing/TestDomains.java`,
//! which repeats the protocol of this crate: the same lease files, the same
//! band. A Java lease that read another band, or wrote another file name,
//! would hand a Java wire test a domain a Rust test holds. This test holds the
//! Java copy to the constants here.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

fn java() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../java-omgdds/java/src/test/java/org/zerodds/testing/TestDomains.java");
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

/// The value of `static final int NAME = <n>;` in the Java source.
fn constant(text: &str, name: &str) -> u32 {
    let marker = format!("int {name} = ");
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
fn the_java_band_is_the_band_of_this_crate() {
    let text = java();
    let start = text.find("RESERVED_DOMAINS = {").expect("RESERVED_DOMAINS");
    let body = &text[start + "RESERVED_DOMAINS = {".len()..];
    let body = &body[..body.find("};").expect("the end of the array")];
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
fn the_java_lease_writes_the_files_of_this_crate() {
    let text = java();
    for needle in [
        "\"zerodds-dcps-domains\"",
        "\"domain-\" + domain + \".lease\"",
        "\"takeover.lock\"",
        "\"pid \" + pid() + \" since \"",
        "System.getenv(\"TMPDIR\")",
    ] {
        assert!(text.contains(needle), "TestDomains.java lacks {needle}");
    }
}
