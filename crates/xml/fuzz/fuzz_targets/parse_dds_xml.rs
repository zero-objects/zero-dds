#![no_main]
use zerodds_xml::zerodds_xml::parse_dds_xml;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let _ = parse_dds_xml(s);
});
