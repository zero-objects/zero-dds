// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bitmask, bitset and alias TypeObjects of DDS-XTypes 1.3 Annex B
//! against the octets two vendors write and hash.
//!
//! Annex B (and `dds-xtypes_typeobject.idl` of DDS-XTypes 1.4 Beta 1,
//! unchanged there) declares `MinimalBitmaskType`, `CompleteBitmaskType`,
//! `MinimalBitsetType`, `CompleteBitsetType`, their headers
//! (`MinimalEnumeratedHeader`, `CompleteEnumeratedHeader`,
//! `Minimal/CompleteBitsetHeader`), their elements
//! (`Minimal/CompleteBitflag`, `Minimal/CompleteBitfield`) and the alias
//! header and body (`MinimalAliasHeader`, `MinimalAliasBody`) as
//! `@appendable`, so each carries a `DHEADER` under XCDR2, the encoding
//! of every TypeObject. The codecs of these five objects wrote none; they
//! were tested by their own round trip, which cannot see that. Measured
//! cost: CycloneDDS 11.0.1 read the `DHEADER` of its own bitmask as
//! `bitmask_flags` and `bit_bound` 0 and refused the OMG `dds-xtypes` case
//! `union_uint32_bitmask32` (XTypes 1.3 Table 15), and it failed to read
//! ZeroDDS' TypeLookup reply for a union over a bitmask ("deserialization
//! failed"), so neither `union_uint32_bitmask32` nor
//! `union_uint32_bitmask16` came to a verdict (OPEN-ITEMS 55, measured on
//! the Linux test host on 2026-11-01).
//!
//! # Where the octets come from
//!
//! Measured on the Linux test host on 2026-11-01 from the IDL in [`IDL`]
//! (`bm.idl`) and `bitset BS { bitfield<3> a; bitfield<5> b; };` (`bs.idl`):
//!
//! * CycloneDDS 11.0.1: `/opt/cyclone/bin/idlc bm.idl`, the
//!   `TYPE_MAP_CDR_bm_*` arrays of `bm.c` (the serialized `TypeMapping`,
//!   §7.6.3.2.1). Cyclone's `idlc` refuses `bs.idl` (syntax error at
//!   `bitset`).
//! * Fast DDS 3.6.1: `fastddsgen` on both files, the generated
//!   `register_*_type_identifier` functions, and the objects of
//!   `TypeObjectRegistry::get_type_objects` serialized with fastcdr in
//!   XCDR2 little endian (the extractor of
//!   `zerodds-examples/idl-conformance/proofs/typeobject`, with the type
//!   list of these two files).
//! * The objects of `types/xml/unions.xml` and `unions_bitmask.xml` of the
//!   OMG suite as CycloneDDS's `eclipse_cyclone-11.0.1_test_main_linux`
//!   sends them in its TypeLookup replies (captured on the wire,
//!   `getTypes` replies of the cases `union_uint32_bitmask32` and
//!   `union_uint32_bitmask16`).
//!
//! For every object the hash beside it is the vendor's own, and
//! `MD5(octets)[0..14]` equals it — the octets are the stream the vendor
//! hashes, `DHEADER` included.
//!
//! The two vendors differ in one field: CycloneDDS writes the bitmask's
//! extensibility into `bitmask_flags` (`IS_FINAL` 0x0001 from `idlc`, whose
//! default extensibility is final, `IS_APPENDABLE` 0x0002 from the XML of
//! its suite client), as DDS-XTypes 1.4 Beta 1 has it ("F, A. No flags ==
//! A"), and refuses a bitmask with neither bit (`ddsi_typewrap.c`,
//! `xt_valid_type_flags`, logged as "invalid type object", measured
//! against ZeroDDS on 2026-11-01); Fast DDS writes 0, the value of
//! DDS-XTypes 1.3 ("Unused. No flags apply"). ZeroDDS writes the
//! extensibility position, as it does for an enumeration
//! (`docs/omg-xtypes-issues.md` §B8). The framing is the same in all three.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_types::from_ast::{complete_type_object, minimal_type_object};
use zerodds_types::qos::TypeConsistencyEnforcement;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_matcher::{TypeMatchResult, TypeMatcher};
use zerodds_types::type_object::TypeObject;
use zerodds_types::type_object::flags::BitmaskTypeFlag;
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, MinimalTypeObject, TypeIdentifier, compute_hash,
};

