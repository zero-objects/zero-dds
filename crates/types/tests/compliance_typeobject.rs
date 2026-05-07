//! WP 1.10 T4 — TypeObject/EquivalenceHash Golden-Vector Tests.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::path::PathBuf;

fn compliance_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("compliance")
}

fn load_hex(path: &std::path::Path) -> Vec<u8> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    for line in text.lines() {
        let stripped = line.split('#').next().unwrap_or("");
        for tok in stripped.split_whitespace() {
            let t = tok.strip_prefix("0x").unwrap_or(tok);
            for chunk in t.as_bytes().chunks(2) {
                let s = std::str::from_utf8(chunk).unwrap();
                out.push(u8::from_str_radix(s, 16).unwrap());
            }
        }
    }
    out
}

#[test]
fn md5_empty_string_first_14_bytes_golden_vector() {
    let p = compliance_root()
        .join("typeobject")
        .join("md5_empty_string.hex");
    let bytes = load_hex(&p);
    assert_eq!(bytes.len(), 14);

    let h = zerodds_types::hash::hash_bytes(b"");
    assert_eq!(h.0.as_slice(), bytes.as_slice());
}

// ============================================================================
// §7.3.4.6 — Indirect Hash TypeIdentifiers (PlainCollectionHeader mit
// EK_MINIMAL/EK_COMPLETE als Element). Fixed-Wire-Bytes-Vektoren gegen
// das XCDR2-LE Encoding (XTypes 1.3 §7.3.4.7.1).
// ============================================================================

use zerodds_types::type_identifier::{
    CollectionElementFlag, EquivalenceHash, PlainCollectionHeader, TypeIdentifier,
};

const EQUIVALENCE_HASH_LEN: usize = 14;

const EK_MINIMAL: u8 = 0xF1;
const EK_COMPLETE: u8 = 0xF2;
const TI_PLAIN_SEQUENCE_SMALL: u8 = 0x80;
const TI_PLAIN_SEQUENCE_LARGE: u8 = 0x81;
const TI_PLAIN_ARRAY_SMALL: u8 = 0x90;
const TI_PLAIN_ARRAY_LARGE: u8 = 0x91;
const TI_PLAIN_MAP_SMALL: u8 = 0xA0;
const TI_PLAIN_MAP_LARGE: u8 = 0xA1;

fn fixed_hash_minimal() -> EquivalenceHash {
    let mut h = [0u8; EQUIVALENCE_HASH_LEN];
    for (i, b) in h.iter_mut().enumerate() {
        *b = u8::try_from(0xA0 + i).unwrap();
    }
    EquivalenceHash(h)
}

fn fixed_hash_complete() -> EquivalenceHash {
    let mut h = [0u8; EQUIVALENCE_HASH_LEN];
    for (i, b) in h.iter_mut().enumerate() {
        *b = u8::try_from(0xC0 + i).unwrap();
    }
    EquivalenceHash(h)
}

#[test]
fn indirect_hash_plain_sequence_small_minimal_wire_vector() {
    // sequence<X, 5> wo X = EK_MINIMAL hash[14]={0xA0..0xAD}.
    // Wire (XCDR2 LE):
    //   0x80                         -- TI_PLAIN_SEQUENCE_SMALL
    //   0xF1                         -- equiv_kind (EK_MINIMAL)
    //   0x00 0x00                    -- element_flags
    //   0x05                         -- bound
    //   0xF1                         -- elem.discriminator (EK_MINIMAL)
    //   0xA0..0xAD                   -- 14 byte hash
    let ti = TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader {
            equiv_kind: EK_MINIMAL,
            element_flags: CollectionElementFlag(0),
        },
        bound: 5,
        element: Box::new(TypeIdentifier::EquivalenceHashMinimal(fixed_hash_minimal())),
    };
    let bytes = ti.to_bytes_le().unwrap();
    let expected = {
        let mut v = vec![
            TI_PLAIN_SEQUENCE_SMALL,
            EK_MINIMAL,
            0x00,
            0x00,
            0x05,
            EK_MINIMAL,
        ];
        v.extend_from_slice(&fixed_hash_minimal().0);
        v
    };
    assert_eq!(bytes, expected);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, ti);
}

