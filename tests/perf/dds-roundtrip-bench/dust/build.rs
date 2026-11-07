// SPDX-License-Identifier: Apache-2.0
//
// The sample type of the Dust DDS row: roundtrip.idl of this directory, the
// IDL every app of the speed setup is generated from, through dust_dds_gen
// (the IDL code generator of Dust DDS, `dust_dds_gen::compile_idl`).

use std::path::PathBuf;

fn main() {
    let idl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../roundtrip.idl");
    println!("cargo:rerun-if-changed={}", idl.display());
    let code = dust_dds_gen::compile_idl(&idl).expect("dust_dds_gen: roundtrip.idl");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("roundtrip.rs");
    std::fs::write(out, code).expect("write roundtrip.rs");
}
