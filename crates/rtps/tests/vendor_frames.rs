// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Every RTPS datagram six foreign stacks actually sent, decoded and
//! written back: **5,774 of them**, 5,016 from the corpus run and 758
//! from the session captures.
//!
//! Nothing under `tools/wire-capture/frames/` or
//! `tools/wire-capture/rtps-sessions/` was written by this tree, and
//! that is the point. A round trip against our own encoder proves only
//! that the encoder and the decoder agree with each other; this tree
//! has recorded that shape of defect twenty-one times, most recently a
//! GIOP message whose four octets of padding no ORB writes.
//!
//! Two capture sets, and they answer different questions:
//!
//! * **`frames/`** is `wire-golden.sh`'s corpus run — the UDP payload of
//!   every packet that carried a *user* `DATA`, per corpus case, per
//!   implementation. Cyclone DDS 11.0.1, Fast DDS 3.6.1, OpenDDS 3.34.0,
//!   RTI Connext 7.7.0 and two ROS 2 RMW stacks.
//! * **`rtps-sessions/`** is `capture-rtps-session.sh` — a whole
//!   session of one implementation on a domain of its own, so the SPDP
//!   participant announcement, the SEDP publication and subscription
//!   announcements, the `HEARTBEAT` a reliable writer sends and the
//!   `ACKNACK` its reader answers with are in it as well.
//!
//! What is checked, per datagram:
//!
//! * it decodes;
//! * re-encoding it gives back the **same octets**, so no field was read
//!   loosely and written differently, and none was dropped;
//! * the datagram is consumed whole — `decode_datagram` walks to the
//!   last octet or fails, so a submessage that was skipped shows up as a
//!   length mismatch rather than as silence.
//!
//! Byte identity is the load-bearing assertion, and RTPS earns it: a
//! datagram has no alignment fill between submessages (§8.3.3.2.3 makes
//! every submessage a whole number of four-octet words) and
//! `serializedPayload` is carried verbatim, so unlike the GIOP captures
//! there is no octet whose value the spec leaves open. Every difference
//! is a difference.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram, encode_datagram};
use zerodds_rtps::error::WireError;
use zerodds_rtps::submessages::set_accept_empty_acknack_base_zero;

/// The ACKNACK setting is one flag for the process; the tests that read it
/// run one at a time and put it back.
static ACKNACK_SETTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EmptyAcknackBaseZero<'a>(#[allow(dead_code)] std::sync::MutexGuard<'a, ()>);

impl<'a> EmptyAcknackBaseZero<'a> {
    fn set(accept: bool) -> Self {
        let guard = ACKNACK_SETTING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_accept_empty_acknack_base_zero(accept);
        Self(guard)
    }
}

impl Drop for EmptyAcknackBaseZero<'_> {
    fn drop(&mut self) {
        set_accept_empty_acknack_base_zero(false);
    }
}

fn capture_root(set: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/wire-capture")
        .join(set)
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "a hex datagram has an even length");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

/// What one capture directory contained.
#[derive(Default, Debug)]
struct Tally {
    datagrams: usize,
    submessages: BTreeMap<&'static str, usize>,
    /// Submessage ids this crate does not decode, with the total number
    /// of body octets each carried. Both halves matter: the id says
    /// which class went uninterpreted, and the octet count says how
    /// much content a re-encoder would have dropped if `Unknown` did
    /// not keep its body.
    undecoded: BTreeMap<u8, (usize, usize)>,
}

fn kind_of(sub: &ParsedSubmessage) -> &'static str {
    match sub {
        ParsedSubmessage::Data(_) => "DATA",
        ParsedSubmessage::DataFrag(_) => "DATA_FRAG",
        ParsedSubmessage::Heartbeat(_) => "HEARTBEAT",
        ParsedSubmessage::HeartbeatFrag(_) => "HEARTBEAT_FRAG",
        ParsedSubmessage::AckNack(_) => "ACKNACK",
        ParsedSubmessage::NackFrag(_) => "NACK_FRAG",
        ParsedSubmessage::Gap(_) => "GAP",
        ParsedSubmessage::HeaderExtension(_) => "HEADER_EXTENSION",
        ParsedSubmessage::InfoSource(_) => "INFO_SRC",
        ParsedSubmessage::InfoReply(_) => "INFO_REPLY",
        ParsedSubmessage::InfoTimestamp(_) => "INFO_TS",
        ParsedSubmessage::InfoDestination(_) => "INFO_DST",
        ParsedSubmessage::Unknown { .. } => "UNDECODED",
        _ => "OTHER",
    }
}

