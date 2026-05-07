#![no_main]
//! Fuzz-Target: Top-Level AMQP-Type-Dekoder (`decode_value`).
//!
//! Spec OASIS amqp-1.0-types §1.3-§1.6 — alle Primitive-,
//! Variable-Width- und Compound-Type-Pfade. Triggert insbesondere:
//! * list8/list32 + map8/map32 Length-Prefix-Validation
//! * Variable-Width-Bound-Checks (str8/str32, vbin8/vbin32)
//! * Recursive Descent in Compound-Werten (DoS-Tiefe)
//! * UTF-8-Validierung in str/symbol

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::types::decode_value(data);
});
