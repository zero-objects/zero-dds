// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a derived type does at run time: the checks that depend on the
//! value, the readings XTypes 1.3 gives a reader of another version of the
//! type, the IDL it exports, the special values of DDS 1.4 §2.3.3, the
//! encapsulation identifiers and the KeyHash.
//!
//! That a derived type and the type `zerodds-build` generates from the
//! equivalent IDL are one type — identifier, object, octets in all four
//! encodings, key holder — is held by the generated codec in
//! `crates/zerodds-build/tests/dds_type.rs`
//! (`xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use core::num::NonZeroU32;
use std::time::{Duration, UNIX_EPOCH};

use zerodds_codec::{CodecError, DdsType, Encapsulation, WireError, WireRepr};

/// A call sign: one to six upper-case letters or digits.
#[derive(WireRepr, Debug, Clone, PartialEq)]
#[wire(idl = "string")]
struct CallSign(String);

impl core::fmt::Display for CallSign {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for CallSign {
    type Error = &'static str;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        let fits = (1..=6).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        if fits {
            Ok(Self(text.to_string()))
        } else {
            Err("a call sign of one to six upper-case letters or digits")
        }
    }
}

/// A channel number: never zero, through `TryFrom<u16>` and `Into<u16>`.
#[derive(WireRepr, Debug, Clone, Copy, PartialEq)]
#[wire(idl = "uint16")]
struct Channel(u16);

impl TryFrom<u16> for Channel {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value == 0 {
            Err("a channel other than zero")
        } else {
            Ok(Self(value))
        }
    }
}

impl From<Channel> for u16 {
    fn from(channel: Channel) -> Self {
        channel.0
    }
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "t::Pose", extensibility = "final")]
struct Pose {
    x: f64,
    y: f64,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "t::Unit", extensibility = "appendable")]
struct Unit {
    #[dds(key)]
    id: NonZeroU32,
    call: CallSign,
    channel: Channel,
    #[dds(bound = 8)]
    tag: String,
    pose: Pose,
    note: Option<String>,
    big: u64,
}

/// `t::Unit` with plain IDL types in its members, to write what `Unit`
/// refuses to.
#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "t::Unit", extensibility = "appendable")]
struct UnitPlain {
    #[dds(key)]
    id: u32,
    call: String,
    channel: u16,
    #[dds(bound = 8)]
    tag: String,
    pose: Pose,
    note: Option<String>,
    big: u64,
}

fn unit() -> Unit {
    Unit {
        id: NonZeroU32::new(7).expect("non-zero"),
        call: CallSign("ALPHA1".into()),
        channel: Channel(12),
        tag: "lead".into(),
        pose: Pose { x: 1.5, y: -2.0 },
        note: None,
        big: u64::MAX,
    }
}

fn plain(call: &str, id: u32, channel: u16) -> UnitPlain {
    UnitPlain {
        id,
        call: call.into(),
        channel,
        tag: String::new(),
        pose: Pose { x: 0.0, y: 0.0 },
        note: None,
        big: 0,
    }
}

fn encoded<T: zerodds_codec::DdsSerialize>(value: &T, encapsulation: Encapsulation) -> Vec<u8> {
    let mut out = Vec::new();
    zerodds_codec::encode(value, encapsulation, &mut out).expect("encode");
    out
}

/// The expectation of a `WireError::OutOfRange`.
fn refusal(error: CodecError) -> (usize, String) {
    match error {
        CodecError::Wire(WireError::OutOfRange { at, expected }) => (at, expected),
        other => panic!("a refusal by value, got {other:?}"),
    }
}

/// XTypes 1.3 §7.4.3.5 with IDL 4.2 §7.4.1.4.4.3: a `string<8>` holding
/// nine characters is refused, and nothing of the sample is left behind.
#[test]
fn xtypes_7_4_3_5_a_value_over_its_bound_is_refused_and_writes_nothing() {
    let mut value = unit();
    value.tag = "ninechars".into();
    let mut out = vec![0xee];
    let error = zerodds_codec::encode(&value, Encapsulation::Xcdr2Le, &mut out)
        .expect_err("a string over its bound");
    assert_eq!(out, [0xee], "the refused sample left nothing behind");
    assert!(
        error.to_string().contains('8'),
        "the refusal names the bound: {error}"
    );
}

