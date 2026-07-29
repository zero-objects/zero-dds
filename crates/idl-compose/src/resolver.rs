// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Filesystem `#include` resolver, extracted from `zerodds-idlc` so
//! non-CLI callers (`zerodds-build`, CMake) resolve `#include` the same
//! way the CLI does: quoted includes relative to the requesting file
//! first, then each configured include directory in order.

use std::path::{Path, PathBuf};

use zerodds_idl::preprocessor::{Include, ResolveError, Resolver};

/// Resolves `#include` directives against the filesystem: quoted includes
/// (`#include "x.idl"`) are tried relative to the requesting file first,
/// then every directory in `include_dirs` is tried in order (matching both
/// quoted and system-style `#include <x.idl>`).
#[derive(Debug, Clone, Default)]
pub struct FsResolver {
    /// Search path for `#include`, in priority order.
    pub include_dirs: Vec<PathBuf>,
}

impl FsResolver {
    /// New resolver with the given include search path.
    #[must_use]
    pub fn new(include_dirs: Vec<PathBuf>) -> Self {
        Self { include_dirs }
    }
}

impl Resolver for FsResolver {
    fn resolve(&self, requesting_file: &str, include: &Include) -> Result<String, ResolveError> {
        let name = include.path();
        let mut candidates: Vec<PathBuf> = Vec::new();
        // Quoted includes: relative to the including file first.
        if matches!(include, Include::Quoted(_)) {
            if let Some(parent) = Path::new(requesting_file).parent() {
                candidates.push(parent.join(name));
            }
        }
        for dir in &self.include_dirs {
            candidates.push(dir.join(name));
        }
        for cand in &candidates {
            if let Ok(text) = std::fs::read_to_string(cand) {
                return Ok(text);
            }
        }
        Err(ResolveError {
            requested: name.to_string(),
            message: format!(
                "include '{name}' not found (searched {} path(s); use include_dir())",
                candidates.len()
            ),
        })
    }
}
