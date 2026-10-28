// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! Every DDS-XRCE datagram a foreign implementation actually sent,
//! decoded and written back: **1,772 of them**, 886 in each direction.
//!
//! Nothing under `tools/wire-capture/xrce-sessions/` was written by this
//! tree — both endpoints are eProsima Micro-XRCE-DDS 3.0.1, built from
//! source on the measurement host and run against each other with no
//! ZeroDDS code in either process. Provenance is in that directory's
//! `README.md`. A round trip against our own encoder proves only that
//! the encoder and the decoder agree with each other; this tree has
//! recorded that shape of defect twenty-five times.
//!
//! What is checked, per datagram:
//!
//! * it decodes;
//! * every submessage converts to its typed payload and back to the
//!   same submessage — an octet the payload codec drops shows up here
//!   and not in the datagram walk, which copies bodies verbatim;
//! * re-encoding gives back the same octets, up to the one difference
//!   the specification leaves open, which is asserted rather than
//!   tolerated: §8.3.3 requires a submessage to *start* at a multiple
//!   of four, so padding after the **last** submessage is optional.
//!   Micro-XRCE-DDS writes it, this crate does not, and the assertion
//!   is that the two differ by nothing else — the re-encoding is a
//!   prefix of the datagram and the remainder is 1..3 zero octets.
//!
//! That last point is the reason the round trip is not a bare equality:
//! it is the one place where two conforming senders may legally emit
//! different octets for the same message, and it is pinned here so a
//! future change to either side is not silent.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use zerodds_xrce::submessages::{
    AckNackPayload, CreateClientPayload, CreatePayload, DataPayload, DeletePayload,
    FragmentPayload, GetInfoPayload, HeartbeatPayload, InfoPayload, ReadDataPayload, ResetPayload,
    StatusAgentPayload, StatusPayload, TimestampPayload, TimestampReplyPayload, WriteDataPayload,
};
use zerodds_xrce::{Message, Submessage, SubmessageId};

