// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! This crate against GCC's binary128.
//!
//! `binary128.rs` checks the type against itself and against `f64`. That
//! catches a great deal and cannot catch a shared misreading of the
//! format — a printer and a parser that are wrong the same way agree
//! with each other perfectly.
//!
//! So `tests/oracle/quad_oracle.c` was run on the Linux test host, where
//! GCC's `__float128` is a real binary128 with libquadmath's own printer
//! and parser, over 518 values: every corner this crate names plus 500
//! generated patterns. `tests/oracle/x86_64-gcc-14.2.txt` is what came
//! back, and this compares against it in both directions.
//!
//! Regenerating it is `tests/oracle/README.md`. A change to the printer
//! makes column three stale, and that is meant to fail here rather than
//! pass quietly on a rule somebody remembered.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_float128::Float128;

/// Where the two files live, relative to the crate root.
const PATTERNS: &str = include_str!("oracle/patterns.txt");
const ORACLE: &str = include_str!("oracle/x86_64-gcc-14.2.txt");

/// One row of each file, joined on the bit pattern.
struct Row {
    bits: u128,
    /// What this crate printed when the input file was generated.
    ours: String,
    /// What libquadmath printed for the same pattern, 37 significant
    /// digits.
    theirs: String,
    /// What libquadmath's parser made of `ours`.
    reparsed: u128,
}

fn rows() -> Vec<Row> {
    let mut ours = Vec::new();
    for line in PATTERNS.lines() {
        let mut parts = line.split_whitespace();
        let (Some(pattern), Some(text)) = (parts.next(), parts.next()) else {
            continue;
        };
        ours.push((
            u128::from_str_radix(pattern, 16).expect("a 32-digit hex pattern"),
            text.to_string(),
        ));
    }
    let mut out = Vec::new();
    for (index, line) in ORACLE.lines().enumerate() {
        let mut parts = line.split_whitespace();
        let (Some(pattern), Some(printed), Some(reparsed)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let bits = u128::from_str_radix(pattern, 16).expect("a 32-digit hex pattern");
        let (ours_bits, ours_text) = ours.get(index).expect("the two files line up").clone();
        assert_eq!(
            ours_bits, bits,
            "row {index} of the two oracle files is not the same value"
        );
        out.push(Row {
            bits,
            ours: ours_text,
            theirs: printed.to_string(),
            reparsed: u128::from_str_radix(reparsed, 16).expect("a 32-digit hex pattern"),
        });
    }
    assert_eq!(out.len(), 518, "the oracle file lost rows");
    out
}

#[test]
fn our_parser_reads_what_gcc_prints() {
    for row in rows() {
        let parsed: Float128 = row.theirs.parse().unwrap_or_else(|error| {
            panic!(
                "libquadmath printed `{}`, which this parser rejects: {error}",
                row.theirs
            )
        });
        assert_eq!(
            parsed.to_bits(),
            row.bits,
            "libquadmath printed `{}` for 0x{:032x}; this parser read it as 0x{:032x}",
            row.theirs,
            row.bits,
            parsed.to_bits()
        );
    }
}

#[test]
fn gcc_reads_what_our_printer_writes() {
    for row in rows() {
        assert_eq!(
            row.reparsed, row.bits,
            "this crate printed `{}` for 0x{:032x}; libquadmath read it back as 0x{:032x}",
            row.ours, row.bits, row.reparsed
        );
    }
}

#[test]
fn the_printer_still_writes_what_the_oracle_was_run_against() {
    // The bind: column three of the oracle file is only evidence while
    // the printer still produces the text it was measured on. If this
    // fails, the printer changed and `tests/oracle/README.md` says how
    // to measure it again — the answer is never to edit the file.
    for row in rows() {
        let value = Float128::from_bits(row.bits);
        assert_eq!(
            value.to_shortest_decimal(),
            row.ours,
            "the printer changed for 0x{:032x}; re-run the oracle rather than editing the record",
            row.bits
        );
    }
}
