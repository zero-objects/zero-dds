#![no_main]
//! Fuzz target: performative decoder.
//!
//! Spec OASIS amqp-1.0-transport §2.7 — open/begin/attach/flow/
//! transfer/disposition/detach/end/close. Returns
//! `(descriptor: u64, body: AmqpExtValue, consumed: usize)`. Goal:
//! descriptor switch + composite-body validation against hostile
//! wire input.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::performatives::decode_performative(data);
});
