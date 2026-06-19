// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 Performatives — Spec `amqp-1.0-transport` §2.7.
//!
//! The 9 transport performatives (`open`, `begin`, `attach`, `flow`,
//! `transfer`, `disposition`, `detach`, `end`, `close`) are
//! described composites with a descriptor code (ulong) + body (list).
//!
//! Cross-Ref: DDS-AMQP-1.0 §7.4 Settlement-Mode-Mapping + §6.1
//! Direct-Embed-Topology.

use alloc::vec::Vec;

use crate::extended_types::AmqpExtValue;
use crate::types::{TypeError, codes};

/// Spec `amqp-1.0-transport` §2.7 Tab 2.7 — descriptor codes of the
/// 9 performatives.
pub mod descriptor {
    /// `0x10` — Open (connection open).
    pub const OPEN: u64 = 0x0000_0000_0000_0010;
    /// `0x11` — Begin (session begin).
    pub const BEGIN: u64 = 0x0000_0000_0000_0011;
    /// `0x12` — Attach (link attach).
    pub const ATTACH: u64 = 0x0000_0000_0000_0012;
    /// `0x13` — Flow (flow control).
    pub const FLOW: u64 = 0x0000_0000_0000_0013;
    /// `0x14` — Transfer (message transfer).
    pub const TRANSFER: u64 = 0x0000_0000_0000_0014;
    /// `0x15` — Disposition (settlement).
    pub const DISPOSITION: u64 = 0x0000_0000_0000_0015;
    /// `0x16` — Detach (link detach).
    pub const DETACH: u64 = 0x0000_0000_0000_0016;
    /// `0x17` — End (session end).
    pub const END: u64 = 0x0000_0000_0000_0017;
    /// `0x18` — Close (connection close).
    pub const CLOSE: u64 = 0x0000_0000_0000_0018;

    // ---- SASL performatives (Spec Part 5 §5.3, amqp:sasl namespace) ----
    /// `0x40` — sasl-mechanisms (server → client: offered mechanisms).
    pub const SASL_MECHANISMS: u64 = 0x0000_0000_0000_0040;
    /// `0x41` — sasl-init (client → server: chosen mechanism + response).
    pub const SASL_INIT: u64 = 0x0000_0000_0000_0041;
    /// `0x42` — sasl-challenge (server → client).
    pub const SASL_CHALLENGE: u64 = 0x0000_0000_0000_0042;
    /// `0x43` — sasl-response (client → server).
    pub const SASL_RESPONSE: u64 = 0x0000_0000_0000_0043;
    /// `0x44` — sasl-outcome (server → client: result code).
    pub const SASL_OUTCOME: u64 = 0x0000_0000_0000_0044;
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
//  Each one builds a list body and emits the canonical described
//  composite. Field semantics follow Spec §2.7.x; fields that are
//  optional per spec and are not set are encoded as `Null`.
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

/// Spec §2.7.3 Attach **with a node address** — the form a broker (RabbitMQ)
/// needs to route. A sender carries a `target` `{address}` (descriptor `0x29`);
/// a receiver carries a `source` `{address}` (descriptor `0x28`). The sender
/// also sets `initial-delivery-count` (required for `role=sender`).
///
/// `address` is the RabbitMQ v2 form, e.g. `/queues/<name>` or
/// `/exchanges/<name>/<routing-key>`.
///
/// # Errors
/// see [`TypeError`].
pub fn attach_to(
    name: &str,
    handle: u32,
    is_sender: bool,
    address: &str,
) -> Result<Vec<u8>, TypeError> {
    use alloc::boxed::Box;
    // source (0x28) / target (0x29) = described list whose first field is the
    // address string; remaining node fields default to null.
    let node = |desc: u64| AmqpExtValue::Described {
        descriptor: desc,
        value: Box::new(AmqpExtValue::List(alloc::vec![AmqpExtValue::Str(
            address.into()
        )])),
    };
    let (source, target) = if is_sender {
        (AmqpExtValue::Null, node(0x0000_0000_0000_0029))
    } else {
        (node(0x0000_0000_0000_0028), AmqpExtValue::Null)
    };
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Str(name.into()),
        AmqpExtValue::Uint(handle),
        AmqpExtValue::Boolean(!is_sender), // role: false=sender, true=receiver
        AmqpExtValue::Null,                // snd-settle-mode
        AmqpExtValue::Null,                // rcv-settle-mode
        source,
        target,
        AmqpExtValue::Null,           // unsettled
        AmqpExtValue::Boolean(false), // incomplete-unsettled
        // initial-delivery-count: required for a sender; null for a receiver.
        if is_sender {
            AmqpExtValue::Uint(0)
        } else {
            AmqpExtValue::Null
        },
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

/// Spec §2.7.4 Flow that **grants link credit** on a receiver link — the way a
/// consumer tells the broker how many messages it is ready to receive. Carries
/// the link `handle`, the receiver's `delivery-count`, and `link-credit`.
///
/// # Errors
/// see [`TypeError`].
pub fn flow_link_credit(
    handle: u32,
    delivery_count: u32,
    link_credit: u32,
) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Null,         // next-incoming-id
        AmqpExtValue::Uint(2048),   // incoming-window
        AmqpExtValue::Uint(0),      // next-outgoing-id
        AmqpExtValue::Uint(2048),   // outgoing-window
        AmqpExtValue::Uint(handle), // handle
        AmqpExtValue::Uint(delivery_count),
        AmqpExtValue::Uint(link_credit),
    ]);
    encode_performative(descriptor::FLOW, &body)
}