/// GitHub #33: a received value its Rust type refuses rejects the sample,
/// with the type's reason and the offset the value was read from — a call
/// sign in lower case, a channel of zero, a key of zero for `NonZeroU32`.
#[test]
fn issue_33_a_value_the_rust_type_refuses_rejects_the_sample_with_its_reason() {
    let bytes = encoded(&plain("alpha", 7, 12), Encapsulation::Xcdr2Le);
    let (at, expected) = refusal(
        zerodds_codec::decode::<Unit>(&bytes, Encapsulation::Xcdr2Le).expect_err("lower case"),
    );
    assert!(expected.contains("upper-case"), "{expected}");
    // DHEADER (4), id (4), then the string's length at 8.
    assert_eq!(at, 8, "the offset of the refused value");

    let bytes = encoded(&plain("BRAVO", 7, 0), Encapsulation::Xcdr1Be);
    let (_, expected) = refusal(
        zerodds_codec::decode::<Unit>(&bytes, Encapsulation::Xcdr1Be).expect_err("channel 0"),
    );
    assert!(expected.contains("channel other than zero"), "{expected}");

    let bytes = encoded(&plain("BRAVO", 0, 3), Encapsulation::Xcdr2Be);
    let (_, expected) =
        refusal(zerodds_codec::decode::<Unit>(&bytes, Encapsulation::Xcdr2Be).expect_err("id 0"));
    assert!(expected.contains("zero"), "{expected}");

    // Through the DCPS trait: refused, with the reason.
    let bytes = encoded(&plain("alpha", 7, 12), Encapsulation::Xcdr2Le);
    let error = <Unit as zerodds_dcps::DdsType>::decode(&bytes).expect_err("lower case");
    assert!(error.to_string().contains("upper-case"), "{error}");
}

