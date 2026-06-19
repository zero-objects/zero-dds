#![no_main]
//! Fuzz target: message-section decoder.
//!
//! Spec OASIS amqp-1.0-messaging §3.2 — Header / Delivery-
//! Annotations / Message-Annotations / Properties /
//! Application-Properties / Body / Footer. `MessageSection::decode`
//! routes to the matching composite path — we fuzz the switch
//! plus the individual body decoders.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::sections::MessageSection::decode(data);
});
