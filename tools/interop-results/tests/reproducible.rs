// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every recorded octet is one a re-run would produce.
//!
//! `committed_tree.rs` proves the tree is what `sources/` says. Nothing
//! proved that `sources/` is what a *run* says, and nothing on a clone
//! with only a Rust toolchain can re-run RTI, three hand-built ORBs or
//! a ROS installation. So this asks the record a question it can answer
//! about itself: a case and its `-unannotated` twin carry the same
//! sample, and for a column whose front end drops the annotations they
//! are the same type — so their octets have to be the same octets.
//!
//! It is not a thought experiment. Six rows held a heap address each
//! for as long as they were committed, and `--verify` passed on them
//! every time, because `--verify` compares the tree against those rows.
//! `crates/wire-probe/src/ros_idl.rs::ByteOrderHazard` and
//! `tools/wireprobe/wireprobe-ros` are the refusal that stops them
//! being recorded; this is the gate that would have found them if the
//! refusal had not been there.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::Inputs;
use zerodds_interop_results::reproducible::twin_disagreements;
use zerodds_interop_results::systems::SYSTEMS;

fn disagreements() -> Vec<zerodds_interop_results::reproducible::Disagreement> {
    let root = results::repository_root();
    let sources = results::results_root(&root).join("sources");
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&sources).expect("the sources");
    twin_disagreements(&cases, &[&inputs.foreign, &inputs.orbs])
}

/// No recorded octet differs between twins in a way a reader can see.
#[test]
fn no_recorded_octet_is_something_other_than_the_sample() {
    let found: Vec<String> = disagreements()
        .iter()
        .filter(|twin| !twin.reaches_no_reader)
        .map(zerodds_interop_results::reproducible::Disagreement::line)
        .collect();
    assert!(
        found.is_empty(),
        "{} recorded octet(s) are not a function of the sample:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// The padding allowance is a no-op now, and that is the measurement.
///
/// It was not. omniORB's twins disagreed on 23 pairs, every one of them
/// same-length and every differing octet one this project writes as
/// `00`, because `cdrMemoryStream(0, false)` left the fill uninitialised
/// and two runs of one process differed there. The allowance existed to
/// let those through without letting a *value* through.
///
/// Since 2026-08-13 `omniorb-template.cpp` and `tao-template.cpp` write
/// a fixed `0xAA` at the fill, so the recording is a function of the
/// sample and a case and its `-unannotated` twin produce the same
/// octets — **zero** disagreements, not 23 forgiven ones. That is the
/// stronger result and it is asserted here rather than left to be
/// inferred from a check that stopped firing: a run that starts
/// producing twin disagreements again has either lost the marker or
/// found something the marker does not cover, and either is worth being
/// told about.
///
/// [`crate::reproducible::Disagreement::reaches_no_reader`] and the
/// allowance in `no_recorded_octet_is_something_other_than_the_sample`
/// stay, because they are what would keep a *padding* difference from
/// reading as a value difference if one appeared.
#[test]
fn the_padding_allowance_has_nothing_left_to_forgive() {
    let found = disagreements();
    assert!(
        found.is_empty(),
        "{} twin pair(s) carry one sample and two byte strings; the probes' fill is supposed \
         to be a function of the sample now:\n{}",
        found.len(),
        found
            .iter()
            .map(zerodds_interop_results::reproducible::Disagreement::line)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The six octets it was built for, put back, and it goes red.
///
/// A gate nobody has seen fail is a gate nobody has seen. These are the
/// rows `sources/ros-corpus.tsv` carried at `b0bc71313`, verbatim:
/// `sequence-bounded-full` and its twin under `--endian be`, whose
/// `values.json` is the same file twice and whose three `long` came out
/// as a heap address and the stack word after it.
#[test]
fn the_six_octets_it_was_built_for_are_caught() {
    let root = results::repository_root();
    let cases = results::corpus(&root).expect("the corpus");
    let mut recorded = zerodds_interop_results::sources::Measured::new();
    for (case, hex) in [
        (
            "sequence-bounded-full",
            "00000003972402b00000563269204310ab",
        ),
        (
            "sequence-bounded-full-unannotated",
            "0000000355ac22b0000061e114f5e380ab",
        ),
    ] {
        recorded.insert(
            format!("{case}/ros-fastrtps/xcdr1/be"),
            zerodds_interop_results::sources::Answer::Bytes(hex.to_string()),
        );
    }
    let found = twin_disagreements(&cases, &[&recorded]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        !found[0].reaches_no_reader,
        "an address is not padding: {}",
        found[0].line()
    );
    assert!(found[0].line().contains("sequence-bounded-full"));
}

/// Two twins that agree are not reported, so the check is a check and
/// not a report of every pair it looks at.
#[test]
fn twins_that_agree_are_not_reported() {
    let root = results::repository_root();
    let cases = results::corpus(&root).expect("the corpus");
    let mut recorded = zerodds_interop_results::sources::Measured::new();
    for case in ["sequence-bounded-full", "sequence-bounded-full-unannotated"] {
        recorded.insert(
            format!("{case}/ros-fastrtps/xcdr1/le"),
            zerodds_interop_results::sources::Answer::Bytes(
                "030000007929edffffffff7fffffffffab".to_string(),
            ),
        );
    }
    assert!(twin_disagreements(&cases, &[&recorded]).is_empty());
}

/// Which columns the check asks, spelled out rather than counted.
///
/// A column that quietly gains or loses the flag changes what this
/// whole file covers, and a count would not say which.
#[test]
fn the_columns_whose_twins_are_one_type_are_the_measured_ones() {
    let asked: Vec<&str> = SYSTEMS
        .iter()
        .filter(|system| system.twins_are_one_type)
        .map(|system| system.id)
        .collect();
    assert_eq!(
        asked,
        ["omniorb", "tao", "jacorb", "ros-fastrtps", "ros-cyclone"]
    );
}
