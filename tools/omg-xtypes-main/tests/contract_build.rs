// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-main` with `zerodds-build`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-build`: its build script compiles the 25
//! types of `interop/shapes/omg-dds-xtypes.idl` with
//! `default_extensibility(DefaultExt::Appendable)` and
//! `dds_type_impls(true)`; `src/shape_type.rs` includes the generated module
//! and drives every type through its `DdsType` impl, whose extensibility,
//! member ids and octets are what the suite measures against the other
//! vendors. The contract includes the same module from `OUT_DIR`.
//!
//! Oracle: the suite's IDL and DDS-XTypes 1.3: §7.3.1.2.1.8 (an
//! unannotated aggregate takes the configured default), §7.3.1.2.1.1 and
//! §7.3.1.2.1.2 (a member without `@id` takes the id after the previous
//! member's, the first one 0), §7.4.3.4.2 (FINAL is the members alone,
//! APPENDABLE a DHEADER before them, MUTABLE a DHEADER and an EMHEADER per
//! member with `M_FLAG` set on a key member and `LC` 2 for a four-octet
//! member), §7.4.3.5.3 (a string).
#![allow(clippy::unwrap_used, clippy::panic, dead_code, missing_docs)]

mod generated {
    include!(concat!(env!("OUT_DIR"), "/omg-dds-xtypes.rs"));
}

use generated::{Shape1Default, Shape1Final, Shape1Mutable, Shape1MutableExplicitID};
use zerodds_dcps::DdsType;
use zerodds_dcps::dds_type::Extensibility;
use zerodds_types::type_object::TypeObject;
use zerodds_types::type_object::complete::CompleteTypeObject;
use zerodds_types::type_object::flags::StructMemberFlag;

/// `RED`: the string's four octets with the NUL and their length.
const RED: [u8; 8] = [4, 0, 0, 0, b'R', b'E', b'D', 0];

fn members<T: DdsType>() -> Vec<(u32, bool)> {
    let Some(TypeObject::Complete(CompleteTypeObject::Struct(object))) = T::type_object() else {
        panic!("{}: no COMPLETE struct object", T::TYPE_NAME);
    };
    object
        .member_seq
        .iter()
        .map(|member| {
            (
                member.common.member_id,
                member.common.member_flags.has(StructMemberFlag::IS_KEY),
            )
        })
        .collect()
}

#[test]
fn xtypes_7_3_1_2_1_8_each_extensibility_is_the_annotations_or_the_configured_default() {
    assert_eq!(
        <Shape1Default as DdsType>::EXTENSIBILITY,
        Extensibility::Appendable
    );
    assert_eq!(
        <Shape1Final as DdsType>::EXTENSIBILITY,
        Extensibility::Final
    );
    assert_eq!(
        <Shape1Mutable as DdsType>::EXTENSIBILITY,
        Extensibility::Mutable
    );
}

#[test]
fn xtypes_7_4_3_4_2_final_is_the_members_and_appendable_puts_a_dheader_before_them() {
    let mut final_ = Vec::new();
    DdsType::encode(
        &Shape1Final {
            color: "RED".into(),
            x: 1,
            y: 2,
            shapesize: 3,
        },
        &mut final_,
    )
    .unwrap();
    let mut expected = RED.to_vec();
    expected.extend_from_slice(&[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(final_, expected);

    let mut appendable = Vec::new();
    DdsType::encode(
        &Shape1Default {
            color: "RED".into(),
            x: 1,
            y: 2,
            shapesize: 3,
        },
        &mut appendable,
    )
    .unwrap();
    let mut delimited = vec![20, 0, 0, 0];
    delimited.extend_from_slice(&expected);
    assert_eq!(appendable, delimited);
}

#[test]
fn xtypes_7_3_1_2_1_2_a_member_without_an_id_takes_the_one_after_the_previous() {
    assert_eq!(
        members::<Shape1Mutable>(),
        [(0, true), (1, false), (2, false), (3, false)]
    );
    assert_eq!(
        members::<Shape1MutableExplicitID>(),
        [(10, true), (20, false), (21, false), (30, false)]
    );
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_member_carries_its_id_and_the_key_carries_the_must_understand_flag() {
    let mut out = Vec::new();
    DdsType::encode(
        &Shape1MutableExplicitID {
            color: "RED".into(),
            x: 1,
            y: 2,
            shapesize: 3,
        },
        &mut out,
    )
    .unwrap();
    let word = |at: usize| u32::from_le_bytes(out[at..at + 4].try_into().unwrap());
    assert_eq!(
        word(0) as usize,
        out.len() - 4,
        "the DHEADER counts what follows it"
    );
    // The EMHEADERs, walked by their length codes (Table 34): LC 0 to 3 a
    // value of 1, 2, 4 or 8 octets; LC 4 to 7 a NEXTINT that counts the
    // member, for LC 4 itself a part of it.
    let mut at = 4;
    let mut seen = Vec::new();
    while at < out.len() {
        let header = word(at);
        let (must, lc, id) = (header >> 31, (header >> 28) & 0x7, header & 0x0fff_ffff);
        at += 4;
        let length = match lc {
            0..=3 => 1 << lc,
            4 => 4 + word(at) as usize,
            5 => 4 + word(at) as usize,
            6 => 4 + 4 * word(at) as usize,
            _ => 4 + 8 * word(at) as usize,
        };
        seen.push((id, must, lc));
        at += length;
        if at < out.len() {
            at = at.next_multiple_of(4);
        }
    }
    assert_eq!(at, out.len(), "the members fill the DHEADER exactly");
    assert_eq!(
        seen.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
        [10, 20, 21, 30]
    );
    assert_eq!(seen[0].1, 1, "the key member must be understood");
    for (id, must, lc) in &seen[1..] {
        assert_eq!(
            (*must, *lc),
            (0, 2),
            "member {id}: a long is LC 2 and not a key"
        );
    }
}

#[test]
fn every_type_of_the_suite_carries_its_type_object_under_its_idl_name() {
    assert_eq!(
        <Shape1MutableExplicitID as DdsType>::TYPE_NAME,
        "Shape1MutableExplicitID"
    );
    assert!(<Shape1Default as DdsType>::type_object().is_some());
    assert!(<Shape1Final as DdsType>::has_key());
}
