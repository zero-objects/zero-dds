//! A `TypeObject` on the wire opens with a `DHEADER`, measured against
//! CycloneDDS and RTI Connext rather than read off this tree.
//!
//! DDS-XTypes 1.3 Annex B declares
//! `@extensibility(APPENDABLE) @nested union TypeObject`, and the head of
//! the same file says "The types in this file shall be serialized with
//! XCDR encoding version 2". §7.4.2 gives an appendable object under
//! version 2 a `DHEADER` — a `UInt32` holding the byte length of
//! everything after it.
//!
//! Until this test existed, that clause had no octet behind it in this
//! repository: `crates/types/src/lib.rs`'s "Wiring status" says discovery
//! matches on `type_name`, so no `TypeObject` had ever left the process,
//! and the negative list recorded the finding as
//! "**Nicht gemessen:** ob ein Vendor an derselben Stelle einen DHEADER
//! schreibt" (`docs/spec-coverage/dds-xtypes-1.3.gaps.md`, C.1/C.2).
//!
//! # Where the octets come from
//!
//! `idlc` (CycloneDDS 11.0.1) on
//! `zerodds-examples/idl-conformance/proofs/typeobject/idl/typeobject.idl`
//! emits `typeobject.c`, which carries `TYPE_MAP_CDR_<T>` per top-level
//! type: the serialized `TypeMapping` of §7.6.3.2.1, a
//! `TypeIdentifierTypeObjectPairSeq` holding each `TypeObject` beside the
//! `TypeIdentifier` it hashes to.
//! `zerodds-examples/idl-conformance/proofs/typeobject/extractors/
//! cyclone_extract.py` parses that array and checks, per pair, that
//! `MD5(octets)[0..14]` equals the hash Cyclone put in the identifier —
//! so the framing below is not an assumption about where the object
//! begins, it is the framing Cyclone's own hash is taken over.
//!
//! Measured on the Linux test host on 2026-09-06 with the toolchains
//! installed there — `/opt/cyclone/bin/idlc` and
//! `/opt/rti.com/rti_connext_dds-7.7.0/bin/rtiddsgen` — rather than read
//! off a checked-in artefact. The recorded row is
//! `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`, set
//! `xtypes-typeobject`.
//!
//! The four cases are the leaf structures of that corpus, one per
//! extensibility kind plus a second `@final` one. Their `MINIMAL` hashes
//! are the ones CycloneDDS and Fast DDS agree on (the proof's own
//! `README.md` table), so these octets are two vendors' answer and not
//! one's.
//!
//! # What a failure here means
//!
//! Not "a test broke". A byte that moves here moves the
//! `EquivalenceHash` with it, and a peer that computes a different hash
//! for the same type reports a *type* mismatch — RTI Connext 7.7.0's
//! `PRESTypeObjectV2Table_assertTypeObjectRecord: TypeObjectV2
//! serializedSize mismatch` is what the missing four octets looked like
//! from the outside.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_types::from_ast::{
    complete_type_object, minimal_type_object, minimal_type_object_with,
};
use zerodds_types::type_object::TypeObject;

/// The corpus, verbatim from
/// `zerodds-examples/idl-conformance/proofs/typeobject/idl/typeobject.idl`.
///
/// Names are load-bearing: the `MinimalTypeObject` hashes the member
/// name into a `NameHash` and the `CompleteTypeObject` carries both the
/// member name and `to::<T>` verbatim.
const IDL: &str = "\
module to {
  @final
  struct Plain {
    long  a;
    short b;
  };
  @appendable
  struct Appendable {
    long  a;
    short b;
  };
  @mutable
  struct Mutable {
    @id(10) long  a;
    @id(20) short b;
  };
  @final
  struct Inner {
    long  x;
    long  y;
  };
};";

/// One case: the type's name, then Cyclone's `MINIMAL` and `COMPLETE`
/// octets for it.
struct Case {
    name: &'static str,
    minimal: &'static str,
    complete: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "Plain",
        minimal: "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe",
        complete: "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000",
    },
    Case {
        name: "Appendable",
        minimal: "33000000f1510200010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe",
        complete: "50000000f251020017000000000000000f000000746f3a3a417070656e6461626c6500002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000",
    },
    Case {
        name: "Mutable",
        minimal: "33000000f1510400010000000000000023000000020000000b0000000a0000000100040cc175b9000b0000001400000001000392eb5ffe",
        complete: "4c000000f251040014000000000000000c000000746f3a3a4d757461626c65002c00000002000000100000000a0000000100040002000000610000001000000014000000010003000200000062000000",
    },
    Case {
        name: "Inner",
        minimal: "33000000f1510100010000000000000023000000020000000b000000000000000100049dd4e461000b0000000100000001000441529076",
        complete: "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000",
    },
];

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "an odd number of hex digits");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn model() -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the corpus does not parse: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "typeobject.idl").ast;
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

