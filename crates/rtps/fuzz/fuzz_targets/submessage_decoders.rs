#![no_main]
//! Fuzz target: per-submessage decoder. Distributes the input by
//! the first byte across the individual read_body paths.

use libfuzzer_sys::fuzz_target;

use zerodds_rtps::submessages::{
    AckNackSubmessage, DataFragSubmessage, DataSubmessage, FragmentNumberSet, GapSubmessage,
    HeartbeatFragSubmessage, HeartbeatSubmessage, NackFragSubmessage, SequenceNumberSet,
};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let le = data[0] & 1 != 0;
    let body = &data[1..];
    match data[0] >> 1 {
        0 => {
            let _ = DataSubmessage::read_body(body, le);
        }
        1 => {
            let _ = DataFragSubmessage::read_body(body, le, false, false, false, false);
        }
        2 => {
            let _ = HeartbeatSubmessage::read_body(body, le, false, false, false);
            let _ = HeartbeatSubmessage::read_body(body, le, false, false, true);
        }
        3 => {
            let _ = HeartbeatFragSubmessage::read_body(body, le);
        }
        4 => {
            let _ = AckNackSubmessage::read_body(body, le, false);
        }
        5 => {
            let _ = NackFragSubmessage::read_body(body, le);
        }
        6 => {
            let _ = GapSubmessage::read_body(body, le, false, false);
            let _ = GapSubmessage::read_body(body, le, true, true);
        }
        7 => {
            let _ = SequenceNumberSet::read_from(body, 0, le);
        }
        _ => {
            let _ = FragmentNumberSet::read_from(body, 0, le);
        }
    }
});
