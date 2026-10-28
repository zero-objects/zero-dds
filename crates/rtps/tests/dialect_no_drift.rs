// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two dialect lists name the same implementations the same way.
//!
//! `zerodds_ast_layout::Dialect` names **wire-format** dialects and
//! `zerodds_rtps::dialect::RtpsDialect` names **discovery-behaviour**
//! dialects. They are deliberately separate: an implementation can occupy an
//! axis in one table and none in the other, and one list would blur that.
//!
//! What the separation must not cost is a shared vocabulary. A reader who
//! configures `cyclone` in one place and `cyclone` in the other has to be
//! naming the same product at the same version, and a rename or a version
//! bump on one side has to be visible from the other. That is what this file
//! checks, so that the merge of the two lists stays a decision and not a
//! repair.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_ast_layout::key::Dialect;
use zerodds_rtps::dialect::RtpsDialect;

/// The RTPS-side dialects that name an implementation the wire-format table
/// also names, and the version each side has to agree on.
const SHARED: &[(&str, &str)] = &[
    ("cyclone", "11.0.1"),
    ("fastdds", "3.6.1"),
    ("opendds", "3.34.0"),
];

/// Every shared spelling parses on both sides, and round-trips.
///
/// A rename on either side breaks this: `Dialect::parse` stops accepting the
/// spelling, or `RtpsDialect::parse` does.
#[test]
fn every_shared_vendor_name_is_spelled_the_same_on_both_sides() {
    for (spelling, _) in SHARED {
        let rtps = RtpsDialect::parse(spelling).unwrap_or_else(|| {
            panic!("`{spelling}` is listed as shared but RtpsDialect::parse rejects it")
        });
        assert_eq!(rtps.as_str(), *spelling, "RtpsDialect round-trip");
        let layout = Dialect::parse(spelling).unwrap_or_else(|| {
            panic!(
                "`{spelling}` names an implementation in zerodds-rtps but \
                 zerodds_ast_layout::Dialect::parse rejects it — one of the two \
                 lists was renamed without the other"
            )
        });
        assert_eq!(layout.as_str(), *spelling, "Dialect round-trip");
    }
}

/// Both sides name the same version for a shared implementation.
///
/// `zerodds_ast_layout::Dialect` carries its versions in the doc comments of
/// its variants rather than in an accessor, so the comparison reads the
/// source. That is the point: the version is written down there, and this
/// test fails when it changes there and not here.
#[test]
fn every_shared_vendor_names_the_same_version_on_both_sides() {
    let key_rs = concat!(env!("CARGO_MANIFEST_DIR"), "/../ast-layout/src/key.rs");
    let source = std::fs::read_to_string(key_rs)
        .unwrap_or_else(|why| panic!("{key_rs}: {why} — the drift check needs the other list"));
    for (spelling, version) in SHARED {
        let rtps = RtpsDialect::parse(spelling).expect("checked above");
        assert_eq!(
            rtps.measured_version(),
            Some(*version),
            "`{spelling}`: RtpsDialect::measured_version disagrees with this test's table"
        );
        assert!(
            source.contains(version),
            "`{spelling}`: zerodds-rtps reproduces version {version}, but that \
             string does not appear in ast-layout/src/key.rs — either the \
             wire-format table moved to another version of the same \
             implementation, or this one did"
        );
    }
}

/// The default of each table is its own, and neither borrows the other's
/// name. `Dialect`'s default is the clause reading; `RtpsDialect`'s cannot be,
/// because on its axes no clause picks a value (§8.3.7.2.5 Table 8.43 permits
/// either flag form, §9.6.3.9 puts the semantics in `PID_STATUS_INFO`).
#[test]
fn the_two_defaults_are_named_for_what_they_are() {
    assert_eq!(Dialect::default().as_str(), "specification");
    assert_eq!(
        RtpsDialect::default().as_str(),
        "maximally-interoperable",
        "the RTPS default must not claim to be a specification reading"
    );
    assert_eq!(
        RtpsDialect::parse("specification"),
        None,
        "`specification` must not be an accepted RTPS dialect: there is no \
         clause reading to select on these axes"
    );
    assert!(RtpsDialect::default().measured_version().is_none());
}

/// Every RTPS dialect is either the default or shared with the wire-format
/// table. A new one that is neither has to be added to `SHARED` (and then to
/// `Dialect`) or the two vocabularies start to drift silently.
#[test]
fn no_rtps_dialect_is_unaccounted_for() {
    for d in RtpsDialect::all() {
        if d.is_default() {
            continue;
        }
        assert!(
            SHARED.iter().any(|(s, _)| *s == d.as_str()),
            "{d:?} (`{}`) is neither the default nor listed as shared — add it \
             to SHARED here and to zerodds_ast_layout::Dialect, or the two \
             dialect vocabularies drift apart",
            d.as_str()
        );
    }
}
