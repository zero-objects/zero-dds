// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-idl-compose` (`src/idl.rs`, `compile`,
//! with `daemon`): `compose(path, &PathLoader::new(search_roots))` over an
//! operator's IDL file; an unreadable path is the `Err` of `compose`, naming
//! the path; `Composed::failed()` with the error-severity diagnostics refuses
//! the file; `Composed::ast` is the model the daemon builds its
//! `SerializedTypeSupport` from.
//!
//! Oracle: IDL 4.2 §7.3 (the preprocessor is the C one: a quoted
//! `#include` is searched beside the including file, then in the search
//! roots, ISO C §6.10.2) and §7.4.1.4.4.4.4 (a member whose type names
//! nothing is not a valid declaration).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_idl_compose::{PathLoader, compose};

fn temporary(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("opcua-contract-idl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn idl_7_3_an_include_resolves_beside_the_file_and_then_in_the_search_roots() {
    let dir = temporary("roots");
    let roots = dir.join("roots");
    std::fs::create_dir_all(&roots).unwrap();
    std::fs::write(roots.join("units.idl"), "enum Unit { METRE, SECOND };").unwrap();
    std::fs::write(dir.join("beside.idl"), "struct Header { long id; };").unwrap();
    let main = dir.join("main.idl");
    std::fs::write(
        &main,
        "#include \"beside.idl\"\n#include \"units.idl\"\nstruct Reading { Header h; Unit u; };",
    )
    .unwrap();
    let composed = compose(main.to_str().unwrap(), &PathLoader::new(vec![roots])).unwrap();
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    for name in ["Header", "Unit", "Reading"] {
        assert!(composed.ast.types().any(|(_, n)| n.name == name), "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_path_is_an_error_that_names_it() {
    let Err(error) = compose("/nonexistent/operator.idl", &PathLoader::bare()) else {
        panic!("a path that does not exist must not compose");
    };
    assert!(error.contains("operator.idl"), "{error}");
}

#[test]
fn idl_7_4_1_4_4_4_4_a_member_whose_type_names_nothing_fails_the_file() {
    let dir = temporary("broken");
    let main = dir.join("broken.idl");
    std::fs::write(&main, "struct Reading { Missing m; };").unwrap();
    let composed = compose(main.to_str().unwrap(), &PathLoader::bare()).unwrap();
    assert!(composed.failed());
    let _ = std::fs::remove_dir_all(&dir);
}
