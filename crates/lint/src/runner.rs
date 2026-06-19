// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Orchestrates scanner + lints + reporter.
//!
//! Entry point: [`run`].

use std::path::Path;

use anyhow::{Context, Result};

use crate::diagnostic::{Diagnostic, Severity};
use crate::lints::{FileLint, FileLintContext, default_crate_lints, default_file_lints};
use crate::scanner::{self, CrateInfo};

/// Configuration for a lint run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Workspace root (contains the workspace `Cargo.toml`).
    pub workspace_root: std::path::PathBuf,
    /// If `true`, warnings also abort the run.
    pub fail_on_warning: bool,
}

/// Result of a lint run.
#[derive(Debug, Default)]
pub struct RunReport {
    /// All findings (errors + warnings).
    pub diagnostics: Vec<Diagnostic>,
    /// Number of scanned crates.
    pub crates_scanned: usize,
    /// Number of scanned files.
    pub files_scanned: usize,
}

impl RunReport {
    /// Number of errors.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// Number of warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }
}

/// Runs all default lints on the workspace.
///
/// # Errors
/// Errors reading individual source files are noted as diagnostics in the report;
/// only fundamental I/O errors (e.g. a missing workspace manifest)
/// abort the run as `Err`.
pub fn run(cfg: &RunConfig) -> Result<RunReport> {
    let crates = scanner::scan_workspace(&cfg.workspace_root)?;
    let file_lints = default_file_lints();
    let crate_lints = default_crate_lints();
    let mut report = RunReport {
        crates_scanned: crates.len(),
        ..RunReport::default()
    };

    for krate in &crates {
        for lint in &crate_lints {
            report.diagnostics.extend(lint.check(krate));
        }
        for file in &krate.source_files {
            report.files_scanned += 1;
            report
                .diagnostics
                .extend(check_file(file, krate, file_lints.as_slice()));
        }
    }
    Ok(report)
}

fn check_file(file: &Path, krate: &CrateInfo, lints: &[Box<dyn FileLint>]) -> Vec<Diagnostic> {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            return vec![Diagnostic::error(
                file,
                0,
                0,
                "dds_io_error",
                format!("could not read file: {e}"),
            )];
        }
    };
    let ast = match syn::parse_file(&source) {
        Ok(a) => a,
        Err(e) => {
            let span = e.span().start();
            return vec![Diagnostic::error(
                file,
                span.line,
                span.column.saturating_add(1),
                "dds_parse_error",
                format!("syn parse failed: {e}"),
            )];
        }
    };
    let ctx = FileLintContext {
        file,
        source: &source,
        ast: &ast,
        crate_class: krate.classification,
        crate_name: &krate.name,
    };
    let mut out = Vec::new();
    for lint in lints {
        out.extend(lint.check(&ctx));
    }
    out
}

/// Entry point for the CLI: determines the workspace root via
/// `cargo locate-project` if needed.
///
/// # Errors
/// If `cargo` is not reachable or no workspace is found.
pub fn locate_workspace_root(start: &Path) -> Result<std::path::PathBuf> {
    // Direct path: is there a `Cargo.toml` with `[workspace]` in the start path?
    let mut cur = start.to_path_buf();
    loop {
        let manifest = cur.join("Cargo.toml");
        if manifest.exists() {
            let content = std::fs::read_to_string(&manifest)
                .with_context(|| format!("read {}", manifest.display()))?;
            if content.contains("[workspace]") {
                return Ok(cur);
            }
        }
        if !cur.pop() {
            anyhow::bail!("no workspace Cargo.toml found from {}", start.display());
        }
    }
}
