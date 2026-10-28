// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Propagation of the client priority over GIOP (RT-CORBA §5.4.2).
//!
//! With `CLIENT_PROPAGATED`, the client attaches its CORBA priority as an IOP
//! `ServiceContext` with the id [`RT_CORBA_PRIORITY_SC_ID`] (= 10) — a
//! CDR encapsulation (byte-order octet + `short`). The server reads it and runs
//! the request at that priority.

use alloc::format;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, WireError, Writer};

use crate::priority::Priority;

/// IOP ServiceContext id `RTCorbaPriority` (RT-CORBA §5.4.2) = 10.
pub const RT_CORBA_PRIORITY_SC_ID: u32 = 10;

/// Builds the `RTCorbaPriority` ServiceContext data: an encapsulation, so
/// a byte-order octet and a `short` priority.
///
/// # Errors
/// Never, at present — the signature is the one every context builder
/// here has, and a future bounded form would use it.
pub fn encode_priority_context(
    priority: Priority,
    endianness: Endianness,
) -> Result<Vec<u8>, WireError> {
    let mut w = Writer::new(endianness);
    w.write_u8(match endianness {
        Endianness::Big => 0,
        Endianness::Little => 1,
    });
    w.write_i16(priority.value());
    Ok(w.finish())
}

/// Reads the client priority out of `RTCorbaPriority` ServiceContext data.
///
/// # Errors
/// [`WireError::Truncated`] for empty or short data, and
/// [`WireError::OutOfRange`] for a priority outside `0..=32767`.
pub fn decode_priority_context(data: &[u8]) -> Result<Priority, WireError> {
    let bo = *data.first().ok_or(WireError::Truncated {
        at: 0,
        needed: 1,
        available: 0,
    })?;
    let e = if bo == 0 {
        Endianness::Big
    } else {
        Endianness::Little
    };
    let mut r = Reader::new(data, e);
    r.read_u8()?; // the byte-order octet, so the position is now 1
    let raw = r.read_i16()?;
    Priority::new(raw).ok_or_else(|| WireError::OutOfRange {
        at: 2,
        expected: format!("an RTCorbaPriority in 0..=32767, not {raw}"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sc_id_is_ten() {
        assert_eq!(RT_CORBA_PRIORITY_SC_ID, 10);
    }

    #[test]
    fn priority_context_roundtrip() {
        for e in [Endianness::Big, Endianness::Little] {
            let prio = Priority::new(12345).unwrap();
            let data = encode_priority_context(prio, e).unwrap();
            assert_eq!(data[0], u8::from(e == Endianness::Little));
            assert_eq!(decode_priority_context(&data).unwrap(), prio);
        }
    }

    #[test]
    fn byte_exact_big_endian() {
        // Priority 0x0539 = 1337, big-endian: BO=00, pad to short(2)=offset 2? short
        // aligns to 2 → position 1 (after BO) is odd, pad 1 byte → 0539 at offset 2.
        let data = encode_priority_context(Priority::new(1337).unwrap(), Endianness::Big).unwrap();
        let hex: alloc::string::String = data.iter().map(|b| alloc::format!("{b:02x}")).collect();
        assert_eq!(hex, "00000539");
    }
}
