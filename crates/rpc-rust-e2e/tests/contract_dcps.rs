// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rpc-rust-e2e` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`: `src/lib.rs` includes the DDS-RPC
//! bindings `build.rs` generates (`emit::interface_rust`,
//! `emit::type_pair_rust` of `zerodds-lang-rules`). The generated code
//! - implements `zerodds_dcps::dds_type::DdsType` for each topic type with
//!   `TYPE_NAME`, `encode` and `decode` only (a `zerodds_wire::Writer` /
//!   `Reader` with MAXALIGN 4, little-endian, the `WireError` turned into
//!   `dds_type::EncodeError` / `DecodeError` by `?`), and for a type with
//!   the classic-CDR pair also `encode_xcdr1`, `decode_xcdr1`,
//!   `encode_continuing` and `decode_continuing`; every other item keeps the
//!   trait's default;
//! - takes `&zerodds_dcps::participant::DomainParticipant` in the requester
//!   and replier constructors of a client and of a type pair, and hands it
//!   to `zerodds-rpc`.
//!
//! What it relies on: the defaults of `DdsType` refuse what the generated
//! type does not declare (big-endian, XCDR1, an offset off the alignment
//! cap, a key holder) rather than writing or reading another form; a type
//! that declares nothing else is a keyless `@final` type without a
//! `TypeObject`; `?` keeps a DISCARD outcome of `@try_construct` apart from
//! a broken stream.
//!
//! The tests implement `DdsType` for a local type the way the generated
//! bridge does; the generated types themselves are exercised by this crate's
//! other targets, which need a running participant.
//!
//! Oracles: DDS-XTypes 1.3 §7.2.4.1 ("the default behavior is to discard the
//! O2 object to avoid misinterpretation"), §7.3.1.2.1.12 (DISCARD is the
//! default of `@try_construct`), §7.3.4.2 (`TK_NONE`: no identifier
//! provided), §7.4.3.2.2 Table 37 (MAXALIGN 8 under XCDR1, 4 under XCDR2),
//! Table 60 (XCDR1 and XCDR2 are different streams), §7.6.8 (only a keyed
//! type has a KeyHash), and the rustdoc of `DdsType::{EXTENSIBILITY, HAS_KEY,
//! TYPE_IDENTIFIER, type_object, encode_be, decode_be, encode_xcdr1,
//! decode_xcdr1, encode_xcdr1_be, decode_xcdr1_be, encode_continuing,
//! decode_continuing, decode_key_holder_be}` and of `DecodeError::Discarded`.
//! No socket is opened.
#![allow(clippy::unwrap_used)]

use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError, Extensibility};
use zerodds_types::TypeIdentifier;
use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `demo::Calculator_add_In` as the bridge declares a topic type: two
/// `long`, the inherent codec over a `Writer` / `Reader`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AddIn {
    a: i32,
    b: i32,
}

impl AddIn {
    fn write(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_i32(self.a);
        w.write_i32(self.b);
        Ok(())
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            a: r.read_i32()?,
            b: r.read_i32()?,
        })
    }
}

impl DdsType for AddIn {
    const TYPE_NAME: &'static str = "demo::Calculator_add_In";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let mut w = Writer::with_max_align(Endianness::Little, 4);
        self.write(&mut w)?;
        out.extend_from_slice(&w.finish());
        Ok(())
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::with_max_align(bytes, Endianness::Little, 4);
        Ok(Self::read(&mut r)?)
    }
}

/// `a = 1, b = 2`, PLAIN_CDR2 little-endian.
const ONE_TWO: [u8; 8] = [1, 0, 0, 0, 2, 0, 0, 0];

fn one_two() -> AddIn {
    AddIn { a: 1, b: 2 }
}

#[test]
fn a_topic_type_that_declares_nothing_else_is_a_keyless_final_type_without_a_type_object() {
    assert_eq!(AddIn::type_name(), "demo::Calculator_add_In");
    // `EXTENSIBILITY`: "Default `Final`"; `HAS_KEY`: "Default `false`".
    assert_eq!(AddIn::extensibility(), Extensibility::Final);
    assert!(!AddIn::has_key());
    // `TYPE_IDENTIFIER`: "Default `TypeIdentifier::None` signals 'type-id
    // not provided'"; `type_object`: "Default `None`".
    assert_eq!(AddIn::type_identifier(), TypeIdentifier::None);
    assert!(AddIn::type_object().is_none());
    assert!(AddIn::type_object_dependencies().is_empty());
    // §7.6.8: a keyless type has no KeyHash.
    assert_eq!(one_two().compute_key_hash(), None);
    let mut out = Vec::new();
    one_two().encode(&mut out).unwrap();
    assert_eq!(out, ONE_TWO);
    assert_eq!(AddIn::decode(&ONE_TWO).unwrap(), one_two());
}

