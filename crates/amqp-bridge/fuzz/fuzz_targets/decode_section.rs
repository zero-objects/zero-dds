#![no_main]
//! Fuzz-Target: Message-Section-Dekoder.
//!
//! Spec OASIS amqp-1.0-messaging §3.2 — Header / Delivery-
//! Annotations / Message-Annotations / Properties /
//! Application-Properties / Body / Footer. `MessageSection::decode`
//! routet auf den passenden Composite-Pfad — wir fuzzen den Switch
//! plus die einzelnen Body-Decoder.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::sections::MessageSection::decode(data);
});
