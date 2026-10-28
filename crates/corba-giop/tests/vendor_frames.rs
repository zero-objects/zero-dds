// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Every GIOP message three foreign ORBs actually sent, decoded and
//! written back.
//!
//! `tools/wire-capture/frames/{omniorb,jacorb,tao}/*.frames` hold the TCP
//! payloads of real omniORB 4.3, JacORB 3.9 and TAO 2.5 sessions,
//! reassembled per direction — one line is one stream, and a stream is
//! whole GIOP messages back to back. Nothing in them was written by this
//! tree, which is the point: a round-trip against our own encoder proves
//! only that the encoder and the decoder agree with each other, and this
//! tree has recorded six "vendor differences" that turned out to be its
//! own measurement.
//!
//! What is checked, per message:
//!
//! * it decodes;
//! * re-encoding it gives back the **same octets**, so nothing was read
//!   loosely and written differently;
//! * the stream is consumed to its end, so no message was skipped and
//!   none was read short.
//!
//! Byte-identity is the load-bearing assertion. GIOP 1.2 aligns a
//! request body to eight **from the start of the message header**
//! (§15.4.2), and a codec that counted alignment from the start of the
//! body instead would round-trip against itself and put the body four
//! octets off against every one of these captures.
//!
//! The one exemption is alignment padding, and it is not granted by
//! looking at the difference. CDR says nothing about what fills an
//! alignment gap, and the ORBs disagree: omniORB and JacORB write zeros
//! and reach byte-identity here with no exemption at all, TAO writes
//! what its buffer held. `encode_message_with_padding` says which
//! octets of *our* message the layout filled, and only those are
//! allowed to differ — read off the plan, not off the mismatch.

use std::fs;
use std::path::PathBuf;

use zerodds_corba_giop::{
    Message, MessageType, Version, decode_message_ctx, encode_message_with_padding,
};

/// How much a vendor's alignment padding is allowed to differ from ours.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Padding {
    /// Every octet matches, fill included. omniORB 4.3 and JacORB 3.9.
    Identical,
    /// Every octet outside an alignment gap matches. TAO 2.5.
    FillMayDiffer,
}

