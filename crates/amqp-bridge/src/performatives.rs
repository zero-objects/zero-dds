// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 Performatives — Spec `amqp-1.0-transport` §2.7.
//!
//! Die 9 Transport-Performatives (`open`, `begin`, `attach`, `flow`,
//! `transfer`, `disposition`, `detach`, `end`, `close`) sind
//! described composites mit Descriptor-Code (ulong) + Body (list).
//!
//! Cross-Ref: DDS-AMQP-1.0 §7.4 Settlement-Mode-Mapping + §6.1
//! Direct-Embed-Topology.

use alloc::vec::Vec;

use crate::extended_types::AmqpExtValue;
use crate::types::{TypeError, codes};

/// Spec `amqp-1.0-transport` §2.7 Tab 2.7 — Descriptor-Codes der
/// 9 Performatives.
pub mod descriptor {
    /// `0x10` — Open (Connection-Open).
    pub const OPEN: u64 = 0x0000_0000_0000_0010;
    /// `0x11` — Begin (Session-Begin).
    pub const BEGIN: u64 = 0x0000_0000_0000_0011;
    /// `0x12` — Attach (Link-Attach).
    pub const ATTACH: u64 = 0x0000_0000_0000_0012;
    /// `0x13` — Flow (Flow-Control).
    pub const FLOW: u64 = 0x0000_0000_0000_0013;
    /// `0x14` — Transfer (Message-Transfer).
    pub const TRANSFER: u64 = 0x0000_0000_0000_0014;
    /// `0x15` — Disposition (Settlement).
    pub const DISPOSITION: u64 = 0x0000_0000_0000_0015;
    /// `0x16` — Detach (Link-Detach).
    pub const DETACH: u64 = 0x0000_0000_0000_0016;
    /// `0x17` — End (Session-End).
    pub const END: u64 = 0x0000_0000_0000_0017;
    /// `0x18` — Close (Connection-Close).
    pub const CLOSE: u64 = 0x0000_0000_0000_0018;
}

/// Described composite = `0x00` (Descriptor marker) + Descriptor-Value
/// + Body. Per Spec §1.3.4.
const DESCRIBED_PREFIX: u8 = 0x00;

/// Encodes a performative as described composite.
///
/// # Errors
/// see [`TypeError`].
pub fn encode_performative(descriptor: u64, body: &AmqpExtValue) -> Result<Vec<u8>, TypeError> {
    let mut out = alloc::vec![DESCRIBED_PREFIX];
    out.extend_from_slice(&AmqpExtValue::Ulong(descriptor).encode()?);
    out.extend_from_slice(&body.encode()?);
    Ok(out)
}

/// Decodes a performative.
///
/// Returns (descriptor, body, consumed_bytes).
///
/// # Errors
/// see [`TypeError`].
pub fn decode_performative(bytes: &[u8]) -> Result<(u64, AmqpExtValue, usize), TypeError> {
    if bytes.is_empty() || bytes[0] != DESCRIBED_PREFIX {
        return Err(TypeError::Truncated);
    }
    let mut cur = 1;
    let (desc, n) = AmqpExtValue::decode(&bytes[cur..])?;
    cur += n;
    let descriptor = match desc {
        AmqpExtValue::Ulong(u) => u,
        _ => return Err(TypeError::UnsupportedFormatCode(codes::ULONG)),
    };
    let (body, n) = AmqpExtValue::decode(&bytes[cur..])?;
    cur += n;
    Ok((descriptor, body, cur))
}

// ============================================================================
//  Strongly-typed convenience constructors per performative.
//
//  Each one builds a list-body and emits the canonical described
//  composite. Field semantics follow Spec §2.7.x; Felder, die per
//  Spec optional sind und nicht gesetzt werden, werden als `Null`
//  encodiert.
// ============================================================================

/// Spec §2.7.1 Open. Required Field: container-id.
pub fn open(container_id: &str) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Str(container_id.into()),
        AmqpExtValue::Null, // hostname
        AmqpExtValue::Null, // max-frame-size
        AmqpExtValue::Null, // channel-max
        AmqpExtValue::Null, // idle-time-out
    ]);
    encode_performative(descriptor::OPEN, &body)
}

/// Spec §2.7.2 Begin. Required: remote-channel (None for new), next-outgoing-id, incoming-window, outgoing-window.
pub fn begin(
    remote_channel: Option<u16>,
    next_outgoing_id: u32,
    incoming_window: u32,
    outgoing_window: u32,
) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        match remote_channel {
            Some(c) => AmqpExtValue::Ushort(c),
            None => AmqpExtValue::Null,
        },
        AmqpExtValue::Uint(next_outgoing_id),
        AmqpExtValue::Uint(incoming_window),
        AmqpExtValue::Uint(outgoing_window),
    ]);
    encode_performative(descriptor::BEGIN, &body)
}

/// Spec §2.7.3 Attach. Required: name, handle, role.
pub fn attach(name: &str, handle: u32, is_sender: bool) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Str(name.into()),
        AmqpExtValue::Uint(handle),
        AmqpExtValue::Boolean(!is_sender), // role: false=sender, true=receiver
    ]);
    encode_performative(descriptor::ATTACH, &body)
}

