//! Property-Based Tests fuer CDR-Encode-Decode-Roundtrip.
//!
//! Invariante: `decode(encode(x)) == x` fuer alle Primitive-Typen,
//! beide Endiannesses. Spec-Anker: OMG-CDR (Common Data
//! Representation), XCDR1/XCDR2.

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

use proptest::prelude::*;
use zerodds_cdr::Endianness;
use zerodds_cdr::buffer::{BufferReader, BufferWriter};
use zerodds_cdr::encode::{CdrDecode, CdrEncode};

fn roundtrip<T: CdrEncode + CdrDecode + PartialEq + std::fmt::Debug>(
    value: &T,
    endianness: Endianness,
) {
    let mut w = BufferWriter::new(endianness);
    value.encode(&mut w).expect("encode must succeed");
    let bytes = w.into_bytes();
    let mut r = BufferReader::new(&bytes, endianness);
    let decoded = T::decode(&mut r).expect("decode must succeed");
    assert_eq!(value, &decoded, "roundtrip mismatch");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn roundtrip_u8_le(v in any::<u8>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_u8_be(v in any::<u8>()) {
        roundtrip(&v, Endianness::Big);
    }

    #[test]
    fn roundtrip_u16_le(v in any::<u16>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_u16_be(v in any::<u16>()) {
        roundtrip(&v, Endianness::Big);
    }

    #[test]
    fn roundtrip_u32_le(v in any::<u32>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_u32_be(v in any::<u32>()) {
        roundtrip(&v, Endianness::Big);
    }

    #[test]
    fn roundtrip_u64_le(v in any::<u64>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_u64_be(v in any::<u64>()) {
        roundtrip(&v, Endianness::Big);
    }

    #[test]
    fn roundtrip_i8_le(v in any::<i8>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_i16_le(v in any::<i16>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_i32_le(v in any::<i32>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_i64_le(v in any::<i64>()) {
        roundtrip(&v, Endianness::Little);
    }

    #[test]
    fn roundtrip_f32_le(v in any::<u32>()) {
        // Verwende u32-Bits, dann zu f32 — vermeidet NaN-vergleichs-Stolperfalle.
        let f = f32::from_bits(v);
        if f.is_nan() {
            return Ok(());
        }
        roundtrip(&f, Endianness::Little);
    }

    #[test]
    fn roundtrip_f64_le(v in any::<u64>()) {
        let f = f64::from_bits(v);
        if f.is_nan() {
            return Ok(());
        }
        roundtrip(&f, Endianness::Little);
    }

    #[test]
    fn roundtrip_bool(v in any::<bool>()) {
        roundtrip(&v, Endianness::Little);
        roundtrip(&v, Endianness::Big);
    }
}