/// `bm.idl`, the input of both vendors' code generators. Names are
/// load-bearing: the minimal objects hash them, the complete ones carry
/// them.
const IDL: &str = "\
module bm {
  @bit_bound(16) bitmask B_16 { A_FLAG_1, A_FLAG_2 };
  @bit_bound(32) bitmask B_32 { B_FLAG_1, B_FLAG_2, B_FLAG_3 };
  @final struct HoldsBitmasks { B_16 a; B_32 b; };
  typedef long MyLong;
  @final struct HoldsAlias { MyLong x; };
};";

/// The two bitmasks of the OMG suite's `types/xml/unions_bitmask.xml` in
/// IDL. Their flags share names, so they live in two modules; a minimal
/// object carries the hashes of the flag names and not the type's name.
/// Neither says an extensibility, so both are appendable, as in the XML
/// CycloneDDS's suite client reads.
const SUITE_IDL: &str = "\
module m16 { @bit_bound(16) bitmask B_16 { B_FLAG_1, B_FLAG_2 }; };
module m32 { @bit_bound(32) bitmask B_32 { B_FLAG_1, B_FLAG_2, B_FLAG_3 }; };";

/// One serialized TypeObject and the vendor's hash of it.
struct Object {
    vendor: &'static str,
    octets: &'static str,
    hash: &'static str,
}