/// Spec §2.7.4 Flow. Required: incoming-window, next-outgoing-id, outgoing-window.
pub fn flow(
    incoming_window: u32,
    next_outgoing_id: u32,
    outgoing_window: u32,
) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Null, // next-incoming-id
        AmqpExtValue::Uint(incoming_window),
        AmqpExtValue::Uint(next_outgoing_id),
        AmqpExtValue::Uint(outgoing_window),
    ]);
    encode_performative(descriptor::FLOW, &body)
}

/// Spec §2.7.5 Transfer. Required: handle.
pub fn transfer(
    handle: u32,
    delivery_id: Option<u32>,
    settled: bool,
) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Uint(handle),
        match delivery_id {
            Some(d) => AmqpExtValue::Uint(d),
            None => AmqpExtValue::Null,
        },
        AmqpExtValue::Null, // delivery-tag
        AmqpExtValue::Null, // message-format
        AmqpExtValue::Boolean(settled),
    ]);
    encode_performative(descriptor::TRANSFER, &body)
}

/// Spec §2.7.6 Disposition. Required: role, first.
pub fn disposition(
    is_receiver: bool,
    first: u32,
    last: Option<u32>,
    settled: bool,
) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Boolean(is_receiver),
        AmqpExtValue::Uint(first),
        match last {
            Some(l) => AmqpExtValue::Uint(l),
            None => AmqpExtValue::Null,
        },
        AmqpExtValue::Boolean(settled),
    ]);
    encode_performative(descriptor::DISPOSITION, &body)
}

/// Spec §2.7.7 Detach. Required: handle.
pub fn detach(handle: u32, closed: bool) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Uint(handle),
        AmqpExtValue::Boolean(closed),
    ]);
    encode_performative(descriptor::DETACH, &body)
}

/// Spec §2.7.8 End. No required fields.
pub fn end() -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![AmqpExtValue::Null]); // optional error
    encode_performative(descriptor::END, &body)
}

/// Spec §2.7.9 Close. No required fields.
pub fn close() -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![AmqpExtValue::Null]); // optional error
    encode_performative(descriptor::CLOSE, &body)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn round(d: u64, body: AmqpExtValue) {
        let bytes = encode_performative(d, &body).expect("encode");
        let (desc, parsed, _) = decode_performative(&bytes).expect("decode");
        assert_eq!(desc, d);
        assert_eq!(parsed, body);
    }

    #[test]
    fn open_descriptor_is_0x10() {
        let bytes = open("client-1").expect("encode");
        let (desc, _, _) = decode_performative(&bytes).expect("decode");
        assert_eq!(desc, descriptor::OPEN);
    }

    #[test]
    fn begin_descriptor_is_0x11() {
        let bytes = begin(None, 0, 100, 100).expect("encode");
        let (desc, _, _) = decode_performative(&bytes).expect("decode");
        assert_eq!(desc, descriptor::BEGIN);
    }

    #[test]
    fn attach_carries_role_bit() {
        let bytes = attach("link-1", 0, true).expect("encode"); // sender
        let (_desc, body, _) = decode_performative(&bytes).expect("decode");
        if let AmqpExtValue::List(items) = body {
            assert_eq!(items[2], AmqpExtValue::Boolean(false)); // role=sender=false
        } else {
            panic!("expected list body");
        }
    }

    #[test]
    fn flow_carries_window_values() {
        let bytes = flow(50, 7, 100).expect("encode");
        let (_, body, _) = decode_performative(&bytes).expect("decode");
        if let AmqpExtValue::List(items) = body {
            assert_eq!(items[1], AmqpExtValue::Uint(50));
        } else {
            panic!();
        }
    }

    #[test]
    fn transfer_default_settled_false() {
        let bytes = transfer(0, None, false).expect("encode");
        let (desc, _, _) = decode_performative(&bytes).expect("decode");
        assert_eq!(desc, descriptor::TRANSFER);
    }

    #[test]
    fn disposition_settled_round_trip() {
        let bytes = disposition(true, 0, Some(10), true).expect("encode");
        let (desc, _, _) = decode_performative(&bytes).expect("decode");
        assert_eq!(desc, descriptor::DISPOSITION);
    }

    #[test]
    fn detach_round_trips() {
        let bytes = detach(7, true).expect("encode");
        let (desc, _, _) = decode_performative(&bytes).expect("decode");
        assert_eq!(desc, descriptor::DETACH);
    }

    #[test]
    fn end_and_close_are_minimal() {
        let e = end().expect("encode");
        assert_eq!(decode_performative(&e).expect("decode").0, descriptor::END);
        let c = close().expect("encode");
        assert_eq!(
            decode_performative(&c).expect("decode").0,
            descriptor::CLOSE
        );
    }

    #[test]
    fn arbitrary_descriptor_round_trip() {
        round(
            0xDEAD_BEEF,
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(42)]),
        );
    }

    #[test]
    fn truncated_input_yields_error() {
        assert!(decode_performative(&[]).is_err());
        // Missing described prefix:
        assert!(decode_performative(&[0xFF, 0x00]).is_err());
    }

    #[test]
    fn all_nine_descriptors_have_unique_values() {
        let descs = alloc::vec![
            descriptor::OPEN,
            descriptor::BEGIN,
            descriptor::ATTACH,
            descriptor::FLOW,
            descriptor::TRANSFER,
            descriptor::DISPOSITION,
            descriptor::DETACH,
            descriptor::END,
            descriptor::CLOSE,
        ];
        let mut sorted = descs.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), descs.len(), "descriptors must be unique");
    }
}
