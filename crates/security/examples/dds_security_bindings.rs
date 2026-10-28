// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Writes the language representations of `idl/dds_security.idl`
//! (DDS-Security 1.2 clause 11) below a directory:
//! `cargo run -p zerodds-security --example dds_security_bindings -- <dir>`.
//! `<dir>/c`, `cpp`, `cpp11`, `java`, `java-modern`.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::unreachable,
    clippy::exit,
    clippy::too_many_lines,
    clippy::needless_pass_by_value
)]

#[path = "../tests/idl_bindings/mod.rs"]
mod idl_bindings;

fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("usage: dds_security_bindings <output directory>");
        std::process::exit(2);
    };
    for (path, text) in idl_bindings::render_all() {
        let target = std::path::Path::new(&dir).join(&path);
        std::fs::create_dir_all(target.parent().unwrap()).expect("create directory");
        std::fs::write(&target, text).expect("write file");
        println!("{}", target.display());
    }
}
