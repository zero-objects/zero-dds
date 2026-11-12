// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Filesystem identity regressions for include guards.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;
use zerodds_idl_compose::{PathLoader, compose};

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("idl-once-{name}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn direct_and_transitive_aliases_preserve_dependencies() {
    let f = Fixture::new("aliases");
    f.write(
        "shared.idl",
        "#pragma once\nstruct Shared { long number; };\n",
    );
    f.write(
        "a.idl",
        "#pragma once\n#include \"shared.idl\"\nstruct A { Shared value; };\n",
    );
    for (name, source) in [
        (
            "direct.idl",
            "#include \"shared.idl\"\n#include \"./shared.idl\"\nstruct B { Shared value; };\n",
        ),
        (
            "transitive.idl",
            "#include \"a.idl\"\n#include \"./shared.idl\"\nstruct B { A first; Shared value; };\n",
        ),
    ] {
        f.write(name, source);
        let loader = PathLoader::bare();
        let result = compose(f.0.join(name).to_str().unwrap(), &loader).unwrap();
        assert!(!result.failed(), "{:?}", result.diagnostics);
        assert_eq!(
            result
                .ast
                .types()
                .filter(|(_, node)| node.name == "Shared")
                .count(),
            1
        );
        assert_eq!(
            loader
                .opened()
                .iter()
                .filter(|p| p.ends_with("shared.idl"))
                .count(),
            2
        );
    }
}

#[test]
fn different_files_with_the_same_basename_are_distinct() {
    let f = Fixture::new("distinct");
    f.write("one/shared.idl", "#pragma once\nstruct One { long n; };\n");
    f.write("two/shared.idl", "#pragma once\nstruct Two { long n; };\n");
    f.write("main.idl", "#include \"one/shared.idl\"\n#include \"two/shared.idl\"\nstruct Both { One a; Two b; };\n");
    let result = compose(f.0.join("main.idl").to_str().unwrap(), &PathLoader::bare()).unwrap();
    assert!(!result.failed(), "{:?}", result.diagnostics);
    for name in ["One", "Two"] {
        assert!(result.ast.types().any(|(_, node)| node.name == name));
    }
}

#[test]
fn root_alias_and_parent_components_are_guarded() {
    let f = Fixture::new("root");
    f.write("sub/placeholder", "");
    f.write(
        "main.idl",
        "#pragma once\n#include \"sub/../main.idl\"\nstruct Root { long n; };\n",
    );
    let result = compose(f.0.join("main.idl").to_str().unwrap(), &PathLoader::bare()).unwrap();
    assert!(!result.failed(), "{:?}", result.diagnostics);
}

#[test]
fn unguarded_alias_cycle_is_reported() {
    let f = Fixture::new("cycle");
    f.write("main.idl", "#include \"./main.idl\"\n");
    let result = compose(f.0.join("main.idl").to_str().unwrap(), &PathLoader::bare()).unwrap();
    assert!(result.failed());
    assert!(format!("{:?}", result.diagnostics).contains("includes itself"));
}

#[cfg(windows)]
#[test]
fn windows_case_variants_are_aliases() {
    let f = Fixture::new("case");
    f.write("Shared.idl", "#pragma once\nstruct Shared { long n; };\n");
    f.write(
        "main.idl",
        "#include \"Shared.idl\"\n#include \"shared.idl\"\n",
    );
    let result = compose(f.0.join("main.idl").to_str().unwrap(), &PathLoader::bare()).unwrap();
    assert!(!result.failed(), "{:?}", result.diagnostics);
}

#[cfg(unix)]
#[test]
fn symbolic_link_is_an_alias() {
    let f = Fixture::new("symlink");
    f.write("shared.idl", "#pragma once\nstruct Shared { long n; };\n");
    std::os::unix::fs::symlink("shared.idl", f.0.join("alias.idl")).unwrap();
    f.write(
        "main.idl",
        "#include \"shared.idl\"\n#include \"alias.idl\"\n",
    );
    let result = compose(f.0.join("main.idl").to_str().unwrap(), &PathLoader::bare()).unwrap();
    assert!(!result.failed(), "{:?}", result.diagnostics);
}

#[cfg(unix)]
#[test]
fn distinct_non_utf8_symlink_targets_are_not_merged() {
    use std::ffi::OsStr;
    use std::os::unix::{ffi::OsStrExt, fs::symlink};

    let f = Fixture::new("non-utf8");
    for (target, alias, name) in [
        (b"\xfe.idl", "one.idl", "One"),
        (b"\xff.idl", "two.idl", "Two"),
    ] {
        let target = OsStr::from_bytes(target);
        fs::write(
            f.0.join(target),
            format!("#pragma once\nstruct {name} {{ long n; }};\n"),
        )
        .unwrap();
        symlink(target, f.0.join(alias)).unwrap();
    }
    // Both distinct targets have the same lossy display name. Include each
    // again through another spelling to check both distinction and aliasing.
    f.write(
        "main.idl",
        "#include \"one.idl\"\n#include \"two.idl\"\n\
         #include \"./one.idl\"\n#include \"./two.idl\"\n\
         struct Both { One a; Two b; };\n",
    );
    let loader = PathLoader::bare();
    let result = compose(f.0.join("main.idl").to_str().unwrap(), &loader).unwrap();
    assert!(!result.failed(), "{:?}", result.diagnostics);
    for name in ["One", "Two"] {
        assert_eq!(
            result
                .ast
                .types()
                .filter(|(_, node)| node.name == name)
                .count(),
            1
        );
    }
    assert_eq!(loader.opened().len(), 4);
}
