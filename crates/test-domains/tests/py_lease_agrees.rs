// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OMG suite jobs of the interop pillar lease their domains with
//! `tests/interop/omg/test_domains.py`, which repeats the protocol of this
//! crate: the same lease files, the same band, the same heartbeat. A lease
//! that read another band, or wrote another file name, would hand a suite job
//! a domain a Rust test on the same runner host holds. This test holds the
//! Python copy to the constants here, as `ts_lease_agrees.rs` holds the
//! TypeScript one.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

fn python() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop/omg/test_domains.py");
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

/// The value of a line `NAME = <n>` in the Python source.
fn constant(text: &str, name: &str) -> u64 {
    let marker = format!("\n{name} = ");
    let at = text
        .find(&marker)
        .unwrap_or_else(|| panic!("`{name}` is not assigned"));
    let rest = &text[at + marker.len()..];
    rest[..rest.find('\n').expect("the end of the line")]
        .trim()
        .replace('_', "")
        .parse()
        .unwrap_or_else(|why| panic!("`{name}`: {why}"))
}

#[test]
fn the_python_band_is_the_band_of_this_crate() {
    let text = python();
    let marker = "RESERVED_DOMAINS = [";
    let start = text.find(marker).expect("RESERVED_DOMAINS");
    let body = &text[start + marker.len()..];
    let body = &body[..body.find(']').expect("the end of the list")];
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
    let base = u32::try_from(constant(&text, "PORT_BASE")).unwrap();
    let gain = u32::try_from(constant(&text, "DOMAIN_GAIN")).unwrap();
    let spdp = u32::try_from(constant(&text, "OFFSET_SPDP_UNICAST")).unwrap();
    let participant = u32::try_from(constant(&text, "PARTICIPANT_GAIN")).unwrap();
    let index = u32::try_from(constant(&text, "MAX_PARTICIPANT_INDEX")).unwrap();
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
fn the_python_lease_writes_the_files_of_this_crate() {
    let text = python();
    for needle in [
        "\"zerodds-dcps-domains\"",
        "\"domain-\" + str(domain) + \".lease\"",
        "\"takeover.lock\"",
        "\"pid \" + str(os.getpid()) + \" since \"",
        "os.environ.get(\"TMPDIR\")",
        "LEASE_DIR_VARIABLE = \"ZERODDS_TEST_DOMAIN_DIR\"",
        "\" ns \" + ns",
        "\"/proc/self/ns/pid\"",
        "os.utime(file(domain))",
        "os.O_CREAT | os.O_EXCL",
    ] {
        assert!(text.contains(needle), "test_domains.py lacks {needle}");
    }
}

#[test]
fn the_python_heartbeat_has_the_timing_of_this_crate() {
    let text = python();
    assert_eq!(
        constant(&text, "HEARTBEAT_MS"),
        zerodds_test_domains::HEARTBEAT_MILLIS
    );
    assert_eq!(
        constant(&text, "HEARTBEAT_STALE_MS"),
        zerodds_test_domains::HEARTBEAT_STALE_MILLIS
    );
}

#[test]
fn the_python_share_is_the_share_of_this_crate() {
    let text = python();
    let marker = "FOREIGN_DOMAINS = [";
    let start = text.find(marker).expect("FOREIGN_DOMAINS");
    let body = &text[start + marker.len()..];
    let foreign: Vec<i32> = body[..body.find(']').expect("the end of the list")]
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().expect("a domain"))
        .collect();
    assert_eq!(foreign, zerodds_test_domains::FOREIGN_DOMAINS);
    assert_eq!(
        i32::try_from(constant(&text, "CI_SHARE_LOW")).unwrap(),
        zerodds_test_domains::CI_SHARE_LOW
    );
    assert_eq!(
        i32::try_from(constant(&text, "CI_SHARE_HIGH")).unwrap(),
        zerodds_test_domains::CI_SHARE_HIGH
    );
    assert!(text.contains(&format!(
        "BAND_VARIABLE = \"{}\"",
        zerodds_test_domains::BAND_VARIABLE
    )));
}