fn frames_dir(vendor: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/wire-capture/frames")
        .join(vendor)
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "a hex stream has an even length");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// Decodes every message of every stream of every capture under
/// `vendor`, and returns how many there were.
fn walk_vendor(vendor: &str, padding: Padding) -> usize {
    let dir = frames_dir(vendor);
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("no captures at {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "frames"))
        .collect();
    entries.sort();
    assert!(
        !entries.is_empty(),
        "no .frames under {} — a capture set that is not there passes every \
         assertion it does not run",
        dir.display()
    );

    let mut messages = 0usize;
    for path in entries {
        let text = fs::read_to_string(&path).unwrap();
        for (line_no, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let stream = hex_to_bytes(line);
            let mut rest: &[u8] = &stream;
            let mut in_stream = 0usize;
            while !rest.is_empty() {
                let consumed_before = stream.len() - rest.len();
                let (msg, endianness, tail) = decode_message_ctx(rest).unwrap_or_else(|e| {
                    panic!(
                        "{}:{} message {in_stream} at octet {consumed_before}: {e:?}",
                        path.display(),
                        line_no + 1
                    )
                });
                let message_len = rest.len() - tail.len();
                let original = &rest[..message_len];

                // GIOP 1.2 throughout these captures; the version is read
                // back out of the header rather than assumed, so a
                // capture set that ever gains a 1.0 stream is encoded as
                // 1.0 and not silently as 1.2.
                let version = version_of(original);
                let more_fragments = original[6] & 0x02 != 0;
                let (re, spans) =
                    encode_message_with_padding(version, endianness, more_fragments, &msg)
                        .unwrap_or_else(|e| {
                            panic!(
                                "{}:{} message {in_stream}: re-encode refused: {e:?}",
                                path.display(),
                                line_no + 1
                            )
                        });
                let where_it_differs = differences(&re, original, spans, padding);
                assert!(
                    where_it_differs.is_empty(),
                    "{}:{} message {in_stream} ({:?}, {endianness:?}) is not written back the \
                     way it arrived — octets {where_it_differs:?}\n  ours: {re:02x?}\n theirs: \
                     {original:02x?}",
                    path.display(),
                    line_no + 1,
                    msg.message_type(),
                );

                // §15.4.2: the request body starts 8-aligned from the
                // message header, which is the property a body-local
                // alignment origin gets wrong while still round-tripping.
                if let Message::Request(req) = &msg
                    && !req.body.is_empty()
                {
                    let body_at = message_len - req.body.len();
                    assert_eq!(
                        body_at % 8,
                        0,
                        "{}:{} message {in_stream}: body at {body_at}",
                        path.display(),
                        line_no + 1
                    );
                }

                rest = tail;
                in_stream += 1;
                messages += 1;
            }
        }
    }
    messages
}

/// The offsets at which `ours` and `theirs` differ, minus the ones the
/// exemption covers. A length mismatch is reported as the offsets past
/// the shorter of the two, so a message that is four octets long is not
/// silently compared over its prefix.
fn differences(
    ours: &[u8],
    theirs: &[u8],
    padding: Vec<(usize, usize)>,
    exemption: Padding,
) -> Vec<usize> {
    let mut out: Vec<usize> =
        (ours.len().min(theirs.len())..ours.len().max(theirs.len())).collect();
    let is_fill = |i: usize| {
        exemption == Padding::FillMayDiffer
            && padding.iter().any(|(at, len)| i >= *at && i < at + len)
    };
    out.extend((0..ours.len().min(theirs.len())).filter(|&i| ours[i] != theirs[i] && !is_fill(i)));
    out.sort_unstable();
    out
}

fn version_of(message: &[u8]) -> Version {
    Version {
        major: message[4],
        minor: message[5],
    }
}

/// omniORB 4.3 zeroes its alignment fill, so every octet matches.
#[test]
fn omniorb_messages_decode_and_re_encode_byte_for_byte() {
    let n = walk_vendor("omniorb", Padding::Identical);
    assert_eq!(n, 1003, "omniORB capture message count");
}

/// JacORB 3.9, big-endian on a little-endian host, and likewise
/// octet-for-octet.
#[test]
fn jacorb_messages_decode_and_re_encode_byte_for_byte() {
    let n = walk_vendor("jacorb", Padding::Identical);
    assert_eq!(n, 798, "JacORB capture message count");
}

/// TAO 2.5 leaves the alignment gap before a request body holding
/// whatever its buffer had — `01 00 00 00` in
/// `annotation-key-multiple.xcdr1`. Everything that carries a value
/// matches.
#[test]
fn tao_messages_decode_and_re_encode_outside_the_alignment_fill() {
    let n = walk_vendor("tao", Padding::FillMayDiffer);
    assert_eq!(n, 798, "TAO capture message count");
}

/// The exemption TAO gets is an exemption and not a blanket: under
/// `Padding::Identical` the TAO captures do differ, so the comparison is
/// looking at those octets and would report a real divergence in them.
#[test]
fn the_tao_exemption_is_load_bearing() {
    let result = std::panic::catch_unwind(|| walk_vendor("tao", Padding::Identical));
    assert!(
        result.is_err(),
        "TAO's captures compared octet for octet should fail on its alignment fill"
    );
}

/// The header-to-body gap is actually present, and what each ORB puts
/// in it.
///
/// Without this the three tests above could pass over captures where
/// every message's gap is empty, and the alignment fill would never have
/// been compared at all — the assertion would be about octets that do
/// not exist.
///
/// Measured over the whole capture set: 57 JacORB messages, 59 omniORB
/// and 285 TAO carry a four-octet gap. JacORB and omniORB write
/// `00 00 00 00` in every one; TAO writes `01 00 00 00` in every one.
///
/// One limit of the measurement, and it is the one
/// `docs/OPEN-ITEMS.md`'s open item about `markHeaderEnd` names: all 57
/// JacORB instances are **the first message of their connection**, one
/// per capture file and so one per JVM. JacORB's `BufferManager` pool
/// hands back buffers uncleared, so these captures cannot separate "the
/// gap is zeroed" from "the buffer was fresh". TAO's are messages 1 to
/// 5 of each stream and never message 0, which is what reuse looks
/// like. Answering the JacORB half needs a capture with several
/// body-carrying requests on one connection, and this set has none.
#[test]
fn the_alignment_gap_is_present_and_each_orb_fills_it_its_own_way() {
    let mut fills: std::collections::BTreeMap<&str, Vec<(usize, String)>> =
        std::collections::BTreeMap::new();
    for vendor in ["omniorb", "jacorb", "tao"] {
        for path in fs::read_dir(frames_dir(vendor)).unwrap() {
            let path = path.unwrap().path();
            if path.extension().is_none_or(|x| x != "frames") {
                continue;
            }
            for line in fs::read_to_string(&path).unwrap().lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let stream = hex_to_bytes(line);
                let mut rest: &[u8] = &stream;
                let mut index = 0usize;
                while !rest.is_empty() {
                    let (msg, endianness, tail) = decode_message_ctx(rest).unwrap();
                    let original = &rest[..rest.len() - tail.len()];
                    let (_, spans) =
                        encode_message_with_padding(version_of(original), endianness, false, &msg)
                            .unwrap();
                    for (at, len) in spans {
                        // The gap that matters is the one before an
                        // 8-aligned body; the ones inside the header
                        // fields are two and four octets wide at most
                        // and sit before the operation string.
                        if len == 4 && (at + len) % 8 == 0 {
                            let octets: String = original[at..at + len]
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect();
                            fills.entry(vendor).or_default().push((index, octets));
                        }
                    }
                    rest = tail;
                    index += 1;
                }
            }
        }
    }
    for vendor in ["omniorb", "jacorb", "tao"] {
        let seen = fills.get(vendor).map(Vec::as_slice).unwrap_or_default();
        assert!(
            !seen.is_empty(),
            "{vendor}: no message carries a header-to-body gap, so the fill was never compared"
        );
        let expected = if vendor == "tao" {
            "01000000"
        } else {
            "00000000"
        };
        for (index, octets) in seen {
            assert_eq!(octets, expected, "{vendor} message {index}: alignment fill");
        }
    }
    // One tuple so a moved capture set reports all three counts in a
    // single run instead of one per run.
    assert_eq!(
        (
            fills["jacorb"].len(),
            fills["omniorb"].len(),
            fills["tao"].len()
        ),
        (57, 59, 285),
        "(JacORB, omniORB, TAO) gap counts"
    );
    // Every JacORB gap is the first message of its connection, which is
    // why the zeros do not settle whether the fill is written or found.
    assert!(
        fills["jacorb"].iter().all(|(index, _)| *index == 0),
        "JacORB gaps are all at message 0: {:?}",
        fills["jacorb"]
    );
    assert!(
        fills["tao"].iter().all(|(index, _)| *index > 0),
        "TAO gaps are all past message 0"
    );
}

