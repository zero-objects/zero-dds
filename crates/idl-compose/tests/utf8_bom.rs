// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A leading UTF-8 BOM (U+FEFF) — some editors prepend it — is not part of the
//! IDL and must be stripped before preprocessing/parsing, otherwise it is an
//! illegal leading token. Covers the main compose entry and `#include`d files.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;

use zerodds_idl_compose::{ComposeOptions, compose};

#[test]
fn leading_bom_is_stripped_from_top_level_idl() {
    let dir = std::env::temp_dir().join("zdw_bom_top");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    let idl = dir.join("m.idl");
    // U+FEFF then a normal module.
    fs::write(&idl, "\u{feff}module m { struct S { long x; }; };").expect("write");

    let out = compose(&idl, &ComposeOptions::default());
    assert!(
        out.is_ok(),
        "BOM-prefixed IDL must compose: {:?}",
        out.err()
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn leading_bom_is_stripped_from_included_idl() {
    let dir = std::env::temp_dir().join("zdw_bom_inc");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    // The included file carries the BOM.
    fs::write(
        dir.join("common.idl"),
        "\u{feff}module common { struct C { long y; }; };",
    )
    .expect("write common");
    let top = dir.join("top.idl");
    fs::write(
        &top,
        "#include \"common.idl\"\nmodule top { struct T { common::C c; }; };",
    )
    .expect("write top");

    let opts = ComposeOptions {
        include_dirs: vec![dir.clone()],
        ..ComposeOptions::default()
    };
    let out = compose(&top, &opts);
    assert!(
        out.is_ok(),
        "IDL with a BOM-prefixed #include must compose: {:?}",
        out.err()
    );
    let _ = fs::remove_dir_all(&dir);
}