/// The offsets at which two datagrams differ. A length mismatch is
/// reported as the offsets past the shorter of the two, so a datagram
/// written four octets short is not silently compared over its prefix.
fn differences(ours: &[u8], theirs: &[u8]) -> Vec<usize> {
    let mut out: Vec<usize> =
        (ours.len().min(theirs.len())..ours.len().max(theirs.len())).collect();
    out.extend((0..ours.len().min(theirs.len())).filter(|&i| ours[i] != theirs[i]));
    out.sort_unstable();
    out
}

/// Decodes and re-encodes every datagram of every capture under
/// `<set>/<vendor>`, and reports what was in them.
fn walk(set: &str, vendor: &str) -> Tally {
    let dir = capture_root(set).join(vendor);
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

    let mut tally = Tally::default();
    for path in entries {
        let text = fs::read_to_string(&path).unwrap();
        for (line_no, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let original = hex_to_bytes(line);
            let parsed = decode_datagram(&original).unwrap_or_else(|e| {
                panic!(
                    "{}:{}: refused: {e:?}\n  {original:02x?}",
                    path.display(),
                    line_no + 1
                )
            });
            for frame in &parsed.submessages {
                *tally
                    .submessages
                    .entry(kind_of(&frame.submessage))
                    .or_default() += 1;
                if let ParsedSubmessage::Unknown { id, body, .. } = &frame.submessage {
                    let seen = tally.undecoded.entry(*id).or_default();
                    seen.0 += 1;
                    seen.1 += body.len();
                }
            }
            let re = encode_datagram(&parsed).unwrap_or_else(|e| {
                panic!(
                    "{}:{}: re-encode refused: {e:?}",
                    path.display(),
                    line_no + 1
                )
            });
            let where_it_differs = differences(&re, &original);
            assert!(
                where_it_differs.is_empty(),
                "{}:{} is not written back the way it arrived — octets \
                 {where_it_differs:?}\n  ours: {re:02x?}\n theirs: {original:02x?}\n \
                 parsed: {parsed:?}",
                path.display(),
                line_no + 1,
            );
            tally.datagrams += 1;
        }
    }
    tally
}

/// The count is pinned per capture set so that a capture directory that
/// loses its files fails instead of passing an assertion it never ran.
fn check(set: &str, vendor: &str, datagrams: usize, expected: &[(&str, usize)]) {
    let tally = walk(set, vendor);
    assert_eq!(
        tally.datagrams, datagrams,
        "{set}/{vendor}: datagram count moved — {tally:?}"
    );
    for (kind, count) in expected {
        assert_eq!(
            tally.submessages.get(kind).copied().unwrap_or(0),
            *count,
            "{set}/{vendor}: {kind} count moved — {tally:?}"
        );
    }
    assert_eq!(
        tally.submessages.get("OTHER").copied().unwrap_or(0),
        0,
        "{set}/{vendor}: a submessage class this crate does not name — {tally:?}"
    );
    // A class that goes undecoded is only allowed where the expectation
    // says so, and the expectation has to say how many.
    if !expected.iter().any(|(kind, _)| *kind == "UNDECODED") {
        assert_eq!(
            tally.submessages.get("UNDECODED").copied().unwrap_or(0),
            0,
            "{set}/{vendor}: a submessage went uninterpreted — {tally:?}"
        );
    }
}

// ---------------------------------------------------------------------
// The corpus run: user DATA, per corpus case, per implementation.
// ---------------------------------------------------------------------

/// Eclipse Cyclone DDS 11.0.1, protocol 2.5, vendor id 01.10.
#[test]
fn cyclone_datagrams_re_encode_byte_for_byte() {
    check(
        "frames",
        "cyclone",
        1015,
        &[
            ("DATA", 1144),
            ("HEARTBEAT", 1151),
            ("ACKNACK", 53),
            ("INFO_TS", 1144),
            ("INFO_DST", 145),
        ],
    );
}

