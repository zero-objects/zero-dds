// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Include-resolver path fidelity: a `-I`-found file that in turn uses a
//! relative `#include` must resolve against its *real* directory, and the
//! reported dependency list must contain the actual on-disk (canonical)
//! paths — not the raw `#include` spellings.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;

use zerodds_idl_compose::{ComposeOptions, compose};

#[test]
fn dash_i_found_file_resolves_its_own_relative_include_and_reports_real_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    // Layout:
    //   <root>/main.idl                 -> #include "pkg/a.idl"  (found via -I)
    //   <root>/include_dir/pkg/a.idl    -> #include "b.idl"      (relative to a.idl)
    //   <root>/include_dir/pkg/b.idl
    let include_dir = root.join("include_dir");
    let pkg = include_dir.join("pkg");
    fs::create_dir_all(&pkg).expect("mkdir pkg");

    let main_idl = root.join("main.idl");
    fs::write(
        &main_idl,
        "#include \"pkg/a.idl\"\nstruct Main { long m; };\n",
    )
    .expect("write main");
    let a_idl = pkg.join("a.idl");
    fs::write(&a_idl, "#include \"b.idl\"\nstruct A { long a; };\n").expect("write a");
    let b_idl = pkg.join("b.idl");
    fs::write(&b_idl, "struct B { long b; };\n").expect("write b");

    let opts = ComposeOptions {
        include_dirs: vec![include_dir.clone()],
        ..ComposeOptions::default()
    };

    // Generation must succeed: a.idl's relative `#include "b.idl"` only
    // resolves if the base directory is a.idl's *real* location (found via
    // -I), not the raw string "pkg/a.idl".
    let composed = compose(&main_idl, &opts).expect("compose should succeed");

    // main.idl + a.idl + b.idl each contribute one top-level struct; if the
    // relative `#include "b.idl"` had failed, compose() would have errored
    // above rather than reaching here.
    assert_eq!(
        composed.ast.definitions.len(),
        3,
        "expected Main + A + B as top-level definitions, got {}",
        composed.ast.definitions.len()
    );

    // Dependency list must contain the REAL canonical paths of the includes,
    // not the raw `#include` spellings ("pkg/a.idl" / "b.idl").
    let want_a = fs::canonicalize(&a_idl)
        .expect("canonicalize a")
        .to_string_lossy()
        .into_owned();
    let want_b = fs::canonicalize(&b_idl)
        .expect("canonicalize b")
        .to_string_lossy()
        .into_owned();

    assert!(
        composed.dependency_files.contains(&want_a),
        "dependency_files must contain real path {want_a:?}; got {:?}",
        composed.dependency_files
    );
    assert!(
        composed.dependency_files.contains(&want_b),
        "dependency_files must contain real path {want_b:?}; got {:?}",
        composed.dependency_files
    );

    // The raw spellings must NOT appear as standalone dependency entries.
    assert!(
        !composed.dependency_files.iter().any(|d| d == "pkg/a.idl"),
        "raw include string 'pkg/a.idl' leaked into deps: {:?}",
        composed.dependency_files
    );
    assert!(
        !composed.dependency_files.iter().any(|d| d == "b.idl"),
        "raw include string 'b.idl' leaked into deps: {:?}",
        composed.dependency_files
    );
}
