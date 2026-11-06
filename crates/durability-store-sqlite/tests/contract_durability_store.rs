// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of a store adapter with `zerodds-durability-store`
//! (internal/release/rc8-test-suite.md, "Crate tier"). What the adapter uses:
//! the `DurabilityStore` trait it implements, the `Contract` it enforces and
//! keeps in a row (`to_row`/`from_row`, `effective_depth`, the `*_bounded`
//! checks), the columns of the representation identifier
//! (`identifier_columns`/`identifier_from_columns`) and `Selector` with its
//! cursor. The oracles: DDS 1.4 §2.2.3.5 (DURABILITY_SERVICE: history kind
//! and depth, the caps, `LENGTH_UNLIMITED` = -1, the depth of KEEP_LAST at
//! least 1) and DDS-XTypes 1.3 Table 60 (the CDR identifiers).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, Selector, identifier_columns, identifier_from_columns,
};
use zerodds_qos::policies::history::HistoryKind;

/// A contract survives its row: the seven fields an adapter keeps; a
/// history kind that is neither KEEP_LAST nor KEEP_ALL is refused.
#[test]
fn dcps_2_2_3_5_a_contract_comes_back_from_its_row() {
    let c = Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 5,
        max_samples: 100,
        max_instances: 10,
        max_samples_per_instance: 3,
        cleanup_delay: Duration::new(7, 42),
    };
    let (a, b, cc, d, e, f, g) = c.to_row();
    assert_eq!(Contract::from_row(a, b, cc, d, e, f, g), Some(c));
    assert_eq!(Contract::from_row(99, 0, -1, -1, -1, 0, 0), None);
}

/// `LENGTH_UNLIMITED` (-1) is no bound; a KEEP_LAST depth of zero or less
/// keeps one sample.
#[test]
fn dcps_2_2_3_5_unlimited_caps_and_the_least_depth() {
    let all = Contract::keep_all();
    assert_eq!(all.history_kind, HistoryKind::KeepAll);
    assert_eq!(
        (
            all.max_samples,
            all.max_instances,
            all.max_samples_per_instance
        ),
        (-1, -1, -1)
    );
    assert!(!all.samples_bounded() && !all.instances_bounded() && !all.per_instance_bounded());
    let last = Contract {
        history_kind: HistoryKind::KeepLast,
        history_depth: 0,
        max_samples_per_instance: 2,
        ..all
    };
    assert_eq!(last.effective_depth(), 1);
    assert!(last.per_instance_bounded());
}

/// Every CDR identifier of Table 60 comes back from its two columns, and a
/// column pair of an earlier store (1 = XCDR2) reads as CDR2.
#[test]
fn xtypes_7_6_3_1_2_the_identifier_comes_back_from_its_columns() {
    for second in [0x00, 0x01, 0x02, 0x03, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b] {
        let (representation, big_endian) = identifier_columns([0x00, second]);
        assert_eq!(
            identifier_from_columns(representation, big_endian),
            [0x00, second]
        );
    }
    assert_eq!(identifier_from_columns(1, false), [0x00, 0x07]);
}

/// A selector matches by instance and sequence range, and the cursor of a
/// page continues strictly after it.
#[test]
fn a_selector_and_its_cursor_continue_after_the_page() {
    let sample = |inst: u8, seq: u64| DurabilitySample {
        topic: "T".into(),
        instance_key: [inst; 16],
        sequence: seq,
        payload: vec![],
        representation_identifier: [0x00, 0x07],
        created_at: SystemTime::UNIX_EPOCH,
        source_guid: [0; 16],
        source_sequence: -1,
    };
    let sel = Selector {
        instance_key: Some([2; 16]),
        seq_from: Some(1),
        seq_to: Some(3),
        ..Selector::default()
    };
    assert!(sel.matches(&sample(2, 1)) && sel.matches(&sample(2, 3)));
    assert!(!sel.matches(&sample(2, 4)) && !sel.matches(&sample(1, 2)));
    let next = sel.after_cursor(([2; 16], 1));
    assert!(!next.matches(&sample(2, 1)) && next.matches(&sample(2, 2)));
}
