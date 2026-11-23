// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex B, "IDL for DDS-DLRL Ports and Connectors": the IDL of the annex
//! (`idl/ccm_dds_dlrl.idl`, with `dds_rtf2_dlrl.idl` over the generic DLRL
//! IDL of `crates/dlrl/idl`) through a foreign IDL compiler, omniORB's
//! omniidl with its C++ back end.
//!
//! Needs omniidl (Debian package `omniidl`); the weekly job
//! `integration-bridges-vendor-corba` provisions omniORB and runs it.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

const ANNEX_IDL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/idl");
const DLRL_IDL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../dlrl/idl");

/// A fresh directory under the test target directory.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// omniidl -bcxx on `file` with `include` on the include path, the output
/// in `dir`: its exit status and its error output.
fn omniidl(dir: &Path, include: &[&str], file: &Path) -> (bool, String) {
    let mut command = Command::new("omniidl");
    command.args(["-bcxx", "-C"]).arg(dir);
    for path in include {
        command.arg(format!("-I{path}"));
    }
    let out = command.arg(file).output().expect("omniidl runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The file of the annex compiles with omniidl's C++ back end, which
/// writes the class of the local interface `CCM_DDS::CacheOperation` with
/// its attributes and operations over the types of the DLRL IDL.
#[test]
#[ignore = "needs omniidl (Linux test host, weekly job integration-bridges-vendor-corba)"]
fn dds4ccm_annex_b_omniidl_compiles_the_idl_of_the_annex() {
    let dir = workdir("dds4ccm_annex_b_omniidl");
    let file = Path::new(ANNEX_IDL).join("ccm_dds_dlrl.idl");
    let (ok, stderr) = omniidl(&dir, &[DLRL_IDL], &file);
    assert!(ok, "omniidl refused the IDL of Annex B:\n{stderr}");
    let header = std::fs::read_to_string(dir.join("ccm_dds_dlrl.hh")).unwrap();
    assert!(
        header.contains("class CacheOperation"),
        "no class CacheOperation in the C++ header"
    );
    for member in [
        "cache_usage",
        "objects",
        "updates_enabled",
        "homes",
        "sub_accesses",
        "listeners",
        "refresh",
        "attach_listener",
        "detach_listener",
        "enable_updates",
        "disable_updates",
        "create_access",
        "delete_access",
    ] {
        assert!(
            header.contains(&format!(" {member}(")),
            "{member} not in the C++ header"
        );
    }
    // The signatures name the types of module DDS of the generic DLRL IDL.
    for type_ in [
        "DDS::CacheUsage",
        "DDS::ObjectRootSeq",
        "DDS::ObjectHomeSeq",
        "DDS::CacheAccessSeq",
        "DDS::CacheListenerSeq",
        "DDS::CacheAccess",
        "DDS::CacheListener",
    ] {
        assert!(header.contains(type_), "{type_} not in the C++ header");
    }
}

/// Without the generic DLRL IDL on the include path, omniidl does not find
/// `dds_dlrl.idl` and refuses the file: the annex needs the DLRL IDL of
/// DDS, as its `#include "dds_rtf2_dlrl.idl"` says.
#[test]
#[ignore = "needs omniidl (Linux test host, weekly job integration-bridges-vendor-corba)"]
fn dds4ccm_annex_b_omniidl_refuses_the_annex_without_the_dlrl_idl() {
    let dir = workdir("dds4ccm_annex_b_omniidl_alone");
    let file = Path::new(ANNEX_IDL).join("ccm_dds_dlrl.idl");
    let (ok, stderr) = omniidl(&dir, &[], &file);
    assert!(!ok, "omniidl compiled Annex B without the DLRL IDL");
    assert!(
        stderr.contains("dds_dlrl.idl"),
        "omniidl's error does not name dds_dlrl.idl:\n{stderr}"
    );
}
