// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Which octets of a stream carry no value, from the writer that put
//! them there.
//!
//! `Writer::align_to` is the one place the walk emits an octet XTypes
//! 1.3 §7.4.1's alignment rules put there instead of a value, and
//! `Writer::recording_padding` makes it note each span. The record is
//! what the interop comparison asks when it has to decide whether a
//! peer's differing octet is padding content or a value — computed from
//! the layout, rather than read off the difference with the peer.
//!
//! Two rewrites move octets after they are written, and both correct
//! the record. Neither is reachable from the interop corpus — no case
//! has a `@mutable` member over 64 KiB and none combines a short length
//! code with padding inside the member — so the corpus cannot check
//! them and the two tests at the foot of this file are where they are
//! checked instead.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_wire::{Endianness, ParameterRole, Writer};

/// Nothing is recorded unless the record is asked for.
///
/// The default has to stay free: every sample this project writes goes
/// through this writer, and a `Vec` push per aligned member would be
/// paid on all of them for one tool's benefit.
#[test]
fn a_writer_records_nothing_until_it_is_asked_to() {
    let mut plain = Writer::little_endian();
    plain.write_u8(1);
    plain.write_u32(2);
    assert!(!plain.is_recording_padding());
    assert!(plain.padding_spans().is_empty());

    let mut recording = Writer::little_endian().recording_padding();
    recording.write_u8(1);
    recording.write_u32(2);
    assert!(recording.is_recording_padding());
    assert_eq!(recording.padding_spans(), [(1, 3)]);
}

/// The span is the alignment gap and stops at the value.
#[test]
fn the_span_is_the_gap_the_alignment_rule_opens() {
    let mut w = Writer::little_endian().recording_padding();
    w.write_u8(0xFF);
    w.write_f64(1.0);
    assert_eq!(w.padding_spans(), [(1, 7)]);
    assert!(
        w.as_bytes()[1..8].iter().all(|octet| *octet == 0),
        "{:?}",
        w.as_bytes()
    );
}

/// An aligned member opens no gap and records no span.
///
/// Otherwise the record would say "padding" at every member boundary,
/// which is the mask being all true and the criterion being none.
#[test]
fn an_already_aligned_member_records_nothing() {
    let mut w = Writer::little_endian().recording_padding();
    w.write_u32(1);
    w.write_u32(2);
    assert!(w.padding_spans().is_empty(), "{:?}", w.padding_spans());
}

/// Every recorded octet is one the writer wrote as zero.
///
/// The weaker of the two criteria the interop comparison uses, checked
/// here against the stronger one: a recorded span that held a value
/// would make the two disagree on every row.
#[test]
fn every_recorded_octet_is_a_zero_the_writer_wrote() {
    let mut w = Writer::little_endian().recording_padding();
    w.write_u8(1);
    w.write_u16(0x0203);
    w.write_u8(4);
    w.write_f64(5.0);
    w.write_u8(6);
    w.write_u32(7);
    let spans = w.padding_spans().to_vec();
    assert!(!spans.is_empty());
    let bytes = w.finish();
    for (at, length) in spans {
        assert!(
            bytes[at..at + length].iter().all(|octet| *octet == 0),
            "span ({at}, {length}) is not zero in {bytes:?}"
        );
    }
}

/// A promoted parameter header moves the padding behind it, and the
/// record moves with it.
///
/// `Writer::promote_pid` splices eight octets between a short parameter
/// header and its value when the value turns out to overrun the short
/// form's sixteen-bit length. Every offset above the splice moves by
/// eight; a record that did not move would name eight octets of the
/// value as padding.
///
/// Not reachable from the interop corpus — no recorded case carries a
/// `@mutable` member over 64 KiB — which is why it is checked here.
#[test]
fn a_promoted_parameter_header_carries_the_record_with_it() {
    let mut w = Writer::little_endian().recording_padding();
    let header = w
        .write_pid(1, ParameterRole::Member, false)
        .expect("a header the plan chose");
    // One octet, then a gap to the next four-octet boundary: the span
    // is at offset 5 while the header is still the short form.
    w.write_u8(0xAB);
    w.align_to(4);
    assert_eq!(w.padding_spans(), [(5, 3)]);
    // Long enough that the short form cannot state the length.
    for _ in 0..70_000_u32 {
        w.write_u8(0xCD);
    }
    w.finish_pid(header);

    let spans = w.padding_spans().to_vec();
    let bytes = w.finish();
    assert_eq!(
        spans.first().copied(),
        Some((13, 3)),
        "the splice moved the value by eight and the record did not follow: {spans:?}"
    );
    assert_eq!(&bytes[12..16], &[0xAB, 0, 0, 0], "{:?}", &bytes[..20]);
    for (at, length) in spans {
        assert!(
            bytes[at..at + length].iter().all(|octet| *octet == 0),
            "span ({at}, {length}) is not zero"
        );
    }
}

/// A shortened EMHEADER moves the value down, and the record with it.
///
/// `shorten_emheader` removes the four-octet NEXTINT when the length
/// turns out to fit a short code. Every offset above it moves down by
/// four; a record that did not move would name four octets of the value
/// as padding and miss the four that are.
///
/// Not reachable from the interop corpus either: `SizedOrNextInt` is
/// the setting that turns the rewrite on, and no case it reaches has
/// alignment padding inside a member whose length is 1, 2, 4 or 8.
#[test]
fn a_shortened_emheader_carries_the_record_with_it() {
    // XCDR2's MAXALIGN, which is the encoding the rewrite is legal in.
    let mut w = Writer::with_max_align(Endianness::Little, 4)
        .with_short_length_codes(true)
        .recording_padding();
    let header = w
        .write_emheader(1, 4, false)
        .expect("a header the plan chose");
    // The member is `octet` then `long`: one octet, three of padding,
    // four of value — a length of eight, which is a short code.
    w.write_u8(0xAB);
    w.write_u32(0x0102_0304);
    assert_eq!(
        w.padding_spans(),
        [(9, 3)],
        "with the NEXTINT still there the gap is at nine"
    );
    w.finish_emheader(header)
        .expect("the member is the length its code declares");

    let spans = w.padding_spans().to_vec();
    let bytes = w.finish();
    assert_eq!(
        spans.first().copied(),
        Some((5, 3)),
        "the NEXTINT was removed and the record did not follow: {spans:?}"
    );
    assert_eq!(bytes.len(), 12, "{bytes:?}");
    assert_eq!(&bytes[4..12], &[0xAB, 0, 0, 0, 0x04, 0x03, 0x02, 0x01]);
    for (at, length) in spans {
        assert!(
            bytes[at..at + length].iter().all(|octet| *octet == 0),
            "span ({at}, {length}) is not zero"
        );
    }
}
