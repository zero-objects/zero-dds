// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Workspace-Scanner.
//!
//! Enumerates all member crates of the workspace via `cargo_metadata`,
//! reads the safety classification per crate and collects the source
//! `.rs` files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use walkdir::WalkDir;

use crate::classification::{self, SafetyClass};

/// A crate as the scanner sees it.
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// Name from `Cargo.toml`.
    pub name: String,
    /// Crate root directory (contains `Cargo.toml`).
    pub root: PathBuf,
    /// `src/lib.rs` if present — source of the classification.
    pub lib_rs: Option<PathBuf>,
    /// Classification, if annotated in `lib.rs`.
    pub classification: Option<SafetyClass>,
    /// All `.rs` files under `src/`, `examples/` and `tests/`.
    pub source_files: Vec<PathBuf>,
}

/// Scans a workspace from `root` (path to the workspace `Cargo.toml`).
///
/// # Errors
/// If `cargo metadata` fails or an I/O error occurs reading
/// Quelldateien auftreten.
pub fn scan_workspace(root: &Path) -> Result<Vec<CrateInfo>> {
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .with_context(|| format!("cargo metadata failed for {}", root.display()))?;

    let mut crates = Vec::new();
    for pkg in metadata.workspace_packages() {
        let manifest_path: &Path = pkg.manifest_path.as_ref();
        let crate_root = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let lib_rs = crate_root.join("src").join("lib.rs");
        let lib_rs_opt = if lib_rs.exists() {
            Some(lib_rs.clone())
        } else {
            None
        };
        let classification = lib_rs_opt
            .as_ref()
            .and_then(|p| classification::read_from_file(p).ok().flatten());
        let source_files = collect_rust_files(&crate_root);

        crates.push(CrateInfo {
            name: pkg.name.clone(),
            root: crate_root,
            lib_rs: lib_rs_opt,
            classification,
            source_files,
        });
    }
    Ok(crates)
}

/// Collects all `.rs` files under `src/`, `examples/` and `tests/` of a crate.
fn collect_rust_files(crate_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for sub in ["src", "examples", "tests"] {
        let dir = crate_root.join(sub);
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir).into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if p.is_file() && p.extension().is_some_and(|e| e == "rs") {
                files.push(p.to_path_buf());
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir tmp");
        }
        std::fs::write(path, content).expect("write tmp");
    }

    #[test]
    fn collects_rust_files_from_standard_dirs() {
        let dir = TempDir::new().expect("tmp");
        let root = dir.path();
        write(&root.join("src/lib.rs"), "// noop");
        write(&root.join("src/sub/inner.rs"), "// noop");
        write(&root.join("examples/demo.rs"), "// noop");
        write(&root.join("tests/it.rs"), "// noop");
        write(&root.join("README.md"), "ignored");

        let files = collect_rust_files(root);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.strip_prefix(root).unwrap_or(p).display().to_string())
            .collect();
        assert!(names.iter().any(|n| n.ends_with("lib.rs")));
        assert!(names.iter().any(|n| n.ends_with("inner.rs")));
        assert!(names.iter().any(|n| n.ends_with("demo.rs")));
        assert!(names.iter().any(|n| n.ends_with("it.rs")));
        assert!(!names.iter().any(|n| n.ends_with("README.md")));
    }
}