/// XTypes 1.3 §7.4.3.4.2 and §7.2.2.4.4.4.3: the members of a `@mutable`
/// type are found by their member ids, in whatever order the writer
/// declared them.
#[test]
fn xtypes_7_4_3_4_a_mutable_type_reads_its_members_by_id_in_any_order() {
    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::M", extensibility = "mutable")]
    struct Forward {
        #[dds(id = 1)]
        x: i32,
        #[dds(id = 2)]
        y: String,
        #[dds(id = 3)]
        z: Option<f64>,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::M", extensibility = "mutable")]
    struct Backward {
        #[dds(id = 3)]
        z: Option<f64>,
        #[dds(id = 2)]
        y: String,
        #[dds(id = 1)]
        x: i32,
    }

    for encapsulation in Encapsulation::ALL {
        let forward = Forward {
            x: -4,
            y: "why".into(),
            z: Some(0.25),
        };
        let back: Backward =
            zerodds_codec::decode(&encoded(&forward, encapsulation), encapsulation)
                .unwrap_or_else(|e| panic!("{encapsulation:?}: {e}"));
        assert_eq!(
            back,
            Backward {
                z: Some(0.25),
                y: "why".into(),
                x: -4
            },
            "{encapsulation:?}"
        );
    }
}

/// XTypes 1.3 §7.2.2.4.4.4.2 and Table 19: a reader of an `@appendable`
/// type skips the members a newer writer appended and gives the members
/// an older writer lacks their default.
#[test]
fn xtypes_7_2_2_4_4_4_2_an_appendable_type_reads_a_newer_and_an_older_writer() {
    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::Rec")]
    struct Old {
        a: i32,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::Rec")]
    struct New {
        a: i32,
        b: String,
        c: f64,
    }

    for encapsulation in [Encapsulation::Xcdr2Le, Encapsulation::Xcdr2Be] {
        let new = New {
            a: 3,
            b: "appended".into(),
            c: 9.5,
        };
        let old: Old = zerodds_codec::decode(&encoded(&new, encapsulation), encapsulation)
            .expect("an older reader");
        assert_eq!(old, Old { a: 3 });
        let new: New = zerodds_codec::decode(&encoded(&old, encapsulation), encapsulation)
            .expect("a newer reader");
        assert_eq!(
            new,
            New {
                a: 3,
                b: String::new(),
                c: 0.0
            }
        );
    }
}

/// GitHub #33: the IDL a derived type exports, with every type it reaches
/// declared before its use and a type that reaches itself declared ahead.
#[test]
fn issue_33_a_derived_type_exports_the_idl_it_stands_for() {
    #[derive(DdsType, Debug, Clone, Copy, PartialEq)]
    #[dds(name = "t::Mode", bit_bound = 8)]
    enum Mode {
        #[dds(name = "IDLE")]
        Idle,
        #[dds(name = "BUSY", value = 4)]
        Busy,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::tree::Node", extensibility = "mutable", autoid = "hash")]
    struct Node {
        #[dds(key, id = 7)]
        id: u32,
        #[dds(hashid = "kids")]
        children: Vec<Node>,
        mode: Mode,
        #[dds(bound = 4)]
        names: Vec<String>,
        grid: [[i16; 3]; 2],
        when: Duration,
        r#in: Option<u8>,
    }

    assert_eq!(
        Node::idl().expect("the IDL"),
        "module t { module tree { struct Node; }; };\n\
         module t { @bit_bound(8) enum Mode { IDLE, @value(4) BUSY }; };\n\
         module DDS { @final struct Duration_t { long sec; unsigned long nanosec; }; };\n\
         module t { module tree { @mutable @autoid(HASH) struct Node { @key @id(7) unsigned long id; \
         @hashid(\"kids\") sequence<::t::tree::Node> children; ::t::Mode mode; \
         sequence<string, 4> names; short grid[2][3]; ::DDS::Duration_t when; \
         @optional octet _in; }; }; };\n"
    );
}

/// DDS 1.4 §2.3.3: `Duration::MAX` is `DURATION_INFINITE`
/// (`{0x7fffffff, 0x7fffffff}`), a `Duration` of `0x7fffffff` seconds or
/// more that is not `Duration::MAX` is refused rather than cut, and a
/// `SystemTime` before 1970 has no `Time_t`.
#[test]
fn dds_1_4_2_3_3_duration_infinite_and_the_values_a_time_t_cannot_hold() {
    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::Times", extensibility = "final")]
    struct Times {
        d: Duration,
        t: std::time::SystemTime,
    }

    let infinite = Times {
        d: Duration::MAX,
        t: UNIX_EPOCH,
    };
    let bytes = encoded(&infinite, Encapsulation::Xcdr2Le);
    assert_eq!(
        bytes,
        [
            0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(
        zerodds_codec::decode::<Times>(&bytes, Encapsulation::Xcdr2Le).expect("read back"),
        infinite
    );

    let mut out = Vec::new();
    let too_long = Times {
        d: Duration::from_secs(0x7fff_ffff),
        t: UNIX_EPOCH,
    };
    assert!(zerodds_codec::encode(&too_long, Encapsulation::Xcdr2Le, &mut out).is_err());
    let before = Times {
        d: Duration::ZERO,
        t: UNIX_EPOCH - Duration::from_secs(1),
    };
    assert!(zerodds_codec::encode(&before, Encapsulation::Xcdr2Le, &mut out).is_err());
    assert!(out.is_empty(), "a refused sample writes nothing");
}

/// DDS 1.4 §B.2.1 with XTypes 1.3 §7.6.7.2: the values a content filter
/// compares — a member by name, a nested member by its dotted path, the
/// IDL value a `WireRepr` member writes, `null` for an absent `@optional`
/// member, nothing for an `unsigned long long`, which the filter's `i64`
/// does not hold.
#[test]
fn dds_1_4_b_2_1_a_derived_type_answers_the_content_filter() {
    use zerodds_dcps::FilterValue;
    let value = unit();
    let field = |path| zerodds_dcps::DdsType::field_value(&value, path);
    assert_eq!(field("id"), Some(FilterValue::Int(7)));
    assert_eq!(field("call"), Some(FilterValue::String("ALPHA1".into())));
    assert_eq!(field("channel"), Some(FilterValue::Int(12)));
    assert_eq!(field("pose.y"), Some(FilterValue::Float(-2.0)));
    assert_eq!(field("note"), Some(FilterValue::Null));
    assert_eq!(field("big"), None);
    assert_eq!(field("pose"), None);
    assert_eq!(field("missing"), None);
}

/// XTypes 1.3 §7.6.3.1.2 (Table 60) with DDSI-RTPS 2.5 §10.5: the
/// encapsulation identifier of each encoding and extensibility.
#[test]
fn xtypes_7_6_3_1_2_the_encapsulation_identifiers() {
    use zerodds_codec::codec::Extensibility::{Appendable, Final, Mutable};
    let id = |encapsulation: Encapsulation, extensibility| encapsulation.identifier(extensibility);
    assert_eq!(id(Encapsulation::Xcdr1Be, Final), [0x00, 0x00]);
    assert_eq!(id(Encapsulation::Xcdr1Le, Appendable), [0x00, 0x01]);
    assert_eq!(id(Encapsulation::Xcdr1Be, Mutable), [0x00, 0x02]);
    assert_eq!(id(Encapsulation::Xcdr1Le, Mutable), [0x00, 0x03]);
    assert_eq!(id(Encapsulation::Xcdr2Be, Final), [0x00, 0x06]);
    assert_eq!(id(Encapsulation::Xcdr2Le, Final), [0x00, 0x07]);
    assert_eq!(id(Encapsulation::Xcdr2Be, Appendable), [0x00, 0x08]);
    assert_eq!(id(Encapsulation::Xcdr2Le, Appendable), [0x00, 0x09]);
    assert_eq!(id(Encapsulation::Xcdr2Be, Mutable), [0x00, 0x0a]);
    assert_eq!(id(Encapsulation::Xcdr2Le, Mutable), [0x00, 0x0b]);
    assert_eq!(
        Encapsulation::from_identifier([0x00, 0x09], Appendable),
        Some(Encapsulation::Xcdr2Le)
    );
    assert_eq!(Encapsulation::from_identifier([0x00, 0x09], Final), None);
}

/// XTypes 1.3 §7.6.8 and DDSI-RTPS 2.5 §9.6.3.8: the key holder is the
/// key members in PLAIN_CDR2 big-endian; a holder that never exceeds
/// sixteen octets is the KeyHash zero-padded, a longer one its MD5.
#[test]
fn xtypes_7_6_8_the_key_hash_is_the_holder_or_its_md5() {
    let value = unit();
    assert_eq!(
        zerodds_codec::key_holder(&value).expect("holder"),
        [0, 0, 0, 7]
    );
    let mut expected = [0u8; 16];
    expected[3] = 7;
    assert_eq!(
        zerodds_codec::key_hash(&value).expect("hash"),
        Some(expected)
    );
    assert_eq!(zerodds_dcps::DdsType::key_hash(&value), Some(expected));

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "t::Named", extensibility = "final")]
    struct Named {
        #[dds(key)]
        name: String,
        v: i32,
    }

    let named = Named {
        name: "x".into(),
        v: 1,
    };
    let holder = zerodds_codec::key_holder(&named).expect("holder");
    assert_eq!(holder, [0, 0, 0, 2, b'x', 0]);
    assert_eq!(
        zerodds_codec::key_hash(&named).expect("hash"),
        Some(zerodds_wire::md5(&holder)),
        "an unbounded string key can exceed sixteen octets: MD5"
    );
    let back: Named = zerodds_codec::from_key_holder(&holder).expect("from the holder");
    assert_eq!(
        back,
        Named {
            name: "x".into(),
            v: 0
        }
    );
}
