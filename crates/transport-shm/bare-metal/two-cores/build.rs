// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Links with `link.x`, by its absolute path.

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    println!("cargo:rustc-link-arg=-T{dir}/link.x");
    println!("cargo:rerun-if-changed=link.x");
}