#[test]
fn indirect_hash_plain_sequence_large_complete_wire_vector() {
    // sequence<X, 1000> wo X = EK_COMPLETE hash[14]={0xC0..0xCD}.
    let ti = TypeIdentifier::PlainSequenceLarge {
        header: PlainCollectionHeader {
            equiv_kind: EK_COMPLETE,
            element_flags: CollectionElementFlag(0),
        },
        bound: 1000,
        element: Box::new(TypeIdentifier::EquivalenceHashComplete(
            fixed_hash_complete(),
        )),
    };
    let bytes = ti.to_bytes_le().unwrap();
    let mut expected = vec![
        TI_PLAIN_SEQUENCE_LARGE,
        EK_COMPLETE,
        0x00,
        0x00,
        0xE8,
        0x03,
        0x00,
        0x00, // bound = 1000 LE
        EK_COMPLETE,
    ];
    expected.extend_from_slice(&fixed_hash_complete().0);
    assert_eq!(bytes, expected);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, ti);
}

#[test]
fn indirect_hash_plain_array_small_minimal_wire_vector() {
    // T[3,4] wo T = EK_MINIMAL hash. array_bounds = octet seq.
    let ti = TypeIdentifier::PlainArraySmall {
        header: PlainCollectionHeader {
            equiv_kind: EK_MINIMAL,
            element_flags: CollectionElementFlag(0),
        },
        array_bounds: vec![3, 4],
        element: Box::new(TypeIdentifier::EquivalenceHashMinimal(fixed_hash_minimal())),
    };
    let bytes = ti.to_bytes_le().unwrap();
    let mut expected = vec![
        TI_PLAIN_ARRAY_SMALL,
        EK_MINIMAL,
        0x00,
        0x00,
        0x02,
        0x00,
        0x00,
        0x00, // sequence length = 2 (LE)
        0x03, // dim 0
        0x04, // dim 1
        EK_MINIMAL,
    ];
    expected.extend_from_slice(&fixed_hash_minimal().0);
    assert_eq!(bytes, expected);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, ti);
}

#[test]
fn indirect_hash_plain_array_large_complete_wire_vector() {
    // T[300, 400] wo T = EK_COMPLETE hash. array_bounds = uint32 seq.
    let ti = TypeIdentifier::PlainArrayLarge {
        header: PlainCollectionHeader {
            equiv_kind: EK_COMPLETE,
            element_flags: CollectionElementFlag(0),
        },
        array_bounds: vec![300, 400],
        element: Box::new(TypeIdentifier::EquivalenceHashComplete(
            fixed_hash_complete(),
        )),
    };
    let bytes = ti.to_bytes_le().unwrap();
    let mut expected = vec![
        TI_PLAIN_ARRAY_LARGE,
        EK_COMPLETE,
        0x00,
        0x00,
        0x02,
        0x00,
        0x00,
        0x00, // dim count
        0x2C,
        0x01,
        0x00,
        0x00, // 300 LE
        0x90,
        0x01,
        0x00,
        0x00, // 400 LE
        EK_COMPLETE,
    ];
    expected.extend_from_slice(&fixed_hash_complete().0);
    assert_eq!(bytes, expected);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, ti);
}

