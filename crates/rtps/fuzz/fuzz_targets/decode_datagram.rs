#![no_main]
//! Fuzz-Target: top-level `decode_datagram`. Coverage-guided Panic-Jagd.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_rtps::datagram::decode_datagram(data);
});
