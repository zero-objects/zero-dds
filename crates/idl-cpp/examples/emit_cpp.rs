// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Minimal codegen driver: reads an IDL file and writes the
//! generated C++ header to stdout. Used to (re)generate the
//! checked-in bench headers `gen/zerodds/*.hpp`:
//!
//! ```sh
//! cargo run -p zerodds-idl-cpp --example emit_cpp -- roundtrip.idl > Roundtrip.hpp
//! ```

#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs)]

use std::path::PathBuf;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn main() {
    let path: PathBuf = std::env::args()
        .nth(1)
        .expect("usage: emit_cpp <file.idl>")
        .into();
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let ast = zerodds_idl::parse(&src, &ParserConfig::default())
        .unwrap_or_else(|e| panic!("parse {}: {e:?}", path.display()));
    let header = generate_cpp_header(&ast, &CppGenOptions::default())
        .unwrap_or_else(|e| panic!("gen {}: {e:?}", path.display()));
    print!("{header}");
}
