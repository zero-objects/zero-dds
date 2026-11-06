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

/// A sender link whose deliveries the receiver settles (Spec §2.7.3 with
/// `snd-settle-mode` = `unsettled` (0, §2.8.2) and `rcv-settle-mode` =
/// `first` (0, §2.8.3)): the broker answers each transfer with a
/// disposition carrying its outcome, which is what a sender that has to
/// know the broker holds the message waits for. Otherwise as
/// [`attach_to`] for a sender.
///
/// # Errors
/// see [`TypeError`].
pub fn attach_sender_unsettled(
    name: &str,
    handle: u32,
    address: &str,
) -> Result<Vec<u8>, TypeError> {
    use alloc::boxed::Box;
    let target = AmqpExtValue::Described {
        descriptor: 0x0000_0000_0000_0029,
        value: Box::new(AmqpExtValue::List(alloc::vec![AmqpExtValue::Str(
            address.into()
        )])),
    };
    let body = AmqpExtValue::List(alloc::vec![
        AmqpExtValue::Str(name.into()),
        AmqpExtValue::Uint(handle),
        AmqpExtValue::Boolean(false), // role: sender
        AmqpExtValue::Ubyte(0),       // snd-settle-mode: unsettled
        AmqpExtValue::Ubyte(0),       // rcv-settle-mode: first
        AmqpExtValue::Null,           // source
        target,
        AmqpExtValue::Null,           // unsettled
        AmqpExtValue::Boolean(false), // incomplete-unsettled
        AmqpExtValue::Uint(0),        // initial-delivery-count
    ]);
    encode_performative(descriptor::ATTACH, &body)
}

/// The delivery-state descriptors of Spec Part 3 §3.4.
pub mod delivery_state {
    /// `received` (§3.4.1) — not an outcome; the delivery is still open.
    pub const RECEIVED: u64 = 0x0000_0000_0000_0023;
    /// `accepted` (§3.4.2).
    pub const ACCEPTED: u64 = 0x0000_0000_0000_0024;
    /// `rejected` (§3.4.3).
    pub const REJECTED: u64 = 0x0000_0000_0000_0025;
    /// `released` (§3.4.4).
    pub const RELEASED: u64 = 0x0000_0000_0000_0026;
    /// `modified` (§3.4.5).
    pub const MODIFIED: u64 = 0x0000_0000_0000_0027;
}

/// The fields of a decoded **disposition** (Spec §2.7.6) a sender waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispositionView {
    /// `role`: `true` when the receiver of the link sent it.
    pub from_receiver: bool,
    /// `first` delivery-id the disposition covers.
    pub first: u32,
    /// `last` delivery-id; `None` means `first` alone.
    pub last: Option<u32>,
    /// `settled`.
    pub settled: bool,
    /// The descriptor of `state`, one of [`delivery_state`], or `None`
    /// when the field is absent.
    pub state: Option<u64>,
}

impl DispositionView {
    /// Whether the disposition covers `delivery_id`.
    #[must_use]
    pub fn covers(&self, delivery_id: u32) -> bool {
        let last = self.last.unwrap_or(self.first);
        self.first <= delivery_id && delivery_id <= last
    }
}

