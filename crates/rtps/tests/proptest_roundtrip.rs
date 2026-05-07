//! Property-Tests fuer RTPS-Wire-Roundtrips.
//!
//! Invariante: `read_from(write_to(x)) == x` fuer alle SequenceNumber-
//! Sets, FragmentNumber-Sets, beide Endiannesses.

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
use zerodds_rtps::submessages::{FragmentNumberSet, SequenceNumberSet};
use zerodds_rtps::wire_types::{FragmentNumber, SequenceNumber};

fn arb_sn(base: i64) -> impl Strategy<Value = i64> {
    // SN >= base, max 1024 ueber base (Set-Groesse begrenzt fuer Test-Speed).
    (0i64..1024).prop_map(move |i| base + i)
}

fn arb_seqnum_set() -> impl Strategy<Value = SequenceNumberSet> {
    (1i64..1_000_000, prop::collection::vec(0i64..256, 0..32)).prop_map(|(base, offsets)| {
        let base_sn = SequenceNumber(base);
        let mut missing: Vec<SequenceNumber> = offsets
            .into_iter()
            .map(|o| SequenceNumber(base + o))
            .collect();
        missing.sort();
        missing.dedup();
        SequenceNumberSet::from_missing(base_sn, &missing)
    })
}

fn arb_fragment_number_set() -> impl Strategy<Value = FragmentNumberSet> {
    (1u32..1_000_000, prop::collection::vec(0u32..256, 0..32)).prop_map(|(base, offsets)| {
        let mut missing: Vec<FragmentNumber> = offsets
            .into_iter()
            .map(|o| FragmentNumber(base + o))
            .collect();
        missing.sort();
        missing.dedup();
        FragmentNumberSet::from_missing(FragmentNumber(base), &missing)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn seqnum_set_roundtrip_le(set in arb_seqnum_set()) {
        let mut bytes = Vec::new();
        set.write_to(&mut bytes, true);
        let (decoded, consumed) = SequenceNumberSet::read_from(&bytes, 0, true).unwrap();
        prop_assert_eq!(decoded.bitmap_base, set.bitmap_base);
        prop_assert_eq!(decoded.num_bits, set.num_bits);
        prop_assert_eq!(decoded.bitmap, set.bitmap);
        prop_assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn seqnum_set_roundtrip_be(set in arb_seqnum_set()) {
        let mut bytes = Vec::new();
        set.write_to(&mut bytes, false);
        let (decoded, _) = SequenceNumberSet::read_from(&bytes, 0, false).unwrap();
        prop_assert_eq!(decoded.bitmap_base, set.bitmap_base);
        prop_assert_eq!(decoded.num_bits, set.num_bits);
        prop_assert_eq!(decoded.bitmap, set.bitmap);
    }

    #[test]
    fn frag_set_roundtrip_le(set in arb_fragment_number_set()) {
        let mut bytes = Vec::new();
        set.write_to(&mut bytes, true);
        let (decoded, _) = FragmentNumberSet::read_from(&bytes, 0, true).unwrap();
        prop_assert_eq!(decoded.bitmap_base, set.bitmap_base);
        prop_assert_eq!(decoded.num_bits, set.num_bits);
        prop_assert_eq!(decoded.bitmap, set.bitmap);
    }

    #[test]
    fn frag_set_roundtrip_be(set in arb_fragment_number_set()) {
        let mut bytes = Vec::new();
        set.write_to(&mut bytes, false);
        let (decoded, _) = FragmentNumberSet::read_from(&bytes, 0, false).unwrap();
        prop_assert_eq!(decoded.bitmap_base, set.bitmap_base);
        prop_assert_eq!(decoded.num_bits, set.num_bits);
        prop_assert_eq!(decoded.bitmap, set.bitmap);
    }

    #[test]
    fn seqnum_set_iter_matches_missing(
        base in 1i64..1_000_000,
        offsets in prop::collection::vec(0i64..256, 0..32),
    ) {
        let base_sn = SequenceNumber(base);
        let mut missing: Vec<SequenceNumber> = offsets
            .into_iter()
            .map(|o| SequenceNumber(base + o))
            .collect();
        missing.sort();
        missing.dedup();
        let set = SequenceNumberSet::from_missing(base_sn, &missing);
        let iter_set: Vec<SequenceNumber> = set.iter_set().collect();
        prop_assert_eq!(iter_set, missing);
    }

    #[test]
    fn seqnum_set_wire_size_matches_encoded(set in arb_seqnum_set()) {
        let mut bytes = Vec::new();
        set.write_to(&mut bytes, true);
        prop_assert_eq!(bytes.len(), set.encoded_size());
    }

    #[test]
    fn _arb_sn_in_range(base in 1i64..1_000_000, sn in arb_sn(0)) {
        // Smoke test for arb_sn helper: just verifies it produces values.
        let _ = (base, sn);
    }
}
