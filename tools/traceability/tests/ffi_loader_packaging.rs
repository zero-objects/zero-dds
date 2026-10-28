//! `zerodds-ffi-loader-1.0` §6 — the library, the header and the
//! pkg-config descriptor checked against the paths the clause names.
//!
//! §6 is a statement about files: which artefact lands under which
//! prefix on which platform. Each case below opens the packaging file
//! the clause points at and asserts the path it installs to, rather
//! than asserting that the file exists.
//!
//! The Windows half of the clause (`%PROGRAMFILES%\ZeroDDS\include\
//! zerodds.h`, `bin\zerodds.dll`, `lib\zerodds.lib`) has no case here
//! because the MSI does not ship it — see the `partial` status of the
//! §6 item and the `open` item beside it.
//!
//! Paths are resolved from the repository root, two levels above this
//! crate's manifest (`tools/traceability`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root above tools/traceability")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// §6: the C header the loaders compile against is the one the C-API
/// crate ships, and it declares the six entry points the §4.1 sample
/// lifecycle walks through.
#[test]
fn the_shipped_header_declares_the_lifecycle_entry_points() {
    let header = read("crates/zerodds-c-api/include/zerodds.h");
    for symbol in [
        "zerodds_runtime_create",
        "zerodds_writer_create",
        "zerodds_reader_create",
        "zerodds_writer_write",
        "zerodds_reader_take",
        "zerodds_runtime_destroy",
        "zerodds_abi_revision",
    ] {
        assert!(
            header.contains(symbol),
            "zerodds.h does not declare {symbol}"
        );
    }
    // The crate builds the shared library the clause names.
    let manifest = read("crates/zerodds-c-api/Cargo.toml");
    assert!(
        manifest.contains("cdylib"),
        "the C-API crate must build a cdylib for libzerodds.{{so,dylib,dll}}"
    );
}

/// §6 Linux: `libzerodds.so` with its SONAME chain goes into the core
/// package, and the pkg-config descriptor resolves the include and
/// library prefixes the clause names.
#[test]
fn the_linux_packages_install_the_library_and_resolve_the_prefixes() {
    let assemble = read("packaging/linux/deb/assemble-debs.sh");
    for line in [
        "target/$TARGET/release/libzerodds.so",
        "$stage/usr/lib/libzerodds.so.1.0.0",
        "libzerodds.so.1.0.0 \"$stage/usr/lib/libzerodds.so.1\"",
        "libzerodds.so.1     \"$stage/usr/lib/libzerodds.so\"",
    ] {
        assert!(
            assemble.contains(line),
            "assemble-debs.sh does not install {line:?}"
        );
    }

    let pc = read("packaging/linux/rpm/zerodds.pc");
    for field in [
        "prefix=/usr",
        "includedir=${prefix}/include",
        "libdir=${exec_prefix}/lib",
        "-lzerodds",
    ] {
        assert!(pc.contains(field), "zerodds.pc does not carry {field:?}");
    }
    assert!(
        pc.contains("-I${includedir}"),
        "zerodds.pc must hand the header prefix to the compiler"
    );
}

/// §6 macOS: both the Homebrew formula and the `.pkg` build script put
/// the header and the dylib under the `/usr/local` prefix.
#[test]
fn the_macos_packages_install_the_header_and_the_dylib() {
    let formula = read("packaging/macos/homebrew/zerodds.rb");
    assert!(
        formula.contains("lib.install Dir[\"lib/libzerodds.*\"]"),
        "the formula does not install the dylib"
    );
    assert!(
        formula.contains("include.install \"include/zerodds.h\""),
        "the formula does not install the header"
    );

    let pkg = read("packaging/macos/pkg/build-pkg.sh");
    assert!(
        pkg.contains("libzerodds.dylib \"$ROOT/usr/local/lib/\""),
        "build-pkg.sh does not stage the dylib under /usr/local/lib"
    );
    assert!(
        pkg.contains("include/zerodds.h \"$ROOT/usr/local/include/\""),
        "build-pkg.sh does not stage the header under /usr/local/include"
    );
}

/// §6 per-language packages: the one the clause names that this tree
/// actually carries a distribution manifest for is the Python wheel.
/// The Java module has a Maven manifest, but under different
/// coordinates than the clause names — the `open` item beside §6
/// records that.
#[test]
fn the_python_wheel_carries_the_distribution_name_of_the_clause() {
    let pyproject = read("crates/py/pyproject.toml");
    assert!(
        pyproject.contains("name = \"zerodds\""),
        "the wheel is not named `zerodds`"
    );
    assert!(
        pyproject.contains("module-name = \"zerodds._core\""),
        "the wheel does not ship the native module inside the `zerodds` package"
    );

    // The Maven module exists, but with the coordinates the pom declares,
    // not the ones §6 names. Asserting what is there keeps the divergence
    // visible instead of implying the clause is met.
    let pom = read("crates/java-omgdds/java/pom.xml");
    assert!(
        pom.contains("<groupId>org.zerodds</groupId>"),
        "the Maven group of the module changed"
    );
    assert!(
        pom.contains("<artifactId>omgdds</artifactId>"),
        "the Maven artifact of the module changed"
    );
}
