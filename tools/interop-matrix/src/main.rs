// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `interop-matrix` — CLI: JSON-Input → HTML-Output, optionaler
//! Regression-Exit-Code.

#![warn(missing_docs)]
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use zerodds_interop_matrix::{parse_matrix_json, render_html};

fn print_help() {
    println!(
        "interop-matrix — render Cross-Vendor-Interop-Matrix

USAGE:
  interop-matrix INPUT.json [--out OUT.html] [--fail-on-red]

OPTIONS:
  --out PATH       HTML-Output-Pfad (default: stdout)
  --fail-on-red    exit 1 wenn die Matrix rote Zellen enthaelt
                   (fuer CI-Regressions-Gate)
"
    );
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut fail_on_red = false;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--out" => {
                if let Some(v) = argv.get(i + 1) {
                    out = Some(PathBuf::from(v));
                }
                i += 2;
            }
            "--fail-on-red" => {
                fail_on_red = true;
                i += 1;
            }
            "--help" | "-h" | "help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other if !other.starts_with("--") => {
                input = Some(PathBuf::from(other));
                i += 1;
            }
            other => {
                eprintln!("unknown flag: {other}");
                i += 1;
            }
        }
    }
    let Some(input) = input else {
        eprintln!("error: INPUT.json positional required");
        print_help();
        return ExitCode::from(2);
    };
    let json = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("read {input:?}: {e}");
            return ExitCode::from(1);
        }
    };
    let m = match parse_matrix_json(&json) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("parse: {e}");
            return ExitCode::from(1);
        }
    };
    let html = render_html(&m);
    match out {
        Some(p) => {
            if let Err(e) = fs::write(&p, &html) {
                eprintln!("write {p:?}: {e}");
                return ExitCode::from(1);
            }
            eprintln!(
                "interop-matrix: {} vendors x {} profiles → {p:?} ({} fail)",
                m.vendors.len(),
                m.profiles.len(),
                m.fail_count()
            );
        }
        None => {
            print!("{html}");
        }
    }
    if fail_on_red && m.has_failures() {
        eprintln!("interop-matrix: {} red cell(s), exiting 1", m.fail_count());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
