// SPDX-License-Identifier: Apache-2.0
//
// The sample type of the HDDS row: roundtrip.idl of this directory, the IDL
// every app of the speed setup is generated from, through hddsgen's Rust
// backend (what `hddsgen gen rust roundtrip.idl` writes).

use std::path::PathBuf;

fn main() {
    let idl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../roundtrip.idl");
    println!("cargo:rerun-if-changed={}", idl.display());
    let source = std::fs::read_to_string(&idl).expect("roundtrip.idl");
    let mut parser = hddsgen::Parser::try_new(&source).expect("hddsgen: lexer");
    let ast = parser.parse().expect("hddsgen: parse roundtrip.idl");
    let code = hddsgen::Backend::Rust
        .generator()
        .generate(&ast)
        .expect("hddsgen: generate Rust");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("roundtrip.rs");
    std::fs::write(out, code).expect("write roundtrip.rs");
}
