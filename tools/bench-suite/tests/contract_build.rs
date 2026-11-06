// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-build`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-build`: its build script compiles
//! `idl/roundtrip.idl` (byte-equal to the IDL the Cyclone, RTI and Fast DDS
//! apps of `tests/perf/dds-roundtrip-bench` compile) with
//! `default_extensibility(DefaultExt::Final)` and `dds_type_impls(true)`,
//! and `src/bin/roundtrip_typed.rs` includes the generated module and sends
//! `RoundtripBench::Roundtrip` through its `DdsType` impl. The bench is a
//! comparison of four codecs generated from one declaration, so it relies
//! on the generated one writing the octets the other three write. The
//! contract includes the same module from `OUT_DIR`.
//!
//! Oracle: the IDL (`@final`, `@autoid(SEQUENTIAL)`, `unsigned long`,
//! `unsigned long long`, `sequence<octet, 65536>`) and DDS-XTypes 1.3
//! §7.4.3.4.2 (FINAL: the members alone, no DHEADER), §7.4.1.1.1 (under
//! XCDR2 an eight-octet value aligns to four), §7.4.3.5.4 (a sequence of a
//! primitive is its length and the elements, no DHEADER), §7.2.2.4.3 (a
//! bounded sequence holds at most its bound).
#![allow(clippy::unwrap_used, clippy::panic, dead_code, non_snake_case)]

mod generated {
    include!(concat!(env!("OUT_DIR"), "/roundtrip.rs"));
}

use generated::RoundtripBench::Roundtrip;
use zerodds_dcps::DdsType;

#[test]
fn xtypes_7_4_3_4_2_the_final_roundtrip_sample_is_its_members_at_four_octet_alignment() {
    let sample = Roundtrip {
        sequence_id: 7,
        t_send_ns: 0x0102_0304_0506_0708,
        payload: vec![0xaa, 0xbb],
    };
    let mut out = Vec::new();
    DdsType::encode(&sample, &mut out).unwrap();
    assert_eq!(
        out,
        [7, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 2, 0, 0, 0, 0xaa, 0xbb],
        "no DHEADER, the u64 at offset 4 (MAXALIGN 4), the sequence's length and its octets"
    );
    let back = <Roundtrip as DdsType>::decode(&out).unwrap();
    assert_eq!(
        (back.sequence_id, back.t_send_ns),
        (7, 0x0102_0304_0506_0708)
    );
    assert_eq!(back.payload, [0xaa, 0xbb]);
}

#[test]
fn xtypes_7_2_2_4_3_a_payload_past_its_bound_is_refused() {
    let sample = Roundtrip {
        sequence_id: 1,
        t_send_ns: 1,
        payload: vec![0; 65_537],
    };
    let mut out = Vec::new();
    assert!(
        DdsType::encode(&sample, &mut out).is_err(),
        "sequence<octet, 65536>"
    );
}

#[test]
fn the_roundtrip_type_is_final_keyless_and_named_after_its_module() {
    assert_eq!(
        <Roundtrip as DdsType>::TYPE_NAME,
        "RoundtripBench::Roundtrip"
    );
    assert_eq!(
        <Roundtrip as DdsType>::EXTENSIBILITY,
        zerodds_dcps::dds_type::Extensibility::Final
    );
    assert!(!<Roundtrip as DdsType>::has_key());
}