/// The captures have to contain more than one message type and more than
/// one byte order, or the three tests above are one test run three times.
#[test]
fn the_captures_cover_both_byte_orders_and_several_message_types() {
    let mut types = std::collections::BTreeSet::new();
    let mut orders = std::collections::BTreeSet::new();
    for vendor in ["omniorb", "jacorb", "tao"] {
        for path in fs::read_dir(frames_dir(vendor)).unwrap() {
            let path = path.unwrap().path();
            if path.extension().is_none_or(|x| x != "frames") {
                continue;
            }
            for line in fs::read_to_string(&path).unwrap().lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let stream = hex_to_bytes(line);
                let mut rest: &[u8] = &stream;
                while !rest.is_empty() {
                    let (msg, endianness, tail) = decode_message_ctx(rest).unwrap();
                    types.insert(msg.message_type() as u8);
                    orders.insert(format!("{endianness:?}"));
                    rest = tail;
                }
            }
        }
    }
    assert_eq!(orders.len(), 2, "both byte orders: {orders:?}");
    assert!(
        types.contains(&(MessageType::Request as u8))
            && types.contains(&(MessageType::Reply as u8))
            && types.contains(&(MessageType::LocateRequest as u8))
            && types.contains(&(MessageType::LocateReply as u8))
            && types.contains(&(MessageType::CloseConnection as u8)),
        "message types present: {types:?}"
    );
}
