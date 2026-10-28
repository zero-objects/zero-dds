// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Cache publication is tested without any vendor or language toolchain.
#![cfg(unix)]
#![allow(clippy::expect_used)]

#[test]
fn runtime_cache_publication_preserves_complete_entries() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/wireprobe/test-runtime-cache.sh");
    let output = std::process::Command::new("bash")
        .arg(script)
        .output()
        .expect("run the runtime cache regression script");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