/// eProsima Fast DDS 3.6.1, protocol 2.3, vendor id 01.0f. The only one
/// of the six that appends a submessage this crate does not decode.
///
/// Its id is `0x80`, which §8.3.7.3 gives to `HeaderExtension` from
/// RTPS 2.5 on and §8.3.3.2 leaves to the vendor below it, and the
/// capture says it is the vendor's: the header announces protocol 2.3,
/// the submessage is the **last** one and not the first as §8.3.7.3
/// requires, and its flag byte is `0x01` — no `L`, `W`, `U`, `V`,
/// checksum or `P` — under which a `HeaderExtension` body would be
/// empty. It is 56 octets long.
///
/// So the version gate in `decode_datagram` reads it the way the
/// message asks and this is not a decoding defect. It is the case that
/// made `Unknown` keep its body: 1,338 submessages times 56 octets is
/// 74,928 octets that a decode-and-re-encode would have dropped
/// without a word.
#[test]
fn fastdds_datagrams_re_encode_byte_for_byte() {
    check(
        "frames",
        "fastdds",
        1338,
        &[
            ("DATA", 1338),
            ("INFO_TS", 1338),
            ("INFO_DST", 1338),
            ("UNDECODED", 1338),
        ],
    );
    let tally = walk("frames", "fastdds");
    assert_eq!(
        tally.undecoded.get(&0x80).copied(),
        Some((1338, 74_928)),
        "the undecoded submessages are 0x80 and they carry a body"
    );
}

/// OpenDDS 3.34.0, protocol 2.4, vendor id 01.03. The only one of the
/// six that writes `octetsToNextHeader = 0` on a last submessage — 51
/// counted in its session capture — and the one that announces every
/// instance with a `D=0, K=1` DATA.
#[test]
fn opendds_datagrams_re_encode_byte_for_byte() {
    check(
        "frames",
        "opendds",
        1043,
        &[("DATA", 1043), ("INFO_TS", 1043), ("INFO_DST", 129)],
    );
}

/// RTI Connext 7.7.0, protocol 2.5, vendor id 01.01.
#[test]
fn rti_datagrams_re_encode_byte_for_byte() {
    check(
        "frames",
        "rti",
        966,
        &[
            ("DATA", 966),
            ("HEARTBEAT", 123),
            ("INFO_TS", 966),
            ("INFO_DST", 123),
        ],
    );
}

/// `rmw_cyclonedds_cpp` 1.3.4 over Cyclone 0.10.5 — protocol 2.1, which
/// is what makes it a different reading of submessage id `0x80` from
/// the Fast DDS capture's.
#[test]
fn ros_cyclone_datagrams_re_encode_byte_for_byte() {
    check(
        "frames",
        "ros-cyclone",
        336,
        &[
            ("DATA", 504),
            ("HEARTBEAT", 432),
            ("ACKNACK", 66),
            ("INFO_TS", 504),
            ("INFO_DST", 134),
        ],
    );
}

/// `rosidl_typesupport_fastrtps_c` 2.2.2 over Fast DDS 2.6.10,
/// protocol 2.2.
#[test]
fn ros_fastrtps_datagrams_re_encode_byte_for_byte() {
    check(
        "frames",
        "ros-fastrtps",
        318,
        &[("DATA", 333), ("INFO_TS", 333), ("INFO_DST", 318)],
    );
}

// ---------------------------------------------------------------------
// The session captures: everything one implementation put on a socket
// for the length of a run, discovery included.
// ---------------------------------------------------------------------

/// Cyclone DDS 11.0.1, a `ddsperf` pair on a domain of its own: five
/// SPDP announcements, 25 SEDP publication and 17 SEDP subscription
/// announcements, 25 participant-message samples, and the reliable
/// `HEARTBEAT`/`ACKNACK` traffic under 248 samples.
#[test]
fn cyclone_session_re_encodes_byte_for_byte() {
    check(
        "rtps-sessions",
        "cyclone",
        464,
        &[
            ("DATA", 248),
            ("HEARTBEAT", 236),
            ("ACKNACK", 236),
            ("INFO_TS", 248),
            ("INFO_DST", 234),
        ],
    );
}

