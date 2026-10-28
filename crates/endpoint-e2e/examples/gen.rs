// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Materializes the `Ping`/`Pong` types plus their XCDR2 codec in one endpoint
//! language, through the same `codegen::source` every test in this crate takes.
//! Run:
//!   cargo run -p zerodds-endpoint-e2e --example gen -- go > types.go
//!
//! Replaces the former `gen-go` example, which called the removed
//! `zerodds_idl_go::generate_go_module`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use zerodds_endpoint_e2e::codegen::{Language, source};

fn main() {
    let Some(want) = std::env::args().nth(1) else {
        eprintln!("usage: gen <language>");
        std::process::exit(2);
    };
    let Some(language) = Language::ALL
        .into_iter()
        .find(|l| format!("{l:?}").eq_ignore_ascii_case(&want))
    else {
        eprintln!("unknown language {want:?}");
        eprintln!(
            "known: {}",
            Language::ALL
                .iter()
                .map(|l| format!("{l:?}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        std::process::exit(2);
    };
    print!("{}", source(language));
}
