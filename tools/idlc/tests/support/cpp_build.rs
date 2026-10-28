// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ build the generated-header tests share: one translation unit
//! against the generated headers, the C++ wire runtime and the DDS-PSM-Cxx
//! headers.

use std::path::Path;
use std::process::{Command, Output};

/// `cxx` compiling `unit` into `program`, with `generated` on the include
/// path ahead of the runtime's and the PSM's, and the wire runtime's four
/// translation units (`runtime/cpp/Makefile`) linked in. C++17, every
/// warning an error.
pub fn compile(cxx: &str, generated: &Path, unit: &Path, program: &Path) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime = root.join("runtime/cpp");
    Command::new(cxx)
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-I{}", generated.display()))
        .arg(format!("-I{}", runtime.join("include").display()))
        .arg(format!("-I{}", root.join("crates/cpp/include").display()))
        .arg(unit)
        .arg(runtime.join("src/writer.cpp"))
        .arg(runtime.join("src/reader.cpp"))
        .arg(runtime.join("src/md5.cpp"))
        .arg(runtime.join("src/float128.cpp"))
        .arg("-o")
        .arg(program)
        .output()
        .unwrap_or_else(|e| panic!("{cxx} does not run: {e}"))
}
