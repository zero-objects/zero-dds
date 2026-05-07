#![no_main]
//! Fuzz-Target: AMQP-Frame-Header-Dekoder.
//!
//! Spec OASIS amqp-1.0-transport §2.3 — 8-Byte-Header + variable
//! Extended-Header. Coverage-guided Panic-/UB-Jagd auf alle
//! Length-/Offset-Validierungs-Pfade.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::frame::decode_frame_header(data);
});
