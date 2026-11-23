// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-coap-bridge` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses (`src/daemon/representation.rs`, feature `daemon`):
//! `compose` of a topic's `idl_file` with a `PathLoader` over the file's own
//! directory, so that its `#include`s resolve there; `compose_source_with`
//! for IDL text; `Composed::failed`, the diagnostics' `message`, and the
//! model in `ast`; `Composed::expanded`'s `text`, the IDL `GET
//! /schema/<slug>` returns (spec §5.2). It relies on: an include beside the
//! file resolves and its declarations are in the expanded text; a file that
//! does not compile is `failed` with a message; a missing file is an error.
//!
//! Oracle: IDL 4.2 §7.3 (`#include`), §7.4 (the grammar).
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[test]
fn idl_7_3_a_file_compiles_with_its_includes_from_its_own_directory() {
    let dir =
        std::env::temp_dir().join(format!("zerodds-coap-contract-idl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("base.idl"),
        "module m { struct Base { long a; }; };\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("top.idl"),
        "#include \"base.idl\"\nmodule m { struct Top { Base b; }; };\n",
    )
    .unwrap();
    let loader = zerodds_idl_compose::PathLoader::new(vec![dir.clone()]);
    let top = dir.join("top.idl");
    let composed = zerodds_idl_compose::compose(top.to_str().unwrap(), &loader).unwrap();
    assert!(!composed.failed());
    let expanded = &composed.expanded.as_ref().expect("one unit").text;
    assert!(expanded.contains("Base"), "{expanded}");
    assert!(expanded.contains("Top"), "{expanded}");
    assert!(!expanded.contains("#include"), "{expanded}");

    std::fs::write(dir.join("broken.idl"), "struct Broken { long ;").unwrap();
    let broken = dir.join("broken.idl");
    let composed = zerodds_idl_compose::compose(broken.to_str().unwrap(), &loader).unwrap();
    assert!(composed.failed());
    assert!(composed.diagnostics.iter().any(|d| !d.message.is_empty()));

    let missing = dir.join("missing.idl");
    assert!(zerodds_idl_compose::compose(missing.to_str().unwrap(), &loader).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
