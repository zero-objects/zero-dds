#![no_main]
//! Fuzz-Target: `zerodds_idl::parse` — IDL-Parser.

use zerodds_idl::config::ParserConfig;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let _ = zerodds_idl::parse(s, &ParserConfig::default());
});
