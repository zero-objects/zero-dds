#![no_main]
use zerodds_xml::qos_parser::parse_qos_libraries;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let _ = parse_qos_libraries(s);
});
