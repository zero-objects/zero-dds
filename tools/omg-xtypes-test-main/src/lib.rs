// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds_test_main` — the ZeroDDS client for the OMG `dds-xtypes`
//! interoperability test suite, version 2.
//!
//! Crate `zerodds-omg-xtypes-test-main`. Safety classification: **TOOLING**.
//!
//! The suite has two versions and they are two different programs.
//! `master` ships `srcCxx/ShapeType_main.cxx`, 25 compiled-in types, a
//! command line of single-dash words (`-pub`, `-type Shape1Final`) and
//! `compatibility_matrix.xlsx` — a spreadsheet a person reads. That one
//! is `tools/omg-xtypes-main`. The `feature/xml-driver-enhanced` branch,
//! which is what the suite's own release `v2.0.2025-wip` ships binaries
//! for and what `1_run_interoperability_tests.yml` runs, ships
//! `src/cxx/test_main.cxx`, ten type-definition files read at run time, a
//! command line of POSIX-shaped short options plus double-dash words, and
//! `interoperability_report.py` — a `pexpect` harness with 335 test cases
//! and a JUnit report. This crate is the client for that one.
//!
//! Everything the harness reads is a printed line, and
//! [`listeners`] carries them with the `expect` index each one satisfies.
//! Everything the suite measures is the type: [`model`] reads it from the
//! file the command line names, [`carrier`] presents it to DDS, and the
//! two steps of XTypes 1.3 §7.6.3.4.2 are decided in `crates/dcps`, which
//! is the point.

pub mod app;
pub mod carrier;
pub mod cli;
pub mod data;
pub mod listeners;
pub mod model;