fn capture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/wire-capture/xrce-sessions")
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "a hex datagram has an even length");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn datagrams(file: &str) -> Vec<Vec<u8>> {
    let path = capture_root().join(file);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("capture {} is missing: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(hex_to_bytes)
        .collect()
}

/// Round-trips a submessage through the typed payload of its kind.
///
/// Returns the submessage the payload codec produces. For the kinds
/// whose payload this crate models as an opaque representation the
/// conversion is still a real check: it asserts the submessage id and
/// re-derives the flag byte from the decoded flags.
fn payload_roundtrip(sm: &Submessage) -> Submessage {
    match sm.header.submessage_id {
        SubmessageId::CreateClient => CreateClientPayload::try_from_submessage(sm)
            .expect("CREATE_CLIENT payload")
            .into_submessage()
            .expect("CREATE_CLIENT back"),
        SubmessageId::Create => CreatePayload::try_from_submessage(sm)
            .expect("CREATE payload")
            .into_submessage()
            .expect("CREATE back"),
        SubmessageId::GetInfo => GetInfoPayload::try_from_submessage(sm)
            .expect("GET_INFO payload")
            .into_submessage()
            .expect("GET_INFO back"),
        SubmessageId::Delete => DeletePayload::try_from_submessage(sm)
            .expect("DELETE payload")
            .into_submessage()
            .expect("DELETE back"),
        SubmessageId::StatusAgent => StatusAgentPayload::try_from_submessage(sm)
            .expect("STATUS_AGENT payload")
            .into_submessage()
            .expect("STATUS_AGENT back"),
        SubmessageId::Status => StatusPayload::try_from_submessage(sm)
            .expect("STATUS payload")
            .into_submessage()
            .expect("STATUS back"),
        SubmessageId::Info => InfoPayload::try_from_submessage(sm)
            .expect("INFO payload")
            .into_submessage()
            .expect("INFO back"),
        SubmessageId::AckNack => {
            let e = sm.header.body_endianness();
            let p = AckNackPayload::decode_body(&sm.body, e).expect("ACKNACK body");
            let body = p.encode_body(e).expect("ACKNACK back");
            Submessage::new(SubmessageId::AckNack, sm.header.flags, body).expect("ACKNACK sm")
        }
        SubmessageId::Heartbeat => {
            let e = sm.header.body_endianness();
            let p = HeartbeatPayload::decode_body(&sm.body, e).expect("HEARTBEAT body");
            let body = p.encode_body(e).expect("HEARTBEAT back");
            Submessage::new(SubmessageId::Heartbeat, sm.header.flags, body).expect("HEARTBEAT sm")
        }
        SubmessageId::Timestamp => {
            let e = sm.header.body_endianness();
            let p = TimestampPayload::decode_body(&sm.body, e).expect("TIMESTAMP body");
            let body = p.encode_body(e).expect("TIMESTAMP back");
            Submessage::new(SubmessageId::Timestamp, sm.header.flags, body).expect("TIMESTAMP sm")
        }
        SubmessageId::TimestampReply => {
            let e = sm.header.body_endianness();
            let p = TimestampReplyPayload::decode_body(&sm.body, e).expect("TIMESTAMP_REPLY body");
            let body = p.encode_body(e).expect("TIMESTAMP_REPLY back");
            Submessage::new(SubmessageId::TimestampReply, sm.header.flags, body)
                .expect("TIMESTAMP_REPLY sm")
        }
        SubmessageId::WriteData => WriteDataPayload::try_from_submessage(sm)
            .expect("WRITE_DATA payload")
            .into_submessage()
            .expect("WRITE_DATA back"),
        // The captured DataDeliveryControl has no DHEADER (see the module doc
        // of `submessages::read_data`): written back in that form.
        SubmessageId::ReadData => ReadDataPayload::try_from_submessage(sm)
            .expect("READ_DATA payload")
            .into_submessage_with(
                zerodds_xrce::encoding::Endianness::Little,
                zerodds_xrce::submessages::read_data::DeliveryControlForm::Bare,
            )
            .expect("READ_DATA back"),
        SubmessageId::Data => DataPayload::try_from_submessage(sm)
            .expect("DATA payload")
            .into_submessage()
            .expect("DATA back"),
        SubmessageId::Fragment => FragmentPayload::try_from_submessage(sm)
            .expect("FRAGMENT payload")
            .into_submessage()
            .expect("FRAGMENT back"),
        SubmessageId::Reset => ResetPayload::try_from_submessage(sm)
            .expect("RESET payload")
            .into_submessage()
            .expect("RESET back"),
    }
}

struct Tally {
    datagrams: usize,
    submessages: usize,
    identical: usize,
    padded: usize,
    kinds: BTreeMap<u8, usize>,
}

/// The capture is the reference implementation's: the types of Annex A without
/// `@extensibility` are in the `final` form there (no DHEADER), so the payload
/// codecs write them back in that form.
fn walk(file: &str) -> Tally {
    zerodds_xrce::encoding::with_unannotated_types(
        zerodds_xrce::encoding::UnannotatedTypes::Final,
        || walk_in_final_form(file),
    )
}

fn walk_in_final_form(file: &str) -> Tally {
    let mut t = Tally {
        datagrams: 0,
        submessages: 0,
        identical: 0,
        padded: 0,
        kinds: BTreeMap::new(),
    };

    for (i, bytes) in datagrams(file).iter().enumerate() {
        t.datagrams += 1;
        let msg =
            Message::decode(bytes).unwrap_or_else(|e| panic!("{file}[{i}] does not decode: {e:?}"));

        for sm in &msg.submessages {
            t.submessages += 1;
            *t.kinds.entry(sm.header.submessage_id.as_u8()).or_default() += 1;
            let back = payload_roundtrip(sm);
            assert_eq!(
                back.header, sm.header,
                "{file}[{i}]: payload codec changed the submessage header of {:?}",
                sm.header.submessage_id
            );
            assert_eq!(
                back.body, sm.body,
                "{file}[{i}]: payload codec changed the body of {:?}",
                sm.header.submessage_id
            );
        }

        let re = msg
            .encode()
            .unwrap_or_else(|e| panic!("{file}[{i}] does not re-encode: {e:?}"));

        if re == *bytes {
            t.identical += 1;
            continue;
        }

        // The only legal difference: the trailing padding of the last
        // submessage, which §8.3.3 requires of neither sender.
        assert!(
            bytes.starts_with(&re),
            "{file}[{i}]: re-encoding is not a prefix of the datagram\n  in  {}\n  out {}",
            hex(bytes),
            hex(&re)
        );
        let tail = &bytes[re.len()..];
        assert!(
            tail.len() < 4 && tail.iter().all(|b| *b == 0),
            "{file}[{i}]: trailing octets are not 1..3 zeros: {}",
            hex(tail)
        );
        assert_eq!(
            bytes.len(),
            re.len().div_ceil(4) * 4,
            "{file}[{i}]: the datagram is not the re-encoding padded to four"
        );
        t.padded += 1;
    }
    t
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn every_client_datagram_decodes_and_writes_back() {
    let t = walk("client-to-agent.hex");
    assert_eq!(t.datagrams, 886, "the capture lost datagrams");
    assert_eq!(t.identical + t.padded, t.datagrams);
    // The clients of the reference implementation pad the last
    // submessage; the count is pinned so a change is not silent.
    assert_eq!(t.padded, 13, "client-to-agent trailing-padding count moved");
    assert!(t.submessages >= t.datagrams);
}

#[test]
fn every_agent_datagram_decodes_and_writes_back() {
    let t = walk("agent-to-client.hex");
    assert_eq!(t.datagrams, 886, "the capture lost datagrams");
    assert_eq!(t.identical + t.padded, t.datagrams);
    // The agent's own messages happen to be four-octet multiples already,
    // so its 886 datagrams come back octet for octet.
    assert_eq!(t.padded, 0, "agent-to-client trailing-padding count moved");
    assert_eq!(t.identical, 886);
}

#[test]
fn the_session_carries_twelve_of_the_seventeen_submessage_kinds() {
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    for file in ["client-to-agent.hex", "agent-to-client.hex"] {
        for (k, v) in walk(file).kinds {
            *kinds.entry(k).or_default() += v;
        }
    }
    let seen: Vec<u8> = kinds.keys().copied().collect();
    assert_eq!(
        seen,
        vec![
            0x00, 0x01, 0x03, 0x04, 0x05, 0x07, 0x08, 0x09, 0x0a, 0x0d, 0x0e, 0x0f
        ],
        "the set of submessage kinds in the capture moved"
    );
    // FRAGMENT is the one the crate's own tests could not reach without
    // a peer: it only appears when a payload exceeds the transport MTU.
    assert_eq!(kinds[&0x0d], 303, "FRAGMENT count moved");
}
