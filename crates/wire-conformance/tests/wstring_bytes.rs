// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The octets the generated `wstring` codec puts on the wire.
//!
//! Everything here runs generated code, as `bytes.rs` and
//! `map_bytes.rs` do. The expected octets are a **recorded foreign
//! measurement** and not a second sequence this project wrote itself.
//!
//! # The foreign measurements
//!
//! `tests/interop-results/sources/vendor-corpus.tsv`, four rows:
//!
//! ```text
//! wstring-unbounded-utf8  cyclone  xcdr1  le  wrote  0e000000e400f600fc002000341220005a000000ffffffff
//! wstring-unbounded-utf8  opendds  xcdr1  be  wrote  0000000e00e400f600fc002012340020005a0000ffffffff
//! wstring-bounded-utf8    opendds  xcdr1  le  wrote  060000003412e4005a000000ffffffff
//! wstring-bounded-utf8    opendds  xcdr1  be  wrote  00000006123400e4005a0000ffffffff
//! ```
//!
//! The IDL is `tests/interop-corpus/wstring-unbounded-utf8/type.idl` —
//! `@final struct Sample { wstring text; long tail; }` — and its sample
//! is that case's `values.json`: `text = "äöü ሴ Z"`, `tail = -1`. The
//! bounded case is `wstring<16>` and `text = "ሴäZ"`.
//!
//! OpenDDS carries the big-endian half because **Cyclone's big-endian
//! row is not this cell's**: Cyclone swaps the length word and does not
//! swap the content, so its `0000000e e400f600…` is a big-endian length
//! over little-endian units — `WstringUnitOrder::HostUnswapped`, which
//! no dialect takes as a default because a default has to be the form a
//! conforming reader gets right. That divergence is held against
//! Cyclone's own octets in `crates/wire/tests/wstring.rs`.
//!
//! Cyclone's bounded row is `refused — the fill: error: assignment to
//! expression with array type`, which is `idlc`'s own output and not a
//! wire finding.
//!
//! # What the rows settle
//!
//! - **The length counts octets and not characters.** `0e000000` for
//!   seven characters over two-octet units. §7.4.1.1.2: "the number of
//!   bytes in the CDR buffer taken by the String<Char16> characters.
//!   This is twice the number of characters in the string."
//! - **No terminator.** The run ends at the seventh unit, `5a00`, and
//!   the `0000` behind it is alignment padding for the `long tail` —
//!   which the big-endian row proves, because there the padding is at
//!   the same two offsets and the units are the other way round.
//! - **Two-octet units, in the stream's order.** `e400` little-endian
//!   against `00e4` big-endian, for the same character.
//! - **The bound is not on the wire.** `wstring<16>` and an unbounded
//!   `wstring` frame their value identically; the bound is a promise
//!   about the source.
//!
//! # The mutations
//!
//! | mutation | red |
//! |---|---|
//! | C1's arms swapped in `Writer::write_wstring` — a character count where the cell says octets | both unbounded tests at byte 0: `07000000` where the row has `0e000000` |
//! | C2's `units.push(0)` reached under `WstringTerminator::Absent` | both unbounded tests at byte 4 of the length and two octets longer |
//! | C3 read as four octets — `WstringUnitWidth::Four` in the emitted cell | both unbounded tests at byte 0: `1c000000` |
//! | `zerodds_wire::WirePosition::Element` emitted for the member | no red **by design** — the nine measured implementations answer the two positions alike, which `wstring.rs::the_two_positions_still_agree` states as a measurement |
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire_conformance::generated::{WstrBounded, WstrUtf8};
use zerodds_wire_conformance::{Endianness, Reader, WireError, Writer};

/// A hex string as octets, so a recorded row can be quoted verbatim.
fn octets(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "an odd number of hex digits");
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("a hex pair"))
        .collect()
}

/// The octets one generated `encode` produces, in `endianness`.
fn encoded(
    endianness: Endianness,
    f: impl FnOnce(&mut Writer) -> Result<(), WireError>,
) -> Vec<u8> {
    let mut w = Writer::with_max_align(endianness, 8);
    f(&mut w).expect("every value here fits its member");
    w.finish()
}