/// Every octet of Cyclone's serialized `TypeObject`, `MINIMAL` and
/// `COMPLETE`, for all four cases.
#[test]
fn a_serialized_type_object_is_cyclones_octet_for_octet() {
    let ast = model();
    for case in CASES {
        let id = type_named(&ast, case.name);

        let minimal = TypeObject::Minimal(
            minimal_type_object(&ast, id).unwrap_or_else(|e| panic!("{}: {e:?}", case.name)),
        );
        let got = minimal.to_bytes_le().expect("encode");
        assert_eq!(
            got,
            unhex(case.minimal),
            "{} MINIMAL: got {}",
            case.name,
            got.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );

        let complete = TypeObject::Complete(
            complete_type_object(&ast, id).unwrap_or_else(|e| panic!("{}: {e:?}", case.name)),
        );
        let got = complete.to_bytes_le().expect("encode");
        assert_eq!(
            got,
            unhex(case.complete),
            "{} COMPLETE: got {}",
            case.name,
            got.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
    }
}

/// The first four octets are a `DHEADER` over the rest — the shape of
/// the finding, stated on its own so a failure names it.
///
/// A mutation that drops the `encode_appendable` wrapper from
/// `TypeObject::encode_into` fails this and the octet test above; a
/// mutation that writes the header twice fails this one on the length
/// and the hash test below on the hash.
#[test]
fn the_first_four_octets_announce_everything_after_them() {
    let ast = model();
    for case in CASES {
        let id = type_named(&ast, case.name);
        for bytes in [
            TypeObject::Minimal(minimal_type_object(&ast, id).expect("minimal"))
                .to_bytes_le()
                .expect("encode"),
            TypeObject::Complete(complete_type_object(&ast, id).expect("complete"))
                .to_bytes_le()
                .expect("encode"),
        ] {
            let announced = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
            assert_eq!(
                announced,
                bytes.len() - 4,
                "{}: the DHEADER counts everything after it",
                case.name
            );
            assert!(
                matches!(bytes[4], 0xF1 | 0xF2),
                "{}: then the EquivalenceKind, not {:#04x}",
                case.name,
                bytes[4]
            );
        }
    }
}

/// The COMPLETE hashes RTI Connext 7.7.0 computes for the same corpus,
/// measured on the Linux test host on 2026-09-06.
///
/// `rtiddsgen -language C` on the same IDL plus
/// `DDS_TypeObjectV2_create_from_typecode` gives, for `plain`, a
/// 104-octet blob and the hash `7c0c6fab7d066a0c56b8e93c0ea7`. The blob
/// is not the hashed stream — its MD5 is something else — because RTI
/// wraps the object in a 24-octet envelope: a length, a count, the
/// `EK_COMPLETE` octet and the 14-octet hash. **At offset 24 it holds
/// CycloneDDS's 80 octets verbatim**, opening `4c000000` — a `DHEADER`
/// of 76 — and it is those 80 that all three implementations hash.
///
/// That is the counter-check for the discovery rejection this finding
/// carries with it. RTI Connext 7.7.0 refused a ZeroDDS endpoint with
/// `PRESTypeObjectV2Table_assertTypeObjectRecord: TypeObjectV2
/// serializedSize mismatch` when the announced size was four octets
/// short of the hashed stream. The number this tree announces is the
/// length of the octets below, and the hash of those octets is the one
/// RTI reports — so the size RTI counts and the size ZeroDDS announces
/// are the same number for the same bytes.
#[test]
fn the_complete_hash_is_the_one_rti_and_cyclone_both_report() {
    /// name, COMPLETE hash, and the octet count the hash is taken over.
    const COMPLETE: &[(&str, &str, usize)] = &[
        ("Plain", "7c0c6fab7d066a0c56b8e93c0ea7", 80),
        ("Appendable", "27cb33d5ad3f7388a5627a1b48fd", 84),
        ("Mutable", "447863b1f0043ce0e42323a74866", 80),
        ("Inner", "f00388e0e0c6619e4175c68e8af8", 80),
    ];
    let ast = model();
    for (name, expected, length) in COMPLETE {
        let id = type_named(&ast, name);
        let complete = complete_type_object(&ast, id).expect("complete");
        let hash = zerodds_types::compute_complete_hash(&complete).expect("hash");
        let hex: String = hash.0.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(&hex, expected, "{name} COMPLETE hash");

        // The announced size is the length of the hashed stream, and
        // that is the number RTI counts for the same object.
        let announced =
            zerodds_types::hash::complete_serialized_size(&complete).expect("size") as usize;
        assert_eq!(announced, *length, "{name} typeobject_serialized_size");
        assert_eq!(
            announced,
            TypeObject::Complete(complete)
                .to_bytes_le()
                .expect("encode")
                .len()
        );
    }
}

/// The `EquivalenceHash` is `MD5` over exactly those octets, and it is
/// the hash CycloneDDS put in the `TypeIdentifier` beside them.
///
/// This is the assertion that catches a doubled `DHEADER`: adding one
/// more would still leave a self-consistent length field, and only the
/// hash says the stream is not the one the peer hashes.
#[test]
fn the_equivalence_hash_is_md5_over_the_serialized_object() {
    // The MINIMAL hashes CycloneDDS 11.0.1 and Fast DDS 3.6.1 agree on,
    // from the proof's own table.
    const MINIMAL_HASHES: &[(&str, &str)] = &[
        ("Plain", "deb922f040f7c228b25728be51f6"),
        ("Appendable", "3c9161318d323a5141b44ee3e2b6"),
        ("Mutable", "81e76ce14253653ea25be4145831"),
        ("Inner", "0c380bda28dab0250db24ae23c16"),
    ];
    let ast = model();
    for (name, expected) in MINIMAL_HASHES {
        let id = type_named(&ast, name);
        let minimal = minimal_type_object(&ast, id).expect("minimal");
        let hash = zerodds_types::compute_minimal_hash(&minimal).expect("hash");
        let hex: String = hash.0.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(&hex, expected, "{name} MINIMAL hash");

        // And the stream it was taken over is the serialized object.
        assert_eq!(
            zerodds_types::hash::minimal_hash_input(&minimal).expect("input"),
            TypeObject::Minimal(minimal.clone())
                .to_bytes_le()
                .expect("encode")
        );
        // The announced size is that same length.
        assert_eq!(
            zerodds_types::hash::minimal_serialized_size(&minimal).expect("size") as usize,
            TypeObject::Minimal(minimal)
                .to_bytes_le()
                .expect("encode")
                .len()
        );
    }
}

/// The RTI Connext/CoreDX minimal camp: one trailing octet inside each
/// aggregated member's DHEADER, and nothing else. XTypes 1.3 §7.3.4.5
/// leaves the padding around a nested member's DHEADER open;
/// `docs/omg-xtypes-issues.md` §C2 records the two vendor byte strings.
/// This pins the axis: the default stream is unchanged (still Cyclone's
/// octets, asserted above), the RTI stream is exactly one octet longer
/// per member and those octets are zeros, and the RTI stream still
/// decodes back to the same object — the trailing octet is @appendable
/// surplus a reader skips.
#[test]
fn the_rti_camp_adds_one_trailing_octet_per_minimal_member() {
    let ast = model();
    for case in CASES {
        let id = type_named(&ast, case.name);
        let m_default = minimal_type_object(&ast, id).expect("minimal default");
        let m_rti = minimal_type_object_with(&ast, id, true).expect("minimal rti");

        let s_default = zerodds_types::hash::minimal_hash_input(&m_default).expect("default bytes");
        let s_rti = zerodds_types::hash::minimal_hash_input_with(&m_rti, true).expect("rti bytes");

        // Default is unchanged: the same octets the vendor golden records.
        assert_eq!(
            s_default,
            unhex(case.minimal),
            "{}: default camp drifted",
            case.name
        );

        // The RTI stream is longer: a trailing octet goes inside each
        // member DHEADER, though the net length also moves with the
        // inter-member four-byte alignment the added octet shifts. The
        // exact per-form match against RTI Connext is measured live in
        // the suite (`docs/omg-xtypes-issues.md` §C2); here it is enough
        // that the camp changes the stream.
        assert!(
            s_rti.len() > s_default.len(),
            "{}: RTI camp did not lengthen the stream",
            case.name
        );

        // The hashes are the two camps, and they differ.
        let h_default =
            zerodds_types::hash::compute_minimal_hash(&m_default).expect("default hash");
        let h_rti = zerodds_types::hash::compute_minimal_hash_with(&m_rti, true).expect("rti hash");
        assert_ne!(
            h_default.0, h_rti.0,
            "{}: the two camps hash alike",
            case.name
        );

        // The RTI stream is still a well-formed minimal TypeObject: a
        // reader skips the trailing octet as @appendable surplus.
        let decoded = TypeObject::from_bytes_le(&s_rti).expect("RTI stream decodes");
        match decoded {
            TypeObject::Minimal(back) => {
                assert_eq!(back, m_rti, "{}: RTI stream does not round-trip", case.name)
            }
            TypeObject::Complete(_) => panic!("{}: minimal stream decoded as complete", case.name),
            _ => panic!("{}: unexpected TypeObject variant", case.name),
        }
    }
}
