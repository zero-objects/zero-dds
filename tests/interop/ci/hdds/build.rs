// SPDX-License-Identifier: Apache-2.0
//
// The `Robot` type of the HDDS cells: ../fastdds/robot.idl, the file the
// Fast DDS and RTI cells generate from, through hddsgen's Rust backend.
// HDDS_ROBOT_IDL names the file when build.sh builds a copy of this crate
// elsewhere. The resolved HDDS version and source come from Cargo.lock,
// for `hdds_robot version`.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let idl = std::env::var_os("HDDS_ROBOT_IDL")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("../fastdds/robot.idl"));
    println!("cargo:rerun-if-env-changed=HDDS_ROBOT_IDL");
    println!("cargo:rerun-if-changed={}", idl.display());
    let source = std::fs::read_to_string(&idl).expect("robot.idl");
    let mut parser = hddsgen::Parser::try_new(&source).expect("hddsgen: lexer");
    let ast = parser.parse().expect("hddsgen: parse robot.idl");
    let code = hddsgen::Backend::Rust
        .generator()
        .generate(&ast)
        .expect("hddsgen: generate Rust");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("robot.rs");
    std::fs::write(out, code).expect("write robot.rs");

    let lock = dir.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let lock = std::fs::read_to_string(lock).unwrap_or_default();
    let hdds = lock
        .split("[[package]]")
        .find(|p| p.lines().any(|l| l.trim() == "name = \"hdds\""))
        .map(|p| {
            let field = |key: &str| {
                p.lines()
                    .find_map(|l| l.trim().strip_prefix(key))
                    .map(|v| v.trim_start_matches(" = ").trim_matches('"').to_string())
                    .unwrap_or_default()
            };
            format!("{} {}", field("version"), field("source"))
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=HDDS_ROBOT_HDDS={hdds}");
}
