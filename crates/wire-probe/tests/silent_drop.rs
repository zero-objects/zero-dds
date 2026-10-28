// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Regressions for paths that dropped data and reported success.
//!
//! Each test here failed before the fix named at it, and each failed by
//! producing plausible output rather than an error — which is why none
//! of them was found by a test that only checked for `Ok`.
//!
//! They are written at the probe's level, not the planner's: a KeyHash
//! that two distinct instances share is what a user sees, and asserting
//! on the internal member list would have passed for the wrong reason.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, PidFlagPolicy, keyhash::key_members};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl};
use zerodds_wire_probe::value::{Emit, ProbeRequest};

/// The sixteen-byte KeyHash of one sample.
fn key_hash(idl: &str, type_name: &str, values: &serde_json::Value) -> String {
    let ast = model_from_idl(idl, "silent_drop.idl").expect("the model");
    let request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Little)
        .emitting(Emit::KeyHash);
    hex(&encode(&ast, &request, values).expect("the probe"))
}

/// The names of a type's key members, in the order the key holder takes.
fn key_member_names(idl: &str, type_name: &str) -> Vec<String> {
    let ast = model_from_idl(idl, "silent_drop.idl").expect("the model");
    let (id, _) = ast
        .types()
        .find(|(_, node)| node.name == type_name)
        .expect("the type");
    key_members(&ast, id)
        .expect("the key members")
        .iter()
        .filter_map(|node| match ast.resolve_node_id(*node) {
            Some(zerodds_ast::AnyId::Member(member, _)) => {
                ast.member(member).ok().map(|m| m.name.clone())
            }
            _ => None,
        })
        .collect()
}

/// A `@key` member a struct inherits belongs to its key.
///
/// `collect_key_members` walked the type's own member list while the
/// holder plan next to it walked `all_members_of`. The two disagreed,
/// and the shorter one decided what went into the hash: two samples
/// differing only in the base's key member came out as one instance.
/// XTypes 1.3 §7.4.1.1.5.1, DDSI-RTPS 2.5 §9.6.3.8.
///
/// Both keys sit in the base: §7.2.2.4.5 gives a derived structure no key
/// fields of its own, so a derived structure's keys are its root base's.
/// This test used `@key` in `Derived`, which lowering now refuses.
#[test]
fn an_inherited_key_member_is_in_the_key_hash() {
    let idl = "
        struct Base { @key long b; @key long d; };
        struct Derived : Base { long e; };
    ";
    assert_eq!(key_member_names(idl, "Derived"), ["b", "d"]);

    // Same `d`, different `b`. Two instances, so two handles.
    let one = key_hash(idl, "Derived", &json!({"b": 1, "d": 9, "e": 0}));
    let two = key_hash(idl, "Derived", &json!({"b": 2, "d": 9, "e": 0}));
    assert_ne!(one, two, "two instances were given one KeyHash");

    // And the base's member leads, because the holder plan writes the
    // inherited members first.
    assert_eq!(one, "00000001000000090000000000000000");
}

/// A typedef is transparent to the key set, as it is to the wire.
///
/// `collect_key_members` returned `Ok(())` for any kind that was not a
/// structure, so an alias contributed nothing and the aliased type's
/// keyed members never joined the key.
#[test]
fn a_key_member_behind_a_typedef_keeps_its_nested_keys() {
    let direct = "
        struct Inner { @key long i; long j; };
        struct Outer { @key Inner a; long b; };
    ";
    let aliased = "
        struct Inner { @key long i; long j; };
        typedef Inner InnerAlias;
        struct Outer { @key InnerAlias a; long b; };
    ";
    assert_eq!(
        key_member_names(direct, "Outer"),
        key_member_names(aliased, "Outer"),
        "the typedef changed the key set"
    );

    // The non-key `j` stays out of the hash on both sides.
    let values = json!({"a": {"i": 1, "j": 7}, "b": 0});
    let other = json!({"a": {"i": 1, "j": 8}, "b": 0});
    assert_eq!(
        key_hash(aliased, "Outer", &values),
        key_hash(aliased, "Outer", &other)
    );
    assert_eq!(
        key_hash(direct, "Outer", &values),
        key_hash(aliased, "Outer", &values)
    );
}

