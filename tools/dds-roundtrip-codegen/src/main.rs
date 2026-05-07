// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Generiert C++17-Header aus `tests/perf/dds-roundtrip-bench/roundtrip.idl`
//! via `zerodds-idl-cpp` library-API. Output zu stdout oder `--out FILE`.
use std::io::Write;
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{generate_cpp_header, CppGenOptions};

fn main() -> std::io::Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--out" if i + 1 < argv.len() => { output = Some(argv[i+1].clone()); i += 2; }
            other if !other.starts_with("--") => { input = Some(other.to_string()); i += 1; }
            _ => { eprintln!("usage: dds-roundtrip-codegen <file.idl> [--out OUT]"); std::process::exit(2); }
        }
    }
    let path = input.expect("input idl file required");
    let src = std::fs::read_to_string(&path)?;
    let ast = zerodds_idl::parse(&src, &ParserConfig::default())
        .unwrap_or_else(|e| { eprintln!("parse error: {e:?}"); std::process::exit(1); });
    let opts = CppGenOptions::default();
    let cpp = generate_cpp_header(&ast, &opts)
        .unwrap_or_else(|e| { eprintln!("emit error: {e:?}"); std::process::exit(1); });
    match output {
        Some(p) => std::fs::write(&p, &cpp)?,
        None => { std::io::stdout().write_all(cpp.as_bytes())?; }
    }
    Ok(())
}
