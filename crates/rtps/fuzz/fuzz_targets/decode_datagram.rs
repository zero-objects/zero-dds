#![no_main]
//! Fuzz target: the top-level `decode_datagram`, hunting panics under
//! coverage guidance.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_rtps::datagram::decode_datagram(data);
});