/// A PL_CDR1 parameter is its own block, and its length says so.
///
/// The writer aligned a member's value against the enclosing stream, so
/// a `double` took four bytes of padding *inside* its parameter block,
/// and `finish_pid` counted them: `01000c00` where the three measured
/// implementations all write `01000800`. Nothing was dropped — the
/// declared length was simply wrong, which does the same damage. A
/// reader that trusts it takes four bytes too many and every member
/// after that one is misread.
///
/// The expectation is the `fastdds` row of
/// `crates/ast-layout/tests/fixtures/vendor_bytes.md` §"2. PL_CDR1
/// member headers", measured on the Linux test host, which is why the
/// request names that dialect: the rounding of the trailing `short` and
/// the must-understand sentinel are the recorded policy splits and not
/// this defect. XTypes 1.3 §7.4.1.2.
///
/// The `double` is what the defect showed on, and it is asserted under
/// every dialect below: the interior padding is outside the declared
/// length whatever else the framing does, because all three measured
/// streams write `0008` there.
#[test]
fn a_pl_cdr1_parameter_declares_its_members_own_length() {
    let idl = "@mutable struct Ext { long a; double b; short c; };";
    let values = json!({"a": 7, "b": 1.5, "c": -3});
    let ast = model_from_idl(idl, "silent_drop.idl").expect("the model");
    let request = ProbeRequest::in_dialect(
        "Ext",
        Encoding::PlCdr1,
        Endianness::Little,
        Dialect::FastDds,
    );
    let bytes = encode(&ast, &request, &values).expect("the probe");

    assert_eq!(
        hex(&bytes),
        concat!(
            "00000400",
            "07000000",
            "01000800",
            "000000000000f83f",
            "02000200",
            "fdff",
            "0000",
            "023f0000",
        ),
    );
    assert_eq!(bytes.len(), 32, "the block carried padding it should not");

    // The claim is about the **length**, so the id it follows is
    // derived rather than listed: the short header's id carries the
    // framing's flags — `0100` bare, `01c0` where
    // `PidFlagPolicy::ImplExtensionOnMembers` sets both bits — and a
    // second literal would have to be added for every framing that
    // spells the id differently. The extended header states the length
    // in its own word and the id is four octets away from it.
    for dialect in Dialect::ALL {
        let request =
            ProbeRequest::in_dialect("Ext", Encoding::PlCdr1, Endianness::Little, dialect);
        let text = hex(&encode(&ast, &request, &values).expect("the probe"));
        let expected = if dialect.pl_cdr1_headers().always_extended() {
            "08000000000000000000f83f".to_string()
        } else {
            let flags = match dialect.pid_flags() {
                PidFlagPolicy::ImplExtensionOnMembers => "c0",
                PidFlagPolicy::Bare | PidFlagPolicy::SentinelMustUnderstand => "00",
            };
            format!("01{flags}0800000000000000f83f")
        };
        assert!(
            text.contains(&expected),
            "{dialect:?}: the double's parameter does not declare eight bytes: {text}"
        );
    }
}

#[test]
fn a_value_longer_than_its_string_bound_is_refused_and_not_written() {
    // `silent-drop-sweep.md`'s "Unclear" row 4 and inventory row D6.
    // `write_string` had `let _ = bound;` under a comment claiming the
    // front end caught an overrun — the front end never sees the value.
    // Eight characters went into a `string<4>` whole, and the probe,
    // whose whole purpose is that a difference is loud, said nothing.
    //
    // The bound counts octets for a narrow string and UTF-16 code units
    // for a wide one, which is what the receiving side has room for and
    // what `runtime/python/zerodds_wire/typesupport.py` counts.
    let idl = "@final struct S { string<4> text; wstring<2> w; };";
    let ast = model_from_idl(idl, "bound.idl").expect("the model");
    let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);

    let fits = encode(&ast, &request, &json!({ "text": "abcd", "w": "ab" }));
    assert!(fits.is_ok(), "{fits:?}");

    let over = encode(&ast, &request, &json!({ "text": "abcdefgh", "w": "ab" }))
        .expect_err("eight octets do not fit `string<4>`");
    assert!(format!("{over}").contains("at most 4 octets"), "{over}");

    // Four scalars of two octets each is eight octets, and a `string<4>`
    // holds four. Counting characters would let this one through.
    let wide = encode(&ast, &request, &json!({ "text": "üüüü", "w": "ab" }))
        .expect_err("eight octets of two-byte characters do not fit either");
    assert!(format!("{wide}").contains("at most 4 octets"), "{wide}");

    // One scalar above the Basic Multilingual Plane is two code units,
    // so two of them overrun a `wstring<2>` where two characters do not.
    let units = encode(
        &ast,
        &request,
        &json!({ "text": "ab", "w": "\u{1D11E}\u{1D11E}" }),
    )
    .expect_err("four code units do not fit `wstring<2>`");
    assert!(
        format!("{units}").contains("at most 2 UTF-16 code units"),
        "{units}"
    );
}
