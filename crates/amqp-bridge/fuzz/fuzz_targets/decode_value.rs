#![no_main]
//! Fuzz target: top-level AMQP type decoder (`decode_value`).
//!
//! Spec OASIS amqp-1.0-types §1.3-§1.6 — all primitive,
//! variable-width and compound type paths. Triggers in particular:
//! * list8/list32 + map8/map32 length-prefix validation
//! * variable-width bound checks (str8/str32, vbin8/vbin32)
//! * recursive descent in compound values (DoS depth)
//! * UTF-8 validation in str/symbol

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::types::decode_value(data);
});