/// The first differing offset, because a wrong length or a swapped unit
/// is one offset and a message that prints two arrays does not say
/// which.
fn same(ours: &[u8], theirs: &[u8], who: &str) {
    if let Some(at) = (0..ours.len().min(theirs.len())).find(|at| ours[*at] != theirs[*at]) {
        panic!(
            "byte {at}: this column writes {:#04x} where {who} wrote {:#04x}\n  ours: \
             {ours:02x?}\n  theirs: {theirs:02x?}",
            ours[at], theirs[at]
        );
    }
    assert_eq!(
        ours.len(),
        theirs.len(),
        "the two agree on every shared byte and not on how many there are\n  ours: \
         {ours:02x?}\n  theirs: {theirs:02x?}"
    );
}

/// `tests/interop-corpus/wstring-unbounded-utf8/values.json`.
fn unbounded_sample() -> WstrUtf8 {
    WstrUtf8 {
        text: "äöü ሴ Z".to_string(),
        tail: -1,
    }
}

/// `tests/interop-corpus/wstring-bounded-utf8/values.json`.
fn bounded_sample() -> WstrBounded {
    WstrBounded {
        text: "ሴäZ".to_string(),
        tail: -1,
    }
}

#[test]
fn the_unbounded_octets_are_the_recorded_cyclone_and_opendds_rows() {
    same(
        &encoded(Endianness::Little, |w| unbounded_sample().encode(w)),
        &octets("0e000000e400f600fc002000341220005a000000ffffffff"),
        "Cyclone",
    );
    same(
        &encoded(Endianness::Big, |w| unbounded_sample().encode(w)),
        &octets("0000000e00e400f600fc002012340020005a0000ffffffff"),
        "OpenDDS",
    );
}

#[test]
fn the_bounded_octets_are_the_recorded_opendds_rows() {
    same(
        &encoded(Endianness::Little, |w| bounded_sample().encode(w)),
        &octets("060000003412e4005a000000ffffffff"),
        "OpenDDS",
    );
    same(
        &encoded(Endianness::Big, |w| bounded_sample().encode(w)),
        &octets("00000006123400e4005a0000ffffffff"),
        "OpenDDS",
    );
}

/// The generated decoder reads the vendors' own octets back into the
/// sample the case declares.
///
/// The write side agreeing with a row is half of it: a reader that
/// mirrored a wrong writer would agree with itself. These are the
/// vendors' bytes going in.
#[test]
fn the_generated_decoder_reads_the_recorded_rows_back() {
    let bytes = octets("0e000000e400f600fc002000341220005a000000ffffffff");
    let mut r = Reader::with_max_align(&bytes, Endianness::Little, 8);
    let back = WstrUtf8::decode(&mut r).expect("Cyclone's own octets");
    assert_eq!(back.text, "äöü ሴ Z");
    assert_eq!(back.tail, -1);

    let bytes = octets("00000006123400e4005a0000ffffffff");
    let mut r = Reader::with_max_align(&bytes, Endianness::Big, 8);
    let back = WstrBounded::decode(&mut r).expect("OpenDDS's own octets");
    assert_eq!(back.text, "ሴäZ");
    assert_eq!(back.tail, -1);
}

/// A `wstring<16>` refuses a seventeenth code unit rather than writing
/// it short.
///
/// The bound is not on the wire, so nothing downstream could have seen
/// the overrun. §7.2.2.7's TRIM is a *read*-side behaviour and belongs
/// to the receiver's bound, not the sender's.
#[test]
fn a_bounded_wstring_refuses_a_value_that_overruns_it() {
    let sample = WstrBounded {
        text: "\u{1234}".repeat(17),
        tail: -1,
    };
    let mut w = Writer::with_max_align(Endianness::Little, 8);
    let error = sample
        .encode(&mut w)
        .expect_err("seventeen code units do not fit wstring<16>");
    assert!(
        matches!(error, WireError::OutOfRange { .. }),
        "expected an OutOfRange, got {error:?}"
    );
}