const FASTDDS_B16_MINIMAL: Object = Object {
    vendor: "Fast DDS 3.6.1",
    octets: "34000000f14100002c0000000000000002000000100000001c0000000200000008000000000000003b4e04f50800000001000000387b1174",
    hash: "ebc462ea62240e3157c178afee04",
};
const FASTDDS_B32_MINIMAL: Object = Object {
    vendor: "Fast DDS 3.6.1",
    octets: "40000000f141000038000000000000000200000020000000280000000300000008000000000000000b231dee080000000100000077f96384080000000200000063ea0231",
    hash: "b6e8a54c099e90683ae6a4fb6b2d",
};
const FASTDDS_B16_COMPLETE: Object = Object {
    vendor: "Fast DDS 3.6.1",
    octets: "5b000000f24100005300000000000000110000001000000009000000626d3a3a425f3136000000003300000002000000130000000000000009000000415f464c41475f3100000000130000000100000009000000415f464c41475f32000000",
    hash: "4ae6c08d335d037be3836cd79e13",
};
const FASTDDS_B32_COMPLETE: Object = Object {
    vendor: "Fast DDS 3.6.1",
    octets: "73000000f24100006b00000000000000110000002000000009000000626d3a3a425f3332000000004b00000003000000130000000000000009000000425f464c41475f3100000000130000000100000009000000425f464c41475f3200000000130000000200000009000000425f464c41475f33000000",
    hash: "576e5f7c0ccd4e8789c340541825",
};
const CYCLONE_B16_MINIMAL: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc",
    octets: "34000000f14100002c0000000100000002000000100000001c0000000200000008000000000000003b4e04f50800000001000000387b1174",
    hash: "81b1f83e9ef401b658b59b1d3816",
};
const CYCLONE_B32_MINIMAL: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc",
    octets: "40000000f141000038000000010000000200000020000000280000000300000008000000000000000b231dee080000000100000077f96384080000000200000063ea0231",
    hash: "220f3262ec6324317f3982d911c2",
};
const CYCLONE_B16_COMPLETE: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc",
    octets: "5b000000f24100005300000001000000110000001000000009000000626d3a3a425f3136000000003300000002000000130000000000000009000000415f464c41475f3100000000130000000100000009000000415f464c41475f32000000",
    hash: "a55445dbd0825658cfca88d8b36a",
};
const CYCLONE_B32_COMPLETE: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc",
    octets: "73000000f24100006b00000001000000110000002000000009000000626d3a3a425f3332000000004b00000003000000130000000000000009000000425f464c41475f3100000000130000000100000009000000425f464c41475f3200000000130000000200000009000000425f464c41475f33000000",
    hash: "bd3fb385e1430430b31e40e3c399",
};
/// `MyLong`: CycloneDDS's `idlc` and Fast DDS write the same octets.
const BOTH_MYLONG_MINIMAL: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc and Fast DDS 3.6.1",
    octets: "0f000000f13000000000000003000000000004",
    hash: "8ea97f4529a6c07cb8096e567bd3",
};
const BOTH_MYLONG_COMPLETE: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc and Fast DDS 3.6.1",
    octets: "25000000f230000013000000000000000b000000626d3a3a4d794c6f6e670000050000000000040000",
    hash: "d2bc1a5dab614b9a5c94da548600",
};
/// `HoldsAlias`, whose member refers to the minimal `MyLong` by its hash.
const CYCLONE_HOLDS_ALIAS_MINIMAL: Object = Object {
    vendor: "CycloneDDS 11.0.1 idlc",
    octets: "31000000f15101000100000000000000210000000100000019000000000000000100f18ea97f4529a6c07cb8096e567bd39dd4e461",
    hash: "042f44023da59761101319b2db10",
};
const FASTDDS_BS_MINIMAL: Object = Object {
    vendor: "Fast DDS 3.6.1",
    octets: "36000000f15300002e000000000000000000000022000000020000000a00000000000000030d0cc175b900000a00000003000000050d92eb5ffe",
    hash: "c8d3c3598c12a1dfa4484a6b3c1d",
};
const FASTDDS_BS_COMPLETE: Object = Object {
    vendor: "Fast DDS 3.6.1",
    octets: "50000000f253000048000000000000000f000000000000000700000062733a3a425300002c000000020000001000000000000000030d000002000000610000001000000003000000050d00000200000062000000",
    hash: "bbfea174120e5c21f3db79944a3b",
};

/// The OMG suite's types as `eclipse_cyclone-11.0.1_test_main_linux`
/// answers `getTypes` for them.
const SUITE_B32: Object = Object {
    vendor: "CycloneDDS 11.0.1 test_main, Test::B_32",
    octets: "40000000f141000038000000020000000200000020000000280000000300000008000000000000000b231dee080000000100000077f96384080000000200000063ea0231",
    hash: "93b82de6bf46a7475888ab2f0148",
};
const SUITE_B16: Object = Object {
    vendor: "CycloneDDS 11.0.1 test_main, Test::B_16",
    octets: "34000000f14100002c0000000200000002000000100000001c0000000200000008000000000000000b231dee080000000100000077f96384",
    hash: "d829554cc57ed3ee99c74dde4216",
};
const SUITE_UNION_BITMASK32: Object = Object {
    vendor: "CycloneDDS 11.0.1 test_main, Test::union_bitmask32",
    octets: "58000000f152020000000000110000001100f193b82de6bf46a7475888ab2f0148000000340000000200000014000000010000000100030001000000020000006dbf9ac214000000020000000100040001000000010000008e683187",
    hash: "444428b62a57ec50776b0980417a",
};
const SUITE_UNION_BITMASK16: Object = Object {
    vendor: "CycloneDDS 11.0.1 test_main, Test::union_bitmask16",
    octets: "58000000f152020000000000110000001100f1d829554cc57ed3ee99c74dde4216000000340000000200000014000000010000000100030001000000020000006dbf9ac214000000020000000100040001000000010000008e683187",
    hash: "3f1f5e976260943cfc127a3ab90c",
};
/// `Test::union_uint32`: the octets CycloneDDS's client and ZeroDDS'
/// client both sent for it in the same captures.
const SUITE_UNION_UINT32: Object = Object {
    vendor: "CycloneDDS 11.0.1 test_main, Test::union_uint32",
    octets: "48000000f1520200000000000300000011000700340000000200000014000000010000000100030001000000020000006dbf9ac214000000020000000100040001000000010000008e683187",
    hash: "396f46dc56effbc37811fb6a874c",
};

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "an odd number of hex digits");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_of(object: &Object) -> EquivalenceHash {
    let bytes = unhex(object.hash);
    let mut out = [0u8; 14];
    out.copy_from_slice(&bytes);
    EquivalenceHash(out)
}

