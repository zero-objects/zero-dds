#![no_main]
//! Fuzz target: AMQP frame-header decoder.
//!
//! Spec OASIS amqp-1.0-transport §2.3 — 8-byte header + variable
//! extended header. Coverage-guided panic/UB hunt over all
//! length/offset validation paths.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::frame::decode_frame_header(data);
});
