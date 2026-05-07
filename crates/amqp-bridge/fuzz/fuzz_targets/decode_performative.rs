#![no_main]
//! Fuzz-Target: Performative-Dekoder.
//!
//! Spec OASIS amqp-1.0-transport §2.7 — open/begin/attach/flow/
//! transfer/disposition/detach/end/close. Liefert
//! `(descriptor: u64, body: AmqpExtValue, consumed: usize)`. Ziel:
//! Descriptor-Switch + Composite-Body-Validierung gegen feindlichen
//! Wire-Input.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = zerodds_amqp_bridge::performatives::decode_performative(data);
});