/// Reads the vendor's octets and writes them back. Both directions have
/// to give the vendor's octets, and the hash of the written ones has to
/// be the vendor's hash.
fn read_and_write_back(object: &Object) -> TypeObject {
    let octets = unhex(object.octets);
    let decoded = TypeObject::from_bytes_le(&octets)
        .unwrap_or_else(|e| panic!("{}: does not read: {e:?}", object.vendor));
    let written = decoded.to_bytes_le().expect("encode");
    assert_eq!(
        hex(&written),
        object.octets,
        "{}: written back differently",
        object.vendor
    );
    let hash = compute_hash(&decoded).expect("hash");
    assert_eq!(hex(&hash.0), object.hash, "{}: hash", object.vendor);
    decoded
}

fn model() -> zerodds_ast::Ast {
    model_of(IDL)
}

fn model_of(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "bm.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);
    ast
}

fn type_named(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type named `{name}`"))
}

/// Built from `bm.idl`, the object is the vendor's octet for octet and
/// hashes to the vendor's hash.
fn built_is(ast: &zerodds_ast::Ast, name: &str, minimal: bool, object: &Object) {
    let id = type_named(ast, name);
    let built = if minimal {
        TypeObject::Minimal(minimal_type_object(ast, id).expect("minimal"))
    } else {
        TypeObject::Complete(complete_type_object(ast, id).expect("complete"))
    };
    let octets = built.to_bytes_le().expect("encode");
    assert_eq!(
        hex(&octets),
        object.octets,
        "{name} ({}): not {}'s octets",
        if minimal { "MINIMAL" } else { "COMPLETE" },
        object.vendor
    );
    assert_eq!(
        hex(&compute_hash(&built).expect("hash").0),
        object.hash,
        "{name}: not {}'s hash",
        object.vendor
    );
}

/// The vendor's object, read, with ZeroDDS' `bitmask_flags` in place of
/// the vendor's, is the object built from IDL.
fn same_but_flags(built: TypeObject, vendor: TypeObject, flags: u16) {
    let vendor = match vendor {
        TypeObject::Minimal(MinimalTypeObject::Bitmask(mut b)) => {
            b.bitmask_flags = BitmaskTypeFlag(flags);
            TypeObject::Minimal(MinimalTypeObject::Bitmask(b))
        }
        TypeObject::Complete(CompleteTypeObject::Bitmask(mut b)) => {
            b.bitmask_flags = BitmaskTypeFlag(flags);
            TypeObject::Complete(CompleteTypeObject::Bitmask(b))
        }
        other => panic!("not a bitmask: {other:?}"),
    };
    assert_eq!(built, vendor);
}

