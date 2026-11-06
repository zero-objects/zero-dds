// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `.zddsrec` format against the octets of its specification
//! (`docs/specs/zddsrec-1.0.md`): every value the writer writes is the
//! layout's octets, written out here by hand from the spec, and the reader
//! reads those octets as the values. The section numbers are those of the
//! coverage document `docs/spec-coverage/zddsrec-1.0.md`: §1 the file
//! structure, §2 the indices and `TopicAdd`, §3 the sample kinds, §4 the
//! versions.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_recorder::writer::{header_with, write_all};
use zerodds_recorder::{
    Frame, Header, ReadError, RecordReader, RecordWriter, SampleKind, TopicEntry, WriteError,
};

/// `n` as u32 little endian, then `s`.
fn string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&u32::try_from(s.len()).unwrap().to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn header() -> Header {
    header_with(
        0x0102_0304_0506_0708,
        vec![("talker".to_string(), [0xA1; 16])],
        vec![("/chatter".to_string(), "std_msgs::msg::String".to_string())],
    )
}

/// The header's octets: magic, version 3, time base, the two counts, each
/// participant (GUID, name), each topic (type name, then name).
fn header_octets() -> Vec<u8> {
    let mut o = b"ZDDS".to_vec();
    o.extend_from_slice(&[3, 0, 0, 0]);
    o.extend_from_slice(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    o.extend_from_slice(&[1, 0, 0, 0]);
    o.extend_from_slice(&[1, 0, 0, 0]);
    o.extend_from_slice(&[0xA1; 16]);
    string(&mut o, "talker");
    string(&mut o, "std_msgs::msg::String");
    string(&mut o, "/chatter");
    o
}

fn frame(kind: SampleKind, identifier: Option<[u8; 2]>, payload: &[u8]) -> Frame {
    Frame {
        timestamp_delta_ns: 7,
        participant_idx: 0,
        topic_idx: 0,
        sample_kind: kind,
        representation_identifier: identifier,
        payload: payload.to_vec(),
    }
}

/// A frame's octets: 'F', the time delta, the two indices, the kind,
/// `ReprPresent` and `ReprId` (`00 00` when not known), the length and the
/// payload.
fn frame_octets(kind: u8, identifier: Option<[u8; 2]>, payload: &[u8]) -> Vec<u8> {
    let mut o = vec![b'F'];
    o.extend_from_slice(&7i64.to_le_bytes());
    o.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    o.push(kind);
    match identifier {
        Some(id) => {
            o.push(1);
            o.extend_from_slice(&id);
        }
        None => o.extend_from_slice(&[0, 0, 0]),
    }
    o.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    o.extend_from_slice(payload);
    o
}

#[test]
fn zddsrec_1_a_header_and_its_frames_are_the_octets_of_the_layout() {
    let frames = vec![
        frame(SampleKind::Alive, Some([0x00, 0x09]), &[4, 5, 6]),
        frame(SampleKind::NotAliveDisposed, None, &[]),
    ];
    let mut want = header_octets();
    want.extend(frame_octets(0, Some([0x00, 0x09]), &[4, 5, 6]));
    want.extend(frame_octets(1, None, &[]));
    let written = write_all(Vec::new(), &header(), frames.clone())
        .unwrap()
        .into_inner();
    assert_eq!(written, want);

    let mut r = RecordReader::new(&want);
    assert_eq!(r.parse_header().unwrap(), header());
    assert_eq!(r.version(), 3);
    assert_eq!(r.next_frame().unwrap(), Some(frames[0].clone()));
    assert_eq!(r.next_frame().unwrap(), Some(frames[1].clone()));
    assert_eq!(
        r.next_frame().unwrap(),
        None,
        "the end of the file ends the stream"
    );
}

/// The writer writes the header once and first, and a frame indexes into
/// the header's lists.
#[test]
fn zddsrec_1_the_header_comes_once_and_first_and_a_frame_indexes_into_its_lists() {
    let mut w = RecordWriter::new(Vec::<u8>::new());
    let f = frame(SampleKind::Alive, None, &[]);
    assert!(matches!(w.write_frame(&f), Err(WriteError::HeaderMissing)));
    w.write_header(&header()).unwrap();
    assert!(matches!(
        w.write_header(&header()),
        Err(WriteError::HeaderAlreadyWritten)
    ));
    let past = Frame {
        participant_idx: 1,
        ..f.clone()
    };
    assert!(matches!(
        w.write_frame(&past),
        Err(WriteError::OutOfRangeIdx {
            field: "participant",
            ..
        })
    ));
    let past = Frame { topic_idx: 1, ..f };
    assert!(matches!(
        w.write_frame(&past),
        Err(WriteError::OutOfRangeIdx { field: "topic", .. })
    ));
}

/// A `TopicAdd` is 'T', the type name, then the name; the topic it adds
/// takes the index one past the header's list.
#[test]
fn zddsrec_2_a_topic_add_continues_the_header_list() {
    let mut w = RecordWriter::new(Vec::<u8>::new());
    w.write_header(&header()).unwrap();
    let late = TopicEntry {
        name: "/late".into(),
        type_name: "Late".into(),
    };
    assert_eq!(w.add_topic(&late).unwrap(), 1);
    w.write_frame(&Frame {
        topic_idx: 1,
        ..frame(SampleKind::Alive, Some([0x00, 0x01]), &[9])
    })
    .unwrap();
    let mut want = header_octets();
    want.push(b'T');
    string(&mut want, "Late");
    string(&mut want, "/late");
    let mut f = frame_octets(0, Some([0x00, 0x01]), &[9]);
    f[13..17].copy_from_slice(&1u32.to_le_bytes());
    want.extend(f);
    assert_eq!(w.into_inner(), want);

    let mut r = RecordReader::new(&want);
    r.parse_header().unwrap();
    assert_eq!(r.next_frame().unwrap().unwrap().topic_idx, 1);
    assert_eq!(r.added_topics(), [late]);
}

/// The kinds are 0, 1, 2 (ALIVE, NOT_ALIVE_DISPOSED,
/// NOT_ALIVE_UNREGISTERED); any other value is `BadSampleKind`.
#[test]
fn zddsrec_3_the_sample_kinds_are_0_1_and_2() {
    for (octet, kind) in [
        (0, SampleKind::Alive),
        (1, SampleKind::NotAliveDisposed),
        (2, SampleKind::NotAliveUnregistered),
    ] {
        let mut file = header_octets();
        file.extend(frame_octets(octet, None, &[]));
        let mut r = RecordReader::new(&file);
        r.parse_header().unwrap();
        assert_eq!(r.next_frame().unwrap().unwrap().sample_kind, kind);
        assert_eq!(kind.to_u8(), octet);
    }
    let mut file = header_octets();
    file.extend(frame_octets(99, None, &[]));
    let mut r = RecordReader::new(&file);
    r.parse_header().unwrap();
    assert!(matches!(r.next_frame(), Err(ReadError::BadSampleKind(99))));
}

/// A version 2 file has no `ReprPresent`/`ReprId`: its frame reads with no
/// identifier. A version above 3 is refused.
#[test]
fn zddsrec_4_a_version_2_frame_reads_without_identifier_and_a_later_version_is_refused() {
    let mut file = header_octets();
    file[4] = 2;
    file.push(b'F');
    file.extend_from_slice(&7i64.to_le_bytes());
    file.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    file.push(0);
    file.extend_from_slice(&3u32.to_le_bytes());
    file.extend_from_slice(&[4, 5, 6]);
    let mut r = RecordReader::new(&file);
    r.parse_header().unwrap();
    assert_eq!(r.version(), 2);
    assert_eq!(
        r.next_frame().unwrap(),
        Some(frame(SampleKind::Alive, None, &[4, 5, 6]))
    );
    assert_eq!(r.next_frame().unwrap(), None);

    let mut file = header_octets();
    file[4] = 4;
    assert!(matches!(
        RecordReader::new(&file).parse_header(),
        Err(ReadError::UnsupportedVersion(4))
    ));
}

/// What is not a `.zddsrec` or is cut short is an error and no value: a
/// wrong magic, a file that ends inside the header or inside a frame, an
/// unknown frame magic, and counts that the rest of the file cannot hold
/// (refused before anything is allocated for them).
#[test]
fn zddsrec_1_a_file_that_breaks_the_layout_is_an_error() {
    assert!(matches!(
        RecordReader::new(&[0u8; 32]).parse_header(),
        Err(ReadError::BadMagic)
    ));
    let full = header_octets();
    assert!(matches!(
        RecordReader::new(&full[..full.len() - 1]).parse_header(),
        Err(ReadError::Truncated { .. })
    ));
    let mut file = header_octets();
    let frame = frame_octets(0, Some([0x00, 0x09]), &[4, 5, 6]);
    file.extend_from_slice(&frame[..frame.len() - 1]);
    let mut r = RecordReader::new(&file);
    r.parse_header().unwrap();
    assert!(matches!(r.next_frame(), Err(ReadError::Truncated { .. })));
    let mut file = header_octets();
    file.push(b'X');
    let mut r = RecordReader::new(&file);
    r.parse_header().unwrap();
    assert!(matches!(
        r.next_frame(),
        Err(ReadError::BadFrameMagic(b'X'))
    ));
    for (offset, what) in [(16, "participant_count"), (20, "topic_count")] {
        let mut file = header_octets()[..24].to_vec();
        file[16..24].copy_from_slice(&[0; 8]);
        file[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let res = RecordReader::new(&file).parse_header();
        assert!(
            matches!(res, Err(ReadError::Truncated { what: w, .. }) if w == what),
            "{what}: {res:?}"
        );
    }
}
