// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the tests that run a C++ program against `libzerodds` share.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The file name of the library the programs link.
#[cfg(target_os = "macos")]
pub const LIBRARY: &str = "libzerodds.dylib";
/// The file name of the library the programs link.
#[cfg(not(target_os = "macos"))]
pub const LIBRARY: &str = "libzerodds.so";

/// The repository root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The directory with `libzerodds` (`zerodds-c-api`, debug).
///
/// `tests/integration/rpc_cpp.sh` builds it into
/// `$CARGO_TARGET_DIR/cpp-rpc` before the job `integration-sdk` runs these
/// tests; a library found there is taken as it is, and a test builds only
/// where there is none (a developer who changed `zerodds-c-api` runs the
/// script again). Built inside the test, the cargo call took the test past
/// its 25 s in the merge lane (pipeline 8642, job 227953).
pub fn library_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| root().join("target"), PathBuf::from)
            .join("cpp-rpc");
        let built = target.join("debug");
        if !built.join(LIBRARY).is_file() {
            let output = Command::new(env!("CARGO"))
                .args(["build", "-p", "zerodds-c-api", "--target-dir"])
                .arg(&target)
                .current_dir(root())
                .output()
                .unwrap_or_else(|e| panic!("cargo build zerodds-c-api: {e}"));
            assert!(
                output.status.success(),
                "cargo build zerodds-c-api failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(built.join(LIBRARY).is_file(), "{LIBRARY} not in {built:?}");
        built
    })
}