/// Annex B: `MinimalBitmaskType`, its header and its flags are
/// `@appendable`. The suite's two bitmasks built from IDL are the octets
/// and hashes CycloneDDS's suite client sends for them; the objects of
/// `idlc` and Fast DDS read and write back to their octets and hashes,
/// and differ from the object built from `bm.idl` in `bitmask_flags`
/// only.
#[test]
fn xtypes_annex_b_minimal_bitmask_is_the_vendors_octets() {
    let suite = model_of(SUITE_IDL);
    built_is(&suite, "B_16", true, &SUITE_B16);
    built_is(&suite, "B_32", true, &SUITE_B32);

    let ast = model();
    for (name, objects, bound, flags, positions) in [
        (
            "B_16",
            [&CYCLONE_B16_MINIMAL, &FASTDDS_B16_MINIMAL],
            16,
            [BitmaskTypeFlag::IS_FINAL, 0],
            vec![0, 1],
        ),
        (
            "B_32",
            [&CYCLONE_B32_MINIMAL, &FASTDDS_B32_MINIMAL],
            32,
            [BitmaskTypeFlag::IS_FINAL, 0],
            vec![0, 1, 2],
        ),
    ] {
        let built = TypeObject::Minimal(
            minimal_type_object(&ast, type_named(&ast, name)).expect("minimal"),
        );
        for (object, vendor_flags) in objects.iter().zip(flags) {
            let read = read_and_write_back(object);
            let TypeObject::Minimal(MinimalTypeObject::Bitmask(b)) = &read else {
                panic!("{}: not a minimal bitmask", object.vendor);
            };
            assert_eq!(b.bit_bound, bound, "{}: bit_bound", object.vendor);
            assert_eq!(
                b.bitmask_flags.0, vendor_flags,
                "{}: bitmask_flags",
                object.vendor
            );
            let got: Vec<u16> = b.flag_seq.iter().map(|f| f.common.position).collect();
            assert_eq!(got, positions, "{}: positions", object.vendor);
            same_but_flags(built.clone(), read, BitmaskTypeFlag::IS_APPENDABLE);
        }
    }
}

/// Annex B: `CompleteBitmaskType`, `CompleteEnumeratedHeader` and
/// `CompleteBitflag` are `@appendable`.
#[test]
fn xtypes_annex_b_complete_bitmask_is_the_vendors_octets() {
    let ast = model();
    for (name, objects, names) in [
        (
            "B_16",
            [&CYCLONE_B16_COMPLETE, &FASTDDS_B16_COMPLETE],
            vec!["A_FLAG_1", "A_FLAG_2"],
        ),
        (
            "B_32",
            [&CYCLONE_B32_COMPLETE, &FASTDDS_B32_COMPLETE],
            vec!["B_FLAG_1", "B_FLAG_2", "B_FLAG_3"],
        ),
    ] {
        let built = TypeObject::Complete(
            complete_type_object(&ast, type_named(&ast, name)).expect("complete"),
        );
        for object in objects {
            let read = read_and_write_back(object);
            let TypeObject::Complete(CompleteTypeObject::Bitmask(b)) = &read else {
                panic!("{}: not a complete bitmask", object.vendor);
            };
            let got: Vec<&str> = b.flag_seq.iter().map(|f| f.detail.name.as_str()).collect();
            assert_eq!(got, names, "{}: flag names", object.vendor);
            same_but_flags(built.clone(), read, BitmaskTypeFlag::IS_APPENDABLE);
        }
    }
}

/// Annex B: `MinimalAliasHeader` (empty) and `MinimalAliasBody` are
/// `@appendable` inside the `@final` `MinimalAliasType`. Both vendors
/// write the same octets for `typedef long MyLong`, and a structure that
/// names the alias refers to it by that hash.
#[test]
fn xtypes_annex_b_minimal_alias_is_the_vendors_octets() {
    let ast = model();
    built_is(&ast, "MyLong", true, &BOTH_MYLONG_MINIMAL);
    built_is(&ast, "MyLong", false, &BOTH_MYLONG_COMPLETE);
    built_is(&ast, "HoldsAlias", true, &CYCLONE_HOLDS_ALIAS_MINIMAL);

    let TypeObject::Minimal(MinimalTypeObject::Alias(a)) =
        read_and_write_back(&BOTH_MYLONG_MINIMAL)
    else {
        panic!("not a minimal alias");
    };
    assert_eq!(
        a.body.common.related_type,
        TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int32)
    );
    read_and_write_back(&BOTH_MYLONG_COMPLETE);
}