#[test]
fn what_the_bridge_does_not_declare_is_refused_not_written_in_another_form() {
    // XCDR1 is another stream than XCDR2 (Table 60); "A type that cannot be
    // written as XCDR1 must say so; it must not be silently written as
    // something else."
    let mut out = Vec::new();
    assert!(matches!(
        one_two().encode_xcdr1(&mut out),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(matches!(
        AddIn::decode_xcdr1(&ONE_TWO),
        Err(DecodeError::Invalid { .. })
    ));
    assert!(matches!(
        one_two().encode_xcdr1_be(&mut out),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(matches!(
        AddIn::decode_xcdr1_be(&ONE_TWO),
        Err(DecodeError::Invalid { .. })
    ));
    // Big-endian XCDR2: the defaults delegate only for a byte-order-agnostic
    // type; otherwise every multi-octet field would come back swapped.
    assert!(matches!(
        one_two().encode_be(&mut out),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(matches!(
        AddIn::decode_be(&ONE_TWO),
        Err(DecodeError::Invalid { .. })
    ));
    assert!(out.is_empty(), "a refusal writes nothing");
    // A key holder the type does not read back is refused, not guessed.
    assert!(matches!(
        AddIn::decode_key_holder_be(&[0, 0, 0, 1]),
        Err(DecodeError::Invalid { .. })
    ));
}

#[test]
fn xtypes_7_4_3_2_2_the_default_continuing_codec_keeps_to_maxalign() {
    // Under XCDR2 (MAXALIGN 4) the value continues a prefix whose length is
    // a multiple of four as it stands alone.
    let mut behind_eight = vec![0xee; 8];
    one_two()
        .encode_continuing(&mut behind_eight, false)
        .unwrap();
    assert_eq!(&behind_eight[8..], ONE_TWO.as_slice());
    assert_eq!(
        AddIn::decode_continuing(&behind_eight, 8, false).unwrap(),
        one_two()
    );
    // Anywhere else it "cannot know whether the type has a member that
    // cares", and refuses.
    let mut behind_six = vec![0xee; 6];
    assert!(matches!(
        one_two().encode_continuing(&mut behind_six, false),
        Err(EncodeError::Invalid { .. })
    ));
    assert_eq!(behind_six.len(), 6);
    assert!(matches!(
        AddIn::decode_continuing(&behind_eight, 6, false),
        Err(DecodeError::Invalid { .. })
    ));
    // Under XCDR1 (MAXALIGN 8) a type without the classic-CDR pair is
    // refused at an aligned offset too.
    let mut behind_sixteen = vec![0xee; 16];
    assert!(matches!(
        one_two().encode_continuing(&mut behind_sixteen, true),
        Err(EncodeError::Invalid { .. })
    ));
    assert!(matches!(
        AddIn::decode_continuing(&behind_sixteen, 8, true),
        Err(DecodeError::Invalid { .. })
    ));
    // An offset past the end of the sample is no value.
    assert!(AddIn::decode_continuing(&ONE_TWO, 12, false).is_err());
}

#[test]
fn xtypes_7_2_4_1_the_question_mark_keeps_a_discard_apart_from_a_broken_stream() {
    // A short stream: the generated `decode` reports a failure.
    let short = AddIn::decode(&ONE_TWO[..6]).unwrap_err();
    assert!(matches!(short, DecodeError::Invalid { .. }));
    assert!(!short.is_discarded());
    // `@try_construct(DISCARD)`, the default (§7.3.1.2.1.12): the sample is
    // dropped, the stream is not broken.
    let discarded = DecodeError::from(WireError::Discarded {
        at: 4,
        expected: "string<8>".into(),
        found: "12 characters".into(),
    });
    assert!(discarded.is_discarded());
    assert!(matches!(discarded, DecodeError::Discarded { .. }));
    for broken in [
        WireError::Truncated {
            at: 0,
            needed: 4,
            available: 2,
        },
        WireError::ImpossibleLength {
            at: 0,
            claimed: 100,
            available: 4,
        },
        WireError::NotUtf8 { at: 4 },
        WireError::OutOfRange {
            at: 0,
            expected: "an enumerator".into(),
        },
    ] {
        assert!(
            !DecodeError::from(broken.clone()).is_discarded(),
            "{broken:?}"
        );
        assert!(matches!(
            EncodeError::from(broken),
            EncodeError::Invalid { .. }
        ));
    }
}