#[test]
fn indirect_hash_plain_map_small_minimal_keys_complete_values_wire_vector() {
    // map<K, V, 10> wo K = EK_MINIMAL, V = EK_COMPLETE, bound=10.
    let ti = TypeIdentifier::PlainMapSmall {
        header: PlainCollectionHeader {
            equiv_kind: EK_COMPLETE,
            element_flags: CollectionElementFlag(0),
        },
        bound: 10,
        element: Box::new(TypeIdentifier::EquivalenceHashComplete(
            fixed_hash_complete(),
        )),
        key_flags: CollectionElementFlag(0),
        key: Box::new(TypeIdentifier::EquivalenceHashMinimal(fixed_hash_minimal())),
    };
    let bytes = ti.to_bytes_le().unwrap();
    let mut expected = vec![
        TI_PLAIN_MAP_SMALL,
        EK_COMPLETE,
        0x00,
        0x00,
        0x0A,        // bound
        EK_COMPLETE, // value-element discriminator
    ];
    expected.extend_from_slice(&fixed_hash_complete().0);
    expected.extend_from_slice(&[0x00, 0x00]); // key_flags
    expected.push(EK_MINIMAL);
    expected.extend_from_slice(&fixed_hash_minimal().0);
    assert_eq!(bytes, expected);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, ti);
}

#[test]
fn indirect_hash_plain_map_large_complete_keys_minimal_values_wire_vector() {
    // map<K, V, 65536> wo bound > 255 → MapLarge.
    let ti = TypeIdentifier::PlainMapLarge {
        header: PlainCollectionHeader {
            equiv_kind: EK_MINIMAL,
            element_flags: CollectionElementFlag(0),
        },
        bound: 65536,
        element: Box::new(TypeIdentifier::EquivalenceHashMinimal(fixed_hash_minimal())),
        key_flags: CollectionElementFlag(0),
        key: Box::new(TypeIdentifier::EquivalenceHashComplete(
            fixed_hash_complete(),
        )),
    };
    let bytes = ti.to_bytes_le().unwrap();
    let mut expected = vec![
        TI_PLAIN_MAP_LARGE,
        EK_MINIMAL,
        0x00,
        0x00,
        0x00,
        0x00,
        0x01,
        0x00, // bound = 65536 LE
        EK_MINIMAL,
    ];
    expected.extend_from_slice(&fixed_hash_minimal().0);
    // 1 byte alignment-pad: write_u16(key_flags) faellt auf odd offset
    // 23 (1+1+2+4+1+14) — Buffer aligned auf 2-byte boundary.
    expected.push(0x00);
    expected.extend_from_slice(&[0x00, 0x00]);
    expected.push(EK_COMPLETE);
    expected.extend_from_slice(&fixed_hash_complete().0);
    assert_eq!(bytes, expected);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, ti);
}

#[test]
fn indirect_hash_nested_sequence_of_sequence_minimal_wire_roundtrip() {
    // sequence<sequence<X,3>,5> mit X = EK_MINIMAL — geschachtelte
    // PlainCollections. Test verifiziert dass Box-rekursiver Encoder/
    // Decoder fuer indirekte Hash-Element-Typen byte-genau ist.
    let inner = TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader {
            equiv_kind: EK_MINIMAL,
            element_flags: CollectionElementFlag(0),
        },
        bound: 3,
        element: Box::new(TypeIdentifier::EquivalenceHashMinimal(fixed_hash_minimal())),
    };
    let outer = TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader {
            equiv_kind: EK_MINIMAL,
            element_flags: CollectionElementFlag(0),
        },
        bound: 5,
        element: Box::new(inner.clone()),
    };
    let bytes = outer.to_bytes_le().unwrap();
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    assert_eq!(decoded, outer);
}

#[test]
fn indirect_hash_invalid_equiv_kind_in_header_decodes_as_unknown_ek_byte() {
    // Wenn equiv_kind weder EK_MINIMAL/EK_COMPLETE/EK_BOTH ist, soll der
    // Decoder das nicht stillschweigend als EK_MINIMAL fehl-deuten —
    // header.equiv_kind speichert das Wire-Byte 1:1.
    let mut bytes = vec![
        TI_PLAIN_SEQUENCE_SMALL,
        0xAB, // garbage equiv_kind
        0x00,
        0x00,
        0x05,
        EK_MINIMAL,
    ];
    bytes.extend_from_slice(&fixed_hash_minimal().0);
    let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
    if let TypeIdentifier::PlainSequenceSmall { header, .. } = decoded {
        assert_eq!(header.equiv_kind, 0xAB);
    } else {
        panic!("expected PlainSequenceSmall");
    }
}