/// Annex B: `Minimal/CompleteBitsetType`, their headers and their
/// bitfields are `@appendable`. Only Fast DDS writes a bitset; its
/// octets read with the positions, widths and holders they carry and
/// write back to the same octets and hash.
///
/// Not compared with a bitset built from IDL: Fast DDS gives a
/// `bitfield<3>` without a destination type the holder `TK_UINT8`
/// (0x0d), IDL 4.2 §7.4.13.4.3.2 gives it `octet`, which is `TK_BYTE`
/// (0x02), and that is what `from_ast` writes.
#[test]
fn xtypes_annex_b_bitset_is_the_vendors_octets() {
    let TypeObject::Minimal(MinimalTypeObject::Bitset(m)) =
        read_and_write_back(&FASTDDS_BS_MINIMAL)
    else {
        panic!("not a minimal bitset");
    };
    let fields: Vec<(u16, u8, u8)> = m
        .field_seq
        .iter()
        .map(|f| (f.common.position, f.common.bitcount, f.common.holder_type))
        .collect();
    assert_eq!(fields, [(0, 3, 0x0d), (3, 5, 0x0d)]);

    let TypeObject::Complete(CompleteTypeObject::Bitset(c)) =
        read_and_write_back(&FASTDDS_BS_COMPLETE)
    else {
        panic!("not a complete bitset");
    };
    assert_eq!(c.detail.type_name, "bs::BS");
    let names: Vec<&str> = c.field_seq.iter().map(|f| f.detail.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
}

/// XTypes 1.3 §7.2.4.4.4.5 (union) with Table 15: a `uint32`
/// discriminator is assignable to a bitmask of bound 17 to 32 and to no
/// other. The OMG suite expects the same of its pairs:
/// `union_uint32_bitmask32` communicates, `union_uint32_bitmask16` is
/// `INCONSISTENT_TOPIC` on both sides (`test_suite.py` of
/// `omg-dds/dds-xtypes` at `5a6454ad631`).
///
/// The objects are CycloneDDS's, as its client answered `getTypes`, and
/// the policy is the one it announced for its reader (PID 0x0074,
/// `01 00 01 01 01 00 00 00`). This is the decision the ZeroDDS writer
/// took against them and got wrong while the bitmask read as bound 0.
#[test]
fn xtypes_7_2_4_4_4_5_a_uint32_union_against_the_suites_bitmask_unions() {
    let mut registry = TypeRegistry::new();
    for object in [
        &SUITE_B16,
        &SUITE_B32,
        &SUITE_UNION_BITMASK16,
        &SUITE_UNION_BITMASK32,
        &SUITE_UNION_UINT32,
    ] {
        let TypeObject::Minimal(m) = read_and_write_back(object) else {
            panic!("{}: not minimal", object.vendor);
        };
        registry.insert_minimal(hash_of(object), m);
    }
    let tce = TypeConsistencyEnforcement::from_bytes_le(&[0x01, 0x00, 0x01, 0x01, 0x01, 0, 0, 0]);
    let writer = TypeIdentifier::EquivalenceHashMinimal(hash_of(&SUITE_UNION_UINT32));

    let to_32 = TypeMatcher::new(&tce).match_types(
        &writer,
        &TypeIdentifier::EquivalenceHashMinimal(hash_of(&SUITE_UNION_BITMASK32)),
        &registry,
    );
    assert_eq!(to_32, TypeMatchResult::Matches, "union_uint32_bitmask32");

    let to_16 = TypeMatcher::new(&tce).match_types(
        &writer,
        &TypeIdentifier::EquivalenceHashMinimal(hash_of(&SUITE_UNION_BITMASK16)),
        &registry,
    );
    assert!(
        matches!(to_16, TypeMatchResult::Incompatible { .. }),
        "union_uint32_bitmask16: {to_16:?}"
    );
}
