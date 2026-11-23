// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.2.1.2: the generic DLRL IDL of `crates/dlrl/idl/dds_dlrl.idl`
//! and the implied IDL `generate_implied_idl` writes, through a foreign
//! IDL compiler: omniORB's omniidl with its C++ back end. And the
//! listeners of the printed generic IDL, which the implied IDL redefines,
//! refused by omniidl as well.
//!
//! Needs omniidl (Debian package `omniidl`); the weekly job
//! `integration-bridges-vendor-corba` provisions omniORB and runs it.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_dlrl_codegen::generate_implied_idl;

const DLRL_IDL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../dlrl/idl");

/// A fresh directory under the test target directory.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// omniidl -bcxx on `file`, the generic IDL's directory on the include
/// path; its exit status and its error output.
fn omniidl(dir: &Path, file: &Path) -> (bool, String) {
    let out = Command::new("omniidl")
        .args(["-bcxx", "-C"])
        .arg(dir)
        .arg(format!("-I{DLRL_IDL}"))
        .arg(file)
        .output()
        .expect("omniidl runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The generic IDL and the implied IDL of Track, Track3D, Radar (module
/// demo) and Foo compile with omniidl's C++ back end, which writes the
/// headers of the local interfaces and valuetypes.
#[test]
#[ignore = "needs omniidl (Linux test host, weekly job integration-bridges-vendor-corba)"]
fn dlrl_8_2_1_2_omniidl_compiles_generic_and_implied_idl() {
    let dir = workdir("dlrl_omniidl_implied");
    let mut source = String::from(
        "#include \"dds_dlrl.idl\"
module demo {
    valuetype Radar : DDS::ObjectRoot { public string name; };
    valuetype Track : DDS::ObjectRoot { public double x; public double y; };
    valuetype Track3D : Track { public double z; };
};
valuetype Foo : DDS::ObjectRoot { public long n; };
",
    );
    for class in ["demo::Radar", "demo::Track", "demo::Track3D", "Foo"] {
        source.push_str(&generate_implied_idl(class));
    }
    let file = dir.join("track_dlrl.idl");
    std::fs::write(&file, source).unwrap();
    let (ok, stderr) = omniidl(&dir, &file);
    assert!(ok, "omniidl refused the generic and implied IDL:\n{stderr}");
    let header = std::fs::read_to_string(dir.join("track_dlrl.hh")).unwrap();
    for name in [
        "TrackHome",
        "Track3DListener",
        "FooIntMap",
        "RadarSelection",
    ] {
        assert!(header.contains(name), "{name} not in the C++ header");
    }
}

/// The printed generic IDL declares `on_object_created` in
/// `DDS::ObjectListener`; the implied `FooListener` redefines it, which
/// omniidl refuses as IDL 4.2 §7.4.3 says.
#[test]
#[ignore = "needs omniidl (Linux test host, weekly job integration-bridges-vendor-corba)"]
fn dlrl_8_2_1_2_omniidl_refuses_the_printed_listener_redefinition() {
    let dir = workdir("dlrl_omniidl_printed");
    let file = dir.join("printed.idl");
    std::fs::write(
        &file,
        "module DDS {
    valuetype ObjectRoot { public long n; };
    local interface ObjectListener {
        boolean on_object_created(in ObjectRoot the_object);
    };
};
valuetype Foo : DDS::ObjectRoot { public long m; };
local interface FooListener : DDS::ObjectListener {
    void on_object_created(in Foo the_object);
};
",
    )
    .unwrap();
    let (ok, stderr) = omniidl(&dir, &file);
    assert!(!ok, "omniidl accepted the redefinition");
    assert!(
        stderr.contains("on_object_created"),
        "omniidl's error does not name the operation:\n{stderr}"
    );
}