/// Spec §2.7.6 + §3.4.2 Disposition with the terminal **Accepted** outcome —
/// a receiver settling a delivery (or range `first..=last`) as accepted.
///
/// # Errors
/// see [`TypeError`].
pub fn disposition_accept(first: u32, last: Option<u32>) -> Result<Vec<u8>, TypeError> {
    use alloc::boxed::Box;
    // accepted = described 0x24 with an empty list.
    let accepted = AmqpExtValue::Described {
        descriptor: 0x0000_0000_0000_0024,
        value: Box::new(AmqpExtValue::List(alloc::vec![])),
    };
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Boolean(true), // role: receiver
        AmqpExtValue::Uint(first),
        match last {
            Some(l) => AmqpExtValue::Uint(l),
            None => AmqpExtValue::Null,
        },
        AmqpExtValue::Boolean(true), // settled
        accepted,                    // state
    ]);
    encode_performative(descriptor::DISPOSITION, &body)
}

/// Extracts the **Data section** payload from a transfer frame body (the bytes
/// after the `transfer` performative). Returns the first `Data` section's
/// octets, or `None` if the frame carries no `Data` section (e.g. an
/// amqp-value body, or a bare transfer continuation).
#[must_use]
pub fn data_payload_from_transfer(frame_body: &[u8]) -> Option<Vec<u8>> {
    // Skip the transfer performative.
    let (_desc, _body, consumed) = decode_performative(frame_body).ok()?;
    let mut rest = &frame_body[consumed..];
    // Walk message sections; return the first Data section.
    while !rest.is_empty() {
        let (section, n) = crate::sections::MessageSection::decode(rest).ok()?;
        if let crate::sections::MessageSection::Data(b) = section {
            return Some(b);
        }
        if n == 0 {
            break;
        }
        rest = &rest[n..];
    }
    None
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

/// Builds a complete **message-bearing transfer** frame body (Spec §2.7.5 +
/// §3.2.6): the `transfer` performative — with a real `delivery-tag` (brokers
/// such as RabbitMQ reject a null tag) and `message-format` 0 — immediately
/// followed by a `Data` message section carrying `payload`. The result is the
/// AMQP frame body for a single settled (or unsettled) delivery.
///
/// # Errors
/// see [`TypeError`].
pub fn transfer_message(
    handle: u32,
    delivery_id: u32,
    delivery_tag: &[u8],
    payload: &[u8],
    settled: bool,
) -> Result<Vec<u8>, TypeError> {
    let perf = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Uint(handle),
        AmqpExtValue::Uint(delivery_id),
        AmqpExtValue::Binary(delivery_tag.to_vec()),
        AmqpExtValue::Uint(0), // message-format
        AmqpExtValue::Boolean(settled),
    ]);
    let mut out = encode_performative(descriptor::TRANSFER, &perf)?;
    out.extend_from_slice(&crate::sections::MessageSection::Data(payload.to_vec()).encode()?);
    Ok(out)
}

// ============================================================================
// SASL performatives (Spec Part 5 §5.3) — carried in SASL frames (type 0x01)
// ============================================================================

/// Spec §5.3.1 sasl-init: client → server. `mechanism` (symbol), the
/// `initial-response` (binary, e.g. PLAIN's `\0user\0pass`), no hostname.
///
/// # Errors
/// see [`TypeError`].
pub fn sasl_init(mechanism: &str, initial_response: &[u8]) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Symbol(mechanism.into()),
        AmqpExtValue::Binary(initial_response.to_vec()),
        AmqpExtValue::Null, // hostname
    ]);
    encode_performative(descriptor::SASL_INIT, &body)
}

/// Spec §5.3.2 sasl-response: client → server (challenge/response mechanisms).
///
/// # Errors
/// see [`TypeError`].
pub fn sasl_response(response: &[u8]) -> Result<Vec<u8>, TypeError> {
    let body = AmqpExtValue::List(alloc::vec![AmqpExtValue::Binary(response.to_vec())]);
    encode_performative(descriptor::SASL_RESPONSE, &body)
}