/// Reads the fields of a disposition from its decoded performative body.
/// `None` when `role` or `first` — the two mandatory fields — are missing
/// or mistyped, or another field has a type the clause does not allow.
#[must_use]
pub fn disposition_from_body(body: &AmqpExtValue) -> Option<DispositionView> {
    let AmqpExtValue::List(fields) = body else {
        return None;
    };
    let from_receiver = match fields.first()? {
        AmqpExtValue::Boolean(role) => *role,
        _ => return None,
    };
    let first = match fields.get(1)? {
        AmqpExtValue::Uint(first) => *first,
        _ => return None,
    };
    let last = match fields.get(2) {
        None | Some(AmqpExtValue::Null) => None,
        Some(AmqpExtValue::Uint(last)) => Some(*last),
        Some(_) => return None,
    };
    let settled = match fields.get(3) {
        None | Some(AmqpExtValue::Null) => false,
        Some(AmqpExtValue::Boolean(settled)) => *settled,
        Some(_) => return None,
    };
    let state = match fields.get(4) {
        None | Some(AmqpExtValue::Null) => None,
        Some(AmqpExtValue::Described { descriptor, .. }) => Some(*descriptor),
        Some(_) => return None,
    };
    Some(DispositionView {
        from_receiver,
        first,
        last,
        settled,
        state,
    })
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

    // Expected octets from OASIS AMQP 1.0: a performative is a described
    // list (Part 1 §1.4), 0x00, its descriptor code as a smallulong 0x53
    // (Part 2 §2.7: open 0x10 to close 0x18; Part 5 §5.3.3: the SASL
    // frames 0x40 to 0x44), then `list8` 0xc0 with size and count, the
    // fields in their order each with its constructor.

    /// A performative with descriptor `code` and list `fields` of `count`.
    fn performative(code: u8, count: u8, fields: &[u8]) -> Vec<u8> {
        let mut out = alloc::vec![0x00, 0x53, code, 0xc0, (fields.len() + 1) as u8, count];
        out.extend_from_slice(fields);
        out
    }

    /// §2.7 and §5.3.3: the descriptor codes of the nine performatives and
    /// the five SASL frames.
    #[test]
    fn amqp10_2_7_the_descriptor_codes_are_0x10_to_0x18_and_0x40_to_0x44() {
        let codes = [
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
        assert_eq!(
            codes,
            [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]
        );
        let sasl = [
            descriptor::SASL_MECHANISMS,
            descriptor::SASL_INIT,
            descriptor::SASL_CHALLENGE,
            descriptor::SASL_RESPONSE,
            descriptor::SASL_OUTCOME,
        ];
        assert_eq!(sasl, [0x40, 0x41, 0x42, 0x43, 0x44]);
    }

    /// §2.7.1 open: container-id first.
    #[test]
    fn amqp10_2_7_1_open_is_its_container_id_and_the_fields_after_it() {
        let mut fields = alloc::vec![0xa1, 0x08];
        fields.extend_from_slice(b"client-1");
        fields.extend_from_slice(&[0x40, 0x40, 0x40, 0x40]);
        assert_eq!(open("client-1").unwrap(), performative(0x10, 5, &fields));
    }

    /// §2.7.2 begin: remote-channel, next-outgoing-id, incoming-window,
    /// outgoing-window.
    #[test]
    fn amqp10_2_7_2_begin_is_channel_and_the_three_numbers() {
        assert_eq!(
            begin(None, 0, 100, 100).unwrap(),
            performative(0x11, 4, &[0x40, 0x43, 0x52, 0x64, 0x52, 0x64])
        );
    }

    /// §2.7.3 attach: name, handle, role (§2.8.1: false is the sender).
    #[test]
    fn amqp10_2_7_3_attach_is_name_handle_and_role() {
        let mut fields = alloc::vec![0xa1, 0x06];
        fields.extend_from_slice(b"link-1");
        fields.extend_from_slice(&[0x43, 0x42]);
        assert_eq!(
            attach("link-1", 0, true).unwrap(),
            performative(0x12, 3, &fields)
        );
    }

    /// §2.7.3 with Part 3 §3.5.4: a sender attach names its target, the
    /// described list 0x29 whose first field is the address; it carries
    /// initial-delivery-count.
    #[test]
    fn amqp10_2_7_3_a_sender_attach_names_its_target_address() {
        let mut fields = alloc::vec![0xa1, 0x06];
        fields.extend_from_slice(b"link-1");
        fields.extend_from_slice(&[0x43, 0x42, 0x40, 0x40, 0x40]);
        fields.extend_from_slice(&[0x00, 0x53, 0x29, 0xc0, 0x0f, 0x01, 0xa1, 0x0c]);
        fields.extend_from_slice(b"/queues/zd10");
        fields.extend_from_slice(&[0x40, 0x42, 0x43]);
        assert_eq!(
            attach_to("link-1", 0, true, "/queues/zd10").unwrap(),
            performative(0x12, 10, &fields)
        );
    }

    /// §2.7.3, §2.8.2, §2.8.3: a sender attach that leaves settlement to
    /// the receiver says `snd-settle-mode` unsettled (0) and
    /// `rcv-settle-mode` first (0).
    #[test]
    fn amqp10_2_7_3_an_unsettled_sender_attach_says_so() {
        let mut fields = alloc::vec![
            0xa1, 0x01, b'L', 0x52, 0x03, 0x42, 0x50, 0x00, 0x50, 0x00, 0x40
        ];
        fields.extend_from_slice(&[0x00, 0x53, 0x29, 0xc0, 0x0c, 0x01, 0xa1, 0x09]);
        fields.extend_from_slice(b"/queues/q");
        fields.extend_from_slice(&[0x40, 0x42, 0x43]);
        assert_eq!(
            attach_sender_unsettled("L", 3, "/queues/q").unwrap(),
            performative(0x12, 10, &fields)
        );
    }

    /// §2.7.4 flow: next-incoming-id, incoming-window, next-outgoing-id,
    /// outgoing-window, then handle, delivery-count and link-credit.
    #[test]
    fn amqp10_2_7_4_flow_is_the_windows_then_handle_count_and_credit() {
        assert_eq!(
            flow(50, 7, 100).unwrap(),
            performative(0x13, 4, &[0x40, 0x52, 0x32, 0x52, 0x07, 0x52, 0x64])
        );
        assert_eq!(
            flow_link_credit(1, 0, 10).unwrap(),
            performative(
                0x13,
                7,
                &[
                    0x40, 0x70, 0x00, 0x00, 0x08, 0x00, 0x43, 0x70, 0x00, 0x00, 0x08, 0x00, 0x52,
                    0x01, 0x43, 0x52, 0x0a,
                ]
            )
        );
    }

    /// §2.7.5 transfer: handle, delivery-id, delivery-tag, message-format,
    /// settled; the message follows the performative (Part 3 §3.2.6, data
    /// 0x75).
    #[test]
    fn amqp10_2_7_5_transfer_is_handle_id_tag_format_settled_then_the_message() {
        assert_eq!(
            transfer(0, None, false).unwrap(),
            performative(0x14, 5, &[0x43, 0x40, 0x40, 0x40, 0x42])
        );
        let mut expected = performative(
            0x14,
            5,
            &[
                0x43, 0x52, 0x01, 0xa0, 0x04, 0x00, 0x00, 0x00, 0x01, 0x43, 0x41,
            ],
        );
        expected.extend_from_slice(&[0x00, 0x53, 0x75, 0xa0, 0x09]);
        expected.extend_from_slice(b"hello-rmq");
        assert_eq!(
            transfer_message(0, 1, &[0, 0, 0, 1], b"hello-rmq", true).unwrap(),
            expected
        );
        assert_eq!(
            data_payload_from_transfer(&expected).as_deref(),
            Some(&b"hello-rmq"[..])
        );
    }

    /// §2.7.6 disposition: role, first, last, settled, state (Part 3
    /// §3.4.2: accepted is the described list 0x24).
    #[test]
    fn amqp10_2_7_6_disposition_is_role_first_last_settled_and_state() {
        assert_eq!(
            disposition(true, 0, Some(10), true).unwrap(),
            performative(0x15, 4, &[0x41, 0x43, 0x52, 0x0a, 0x41])
        );
        assert_eq!(
            disposition_accept(4, Some(6)).unwrap(),
            performative(
                0x15,
                5,
                &[0x41, 0x52, 0x04, 0x52, 0x06, 0x41, 0x00, 0x53, 0x24, 0x45]
            )
        );
    }

    /// §2.7.6: a sender reads a receiver's disposition: the range from
    /// first to last (last absent: first alone), and the state.
    #[test]
    fn amqp10_2_7_6_a_received_disposition_reads_its_range_and_state() {
        let accepted = performative(
            0x15,
            5,
            &[0x41, 0x52, 0x04, 0x52, 0x06, 0x41, 0x00, 0x53, 0x24, 0x45],
        );
        let (_, body, _) = decode_performative(&accepted).expect("decode");
        let view = disposition_from_body(&body).expect("view");
        assert_eq!(
            view,
            DispositionView {
                from_receiver: true,
                first: 4,
                last: Some(6),
                settled: true,
                state: Some(delivery_state::ACCEPTED),
            }
        );
        assert!(view.covers(4) && view.covers(6));
        assert!(!view.covers(3) && !view.covers(7));
        let bare = performative(0x15, 4, &[0x42, 0x52, 0x09, 0x40, 0x42]);
        let (_, body, _) = decode_performative(&bare).expect("decode");
        let view = disposition_from_body(&body).expect("view");
        assert_eq!(view.state, None);
        assert!(view.covers(9) && !view.covers(10));
        assert_eq!(
            disposition_from_body(&AmqpExtValue::List(alloc::vec![AmqpExtValue::Uint(1)])),
            None,
            "a disposition without role"
        );
    }

    /// §2.7.7 detach: handle, closed. §2.7.8 end and §2.7.9 close: the
    /// optional error.
    #[test]
    fn amqp10_2_7_7_detach_end_and_close_are_their_fields() {
        assert_eq!(
            detach(7, true).unwrap(),
            performative(0x16, 2, &[0x52, 0x07, 0x41])
        );
        assert_eq!(end().unwrap(), performative(0x17, 1, &[0x40]));
        assert_eq!(close().unwrap(), performative(0x18, 1, &[0x40]));
    }

    /// Part 1 §1.5: a descriptor code above 255 is a full `ulong`.
    #[test]
    fn amqp10_1_5_a_descriptor_code_above_255_is_a_full_ulong() {
        let octets = [
            0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef, 0xc0, 0x03, 0x01, 0x54,
            0x2a,
        ];
        let body = AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(42)]);
        assert_eq!(encode_performative(0xdead_beef, &body).unwrap(), octets);
        assert_eq!(
            decode_performative(&octets).unwrap(),
            (0xdead_beef, body, octets.len())
        );
    }

    /// Part 1 §1.2: an input without the described prefix 0x00 is no
    /// performative.
    #[test]
    fn amqp10_1_2_a_performative_without_the_described_prefix_is_refused() {
        assert!(decode_performative(&[]).is_err());
        assert!(decode_performative(&[0xff, 0x00]).is_err());
    }

    /// §5.3.3.2 sasl-init: mechanism (symbol), initial-response (binary),
    /// hostname; the PLAIN response is `\0user\0password` (RFC 4616 §2).
    #[test]
    fn amqp10_5_3_3_2_sasl_init_is_mechanism_response_and_hostname() {
        let response = sasl_plain_response("zerodds", "zerodds");
        assert_eq!(response, b"\x00zerodds\x00zerodds");
        let mut fields = alloc::vec![0xa3, 0x05];
        fields.extend_from_slice(b"PLAIN");
        fields.extend_from_slice(&[0xa0, 0x10]);
        fields.extend_from_slice(b"\x00zerodds\x00zerodds");
        fields.push(0x40);
        assert_eq!(
            sasl_init("PLAIN", &response).unwrap(),
            performative(0x41, 3, &fields)
        );
    }

    /// §5.3.3.1 sasl-mechanisms: the offered mechanisms, an array of
    /// symbols or one symbol.
    #[test]
    fn amqp10_5_3_3_1_the_offered_mechanisms_are_read_from_an_array_or_one_symbol() {
        let mut array = alloc::vec![0xe0, 0x11, 0x02, 0xa3, 0x05];
        array.extend_from_slice(b"PLAIN");
        array.push(0x08);
        array.extend_from_slice(b"AMQPLAIN");
        let (desc, body, _) = decode_performative(&performative(0x40, 1, &array)).unwrap();
        assert_eq!(desc, descriptor::SASL_MECHANISMS);
        assert_eq!(
            sasl_mechanisms_from_body(&body),
            alloc::vec!["PLAIN".to_string(), "AMQPLAIN".to_string()]
        );
        let mut one = alloc::vec![0xa3, 0x08];
        one.extend_from_slice(b"EXTERNAL");
        let (_, body, _) = decode_performative(&performative(0x40, 1, &one)).unwrap();
        assert_eq!(
            sasl_mechanisms_from_body(&body),
            alloc::vec!["EXTERNAL".to_string()]
        );
    }

    /// §5.3.3.5 sasl-outcome: the code (§5.3.3.6: 0 ok, 1 auth) as a
    /// ubyte.
    #[test]
    fn amqp10_5_3_3_5_the_outcome_code_is_read() {
        let (_, ok, _) = decode_performative(&performative(0x44, 1, &[0x50, 0x00])).unwrap();
        assert_eq!(sasl_outcome_code(&ok), Some(0));
        let (_, auth, _) = decode_performative(&performative(0x44, 1, &[0x50, 0x01])).unwrap();
        assert_eq!(sasl_outcome_code(&auth), Some(1));
    }
}