/// RTI Connext 7.7.0, an `rtiddsping` pair. The only capture in either
/// set that carries a `GAP`.
#[test]
fn rti_session_re_encodes_byte_for_byte() {
    let _setting = EmptyAcknackBaseZero::set(true);
    check(
        "rtps-sessions",
        "rti",
        75,
        &[
            ("DATA", 29),
            ("HEARTBEAT", 21),
            ("ACKNACK", 26),
            ("GAP", 1),
            ("INFO_TS", 29),
            ("INFO_DST", 48),
        ],
    );
}

/// Fast DDS 2.6.10 under `rmw_fastrtps_cpp`, a `ros2 topic` pair and the
/// `_ros2daemon` beside them. Protocol 2.2, and no submessage `0x80` —
/// the one the corpus capture of Fast DDS 3.6.1 appends to every
/// datagram is the newer stack's.
#[test]
fn fastdds_session_re_encodes_byte_for_byte() {
    check(
        "rtps-sessions",
        "fastdds",
        93,
        &[
            ("DATA", 35),
            ("HEARTBEAT", 26),
            ("ACKNACK", 34),
            ("INFO_TS", 35),
            ("INFO_DST", 67),
        ],
    );
}

/// OpenDDS 3.34.0, the DevGuide `Messenger` over RTPS discovery. Its
/// ACKNACK count is higher than anyone else's because OpenDDS answers
/// every heartbeat.
#[test]
fn opendds_session_re_encodes_byte_for_byte() {
    let _setting = EmptyAcknackBaseZero::set(true);
    check(
        "rtps-sessions",
        "opendds",
        126,
        &[
            ("DATA", 51),
            ("HEARTBEAT", 100),
            ("ACKNACK", 126),
            ("GAP", 2),
            ("INFO_TS", 44),
            ("INFO_DST", 82),
        ],
    );
}

/// OpenDDS 3.34.0 and RTI Connext 7.7.0 answer a HEARTBEAT with an ACKNACK
/// whose `readerSNState` is `0/0:` (bitmapBase 0, numBits 0) while they hold
/// nothing. DDSI-RTPS 2.5 §9.4.2.6 requires "bitmapBase >= 1", §8.3.5.5
/// "minimum(SequenceNumberSet) >= 1", so by default such an ACKNACK is
/// invalid and, by §8.3.4.1 rule 6, so is the rest of its message. The
/// sessions of the two peers hold 40 and 8 of them; the Cyclone DDS and
/// Fast DDS sessions hold none. The two session tests above read the same
/// frames with the setting
/// `wire.accept_empty_acknack_base_zero` on and re-encode them byte for byte.
#[test]
fn the_empty_acknack_of_opendds_and_rti_is_invalid_by_default() {
    let _setting = EmptyAcknackBaseZero::set(false);
    for (vendor, at_least) in [("opendds", 1usize), ("rti", 1)] {
        let path = capture_root("rtps-sessions")
            .join(vendor)
            .join("session.frames");
        let text = fs::read_to_string(&path).unwrap();
        let mut refused = 0usize;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let parsed = decode_datagram(&hex_to_bytes(line.trim())).unwrap();
            if let Some(WireError::ValueOutOfRange { message }) = &parsed.invalid_rest {
                assert!(
                    message.contains("SequenceNumberSet.bitmapBase below 1"),
                    "{vendor}: another cause: {message}"
                );
                refused += 1;
            }
        }
        assert!(
            refused >= at_least,
            "{vendor}: {refused} messages with an invalid rest, expected {at_least}"
        );
    }
}

/// Neither HEARTBEAT `firstSN` 0, nor a DATA_FRAG with `fragmentSize` above
/// `dataSize`, nor any other condition of the "Validity" clauses of §8.3.8
/// is in any of the four recorded sessions: with the ACKNACK setting on, no
/// datagram of them has an invalid rest.
#[test]
fn the_recorded_sessions_hold_no_other_invalid_submessage() {
    let _setting = EmptyAcknackBaseZero::set(true);
    for vendor in ["cyclone", "fastdds", "opendds", "rti"] {
        let path = capture_root("rtps-sessions")
            .join(vendor)
            .join("session.frames");
        let text = fs::read_to_string(&path).unwrap();
        for (n, line) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            let parsed = decode_datagram(&hex_to_bytes(line.trim())).unwrap();
            assert_eq!(
                parsed.invalid_rest,
                None,
                "{vendor}:{}: {:?}",
                n + 1,
                parsed.invalid_rest
            );
        }
    }
}