/// The offered mechanisms from a decoded **sasl-mechanisms** performative body
/// (`sasl-server-mechanisms`: a symbol, or an array of symbols). Returns the
/// mechanism names. Empty when none could be extracted.
#[must_use]
pub fn sasl_mechanisms_from_body(body: &AmqpExtValue) -> alloc::vec::Vec<alloc::string::String> {
    let field = match body {
        AmqpExtValue::List(items) => items.first(),
        other => Some(other),
    };
    match field {
        Some(AmqpExtValue::Symbol(s)) => alloc::vec![s.clone()],
        Some(AmqpExtValue::Array(items)) => items
            .iter()
            .filter_map(|v| match v {
                AmqpExtValue::Symbol(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => alloc::vec::Vec::new(),
    }
}

/// The result code from a decoded **sasl-outcome** performative body
/// (`code`: ubyte — 0=ok, 1=auth, 2=sys, 3=sys-perm, 4=sys-temp). `None` if the
/// code field is absent/mistyped.
#[must_use]
pub fn sasl_outcome_code(body: &AmqpExtValue) -> Option<u8> {
    let field = match body {
        AmqpExtValue::List(items) => items.first(),
        other => Some(other),
    };
    match field {
        Some(AmqpExtValue::Ubyte(c)) => Some(*c),
        _ => None,
    }
}

/// Builds the SASL PLAIN `initial-response` (RFC 4616):
/// `[authzid] \0 authcid \0 passwd` — we send an empty authzid.
#[must_use]
pub fn sasl_plain_response(username: &str, password: &str) -> alloc::vec::Vec<u8> {
    let mut r = alloc::vec::Vec::with_capacity(username.len() + password.len() + 2);
    r.push(0); // empty authzid
    r.extend_from_slice(username.as_bytes());
    r.push(0);
    r.extend_from_slice(password.as_bytes());
    r
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

    #[test]
    fn attach_to_carries_target_address() {
        let frame = attach_to("link-1", 0, true, "/queues/zd10").unwrap();
        let (desc, body, _n) = decode_performative(&frame).unwrap();
        assert_eq!(desc, descriptor::ATTACH);
        if let AmqpExtValue::List(items) = body {
            assert_eq!(items[0], AmqpExtValue::Str("link-1".into()));
            assert_eq!(items[2], AmqpExtValue::Boolean(false)); // sender role
            // target (index 6) is a described 0x29 with the address.
            if let AmqpExtValue::Described {
                descriptor: d,
                value,
            } = &items[6]
            {
                assert_eq!(*d, 0x29);
                if let AmqpExtValue::List(tf) = value.as_ref() {
                    assert_eq!(tf[0], AmqpExtValue::Str("/queues/zd10".into()));
                } else {
                    panic!("target body must be a list");
                }
            } else {
                panic!("target must be described");
            }
        } else {
            panic!("attach body must be a list");
        }
    }

    #[test]
    fn transfer_message_has_tag_and_data() {
        let f = transfer_message(0, 1, b"\x00\x00\x00\x01", b"hello-rmq", true).unwrap();
        // first performative = transfer; then a Data section follows.
        let (desc, _body, consumed) = decode_performative(&f).unwrap();
        assert_eq!(desc, descriptor::TRANSFER);
        assert!(
            consumed < f.len(),
            "a Data section must follow the transfer performative"
        );
    }

    #[test]
    fn sasl_init_descriptor_and_roundtrip() {
        let resp = sasl_plain_response("zerodds", "zerodds");
        // RFC 4616: \0 zerodds \0 zerodds.
        assert_eq!(resp[0], 0);
        assert_eq!(&resp[1..8], b"zerodds");
        assert_eq!(resp[8], 0);
        let frame = sasl_init("PLAIN", &resp).unwrap();
        let (desc, body, _n) = decode_performative(&frame).unwrap();
        assert_eq!(desc, descriptor::SASL_INIT);
        // body = [Symbol("PLAIN"), Binary(resp), Null].
        if let AmqpExtValue::List(items) = body {
            assert_eq!(items[0], AmqpExtValue::Symbol("PLAIN".into()));
            assert_eq!(items[1], AmqpExtValue::Binary(resp));
        } else {
            panic!("sasl-init body must be a list");
        }
    }

    #[test]
    fn sasl_mechanisms_extracts_symbols() {
        // Array form (server offers several).
        let arr = AmqpExtValue::List(alloc::vec![AmqpExtValue::Array(alloc::vec![
            AmqpExtValue::Symbol("PLAIN".into()),
            AmqpExtValue::Symbol("AMQPLAIN".into()),
        ])]);
        let m = sasl_mechanisms_from_body(&arr);
        assert_eq!(m, alloc::vec!["PLAIN".to_string(), "AMQPLAIN".to_string()]);
        // Single-symbol form.
        let one = AmqpExtValue::List(alloc::vec![AmqpExtValue::Symbol("EXTERNAL".into())]);
        assert_eq!(
            sasl_mechanisms_from_body(&one),
            alloc::vec!["EXTERNAL".to_string()]
        );
    }

    #[test]
    fn sasl_outcome_code_extracted() {
        let ok = AmqpExtValue::List(alloc::vec![AmqpExtValue::Ubyte(0)]);
        assert_eq!(sasl_outcome_code(&ok), Some(0));
        let auth = AmqpExtValue::List(alloc::vec![AmqpExtValue::Ubyte(1)]);
        assert_eq!(sasl_outcome_code(&auth), Some(1));
    }
}
